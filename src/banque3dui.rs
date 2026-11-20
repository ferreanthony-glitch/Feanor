//! La banque 3D dans la Bibliothèque (partie 181) : l'onglet « 3D » du panneau, à côté de « Fichiers ». Les 45 modèles de
//! Kenney (`banque3d.rs`) en vignettes, rangés par famille (Nature, Routes, Ville rétro, Banlieue, Commerces), une recherche,
//! une bulle (nom, triangles, kit, « CC0 · Kenney »). Un clic pose le modèle dans le décor 3D de la séquence du plan courant,
//! devant la caméra ; on peut aussi le glisser sur le dessin (devant la caméra) ou sur la vue Espace 3D (au point du sol sous le
//! pointeur). Dans les deux cas, le même chemin qu'un fichier importé (`App::decor3d_lancer` : lecture, budget de la machine
//! pour tout le décor, fenêtre « Ce décor est lourd »).
//!
//! **Sans latence** : les aperçus (512 × 512 dans les kits) se décodent et se réduisent sur un autre fil, à la première
//! ouverture de l'onglet, jamais au lancement ; l'interface ne fait que poser les textures prêtes.

use std::thread::JoinHandle;

use eframe::egui::{self, vec2, Color32, RichText, Sense, TextureHandle};

use crate::app::App;
use crate::theme;

/// Côté des aperçus réduits (pixels).
const APERCU_PX: u32 = 128;
/// Côté d'une vignette à l'écran (points), et la hauteur du nom dessous.
const VIGNETTE: f32 = 76.0;
const NOM_H: f32 = 16.0;

/// La marge laissée autour du modèle cadré dans son aperçu (part du côté).
pub(crate) const MARGE_APERCU: f32 = 0.08;

/// Partie 198.5 : l'aperçu `img` cadré sur ce qu'il dessine (les pixels qui ne sont ni transparents ni de la couleur du fond, celle
/// du coin), en un carré avec une marge de `MARGE_APERCU` de chaque côté : le modèle remplit sa case. Rien de dessiné : tel quel.
pub(crate) fn cadrer(img: &image::RgbaImage) -> image::RgbaImage {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return img.clone();
    }
    let fond = *img.get_pixel(0, 0);
    let dessine = |p: &image::Rgba<u8>| {
        if p[3] <= 24 {
            return false;
        }
        fond[3] <= 24 || (0..3).any(|k| (p[k] as i32 - fond[k] as i32).abs() > 24)
    };
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
    for (x, y, p) in img.enumerate_pixels() {
        if dessine(p) {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    if x1 < x0 || y1 < y0 {
        return img.clone();
    }
    let cote = ((x1 - x0 + 1).max(y1 - y0 + 1) as f32 * (1.0 + 2.0 * MARGE_APERCU)).ceil() as u32;
    let (cx, cy) = ((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0);
    let ox = (cx - cote as f32 / 2.0).round() as i64;
    let oy = (cy - cote as f32 / 2.0).round() as i64;
    let mut out = image::RgbaImage::from_pixel(cote, cote, if fond[3] <= 24 { image::Rgba([0, 0, 0, 0]) } else { fond });
    for y in 0..cote {
        for x in 0..cote {
            let (sx, sy) = (ox + x as i64, oy + y as i64);
            if sx >= 0 && sy >= 0 && (sx as u32) < w && (sy as u32) < h {
                out.put_pixel(x, y, *img.get_pixel(sx as u32, sy as u32));
            }
        }
    }
    out
}

/// L'onglet du panneau Bibliothèque.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum OngletBiblio {
    #[default]
    Fichiers,
    /// La banque 3D.
    Banque3d,
}

/// Un modèle de la banque glissé (la charge du glisser-déposer d'egui) : son rang dans le catalogue.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Banque3dDrag {
    pub rang: usize,
}

/// L'état de l'onglet (dans `BiblioUi` : le même dans tous les onglets de projet).
#[derive(Default)]
pub(crate) struct Banque3dUi {
    pub recherche: String,
    /// Les aperçus réduits, en cours de décodage.
    fil: Option<JoinHandle<Vec<Option<egui::ColorImage>>>>,
    /// Les textures des aperçus, une par modèle (rien : aperçu illisible).
    textures: Option<Vec<Option<TextureHandle>>>,
}

