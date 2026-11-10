//! Barres de commandes à l'étroit (INTERFACE.md, 6.3) : la barre de commandes du haut, celle du
//! montage et celle de la timeline ne montrent d'office que l'essentiel (9, 12 et une vingtaine de
//! commandes au lieu de 19, 21 et une trentaine) ; le reste est sous un bouton « Plus… » (`DOTS_THREE`)
//! au bout de la barre. Rien n'est retiré : chaque commande a son action (menus, palette Ctrl+K,
//! raccourcis), et dans « Plus… » une ligne avec son icône, son nom et son raccourci.
//!
//! - **Épingler** : chaque ligne de « Plus… » a une épingle (« Épingler dans la barre ») qui remet la
//!   commande dans la barre ; le sous-menu « Dans la barre » permet l'inverse (retirer une commande
//!   montrée d'office). Le choix est retenu par barre dans les réglages (`Settings::bars`) ; le
//!   fichier ne garde que l'écart avec les commandes d'origine.
//! - **Jamais de défilement** : une barre ne défile plus. Si la place manque, les dernières commandes
//!   de la ligne passent d'elles-mêmes sous « Plus… » (elles restent épinglées) et reviennent quand la
//!   fenêtre s'élargit.
//! - Les icônes allumées prennent l'encre lisible sur l'accent (`bulles::symbole_etat`, `theme::sur_accent`).
//!
//! Les barres sont des tableaux de `Cmd` (une ligne par commande, dans l'ordre d'affichage) ; ce
//! qu'une commande fait vient de `executer_commande`, ce qu'elle montre de `vue_de`.

use std::sync::OnceLock;

use eframe::egui::{self, vec2, RichText, Ui};
use egui_phosphor::regular as ph;
use serde::{Deserialize, Serialize};

use crate::app::{App, Mode};
use crate::bulles::{self, bulle};

/// Les quatre barres qui ont un « Plus… ». (La barre des panneaux, à droite, n'en a pas.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Barre {
    /// Sous les menus.
    Commandes,
    /// En tête de la bande du montage (espaces Storyboard).
    Montage,
    /// En tête de la bande du montage dans l'espace Montage : les commandes du travail de montage
    /// (coupes, rognage, transitions, son, relecture, exports) y sont montrées d'office.
    MontageEdition,
    /// En tête de la timeline.
    Timeline,
}

impl Barre {
    #[cfg(test)]
    pub const TOUTES: [Barre; 4] = [Barre::Commandes, Barre::Montage, Barre::MontageEdition, Barre::Timeline];

    pub fn nom(self) -> &'static str {
        match self {
            Barre::Commandes => "Barre de commandes",
            Barre::Montage => "Barre du montage",
            Barre::MontageEdition => "Barre de l'espace Montage",
            Barre::Timeline => "Barre de la timeline",
        }
    }

    /// Les commandes de la barre, dans l'ordre où elles s'affichent.
    pub fn commandes(self) -> &'static [Cmd] {
        match self {
            Barre::Commandes => COMMANDES,
            Barre::Montage => MONTAGE,
            Barre::MontageEdition => montage_edition(),
            Barre::Timeline => TIMELINE,
        }
    }

    pub fn cherche(self, id: &str) -> Option<&'static Cmd> {
        self.commandes().iter().find(|c| c.id == id)
    }

    /// Une des deux barres de la bande du montage (celle des espaces Storyboard ou celle de l'espace
    /// Montage) : leurs commandes de plans agissent de la même façon.
    pub fn est_du_montage(self) -> bool {
        matches!(self, Barre::Montage | Barre::MontageEdition)
    }

    /// La barre qui coiffe la bande du montage dans cet espace de travail.
    pub fn du_montage_dans(espace: crate::workspace::Espace) -> Barre {
        if espace == crate::workspace::Espace::Montage {
            Barre::MontageEdition
        } else {
            Barre::Montage
        }
    }
}

/// Ce que montre une ligne d'une barre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Genre {
    /// Un bouton icône qui lance son action.
    Bouton,
    /// Un bouton icône qui montre un état (enfoncé quand il est vrai).
    Etat,
    /// Un bouton icône qui ouvre un menu.
    Menu,
    /// Une valeur qu'on règle (un champ qu'on glisse ou qu'on tape).
    Reglage,
    /// Un affichage qui reste dans la barre (la position, le titre du plan…) : ni épingle ni « Plus… ».
    Fixe,
}

/// Une commande d'une barre.
#[derive(Clone, Copy)]
pub struct Cmd {
    /// Son identifiant dans la barre (l'action quand elle n'a qu'une icône) : celui des réglages.
    pub id: &'static str,
    pub genre: Genre,
    pub icone: &'static str,
    pub nom: &'static str,
    pub description: &'static str,
    /// Son action (`keymap::ACTIONS`), vide pour un menu, un réglage ou un affichage.
    pub action: &'static str,
    /// Dans la barre sans que l'utilisateur l'ait épinglée.
    pub d_office: bool,
    /// Les commandes d'un même groupe se suivent ; un trait sépare deux groupes.
    pub groupe: u8,
    /// Place qu'elle prend à l'écran (points), pour décider ce qui passe sous « Plus… » quand la
    /// barre est trop étroite.
    pub largeur: f32,
}

const fn cmd(id: &'static str, genre: Genre, icone: &'static str, nom: &'static str, description: &'static str, action: &'static str, d_office: bool, groupe: u8) -> Cmd {
    let largeur = match genre {
        Genre::Reglage => 62.0,
        Genre::Fixe => 0.0,
        _ => 30.0,
    };
    Cmd { id, genre, icone, nom, description, action, d_office, groupe, largeur }
}

const fn bouton(action: &'static str, icone: &'static str, nom: &'static str, description: &'static str, d_office: bool, groupe: u8) -> Cmd {
    cmd(action, Genre::Bouton, icone, nom, description, action, d_office, groupe)
}

const fn etat(action: &'static str, icone: &'static str, nom: &'static str, description: &'static str, d_office: bool, groupe: u8) -> Cmd {
    cmd(action, Genre::Etat, icone, nom, description, action, d_office, groupe)
}

const fn menu(id: &'static str, icone: &'static str, nom: &'static str, description: &'static str, d_office: bool, groupe: u8) -> Cmd {
    cmd(id, Genre::Menu, icone, nom, description, "", d_office, groupe)
}

