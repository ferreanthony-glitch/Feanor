//! Réglages nommés des pinceaux (3d-2) : la case d'un réglage avec l'aperçu de son trait et sa
//! taille (6d), le menu d'un réglage (renommer, dupliquer, supprimer), l'import des pinceaux. Le
//! panneau Pinceaux qui les range est dans `pinceauxui.rs`.

use std::collections::HashMap;

use eframe::egui::{self, pos2, vec2, Color32, RichText, Sense, TextureHandle, Vec2};

use crate::app::{App, Tool};
use crate::brush::{stroke_preview, BrushSettings};
use crate::brushtex::{BrushLibrary, Target};

/// Case d'un réglage dans la grille, et l'aperçu du trait qu'elle contient.
const TILE: Vec2 = vec2(58.0, 48.0);
const PREVIEW: Vec2 = vec2(54.0, 30.0);

/// Bulle du bouton « Importer des pinceaux » (panneau Pinceaux).
pub(crate) const IMPORT_HINT: &str = "Brosses (.tvpx, .bin, .ini, .abr, .brush, .brushset, .kpp, .bundle, .myb), plusieurs fichiers à la fois ; elles restent pour les fois suivantes.";

/// Nom d'un réglage en cours de saisie (moteur de dessin, étape 9) : enregistrer le réglage
/// courant dans « Mes brosses », ou renommer le réglage `rename`.
pub(crate) struct PresetEdit {
    pub name: String,
    pub target: Target,
    pub rename: Option<usize>,
    /// La saisie vient de l'icône « Enregistrer » du panneau Réglages (elle s'y montre, pas dans
    /// la bibliothèque).
    pub summary: bool,
    focused: bool,
}

/// Un réglage de la bibliothèque tel qu'il était avant d'être remplacé par le réglage courant
/// (icône « Enregistrer » du panneau Réglages) : de quoi rétablir l'ancien.
#[derive(Clone)]
pub(crate) struct Ancien {
    pub target: Target,
    pub name: String,
    pub source: String,
    pub before: BrushSettings,
    pub after: BrushSettings,
}

/// Ce que demande le menu d'un réglage (clic droit).
#[derive(Clone, Copy)]
pub(crate) enum PresetMenu {
    Rename,
    Duplicate,
    Update,
    Delete,
    /// « Ajouter à Mon panneau » (essai du Labo ; seulement quand ce panneau est ouvert).
    Panneau,
}

/// Le menu d'un réglage : renommer, dupliquer dans « Mes brosses », remplacer par le réglage
/// courant, supprimer. Ceux de Pelure ne se dupliquent que.
pub(crate) fn preset_menu(ui: &mut egui::Ui, source: &str) -> Option<PresetMenu> {
    let mut out = None;
    let own = source != "Pelure";
    if own && ui.button("Renommer…").clicked() {
        out = Some(PresetMenu::Rename);
    }
    if ui.button("Dupliquer dans « Mes brosses »").clicked() {
        out = Some(PresetMenu::Duplicate);
    }
    if own && ui.button("Remplacer par le réglage courant").clicked() {
        out = Some(PresetMenu::Update);
    }
    if own && ui.button("Supprimer").clicked() {
        out = Some(PresetMenu::Delete);
    }
    if crate::monpanneauui::ouvert(ui.ctx()) && ui.button(format!("{}  Ajouter à Mon panneau", egui_phosphor::regular::STAR)).clicked() {
        out = Some(PresetMenu::Panneau);
    }
    if out.is_some() {
        ui.close_menu();
    }
    out
}

/// Aperçus des réglages (par numéro de réglage), refaits quand le réglage ou l'échelle de
/// l'écran change : en vignettes, en ligne de liste, et le trait du pinceau courant ; plus ce que
/// le panneau Pinceaux ne retient pas d'une séance à l'autre (la recherche, les valeurs d'une
/// catégorie éteinte).
#[derive(Default)]
pub(crate) struct BrushPreviews {
    pub(crate) cache: HashMap<usize, (BrushSettings, f32, TextureHandle)>,
    pub(crate) list: HashMap<usize, (BrushSettings, [usize; 2], TextureHandle)>,
    pub(crate) current: Option<(BrushSettings, [usize; 2], TextureHandle)>,
    /// Texte de la recherche par nom.
    pub(crate) search: String,
    /// Les valeurs d'une catégorie au moment où on l'a éteinte (par outil), pour la rallumer.
    pub(crate) memory: HashMap<(crate::pinceaux::Categorie, u8), BrushSettings>,
    /// Le dernier réglage choisi dans la bibliothèque (cible, nom, source) : celui que l'icône
    /// « Enregistrer » du panneau Réglages remplace, même une fois le réglage retouché à la main.
    pub(crate) base: Option<(Target, String, String)>,
    /// Le dernier réglage remplacé par « Enregistrer » : on peut le rétablir.
    pub(crate) ancien: Option<Ancien>,
}

