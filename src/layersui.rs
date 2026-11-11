//! Panneau Calques (6c) : chaque calque avec la vignette de son dessin à l'image courante,
//! visibilité et verrou, opacité du calque courant en haut, commandes en icônes en bas, réglages
//! du calque courant dessous.

use std::collections::HashMap;

use eframe::egui::{self, vec2, Color32, RichText, Sense, Stroke, StrokeKind, TextureHandle};

use crate::app::App;
use crate::blend::BlendMode;
use crate::document::CelId;

/// Taille des vignettes de calque.
const THUMB: [usize; 2] = [64, 36];

/// La place du filet du dossier, tout à gauche de chaque rang (Anthony, 2026-11-11 : « les bandes de couleur du dossier dans le panneau
/// Calques également »). Comme dans la timeline, la couleur du dossier et celle de la sorte du calque ne sont jamais au même endroit :
/// le filet du dossier est tout à gauche, la bande de la sorte vient juste après. La place n'est prise que dans un plan qui a au
/// moins un dossier (ou un calque rangé dans un groupe de couleur) : ailleurs, les rangs restent comme avant.
const GOUTTIERE: f32 = 6.0;

/// La largeur du filet du dossier (celle du filet de la colonne des pistes, `pistes::montage_head`).
const FILET: f32 = 3.0;

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

    pub(crate) fn layers_panel(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        // Tout le film est écarté : le plan vide qui le tient n'a pas de calque à montrer (`planannule.rs`).
        if self.panneau_film_vide(ui) {
            return;
        }
        // Un clic dans le panneau lui donne la main : après une copie d'effets, Ctrl+V les colle sur le
        // calque cliqué (`fxcopie.rs`).
        if ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(ui.clip_rect()) {
            self.take_layers_pane_focus();
        }
        // Les commandes, en haut et toujours visibles (Anthony, 2026-10-17 : « la zone [des
        // boutons] doit rester en haut ») ; seule la liste défile.
        self.layer_commands(ui);
        ui.add_space(2.0);
        // Mode de fusion et opacité : ceux du seul calque courant (comme en haut d'une liste de calques),
        // pas un par ligne.
        let li = self.cur_layer;
        // (L'opacité à l'image courante : celle que donnent les clés d'opacité du calque, `opacitecles.rs`.)
        let mut opacity = self.layers()[li].opacity_at(self.cur_frame) * 100.0;
        let mut mode = self.layers()[li].blend;
        // (À la ligne si le panneau est étroit ; la liste se resserre.)
        let combo_w = (ui.available_width() - 90.0).clamp(60.0, 120.0);
        ui.horizontal_wrapped(|ui| {
            // Mode de fusion (8), comme dans les logiciels de retouche.
            let resp = egui::ComboBox::from_id_salt("fusion_calque")
                .selected_text(mode.label())
                .width(combo_w)
                .show_ui(ui, |ui| {
                    for m in BlendMode::ALL {
                        // Familles : normal, assombrir, éclaircir, contraste, écarts, couleur.
                        if BlendMode::FAMILY_STARTS.contains(&m) {
                            ui.separator();
                        }
                        ui.selectable_value(&mut mode, m, m.label()).on_hover_text(m.hint());
                    }
                })
                .response;
            crate::bulles::bulle(resp, "Mode de fusion", mode.hint(), "");
            crate::bulles::etiquette(ui, ph::CIRCLE_HALF, "Opacité du calque", "");
            let resp = ui.add(egui::DragValue::new(&mut opacity).range(0.0..=100.0).max_decimals(0).suffix(" %").speed(0.5));
            let resp = crate::bulles::bulle(resp, "Opacité du calque", "Glisser ou taper une valeur.", "");
            if self.edited(&resp) {
                // Sans clé : l'opacité fixe ; avec des clés : la clé de l'image courante (posée au besoin). Plusieurs calques
                // choisis : tous prennent cette valeur (`choixtout.rs`).
                self.regler_opacite(li, opacity / 100.0);
            }
            // « Créer une clé » : le losange des clés d'opacité, comme celui des effets.
            self.op_keys_buttons(ui);
        });
        // Calque Couleur unie : sa couleur, avec les réglages du calque.
        self.couleur_unie_ui(ui);
        if mode != self.layers()[li].blend {
            self.regler_mode_calques(li, mode);
        }
        ui.add_space(2.0);
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("liste_calques")
            .auto_shrink([false, false])
            .show(ui, |ui| self.layer_list(ui));
    }

    /// Les commandes des calques : cinq icônes (le « + » des nouveaux calques, dupliquer, supprimer, monter,
    /// descendre) et « … » pour tout le reste (passe de simplicité, 2026-10-21). Copier, coller, fusionner, l'aplat,
    /// l'interrupteur des effets, les dossiers, les groupes… y sont, et dans le clic droit d'un calque ; toutes ces
    /// commandes restent aussi dans la palette de commandes (Ctrl+K).
    fn layer_commands(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            self.layer_plus_button(ui);
            if self
                .bouton(ui, ph::COPY_SIMPLE, "Dupliquer les calques choisis", "Une copie de chaque calque choisi et de ses dessins, juste au-dessus de lui.", "calque.dupliquer")
                .clicked()
            {
                self.duplicate_chosen_layers();
            }
            if self.bouton(ui, ph::TRASH, "Supprimer les calques choisis", "Le calque courant et ceux choisis par Ctrl + clic dans la timeline.", "calque.supprimer").clicked() {
                self.delete_chosen_layers();
            }
            if self.bouton(ui, ph::ARROW_UP, "Monter le calque", "D'un rang vers le haut.", "calque.monter").clicked() {
                self.move_layer(true);
            }
            if self.bouton(ui, ph::ARROW_DOWN, "Descendre le calque", "D'un rang vers le bas.", "calque.descendre").clicked() {
                self.move_layer(false);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| self.layer_more_button(ui));
        });
    }

    /// Le bouton « … » : ce qui n'a plus son icône (copier, coller, fusionner, aplat, effets, dossiers…).
    fn layer_more_button(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        let button = egui::Button::new(RichText::new(ph::DOTS_THREE).size(crate::bulles::TAILLE)).min_size(vec2(24.0, 0.0));
        let resp = egui::menu::menu_custom_button(ui, button, |ui| self.layer_more_menu(ui)).response;
        crate::bulles::bulle(
            resp,
            "Plus de commandes",
            "Copier, coller, fusionner, aplat sous le trait, effets du calque, dossiers, groupes… Le clic droit sur un calque donne les mêmes.",
            "",
        );
    }

    /// La liste : un rang par calque, le plus haut en premier. Clic sur le rang : il devient
    /// courant ; glisser : il monte ou descend ; clic droit : tout ce qui touche au calque
    /// (`layer_full_menu`) ; double-clic sur le nom : renommer.
    fn layer_list(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        // Le calque cliqué, et comment : Ctrl l'ajoute ou le retire du choix, Maj choisit ceux
        // d'entre (pour fusionner les calques choisis, Anthony, 2026-10-17) ; clic droit.
        let mut select: Option<(usize, bool)> = None;
        let mods = ui.input(|i| i.modifiers);
        let mut toggle = None;
        let mut lock = None;
        let mut rename_done: Option<bool> = None;
        // Sous-ligne d'aplat cliquée : sa cible du dessin.
        let mut aplat_choisi: Option<usize> = None;
        let lock_key = self.raccourci("calque.verrouiller");
        let frame = self.cur_frame;
        let accent = crate::theme::accent();
        // Vignette plus petite dans un panneau étroit : le nom reste lisible.
        let thumb_w = (ui.available_width() * 0.3).clamp(32.0, THUMB[0] as f32);
        let thumb_size = vec2(thumb_w, thumb_w * THUMB[1] as f32 / THUMB[0] as f32);
        // Glisser une ligne la monte ou la descend (passe interface) : les lignes de cette image,
        // pour savoir où elle se pose.
        let list_top = ui.cursor().top();
        let mut drag_rows: Vec<crate::layerheads::Row> = Vec::new();
        let several = self.chosen_layers().len() >= 2;
        // Dossiers fermés (comme dans la timeline : `collapsed`) : leurs calques n'ont plus de rang ici,
        // sauf le calque courant, qu'on ne perd pas de vue.
        let closed: Vec<u64> = self.layers().iter().filter(|l| l.folder && l.collapsed).map(|l| l.id).collect();
        let mut fold: Option<usize> = None;
        // Le filet du dossier (`folders::couleur_du_rangement`, la même couleur que dans la timeline) : il court du rang du dossier
        // jusqu'à son dernier calque, sans coupure entre les rangs. `filet_d_avant` : le dossier du rang d'avant et son bas.
        let filets: Vec<Option<Color32>> = (0..self.layers().len()).map(|i| crate::folders::couleur_du_rangement(self.layers(), i, &self.doc.groups)).collect();
        let gouttiere = if filets.iter().any(Option::is_some) { GOUTTIERE } else { 0.0 };
        let mut filet_d_avant: Option<(usize, f32)> = None;
        // Le menu du clic droit d'un rang se montre au milieu des rangs : s'il change les calques (en supprimer, en fusionner,
        // en ajouter), les rangs d'après ne désignent plus les mêmes calques. La liste s'arrête là, les clics de cette image (qui
        // visaient les rangs d'avant) sont lâchés ; l'image suivante refait la liste (passe de débogage, 2026-10-22).
        let avant = self.empreinte_calques();
        let mut tient = true;
        for li in (0..self.layers().len()).rev() {
            if !self.calques_tiennent(&avant, li) {
                tient = false;
                break;
            }
            let (id, visible, locked, kind, name, cel, folder, couleur, has_fond) = {
                let l = &self.layers()[li];
                (l.id, l.visible, l.locked, l.kind, l.name.clone(), l.cel_at(frame), l.folder, l.couleur.is_some(), l.fond.is_some())
            };
            let (in_folder, shut, kids) = {
                let l = &self.layers()[li];
                (l.parent.is_some(), l.folder && l.collapsed, if l.folder { crate::folders::children_of(self.layers(), li).len() } else { 0 })
            };
            if self.layers()[li].parent.is_some_and(|p| closed.contains(&p)) && li != self.cur_layer {
                continue;
            }
            // Calque de couleur : rangé sous son trait, en retrait ; calque de trait : le nombre de
            // couleurs qu'il porte (`calquecouleur.rs`).
            let colour_of = crate::calquecouleur::trait_de(self.layers(), li).map(|ti| self.layers()[ti].name.clone());
            let colours = crate::calquecouleur::couleurs_de(self.layers(), li).len();
            // Code couleur de sa sorte (`theme.rs`).
            let cfx = self.layers()[li].effet.is_some();
            let hue = if folder {
                accent
            } else if cfx {
                crate::theme::palette().dessin_cfx
            } else if self.layers()[li].annotation {
                crate::theme::palette().dessin_annotation
            } else if self.layers()[li].unie.is_some() {
                crate::theme::palette().dessin_unie
            } else {
                crate::theme::couleur_calque(kind, couleur)
            };
            let current = li == self.cur_layer;
            let chosen = !current && self.is_chosen_layer(li);
            let aplat_vise = current && self.aplat_en_cible();
            // Atelier, Papier et Gris sombre : le calque courant prend le fond plein de sa sorte (jade sombre pour le vectoriel), avec un texte qui s'y lit.
            let marque = crate::theme::palette();
            let plein = ((marque.identite || marque.jade_discret) && current && !aplat_vise).then(|| {
                if folder {
                    (marque.selection, marque.sur_selection)
                } else {
                    (crate::theme::fond_du_calque(&self.layers()[li]), marque.sur_sorte)
                }
            });
            let thumb =cel.and_then(|c| self.layer_thumb(ui.ctx(), c));
            // Le rang entier, posé avant ses boutons (à sa place de l'image d'écran d'avant) : les
            // boutons restent au-dessus et gardent leurs clics (posé après eux, ce fond qui se
            // glisse les rendait tous muets : œil, verrou, clic droit, 2026-10-17).
            let grip_id = ui.id().with(("glisser_calque", id));
            let grip = ui.ctx().data(|d| d.get_temp::<egui::Rect>(grip_id)).map(|r| ui.interact(r, grip_id, Sense::click_and_drag()));
            let shown = egui::Frame::new()
                .fill(match (current, chosen) {
                    // (Quand la sous-ligne de l'aplat est la cible du dessin, c'est elle qui est surlignée en plein.)
                    _ if plein.is_some() => plein.map_or(Color32::TRANSPARENT, |p| p.0),
                    (true, _) if aplat_vise => hue.gamma_multiply(0.25),
                    (true, _) => hue.gamma_multiply(0.55),
                    (false, true) => hue.gamma_multiply(0.25),
                    (false, false) => Color32::TRANSPARENT,
                })
                .corner_radius(3.0)
                .inner_margin(egui::Margin::symmetric(3, 2))
                .show(ui, |ui| {
                    // Les textes du rang ne se sélectionnent pas à la souris : pris par son nom, le calque se glisse (sans cela, un
                    // glissé parti du nom sélectionnait ses lettres et le calque ne bougeait pas ; Anthony, 2026-11-11 : « glisser
                    // des calques dans un dossier ne fonctionne pas »).
                    ui.style_mut().interaction.selectable_labels = false;
                    if let Some((fond, encre)) = plein {
                        let v = ui.visuals_mut();
                        v.override_text_color = Some(encre);
                        // (Le texte « faible » se mêle à ce fond-là, pas à celui du panneau.)
                        v.widgets.noninteractive.weak_bg_fill = fond;
                        for etat in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
                            etat.fg_stroke.color = encre;
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(3.0, 0.0);
                        if gouttiere > 0.0 {
                            ui.add_space(gouttiere);
                        }
                        if colour_of.is_some() {
                            ui.add_space(14.0);
                        }
                        // Dossier : son triangle l'ouvre et le ferme ; un calque rangé dedans est décalé.
                        if folder {
                            let caret = if shut { ph::CARET_RIGHT } else { ph::CARET_DOWN };
                            let resp = ui.add(egui::Button::new(RichText::new(caret).size(13.0)).frame(false).min_size(vec2(16.0, 36.0)));
                            let (what, why) = if shut { ("Ouvrir le dossier", "Montrer ses calques.") } else { ("Fermer le dossier", "Cacher ses calques ici et dans la timeline (ils restent à l'image).") };
                            if crate::bulles::bulle(resp, what, why, "").clicked() {
                                fold = Some(li);
                            }
                        } else if in_folder {
                            ui.add_space(14.0);
                        }
                        let eye = if visible { ph::EYE } else { ph::EYE_SLASH };
                        let resp = ui.add(egui::Button::new(RichText::new(eye).size(15.0)).frame(false).min_size(vec2(20.0, 36.0)));
                        let resp = crate::bulles::bulle(resp, "Afficher / masquer", "Montrer ou cacher le calque.", "");
                        if resp.clicked() {
                            toggle = Some(li);
                        }
                        resp.context_menu(|ui| self.layer_full_menu(ui, li, false));
                        let pad = if locked { ph::LOCK_SIMPLE } else { ph::LOCK_SIMPLE_OPEN };
                        let pad = RichText::new(pad).size(14.0);
                        let pad = if locked { pad } else { pad.weak() };
                        let resp = ui.add(egui::Button::new(pad).frame(false).min_size(vec2(18.0, 36.0)));
                        let resp = crate::bulles::bulle(resp, "Verrouiller / déverrouiller", "Un calque verrouillé ne peut plus être dessiné.", &lock_key);
                        if resp.clicked() {
                            lock = Some(li);
                        }
                        resp.context_menu(|ui| self.layer_full_menu(ui, li, false));
                        // Bouton « fx » : les effets du calque, d'un coup (seulement s'il en a ; `fxui.rs`).
                        let ink = ui.visuals().text_color();
                        if let Some(resp) = self.fx_switch(ui, li, None, ink) {
                            if resp.clicked() {
                                self.toggle_layer_fx(li);
                            }
                        }
                        // Vignette du dessin à l'image courante (vide : cadre gris).
                        let (rect, resp) = ui.allocate_exact_size(thumb_size, Sense::click());
                        match &thumb {
                            Some(t) => {
                                ui.painter().image(
                                    t.id(),
                                    rect,
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    if visible { Color32::WHITE } else { Color32::from_gray(120) },
                                );
                            }
                            None => {
                                ui.painter().rect_filled(rect, 0.0, crate::theme::controle());
                                // Un dossier n'a pas de dessin : son icône à la place de la vignette.
                                // (De la couleur du dossier, celle de son filet et de sa ligne dans la timeline.)
                                if folder {
                                    let icon = if shut { ph::FOLDER } else { ph::FOLDER_OPEN };
                                    let encre = match filets.get(li).copied().flatten() {
                                        Some(c) if visible => c,
                                        Some(c) => c.gamma_multiply(0.5),
                                        None if visible => crate::theme::texte(),
                                        None => crate::theme::texte_doux(),
                                    };
                                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional(18.0), encre);
                                }
                            }
                        }
                        let border = if current { Stroke::new(2.0_f32, crate::theme::contour()) } else { Stroke::new(1.0_f32, crate::theme::bord()) };
                        ui.painter().rect_stroke(rect, 0.0, border, StrokeKind::Inside);
                        if resp.clicked() || resp.secondary_clicked() {
                            select = Some((li, resp.secondary_clicked()));
                        }
                        resp.context_menu(|ui| self.layer_full_menu(ui, li, false));
                        ui.vertical(|ui| {
                            let renaming = self.layer_rename.as_ref().is_some_and(|r| r.id == id && !r.in_timeline && !r.in_montage);
                            if renaming {
                                rename_done = self.layer_rename_field(ui, 120.0);
                            } else {
                                let text = if visible { RichText::new(&name) } else { RichText::new(&name).weak() };
                                let resp = ui.add(egui::Label::new(text).sense(Sense::click()).truncate());
                                if resp.clicked() || resp.secondary_clicked() {
                                    select = Some((li, resp.secondary_clicked()));
                                }
                                if resp.double_clicked() {
                                    self.layer_rename = Some(LayerRename::new(id, &name, false));
                                }
                                resp.on_hover_text("Double-clic : renommer ; clic droit : tout ce qui touche au calque").context_menu(|ui| self.layer_full_menu(ui, li, false));
                            }
                            // La sorte du calque : une petite icône (vectoriel, bitmap, couleur, effet, vidéo, dossier)
                            // avec sa bulle, plus le mot.
                            let sorte = crate::calquemenu::Sorte::of(&self.layers()[li]);
                            let label = RichText::new(sorte.icone_ligne()).size(13.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                let about = ui.label(if folder || current { label.weak() } else { label.color(hue) });
                                if folder {
                                    crate::bulles::bulle(about, "Dossier", "Dossier de calques. Choisis-le pour régler son opacité (en haut) : elle s'ajoute à celle de chacun de ses calques. Son œil et son verrou agissent sur tous ses calques. Monter ou descendre le dossier emmène ses calques.", "");
                                    ui.label(RichText::new(kids.to_string()).small().weak())
                                        .on_hover_text(format!("{kids} calque{} dans ce dossier.", if kids > 1 { "s" } else { "" }));
                                } else {
                                    crate::bulles::bulle(about, sorte.nom(), sorte.phrase(), "");
                                }
                                if let Some(line) = &colour_of {
                                    ui.label(RichText::new(format!("{} sous {line}", ph::LINK_SIMPLE)).small().weak())
                                        .on_hover_text("Calque de couleur : ses images suivent celles du calque de trait. On colorie dessous, le trait reste intact.");
                                } else if colours > 0 {
                                    ui.label(RichText::new(format!("{} {colours}", ph::PALETTE)).small().weak())
                                        .on_hover_text("Ce calque de trait porte des calques de couleur, rangés juste dessous.");
                                }
                                // Planche-mère (`partageui.rs`) : le maillon et les plans qui partagent ce dessin.
                                self.maillon_partage(ui, li);
                                // Plusieurs calques choisis : une marque pour chacun, et une autre
                                // pour le courant (c'est lui que les réglages montrent).
                                if several && current {
                                    ui.label(RichText::new(ph::TARGET).small()).on_hover_text("Calque courant : celui des réglages. Les autres calques choisis portent une coche.");
                                } else if several && chosen {
                                    ui.label(RichText::new(ph::CHECK_CIRCLE).small().color(hue)).on_hover_text("Calque choisi avec les autres (Ctrl ou Maj + clic) : les commandes sur plusieurs calques, la sélection et l'outil Transformer le prennent.");
                                }
                                // Aplat sous le trait : un bouton par calque (allumé s'il en a un) ; c'est ici qu'on
                                // l'active (`fondui.rs`). Sa sous-ligne vient sous le rang.
                                if let Some(resp) = self.fond_layer_button(ui, li, true) {
                                    if resp.clicked() {
                                        self.toggle_fond(li);
                                    }
                                }
                            });
                        });
                    });
                });
            let rect = shown.response.rect;
            if !folder {
                // La bande de sa sorte, au bord gauche du rang (après la place du filet du dossier, quand le plan en a).
                let band = egui::Rect::from_min_size(rect.left_top() + vec2(gouttiere, 2.0), vec2(3.0, rect.height() - 4.0));
                ui.painter().rect_filled(band, 1.0, hue);
            }
            // Le filet du dossier, tout à gauche : il reprend au bas du rang d'avant quand c'est le même dossier.
            let dossier_du_rang = crate::folders::parent_index(self.layers(), li).unwrap_or(li);
            filet_d_avant = filets.get(li).copied().flatten().map(|c| {
                let haut = match filet_d_avant {
                    Some((d, bas)) if d == dossier_du_rang => bas,
                    _ => rect.top(),
                };
                ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(rect.left(), haut), egui::pos2(rect.left() + FILET, rect.bottom())), 0.0, c);
                (dossier_du_rang, rect.bottom())
            });
            ui.ctx().data_mut(|d| d.insert_temp(grip_id, rect));
            drag_rows.push(crate::layerheads::Row { li, top: rect.top() - list_top, h: rect.height() });
            if let Some(grip) = grip {
                if grip.drag_started() && self.layer_rename.is_none() {
                    self.panel_layer_drag = Some(li);
                }
                if grip.clicked() || grip.secondary_clicked() {
                    select = Some((li, grip.secondary_clicked()));
                }
                grip.context_menu(|ui| self.layer_full_menu(ui, li, false));
            }
            // La sous-ligne de l'aplat, fine, juste sous son calque (ce panneau seulement : ni la timeline ni le
            // montage). Dossier fermé : elle suit son calque, qui n'a pas de rang non plus.
            if has_fond {
                let resp = self.aplat_subrow(ui, li, hue, gouttiere + (in_folder as u8 + colour_of.is_some() as u8) as f32 * 14.0);
                if resp.clicked() {
                    aplat_choisi = Some(li);
                }
                // (Le filet du dossier longe aussi la sous-ligne.)
                if let (Some(c), Some((d, bas))) = (filets.get(li).copied().flatten(), filet_d_avant) {
                    ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(rect.left(), bas), egui::pos2(rect.left() + FILET, resp.rect.bottom())), 0.0, c);
                    filet_d_avant = Some((d, resp.rect.bottom()));
                }
                resp.context_menu(|ui| self.aplat_menu(ui, li));
            }
        }
        tient &= self.calques_tiennent(&avant, usize::MAX);
        if !tient {
            ui.ctx().request_repaint();
            return;
        }
        if let Some(from) = self.panel_layer_drag {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            // Un trait entre deux rangs, ou le cadre du dossier quand on est sur son rang : lâchés là, les calques y entrent
            // (Anthony, 2026-11-11 : « glisser des calques dans un dossier ne fonctionne pas »). Les calques choisis partent ensemble.
            let pointer = ui.input(|i| i.pointer.interact_pos());
            let cible = pointer.and_then(|p| self.cible_des_calques_glisses(&drag_rows, from, p.y - list_top));
            if let Some((c, dans)) = cible {
                crate::layerheads::peindre_cible(ui.painter(), ui.max_rect().x_range(), list_top, c, dans, gouttiere + 14.0);
            }
            if ui.input(|i| !i.pointer.any_down()) {
                self.panel_layer_drag = None;
                if let Some((c, _)) = cible {
                    if !self.content_refuses() {
                        self.poser_les_calques(from, c.pose());
                    }
                }
            }
        }
        if let Some(li) = fold {
            // (Comme dans la timeline : un réglage de l'affichage, pas une étape d'annulation.)
            let shut = self.layers()[li].collapsed;
            self.layers_mut()[li].collapsed = !shut;
            self.modified = true;
        }
        if let Some(li) = toggle {
            // (Un calque de trait emmène ses calques de couleur.)
            self.toggle_layer_visible(li);
        }
        if let Some(li) = lock {
            self.toggle_layer_lock(li);
        }
        if let Some((li, secondary)) = select {
            // Clic droit sur un calque déjà choisi : le choix reste (le menu vise les choisis).
            if !(secondary && self.is_chosen_layer(li)) {
                self.click_layer_head(li, mods.command && !secondary, mods.shift && !secondary);
            }
        }
        self.finish_layer_rename(rename_done);
        if let Some(li) = aplat_choisi {
            self.select_aplat(li);
        }

        // Clic droit dans le vide sous la liste : de nouveaux calques.
        let rest = ui.available_rect_before_wrap();
        if rest.height() > 4.0 {
            let resp = ui.interact(rest, ui.id().with("vide_calques"), Sense::click());
            resp.context_menu(|ui| {
                self.layer_new_menu(ui);
                ui.separator();
                self.action_item(ui, "Coller le calque", "calque.coller");
            });
        }
    }
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
        let resp = ui
            .horizontal(|ui| {
                ui.label("Profondeur");
                // Position verrouillée (13b) : la profondeur ne bouge plus non plus.
                ui.add_enabled(
                    !fixed,
                    egui::DragValue::new(&mut depth).speed(0.01).range(crate::document::Layer::MIN_DEPTH..=1000.0).max_decimals(2),
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
        }
    }
}
