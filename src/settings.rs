//! Réglages de l'application (pas du projet), dans %APPDATA%\SHOT\reglages.toml.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::barres::ReglageBarres;
use crate::onion::OnionSettings;
use crate::workspace::{Espace, Layout, Workspace};

#[derive(Serialize, Deserialize)]
pub struct Settings {
    /// ffmpeg choisi à la main (sinon : cherché automatiquement à chaque export).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ffmpeg: Option<PathBuf>,
    /// Copie de secours toutes les N minutes (0 = jamais). 1 par défaut.
    #[serde(default = "default_autosave", rename = "copie_secours_minutes")]
    pub autosave_minutes: u32,
    /// FEANOR écrit le projet tout seul dès que le stylet se lève, fermer ne demande rien, Ctrl+S pose un jalon (`continu.rs`,
    /// 2026-11-10). Décoché : comme avant, seul « Enregistrer » écrit le projet.
    #[serde(default = "default_continu", rename = "enregistrer_en_continu")]
    pub enregistrer_en_continu: bool,
    /// Croix du système sur le dessin, en plus du cercle du pinceau : Windows la déplace dès que
    /// le stylet bouge, sans attendre l'image suivante (le cercle, dessiné par SHOT, la suit).
    #[serde(default = "yes", rename = "curseur_systeme")]
    pub system_cursor: bool,
    /// Dossier du dernier export pour le montage (EDL, XML, OTIO : `editorialui.rs`).
    #[serde(default, rename = "dossier_export_montage", skip_serializing_if = "Option::is_none")]
    pub montage_export_dir: Option<PathBuf>,
    /// La fiche technique (`fiche.rs`, `ficheui.rs`) : colonnes choisies et leur ordre, format, séparateur, time
    /// code de départ, dernier dossier.
    #[serde(default, rename = "fiche_technique")]
    pub fiche: crate::fiche::Reglages,
    /// La liste des plans pour le suivi de production (listeplans.rs) : profil, format, séparateur, image d'entrée, vignettes.
    #[serde(default, rename = "liste_des_plans")]
    pub liste_plans: crate::listeplans::Reglages,
    /// Le paquet de livraison (paquet.rs) : ce qu'il contient, le modèle de chemin, le dernier dossier.
    #[serde(default, rename = "paquet_de_livraison")]
    pub paquet: crate::paquet::Reglages,
    /// Le pont Kitsu (kitsu.rs) : éteint d'origine ; serveur, e-mail, projet, type de tâche, statuts. Ni mot de passe ni jeton ici.
    #[serde(default, rename = "kitsu")]
    pub kitsu: crate::kitsu::Reglages,
    /// Pelure d'oignon.
    #[serde(default, rename = "pelure")]
    pub onion: OnionSettings,
    /// Ce que montre la feuille d'exposition (13g).
    #[serde(default, rename = "feuille_expo")]
    pub xsheet: crate::xsheet::XSheetSettings,
    /// Les flips (13h).
    #[serde(default, rename = "flips")]
    pub flips: crate::flips::FlipSettings,
    /// Couleurs d'avant-plan et d'arrière-plan, curseurs, roue, couleurs récentes et nuanciers
    /// (passe couleur).
    #[serde(default, rename = "couleurs")]
    pub colors: crate::couleur::ColorSettings,
    /// Disposition de l'espace Storyboard (`Espace::Storyboard`), retenue au fil des changements. Le
    /// champ garde son nom d'avant les espaces liés aux modes : l'unique disposition d'un ancien
    /// fichier de réglages devient donc, sans conversion, celle du storyboard.
    #[serde(default, rename = "disposition", skip_serializing_if = "Option::is_none")]
    pub layout: Option<Layout>,
    /// Dispositions des espaces Animation et Montage (absentes : leur disposition d'origine,
    /// `Espace::defaut`), et celui des deux espaces du côté storyboard où l'on était (Storyboard ou
    /// Montage : le retour de l'animation y revient).
    #[serde(default, rename = "disposition_animation", skip_serializing_if = "Option::is_none")]
    pub animation_layout: Option<Layout>,
    #[serde(default, rename = "disposition_montage", skip_serializing_if = "Option::is_none")]
    pub montage_layout: Option<Layout>,
    /// Disposition de l'espace Vignettes (le quatrième espace, `Espace::Vignettes`) ; absente : son origine.
    #[serde(default, rename = "disposition_vignettes", skip_serializing_if = "Option::is_none")]
    pub vignettes_layout: Option<Layout>,
    /// Le panneau « Vignettes » (2026-10-24) a été posé, une fois, dans la disposition retenue de l'espace Vignettes
    /// (`poser_le_panneau_vignettes`) : une disposition d'avant lui ne l'avait pas, et il se voit avec l'onglet. Fermé ensuite à la main,
    /// il reste fermé.
    #[serde(default, rename = "panneau_vignettes_pose", skip_serializing_if = "std::ops::Not::not")]
    pub panneau_vignettes_pose: bool,
    /// « Avec les annotations » des fenêtres d'export (la planche des Vignettes, les images, la vidéo, le GIF, le storyboard PDF) : coché,
    /// les calques d'annotation (de la planche et des plans) sont dans l'export. Décoché d'origine ; retenu par poste de travail, d'une séance à
    /// l'autre (`calqueannot.rs`).
    #[serde(default, rename = "export_avec_annotations", skip_serializing_if = "std::ops::Not::not")]
    pub export_annotations: bool,
    /// Les choix d'export retenus d'une séance à l'autre : taille et cadence du film, informations gravées, mise en page du PDF, images par plan
    /// (`export.rs`, `ExportPrefs`).
    #[serde(default, rename = "exports_v1")]
    pub exports: crate::export::ExportPrefs,
    #[serde(default, rename = "espace_storyboard")]
    pub storyboard_space: Espace,
    /// Commandes épinglées dans la barre de commandes, la barre du montage et celle de la timeline,
    /// en plus ou en moins de celles que chaque barre montre d'office (`barres.rs`).
    #[serde(default, rename = "barres", skip_serializing_if = "ReglageBarres::est_vide")]
    pub bars: ReglageBarres,
    /// Quelles sortes de clés le montage et la timeline montrent (`legendecles.rs`) : un affichage, rien de plus.
    #[serde(default, rename = "cles_vues", skip_serializing_if = "crate::legendecles::FiltreCles::est_complet")]
    pub key_filter: crate::legendecles::FiltreCles,
    /// Espaces de travail enregistrés.
    #[serde(default, rename = "espaces")]
    pub workspaces: Vec<Workspace>,
    /// Largeur de la colonne de gauche du montage (caméra, plans, calques), qu'on étire pour
    /// voir les réglages des calques (Anthony, 2026-10-17).
    #[serde(default = "default_head_w", rename = "largeur_entetes_montage")]
    pub montage_head_w: f32,
    /// Pistes repliables du montage (`pistes.rs`) : le calque courant déplie sa piste tout seul
    /// (les autres gardent leur état).
    #[serde(default = "yes", rename = "deplier_calque_courant")]
    pub montage_auto_open: bool,
    /// Les calques dans la timeline du Storyboard et du Montage (`pistes.rs`, barre du bas de la bande et Préférences > Montage) : trois
    /// à la fois, le calque actif entre ses deux voisins (d'office), ou toute la pile. Un réglage du poste : rien n'est écrit dans le
    /// projet. (Les anciens réglages « calques_non_actifs » et « repliage_pistes » n'existent plus : une vieille clé est ignorée.)
    #[serde(default = "default_pile", rename = "calques_timeline", skip_serializing_if = "crate::pistes::Pile::est_trois")]
    pub montage_pile: crate::pistes::Pile,
    /// La hauteur de la rangée des plans dans le Storyboard et le Montage : moyenne (d'office) ou en bandeau (`pistes.rs`).
    #[serde(default, rename = "hauteur_plans", skip_serializing_if = "crate::pistes::HauteurPlans::est_moyenne")]
    pub montage_plans: crate::pistes::HauteurPlans,
    /// Pinceau et crayon : « Peindre derrière » (`brushfill`), retenu d'une séance à l'autre (Anthony, 2026-10-17).
    /// La clé garde son nom d'avant le 2026-10-23, quand l'interrupteur s'appelait « Peint sur l'aplat » : les anciens
    /// réglages donnent donc « Peindre derrière ».
    #[serde(default, rename = "peint_derriere")]
    pub paint_behind: bool,
    /// Pinceau et crayon : « Peint sur l'aplat » (la peinture va directement dans l'aplat du dessin, pas sur le trait ;
    /// exclusif avec « Peindre derrière »), retenu d'une séance à l'autre. Éteint d'office.
    #[serde(default, rename = "peint_sur_aplat")]
    pub paint_on_aplat: bool,
    /// L'ancien « Remplir la forme » (retiré le 2026-10-23 : il faisait double emploi avec l'aplat sous le trait). Lu
    /// pour qu'un ancien fichier de réglages s'ouvre, jamais écrit, sans effet.
    #[serde(default, rename = "remplir_la_forme", skip_serializing)]
    #[allow(dead_code)]
    pub ancien_remplir_la_forme: bool,
    /// Projets récents, le plus récent d'abord (`recents.rs`), et rouvrir le dernier au lancement.
    #[serde(default, rename = "projets_recents", skip_serializing_if = "Vec::is_empty")]
    pub recent_projects: Vec<PathBuf>,
    #[serde(default = "yes", rename = "rouvrir_dernier_projet")]
    pub reopen_last: bool,
    /// Onglets de projets (`onglets.rs`) : Ouvrir, Nouveau, projets récents et imports ouvrent un nouvel
    /// onglet (sinon : le projet courant est remplacé, après la question habituelle).
    #[serde(default = "yes", rename = "ouvrir_dans_un_nouvel_onglet")]
    pub open_in_new_tab: bool,
    /// Les projets ouverts dans les onglets à la fin de la séance, dans l'ordre, et l'onglet actif : ils
    /// reviennent au lancement avec « Rouvrir le dernier projet au lancement ».
    #[serde(default, rename = "onglets_ouverts", skip_serializing_if = "Vec::is_empty")]
    pub session_tabs: Vec<PathBuf>,
    #[serde(default, rename = "onglet_actif", skip_serializing_if = "is_zero")]
    pub session_active: usize,
    /// La séance d'avant s'est finie sans aucun projet ouvert (Anthony avait fermé le dernier) : au lancement,
    /// « Rouvrir le dernier projet » ne rouvre rien et SHOT démarre sur l'accueil (`sansprojet.rs`).
    #[serde(default, rename = "sans_projet_a_la_fin", skip_serializing_if = "std::ops::Not::not")]
    pub session_vide: bool,
    /// Les panneaux de réglages prennent la hauteur de leur contenu (`ajuste.rs`), sauf entre les
    /// paires (premier panneau du haut, premier du bas) dont on a tiré le bord à la main.
    #[serde(default = "yes", rename = "panneaux_ajustes")]
    pub panes_fit: bool,
    /// La barre des panneaux, sur le bord droit de la fenêtre (`barrepanneaux.rs`).
    #[serde(default = "yes", rename = "barre_panneaux")]
    pub panel_bar: bool,
    #[serde(default, rename = "panneaux_tires_a_la_main", skip_serializing_if = "Vec::is_empty")]
    pub panes_fixed: Vec<(crate::dock::Pane, crate::dock::Pane)>,
    /// Toucher du stylet (sensation des brosses, 2026-10-18) : courbe de pression globale,
    /// plancher de taille, traits fins, bord, seuil d'appui ; commun à tous les pinceaux.
    #[serde(default, rename = "toucher")]
    pub toucher: crate::brush::Toucher,
    /// La liste des pinceaux (l'ancien panneau Pinceaux, 2026-10-18, depuis le 2026-10-24 dans les réglages de
    /// l'outil) : dépliée ou non, liste ou vignettes, la catégorie des réglages et les dossiers ouverts ou repliés.
    #[serde(default, rename = "panneau_pinceaux")]
    pub brushes_panel: crate::pinceaux::Panneau,
    /// Fenêtre « Nouveau projet » (2026-10-18) : dernières valeurs, dernier dossier et préréglages
    /// de l'utilisateur (`nouveau.rs`).
    #[serde(default, rename = "nouveau_projet")]
    pub new_project: crate::nouveau::NewProjectSettings,
    /// Thème de l'interface (2026-10-18) : Gris sombre (par défaut), Gris clair, Sombre ou
    /// Personnalisé avec ses neuf couleurs (`theme.rs`, Fichier > Préférences…).
    #[serde(default, rename = "theme")]
    pub theme: crate::theme::ReglageTheme,
    /// Nom qui signe les commentaires de la timeline (`commentaires.rs`) ; vide : l'utilisateur de
    /// Windows.
    #[serde(default, rename = "auteur_commentaires", skip_serializing_if = "String::is_empty")]
    pub comment_author: String,
    /// Aides au cadrage de la caméra (tiers, grille d'or, spirale, diagonales, croix) et leur
    /// aspect (`cadrage.rs`).
    #[serde(default, rename = "aides_cadrage")]
    pub framing: crate::cadrage::FramingAids,
    /// Réglages de performance (2026-10-19) : effets, traits texturés, caches, fils, vignettes,
    /// animations (`diagnostic.rs`, Fichier > Préférences…, Aide > Diagnostic des performances).
    #[serde(default, rename = "performance")]
    pub perf: crate::diagnostic::PerfReglages,
    /// Ce que le diagnostic des performances a fait : proposé au premier lancement, dernier profil
    /// appliqué et réglages d'avant (« Revenir comme avant »).
    #[serde(default, rename = "diagnostic")]
    pub diagnostic: crate::diagnostic::EtatDiagnostic,
    /// Les dossiers de sons de la bibliothèque de bruitages (`bruitages.rs`).
    #[serde(default, rename = "bruitages")]
    pub bruitages: crate::bruitages::BruitagesReglages,
    /// Le panneau Bibliothèque (`biblio.rs`) : favoris, dernier dossier, vue, taille des vignettes, volume de l'aperçu.
    #[serde(default, rename = "bibliotheque")]
    pub bibliotheque: crate::biblio::BiblioReglages,
    /// La génération sonore (`genreglages.rs`) : pour chaque rôle, local ou service en ligne. Jamais
    /// une clé : elles sont dans le Gestionnaire d'identifiants de Windows (`clessecretes.rs`).
    #[serde(default, rename = "generation_son")]
    pub generation_son: crate::genreglages::GenerationSonReglages,
    /// Opacité du passe-partout, en pour cent (0 à 100) : ce qui couvre le hors-champ autour du cadre de la caméra
    /// quand « Passe-partout » est allumé (`camtools.rs`, Fichier > Préférences…) ; 100 % : opaque, d'origine.
    /// Migration (2026-10-24) : l'ancien voile translucide gardait son opacité sous la clé `opacite_passe_partout` (27 %
    /// d'origine). Cette clé n'est plus lue : un passe-partout est opaque et noir d'origine, et un ancien fichier ne le
    /// rend pas gris à 27 %. La clé du nouveau réglage est donc autre.
    #[serde(default = "default_passe_partout", rename = "passe_partout_opacite")]
    pub passe_partout: u8,
    /// Le tremblement du trait par défaut (`tremblement.rs`, Préférences > Dessin) : éteint d'origine. Appliqué au rendu à tout
    /// calque vectoriel qui n'a pas le sien ; rien n'est écrit dans les projets.
    #[serde(default, rename = "tremblement_par_defaut", skip_serializing_if = "crate::tremblement::Defaut::est_defaut")]
    pub tremblement_defaut: crate::tremblement::Defaut,
    /// Le passe-partout est allumé (icône de la barre sous le dessin, commande `camera.masque`) : allumé d'origine,
    /// retenu d'une séance à l'autre sur ce poste.
    #[serde(default = "yes", rename = "passe_partout_actif")]
    pub passe_partout_actif: bool,
    /// Couleur du passe-partout (rouge, vert, bleu) : noir d'origine.
    #[serde(default = "default_passe_partout_couleur", rename = "passe_partout_couleur")]
    pub passe_partout_couleur: [u8; 3],
    /// Couleur du cadre de format tracé dans le cadre de la caméra (Cadrage > Format) : gris d'origine depuis le 2026-10-28
    /// (rose avant ; Anthony le trouvait criard, et sans étiquette « 16:9 » dans le champ).
    #[serde(default = "default_cadre_format_couleur", rename = "cadre_format_couleur")]
    pub cadre_format_couleur: [u8; 3],
    /// Coller ce qui vient d'un projet d'une autre taille ou d'une autre cadence (`adaptation.rs`, `collage.rs`) : contenir
    /// ou remplir, images ou secondes, et poser la question ou non.
    #[serde(default, rename = "collage")]
    pub collage: crate::adaptation::ReglageCollage,
    /// Travailler à plusieurs sur un film partagé (`equipe.rs`, `cooperationui.rs`) : mon identité d'équipier, la durée d'une
    /// prise sans nouvelles, la prise automatique, les notifications.
    #[serde(default, rename = "equipe")]
    pub equipe: crate::equipe::ReglageEquipe,
    /// Les brosses dédiées **Classique** de la barre (`personnages.rs`, `personnagesui.rs`) : un réglage de l'application, commun à tous les
    /// projets (les brosses Personnage, elles, sont celles du projet). Avant le 2026-10-29 : les « couleurs neutres » (même clé du fichier).
    #[serde(default = "crate::personnages::classiques_par_defaut", rename = "couleurs_neutres")]
    pub brosses_classiques: Vec<crate::personnages::Brosse>,
    /// Les brosses dédiées **Décor** de la barre : un réglage de l'application. Remplacent les trois pinceaux de décor (crayon, feutre gris,
    /// aplat) de Préférences > Vignettes. Absentes d'un ancien fichier : les trois d'origine (avec les anciennes tailles, `Settings::load`).
    #[serde(default = "crate::personnages::decors_par_defaut", rename = "brosses_decor")]
    pub brosses_decor: Vec<crate::personnages::Brosse>,
    /// L'onglet ouvert de la fenêtre Préférences… (`prefsui.rs`), retenu d'une séance à l'autre.
    #[serde(default, rename = "onglet_preferences", skip_serializing_if = "crate::prefsonglets::Onglet::est_defaut")]
    pub prefs_onglet: crate::prefsonglets::Onglet,
    /// Les conseils d'un seul passage déjà vus (`conseils.rs`) ; Préférences > Général > « Revoir les conseils » les oublie.
    #[serde(default, rename = "conseils", skip_serializing_if = "crate::conseils::ReglageConseils::est_vide")]
    pub conseils: crate::conseils::ReglageConseils,
    /// Le parcours « Premier jour » (`premierjour.rs`) : la réponse à la proposition et l'étape où l'on en est. Un réglage du poste.
    #[serde(default, rename = "premier_jour", skip_serializing_if = "crate::premierjour::Reglage::est_vide")]
    pub premier_jour: crate::premierjour::Reglage,
    /// « Afficher les fonctions expérimentales » (Préférences > Général) : le menu Labo et les panneaux à l'essai (suggestions sonores,
    /// Mon panneau, banc d'essai du moteur de dessin) ne se montrent que si cette case est cochée. Rien n'est supprimé : les commandes
    /// restent dans la palette (Ctrl+K). Décochée d'origine.
    #[serde(default, rename = "fonctions_experimentales", skip_serializing_if = "std::ops::Not::not")]
    pub experimental: bool,
    /// La taille de l'interface, en pour cent : 90, 100, 125 ou 150 (`echelle.rs`, Préférences > Apparence).
    #[serde(default = "crate::echelle::defaut", rename = "echelle_interface", skip_serializing_if = "crate::echelle::est_defaut")]
    pub echelle: u16,
    /// Le dossier des images de l'écran d'accueil (`accueilimages.rs`, Préférences > Apparence > Images d'accueil) : un réglage de ce
    /// poste, pas du projet. Vide : `Documents\SHOT\Identité\Accueil`.
    #[serde(default, rename = "dossier_accueil", skip_serializing_if = "String::is_empty")]
    pub dossier_accueil: String,
    /// Les réglages des Vignettes (`vignettesreglages.rs`, Préférences > Vignettes) : débord du trait, aimant et couleur des
    /// cadres de caméra, « Report du décor », couleur et tailles des brosses du décor. Absents des anciens réglages : ceux d'avant.
    #[serde(default, rename = "vignettes", skip_serializing_if = "crate::vignettesreglages::ReglagesVignettes::est_defaut")]
    pub vignettes: crate::vignettesreglages::ReglagesVignettes,
    /// Le point fixe d'un coin ou d'un côté tiré, dans les boîtes de la Sélection et du Transformer (`selbox.rs`) : la
    /// poignée opposée (d'office) ou le centre. Absent des anciens réglages : la poignée opposée.
    #[serde(default, rename = "point_fixe_poignees", skip_serializing_if = "crate::selbox::PointFixe::est_defaut")]
    pub point_fixe: crate::selbox::PointFixe,
    /// Le centre d'une rotation prise hors d'un coin : le pivot (d'office) ou le coin opposé.
    #[serde(default, rename = "rotation_autour_de", skip_serializing_if = "crate::selbox::CentreRotation::est_defaut")]
    pub rotation_autour: crate::selbox::CentreRotation,
    /// La durée d'un plan neuf, en secondes (`dureeplans.rs`, Préférences > Montage) : 2 s d'origine. Un réglage de l'application,
    /// commun à tous les projets ; seuls les plans créés ensuite l'ont (les plans qui existent gardent leur durée).
    #[serde(default = "crate::dureeplans::secondes_d_origine", rename = "duree_nouveau_plan", skip_serializing_if = "crate::dureeplans::est_d_origine")]
    pub new_plan_secs: f32,
    /// « Mon panneau » (essai du menu Labo, `monpanneau.rs`) : les boutons que ce poste y a rangés (commandes, brosses, outils perso,
    /// séparateurs) et « Afficher les noms ». Un réglage du poste : rien n'est écrit dans le projet. Absent : le panneau vide.
    #[serde(default, rename = "mon_panneau", skip_serializing_if = "crate::monpanneau::Reglage::est_vide")]
    pub mon_panneau: crate::monpanneau::Reglage,
    /// La taille du texte du panneau Scénario (petit, moyen, grand ; `scenariogarde::Taille`). Un réglage du poste : moyen d'origine.
    #[serde(default, rename = "scenario_taille", skip_serializing_if = "crate::scenariogarde::Taille::est_defaut")]
    pub scenario_taille: crate::scenariogarde::Taille,
    /// Mes dégradés (`degrade::Enregistre`, Réglages de l'outil Dégradé) : un réglage du poste, jamais écrit dans le projet. Absent : les
    /// dégradés de départ (`degrade::enregistres_d_origine`) ; la liste vidée à la main reste vide.
    #[serde(default, rename = "degrades_enregistres", skip_serializing_if = "Option::is_none")]
    pub degrades: Option<Vec<crate::degrade::Enregistre>>,
    /// La fenêtre à part de la planche de références (détachée, plein écran, écran, transparente, toujours au-dessus ;
    /// `planchref::FenetreReglage`). Un réglage du poste : rien n'est écrit dans le projet.
    #[serde(default, rename = "references_fenetre", skip_serializing_if = "crate::planchref::FenetreReglage::est_defaut")]
    pub references_fenetre: crate::planchref::FenetreReglage,
    /// Les projets neufs naissent avec la numérotation des plans verrouillée (un plan inséré reçoit 2A, comme en studio) :
    /// coché d'origine (`numerotation.rs`, Préférences > Montage > Plans neufs). Ne touche jamais un projet qui existe.
    #[serde(default = "yes", rename = "numerotation_neufs_verrouillee", skip_serializing_if = "is_true")]
    pub numbering_locked_new: bool,
    /// Les hauteurs du montage retenues par espace de travail (`montagehauteur.rs`) : la hauteur d'une piste et le réglage
    /// petite / moyenne / grande.
    #[serde(default, rename = "pistes_hauteur", skip_serializing_if = "crate::montagehauteur::Reglage::est_defaut")]
    pub track_height: crate::montagehauteur::Reglage,
    /// L'anneau sous le stylet (`anneau.rs`, Préférences > Anneau) : en service ou non, le délai avant qu'il se dessine, la
    /// commande de chacune de ses huit directions. Un réglage du poste. Absent : l'anneau d'origine.
    #[serde(default, rename = "anneau", skip_serializing_if = "crate::anneau::Reglages::est_d_origine")]
    pub anneau: crate::anneau::Reglages,
    /// Le compteur d'usage du poste est allumé (`usage.rs`, Aide > Mes habitudes) : chaque commande lancée est comptée dans
    /// un fichier à côté des réglages, qui ne quitte jamais le poste. Allumé d'origine.
    #[serde(default = "yes", rename = "compter_les_commandes", skip_serializing_if = "is_true")]
    pub usage_actif: bool,
    /// Le bouton et la gomme du stylet lus auprès de Windows (`stylet.rs`, `styletlien.rs`) : le bouton du stylet est le
    /// clic droit, le stylet retourné prend la Gomme. Allumé d'origine ; à éteindre si une tablette s'en trouve mal.
    #[serde(default = "yes", rename = "stylet_lu_aupres_de_windows", skip_serializing_if = "is_true")]
    pub stylet_windows: bool,
}