const fn reglage(id: &'static str, icone: &'static str, nom: &'static str, description: &'static str, d_office: bool, groupe: u8) -> Cmd {
    cmd(id, Genre::Reglage, icone, nom, description, "", d_office, groupe)
}

const fn fixe(id: &'static str, groupe: u8) -> Cmd {
    cmd(id, Genre::Fixe, "", "", "", "", true, groupe)
}

/// Barre de commandes : les neuf icônes du fichier et de l'édition. Les plans et le transport n'y sont plus, même
/// sous « Plus… » (simplification des barres, 2026-10-21) : ils sont dans la barre du montage et celle de la
/// timeline, et toujours à la palette (Ctrl+K) et aux menus. Une commande a un seul endroit principal
/// (`docs/DOUBLONS.md`).
pub(crate) const COMMANDES: &[Cmd] = &[
    bouton("fichier.nouveau", ph::FILE_PLUS, "Nouveau projet", "Commencer un projet vide.", true, 0),
    bouton("fichier.ouvrir", ph::FOLDER_OPEN, "Ouvrir", "Ouvrir un projet FEANOR.", true, 0),
    bouton("fichier.enregistrer", ph::FLOPPY_DISK, "Enregistrer", "Enregistrer le projet. Quand FEANOR l'écrit en continu : poser un jalon, un moment auquel revenir.", true, 0),
    bouton("fichier.exporter_film", ph::EXPORT, "Exporter le film", "Exporter le montage.", true, 0),
    bouton("edition.annuler", ph::ARROW_COUNTER_CLOCKWISE, "Annuler", "Défaire la dernière opération.", true, 1),
    bouton("edition.retablir", ph::ARROW_CLOCKWISE, "Rétablir", "Refaire ce qui vient d'être annulé.", true, 1),
    bouton("edition.couper", ph::SCISSORS, "Couper", "Couper la sélection.", true, 1),
    bouton("edition.copier", ph::COPY, "Copier", "Copier la sélection.", true, 1),
    bouton("edition.coller", ph::CLIPBOARD, "Coller", "Coller ce qui a été copié.", true, 1),
];

