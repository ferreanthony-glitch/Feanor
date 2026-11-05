//! La marque FEANOR dans l'interface : les polices de marque, le mot, la devise, le « lockup » (le Joyau à la place du O)
//! et l'animation d'ouverture de l'accueil.
//!
//! **Les polices** (SIL OFL 1.1, `assets/polices/`, licences à côté, fichiers non modifiés) : Big Shoulders Display
//! ExtraBold pour le mot, IBM Plex Sans Medium pour la devise. Elles sont embarquées dans l'exécutable. Pour changer de
//! police de marque, on ne touche qu'aux trois constantes `MOT_*` / `DEVISE_*` de ce fichier.
//!
//! **Le lockup** (livre de marque) : le mot « FEANOR » en capitales, interlettrage 0,02 em. Le lockup principal met le Joyau jade
//! devant le mot (devise alignée à droite sur le R) ; le moment fort (l'accueil) met le Joyau irisé à la place du O (devise
//! à partir du F). Dans les deux cas la taille du Joyau
//! taille vient de la hauteur réelle des capitales de la police (lue dans la table `OS/2`) : la pierre fait exactement la
//! hauteur des capitales, soit un Joyau de `cap × 64 / 39`, enfoncé sous la ligne de base de `taille × 12 / 64`, et il
//! mord de `taille × 7 / 64` sur chaque lettre voisine. Sous 24 pixels de capitales, le Joyau est seul ; la devise
//! « FORGE YOUR STORY » (capitales espacées de 0,18 em) disparaît sous 24 pixels de hauteur de logo.
//!
//! **L'animation d'ouverture** (`logo::etat_animation`) n'est jouée qu'une fois par lancement, sur l'accueil, sans
//! bloquer l'interface ; l'écran n'est repeint que pendant qu'elle court, puis le Joyau reste figé.

use std::sync::{Arc, OnceLock};

use eframe::egui::{self, pos2, text::LayoutJob, vec2, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, TextFormat, TextureHandle};

use crate::logo::{self, EtatAnim, Joyau};

// ------------------------------------------------------------------------------------------- polices

/// La police du mot FEANOR (capitales).
pub const MOT_OCTETS: &[u8] = include_bytes!("../assets/polices/BigShouldersDisplay-ExtraBold.ttf");
/// La police de la devise (poids 500).
pub const DEVISE_OCTETS: &[u8] = include_bytes!("../assets/polices/IBMPlexSans-Medium.ttf");
/// Le nom des deux familles dans egui.
pub const MOT_FAMILLE: &str = "feanor-mot";
pub const DEVISE_FAMILLE: &str = "feanor-devise";

/// Hauteur d'un logo (Joyau) en dessous de laquelle la devise disparaît, en points.
pub const DEVISE_HAUTEUR_MIN: f32 = 24.0;
/// Hauteur de capitales en dessous de laquelle le Joyau est seul, en points.
pub const MOT_CAPITALES_MIN: f32 = 24.0;
/// Interlettrage du mot (em) et de la devise (em).
pub const MOT_INTERLETTRAGE: f32 = 0.02;
pub const DEVISE_INTERLETTRAGE: f32 = 0.18;
/// La devise.
pub const DEVISE: &str = "FORGE YOUR STORY";

/// Ajoute les polices de marque à egui (familles nommées ; la famille proportionnelle n'est pas touchée).
pub fn polices(fonts: &mut FontDefinitions) {
    fonts.font_data.insert(MOT_FAMILLE.into(), Arc::new(FontData::from_static(MOT_OCTETS)));
    fonts.families.insert(FontFamily::Name(MOT_FAMILLE.into()), vec![MOT_FAMILLE.into()]);
    fonts.font_data.insert(DEVISE_FAMILLE.into(), Arc::new(FontData::from_static(DEVISE_OCTETS)));
    fonts.families.insert(FontFamily::Name(DEVISE_FAMILLE.into()), vec![DEVISE_FAMILLE.into()]);
}

