//! Raccourcis clavier, lus dans `keymap.toml` et modifiables sans recompiler.
//!
//! Chaque action du logiciel a un identifiant stable (`ACTIONS`). Le fichier associe des touches
//! à ces identifiants. Le fichier par défaut est intégré au programme ; un fichier utilisateur
//! (à côté de l'exécutable, ou dans %APPDATA%\Pelure) remplace action par action ce qu'il redéfinit.

use std::path::PathBuf;

use eframe::egui::{Key, Modifiers};
use serde::Deserialize;

use crate::app::Mode;

/// Fichier de raccourcis par défaut, intégré au programme.
pub const DEFAULT_KEYMAP: &str = include_str!("../keymap.toml");

pub struct ActionInfo {
    pub id: &'static str,
    pub label: &'static str,
    /// La fonction existe dans le logiciel.
    pub implemented: bool,
    /// Le raccourci se répète tant que la touche reste enfoncée.
    pub repeat: bool,
}

const fn act(id: &'static str, label: &'static str, implemented: bool, repeat: bool) -> ActionInfo {
    ActionInfo {
        id,
        label,
        implemented,
        repeat,
    }
}

/// Toutes les actions connues. Les identifiants ne changent jamais : on en ajoute, on n'en renomme pas.
pub const ACTIONS: &[ActionInfo] = &[
    act("fichier.nouveau", "Nouveau projet", true, false),
    act("fichier.ouvrir", "Ouvrir", true, false),
    act("fichier.enregistrer", "Enregistrer", true, false),
    act("fichier.enregistrer_sous", "Enregistrer sous", true, false),
    act("fichier.fermer", "Fermer le projet", false, false),
    act("fichier.quitter", "Quitter", true, false),
    act("fichier.exporter_film", "Exporter le film (vidéo)", true, false),
    act("fichier.exporter_images", "Exporter des images (PNG)", true, false),
    act("fichier.exporter_son", "Exporter le son (WAV)", true, false),
    act("fichier.exporter_pdf", "Exporter le storyboard (PDF)", true, false),
    act("fichier.exporter_image_courante", "Exporter l'image courante (PNG)", true, false),
    act("edition.annuler", "Annuler", true, true),
    act("edition.retablir", "Rétablir", true, true),
    act("edition.couper", "Couper", true, false),
    act("edition.copier", "Copier", true, false),
    act("edition.coller", "Coller", true, false),
    act("edition.coller_sur_place", "Coller sur place", false, false),
    act("edition.supprimer", "Supprimer (la sélection, sinon effacer le dessin)", true, false),
    act("edition.supprimer_et_resserrer", "Supprimer et resserrer", false, false),
    act("edition.tout_selectionner", "Tout sélectionner", true, false),
    act("edition.selectionner_dessins_scene", "Sélectionner tous les dessins de la scène", false, false),
    act("edition.tout_deselectionner", "Tout désélectionner", true, false),
    act("vue.outil_zoom", "Outil zoom", false, false),
    act("vue.zoom_avant", "Zoom avant", true, true),
    act("vue.zoom_arriere", "Zoom arrière", true, true),
    act("vue.plein_ecran", "Plein écran", true, false),
    act("vue.rotation_horaire", "Tourner la vue (sens horaire)", true, true),
    act("vue.rotation_antihoraire", "Tourner la vue (sens antihoraire)", true, true),
    act("vue.reinit_rotation", "Remettre la vue droite", true, false),
    act("vue.reinit_zoom", "Zoom 100 %", true, false),
    act("vue.ajuster", "Ajuster à la fenêtre", true, false),
    act("vue.pan_libre", "Déplacer la vue (maintenir)", true, false),
    act("vue.rotation_temporaire", "Rotation temporaire (maintenir)", false, false),
    act("vue.zoom_temporaire_avant", "Zoom avant temporaire (maintenir)", false, false),
    act("vue.zoom_temporaire_arriere", "Zoom arrière temporaire (maintenir)", false, false),
    act("vue.grille", "Afficher la grille", false, false),
    act("vue.pelure", "Pelure d'oignon", true, false),
    act("vue.miroir", "Miroir de la vue", true, false),
    act("vue.planches", "Storyboard : vue dessin / vue vignettes", true, false),
    act("mode.basculer", "Passer du storyboard à l'animation, et retour", true, false),
    act("aide.raccourcis", "Liste des raccourcis", true, false),
    act("outil.pinceau", "Outil pinceau", true, false),
    act("outil.crayon", "Outil crayon", true, false),
    act("outil.gomme", "Outil gomme", true, false),
    act("outil.remplissage", "Outil remplissage", true, false),
    act("outil.peindre_non_peint", "Outil peindre les zones non peintes", false, false),
    act("outil.pipette", "Outil pipette", true, false),
    act("outil.selection", "Outil sélection", true, false),
    act("outil.cutter", "Outil cutter", false, false),
    act("outil.pochoir", "Outil pinceau pochoir", false, false),
    act("outil.formes", "Outil formes (dernière forme utilisée)", true, false),
    act("outil.ligne", "Outil ligne", true, false),
    act("outil.ligne_droite", "Outil ligne, droite", true, false),
    act("outil.ligne_courbe", "Outil ligne, courbe", true, false),
    act("outil.ligne_s", "Outil ligne, courbe en S", true, false),
    act("outil.rectangle", "Outil rectangle", true, false),
    act("outil.ellipse", "Outil ellipse", true, false),
    act("outil.polyligne", "Outil polyligne", true, false),
    act("outil.texte", "Outil texte", true, false),
    act("outil.tampon", "Outil tampon", true, false),
    act("outil.tampon_duplication", "Outil tampon de duplication", true, false),
    act("outil.perspective", "Outil perspective (déformer par les quatre coins)", true, false),
    act("outil.main", "Outil main", true, false),
    act("outil.epaisseur_crayon", "Outil épaisseur du trait de crayon", false, false),
    act("outil.taille_plus", "Pinceau plus gros", true, true),
    act("outil.taille_moins", "Pinceau plus fin", true, true),
    act("selection.miroir_h", "Retourner la sélection horizontalement", true, false),
    act("selection.miroir_v", "Retourner la sélection verticalement", true, false),
    act("dessin.supprimer_hors_selection", "Effacer ce qui est hors de la sélection", true, false),
    act("dessin.aplatir", "Aplatir le dessin", false, false),
    act("dessin.aimanter_contour", "Aimanter au contour", false, false),
    act("dessin.remplir_zones_auto", "Remplir automatiquement les zones fermées", false, false),
    act("lecture.basculer", "Lecture / pause", true, false),
    act("image.precedente", "Image précédente", true, true),
    act("image.suivante", "Image suivante", true, true),
    act("image.premiere", "Première image du panneau", true, false),
    act("image.derniere", "Dernière image du panneau", true, false),
    act("montage.debut", "Début du montage", true, false),
    act("montage.fin", "Fin du montage", true, false),
    act("panneau.precedent", "Panneau précédent", true, true),
    act("panneau.suivant", "Panneau suivant", true, true),
    act("panneau.nouveau", "Nouveau panneau", true, false),
    act("panneau.dupliquer", "Dupliquer le panneau", true, false),
    act("scene.nouvelle", "Nouvelle scène", true, false),
    act("scene.dupliquer", "Dupliquer la scène", true, false),
    act("scene.supprimer", "Supprimer la scène", true, false),
    act("scene.precedente", "Scène précédente", true, true),
    act("scene.suivante", "Scène suivante", true, true),
    act("sequence.nouvelle", "Nouvelle séquence", true, false),
    act("panneau.supprimer", "Supprimer les panneaux choisis", true, false),
    act("panneau.copier", "Copier les panneaux choisis", true, false),
    act("panneau.coller", "Coller les panneaux après le panneau courant", true, false),
    act("panneau.ajouter_image", "Allonger le panneau d'une image", true, true),
    act("panneau.retirer_image", "Raccourcir le panneau d'une image", true, true),
    act("panneau.proprietes", "Propriétés du panneau", false, false),
    act("panneau.couper", "Couper le panneau à l'image courante", true, false),
    act("panneau.fusionner_suivant", "Réunir le panneau et le suivant", true, false),
    act("scene.couper", "Couper la scène au panneau courant", true, false),
    act("scene.joindre_precedente", "Joindre la scène à la précédente", true, false),
    act("scene.aller_a", "Aller à une scène", false, false),
    act("dessin.nouveau", "Nouveau dessin après l'exposition", true, false),
    act("dessin.dupliquer", "Dupliquer le dessin", true, false),
    act("dessin.supprimer", "Retirer le dessin de la timeline", true, false),
    act("exposition.allonger", "Tenir le dessin une image de plus", true, true),
    act("exposition.raccourcir", "Tenir le dessin une image de moins", true, true),
    act("calque.nouveau_vectoriel", "Nouveau calque vectoriel", true, false),
    act("calque.nouveau_bitmap", "Nouveau calque bitmap", true, false),
    act("calque.copier", "Copier le calque", true, false),
    act("calque.coller", "Coller le calque", true, false),
    act("calque.supprimer", "Supprimer le calque", true, false),
    act("calque.flou", "Flou sur le calque bitmap", false, false),
    act("calque.verrouiller", "Verrouiller le calque", true, false),
    act("calque.fusionner_dessous", "Fusionner avec le calque du dessous", true, false),
    act("calque.reinit_transformation", "Annuler la transformation du calque", true, false),
    act("calque.dessus", "Calque du dessus", true, true),
    act("calque.dessous", "Calque du dessous", true, true),
    act("outil.camera", "Outil caméra", true, false),
    act("outil.transformer", "Outil transformer le calque", true, false),
    act("camera.vue", "Vue à travers la caméra", true, false),
    act("camera.masque", "Masque hors-champ de la caméra", true, false),
    act("camera.reinitialiser", "Réinitialiser la caméra", true, false),
    act("camera.cle_debut", "Image clé de caméra au début du panneau", true, false),
    act("camera.cle_ici", "Image clé de caméra à l'image courante", true, false),
    act("camera.cle_fin", "Image clé de caméra à la fin du panneau", true, false),
    act("camera.cle_supprimer", "Retirer l'image clé de caméra de l'image courante", true, false),
    act("camera.cle_adoucir", "Adoucir le mouvement à cette image clé", true, false),
    act("vue.table_lumineuse", "Table lumineuse (autres calques estompés)", true, false),
    act("vue.dessus", "Vue de dessus (profondeur des calques)", true, false),
    act("vue.cadres_securite", "Zones de sécurité (action, titres)", true, false),
    act("vue.3d", "Plateau en 3D", true, false),
    act("vue3d.recadrer", "Plateau en 3D : recadrer sur tout le plateau", true, false),
    act("outil3d.orbite", "Plateau en 3D : tourner autour", true, false),
    act("outil3d.glisser", "Plateau en 3D : glisser la vue", true, false),
    act("outil3d.avancer", "Plateau en 3D : avancer / reculer", true, false),
    act("outil3d.deplacer", "Plateau en 3D : déplacer un calque ou la caméra", true, false),
    act("outil.repere3d", "Outil repère 3D (tourner autour du modèle)", true, false),
    act("fichier.importer_modele3d", "Importer un modèle 3D (glTF) comme repère", true, false),
    act("calque.nouveau_fond", "Nouveau calque de fond pour toute la scène", true, false),
    act("calque.toute_la_scene", "Calque de toute la scène (oui / non)", true, false),
    act("espace.standard", "Espace de travail : Standard", true, false),
    act("espace.dessin", "Espace de travail : Dessin", true, false),
    act("espace.timeline", "Espace de travail : Timeline", true, false),
    act("espace.ensemble", "Espace de travail : Vue d'ensemble", true, false),
    act("espace.enregistrer", "Enregistrer la disposition comme espace de travail", true, false),
    act("vue.panneau_outils", "Afficher / masquer le panneau Outils", true, false),
    act("vue.panneau_fiche", "Afficher / masquer la fiche et les calques", true, false),
    act("vue.panneau_bande", "Afficher / masquer le montage ou la timeline", true, false),
    act("guide.activer", "Guide de perspective : afficher et utiliser", true, false),
    act("guide.aligner", "Guide : aligner les traits", true, false),
    act("guide.verrouiller", "Guide : verrouiller", true, false),
    act("guide.lignes_reference", "Guide : lignes de référence", true, false),
    act("guide.gomme", "Guide : aligner aussi la gomme", true, false),
    act("guide.curseur_complet", "Guide : lignes du curseur jusqu'au bord", true, false),
    act("son.importer", "Importer un son", true, false),
    act("son.nouvelle_piste", "Nouvelle piste son", true, false),
    act("son.couper", "Couper le son à l'image courante", true, false),
    act("son.defilement", "Son au défilement (activer / couper)", true, false),
    act("son.lecture", "Son pendant la lecture (activer / couper)", true, false),
    act("clips.lier_au_panneau", "Lier les clips au panneau", false, false),
    act("reference.decharger", "Décharger la référence courante", false, false),
    act("legende.chercher", "Chercher dans les légendes", false, false),
];

