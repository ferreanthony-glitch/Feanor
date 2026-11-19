//! Ce qui reste de l'ancien panneau Calques (6c ; le panneau n'existe plus depuis la partie 160, ses commandes sont sur la ligne
//! du calque courant de la timeline et de la bande des plans, et dans la barre d'actions `barrecalques.rs`) : les vignettes des
//! dessins des calques, le renommage d'un calque, la profondeur.

use std::collections::HashMap;

use eframe::egui::{self, TextureHandle};

use crate::app::App;
use crate::document::CelId;

/// Taille des vignettes de calque.
const THUMB: [usize; 2] = [64, 36];

/// Vignettes des dessins (par dessin et version), refaites quand le dessin change. Elles se composent
/// sur des fils de fond (`thumbs.rs`) : l'ancienne reste à l'écran en attendant la neuve, et un dessin
/// qui n'en a pas encore montre un cadre vide.
#[derive(Default)]
pub(crate) struct LayerThumbs {
    cache: HashMap<CelId, LayerEntry>,
    pool: Option<crate::thumbs::Pool>,
}

impl Drop for LayerThumbs {
    /// Un onglet qu'on ferme rend ses fils de vignettes (sans cela ils attendraient pour toujours).
    fn drop(&mut self) {
        if let Some(pool) = self.pool.take() {
            pool.stop();
        }
    }
}

struct LayerEntry {
    /// Version du dessin dont elle vient.
    version: u64,
    texture: TextureHandle,
    /// Dernière image d'écran où elle a servi.
    used: u64,
    /// Numéro de la demande dont elle vient (une réponse plus ancienne ne la remplace pas).
    seq: u64,
    /// L'image envoyée à la carte graphique (essais).
    #[cfg(test)]
    image: egui::ColorImage,
}

impl LayerThumbs {
    /// Nombre de vignettes gardées (une texture chacune) avant de lâcher celles qui ne servent plus.
    const LIMIT: usize = 1500;

    /// Oublie toutes les vignettes (leur résolution a changé : elles se refont à leur tour).
    pub(crate) fn clear(&mut self) {
        self.cache.clear();
        if let Some(pool) = &self.pool {
            pool.shared.reset();
        }
    }

    /// Le projet n'est plus affiché (onglet inactif) : les vignettes finies restent, les demandes en cours
    /// sont abandonnées (`Thumbs::abandonner`).
    pub(crate) fn abandonner(&mut self) {
        if let Some(pool) = &self.pool {
            pool.shared.reset();
        }
    }

    /// Début d'une image d'écran : les demandes qu'on ne renouvelle pas vieillissent, les vignettes finies
    /// arrivent à la carte graphique (`thumbs::UPLOAD_TIME` au plus).
    pub(crate) fn begin_frame(&mut self, ctx: &egui::Context) {
        let Some(pool) = &self.pool else { return };
        let shared = pool.shared.clone();
        shared.new_frame();
        let pass = ctx.cumulative_pass_nr();
        let start = std::time::Instant::now();
        while let Some(done) = shared.next_done() {
            self.apply(ctx, done, pass);
            if start.elapsed() >= crate::thumbs::UPLOAD_TIME {
                break;
            }
        }
        if shared.has_done() {
            ctx.request_repaint();
        }
    }

    fn apply(&mut self, ctx: &egui::Context, done: crate::thumbs::Done, pass: u64) {
        let cel = done.slot.0;
        let crate::thumbs::Key::Cel(version, _, _) = *done.key else { return };
        if let Some(e) = self.cache.get_mut(&cel) {
            if done.seq >= e.seq {
                #[cfg(test)]
                {
                    e.image = done.image.clone();
                }
                e.texture.set(done.image, egui::TextureOptions::LINEAR);
                (e.version, e.used, e.seq) = (version, pass, done.seq);
            }
            return;
        }
        // Trop de vignettes : on garde celles des deux dernières images d'écran (ce qu'on voit) ;
        // tout vider refaisait, à chaque image, toutes celles qui restaient à l'écran.
        if self.cache.len() >= Self::LIMIT {
            self.cache.retain(|_, e| pass.saturating_sub(e.used) <= 2);
            if self.cache.len() >= Self::LIMIT {
                self.cache.clear();
            }
        }
        #[cfg(test)]
        let image = done.image.clone();
        let texture = ctx.load_texture(format!("calque {cel}"), done.image, egui::TextureOptions::LINEAR);
        self.cache.insert(
            cel,
            LayerEntry {
                version,
                texture,
                used: pass,
                seq: done.seq,
                #[cfg(test)]
                image,
            },
        );
    }

