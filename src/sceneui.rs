//! Partie 197 : l'affichage du panneau Scène (`scene.rs` pour l'arbre, `docs/INTERFACE.md` partie 197) : « Depuis : Film · Séquence ·
//! Plan » et le fil d'Ariane en haut, puis l'arbre, dont seules les lignes visibles sont peintes.
//!
//! Chaque ligne fait ce que fait la ligne de la timeline, par le même code : un clic rend le calque courant (`click_layer_head`,
//! après `select_panel` pour un calque d'un autre plan), l'œil et le cadenas sont les colonnes de la timeline (`agir_colonne`), le
//! triangle d'un dossier est celui de ses pistes (`basculer_dossier`), le clic droit ouvre le menu de la ligne (`montage_layer_menu`
//! dans le Storyboard, `layer_head_menu` dans l'Animation), et la ligne du calque courant est **la ligne complète de la timeline**
//! elle-même (`layerheads::layer_head` : mode, opacité, pelure, effets, accroche à la caméra, losange des clés d'opacité, ⋮),
//! peinte ici avec ses propres identifiants (`App::tete_ailleurs`). Un calque glissé se pose comme dans la timeline
//! (`cible_des_calques_glisses`, `poser_les_calques`), dans les calques du plan courant.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles::bulle;
use crate::layerheads::{icon_at, Colonne};
use crate::scene::{Ligne, Ou, Quoi, Racine, RETRAIT};

/// Ce que `tete_ailleurs` vaut pendant que la ligne complète du calque courant se peint dans le panneau.
const SEL_SCENE: u64 = 197;

/// Pour les essais, les bulles du panneau sont notées sous « Scène › » et leur nom (`bulles::vus::prefixer`) : celles des lignes de
/// la timeline qu'il reprend ne se confondent pas avec celles de la timeline.
#[cfg(test)]
pub(crate) const PREFIXE_DES_ESSAIS: &str = "Scène › ";