pub fn action_info(id: &str) -> Option<&'static ActionInfo> {
    ACTIONS.iter().find(|a| a.id == id)
}

/// Comment reconnaître la touche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyMatch {
    /// Le caractère tapé : la lettre écrite sur la touche, quelle que soit la disposition du clavier.
    Char(Key),
    /// La position de la touche, nommée d'après un clavier QWERTY.
    Pos(Key),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Combo {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: KeyMatch,
    /// Maj indifférente (pour « + », que l'on tape avec ou sans Maj selon le clavier).
    pub any_shift: bool,
}

impl Combo {
    pub fn matches(&self, key: Key, physical: Option<Key>, m: Modifiers) -> bool {
        if m.command != self.ctrl || m.alt != self.alt || (!self.any_shift && m.shift != self.shift) {
            return false;
        }
        match self.key {
            KeyMatch::Char(k) => key == k,
            KeyMatch::Pos(k) => physical == Some(k),
        }
    }

    pub fn label(&self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s += "Ctrl+";
        }
        if self.shift && !self.any_shift {
            s += "Maj+";
        }
        if self.alt {
            s += "Alt+";
        }
        s += &key_label(self.key, self.any_shift);
        s
    }
}

fn key_label(k: KeyMatch, plus: bool) -> String {
    if plus {
        return "+".into();
    }
    match k {
        KeyMatch::Char(key) => match key {
            Key::Enter => "Entrée".into(),
            Key::Space => "Espace".into(),
            Key::Delete => "Suppr".into(),
            Key::Backspace => "Retour arrière".into(),
            Key::Escape => "Échap".into(),
            Key::Insert => "Inser".into(),
            Key::Home => "Origine".into(),
            Key::End => "Fin".into(),
            Key::PageUp => "Pg préc.".into(),
            Key::PageDown => "Pg suiv.".into(),
            Key::ArrowLeft => "Flèche gauche".into(),
            Key::ArrowRight => "Flèche droite".into(),
            Key::ArrowUp => "Flèche haut".into(),
            Key::ArrowDown => "Flèche bas".into(),
            other => other.name().to_string(),
        },
        // Touche repérée par sa position : on montre ce qui est écrit dessus en AZERTY.
        KeyMatch::Pos(key) => match (azerty_glyph(key), qwerty_glyph(key)) {
            (Some(a), Some(q)) if a != q => format!("{a} (QWERTY {q})"),
            (Some(a), _) => a.to_string(),
            _ => key.name().to_string(),
        },
    }
}

