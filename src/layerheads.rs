//! En-tête des calques dans la timeline (13b), sur le modèle de TVPaint : une ligne par calque,
//! le plus haut en premier. Ligne complète : triangle pour la réduire, bande de la couleur de son
//! groupe, vignette du dessin courant, nom et menu d'options ; dessous les réglages en icônes
//! (afficher, verrouiller, pelure d'oignon, préserver la transparence, pochoir et son sens,
//! position verrouillée, groupe) ; dessous encore la barre d'opacité et le mode de fusion
//! (petits triangles : mode précédent / suivant). Ligne réduite : afficher, nom, verrou.
//! Glisser une ligne change le rang du calque ; double-clic sur le nom : le renommer.

use eframe::egui::{self, pos2, vec2, Align, Align2, Color32, FontId, Layout, Rect, RichText, Sense, Stroke, StrokeKind, Ui};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::blend::BlendMode;
use crate::bulles::bulle;
use crate::document::{Behaviour, Layer, LayerKind};
use crate::groups::LayerGroup;
use crate::theme;

/// Largeur de l'en-tête des calques.
pub(crate) const HEAD_W: f32 = 312.0;
/// Hauteur d'une ligne réduite (une ligne de texte).
pub(crate) const ROW_SMALL: f32 = 24.0;
/// Hauteur d'une ligne complète : nom, réglages, opacité et mode.
pub(crate) const ROW_FULL: f32 = 56.0;
/// Hauteur de la ligne d'un dossier (13b-5) : nom ; afficher, verrouiller, opacité.
pub(crate) const ROW_FOLDER: f32 = 40.0;
/// Décalage des lignes des calques rangés dans un dossier.
const INDENT: f32 = 12.0;
/// Largeur d'une icône de réglage.
const ICON_W: f32 = 19.0;

/// Une ligne de calque de la timeline : le calque, le haut de la ligne (depuis le haut des
/// lignes) et sa hauteur.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Row {
    pub li: usize,
    pub top: f32,
    pub h: f32,
}

/// Lignes des calques, du plus haut au plus bas ; les calques d'un groupe caché de la timeline
/// (filtre de groupe, 13b-4) ou d'un dossier fermé (13b-5) n'en ont pas.
pub(crate) fn rows_of(layers: &[Layer], groups: &[LayerGroup]) -> Vec<Row> {
    let closed: Vec<u64> = layers.iter().filter(|l| l.folder && l.collapsed).map(|l| l.id).collect();
    let mut top = 0.0;
    layers
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, l)| groups.get(l.group as usize).is_none_or(|g| g.shown))
        .filter(|(_, l)| l.parent.is_none_or(|p| !closed.contains(&p)))
        .map(|(li, l)| {
            let h = match (l.folder, l.collapsed) {
                (true, _) => ROW_FOLDER,
                (false, true) => ROW_SMALL,
                (false, false) => ROW_FULL,
            };
            let row = Row { li, top, h };
            top += h;
            row
        })
        .collect()
}

/// Hauteur de toutes les lignes.
pub(crate) fn rows_height(rows: &[Row]) -> f32 {
    rows.iter().map(|r| r.h).sum()
}

/// Ligne sous `y` (depuis le haut des lignes).
pub(crate) fn row_at(rows: &[Row], y: f32) -> Option<Row> {
    rows.iter().copied().find(|r| y >= r.top && y < r.top + r.h)
}

/// Où tombe ce qu'on glisse dans une liste de calques (panneau Calques, colonne de la bande du montage, en-têtes de la timeline),
/// lu sous le pointeur (Anthony, 2026-11-11 : « glisser des calques dans un dossier ne fonctionne pas »).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Cible {
    /// Entre deux lignes : où poser, et l'ordonnée du trait d'insertion (depuis le haut des lignes).
    Trait(crate::folders::Pose, f32),
    /// Sur la ligne d'un dossier : dedans. Le haut et la hauteur de sa ligne, pour l'encadrer.
    Dossier(crate::folders::Pose, f32, f32),
}

/// Le haut de la ligne d'un dossier qui reste « au-dessus de lui » pour un glissé (le quart de la ligne si elle est plus fine) : le
/// reste de la ligne est « dedans ».
pub(crate) const BORD_DU_DOSSIER: f32 = 6.0;

impl Cible {
    pub(crate) fn pose(&self) -> crate::folders::Pose {
        match *self {
            Cible::Trait(p, _) | Cible::Dossier(p, _, _) => p,
        }
    }
}

/// La cible d'un glissé lâché à `y` (depuis le haut des lignes). Sur la ligne d'un dossier, hors de son bord du haut (six points au
/// plus, qui restent « au-dessus de lui ») : dans le dossier, qu'il soit ouvert ou fermé, vide ou non (`dans_un_dossier` : faux quand c'est un dossier
/// qu'on glisse, un dossier n'entre jamais dans un dossier). Sinon entre les deux lignes dont les milieux encadrent `y` : juste
/// au-dessus du calque de la ligne d'en dessous, ou juste sous la dernière ligne (des calques sans ligne, groupe caché ou dossier
/// fermé, peuvent être entre deux lignes : ils ne bougent pas). Rien sur la ligne d'un des calques glissés (`parts`, leurs rangs) :
/// un clic qui a tremblé ne range ni ne sort rien.
pub(crate) fn cible_du_glisse(rows: &[Row], layers: &[Layer], parts: &[usize], y: f32, dans_un_dossier: bool) -> Option<Cible> {
    use crate::folders::Pose;
    let last = rows.last()?;
    if let Some(r) = row_at(rows, y) {
        if parts.contains(&r.li) {
            return None;
        }
        if dans_un_dossier && layers.get(r.li).is_some_and(|l| l.folder) && y >= r.top + (r.h * 0.25).min(BORD_DU_DOSSIER) {
            return Some(Cible::Dossier(Pose::Dans(layers[r.li].id), r.top, r.h));
        }
    }
    let slot = rows.iter().filter(|r| r.top + r.h * 0.5 < y).count();
    Some(match rows.get(slot) {
        Some(dessous) => Cible::Trait(Pose::AuDessusDe(layers.get(dessous.li)?.id), dessous.top),
        None => Cible::Trait(Pose::Sous(layers.get(last.li)?.id), last.top + last.h),
    })
}