// Les lignes peintes à la dernière image (ce qu'elles montrent, où), pour les essais.
#[cfg(test)]
thread_local! {
    pub(crate) static LIGNES_SCENE: std::cell::RefCell<Vec<(Quoi, Rect)>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl App {
    /// La timeline que le panneau suit : celle du Storyboard (ses pistes) en storyboard, celle de l'Animation (sa colonne des calques)
    /// en animation.
    pub(crate) fn scene_suit_le_storyboard(&self) -> bool {
        self.mode == crate::app::Mode::Storyboard
    }

    /// Où l'arbre du panneau en est.
    pub(crate) fn scene_ou(&self) -> Ou {
        Ou { racine: self.settings.scene.racine, sb: self.scene_suit_le_storyboard(), cur_panel: self.cur_panel, cur_layer: self.cur_layer }
    }

    /// Fait `f` sur le plan `pi` comme s'il était le plan courant (l'œil, le cadenas, le dossier d'un calque d'un autre plan, par le
    /// code de la timeline), puis revient au plan et au calque courants. Les calques choisis ne comptent pas là-bas.
    pub(crate) fn sur_le_plan<R>(&mut self, pi: usize, f: impl FnOnce(&mut Self) -> R) -> R {
        if pi == self.cur_panel || pi >= self.doc.panels.len() {
            return f(self);
        }
        self.end_stroke();
        let (plan, calque) = (self.cur_panel, self.cur_layer);
        let choix = std::mem::take(&mut self.layer_sel);
        self.cur_panel = pi;
        self.cur_layer = 0;
        let r = f(self);
        self.cur_panel = plan.min(self.doc.panels.len().saturating_sub(1));
        self.cur_layer = calque.min(self.layers().len().saturating_sub(1));
        self.layer_sel = choix;
        r
    }

    /// Le panneau Scène.
    pub(crate) fn scene_panel(&mut self, ui: &mut egui::Ui) {
        // (Tout le film écarté : pas le plan vide qui le tient, `planannuleui.rs`.)
        if self.doc.panels.is_empty() || self.panneau_film_vide(ui) {
            return;
        }
        #[cfg(test)]
        crate::bulles::vus::prefixer(PREFIXE_DES_ESSAIS);
        self.scene_panel_en(ui);
        #[cfg(test)]
        crate::bulles::vus::prefixer("");
    }

    fn scene_panel_en(&mut self, ui: &mut egui::Ui) {
        // (Partie 207 : la hauteur de la barre est retenue, pour la hauteur que le panneau demande à la colonne, `colonne_voulue`.)
        let haut = ui.cursor().top();
        self.scene_barre(ui);
        ui.add_space(2.0);
        let barre = ui.cursor().top() - haut;
        if barre.is_finite() && (barre - self.scene_arbre.barre).abs() > 0.5 {
            self.scene_arbre.barre = barre;
            ui.ctx().request_repaint();
        }
        let ou = self.scene_ou();
        self.scene_arbre.a_jour(&self.doc, &self.settings.scene, ou);
        #[cfg(test)]
        LIGNES_SCENE.with(|l| l.borrow_mut().clear());
        let numeros = crate::numerotation::Numeros::pour_l_ecran(&self.doc);
        egui::ScrollArea::vertical().id_salt("scene_liste").auto_shrink([false, false]).show_viewport(ui, |ui, vue| {
            ui.set_height(self.scene_arbre.hauteur);
            let origine = ui.max_rect().min;
            let w = ui.max_rect().width();
            let rangs = self.scene_arbre.visibles(vue.min.y, vue.max.y);
            let lignes: Vec<Ligne> = self.scene_arbre.lignes[rangs].to_vec();
            self.scene_arbre.peintes = lignes.len();
            // (Le menu d'une ligne peut changer les calques du plan courant ; l'arbre de cette image ne vaut plus : on s'arrête, l'image
            // suivante le refait.)
            let avant = self.empreinte_calques();
            for l in &lignes {
                if !self.calques_tiennent(&avant, 0) {
                    self.scene_arbre.oublier();
                    ui.ctx().request_repaint();
                    break;
                }
                let rect = Rect::from_min_size(pos2(origine.x, origine.y + l.top), vec2(w, l.h));
                #[cfg(test)]
                LIGNES_SCENE.with(|v| v.borrow_mut().push((l.quoi, rect)));
                match l.quoi {
                    Quoi::Sequence { seq, debut, n } => self.scene_sequence(ui, rect, *l, seq, debut, n, &numeros),
                    Quoi::Plan { pi } => self.scene_plan(ui, rect, *l, pi, &numeros),
                    Quoi::Calque { pi, li, id } => self.scene_calque(ui, rect, *l, pi, li, id),
                    Quoi::Camera { pi } => self.scene_camera(ui, rect, *l, pi),
                }
            }
            self.scene_glisser(ui, &lignes, origine.y, egui::Rangef::new(origine.x, origine.x + w));
        });
    }

    /// La barre du haut : « Depuis » et ses trois choix, puis le fil d'Ariane.
    fn scene_barre(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(RichText::new("Depuis").small().weak());
            let actuelle = self.settings.scene.racine;
            for r in Racine::TOUTES {
                let resp = ui.selectable_label(actuelle == r, r.nom());
                if bulle(resp, &format!("Depuis : {}", r.nom()), r.aide(), "").clicked() && actuelle != r {
                    self.settings.scene.racine = r;
                    let _ = self.settings.save();
                }
            }
            ui.add_space(6.0);
            // Le fil d'Ariane : il passe à la ligne **entre** ses éléments, jamais au milieu de l'un (partie 207 : « Film › Séquence » /
            // « 1 › Plan 1A ») ; « › » reste collé à l'élément qui le suit ; un élément plus large que la colonne est coupé par « … »
            // (le nom entier dans la bulle).
            let fil = crate::scene::fil(&self.doc, self.settings.scene.racine, self.cur_panel);
            let n = fil.len();
            let large = ui.max_rect().width();
            for (k, t) in fil.into_iter().enumerate() {
                let texte = if k > 0 { format!("› {t}") } else { t.clone() };
                let rich = if k + 1 == n { RichText::new(&texte).small().strong() } else { RichText::new(&texte).small().weak() };
                let police = egui::TextStyle::Small.resolve(ui.style());
                let tient = ui.fonts(|f| f.layout_no_wrap(texte.clone(), police, egui::Color32::WHITE).size().x) <= large;
                let label = egui::Label::new(rich).wrap_mode(if tient { egui::TextWrapMode::Extend } else { egui::TextWrapMode::Truncate });
                let r = ui.add(label);
                if !tient {
                    r.on_hover_text(t);
                }
            }
        });
    }

    /// Le triangle d'une ligne qui se déplie (séquence, plan, dossier), à `x` ; rend vrai au clic.
    fn scene_triangle(ui: &mut egui::Ui, rect: Rect, x: f32, ouvert: bool, quoi: &str) -> bool {
        let case = Rect::from_center_size(pos2(x + 7.0, rect.center().y), vec2(16.0, rect.height().min(18.0)));
        let resp = icon_at(ui, case, if ouvert { ph::CARET_DOWN } else { ph::CARET_RIGHT }, ui.visuals().weak_text_color());
        let nom = if ouvert { format!("Replier {quoi}") } else { format!("Déplier {quoi}") };
        bulle(resp, &nom, "", "").clicked()
    }

    /// Le fond d'une ligne : l'accent pour ce qui est courant, plus léger pour ce qui est choisi, rien sinon.
    fn scene_fond(ui: &egui::Ui, rect: Rect, courant: bool, choisi: bool) {
        let r = rect.shrink2(vec2(2.0, 1.0));
        if courant {
            ui.painter().rect_filled(r, 3.0, crate::theme::surface_active(crate::theme::accent(), 0.45));
        } else if choisi {
            ui.painter().rect_filled(r, 3.0, crate::theme::surface_active(crate::theme::accent(), 0.22));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn scene_sequence(&mut self, ui: &mut egui::Ui, rect: Rect, l: Ligne, seq: u64, debut: usize, n: usize, numeros: &crate::numerotation::Numeros) {
        let courante = (debut..debut + n).contains(&self.cur_panel);
        let resp = ui.interact(rect, egui::Id::new(("scene_sequence", seq, debut)), Sense::click());
        Self::scene_fond(ui, rect, false, courante && !l.ouvert);
        let x = rect.left() + 6.0 + l.niveau as f32 * RETRAIT;
        let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        p.text(pos2(x + 18.0, rect.center().y), Align2::LEFT_CENTER, ph::FILM_SLATE, FontId::proportional(crate::tailleicones::icone()), ui.visuals().text_color());
        let nom = format!("Séquence {}", numeros.sequence_label(&self.doc, seq));
        let encre = if courante { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
        let g = p.text(pos2(x + 36.0, rect.center().y), Align2::LEFT_CENTER, nom, FontId::proportional(12.5), encre);
        p.text(pos2(g.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, format!("{n} plan{}", if n > 1 { "s" } else { "" }), FontId::proportional(10.5), ui.visuals().weak_text_color());
        // (Partie 201 : des objets du décor 3D glissés depuis « Scène 3D » se lâchent sur une séquence.)
        if self.scene_lacher_sur_sequence(ui, &resp, rect, seq) {
            return;
        }
        if Self::scene_triangle(ui, rect, x, l.ouvert, "la séquence") {
            self.settings.scene.basculer_sequence(seq, courante);
            let _ = self.settings.save();
        } else if bulle(resp, "Séquence", "Un clic : son premier plan. Le triangle : ses plans.", "").clicked() {
            self.select_panel(debut);
        }
    }

    fn scene_plan(&mut self, ui: &mut egui::Ui, rect: Rect, l: Ligne, pi: usize, numeros: &crate::numerotation::Numeros) {
        let courant = pi == self.cur_panel;
        let id = self.doc.panels[pi].id;
        let resp = ui.interact(rect, egui::Id::new(("scene_plan", id)), Sense::click());
        Self::scene_fond(ui, rect, courant, false);
        let x = rect.left() + 6.0 + l.niveau as f32 * RETRAIT;
        // La petite vignette (celle de la bande), le nom et la durée.
        let h = rect.height() - 6.0;
        let fr = self.doc.frame_rect();
        let aspect = (fr.width() / fr.height().max(1.0)).clamp(0.5, 3.0);
        let vignette = Rect::from_min_size(pos2(x + 18.0, rect.top() + 3.0), vec2(h * aspect, h));
        let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        p.rect_filled(vignette, 1.0, crate::theme::controle());
        let fin = std::time::Instant::now() + std::time::Duration::from_millis(2);
        if let Some(tex) = self.thumbs.texture(ui.ctx(), &self.doc, pi, crate::thumbs::STRIP_WIDTH, fin) {
            p.image(tex, vignette, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        p.rect_stroke(vignette, 1.0, Stroke::new(1.0_f32, crate::theme::bord()), StrokeKind::Inside);
        let nom = crate::storyboard::panel_title_n(&self.doc, numeros, pi);
        let encre = if courant { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
        let g = p.text(pos2(vignette.right() + 6.0, rect.center().y), Align2::LEFT_CENTER, nom, FontId::proportional(12.5), encre);
        let duree = crate::storyboard::duration_label(self.doc.panels[pi].frame_count(), self.doc.fps);
        p.text(pos2(g.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, duree, FontId::proportional(10.5), ui.visuals().weak_text_color());
        if Self::scene_triangle(ui, rect, x, l.ouvert, "le plan") {
            self.settings.scene.basculer_plan(id, courant);
            let _ = self.settings.save();
        } else if bulle(resp, "Plan", "Un clic : aller à ce plan. Le triangle : ses calques.", "").clicked() && !courant {
            self.select_panel(pi);
        }
    }

    /// Mène au plan `pi` et y rend le calque `li` courant (le clic sur une ligne d'un autre plan) ; dans le plan courant, le clic de la
    /// timeline (Ctrl : ajouté au choix ; Maj : ceux d'entre).
    fn scene_choisir(&mut self, pi: usize, li: usize, ctrl: bool, shift: bool) {
        if pi != self.cur_panel {
            self.select_panel(pi);
            self.click_layer_head(li, false, false);
        } else {
            self.click_layer_head(li, ctrl, shift);
        }
    }

    fn scene_calque(&mut self, ui: &mut egui::Ui, rect: Rect, l: Ligne, pi: usize, li: usize, id: u64) {
        let Some(layer) = self.doc.panels.get(pi).and_then(|p| p.layers.get(li)).filter(|x| x.id == id) else {
            self.scene_arbre.oublier();
            ui.ctx().request_repaint();
            return;
        };
        let (dossier, ferme, nom, visible, verrou) = (layer.folder, layer.collapsed, layer.name.clone(), layer.visible, layer.locked);
        let (encre_sorte, signe) = (crate::layerheads::encre_de_sorte(layer), crate::layerheads::signe_de_sorte(layer));
        let filet = crate::folders::couleur_du_rangement(&self.doc.panels[pi].layers, li, &self.doc.groups);
        let sb = self.scene_suit_le_storyboard();
        let courant = pi == self.cur_panel && li == self.cur_layer && !self.e3_camera_seule();
        let x = rect.left() + 6.0 + l.niveau as f32 * RETRAIT;
        // Le fond de la ligne : choisir, glisser, double-clic sur le nom, menu. (Sous la ligne complète du calque courant aussi : son
        // menu, ouvert avant qu'il ne devienne courant, reste ouvert.)
        let resp = ui.interact(rect, egui::Id::new(("scene_calque", id)), Sense::click_and_drag());
        let menu = |app: &mut Self, ui: &mut egui::Ui| {
            if sb {
                app.montage_layer_menu(ui, li);
            } else {
                app.layer_head_menu(ui, li);
            }
        };
        if courant && !dossier {
            // La ligne complète de la timeline, celle de son calque courant, à son retrait.
            resp.context_menu(|ui| menu(self, ui));
            let r = Rect::from_min_max(pos2((x - 4.0).max(rect.left()), rect.top()), rect.max);
            self.tete_ailleurs = SEL_SCENE;
            self.layer_head(ui, r, li);
            self.tete_ailleurs = 0;
            return;
        }
        let choisi = pi == self.cur_panel && self.is_chosen_layer(li);
        Self::scene_fond(ui, rect, courant, choisi);
        if let Some(c) = filet {
            ui.painter().rect_filled(Rect::from_min_size(pos2(x - 3.0, rect.top() + 3.0), vec2(2.0, rect.height() - 6.0)), 0.0, c);
        }
        let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        let y = rect.center().y;
        // La pastille de la sorte, son signe, le nom.
        p.circle_filled(pos2(x + 22.0, y), 3.5, encre_sorte);
        let encre = if !visible { ui.visuals().weak_text_color() } else if courant { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
        let x_signe = x + 30.0;
        if dossier {
            p.text(pos2(x_signe, y), Align2::LEFT_CENTER, if ferme { ph::FOLDER } else { ph::FOLDER_OPEN }, FontId::proportional(crate::tailleicones::icone()), filet.unwrap_or(encre));
        } else {
            let police = if signe.1 { FontId::monospace(10.5) } else { FontId::proportional(crate::tailleicones::icone()) };
            p.text(pos2(x_signe, y), Align2::LEFT_CENTER, signe.0, police, encre_sorte);
        }
        // À droite : le cadenas et l'œil, les colonnes de la timeline.
        let w = crate::layerheads::icon_w();
        let oeil = Rect::from_center_size(pos2(rect.right() - 4.0 - w * 0.5, y), vec2(w, 17.0));
        let cadenas = oeil.translate(vec2(-w, 0.0));
        let nom_rect = Rect::from_min_max(pos2(x_signe + 16.0, rect.top()), pos2(cadenas.left() - 2.0, rect.bottom()));
        let renomme = self.layer_rename.as_ref().is_some_and(|n| n.id == id && n.in_scene);
        if renomme {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(nom_rect.shrink2(vec2(0.0, 1.0))).id_salt(("scene_nom", id)));
            let fait = self.layer_rename_field(&mut child, nom_rect.width());
            self.finish_layer_rename(fait);
        } else {
            p.with_clip_rect(nom_rect).text(pos2(nom_rect.left(), y), Align2::LEFT_CENTER, &nom, FontId::proportional(12.0), encre);
        }
        if dossier {
            if Self::scene_triangle(ui, rect, x, !ferme, "le dossier") {
                self.sur_le_plan(pi, |app| app.basculer_dossier(li));
            }
        }
        for (col, case, on) in [(Colonne::Verrouiller, cadenas, verrou), (Colonne::Afficher, oeil, visible)] {
            let icone = match col {
                Colonne::Afficher if on => ph::EYE,
                Colonne::Afficher => ph::EYE_SLASH,
                _ if on => ph::LOCK_SIMPLE,
                _ => ph::LOCK_SIMPLE_OPEN,
            };
            // (Allumé, l'œil ou le cadenas fermé, à l'encre du texte ; éteint, pâle.)
            let teinte = if on { ui.visuals().text_color() } else { ui.visuals().weak_text_color().gamma_multiply(0.6) };
            let r = icon_at(ui, case, icone, teinte);
            if self.aide(r, col.nom(), col.description(), col.action()).clicked() {
                self.sur_le_plan(pi, |app| app.agir_colonne(col, li));
            }
        }
        // Les gestes de la ligne.
        if resp.drag_started() && pi == self.cur_panel && !self.content_refuses() {
            self.select_layer(li);
            self.panel_layer_drag = Some(li);
        }
        if resp.secondary_clicked() && !(pi == self.cur_panel && self.is_chosen_layer(li)) {
            // (Comme dans la timeline : le clic droit vise ce calque ; d'un autre plan, on y va d'abord.)
            self.scene_choisir(pi, li, false, false);
        }
        resp.context_menu(|ui| menu(self, ui));
        if resp.double_clicked() && pi == self.cur_panel && resp.interact_pointer_pos().is_some_and(|q| nom_rect.contains(q)) {
            self.layer_rename = Some(crate::layersui::LayerRename { in_scene: true, ..crate::layersui::LayerRename::new(id, &nom, false) });
        } else if resp.clicked() {
            if dossier && sb {
                // (Dans le Storyboard, un clic sur la ligne d'un dossier le ferme ou l'ouvre, comme sa ligne dans la timeline.)
                self.sur_le_plan(pi, |app| app.basculer_dossier(li));
            } else {
                let m = ui.input(|i| i.modifiers);
                self.scene_choisir(pi, li, m.command, m.shift);
            }
        }
        let quoi = if dossier { "Dossier" } else { "Calque" };
        bulle(resp, quoi, "Un clic : il devient le calque courant (d'un autre plan : on y va). Double-clic sur le nom : le renommer. Glisser : le ranger. Clic droit : son menu, celui de la timeline.", "");
    }

    fn scene_camera(&mut self, ui: &mut egui::Ui, rect: Rect, l: Ligne, pi: usize) {
        let id = self.doc.panels[pi].id;
        let courante = pi == self.cur_panel && self.e3_camera_seule();
        let resp = ui.interact(rect, egui::Id::new(("scene_camera", id)), Sense::click());
        Self::scene_fond(ui, rect, courante, false);
        let x = rect.left() + 6.0 + l.niveau as f32 * RETRAIT;
        let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        p.circle_filled(pos2(x + 22.0, rect.center().y), 3.5, crate::theme::camera());
        p.text(pos2(x + 30.0, rect.center().y), Align2::LEFT_CENTER, ph::VIDEO_CAMERA, FontId::proportional(crate::tailleicones::icone()), crate::theme::camera());
        p.text(pos2(x + 48.0, rect.center().y), Align2::LEFT_CENTER, "Caméra", FontId::proportional(12.0), ui.visuals().text_color());
        let sb = self.scene_suit_le_storyboard();
        if resp.secondary_clicked() && pi != self.cur_panel {
            self.select_panel(pi);
        }
        resp.context_menu(|ui| {
            if sb {
                self.menu_camera_sb(ui);
            } else {
                self.grid_menu_ui(ui, crate::handles::GridMenu::Camera);
            }
        });
        if bulle(resp, "Caméra", "La caméra du plan : un clic la choisit seule dans la vue Espace 3D (comme sa ligne dans la timeline). Clic droit : son menu.", "").clicked() {
            if pi != self.cur_panel {
                self.select_panel(pi);
            }
            self.e3_piste_camera_cliquee();
        }
    }

    /// Un calque glissé dans le panneau : où il va se poser (un trait, ou le cadre d'un dossier), et lâché, il s'y pose, par le code
    /// de la timeline. Seulement parmi les calques du plan courant.
    fn scene_glisser(&mut self, ui: &mut egui::Ui, lignes: &[Ligne], haut: f32, x: egui::Rangef) {
        let Some(from) = self.panel_layer_drag else { return };
        let cur = self.cur_panel;
        let rows: Vec<crate::layerheads::Row> = lignes
            .iter()
            .filter_map(|l| match l.quoi {
                Quoi::Calque { pi, li, .. } if pi == cur => Some(crate::layerheads::Row { li, top: l.top, h: l.h }),
                _ => None,
            })
            .collect();
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let pointeur = ui.input(|i| i.pointer.interact_pos());
        let dans_les_lignes = |y: f32| rows.first().is_some_and(|a| y >= a.top - 10.0) && rows.last().is_some_and(|z| y <= z.top + z.h + 10.0);
        let cible = pointeur.map(|p| p.y - haut).filter(|y| dans_les_lignes(*y)).and_then(|y| self.cible_des_calques_glisses(&rows, from, y));
        if let Some((c, dans)) = cible {
            crate::layerheads::peindre_cible(ui.painter(), x, haut, c, dans, RETRAIT);
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
}