fn is_true(b: &bool) -> bool {
    *b
}

fn default_passe_partout() -> u8 {
    crate::camtools::PASSE_PARTOUT_DEFAUT
}

fn default_passe_partout_couleur() -> [u8; 3] {
    crate::camtools::PASSE_PARTOUT_COULEUR
}

/// Le gris d'origine du cadre de format.
pub const CADRE_FORMAT_COULEUR: [u8; 3] = [150, 150, 150];

fn default_cadre_format_couleur() -> [u8; 3] {
    CADRE_FORMAT_COULEUR
}

fn default_autosave() -> u32 {
    1
}

/// L'écriture continue est allumée d'origine. Dans les essais, elle ne l'est que pour ceux qui la demandent : les autres
/// regardent l'état « modifié » et la question de fermeture d'avant, qui valent encore pour un projet sans dossier.
fn default_continu() -> bool {
    !cfg!(test)
}

/// Trois calques à la fois dans l'application ; « Toute la pile » dans les essais : ceux d'avant visent les pistes par leur rangée et
/// toutes doivent y être (les essais de la fenêtre des trois calques l'allument eux-mêmes, `calques_timeline_tests.rs`).
fn default_pile() -> crate::pistes::Pile {
    if cfg!(test) {
        crate::pistes::Pile::Toute
    } else {
        crate::pistes::Pile::Trois
    }
}

