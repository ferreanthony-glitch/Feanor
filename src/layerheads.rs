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
/// Hauteur des lignes de la caméra, des notes et du son sous les calques (une ligne de texte ; c'était aussi celle d'une ligne
/// de calque réduite à la main, avant la partie 160).
pub(crate) const ROW_SMALL: f32 = 24.0;
/// Hauteur de toutes les lignes de la timeline, calques et dossiers (partie 168 : avant, le calque courant avait une ligne complète de
/// 56 points et un dossier courant une ligne de 40 ; ses réglages sont maintenant dans la barre du calque, `barre_du_calque`).
/// Partie 168.4 (Anthony : « la taille d'une ligne de calque est plus épaisse ») : 28 points au lieu de 22.
pub(crate) const ROW_FINE: f32 = 28.0;
/// Hauteur de la barre du calque courant, au-dessus des lignes (partie 168).
pub(crate) const BARRE_H: f32 = 24.0;
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
/// (filtre de groupe, 13b-4) ou d'un dossier fermé (13b-5) n'en ont pas, sauf le calque courant (partie 165.4 : il garde toujours sa
/// ligne). Toutes les lignes ont la même hauteur (partie 168) : changer de calque courant ne fait plus bouger la timeline.
pub(crate) fn rows_of(layers: &[Layer], groups: &[LayerGroup], cur: usize) -> Vec<Row> {
    let closed: Vec<u64> = layers.iter().filter(|l| l.folder && l.collapsed).map(|l| l.id).collect();
    let mut top = 0.0;
    layers
        .iter()
        .enumerate()
        .rev()
        // (Partie 165.4 : le calque courant garde toujours sa ligne, même dans un groupe caché ou un dossier fermé.)
        .filter(|(li, l)| *li == cur || groups.get(l.group as usize).is_none_or(|g| g.shown))
        .filter(|(li, l)| *li == cur || l.parent.is_none_or(|p| !closed.contains(&p)))
        .map(|(li, _)| {
            let row = Row { li, top, h: ROW_FINE };
            top += ROW_FINE;
            row
        })
        .collect()
}

/// Partie 165.5 : largeur de la colonne des calques de la timeline : celle d'une ligne complète (HEAD_W), ou plus, celle de la ligne
/// des boutons de la timeline mesurée à son dessin (une estimation avant).
pub(crate) fn largeur_colonne_timeline() -> f32 {
    let mesure = crate::barres::largeur_mesuree(crate::barres::Barre::Timeline);
    HEAD_W.max(if mesure > 0.0 { mesure } else { LARGEUR_BARRE_TIMELINE_AVANT_MESURE })
}

/// Partie 165.5 : la largeur de la ligne des boutons de la timeline avant sa première mesure (partie 165.6 : la plus longue des deux
/// lignes).
const LARGEUR_BARRE_TIMELINE_AVANT_MESURE: f32 = 400.0;

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