impl Banque3dUi {
    /// Lance le décodage des aperçus (une fois).
    fn demarrer(&mut self) {
        if self.fil.is_some() || self.textures.is_some() {
            return;
        }
        self.fil = Some(std::thread::spawn(|| {
            crate::banque3d::modeles()
                .iter()
                .map(|m| {
                    let img = image::load_from_memory_with_format(m.png()?, image::ImageFormat::Png).ok()?;
                    // (Partie 198.5, vu à l'écran : le modèle n'occupait que quelques pixels au milieu de sa case. Il est cadré sur
                    // ce qu'il dessine, avec une marge, avant d'être réduit.)
                    let cadre = cadrer(&img.to_rgba8());
                    let petit = image::DynamicImage::ImageRgba8(cadre).thumbnail(APERCU_PX, APERCU_PX).to_rgba8();
                    Some(egui::ColorImage::from_rgba_unmultiplied([petit.width() as usize, petit.height() as usize], petit.as_raw()))
                })
                .collect()
        }));
    }

    /// Pose les textures des aperçus décodés ; vrai si on attend encore.
    fn relever(&mut self, ctx: &egui::Context) -> bool {
        if self.fil.as_ref().is_some_and(|h| h.is_finished()) {
            let images = self.fil.take().and_then(|h| h.join().ok()).unwrap_or_default();
            self.textures = Some(images.into_iter().enumerate().map(|(i, img)| img.map(|img| ctx.load_texture(format!("banque3d-{i}"), img, egui::TextureOptions::LINEAR))).collect());
        }
        self.fil.is_some()
    }
}

impl App {
    /// Pose le modèle `rang` de la banque dans le décor 3D de la séquence du plan courant : devant la caméra, ou au point du sol
    /// `xz` (lâché sur la vue Espace 3D). Le même chemin qu'un fichier importé.
    pub(crate) fn decor3d_ajouter_banque(&mut self, rang: usize, xz: Option<[f32; 2]>) {
        if self.sans_projet() || self.content_refuses() {
            return;
        }
        let Some(m) = crate::banque3d::modeles().get(rang) else { return };
        let nom = m.nom_lisible();
        let mut cible = self.decor3d_cible_ici();
        cible.xz = xz;
        if self.decor3d_lancer(cible, move || crate::banque3d::analyser(rang)) && self.decor3d_budget().is_some() {
            self.status = format!("Banque 3D : « {nom} » dans le décor de la séquence…");
        }
    }

    /// Un modèle de la banque glissé, lâché en `pos` : sur la vue Espace 3D, au point du sol sous le pointeur ; sur le dessin,
    /// devant la caméra ; ailleurs, rien (la barre d'état le dit).
    pub(crate) fn banque3d_lacher(&mut self, rang: usize, pos: egui::Pos2) {
        if let Some(xz) = self.decor3d_sol_sous(pos) {
            self.decor3d_ajouter_banque(rang, Some(xz));
        } else if self.canvas_rect.contains(pos) || (self.espace3d_vue_ouverte() && self.espace3d.rect.contains(pos)) {
            self.decor3d_ajouter_banque(rang, None);
        } else {
            self.status = "Lâche le modèle 3D sur le dessin ou sur la vue Espace 3D.".into();
        }
    }

