//! Fiche technique (Anthony, 2026-10-20) : le découpage du film en tableau, une ligne par plan (ou par
//! image de storyboard), pour la production, le layout et le montage. Fichier > Exporter > « Fiche
//! technique (tableur) ».
//!
//! Ce module est **pur** (pas d'interface, pas de fichier) : `build` lit le document et rend une
//! `Fiche` (des colonnes, des lignes de cellules) ; `ficheformats.rs` en fait du CSV, du texte pour le
//! presse-papiers ou un classeur XLSX ; `fichejob.rs` écrit en tâche de fond, avec les vignettes ;
//! `ficheui.rs` est la fenêtre.
//!
//! # Colonnes
//!
//! Chaque colonne (`Colonne`) a un identifiant stable (celui des réglages), un titre, une sorte de
//! valeur (texte, entier, décimal) et une largeur. L'utilisateur choisit lesquelles et dans quel ordre
//! (`Choix`) ; un identifiant inconnu (réglages d'une autre version) est ignoré.
//!
//! # Temps
//!
//! Les durées sont en images du projet. Les time codes (`HH:MM:SS:FF`, sans saut d'images, comme
//! l'export pour le montage) partent de `start_tc` (01:00:00:00 d'office) et sont **ceux du film
//! entier** : un plan garde son time code même si l'on n'exporte que quelques plans. « TC sortie » est
//! la première image qui suit le plan (entrée + durée), comme dans un EDL.
//!
//! # Mouvement de caméra
//!
//! Déduit des images clés de la caméra du plan (ou de l'image de storyboard : sa part du plan). Entre
//! deux clés chaque réglage va de l'un à l'autre sans aller-retour : les extrêmes sont donc aux clés,
//! et l'on ne regarde que la caméra au début, à la fin et à chaque clé. Un réglage « bouge » quand son
//! étendue dépasse un seuil (voir `SEUIL_*`) ; son sens est celui du début à la fin, ou « aller-retour »
//! si la caméra revient presque à son point de départ.
//!
//! | Réglage de la caméra | Mot | Sens |
//! |---|---|---|
//! | X (la caméra glisse sur la feuille) | travelling | à droite / à gauche |
//! | Y | travelling | vers le haut / vers le bas |
//! | Z (avancée vers la feuille) | travelling | avant / arrière |
//! | Zoom (focale) | zoom | avant / arrière |
//! | Panoramique (la caméra tourne sur place) | panoramique | à droite / à gauche |
//! | Bascule | bascule | vers le haut / vers le bas |
//! | Rotation (roulis) | rotation | horaire / antihoraire |
//!
//! Rien ne bouge : « fixe ». Plusieurs réglages : leurs mots, séparés par « + » (X et Y ensemble :
//! « travelling à droite et vers le bas »). La colonne de détail dit, pour chaque réglage qui bouge, sa
//! valeur au début et à la fin.

use std::collections::HashMap;

use crate::camera::{self, Camera};
use crate::document::Document;
use crate::editorial;
use crate::envois::Etat;

// ----------------------------------------------------------------------------------- colonnes

/// Une colonne de la fiche.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Colonne {
    Sequence,
    Numero,
    Reference,
    Version,
    Nom,
    Image,
    DureeImages,
    DureeSecondes,
    DureeTc,
    TcEntree,
    TcSortie,
    Cadrage,
    Mouvement,
    MouvementDetail,
    ImagesStoryboard,
    Action,
    Dialogue,
    Notes,
    TransitionEntree,
    TransitionSortie,
    Sons,
    Commentaires,
    CommentairesTexte,
    Modifie,
    Envoi,
    Couleur,
    Calques,
    Effets,
    Vignette,
}

/// La sorte de valeur d'une colonne.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sorte {
    Texte,
    Entier,
    Decimal,
}

impl Colonne {
    /// Toutes les colonnes, dans l'ordre où la fenêtre les propose.
    pub const TOUTES: [Colonne; 29] = [
        Colonne::Sequence,
        Colonne::Numero,
        Colonne::Reference,
        Colonne::Version,
        Colonne::Nom,
        Colonne::Image,
        Colonne::Vignette,
        Colonne::DureeImages,
        Colonne::DureeSecondes,
        Colonne::DureeTc,
        Colonne::TcEntree,
        Colonne::TcSortie,
        Colonne::Cadrage,
        Colonne::Mouvement,
        Colonne::MouvementDetail,
        Colonne::ImagesStoryboard,
        Colonne::Action,
        Colonne::Dialogue,
        Colonne::Notes,
        Colonne::TransitionEntree,
        Colonne::TransitionSortie,
        Colonne::Sons,
        Colonne::Commentaires,
        Colonne::CommentairesTexte,
        Colonne::Modifie,
        Colonne::Envoi,
        Colonne::Couleur,
        Colonne::Calques,
        Colonne::Effets,
    ];

