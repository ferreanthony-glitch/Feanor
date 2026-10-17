//! Thème de l'interface (12a, repris de Pelure Studio) : les gris sombres et le bleu d'accent du
//! système Spectrum (thème sombre ; valeurs de @spectrum-css/tokens 16), une interface dense, et
//! la police Segoe UI si Windows l'a (sinon celle d'egui). Seulement des couleurs et des tailles :
//! aucun élément ni ressource graphique repris.

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Vec2};

const fn gris(v: u8) -> Color32 {
    Color32::from_rgb(v, v, v)
}

/// Autour de la feuille et entre les panneaux (gray-25).
pub const FOND: Color32 = gris(17);
/// Barres d'onglets et barres d'outils (gray-50).
pub const ENTETE: Color32 = gris(27);
/// Fond des panneaux (gray-75).
pub const PANNEAU: Color32 = gris(34);
/// Onglet actif, ligne choisie, survol léger (gray-100).
pub const ELEVE: Color32 = gris(44);
/// Traits de séparation (gray-200).
pub const BORD: Color32 = gris(50);
/// Boutons au repos, survolés, enfoncés (gray-300, 400, 500).
pub const CONTROLE: Color32 = gris(57);
pub const CONTROLE_SURVOL: Color32 = gris(68);
pub const CONTROLE_ACTIF: Color32 = gris(109);
/// Champs de saisie (plus sombres que le panneau).
pub const CHAMP: Color32 = gris(22);
/// Texte, texte secondaire (gray-800, 600), texte fort (gray-900).
pub const TEXTE: Color32 = gris(219);
pub const TEXTE_DOUX: Color32 = gris(138);
pub const TEXTE_FORT: Color32 = gris(242);
/// Bleu d'accent : fonds (blue-800) et textes (blue-900).
pub const ACCENT: Color32 = Color32::from_rgb(64, 105, 253);
pub const ACCENT_TEXTE: Color32 = Color32::from_rgb(86, 129, 255);

// ------------------------------------------------------------ timeline et montage (12c)

/// Règle des images (au-dessus des rangées).
pub const REGLE: Color32 = ENTETE;
/// Rangée d'un calque ; rangées de la caméra, du son et des pistes, plus sombres.
pub const RANGEE: Color32 = gris(40);
pub const RANGEE_SOMBRE: Color32 = gris(30);
/// Étiquette d'une rangée dans la colonne des noms, case d'un calque sous un panneau.
pub const ETIQUETTE: Color32 = ELEVE;
/// Graduations : une image, une seconde.
pub const GRADUATION: Color32 = BORD;
pub const GRADUATION_SECONDE: Color32 = gris(82);
/// Bloc d'un panneau dans le montage, et sa poignée de durée.
pub const BLOC: Color32 = CONTROLE;
pub const POIGNEE: Color32 = TEXTE_DOUX;
/// Dessins exposés (timeline) : vectoriel (blue-900), bitmap (seafoam-900), dessin vide.
pub const DESSIN_VECTORIEL: Color32 = Color32::from_rgb(86, 129, 255);
pub const DESSIN_BITMAP: Color32 = Color32::from_rgb(10, 154, 128);
pub const DESSIN_VIDE: Color32 = gris(115);
/// Les mêmes, plus sourds, sous les panneaux du montage (blue-700, seafoam-700).
pub const CALQUE_VECTORIEL: Color32 = Color32::from_rgb(52, 91, 248);
pub const CALQUE_BITMAP: Color32 = Color32::from_rgb(6, 122, 103);
/// Image courante (red-900), clés de caméra (le jaune du cadre de la caméra).
pub const IMAGE_COURANTE: Color32 = Color32::from_rgb(252, 67, 46);
pub const CAMERA: Color32 = Color32::from_rgb(255, 196, 60);
/// Marques d'image (13a) : orange (orange-900), distinct du jaune des clés de caméra.
pub const MARQUE: Color32 = Color32::from_rgb(244, 129, 12);
/// Son : extrait (cyan-600, cyan-700 choisi), forme d'onde, piste muette (red-800).
pub const SON_EXTRAIT: Color32 = Color32::from_rgb(3, 99, 140);
pub const SON_CHOISI: Color32 = Color32::from_rgb(8, 115, 168);
pub const SON_ONDE: Color32 = Color32::from_rgb(140, 205, 240);
pub const SON_MUET: Color32 = Color32::from_rgb(223, 52, 34);
/// Barres des séquences et des scènes, alternées pour qu'on voie les limites.
pub const fn barre(sequence: bool, impaire: bool) -> Color32 {
    match (sequence, impaire) {
        (true, false) => gris(40),
        (true, true) => gris(52),
        (false, false) => gris(52),
        (false, true) => gris(64),
    }
}