// ------------------------------------------------------------------------------------------- police de l'interface

/// La police du texte de l'interface : IBM Plex Sans (Regular pour le texte, Medium pour les boutons et les étiquettes,
/// SemiBold pour les titres) et IBM Plex Mono pour les timecodes et les numéros d'image (SIL OFL 1.1, `assets/polices/`).
/// Les polices d'avant (Segoe UI s'il est là, celles d'egui) et les icônes restent derrière : aucun caractère ne manque.
pub const PLEX_REGULAR: &[u8] = include_bytes!("../assets/polices/IBMPlexSans-Regular.ttf");
pub const PLEX_MEDIUM: &[u8] = include_bytes!("../assets/polices/IBMPlexSans-Medium.ttf");
pub const PLEX_SEMIBOLD: &[u8] = include_bytes!("../assets/polices/IBMPlexSans-SemiBold.ttf");
pub const PLEX_MONO: &[u8] = include_bytes!("../assets/polices/IBMPlexMono-Regular.ttf");
/// Les familles nommées des poids intermédiaires (egui n'a pas de graisse : une famille par graisse).
pub const FAMILLE_MEDIUM: &str = "plex-medium";
pub const FAMILLE_SEMIBOLD: &str = "plex-semibold";

/// Note, dans la mémoire du contexte, que les polices Plex y sont chargées (`set_fonts` fait) : `theme::appliquer` peut alors
/// donner leurs familles aux boutons et aux titres.
pub fn marquer_polices(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("feanor_polices_plex"), true));
}

/// Les polices Plex sont-elles chargées dans ce contexte ?
pub fn polices_pretes(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new("feanor_polices_plex"))).unwrap_or(false)
}

/// Met IBM Plex en tête des familles de l'interface. À appeler APRÈS l'ajout des icônes, pour que les familles nommées
/// reprennent aussi les polices de secours et les icônes.
pub fn polices_interface(fonts: &mut FontDefinitions) {
    for (nom, octets) in [("plex-regular", PLEX_REGULAR), (FAMILLE_MEDIUM, PLEX_MEDIUM), (FAMILLE_SEMIBOLD, PLEX_SEMIBOLD), ("plex-mono", PLEX_MONO)] {
        fonts.font_data.insert(nom.into(), Arc::new(FontData::from_static(octets)));
    }
    let derriere = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    for nom in [FAMILLE_MEDIUM, FAMILLE_SEMIBOLD] {
        let mut v = vec![nom.to_string()];
        v.extend(derriere.iter().cloned());
        fonts.families.insert(FontFamily::Name(nom.into()), v);
    }
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "plex-regular".into());
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "plex-mono".into());
}
/// Mesures d'une police (en unités de la grille de la police).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metriques {
    pub unites_par_em: f32,
    pub hauteur_capitales: f32,
    pub ascendante: f32,
}

