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
/// Hauteur d'une ligne fine (partie 160) : tout calque qui n'est pas le calque courant, et tout dossier qui ne l'est pas
/// (nom, œil, verrou, effets ; le triangle d'un dossier).
pub(crate) const ROW_FINE: f32 = 22.0;
/// Hauteur d'une ligne complète, celle du calque courant, sur trois rangées (partie 174) : ⊖ et le nom (20) ; la vignette, les
/// indicateurs et les colonnes (21) ; le mode, l'opacité et le losange (19).
pub(crate) const ROW_FULL: f32 = 60.0;
/// Partie 174 : le haut de la deuxième et de la troisième rangée de la ligne complète, depuis le haut de la ligne.
const RANGEE_B: f32 = 20.0;
const RANGEE_C: f32 = 41.0;
/// Partie 174 : la hauteur de la rangée d'en-tête de la colonne des calques (« Calques » et une icône par colonne), sous les deux
/// lignes de boutons ; la règle des images a la même hauteur en plus, en face (`timeline::HEADER_H`).
pub(crate) const ENTETE_H: f32 = 20.0;
/// Partie 174 : la marge à droite des colonnes de réglages.
const MARGE_COLONNES: f32 = 3.0;
/// Partie 174 : la liste du mode de fusion de la troisième rangée (« Superposition ▾ » en entier) et ses flèches ‹ ›.
const LISTE_W: f32 = 96.0;
const FLECHE_W: f32 = 14.0;
/// Partie 174 : l'action « replier / déplier tous les calques » de la timeline (⊖ / ⊕ de l'en-tête).
pub(crate) const ACTION_REPLIER: &str = "calques.replier_tout";
/// Hauteur de la ligne d'un dossier courant (13b-5) : nom ; afficher, verrouiller, opacité.
pub(crate) const ROW_FOLDER: f32 = 40.0;
/// Décalage des lignes des calques rangés dans un dossier.
const INDENT: f32 = 12.0;
/// Largeur d'une icône de réglage.
pub(crate) const ICON_W: f32 = 19.0;

/// Une ligne de calque de la timeline : le calque, le haut de la ligne (depuis le haut des
/// lignes) et sa hauteur.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Row {
    pub li: usize,
    pub top: f32,
    pub h: f32,
}

/// Lignes des calques, du plus haut au plus bas ; les calques d'un groupe caché de la timeline
/// (filtre de groupe, 13b-4) ou d'un dossier fermé (13b-5) n'en ont pas, sauf le calque courant (partie 165.4). Le calque courant
/// `cur` a toujours sa ligne complète,
/// tous les autres une ligne fine (partie 160, automatique : le drapeau « réduit » d'un calque ne compte plus ici ; celui d'un
/// dossier dit toujours s'il est fermé).
#[cfg(test)]
pub(crate) fn rows_of(layers: &[Layer], groups: &[LayerGroup], cur: usize) -> Vec<Row> {
    rows_of_en(layers, groups, cur, false)
}

/// Partie 174 (Anthony : « on peut aussi replier tous les calques ») : les lignes de `rows_of`, et `tout_replie` : le calque courant
/// aussi sur une ligne fine (il garde sa place et sa surbrillance ; ses réglages restent dans les colonnes et au clic droit).
pub(crate) fn rows_of_en(layers: &[Layer], groups: &[LayerGroup], cur: usize, tout_replie: bool) -> Vec<Row> {
    let closed: Vec<u64> = layers.iter().filter(|l| l.folder && l.collapsed).map(|l| l.id).collect();
    let mut top = 0.0;
    layers
        .iter()
        .enumerate()
        .rev()
        // (Partie 165.4 : le calque courant garde toujours sa ligne complète, même dans un groupe caché ou un dossier fermé.)
        .filter(|(li, l)| *li == cur || groups.get(l.group as usize).is_none_or(|g| g.shown))
        .filter(|(li, l)| *li == cur || l.parent.is_none_or(|p| !closed.contains(&p)))
        .map(|(li, l)| {
            let h = match (l.folder, li == cur && !tout_replie) {
                (true, true) => ROW_FOLDER,
                (false, true) => ROW_FULL,
                (_, false) => ROW_FINE,
            };
            let row = Row { li, top, h };
            top += h;
            row
        })
        .collect()
}

/// Partie 165.4 : où se dessine la ligne `r` du calque courant quand on ne voit que `vue` de la colonne : à sa place si elle y
/// tient entière ; sinon collée au bord de `vue` qu'elle dépasse (en haut d'abord, si la vue est plus basse qu'elle).
pub(crate) fn ligne_toujours_visible(r: Rect, vue: Rect) -> Rect {
    if !vue.is_positive() {
        return r;
    }
    let mut y = r.top();
    if r.bottom() > vue.bottom() {
        y = vue.bottom() - r.height();
    }
    if y < vue.top() {
        y = vue.top();
    }
    Rect::from_min_size(pos2(r.left(), y), r.size())
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

/// Le fond d'une ligne de calque (en-tête et grille), teinté de la couleur du rangement du calque (partie 179, venue de la timeline
/// plus claire) : celle de son dossier, sinon celle de son groupe ; rien, le fond d'origine. Un dossier est teinté plus fort : sa
/// ligne sert de titre.
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

/// Partie 174 (la disposition validée par Anthony) : les colonnes de réglages de la colonne des calques, dans l'ordre de la rangée
/// d'en-tête, de gauche à droite. Chaque calque a les siennes sur les mêmes x que l'en-tête, qu'il soit le calque courant ou non.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Colonne {
    Afficher,
    Verrouiller,
    Pelure,
    Transparence,
    Pochoir,
    Aplat,
    Camera,
}

