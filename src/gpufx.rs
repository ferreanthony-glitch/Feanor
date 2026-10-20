//! Effets de calque sur la carte graphique (Anthony, 2026-10-19 : « de l'ordre du temps réel à pleine
//! résolution »). Jusque-là, un calque à effets passait par un calcul sur processeur, en arrière-plan
//! (`fxcache.rs`) : pleine résolution seulement au repos, aperçu au quart en lecture sur une feuille 4K.
//!
//! Ici, la pile d'effets d'un calque est une **chaîne de passes** de dessin sur des textures de travail :
//! chaque effet est une ou plusieurs passes (le flou en a deux, la lueur quatre…), la dernière écrit
//! dans la texture du calque, qui est ensuite composée comme n'importe quel dessin (`gpu.rs`). Les
//! réglages (déjà interpolés entre les clés, sur le processeur : `fx::steps_at`) partent à la carte
//! graphique en uniformes, à chaque image. Un calque d'ajustement (`calqueffet.rs`) passe sa pile sur
//! l'image composée de tout ce qui est dessous, à l'endroit même où il se trouve dans la pile.
//!
//! # Exactitude
//!
//! Le calcul sur processeur (`fx.rs`, `fxfilters.rs`, `fxcine.rs`) reste la référence : c'est lui qui
//! sert à l'export, aux vignettes, et en repli sans carte graphique. Chaque passe reproduit sa formule
//! (`gpufx.wgsl`), avec les mêmes arrondis (les textures de travail sont en 8 bits comme les dessins) et
//! le même hasard (bruit, grain, rayons de l'image d'impact : la même fonction de hachage). Le flou
//! gaussien est celui du processeur, tel quel : trois flous en boîte en lignes puis trois en colonnes, en
//! entiers, avec la même perte au bord de la feuille (un noyau unique, plus rapide à écrire, aurait
//! donné jusqu'à 40 niveaux d'écart sur les bords de la feuille). Écarts mesurés : `gpufx_tests.rs`.
//!
//! # Ce qui reste sur le processeur
//!
//! Tous les effets de la pile ont une version sur la carte graphique sauf le tremblement du trait
//! (`tremblement.rs` : il redessine le dessin vectoriel lui-même, calque vectoriel seulement), qui garde
//! le calcul de fond. Règle de mélange : **une pile qui contient un seul effet sans passe est calculée
//! tout entière sur le processeur** (l'ordre des effets compte, et le calcul sur processeur ne sait pas
//! partir d'une texture de la carte graphique). Même chose pour un calque rempli par CTG, un fond sous le
//! trait, une image de vidéo ou une feuille flottante en cours de geste. Un calque d'ajustement dans un
//! dossier, ou à côté d'un qui ne passe pas, est de même résolu sur le processeur.
//!
//! # Géométrie
//!
//! Chaque texture montre un rectangle de la feuille (`Geo`), de la taille de ce que l'effet peut
//! toucher (le contenu du calque, plus la portée du flou…), arrondie à 128 pixels (`gpu::texture_axis`,
//! pour que les textures de travail se recyclent d'une image à l'autre). Hors du rectangle, une lecture
//! rend du transparent. Les effets qui remplissent toute l'image (flash, vignettage, secousse, image
//! d'impact, lueur diffuse) écrivent la feuille entière.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::num::NonZeroU64;

use eframe::egui_wgpu::wgpu;

use crate::fx::{EffectKind, FxStep};

const SHADER: &str = include_str!("gpufx.wgsl");

/// Octets de la structure d'uniformes `P` du shader : 5 vec4 d'entiers, 8 vec4 de réels.
const PARAM_BYTES: u64 = 5 * 16 + 8 * 16;
/// Passes que le tampon d'uniformes contient (256 Ko ; une image en compte rarement plus de cent).
const FIRST_SLOTS: u64 = 1024;
/// Au-delà, les textures de travail libres sont jetées (octets).
const POOL_BYTES: u64 = 256 << 20;
/// Une texture de travail libre depuis autant de préparations (8 s à 60 images/s) est jetée.
const POOL_AGE: u64 = 480;

// ---------------------------------------------------------------------- géométrie

/// Un rectangle de pixels : coin haut-gauche et taille.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Geo {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Geo {
    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn from_rect(r: [u32; 4]) -> Geo {
        Geo { x: r[0] as i32, y: r[1] as i32, w: r[2] as i32, h: r[3] as i32 }
    }

    pub fn rect(&self) -> [u32; 4] {
        [self.x as u32, self.y as u32, self.w as u32, self.h as u32]
    }

    fn union(self, o: Geo) -> Geo {
        let (x0, y0) = (self.x.min(o.x), self.y.min(o.y));
        Geo { x: x0, y: y0, w: self.right().max(o.right()) - x0, h: self.bottom().max(o.bottom()) - y0 }
    }

    fn array(&self) -> [i32; 4] {
        [self.x, self.y, self.w, self.h]
    }
}