/// Ce qui est écrit sur la touche à cette position, sur un clavier AZERTY français.
pub fn azerty_glyph(k: Key) -> Option<&'static str> {
    Some(match k {
        Key::Comma => ";",
        Key::Period => ":",
        Key::Semicolon => "M",
        Key::Quote => "ù",
        Key::OpenBracket => "^",
        Key::CloseBracket => "$",
        Key::Minus => ")",
        Key::Equals => "=",
        Key::Slash => "!",
        Key::Backslash => "*",
        Key::Backtick => "²",
        Key::Plus => "+",
        _ => return digit(k),
    })
}

pub fn qwerty_glyph(k: Key) -> Option<&'static str> {
    Some(match k {
        Key::Comma => ",",
        Key::Period => ".",
        Key::Semicolon => ";",
        Key::Quote => "'",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Minus => "-",
        Key::Equals => "=",
        Key::Slash => "/",
        Key::Backslash => "\\",
        Key::Backtick => "`",
        Key::Plus => "+",
        _ => return digit(k),
    })
}

fn digit(k: Key) -> Option<&'static str> {
    Some(match k {
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num7 => "7",
        Key::Num8 => "8",
        Key::Num9 => "9",
        _ => return None,
    })
}

/// Lit une touche seule (« A », « F5 », « Entrée », « [ », « pos:, »…).
/// Renvoie plusieurs façons de la reconnaître quand il y en a plusieurs (« + »).
fn parse_key(name: &str) -> Option<(Vec<KeyMatch>, bool)> {
    let (force, name) = if let Some(rest) = strip_prefix_ci(name, "pos:") {
        (Some(true), rest)
    } else if let Some(rest) = strip_prefix_ci(name, "car:") {
        (Some(false), rest)
    } else {
        (None, name)
    };
    let lower = name.trim().to_lowercase();
    let named = match lower.as_str() {
        "entrée" | "entree" | "return" | "enter" => Some(Key::Enter),
        "espace" | "space" => Some(Key::Space),
        "suppr" | "delete" | "del" | "supprimer" => Some(Key::Delete),
        "retour arrière" | "retour arriere" | "backspace" => Some(Key::Backspace),
        "échap" | "echap" | "esc" | "escape" => Some(Key::Escape),
        "tab" => Some(Key::Tab),
        "inser" | "insert" | "ins" => Some(Key::Insert),
        "origine" | "home" => Some(Key::Home),
        "fin" | "end" => Some(Key::End),
        "pgpréc" | "pgprec" | "pg préc." | "pageup" => Some(Key::PageUp),
        "pgsuiv" | "pg suiv." | "pagedown" => Some(Key::PageDown),
        "gauche" | "flèche gauche" | "left" => Some(Key::ArrowLeft),
        "droite" | "flèche droite" | "right" => Some(Key::ArrowRight),
        "haut" | "flèche haut" | "up" => Some(Key::ArrowUp),
        "bas" | "flèche bas" | "down" => Some(Key::ArrowDown),
        _ => None,
    };
    let make = |k: Key, default_pos: bool| {
        if force.unwrap_or(default_pos) {
            KeyMatch::Pos(k)
        } else {
            KeyMatch::Char(k)
        }
    };
    if let Some(k) = named {
        return Some((vec![make(k, false)], false));
    }
    if lower.len() >= 2 && lower.starts_with('f') && lower[1..].chars().all(|c| c.is_ascii_digit()) {
        return Key::from_name(&name.trim().to_uppercase()).map(|k| (vec![make(k, false)], false));
    }
    let mut chars = name.trim().chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    if c.is_ascii_alphabetic() {
        let k = Key::from_name(&c.to_ascii_uppercase().to_string())?;
        return Some((vec![make(k, false)], false));
    }
    if c == '+' {
        // La touche « =/+ » du clavier principal ou le + du pavé numérique, Maj indifférente.
        return Some((vec![KeyMatch::Pos(Key::Equals), KeyMatch::Pos(Key::Plus)], true));
    }
    let k = match c {
        '0' => Key::Num0,
        '1' => Key::Num1,
        '2' => Key::Num2,
        '3' => Key::Num3,
        '4' => Key::Num4,
        '5' => Key::Num5,
        '6' => Key::Num6,
        '7' => Key::Num7,
        '8' => Key::Num8,
        '9' => Key::Num9,
        ',' => Key::Comma,
        '.' => Key::Period,
        ';' => Key::Semicolon,
        ':' => Key::Colon,
        '\'' => Key::Quote,
        '[' => Key::OpenBracket,
        ']' => Key::CloseBracket,
        '-' => Key::Minus,
        '=' => Key::Equals,
        '/' => Key::Slash,
        '\\' => Key::Backslash,
        '`' => Key::Backtick,
        _ => return None,
    };
    Some((vec![make(k, true)], false))
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    (s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)).then(|| &s[prefix.len()..])
}

