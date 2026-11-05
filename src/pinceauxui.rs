//! Le pinceau dans le panneau Réglages de l'outil : la taille et l'opacité, le nom de la brosse, l'aperçu
//! du trait et, à l'extrême droite de cet aperçu, un bouton qui déplie la liste des brosses juste dessous
//! (`brush_list_ui`), puis les réglages détaillés (`brush_settings_ui`).
//!
//! La liste des brosses était un panneau à part (« Pinceaux », en onglet avec la couleur) : depuis le
//! 2026-10-24 elle est là, sous l'aperçu (Anthony : « le panneau Pinceaux disparaît, il rejoint les
//! réglages du pinceau »). Elle garde tout ce que le panneau offrait : recherche, liste ou vignettes, « Mes
//! brosses » en premier, les dossiers de SHOT et des fichiers importés, clic droit (renommer, dupliquer,
//! remplacer, supprimer), et en bas importer, garder le réglage courant, supprimer.
//!
//! La taille de l'outil courant (curseur et champ en px) est en tête du panneau (`brush_summary_ui`).
//! Les réglages du pinceau (les catégories à gauche, les réglages de la catégorie à droite) sont sous
//! l'aperçu, derrière « Plus de réglages ».
//!
//! Le rangement, la recherche et l'état retenu sont dans `pinceaux.rs`.

use eframe::egui::{self, pos2, vec2, Align, Align2, Color32, FontId, Layout, Rect, RichText, Sense, Vec2};
use egui_phosphor::regular as ph;

use crate::app::{App, Tool};
use crate::brush::{stroke_preview, BrushSettings};
use crate::brushtex::Target;
use crate::brushui::{preset_menu, IMPORT_HINT};
use crate::bulles::{bulle, choix, icone, icone_si, interrupteur};
use crate::dockui::{fit_to_pane, fit_w};
use crate::dock::Pane;
use crate::document::LayerKind;
use crate::pinceaux::{choisi, dossier_de, ranger, supprimable, Categorie};

/// Hauteur d'une ligne de la liste, et l'aperçu du trait qu'elle porte.
const LINE_H: f32 = 26.0;
const LINE_THUMB: Vec2 = vec2(64.0, 18.0);
/// Largeur de la liste en dessous de laquelle la ligne n'a plus d'aperçu de trait.
const LINE_THUMB_MIN_W: f32 = 200.0;
/// Hauteur de l'aperçu du trait courant (panneau Réglages de l'outil).
const CURRENT_H: f32 = 38.0;
/// Place du bouton de la liste, à droite de l'aperçu du trait.
const LIST_BUTTON_W: f32 = 30.0;
/// Hauteur de la liste des brosses dépliée (elle défile au-delà) : de quoi voir « Mes brosses » et le début des
/// dossiers sans chasser les réglages sous elle.
const LIST_H: f32 = 300.0;
/// Largeur de la colonne des catégories ; en dessous de `NARROW` points de panneau, elle devient
/// une liste déroulante au-dessus des réglages.
const CATEGORIES_W: f32 = 142.0;
const NARROW: f32 = 330.0;

fn note(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().weak());
}

fn icone_categorie(c: Categorie) -> &'static str {
    match c {
        Categorie::Pointe => ph::PEN_NIB,
        Categorie::Taille => ph::HAND_POINTING,
        Categorie::Dispersion => ph::SPRAY_BOTTLE,
        Categorie::Grain => ph::GRAINS,
        Categorie::Couleur => ph::DROP,
        Categorie::Lissage => ph::WAVE_SINE,
        Categorie::Toucher => ph::HAND_TAP,
    }
}

impl App {
    /// Le réglage que montre le panneau (en copie).
    pub(crate) fn panel_brush(&self) -> BrushSettings {
        match self.tool {
            Tool::Pencil => self.pencil,
            Tool::Eraser => self.eraser,
            _ => self.brush,
        }
    }

    /// Une fois par image : SHOT retient le dernier pinceau utilisé sur un calque vectoriel et le
    /// dernier sur un calque bitmap, et rend celui de la sorte du calque courant quand on change de
    /// calque (Anthony, 2026-10-20 : « encre » en vectoriel, « gouache » en bitmap). Retenu dans les
    /// réglages de l'application (`Panneau::pinceau_vecto`, `pinceau_bitmap`).
    pub(crate) fn suivre_sorte_pinceau(&mut self) {
        // (Une brosse du décor a ses réglages, sur un calque bitmap : le pinceau de chaque sorte attend qu'on la lâche, `decorui.rs`.)
        if self.decor_actif().is_some() {
            return;
        }
        let Some(l) = self.layers().get(self.cur_layer) else { return };
        // (Un dossier ou un calque d'effet ne dessine pas : la sorte vue ne change pas.)
        if l.folder || l.effet.is_some() {
            return;
        }
        let (sorte, brush) = (l.kind, self.brush);
        let lancement = self.pinceaux.sorte_vue.is_none();
        if let Some(b) = self.pinceaux.suivre_sorte(sorte, &brush) {
            self.brush = b;
            self.brush_previews.memory.clear();
            if !lancement {
                self.status = format!("Pinceau de {} retrouvé.", if sorte == LayerKind::Vector { "calque vectoriel" } else { "calque bitmap" });
            }
        }
    }

    /// Le réglage de la bibliothèque visé : ceux de la gomme avec la gomme, sinon ceux du pinceau.
    fn panel_target(&self) -> Target {
        if self.tool == Tool::Eraser {
            Target::Eraser
        } else {
            Target::Brush
        }
    }

    fn tool_tag(&self) -> u8 {
        match self.tool {
            Tool::Pencil => 1,
            Tool::Eraser => 2,
            _ => 0,
        }
    }

    /// Les outils qui ont des brosses : le pinceau, le crayon et la gomme (la liste des brosses se montre dans leur
    /// panneau Réglages).
    fn outil_a_brosses(&self) -> bool {
        matches!(self.tool, Tool::Brush | Tool::Mixer | Tool::Pencil | Tool::Eraser)
    }