    /// Un modèle glissé depuis la banque, lâché hors du panneau (à chaque image, panneau ouvert ou non).
    pub(crate) fn poll_banque3d(&mut self, ctx: &egui::Context) {
        if let Some(d) = egui::DragAndDrop::payload::<Banque3dDrag>(ctx) {
            if ctx.input(|i| i.pointer.any_released()) {
                let pos = ctx.input(|i| i.pointer.interact_pos().or(i.pointer.latest_pos()));
                let _ = egui::DragAndDrop::take_payload::<Banque3dDrag>(ctx);
                let sur_la_bibliotheque = pos.zip(self.biblio.rect).is_some_and(|(p, r)| r.contains(p));
                if let (Some(pos), false) = (pos, sur_la_bibliotheque) {
                    if !self.modal_open() {
                        self.banque3d_lacher(d.rang, pos);
                    }
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        if self.biblio.banque.fil.is_some() && self.biblio.banque.relever(ctx) {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }

    /// L'onglet « 3D » du panneau Bibliothèque.
    pub(crate) fn banque3d_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.biblio.banque.demarrer();
        let attend = self.biblio.banque.relever(&ctx);
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.biblio.banque.recherche).hint_text("Chercher un modèle (arbre, route, maison…)").desired_width(f32::INFINITY));
        });
        let familles = crate::banque3d::par_famille(&self.biblio.banque.recherche);
        let n: usize = familles.iter().map(|(_, r)| r.len()).sum();
        ui.label(RichText::new(format!("{n} modèle{} · clic : dans le décor 3D de la séquence, devant la caméra ; glisser : sur le dessin ou la vue Espace 3D", if n > 1 { "s" } else { "" })).small().weak());
        if familles.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("Aucun modèle ne correspond.").weak());
            return;
        }
        let mut poser: Option<usize> = None;
        let mut glisser: Option<usize> = None;
        let textures = self.biblio.banque.textures.clone();
        egui::ScrollArea::vertical().id_salt("banque3d").auto_shrink([false, false]).show(ui, |ui| {
            for (famille, rangs) in &familles {
                egui::CollapsingHeader::new(RichText::new(format!("{famille} ({})", rangs.len())).strong()).id_salt(("banque3d_famille", famille.as_str())).default_open(true).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                        for &i in rangs {
                            let m = &crate::banque3d::modeles()[i];
                            let (rect, resp) = ui.allocate_exact_size(vec2(VIGNETTE, VIGNETTE + NOM_H), Sense::click_and_drag());
                            let image = egui::Rect::from_min_size(rect.min, vec2(VIGNETTE, VIGNETTE));
                            let p = ui.painter();
                            let fond = if resp.hovered() { theme::texte_doux().gamma_multiply(0.25) } else { Color32::from_black_alpha(40) };
                            p.rect_filled(image, 4.0, fond);
                            match textures.as_ref().and_then(|t| t.get(i)).and_then(|t| t.as_ref()) {
                                Some(t) => {
                                    p.image(t.id(), image.shrink(3.0), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                                }
                                None if attend => {
                                    p.text(image.center(), egui::Align2::CENTER_CENTER, "…", egui::FontId::proportional(14.0), theme::texte_doux());
                                }
                                None => {}
                            }
                            let nom = egui::Rect::from_min_size(egui::pos2(rect.left(), image.bottom()), vec2(VIGNETTE, NOM_H));
                            p.with_clip_rect(nom).text(nom.center(), egui::Align2::CENTER_CENTER, m.nom_lisible(), egui::FontId::proportional(10.5), theme::texte_doux());
                            let resp = resp.on_hover_text(m.bulle());
                            if resp.drag_started() {
                                glisser = Some(i);
                            } else if resp.clicked() {
                                poser = Some(i);
                            }
                        }
                    });
                });
            }
        });
        if let Some(i) = glisser {
            egui::DragAndDrop::set_payload(&ctx, Banque3dDrag { rang: i });
        }
        if let Some(i) = poser {
            self.decor3d_ajouter_banque(i, None);
        }
        // Le nom du modèle glissé suit le pointeur.
        if let Some(d) = egui::DragAndDrop::payload::<Banque3dDrag>(&ctx) {
            if let (Some(p), Some(m)) = (ctx.pointer_latest_pos(), crate::banque3d::modeles().get(d.rang)) {
                egui::Area::new(egui::Id::new("banque3d_glisse")).order(egui::Order::Tooltip).fixed_pos(p + vec2(14.0, 10.0)).interactable(false).show(&ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(format!("{} {}", egui_phosphor::regular::CUBE, m.nom_lisible()));
                    });
                });
            }
        }
    }
}