/// Lit un raccourci complet (« Ctrl+Maj+Z », « Alt+B », « + », « Ctrl++ »).
pub fn parse_combo(text: &str) -> Result<Vec<Combo>, String> {
    let s = text.trim();
    let (mods, key) = if s == "+" {
        ("", "+")
    } else if let Some(m) = s.strip_suffix("++") {
        (m, "+")
    } else {
        match s.rfind('+') {
            Some(i) => (&s[..i], &s[i + 1..]),
            None => ("", s),
        }
    };
    let (mut ctrl, mut shift, mut alt) = (false, false, false);
    for m in mods.split('+').map(str::trim).filter(|m| !m.is_empty()) {
        match m.to_lowercase().as_str() {
            "ctrl" | "control" | "contrôle" | "cmd" => ctrl = true,
            "maj" | "shift" => shift = true,
            "alt" => alt = true,
            other => return Err(format!("modificateur inconnu « {other} » dans « {text} »")),
        }
    }
    let (keys, any_shift) = parse_key(key).ok_or_else(|| format!("touche inconnue dans « {text} »"))?;
    Ok(keys
        .into_iter()
        .map(|key| Combo {
            ctrl,
            shift,
            alt,
            key,
            any_shift,
        })
        .collect())
}

#[derive(Clone, Debug)]
pub struct Binding {
    pub action: String,
    /// Les touches telles qu'écrites dans le fichier.
    pub keys: Vec<String>,
    pub combos: Vec<Combo>,
    pub context: Option<Mode>,
    pub active: bool,
    /// Marqué « non implémenté » dans le fichier.
    pub reserved: bool,
    pub source: String,
}