    /// Déplie la liste des brosses dans le panneau Réglages de l'outil : ce que faisait l'ouverture de l'ancien
    /// panneau Pinceaux (l'action `vue.pinceaux`, un raccourci qu'on lui avait donné). Le panneau Réglages s'ouvre ou
    /// vient devant ; avec un outil sans brosses (sélection, remplissage…), le pinceau est pris, c'est lui que la
    /// liste sert.
    pub(crate) fn ouvrir_liste_pinceaux(&mut self) {
        if !self.outil_a_brosses() {
            self.set_tool(Tool::Brush);
        }
        self.pinceaux.liste_ouverte = true;
        self.show_pane(Pane::ToolOptions);
    }

    /// Replie la liste des brosses (et laisse de côté un nom en cours de saisie qui n'aurait plus de ligne où se
    /// montrer).
    pub(crate) fn replier_liste_pinceaux(&mut self) {
        self.pinceaux.liste_ouverte = false;
        if self.preset_edit.as_ref().is_some_and(|e| !e.summary) {
            self.preset_edit = None;
        }
    }

    /// L'action `vue.pinceaux` : déplie la liste ; si elle est déjà dépliée et qu'on la voit, la replie (comme
    /// l'ancien panneau, qu'un second appui fermait).
    pub(crate) fn basculer_liste_pinceaux(&mut self) {
        let visible = self.pinceaux.liste_ouverte && self.outil_a_brosses() && self.pane_shown(Pane::ToolOptions);
        if visible {
            self.replier_liste_pinceaux();
        } else {
            self.ouvrir_liste_pinceaux();
        }
    }

