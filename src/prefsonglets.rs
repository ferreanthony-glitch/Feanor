//! Les onglets de la fenêtre Préférences… (Anthony, 2026-10-21 : « la fenêtre Préférences est très mal
//! organisée et trop verticale »). Ce fichier est la table des réglages : chaque réglage que la fenêtre montre
//! y figure une fois, avec son onglet, son groupe, le texte de son étiquette (celui qu'on lit à l'écran) et des
//! mots pour la recherche. `prefsui.rs` dessine un onglet en parcourant cette table, la recherche la lit, et un
//! essai (`prefsui_tests.rs`) vérifie que chaque réglage se trouve dans un seul onglet, étiquette lisible.
//!
//! Dix onglets, dans une liste verticale à gauche (icône et nom) : une rangée de dix noms ne tiendrait pas dans
//! une fenêtre de largeur raisonnable, et une liste à gauche laisse toute la hauteur au contenu. (« Vignettes » : 2026-10-22,
//! tout ce qui se règle de la planche, avec le décor.)

use egui_phosphor::regular as ph;
use serde::{Deserialize, Serialize};

/// Un onglet de la fenêtre. Retenu dans les réglages (`onglet_preferences`) : la fenêtre rouvre là où on l'a quittée.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Onglet {
    #[default]
    General,
    Apparence,
    Dessin,
    Montage,
    Vignettes,
    Raccourcis,
    Export,
    Equipe,
    Generation,
    Performance,
}

impl Onglet {
    /// Dans l'ordre de la liste.
    pub const TOUS: [Onglet; 10] = [
        Onglet::General,
        Onglet::Apparence,
        Onglet::Dessin,
        Onglet::Montage,
        Onglet::Vignettes,
        Onglet::Raccourcis,
        Onglet::Export,
        Onglet::Equipe,
        Onglet::Generation,
        Onglet::Performance,
    ];

    pub fn est_defaut(&self) -> bool {
        *self == Onglet::default()
    }

    pub fn nom(self) -> &'static str {
        match self {
            Onglet::General => "Général",
            Onglet::Apparence => "Apparence",
            Onglet::Dessin => "Dessin",
            Onglet::Montage => "Montage",
            Onglet::Vignettes => "Vignettes",
            Onglet::Raccourcis => "Raccourcis",
            Onglet::Export => "Export",
            Onglet::Equipe => "Équipe",
            Onglet::Generation => "Génération",
            Onglet::Performance => "Performance",
        }
    }

    pub fn icone(self) -> &'static str {
        match self {
            Onglet::General => ph::GEAR_SIX,
            Onglet::Apparence => ph::PALETTE,
            Onglet::Dessin => ph::PENCIL_SIMPLE,
            Onglet::Montage => ph::FILM_STRIP,
            Onglet::Vignettes => ph::SQUARES_FOUR,
            Onglet::Raccourcis => ph::KEYBOARD,
            Onglet::Export => ph::EXPORT,
            Onglet::Equipe => ph::USERS_THREE,
            Onglet::Generation => ph::SPARKLE,
            Onglet::Performance => ph::GAUGE,
        }
    }

    /// Une phrase sous le titre de l'onglet, et dans la bulle de son entrée.
    pub fn resume(self) -> &'static str {
        match self {
            Onglet::General => "Ouvrir et rouvrir les projets, copies de secours, collage entre projets, conseils.",
            Onglet::Apparence => "Le thème et ses couleurs, la taille de l'interface, la hauteur des panneaux, les images de l'accueil.",
            Onglet::Dessin => "Le curseur du stylet, sa pression, le point fixe des poignées, le passe-partout autour de la caméra et le tremblement du trait par défaut.",
            Onglet::Montage => "Les pistes du montage et leur repliage, la durée d'un nouveau plan.",
            Onglet::Vignettes => "La planche : cases pâles, débord du trait, cadres de caméra, et le décor partagé entre les cases d'un plan.",
            Onglet::Raccourcis => "Le jeu de raccourcis du clavier et ses fichiers.",
            Onglet::Export => "Les programmes et les modèles dont les exports ont besoin.",
            Onglet::Equipe => "Ton nom, ta couleur et la façon de travailler à plusieurs.",
            Onglet::Generation => "Les fournisseurs des sons générés, le serveur local, les clés.",
            Onglet::Performance => "Les effets, les caches et les fils de calcul de cet ordinateur.",
        }
    }
}

