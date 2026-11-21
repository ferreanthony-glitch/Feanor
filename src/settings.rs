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
    /// Partie 198 : la disposition de l'onglet 3D (`Espace::TroisD`) ; absente : son origine (`DockLayout::pour_la_3d`).
    #[serde(default, rename = "disposition_3d", skip_serializing_if = "Option::is_none")]
    pub trois_d_layout: Option<Layout>,
    /// Le panneau « Vignettes » (2026-10-24) a été posé, une fois, dans la disposition retenue de l'espace Vignettes
    /// (`poser_le_panneau_vignettes`) : une disposition d'avant lui ne l'avait pas, et il se voit avec l'onglet. Fermé ensuite à la main,
    /// il reste fermé.
    #[serde(default, rename = "panneau_vignettes_pose", skip_serializing_if = "std::ops::Not::not")]
    pub panneau_vignettes_pose: bool,
    /// Le panneau « Espace 3D » (2026-11-12, `docs/INTERFACE.md` partie 150) a été posé, une fois, dans les dispositions
    /// retenues du Storyboard et de l'Animation et dans les espaces de travail enregistrés (`poser_le_panneau_espace3d`) :
    /// une disposition d'avant lui ne l'avait pas. Fermé ensuite à la main, il reste fermé, jusqu'à ce que la vue
    /// Espace 3D s'ouvre (elle le remet à sa place).
    #[serde(default, rename = "panneau_espace3d_pose", skip_serializing_if = "std::ops::Not::not")]
    pub panneau_espace3d_pose: bool,
    /// Le panneau « Espace 3D » a quitté, une fois, la place que la partie 150 lui donnait dans les dispositions retenues
    /// (un onglet du groupe de la couleur) : depuis la partie 152 (2026-11-13, « Onglet de Réglages »), il arrive en onglet
    /// du groupe de Réglages quand la vue Espace 3D s'ouvre (`poser_le_panneau_espace3d`). (Le nom de la marque vient du
    /// premier essai de la partie 152 : une colonne à lui, « à côté » de Réglages.)
    #[serde(default, rename = "panneau_espace3d_a_cote", skip_serializing_if = "std::ops::Not::not")]
    pub panneau_espace3d_a_cote: bool,
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
    /// Partie 173.5 (Anthony : « Plus petite et ajustable ») : la largeur de la colonne de gauche de la bande hors de l'espace
    /// Montage (Storyboard, Vignettes, Animation), où la barre du montage est en tête de la colonne. L'espace Montage garde la sienne
    /// (`montage_head_w`).
    #[serde(default = "default_storyboard_head_w", rename = "largeur_colonne_storyboard")]
    pub storyboard_head_w: f32,
    /// Pistes repliables du montage (`pistes.rs`) : le calque courant déplie sa piste tout seul
    /// (les autres gardent leur état).
    #[serde(default = "yes", rename = "deplier_calque_courant")]
    pub montage_auto_open: bool,
    /// Partie 174 (Anthony : « on peut aussi replier tous les calques ») : la colonne des calques de la timeline montre tous les
    /// calques sur une ligne, le calque courant compris (⊖ / ⊕ de sa rangée d'en-tête). Un réglage d'affichage du poste.
    #[serde(default, rename = "timeline_tout_replie")]
    pub calques_replies: bool,
    /// Partie 179.13 (Anthony, sur la capture du calque déplié : « Il faut que tous les calques aient cette épaisseur de base ») :
    /// d'origine, tous les calques des timelines du Storyboard et de l'Animation sont sur deux rangées (le nom et les réglages ; la
    /// vignette, le mode, l'opacité), le calque actif seulement surligné ; allumé, « Replier les calques » les met tous sur une ligne
    /// fine. L'espace Montage garde son réglage à lui (`montage_auto_open`). Un réglage d'affichage du poste. (Il remplace
    /// `deplier_calque_actif` de la partie 179.10, au sens inverse, qui n'est pas repris.)
    #[serde(default, rename = "replier_calques", skip_serializing_if = "std::ops::Not::not")]
    pub replier_calques: bool,
    /// Partie 179.13 (Anthony : « Ajoute dans les paramètres la possibilité d'ajuster la taille de chaque ligne. Séquence, plan,
    /// caméra, calques ») : la hauteur des lignes de la timeline du Storyboard, en points (`hauteurstimeline.rs`, Préférences >
    /// Apparence > Tailles). Absentes : celles d'origine (20, 26, 22, 40).
    #[serde(default = "crate::hauteurstimeline::seq_defaut", rename = "hauteur_ligne_sequence", skip_serializing_if = "crate::hauteurstimeline::seq_est_defaut")]
    pub hauteur_sequence: u8,
    #[serde(default = "crate::hauteurstimeline::plans_defaut", rename = "hauteur_ligne_plans", skip_serializing_if = "crate::hauteurstimeline::plans_est_defaut")]
    pub hauteur_plans: u8,
    #[serde(default = "crate::hauteurstimeline::camera_defaut", rename = "hauteur_ligne_camera", skip_serializing_if = "crate::hauteurstimeline::camera_est_defaut")]
    pub hauteur_camera: u8,
    #[serde(default = "crate::hauteurstimeline::calques_defaut", rename = "hauteur_ligne_calques", skip_serializing_if = "crate::hauteurstimeline::calques_est_defaut")]
    pub hauteur_calques: u8,
    /// Partie 179.14 (Anthony : « la possibilité de donner une couleur à chaque ligne ») : la couleur de fond de chaque ligne de la
    /// timeline du Storyboard (hauteurstimeline.rs). Absente : celle du thème.
    #[serde(default, rename = "couleur_ligne_sequence", skip_serializing_if = "Option::is_none")]
    pub couleur_ligne_sequence: Option<[u8; 3]>,
    #[serde(default, rename = "couleur_ligne_plans", skip_serializing_if = "Option::is_none")]
    pub couleur_ligne_plans: Option<[u8; 3]>,
    #[serde(default, rename = "couleur_ligne_camera", skip_serializing_if = "Option::is_none")]
    pub couleur_ligne_camera: Option<[u8; 3]>,
    #[serde(default, rename = "couleur_ligne_calques", skip_serializing_if = "Option::is_none")]
    pub couleur_ligne_calques: Option<[u8; 3]>,
    /// Partie 178 (la timeline du Storyboard d'Anthony) : la rangée Caméra dépliée, la rangée des préréglages de caméra sous ses
    /// clés (« On déplie uniquement pour poser des préréglages »). Repliée d'office. Un réglage d'affichage du poste.
    #[serde(default, rename = "storyboard_camera_depliee")]
    pub storyboard_camera_depliee: bool,
    /// Partie 178 : la rangée Calques de la timeline du Storyboard repliée en une ligne de résumé (sans piste). Dépliée d'office.
    #[serde(default, rename = "storyboard_calques_replies")]
    pub storyboard_calques_replies: bool,
    /// Partie 197 : ce que montre le panneau Scène (depuis le film, la séquence ou le plan) et ce qu'on y a déplié ou replié (les
    /// plans, les séquences). Les dossiers de calques, eux, se replient dans le projet, comme dans la timeline. Un réglage du poste.
    #[serde(default, rename = "panneau_scene", skip_serializing_if = "crate::scene::ChoixScene::est_d_origine")]
    pub scene: crate::scene::ChoixScene,
    /// Partie 199 : l'aspect du décor 3D dans la vue (onglet 3D, vue Espace 3D, vue à côté) : matières, couleurs, argile. Un réglage
    /// du poste ; celui des images des plans est dans le décor de chaque séquence (« Dans les plans »).
    #[serde(default, rename = "aspect_vue_3d", skip_serializing_if = "crate::couleurs3d::Rendu::est_matieres")]
    pub aspect_vue_3d: crate::couleurs3d::Rendu,
    /// Partie 201 : un calque neuf se pose devant la caméra du plan quand elle a quitté la feuille (`calquedevant.rs`) ; coché
    /// d'origine, écrit seulement décoché.
    #[serde(default = "yes", rename = "calque_devant_la_camera", skip_serializing_if = "is_true")]
    pub calque_devant_la_camera: bool,
    // (Partie 165.7 : le réglage « calques_timeline » — trois calques ou toute la pile — n'existe plus, toute la pile est toujours
    // montrée ; une vieille clé, « trois » compris, est ignorée, comme « calques_non_actifs » et « repliage_pistes » avant elle.)
    /// La hauteur de la rangée des plans dans le Storyboard et le Montage : moyenne (d'office) ou en bandeau (`pistes.rs`).
    #[serde(default, rename = "hauteur_plans", skip_serializing_if = "crate::pistes::HauteurPlans::est_moyenne")]
    pub montage_plans: crate::pistes::HauteurPlans,
    /// Quand la bande a de la place, les calques les plus proches du calque actif prennent aussi la grande hauteur (`pistes::agrandir`,
    /// Préférences > Montage). Allumé d'office (Anthony, 2026-11-11 : « il faut que ce soit toujours le plus confortable possible »).
    #[serde(default = "default_grands", rename = "calques_grands_si_place")]
    pub montage_grands: bool,
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
    /// L'ancienne barre des panneaux du bord droit (2026-10-18), montrée ou non : elle n'existe plus depuis la partie 194 (les
    /// onglets d'icônes de Propriétés la remplacent). Gardé pour lire et réécrire les réglages tels quels.
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
    /// Le budget 3D de cette machine (`decor3dbudget.rs`, partie 180) : mesuré par un étalonnage court la première fois qu'un
    /// décor 3D est importé ou ouvert, refait quand la carte, son pilote ou le processeur changent (sa clé), ou par
    /// « Remesurer » (Préférences > Performance, diagnostic des performances).
    #[serde(default, rename = "budget_decor_3d", skip_serializing_if = "Option::is_none")]
    pub decor3d: Option<crate::decor3dbudget::BudgetMachine>,
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
    /// L'aide animée (`aideanim.rs`, partie 153) : les animations déjà vues (leur identifiant), qui ne rejouent plus d'elles-mêmes.
    #[serde(default, rename = "explications_vues", skip_serializing_if = "Vec::is_empty")]
    pub explications_vues: Vec<String>,
    /// Aide > « Afficher les explications » : cochée, une animation rejoue à chaque prise de son outil. Décochée d'origine.
    #[serde(default, rename = "afficher_explications", skip_serializing_if = "std::ops::Not::not")]
    pub afficher_explications: bool,
    /// Partie 195 : le compteur de la bande du Storyboard (« 0:00:00 / 0:22:00 ») en numéros d'images (« image 63 / 528 ») ; Ctrl + clic
    /// sur lui bascule (`compteur.rs`).
    #[serde(default, rename = "compteur_en_images", skip_serializing_if = "std::ops::Not::not")]
    pub compteur_en_images: bool,
    /// Partie 195 : le compteur de la timeline de l'Animation en time code plutôt qu'en numéros d'images (Ctrl + clic sur lui).
    #[serde(default, rename = "compteur_timeline_en_temps", skip_serializing_if = "std::ops::Not::not")]
    pub compteur_timeline_en_temps: bool,
    /// Partie 206 : le bouton « Décaler la suite » de la timeline, allumé (il tient lieu de Ctrl pour le stylet : le bord d'une clé
    /// décale la suite du calque, la fin du plan fait suivre les clés). Éteint d'origine.
    #[serde(default, rename = "cles_decaler_suite", skip_serializing_if = "std::ops::Not::not")]
    pub cles_decaler_suite: bool,
    /// Partie 206 : le bouton « Étirer en proportion » allumé (il tient lieu de Maj : la fin du plan étire toutes ses clés en
    /// proportion). Éteint d'origine.
    #[serde(default, rename = "cles_etirer_proportion", skip_serializing_if = "std::ops::Not::not")]
    pub cles_etirer_proportion: bool,
    /// Partie 206 : la vignette du dessin dans les clés assez larges des lignes de calques de la timeline (Préférences > Apparence >
    /// Timeline). Éteinte d'origine.
    #[serde(default, rename = "cles_vignettes", skip_serializing_if = "std::ops::Not::not")]
    pub cles_vignettes: bool,
    /// Partie 206 : la durée écrite (« 12 i ») dans les clés assez larges. Éteinte d'origine.
    #[serde(default, rename = "cles_durees", skip_serializing_if = "std::ops::Not::not")]
    pub cles_durees: bool,
    /// La taille de l'interface, en pour cent : 90, 100, 125 ou 150 (`echelle.rs`, Préférences > Apparence).
    #[serde(default = "crate::echelle::defaut", rename = "echelle_interface", skip_serializing_if = "crate::echelle::est_defaut")]
    pub echelle: u16,
    /// Partie 179.5 : la taille des icônes de la timeline et de sa barre de lecture, en points : 11 à 16 (`tailleicones.rs`,
    /// Préférences > Apparence > Tailles). Un réglage du poste ; absente des anciens réglages : 13, la taille d'origine.
    #[serde(default = "crate::tailleicones::defaut", rename = "taille_icones", skip_serializing_if = "crate::tailleicones::est_defaut")]
    pub taille_icones: u8,
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
    /// Les réglages du Cutter (`cutter.rs`, partie 169) : écart toléré, bouts effilés, mode, longueur des dépassements, portée,
    /// geste. Un réglage du poste, jamais écrit dans le projet. Absent : ceux d'origine (le Cutter d'avant).
    #[serde(default, rename = "cutter", skip_serializing_if = "crate::cutter::Reglages::est_defaut")]
    pub cutter: crate::cutter::Reglages,
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