    /// La taille de l'outil courant (pinceau, crayon ou gomme ; la largeur d'une forme), en grand :
    /// la même valeur que les touches ^ et $.
    fn size_row(&mut self, ui: &mut egui::Ui) {
        let shape = self.tool.is_shape();
        let max = if shape { 200.0 } else { 400.0 };
        let before = if shape { self.shape.width } else { self.panel_brush().size };
        let mut size = before;
        let (plus, moins) = (self.raccourci("outil.taille_plus"), self.raccourci("outil.taille_moins"));
        let key = [plus, moins].into_iter().filter(|k| !k.is_empty()).collect::<Vec<_>>().join(" / ");
        let w = ui.available_width();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Taille").strong());
            ui.label(RichText::new(self.tool.label()).small().weak());
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().slider_width = (w - 78.0).clamp(40.0, 700.0);
            ui.spacing_mut().slider_rail_height = 10.0;
            let resp = ui.add(egui::Slider::new(&mut size, 0.5..=max).logarithmic(true).show_value(false));
            bulle(resp, "Taille du pinceau", "La taille de l'outil courant (pinceau, crayon ou gomme), en pixels du dessin. Les touches ^ et $ la changent aussi.", &key);
            let resp = ui.add(egui::DragValue::new(&mut size).range(0.5..=max).speed(0.2).max_decimals(1).suffix(" px"));
            bulle(resp, "Taille en pixels", "Glisser ou taper la taille exacte.", "");
        });
        if size != before {
            if shape {
                self.shape.width = size;
            } else {
                self.panel_brush_mut().size = size;
            }
        }
    }

    // ------------------------------------------------------------------ la liste des brosses

    /// La liste des brosses, dépliée sous l'aperçu du trait : dans un cadre, recherche, liste ou vignettes,
    /// dossiers repliables (« Mes brosses » en premier), trois boutons en bas.
    fn brush_list_ui(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            self.brush_list_content(ui);
        });
    }

    fn brush_list_content(&mut self, ui: &mut egui::Ui) {
        let target = self.panel_target();
        let current = match target {
            Target::Eraser => self.eraser,
            Target::Brush => self.brush,
        };
        // Recherche par nom, et liste ou vignettes.
        let mut liste = self.pinceaux.liste;
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 2.0 * 30.0 - 8.0).max(40.0);
            ui.add(egui::TextEdit::singleline(&mut self.brush_previews.search).hint_text(format!("{} Rechercher", ph::MAGNIFYING_GLASS)).desired_width(w));
            choix(ui, &mut liste, true, ph::LIST_BULLETS, "Liste", "Une ligne par pinceau : la taille, l'aperçu du trait, le nom.", "");
            choix(ui, &mut liste, false, ph::SQUARES_FOUR, "Vignettes", "Une grille de vignettes : l'aperçu du trait et la taille ; le nom au survol.", "");
        });
        self.pinceaux.liste = liste;
        self.preset_edit_row(ui, target, current, false);

        let search = self.brush_previews.search.clone();
        let searching = !search.trim().is_empty();
        let folders = ranger(&self.brushes.presets, target, &search);
        let selected = choisi(&self.brushes.presets, target, &current);
        let mut pick = None;
        let mut menu = None;
        let mut toggles: Vec<(String, bool)> = Vec::new();
        egui::ScrollArea::vertical().id_salt("pinceaux_dossiers").auto_shrink([false, true]).max_height(LIST_H).show(ui, |ui| {
            if folders.is_empty() {
                ui.label(RichText::new("Aucun pinceau de ce nom.").weak());
            }
            for d in &folders {
                // Une recherche ouvre les dossiers qui ont des réponses, sans changer ce qui est retenu.
                let open = searching || self.pinceaux.ouvert(&d.nom);
                let title = RichText::new(format!("{} ({})", d.nom, d.rangs.len())).strong();
                let resp = egui::CollapsingHeader::new(title).id_salt(("dossier_pinceaux", d.nom.as_str())).open(Some(open)).show(ui, |ui| {
                    if d.rangs.is_empty() {
                        note(ui, "Rien encore : le bouton + garde le réglage courant ici.");
                    }
                    if self.pinceaux.liste {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        for &i in &d.rangs {
                            let (name, source) = {
                                let p = &self.brushes.presets[i];
                                (p.name.clone(), p.source.clone())
                            };
                            let resp = self.preset_line(ui, i, &name, &source, current);
                            if resp.clicked() {
                                pick = Some(i);
                            }
                            resp.context_menu(|ui| {
                                if let Some(a) = preset_menu(ui, &source) {
                                    menu = Some((i, a));
                                }
                            });
                        }
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
                            for &i in &d.rangs {
                                let (name, source) = {
                                    let p = &self.brushes.presets[i];
                                    (p.name.clone(), p.source.clone())
                                };
                                let resp = self.preset_tile(ui, i, &name, &source, current);
                                if resp.clicked() {
                                    pick = Some(i);
                                }
                                resp.context_menu(|ui| {
                                    if let Some(a) = preset_menu(ui, &source) {
                                        menu = Some((i, a));
                                    }
                                });
                            }
                        });
                    }
                });
                if resp.header_response.clicked() && !searching {
                    toggles.push((d.nom.clone(), !open));
                }
            }
        });
        for (nom, ouvert) in toggles {
            self.pinceaux.regler(&nom, ouvert);
        }

        // Les trois boutons du bas.
        let (mut import, mut nouvelle, mut supprimer) = (false, false, false);
        let can_delete = selected.is_some_and(|i| supprimable(&self.brushes.presets[i]));
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            import = icone(ui, ph::FILE_ARROW_DOWN, "Importer des pinceaux", IMPORT_HINT, "").clicked();
            nouvelle = icone(ui, ph::PLUS, "Nouvelle brosse", "Garder le réglage courant sous un nom, dans « Mes brosses » (clic droit sur un pinceau : renommer, dupliquer, remplacer, supprimer).", "").clicked();
            supprimer = icone_si(ui, can_delete, ph::TRASH, "Supprimer la brosse", "Supprimer la brosse choisie (celles de FEANOR restent).", "").clicked();
        });

        if let Some(i) = pick {
            let settings = self.brushes.presets[i].settings;
            match target {
                Target::Eraser => self.eraser = settings,
                Target::Brush => {
                    self.brush = settings;
                    if !matches!(self.tool, Tool::Brush | Tool::Mixer) {
                        self.set_tool(Tool::Brush);
                    }
                }
            }
            self.brush_previews.memory.clear();
            let p = &self.brushes.presets[i];
            self.brush_previews.base = Some((p.target, p.name.clone(), p.source.clone()));
            self.status = format!("Réglage « {} ».", p.name);
        }
        if let Some((i, a)) = menu {
            self.apply_preset_menu(i, a, target, current);
        }
        if import {
            let before: Vec<String> = self.brushes.presets.iter().map(|p| p.source.clone()).collect();
            self.import_brushes();
            // Le dossier d'un fichier qu'on vient d'importer s'ouvre : c'est ce qu'on veut voir.
            let nouveaux: Vec<String> = self.brushes.presets.iter().filter(|p| !before.contains(&p.source)).map(dossier_de).collect();
            for nom in nouveaux {
                self.pinceaux.regler(&nom, true);
            }
        }
        if nouvelle {
            self.start_save_preset(target);
        }
        if supprimer {
            if let Some(i) = selected {
                self.apply_preset_menu(i, crate::brushui::PresetMenu::Delete, target, current);
            }
        }
    }

    /// Texture du trait d'un réglage pour la liste (refaite quand le réglage ou l'échelle change).
    fn list_texture(&mut self, ui: &egui::Ui, index: usize, settings: BrushSettings) -> egui::TextureId {
        let ppp = ui.ctx().pixels_per_point();
        let px = [(LINE_THUMB.x * ppp).round() as usize, (LINE_THUMB.y * ppp).round() as usize];
        let fresh = self.brush_previews.list.get(&index).is_some_and(|(s, size, _)| *s == settings && *size == px);
        if !fresh {
            let alpha = stroke_preview(&settings, self.brushes.textures(&settings), px[0], px[1]);
            let pixels = alpha.iter().map(|&a| Color32::from_rgba_premultiplied(a, a, a, a)).collect();
            let tex = ui.ctx().load_texture(format!("pinceau liste {index}"), egui::ColorImage { size: px, pixels }, egui::TextureOptions::LINEAR);
            self.brush_previews.list.insert(index, (settings, px, tex));
        }
        self.brush_previews.list[&index].2.id()
    }

    /// Une ligne de la liste : la pastille de la pointe avec la taille, l'aperçu du trait, le nom.
    fn preset_line(&mut self, ui: &mut egui::Ui, index: usize, name: &str, source: &str, current: BrushSettings) -> egui::Response {
        let settings = self.brushes.presets[index].settings;
        let width = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(width, LINE_H), Sense::click());
        // Hors de l'écran (une liste de centaines de brosses) : ni aperçu ni texte.
        if ui.is_rect_visible(rect) {
            let chosen = settings == current;
            let v = ui.visuals();
            let bg = if chosen {
                crate::theme::accent()
            } else if resp.hovered() {
                v.widgets.hovered.weak_bg_fill
            } else {
                Color32::TRANSPARENT
            };
            let ink = if chosen { crate::theme::sur_accent() } else { v.text_color() };
            let weak = if chosen { ink } else { v.weak_text_color() };
            ui.painter().rect_filled(rect, 3.0, bg);
            let cy = rect.center().y;
            // La pastille : un rond à l'échelle de la taille, puis le nombre.
            let diameter = (settings.size.sqrt() * 1.4).clamp(2.5, 16.0);
            ui.painter().circle_filled(pos2(rect.left() + 12.0, cy), diameter * 0.5, ink);
            let size = if settings.size < 10.0 { format!("{:.1}", settings.size) } else { format!("{:.0}", settings.size) };
            ui.painter().text(pos2(rect.left() + 24.0, cy), Align2::LEFT_CENTER, size, FontId::proportional(10.5), weak);
            let mut x = rect.left() + 52.0;
            if width >= LINE_THUMB_MIN_W {
                let tex = self.list_texture(ui, index, settings);
                let image = Rect::from_min_size(pos2(x, cy - LINE_THUMB.y * 0.5), LINE_THUMB);
                let (papier, trait_) = self.preview_colors(index);
                ui.painter().rect_filled(image, 2.0, papier);
                ui.painter().image(tex, image, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), trait_);
                x = image.right() + 8.0;
            }
            let clip = Rect::from_min_max(pos2(x, rect.top()), pos2(rect.right() - 4.0, rect.bottom()));
            ui.painter().with_clip_rect(clip).text(pos2(x, cy), Align2::LEFT_CENTER, name, FontId::proportional(13.0), ink);
        }
        bulle(resp, name, source, "")
    }

    // ------------------------------------------------------------------ réglages du pinceau

    /// Dans le panneau Réglages de l'outil, sous le résumé : l'aperçu du trait courant (on le voit
    /// changer pendant qu'on règle), puis les catégories (à gauche, ou en liste déroulante si le
    /// panneau est étroit) et les réglages de la catégorie choisie. Le panneau défile lui-même.
    pub(crate) fn brush_settings_ui(&mut self, ui: &mut egui::Ui) {
        let vector = self.layer().kind == LayerKind::Vector;
        let cat = self.pinceaux.categorie;
        let wide = ui.available_width() >= NARROW;
        ui.add_space(4.0);
        self.current_preview(ui);
        // La liste des brosses (l'ancien panneau Pinceaux), dépliée par le bouton à droite de l'aperçu.
        if self.pinceaux.liste_ouverte {
            ui.add_space(4.0);
            self.brush_list_ui(ui);
        }
        // Pinceau et crayon : « Peint sur l'aplat » et l'aplat du calque, toujours à portée, sous l'aperçu (Anthony,
        // 2026-10-23 ; avant, dans « Plus de réglages »).
        if matches!(self.tool, Tool::Brush | Tool::Pencil) {
            self.brush_extras_ui(ui);
        }
        // Le reste est replié derrière une petite flèche (retenu) : « Plus de réglages », comme pour les autres
        // outils (`repli.rs`), avec le mode d'emploi.
        let mut open = self.pinceaux.reglages_ouverts;
        crate::repli::entete(
            ui,
            &mut open,
            "Plus de réglages",
            "Les catégories de réglages (pointe, taille et appui, dispersion, grain, couleur, lissage, toucher) et le mode d'emploi.",
        );
        self.pinceaux.reglages_ouverts = open;
        if !open {
            return;
        }
        ui.add_space(4.0);
        if wide {
            ui.horizontal_top(|ui| {
                let left = ui
                    .vertical(|ui| {
                        ui.set_width(CATEGORIES_W);
                        for c in Categorie::ALL {
                            self.category_row(ui, c, cat);
                        }
                    })
                    .response
                    .rect;
                ui.add_space(6.0);
                let right = ui
                    .vertical(|ui| {
                        fit_to_pane(ui);
                        ui.label(RichText::new(cat.label()).strong());
                        note(ui, cat.help());
                        ui.add_space(2.0);
                        self.category_ui(ui, cat, vector);
                    })
                    .response
                    .rect;
                let x = left.right() + 3.0;
                let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
                ui.painter().vline(x, left.top()..=left.bottom().max(right.bottom()), stroke);
            });
        } else {
            self.category_combo(ui, cat);
            ui.add_space(2.0);
            self.category_ui(ui, cat, vector);
        }
    }

    /// Allume ou éteint une catégorie du réglage courant ; ses valeurs sont gardées pour la
    /// rallumer telle qu'elle était.
    fn toggle_category(&mut self, c: Categorie, on: bool) {
        let tag = self.tool_tag();
        let before = self.panel_brush();
        if on {
            let memory = self.brush_previews.memory.get(&(c, tag)).copied();
            c.allumer(self.panel_brush_mut(), memory.as_ref());
        } else {
            self.brush_previews.memory.insert((c, tag), before);
            c.eteindre(self.panel_brush_mut());
        }
    }

    /// Une ligne de la colonne des catégories : la case (s'il y en a une), l'icône et le nom.
    fn category_row(&mut self, ui: &mut egui::Ui, c: Categorie, selected: Categorie) {
        let on = c.allumee(&self.panel_brush());
        let mut toggle = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            if c.a_une_case() {
                let mut v = on;
                let resp = ui.checkbox(&mut v, "");
                bulle(resp.clone(), c.label(), if on { "Allumé : décocher l'éteint, sans perdre ses valeurs." } else { "Éteint : cocher le rallume avec ses valeurs d'avant." }, "");
                if resp.changed() {
                    toggle = Some(v);
                }
            } else {
                ui.allocate_space(vec2(ui.spacing().icon_width, ui.spacing().interact_size.y));
            }
            let mut text = RichText::new(format!("{}  {}", icone_categorie(c), c.label()));
            if c.a_une_case() && !on {
                text = text.weak();
            }
            let resp = ui.with_layout(Layout::left_to_right(Align::Center).with_main_justify(true), |ui| ui.selectable_label(selected == c, text)).inner;
            if resp.clicked() {
                self.pinceaux.categorie = c;
            }
        });
        if let Some(v) = toggle {
            self.toggle_category(c, v);
        }
    }

    /// Panneau étroit : les catégories dans une liste déroulante, avec la case de celle choisie.
    fn category_combo(&mut self, ui: &mut egui::Ui, cat: Categorie) {
        let b = self.panel_brush();
        let mut chosen = cat;
        let mut toggle = None;
        ui.horizontal(|ui| {
            let reserve = if cat.a_une_case() { 28.0 } else { 0.0 };
            egui::ComboBox::from_id_salt("categorie_pinceau")
                .selected_text(format!("{} {}", icone_categorie(cat), cat.label()))
                .width((fit_w(ui, 220.0) - reserve).max(48.0))
                .show_ui(ui, |ui| {
                    for c in Categorie::ALL {
                        let mark = match (c.a_une_case(), c.allumee(&b)) {
                            (true, true) => "● ",
                            (true, false) => "○ ",
                            _ => "",
                        };
                        ui.selectable_value(&mut chosen, c, format!("{mark}{}", c.label())).on_hover_text(c.help());
                    }
                });
            if cat.a_une_case() {
                let mut v = cat.allumee(&b);
                let resp = ui.checkbox(&mut v, "");
                bulle(resp.clone(), cat.label(), "Allumer ou éteindre cette catégorie, sans perdre ses valeurs.", "");
                if resp.changed() {
                    toggle = Some(v);
                }
            }
        });
        self.pinceaux.categorie = chosen;
        if let Some(v) = toggle {
            self.toggle_category(cat, v);
        }
    }

    /// Les réglages d'une catégorie (ceux de l'ancien panneau Réglages de l'outil).
    fn category_ui(&mut self, ui: &mut egui::Ui, c: Categorie, vector: bool) {
        match c {
            Categorie::Pointe => {
                self.tip_ui(ui);
                let b = self.panel_brush_mut();
                hardness_ui(ui, b, vector);
                crate::dynui::shape_ui(ui, b);
                let resp = ui.add(egui::Slider::new(&mut b.spacing, 0.02..=1.0).text("Espacement"));
                bulle(resp, "Espacement", "Distance entre deux empreintes, en part du diamètre.", "");
            }
            Categorie::Taille => size_and_pressure_ui(ui, self.panel_brush_mut(), vector),
            Categorie::Dispersion => crate::dynui::scatter_ui(ui, self.panel_brush_mut()),
            Categorie::Grain => self.paper_ui(ui),
            Categorie::Couleur => {
                let mixer = self.tool == Tool::Mixer;
                let b = self.panel_brush_mut();
                crate::dynui::wet_ui(ui, b, vector, mixer);
                ui.add_space(4.0);
                crate::dynui::dynamics_ui(ui, b, vector, true);
            }
            Categorie::Lissage => smoothing_ui(ui, self.panel_brush_mut()),
            Categorie::Toucher => self.feel_ui(ui),
        }
    }

    /// Le trait du pinceau courant, tracé par le moteur, sur un fond gris ; à son extrême droite, le bouton qui
    /// déplie ou replie la liste des brosses (`brush_list_ui`).
    fn current_preview(&mut self, ui: &mut egui::Ui) {
        let settings = self.panel_brush();
        let ppp = ui.ctx().pixels_per_point();
        // Largeur par paliers : on ne refait pas l'aperçu à chaque point quand on tire un bord.
        let shown = ((ui.available_width() - 4.0 - LIST_BUTTON_W - 4.0) / 24.0).floor().max(2.0) * 24.0;
        let px = [(shown * ppp).round() as usize, (CURRENT_H * ppp).round() as usize];
        let fresh = self.brush_previews.current.as_ref().is_some_and(|(s, size, _)| *s == settings && *size == px);
        if !fresh {
            let alpha = stroke_preview(&settings, self.brushes.textures(&settings), px[0], px[1]);
            let pixels = alpha.iter().map(|&a| Color32::from_rgba_premultiplied(a, a, a, a)).collect();
            let tex = ui.ctx().load_texture("pinceau courant", egui::ColorImage { size: px, pixels }, egui::TextureOptions::LINEAR);
            self.brush_previews.current = Some((settings, px, tex));
        }
        let Some((_, _, tex)) = &self.brush_previews.current else { return };
        let tex = tex.id();
        let [cr, cg, cb] = self.color;
        let trait_ = if self.tool == Tool::Eraser { Color32::from_gray(150) } else { Color32::from_rgb(cr, cg, cb) };
        let ouverte = self.pinceaux.liste_ouverte;
        let mut clic = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let (rect, resp) = ui.allocate_exact_size(vec2(shown, CURRENT_H), Sense::hover());
            // Fond : un gris un peu plus clair que le panneau, pas le papier du projet (Anthony,
            // 2026-10-20) ; poussé vers le blanc ou le noir si le trait s'y perdrait.
            ui.painter().rect_filled(rect, 3.0, crate::theme::fond_apercu_trait(trait_));
            ui.painter().image(tex, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), trait_);
            bulle(resp, "Aperçu du trait", "Le trait du pinceau courant, tracé par le moteur avec tous ses réglages.", "");
            // Tout à droite : la liste des brosses (enfoncé quand elle est dépliée).
            let reste = ui.available_width();
            ui.allocate_ui_with_layout(vec2(reste, CURRENT_H), Layout::right_to_left(Align::Center), |ui| {
                clic = self
                    .bouton_etat(
                        ui,
                        ouverte,
                        ph::BOOKS,
                        "Liste des pinceaux",
                        "Les brosses de FEANOR, « Mes brosses » et celles importées : en choisir une, en importer, garder le réglage courant, renommer, supprimer. Un clic déplie la liste sous l'aperçu, un autre la replie.",
                        "vue.pinceaux",
                    )
                    .clicked();
            });
        });
        if clic {
            if ouverte {
                self.replier_liste_pinceaux();
            } else {
                self.pinceaux.liste_ouverte = true;
            }
        }
    }

    // ------------------------------------------------------------------ pointe, papier, toucher

    /// La pointe : ronde ou une image (de la bibliothèque, ou importée), et ses cases.
    fn tip_ui(&mut self, ui: &mut egui::Ui) {
        let tips = self.brushes.tips();
        let tip_name = self.panel_brush().tip.and_then(|id| self.brushes.tip_name(id).map(String::from));
        let mut import_tip = false;
        let b = self.panel_brush_mut();
        ui.horizontal(|ui| {
            ui.label("Pointe");
            let name = tip_name.clone().unwrap_or_else(|| "ronde".to_string());
            let before = b.tip;
            egui::ComboBox::from_id_salt("pointe_pinceau").selected_text(name).width(fit_w(ui, 120.0)).show_ui(ui, |ui| {
                ui.selectable_value(&mut b.tip, None, "ronde");
                for (id, name) in &tips {
                    ui.selectable_value(&mut b.tip, Some(*id), name);
                }
            });
            // Une pointe de Pelure vient avec son nombre de cases.
            if b.tip != before {
                if let Some(n) = b.tip.and_then(crate::brushtex::builtin_tip_cells) {
                    b.tip_cells = n;
                    b.tip_order = crate::brush::TipOrder::Random;
                }
            }
            // Une image comme pointe (moteur de dessin, étape 8) : une seule, ou plusieurs dessins
            // côte à côte (pointe en plusieurs cases).
            import_tip = icone(ui, ph::PAINT_BRUSH_HOUSEHOLD, "Importer une pointe", "Une image devient la pointe : sa transparence, ou son noir sur fond blanc. Plusieurs dessins côte à côte : une pointe en plusieurs cases.", "").clicked();
        });
        if b.tip.is_some() {
            ui.horizontal_wrapped(|ui| {
                let resp = ui.add(egui::Slider::new(&mut b.tip_cells, 1..=64).text("Cases"));
                bulle(resp, "Cases", "Nombre de dessins côte à côte dans l'image de la pointe ; chaque empreinte en prend un.", "");
                if b.tip_cells > 1 {
                    egui::ComboBox::from_id_salt("pointe_ordre").selected_text(b.tip_order.label()).width(fit_w(ui, 120.0)).show_ui(ui, |ui| {
                        for o in crate::brush::TipOrder::ALL {
                            ui.selectable_value(&mut b.tip_order, o, o.label());
                        }
                    });
                }
            });
        }
        if import_tip {
            self.import_tip();
        }
    }

    /// Papier, grain, et le grain propre du crayon.
    fn paper_ui(&mut self, ui: &mut egui::Ui) {
        let papers = self.brushes.papers();
        let mut import = false;
        let b = self.panel_brush_mut();
        ui.horizontal(|ui| {
            ui.label("Papier");
            let name = b.paper.and_then(|id| papers.iter().find(|p| p.0 == id)).map_or("lisse".to_string(), |p| p.1.clone());
            egui::ComboBox::from_id_salt("papier_pinceau").selected_text(name).width(fit_w(ui, 120.0)).show_ui(ui, |ui| {
                ui.selectable_value(&mut b.paper, None, "lisse");
                for (id, name) in &papers {
                    ui.selectable_value(&mut b.paper, Some(*id), name);
                }
            });
            // Une image comme papier (moteur de dessin, étape 7) : par exemple un papier PNG de
            // TVPaint, choisi par Anthony.
            import = icone(ui, ph::IMAGE_SQUARE, "Importer un papier", "Une image (PNG, JPG…) devient un papier : son gris est le relief.", "").clicked();
        });
        if b.paper.is_some() {
            let resp = ui.add(egui::Slider::new(&mut b.grain, 0.0..=1.0).text("Grain"));
            bulle(resp, "Grain", "Le grain reste fixe sur le dessin ; appuyer plus fort remplit les creux du papier.", "");
            let resp = ui.add(egui::Slider::new(&mut b.paper_scale, 0.1..=8.0).logarithmic(true).text("Taille du papier"));
            bulle(resp, "Taille du papier", "Plus grand : un grain plus large.", "");
            ui.horizontal_wrapped(|ui| {
                let resp = ui.add(egui::Slider::new(&mut b.paper_angle, -180.0..=180.0).suffix("°").text("Angle"));
                bulle(resp, "Angle du papier", "Tourne le grain du papier.", "");
                ui.checkbox(&mut b.paper_invert, "Inverser").on_hover_text("Les creux du papier deviennent des sommets.");
            });
            // Rendu des brosses, étape 2 : un grain qui répond en douceur.
            let resp = ui.add(egui::Slider::new(&mut b.paper_softness, 0.05..=0.5).text("Douceur du grain"));
            bulle(resp, "Douceur du grain", "Largeur de la transition entre un creux vide et un sommet plein : petit = grain net et dur (0,12 : les anciens réglages), grand = grain qui répond en douceur (0,3 : les brosses de FEANOR).", "");
        }
        // Rendu des brosses, étape 3 : le grain propre du crayon, même sans papier.
        ui.horizontal(|ui| {
            ui.label("Grain du crayon");
            let resp = egui::ComboBox::from_id_salt("grain_crayon").selected_text(b.pencil_grain.label()).width(fit_w(ui, 100.0)).show_ui(ui, |ui| {
                for g in crate::brush::PencilGrain::ALL {
                    ui.selectable_value(&mut b.pencil_grain, g, g.label());
                }
            });
            bulle(resp.response, "Grain du crayon", "Un grain fin collé à la feuille, même sans papier : Soie, Doux, Moyen ou Dur.", "");
        });
        if b.paper.is_some() || b.pencil_grain != crate::brush::PencilGrain::None {
            ui.checkbox(&mut b.paper_follow, "Le grain suit le trait").on_hover_text("Le relief est lu depuis le début du trait au lieu de rester collé à la feuille (collé : repasser retrouve les mêmes creux).");
        }
        if import {
            self.import_paper();
        }
    }

    /// Toucher du stylet (sensation des brosses, 2026-10-18) : la réponse à la pression commune à
    /// tous les pinceaux, gardée dans les réglages de l'application (enregistrés dans `update`,
    /// comme la pelure) ; l'ancien toucher à portée de main pour comparer à la tablette.
    fn feel_ui(&mut self, ui: &mut egui::Ui) {
        use crate::brush::{Curve, Toucher};
        let t = &mut self.toucher;
        if *t != Toucher::default() {
            note(ui, "Réglé : différent de celui de l'étude.");
        }
        ui.horizontal_wrapped(|ui| {
            let resp = crate::dynui::curve_editor(ui, &mut t.pressure_curve, "toucher");
            bulle(resp, "Courbe de pression", "L'appui du stylet passe par cette courbe avant tout le reste. Glisser un point ; double-clic : ajouter ; clic droit : retirer.", "");
            ui.vertical(|ui| {
                if ui.small_button("Droite").on_hover_text("L'appui tel quel.").clicked() {
                    t.pressure_curve = Curve::linear();
                }
                if ui.small_button("Douce").on_hover_text("Plus de finesse dans les appuis légers : il faut appuyer pour grossir.").clicked() {
                    t.pressure_curve = Curve::from_points(&[[0.0, 0.0], [0.5, 0.35], [1.0, 1.0]]);
                }
                if ui.small_button("Ferme").on_hover_text("Le trait prend du corps dès qu'on effleure.").clicked() {
                    t.pressure_curve = Curve::from_points(&[[0.0, 0.0], [0.5, 0.65], [1.0, 1.0]]);
                }
            });
        });
        let resp = ui.add(egui::Slider::new(&mut t.size_floor, 0.0..=0.5).text("Plancher de taille"));
        bulle(resp, "Plancher de taille", "Part de la taille au-dessous de laquelle le trait ne descend jamais quand la taille suit l'appui (ancien : 3 %).", "");
        let resp = ui.add(egui::Slider::new(&mut t.threshold, 0.0..=0.2).text("Seuil d'appui"));
        bulle(resp, "Seuil d'appui", "Au-dessous, le stylet ne pose rien : ni début fantôme ni pointe au levé (0 : dès le premier échantillon). La souris n'est pas concernée.", "");
        ui.checkbox(&mut t.thin_full, "Traits fins pleins").on_hover_text("Plus fin qu'un pixel : un pixel plein plutôt qu'un fil gris (ancien : éclairci).");
        ui.checkbox(&mut t.soft_edge, "Bord doux").on_hover_text("Bord des empreintes rondes en une seule chute douce d'au moins un pixel (ancien : rampe d'un pixel coupée par la dureté, un léger cran, des petites tailles grises).");
        ui.horizontal(|ui| {
            if ui.small_button("Nouveau toucher").on_hover_text("Les réglages de l'étude.").clicked() {
                *t = Toucher::default();
            }
            if ui.small_button("Ancien toucher").on_hover_text("Le comportement d'avant, pour comparer.").clicked() {
                *t = Toucher::ancien();
            }
        });
        note(ui, "Réponse à la pression commune à tous les pinceaux, appliquée avant leurs réglages. « Ancien » : le toucher d'avant l'étude, pour comparer.");
    }

    // ------------------------------------------------------------------ résumé du panneau Réglages de l'outil

    /// Le haut du panneau Réglages de l'outil pour le pinceau, le crayon et la gomme : la taille en
    /// premier, en grand (Anthony, 2026-10-18 : elle n'est plus que là, plus dans le panneau
    /// Pinceaux), l'opacité, puis le nom du réglage. La liste des brosses se déplie sous l'aperçu du trait
    /// (`brush_settings_ui`, bouton à droite de l'aperçu). La taille et l'opacité selon l'appui ne sont que dans « Plus de réglages » (Anthony,
    /// 2026-10-22 : elles y étaient déjà, en double ici).
    pub(crate) fn brush_summary_ui(&mut self, ui: &mut egui::Ui) {
        self.size_row(ui);
        let target = self.panel_target();
        let current = self.panel_brush();
        let exact = choisi(&self.brushes.presets, target, &current);
        let name = exact.map_or("(réglé à la main)".to_string(), |i| self.brushes.presets[i].name.clone());
        let base = self.base_preset(target);
        let s = self.panel_brush_mut();
        ui.add(egui::Slider::new(&mut s.opacity, 0.0..=1.0).text("Opacité"));
        ui.horizontal(|ui| {
            ui.label(RichText::new(name).strong());
            // Retouché à la main : on rappelle d'après quelle brosse (c'est elle que « Enregistrer » remplace).
            if let (None, Some(b)) = (exact, base) {
                ui.label(RichText::new(format!("d'après « {} »", self.brushes.presets[b].name)).small().weak());
            }
            self.save_brush_button(ui, target, current, exact, base);
        });
        self.preset_edit_row(ui, target, current, true);
    }

    /// Le dernier réglage choisi dans la bibliothèque (ou gardé sous un nom), s'il existe encore.
    pub(crate) fn base_preset(&self, target: Target) -> Option<usize> {
        let (t, name, source) = self.brush_previews.base.as_ref()?;
        if *t != target {
            return None;
        }
        self.brushes.presets.iter().position(|p| p.target == target && p.name == *name && p.source == *source)
    }

    /// L'icône « Enregistrer » du panneau Réglages : garder le réglage courant dans la brosse dont il
    /// vient (la remplacer), ou comme nouvelle brosse ; rétablir l'ancien réglage qu'on vient de
    /// remplacer. Les brosses fournies par SHOT ne se remplacent pas.
    fn save_brush_button(&mut self, ui: &mut egui::Ui, target: Target, current: BrushSettings, exact: Option<usize>, base: Option<usize>) {
        // La brosse que « Enregistrer » remplacerait : celle d'où vient le réglage.
        let over = exact.or(base).filter(|&i| supprimable(&self.brushes.presets[i]));
        let over_name = over.map(|i| self.brushes.presets[i].name.clone());
        let differs = over.is_some_and(|i| self.brushes.presets[i].settings != current);
        let ancien = self.brush_previews.ancien.clone().filter(|a| a.target == target && self.brushes.presets.iter().any(|p| p.target == a.target && p.name == a.name && p.source == a.source && p.settings == a.after));
        let mut action = None;
        let resp = ui.menu_button(RichText::new(ph::FLOPPY_DISK).size(crate::bulles::TAILLE), |ui| {
            let label = match &over_name {
                Some(n) => format!("Enregistrer dans « {n} »"),
                None => "Enregistrer dans la brosse".to_string(),
            };
            let hint = if over.is_some() {
                "Remplace le réglage de cette brosse par le réglage courant (on peut le rétablir juste après)."
            } else {
                "Choisis d'abord une brosse de « Mes brosses » ou importée : celles de FEANOR ne se remplacent pas (utilise « comme nouvelle brosse »)."
            };
            if ui.add_enabled(over.is_some() && differs, egui::Button::new(label)).on_hover_text(hint).on_disabled_hover_text(if over.is_some() { "Rien à enregistrer : la brosse a déjà ce réglage." } else { hint }).clicked() {
                action = Some(0);
                ui.close_menu();
            }
            if ui.button("Enregistrer comme nouvelle brosse…").on_hover_text("Garde le réglage courant sous un nom, dans « Mes brosses ».").clicked() {
                action = Some(1);
                ui.close_menu();
            }
            if let Some(a) = &ancien {
                if ui.button(format!("Rétablir l'ancien réglage de « {} »", a.name)).on_hover_text("Annule le dernier « Enregistrer dans… ».").clicked() {
                    action = Some(2);
                    ui.close_menu();
                }
            }
        });
        let tip = if differs { "Le réglage a changé : l'enregistrer dans sa brosse, ou comme nouvelle brosse." } else { "Garder les réglages de ce pinceau dans une brosse (la remplacer, ou en faire une nouvelle)." };
        bulle(resp.response, "Enregistrer la brosse", tip, "");
        match action {
            Some(0) => {
                if let Some(i) = over {
                    self.apply_preset_menu(i, crate::brushui::PresetMenu::Update, target, current);
                }
            }
            Some(1) => self.start_save_preset_from_summary(target),
            Some(2) => self.restore_preset(),
            _ => {}
        }
    }
}