fn default_head_w() -> f32 {
    crate::soundtrack::HEADERS_W
}

fn yes() -> bool {
    true
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ffmpeg: None,
            autosave_minutes: default_autosave(),
            enregistrer_en_continu: default_continu(),
            system_cursor: true,
            montage_export_dir: None,
            fiche: Default::default(),
            liste_plans: Default::default(),
            paquet: Default::default(),
            kitsu: Default::default(),
            onion: OnionSettings::default(),
            xsheet: Default::default(),
            flips: Default::default(),
            colors: Default::default(),
            layout: None,
            animation_layout: None,
            montage_layout: None,
            vignettes_layout: None,
            panneau_vignettes_pose: false,
            export_annotations: false,
            exports: Default::default(),
            storyboard_space: Espace::Storyboard,
            bars: ReglageBarres::default(),
            key_filter: Default::default(),
            workspaces: Vec::new(),
            montage_head_w: default_head_w(),
            montage_auto_open: true,
            montage_pile: default_pile(),
            montage_plans: Default::default(),
            paint_behind: false,
            paint_on_aplat: false,
            ancien_remplir_la_forme: false,
            recent_projects: Vec::new(),
            reopen_last: true,
            open_in_new_tab: true,
            session_tabs: Vec::new(),
            session_active: 0,
            session_vide: false,
            panes_fit: true,
            panel_bar: true,
            panes_fixed: Vec::new(),
            toucher: Default::default(),
            brushes_panel: Default::default(),
            new_project: Default::default(),
            theme: Default::default(),
            comment_author: String::new(),
            framing: Default::default(),
            perf: Default::default(),
            diagnostic: Default::default(),
            bruitages: Default::default(),
            bibliotheque: Default::default(),
            generation_son: Default::default(),
            passe_partout: default_passe_partout(),
            tremblement_defaut: Default::default(),
            passe_partout_actif: true,
            passe_partout_couleur: default_passe_partout_couleur(),
            cadre_format_couleur: default_cadre_format_couleur(),
            collage: Default::default(),
            equipe: Default::default(),
            brosses_classiques: crate::personnages::classiques_par_defaut(),
            brosses_decor: crate::personnages::decors_par_defaut(),
            prefs_onglet: Default::default(),
            conseils: Default::default(),
            premier_jour: Default::default(),
            experimental: false,
            echelle: crate::echelle::ECHELLE_DEFAUT,
            dossier_accueil: String::new(),
            vignettes: Default::default(),
            point_fixe: Default::default(),
            rotation_autour: Default::default(),
            new_plan_secs: crate::dureeplans::secondes_d_origine(),
            mon_panneau: Default::default(),
            scenario_taille: Default::default(),
            degrades: None,
            references_fenetre: Default::default(),
            numbering_locked_new: true,
            track_height: Default::default(),
            anneau: Default::default(),
            usage_actif: true,
            stylet_windows: true,
        }
    }
}