    /// La vignette du dessin `cel` (`c`) : celle qu'on garde si elle est à jour ; sinon l'ancienne en
    /// attendant (ou rien), et une demande aux fils de fond.
    pub(crate) fn texture(&mut self, ctx: &egui::Context, cel: CelId, c: &crate::document::Cel, fr: egui::Rect, size: (usize, usize), pass: u64) -> Option<TextureHandle> {
        if let Some(e) = self.cache.get_mut(&cel) {
            e.used = pass;
            if e.version == c.version {
                return Some(e.texture.clone());
            }
        }
        let pool = self.pool.get_or_insert_with(|| crate::thumbs::Pool::start(ctx, 2));
        pool.request((cel, 0), crate::thumbs::Key::Cel(c.version, size.0, size.1), |_| crate::thumbs::Work::Cel { raster: c.raster.clone(), frame: fr, size });
        self.cache.get(&cel).map(|e| e.texture.clone())
    }

    /// Vignettes gardées (essais).
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.cache.len()
    }

    /// Des vignettes se font encore, ou attendent d'être envoyées (essais).
    #[cfg(test)]
    pub(crate) fn busy(&self) -> bool {
        self.pool.as_ref().is_some_and(|p| p.shared.outstanding() > 0)
    }

    /// L'image de la vignette gardée du dessin `cel` (essais).
    #[cfg(test)]
    pub(crate) fn image_of(&self, cel: CelId) -> Option<&egui::ColorImage> {
        self.cache.get(&cel).map(|e| &e.image)
    }
}

/// Renommage d'un calque en cours : le calque, le nom tapé, le champ a déjà pris le clavier, et
/// où est le champ (en-tête de la timeline, 13b ; colonne de gauche du montage ; sinon panneau
/// Calques).
pub(crate) struct LayerRename {
    pub id: u64,
    pub name: String,
    pub focused: bool,
    pub in_timeline: bool,
    pub in_montage: bool,
}

impl LayerRename {
    pub(crate) fn new(id: u64, name: &str, in_timeline: bool) -> Self {
        Self { id, name: name.to_string(), focused: false, in_timeline, in_montage: false }
    }
}

impl App {
    /// Vignette du dessin `cel` : la partie filmée (le cadre), sur fond blanc.
    pub(crate) fn layer_thumb(&mut self, ctx: &egui::Context, cel: CelId) -> Option<TextureHandle> {
        let c = self.doc.cels.get(&cel)?;
        let fr = self.doc.frame_rect();
        let [w, h] = THUMB;
        // (Résolution des vignettes du diagnostic des performances : définition plus basse, même
        // taille à l'écran.)
        let w = crate::thumbs::scaled(w, crate::thumbs::scale_pct(), 16);
        let h = ((w as f32 * fr.height() / fr.width().max(1.0)).round() as usize).clamp(8, h * 2);
        // (Composée sur un fil de fond, `thumbs.rs` : rien ici ne parcourt les pixels du dessin.)
        self.layer_thumbs.texture(ctx, cel, c, fr, (w, h), ctx.cumulative_pass_nr())
    }

    /// Calques de toute la scène accordés d'un panneau à l'autre (6f), à chaque image ; le
    /// calque courant reste le même.
    pub(crate) fn sync_scene_layers(&mut self) {
        let id = self.layers().get(self.cur_layer).map(|l| l.id);
        // (Les calques partagés entre plans, la planche-mère, s'accordent de même : `partage.rs`.)
        let partages = self.doc.sync_partages(self.cur_panel);
        if self.doc.sync_scene_layers(self.cur_panel, self.cur_frame) || partages {
            if let Some(i) = id.and_then(|id| self.layers().iter().position(|l| l.id == id)) {
                self.cur_layer = i;
            }
            self.clamp_nav();
            self.renderer.invalidate_all();
        }
    }

    // (Le panneau Calques n'existe plus depuis la partie 160 : ses commandes sont sur la ligne du calque courant de la timeline et
    // de la colonne de la bande des plans, et dans la barre d'actions sous la colonne des calques, `barrecalques.rs`.)