/// Le rectangle [x0, x1) × [y0, y1) dans un espace de `space` pixels, borné à cet espace, avec le
/// pixel de transparent de marge et l'arrondi aux 128 pixels de `gpu::texture_axis`.
fn bucket(x0: i32, y0: i32, x1: i32, y1: i32, space: (i32, i32)) -> Geo {
    let (sw, sh) = (space.0.max(1), space.1.max(1));
    let (x0, y0) = (x0.clamp(0, sw - 1), y0.clamp(0, sh - 1));
    let (x1, y1) = (x1.clamp(x0 + 1, sw), y1.clamp(y0 + 1, sh));
    let (ox, w) = crate::gpu::texture_axis(x0 as usize, x1 as usize, sw as usize);
    let (oy, h) = crate::gpu::texture_axis(y0 as usize, y1 as usize, sh as usize);
    Geo { x: ox as i32, y: oy as i32, w: w as i32, h: h as i32 }
}

// ---------------------------------------------------------------------- la chaîne de passes

/// Une texture que lit ou écrit une passe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tex {
    /// Ce que l'effet reçoit (la texture du dessin, ou l'image composée).
    Input,
    /// Une texture de travail.
    Temp(usize),
}

/// Une passe de dessin : un point d'entrée du shader, ses textures, ses réglages.
#[derive(Clone, Debug)]
pub struct Pass {
    pub entry: &'static str,
    pub a: Tex,
    pub ga: Geo,
    pub b: Option<Tex>,
    pub gb: Geo,
    pub dst: Tex,
    pub gd: Geo,
    pub sheet: (i32, i32),
    pub n: [i32; 4],
    pub f: [[f32; 4]; 8],
}

/// La pile d'effets d'un calque, en passes. La dernière écrit dans `out_temp` (que l'appelant remplace
/// par sa texture de résultat, de la taille `out`).
#[derive(Clone, Debug)]
pub struct Chain {
    pub passes: Vec<Pass>,
    pub temps: Vec<Geo>,
    pub out_temp: usize,
    pub out: Geo,
}

/// L'effet a-t-il une version sur la carte graphique ?
pub fn supported(kind: EffectKind) -> bool {
    // (Les effets du trait lisent la géométrie du dessin vectoriel : `fxtrait.rs`, calcul de fond. Le
    // tremblement, lui, relit les pixels du dessin à travers un champ de déplacement : `fs_boil`.)
    kind != EffectKind::Boil && !kind.is_trait()
}

/// Toute la pile passe-t-elle par la carte graphique ? (Un seul effet qui n'y passe pas : toute la pile
/// reste sur le processeur.)
pub fn steps_supported(steps: &[FxStep]) -> bool {
    !steps.is_empty() && steps.iter().all(|s| supported(s.kind))
}

/// Tous les repères de calques d'ajustement du plan passent-ils par la carte graphique ? Oui s'ils sont
/// hors dossier (ils couvrent alors tout ce qui est dessous : l'image composée jusque-là) et si toute leur
/// pile a des passes. Sinon, tous les repères du plan sont résolus sur le processeur (`calqueffet::resolve`) :
/// le calcul sur processeur ne sait pas lire un repère que la carte graphique résout.
pub fn adjustments_supported(plan: &crate::render::Plan) -> bool {
    plan.items.iter().all(|i| match &i.fond {
        Some(crate::render::FondPart::Reglage(r)) => r.root && steps_supported(&r.steps),
        _ => true,
    })
}

/// Les rayons des trois flous en boîte qui font un flou gaussien de portée `radius` (ceux du calcul sur
/// processeur, `fx::gaussian` : les mêmes, donc les mêmes arrondis et la même perte au bord de la feuille).
pub fn box_radii(radius: f32) -> [i32; 3] {
    crate::fx::boxes_for(radius / 3.0).map(|r| r as i32)
}

struct Builder {
    passes: Vec<Pass>,
    temps: Vec<Geo>,
    sheet: (i32, i32),
}

type Cur = (Tex, Geo);

impl Builder {
    fn temp(&mut self, g: Geo) -> Cur {
        self.temps.push(g);
        (Tex::Temp(self.temps.len() - 1), g)
    }

    #[allow(clippy::too_many_arguments)]
    fn pass(&mut self, entry: &'static str, a: Cur, b: Option<Cur>, dst: Cur, space: (i32, i32), n: [i32; 4], f: [[f32; 4]; 8]) {
        self.passes.push(Pass { entry, a: a.0, ga: a.1, b: b.map(|b| b.0), gb: b.map_or(Geo::default(), |b| b.1), dst: dst.0, gd: dst.1, sheet: space, n, f });
    }

    /// Une passe qui écrit le même rectangle que celui qu'elle lit.
    fn same(&mut self, entry: &'static str, cur: Cur, n: [i32; 4], f: [[f32; 4]; 8]) -> Cur {
        let dst = self.temp(cur.1);
        self.pass(entry, cur, None, dst, self.sheet, n, f);
        dst
    }

