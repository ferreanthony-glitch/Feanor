//! Synthèse sur la carte graphique (étape d), avec wgpu. Mêmes entrées, mêmes réglages et même
//! résultat (`Output`) que `synthesize` sur le processeur ; l'algorithme est celui de la version
//! CUDA d'ebsynth (noyaux dans `synthese.wgsl`) :
//! - PatchMatch « de Jacobi » : tous les pixels à la fois essaient leurs voisins à 4, 2 puis 1
//!   pixel, puis des tirages au hasard dans des carrés de rayon 1, 2, 4… ;
//! - l'occupation du style (uniformité) est tenue par des additions atomiques ;
//! - les pixels dont le style ne bouge plus (`Params::stop_threshold`) ne sont plus cherchés.
//!
//! Toute la pyramide reste sur la carte : on n'envoie que les images entières et on ne relit que
//! le résultat.

use std::sync::mpsc;

use wgpu::util::DeviceExt;

use crate::blend::{BlendParams, ErrorMap};
use crate::patchmatch::level_size;
use crate::{check, Error, GuidePair, Image, Output, Params, Synthesizer};

/// Réglages d'un passage d'un noyau (`Params` dans synthese.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct Pass {
    tw: u32,
    th: u32,
    sw: u32,
    sh: u32,
    pw: u32,
    ph: u32,
    words: u32,
    style_words: u32,
    patch: u32,
    radius: i32,
    seed: u32,
    stop: u32,
    lambda: f32,
    omega_best: f32,
    pad: [u32; 2],
}

/// Les noyaux, dans l'ordre de `KERNELS`.
#[derive(Clone, Copy)]
enum Kernel {
    Resample,
    InitRandom,
    Upscale,
    OmegaClear,
    OmegaInit,
    MaskAll,
    Vote,
    EvalError,
    Propagate,
    RandomSearch,
    EvalMask,
    Dilate,
}

const KERNELS: [&str; 12] = [
    "resample",
    "init_random",
    "upscale",
    "omega_clear",
    "omega_init",
    "mask_all",
    "vote",
    "eval_error",
    "propagate",
    "random_search",
    "eval_mask",
    "dilate",
];

/// Groupes de liaisons d'un niveau.
const MAIN_A: usize = 0; // correspondances courantes dans `a`
const MAIN_B: usize = 1; // dans `b`
const RESAMPLE_SOURCE: usize = 2;
const RESAMPLE_TARGET: usize = 3;
const UPSCALE: usize = 4;

/// Au plus tant de passages par envoi à la carte (Windows relance le pilote si un envoi dure plus
/// de deux secondes).
const OPS_PER_SUBMIT: usize = 64;

/// Les noyaux de la synthèse, prêts sur une carte graphique.
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    pipelines: Vec<wgpu::ComputePipeline>,
    /// Résolution du mélange (blend.wgsl).
    relax_layout: wgpu::BindGroupLayout,
    relax: wgpu::ComputePipeline,
    name: String,
}

/// Réglages d'une demi-passe de relaxation (`Relax` dans blend.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct Relax {
    w: u32,
    h: u32,
    parity: u32,
    pad0: u32,
    lambda: f32,
    omega: f32,
    pad1: [f32; 2],
}