    /// Champ du nouveau nom du calque en cours de renommage ; renvoie `Some(true)` quand on le
    /// valide (Entrée, clic ailleurs), `Some(false)` quand on abandonne (Échap).
    pub(crate) fn layer_rename_field(&mut self, ui: &mut egui::Ui, width: f32) -> Option<bool> {
        let r = self.layer_rename.as_mut()?;
        let mut out = egui::TextEdit::singleline(&mut r.name).desired_width(width).show(ui);
        let resp = out.response.clone();
        if !r.focused {
            // Tout le nom choisi : on tape le nouveau directement.
            resp.request_focus();
            let n = r.name.chars().count();
            out.state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(n),
            )));
            out.state.store(ui.ctx(), resp.id);
            r.focused = true;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            Some(false)
        } else if resp.lost_focus() {
            Some(true)
        } else {
            None
        }
    }

    /// Fin du renommage : `Some(true)` pose le nom, `Some(false)` l'abandonne.
    pub(crate) fn finish_layer_rename(&mut self, done: Option<bool>) {
        match done {
            Some(true) => self.commit_layer_rename(),
            Some(false) => self.layer_rename = None,
            None => {}
        }
    }

    /// Pose le nouveau nom du calque en cours de renommage (une étape d'annulation).
    fn commit_layer_rename(&mut self) {
        let Some(LayerRename { id, name, .. }) = self.layer_rename.take() else { return };
        let name = name.trim().to_string();
        let Some(li) = self.layers().iter().position(|l| l.id == id) else { return };
        if !name.is_empty() && self.layers()[li].name != name {
            self.record_step();
            self.layers_mut()[li].name = name;
        }
    }

    /// (Le menu d'un calque du clic droit est `layer_head_menu` dans l'en-tête de la timeline, `montage_layer_menu` dans
    /// le montage et `layer_full_menu` dans le panneau Calques ; ils partagent ces entrées.)
    ///
    /// « Renommer… » (le champ vient dans la timeline, la colonne du montage ou le panneau).
    pub(crate) fn rename_item(&mut self, ui: &mut egui::Ui, li: usize, in_timeline: bool, in_montage: bool) {
        let Some((id, name)) = self.layers().get(li).map(|l| (l.id, l.name.clone())) else { return };
        if ui.button(format!("{}  Renommer…", egui_phosphor::regular::PENCIL_SIMPLE)).on_hover_text("Double-clic sur le nom du calque : la même chose.").clicked() {
            ui.close_menu();
            self.layer_rename = Some(LayerRename { in_montage, ..LayerRename::new(id, &name, in_timeline) });
        }
    }

    /// Profondeur du calque (multiplan).
    pub(crate) fn depth_item(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some((mut depth, fixed)) = self.layers().get(li).map(|l| (l.depth, l.position_locked)) else { return };
        // Plan en Espace 3D (partie 148) : la profondeur est libre, une carte peut être derrière la caméra d'origine du plan.
        // Le champ descend donc sous la borne des plans ordinaires : borné, il ramènerait la carte à sa borne rien qu'en
        // se montrant (un champ écrit la valeur qu'il a dû borner), sans qu'on y touche.
        // (Partie 159 : la borne de toujours, sauf pour un calque que la vue de la scène a déjà posé au-delà.)
        let plus_pres = if depth < crate::document::Layer::MIN_DEPTH { -crate::document::Layer::MAX_DEPTH_LIBRE } else { crate::document::Layer::MIN_DEPTH };
        let resp = ui
            .horizontal(|ui| {
                ui.label("Profondeur");
                // Position verrouillée (13b) : la profondeur ne bouge plus non plus.
                ui.add_enabled(
                    !fixed,
                    egui::DragValue::new(&mut depth).speed(0.01).range(plus_pres..=1000.0).max_decimals(2),
                )
            })
            .inner
            .on_hover_text(
                "Distance du calque quand la caméra se déplace (multiplan) :\n\
                 0 = plan de référence, défile avec la caméra ;\n\
                 1 = deux fois plus loin, défile deux fois moins vite ;\n\
                 négatif = premier plan, défile plus vite.",
            );
        if self.edited(&resp) {
            self.layers_mut()[li].depth = depth;
            // (Une carte aimantée au sol y reste à sa nouvelle profondeur, partie 182.)
            self.aimanter_si(li);
        }
    }
}
