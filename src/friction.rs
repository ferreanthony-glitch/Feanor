//! Noter une friction sans quitter le dessin (2026-11-06, demande d'Anthony) : la partie sans interface. Une touche, une
//! ligne de texte si on veut, et FEANOR range ce qui te freinait avec de quoi le comprendre plus tard : une capture de la
//! fenêtre telle qu'elle était, ce que tu venais de faire (les dernières commandes, `usage.rs`), où tu étais (projet,
//! espace de travail, outil, plan).
//!
//! Tout va dans un dossier du poste, `<Documents>\SHOT\Frictions` : un fichier `frictions.md` où chaque note s'ajoute à la
//! fin, et une image par note à côté. Rien n'est envoyé nulle part ; rien n'est écrit dans le projet.
//!
//! L'application (la touche, la capture, le champ de texte, l'écriture sur un fil de fond) est dans `frictionui.rs`.

use std::path::{Path, PathBuf};

/// Le nom du dossier, dans `<Documents>\SHOT`.
pub const DOSSIER: &str = "Frictions";
/// Le fichier où les notes s'ajoutent.
pub const FICHIER: &str = "frictions.md";
/// Combien de commandes d'avant une note retient.
pub const AVANT: usize = 12;
/// L'en-tête du fichier, écrit une fois.
pub const EN_TETE: &str = "# Frictions notées dans FEANOR\n\nUne note par moment où quelque chose t'a freiné, la plus récente en bas. Chaque note dit où tu étais, ce que tu venais\nde faire et montre la fenêtre telle qu'elle était.\n";

/// Une commande faite juste avant la note.
#[derive(Clone, Debug, PartialEq)]
pub struct Avant {
    /// Son nom, comme dans l'interface.
    pub nom: String,
    /// Par où elle est partie (« clavier », « icône »…).
    pub chemin: &'static str,
    /// Combien de secondes avant la note.
    pub il_y_a: f64,
}

/// Une note de friction, prête à être écrite.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Note {
    /// La date et l'heure du poste : année, mois, jour, heure, minute, seconde.
    pub quand: (i64, u32, u32, u32, u32, u32),
    /// Ce qu'Anthony a tapé (peut être vide : la capture et le contexte suffisent parfois).
    pub texte: String,
    pub projet: String,
    pub espace: String,
    pub outil: String,
    pub plan: String,
    /// Les commandes d'avant, de la plus récente à la plus ancienne.
    pub avant: Vec<Avant>,
    /// Une capture de la fenêtre accompagne la note.
    pub capture: bool,
    pub version: String,
}

impl Note {
    /// « 2026-11-06 10:15:23 ».
    pub fn horodatage(&self) -> String {
        let (a, m, j, h, mi, s) = self.quand;
        format!("{a:04}-{m:02}-{j:02} {h:02}:{mi:02}:{s:02}")
    }

    /// Le nom du fichier de la capture : « friction-2026-11-06-101523.png ».
    pub fn nom_de_capture(&self) -> String {
        let (a, m, j, h, mi, s) = self.quand;
        format!("friction-{a:04}-{m:02}-{j:02}-{h:02}{mi:02}{s:02}.png")
    }

    /// La note en Markdown, telle qu'elle s'ajoute au fichier.
    pub fn entree(&self) -> String {
        let texte = self.texte.trim();
        // (Une note tient sur une ligne de titre : les retours à la ligne deviennent des espaces.)
        let titre = if texte.is_empty() { "(sans texte)".to_string() } else { texte.split_whitespace().collect::<Vec<_>>().join(" ") };
        let mut s = format!("\n## {} — {titre}\n\n", self.horodatage());
        let mut ou: Vec<String> = Vec::new();
        for (nom, valeur) in [("Projet", &self.projet), ("Espace", &self.espace), ("Outil", &self.outil), ("Plan", &self.plan)] {
            if !valeur.trim().is_empty() {
                ou.push(format!("{nom} : {}", valeur.trim()));
            }
        }
        if !ou.is_empty() {
            s += &format!("- {}\n", ou.join(" · "));
        }
        if self.avant.is_empty() {
            s += "- Juste avant : aucune commande depuis le lancement\n";
        } else {
            let suite: Vec<String> = self.avant.iter().map(|a| format!("{} ({}, il y a {})", a.nom, a.chemin, duree(a.il_y_a))).collect();
            s += &format!("- Juste avant, de la plus récente à la plus ancienne : {}\n", suite.join(" ; "));
        }
        if self.capture {
            s += &format!("- Capture : ![{}]({})\n", self.nom_de_capture(), self.nom_de_capture());
        } else {
            s += "- Capture : aucune (la fenêtre n'a pas pu être capturée)\n";
        }
        if !self.version.is_empty() {
            s += &format!("- {}\n", self.version);
        }
        s
    }
}

/// « 3 s », « 2 min », « 1 h 05 » : une durée lisible d'un coup d'œil.
pub fn duree(secondes: f64) -> String {
    let s = secondes.max(0.0).round() as u64;
    match s {
        0..=59 => format!("{s} s"),
        60..=3599 => format!("{} min", s / 60),
        _ => format!("{} h {:02}", s / 3600, (s % 3600) / 60),
    }
}

/// Ce qu'il faut pour écrire une note sur un fil de fond.
pub struct Ecriture {
    pub dossier: PathBuf,
    pub note: Note,
    /// Les pixels de la capture (rouge, vert, bleu, alpha, ligne par ligne) et sa taille.
    pub image: Option<(Vec<u8>, u32, u32)>,
}

impl Ecriture {
    /// Écrit la capture puis ajoute la note au fichier. Rend le chemin du fichier des notes. Si la capture ne s'écrit pas,
    /// la note est écrite quand même, et dit qu'elle n'a pas de capture.
    pub fn faire(mut self) -> Result<PathBuf, String> {
        std::fs::create_dir_all(&self.dossier).map_err(|e| format!("{} : {e}", self.dossier.display()))?;
        self.note.capture = match self.image.take() {
            // (Des pixels qui ne font pas la taille annoncée feraient échouer l'encodeur avec fracas : pas de capture, la note part.)
            Some((pixels, largeur, hauteur)) if largeur > 0 && hauteur > 0 && pixels.len() == largeur as usize * hauteur as usize * 4 => {
                image::save_buffer(self.dossier.join(self.note.nom_de_capture()), &pixels, largeur, hauteur, image::ColorType::Rgba8).is_ok()
            }
            _ => false,
        };
        ajouter(&self.dossier.join(FICHIER), &self.note.entree())
    }
}

/// Ajoute une entrée à la fin du fichier des notes (créé avec son en-tête s'il n'existe pas).
pub fn ajouter(fichier: &Path, entree: &str) -> Result<PathBuf, String> {
    use std::io::Write;
    let neuf = !fichier.exists();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(fichier).map_err(|e| format!("{} : {e}", fichier.display()))?;
    if neuf {
        f.write_all(EN_TETE.as_bytes()).map_err(|e| e.to_string())?;
    }
    f.write_all(entree.as_bytes()).map_err(|e| e.to_string())?;
    Ok(fichier.to_path_buf())
}