/// Barre du montage : la lecture, un plan ou une séquence de plus, les menus Plans et Son, entrer
/// dans le plan ; le reste sous « Plus… ». (Les vues dessin et vignettes ne sont plus là : Maj+Tab, et
/// l'onglet Vignettes.)
pub(crate) const MONTAGE: &[Cmd] = &[
    bouton("montage.debut", ph::SKIP_BACK, "Début du montage", "Revenir à la première image du film.", true, 0),
    bouton("lecture.basculer", ph::PLAY, "Lire le montage / pause", "Tout le film à la suite (animatique), avec le son.", true, 0),
    etat("lecture.boucle", ph::REPEAT, "Lecture en boucle", "Reprendre au début une fois à la fin.", true, 0),
    etat("son.lecture", ph::SPEAKER_HIGH, "Son pendant la lecture", "Entendre les pistes son en lisant.", false, 0),
    fixe("position", 0),
    bouton("panneau.nouveau", ph::PLUS_SQUARE, "Nouveau plan", "Après le plan courant, dans la même séquence.", true, 1),
    bouton("sequence.nouvelle", ph::STACK_PLUS, "Nouvelle séquence", "Après la séquence courante, avec un plan.", true, 1),
    bouton("calque.nouveau_vectoriel", ph::BEZIER_CURVE, "Nouveau calque vectoriel", "Dans le plan courant, au-dessus du calque courant : une piste de plus.", false, 1),
    bouton("edition.copier", ph::COPY, "Copier les plans", "Le plan courant ; Ctrl ou Maj + clic pour en choisir plusieurs.", false, 1),
    bouton("edition.coller", ph::CLIPBOARD, "Coller les plans", "Après le plan courant, dans sa séquence.", false, 1),
    bouton("edition.supprimer", ph::TRASH, "Supprimer les plans", "Le plan courant ; Ctrl ou Maj + clic pour en choisir plusieurs.", false, 1),
    menu("plans", ph::LIST_BULLETS, "Plans", "Dupliquer, copier, couper, réunir, marques d'image.", true, 1),
    bouton("plan.avancer", ph::CARET_DOUBLE_LEFT, "Avancer le plan", "Le placer plus tôt dans le montage.", false, 1),
    bouton("plan.reculer", ph::CARET_DOUBLE_RIGHT, "Reculer le plan", "Le placer plus tard dans le montage.", false, 1),
    etat(
        "storyboard.verrouiller",
        ph::LOCK_OPEN,
        "Verrouiller le storyboard",
        "Verrouillé : l'ordre, le nombre et la durée des plans, leurs dessins et leurs calques ne changent plus.",
        false,
        1,
    ),
    // Numérotation des plans (`numerotation.rs`) : verrouillée, chaque plan garde son numéro (12, 12A, 13).
    etat(
        "numerotation.verrouiller",
        ph::LIST_NUMBERS,
        "Verrouiller la numérotation",
        "Verrouillée : chaque plan garde son numéro. Inséré entre le 12 et le 13, un plan devient le 12A ; supprimé, il laisse un trou ; déplacé, il garde son numéro.",
        false,
        1,
    ),
    bouton(
        "numerotation.renumeroter",
        ph::HASH,
        "Renuméroter les plans",
        "Remet des numéros propres, 1, 2, 3… dans l'ordre du film (numérotation verrouillée ; après confirmation, une étape d'annulation).",
        false,
        1,
    ),
    etat(
        "montage.aimanter",
        ph::MAGNET,
        "Aimantation des bords",
        "Rogner un plan à la souris : le bord se colle à la tête de lecture, aux autres coupes et aux clés de caméra et d'effets. Éteint, il suit la souris à l'image près.",
        false,
        1,
    ),
    // Pistes repliables (`pistes.rs`) : tout replier en bandes fines, tout déplier, et le calque courant qui
    // se déplie tout seul.
    bouton(
        "piste.replier_tout",
        ph::ARROWS_IN_LINE_VERTICAL,
        "Replier toutes les pistes",
        "Tous les calques en bandes fines, même le calque actif : le nom, les dessins et les clés restent visibles. Double-clic sur un calque : le déplier seul.",
        false,
        1,
    ),
    bouton("piste.deplier_tout", ph::ARROWS_OUT_LINE_VERTICAL, "Déplier toutes les pistes", "Toutes les pistes de calques à leur pleine hauteur (les dossiers s'ouvrent). Un clic sur un autre calque les replie de nouveau.", false, 1),
    etat(
        "piste.auto_deplier",
        ph::CURSOR_CLICK,
        "Déplier le calque courant",
        "Allumé, seul le calque actif est déplié, dès que la souris est relâchée. Éteint, tous les calques restent en bandes fines tant qu'on ne les déplie pas.",
        false,
        1,
    ),
    // Une transition à la coupe la plus proche de la tête de lecture (`transitionsui.rs`) ; le clic
    // droit sur une coupe, dans la bande, en choisit la sorte.
    bouton(
        "transition.ajouter",
        ph::GRADIENT,
        "Ajouter une transition",
        "À la coupe la plus proche de la tête de lecture : un fondu enchaîné entre deux plans, un fondu au noir au bord du film. Clic droit sur une coupe pour choisir la sorte.",
        false,
        1,
    ),
    // Les envois (`envoisui.rs`) : noter l'état de tous les plans quand le film part, repérer ensuite
    // ceux qui ont changé.
    bouton(
        "envoi.marquer",
        ph::PAPER_PLANE_TILT,
        "Marquer un envoi…",
        "Noter l'état de tous les plans (par exemple « v2 au réal ») : ceux qui changent ensuite sont repérés dans la bande et sur la planche.",
        false,
        1,
    ),
    etat("envoi.liste", ph::GIT_DIFF, "Envois", "La liste des envois et des plans modifiés depuis le dernier.", false, 1),
    bouton("envoi.journal", ph::ARROWS_LEFT_RIGHT, "Journal des différences", "Ce qui a changé entre deux envois : les plans modifiés avant / après, ajoutés, retirés, déplacés, et les notes traitées. En PDF et en page HTML.", false, 1),
    bouton("envoi.modifie_precedent", ph::ARROW_FAT_LEFT, "Plan modifié précédent", "Aller au plan modifié avant le plan courant, depuis le dernier envoi.", false, 1),
    bouton("envoi.modifie_suivant", ph::ARROW_FAT_RIGHT, "Plan modifié suivant", "Aller au plan modifié après le plan courant, depuis le dernier envoi.", false, 1),
    // Les versions du plan courant (`planversui.rs`).
    bouton("version.nouvelle", ph::GIT_FORK, "Nouvelle version du plan", "Garde le plan tel qu'il est comme version plus ancienne (V1) et continue sur une copie (V2). Les versions se changent en survolant « V2 » sur le plan, dans le montage.", false, 1),
    etat("comparer.ouvrir", ph::COLUMNS, "Comparer le plan", "Deux versions du plan, ou le plan d'aujourd'hui et son état à un envoi : côte à côte, en volet, en fondu ou en superposition. Lecture seule.", false, 1),
    bouton("fichier.exporter_montage", ph::FILM_REEL, "Exporter pour le montage", "Un fichier vidéo par plan et le montage en EDL, XML de montage ou OpenTimelineIO.", false, 2),
    bouton("fichier.exporter_fiche", ph::TABLE, "Exporter la fiche technique", "Le découpage du film en tableau (CSV ou classeur XLSX) : un plan par ligne, avec durées, time codes, cadrage, mouvement de caméra, textes, sons et vignettes.", false, 2),
    // Relecture par le réalisateur (`relectureui.rs`) : la page à envoyer, puis ses notes qui reviennent.
    bouton("fichier.exporter_relecture", ph::EYEGLASSES, "Exporter pour relecture", "Une page sans internet à envoyer au réalisateur : il y écrit ses notes et dessine dessus, puis vous renvoie un petit fichier.", false, 2),
    bouton("fichier.importer_notes", ph::CHATS, "Importer les notes de relecture", "Le fichier de notes renvoyé par le réalisateur devient des commentaires sur les bons plans. On peut aussi le lâcher sur la fenêtre.", false, 2),
    bouton("son.importer", ph::MUSIC_NOTES_PLUS, "Importer un son", "WAV, MP3, FLAC ou OGG, à l'image courante ; on peut aussi lâcher le fichier sur la fenêtre.", true, 2),
    menu("son", ph::SPEAKER_HIGH, "Son", "Pistes son, son au défilement et pendant la lecture.", true, 2),
    bouton(
        "fichier.importer_video",
        ph::FILE_VIDEO,
        "Importer une vidéo",
        "Une vidéo devient un calque vidéo du plan courant, image par image (à dessiner par-dessus, ou à transformer en calque d'animation).",
        false,
        2,
    ),
    bouton(
        "commentaire.ajouter",
        ph::CHAT_CENTERED_TEXT,
        "Nouveau commentaire",
        "Laisser un commentaire à l'image courante : un fanion dans la règle (à déplacer), qui s'écrit dans le panneau Commentaires. Ou double-clic dans la règle.",
        false,
        3,
    ),
    // (Le double-clic sur le plan fait la même chose ; il n'y a plus d'onglet Animation.)
    bouton("mode.basculer", ph::FILM_SLATE, "Animer ce plan", "Entrer dans le plan courant pour l'animer (ou double-clic sur le plan).", false, 4),
];