/// Dureté, pixels nets et profil de la pointe (moteur de dessin, étape 3).
fn hardness_ui(ui: &mut egui::Ui, s: &mut BrushSettings, vector: bool) {
    ui.horizontal(|ui| {
        if (!s.aliased || vector) && s.profile.is_none() {
            ui.add(egui::Slider::new(&mut s.hardness, 0.0..=1.0).text("Dureté"));
        }
        if !vector {
            interrupteur(ui, &mut s.aliased, ph::CHECKERBOARD, "Pixels nets", "Sans anticrénelage : un bord net, pixel par pixel (sans dureté).", "");
        }
        // Profil de la pointe (moteur de dessin, étape 3) : il part de la dureté réglée.
        let mut on = s.profile.is_some();
        if interrupteur(ui, &mut on, ph::CHART_LINE, "Profil de la pointe", "Régler en courbe la couverture de la pointe, du centre au bord (remplace la dureté).", "").clicked() {
            s.profile = on.then(|| BrushSettings::profile_from_hardness(s.hardness));
        }
    });
    if let Some(profile) = &mut s.profile {
        ui.horizontal_wrapped(|ui| {
            let resp = crate::dynui::curve_editor(ui, profile, "profil");
            bulle(resp, "Profil de la pointe", "Couverture du centre (à gauche) au bord (à droite). Glisser un point ; double-clic : ajouter ; clic droit : retirer.", "");
            ui.vertical(|ui| {
                let presets: [(&str, &str, &[[f32; 2]]); 3] = [
                    ("Dur", "Plein jusqu'au bord.", &[[0.0, 1.0], [0.9, 1.0], [1.0, 0.0]]),
                    ("Doux", "Plein au centre, chute douce jusqu'au bord.", &[[0.0, 1.0], [0.2, 0.95], [0.5, 0.5], [0.8, 0.05], [1.0, 0.0]]),
                    ("Diffus", "Peu de cœur, un large halo.", &[[0.0, 1.0], [0.15, 0.6], [0.4, 0.25], [0.7, 0.07], [1.0, 0.0]]),
                ];
                for (name, help, pts) in presets {
                    if ui.small_button(name).on_hover_text(help).clicked() {
                        *profile = crate::brush::Curve::from_points(pts);
                    }
                }
            });
        });
    }
}