impl Colonne {
    pub(crate) const TOUTES: [Colonne; 7] = [Colonne::Afficher, Colonne::Verrouiller, Colonne::Pelure, Colonne::Transparence, Colonne::Pochoir, Colonne::Aplat, Colonne::Camera];

    /// L'icône de la colonne (dans l'en-tête, et sur un calque où le réglage est allumé).
    pub(crate) fn icone(self) -> &'static str {
        match self {
            Colonne::Afficher => ph::EYE,
            Colonne::Verrouiller => ph::LOCK_SIMPLE,
            Colonne::Pelure => ph::CIRCLES_THREE,
            Colonne::Transparence => ph::CHECKERBOARD,
            Colonne::Pochoir => ph::INTERSECT,
            Colonne::Aplat => ph::USER_SQUARE,
            Colonne::Camera => ph::VIDEO_CAMERA,
        }
    }

    /// Le nom de sa bulle (le même que celui de l'icône d'avant).
    pub(crate) fn nom(self) -> &'static str {
        match self {
            Colonne::Afficher => "Afficher / cacher",
            Colonne::Verrouiller => "Verrouiller / déverrouiller",
            Colonne::Pelure => "Pelure d'oignon du calque",
            Colonne::Transparence => "Préserver la transparence",
            Colonne::Pochoir => "Pochoir",
            Colonne::Aplat => "Aplat sous le trait",
            Colonne::Camera => crate::accroche::NOM,
        }
    }

    /// Ce que fait le réglage (la description de sa bulle).
    pub(crate) fn description(self) -> &'static str {
        match self {
            Colonne::Afficher => "Montrer ou cacher le calque (et sa couleur, si c'est un trait).",
            Colonne::Verrouiller => "Un calque verrouillé ne peut plus être dessiné.",
            Colonne::Pelure => "Quand la pelure d'oignon est allumée, montrer aussi les dessins voisins de ce calque, même s'il n'est pas le calque courant. Clic droit : le panneau de la pelure.",
            Colonne::Transparence => "Le pinceau ne colore que ce qui est déjà dessiné : le vide reste vide. La gomme gomme normalement.",
            Colonne::Pochoir => "Le dessin de ce calque limite où l'on peint sur les autres calques (pinceau, crayon, gomme bitmap). Son sens (+ / −) est à côté de la vignette du calque courant.",
            Colonne::Aplat => "Tout ce que le trait enferme, plus un léger débord, en une seule forme sous le trait, qui se recalcule à chaque trait. Ses réglages : clic droit sur le calque, « Réglages de l'aplat ».",
            Colonne::Camera => crate::accroche::PHRASE,
        }
    }

    /// L'action du fichier de raccourcis qui fait la même chose (son raccourci est écrit dans la bulle), s'il y en a une.
    pub(crate) fn action(self) -> &'static str {
        match self {
            Colonne::Verrouiller => "calque.verrouiller",
            Colonne::Pelure => "calque.pelure",
            Colonne::Aplat => "pinceau.fond",
            Colonne::Camera => crate::accroche::ACTION,
            _ => "",
        }
    }

    /// L'état du réglage sur le calque `l` ; rien quand il n'a pas de sens pour lui (un dossier n'a que l'œil et le cadenas ; un
    /// calque vidéo n'a pas d'aplat ; un calque d'effet ou de couleur liée ne s'accroche pas lui-même à la caméra).
    pub(crate) fn etat(self, l: &Layer) -> Option<bool> {
        match self {
            Colonne::Afficher => Some(l.visible),
            Colonne::Verrouiller => Some(l.locked),
            _ if l.folder => None,
            Colonne::Pelure => Some(l.onion),
            Colonne::Transparence => Some(l.keep_alpha),
            Colonne::Pochoir => Some(l.stencil),
            Colonne::Aplat => crate::fondui::takes_fond(l).then_some(l.fond.is_some()),
            Colonne::Camera => (l.effet.is_none() && !l.keys_follow_line()).then_some(l.accroche),
        }
    }
}

/// Partie 174 : la case de la colonne `k` (rang dans `Colonne::TOUTES`) d'une colonne des calques dont le bord droit est `droite`,
/// à la hauteur `y` : les mêmes x pour l'en-tête et pour chaque calque, dossiers et calques rangés compris (leur retrait ne touche
/// que leur gauche).
pub(crate) fn case_de_colonne(droite: f32, k: usize, y: f32) -> Rect {
    let n = Colonne::TOUTES.len();
    let gauche = droite - MARGE_COLONNES - (n - k) as f32 * ICON_W;
    Rect::from_min_size(pos2(gauche, y), vec2(ICON_W, 17.0))
}