    /// Une passe qui écrit `g` en lisant `cur`.
    fn into(&mut self, entry: &'static str, cur: Cur, g: Geo, n: [i32; 4], f: [[f32; 4]; 8]) -> Cur {
        let dst = self.temp(g);
        self.pass(entry, cur, None, dst, self.sheet, n, f);
        dst
    }

    fn full(&self) -> Geo {
        Geo { x: 0, y: 0, w: self.sheet.0, h: self.sheet.1 }
    }

    /// Flou gaussien dans un espace de `space` pixels : trois flous en boîte en lignes, puis trois en
    /// colonnes, comme le calcul sur processeur (`fx::gaussian`) ; chacun écrit un rectangle élargi de
    /// son rayon, borné à la feuille (ce qui sort de la feuille est perdu à chaque passe, comme là-bas).
    fn blur(&mut self, cur: Cur, radius: f32, space: (i32, i32)) -> Cur {
        let radii = box_radii(radius);
        let mut cur = cur;
        for r in radii.into_iter().filter(|&r| r > 0) {
            let g = bucket(cur.1.x - r, cur.1.y, cur.1.right() + r, cur.1.bottom(), space);
            let next = self.temp(g);
            self.pass("fs_box_h", cur, None, next, space, [r, 0, 0, 0], [[0.0; 4]; 8]);
            cur = next;
        }
        for r in radii.into_iter().filter(|&r| r > 0) {
            let g = bucket(cur.1.x, cur.1.y - r, cur.1.right(), cur.1.bottom() + r, space);
            let next = self.temp(g);
            self.pass("fs_box_v", cur, None, next, space, [r, 0, 0, 0], [[0.0; 4]; 8]);
            cur = next;
        }
        cur
    }