/// Un groupe de réglages : un titre court et un bloc dessiné d'un seul tenant (`prefsui.rs`). Un groupe est dans
/// un seul onglet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Groupe {
    Ouverture,
    Secours,
    Collage,
    Theme,
    Panneaux,
    Stylet,
    Camera,
    /// Le tremblement du trait par défaut de tous les calques vectoriels (`tremblement::Defaut`).
    Tremblement,
    Pistes,
    /// La durée d'un plan neuf (`dureeplans.rs`).
    PlansNeufs,
    /// La planche des Vignettes : cases pâles, colonnes, débord du trait (papier infini).
    Vignettes,
    /// Les cadres de caméra dessinés dans les cases : l'aimant et la couleur.
    CadresCamera,
    /// Le décor partagé entre les cases d'un plan (`decor.rs`) : le report et la sorte du calque (les brosses Décor, elles, sont dans le panneau Brosses perso).
    Decor,
    Raccourcis,
    Programmes,
    Commentaires,
    Equipe,
    Generation,
    Performance,
    Affichage,
    /// Taille de l'interface (`echelle.rs`).
    Echelle,
    /// Les images de l'écran d'accueil : le dossier, l'import, la liste (`accueilimages.rs`).
    ImagesAccueil,
    /// Conseils d'un seul passage (`conseils.rs`).
    Conseils,
    /// Régler le stylet à sa main (`reglagestylet.rs`).
    StyletMain,
    /// Les poignées des boîtes de la Sélection et du Transformer : point fixe et centre des rotations (`selbox.rs`).
    Poignees,
}

impl Groupe {
    pub fn titre(self) -> &'static str {
        match self {
            Groupe::Ouverture => "Au lancement et à l'ouverture",
            Groupe::Secours => "En cas de plantage",
            Groupe::Collage => "Coller depuis un autre projet",
            Groupe::Theme => "Thème et couleurs",
            Groupe::Panneaux => "Panneaux",
            Groupe::Stylet => "Stylet et pinceau",
            Groupe::Camera => "Caméra",
            Groupe::Tremblement => "Tremblement des traits",
            Groupe::Pistes => "Pistes du montage",
            Groupe::PlansNeufs => "Plans neufs",
            Groupe::Vignettes => "Planche des Vignettes",
            Groupe::CadresCamera => "Cadres de caméra",
            Groupe::Decor => "Décor",
            Groupe::Raccourcis => "Raccourcis clavier",
            Groupe::Programmes => "Programmes et modèles",
            Groupe::Commentaires => "Commentaires",
            Groupe::Equipe => "Travail à plusieurs",
            Groupe::Generation => "Génération sonore",
            Groupe::Performance => "Réglages de performance",
            Groupe::Affichage => "Affichage",
            Groupe::Echelle => "Taille de l'interface",
            Groupe::ImagesAccueil => "Images d'accueil",
            Groupe::Conseils => "Conseils",
            Groupe::StyletMain => "Stylet à ma main",
            Groupe::Poignees => "Poignées de la Sélection et du Transformer",
        }
    }
}

/// Un réglage montré par la fenêtre.
#[derive(Clone, Copy, Debug)]
pub struct Reglage {
    pub onglet: Onglet,
    pub groupe: Groupe,
    /// Identifiant stable (`onglet.nom`), lu par les essais.
    #[cfg_attr(not(test), allow(dead_code))]
    pub id: &'static str,
    /// Le début du texte qu'on lit à l'écran devant (ou sur) le réglage.
    pub libelle: &'static str,
    /// Mots en plus, pour la recherche.
    pub mots: &'static str,
}

const fn r(onglet: Onglet, groupe: Groupe, id: &'static str, libelle: &'static str, mots: &'static str) -> Reglage {
    Reglage { onglet, groupe, id, libelle, mots }
}

use Groupe as G;
use Onglet as O;