/// Montre où ce qu'on glisse va se poser : un trait entre deux lignes, en retrait de `retrait` points quand il tombe dans un dossier
/// (`dans`), ou le cadre de la ligne du dossier visé. `x` : la largeur de la liste ; `top` : l'ordonnée écran du haut des lignes.
pub(crate) fn peindre_cible(painter: &egui::Painter, x: egui::Rangef, top: f32, cible: Cible, dans: bool, retrait: f32) {
    let trait_ = Stroke::new(2.0_f32, theme::contour());
    match cible {
        Cible::Trait(_, y) => {
            let gauche = x.min + if dans { retrait } else { 0.0 };
            painter.line_segment([pos2(gauche, top + y), pos2(x.max, top + y)], trait_);
            if dans {
                // (Un petit crochet : le trait part du bord du dossier.)
                painter.line_segment([pos2(gauche, top + y - 4.0), pos2(gauche, top + y + 4.0)], trait_);
            }
        }
        Cible::Dossier(_, haut, h) => {
            let r = Rect::from_min_max(pos2(x.min, top + haut), pos2(x.max, top + haut + h));
            painter.rect(r, 3.0, theme::contour().gamma_multiply(0.18), trait_, StrokeKind::Inside);
        }
    }
}

/// Couleur d'un groupe.
pub(crate) fn group_color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Icône sans cadre posée dans `rect`.
pub(crate) fn icon_at(ui: &mut Ui, rect: Rect, icon: &str, color: Color32) -> egui::Response {
    ui.put(rect, egui::Button::new(RichText::new(icon).size(13.0).color(color)).frame(false))
}

/// Liste des modes de fusion posée dans `rect`, aussi basse que la barre d'opacité (en-têtes de
/// la timeline et du montage) : le mode choisi, s'il a changé.
pub(crate) fn blend_combo(ui: &mut Ui, rect: Rect, salt: egui::Id, blend: BlendMode) -> Option<BlendMode> {
    let mut mode = blend;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(salt.with("cadre")).layout(Layout::left_to_right(Align::Center)));
    // Aussi bas que la barre d'opacité, pour tenir dans la ligne.
    child.spacing_mut().interact_size.y = 16.0;
    child.spacing_mut().button_padding = vec2(4.0, 0.0);
    let resp = egui::ComboBox::from_id_salt(salt)
        .selected_text(RichText::new(blend.label()).size(11.0))
        .width(rect.width() - 4.0)
        .show_ui(&mut child, |ui| {
            for m in BlendMode::ALL {
                if BlendMode::FAMILY_STARTS.contains(&m) {
                    ui.separator();
                }
                ui.selectable_value(&mut mode, m, m.label()).on_hover_text(m.hint());
            }
        })
        .response;
    bulle(resp, "Mode de fusion", blend.hint(), "");
    (mode != blend).then_some(mode)
}

/// Ce que la ligne d'un calque montre (copié pour ne pas garder le calque emprunté).
struct Head {
    id: u64,
    name: String,
    kind: LayerKind,
    /// Calque d'effet (`calqueffet.rs`) : une étincelle cyan à la place de la lettre.
    cfx: bool,
    /// Calque d'annotation (`calqueannot.rs`) : un feutre rouge-rose à la place de la lettre.
    annotation: bool,
    /// Calque Couleur unie (`couleurunie.rs`) : un carre vert citron a la place de la lettre.
    unie: bool,
    /// Calque de couleur (`calquecouleur.rs`) : la lettre C, rose.
    couleur: bool,
    /// Calque de couleur lié à un calque de trait : la ligne est décalée sous son trait (un calque libre garde sa place).
    sous_trait: bool,
    visible: bool,
    locked: bool,
    onion: bool,
    keep_alpha: bool,
    stencil: bool,
    negative: bool,
    fixed: bool,
    /// Accroché à la caméra (`accroche.rs`).
    accroche: bool,
    group: usize,
    collapsed: bool,
    opacity: f32,
    blend: BlendMode,
    cel: Option<crate::document::CelId>,
    /// Dossier (13b-5), et nombre de ses calques.
    folder: bool,
    children: usize,
    /// Rangé dans un dossier : la ligne est décalée vers la droite.
    in_folder: bool,
}