    /// Un effet de plus sur `cur` ; rien : il n'a pas de passe.
    fn effect(&mut self, s: &FxStep, cur: Cur) -> Option<Cur> {
        let at = |k: usize| s.values.get(k).copied().unwrap_or(0.0);
        let mut f = [[0.0f32; 4]; 8];
        let mut n = [0i32; 4];
        let full = self.full();
        let (sw, sh) = (self.sheet.0 as f32, self.sheet.1 as f32);
        Some(match s.kind {
            EffectKind::BrightnessContrast => {
                let c = at(1);
                let k = if c >= 0.0 { 1.0 + c / 100.0 * 3.0 } else { 1.0 + c / 100.0 };
                f[0] = [k, at(0) / 100.0 * 0.5, 0.0, 0.0];
                self.same("fs_bc", cur, n, f)
            }
            EffectKind::Levels => {
                let (black, white, gamma) = (at(0), at(1), at(2));
                f[0] = [black, (white - black).max(1.0), 1.0 / gamma.max(0.01), 0.0];
                self.same("fs_levels", cur, n, f)
            }
            EffectKind::HueSaturation => {
                f[0] = [at(0) / 360.0, 1.0 + at(1) / 100.0, at(2) / 100.0, 0.0];
                self.same("fs_hue", cur, n, f)
            }
            EffectKind::BlackWhite => {
                f[0][0] = (at(0) / 100.0).clamp(0.0, 1.0);
                self.same("fs_bw", cur, n, f)
            }
            EffectKind::Invert => {
                f[0][0] = (at(0) / 100.0).clamp(0.0, 1.0);
                self.same("fs_invert", cur, n, f)
            }
            EffectKind::Posterize => {
                f[0][0] = at(0).round().max(2.0) - 1.0;
                self.same("fs_poster", cur, n, f)
            }
            EffectKind::Threshold => {
                f[0][0] = at(0);
                self.same("fs_thresh", cur, n, f)
            }
            EffectKind::Noise => {
                f[0][0] = at(0) / 100.0 * 0.5;
                self.same("fs_noise", cur, n, f)
            }
            EffectKind::Grain => {
                f[0][0] = (at(0) / 100.0).clamp(0.0, 1.0) * 0.5;
                n = [(at(1).round().max(1.0) as i32).max(1), at(2).max(0.0) as i32, at(3).max(0.0) as i32, 0];
                self.same("fs_grain", cur, n, f)
            }
            EffectKind::Flash => {
                f[0] = [(at(0) / 100.0).clamp(0.0, 1.0), (at(1) / 255.0).clamp(0.0, 1.0), (at(2) / 255.0).clamp(0.0, 1.0), (at(3) / 255.0).clamp(0.0, 1.0)];
                n[0] = i32::from(at(4) >= 0.5);
                self.into("fs_flash", cur, full, n, f)
            }
            EffectKind::Vignette => {
                let (ox, oy) = (sw * at(3) / 100.0, sh * at(4) / 100.0);
                let (rx, ry) = ((sw * at(5) / 200.0).max(1.0), (sh * at(6) / 200.0).max(1.0));
                let r0 = at(1) / 100.0;
                f[0] = [(at(0) / 100.0).clamp(0.0, 1.0), r0, r0 + (at(2) / 100.0).max(0.01), 0.0];
                f[1] = [ox, oy, rx, ry];
                self.into("fs_vignette", cur, full, n, f)
            }
            EffectKind::Aberration => {
                let reach = ((sw * sw + sh * sh).sqrt() / 2.0).max(1.0);
                f[0] = [at(0) / reach, sw * at(1) / 100.0, sh * at(2) / 100.0, 0.0];
                let m = (2.0 * at(0)).ceil() as i32 + 2;
                let g = bucket(cur.1.x - m, cur.1.y - m, cur.1.right() + m, cur.1.bottom() + m, self.sheet);
                self.into("fs_aberr", cur, g, n, f)
            }
            EffectKind::Shake => {
                let (dx, dy, angle, zoom, blur) = (at(0), at(1), at(2), at(3), at(4));
                let z = (zoom / 100.0).max(0.05);
                let (sn, cs) = (-angle).to_radians().sin_cos();
                let len = (dx * dx + dy * dy).sqrt();
                let spread = (blur / 100.0).clamp(0.0, 1.0);
                n[0] = if spread > 0.0 && len >= 1.0 { ((len * spread / 3.0).ceil() as i32 + 1).clamp(2, 6) } else { 1 };
                f[0] = [sw / 2.0, sh / 2.0, z, dx];
                f[1] = [dy, sn, cs, spread];
                self.into("fs_shake", cur, full, n, f)
            }
            EffectKind::Impact => {
                let tint = [at(3), at(4), at(5)].map(|c| (c / 255.0).clamp(0.0, 1.0));
                f[0] = [(at(0) / 100.0).clamp(0.0, 1.0), at(2) / 255.0, (at(6) / 100.0).clamp(0.0, 1.0), (at(7) / 100.0).clamp(0.0, 1.0)];
                f[1] = [tint[0], tint[1], tint[2], (at(8) / 100.0).clamp(0.0, 1.0)];
                f[2] = [sw * at(9) / 100.0, sh * at(10) / 100.0, sw, sh];
                f[3][0] = (sw.min(sh) / 2.0).max(1.0);
                n = [at(1).round() as i32, at(11).max(0.0) as i32, 0, 0];
                self.into("fs_impact", cur, full, n, f)
            }
            EffectKind::GaussianBlur => self.blur(cur, at(0), self.sheet),
            EffectKind::MotionBlur => {
                let (length, angle) = (at(0), at(1));
                let (sn, cs) = angle.to_radians().sin_cos();
                n[0] = (length.ceil() as i32).clamp(2, 64);
                f[0] = [cs, sn, length, 0.0];
                let m = (length / 2.0).ceil() as i32 + 2;
                let g = bucket(cur.1.x - m, cur.1.y - m, cur.1.right() + m, cur.1.bottom() + m, self.sheet);
                self.into("fs_motion", cur, g, n, f)
            }
            EffectKind::Sharpen => {
                let halo = self.blur(cur, at(1).max(0.5), self.sheet);
                f[0][0] = at(0) / 100.0;
                let dst = self.temp(cur.1);
                self.pass("fs_sharpen", cur, Some(halo), dst, self.sheet, n, f);
                dst
            }
            EffectKind::Glow => {
                let halo = self.blur(cur, at(0).max(1.0), self.sheet);
                f[0][0] = at(1) / 100.0;
                let dst = self.temp(cur.1.union(halo.1));
                self.pass("fs_glow_over", cur, Some(halo), dst, self.sheet, n, f);
                dst
            }
            EffectKind::DropShadow => {
                let (dx, dy) = (at(0).round() as i32, at(1).round() as i32);
                f[0][0] = (at(3) / 100.0).clamp(0.0, 1.0);
                n = [dx, dy, 0, 0];
                let g = bucket(cur.1.x + dx.min(0), cur.1.y + dy.min(0), cur.1.right() + dx.max(0), cur.1.bottom() + dy.max(0), self.sheet);
                let sil = self.into("fs_shadow_sil", cur, g, n, f);
                let shadow = if at(2) >= 0.5 { self.blur(sil, at(2), self.sheet) } else { sil };
                let dst = self.temp(cur.1.union(shadow.1));
                self.pass("fs_shadow_over", cur, Some(shadow), dst, self.sheet, [0; 4], [[0.0; 4]; 8]);
                dst
            }
            EffectKind::Wave => {
                let (sn, cs) = at(2).to_radians().sin_cos();
                f[0] = [at(0), std::f32::consts::TAU / at(1).max(1.0), sn, cs];
                self.into("fs_warp", cur, full, [0; 4], f)
            }
            EffectKind::Ripple => {
                f[0] = [at(0), std::f32::consts::TAU / at(1).max(1.0), sw * at(2) / 100.0, sh * at(3) / 100.0];
                self.into("fs_warp", cur, full, [1, 0, 0, 0], f)
            }
            EffectKind::Twirl => {
                let half = sw.min(sh) / 2.0;
                f[0] = [sw * at(2) / 100.0, sh * at(3) / 100.0, (half * at(1) / 100.0).max(1.0), at(0).to_radians()];
                self.into("fs_warp", cur, full, [2, 0, 0, 0], f)
            }
            EffectKind::Pinch => {
                let half = sw.min(sh) / 2.0;
                f[0] = [sw * at(2) / 100.0, sh * at(3) / 100.0, (half * at(1) / 100.0).max(1.0), 1.0 - (at(0) / 100.0).clamp(-1.0, 1.0) * 0.9];
                self.into("fs_warp", cur, full, [3, 0, 0, 0], f)
            }
            EffectKind::Clouds => {
                f[0] = [1.0 / at(0).max(2.0), (at(1) / 100.0).clamp(0.0, 1.0), 0.0, 0.0];
                self.same("fs_shade", cur, [0, at(2).round() as u32 as i32, 0, 0], f)
            }
            EffectKind::Lighting => {
                let half = sw.min(sh) / 2.0;
                f[0] = [sw * at(0) / 100.0, sh * at(1) / 100.0, (half * at(2) / 100.0).max(1.0), (at(3) / 100.0).clamp(0.0, 1.0)];
                self.same("fs_shade", cur, [1, 0, 0, 0], f)
            }
            EffectKind::Gradient => {
                let (sn, cs) = at(0).to_radians().sin_cos();
                f[0] = [sw / 2.0, sh / 2.0, cs, sn];
                f[1] = [(sw * cs.abs() + sh * sn.abs()).max(1.0) / 2.0, (at(1) / 100.0).clamp(0.0, 1.0), 0.0, 0.0];
                self.same("fs_shade", cur, [2, 0, 0, 0], f)
            }
            EffectKind::Mosaic => {
                // Une valeur par carré (texture de la taille des carrés, une passe par carré), puis chaque
                // pixel prend celle de son carré.
                let s = (at(0).round().max(1.0) as i32).max(1);
                let cells = ((self.sheet.0 + s - 1) / s, (self.sheet.1 + s - 1) / s);
                let (cx0, cy0) = (cur.1.x / s, cur.1.y / s);
                let (cx1, cy1) = ((cur.1.right() + s - 1) / s, (cur.1.bottom() + s - 1) / s);
                let small = bucket(cx0, cy0, cx1, cy1, cells);
                let avg = self.temp(small);
                self.pass("fs_mosaic_avg", cur, None, avg, self.sheet, [s, 0, 0, 0], f);
                let g = bucket(small.x * s, small.y * s, small.right() * s, small.bottom() * s, self.sheet);
                self.into("fs_mosaic_fill", avg, g, [s, 0, 0, 0], f)
            }
            EffectKind::Bloom => {
                let (radius, intensity, threshold) = (at(0), at(1), at(2));
                let level = crate::fxcine::blur_level(radius);
                let div = 1i32 << level;
                let small = ((self.sheet.0 + div - 1) / div, (self.sheet.1 + div - 1) / div);
                let small_geo = Geo { x: 0, y: 0, w: small.0, h: small.1 };
                f[0][0] = (threshold / 255.0).clamp(0.0, 0.99);
                let dst = self.temp(small_geo);
                self.pass("fs_bloom_down", cur, None, dst, small, [div, 0, 0, 0], f);
                let halo = self.blur(dst, (radius / div as f32).max(1.0), small);
                let mut f2 = [[0.0f32; 4]; 8];
                f2[0][0] = (intensity / 100.0).max(0.0);
                let out = self.temp(full);
                self.pass("fs_bloom_final", cur, Some(halo), out, self.sheet, [div, 0, 0, 0], f2);
                out
            }
            _ => return None,
        })
    }
}