/// Les calques proches du calque actif grandissent quand la bande a de la place : dans l'application. Pas dans les essais : ceux
/// d'avant comptent sur les hauteurs fixes des pistes (les essais de cette règle l'allument eux-mêmes, `calquesplace_tests.rs`).
fn default_grands() -> bool {
    !cfg!(test)
}

fn default_head_w() -> f32 {
    crate::soundtrack::HEADERS_W
}

fn default_storyboard_head_w() -> f32 {
    crate::montagehead::LARGEUR_STORYBOARD
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
            trois_d_layout: None,
            panneau_vignettes_pose: false,
            panneau_espace3d_pose: false,
            panneau_espace3d_a_cote: false,
            export_annotations: false,
            exports: Default::default(),
            storyboard_space: Espace::Storyboard,
            bars: ReglageBarres::default(),
            key_filter: Default::default(),
            workspaces: Vec::new(),
            montage_head_w: default_head_w(),
            storyboard_head_w: default_storyboard_head_w(),
            montage_auto_open: true,
            calques_replies: false,
            replier_calques: false,
            hauteur_sequence: crate::hauteurstimeline::SEQ_DEFAUT,
            hauteur_plans: crate::hauteurstimeline::PLANS_DEFAUT,
            hauteur_camera: crate::hauteurstimeline::CAMERA_DEFAUT,
            hauteur_calques: crate::hauteurstimeline::CALQUES_DEFAUT,
            couleur_ligne_sequence: None,
            couleur_ligne_plans: None,
            couleur_ligne_camera: None,
            couleur_ligne_calques: None,
            storyboard_camera_depliee: false,
            storyboard_calques_replies: false,
            scene: Default::default(),
            aspect_vue_3d: Default::default(),
            calque_devant_la_camera: true,
            montage_plans: Default::default(),
            montage_grands: default_grands(),
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
            decor3d: None,
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
            explications_vues: Vec::new(),
            afficher_explications: false,
            compteur_en_images: false,
            compteur_timeline_en_temps: false,
            cles_decaler_suite: false,
            cles_etirer_proportion: false,
            cles_vignettes: false,
            cles_durees: false,
            echelle: crate::echelle::ECHELLE_DEFAUT,
            taille_icones: crate::tailleicones::TAILLE_DEFAUT,
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
            cutter: Default::default(),
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
            Espace::TroisD => self.trois_d_layout.as_ref(),
        }
    }

    /// Retient la disposition de cet espace de travail.
    pub fn set_layout_of(&mut self, espace: Espace, layout: Layout) {
        match espace {
            Espace::Storyboard => self.layout = Some(layout),
            Espace::Animation => self.animation_layout = Some(layout),
            Espace::Montage => self.montage_layout = Some(layout),
            Espace::Vignettes => self.vignettes_layout = Some(layout),
            Espace::TroisD => self.trois_d_layout = Some(layout),
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
        s.taille_icones = crate::tailleicones::taille_valide(s.taille_icones);
        s.vignettes = std::mem::take(&mut s.vignettes).nettoyes();
        s.reprendre_les_anciens_pinceaux_de_decor();
        s.new_plan_secs = crate::dureeplans::secondes_valides(s.new_plan_secs);
        // Mon panneau : une commande qui n'existe plus (réglages d'une autre version) est ignorée, sans rien casser.
        s.mon_panneau = std::mem::take(&mut s.mon_panneau).nettoye(crate::monpanneau::commande_connue);
        // L'anneau sous le stylet : huit commandes qui existent, toutes différentes, un délai dans ses bornes.
        s.anneau = std::mem::take(&mut s.anneau).nettoyes(crate::monpanneau::commande_connue);
        s.cutter = s.cutter.nettoyes();
        s.poser_le_panneau_vignettes();
        s.poser_le_panneau_espace3d();
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

    /// Le panneau « Espace 3D » dans les dispositions retenues (toutes : le Storyboard, l'Animation, le Montage, les
    /// Vignettes, et les espaces de travail enregistrés).
    ///
    /// **Partie 152** (2026-11-13, Anthony : « Place-le à côté de réglages plutôt », puis, entre une colonne à part et un
    /// onglet : « Onglet de Réglages ») : le panneau n'est plus un onglet du groupe de la couleur. Il arrive en onglet du
    /// groupe de Réglages quand la vue Espace 3D s'ouvre, et y reste (`espace3dpanneau.rs`) : une disposition n'a donc pas
    /// besoin de l'avoir d'avance. Une fois (`panneau_espace3d_a_cote`), il quitte la place que la partie 150 lui avait
    /// donnée dans les dispositions retenues (`DockLayout::retirer_de_la_place_150` : un onglet du groupe de la couleur ou
    /// des brosses dédiées, ou seul juste sous les réglages de l'outil), sans rien y changer d'autre ; il revient en
    /// onglet de Réglages à la prochaine ouverture de la vue. Rangé ailleurs à la main (en onglet de Réglages, dans un
    /// autre groupe, un panneau flottant à lui, sa propre fenêtre), il y reste. Un panneau remis à la main dans le groupe
    /// de la couleur ne se distingue pas de celui que la partie 150 y avait posé : il le quitte aussi, cette fois-là ;
    /// remis ensuite, il y reste.
    ///
    /// **Partie 150** (2026-11-12) : le panneau était ajouté, une fois, aux dispositions retenues d'avant lui
    /// (`panneau_espace3d_pose`). Ce n'est plus fait : la marque est seulement posée, pour qu'un fichier de réglages
    /// garde le même sens d'une version à l'autre.
    pub fn poser_le_panneau_espace3d(&mut self) {
        self.panneau_espace3d_pose = true;
        if std::mem::replace(&mut self.panneau_espace3d_a_cote, true) {
            return;
        }
        use crate::dock::Pane;
        let ranger = |l: &mut Layout| {
            // (Une disposition d'avant 6a devient la disposition d'origine du jour, sans le panneau : `Layout::modernisee`.)
            if l.is_before_6a() {
                return;
            }
            let mut d = l.dock_layout();
            if d.retirer_de_la_place_150(Pane::Espace3d) {
                l.panneaux = Some(d);
            }
        };
        for l in [self.layout.as_mut(), self.animation_layout.as_mut(), self.montage_layout.as_mut(), self.vignettes_layout.as_mut()].into_iter().flatten() {
            ranger(l);
        }
        for w in self.workspaces.iter_mut() {
            ranger(&mut w.layout);
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