/// Barre de la timeline : le plan, le transport, les dessins et la caméra ; les dessins qu'on retire ou vide, la
/// cadence, la marque, le commentaire et les flips sous « Plus… ». Ni la pelure d'oignon (sa place est la barre sous
/// le dessin, qui la règle d'un clic) ni le panneau Effets (sa place est la barre des panneaux) ; l'onglet Animation
/// montre le plan : « Retour au storyboard » est dans « Plus… ».
pub(crate) const TIMELINE: &[Cmd] = &[
    bouton("mode.basculer", ph::FILM_SLATE, "Animer ce plan", "Le dessin montre ce plan pour l'animer.", false, 0),
    fixe("plan", 0),
    bouton("image.premiere", ph::SKIP_BACK, "Première image", "Première image du plan.", true, 1),
    bouton("image.precedente", ph::CARET_LEFT, "Image précédente", "Reculer d'une image.", true, 1),
    bouton("lecture.basculer", ph::PLAY, "Lecture / pause", "Lire le plan.", true, 1),
    bouton("image.suivante", ph::CARET_RIGHT, "Image suivante", "Avancer d'une image.", true, 1),
    bouton("image.derniere", ph::SKIP_FORWARD, "Dernière image", "Dernière image du plan.", true, 1),
    etat("lecture.boucle", ph::REPEAT, "Lecture en boucle", "Reprendre au début une fois à la fin.", true, 1),
    reglage("cadence", ph::GAUGE, "Cadence", "Images par seconde, pour tout le projet.", false, 1),
    fixe("image_courante", 1),
    menu("reperes", ph::BOOKMARK_SIMPLE, "Repères", "Signets, points d'entrée et de sortie, boucle, numéros (image, time code, trame), image de départ.", true, 1),
    menu("affichage", ph::MAGNIFYING_GLASS, "Affichage de la timeline", "Zoom (aussi Ctrl + molette), Adapter, croix des instances vides, miniatures.", true, 1),
    bouton("dessin.nouveau", ph::FILE_PLUS, "Nouveau dessin", "Un dessin vide après l'exposition courante.", true, 2),
    bouton("dessin.dupliquer", ph::COPY, "Dupliquer le dessin", "Une copie du dessin courant, après lui.", false, 2),
    bouton("exposition.allonger", ph::PLUS, "Allonger l'exposition", "Tenir le dessin une image de plus.", true, 2),
    bouton("exposition.raccourcir", ph::MINUS, "Raccourcir l'exposition", "Tenir le dessin une image de moins.", true, 2),
    bouton("dessin.supprimer", ph::TRASH, "Supprimer le dessin", "Retirer le dessin et ses expositions de la timeline.", false, 2),
    bouton("dessin.vider", ph::ERASER, "Vider le dessin", "Effacer tout ce qu'il contient ; il reste exposé.", false, 2),
    reglage("exposition_defaut", ph::HOURGLASS, "Exposition des nouveaux dessins", "Nombre d'images que tient chaque nouveau dessin (« en 1 », « en 2 »…).", true, 2),
    menu(
        "images",
        ph::FILM_STRIP,
        "Images",
        "Supprimer, couper, copier, coller, dupliquer, inverser, insérer… Maj + clic dans la grille pour choisir des images.",
        true,
        2,
    ),
    bouton("camera.cle_debut", ph::DIAMOND, "Clé de caméra au début", "Une clé de caméra sur la première image du plan.", true, 3),
    bouton("camera.cle_ici", ph::DIAMOND, "Clé de caméra ici", "Une clé de caméra à l'image courante.", true, 3),
    bouton("camera.cle_fin", ph::DIAMOND, "Clé de caméra à la fin", "Une clé de caméra sur la dernière image du plan.", true, 3),
    bouton("camera.cle_supprimer", ph::X_CIRCLE, "Supprimer la clé de caméra", "Supprimer la clé de caméra de cette image.", true, 3),
    etat(
        "camera.cle_adoucir",
        ph::WAVE_SINE,
        "Adoucir",
        "Pose un lissage de chaque côté de la clé : deux petits ronds creux apparaissent sur la piste de caméra, à tirer pour régler où il commence et où il finit. Un second clic le retire.",
        false,
        3,
    ),
    etat(
        "marque.basculer",
        ph::FLAG_PENNANT,
        "Marque d'image",
        "Une image de storyboard de plus dans le plan, à l'image courante (vignettes, PDF). Clic : poser ou retirer.",
        false,
        4,
    ),
    bouton(
        "commentaire.ajouter",
        ph::CHAT_CENTERED_TEXT,
        "Nouveau commentaire",
        "Laisser un commentaire à l'image courante : un fanion dans la règle (à déplacer), qui s'écrit dans le panneau Commentaires. Ou double-clic dans la règle.",
        false,
        4,
    ),
    // La pelure d'oignon est réglée sous le dessin (`viewbar.rs`) : ici, sous « Plus… » seulement.
    etat("vue.pelure", ph::CIRCLES_THREE, "Pelure d'oignon", "Montrer les dessins d'avant et d'après.", false, 5),
    reglage("pelure_avant", ph::CARET_DOUBLE_LEFT, "Pelure : dessins avant", "Nombre de dessins (ou d'images) montrés avant.", false, 5),
    reglage("pelure_apres", ph::CARET_DOUBLE_RIGHT, "Pelure : dessins après", "Nombre de dessins (ou d'images) montrés après.", false, 5),
    etat(
        "pelure.panneau",
        ph::GEAR,
        "Réglages de la pelure d'oignon",
        "Mode, devant, couleurs, chaque image et son opacité, réglages rangés, décalage.",
        false,
        5,
    ),
    etat("flip.panneau", ph::BOOK_OPEN, "Flips", "Feuilleter vite des images pour juger un mouvement.", false, 6),
    etat("son.lecture", ph::SPEAKER_HIGH, "Son pendant la lecture", "Entendre les pistes son en lisant.", false, 6),
];