    /// L'identifiant enregistré dans les réglages (ne change jamais).
    pub fn id(self) -> &'static str {
        match self {
            Colonne::Sequence => "sequence",
            Colonne::Numero => "numero",
            Colonne::Reference => "reference",
            Colonne::Version => "version",
            Colonne::Nom => "nom",
            Colonne::Image => "image",
            Colonne::DureeImages => "duree_images",
            Colonne::DureeSecondes => "duree_secondes",
            Colonne::DureeTc => "duree_tc",
            Colonne::TcEntree => "tc_entree",
            Colonne::TcSortie => "tc_sortie",
            Colonne::Cadrage => "cadrage",
            Colonne::Mouvement => "mouvement",
            Colonne::MouvementDetail => "mouvement_detail",
            Colonne::ImagesStoryboard => "images_storyboard",
            Colonne::Action => "action",
            Colonne::Dialogue => "dialogue",
            Colonne::Notes => "notes",
            Colonne::TransitionEntree => "transition_entree",
            Colonne::TransitionSortie => "transition_sortie",
            Colonne::Sons => "sons",
            Colonne::Commentaires => "commentaires",
            Colonne::CommentairesTexte => "commentaires_texte",
            Colonne::Modifie => "modifie",
            Colonne::Envoi => "envoi",
            Colonne::Couleur => "couleur",
            Colonne::Calques => "calques",
            Colonne::Effets => "effets",
            Colonne::Vignette => "vignette",
        }
    }

    pub fn depuis_id(id: &str) -> Option<Colonne> {
        Colonne::TOUTES.iter().copied().find(|c| c.id() == id)
    }

    /// Le titre de l'en-tête.
    pub fn titre(self) -> &'static str {
        match self {
            Colonne::Sequence => "Séquence",
            Colonne::Numero => "N° de plan",
            Colonne::Reference => "Référence",
            Colonne::Version => "Version",
            Colonne::Nom => "Nom du plan",
            Colonne::Image => "Image",
            Colonne::DureeImages => "Durée (images)",
            Colonne::DureeSecondes => "Durée (s)",
            Colonne::DureeTc => "Durée (TC)",
            Colonne::TcEntree => "TC entrée",
            Colonne::TcSortie => "TC sortie",
            Colonne::Cadrage => "Cadrage",
            Colonne::Mouvement => "Mouvement de caméra",
            Colonne::MouvementDetail => "Détail du mouvement",
            Colonne::ImagesStoryboard => "Images de storyboard",
            Colonne::Action => "Action",
            Colonne::Dialogue => "Dialogue",
            Colonne::Notes => "Notes",
            Colonne::TransitionEntree => "Transition d'entrée",
            Colonne::TransitionSortie => "Transition de sortie",
            Colonne::Sons => "Sons",
            Colonne::Commentaires => "Commentaires ouverts",
            Colonne::CommentairesTexte => "Texte des commentaires",
            Colonne::Modifie => "Modifié depuis l'envoi",
            Colonne::Envoi => "Dernier envoi",
            Colonne::Couleur => "Couleur du plan",
            Colonne::Calques => "Calques",
            Colonne::Effets => "Effets",
            Colonne::Vignette => "Vignette",
        }
    }

    /// Ce que dit la colonne, en une phrase (bulle de la fenêtre).
    pub fn aide(self) -> &'static str {
        match self {
            Colonne::Sequence => "Le nom ou le numéro de la séquence.",
            Colonne::Numero => "Le numéro du plan, tel que la numérotation le donne (12A pour un plan inséré).",
            Colonne::Reference => "Le nom du clip de l'export pour le montage : SQ01_PL012A, avec _V2 si le plan a plusieurs versions.",
            Colonne::Version => "V2 quand le plan a plusieurs versions (la version affichée) ; vide sinon.",
            Colonne::Nom => "Le nom que tu as donné au plan, s'il en a un.",
            Colonne::Image => "En mode « une ligne par image de storyboard » : le rang de l'image dans le plan (2 / 3).",
            Colonne::DureeImages => "La durée en images (un nombre, pour calculer dessus).",
            Colonne::DureeSecondes => "La durée en secondes (un nombre).",
            Colonne::DureeTc => "La durée en time code HH:MM:SS:FF.",
            Colonne::TcEntree => "Le time code de la première image du plan dans le film.",
            Colonne::TcSortie => "Le time code de l'image qui suit le plan.",
            Colonne::Cadrage => "L'objectif du plan si c'est un des objectifs proposés (Portrait, Normale…), sinon sa focale en mm.",
            Colonne::Mouvement => "Fixe, panoramique, travelling, zoom, bascule, rotation, ou plusieurs ensemble : déduit des images clés de la caméra.",
            Colonne::MouvementDetail => "Pour chaque réglage de la caméra qui bouge, sa valeur au début et à la fin.",
            Colonne::ImagesStoryboard => "Le nombre d'images de storyboard du plan (le plan et ses marques d'image).",
            Colonne::Action => "Le texte « Action » du plan.",
            Colonne::Dialogue => "Le texte « Dialogue » du plan.",
            Colonne::Notes => "Le texte « Notes » du plan.",
            Colonne::TransitionEntree => "La transition posée sur la coupe qui précède le plan.",
            Colonne::TransitionSortie => "La transition posée sur la coupe qui suit le plan.",
            Colonne::Sons => "Les noms des sons posés sur les pistes pendant le plan.",
            Colonne::Commentaires => "Le nombre de commentaires pas encore résolus sur le plan.",
            Colonne::CommentairesTexte => "Le texte des commentaires pas encore résolus, avec leur auteur.",
            Colonne::Modifie => "Modifié ou ajouté depuis l'envoi de référence (vide : inchangé ou aucun envoi).",
            Colonne::Envoi => "Le nom de l'envoi de référence : le dernier, ou celui choisi pour l'étendue.",
            Colonne::Couleur => "La couleur de repère du plan.",
            Colonne::Calques => "Le nombre de calques du plan (les dossiers ne comptent pas).",
            Colonne::Effets => "Les effets allumés sur les calques du plan.",
            Colonne::Vignette => "Le chemin d'une image PNG du plan (dossier « vignettes » à côté du fichier) ; dans un classeur XLSX, l'image elle-même.",
        }
    }

    pub fn sorte(self) -> Sorte {
        match self {
            Colonne::DureeImages | Colonne::ImagesStoryboard | Colonne::Commentaires | Colonne::Calques => Sorte::Entier,
            Colonne::DureeSecondes => Sorte::Decimal,
            _ => Sorte::Texte,
        }
    }

    /// Largeur dans le classeur, en caractères.
    pub fn largeur(self) -> f64 {
        match self {
            Colonne::Sequence | Colonne::Couleur | Colonne::Envoi => 16.0,
            Colonne::Numero | Colonne::Version | Colonne::Image | Colonne::Calques | Colonne::Commentaires | Colonne::ImagesStoryboard => 11.0,
            Colonne::Reference => 20.0,
            Colonne::Nom => 22.0,
            Colonne::DureeImages | Colonne::DureeSecondes => 10.0,
            Colonne::DureeTc | Colonne::TcEntree | Colonne::TcSortie => 13.0,
            Colonne::Cadrage => 20.0,
            Colonne::Mouvement => 26.0,
            Colonne::MouvementDetail => 38.0,
            Colonne::Action | Colonne::Dialogue | Colonne::Notes => 40.0,
            Colonne::TransitionEntree | Colonne::TransitionSortie => 20.0,
            Colonne::Sons => 28.0,
            Colonne::CommentairesTexte => 44.0,
            Colonne::Modifie => 14.0,
            Colonne::Effets => 28.0,
            Colonne::Vignette => VIGNETTE_COLONNE,
        }
    }

    /// Le classeur coupe les lignes de cette colonne (textes longs).
    pub fn retour_a_la_ligne(self) -> bool {
        matches!(
            self,
            Colonne::Nom
                | Colonne::Cadrage
                | Colonne::Mouvement
                | Colonne::MouvementDetail
                | Colonne::Action
                | Colonne::Dialogue
                | Colonne::Notes
                | Colonne::TransitionEntree
                | Colonne::TransitionSortie
                | Colonne::Sons
                | Colonne::CommentairesTexte
                | Colonne::Effets
        )
    }

    /// Cochée d'office.
    pub fn par_defaut(self) -> bool {
        !matches!(self, Colonne::Reference | Colonne::Image | Colonne::DureeTc | Colonne::MouvementDetail | Colonne::ImagesStoryboard | Colonne::CommentairesTexte | Colonne::Envoi | Colonne::Couleur | Colonne::Calques | Colonne::Effets)
    }
}