impl App {
    /// Le réglage que le panneau Pinceaux modifie : celui du crayon ou de la gomme quand c'est
    /// l'outil courant, sinon celui du pinceau (même avec un autre outil, comme le choix d'un
    /// réglage de la bibliothèque).
    pub(crate) fn panel_brush_mut(&mut self) -> &mut BrushSettings {
        match self.tool {
            Tool::Pencil => &mut self.pencil,
            Tool::Eraser => &mut self.eraser,
            _ => &mut self.brush,
        }
    }

    fn tool_brush_mut(&mut self) -> Option<&mut BrushSettings> {
        Some(self.panel_brush_mut())
    }

    /// La bibliothèque des pinceaux importés se lit encore (sur un autre fil, depuis le lancement) : on
    /// n'y touche pas pour l'instant, sans quoi la lecture écraserait le changement, ou
    /// l'enregistrement effacerait les pinceaux pas encore arrivés. Dit pourquoi dans la barre d'état.
    pub(crate) fn brushes_busy(&mut self) -> bool {
        if self.brushes_job.is_some() {
            self.status = "Les pinceaux importés se chargent encore : réessaie dans un instant.".into();
            return true;
        }
        false
    }

    /// À chaque image : la bibliothèque complète est-elle lue ? Elle remplace alors celle de SHOT seule
    /// (les aperçus se refont).
    pub(crate) fn poll_brushes(&mut self) {
        if !self.brushes_job.as_ref().is_some_and(|j| j.is_finished()) {
            return;
        }
        let Some(job) = self.brushes_job.take() else { return };
        match job.join() {
            Ok(lib) => {
                self.brushes = lib;
                self.brush_previews = BrushPreviews::default();
                crate::demarrage::marque("pinceaux importés arrivés");
            }
            // (La lecture a planté : les pinceaux de SHOT restent ; rien n'est modifié sur le disque.)
            Err(_) => self.status = "Les pinceaux importés n'ont pas pu être lus ; ceux de FEANOR restent disponibles.".into(),
        }
    }

    /// Garde la bibliothèque (importés et « Mes brosses ») ; l'erreur s'il y en a une.
    pub(crate) fn save_brushes(&self) -> Option<String> {
        match BrushLibrary::user_dir() {
            Some(dir) => self.brushes.save(&dir).err(),
            None => Some("dossier %APPDATA% introuvable".into()),
        }
    }

    /// Commence l'enregistrement du réglage courant dans « Mes brosses ».
    pub(crate) fn start_save_preset(&mut self, target: Target) {
        if self.brushes_busy() {
            return;
        }
        let name = self.brushes.free_name("Ma brosse", target);
        self.preset_edit = Some(PresetEdit { name, target, rename: None, summary: false, focused: false });
    }

    /// Comme `start_save_preset`, depuis l'icône « Enregistrer » du panneau Réglages : le nom se
    /// saisit dans ce panneau.
    pub(crate) fn start_save_preset_from_summary(&mut self, target: Target) {
        self.start_save_preset(target);
        if let Some(e) = self.preset_edit.as_mut() {
            e.summary = true;
        }
    }