#[derive(Deserialize)]
struct FileFormat {
    version: u32,
    #[serde(default)]
    raccourci: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    action: String,
    #[serde(default)]
    touches: Vec<String>,
    #[serde(default)]
    source: String,
    #[serde(default)]
    contexte: Option<String>,
    #[serde(default)]
    etat: Option<String>,
}

fn parse_file(text: &str, warnings: &mut Vec<String>) -> Result<Vec<Binding>, String> {
    let file: FileFormat = toml::from_str(text).map_err(|e| e.to_string())?;
    if file.version != 1 {
        warnings.push(format!("version de fichier {} inconnue (attendue : 1)", file.version));
    }
    let mut out = Vec::new();
    for e in file.raccourci {
        let info = action_info(&e.action);
        if info.is_none() {
            warnings.push(format!("action inconnue « {} » (ignorée)", e.action));
            continue;
        }
        let context = match e.contexte.as_deref().map(str::to_lowercase).as_deref() {
            None | Some("") | Some("partout") => None,
            Some("storyboard") => Some(Mode::Storyboard),
            Some("animation") => Some(Mode::Animation),
            Some(other) => {
                warnings.push(format!("contexte inconnu « {other} » pour {}", e.action));
                None
            }
        };
        let etat = e.etat.as_deref().map(str::to_lowercase).unwrap_or_default();
        let reserved = etat.contains("non impl");
        let disabled = etat.contains("désactiv") || etat.contains("desactiv");
        let implemented = info.is_some_and(|i| i.implemented);
        let mut combos = Vec::new();
        for k in &e.touches {
            match parse_combo(k) {
                Ok(c) => combos.extend(c),
                // Un raccourci en réserve peut viser un geste qu'on ne sait pas encore lire.
                Err(err) if !reserved => warnings.push(format!("{} : {err}", e.action)),
                Err(_) => {}
            }
        }
        if !reserved && !implemented {
            warnings.push(format!("« {} » n'existe pas encore : raccourci gardé en réserve", e.action));
        }
        out.push(Binding {
            action: e.action,
            keys: e.touches,
            combos,
            context,
            active: implemented && !reserved && !disabled,
            reserved: reserved || !implemented,
            source: e.source,
        });
    }
    Ok(out)
}