/// Largeur de la colonne des vignettes dans le classeur, en caractères (≈ 165 pixels).
pub const VIGNETTE_COLONNE: f64 = 23.5;
/// Taille d'une vignette dans le classeur, en pixels : elle tient dans ce cadre.
pub const VIGNETTE_AFFICHEE: [u32; 2] = [160, 90];
/// Largeur des fichiers PNG de vignettes (le double de l'affichage : net quand on agrandit).
pub const VIGNETTE_LARGEUR: u32 = 320;
/// Le dossier des vignettes, à côté du fichier de la fiche.
pub const DOSSIER_VIGNETTES: &str = "vignettes";

/// Les colonnes cochées d'office, dans l'ordre de la fenêtre.
pub fn colonnes_par_defaut() -> Vec<Colonne> {
    Colonne::TOUTES.iter().copied().filter(|c| c.par_defaut()).collect()
}

/// Les identifiants des réglages en colonnes : l'ordre est gardé, les inconnus et les doublons
/// sont ignorés.
pub fn colonnes_depuis_ids(ids: &[String]) -> Vec<Colonne> {
    let mut out: Vec<Colonne> = Vec::new();
    for id in ids {
        if let Some(c) = Colonne::depuis_id(id) {
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out
}

// ----------------------------------------------------------------------------------- réglages

/// Ce que la fenêtre retient d'une séance à l'autre (réglages de l'application, `reglages.toml`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Reglages {
    /// Les colonnes choisies, dans leur ordre (identifiants de `Colonne::id`).
    #[serde(default = "ids_par_defaut")]
    pub colonnes: Vec<String>,
    #[serde(default)]
    pub format: crate::ficheformats::Format,
    #[serde(default)]
    pub separateur: crate::ficheformats::Separateur,
    /// Le time code de la première image du film.
    #[serde(default = "tc_par_defaut", rename = "time_code_depart")]
    pub start_tc: String,
    /// Une ligne par image de storyboard (sinon : une par plan).
    #[serde(default, rename = "une_ligne_par_image")]
    pub par_image: bool,
    /// Le dossier de la dernière fiche.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dossier: Option<std::path::PathBuf>,
}

fn ids_par_defaut() -> Vec<String> {
    colonnes_par_defaut().iter().map(|c| c.id().to_string()).collect()
}

fn tc_par_defaut() -> String {
    "01:00:00:00".into()
}

impl Default for Reglages {
    fn default() -> Self {
        Reglages { colonnes: ids_par_defaut(), format: Default::default(), separateur: Default::default(), start_tc: tc_par_defaut(), par_image: false, dossier: None }
    }
}

// ----------------------------------------------------------------------------------- la fiche

/// Une cellule.
#[derive(Clone, Debug, PartialEq)]
pub enum Cellule {
    Vide,
    Texte(String),
    Entier(i64),
    Decimal(f64),
}

impl Cellule {
    fn texte(s: impl Into<String>) -> Cellule {
        let s: String = s.into();
        if s.is_empty() {
            Cellule::Vide
        } else {
            Cellule::Texte(s)
        }
    }
}

/// La vignette d'une ligne : le fichier PNG à écrire et l'image du film à y mettre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vignette {
    /// « SQ01_PL012A.png » (dans `vignettes/`).
    pub fichier: String,
    /// (plan, image du plan) à rendre.
    pub image: (usize, usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ligne {
    /// Une cellule par colonne de la fiche.
    pub cellules: Vec<Cellule>,
    pub vignette: Option<Vignette>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fiche {
    pub colonnes: Vec<Colonne>,
    pub lignes: Vec<Ligne>,
    /// Ce que l'utilisateur doit savoir de cette fiche (relevé des envois pas prêt…).
    pub avertissements: Vec<String>,
}

impl Fiche {
    /// Les images à rendre pour les vignettes, dans l'ordre des lignes qui en ont.
    pub fn images_vignettes(&self) -> Vec<(usize, usize)> {
        self.lignes.iter().filter_map(|l| l.vignette.as_ref().map(|v| v.image)).collect()
    }

    /// Les noms des fichiers de vignettes, dans le même ordre.
    pub fn fichiers_vignettes(&self) -> Vec<String> {
        self.lignes.iter().filter_map(|l| l.vignette.as_ref().map(|v| v.fichier.clone())).collect()
    }

    /// Les titres de l'en-tête.
    pub fn titres(&self) -> Vec<&'static str> {
        self.colonnes.iter().map(|c| c.titre()).collect()
    }
}

/// Ce dont `build` a besoin en plus du document.
pub struct Entree<'a> {
    /// Les plans (rangs), dans n'importe quel ordre : la fiche suit l'ordre du film.
    pub plans: &'a [usize],
    pub colonnes: &'a [Colonne],
    /// Une ligne par image de storyboard (le plan et ses marques d'image) ; sinon une par plan.
    pub par_image: bool,
    /// Le time code de la première image du film, en images.
    pub start_tc: i64,
    /// L'état des plans (modifié, ajouté) au regard de l'envoi de référence ; rien : pas de relevé.
    pub etats: Option<&'a HashMap<u64, Etat>>,
    /// Le nom de l'envoi de référence ; rien : le projet n'a pas d'envoi.
    pub envoi: Option<&'a str>,
}

/// La version qui part, avec le nom de clip de l'export pour le montage (« SQ01_PL012A_V2 »).
pub fn reference(doc: &Document, p: usize) -> String {
    let id = doc.panels[p].id;
    let base = doc.plan_full_ref(p);
    if doc.nombre_de_versions(id) > 1 {
        format!("{base}_{}", crate::planvers::etiquette(doc.numero_de_version(id)))
    } else {
        base
    }
}

/// Le nom du fichier de vignette d'une ligne : la référence, plus le rang de l'image après la première.
fn nom_vignette(reference: &str, rang: usize) -> String {
    let base = editorial::file_safe(reference);
    if rang == 0 {
        format!("{base}.png")
    } else {
        format!("{base}_img{:02}.png", rang + 1)
    }
}

/// Construit la fiche. Erreur si rien n'est à écrire ou si aucune colonne n'est choisie.
pub fn build(doc: &Document, e: &Entree) -> Result<Fiche, String> {
    if e.colonnes.is_empty() {
        return Err("Choisis au moins une colonne.".into());
    }
    let mut plans: Vec<usize> = e.plans.iter().copied().filter(|&p| p < doc.panels.len()).collect();
    plans.sort_unstable();
    plans.dedup();
    if plans.is_empty() {
        return Err("Aucun plan à écrire.".into());
    }
    let fps = doc.fps.max(1);
    // Première image de chaque plan dans le film (une seule passe).
    let mut debuts = Vec::with_capacity(doc.panels.len());
    let mut at = 0usize;
    for p in &doc.panels {
        debuts.push(at);
        at += p.frame_count();
    }
    let mut avertissements = Vec::new();
    if e.colonnes.contains(&Colonne::Modifie) && e.envoi.is_some() && e.etats.is_none() {
        avertissements.push("Le relevé des plans depuis l'envoi n'est pas prêt : la colonne « Modifié depuis l'envoi » est vide. Réessaie dans un instant.".into());
    }
    let mut lignes = Vec::with_capacity(plans.len());
    for &i in &plans {
        let panel = &doc.panels[i];
        let len = panel.frame_count();
        let boards = crate::board::plan_boards(doc, i);
        let spans: Vec<(usize, usize, usize)> = if e.par_image {
            boards.iter().enumerate().map(|(k, &(_, a, b))| (k, a, b)).collect()
        } else {
            vec![(0, 0, len)]
        };
        let reference = reference(doc, i);
        for (k, a, b) in spans {
            let ligne = ligne_du_plan(doc, e, i, (k, a, b), &boards, &reference, &debuts, fps);
            lignes.push(ligne);
        }
    }
    Ok(Fiche { colonnes: e.colonnes.to_vec(), lignes, avertissements })
}

#[allow(clippy::too_many_arguments)]
fn ligne_du_plan(doc: &Document, e: &Entree, i: usize, (k, a, b): (usize, usize, usize), boards: &[(u64, usize, usize)], reference: &str, debuts: &[usize], fps: u32) -> Ligne {
    let panel = &doc.panels[i];
    let nb_images = boards.len();
    let len = b - a;
    let abs_a = debuts[i] + a;
    let abs_b = debuts[i] + b;
    let premiere = a == 0;
    let derniere = b == panel.frame_count();
    // Les textes : ceux de l'image de storyboard (la marque), ou du plan pour sa première image.
    let (action, dialogue, notes) = if !e.par_image || k == 0 {
        (panel.action.clone(), panel.dialogue.clone(), panel.notes.clone())
    } else {
        let id = boards.get(k).map(|s| s.0);
        match panel.marks.iter().find(|m| Some(m.id) == id) {
            Some(m) => (m.action.clone(), m.dialogue.clone(), m.notes.clone()),
            None => (String::new(), String::new(), String::new()),
        }
    };
    let cameras: Vec<Camera> = echantillons(panel.camera.iter().map(|c| c.frame), a, b).into_iter().map(|f| doc.panel_camera(i, f)).collect();
    let mut cellules = Vec::with_capacity(e.colonnes.len());
    let mut vignette = None;
    for &c in e.colonnes {
        let cel = match c {
            Colonne::Sequence => Cellule::texte(doc.sequence_label(doc.sequence_of(i))),
            Colonne::Numero => Cellule::texte(doc.plan_code(panel.scene)),
            Colonne::Reference => Cellule::texte(reference),
            Colonne::Version => Cellule::texte(if doc.nombre_de_versions(panel.id) > 1 { crate::planvers::etiquette(doc.numero_de_version(panel.id)) } else { String::new() }),
            Colonne::Nom => Cellule::texte(doc.scene(panel.scene).map(|s| s.name.trim().to_string()).unwrap_or_default()),
            Colonne::Image => Cellule::texte(if e.par_image { format!("{} / {}", k + 1, nb_images) } else { String::new() }),
            Colonne::DureeImages => Cellule::Entier(len as i64),
            Colonne::DureeSecondes => Cellule::Decimal((len as f64 / fps as f64 * 1000.0).round() / 1000.0),
            Colonne::DureeTc => Cellule::texte(editorial::timecode(len as i64, fps)),
            Colonne::TcEntree => Cellule::texte(editorial::timecode(e.start_tc + abs_a as i64, fps)),
            Colonne::TcSortie => Cellule::texte(editorial::timecode(e.start_tc + abs_b as i64, fps)),
            Colonne::Cadrage => Cellule::texte(cadrage(cameras.first().copied().unwrap_or_else(|| doc.default_camera()))),
            Colonne::Mouvement => Cellule::texte(mouvement(&cameras, doc.frame).label()),
            Colonne::MouvementDetail => Cellule::texte(mouvement_detail(&cameras, doc.frame)),
            Colonne::ImagesStoryboard => Cellule::Entier(nb_images as i64),
            Colonne::Action => Cellule::texte(action.clone()),
            Colonne::Dialogue => Cellule::texte(dialogue.clone()),
            Colonne::Notes => Cellule::texte(notes.clone()),
            Colonne::TransitionEntree => Cellule::texte(if premiere { transition(doc, i, true) } else { String::new() }),
            Colonne::TransitionSortie => Cellule::texte(if derniere { transition(doc, i, false) } else { String::new() }),
            Colonne::Sons => Cellule::texte(sons(doc, abs_a, abs_b).join("; ")),
            Colonne::Commentaires => Cellule::Entier(commentaires(doc, i, a, b, derniere).len() as i64),
            Colonne::CommentairesTexte => Cellule::texte(commentaires(doc, i, a, b, derniere).iter().map(|c| if c.author.trim().is_empty() { c.text.clone() } else { format!("{} : {}", c.author.trim(), c.text) }).collect::<Vec<_>>().join("\n")),
            Colonne::Modifie => Cellule::texte(match (e.envoi, e.etats) {
                (Some(_), Some(m)) => match m.get(&panel.id) {
                    Some(Etat::Modifie) => "Modifié",
                    Some(Etat::Nouveau) => "Ajouté",
                    _ => "",
                },
                _ => "",
            }),
            Colonne::Envoi => Cellule::texte(e.envoi.unwrap_or("")),
            Colonne::Couleur => Cellule::texte(panel.color.and_then(|k| crate::marks::COLORS.get(k as usize)).map_or("", |c| c.0)),
            Colonne::Calques => Cellule::Entier(panel.layers.iter().filter(|l| !l.folder).count() as i64),
            Colonne::Effets => Cellule::texte(effets(panel).join("; ")),
            Colonne::Vignette => {
                let fichier = nom_vignette(reference, if e.par_image { k } else { 0 });
                let chemin = format!("{DOSSIER_VIGNETTES}/{fichier}");
                vignette = Some(Vignette { fichier, image: (i, a) });
                Cellule::Texte(chemin)
            }
        };
        cellules.push(cel);
    }
    Ligne { cellules, vignette }
}

/// Les images à regarder pour le mouvement d'un intervalle `[a, b)` : le début, la fin, et chaque
/// clé entre les deux.
pub fn echantillons(cles: impl Iterator<Item = usize>, a: usize, b: usize) -> Vec<usize> {
    let last = b.saturating_sub(1).max(a);
    let mut v = vec![a];
    v.extend(cles.filter(|&f| f > a && f < last));
    if last > a {
        v.push(last);
    }
    v
}

// ----------------------------------------------------------------------------------- cadrage

/// Écrit un nombre à la française : virgule décimale, `dec` décimales au plus, sans zéros inutiles.
pub fn nombre(v: f64, dec: usize) -> String {
    let mut s = format!("{v:.dec$}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".into();
    }
    s.replace('.', ",")
}

/// L'objectif de la caméra : « Portrait · 85 mm » si c'est un des objectifs proposés, sinon « 52 mm ».
pub fn cadrage(cam: Camera) -> String {
    let mm = cam.focal();
    match camera::FOCAL_PRESETS.iter().find(|p| (p.mm - mm).abs() < 0.05) {
        Some(p) => format!("{} · {} mm", p.name, nombre(p.mm as f64, 0)),
        None => format!("{} mm", nombre(mm as f64, 1)),
    }
}

// ----------------------------------------------------------------------------------- mouvement

/// Étendue minimale d'un déplacement (X, Y), en part de la largeur (X) ou de la hauteur (Y) du cadre.
pub const SEUIL_POSITION: f32 = 0.01;
/// Étendue minimale de l'avancée (Z), en part de la distance de référence de la caméra.
pub const SEUIL_AVANCEE: f32 = 0.02;
/// Écart minimal de focale : le rapport entre la plus longue et la plus courte (1,02 : 2 %).
pub const SEUIL_ZOOM: f32 = 1.02;
/// Étendue minimale d'une rotation, d'une bascule ou d'un panoramique, en degrés.
pub const SEUIL_ANGLE: f32 = 0.5;

/// Le sens d'un réglage qui bouge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sens {
    /// La valeur monte du début à la fin.
    Plus,
    Moins,
    /// Elle bouge mais revient (presque) où elle était.
    AllerRetour,
}