impl Gpu {
    /// Ouvre la carte graphique la plus puissante.
    pub fn new() -> Result<Gpu, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .ok_or("aucune carte graphique")?;
        let info = adapter.get_info();
        // Les limites de la carte : les tampons d'une image 4K dépassent celles par défaut.
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("synthese"),
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|e| e.to_string())?;
        Ok(Gpu::with_device(device, queue, format!("{} ({:?})", info.name, info.backend)))
    }

    /// Avec un périphérique déjà ouvert (celui de Pelure par exemple). Les noyaux n'utilisent que
    /// 8 tampons de stockage, la limite par défaut.
    pub fn with_device(device: wgpu::Device, queue: wgpu::Queue, name: String) -> Gpu {
        let storage = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let uniform = |binding: u32, dynamic: bool, size: u64| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: dynamic,
                min_binding_size: wgpu::BufferSize::new(size),
            },
            count: None,
        };
        let mut entries = vec![uniform(0, true, std::mem::size_of::<Pass>() as u64), uniform(1, false, 256), storage(2, true)];
        entries.extend((3..10).map(|b| storage(b, false)));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("synthese"), entries: &entries });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("synthese"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("synthese"),
            source: wgpu::ShaderSource::Wgsl(include_str!("synthese.wgsl").into()),
        });
        let pipelines = KERNELS
            .iter()
            .map(|entry| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&pipeline_layout),
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })
            })
            .collect();

        let relax_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("melange"),
            entries: &[uniform(0, false, std::mem::size_of::<Relax>() as u64), storage(1, false), storage(2, true)],
        });
        let relax_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("melange"),
            bind_group_layouts: &[&relax_layout],
            push_constant_ranges: &[],
        });
        let relax_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("melange"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blend.wgsl").into()),
        });
        let relax = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("relax"),
            layout: Some(&relax_pipeline_layout),
            module: &relax_module,
            entry_point: Some("relax"),
            compilation_options: Default::default(),
            cache: None,
        });
        Gpu { device, queue, layout, pipelines, relax_layout, relax, name }
    }

    /// Résout le système du mélange (jusqu'à 4 canaux) par Gauss-Seidel rouge-noir sur-relaxé.
    /// Le facteur optimal vient du rayon spectral de Jacobi, 4 / (4 + λ) ; le nombre de passes
    /// vise une erreur 1000 fois plus petite, avec une marge de 4.
    fn solve(&self, s: &crate::blend::System) -> Result<Vec<Vec<f32>>, Error> {
        let (w, h, c) = (s.width, s.height, s.rhs.len());
        if !self.fits(w, h, 0, 16) {
            return Err(Error::Gpu("image trop grande pour les limites de ce périphérique".into()));
        }
        let pack = |v: &[Vec<f32>]| {
            let mut out = vec![0f32; w * h * 4];
            for (ch, plane) in v.iter().enumerate() {
                for (p, value) in plane.iter().enumerate() {
                    out[p * 4 + ch] = *value;
                }
            }
            out
        };
        let x = self.storage(bytemuck::cast_slice(&pack(&s.start)));
        let rhs = self.storage(bytemuck::cast_slice(&pack(&s.rhs)));
        let jacobi = 4.0 / (4.0 + s.lambda);
        let omega = 2.0 / (1.0 + (1.0 - jacobi * jacobi).sqrt());
        let passes = ((4.0 * 1e-3f32.ln() / (omega - 1.0).max(1e-3).ln()).ceil() as usize).clamp(20, 2000);
        let groups: Vec<wgpu::BindGroup> = (0..2)
            .map(|parity| {
                let params = Relax { w: w as u32, h: h as u32, parity, lambda: s.lambda, omega, ..Relax::default() };
                let uniform = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::bytes_of(&params),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &self.relax_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: x.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 2, resource: rhs.as_entire_binding() },
                    ],
                })
            })
            .collect();
        for chunk in (0..2 * passes).collect::<Vec<_>>().chunks(OPS_PER_SUBMIT) {
            let mut encoder = self.device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_pipeline(&self.relax);
                for half in chunk {
                    pass.set_bind_group(0, &groups[half % 2], &[]);
                    pass.dispatch_workgroups(w.div_ceil(16) as u32, h.div_ceil(16) as u32, 1);
                }
            }
            self.queue.submit(Some(encoder.finish()));
        }
        let data = self.read(&[(&x, w * h * 16)])?;
        let values: &[f32] = bytemuck::cast_slice(&data[0]);
        Ok((0..c).map(|ch| (0..w * h).map(|p| values[p * 4 + ch]).collect()).collect())
    }

    /// Nom de la carte et de l'interface (Vulkan, Direct3D 12…).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Une image `width` × `height` avec ces nombres de canaux de style et de guides tient-elle
    /// dans les limites du périphérique ? (Celles par défaut de wgpu, 128 Mo par tampon, laissent
    /// passer la HD mais pas la 4K.)
    pub fn fits(&self, width: usize, height: usize, style_channels: usize, guide_channels: usize) -> bool {
        let words = style_channels.div_ceil(4) + guide_channels.div_ceil(4);
        let bytes = (width * height * words.max(4) * 4) as u64;
        let limits = self.device.limits();
        words <= 16 && bytes <= limits.max_storage_buffer_binding_size as u64 && bytes <= limits.max_buffer_size
    }

    fn storage(&self, data: &[u8]) -> wgpu::Buffer {
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: data,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        })
    }

    fn empty(&self, size: usize) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size.max(4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Relit des tampons de la carte (tous à la fois).
    fn read(&self, buffers: &[(&wgpu::Buffer, usize)]) -> Result<Vec<Vec<u8>>, Error> {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let staging: Vec<wgpu::Buffer> = buffers
            .iter()
            .map(|(b, size)| {
                let s = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: *size as u64,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                encoder.copy_buffer_to_buffer(b, 0, &s, 0, *size as u64);
                s
            })
            .collect();
        self.queue.submit(Some(encoder.finish()));
        let (tx, rx) = mpsc::channel();
        for s in &staging {
            let tx = tx.clone();
            s.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
        }
        self.device.poll(wgpu::Maintain::Wait);
        for _ in &staging {
            rx.recv().map_err(|e| Error::Gpu(e.to_string()))?.map_err(|e| Error::Gpu(e.to_string()))?;
        }
        Ok(staging.iter().map(|s| s.slice(..).get_mapped_range().to_vec()).collect())
    }
}