/// Tous les réglages de la fenêtre, onglet par onglet, groupe par groupe, dans l'ordre où ils se lisent.
pub const REGLAGES: &[Reglage] = &[
    // ---- Général
    r(O::General, G::Ouverture, "general.rouvrir", "Rouvrir le dernier projet au lancement", "démarrage session onglets rouvrir"),
    r(O::General, G::Ouverture, "general.nouvel_onglet", "Ouvrir dans un nouvel onglet", "projet récent importer nouveau remplacer"),
    r(O::General, G::Ouverture, "general.nouveau_sur", "Un nouveau projet s'ouvre sur", "vignettes storyboard planche timeline page espace créer nouveau projet accueil"),
    r(O::General, G::Secours, "general.secours", "Copie de secours toutes les", "sauvegarde automatique autosave plantage minutes"),
    r(O::General, G::Collage, "collage.proportions", "Proportions différentes", "contenir remplir taille"),
    r(O::General, G::Collage, "collage.cadence", "Cadence différente", "images secondes durée fps"),
    r(O::General, G::Collage, "collage.demander", "Demander à chaque collage", "question fenêtre"),
    r(O::General, G::Conseils, "general.conseils", "Revoir les conseils", "bulles aide premiers pas astuces"),
    // ---- Apparence
    r(O::Apparence, G::Theme, "theme.choix", "Thème", "gris sombre clair personnalisé rétablir"),
    r(O::Apparence, G::Theme, "theme.panneau", "Fond des panneaux", "couleur"),
    r(O::Apparence, G::Theme, "theme.barres", "Barres", "couleur onglets outils règle"),
    r(O::Apparence, G::Theme, "theme.fenetres", "Fenêtres et menus", "couleur listes"),
    r(O::Apparence, G::Theme, "theme.boutons", "Boutons et curseurs", "couleur cases"),
    r(O::Apparence, G::Theme, "theme.texte", "Texte", "couleur police"),
    r(O::Apparence, G::Theme, "theme.accent", "Accent et sélection", "couleur poignées"),
    r(O::Apparence, G::Theme, "theme.autour", "Autour du dessin", "couleur fond feuille"),
    r(O::Apparence, G::Theme, "theme.timeline", "Timeline et montage", "couleur rangées"),
    r(O::Apparence, G::Theme, "theme.bordures", "Bordures", "couleur traits quadrillage"),
    r(O::Apparence, G::Echelle, "apparence.echelle", "Échelle", "taille interface zoom agrandir réduire texte boutons 90 100 125 150 pour cent"),
    r(O::Apparence, G::Panneaux, "panneaux.ajuster", "Ajuster les panneaux à leur contenu", "hauteur fiche guides effets réglages"),
    r(O::Apparence, G::Panneaux, "panneaux.reajuster", "Réajuster les panneaux", "hauteur"),
    r(O::Apparence, G::ImagesAccueil, "accueil.dossier", "Dossier des images d'accueil", "écran d'accueil logo image storyboard choisir changer"),
    r(O::Apparence, G::ImagesAccueil, "accueil.importer", "Importer des images", "copier ajouter png jpeg jpg photo écran d'accueil"),
    r(O::Apparence, G::ImagesAccueil, "accueil.liste", "Images du dossier", "retirer supprimer miniatures liste écran d'accueil"),
    // ---- Dessin
    r(O::Dessin, G::Stylet, "dessin.croix", "Croix du système sur le pinceau", "curseur stylet retard pointe"),
    r(O::Dessin, G::StyletMain, "dessin.regler_stylet", "Régler le stylet à ma main", "pression courbe appui tablette léger moyen fort toucher"),
    r(O::Dessin, G::Poignees, "dessin.point_fixe", "Point fixe des poignées", "ancre poignée opposée centre échelle incliner coin côté transformer sélection boîte ctrl"),
    r(O::Dessin, G::Poignees, "dessin.rotation_autour", "Rotation autour de", "pivot coin opposé tourner rotation transformer sélection boîte"),
    r(O::Dessin, G::Camera, "dessin.passe_partout_couleur", "Couleur du passe-partout", "noir hors-champ voile cadre caméra"),
    r(O::Dessin, G::Camera, "dessin.passe_partout", "Opacité du passe-partout", "voile hors-champ estomper cadre caméra rétablir noir opaque"),
    r(O::Dessin, G::Camera, "dessin.cadre_format_couleur", "Cadre de format", "cadre caméra format 16:9 rose gris cadrage couleur"),
    r(O::Dessin, G::Tremblement, "dessin.tremblement_defaut", "Tremblement du trait par défaut", "wiggle boil vectoriel tous les calques trait qui bouge allumer"),
    r(O::Dessin, G::Tremblement, "dessin.tremblement_intensite", "Intensité du tremblement par défaut", "amplitude pixels"),
    r(O::Dessin, G::Tremblement, "dessin.tremblement_frequence", "Fréquence du tremblement par défaut", "doux nerveux vagues rétablir"),
    r(O::Dessin, G::Tremblement, "dessin.tremblement_variantes", "Variantes du tremblement par défaut", "dessins différents boil nombre aperçu"),
    r(O::Dessin, G::Tremblement, "dessin.tremblement_vitesse", "Vitesse du tremblement par défaut", "images par variante cadence lent rapide"),
    r(O::Dessin, G::Tremblement, "dessin.tremblement_epaisseur", "Épaisseur du tremblement par défaut", "trait varie pour cent bord"),
    r(O::Dessin, G::Tremblement, "dessin.tremblement_graine", "Graine du tremblement par défaut", "hasard autre tremblement numéro"),
    r(O::Dessin, G::Tremblement, "dessin.tremblement_repos", "Animer à l'arrêt (tremblement par défaut)", "image fixe immobile bouge repos aperçu"),
    // ---- Montage
    r(O::Montage, G::Pistes, "montage.repliage", "Calques non actifs", "replier cacher calques actif bande autres"),
    r(O::Montage, G::Pistes, "montage.deplier", "Déplier le calque courant tout seul", "piste automatique"),
    r(O::Montage, G::PlansNeufs, "montage.duree_nouveau_plan", "Durée d'un nouveau plan", "secondes images deux défaut créer plan neuf long court"),
    // ---- Vignettes
    r(O::Vignettes, G::Vignettes, "vignettes.rangees_pales", "Rangées de cases pâles", "vignettes planche papier cases vides dessiner minimum lignes"),
    r(O::Vignettes, G::Vignettes, "vignettes.colonnes", "Cases par rangée", "vignettes planche colonnes largeur six"),
    r(O::Vignettes, G::Vignettes, "vignettes.debord", "Débord du trait hors des cases", "marge anneau déborde dépasse cadre pour cent"),
    r(O::Vignettes, G::CadresCamera, "vignettes.aimant", "Aimant des cadres de caméra", "alignement colonnes rangées points magnétique éteindre zéro"),
    r(O::Vignettes, G::CadresCamera, "vignettes.couleur_cadre", "Couleur des cadres de caméra", "bleu lettres flèche empreinte export"),
    r(O::Vignettes, G::Decor, "vignettes.report_decor", "Report du décor", "continuité coupe plan suivant case réunir décor partagé"),
    r(O::Vignettes, G::Decor, "vignettes.calque_decor", "Calque Décor", "vectoriel bitmap pixels sorte créer brosses du décor pastilles"),
    r(O::Vignettes, G::Decor, "vignettes.brosses_decor", "Brosses Décor", "couleur trait aplat taille crayon feutre pinceau plein pixels panneau brosses perso pastilles"),
    // ---- Raccourcis
    r(O::Raccourcis, G::Raccourcis, "raccourcis.jeu", "Jeu de raccourcis", "clavier storyboard pro touches"),
    r(O::Raccourcis, G::Raccourcis, "raccourcis.liste", "Voir et changer les raccourcis", "assigner touche"),
    r(O::Raccourcis, G::Raccourcis, "raccourcis.fichier", "Modifier le fichier à la main", "toml perso"),
    r(O::Raccourcis, G::Raccourcis, "raccourcis.recharger", "Recharger", "fichier"),
    // ---- Export
    r(O::Export, G::Programmes, "export.ffmpeg", "ffmpeg", "vidéo gif encodeur film son"),
    r(O::Export, G::Programmes, "export.modele_sbp", "Modèle Storyboard Pro", "sboard export projet vide"),
    r(O::Export, G::Programmes, "export.fenetre", "Ouvrir la fenêtre Exporter", "film pdf png gif"),
    // ---- Équipe
    r(O::Equipe, G::Commentaires, "equipe.signature", "Signer mes commentaires", "auteur nom timeline"),
    r(O::Equipe, G::Equipe, "equipe.nom", "Mon nom", "identité couleur équipier coopération"),
    r(O::Equipe, G::Equipe, "equipe.bail", "Une prise périme après", "plan tenu verrou minutes"),
    r(O::Equipe, G::Equipe, "equipe.prise_auto", "Prendre le plan libre que je commence à modifier", "automatique prise"),
    r(O::Equipe, G::Equipe, "equipe.notifications", "Dire dans la barre d'état ce que font les autres", "notifications état"),
    // ---- Génération
    r(O::Generation, G::Generation, "generation.lecture", "Lecture du storyboard", "fournisseur claude règles local en ligne suggestions"),
    r(O::Generation, G::Generation, "generation.son", "Bruitages et ambiances", "fournisseur modèle local en ligne elevenlabs"),
    r(O::Generation, G::Generation, "generation.voix", "Voix témoin des dialogues", "fournisseur voix elevenlabs"),
    r(O::Generation, G::Generation, "generation.dossier", "Dossier des modèles", "serveur local python modèles"),
    r(O::Generation, G::Generation, "generation.python", "Python du serveur", "serveur local venv"),
    r(O::Generation, G::Generation, "generation.demarrage", "Démarrer le serveur tout seul", "serveur local automatique"),
    r(O::Generation, G::Generation, "generation.port", "Adresse 127.0.0.1, port", "serveur local réseau"),
    // ---- Performance
    r(O::Performance, G::Performance, "performance.effets_reduits", "Effets en aperçu réduit", "lecture gestes lent"),
    r(O::Performance, G::Performance, "performance.traits_textures", "Traits vectoriels texturés", "rendu trait texture"),
    r(O::Performance, G::Performance, "performance.tremblement", "Tremblement du trait", "animer à l'arrêt"),
    r(O::Performance, G::Performance, "performance.pelure", "Pelure d'oignon", "images voisines plafond"),
    r(O::Performance, G::Performance, "performance.cache_carte", "Cache de la carte graphique", "mémoire gpu mo"),
    r(O::Performance, G::Performance, "performance.cache_traits", "Cache des traits vectoriels", "mémoire mo"),
    r(O::Performance, G::Performance, "performance.cache_effets", "Cache des effets", "mémoire mo"),
    r(O::Performance, G::Performance, "performance.fils", "Fils de calcul", "processeur threads"),
    r(O::Performance, G::Performance, "performance.vignettes", "Vignettes", "miniatures"),
    r(O::Performance, G::Performance, "performance.animations", "Animations", "interface"),
    r(O::Performance, G::Performance, "performance.effets_gpu", "Effets sur la carte graphique", "flous lueurs bruit gpu"),
    r(O::Performance, G::Performance, "performance.diagnostic", "Diagnostic des performances", "détecter mesurer profil valeurs d'origine"),
    r(O::Performance, G::Affichage, "performance.affichage", "Affichage", "carte graphique moteur"),
];