fn contexts_overlap(a: Option<Mode>, b: Option<Mode>) -> bool {
    a.is_none() || b.is_none() || a == b
}

#[derive(Default)]
pub struct Keymap {
    pub bindings: Vec<Binding>,
    pub warnings: Vec<String>,
    /// Fichier utilisateur pris en compte, le cas échéant.
    pub user_file: Option<PathBuf>,
}

impl Keymap {
    /// Raccourcis par défaut, puis ceux du fichier utilisateur qui remplacent action par action.
    pub fn from_texts(default: &str, user: Option<&str>) -> Keymap {
        let mut warnings = Vec::new();
        let mut bindings = match parse_file(default, &mut warnings) {
            Ok(b) => b,
            Err(e) => {
                warnings.push(format!("fichier par défaut illisible : {e}"));
                Vec::new()
            }
        };
        if let Some(text) = user {
            match parse_file(text, &mut warnings) {
                Ok(user_bindings) => {
                    let redefined: Vec<String> = user_bindings.iter().map(|b| b.action.clone()).collect();
                    bindings.retain(|b| !redefined.contains(&b.action));
                    // Placés devant : en cas de touche partagée, le fichier personnel l'emporte.
                    bindings.splice(0..0, user_bindings);
                }
                Err(e) => warnings.push(format!("fichier de raccourcis illisible, raccourcis par défaut utilisés : {e}")),
            }
        }
        let mut map = Keymap {
            bindings,
            warnings,
            user_file: None,
        };
        map.check_conflicts();
        map
    }