/// Une image rangée pour la carte : par pixel, les mots du style puis ceux des guides (4 octets par
/// mot, complétés par des zéros).
fn pack(w: usize, h: usize, style: Option<&Image>, ns: usize, guides: &[&Image], style_words: usize, words: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h * words * 4];
    for (i, px) in out.chunks_exact_mut(words * 4).enumerate() {
        if let Some(s) = style {
            px[..ns].copy_from_slice(&s.data[i * ns..(i + 1) * ns]);
        }
        let mut o = style_words * 4;
        for g in guides {
            px[o..o + g.channels].copy_from_slice(&g.data[i * g.channels..(i + 1) * g.channels]);
            o += g.channels;
        }
    }
    out
}

/// Le plan d'un niveau : chaque passage (noyau, groupe de liaisons, grille) et ses réglages.
#[derive(Default)]
struct Plan {
    ops: Vec<(Kernel, usize, (usize, usize))>,
    passes: Vec<Pass>,
}

impl Plan {
    fn push(&mut self, kernel: Kernel, group: usize, pass: Pass, grid: (usize, usize)) {
        self.ops.push((kernel, group, grid));
        self.passes.push(pass);
    }
}

impl Synthesizer for Gpu {
    fn blend(&self, a: &Image, ea: &ErrorMap, b: &Image, eb: &ErrorMap, alpha: f32, params: &BlendParams) -> Image {
        if params.linear || a.channels > 4 {
            return crate::blend::blend(a, ea, b, eb, alpha, params);
        }
        let system = crate::blend::system(a, ea, b, eb, alpha, params);
        match self.solve(&system) {
            Ok(channels) => crate::blend::to_image(a.width, a.height, &channels),
            // La carte ne répond pas : le processeur.
            Err(_) => crate::blend::blend(a, ea, b, eb, alpha, params),
        }
    }