/// Un réglage de la caméra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reglage {
    X,
    Y,
    Avancee,
    Zoom,
    Panoramique,
    Bascule,
    Rotation,
}

/// Ce que fait la caméra : les réglages qui bougent, dans l'ordre du tableau de l'en-tête.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Mouvement {
    pub parties: Vec<(Reglage, Sens)>,
}

impl Mouvement {
    pub fn est_fixe(&self) -> bool {
        self.parties.is_empty()
    }

    fn sens(&self, r: Reglage) -> Option<Sens> {
        self.parties.iter().find(|p| p.0 == r).map(|p| p.1)
    }

    /// « fixe », « panoramique à droite + zoom avant »…
    pub fn label(&self) -> String {
        if self.est_fixe() {
            return "fixe".into();
        }
        let mut mots: Vec<String> = Vec::new();
        let lateral = self.sens(Reglage::X).map(|s| match s {
            Sens::Plus => "à droite",
            Sens::Moins => "à gauche",
            Sens::AllerRetour => "latéral aller-retour",
        });
        let vertical = self.sens(Reglage::Y).map(|s| match s {
            Sens::Plus => "vers le bas",
            Sens::Moins => "vers le haut",
            Sens::AllerRetour => "vertical aller-retour",
        });
        match (lateral, vertical) {
            (Some(x), Some(y)) => mots.push(format!("travelling {x} et {y}")),
            (Some(x), None) => mots.push(format!("travelling {x}")),
            (None, Some(y)) => mots.push(format!("travelling {y}")),
            (None, None) => {}
        }
        if let Some(s) = self.sens(Reglage::Avancee) {
            mots.push(
                match s {
                    Sens::Plus => "travelling avant",
                    Sens::Moins => "travelling arrière",
                    Sens::AllerRetour => "travelling avant-arrière",
                }
                .into(),
            );
        }
        if let Some(s) = self.sens(Reglage::Panoramique) {
            mots.push(
                match s {
                    Sens::Plus => "panoramique à droite",
                    Sens::Moins => "panoramique à gauche",
                    Sens::AllerRetour => "panoramique aller-retour",
                }
                .into(),
            );
        }
        if let Some(s) = self.sens(Reglage::Bascule) {
            mots.push(
                match s {
                    Sens::Plus => "bascule vers le haut",
                    Sens::Moins => "bascule vers le bas",
                    Sens::AllerRetour => "bascule aller-retour",
                }
                .into(),
            );
        }
        if let Some(s) = self.sens(Reglage::Zoom) {
            mots.push(
                match s {
                    Sens::Plus => "zoom avant",
                    Sens::Moins => "zoom arrière",
                    Sens::AllerRetour => "zoom avant-arrière",
                }
                .into(),
            );
        }
        if let Some(s) = self.sens(Reglage::Rotation) {
            mots.push(
                match s {
                    Sens::Plus => "rotation horaire",
                    Sens::Moins => "rotation antihoraire",
                    Sens::AllerRetour => "rotation aller-retour",
                }
                .into(),
            );
        }
        mots.join(" + ")
    }
}