fn u16_be(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

/// Lit `head`, `hhea` et `OS/2` d'un fichier TrueType.
pub fn metriques(octets: &[u8]) -> Option<Metriques> {
    let n = u16_be(octets, 4)? as usize;
    let table = |nom: &[u8; 4]| -> Option<usize> {
        (0..n).find_map(|k| {
            let e = 12 + 16 * k;
            (octets.get(e..e + 4)? == nom).then(|| u32::from_be_bytes(octets[e + 8..e + 12].try_into().unwrap()) as usize)
        })
    };
    let head = table(b"head")?;
    let hhea = table(b"hhea")?;
    let os2 = table(b"OS/2")?;
    let unites = u16_be(octets, head + 18)? as f32;
    let ascendante = u16_be(octets, hhea + 4)? as i16 as f32;
    let version = u16_be(octets, os2)?;
    let capitales = if version >= 2 { u16_be(octets, os2 + 88)? as i16 as f32 } else { 0.7 * unites };
    Some(Metriques { unites_par_em: unites, hauteur_capitales: capitales, ascendante })
}

fn metriques_mot() -> Metriques {
    static M: OnceLock<Metriques> = OnceLock::new();
    *M.get_or_init(|| metriques(MOT_OCTETS).unwrap_or(Metriques { unites_par_em: 1000.0, hauteur_capitales: 700.0, ascendante: 1000.0 }))
}

/// La taille de police (points) dont les capitales font `capitales` points de haut.
pub fn taille_pour_capitales(m: &Metriques, capitales: f32) -> f32 {
    capitales * m.unites_par_em / m.hauteur_capitales
}

/// Côté du Joyau dont la pierre a la hauteur des capitales (la pierre fait 39 des 64 unités).
pub fn cote_joyau(capitales: f32) -> f32 {
    capitales * 64.0 / 39.0
}

// ------------------------------------------------------------------------------------------- le mot

fn galley(ctx: &egui::Context, texte: &str, famille: &str, taille: f32, interlettrage_em: f32, couleur: Color32) -> Arc<egui::Galley> {
    let mut job = LayoutJob::default();
    job.append(
        texte,
        0.0,
        TextFormat { font_id: FontId::new(taille, FontFamily::Name(famille.into())), color: couleur, extra_letter_spacing: interlettrage_em * taille, ..Default::default() },
    );
    ctx.fonts(|f| f.layout_job(job))
}

/// **Le mot, en un seul endroit** : « FEANOR » (ou un morceau) dans la police de marque, capitales de `capitales` points
/// de haut. Pour changer de police, voir `MOT_OCTETS`.
pub fn mot(ctx: &egui::Context, texte: &str, capitales: f32, couleur: Color32) -> Arc<egui::Galley> {
    galley(ctx, texte, MOT_FAMILLE, taille_pour_capitales(&metriques_mot(), capitales), MOT_INTERLETTRAGE, couleur)
}

/// Comment le Joyau s'associe au mot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mise {
    /// Le lockup principal : le Joyau jade DEVANT le mot FEANOR en entier ; la devise est alignée à droite sur le R.
    Devant,
    /// Le moment fort (l'accueil) : le Joyau irisé REMPLACE le O ; la devise commence sous le F.
    RemplaceLeO,
}

/// Le dessin du lockup : où le peindre et ce qu'il occupe.
#[derive(Clone, Copy, Debug)]
pub struct Lockup {
    /// Rectangle englobant (mot, Joyau et devise).
    pub rect: Rect,
    /// Rectangle du Joyau.
    pub joyau: Rect,
    /// Ligne de base du mot.
    pub base: f32,
    /// Abscisses du début et de la fin du mot (le Joyau remplaçant le O compris).
    pub mot_gauche: f32,
    pub mot_droite: f32,
}

/// Place le lockup dont l'angle haut gauche des capitales est `haut_gauche` (ligne de base = `haut_gauche.y + capitales`).
pub fn disposer(ctx: &egui::Context, haut_gauche: Pos2, capitales: f32, avec_devise: bool, mise: Mise) -> Lockup {
    let base = haut_gauche.y + capitales;
    let cote = cote_joyau(capitales);
    let enfonce = cote * 12.0 / 64.0;
    let morsure = cote * 7.0 / 64.0;
    let haut_joyau = base + enfonce - cote;
    if capitales < MOT_CAPITALES_MIN {
        let joyau = Rect::from_min_size(pos2(haut_gauche.x, haut_joyau), vec2(cote, cote));
        return Lockup { rect: joyau, joyau, base, mot_gauche: joyau.left(), mot_droite: joyau.right() };
    }
    let (joyau, mot_gauche, mot_droite) = match mise {
        Mise::RemplaceLeO => {
            let gauche = mot(ctx, "FEAN", capitales, Color32::WHITE);
            let droite = mot(ctx, "R", capitales, Color32::WHITE);
            let x_joyau = haut_gauche.x + gauche.size().x - morsure;
            let x_r = x_joyau + cote - morsure;
            (Rect::from_min_size(pos2(x_joyau, haut_joyau), vec2(cote, cote)), haut_gauche.x, x_r + droite.size().x)
        }
        Mise::Devant => {
            let entier = mot(ctx, "FEANOR", capitales, Color32::WHITE);
            // (Le mot commence juste après la pointe droite de l'étoile : 60 sur 64.)
            let x_mot = haut_gauche.x + cote * 61.0 / 64.0;
            (Rect::from_min_size(pos2(haut_gauche.x, haut_joyau), vec2(cote, cote)), x_mot, x_mot + entier.size().x)
        }
    };
    let gauche = if mise == Mise::Devant { joyau.left() } else { haut_gauche.x };
    let mut rect = Rect::from_min_max(pos2(gauche, joyau.top().min(haut_gauche.y)), pos2(mot_droite, joyau.bottom().max(base)));
    if avec_devise && cote >= DEVISE_HAUTEUR_MIN {
        rect.max.y += capitales * 0.28 + capitales * 0.34;
    }
    Lockup { rect, joyau, base, mot_gauche, mot_droite }
}

