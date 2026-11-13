//! Les textes d'explication des Préférences et des fenêtres (partie 153, `aidetextes.rs`) : ils ont quitté leur fenêtre et ne vivent
//! plus que dans Aide > « Registre des explications », rangés par fenêtre. Le contenu est celui qui était écrit dans la fenêtre.

use crate::aidetextes::Texte;

const fn t(fenetre: &'static str, sujet: &'static str, texte: &'static str) -> Texte {
    Texte { fenetre, sujet, texte }
}

const PREFS: &str = "Préférences";
const GENERATION: &str = "Préférences : génération de sons";
const EXPORT: &str = "Fenêtres d'export";
const SCENARIO: &str = "Importer un scénario";
const EQUIPE: &str = "Travailler à plusieurs";

/// Les textes des Préférences et des fenêtres, dans l'ordre du registre.
pub(crate) static FENETRES: &[Texte] = &[
    // ------------------------------------------------------------------ Exporter (chaque export y est aussi : `prefsui::FAMILLES`)
    t(crate::aidetextes::EXPORTS, "", "Ce que fait chaque export de la fenêtre « Exporter » (aussi dans l'info-bulle de son bouton) :"),
    // ------------------------------------------------------------------ Préférences
    t(PREFS, "Couleurs", "Changer une couleur passe en « Personnalisé » ; le survol, le texte secondaire et les couleurs de la timeline s'en déduisent."),
    t(PREFS, "Rangées de cases pâles", "Une rangée pâle de plus apparaît dès qu'on dessine dans la dernière."),
    t(PREFS, "Aimant des cadres de caméra", "0 : pas d'aimant, le cadre suit le pointeur partout."),
    t(PREFS, "Calque Décor", "Ne change pas le calque Décor d'un plan qui en a déjà un : il garde sa sorte et reçoit toujours les traits du décor."),
    t(PREFS, "Brosses Décor", "Brosses Décor : couleurs, pinceau et taille se règlent dans le panneau Brosses dédiées."),
    t(PREFS, "Tremblement du trait par défaut", "Le trait tremble d'une image à l'autre. Un calque qui a son propre « Tremblement du trait » (panneau Effets) garde le sien."),
    t(PREFS, "Curseur", "Le toucher du stylet se règle en bas des réglages du pinceau."),
    t(PREFS, "Point fixe et rotation", "Ctrl tenu pendant le geste : l'autre point fixe. Aussi dans les réglages des outils Sélection et Transformer."),
    t(PREFS, "Signature des commentaires", "Vide : le nom de l'utilisateur Windows."),
    t(PREFS, "Fonctions expérimentales", "Fonctions à l'essai : elles peuvent encore changer. Rien n'est supprimé quand la case est décochée."),
    t(PREFS, "Ouverture de la planche", "À l'ouverture et à la création d'un projet seulement : le zoom ne change pas quand on revient sur l'onglet."),
    t(GENERATION, "", "Le panneau Suggestions sonores lit le storyboard et propose des sons témoins. Chaque rôle a son fournisseur : le serveur local de cet ordinateur, ou un service en ligne."),
    t(GENERATION, "Lecture par règles", "Lit l'action, le dialogue et les notes des plans avec des mots-clés, sans modèle : rien à installer, rien n'est envoyé."),
    t(GENERATION, "Serveur local", "Un petit programme Python (sidecar/serveur.py) qui tourne sur cet ordinateur et fait tourner les modèles : rien ne sort de la machine. Les modèles et le Python sont dans le dossier SHOT-modeles (par défaut Documents\\SHOT\\Modèles\\SHOT-modeles)."),
    t(GENERATION, "Licences", "Licences : les conditions d'utilisation de chaque service et de chaque modèle (en particulier l'usage commercial des sons générés et le sort des données envoyées) sont à vérifier par toi. Ce sont ici des sons témoins, pour l'animatique."),
    // ------------------------------------------------------------------ Exports
    t(EXPORT, "Calques séparés", "Un dossier par plan (SQ01_PL012A), une suite PNG par calque : 01_Fond_0001.png…"),
    t(EXPORT, "Clés de caméra", "Le JSON et son format sont décrits dans docs/EXPORT_MONTAGE.md."),
    t(EXPORT, "Son par plan", "Le mixage du montage (volumes, fondus, égaliseur, écho ; muets et solo respectés), de la première à la dernière image du plan, à l'échantillon près. Un son qui passe une coupe est coupé au bord du plan."),
    t(EXPORT, "Images à la clé", "Les calques cachés, les calques d'effet et les calques vidéo ne font pas de clé ; les annotations seulement avec « Avec les annotations »."),
    t(EXPORT, "Noms des fichiers", "Champs : {projet} {episode} {sequence} {plan} {cle} {calque} {version} {case} {date} ; une largeur complète de zéros : {plan:03}. Le reste (tirets, soulignés, lettres) est écrit tel quel."),
    // ------------------------------------------------------------------ Scénario
    t(SCENARIO, "Rythme", "Un plan dure la somme de ses répliques (mots ÷ vitesse) et de son action (lignes × durée par ligne), entre le plus court et le plus long."),
    t(SCENARIO, "Où poser les plans", "Une seule étape d'annulation. Une séquence ne se coupe pas : après le plan courant, c'est à la fin de sa séquence."),
    t(SCENARIO, "Personnages", "Notés dans les notes du projet (Réglages du projet), pour les voix provisoires."),
    t(SCENARIO, "Depuis l'accueil", "Un nouveau projet le reçoit (FEANOR propose d'abord d'enregistrer le projet ouvert s'il a changé)."),
    // ------------------------------------------------------------------ Travailler à plusieurs
    t(EQUIPE, "Copies en conflit", "Un client de synchronisation de fichiers a gardé deux versions d'un fichier."),
    t(EQUIPE, "Brouillons", "Du travail mis de côté (un plan perdu dans une course, repris pendant une coupure…)."),
    t(EQUIPE, "Création du film partagé", "Le projet ouvert n'est pas touché. Le film partagé s'ouvrira dans un nouvel onglet."),
    t(EQUIPE, "Mon nom et ma couleur", "Le nom et la couleur que les autres voient sur les plans que tu tiens."),
];