/// Le sens d'une suite de valeurs : rien si son étendue ne dépasse pas `seuil`. Un déplacement net d'au
/// moins `seuil` donne son signe ; sinon la valeur est partie et revenue.
fn sens_de(valeurs: &[f32], seuil: f32) -> Option<Sens> {
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for &v in valeurs {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    // (Une suite vide, ou des valeurs qui ne sont pas des nombres, donnent une étendue négative : rien ne bouge.)
    if hi - lo < seuil || valeurs.is_empty() {
        return None;
    }
    let net = valeurs[valeurs.len() - 1] - valeurs[0];
    Some(if net >= seuil {
        Sens::Plus
    } else if net <= -seuil {
        Sens::Moins
    } else {
        Sens::AllerRetour
    })
}

/// Le mouvement d'une suite de caméras (début, clés, fin) pour un cadre de taille `frame`.
pub fn mouvement(cams: &[Camera], frame: [usize; 2]) -> Mouvement {
    if cams.len() < 2 {
        return Mouvement::default();
    }
    let col = |f: &dyn Fn(&Camera) -> f32| -> Vec<f32> { cams.iter().map(f).collect() };
    let (fw, fh) = (frame[0].max(1) as f32, frame[1].max(1) as f32);
    let mut parties = Vec::new();
    let mut ajoute = |r: Reglage, s: Option<Sens>| {
        if let Some(s) = s {
            parties.push((r, s));
        }
    };
    ajoute(Reglage::X, sens_de(&col(&|c| c.x), SEUIL_POSITION * fw));
    ajoute(Reglage::Y, sens_de(&col(&|c| c.y), SEUIL_POSITION * fh));
    ajoute(Reglage::Avancee, sens_de(&col(&|c| c.dolly), SEUIL_AVANCEE * camera::ref_distance(frame)));
    ajoute(Reglage::Panoramique, sens_de(&col(&|c| c.pan), SEUIL_ANGLE));
    ajoute(Reglage::Bascule, sens_de(&col(&|c| c.tilt), SEUIL_ANGLE));
    // Le zoom se compare en rapport : on prend le logarithme de la focale.
    ajoute(Reglage::Zoom, sens_de(&col(&|c| c.zoom.max(0.01).ln()), SEUIL_ZOOM.ln()));
    ajoute(Reglage::Rotation, sens_de(&col(&|c| c.angle), SEUIL_ANGLE));
    Mouvement { parties }
}

/// Pour chaque réglage qui bouge : sa valeur au début et à la fin (« X : 960 → 1 200 px »).
pub fn mouvement_detail(cams: &[Camera], frame: [usize; 2]) -> String {
    let m = mouvement(cams, frame);
    let (Some(d), Some(f)) = (cams.first(), cams.last()) else { return String::new() };
    let mut out = Vec::new();
    for (r, _) in &m.parties {
        out.push(match r {
            Reglage::X => format!("X : {} → {} px", nombre(d.x as f64, 0), nombre(f.x as f64, 0)),
            Reglage::Y => format!("Y : {} → {} px", nombre(d.y as f64, 0), nombre(f.y as f64, 0)),
            Reglage::Avancee => format!("Avancée : {} → {} px", nombre(d.dolly as f64, 0), nombre(f.dolly as f64, 0)),
            Reglage::Panoramique => format!("Panoramique : {} → {}°", nombre(d.pan as f64, 1), nombre(f.pan as f64, 1)),
            Reglage::Bascule => format!("Bascule : {} → {}°", nombre(d.tilt as f64, 1), nombre(f.tilt as f64, 1)),
            Reglage::Zoom => format!("Focale : {} → {} mm", nombre(d.focal() as f64, 1), nombre(f.focal() as f64, 1)),
            Reglage::Rotation => format!("Rotation : {} → {}°", nombre(d.angle as f64, 1), nombre(f.angle as f64, 1)),
        });
    }
    out.join(" ; ")
}

// ----------------------------------------------------------------------------------- le reste

/// La transition posée sur la coupe qui précède (`entree`) ou suit le plan `i` : « Fondu enchaîné (12 i) ».
fn transition(doc: &Document, i: usize, entree: bool) -> String {
    let id = doc.panels[i].id;
    let t = if entree {
        let avant = i.checked_sub(1).map(|k| doc.panels[k].id);
        doc.transition_on(avant, Some(id))
    } else {
        doc.transition_on(Some(id), doc.panels.get(i + 1).map(|p| p.id))
    };
    t.map(|t| format!("{} ({} i)", t.kind.name(), t.frames)).unwrap_or_default()
}

/// Les noms des sons posés sur les pistes pendant les images `[a, b)` du film, une fois chacun, piste après piste.
fn sons(doc: &Document, a: usize, b: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for track in &doc.audio_tracks {
        let mut clips: Vec<&crate::document::SoundClip> = track.clips.iter().filter(|c| c.length > 0 && c.start < b && c.end() > a).collect();
        clips.sort_by_key(|c| c.start);
        for c in clips {
            let name = doc.sounds.get(&c.sound).map_or_else(|| "(son manquant)".to_string(), |s| s.name.clone());
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

/// Les commentaires pas résolus du plan `i` qui tombent dans les images `[a, b)` (ceux qui dépassent le plan,
/// sur sa dernière image).
fn commentaires(doc: &Document, i: usize, a: usize, b: usize, derniere: bool) -> Vec<&crate::commentaires::Comment> {
    let p = &doc.panels[i];
    let len = p.frame_count();
    let mut v: Vec<&crate::commentaires::Comment> = doc
        .comments
        .iter()
        .filter(|c| c.panel == p.id && !c.done)
        .filter(|c| {
            let at = c.frame.min(len.saturating_sub(1));
            at >= a && (at < b || derniere)
        })
        .collect();
    v.sort_by_key(|c| (c.frame, c.id));
    v
}

/// Les effets allumés sur les calques du plan : « Flou gaussien ; Lueur × 2 ».
fn effets(panel: &crate::document::Panel) -> Vec<String> {
    let mut noms: Vec<(&'static str, usize)> = Vec::new();
    for l in panel.layers.iter().filter(|l| !l.folder && l.fx_enabled) {
        for e in l.fx.iter().filter(|e| e.enabled) {
            let nom = e.kind.label();
            match noms.iter_mut().find(|n| n.0 == nom) {
                Some(n) => n.1 += 1,
                None => noms.push((nom, 1)),
            }
        }
    }
    noms.into_iter().map(|(n, k)| if k > 1 { format!("{n} × {k}") } else { n.to_string() }).collect()
}