/// Le fond d'une ligne de calque (en-tête et grille), teinté de la couleur du rangement du calque (partie 168) : celle de son dossier,
/// sinon celle de son groupe ; rien, le fond d'origine. Un dossier est teinté plus fort : sa ligne sert de titre.
pub(crate) fn fond_teinte(base: Color32, rangement: Option<Color32>, dossier: bool) -> Color32 {
    let Some(c) = rangement else { return base };
    let t = if dossier { 0.24 } else { 0.11 };
    let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgb(m(base.r(), c.r()), m(base.g(), c.g()), m(base.b(), c.b()))
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
        let rows = rows_of(self.layers(), &self.doc.groups, self.cur_layer);
        // Partie 165.5 : la colonne est au moins aussi large que la ligne des boutons de la timeline (Anthony : « toujours garder
        // cette colonne avec un maximum d'information »), qui est en haut de la colonne, en face de la règle des images.
        // (Jamais plus large que la place : dans la fenêtre la plus petite, la bande en a toujours assez ; un écran d'essai plus étroit
        // garde la ligne complète du calque, `HEAD_W`.)
        let w = largeur_colonne_timeline().min(ui.available_width().max(HEAD_W));
        let size = vec2(w, header_h + rows_height(&rows) + below);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect);
        let tete = Rect::from_min_size(rect.min, vec2(w, header_h));
        painter.rect_filled(tete, 0.0, theme::regle());
        // (Les messages « calque courant caché » et « dans un dossier fermé » qui étaient là ne servent plus : le calque courant garde sa
        // ligne dans tous les cas, partie 165.4 ; son groupe se remontre par son menu, son dossier par son triangle.)
        // Partie 165.6 (Anthony : « Deux lignes de boutons ») : la barre sur deux lignes, la première en face des numéros de la règle,
        // la seconde juste dessous ; la colonne ne prend plus que la largeur de la plus longue des deux.
        let h1 = crate::timeline::LIGNE1_H.min(header_h);
        let ligne1 = Rect::from_min_size(tete.min, vec2(w, h1));
        let ligne2 = Rect::from_min_max(pos2(tete.left(), tete.top() + h1), tete.max);
        self.barre_dans_la_colonne_deux_lignes(ui, ligne1, ligne2, crate::barres::Barre::Timeline, crate::timeline::DERNIER_GROUPE_LIGNE1);
        #[cfg(test)]
        crate::timeline::LIGNES_PEINTES.with(|l| l.borrow_mut().0 = rows.iter().map(|row| (row.li, rect.top() + header_h + row.top, rect.top() + header_h + row.top + row.h)).collect());
        // (Partie 160 : Nouveau dossier, Groupes de calques et les opérations sur les calques sont dans la barre d'actions, en bas
        // de la colonne, `barrecalques.rs` ; « Tout réduire / tout agrandir » n'existe plus : le calque courant a toujours sa ligne
        // complète, les autres une ligne fine.)
        // Un clic dans la colonne lui donne la main : après une copie d'effets, Ctrl+V les colle sur le calque cliqué (`fxcopie.rs`,
        // comme le faisait le panneau Calques).
        if ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(rect) {
            self.take_layers_pane_focus();
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
            let r = Rect::from_min_size(pos2(rect.left(), top + row.top), vec2(w, row.h));
            self.layer_head(ui, r, row.li);
        }
        // (Partie 165.4 faisait flotter la ligne complète du calque courant au bord de ce qu'on voit ; avec la barre du calque, fixe
        // au-dessus de la timeline, il n'en a plus besoin : partie 168.4.)
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

    /// Ce que la ligne du calque `li` montre.
    fn head_of(&self, li: usize) -> Head {
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
            folder: l.folder,
            children: crate::folders::children_of(self.layers(), li).len(),
            in_folder: l.parent.is_some(),
        }
    }

    /// Ce qui sort de l'ordinaire sur le calque `li` (partie 168) : ce que sa ligne montre à droite, en petit. Rien : un calque
    /// normal (visible, déverrouillé, opacité pleine, mode normal, sans pochoir…). L'œil et le cadenas ont leur propre case.
    pub(crate) fn exceptions_du_calque(&self, li: usize) -> Vec<(&'static str, String)> {
        let Some(l) = self.layers().get(li) else { return Vec::new() };
        let mut v = Vec::new();
        let opacite = l.opacity_at(self.cur_frame);
        if opacite < 0.995 {
            v.push(("", format!("{:.0} %", opacite * 100.0)));
        }
        if l.folder {
            return v;
        }
        if l.blend != BlendMode::Normal {
            v.push(("", l.blend.label().to_string()));
        }
        if l.stencil {
            v.push((if l.stencil_negative { ph::EXCLUDE } else { ph::INTERSECT }, if l.stencil_negative { "Pochoir négatif".into() } else { "Pochoir".into() }));
        }
        if l.keep_alpha {
            v.push((ph::CHECKERBOARD, "Préserve la transparence".into()));
        }
        if l.onion {
            v.push((ph::CIRCLES_THREE, "Pelure d'oignon du calque".into()));
        }
        if l.position_locked {
            v.push((ph::PUSH_PIN, "Position verrouillée".into()));
        }
        if l.accroche {
            v.push((ph::VIDEO_CAMERA, crate::accroche::NOM.into()));
        }
        v
    }

    /// La ligne du calque `li` dans `rect` (partie 168) : la même pour tous, courant ou non. Œil à gauche, nom, et à droite seulement
    /// ce qui sort de l'ordinaire ; les réglages du calque courant sont dans la barre du calque (`barre_du_calque`). Un dossier : son
    /// triangle, son nom en titre et le nombre de ses calques. Toute la ligne prend la couleur du rangement du calque (son dossier,
    /// sinon son groupe).
    fn layer_head(&mut self, ui: &mut Ui, rect: Rect, li: usize) {
        let h = self.head_of(li);
        // (La caméra du plan choisie seule dans la vue Espace 3D : c'est sa rangée que la timeline montre choisie, et plus
        // aucune rangée de calque, `espace3dui.rs`, partie 152.)
        let camera_seule = self.e3_camera_seule();
        let current = self.calque_montre_courant(li);
        let choisi = self.is_chosen_layer(li) && !camera_seule;
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
        let group = self.doc.groups[h.group].clone();
        // La couleur du rangement du calque — celle de son dossier, sinon celle de son groupe (la même règle que dans le Storyboard et
        // le Montage, `folders::couleur_du_rangement`, 2026-11-11) : le filet de gauche, et une teinte sur toute la ligne (partie 168).
        let rangement = crate::folders::couleur_du_rangement(self.layers(), li, &self.doc.groups);
        let filet = rangement.unwrap_or_else(|| group_color(group.color));
        // Calque courant : l'accent ; calques choisis en plus (Ctrl + clic) : l'accent atténué.
        let bg = match (current, choisi) {
            (true, _) => theme::surface_active(crate::theme::accent(), 1.0),
            (false, true) => theme::surface_active(crate::theme::accent(), 0.45),
            (false, false) => fond_teinte(if h.folder { theme::rangee_sombre() } else { theme::etiquette() }, rangement, h.folder),
        };
        painter.rect_filled(r, 2.0, bg);
        // (Thèmes de la marque : le calque courant est un panneau teinté avec un bord jade, pas un grand aplat jade.)
        if current && theme::palette().identite {
            painter.rect_stroke(r, 2.0, Stroke::new(1.5_f32, crate::theme::accent()), egui::StrokeKind::Inside);
        }
        // Plusieurs calques choisis : un cadre à chacun, plus épais au courant (qui garde le plein).
        if !self.layer_sel.is_empty() && choisi {
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
        painter.rect_filled(Rect::from_min_size(r.min, vec2(4.0, r.height())), 0.0, filet);

        // Le fond de la ligne : choisir, glisser, menu, double-clic sur le nom. Posé avant les
        // boutons, qui restent au-dessus.
        let resp = ui.interact(r, egui::Id::new(("tete_calque", h.id)), Sense::click_and_drag());
        // L'œil et le cadenas ne se montrent que s'ils sont éteints (caché, verrouillé), ou sous la souris pour qu'on les clique.
        let survol = ui.rect_contains_pointer(r);
        let x0 = if h.folder { r.left() + 22.0 } else { r.left() + 40.0 };
        let line1 = r.center().y;
        // À droite : le cadenas, l'œil d'un dossier, les effets, puis ce qui sort de l'ordinaire.
        let lock = Rect::from_min_size(pos2(r.right() - 22.0, r.top() + 2.0), vec2(20.0, 18.0));
        let mut droite = lock.left();
        if h.folder {
            droite -= 20.0;
        }
        let fx_rect = (!h.folder && self.fx_switch_shown(li)).then(|| {
            droite -= 20.0;
            Rect::from_min_size(pos2(droite, r.top() + 2.0), vec2(20.0, 18.0))
        });
        let exceptions = self.exceptions_du_calque(li);
        let mut x_exc = droite - 2.0;
        let mut places = Vec::new();
        for (icone, texte) in exceptions.iter().rev() {
            let w = if icone.is_empty() { painter.layout_no_wrap(texte.clone(), FontId::proportional(10.5), off).size().x + 6.0 } else { 17.0 };
            x_exc -= w;
            places.push((Rect::from_min_size(pos2(x_exc, r.top() + 2.0), vec2(w, 18.0)), *icone, texte.clone()));
        }
        let fin_du_nom = (x_exc - 4.0).max(x0 + 40.0);
        let name_rect = Rect::from_min_max(pos2(x0 + 14.0, r.top() + 2.0), pos2(fin_du_nom, r.bottom() - 2.0));
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

        // Triangle d'un dossier : montrer ou cacher ses calques.
        if h.folder {
            let caret = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 2.0), vec2(16.0, 18.0));
            let resp = icon_at(ui, caret, if h.collapsed { ph::CARET_RIGHT } else { ph::CARET_DOWN }, off.gamma_multiply(2.0));
            let (what, why) = if h.collapsed {
                ("Ouvrir le dossier", "Montrer les lignes de ses calques.")
            } else {
                ("Fermer le dossier", "Cacher les lignes de ses calques (ils restent à l'image, et leurs clés se voient en petit sur la ligne du dossier).")
            };
            if bulle(resp, what, why, "").clicked() {
                self.layers_mut()[li].collapsed = !h.collapsed;
                self.modified = true;
            }
        }
        // Œil (et cadenas) : seulement éteints, ou sous la souris.
        let case = |on: bool| if !on { fg } else if survol { off } else { Color32::TRANSPARENT };
        let eye = if h.folder { Rect::from_min_size(pos2(r.right() - 42.0, r.top() + 2.0), vec2(20.0, 18.0)) } else { Rect::from_min_size(pos2(r.left() + 20.0, r.top() + 2.0), vec2(ICON_W, 18.0)) };
        self.eye_button(ui, eye, li, h.visible, case(h.visible));
        self.lock_button(ui, lock, li, h.locked, case(!h.locked));
        if let Some(fx) = fx_rect {
            if let Some(resp) = self.fx_switch(ui, li, Some(fx), fg) {
                if resp.clicked() {
                    self.toggle_layer_fx(li);
                }
            }
        }
        // Ce qui sort de l'ordinaire : en petit, à droite du nom ; la bulle dit ce que c'est (on le change dans la barre du calque).
        for (at, icone, texte) in places {
            if icone.is_empty() {
                painter.text(at.center(), Align2::CENTER_CENTER, &texte, FontId::proportional(10.5), off.gamma_multiply(2.2));
            } else {
                painter.text(at.center(), Align2::CENTER_CENTER, icone, FontId::proportional(12.0), off.gamma_multiply(2.2));
            }
            ui.interact(at, egui::Id::new(("exception", h.id, texte.as_str())), Sense::hover()).on_hover_text(format!("{texte} (à changer dans la barre du calque, en haut)"));
        }

        // Sorte du calque, puis le nom (ou son champ quand on le renomme) ; un dossier : son nom en titre, et le nombre de ses calques.
        let badge = if current { fg } else if h.cfx { theme::palette().dessin_cfx } else if h.annotation { theme::palette().dessin_annotation } else if h.unie { theme::palette().dessin_unie } else { theme::couleur_calque(h.kind, h.couleur) };
        if h.folder {
            let icon = if h.collapsed { ph::FOLDER } else { ph::FOLDER_OPEN };
            painter.text(pos2(x0 - 2.0, line1), Align2::LEFT_CENTER, icon, FontId::proportional(14.0), if current { fg } else { filet });
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
        } else if h.folder {
            let titre = painter.with_clip_rect(name_rect).text(
                pos2(name_rect.left(), line1),
                Align2::LEFT_CENTER,
                &h.name,
                // (La famille des titres : demi-gras quand les polices Plex sont chargées, `theme::densifier_avec`.)
                FontId::new(12.5, ui.style().text_styles.get(&egui::TextStyle::Heading).map_or(egui::FontFamily::Proportional, |f| f.family.clone())),
                fg,
            );
            let n = h.children;
            painter.with_clip_rect(name_rect).text(pos2(titre.right() + 7.0, line1), Align2::LEFT_CENTER, n.to_string(), FontId::monospace(10.5), off.gamma_multiply(2.0));
        } else {
            painter.with_clip_rect(name_rect).text(pos2(name_rect.left(), line1), Align2::LEFT_CENTER, &h.name, FontId::proportional(13.0), fg);
        }
    }

    /// La barre du calque courant, au-dessus des lignes de la timeline (partie 168) : son nom, puis tous ses réglages sur une ligne —
    /// afficher, verrouiller, pelure d'oignon, préserver la transparence, pochoir et son sens, position verrouillée, groupe, effets,
    /// accroche à la caméra, aplat et planche-mère, opacité et ses clés, mode de fusion — et ses commandes (⋮). Ce que portait la ligne
    /// complète du calque courant (partie 160), qui n'existe plus : les lignes ont toutes la même hauteur. Pour un dossier : afficher,
    /// verrouiller, opacité et le nombre de ses calques.
    pub(crate) fn barre_du_calque(&mut self, ui: &mut Ui) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), BARRE_H), Sense::hover());
        let li = self.cur_layer;
        if li >= self.layers().len() || self.e3_camera_seule() {
            return;
        }
        let h = self.head_of(li);
        let painter = ui.painter_at(rect);
        let group = self.doc.groups[h.group].clone();
        let rangement = crate::folders::couleur_du_rangement(self.layers(), li, &self.doc.groups);
        painter.rect_filled(rect, 2.0, fond_teinte(theme::etiquette(), rangement, false));
        painter.rect_filled(Rect::from_min_size(rect.min, vec2(4.0, rect.height())), 0.0, rangement.unwrap_or_else(|| group_color(group.color)));
        let fg = ui.visuals().text_color();
        let off = fg.gamma_multiply(0.35);
        let shade = |on: bool| if on { fg } else { off };
        let y = rect.top() + (BARRE_H - 18.0) * 0.5;
        let mut x = rect.left() + 10.0;
        let place = |w: f32, x: &mut f32| {
            let r = Rect::from_min_size(pos2(*x, y), vec2(w, 18.0));
            *x += w;
            r
        };
        // Le nom, et ce que la ligne du panneau Calques disait sous lui : « sous <trait> », le nombre de couleurs d'un trait.
        let nom_rect = Rect::from_min_size(pos2(x, y), vec2(170.0, 18.0));
        let nom = painter.with_clip_rect(nom_rect).text(pos2(x, y + 9.0), Align2::LEFT_CENTER, &h.name, FontId::proportional(13.0), theme::texte_fort());
        x = nom.right().min(nom_rect.right()) + 8.0;
        if h.folder {
            let n = h.children;
            painter.text(pos2(x, y + 9.0), Align2::LEFT_CENTER, format!("{n} calque{}", if n > 1 { "s" } else { "" }), FontId::proportional(11.0), off.gamma_multiply(2.0));
            x += 62.0;
        } else if let Some(petit) = self.sous_le_nom(li) {
            let g = painter.text(pos2(x, y + 9.0), Align2::LEFT_CENTER, &petit.0, FontId::proportional(10.5), off.gamma_multiply(2.2));
            let resp = ui.interact(g.expand(2.0), egui::Id::new(("sous_le_nom", h.id)), Sense::hover());
            bulle(resp, &petit.0, petit.1, "");
            x = g.right() + 8.0;
        }
        // Les commandes du calque (aussi au clic droit de sa ligne), juste après le nom : elles restent visibles même dans une timeline
        // étroite, où la fin de la barre est coupée.
        x = x.max(rect.left() + 190.0);
        let opts = place(18.0, &mut x);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(opts).id_salt(("options_calque", h.id)));
        let button = egui::Button::new(RichText::new(ph::DOTS_THREE_VERTICAL).size(13.0).color(fg)).frame(false);
        let resp = egui::menu::menu_custom_button(&mut child, button, |ui| self.layer_head_menu(ui, li)).response;
        bulle(resp, "Commandes du calque", "Renommer, profondeur, dupliquer, verrouiller, supprimer, groupe… (aussi : clic droit sur la ligne).", "");
        x += 8.0;
        let r = place(ICON_W, &mut x);
        self.eye_button(ui, r, li, h.visible, fg);
        let r = place(ICON_W, &mut x);
        self.lock_button(ui, r, li, h.locked, shade(h.locked));
        if !h.folder {
            let resp = icon_at(ui, place(ICON_W, &mut x), ph::CIRCLES_THREE, shade(h.onion));
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
            let resp = icon_at(ui, place(ICON_W, &mut x), ph::CHECKERBOARD, shade(h.keep_alpha));
            let resp = bulle(resp, "Préserver la transparence", "Le pinceau ne colore que ce qui est déjà dessiné : le vide reste vide. La gomme gomme normalement.", "");
            if resp.clicked() {
                self.set_layer_flag(li, |l| l.keep_alpha = !l.keep_alpha);
            }
            let icon = if h.negative { ph::EXCLUDE } else { ph::INTERSECT };
            let resp = icon_at(ui, place(ICON_W, &mut x), icon, shade(h.stencil));
            let resp = bulle(resp, "Pochoir", "Le dessin de ce calque limite où l'on peint sur les autres calques (pinceau, crayon, gomme bitmap).", "");
            if resp.clicked() {
                self.basculer_calques(li, "Pochoir", |l| l.stencil, |l, v| l.stencil = v, false);
            }
            let resp = icon_at(ui, place(ICON_W, &mut x), if h.negative { ph::MINUS } else { ph::PLUS }, shade(h.stencil));
            let resp = bulle(
                resp,
                if h.negative { "Pochoir négatif" } else { "Pochoir positif" },
                "Positif : on ne peint que dans la forme du pochoir. Négatif : seulement en dehors. Clic : changer.",
                "",
            );
            if resp.clicked() {
                self.basculer_calques(li, "Pochoir négatif", |l| l.stencil_negative, |l, v| l.stencil_negative = v, false);
            }
            let resp = icon_at(ui, place(ICON_W, &mut x), ph::PUSH_PIN, shade(h.fixed));
            let resp = bulle(resp, "Verrouiller la position", "L'outil Transformer ne bouge plus le calque, et on ne peut plus dessiner dessus.", "");
            if resp.clicked() {
                self.set_layer_flag(li, |l| l.position_locked = !l.position_locked);
            }
            let r = place(ICON_W, &mut x);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt(("groupe_calque", h.id)));
            let button = egui::Button::new(RichText::new(ph::FOLDER_SIMPLE).size(13.0).color(group_color(group.color))).frame(false);
            let resp = egui::menu::menu_custom_button(&mut child, button, |ui| self.group_menu(ui, li)).response;
            bulle(resp, &format!("Groupe : {}", group.name), "Ranger le calque dans un groupe de couleur.", "");
            // Les effets du calque, d'un coup (seulement s'il en a : `fxui.rs`).
            if self.fx_switch_shown(li) {
                let r = place(ICON_W, &mut x);
                if let Some(resp) = self.fx_switch(ui, li, Some(r), fg) {
                    if resp.clicked() {
                        self.toggle_layer_fx(li);
                    }
                }
            }
            // Accroché à la caméra (`accroche.rs`) : le calque garde sa place dans l'image, quoi que fasse la caméra.
            if !h.cfx && !h.sous_trait {
                let resp = icon_at(ui, place(ICON_W, &mut x), ph::VIDEO_CAMERA, shade(h.accroche));
                if self.aide(resp, crate::accroche::NOM, crate::accroche::PHRASE, crate::accroche::ACTION).clicked() {
                    self.basculer_accroche(li);
                }
            }
            // L'aplat sous le trait et le maillon de la planche-mère.
            let bout = Rect::from_min_size(pos2(x, y), vec2(46.0, 18.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bout).id_salt(("bout_calque", h.id)).layout(Layout::left_to_right(Align::Center)));
            child.spacing_mut().item_spacing.x = 2.0;
            child.spacing_mut().interact_size.y = 16.0;
            child.spacing_mut().button_padding = vec2(1.0, 0.0);
            child.set_clip_rect(bout.intersect(ui.clip_rect()));
            if let Some(resp) = self.fond_layer_button(&mut child, li, true) {
                if resp.clicked() {
                    self.toggle_fond(li);
                }
            }
            self.maillon_partage(&mut child, li);
            x = child.min_rect().right().max(x) + 6.0;
            // Un calque Couleur unie : sa couleur, qu'un clic change (elle était à la place de la vignette de la ligne complète).
            if h.unie {
                let r = place(36.0, &mut x);
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt(("unie_calque", h.id)));
                child.spacing_mut().interact_size = r.size();
                self.couleur_unie_champ(&mut child, li);
                x += 6.0;
            }
        }
        // Opacité. Un calque : sa miniature à l'image courante, sur laquelle on glisse (partie 164.3 : elle se voile, une barre fine
        // dit la valeur). Un dossier et un calque Couleur unie (sa miniature est sa couleur, plus haut) : une barre qu'on tire. À
        // droite, le losange des clés d'opacité.
        // Opacité (partie 165.3 : la barre « 100 % » en texte, de retour). Un calque : d'abord sa miniature à l'image courante, pour
        // voir de quel dessin il s'agit (un calque Couleur unie a sa couleur, plus haut). À droite, le losange des clés d'opacité.
        x += 6.0;
        if !h.folder && !h.unie {
            let fr = self.doc.frame_rect();
            let tw = 36.0;
            let th = (tw * fr.height() / fr.width().max(1.0)).clamp(10.0, BARRE_H - 4.0);
            let thumb = Rect::from_min_size(pos2(x, rect.center().y - th * 0.5), vec2(tw, th));
            x = thumb.right() + 6.0;
            match self.layers()[li].cel_at(self.cur_frame).and_then(|c| self.layer_thumb(ui.ctx(), c)) {
                Some(t) => {
                    let tint = if h.visible { Color32::WHITE } else { Color32::from_gray(120) };
                    painter.image(t.id(), thumb, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
                }
                None => {
                    painter.rect_filled(thumb, 0.0, theme::controle());
                }
            }
            painter.rect_stroke(thumb, 0.0, Stroke::new(1.0_f32, theme::bord()), StrokeKind::Inside);
        }
        let bar = Rect::from_min_size(pos2(x, y + 2.5), vec2(if h.folder { 108.0 } else { 92.0 }, 13.0));
        x = bar.right() + 2.0;
        let (nom_op, desc_op) = if h.folder { ("Opacité du dossier", "Elle s'ajoute à celle de chacun de ses calques.") } else { ("Opacité du calque", "Cliquer ou tirer dans la barre.") };
        self.opacity_bar(ui, &painter, bar, rect, li, egui::Id::new(("opacite_calque", h.id)), h.opacity, true, nom_op, desc_op);
        if !h.folder {
            let losange = place(16.0, &mut x);
            self.losange_opacite(ui, losange, fg, off);
            // Mode de fusion : la liste « Normal ▾ » en texte (partie 165.3), avec précédent / suivant.
            let modes = BlendMode::ALL;
            let at = modes.iter().position(|m| *m == h.blend).unwrap_or(0);
            let mut mode = h.blend;
            let prev = place(14.0, &mut x);
            if bulle(icon_at(ui, prev, ph::CARET_LEFT, fg), "Mode précédent", "", "").clicked() {
                mode = modes[(at + modes.len() - 1) % modes.len()];
            }
            let combo = place(130.0, &mut x);
            mode = blend_combo(ui, combo, egui::Id::new(("fusion_tete", h.id)), h.blend).unwrap_or(mode);
            let next = place(14.0, &mut x);
            if bulle(icon_at(ui, next, ph::CARET_RIGHT, fg), "Mode suivant", "", "").clicked() {
                mode = modes[(at + 1) % modes.len()];
            }
            if mode != h.blend {
                // (Plusieurs calques choisis : tous prennent ce mode, partie 136.)
                self.regler_mode_calques(li, mode);
            }
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
            // (Plusieurs calques choisis : tous prennent cette valeur, comme dans l'ancien panneau Calques, `choixtout.rs`.)
            self.regler_opacite(li, value);
            // (Un réglage : aucune tuile n'est renvoyée à la carte graphique pendant le geste.)
            self.renderer.invalidate_params();
        }
    }

    /// Ce que la ligne du calque `li` dit en petit après son nom, quand il est le calque courant (partie 160 : c'était sous son nom
    /// dans le panneau Calques), et sa bulle : « sous <trait> » pour un calque de couleur lié, le nombre de couleurs d'un trait.
    pub(crate) fn sous_le_nom(&self, li: usize) -> Option<(String, &'static str)> {
        let layers = self.layers();
        if let Some(ti) = crate::calquecouleur::trait_de(layers, li) {
            return Some((format!("{} sous {}", ph::LINK_SIMPLE, layers[ti].name), "Calque de couleur : ses images suivent celles du calque de trait. On colorie dessous, le trait reste intact."));
        }
        let n = crate::calquecouleur::couleurs_de(layers, li).len();
        (n > 0).then(|| (format!("{} {n}", ph::PALETTE), "Ce calque de trait porte des calques de couleur, rangés juste dessous."))
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