    /// La ligne du nom en cours de saisie (enregistrer ou renommer), s'il y en a une pour cet outil
    /// (`summary` : dans le panneau Réglages ou dans la bibliothèque, selon d'où elle est partie).
    pub(crate) fn preset_edit_row(&mut self, ui: &mut egui::Ui, target: Target, current: BrushSettings, summary: bool) {
        let Some(edit) = self.preset_edit.as_mut().filter(|e| e.target == target && e.summary == summary) else { return };
        let mut done = None;
        ui.horizontal(|ui| {
            ui.label(if edit.rename.is_some() { "Nouveau nom" } else { "Enregistrer sous" });
            let resp = ui.add(egui::TextEdit::singleline(&mut edit.name).desired_width(crate::dockui::fit_w(ui, 140.0)));
            if !edit.focused {
                resp.request_focus();
                edit.focused = true;
            }
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("OK").clicked() || enter {
                done = Some(true);
            }
            if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                done = Some(false);
            }
        });
        if done == Some(true) {
            let Some(edit) = self.preset_edit.take() else { return };
            let name = edit.name.trim().to_string();
            match edit.rename {
                Some(i) if i < self.brushes.presets.len() && !name.is_empty() => self.brushes.presets[i].name = name.clone(),
                Some(_) => {}
                None => {
                    self.brushes.save_mine(&name, target, current);
                }
            }
            let err = self.save_brushes();
            self.status = format!("Réglage « {name} » gardé dans « Mes brosses »{}", err.map_or(String::new(), |e| format!(" (pas pour la prochaine fois : {e})")));
            // Le réglage qu'on vient de garder devient celui que « Enregistrer » remplacera.
            self.brush_previews.base = Some((target, name, crate::brushtex::MINE.to_string()));
        } else if done == Some(false) {
            self.preset_edit = None;
        }
    }

    /// Fait ce que demande le menu du réglage `i`.
    pub(crate) fn apply_preset_menu(&mut self, i: usize, action: PresetMenu, target: Target, current: BrushSettings) {
        // Mon panneau ne touche pas à la bibliothèque : il en garde seulement le nom.
        if matches!(action, PresetMenu::Panneau) {
            self.mon_panneau_ajouter_brosse(i);
            return;
        }
        if self.brushes_busy() {
            return;
        }
        let Some(p) = self.brushes.presets.get(i).cloned() else { return };
        match action {
            PresetMenu::Rename => {
                self.preset_edit = Some(PresetEdit { name: p.name, target, rename: Some(i), summary: false, focused: false });
                return;
            }
            PresetMenu::Duplicate => {
                let name = self.brushes.free_name(&p.name, target);
                self.brushes.save_mine(&name, target, p.settings);
                self.status = format!("« {name} » ajouté à « Mes brosses ».");
            }
            PresetMenu::Update => {
                self.brushes.presets[i].settings = current;
                self.brush_previews.ancien = Some(Ancien { target: p.target, name: p.name.clone(), source: p.source.clone(), before: p.settings, after: current });
                self.brush_previews.base = Some((p.target, p.name.clone(), p.source.clone()));
                self.status = format!("« {} » prend le réglage courant.", p.name);
            }
            PresetMenu::Delete => {
                self.brushes.presets.remove(i);
                // Les aperçus sont rangés par rang : ils se refont.
                self.brush_previews = BrushPreviews::default();
                self.status = format!("« {} » supprimé.", p.name);
            }
            PresetMenu::Panneau => {}
        }
        if let Some(e) = self.save_brushes() {
            self.status = format!("{} (pas gardé pour la prochaine fois : {e})", self.status);
        }
    }

    /// Rétablit le réglage de la bibliothèque que « Enregistrer » vient de remplacer.
    pub(crate) fn restore_preset(&mut self) {
        if self.brushes_busy() {
            return;
        }
        let Some(a) = self.brush_previews.ancien.take() else { return };
        let Some(p) = self.brushes.presets.iter_mut().find(|p| p.target == a.target && p.name == a.name && p.source == a.source) else { return };
        p.settings = a.before;
        self.status = format!("« {} » retrouve son ancien réglage.", a.name);
        if let Some(e) = self.save_brushes() {
            self.status = format!("{} (pas gardé pour la prochaine fois : {e})", self.status);
        }
    }

    /// Case d'un réglage de la grille : aperçu du trait et taille ; nom et provenance au survol.
    pub(crate) fn preset_tile(&mut self, ui: &mut egui::Ui, index: usize, name: &str, source: &str, current: BrushSettings) -> egui::Response {
        let settings = self.brushes.presets[index].settings;
        let ppp = ui.ctx().pixels_per_point();
        let fresh = self.brush_previews.cache.get(&index).is_some_and(|(s, p, _)| *s == settings && *p == ppp);
        if !fresh {
            let (w, h) = ((PREVIEW.x * ppp).round() as usize, (PREVIEW.y * ppp).round() as usize);
            let alpha = stroke_preview(&settings, self.brushes.textures(&settings), w, h);
            let pixels = alpha.iter().map(|&a| Color32::from_rgba_premultiplied(a, a, a, a)).collect();
            let tex = ui.ctx().load_texture(
                format!("pinceau {index}"),
                egui::ColorImage { size: [w, h], pixels },
                egui::TextureOptions::LINEAR,
            );
            self.brush_previews.cache.insert(index, (settings, ppp, tex));
        }
        let tex = self.brush_previews.cache[&index].2.id();
        let (rect, resp) = ui.allocate_exact_size(TILE, Sense::click());
        let v = ui.visuals();
        let chosen = settings == current;
        let bg = if chosen {
            v.selection.bg_fill
        } else if resp.hovered() {
            v.widgets.hovered.weak_bg_fill
        } else {
            v.extreme_bg_color
        };
        let ink = if chosen { v.selection.stroke.color } else { v.text_color() };
        let painter = ui.painter();
        painter.rect_filled(rect, 4.0, bg);
        let image = egui::Rect::from_min_size(rect.min + vec2((TILE.x - PREVIEW.x) * 0.5, 2.0), PREVIEW);
        let (papier, trait_) = self.preview_colors(index);
        painter.rect_filled(image, 2.0, papier);
        painter.image(tex, image, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), trait_);
        let size = if settings.size < 10.0 { format!("{:.1}", settings.size) } else { format!("{:.0}", settings.size) };
        painter.text(
            pos2(rect.center().x, rect.bottom() - 8.0),
            egui::Align2::CENTER_CENTER,
            size,
            egui::FontId::proportional(11.0),
            ink,
        );
        crate::bulles::bulle(resp, &name, &source, "")
    }

    /// Couleurs d'un aperçu (12b) : le papier du projet, et le trait dans la couleur courante ; une
    /// gomme en gris (le trait qu'elle enlève).
    pub(crate) fn preview_colors(&self, index: usize) -> (Color32, Color32) {
        let [r, g, b] = self.doc.paper;
        let gomme = self.brushes.presets.get(index).is_some_and(|p| p.target == Target::Eraser);
        let [cr, cg, cb] = self.color;
        let trait_ = if gomme { Color32::from_gray(150) } else { Color32::from_rgb(cr, cg, cb) };
        (Color32::from_rgb(r, g, b), trait_)
    }

    /// Une image choisie devient une pointe de la bibliothèque, gardée pour les fois suivantes, et
    /// la pointe de l'outil courant.
    pub(crate) fn import_tip(&mut self) {
        if self.brushes_busy() {
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title("Image à prendre comme pointe")
            .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tif", "tiff", "webp"])
            .pick_file()
        else {
            return;
        };
        let img = match image::open(&path) {
            Ok(i) => i,
            Err(e) => {
                self.status = format!("Image illisible : {e}");
                return;
            }
        };
        let name = path.file_stem().map_or("pointe".into(), |n| n.to_string_lossy().into_owned());
        let id = self.brushes.add_tip(&name, crate::brushtex::GrayImage::tip_from_image(&img, 2048));
        self.brushes.share();
        if let Some(b) = self.tool_brush_mut() {
            b.tip = Some(id);
            b.tip_cells = 1;
        }
        let saved = match BrushLibrary::user_dir() {
            Some(dir) => self.brushes.save(&dir).err(),
            None => Some("dossier %APPDATA% introuvable".into()),
        };
        self.status = format!(
            "Pointe « {name} » importée (plusieurs dessins côte à côte : régler « Cases »){}",
            saved.map_or(String::new(), |e| format!(" ; non gardée pour la prochaine fois : {e}"))
        );
    }

    /// Importe des brosses TVPaint ; le compte rendu (ce qui n'est pas repris) va dans la fenêtre
    /// des remarques d'import.
    fn import_tvpaint(&mut self, path: &std::path::Path, ext: &str) {
        let source = path.file_name().map_or("import".into(), |n| n.to_string_lossy().into_owned());
        self.brushes.presets.retain(|p| p.source != source);
        let res = crate::tvpbrush::tvpaint_resources();
        let result = match ext {
            "tvpx" => std::fs::read(path)
                .map_err(|e| format!("lecture impossible : {e}"))
                .and_then(|b| crate::tvpbrush::Panel::from_tvpx(&b))
                .map(|p| crate::tvpbrush::import_panel(&p, &source, &mut self.brushes, res.as_deref())),
            "bin" => crate::tvpbrush::Panel::from_bin(path).map(|p| crate::tvpbrush::import_panel(&p, &source, &mut self.brushes, res.as_deref())),
            _ => crate::tvpbrush::import_ini(path, &source, &mut self.brushes),
        };
        match result {
            Ok(r) => {
                self.brushes.share();
                self.brush_previews = BrushPreviews::default();
                let saved = self.save_brushes();
                self.status = format!(
                    "{} brosse(s) importée(s) ({} pointe(s), {} papier(s)){}{}",
                    r.presets,
                    r.tips,
                    r.papers,
                    if r.notes.is_empty() { String::new() } else { format!(" ; {} remarque(s) dans la fenêtre d'import", r.notes.len()) },
                    saved.map_or(String::new(), |e| format!(" ; non gardées pour la prochaine fois : {e}")),
                );
                self.brush_import_notes = (!r.notes.is_empty()).then(|| (source, r.notes));
            }
            Err(e) => self.status = format!("Import impossible : {e}"),
        }
    }

    /// La fenêtre des remarques du dernier import de brosses (ce qui n'a pas pu être repris).
    pub(crate) fn brush_import_window(&mut self, ctx: &egui::Context) {
        let Some((source, notes)) = &self.brush_import_notes else { return };
        let mut open = true;
        egui::Window::new(format!("Import de « {source} »")).open(&mut open).resizable(true).default_width(460.0).show(ctx, |ui| {
            ui.label(RichText::new("Ce qui n'a pas pu être repris tel quel :").strong());
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for n in notes {
                    ui.label(format!("• {n}"));
                }
            });
        });
        if !open {
            self.brush_import_notes = None;
        }
    }

    /// Une image choisie devient un papier de la bibliothèque, gardé pour les fois suivantes, et
    /// le papier de l'outil courant.
    pub(crate) fn import_paper(&mut self) {
        if self.brushes_busy() {
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title("Image à prendre comme papier")
            .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tif", "tiff", "webp"])
            .pick_file()
        else {
            return;
        };
        let img = match image::open(&path) {
            Ok(i) => i,
            Err(e) => {
                self.status = format!("Image illisible : {e}");
                return;
            }
        };
        let name = path.file_stem().map_or("papier".into(), |n| n.to_string_lossy().into_owned());
        let id = self.brushes.add_paper(&name, crate::brushtex::GrayImage::from_image(&img, 1024));
        self.brushes.share();
        if let Some(b) = self.tool_brush_mut() {
            b.paper = Some(id);
            if b.grain <= 0.0 {
                b.grain = 1.0;
            }
        }
        let saved = match BrushLibrary::user_dir() {
            Some(dir) => self.brushes.save(&dir).err(),
            None => Some("dossier %APPDATA% introuvable".into()),
        };
        self.status = format!("Papier « {name} » importé{}", saved.map_or(String::new(), |e| format!(" ; non gardé pour la prochaine fois : {e}")));
    }

    /// Importe un fichier de brosses : TVPaint (`.tvpx`, `.bin`, `.ini`, moteur de dessin,
    /// étape 10) Photoshop, Procreate, Krita ou MyPaint ; réimporter le même fichier remplace ses pinceaux.
    pub(crate) fn import_brushes(&mut self) {
        if self.brushes_busy() {
            return;
        }
        let Some(paths) = rfd::FileDialog::new()
            .set_title("Brosses à importer")
            .add_filter("Toutes les brosses (.tvpx, .bin, .ini, .abr, .brush, .brushset, .kpp, .bundle, .myb)", &["tvpx", "bin", "ini", "abr", "brush", "brushset", "kpp", "bundle", "myb"])
            .add_filter("Brosses (.abr)", &["abr"])
            .add_filter("Brosses (.brush, .brushset)", &["brush", "brushset"])
            .add_filter("Panneaux et réglages de brosses (.tvpx, .bin, .ini)", &["tvpx", "bin", "ini"])
            .add_filter("Réglages et paquets de brosses (.kpp, .bundle, .myb)", &["kpp", "bundle", "myb"])
            .pick_files()
        else {
            return;
        };
        // Krita et MyPaint : un réglage par fichier, souvent plusieurs à la fois.
        let (krita, others): (Vec<_>, Vec<_>) = paths.into_iter().partition(|p| {
            let e = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            e == "kpp" || e == "myb" || e == "bundle"
        });
        if !krita.is_empty() {
            self.import_krita(&krita);
        }
        for path in others {
            self.import_brush_file(&path);
        }
    }

    /// Importe des réglages Krita (`.kpp`) et MyPaint (`.myb`) ; un compte rendu commun.
    fn import_krita(&mut self, paths: &[std::path::PathBuf]) {
        let (mut presets, mut tips, mut notes, mut errors) = (0, 0, Vec::new(), Vec::new());
        // Les pointes en image d'un réglage Krita : dans le dossier « brushes » de Krita, ou dans
        // ses paquets (`.bundle`), à côté du dossier des réglages.
        let mut bundles: Vec<Vec<u8>> = Vec::new();
        if let Some(root) = paths.iter().find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("kpp"))).and_then(|p| p.parent()).and_then(|d| d.parent()) {
            if let Ok(rd) = std::fs::read_dir(root) {
                bundles = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("bundle"))).filter_map(|p| std::fs::read(p).ok()).collect();
            }
        }
        let zips: Vec<crate::zipread::Zip> = bundles.iter().filter_map(|b| crate::zipread::Zip::open(b).ok()).collect();
        for path in paths {
            let source = path.file_name().map_or("import".into(), |n| n.to_string_lossy().into_owned());
            self.brushes.presets.retain(|p| p.source != source);
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let result = if ext == "kpp" {
                let brushes = path.parent().and_then(|d| d.parent()).map(|d| d.join("brushes"));
                let find = |f: &str| -> Option<Vec<u8>> { brushes.as_ref().and_then(|d| std::fs::read(d.join(f)).ok()).or_else(|| zips.iter().find_map(|z| z.find(f))) };
                std::fs::read(path)
                    .map_err(|e| format!("lecture impossible : {e}"))
                    .and_then(|b| crate::kritabrush::import_kpp_with(&b, &source, &mut self.brushes, &find))
            } else if ext == "bundle" {
                std::fs::read(path).map_err(|e| format!("lecture impossible : {e}")).and_then(|b| crate::kritabrush::import_bundle(&b, &source, &mut self.brushes))
            } else {
                let name = path.file_stem().map_or("Brosse importée".into(), |n| n.to_string_lossy().into_owned());
                std::fs::read_to_string(path).map_err(|e| format!("lecture impossible : {e}")).and_then(|t| crate::kritabrush::import_myb(&t, &name, &source, &mut self.brushes))
            };
            match result {
                Ok(r) => {
                    presets += r.presets;
                    tips += r.tips;
                    notes.extend(r.notes);
                }
                Err(e) => errors.push(format!("{source} : {e}")),
            }
        }
        self.brushes.share();
        self.brush_previews = BrushPreviews::default();
        let saved = self.save_brushes();
        notes.extend(errors.iter().map(|e| format!("non importé — {e}")));
        self.status = format!(
            "{presets} réglage(s) importé(s) ({tips} pointe(s)){}{}",
            if notes.is_empty() { String::new() } else { format!(" ; {} remarque(s) dans la fenêtre d'import", notes.len()) },
            saved.map_or(String::new(), |e| format!(" ; non gardés pour la prochaine fois : {e}")),
        );
        let title = if paths.len() == 1 { paths[0].file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned()) } else { format!("{} fichiers", paths.len()) };
        self.brush_import_notes = (!notes.is_empty()).then_some((title, notes));
    }

    /// Importe un fichier de brosses (TVPaint, Photoshop, Procreate).
    fn import_brush_file(&mut self, path: &std::path::Path) {
        let path = path.to_path_buf();
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if matches!(ext.as_str(), "tvpx" | "bin" | "ini") {
            self.import_tvpaint(&path, &ext);
            return;
        }
        if matches!(ext.as_str(), "abr" | "brush" | "brushset") {
            let source = path.file_name().map_or("import".into(), |n| n.to_string_lossy().into_owned());
            self.brushes.presets.retain(|p| p.source != source);
            let read = std::fs::read(&path).map_err(|e| format!("lecture impossible : {e}"));
            let result = if ext == "abr" {
                read.and_then(|b| crate::abr::import(&b, &source, &mut self.brushes))
            } else {
                read.and_then(|b| crate::procreate::import(&b, &source, &mut self.brushes)).map(|(n, t, _, notes)| (n, t, notes))
            };
            let what = if ext == "abr" { ".abr" } else { ".brush" };
            match result {
                Ok((n, tips, notes)) => {
                    self.brushes.share();
                    self.brush_previews = BrushPreviews::default();
                    let saved = self.save_brushes();
                    self.status = format!(
                        "{n} brosse(s) {what} importée(s) ({tips} pointe(s)){}{}",
                        if notes.is_empty() { String::new() } else { format!(" ; {} remarque(s) dans la fenêtre d'import", notes.len()) },
                        saved.map_or(String::new(), |e| format!(" ; non gardées pour la prochaine fois : {e}")),
                    );
                    self.brush_import_notes = (!notes.is_empty()).then_some((source, notes));
                }
                Err(e) => self.status = format!("Import impossible : {e}"),
            }
            return;
        }
        self.status = format!("Import impossible : le format de « {} » n'est pas reconnu.", path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned()));
    }
}