    fn synthesize(&self, style: &Image, style_weight: f32, guides: &[GuidePair], params: &Params) -> Result<Output, Error> {
        let levels = check(style, style_weight, guides, params)?;
        let (sw, sh) = (style.width, style.height);
        let (tw, th) = (guides[0].target.width, guides[0].target.height);
        let ns = style.channels;
        let ng: usize = guides.iter().map(|g| g.source.channels).sum();
        let (style_words, words) = (ns.div_ceil(4), ns.div_ceil(4) + ng.div_ceil(4));
        if words > 16 {
            return Err(Error::TooManyChannels);
        }
        // Une erreur de validation de wgpu arrêterait le programme : on vérifie avant.
        if !self.fits(sw.max(tw), sh.max(th), ns, ng) {
            return Err(Error::Gpu("image trop grande pour les limites de ce périphérique".into()));
        }

        // Poids par canal, rangés comme les mots.
        let mut weights = [0f32; 64];
        weights[..ns].fill(style_weight / ns as f32);
        let mut c = style_words * 4;
        for g in guides {
            weights[c..c + g.source.channels].fill(g.weight / g.source.channels as f32);
            c += g.source.channels;
        }
        let weights = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&weights),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let sources: Vec<&Image> = guides.iter().map(|g| g.source).collect();
        let targets: Vec<&Image> = guides.iter().map(|g| g.target).collect();
        let full_source = self.storage(&pack(sw, sh, Some(style), ns, &sources, style_words, words));
        let full_target = self.storage(&pack(tw, th, None, ns, &targets, style_words, words));
        let align = self.device.limits().min_uniform_buffer_offset_alignment as usize;