/// Barre de l'espace Montage (troisième onglet) : celle du montage, remise pour le travail de montage.
/// Les commandes d'édition sont montrées d'office, dans l'ordre d'un geste de monteur : lire, couper
/// et rogner, transitions et son, pistes, relecture et exports. Les autres commandes de la barre du
/// montage (verrouillages, envois, versions) sont toutes là, sous « Plus… ». Elle a ses propres
/// épingles (`ReglageBarres::montage_edition`) : celles de la barre du storyboard n'y changent rien.
fn montage_edition() -> &'static [Cmd] {
    static BARRE: OnceLock<Vec<Cmd>> = OnceLock::new();
    BARRE.get_or_init(|| {
        let source = |id: &str| -> Cmd {
            *MONTAGE.iter().chain(COMMANDES).chain(TIMELINE).find(|c| c.id == id).unwrap_or_else(|| panic!("barre de l'espace Montage : « {id} » inconnue"))
        };
        let mut v: Vec<Cmd> = Vec::new();
        // La commande de la barre du montage (ou d'une autre), avec sa place ici.
        let reprend = |v: &mut Vec<Cmd>, id: &str, d_office: bool, groupe: u8| v.push(Cmd { d_office, groupe, ..source(id) });
        // 0. Lire.
        reprend(&mut v, "montage.debut", true, 0);
        reprend(&mut v, "lecture.basculer", true, 0);
        reprend(&mut v, "lecture.boucle", true, 0);
        reprend(&mut v, "son.lecture", true, 0);
        reprend(&mut v, "position", true, 0);
        // 1. Plans.
        reprend(&mut v, "panneau.nouveau", true, 1);
        reprend(&mut v, "sequence.nouvelle", true, 1);
        // 2. Couper, rogner, déplacer.
        v.push(bouton("panneau.couper", ph::SCISSORS, "Couper le plan", "En deux plans, à la tête de lecture.", true, 2));
        v.push(bouton("panneau.fusionner_suivant", ph::ARROWS_MERGE, "Réunir avec le suivant", "Le plan courant et le suivant ne font plus qu'un.", true, 2));
        v.push(bouton("montage.debut_a_tete", ph::ARROW_LINE_RIGHT, "Rogner le début", "Raccourcir le début du plan jusqu'à la tête de lecture.", true, 2));
        v.push(bouton("montage.fin_a_tete", ph::ARROW_LINE_LEFT, "Rogner la fin", "Raccourcir la fin du plan jusqu'à la tête de lecture.", true, 2));
        v.push(bouton("montage.duree", ph::TIMER, "Durée du plan", "Taper la durée exacte du plan, en images ou en secondes.", true, 2));
        reprend(&mut v, "plan.avancer", true, 2);
        reprend(&mut v, "plan.reculer", true, 2);
        reprend(&mut v, "montage.aimanter", true, 2);
        reprend(&mut v, "plans", true, 2);
        // 3. Transitions et son.
        reprend(&mut v, "transition.ajouter", true, 3);
        reprend(&mut v, "son.importer", true, 3);
        v.push(bouton("son.nouvelle_piste", ph::LIST_PLUS, "Nouvelle piste son", "Une piste de plus sous les autres, pour poser des sons.", true, 3));
        v.push(bouton("son.couper", ph::SPLIT_HORIZONTAL, "Couper le son", "Couper l'extrait de son à la tête de lecture.", true, 3));
        reprend(&mut v, "son", true, 3);
        // (Les bruitages ont leur icône dans la barre des panneaux : pas une seconde ici.)
        // 4. Pistes.
        reprend(&mut v, "piste.replier_tout", true, 4);
        reprend(&mut v, "piste.deplier_tout", true, 4);
        reprend(&mut v, "piste.auto_deplier", false, 4);
        // 5. Relecture et exports.
        reprend(&mut v, "commentaire.ajouter", true, 5);
        reprend(&mut v, "fichier.exporter_relecture", true, 5);
        reprend(&mut v, "fichier.importer_notes", true, 5);
        reprend(&mut v, "fichier.exporter_montage", true, 5);
        reprend(&mut v, "fichier.exporter_fiche", true, 5);
        // 6. Entrer dans le plan : le double-clic sur le plan le fait aussi, la commande est sous « Plus… ».
        reprend(&mut v, "mode.basculer", false, 6);
        // 7. Le reste de la barre du montage, sous « Plus… ».
        for c in MONTAGE {
            if !v.iter().any(|x| x.id == c.id) {
                v.push(Cmd { d_office: false, groupe: 7, ..*c });
            }
        }
        v
    })
}

/// Ce que choisit l'utilisateur pour une barre, en écart avec ses commandes d'origine : les
/// commandes épinglées en plus, et celles d'origine qu'il a retirées. (Vide : la barre d'origine.)
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReglageBarre {
    #[serde(default, rename = "epinglees", skip_serializing_if = "Vec::is_empty")]
    pub epinglees: Vec<String>,
    #[serde(default, rename = "retirees", skip_serializing_if = "Vec::is_empty")]
    pub retirees: Vec<String>,
}

impl ReglageBarre {
    pub fn est_vide(&self) -> bool {
        self.epinglees.is_empty() && self.retirees.is_empty()
    }

    /// Cette commande est-elle dans la barre (avant que la place manque) ?
    pub fn dans_la_barre(&self, c: &Cmd) -> bool {
        if c.genre == Genre::Fixe {
            return true;
        }
        if c.d_office {
            !self.retirees.iter().any(|i| i == c.id)
        } else {
            self.epinglees.iter().any(|i| i == c.id)
        }
    }

    /// Épingle la commande dans la barre (`oui`) ou la retire. Une commande qui revient à son état
    /// d'origine ne laisse aucune trace.
    pub fn epingler(&mut self, c: &Cmd, oui: bool) {
        if c.genre == Genre::Fixe {
            return;
        }
        let liste = if c.d_office { &mut self.retirees } else { &mut self.epinglees };
        // D'origine dans la barre : on note qu'on la retire ; sinon, qu'on l'épingle.
        let noter = oui != c.d_office;
        liste.retain(|i| i != c.id);
        if noter {
            liste.push(c.id.to_string());
        }
    }

    /// Le réglage lu d'un fichier, remis d'équerre : plus d'identifiant inconnu ni en double, chaque
    /// identifiant dans la bonne liste.
    pub fn nettoye(self, barre: Barre) -> ReglageBarre {
        let propre = |liste: Vec<String>, office: bool| {
            let mut out: Vec<String> = Vec::new();
            for id in liste {
                let bon = barre.cherche(&id).is_some_and(|c| c.genre != Genre::Fixe && c.d_office == office);
                if bon && !out.contains(&id) {
                    out.push(id);
                }
            }
            out
        };
        ReglageBarre { epinglees: propre(self.epinglees, false), retirees: propre(self.retirees, true) }
    }
}

/// Les réglages des trois barres.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReglageBarres {
    #[serde(default, skip_serializing_if = "ReglageBarre::est_vide")]
    pub commandes: ReglageBarre,
    #[serde(default, skip_serializing_if = "ReglageBarre::est_vide")]
    pub montage: ReglageBarre,
    #[serde(default, skip_serializing_if = "ReglageBarre::est_vide")]
    pub montage_edition: ReglageBarre,
    #[serde(default, skip_serializing_if = "ReglageBarre::est_vide")]
    pub timeline: ReglageBarre,
}