/// Partie 174 : le bord gauche de la première colonne (tout ce qui est avant, nom, indicateurs, s'arrête là).
pub(crate) fn debut_des_colonnes(droite: f32) -> f32 {
    case_de_colonne(droite, 0, 0.0).left()
}

/// Partie 174 : une case de réglage : l'icône quand le réglage est allumé ; éteint, un petit point discret (l'icône pâle au survol,
/// pour savoir ce qu'un clic allumera). `id` : son identifiant.
pub(crate) fn case_de_reglage(ui: &mut Ui, rect: Rect, id: egui::Id, icone: &str, on: bool, encre: Color32) -> egui::Response {
    let resp = ui.interact(rect, id, Sense::click());
    let painter = ui.painter();
    if on {
        painter.text(rect.center(), Align2::CENTER_CENTER, icone, FontId::proportional(13.0), encre);
    } else if resp.hovered() {
        painter.text(rect.center(), Align2::CENTER_CENTER, icone, FontId::proportional(13.0), encre.gamma_multiply(0.45));
    } else {
        painter.circle_filled(rect.center(), 1.6, encre.gamma_multiply(0.4));
    }
    resp
}

/// Partie 173 : la liste des modes de fusion de la piste du calque actif (Storyboard), posée exactement dans `rect` : le même
/// aspect que l'ancienne liste de la timeline (une `ComboBox`, remplacée par celle-ci en partie 174), mais un nom trop long est raccourci (« Superpos… ») sans sortir de `rect`, et la
/// liste ouverte reste celle de la piste d'avant (les modes en texte, cochés, par familles). Le mode choisi, s'il a changé.
pub(crate) fn liste_des_modes(ui: &mut Ui, rect: Rect, salt: egui::Id, blend: BlendMode) -> Option<BlendMode> {
    const MARGE: f32 = 4.0;
    let mut mode = blend;
    // La case : un bouton encadré de la taille de `rect`, comme la case d'une liste ; le nom et la flèche sont peints dessus (le
    // nom raccourci à la place qui reste). Le menu est celui de la piste d'avant : il se place de lui-même, sans défiler.
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(salt.with("cadre")));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child.spacing_mut().button_padding = vec2(0.0, 0.0);
    let case = egui::Button::new("").min_size(rect.size());
    let resp = egui::menu::menu_custom_button(&mut child, case, |ui| {
        for m in BlendMode::ALL {
            if BlendMode::FAMILY_STARTS.contains(&m) {
                ui.separator();
            }
            if ui.radio(m == blend, m.label()).on_hover_text(m.hint()).clicked() {
                ui.close_menu();
                mode = m;
            }
        }
    })
    .response;
    let case = resp.rect;
    let encre = child.style().interact(&resp).text_color();
    let fleche = child.spacing().icon_width.min(12.0);
    let place = (case.width() - 2.0 * MARGE - fleche - 2.0).max(1.0);
    let nom = egui::WidgetText::from(RichText::new(blend.label()).size(11.0)).into_galley(&child, Some(egui::TextWrapMode::Truncate), place, egui::TextStyle::Button);
    let painter = child.painter();
    painter.galley(pos2(case.left() + MARGE, case.center().y - nom.size().y * 0.5), nom, encre);
    // La flèche ▾, comme celle des listes.
    let f = Rect::from_center_size(pos2(case.right() - MARGE - fleche * 0.5, case.center().y), vec2(fleche * 0.7, fleche * 0.45));
    painter.add(egui::Shape::convex_polygon(vec![f.left_top(), f.right_top(), f.center_bottom()], encre, Stroke::NONE));
    bulle(resp, "Mode de fusion", &format!("{} : {}", blend.label(), blend.hint()), "");
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
    stencil: bool,
    negative: bool,
    fixed: bool,
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
        let rows = self.lignes_de_la_timeline();
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
        // ligne complète dans tous les cas, partie 165.4 ; son groupe se remontre par son menu, son dossier par son triangle.)
        // Partie 165.6 (Anthony : « Deux lignes de boutons ») : la barre sur deux lignes, la première en face des numéros de la règle,
        // la seconde juste dessous ; la colonne ne prend plus que la largeur de la plus longue des deux.
        let h1 = crate::timeline::LIGNE1_H.min(header_h);
        // Partie 174 : sous les deux lignes de boutons, la rangée d'en-tête des colonnes (seulement si l'en-tête a sa hauteur entière).
        let entete_h = if header_h >= crate::timeline::LIGNE1_H + ENTETE_H { ENTETE_H } else { 0.0 };
        let ligne1 = Rect::from_min_size(tete.min, vec2(w, h1));
        let ligne2 = Rect::from_min_max(pos2(tete.left(), tete.top() + h1), pos2(tete.right(), tete.bottom() - entete_h));
        self.barre_dans_la_colonne_deux_lignes(ui, ligne1, ligne2, crate::barres::Barre::Timeline, crate::timeline::DERNIER_GROUPE_LIGNE1);
        if entete_h > 0.0 {
            let entete = Rect::from_min_max(pos2(tete.left(), tete.bottom() - entete_h), tete.max);
            self.entete_des_colonnes(ui, entete);
        }
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
        let cur = self.cur_layer;
        for row in rows.iter().filter(|r| r.li != cur) {
            if !self.calques_tiennent(&avant, row.li) {
                tient = false;
                break;
            }
            let r = Rect::from_min_size(pos2(rect.left(), top + row.top), vec2(w, row.h));
            self.layer_head(ui, r, row.li);
        }
        // Partie 165.4 (Anthony : « je veux que cette version toujours visible », « jamais repliée ») : la ligne complète du calque
        // courant se dessine en dernier, par-dessus les autres, et ne sort jamais de la partie visible de la colonne. Quand la
        // timeline est trop basse pour tous les calques et qu'on la fait défiler, elle reste collée en haut ou en bas de ce qu'on voit
        // (comme une ligne figée d'un tableur) ; sa rangée dans la grille, elle, défile.
        if tient {
            if let Some(row) = rows.iter().find(|r| r.li == cur) {
                if self.calques_tiennent(&avant, row.li) {
                    let r = Rect::from_min_size(pos2(rect.left(), top + row.top), vec2(w, row.h));
                    let r = ligne_toujours_visible(r, ui.clip_rect());
                    if (r.top() - (top + row.top)).abs() > 0.5 {
                        // (Détachée de sa place : une ombre sous elle et le fond de la colonne derrière, pour qu'on voie qu'elle flotte.)
                        ui.painter().rect_filled(r.expand2(vec2(0.0, 2.0)), 0.0, theme::regle());
                        ui.painter().rect_filled(Rect::from_min_size(pos2(r.left(), r.bottom() + 1.0), vec2(r.width(), 2.0)), 0.0, Color32::from_black_alpha(90));
                    }
                    self.layer_head(ui, r, row.li);
                } else {
                    tient = false;
                }
            }
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
                stencil: l.stencil,
                negative: l.stencil_negative,
                fixed: l.position_locked,
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
        // Calque courant : l'accent ; calques choisis en plus (Ctrl + clic) : l'accent atténué.
        // La couleur du rangement du calque — celle de son dossier, sinon celle de son groupe (la même règle que dans le Storyboard et
        // le Montage, `folders::couleur_du_rangement`, 2026-11-11) : le filet de gauche, et une teinte sur toute la ligne (partie 179,
        // venue de la timeline plus claire ; la grille fait de même, `timeline.rs`).
        let rangement = crate::folders::couleur_du_rangement(self.layers(), li, &self.doc.groups);
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
        let shade = |on: bool| if on { fg } else { off };
        let group = self.doc.groups[h.group].clone();
        // Le filet de gauche : la couleur du rangement du calque — celle de son dossier, sinon celle de son groupe (la même règle que
        // dans le Storyboard et le Montage, `folders::couleur_du_rangement`, 2026-11-11).
        let filet = rangement.unwrap_or_else(|| group_color(group.color));
        painter.rect_filled(Rect::from_min_size(r.min, vec2(4.0, r.height())), 0.0, filet);

        // Le fond de la ligne : choisir, glisser, menu, double-clic sur le nom. Posé avant les
        // boutons, qui restent au-dessus.
        let resp = ui.interact(r, egui::Id::new(("tete_calque", h.id)), Sense::click_and_drag());
        // Partie 160 : la ligne fine (tout calque ou dossier qui n'est pas le courant) ou la ligne complète, celle du courant
        // (`rows_of`). Partie 174 : « tout replier » (⊖ de l'en-tête) met aussi le courant sur une ligne fine.
        let fine = li != self.cur_layer || self.settings.calques_replies;
        // Partie 174 (la disposition validée par Anthony) : les colonnes de réglages, à droite, sur les x de la rangée d'en-tête ;
        // tout le reste de la ligne s'arrête avant elles.
        let cols = debut_des_colonnes(r.right());
        let x0 = if h.folder { r.left() + 22.0 } else { r.left() + 24.0 };
        // (Sur la ligne fine d'un dossier, le nombre de ses calques est juste avant les colonnes ; sur la ligne fine d'un calque, la
        // pastille de sa couleur d'étiquette et ses effets ; sur la ligne complète, le ⋮ du menu est au bout de la première rangée.)
        let fin_du_nom = match (h.folder, fine) {
            (true, true) => cols - 22.0,
            (false, true) => cols - 4.0 - if h.group > 0 { 10.0 } else { 0.0 } - if self.fx_switch_shown(li) { 20.0 } else { 0.0 },
            (_, false) => r.right() - 22.0,
        };
        let name_rect = Rect::from_min_max(pos2(x0 + 14.0, r.top() + 2.0), pos2(fin_du_nom.max(x0 + 15.0), r.top() + 20.0_f32.min(r.height())));
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

        // Triangle d'un dossier : montrer ou cacher ses calques. (Plus de triangle sur les calques, partie 160 : leur ligne est
        // complète quand ils sont le calque courant, fine sinon, d'elle-même.)
        if h.folder {
            let caret = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 2.0), vec2(16.0, 18.0));
            let resp = icon_at(ui, caret, if h.collapsed { ph::CARET_RIGHT } else { ph::CARET_DOWN }, off.gamma_multiply(2.0));
            let (what, why) = if h.collapsed {
                ("Ouvrir le dossier", "Montrer les lignes de ses calques.")
            } else {
                ("Fermer le dossier", "Cacher les lignes de ses calques (ils restent à l'image).")
            };
            if bulle(resp, what, why, "").clicked() {
                self.layers_mut()[li].collapsed = !h.collapsed;
                self.modified = true;
            }
        } else {
            // Partie 174 : ⊖ replie la ligne du calque courant (tous les calques sur une ligne) ; ⊕ sur une ligne fine la déplie : le
            // calque devient le calque courant, sur ses trois rangées (seul le calque courant a sa ligne complète, partie 160).
            let bouton = Rect::from_min_size(pos2(r.left() + 5.0, r.top() + 2.0), vec2(16.0, 18.0));
            let resp = icon_at(ui, bouton, if fine { ph::PLUS_CIRCLE } else { ph::MINUS_CIRCLE }, off.gamma_multiply(2.2));
            let key = self.raccourci(ACTION_REPLIER);
            crate::raccourcisui::shortcut_menu(&resp, ACTION_REPLIER, &key);
            let resp = if fine {
                bulle(resp, "Déplier le calque", "Il devient le calque courant, sa ligne sur trois rangées : la vignette, les réglages, le mode, l'opacité.", &key)
            } else {
                bulle(resp, "Replier le calque", "Tous les calques sur une ligne, le calque courant compris (ses réglages restent dans les colonnes et au clic droit). ⊕ le déplie.", &key)
            };
            if resp.clicked() {
                if fine {
                    if li != self.cur_layer {
                        self.click_layer_head(li, false, false);
                    }
                    self.settings.calques_replies = false;
                } else {
                    self.settings.calques_replies = true;
                }
            }
        }

        // Nom (ou son champ quand on le renomme), sorte du calque, menu d'options.
        let line1 = r.top() + 11.0;
        let badge = if current { fg } else if h.cfx { theme::palette().dessin_cfx } else if h.annotation { theme::palette().dessin_annotation } else if h.unie { theme::palette().dessin_unie } else { theme::couleur_calque(h.kind, h.couleur) };
        if h.folder {
            // (Partie 179 : l'icône du dossier prend sa couleur, sa ligne sert de titre.)
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
        } else {
            // Un dossier : son nom en titre (partie 179, venue de la timeline plus claire), dans la famille des titres — demi-gras quand
            // les polices Plex sont chargées (`theme::densifier_avec`).
            let police = if h.folder {
                FontId::new(12.5, ui.style().text_styles.get(&egui::TextStyle::Heading).map_or(egui::FontFamily::Proportional, |f| f.family.clone()))
            } else {
                FontId::proportional(13.0)
            };
            let nom = painter.with_clip_rect(name_rect).text(pos2(name_rect.left(), line1), Align2::LEFT_CENTER, &h.name, police, fg);
            // Le calque courant : ce que sa ligne du panneau Calques disait sous son nom (partie 160) : « sous <trait> » pour un calque
            // de couleur, le nombre de couleurs d'un calque de trait.
            if !fine && !h.folder {
                if let Some(petit) = self.sous_le_nom(li) {
                    let at = pos2(nom.right() + 8.0, line1);
                    let resp = ui.interact(Rect::from_min_max(pos2(at.x, r.top() + 2.0), pos2(name_rect.right(), r.top() + 20.0)), egui::Id::new(("sous_le_nom", h.id)), Sense::hover());
                    painter.with_clip_rect(name_rect).text(at, Align2::LEFT_CENTER, &petit.0, FontId::proportional(10.5), off.gamma_multiply(2.2));
                    bulle(resp, &petit.0, petit.1, "");
                }
            }
        }

        // Les colonnes de réglages : sur la première (et seule) rangée d'une ligne fine, sur la deuxième d'une ligne complète.
        let y_cols = if fine { r.top() + 2.0 } else { r.top() + RANGEE_B };
        self.colonnes_du_calque(ui, r.right(), y_cols, li, h.id, h.negative, fg);

        if fine && !h.folder {
            // La pastille de sa couleur d'étiquette (son groupe, s'il en a un), puis ses effets, juste avant les colonnes.
            let mut x = cols - 2.0;
            if let Some(resp) = self.fx_switch(ui, li, Some(Rect::from_min_size(pos2(x - 20.0, r.top() + 2.0), vec2(20.0, 18.0))), fg) {
                if resp.clicked() {
                    self.toggle_layer_fx(li);
                }
                x -= 20.0;
            }
            if h.group > 0 {
                let c = Rect::from_center_size(pos2(x - 5.0, line1), vec2(10.0, 18.0));
                painter.circle_filled(c.center(), 3.5, group_color(group.color));
                let resp = ui.interact(c, egui::Id::new(("etiquette_calque", h.id)), Sense::hover());
                bulle(resp, &format!("Groupe : {}", group.name), "La couleur d'étiquette du calque (son groupe). Le changer : sur la ligne du calque courant, ou clic droit.", "");
            }
            return;
        }
        // La ligne fine d'un dossier (partie 160) : à droite du nom, le nombre de ses calques ; son œil et son cadenas sont dans les
        // colonnes ; son opacité est sur sa ligne complète, quand il est le calque courant, et au clic droit (Apparence).
        if h.folder && fine {
            painter.text(pos2(cols - 6.0, line1), Align2::RIGHT_CENTER, h.children.to_string(), FontId::monospace(10.5), off.gamma_multiply(2.0));
            return;
        }
        let opts = Rect::from_min_size(pos2(r.right() - 20.0, r.top() + 2.0), vec2(18.0, 18.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(opts).id_salt(("options_calque", h.id)));
        let button = egui::Button::new(RichText::new(ph::DOTS_THREE_VERTICAL).size(13.0).color(fg)).frame(false);
        let resp = egui::menu::menu_custom_button(&mut child, button, |ui| self.layer_head_menu(ui, li)).response;
        bulle(resp, "Commandes du calque", "Renommer, profondeur, dupliquer, verrouiller, supprimer, groupe… (aussi : clic droit sur la ligne).", "");

        // Dossier (13b-5) : son opacité (elle agit sur ses calques) et le nombre de ses calques ; son œil et son cadenas sont dans les
        // colonnes.
        if h.folder {
            let y2 = r.top() + 21.0;
            let bar = Rect::from_min_size(pos2(r.left() + 20.0, y2 + 2.0), vec2(108.0_f32.min((cols - 70.0 - r.left()).max(40.0)), 13.0));
            self.opacity_bar(ui, &painter, bar, r, li, egui::Id::new(("opacite_calque", h.id)), h.opacity, current, "Opacité du dossier", "Elle s'ajoute à celle de chacun de ses calques.");
            let n = h.children;
            let count = format!("{n} calque{}", if n > 1 { "s" } else { "" });
            painter.with_clip_rect(Rect::from_min_max(pos2(bar.right(), r.top()), pos2(cols - 2.0, r.bottom()))).text(pos2(bar.right() + 8.0, bar.center().y), Align2::LEFT_CENTER, count, FontId::proportional(11.0), off.gamma_multiply(2.0));
            return;
        }

        // ---- La deuxième rangée : la vignette du dessin à l'image courante (vide : cadre gris), les petits indicateurs (effets,
        // groupe, sens du pochoir, position verrouillée, maillon de la planche-mère), puis les colonnes (posées plus haut).
        let yb = r.top() + RANGEE_B;
        let hb = RANGEE_C - RANGEE_B - 1.0;
        let thumb = Rect::from_min_size(pos2(r.left() + 7.0, yb), vec2(hb * 16.0 / 9.0, hb));
        // Un calque Couleur unie : sa couleur à la place de la vignette, qu'un clic change (partie 160 : elle était sous l'opacité du
        // panneau Calques).
        if h.unie {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(thumb).id_salt(("unie_calque", h.id)));
            child.spacing_mut().interact_size = thumb.size();
            self.couleur_unie_champ(&mut child, li);
        } else {
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
        }
        let slot = |x: f32| Rect::from_min_size(pos2(x, yb + 1.0), vec2(ICON_W, 17.0));
        let mut x = thumb.right() + 4.0;
        // Les effets du calque, d'un coup (seulement s'il en a : `fxui.rs`).
        if let Some(resp) = self.fx_switch(ui, li, Some(slot(x)), fg) {
            if resp.clicked() {
                self.toggle_layer_fx(li);
            }
            x += ICON_W;
        }
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(slot(x)).id_salt(("groupe_calque", h.id)));
        let button = egui::Button::new(RichText::new(ph::FOLDER_SIMPLE).size(13.0).color(group_color(group.color))).frame(false);
        let resp = egui::menu::menu_custom_button(&mut child, button, |ui| self.group_menu(ui, li)).response;
        bulle(resp, &format!("Groupe : {}", group.name), "Ranger le calque dans un groupe de couleur.", "");
        x += ICON_W;
        let resp = icon_at(ui, slot(x), if h.negative { ph::MINUS } else { ph::PLUS }, shade(h.stencil));
        let resp = bulle(
            resp,
            if h.negative { "Pochoir négatif" } else { "Pochoir positif" },
            "Positif : on ne peint que dans la forme du pochoir. Négatif : seulement en dehors. Clic : changer.",
            "",
        );
        if resp.clicked() {
            self.basculer_calques(li, "Pochoir négatif", |l| l.stencil_negative, |l, v| l.stencil_negative = v, false);
        }
        x += ICON_W;
        let resp = icon_at(ui, slot(x), ph::PUSH_PIN, shade(h.fixed));
        let resp = bulle(resp, "Verrouiller la position", "L'outil Transformer ne bouge plus le calque, et on ne peut plus dessiner dessus.", "");
        if resp.clicked() {
            self.set_layer_flag(li, |l| l.position_locked = !l.position_locked);
        }
        x += ICON_W;
        // Le maillon de la planche-mère (rien si le calque n'est dans aucun autre plan), jusqu'aux colonnes.
        {
            let bout = Rect::from_min_max(pos2(x + 2.0, yb), pos2(cols - 2.0, yb + 19.0));
            if bout.width() > 8.0 {
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bout).id_salt(("bout_calque", h.id)).layout(Layout::left_to_right(Align::Center)));
                child.set_clip_rect(bout.intersect(ui.clip_rect()));
                self.maillon_partage(&mut child, li);
            }
        }

        // ---- La troisième rangée, sous la vignette : le mode de fusion écrit (la liste des modes en texte au clic), ses flèches
        // ‹ ›, la barre d'opacité, puis le losange des clés d'opacité à droite.
        let yc = r.top() + RANGEE_C;
        let hc = 17.0_f32.min(r.bottom() - yc);
        let modes = BlendMode::ALL;
        let at = modes.iter().position(|m| *m == h.blend).unwrap_or(0);
        let mut mode = h.blend;
        let gauche = r.left() + 6.0;
        let losange = Rect::from_min_size(pos2(r.right() - 18.0, yc), vec2(16.0, hc));
        let reste = losange.left() - 2.0 - gauche;
        // (Les flèches quand la liste entière et une barre lisible tiennent à côté d'elles.)
        let fleches = reste >= LISTE_W + 2.0 * FLECHE_W + 60.0;
        let mut x = gauche;
        if fleches {
            let prev = Rect::from_min_size(pos2(x, yc), vec2(FLECHE_W, hc));
            if bulle(icon_at(ui, prev, ph::CARET_LEFT, fg), "Mode précédent", "", "").clicked() {
                mode = modes[(at + modes.len() - 1) % modes.len()];
            }
            x += FLECHE_W;
        }
        let largeur = LISTE_W.min((reste - if fleches { 2.0 * FLECHE_W } else { 0.0 } - 40.0).max(26.0));
        let liste = Rect::from_min_size(pos2(x, yc), vec2(largeur, hc));
        if let Some(m) = liste_des_modes(ui, liste, egui::Id::new(("fusion_tete", h.id)), h.blend) {
            mode = m;
        }
        x += largeur;
        if fleches {
            let next = Rect::from_min_size(pos2(x, yc), vec2(FLECHE_W, hc));
            if bulle(icon_at(ui, next, ph::CARET_RIGHT, fg), "Mode suivant", "", "").clicked() {
                mode = modes[(at + 1) % modes.len()];
            }
            x += FLECHE_W;
        }
        if mode != h.blend {
            // (Plusieurs calques choisis : tous prennent ce mode, partie 136.)
            self.regler_mode_calques(li, mode);
        }
        let bar = Rect::from_min_max(pos2(x + 4.0, yc + 2.0), pos2((losange.left() - 2.0).max(x + 20.0), yc + 15.0));
        self.opacity_bar(ui, &painter, bar, r, li, egui::Id::new(("opacite_calque", h.id)), h.opacity, current, "Opacité du calque", "Cliquer ou tirer dans la barre.");
        self.losange_opacite(ui, losange, fg, off);
    }

    /// Partie 174 : les sept colonnes de réglages du calque `li` (`Colonne::TOUTES`), sur les x de l'en-tête (`case_de_colonne`) d'une
    /// colonne dont le bord droit est `droite`, à la hauteur `y`. Une case vide quand le réglage n'a pas de sens pour ce calque. Un
    /// clic : la même action qu'avant (`agir_colonne`), annulable.
    pub(crate) fn colonnes_du_calque(&mut self, ui: &mut Ui, droite: f32, y: f32, li: usize, id: u64, negatif: bool, encre: Color32) {
        let Some(l) = self.layers().get(li) else { return };
        let etats: Vec<Option<bool>> = Colonne::TOUTES.iter().map(|c| c.etat(l)).collect();
        for (k, col) in Colonne::TOUTES.into_iter().enumerate() {
            let Some(on) = etats[k] else { continue };
            let case = case_de_colonne(droite, k, y);
            let resp = match col {
                // (L'œil reste un œil, barré quand le calque est caché.)
                Colonne::Afficher => icon_at(ui, case, if on { ph::EYE } else { ph::EYE_SLASH }, if on { encre } else { encre.gamma_multiply(0.45) }),
                Colonne::Pochoir => case_de_reglage(ui, case, egui::Id::new(("colonne", id, k)), if negatif { ph::EXCLUDE } else { ph::INTERSECT }, on, encre),
                _ => case_de_reglage(ui, case, egui::Id::new(("colonne", id, k)), col.icone(), on, encre),
            };
            let resp = self.aide(resp, col.nom(), col.description(), col.action());
            if resp.clicked() {
                self.agir_colonne(col, li);
            }
            if col == Colonne::Pelure && resp.secondary_clicked() {
                self.onion_window = true;
            }
        }
    }

    /// Partie 174 : un clic dans la colonne `col` du calque `li` : les mêmes actions qu'avant sur sa ligne (une étape d'annulation ;
    /// tous les calques choisis, si `li` en est, sauf « préserver la transparence », qui ne touche encore que le calque cliqué).
    pub(crate) fn agir_colonne(&mut self, col: Colonne, li: usize) {
        match col {
            Colonne::Afficher => self.toggle_layer_visible(li),
            Colonne::Verrouiller => self.toggle_layer_lock(li),
            Colonne::Pelure => {
                self.basculer_calques(li, "Pelure d'oignon", |l| l.onion, |l, v| l.onion = v, false);
            }
            Colonne::Transparence => self.set_layer_flag(li, |l| l.keep_alpha = !l.keep_alpha),
            Colonne::Pochoir => {
                self.basculer_calques(li, "Pochoir", |l| l.stencil, |l, v| l.stencil = v, false);
            }
            Colonne::Aplat => self.toggle_fond(li),
            Colonne::Camera => self.basculer_accroche(li),
        }
    }

    /// Partie 174 : la rangée d'en-tête de la colonne des calques, dans `rect` : le bouton ⊖ / ⊕ (replier / déplier tous les
    /// calques), « Calques », puis une icône par colonne, sur les mêmes x que les colonnes des calques. Un clic sur une icône agit
    /// comme un clic dans la case du calque courant : sur lui et, s'il fait partie d'un choix, sur tous les calques choisis.
    fn entete_des_colonnes(&mut self, ui: &mut Ui, rect: Rect) {
        let painter = ui.painter_at(rect);
        painter.line_segment([pos2(rect.left(), rect.bottom() - 0.5), pos2(rect.right(), rect.bottom() - 0.5)], Stroke::new(1.0_f32, theme::bord()));
        let doux = ui.visuals().weak_text_color();
        let replie = self.settings.calques_replies;
        let bouton = Rect::from_min_size(pos2(rect.left() + 5.0, rect.top() + 1.0), vec2(16.0, rect.height() - 2.0));
        let resp = icon_at(ui, bouton, if replie { ph::PLUS_CIRCLE } else { ph::MINUS_CIRCLE }, doux);
        let (nom, quoi) = if replie {
            ("Déplier les calques", "Le calque courant retrouve sa ligne sur trois rangées : la vignette, les réglages, le mode, l'opacité.")
        } else {
            ("Replier tous les calques", "Tous les calques sur une ligne, le calque courant compris : ses réglages restent dans les colonnes et au clic droit.")
        };
        if self.aide(resp, nom, quoi, ACTION_REPLIER).clicked() {
            self.basculer_calques_replies();
        }
        painter.text(pos2(bouton.right() + 4.0, rect.center().y), Align2::LEFT_CENTER, "Calques", FontId::proportional(12.0), doux);
        self.icones_des_colonnes(ui, rect);
    }

    /// Partie 174 : les icônes des sept colonnes d'une rangée d'en-tête `rect`, alignées à droite (`case_de_colonne`). Un clic agit comme
    /// un clic dans la case du calque courant (et de tous les calques choisis). Aussi sur la ligne « Calques » du Storyboard (178).
    pub(crate) fn icones_des_colonnes(&mut self, ui: &mut Ui, rect: Rect) {
        let doux = ui.visuals().weak_text_color();
        let cur = self.cur_layer;
        let etats: Vec<Option<bool>> = match self.layers().get(cur) {
            Some(l) => Colonne::TOUTES.iter().map(|c| c.etat(l)).collect(),
            None => vec![None; Colonne::TOUTES.len()],
        };
        for (k, col) in Colonne::TOUTES.into_iter().enumerate() {
            let case = case_de_colonne(rect.right(), k, rect.top() + (rect.height() - 17.0) * 0.5);
            let resp = icon_at(ui, case, col.icone(), doux);
            let quoi = format!("Colonne : {} Clic : comme dans la case du calque courant (et de tous les calques choisis).", col.description());
            let resp = self.aide(resp, col.nom(), &quoi, col.action());
            if resp.clicked() {
                if etats[k].is_some() {
                    self.agir_colonne(col, cur);
                } else {
                    self.status = format!("{} : le calque courant n'en a pas (un dossier n'a que l'œil et le cadenas).", col.nom());
                }
            }
        }
    }

    /// Partie 174 : replier tous les calques de la timeline sur une ligne, ou les déplier (le calque courant retrouve ses trois
    /// rangées). Un réglage d'affichage du poste, pas du projet.
    pub(crate) fn basculer_calques_replies(&mut self) {
        self.settings.calques_replies = !self.settings.calques_replies;
        self.status = if self.settings.calques_replies {
            "Calques repliés : tous sur une ligne (⊕ ou le même raccourci les déplie).".into()
        } else {
            "Calques dépliés : le calque courant sur trois rangées.".into()
        };
    }

    /// Partie 174 : les lignes de la colonne des calques de la timeline (`rows_of_en`, repliées ou non).
    pub(crate) fn lignes_de_la_timeline(&self) -> Vec<Row> {
        rows_of_en(self.layers(), &self.doc.groups, self.cur_layer, self.settings.calques_replies)
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
                        // (Partie 174 : sur tous les calques choisis, comme l'icône avant les images de la timeline.)
                        self.regler_comportement(li, false, b);
                    }
                }
            });
            ui.menu_button(format!("Après la dernière image : {}", post.label()), |ui| {
                for b in Behaviour::ALL {
                    if ui.radio(b == post, b.label()).clicked() {
                        ui.close_menu();
                        self.regler_comportement(li, true, b);
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