impl Settings {
    /// La disposition retenue de cet espace de travail, s'il en a une.
    pub fn layout_of(&self, espace: Espace) -> Option<&Layout> {
        match espace {
            Espace::Storyboard => self.layout.as_ref(),
            Espace::Animation => self.animation_layout.as_ref(),
            Espace::Montage => self.montage_layout.as_ref(),
            Espace::Vignettes => self.vignettes_layout.as_ref(),
        }
    }

    /// Retient la disposition de cet espace de travail.
    pub fn set_layout_of(&mut self, espace: Espace, layout: Layout) {
        match espace {
            Espace::Storyboard => self.layout = Some(layout),
            Espace::Animation => self.animation_layout = Some(layout),
            Espace::Montage => self.montage_layout = Some(layout),
            Espace::Vignettes => self.vignettes_layout = Some(layout),
        }
    }

    fn path() -> Option<PathBuf> {
        crate::donnees::roaming().map(|d| d.join("reglages.toml"))
    }

    /// Réglages enregistrés ; défauts si le fichier manque ou se lit mal.
    pub fn load() -> Settings {
        let s: Settings = Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default();
        Self::apres_lecture(s)
    }

    /// Les réglages lus, ramenés dans leurs bornes et migrés (ce que fait `load` une fois le fichier lu).
    pub(crate) fn apres_lecture(mut s: Settings) -> Settings {
        s.onion.migrate();
        s.framing = s.framing.sanitized();
        s.perf = s.perf.sanitized();
        s.bars = s.bars.nettoyees();
        // Un dégradé enregistré aux valeurs absurdes (fichier abîmé ou écrit à la main) est écarté, les autres restent.
        if let Some(l) = s.degrades.as_mut() {
            l.retain(|e| e.est_sain());
        }
        // L'espace Montage a maintenant son propre agencement : l'ancienne disposition d'origine,
        // retenue sans que l'utilisateur y ait touché, cède la place à la nouvelle.
        if s.montage_layout.as_ref().is_some_and(crate::workspace::est_le_montage_d_avant) {
            s.montage_layout = None;
        }
        // (Le côté storyboard n'est jamais l'animation.)
        if s.storyboard_space == Espace::Animation {
            s.storyboard_space = Espace::Storyboard;
        }
        s.generation_son = std::mem::take(&mut s.generation_son).sanitized();
        s.passe_partout = s.passe_partout.min(100);
        s.tremblement_defaut = s.tremblement_defaut.sanitized();
        s.equipe = std::mem::take(&mut s.equipe).nettoye();
        s.brosses_classiques = crate::personnages::brosses_nettoyees(std::mem::take(&mut s.brosses_classiques), crate::personnages::MAX_CLASSIQUES);
        s.brosses_decor = crate::personnages::brosses_nettoyees(std::mem::take(&mut s.brosses_decor), crate::personnages::MAX_DECORS);
        s.echelle = crate::echelle::echelle_valide(s.echelle);
        s.vignettes = std::mem::take(&mut s.vignettes).nettoyes();
        s.reprendre_les_anciens_pinceaux_de_decor();
        s.new_plan_secs = crate::dureeplans::secondes_valides(s.new_plan_secs);
        // Mon panneau : une commande qui n'existe plus (réglages d'une autre version) est ignorée, sans rien casser.
        s.mon_panneau = std::mem::take(&mut s.mon_panneau).nettoye(crate::monpanneau::commande_connue);
        // L'anneau sous le stylet : huit commandes qui existent, toutes différentes, un délai dans ses bornes.
        s.anneau = std::mem::take(&mut s.anneau).nettoyes(crate::monpanneau::commande_connue);
        s.poser_le_panneau_vignettes();
        s
    }