impl App {
    /// L'en-tête des calques, à gauche de la grille de la timeline : bande du haut (`header_h`),
    /// une ligne par calque, puis la place des lignes caméra et son (`below`).
    pub(crate) fn timeline_heads(&mut self, ui: &mut Ui, header_h: f32, below: f32) -> Rect {
        let rows = rows_of(self.layers(), &self.doc.groups);
        let size = vec2(HEAD_W, header_h + rows_height(&rows) + below);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(Rect::from_min_size(rect.min, vec2(HEAD_W, header_h)), 0.0, theme::regle());
        // Le calque courant est dans un groupe caché de la timeline : on le dit, et un clic sur
        // la couleur de son groupe le remontre.
        let hidden_group = self.layers().get(self.cur_layer).map(|l| l.group as usize).filter(|&g| !self.doc.groups[g].shown);
        // Ou dans un dossier fermé (13b-5) : un clic sur le dossier le rouvre.
        let closed_folder = crate::folders::parent_index(self.layers(), self.cur_layer).filter(|&fi| self.layers()[fi].collapsed);
        match (hidden_group, closed_folder) {
            (None, Some(fi)) => {
                let name = self.layers()[fi].name.clone();
                let r = Rect::from_min_size(pos2(rect.left() + 4.0, rect.top()), vec2(HEAD_W - 100.0, header_h));
                let resp = ui.interact(r, egui::Id::new("dossier_ferme_courant"), Sense::click());
                painter.text(
                    pos2(r.left() + 2.0, rect.top() + header_h * 0.5),
                    Align2::LEFT_CENTER,
                    format!("{} Calque courant dans « {name} »", ph::FOLDER),
                    FontId::proportional(11.0),
                    theme::marque(),
                );
                if bulle(resp, "Le calque courant est dans un dossier fermé", "Clic : ouvrir le dossier.", "").clicked() {
                    self.layers_mut()[fi].collapsed = false;
                    self.modified = true;
                }
            }
            (Some(g), _) => {
                let group = self.doc.groups[g].clone();
                let swatch = Rect::from_min_size(pos2(rect.left() + 4.0, rect.top() + 3.0), vec2(14.0, header_h - 6.0));
                let resp = ui.interact(swatch, egui::Id::new("groupe_cache_courant"), Sense::click());
                painter.rect_filled(swatch, 2.0, group_color(group.color));
                painter.text(
                    pos2(swatch.right() + 6.0, rect.top() + header_h * 0.5),
                    Align2::LEFT_CENTER,
                    "Calque courant caché",
                    FontId::proportional(11.0),
                    theme::marque(),
                );
                let tip = format!("Il est dans le groupe « {} », caché de la timeline. Clic sur la couleur pour montrer ce groupe.", group.name);
                if bulle(resp, "Le calque est caché", &tip, "").clicked() {
                    self.set_group_shown(g, true);
                }
            }
            (None, None) => {
                painter.text(
                    pos2(rect.left() + 6.0, rect.top() + header_h * 0.5),
                    Align2::LEFT_CENTER,
                    "Calques",
                    FontId::proportional(11.0),
                    ui.visuals().weak_text_color(),
                );
            }
        }
        // Nouveau dossier (13b-5).
        let r = Rect::from_min_size(pos2(rect.right() - 88.0, rect.top()), vec2(20.0, header_h));
        let resp = icon_at(ui, r, ph::FOLDER_PLUS, theme::texte_doux());
        if self.aide(resp, "Nouveau dossier", "Un dossier de calques vide ; glisser des calques dedans, ou menu ☰ > Dossiers.", "dossier.nouveau").clicked() {
            self.new_folder();
        }
        // Groupes de couleur (13b-4).
        let r = Rect::from_min_size(pos2(rect.right() - 66.0, rect.top()), vec2(20.0, header_h));
        let resp = icon_at(ui, r, ph::SWATCHES, theme::texte_doux());
        if self.aide(resp, "Groupes de calques", "Noms, couleurs, et quels groupes la timeline montre.", "groupes.editer").clicked() {
            self.groups_window = !self.groups_window;
        }
        // Menu des opérations sur les calques.
        let r = Rect::from_min_size(pos2(rect.right() - 44.0, rect.top()), vec2(20.0, header_h));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt("menu_calques_timeline"));
        let button = egui::Button::new(RichText::new(ph::LIST).size(13.0).color(theme::texte_doux())).frame(false);
        let resp = egui::menu::menu_custom_button(&mut child, button, |ui| self.layer_ops_menu(ui)).response;
        bulle(
            resp,
            "Opérations sur les calques",
            "Nouveau, dupliquer, supprimer, fusionner, choisir, afficher, pochoirs… Ctrl + clic sur une ligne pour choisir plusieurs calques, Maj + clic pour une suite.",
            "",
        );
        // Tout réduire / tout agrandir.
        let all_small = self.layers().iter().all(|l| l.collapsed);
        let r = Rect::from_min_size(pos2(rect.right() - 22.0, rect.top()), vec2(20.0, header_h));
        let icon = if all_small { ph::ARROWS_OUT_LINE_VERTICAL } else { ph::ARROWS_IN_LINE_VERTICAL };
        let resp = icon_at(ui, r, icon, theme::texte_doux());
        let what = if all_small { "Agrandir toutes les lignes" } else { "Réduire toutes les lignes" };
        if bulle(resp, what, "Lignes complètes (réglages, opacité, mode) ou réduites à leur nom.", "").clicked() {
            for l in self.layers_mut() {
                l.collapsed = !all_small;
            }
            self.modified = true;
        }

        let top = rect.top() + header_h;
        // Le menu du clic droit d'une ligne se montre pendant qu'on peint les lignes : « Supprimer » un dossier emmène ses calques,
        // et les lignes d'après lisaient des rangs qui n'existaient plus (Anthony, 2026-10-22, passe de débogage). Les lignes s'arrêtent
        // dès que les calques ont changé ; l'image suivante les refait.
        let avant = self.empreinte_calques();
        let mut tient = true;
        for row in &rows {
            if !self.calques_tiennent(&avant, row.li) {
                tient = false;
                break;
            }
            let r = Rect::from_min_size(pos2(rect.left(), top + row.top), vec2(HEAD_W, row.h));
            self.layer_head(ui, r, row.li);
        }
        tient &= self.calques_tiennent(&avant, 0);
        if !tient {
            ui.ctx().request_repaint();
        }