/// La hauteur de capitales (au plus `capitales`) pour que le lockup tienne dans `largeur` points : il rétrécit en proportion, puis
/// devient le Joyau seul (sous 24 points de capitales), qui rétrécit encore s'il le faut. Jamais coupé dans une fenêtre étroite.
pub fn ajuster(ctx: &egui::Context, largeur: f32, capitales: f32, mise: Mise) -> f32 {
    let w = disposer(ctx, pos2(0.0, 0.0), capitales, false, mise).rect.width();
    let mut c = if w > largeur && w > 0.0 { capitales * largeur.max(1.0) / w } else { capitales };
    if c < MOT_CAPITALES_MIN {
        // (Le Joyau seul : sa largeur est `cote_joyau(c)`.)
        c = c.min(largeur.max(1.0) * 39.0 / 64.0);
    }
    // (Les tailles de police sont arrondies : on resserre jusqu'à ce que ça tienne.)
    for _ in 0..12 {
        let l = disposer(ctx, pos2(0.0, 0.0), c, false, mise);
        if l.rect.width() <= largeur || c <= 1.0 {
            break;
        }
        c *= 0.97;
    }
    c.max(1.0)
}

/// Peint le lockup : le mot en `encre`, le Joyau (`texture`) devant le mot ou à la place du O (`mise`), la devise en `devise`
/// si elle est donnée et si le logo est assez grand. Rend la place prise.
#[allow(clippy::too_many_arguments)]
pub fn peindre(painter: &egui::Painter, ctx: &egui::Context, texture: &TextureHandle, haut_gauche: Pos2, capitales: f32, encre: Color32, devise: Option<Color32>, mise: Mise) -> Lockup {
    let l = disposer(ctx, haut_gauche, capitales, devise.is_some(), mise);
    let metriques = metriques_mot();
    let taille = taille_pour_capitales(&metriques, capitales);
    // (La ligne de base d'une ligne de texte egui est à `ascendante` sous le haut de la ligne.)
    let haut_ligne = l.base - metriques.ascendante / metriques.unites_par_em * taille;
    let plein_uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    if capitales >= MOT_CAPITALES_MIN {
        match mise {
            Mise::RemplaceLeO => {
                let gauche = mot(ctx, "FEAN", capitales, encre);
                let droite = mot(ctx, "R", capitales, encre);
                let cote = l.joyau.width();
                let x_r = l.joyau.left() + cote - cote * 7.0 / 64.0;
                painter.galley(pos2(haut_gauche.x, haut_ligne), gauche, encre);
                painter.galley(pos2(x_r, haut_ligne), droite, encre);
            }
            Mise::Devant => painter.galley(pos2(l.mot_gauche, haut_ligne), mot(ctx, "FEANOR", capitales, encre), encre),
        }
    }
    painter.image(texture.id(), l.joyau, plein_uv, Color32::WHITE);
    if let Some(couleur) = devise {
        if l.joyau.width() >= DEVISE_HAUTEUR_MIN && capitales >= MOT_CAPITALES_MIN {
            let corps = (capitales * 0.34).max(7.0);
            let t = galley(ctx, DEVISE, DEVISE_FAMILLE, corps, DEVISE_INTERLETTRAGE, couleur);
            let y = l.base + l.joyau.height() * 12.0 / 64.0 + capitales * 0.12;
            let x = match mise {
                Mise::RemplaceLeO => haut_gauche.x,
                // Alignée à droite sur le R (l'espace laissé après la dernière lettre par l'interlettrage est rendu).
                Mise::Devant => l.mot_droite - (t.size().x - DEVISE_INTERLETTRAGE * corps),
            };
            painter.galley(pos2(x, y), t, couleur);
        }
    }
    l
}