    /// Charge le fichier à côté de l'exécutable, sinon celui de %APPDATA%\Pelure, sinon le défaut.
    pub fn load() -> Keymap {
        let path = Self::candidate_paths().into_iter().find(|p| p.is_file());
        let user = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
        let mut map = Keymap::from_texts(DEFAULT_KEYMAP, user.as_deref());
        map.user_file = path;
        map
    }

    fn candidate_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
            paths.push(dir.join("keymap.toml"));
        }
        if let Some(p) = Self::appdata_path() {
            paths.push(p);
        }
        paths
    }

    pub fn appdata_path() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Pelure").join("keymap.toml"))
    }

    fn check_conflicts(&mut self) {
        let active: Vec<&Binding> = self.bindings.iter().filter(|b| b.active).collect();
        for (i, a) in active.iter().enumerate() {
            for b in &active[i + 1..] {
                if a.action == b.action || !contexts_overlap(a.context, b.context) {
                    continue;
                }
                if let Some(c) = a.combos.iter().find(|c| b.combos.contains(c)) {
                    self.warnings.push(format!(
                        "conflit : {} et {} utilisent tous deux {}",
                        a.action,
                        b.action,
                        c.label()
                    ));
                }
            }
        }
    }

    /// Action déclenchée par cette touche dans ce mode.
    pub fn find(&self, key: Key, physical: Option<Key>, m: Modifiers, mode: Mode) -> Option<&str> {
        self.bindings
            .iter()
            .filter(|b| b.active && b.context.is_none_or(|c| c == mode))
            .find(|b| b.combos.iter().any(|c| c.matches(key, physical, m)))
            .map(|b| b.action.as_str())
    }

    /// Raccourci à afficher dans les menus (le premier défini pour l'action).
    pub fn label(&self, action: &str) -> String {
        self.bindings
            .iter()
            .filter(|b| b.active && b.action == action)
            .flat_map(|b| b.combos.first())
            .next()
            .map(Combo::label)
            .unwrap_or_default()
    }

    /// Touche de l'action actuellement maintenue (gestes « maintenir », comme Espace pour la main).
    pub fn held(&self, input: &eframe::egui::InputState, action: &str) -> bool {
        self.bindings
            .iter()
            .filter(|b| b.active && b.action == action)
            .flat_map(|b| b.combos.iter())
            .any(|c| match c.key {
                KeyMatch::Char(k) => input.key_down(k) && input.modifiers.command == c.ctrl && input.modifiers.alt == c.alt,
                KeyMatch::Pos(_) => false,
            })
    }
}