/// Taille et opacité qui suivent l'appui du stylet, débit, accumulation, et les connexions.
fn size_and_pressure_ui(ui: &mut egui::Ui, s: &mut BrushSettings, vector: bool) {
    ui.horizontal(|ui| {
        interrupteur(ui, &mut s.pressure_size, ph::PEN_NIB, "Taille selon la pression", "Plus on appuie avec le stylet, plus le trait est gros.", "");
        ui.label("Taille selon l'appui");
    });
    if s.pressure_size {
        let resp = ui.add(egui::Slider::new(&mut s.min_size, 0.0..=1.0).text("Taille mini"));
        bulle(resp, "Taille mini", "Taille quand on effleure, en part de la taille.", "");
    }
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut s.opacity, 0.0..=1.0).text("Opacité"));
        interrupteur(ui, &mut s.pressure_opacity, ph::PEN_NIB, "Opacité selon la pression", "Plus on appuie avec le stylet, plus le trait couvre.", "");
    });
    if s.pressure_opacity {
        let resp = ui.add(egui::Slider::new(&mut s.min_opacity, 0.0..=1.0).text("Opacité mini"));
        bulle(resp, "Opacité mini", "Opacité quand on effleure, en part de l'opacité.", "");
    }
    ui.horizontal(|ui| {
        let resp = ui.add(egui::Slider::new(&mut s.power, 0.01..=1.0).text("Débit"));
        bulle(resp, "Débit", "Peinture déposée à chaque empreinte ; l'opacité en reste le plafond.", "");
        interrupteur(ui, &mut s.build_up, ph::STACK, "Accumuler", "Repasser dans le même trait fonce, jusqu'à l'opacité. Éteint : le trait reste uniforme.", "");
    });
    ui.add_space(4.0);
    crate::dynui::dynamics_ui(ui, s, vector, false);
}