/// La chaîne de passes qui applique `steps` à une texture qui montre `input` d'une feuille de `sheet`
/// pixels. Rien si un effet n'a pas de passe, ou si la pile ne change rien.
pub fn plan_chain(steps: &[FxStep], input: Geo, sheet: (u32, u32)) -> Option<Chain> {
    let sheet = (sheet.0 as i32, sheet.1 as i32);
    let mut b = Builder { passes: Vec::new(), temps: Vec::new(), sheet };
    let mut cur: Cur = (Tex::Input, input);
    for s in steps {
        cur = b.effect(s, cur)?;
    }
    let Tex::Temp(out_temp) = cur.0 else { return None };
    Some(Chain { passes: b.passes, temps: b.temps, out_temp, out: cur.1 })
}

/// Le numéro de la texture résultat des effets du `nth`-ième élément du plan qui montre le dessin
/// `src` avec cette suite d'effets (les genres seulement : l'animation des réglages ne change pas de
/// texture d'une image à l'autre, elle la redessine). Bit de poids fort mis, comme les dessins dérivés.
pub fn out_id(src: crate::document::CelId, nth: usize, steps: &[FxStep]) -> crate::document::CelId {
    let mut h = DefaultHasher::new();
    "effets sur la carte graphique".hash(&mut h);
    src.hash(&mut h);
    nth.hash(&mut h);
    for s in steps {
        s.kind.hash(&mut h);
    }
    h.finish() | (1 << 63)
}