/// Rapport de contraste (WCAG) entre deux couleurs opaques : de 1 (pareil) à 21 (noir et blanc).
#[cfg(test)]
pub fn contraste(a: Color32, b: Color32) -> f32 {
    let lum = |c: Color32| {
        let lin = |v: u8| {
            let s = v as f32 / 255.0;
            if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Couleurs des fenêtres, panneaux et contrôles.
pub fn visuels() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    let arrondi = CornerRadius::same(4);
    v.panel_fill = PANNEAU;
    v.window_fill = PANNEAU;
    v.window_stroke = Stroke::new(1.0_f32, BORD);
    v.window_corner_radius = CornerRadius::same(6);
    v.menu_corner_radius = CornerRadius::same(5);
    v.window_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(120) };
    v.popup_shadow = Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(110) };
    v.extreme_bg_color = CHAMP;
    v.faint_bg_color = ELEVE;
    v.code_bg_color = ENTETE;
    v.hyperlink_color = ACCENT_TEXTE;
    v.warn_fg_color = Color32::from_rgb(232, 153, 34);
    v.error_fg_color = Color32::from_rgb(247, 92, 70);
    v.selection.bg_fill = ACCENT;
    // Blanc pur sur l'accent : 4,5:1 (le gris-blanc n'y fait que 4:1).
    v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    let w = &mut v.widgets;
    // Textes et cadres qui ne réagissent pas.
    w.noninteractive.bg_fill = PANNEAU;
    w.noninteractive.weak_bg_fill = PANNEAU;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORD);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXTE);
    w.noninteractive.corner_radius = arrondi;
    // Boutons, cases, curseurs : au repos, survolés, enfoncés, ouverts.
    for (etat, fond, texte) in [
        (&mut w.inactive, CONTROLE, TEXTE),
        (&mut w.hovered, CONTROLE_SURVOL, TEXTE_FORT),
        (&mut w.active, CONTROLE_ACTIF, TEXTE_FORT),
        (&mut w.open, CONTROLE_SURVOL, TEXTE_FORT),
    ] {
        etat.bg_fill = fond;
        etat.weak_bg_fill = fond;
        etat.bg_stroke = Stroke::NONE;
        etat.fg_stroke = Stroke::new(1.0_f32, texte);
        etat.corner_radius = arrondi;
        etat.expansion = 0.0;
    }
    w.hovered.bg_stroke = Stroke::new(1.0_f32, CONTROLE_ACTIF);
    v
}

/// Interface dense : espacements serrés, textes un peu plus petits qu'egui par défaut.
pub fn densifier(style: &mut egui::Style) {
    let s = &mut style.spacing;
    s.item_spacing = Vec2::new(6.0, 3.0);
    s.button_padding = Vec2::new(7.0, 2.0);
    s.interact_size = Vec2::new(32.0, 18.0);
    s.indent = 14.0;
    s.slider_width = 110.0;
    s.window_margin = Margin::same(6);
    s.menu_margin = Margin::same(4);
    s.scroll = egui::style::ScrollStyle::thin();
    // Bulles d'aide : seulement quand la souris reste une seconde sur un bouton (12d).
    style.interaction.tooltip_delay = crate::bulles::DELAI;
    let p = FontFamily::Proportional;
    style.text_styles = [
        (TextStyle::Small, FontId::new(10.5, p.clone())),
        (TextStyle::Body, FontId::new(12.5, p.clone())),
        (TextStyle::Button, FontId::new(12.5, p.clone())),
        (TextStyle::Heading, FontId::new(16.0, p)),
        (TextStyle::Monospace, FontId::new(11.5, FontFamily::Monospace)),
    ]
    .into();
}

/// Segoe UI (police de Windows) en tête de la famille proportionnelle, si elle est là : les
/// polices d'egui restent derrière (caractères manquants, icônes). Vrai si elle a été ajoutée.
pub fn polices(fonts: &mut FontDefinitions) -> bool {
    let dossier = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
    let Ok(octets) = std::fs::read(dossier.join("Fonts").join("segoeui.ttf")) else { return false };
    fonts.font_data.insert("segoe-ui".into(), std::sync::Arc::new(FontData::from_owned(octets)));
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "segoe-ui".into());
    true
}

/// Onglets et séparations des panneaux (egui_dock) : barres d'onglets sombres, onglet actif de la
/// couleur du panneau avec un filet bleu, séparations du fond qui bleuissent au survol.
pub fn dock(mut s: egui_dock::Style) -> egui_dock::Style {
    s.main_surface_border_stroke = Stroke::NONE;
    s.separator.width = 4.0;
    s.separator.color_idle = FOND;
    s.separator.color_hovered = ACCENT;
    s.separator.color_dragged = ACCENT;
    s.tab_bar.bg_fill = ENTETE;
    s.tab_bar.hline_color = ENTETE;
    s.tab_bar.height = 26.0;
    // Des onglets qui ne tiennent plus dans un panneau rétréci : une petite barre pour les
    // atteindre (Anthony, 2026-10-17 : « toujours utilisables même lorsqu'ils sont réduits »).
    s.tab_bar.show_scroll_bar_on_overflow = true;
    s.tab_bar.corner_radius = CornerRadius::ZERO;
    let onglet = |fond: Color32, texte: Color32| egui_dock::TabInteractionStyle {
        outline_color: Color32::TRANSPARENT,
        corner_radius: CornerRadius::ZERO,
        bg_fill: fond,
        text_color: texte,
    };
    s.tab.inactive = onglet(ENTETE, TEXTE_DOUX);
    s.tab.hovered = onglet(ENTETE, TEXTE);
    s.tab.active = onglet(PANNEAU, TEXTE);
    s.tab.focused = onglet(PANNEAU, TEXTE_FORT);
    s.tab.inactive_with_kb_focus = s.tab.inactive.clone();
    s.tab.active_with_kb_focus = s.tab.active.clone();
    s.tab.focused_with_kb_focus = s.tab.focused.clone();
    s.tab.hline_below_active_tab_name = false;
    s.tab.tab_body.bg_fill = PANNEAU;
    s.tab.tab_body.stroke = Stroke::NONE;
    s.tab.tab_body.corner_radius = CornerRadius::ZERO;
    s.buttons.close_tab_color = TEXTE_DOUX;
    s.buttons.close_tab_active_color = TEXTE_FORT;
    s.buttons.close_tab_bg_fill = CONTROLE_SURVOL;
    s.overlay.selection_color = ACCENT.gamma_multiply(0.5);
    s
}

/// Applique le thème : couleurs, densité.
pub fn appliquer(ctx: &egui::Context) {
    ctx.set_visuals(visuels());
    ctx.style_mut(densifier);
}