        let mut seed = params.seed;
        let mut next_seed = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 32) as u32
        };
        let mut prev: Option<(wgpu::Buffer, usize, usize)> = None;
        for level in 0..levels {
            let last = level == levels - 1;
            let (lsw, lsh) = level_size((sw, sh), levels, level);
            let (ltw, lth) = level_size((tw, th), levels, level);
            let small_source;
            let small_target;
            let (source, target) = if last {
                (&full_source, &full_target)
            } else {
                small_source = self.empty(lsw * lsh * words * 4);
                small_target = self.empty(ltw * lth * words * 4);
                (&small_source, &small_target)
            };
            let (a, b) = (self.empty(ltw * lth * 8), self.empty(ltw * lth * 8));
            let err = self.empty(ltw * lth * 4);
            let omega = self.empty(lsw * lsh * 4);
            let mask = self.empty(ltw * lth * 4);
            let old = self.empty(ltw * lth * style_words * 4);

            // Le plan du niveau, comme ebsynth_cuda.cu.
            let base = Pass {
                tw: ltw as u32,
                th: lth as u32,
                sw: lsw as u32,
                sh: lsh as u32,
                words: words as u32,
                style_words: style_words as u32,
                patch: params.patch_size as u32,
                stop: params.stop_threshold,
                ..Pass::default()
            };
            let (grid, source_grid) = ((ltw, lth), (lsw, lsh));
            let mut plan = Plan::default();
            if !last {
                plan.push(Kernel::Resample, RESAMPLE_SOURCE, Pass { tw: lsw as u32, th: lsh as u32, pw: sw as u32, ph: sh as u32, ..base }, source_grid);
                plan.push(Kernel::Resample, RESAMPLE_TARGET, Pass { pw: tw as u32, ph: th as u32, ..base }, grid);
            }
            match &prev {
                None => plan.push(Kernel::InitRandom, MAIN_A, Pass { seed: next_seed(), ..base }, grid),
                Some((_, pw, ph)) => plan.push(Kernel::Upscale, UPSCALE, Pass { pw: *pw as u32, ph: *ph as u32, ..base }, grid),
            }
            plan.push(Kernel::OmegaClear, MAIN_A, base, source_grid);
            plan.push(Kernel::OmegaInit, MAIN_A, base, grid);
            let mut current = MAIN_A;
            let mut run = |plan: &mut Plan, patch: usize, lambda: f32, current: &mut usize| {
                let p = Pass {
                    patch: patch as u32,
                    lambda,
                    omega_best: (ltw * lth) as f32 / (lsw * lsh) as f32 * (patch * patch) as f32,
                    ..base
                };
                plan.push(Kernel::Vote, *current, p, grid);
                plan.push(Kernel::MaskAll, *current, p, grid);
                for sv in 0..params.search_vote_iters {
                    plan.push(Kernel::EvalError, *current, p, grid);
                    for _ in 0..params.patchmatch_iters {
                        for radius in [4, 2, 1] {
                            plan.push(Kernel::Propagate, *current, Pass { radius, ..p }, grid);
                            *current = if *current == MAIN_A { MAIN_B } else { MAIN_A };
                        }
                        let mut radius = 1;
                        while radius < lsw.max(lsh) as i32 / 2 {
                            plan.push(Kernel::RandomSearch, *current, Pass { radius, seed: next_seed(), ..p }, grid);
                            radius *= 2;
                        }
                    }
                    plan.push(Kernel::EvalError, *current, p, grid);
                    plan.push(Kernel::Vote, *current, p, grid);
                    if sv + 1 < params.search_vote_iters {
                        plan.push(Kernel::EvalMask, *current, p, grid);
                        plan.push(Kernel::Dilate, *current, p, grid);
                    }
                }
            };
            run(&mut plan, params.patch_size, params.uniformity, &mut current);
            if last && params.extra_pass_3x3 {
                run(&mut plan, 3, 0.0, &mut current);
            }

            // Réglages de chaque passage, à des adresses alignées.
            let mut bytes = vec![0u8; plan.passes.len() * align];
            for (i, p) in plan.passes.iter().enumerate() {
                bytes[i * align..i * align + std::mem::size_of::<Pass>()].copy_from_slice(bytemuck::bytes_of(p));
            }
            let passes = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let group = |src: &wgpu::Buffer, tgt: &wgpu::Buffer, nnf: &wgpu::Buffer, nnf2: &wgpu::Buffer| {
                let buffers = [src, tgt, nnf, nnf2, &err, &omega, &mask, &old];
                let mut entries = vec![
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &passes,
                            offset: 0,
                            size: wgpu::BufferSize::new(std::mem::size_of::<Pass>() as u64),
                        }),
                    },
                    wgpu::BindGroupEntry { binding: 1, resource: weights.as_entire_binding() },
                ];
                entries.extend(buffers.iter().enumerate().map(|(k, buf)| wgpu::BindGroupEntry {
                    binding: k as u32 + 2,
                    resource: buf.as_entire_binding(),
                }));
                self.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &self.layout, entries: &entries })
            };
            let mut groups = vec![group(source, target, &a, &b), group(source, target, &b, &a)];
            if !last {
                groups.push(group(&full_source, source, &a, &b));
                groups.push(group(&full_target, target, &a, &b));
            } else {
                groups.push(group(source, target, &a, &b));
                groups.push(group(source, target, &a, &b));
            }
            groups.push(match &prev {
                Some((p, _, _)) => group(source, target, &a, p),
                None => group(source, target, &a, &b),
            });

            for (chunk, ops) in plan.ops.chunks(OPS_PER_SUBMIT).enumerate() {
                let mut encoder = self.device.create_command_encoder(&Default::default());
                {
                    let mut pass = encoder.begin_compute_pass(&Default::default());
                    for (k, (kernel, g, (gw, gh))) in ops.iter().enumerate() {
                        let i = chunk * OPS_PER_SUBMIT + k;
                        pass.set_pipeline(&self.pipelines[*kernel as usize]);
                        pass.set_bind_group(0, &groups[*g], &[(i * align) as u32]);
                        pass.dispatch_workgroups(gw.div_ceil(16) as u32, gh.div_ceil(16) as u32, 1);
                    }
                }
                self.queue.submit(Some(encoder.finish()));
            }

            let nnf = if current == MAIN_A { a } else { b };
            if last {
                let data = self.read(&[(target, ltw * lth * words * 4), (&nnf, ltw * lth * 8), (&err, ltw * lth * 4)])?;
                let mut image = Vec::with_capacity(tw * th * ns);
                for px in data[0].chunks_exact(words * 4) {
                    image.extend_from_slice(&px[..ns]);
                }
                let nnf = data[1].chunks_exact(8).map(|c| {
                    [i32::from_le_bytes(c[..4].try_into().unwrap()), i32::from_le_bytes(c[4..].try_into().unwrap())]
                });
                let error = data[2].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap()));
                return Ok(Output { image: Image::new(tw, th, ns, image), nnf: nnf.collect(), error: error.collect() });
            }
            prev = Some((nnf, ltw, lth));
        }
        unreachable!("le dernier niveau rend le résultat")
    }
}