/// Ce que la texture résultat doit montrer : les effets et leurs réglages à cette image, le dessin
/// d'origine (sa texture, par sa génération et sa place) et la feuille. Jamais 0.
pub fn signature(steps: &[FxStep], rect: [u32; 4], generation: u64, sheet: (u32, u32)) -> u64 {
    let mut h = DefaultHasher::new();
    (rect, generation, sheet).hash(&mut h);
    for s in steps {
        s.kind.hash(&mut h);
        for v in &s.values {
            v.to_bits().hash(&mut h);
        }
    }
    h.finish() | 1
}

// ---------------------------------------------------------------------- la carte graphique

struct Scratch {
    /// (La vue la garde en vie ; elle est gardée ici pour que sa durée de vie se voie.)
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    w: u32,
    h: u32,
    free: bool,
    used: u64,
}

impl Scratch {
    fn bytes(&self) -> u64 {
        self.w as u64 * self.h as u64 * 4
    }
}

/// Format des textures de travail : celui des dessins (8 bits, valeurs gamma prémultipliées).
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Une texture que les passes peuvent écrire et qu'on peut lire ensuite comme un dessin.
pub fn target_texture(device: &wgpu::Device, w: u32, h: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("effets : texture de travail"),
        size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// Les points d'entrée du shader qui sont une passe (`gpufx.wgsl`).
pub const ENTRIES: [&str; 27] = [
    "fs_bc", "fs_levels", "fs_hue", "fs_bw", "fs_invert", "fs_poster", "fs_thresh", "fs_noise", "fs_grain", "fs_flash", "fs_impact", "fs_vignette", "fs_aberr", "fs_shake", "fs_warp", "fs_shade", "fs_mosaic_avg", "fs_mosaic_fill", "fs_box_h", "fs_box_v", "fs_motion",
    "fs_shadow_sil", "fs_shadow_over", "fs_glow_over", "fs_sharpen", "fs_bloom_down", "fs_bloom_final",
];

/// Ce qu'il faut pour fabriquer les pipelines (aussi sur un autre fil : le périphérique, le shader et la
/// disposition se partagent).
#[derive(Clone)]
struct Maker {
    device: wgpu::Device,
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
}

impl Maker {
    fn make(&self, entry: &'static str) -> wgpu::RenderPipeline {
        self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(entry),
            layout: Some(&self.layout),
            vertex: wgpu::VertexState { module: &self.shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some(entry),
                targets: &[Some(wgpu::ColorTargetState { format: FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }
}

type Pipelines = std::sync::Arc<std::sync::Mutex<HashMap<&'static str, wgpu::RenderPipeline>>>;

/// Les pipelines, les uniformes et les textures de travail des effets sur la carte graphique.
pub struct GpuFx {
    maker: Maker,
    param_layout: wgpu::BindGroupLayout,
    tex_layout: wgpu::BindGroupLayout,
    pipelines: Pipelines,
    params: wgpu::Buffer,
    params_group: wgpu::BindGroup,
    capacity: u64,
    stride: u64,
    slot: u64,
    empty: wgpu::TextureView,
    pool: Vec<Scratch>,
    tick: u64,
    /// Passes dessinées depuis le début (essais et mesures).
    pub passes_drawn: u64,
}

fn param_buffer(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, stride: u64, capacity: u64) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("effets : réglages des passes"),
        size: stride * capacity,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("effets : réglages des passes"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &buffer, offset: 0, size: NonZeroU64::new(PARAM_BYTES) }),
        }],
    });
    (buffer, group)
}

/// Attend un futur qui ne dépend que de la carte graphique (la pose d'une portée d'erreurs répond
/// aussitôt sur les moteurs natifs).
fn block_on<F: std::future::Future>(device: &wgpu::Device, f: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    let mut f = std::pin::pin!(f);
    for _ in 0..2000 {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        device.poll(wgpu::Maintain::Poll);
        std::thread::yield_now();
    }
    // (Jamais arrivé : on attend alors sans fin plutôt que de renvoyer une valeur inventée.)
    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        device.poll(wgpu::Maintain::Wait);
    }
}

impl GpuFx {
    /// Les effets sur la carte graphique, ou rien si le shader est refusé par cette carte (la
    /// validation de wgpu ne doit pas fermer le logiciel : le calcul sur processeur sert alors).
    pub fn try_new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<GpuFx> {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let fx = GpuFx::new(device, queue);
        let error = block_on(device, device.pop_error_scope());
        match error {
            None => Some(fx),
            Some(e) => {
                eprintln!("effets sur la carte graphique impossibles ({e}) : le calcul sur processeur sert");
                None
            }
        }
    }