impl ReglageBarres {
    pub fn est_vide(&self) -> bool {
        self.commandes.est_vide() && self.montage.est_vide() && self.montage_edition.est_vide() && self.timeline.est_vide()
    }

    pub fn de(&self, barre: Barre) -> &ReglageBarre {
        match barre {
            Barre::Commandes => &self.commandes,
            Barre::Montage => &self.montage,
            Barre::MontageEdition => &self.montage_edition,
            Barre::Timeline => &self.timeline,
        }
    }

    pub fn de_mut(&mut self, barre: Barre) -> &mut ReglageBarre {
        match barre {
            Barre::Commandes => &mut self.commandes,
            Barre::Montage => &mut self.montage,
            Barre::MontageEdition => &mut self.montage_edition,
            Barre::Timeline => &mut self.timeline,
        }
    }

    pub fn nettoyees(self) -> ReglageBarres {
        ReglageBarres {
            commandes: self.commandes.nettoye(Barre::Commandes),
            montage: self.montage.nettoye(Barre::Montage),
            montage_edition: self.montage_edition.nettoye(Barre::MontageEdition),
            timeline: self.timeline.nettoye(Barre::Timeline),
        }
    }
}

/// Les commandes montrées dans la barre (avant que la place manque), dans l'ordre.
pub fn dans_la_barre(barre: Barre, reglage: &ReglageBarre) -> Vec<&'static Cmd> {
    barre.commandes().iter().filter(|c| reglage.dans_la_barre(c)).collect()
}

/// Les commandes qu'on trouve sous « Plus… » (hors celles que le manque de place y envoie).
pub fn dans_plus(barre: Barre, reglage: &ReglageBarre) -> Vec<&'static Cmd> {
    barre.commandes().iter().filter(|c| c.genre != Genre::Fixe && !reglage.dans_la_barre(c)).collect()
}

/// Taille des icônes de TOUTES les barres (commandes, outils, panneaux, vue, en-têtes du montage et de la timeline),
/// la case qui les porte et l'écart entre deux cases : les mêmes partout (simplification des barres, 2026-10-21).
pub(crate) const ICONE: f32 = 17.0;
pub(crate) const CASE: [f32; 2] = [28.0, 26.0];
pub(crate) const ECART: f32 = 2.0;
/// Espace de part et d'autre du trait qui sépare deux groupes.
pub(crate) const MARGE_TRAIT: f32 = 3.0;

/// Place du bouton « Plus… », gardée à droite de la dernière commande.
const PLUS_LARGEUR: f32 = 34.0;
/// Largeur des lignes de « Plus… ».
const LIGNE_LARGEUR: f32 = 270.0;

/// Ce qu'une commande montre à l'instant : son icône, son nom, son état et si elle répond.
pub(crate) struct Vue {
    pub icone: String,
    pub nom: String,
    pub description: String,
    pub etat: Option<bool>,
    pub actif: bool,
}

impl App {
    /// Une barre, sur une seule ligne, sans défilement : ses commandes, puis « Plus… ».
    pub(crate) fn barre_ui(&mut self, ui: &mut Ui, barre: Barre) {
        ui.spacing_mut().item_spacing.x = ECART;
        let reglage = self.settings.bars.de(barre).clone();
        let mut debordent: Vec<&'static Cmd> = Vec::new();
        let mut groupe: Option<u8> = None;
        // Le montage et la timeline gardent la place de l'icône de la légende des clés, au bout de la ligne.
        let reserve = if barre == Barre::Commandes { 0.0 } else { CASE[0] + ECART };
        for c in dans_la_barre(barre, &reglage) {
            // Une commande qui ne s'applique pas à l'instant disparaît de la barre (elle reste, grisée et
            // expliquée, sous « Plus… ») : ni cases mortes, ni place prise pour rien.
            if matches!(c.genre, Genre::Bouton | Genre::Etat) && !self.vue_de(barre, c).actif {
                continue;
            }
            // Plus de place (avec celle de « Plus… ») : cette commande et les suivantes y passent.
            if c.genre != Genre::Fixe && (!debordent.is_empty() || ui.available_width() < c.largeur + PLUS_LARGEUR + reserve) {
                debordent.push(c);
                continue;
            }
            if groupe.is_some_and(|g| g != c.groupe) {
                ui.add_space(MARGE_TRAIT);
                ui.separator();
                ui.add_space(MARGE_TRAIT);
            }
            groupe = Some(c.groupe);
            self.commande_dans_la_barre(ui, barre, c);
        }
        let resp = ui.menu_button(RichText::new(ph::DOTS_THREE).size(ICONE), |ui| self.plus_menu(ui, barre, &debordent)).response;
        bulle(resp, "Plus…", "Les autres commandes de la barre. L'épingle d'une ligne la remet dans la barre.", "");
    }

    /// La commande `c` dans la barre : une icône, un menu, un champ ou un affichage.
    fn commande_dans_la_barre(&mut self, ui: &mut Ui, barre: Barre, c: &'static Cmd) {
        match c.genre {
            Genre::Fixe => self.fixe_ui(ui, c.id),
            Genre::Reglage => self.reglage_ui(ui, c.id),
            Genre::Menu => {
                let resp = ui.menu_button(RichText::new(c.icone).size(ICONE), |ui| self.menu_de_barre(ui, c.id)).response;
                bulle(resp, c.nom, c.description, "");
            }
            Genre::Bouton | Genre::Etat => {
                let v = self.vue_de(barre, c);
                let on = v.etat == Some(true);
                // Les mêmes cases partout : plates (un bouton) ou enfoncées (un état), à l'encre lisible sur l'accent.
                let text = RichText::new(v.icone.as_str()).size(ICONE);
                let resp = if v.etat.is_some() {
                    ui.add_sized(CASE, egui::SelectableLabel::new(on, text))
                } else {
                    ui.add_sized(CASE, egui::Button::new(text).frame(false))
                };
                if self.aide(resp, &v.nom, &v.description, c.action).clicked() && v.actif {
                    self.executer_commande(barre, c);
                }
            }
        }
    }