    /// Brosses dédiées Décor (2026-10-29) : les anciens réglages (Préférences > Vignettes, tailles du crayon, du feutre et de l'aplat du décor) sont
    /// repris par les trois brosses Décor d'origine, tant qu'elles n'ont pas été touchées ; la couleur unique du décor, elle, est abandonnée
    /// (chaque brosse Décor a la sienne). Les anciennes clés sont lues, plus jamais écrites.
    pub fn reprendre_les_anciens_pinceaux_de_decor(&mut self) {
        let d = crate::vignettesreglages::ReglagesVignettes::default();
        // (§107 : les brosses Décor suivent maintenant le pinceau ; une taille d'avant qui n'était pas celle d'origine fait une brosse
        // spécifique, comme une brosse réglée à la main : `Brosse::reprendre_ancien_pinceau`.)
        if self.brosses_decor == crate::personnages::decors_par_defaut() && self.vignettes.tailles_decor != d.tailles_decor {
            for ((b, t), genre) in self.brosses_decor.iter_mut().zip(self.vignettes.tailles_decor).zip(crate::decor::Genre::ALL) {
                if t.is_finite() && (t - genre.taille_defaut()).abs() > 1e-3 {
                    b.specifique = Some(crate::personnages::Specifique::depuis_genre(genre, t));
                }
            }
        }
        self.vignettes.tailles_decor = d.tailles_decor;
        self.vignettes.couleur_decor = d.couleur_decor;
    }

    /// Une fois : le panneau « Vignettes » rejoint la disposition retenue de l'espace Vignettes si elle n'avait pas encore de panneau de ce
    /// nom (elle date d'avant lui). Sans disposition retenue, l'origine du jour l'a déjà. Une fois posé, c'est à l'utilisateur de le garder
    /// ou de le fermer.
    pub fn poser_le_panneau_vignettes(&mut self) {
        if std::mem::replace(&mut self.panneau_vignettes_pose, true) {
            return;
        }
        if let Some(l) = self.vignettes_layout.as_mut() {
            let mut d = l.dock_layout();
            if !d.contains(crate::dock::Pane::Vignettes) {
                d.open(crate::dock::Pane::Vignettes);
                l.panneaux = Some(d);
            }
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or("dossier %APPDATA% introuvable")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        // (Atomique : un plantage ou un disque plein ne laisse pas des réglages tronqués, que `load`
        // remplacerait par les réglages d'origine.)
        crate::project::write_atomic(&path, text.as_bytes())
    }
}