        // Glisser une ligne : un trait montre où le calque va se poser (entre deux lignes), ou le cadre du dossier où il va entrer
        // (sur la ligne d'un dossier) ; les calques choisis partent ensemble (`poser_les_calques`).
        if let Some(from) = self.head_drag.filter(|_| tient) {
            let pointer = ui.input(|i| i.pointer.interact_pos());
            let cible = pointer.and_then(|p| self.cible_des_calques_glisses(&rows, from, p.y - top));
            if let Some((c, dans)) = cible {
                peindre_cible(&painter, egui::Rangef::new(rect.left(), rect.right()), top, c, dans, INDENT);
            }
            if ui.input(|i| !i.pointer.any_down()) {
                self.head_drag = None;
                if let Some((c, _)) = cible {
                    self.poser_les_calques(from, c.pose());
                }
            }
        }
        rect
    }

    /// La ligne du calque `li` dans `rect`.
    fn layer_head(&mut self, ui: &mut Ui, rect: Rect, li: usize) {
        let h = {
            let l = &self.layers()[li];
            Head {
                id: l.id,
                name: l.name.clone(),
                kind: l.kind,
                cfx: l.effet.is_some(),
                annotation: l.annotation,
                unie: l.unie.is_some(),
                couleur: l.couleur.is_some(),
                sous_trait: l.keys_follow_line(),
                visible: l.visible,
                locked: l.locked,
                onion: l.onion,
                keep_alpha: l.keep_alpha,
                stencil: l.stencil,
                negative: l.stencil_negative,
                fixed: l.position_locked,
                accroche: l.accroche,
                group: (l.group as usize).min(crate::groups::COUNT - 1),
                collapsed: l.collapsed,
                opacity: l.opacity_at(self.cur_frame),
                blend: l.blend,
                cel: l.cel_at(self.cur_frame),
                folder: l.folder,
                children: crate::folders::children_of(self.layers(), li).len(),
                in_folder: l.parent.is_some(),
            }
        };
        let current = li == self.cur_layer;
        let visuals = ui.visuals().clone();
        // Calque rangé dans un dossier : la ligne est décalée, un trait la relie à son dossier.
        let rect = if h.in_folder || h.sous_trait {
            let gap = Rect::from_min_size(rect.min, vec2(INDENT, rect.height()));
            ui.painter().rect_filled(gap.shrink2(vec2(5.0, 0.0)), 0.0, theme::texte_doux().gamma_multiply(0.6));
            Rect::from_min_max(pos2(rect.left() + INDENT, rect.top()), rect.max)
        } else {
            rect
        };
        let r = rect.shrink2(vec2(0.0, 1.0));
        let painter = ui.painter_at(r);
        // Calque courant : l'accent ; calques choisis en plus (Ctrl + clic) : l'accent atténué.
        let bg = match (current, self.is_chosen_layer(li)) {
            (true, _) => theme::surface_active(crate::theme::accent(), 1.0),
            (false, true) => theme::surface_active(crate::theme::accent(), 0.45),
            (false, false) => theme::etiquette(),
        };
        painter.rect_filled(r, 2.0, bg);
        // (Thèmes de la marque : le calque courant est un panneau teinté avec un bord jade, pas un grand aplat jade.)
        if current && theme::palette().identite {
            painter.rect_stroke(r, 2.0, Stroke::new(1.5_f32, crate::theme::accent()), egui::StrokeKind::Inside);
        }
        // Plusieurs calques choisis : un cadre à chacun, plus épais au courant (qui garde le plein).
        if !self.layer_sel.is_empty() && self.is_chosen_layer(li) {
            let width = if current { 2.0_f32 } else { 1.25_f32 };
            painter.rect_stroke(r, 2.0, Stroke::new(width, crate::theme::accent()), egui::StrokeKind::Inside);
        }
        // Sur le bleu du calque courant, tout passe en blanc (le seul lisible sur l'accent).
        let fg = match (current, h.visible) {
            (true, _) if theme::palette().identite => theme::texte_fort(),
            (true, _) => crate::theme::sur_accent(),
            (false, true) => visuals.text_color(),
            (false, false) => visuals.weak_text_color(),
        };
        let off = fg.gamma_multiply(0.35);
        let shade = |on: bool| if on { fg } else { off };
        let group = self.doc.groups[h.group].clone();
        // Le filet de gauche : la couleur du rangement du calque — celle de son dossier, sinon celle de son groupe (la même règle que
        // dans le Storyboard et le Montage, `folders::couleur_du_rangement`, 2026-11-11).
        let filet = crate::folders::couleur_du_rangement(self.layers(), li, &self.doc.groups).unwrap_or_else(|| group_color(group.color));
        painter.rect_filled(Rect::from_min_size(r.min, vec2(4.0, r.height())), 0.0, filet);

        // Le fond de la ligne : choisir, glisser, menu, double-clic sur le nom. Posé avant les
        // boutons, qui restent au-dessus.
        let resp = ui.interact(r, egui::Id::new(("tete_calque", h.id)), Sense::click_and_drag());
        let x0 = match (h.folder, h.collapsed) {
            (true, _) => r.left() + 22.0,
            (false, true) => r.left() + 40.0,
            (false, false) => r.left() + 74.0,
        };
        let name_rect = Rect::from_min_max(pos2(x0 + 14.0, r.top() + 2.0), pos2(r.right() - 22.0, r.top() + 20.0_f32.min(r.height())));
        if resp.clicked() {
            let m = ui.input(|i| i.modifiers);
            self.click_layer_head(li, m.command, m.shift);
        }
        if resp.double_clicked() && resp.interact_pointer_pos().is_some_and(|p| name_rect.contains(p)) {
            self.layer_rename = Some(crate::layersui::LayerRename::new(h.id, &h.name, true));
        }
        if resp.drag_started() {
            self.select_layer(li);
            self.head_drag = Some(li);
        }
        if self.head_drag.is_some() && resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        resp.context_menu(|ui| self.layer_head_menu(ui, li));

        // Triangle : réduire / agrandir la ligne ; pour un dossier, montrer ou cacher ses calques.
        let caret = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 2.0), vec2(16.0, 18.0));
        let resp = icon_at(ui, caret, if h.collapsed { ph::CARET_RIGHT } else { ph::CARET_DOWN }, off.gamma_multiply(2.0));
        let (what, why) = match (h.folder, h.collapsed) {
            (true, true) => ("Ouvrir le dossier", "Montrer les lignes de ses calques."),
            (true, false) => ("Fermer le dossier", "Cacher les lignes de ses calques (ils restent à l'image)."),
            (false, true) => ("Agrandir la ligne", "Montrer les réglages du calque."),
            (false, false) => ("Réduire la ligne", "Cacher les réglages du calque."),
        };
        if bulle(resp, what, why, "").clicked() {
            self.layers_mut()[li].collapsed = !h.collapsed;
            self.modified = true;
        }

        // Nom (ou son champ quand on le renomme), sorte du calque, menu d'options.
        let line1 = r.top() + 11.0;
        let badge = if current { fg } else if h.cfx { theme::palette().dessin_cfx } else if h.annotation { theme::palette().dessin_annotation } else if h.unie { theme::palette().dessin_unie } else { theme::couleur_calque(h.kind, h.couleur) };
        if h.collapsed && !h.folder {
            let eye = Rect::from_min_size(pos2(r.left() + 20.0, r.top() + 2.0), vec2(ICON_W, 18.0));
            self.eye_button(ui, eye, li, h.visible, fg);
        }
        if h.folder {
            let icon = if h.collapsed { ph::FOLDER } else { ph::FOLDER_OPEN };
            painter.text(pos2(x0 - 2.0, line1), Align2::LEFT_CENTER, icon, FontId::proportional(14.0), fg);
        } else if h.cfx {
            painter.text(pos2(x0 - 1.0, line1), Align2::LEFT_CENTER, ph::SPARKLE, FontId::proportional(13.0), badge);
        } else if h.annotation {
            painter.text(pos2(x0 - 1.0, line1), Align2::LEFT_CENTER, ph::MARKER_CIRCLE, FontId::proportional(13.0), badge);
        } else if h.unie {
            painter.text(pos2(x0 - 1.0, line1), Align2::LEFT_CENTER, ph::SQUARE, FontId::proportional(13.0), badge);
        } else {
            painter.text(pos2(x0, line1), Align2::LEFT_CENTER, if h.couleur { "C" } else { h.kind.badge() }, FontId::monospace(11.0), badge);
        }
        let renaming = self.layer_rename.as_ref().is_some_and(|n| n.id == h.id && n.in_timeline);
        if renaming {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(name_rect).id_salt(("nom_calque", h.id)));
            let done = self.layer_rename_field(&mut child, name_rect.width());
            self.finish_layer_rename(done);
        } else {
            painter.with_clip_rect(name_rect).text(
                pos2(name_rect.left(), line1),
                Align2::LEFT_CENTER,
                &h.name,
                FontId::proportional(13.0),
                fg,
            );
        }
        if h.collapsed && !h.folder {
            let lock = Rect::from_min_size(pos2(r.right() - 22.0, r.top() + 2.0), vec2(20.0, 18.0));
            self.lock_button(ui, lock, li, h.locked, shade(h.locked));
            let fx = Rect::from_min_size(pos2(r.right() - 42.0, r.top() + 2.0), vec2(20.0, 18.0));
            if let Some(resp) = self.fx_switch(ui, li, Some(fx), fg) {
                if resp.clicked() {
                    self.toggle_layer_fx(li);
                }
            }
            return;
        }
        let opts = Rect::from_min_size(pos2(r.right() - 20.0, r.top() + 2.0), vec2(18.0, 18.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(opts).id_salt(("options_calque", h.id)));
        let button = egui::Button::new(RichText::new(ph::DOTS_THREE_VERTICAL).size(13.0).color(fg)).frame(false);
        let resp = egui::menu::menu_custom_button(&mut child, button, |ui| self.layer_head_menu(ui, li)).response;
        bulle(resp, "Commandes du calque", "Renommer, profondeur, dupliquer, verrouiller, supprimer, groupe… (aussi : clic droit sur la ligne).", "");

        // Dossier (13b-5) : afficher, verrouiller, opacité (ils agissent sur ses calques).
        if h.folder {
            let y2 = r.top() + 21.0;
            self.eye_button(ui, Rect::from_min_size(pos2(r.left() + 20.0, y2), vec2(ICON_W, 17.0)), li, h.visible, fg);
            self.lock_button(ui, Rect::from_min_size(pos2(r.left() + 20.0 + ICON_W, y2), vec2(ICON_W, 17.0)), li, h.locked, shade(h.locked));
            let bar = Rect::from_min_size(pos2(r.left() + 64.0, y2 + 2.0), vec2(108.0, 13.0));
            self.opacity_bar(ui, &painter, bar, r, li, egui::Id::new(("opacite_calque", h.id)), h.opacity, current, "Opacité du dossier", "Elle s'ajoute à celle de chacun de ses calques.");
            let n = h.children;
            let count = format!("{n} calque{}", if n > 1 { "s" } else { "" });
            painter.text(pos2(bar.right() + 8.0, bar.center().y), Align2::LEFT_CENTER, count, FontId::proportional(11.0), off.gamma_multiply(2.0));
            return;
        }

        // Vignette du dessin à l'image courante (vide : cadre gris).
        let fr = self.doc.frame_rect();
        let tw = 48.0;
        let th = (tw * fr.height() / fr.width().max(1.0)).clamp(12.0, r.height() - 22.0);
        let thumb = Rect::from_min_size(pos2(r.left() + 20.0, r.top() + 20.0 + (r.height() - 20.0 - th) * 0.5), vec2(tw, th));
        match h.cel.and_then(|c| self.layer_thumb(ui.ctx(), c)) {
            Some(t) => {
                let tint = if h.visible { Color32::WHITE } else { Color32::from_gray(120) };
                painter.image(t.id(), thumb, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
            }
            None => {
                painter.rect_filled(thumb, 0.0, theme::controle());
            }
        }
        painter.rect_stroke(thumb, 0.0, Stroke::new(1.0_f32, theme::bord()), StrokeKind::Inside);

        // Réglages en icônes.
        let y2 = r.top() + 20.0;
        let slot = |k: usize| Rect::from_min_size(pos2(x0 + k as f32 * ICON_W, y2), vec2(ICON_W, 17.0));
        self.eye_button(ui, slot(0), li, h.visible, fg);
        self.lock_button(ui, slot(1), li, h.locked, shade(h.locked));
        let resp = icon_at(ui, slot(2), ph::CIRCLES_THREE, shade(h.onion));
        let resp = bulle(
            resp,
            "Pelure d'oignon du calque",
            "Quand la pelure d'oignon est allumée, montrer aussi les dessins voisins de ce calque, même s'il n'est pas le calque courant. Clic droit : le panneau de la pelure.",
            "",
        );
        if resp.clicked() {
            self.basculer_calques(li, "Pelure d'oignon", |l| l.onion, |l, v| l.onion = v, false);
        }
        if resp.secondary_clicked() {
            self.onion_window = true;
        }
        let resp = icon_at(ui, slot(3), ph::CHECKERBOARD, shade(h.keep_alpha));
        let resp = bulle(
            resp,
            "Préserver la transparence",
            "Le pinceau ne colore que ce qui est déjà dessiné : le vide reste vide. La gomme gomme normalement.",
            "",
        );
        if resp.clicked() {
            self.set_layer_flag(li, |l| l.keep_alpha = !l.keep_alpha);
        }
        let icon = if h.negative { ph::EXCLUDE } else { ph::INTERSECT };
        let resp = icon_at(ui, slot(4), icon, shade(h.stencil));
        let resp = bulle(
            resp,
            "Pochoir",
            "Le dessin de ce calque limite où l'on peint sur les autres calques (pinceau, crayon, gomme bitmap).",
            "",
        );
        if resp.clicked() {
            self.basculer_calques(li, "Pochoir", |l| l.stencil, |l, v| l.stencil = v, false);
        }
        let resp = icon_at(ui, slot(5), if h.negative { ph::MINUS } else { ph::PLUS }, shade(h.stencil));
        let resp = bulle(
            resp,
            if h.negative { "Pochoir négatif" } else { "Pochoir positif" },
            "Positif : on ne peint que dans la forme du pochoir. Négatif : seulement en dehors. Clic : changer.",
            "",
        );
        if resp.clicked() {
            self.basculer_calques(li, "Pochoir négatif", |l| l.stencil_negative, |l, v| l.stencil_negative = v, false);
        }
        let resp = icon_at(ui, slot(6), ph::PUSH_PIN, shade(h.fixed));
        let resp = bulle(
            resp,
            "Verrouiller la position",
            "L'outil Transformer et le déplacement en 3D ne bougent plus le calque, et on ne peut plus dessiner dessus.",
            "",
        );
        if resp.clicked() {
            self.set_layer_flag(li, |l| l.position_locked = !l.position_locked);
        }
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(slot(7)).id_salt(("groupe_calque", h.id)));
        let button = egui::Button::new(RichText::new(ph::FOLDER_SIMPLE).size(13.0).color(group_color(group.color))).frame(false);
        let resp = egui::menu::menu_custom_button(&mut child, button, |ui| self.group_menu(ui, li)).response;
        bulle(resp, &format!("Groupe : {}", group.name), "Ranger le calque dans un groupe de couleur.", "");
        // Les effets du calque, d'un coup (comme l'œil et le cadenas ; seulement s'il en a : `fxui.rs`).
        if let Some(resp) = self.fx_switch(ui, li, Some(slot(8)), fg) {
            if resp.clicked() {
                self.toggle_layer_fx(li);
            }
        }
        // Accroché à la caméra (`accroche.rs`) : le calque garde sa place dans l'image, quoi que fasse la caméra.
        if !h.folder && !h.cfx && !h.sous_trait {
            let resp = icon_at(ui, slot(9), ph::VIDEO_CAMERA, shade(h.accroche));
            if self.aide(resp, crate::accroche::NOM, crate::accroche::PHRASE, crate::accroche::ACTION).clicked() {
                self.basculer_accroche(li);
            }
        }

        // Opacité : une barre qu'on tire.
        let y3 = r.top() + 38.0;
        let bar = Rect::from_min_size(pos2(x0, y3 + 2.0), vec2(108.0, 13.0));
        self.opacity_bar(ui, &painter, bar, r, li, egui::Id::new(("opacite_calque", h.id)), h.opacity, current, "Opacité du calque", "Cliquer ou tirer dans la barre.");

        // Mode de fusion, avec précédent / suivant.
        let modes = BlendMode::ALL;
        let at = modes.iter().position(|m| *m == h.blend).unwrap_or(0);
        let mut mode = h.blend;
        let prev = Rect::from_min_size(pos2(bar.right() + 4.0, y3), vec2(14.0, 17.0));
        if bulle(icon_at(ui, prev, ph::CARET_LEFT, fg), "Mode précédent", "", "").clicked() {
            mode = modes[(at + modes.len() - 1) % modes.len()];
        }
        let combo = Rect::from_min_max(pos2(prev.right(), y3), pos2(r.right() - 18.0, y3 + 17.0));
        mode = blend_combo(ui, combo, egui::Id::new(("fusion_tete", h.id)), h.blend).unwrap_or(mode);
        let next = Rect::from_min_size(pos2(combo.right(), y3), vec2(14.0, 17.0));
        if bulle(icon_at(ui, next, ph::CARET_RIGHT, fg), "Mode suivant", "", "").clicked() {
            mode = modes[(at + 1) % modes.len()];
        }
        if mode != h.blend {
            // (Plusieurs calques choisis : tous prennent ce mode, partie 136.)
            self.regler_mode_calques(li, mode);
        }
    }

    /// Barre d'opacité du calque `li` (identifiée par `key`) qu'on clique ou qu'on tire ; une
    /// étape d'annulation par geste. `clip` : la ligne (le texte n'en sort pas).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn opacity_bar(
        &mut self,
        ui: &mut Ui,
        painter: &egui::Painter,
        bar: Rect,
        clip: Rect,
        li: usize,
        key: egui::Id,
        opacity: f32,
        current: bool,
        name: &str,
        description: &str,
    ) {
        let mut resp = ui.interact(bar, key, Sense::click_and_drag());
        painter.rect_filled(bar, 2.0, theme::champ());
        let fill = Rect::from_min_size(bar.min, vec2(bar.width() * opacity.clamp(0.0, 1.0), bar.height()));
        painter.rect_filled(fill, 2.0, if current { theme::texte_fort() } else { theme::texte_doux() });
        // Le texte a la couleur du fond de la barre sur la partie pleine, celle du texte sur le reste.
        let pct = format!("{:.0} %", opacity * 100.0);
        for (part, ink) in [(fill, theme::champ()), (Rect::from_min_max(pos2(fill.right(), bar.top()), bar.max), theme::texte())] {
            painter.with_clip_rect(part.intersect(clip)).text(bar.center(), Align2::CENTER_CENTER, &pct, FontId::proportional(10.0), ink);
        }
        let mut value = opacity;
        if resp.is_pointer_button_down_on() || resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                value = (((p.x - bar.left()) / bar.width()).clamp(0.0, 1.0) * 100.0).round() / 100.0;
                if value != opacity {
                    resp.mark_changed();
                }
            }
        }
        let resp = bulle(resp, name, description, "");
        if self.edited(&resp) {
            let f = self.cur_frame;
            self.layers_mut()[li].set_opacity_at(f, value);
            // (Un réglage : aucune tuile n'est renvoyée à la carte graphique pendant le geste.)
            self.renderer.invalidate_params();
        }
    }

    pub(crate) fn eye_button(&mut self, ui: &mut Ui, rect: Rect, li: usize, visible: bool, color: Color32) {
        let resp = icon_at(ui, rect, if visible { ph::EYE } else { ph::EYE_SLASH }, color);
        if bulle(resp, "Afficher / cacher", "Montrer ou cacher le calque (et sa couleur, si c'est un trait).", "").clicked() {
            self.toggle_layer_visible(li);
        }
    }

    pub(crate) fn lock_button(&mut self, ui: &mut Ui, rect: Rect, li: usize, locked: bool, color: Color32) {
        let resp = icon_at(ui, rect, if locked { ph::LOCK_SIMPLE } else { ph::LOCK_SIMPLE_OPEN }, color);
        if self.aide(resp, "Verrouiller / déverrouiller", "Un calque verrouillé ne peut plus être dessiné.", "calque.verrouiller").clicked() {
            self.toggle_layer_lock(li);
        }
    }

    /// Change un réglage du calque `li` (une étape d'annulation).
    pub(crate) fn set_layer_flag(&mut self, li: usize, change: impl FnOnce(&mut Layer)) {
        if li >= self.layers().len() {
            return;
        }
        self.end_stroke();
        self.record_step();
        change(&mut self.layers_mut()[li]);
        self.renderer.invalidate_all();
    }

    /// Menu des groupes : ranger le calque `li` dans l'un d'eux.
    pub(crate) fn group_menu(&mut self, ui: &mut Ui, li: usize) {
        let now = self.layers().get(li).map_or(0, |l| l.group as usize);
        for (g, group) in self.doc.groups.clone().iter().enumerate() {
            let text = RichText::new(format!("{}  {}", ph::SQUARE, group.name)).color(group_color(group.color));
            let resp = ui.add(egui::Button::new(text).selected(g == now).frame(false));
            if resp.clicked() {
                ui.close_menu();
                if g != now {
                    self.set_layer_flag(li, |l| l.group = g as u8);
                }
            }
        }
        ui.separator();
        if ui.button("Éditer les groupes…").clicked() {
            ui.close_menu();
            self.groups_window = true;
        }
    }

    /// Pré / post-comportement du calque `li` (13c), et cassure et création automatiques : menus
    /// des en-têtes de la timeline et du montage. Rien pour un dossier.
    pub(crate) fn behaviour_items(&mut self, ui: &mut Ui, li: usize) {
        if let Some((pre, post)) = self.layers().get(li).filter(|l| !l.folder).map(|l| (l.pre, l.post)) {
            ui.menu_button(format!("Avant la première image : {}", pre.label()), |ui| {
                for b in Behaviour::ALL {
                    if ui.radio(b == pre, b.label()).clicked() {
                        ui.close_menu();
                        self.set_layer_flag(li, |l| l.pre = b);
                    }
                }
            });
            ui.menu_button(format!("Après la dernière image : {}", post.label()), |ui| {
                for b in Behaviour::ALL {
                    if ui.radio(b == post, b.label()).clicked() {
                        ui.close_menu();
                        self.set_layer_flag(li, |l| l.post = b);
                    }
                }
            });
            // Cassure et création automatiques (13c).
            let (brk, create) = (self.layers()[li].auto_break, self.layers()[li].auto_create);
            if ui
                .checkbox(&mut brk.clone(), "Cassure automatique de l'instance")
                .on_hover_text("Dessiner sur une image tenue en fait un nouveau dessin (copie du tenu) jusqu'à la fin de l'exposition.")
                .clicked()
            {
                self.set_layer_flag(li, |l| l.auto_break = !brk);
            }
            if ui
                .checkbox(&mut create.clone(), "Création automatique d'image")
                .on_hover_text("Dessiner sur une image vide crée un dessin ; éteinte, il faut d'abord un « Nouveau dessin ».")
                .clicked()
            {
                self.set_layer_flag(li, |l| l.auto_create = !create);
            }
        }
    }

    /// Les calques qui partent avec le calque `li` quand on le glisse dans une liste de calques : lui seul ; ou, s'il est un des
    /// calques choisis, tous les calques choisis (la sélection agit sur tout ce qui est choisi) — sauf les dossiers et les calques
    /// de couleur, qui suivent leur calque de trait. Un dossier se glisse seul, avec ses calques.
    pub(crate) fn calques_glisses(&self, li: usize) -> Vec<u64> {
        let layers = self.layers();
        let Some(l) = layers.get(li) else { return Vec::new() };
        if l.folder || !self.is_chosen_layer(li) {
            return vec![l.id];
        }
        let choisis: Vec<u64> = self.chosen_layers().into_iter().filter(|&k| !layers[k].folder && !layers[k].keys_follow_line()).map(|k| layers[k].id).collect();
        if choisis.len() >= 2 && choisis.contains(&l.id) {
            choisis
        } else {
            vec![l.id]
        }
    }

    /// La cible du glissé parti du calque `from`, le pointeur à `y` (depuis le haut des lignes `rows`), et si le trait d'insertion
    /// tombe dans un dossier (il se peint en retrait). Rien : sur la ligne d'un des calques glissés, ou sans ligne.
    pub(crate) fn cible_des_calques_glisses(&self, rows: &[Row], from: usize, y: f32) -> Option<(Cible, bool)> {
        let layers = self.layers();
        let l = layers.get(from)?;
        let ids = self.calques_glisses(from);
        let parts: Vec<usize> = layers.iter().enumerate().filter(|(_, x)| ids.contains(&x.id)).map(|(i, _)| i).collect();
        let cible = cible_du_glisse(rows, layers, &parts, y, !l.folder)?;
        let dans = matches!(cible, Cible::Trait(..)) && crate::folders::dossier_de_pose(layers, &ids, cible.pose()).is_some();
        Some((cible, dans))
    }

    /// Pose les calques glissés depuis le calque `from` (`calques_glisses`) à `pose` : entre deux lignes, ou dans un dossier quand
    /// ils sont lâchés sur sa ligne (`folders::poser`). Posés entre les calques d'un dossier (ou juste sous lui), ils y entrent ;
    /// posés ailleurs, ils en sortent ; un dossier emmène ses calques (13b-5). Une étape d'annulation ; rien si la pile n'en est pas
    /// changée. Le calque courant reste le même calque, les calques choisis le restent. La barre d'état dit ce qui est entré dans un
    /// dossier ou en est sorti.
    pub(crate) fn poser_les_calques(&mut self, from: usize, pose: crate::folders::Pose) {
        let Some((id, suit_son_trait, avant)) = self.layers().get(from).map(|l| (l.id, l.keys_follow_line(), l.parent)) else { return };
        if suit_son_trait {
            self.status = "Un calque de couleur reste sous son calque de trait : déplace le trait, la couleur le suit.".into();
            return;
        }
        self.end_stroke();
        let ids = self.calques_glisses(from);
        let Some(pile) = crate::folders::poser(self.layers(), &ids, pose) else { return };
        let current = self.layers()[self.cur_layer].id;
        let snap = self.snapshot();
        *self.layers_mut() = pile;
        self.cur_layer = self.layers().iter().position(|l| l.id == current).unwrap_or(0);
        self.vsel = None;
        self.commit_structure(snap);
        let n = ids.len();
        let s = if n > 1 { "s" } else { "" };
        let apres = self.layers().iter().find(|l| l.id == id).and_then(|l| l.parent);
        let nom = |d: Option<u64>| d.and_then(|d| self.layers().iter().find(|l| l.id == d)).map(|l| l.name.clone());
        let (dedans, dehors) = (nom(apres), nom(avant));
        let texte = match (dedans, dehors) {
            (Some(d), _) if apres != avant => format!("{n} calque{s} rangé{s} dans « {d} »."),
            (None, Some(d)) => format!("{n} calque{s} sorti{s} de « {d} »."),
            _ => format!("{n} calque{s} déplacé{s}."),
        };
        self.status = texte;
    }
}