/// Les groupes d'un onglet, dans l'ordre de la table, chacun une fois.
pub fn groupes_de(onglet: Onglet) -> Vec<Groupe> {
    let mut v: Vec<Groupe> = Vec::new();
    for g in REGLAGES.iter().filter(|r| r.onglet == onglet).map(|r| r.groupe) {
        if !v.contains(&g) {
            v.push(g);
        }
    }
    v
}

/// Minuscules sans accents, pour chercher « couleur » comme « Couleur » et « theme » comme « Thème ».
pub fn plier(texte: &str) -> String {
    texte
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' | 'ä' => 'a',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// Ce que la recherche trouve : pour chaque groupe qui répond, ses réglages qui répondent (ou, si seul le titre du
/// groupe ou de l'onglet répond, tous les réglages du groupe).
pub struct Trouve {
    pub onglet: Onglet,
    pub groupe: Groupe,
    pub libelles: Vec<&'static str>,
}

/// Cherche `requete` (tous ses mots, sans tenir compte des majuscules ni des accents) dans les étiquettes, les mots
/// ajoutés, les titres de groupe et les noms d'onglet. Une requête vide ne trouve rien.
pub fn chercher(requete: &str) -> Vec<Trouve> {
    let mots: Vec<String> = plier(requete).split_whitespace().map(str::to_string).collect();
    if mots.is_empty() {
        return Vec::new();
    }
    let tous = |texte: &str| {
        let t = plier(texte);
        mots.iter().all(|m| t.contains(m.as_str()))
    };
    let mut trouves: Vec<Trouve> = Vec::new();
    for onglet in Onglet::TOUS {
        for groupe in groupes_de(onglet) {
            let du_groupe: Vec<&Reglage> = REGLAGES.iter().filter(|r| r.groupe == groupe).collect();
            let contexte = format!("{} {}", onglet.nom(), groupe.titre());
            let propres: Vec<&'static str> = du_groupe.iter().filter(|r| tous(&format!("{} {}", r.libelle, r.mots))).map(|r| r.libelle).collect();
            let libelles = if !propres.is_empty() {
                propres
            } else if tous(&contexte) {
                du_groupe.iter().map(|r| r.libelle).collect()
            } else {
                // Les mots peuvent se partager entre le contexte et l'étiquette (« équipe prise »).
                du_groupe.iter().filter(|r| tous(&format!("{contexte} {} {}", r.libelle, r.mots))).map(|r| r.libelle).collect()
            };
            if !libelles.is_empty() {
                trouves.push(Trouve { onglet, groupe, libelles });
            }
        }
    }
    trouves
}