/// Lissage (moteur de dessin, étape 5) : son type, le réglage qui va avec, et ses options.
fn smoothing_ui(ui: &mut egui::Ui, s: &mut BrushSettings) {
    use crate::brush::SmoothKind;
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("lissage_type").selected_text(s.smooth_kind.label()).width(fit_w(ui, 120.0)).show_ui(ui, |ui| {
            for k in SmoothKind::ALL {
                ui.selectable_value(&mut s.smooth_kind, k, k.label()).on_hover_text(k.help());
            }
        });
        match s.smooth_kind {
            SmoothKind::Follow => {
                ui.add(egui::Slider::new(&mut s.smoothing, 0.0..=0.95).text("Lissage"));
            }
            SmoothKind::PulledString => {
                ui.add(egui::Slider::new(&mut s.smooth_px, 0.0..=80.0).suffix(" px").text("Corde"));
            }
            SmoothKind::AveragePoints => {
                ui.add(egui::Slider::new(&mut s.smooth_points, 1..=40).text("Points"));
            }
            SmoothKind::AverageDistance => {
                ui.add(egui::Slider::new(&mut s.smooth_px, 0.0..=80.0).suffix(" px").text("Distance"));
            }
        }
    })
    .response
    .on_hover_text(s.smooth_kind.help());
    // Options du lissage (sensation des brosses, §2.3), quand il a un effet.
    if Categorie::Lissage.allumee(s) {
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut s.smooth_deferred, "Après le trait")
                .on_hover_text("Pendant le geste, le trait brut suit la pointe sans retard ; au levé, il est rejoué lissé (le lissage « sans temps réel », ton réglage).");
            if s.smooth_kind == SmoothKind::Follow {
                ui.checkbox(&mut s.smooth_time, "Selon le temps")
                    .on_hover_text("Le Suivi rattrape la pointe selon le temps écoulé, non le nombre d'échantillons : la même douceur quelle que soit la cadence du stylet et de l'image (décoché : comme avant).");
            }
            if matches!(s.smooth_kind, SmoothKind::PulledString | SmoothKind::AverageDistance) {
                ui.checkbox(&mut s.smooth_screen, "Pixels d'écran")
                    .on_hover_text("La corde ou la distance se mesurent à l'écran : la même sous le stylet à tout zoom (décoché : en pixels du dessin, comme avant).");
            }
            egui::ComboBox::from_id_salt("rattrapage").selected_text(s.catch_up.label()).width(fit_w(ui, 150.0)).show_ui(ui, |ui| {
                for c in crate::brush::CatchUp::ALL {
                    ui.selectable_value(&mut s.catch_up, c, c.label()).on_hover_text(c.help());
                }
            });
        });
    }
}