    /// Le menu « Plus… » : les commandes qui ne sont pas dans la barre, chacune avec son épingle ;
    /// dessous, celles de la barre (pour en retirer) et le retour à la barre d'origine.
    fn plus_menu(&mut self, ui: &mut Ui, barre: Barre, debordent: &[&'static Cmd]) {
        ui.set_min_width(LIGNE_LARGEUR + 30.0);
        let reglage = self.settings.bars.de(barre).clone();
        let place = |c: &Cmd| barre.commandes().iter().position(|x| x.id == c.id).unwrap_or(usize::MAX);
        let mut liste = dans_plus(barre, &reglage);
        liste.extend(debordent.iter().copied());
        liste.sort_by_key(|c| place(c));
        let mut groupe: Option<u8> = None;
        for c in liste.iter().copied() {
            if groupe.is_some_and(|g| g != c.groupe) {
                ui.separator();
            }
            groupe = Some(c.groupe);
            self.ligne_de_plus(ui, barre, c);
        }
        if liste.is_empty() {
            ui.label(RichText::new("Tout est dans la barre.").weak());
        }
        ui.separator();
        ui.menu_button("Dans la barre", |ui| {
            ui.set_min_width(LIGNE_LARGEUR + 30.0);
            for c in dans_la_barre(barre, &reglage).into_iter().filter(|c| c.genre != Genre::Fixe && !debordent.iter().any(|d| d.id == c.id)) {
                self.ligne_de_plus(ui, barre, c);
            }
        });
        if ui.add_enabled(!reglage.est_vide(), egui::Button::new(format!("{}  Rétablir la barre d'origine", ph::ARROW_COUNTER_CLOCKWISE))).clicked() {
            *self.settings.bars.de_mut(barre) = ReglageBarre::default();
            let _ = self.settings.save();
            ui.close_menu();
        }
    }

    /// Une ligne de « Plus… » : l'épingle, puis l'icône, le nom et le raccourci de la commande.
    fn ligne_de_plus(&mut self, ui: &mut Ui, barre: Barre, c: &'static Cmd) {
        let epinglee = self.settings.bars.de(barre).dans_la_barre(c);
        ui.horizontal(|ui| {
            let pin = ui.add(egui::Button::new(bulles::symbole_etat(ph::PUSH_PIN, epinglee)).selected(epinglee).frame(false));
            let nom = if epinglee { "Retirer de la barre" } else { "Épingler dans la barre" };
            let pin = bulle(pin, nom, &format!("Dans la barre ou sous « Plus… » : le choix est retenu pour la {}.", barre.nom().to_lowercase()), "");
            if pin.clicked() {
                self.epingler_commande(barre, c, !epinglee);
            }
            match c.genre {
                Genre::Menu => {
                    ui.menu_button(format!("{}  {}", c.icone, c.nom), |ui| self.menu_de_barre(ui, c.id));
                }
                Genre::Reglage => {
                    ui.label(format!("{}  {}", c.icone, c.nom));
                    self.reglage_ui(ui, c.id);
                }
                Genre::Fixe => {}
                Genre::Bouton | Genre::Etat => {
                    let v = self.vue_de(barre, c);
                    let on = v.etat == Some(true);
                    let key = self.raccourci(c.action);
                    // Sans raccourci, egui centre le nom dans la ligne : un raccourci blanc le garde à gauche.
                    let affiche = if key.is_empty() { " ".to_string() } else { key.clone() };
                    let text = bulles::encre_etat(RichText::new(format!("{}  {}", v.icone, v.nom)), on);
                    let button = egui::Button::new(text).selected(on).shortcut_text(affiche).min_size(vec2(LIGNE_LARGEUR - 24.0, 0.0));
                    let resp = bulle(ui.add_enabled(v.actif, button), &v.nom, &v.description, &key);
                    if resp.clicked() {
                        ui.close_menu();
                        self.executer_commande(barre, c);
                    } else if resp.secondary_clicked() {
                        ui.close_menu();
                        self.start_assign(c.action);
                    }
                }
            }
        });
    }

    /// Épingle ou retire une commande, et retient le choix.
    pub(crate) fn epingler_commande(&mut self, barre: Barre, c: &Cmd, oui: bool) {
        self.settings.bars.de_mut(barre).epingler(c, oui);
        let _ = self.settings.save();
    }

    /// Ce que la commande montre à l'instant (l'icône qui change, l'état, ce qui grise).
    pub(crate) fn vue_de(&self, barre: Barre, c: &Cmd) -> Vue {
        let mut v = Vue { icone: c.icone.to_string(), nom: c.nom.to_string(), description: c.description.to_string(), etat: None, actif: true };
        match c.id {
            "lecture.basculer" => v.icone = (if self.playing { ph::PAUSE } else { ph::PLAY }).to_string(),
            "lecture.boucle" => v.etat = Some(self.looping),
            "son.lecture" => {
                v.etat = Some(self.sound_on);
                v.icone = (if self.sound_on { ph::SPEAKER_HIGH } else { ph::SPEAKER_SLASH }).to_string();
            }
            "edition.copier" | "edition.supprimer" | "edition.coller" if barre.est_du_montage() => {
                let n = self.board_selection().len();
                let chosen = if n > 1 { format!("Les {n} plans choisis.") } else { "Le plan courant ; Ctrl ou Maj + clic pour en choisir plusieurs.".to_string() };
                match c.id {
                    "edition.copier" | "edition.supprimer" => v.description = chosen,
                    _ => {
                        v.actif = self.panel_clipboard.is_some();
                        if !v.actif {
                            v.description = "Rien à coller : copie d'abord des plans.".to_string();
                        }
                    }
                }
            }
            "storyboard.verrouiller" => {
                v.etat = Some(self.doc.storyboard_locked);
                v.icone = (if self.doc.storyboard_locked { ph::LOCK } else { ph::LOCK_OPEN }).to_string();
            }
            "montage.aimanter" => v.etat = Some(self.snap_on),
            "piste.auto_deplier" => v.etat = Some(self.settings.montage_auto_open),
            "envoi.liste" => v.etat = Some(self.envois_ui.liste),
            "comparer.ouvrir" => v.etat = Some(self.comparaison.is_some()),
            "envoi.modifie_suivant" | "envoi.modifie_precedent" => {
                v.actif = !self.doc.envois.is_empty();
                if !v.actif {
                    v.description = "Aucun envoi pour l'instant : « Marquer un envoi… » d'abord.".to_string();
                }
            }
            "numerotation.verrouiller" => {
                v.etat = Some(self.doc.numbering_locked);
                v.icone = (if self.doc.numbering_locked { ph::LOCK_KEY } else { ph::LIST_NUMBERS }).to_string();
            }
            "numerotation.renumeroter" => {
                v.actif = self.doc.numbering_locked;
                if !v.actif {
                    v.description = "Verrouille d'abord la numérotation des plans.".to_string();
                }
            }
            "mode.basculer" => {
                let (icone, nom, what) = match (self.mode, barre) {
                    (Mode::Animation, Barre::Timeline) => (ph::ARROW_U_UP_LEFT, "Retour au storyboard", "Revenir au montage des plans."),
                    (Mode::Animation, _) => (ph::ARROW_U_UP_LEFT, "Retour au storyboard", "Le dessin montre de nouveau le storyboard (Vue dessin ou vignettes)."),
                    (Mode::Storyboard, _) => (ph::FILM_SLATE, "Animer ce plan", c.description),
                };
                (v.icone, v.nom, v.description) = (icone.to_string(), nom.to_string(), what.to_string());
            }
            "marque.basculer" => v.etat = Some(self.panel().marks.iter().any(|m| m.frame == self.cur_frame)),
            "camera.cle_debut" => v.icone = format!("|{}", ph::DIAMOND),
            "camera.cle_fin" => v.icone = format!("{}|", ph::DIAMOND),
            // « Ici » et « Supprimer » se relaient dans la barre : une clé à cette image, on peut la supprimer ;
            // sinon, en poser une. Grisées sous « Plus… », avec la raison.
            "camera.cle_ici" => {
                v.actif = self.camera_key_here().is_none();
                if !v.actif {
                    v.description = "Il y a déjà une clé de caméra à cette image.".to_string();
                }
            }
            "camera.cle_supprimer" => {
                v.actif = self.camera_key_here().is_some();
                if !v.actif {
                    v.description = "Pas de clé de caméra à cette image.".to_string();
                }
            }
            "camera.cle_adoucir" => {
                v.actif = self.camera_key_here().is_some();
                v.etat = Some(self.camera_key_here().is_some_and(crate::lissage::est_adoucie));
                if !v.actif {
                    v.description = "Pas de clé de caméra à cette image.".to_string();
                }
            }
            "vue.pelure" => v.etat = Some(self.onion.enabled),
            "pelure.panneau" => v.etat = Some(self.onion_window),
            "flip.panneau" => {
                v.etat = Some(self.flips_window);
                v.description = format!("Feuilleter vite des images pour juger un mouvement. Flip courant : « {} ».", self.flips.current().name);
            }
            _ => {}
        }
        if c.genre == Genre::Etat && v.etat.is_none() {
            v.etat = Some(false);
        }
        v
    }

    /// Ce que fait la commande `c` de la barre `barre` quand on la choisit.
    pub(crate) fn executer_commande(&mut self, barre: Barre, c: &Cmd) {
        match (barre, c.id) {
            // Le montage lit tout le film, la timeline le plan ; ailleurs, la zone active décide.
            (b, "lecture.basculer") if b.est_du_montage() => self.toggle_play_scope(true),
            (Barre::Timeline, "lecture.basculer") => self.toggle_play_scope(false),
            // La timeline va d'une image à l'autre du plan, même quand le montage a la main.
            (Barre::Timeline, "image.premiere") => self.goto_frame(0),
            (Barre::Timeline, "image.precedente") => self.goto_frame(self.cur_frame.saturating_sub(1)),
            (Barre::Timeline, "image.suivante") => self.goto_frame(self.cur_frame + 1),
            (Barre::Timeline, "image.derniere") => self.goto_frame(self.frame_count().saturating_sub(1)),
            // Dans la barre du montage, copier, coller et supprimer sont ceux des plans choisis.
            (b, "edition.copier") if b.est_du_montage() => self.copy_panels(),
            (b, "edition.coller") if b.est_du_montage() => self.paste_panels(),
            (b, "edition.supprimer") if b.est_du_montage() => self.delete_chosen_panels(),
            (_, "mode.basculer") => match self.mode {
                Mode::Storyboard => self.enter_animation(self.cur_panel),
                Mode::Animation => self.set_mode(Mode::Storyboard),
            },
            _ => self.run_action(c.action),
        }
    }

    /// Un affichage qui reste dans la barre.
    fn fixe_ui(&mut self, ui: &mut Ui, id: &str) {
        match id {
            "position" => self.montage_position_ui(ui),
            "plan" => self.timeline_plan_ui(ui),
            "image_courante" => self.timeline_image_ui(ui),
            _ => {}
        }
    }

    /// Le contenu du menu d'une barre.
    fn menu_de_barre(&mut self, ui: &mut Ui, id: &str) {
        match id {
            "plans" => self.plans_barre_menu(ui),
            "son" => self.sound_menu(ui),
            "reperes" => self.cues_menu(ui),
            "affichage" => self.timeline_view_menu(ui),
            "images" => self.images_menu(ui),
            _ => {}
        }
    }

    /// Un champ qu'on règle : la cadence, l'exposition des nouveaux dessins, la pelure.
    fn reglage_ui(&mut self, ui: &mut Ui, id: &str) {
        let max = crate::onion::MAX_NEIGHBOURS;
        let Some(c) = Barre::Timeline.cherche(id) else { return };
        let resp = match id {
            "cadence" => {
                let mut fps = self.doc.fps;
                let resp = ui.add(egui::DragValue::new(&mut fps).range(1..=120).suffix(" i/s"));
                let resp = bulle(resp, c.nom, c.description, "");
                if self.edited(&resp) {
                    self.doc.fps = fps;
                }
                return;
            }
            "exposition_defaut" => ui.add(egui::DragValue::new(&mut self.default_exposure).range(1..=12).prefix("en ")),
            "pelure_avant" => ui.add(egui::DragValue::new(&mut self.onion.before).range(0..=max).prefix("‹ ")),
            "pelure_apres" => ui.add(egui::DragValue::new(&mut self.onion.after).range(0..=max).suffix(" ›")),
            _ => return,
        };
        bulle(resp, c.nom, c.description, "");
    }
}