// ------------------------------------------------------------------------------------------- textures du Joyau

/// La texture du Joyau, carrée de `cote` points (à la résolution de l'écran). Une seule texture par clé (et variante), gardée
/// dans la mémoire du contexte et **remplacée** quand la taille ou l'état d'animation change : la précédente est libérée avec
/// son dernier handle.
pub fn texture_joyau(ctx: &egui::Context, cle: &str, cote: f32, v: Joyau, anim: Option<EtatAnim>) -> TextureHandle {
    let pixels = (cote * ctx.pixels_per_point()).round().max(8.0) as u32;
    let id = egui::Id::new(("feanor_joyau", cle, format!("{v:?}")));
    if let Some((px, etat, texture)) = ctx.data(|d| d.get_temp::<(u32, Option<EtatAnim>, TextureHandle)>(id)) {
        if px == pixels && etat == anim {
            return texture;
        }
    }
    let rgba = logo::image(pixels, v, anim.as_ref());
    let image = egui::ColorImage::from_rgba_premultiplied([pixels as usize; 2], &rgba);
    let texture = ctx.load_texture(format!("feanor-joyau-{cle}"), image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, (pixels, anim, texture.clone())));
    texture
}
// ------------------------------------------------------------------------------------------- animation d'ouverture

/// L'état de l'animation dans la mémoire (temporaire) du contexte egui : un contexte par lancement, donc « une fois par
/// lancement ». `Some(debut)` : démarrée à cet instant ; `None` : pas encore ; `terminee` : jouée ou sautée.
#[derive(Clone, Copy, Default)]
struct SuiviAnimation {
    debut: Option<f64>,
    terminee: bool,
}

/// L'état de l'animation d'ouverture de l'accueil à cet instant. Elle est jouée une fois par lancement : le premier appel
/// la démarre, elle dure `logo::DUREE_ANIMATION`, puis l'état de repos (figé) est rendu à chaque appel suivant, sans
/// jamais redemander d'image. Pendant qu'elle court, une image est demandée à chaque fois (sans rien bloquer).
/// Sautée (repos tout de suite) quand egui n'anime plus rien (`animation_time` à 0 : réglage de mouvement réduit).
pub fn animation_accueil(ctx: &egui::Context) -> EtatAnim {
    let cle = egui::Id::new("feanor_animation_accueil");
    let mut suivi: SuiviAnimation = ctx.data(|d| d.get_temp(cle)).unwrap_or_default();
    let etat = if suivi.terminee || ctx.style().animation_time <= 0.0 {
        suivi.terminee = true;
        logo::etat_repos()
    } else {
        let maintenant = ctx.input(|i| i.time);
        let debut = *suivi.debut.get_or_insert(maintenant);
        let t = (maintenant - debut) as f32;
        if t >= logo::DUREE_ANIMATION {
            suivi.terminee = true;
            logo::etat_repos()
        } else {
            ctx.request_repaint();
            logo::etat_animation(t)
        }
    };
    ctx.data_mut(|d| d.insert_temp(cle, suivi));
    etat
}