    pub fn new(device: &wgpu::Device, _queue: &wgpu::Queue) -> GpuFx {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("effets de calque"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let param_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("effets : réglages"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: NonZeroU64::new(PARAM_BYTES) },
                count: None,
            }],
        });
        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("effets : texture"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("effets"),
            bind_group_layouts: &[&param_layout, &tex_layout, &tex_layout],
            push_constant_ranges: &[],
        });
        let align = device.limits().min_uniform_buffer_offset_alignment as u64;
        let stride = PARAM_BYTES.div_ceil(align) * align;
        let (params, params_group) = param_buffer(device, &param_layout, stride, FIRST_SLOTS);
        // (Une texture neuve est transparente : c'est ce que lit une passe qui n'a pas de seconde texture.)
        let empty = target_texture(device, 1, 1).create_view(&Default::default());
        let maker = Maker { device: device.clone(), shader, layout };
        let pipelines: Pipelines = Default::default();
        // Les pipelines se fabriquent d'avance sur un autre fil (de 5 à 30 ms chacun : le premier affichage
        // d'un effet ne ferait sinon qu'un accroc, une seule fois). Un pipeline demandé avant d'être prêt est
        // fabriqué tout de suite. Pas dans les essais : chacun lance sa propre carte.
        if !cfg!(test) {
            let (maker, pipelines) = (maker.clone(), pipelines.clone());
            let _ = std::thread::Builder::new().name("effets : pipelines".into()).spawn(move || {
                for entry in ENTRIES {
                    let known = pipelines.lock().unwrap_or_else(|e| e.into_inner()).contains_key(entry);
                    if !known {
                        let p = maker.make(entry);
                        pipelines.lock().unwrap_or_else(|e| e.into_inner()).entry(entry).or_insert(p);
                    }
                }
            });
        }
        GpuFx { maker, param_layout, tex_layout, pipelines, params, params_group, capacity: FIRST_SLOTS, stride, slot: 0, empty, pool: Vec::new(), tick: 0, passes_drawn: 0 }
    }

    /// Début d'une préparation : l'horloge des textures de travail avance.
    pub fn begin_frame(&mut self) {
        self.tick += 1;
    }

    /// Essais et mesures : mémoire vidéo prise par les textures de travail (octets).
    #[cfg(test)]
    pub fn bytes(&self) -> u64 {
        self.pool.iter().map(Scratch::bytes).sum()
    }

    /// Le pipeline de ce point d'entrée (fabriqué maintenant s'il n'est pas déjà prêt).
    fn pipeline(&self, entry: &'static str) -> wgpu::RenderPipeline {
        if let Some(p) = self.pipelines.lock().unwrap_or_else(|e| e.into_inner()).get(entry) {
            return p.clone();
        }
        let p = self.maker.make(entry);
        self.pipelines.lock().unwrap_or_else(|e| e.into_inner()).entry(entry).or_insert(p).clone()
    }

    /// Essais : tous les pipelines sont fabriqués (un point d'entrée mal écrit se voit ici).
    #[cfg(test)]
    pub fn make_all(&self) {
        for entry in ENTRIES {
            self.pipeline(entry);
        }
    }

    /// Une texture de travail de `w` × `h`, prise dans la réserve ou neuve. Rend son rang.
    fn acquire(&mut self, device: &wgpu::Device, w: u32, h: u32) -> usize {
        let found = self.pool.iter().position(|s| s.free && s.w == w && s.h == h);
        let i = found.unwrap_or_else(|| {
            let texture = target_texture(device, w, h);
            let view = texture.create_view(&Default::default());
            self.pool.push(Scratch { _texture: texture, view, w, h, free: true, used: 0 });
            self.pool.len() - 1
        });
        self.pool[i].free = false;
        self.pool[i].used = self.tick;
        i
    }

    /// Rend la texture de travail `i` à la réserve.
    pub fn release(&mut self, i: usize) {
        if let Some(s) = self.pool.get_mut(i) {
            s.free = true;
        }
    }

    /// La texture de travail `i` (pour la poser comme un dessin).
    pub fn view(&self, i: usize) -> &wgpu::TextureView {
        &self.pool[i].view
    }

    /// Garde les textures de travail les plus récentes, dans la limite de la réserve, et jette celles qui
    /// ne servent plus depuis `POOL_AGE` préparations (des tailles qui ne reviennent pas : un flou dont le
    /// rayon a fini de bouger).
    fn trim(&mut self) {
        // (Les rangs des textures encore prises changent en jetant : on ne jette que si aucune n'est prise.)
        if !self.pool.iter().all(|s| s.free) {
            return;
        }
        let mut order: Vec<usize> = (0..self.pool.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(self.pool[i].used));
        let (mut keep, mut total) = (vec![false; self.pool.len()], 0u64);
        for i in order {
            let s = &self.pool[i];
            if total + s.bytes() <= POOL_BYTES && self.tick.saturating_sub(s.used) <= POOL_AGE {
                keep[i] = true;
                total += s.bytes();
            }
        }
        let mut k = 0;
        self.pool.retain(|_| {
            k += 1;
            keep[k - 1]
        });
    }

    fn write_params(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, pass: &Pass) -> u32 {
        // Les emplacements ne repartent jamais du début : plusieurs commandes de dessin d'une même image
        // d'écran (deux panneaux) préparent leurs passes avant que la carte n'en exécute une seule, et
        // réécrire un emplacement déjà pris effacerait les réglages de l'autre. Quand le tampon est plein,
        // un neuf le remplace (les passes déjà notées gardent l'ancien en vie).
        if self.slot >= self.capacity {
            let (buffer, group) = param_buffer(device, &self.param_layout, self.stride, self.capacity);
            self.params = buffer;
            self.params_group = group;
            self.slot = 0;
        }
        let mut bytes = [0u8; PARAM_BYTES as usize];
        let mut put = |at: usize, v: &[u8]| bytes[at..at + v.len()].copy_from_slice(v);
        let ints = |g: [i32; 4]| -> Vec<u8> { g.iter().flat_map(|v| v.to_ne_bytes()).collect() };
        put(0, &ints(pass.ga.array()));
        put(16, &ints(pass.gb.array()));
        put(32, &ints(pass.gd.array()));
        put(48, &ints([pass.sheet.0, pass.sheet.1, 0, 0]));
        put(64, &ints(pass.n));
        for (i, v) in pass.f.iter().flatten().enumerate() {
            put(80 + i * 4, &v.to_ne_bytes());
        }
        let offset = self.slot * self.stride;
        queue.write_buffer(&self.params, offset, &bytes);
        self.slot += 1;
        offset as u32
    }

    fn texture_group(&self, device: &wgpu::Device, view: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("effets : texture"),
            layout: &self.tex_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) }],
        })
    }

    /// Exécute `chain` : `input` est la texture que montre `chain`'s première passe, `output` reçoit le
    /// résultat (`None` : une texture de travail est prise, que l'appelant rend avec `release`, et son
    /// rang est renvoyé).
    pub fn run(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, chain: &Chain, input: &wgpu::TextureView, output: Option<&wgpu::TextureView>) -> Option<usize> {
        // La dernière lecture de chaque texture de travail.
        let mut last = vec![0usize; chain.temps.len()];
        for (i, p) in chain.passes.iter().enumerate() {
            for t in [Some(p.a), p.b].into_iter().flatten() {
                if let Tex::Temp(k) = t {
                    last[k] = i;
                }
            }
        }
        let mut held: Vec<Option<usize>> = vec![None; chain.temps.len()];
        let mut leased = None;
        if output.is_none() {
            let g = chain.out;
            let i = self.acquire(device, g.w as u32, g.h as u32);
            held[chain.out_temp] = Some(i);
            leased = Some(i);
        }
        for (i, p) in chain.passes.iter().enumerate() {
            // La cible : la texture du résultat, ou une texture de travail.
            let Tex::Temp(d) = p.dst else { continue };
            if held[d].is_none() && !(d == chain.out_temp && output.is_some()) {
                held[d] = Some(self.acquire(device, p.gd.w as u32, p.gd.h as u32));
            }
            let offset = self.write_params(device, queue, p);
            let view_of = |s: &GpuFx, t: Tex| -> wgpu::TextureView {
                match t {
                    Tex::Input => input.clone(),
                    Tex::Temp(k) => match held[k] {
                        Some(j) => s.pool[j].view.clone(),
                        None => s.empty.clone(),
                    },
                }
            };
            let a_group = self.texture_group(device, &view_of(self, p.a));
            let b_view = p.b.map_or_else(|| self.empty.clone(), |t| view_of(self, t));
            let b_group = self.texture_group(device, &b_view);
            let dst_view = if d == chain.out_temp && output.is_some() { output.expect("sortie").clone() } else { self.pool[held[d].expect("cible")].view.clone() };
            let pipeline = self.pipeline(p.entry);
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(p.entry),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &dst_view,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &self.params_group, &[offset]);
                pass.set_bind_group(1, &a_group, &[]);
                pass.set_bind_group(2, &b_group, &[]);
                pass.draw(0..3, 0..1);
            }
            self.passes_drawn += 1;
            // Les textures de travail qui ne serviront plus retournent à la réserve.
            for k in 0..chain.temps.len() {
                if last[k] == i && k != chain.out_temp {
                    if let Some(j) = held[k].take() {
                        self.pool[j].free = true;
                    }
                }
            }
        }
        // Une texture de travail jamais lue (cible d'une passe sans lecteur) retourne aussi à la réserve.
        for (k, h) in held.iter().enumerate() {
            if k != chain.out_temp {
                if let Some(j) = h {
                    self.pool[*j].free = true;
                }
            }
        }
        if leased.is_none() {
            self.trim();
        }
        leased
    }
}
