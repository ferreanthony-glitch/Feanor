//! Projet en dossier (format 3).
//!
//! ```text
//! MonFilm/
//!   MonFilm.shot          manifeste JSON lisible : réglages, séquences > scènes > panneaux > calques,
//!                         pistes son ; numérotation verrouillée (`numerotation_verrouillee`, `numero` des
//!                         séquences et des plans : champs facultatifs, `numerotation.rs`). Avant le
//!                         2026-10-23 : `MonFilm.pelure`, toujours ouvert ; le premier enregistrement écrit
//!                         `MonFilm.shot` puis efface le `.pelure` (`retirer_ancien_manifeste`). La structure
//!                         d'un film partagé garde son nom `.pelure` (`target_for_partage`).
//!   dessins/<id>.png      dessins bitmap
//!   dessins/<id>.json     dessins vectoriels
//!   sons/<id>.<ext>       fichiers son importés, copiés tels quels
//!   modeles/<id>.pelure3d anciens modèles 3D de repère (format 5) : plus lus ni écrits depuis la suppression du
//!                         repère 3D (2026-10-21), mais jamais effacés non plus
//!   commentaires.json     commentaires sur la timeline (`commentaires.rs`), sans changer `VERSION` :
//!                         absent, il n'y a pas de commentaires ; une version qui ne le connaît pas
//!                         le laisse en place (elle n'efface que le contenu de dessins/, sons/…)
//!   references.json      références visuelles (`references.rs`), de même sans changer `VERSION` ;
//!   references/<id>.<ext> leurs fichiers (images copiées telles quelles, images extraites des vidéos
//!                         dans references/<id>/) : une version qui ne les connaît pas les laisse là
//!   scenes3d.json        anciennes scènes 3D de blocage : plus lues ni écrites depuis la suppression des objets 3D
//!                         (2026-10-21), jamais effacées ; le calque « Fond 3D » qu'elles avaient produit est un
//!                         calque bitmap ordinaire
//!   rognages.json         contenu rogné au début et à la fin des plans (`rognage.rs`), de même sans
//!                         changer `VERSION` ; ses dessins restent dans dessins/ (une version qui ne
//!                         le connaît pas les efface à l'enregistrement : le plan reste tel qu'il est)
//!   transitions.json      transitions du montage (`transitions.rs`), de même sans changer `VERSION` :
//!                         une version qui ne le connaît pas le laisse en place et montre le film
//!                         sans les fondus
//!   effets_son.json       réglages d'effets des extraits et des pistes son (`audiofx.rs`), de même
//!   sons_proposes.json    sons proposés par la lecture du storyboard (`indications.rs`), de même sans
 
//!                         changer `VERSION` ; les sons générés, eux, restent dans le cache de SHOT
//!                         jusqu'à leur acceptation (copiés alors dans sons/)
//!   crayonnes.json        (ancien) la planche de crayonnés, supprimée en 2026-10-21 : SHOT ne le lit plus
//!                         et ne l'écrit plus, mais ne l'efface jamais (`crayonnesheritage.rs`)
//!   personnages.json      les personnages de la brosse par personnage (`personnages.rs`) : nom, couleur de
//!                         trait, couleur d'aplat ; de même sans changer `VERSION` (absent : aucun personnage ;
//!                         une version qui ne le connaît pas le laisse en place)
//!   annotations.json      les annotations de la vue Vignettes (`annotations.rs`) : traits du stylo, attachés à
//!                         une case, de même sans changer `VERSION` ; aucun export ne les lit (absent : aucune
//!                         annotation ; une version qui ne le connaît pas le laisse en place)
//!   envois.json           les envois (`envois.rs`) : nom, date et empreinte de chaque plan, de même sans
//!                         changer `VERSION` ; absent, il n'y a pas d'envoi
//!   versions/             les instantanés des envois et leurs dessins, rangés par contenu
//!                         (`versions/dessins/<empreinte>.png`) : jamais effacés
//!   plans_versions.json   les autres versions de chaque plan (`planvers.rs`), de même sans changer
//!                         `VERSION` ; leurs dessins sont dans dessins/ ; absent, aucun plan n'a de version
//!   variantes.json        les variantes rangées des séquences (`variantes.rs`) : scènes, plans, versions de plan,
//!                         contenu rogné, commentaires et transitions de chaque variante, de même sans changer
//!                         `VERSION` ; leurs dessins sont dans dessins/ ; absent, aucune séquence n'a de variante
//!                         (dans un projet partagé : annexe du montage, écrite par le porteur du jeton)
//! ```
//!
//! Un enregistrement n'écrit que les dessins modifiés depuis le précédent, puis le manifeste.
//! Chaque fichier est écrit à côté puis renommé : un plantage en cours d'enregistrement laisse
//! l'ancienne version intacte. Les dessins qui ne servent plus sont effacés en dernier.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::document::{
    fresh_layer_id, AudioTrack, Behaviour, Cel, CelId, Document, Layer, LayerKind, LayerTransform, Panel, Scene, SceneWide,
    Sequence, Sound, SoundClip, Structure,
};
use crate::brushtex::{GrayImage, TextureKind};
use crate::camera::{Camera, CameraKey};
use crate::guides::Guide;
use crate::io::encode_png;
use crate::raster::Raster;
use crate::vector::VectorDrawing;

// Le format partagé (`docs/COOPERATION.md`) : sous-module de celui-ci pour lire les mêmes types privés.
#[path = "projet_partage.rs"]
pub(crate) mod partage_io;

pub const FORMAT: &str = "pelure-projet";
/// 4 (phase 2b) : la feuille de dessin est plus grande que le cadre de la caméra (`cadre`).
/// 5 (phase 2d) : modèles 3D (`modeles`) et repère 3D par panneau (`repere3d`) ; ces champs sont ignorés à la lecture
/// depuis la suppression du repère 3D (2026-10-21).
/// 6 (phase 5d) : textes dans les dessins vectoriels (éléments `Text`, inconnus des versions
/// précédentes : elles refusent le projet plutôt que de perdre ces dessins).
/// 7 (phase 6f) : calques de toute la scène (`toute_la_scene`) ; une version précédente en ferait
/// des calques séparés qui partagent un même dessin.
/// 8 (phase 7) : notes de la planche (`planche`), traits vectoriels texturés (`pinceau` dans les
/// dessins, textures dans `textures/`), qu'une version précédente perdrait.
/// 9 (phase 8) : notes attachées à une vignette (`attache`), textes de la planche
/// (`planche_textes`), mode de fusion des calques (`fusion`).
/// 10 (phase 9) : caméra avancée (`avance`) et adoucis en images (`adoucir_entree`, `adoucir_sortie`).
/// 11 (9f) : bascule et panoramique de la caméra (`bascule`, `panoramique`), qu'une version
/// précédente ignorerait (le film changerait sans prévenir).
/// 12 (13a) : un seul panneau par scène, le plan (les panneaux d'un projet plus ancien sont mis
/// bout à bout à l'ouverture), et marques d'image (`marques`) qu'une version précédente perdrait.
/// 13 (13b) : réglages de l'en-tête des calques (pelure d'oignon du calque, préserver la
/// transparence, pochoir, position verrouillée, groupe, ligne réduite) et groupes de couleur
/// (`groupes`), qu'une version précédente perdrait.
/// 14 (13b-5) : dossiers de calques (`dossier`, `dans_dossier`) ; une version précédente en
/// ferait des calques vides et perdrait l'opacité et l'affichage des dossiers.
/// 15 (13c) : pré / post-comportement des calques (`avant`, `apres`), qu'une version
/// précédente ignorerait (le film changerait sans prévenir) ; cassure et création automatiques
/// (`cassure_auto`, `creation_auto`) ; noms d'instance (`noms`).
/// 16 (13d) : signets et points d'entrée / sortie des plans (`signets`, `entree`, `sortie`),
/// image de départ du projet (`image_de_depart`), marques de couleur des dessins
/// (`marques_couleur`).
/// 17 (13e) : arrière-plan (`fond` : aucun, couleur ou damier, couleur B), notes dessinées des
/// plans (`notes_dessinees`), verrou du storyboard et propriétés du projet.
/// 18 (13g-2) : rôles des images d'un calque (`roles` : image → clé, breakdown, intervalle…).
/// 19 (13i) : pile d'effets d'un calque (`effets` : sorte, actif, valeurs, clés).
/// 20 (13j) : calque CTG (`ctg` : rangs des calques sources).
/// 21 (13k) : calque vidéo (`video` : id, nom, images, ips, début, extension ; images dans
/// `videos/<id>/`).
/// 22 (2026-10-18) : étirement des calques (`etirement_x`, `etirement_y`), qu'une version précédente
/// ignorerait (le calque reprendrait ses proportions sans prévenir).
/// 23 (2026-10-18) : fond sous le trait (`fond` d'un calque ; la strate Fond d'un dessin dans
/// `dessins/<id>_fond.png`).
/// 24 (2026-10-18) : couleur de repère des plans (`couleur` : rang dans la palette des marques).
/// 25 (2026-10-18, rendu des brosses, étape 5) : les traits vectoriels gardent leur `trace`
/// (appui et instant par point en entiers — `appui` au 1/65535, `instants` en dixièmes de
/// milliseconde depuis le premier —, toucher, couleur B) et leur réglage complet, et sont rejoués
/// par le moteur bitmap. Comme pour tout nouveau format, une version précédente de Pelure **refuse**
/// d'ouvrir le projet (`load` : « créé par une version plus récente ») ; elle ne le lit pas en
/// rendu approché.
/// 26 (2026-10-19) : tremblement du trait des calques vectoriels (effet `tremblement` de la pile
/// `effets` : intensité, fréquence, variantes, vitesse, épaisseur, graine, animer à l'arrêt ; voir
/// `tremblement.rs`). Une version précédente refuse le projet au lieu de montrer le dessin sans
/// son tremblement (elle ne connaît pas la sorte d'effet) : le film changerait sans prévenir.
/// 27 (2026-10-19) : calques d'effets (`calque_effet` d'un calque : ajustement, qui applique sa pile
/// d'effets à tout ce qui est dessous, ou incrustation de texte sur l'image finale — timecode, numéro
/// d'image, nom du plan… ; voir `calqueffet.rs`) et sept effets « Cinéma » dans la pile `effets`
/// (flash, image d'impact, lueur diffuse, secousse, vignettage, grain, aberration chromatique ; voir
/// `fxcine.rs`). Une version précédente refuse le projet : elle en ferait des calques vides et
/// perdrait l'effet, le film changerait sans prévenir.
/// 30 (2026-10-20) : calques de couleur (`calque_couleur` d'un calque : rang du calque de trait, dessins de
/// couleur qui répondent aux dessins du trait, fermeture des trous, pinceau dans la zone ; voir
/// `calquecouleur.rs`) et toile infinie (`dessins_hors`). Une version précédente refuse le projet : elle en
/// ferait des calques bitmap ordinaires aux images indépendantes, et la couleur ne suivrait plus le trait.
/// (Les numéros 28 et 29 sont ceux d'autres chantiers menés en parallèle, restés sans changement de format.)
/// 31 (2026-10-20) : effets du trait des calques vectoriels dans la pile `effets` (écriture du trait,
/// crayonné multiple, respiration, ondulation du trait, pointillés, désagrégation ; voir
/// `fxtrait.rs`). Une version précédente refuse le projet : elle ne connaît pas ces sortes d'effet et
/// montrerait le dessin sans eux, le film changerait sans prévenir.
/// 32 (2026-10-20) : effets de matière dans la pile `effets` (liseré, trait sec, encre qui bave, dégradé le
/// long du trait, changer de plume ; voir `fxmatiere.rs`). Même raisonnement que pour la 31 : une version
/// précédente refuse le projet au lieu d'en montrer le dessin sans liseré, sans dégradé ni plume (et la
/// plume est désignée par un numéro tiré du nom du réglage de pinceau). Un projet de version 31 ou moins
/// s'ouvre tel quel.
/// 33 (2026-10-20, notes d'Anthony « calques / effets / montage ») : interrupteur « fx » d'un calque
/// (`effets_eteints` : la pile d'effets est éteinte d'un coup, sans être vidée). Une version précédente
/// refuse le projet : elle montrerait les effets d'un calque que le réalisateur a éteints, le film
/// changerait sans prévenir. Le champ est absent (donc allumé) tant qu'on n'éteint rien. Un projet de
/// version 32 ou moins s'ouvre tel quel, effets allumés. (Une autre branche peut aussi prendre le numéro 33
/// pour ses propres champs : chacun a son nom, les deux se lisent sous le même numéro.)
/// 33 (2026-10-20, notes d'Anthony « dessin ») : clés de position des calques (`cles_position` d'un calque :
/// pour chaque clé, l'image du plan, le décalage `x` `y`, l'`echelle`, la `rotation`, les étirements
/// (`etirement_x`, `etirement_y`), et `lisse`, `adoucir_entree`, `adoucir_sortie` ; voir `poskeys.rs`).
/// Une version précédente refuse le projet : elle montrerait le calque immobile à sa pose fixe, le film
/// changerait sans prévenir. Un projet de version 32 ou moins s'ouvre tel quel. (Si une autre branche a aussi
/// pris le numéro 33 pour ses propres champs, la fusion garde le 33 et réunit les deux descriptions.)
/// 35 (2026-10-20, `docs/ESPACE3D.md`) : **Espace 3D** — un calque est une carte posée dans l'espace : son inclinaison
/// (`inclinaison_x`, `inclinaison_y` de sa `transformation`, en degrés) et, dans chaque clé de position, `inclinaison_x`,
/// `inclinaison_y` et `ecart_profondeur` (écart à la `profondeur` du calque). Tous omis quand ils valent 0. Une version
/// précédente refuse le projet : elle montrerait les cartes inclinées à plat, le film changerait sans prévenir. Un projet de
/// version 34 ou moins s'ouvre tel quel, à l'identique (cartes à plat, aucun écart). Le numéro 34 est celui des projets
/// partagés (ci-dessous) : comme leur numéro est celui du format classique quand celui-ci le dépasse, un projet partagé écrit
/// désormais 35 (`version_partage_ecrite`).
/// 36 (2026-10-21, « On fusionne », `docs/INTERFACE.md`) : **le calque CTG est fondu dans le calque de couleur** — un calque
/// de couleur (`calque_couleur`) peut remplir des zones d'un coup de griffonnage : `griffonnages` (couples dessin de peinture /
/// dessin de griffonnages, dont les fichiers sont des dessins ordinaires de `dessins/`), `remplissage` (le calque a des
/// griffonnages), `griffonner` (le mode), `sources` (rangs des calques dont les traits ferment aussi les zones) ; un calque de
/// couleur « libre » n'a pas de `trait` (ses images sont à lui). Le champ `ctg` d'un calque n'est plus écrit : un projet d'avant
/// qui en a un est lu comme avant, et son calque CTG devient un calque de couleur qui montre la même image
/// (`griffonnage::migrer_ctg`) ; ses fichiers de graffitis servent tels quels de dessins de griffonnages. Une version
/// précédente refuse le projet : elle montrerait les calques de couleur sans leur remplissage, le film changerait sans
/// prévenir. Un projet de version 35 ou moins s'ouvre tel quel, à l'identique.
/// 37 (2026-10-24, `docs/INTERFACE.md` partie 47) : **postérisation temporelle de la caméra** d'un plan (`saccade` d'un plan :
/// `actif`, `cadence` en images, au moins 2) : le mouvement de caméra est montré par paliers. Omise quand elle est éteinte avec
/// la cadence 2. Une version précédente refuse le projet : elle montrerait le mouvement de caméra en douceur, le film changerait
/// sans prévenir. Un projet de version 36 ou moins s'ouvre tel quel, la postérisation éteinte. (Si une autre branche a aussi
/// pris le numéro 37, la fusion garde le 37 et réunit les deux descriptions.)
/// 38 (2026-10-28, `docs/INTERFACE.md` partie 53, outil Dégradé) : **zone vectorielle en dégradé** — un élément `Fill` d'un dessin
/// vectoriel peut porter `degrade` (`genre` « lineaire » ou « radial », `depart` et `arrivee` : les deux points de l'axe, en
/// pixels du dessin, `couleur_depart`, `couleur_arrivee`, `opacite_depart`, `opacite_arrivee`) ; la zone se rend avec son dégradé,
/// et l'axe suit la forme quand elle se déplace, tourne ou se déforme. Omis : couleur unie, comme avant. Une version précédente
/// refuse le projet : elle montrerait ces zones en couleur unie, le film changerait sans prévenir. Un projet de version 37 ou
/// moins s'ouvre tel quel, à l'identique. (Si une autre branche a aussi pris le numéro 38, la fusion garde le 38 et réunit les
/// deux descriptions.)
/// 39 (2026-10-28, `docs/INTERFACE.md` partie 59) : **versions de séquence qui partagent leurs plans** — un plan existe une seule
/// fois, avec toutes ses versions (V1, V2…), et chaque version de séquence (A, B…) n'est plus qu'un découpage : les plans qu'elle
/// montre, leur ordre, leur durée et **la version de chaque plan** qu'elle montre (`emplacements` de `variantes.json`, dont le
/// format passe à 2 ; les plans qu'aucune version choisie ne montre sont au `garage` du même fichier). Une version précédente
/// refuse le projet : elle ne saurait pas retrouver les plans que ces versions de séquence montrent sans les avoir à elles, et
/// montrerait un film différent sans prévenir. Un projet de version 38 ou moins s'ouvre tel quel, sans rien perdre : ses variantes
/// (copies complètes de plans) deviennent des plans du garage, montrés par leurs versions de séquence. (Si une autre branche a
/// aussi pris le numéro 39, la fusion garde le 39 et réunit les deux descriptions.)
/// 41 (2026-10-28, `docs/INTERFACE.md` partie 63) : **aplat intelligent** — le réglage `intelligent` de l'aplat d'un calque
/// (`FondReglages`, case « Aplat intelligent »), écrit dans le manifeste avec les autres réglages de l'aplat, seulement s'il est
/// allumé ; absent, il est éteint. Une version précédente qui ouvrirait le projet perdrait ce réglage (non les couleurs, qui sont de
/// la peinture ordinaire dans la strate de l'aplat) : elle le refuse donc, au lieu de l'effacer sans le dire. Un projet de version
/// 40 ou moins s'ouvre tel quel. (Le numéro 40 est réservé à une autre branche : la fusion garde le plus grand et réunit les
/// descriptions.)
/// 42 (2026-10-29, `docs/INTERFACE.md` partie 69) : **images collées en objets vectoriels** — l'élément `Image` d'un dessin vectoriel
/// (`vector::Element::Image` : empreinte du contenu, taille, placement, opacité), dont les pixels sont gardés une seule fois, en PNG,
/// dans `images/<empreinte>.png` du dossier du projet. Une version précédente qui ouvrirait le projet ne saurait pas lire ces
/// éléments (elle refuserait le dessin) : elle le refuse donc en entier. Un projet de version 41 ou moins s'ouvre tel quel.
/// 43 (2026-10-29, `docs/INTERFACE.md` partie 72) : **clés d'opacité et calque « Couleur unie »** — `cles_opacite` (l'opacité du calque
/// à certaines images du plan) et `couleur_unie` (le fond d'une seule couleur d'un calque) dans le manifeste de chaque calque. Une version
/// précédente qui ouvrirait le projet perdrait l'animation de l'opacité et le fond : elle le refuse donc. Un projet de version 42 ou
/// moins s'ouvre tel quel (sans clé d'opacité ni couleur unie).
/// 44 (2026-10-30, `docs/INTERFACE.md` partie 80) : **dégradés à plusieurs pastilles** — le champ `degrade` d'une zone vectorielle porte
/// `pastilles` (couleurs : position, couleur, milieu), `transparence` (opacités : position, opacité, milieu), `tramage`, et les formes
/// `angle`, `reflechi`, `losange` (`degrade.rs`). Une version précédente refuse le projet : elle lirait ces dégradés comme deux couleurs
/// (ou refuserait la forme), le film changerait sans prévenir. Un projet de version 43 ou moins s'ouvre tel quel, à l'identique : un
/// ancien dégradé (`couleur_depart`, `couleur_arrivee`, `opacite_depart`, `opacite_arrivee`) se lit comme deux pastilles de chaque, sans
/// tramage. (Une autre branche peut aussi prendre le numéro 44 : la fusion garde le plus grand et réunit les descriptions.)
/// 45 (2026-10-30, `docs/INTERFACE.md` partie 87) : **production** — `serie`, `episode` et `modele_noms` dans `proprietes` (le modèle de noms des
/// plans, `nommage.rs`). Une version précédente qui ouvrirait le projet perdrait le modèle de noms et le numéro d'épisode à l'enregistrement : elle
/// le refuse donc. Un projet de version 44 ou moins s'ouvre tel quel, à l'identique (modèle de noms d'origine : « SQ01_PL012A »).
/// 46 (2026-10-30, `docs/INTERFACE.md` partie 91) : **budget de durée** — `budget_ms` (la cible du film) et `budgets_sequences` (la cible de chaque
/// séquence) dans `proprietes` (`budget.rs`). Une version précédente qui ouvrirait le projet perdrait les cibles à l'enregistrement : elle le refuse
/// donc. Un projet de version 45 ou moins s'ouvre tel quel, à l'identique (aucune cible).
/// 47 (2026-11-06, `docs/INTERFACE.md` partie 104) : **corbeille du projet** — `corbeille.json` à côté du manifeste (`corbeille.rs`) : les plans, les
/// calques, les dessins recouverts et les images rognées que les gestes du montage retirent du film, avec leurs dessins gardés dans `dessins/`.
/// Une version précédente qui ouvrirait le projet effacerait ces dessins à l'enregistrement (rien ne les cite plus dans le manifeste) : elle le
/// refuse donc. Un projet de version 46 ou moins s'ouvre tel quel, sans rien perdre, corbeille vide.
/// 48 (2026-11-06, `docs/INTERFACE.md` partie 115) : **plans annulés** — `plans_annules.json` à côté du manifeste (`planannule.rs`) : des plans sortis du
/// film, gardés tels quels, avec leurs dessins dans `dessins/`. Une version précédente qui ouvrirait le projet effacerait ces dessins à l'enregistrement
/// (rien d'autre ne les cite) : elle le refuse donc. Un projet de version 47 ou moins s'ouvre tel quel, sans rien perdre, aucun plan annulé.
/// 49 (2026-11-07, `docs/INTERFACE.md` partie 119) : **trajectoire d'un calque** — `trajectoire` d'un calque (`trajectoire.rs`) : le chemin dessiné
/// (points en pixels de la feuille), ses marques et son pas, qui posent les clés de position du calque. Une version précédente qui ouvrirait le
/// projet garderait les clés mais perdrait le chemin à l'enregistrement : elle le refuse donc. Un projet de version 48 ou moins s'ouvre tel quel.
/// 50 (2026-11-08, `docs/INTERFACE.md` partie 119) : **le dessin tourne avec le chemin** — `tourne` de la trajectoire d'un calque et `angle` de
/// chacun de ses dessins posés (`dessins[]`). Une version précédente qui ouvrirait le projet perdrait la rotation à l'enregistrement (le dessin
/// repartirait droit au prochain calcul) : elle le refuse donc. Un projet de version 49 ou moins s'ouvre tel quel.
/// 51 (2026-11-08, `docs/INTERFACE.md` partie 126) : **la planche-mère** — `partage` d'un calque (`partage.rs`) : la clé que portent les
/// exemplaires d'un même calque partagé entre plusieurs plans (mêmes dessins, chacun sous sa caméra). Une version précédente qui ouvrirait le
/// projet en ferait des calques séparés qui partagent les mêmes dessins, et perdrait le lien à l'enregistrement : elle le refuse donc. Un
/// projet de version 50 ou moins s'ouvre tel quel.
pub const VERSION: u32 = 51;
/// 34 (2026-10-20, `docs/COOPERATION.md`) : **projet partagé** — le manifeste ne porte que la structure (séquences, ordre
/// des plans, pistes son, propriétés…) et un bloc `partage` ; chaque plan est dans `plans/<n°>.json`. Seul un projet
/// partagé porte un numéro à part : `max(VERSION_PARTAGE, VERSION)` (`version_partage_ecrite`) ; les autres s'écrivent et se
/// lisent comme avant. Une version de SHOT qui ne connaît pas le partage refuse le projet (« créé par une version plus
/// récente ») au lieu de montrer un film sans plans.
pub const VERSION_PARTAGE: u32 = 34;

/// Le numéro que porte la structure d'un projet partagé : le plus grand du numéro des projets partagés et du format classique
/// (un projet partagé contient aussi des plans classiques : il doit être refusé par toute version qui ne connaît pas ce
/// format, et rester lisible par celles qui connaissent le sien).
pub const fn version_partage_ecrite() -> u32 {
    if VERSION > VERSION_PARTAGE { VERSION } else { VERSION_PARTAGE }
}
const DRAWINGS: &str = "dessins";
/// Fils qui lisent ou écrivent les dessins d'un projet en parallèle : au-delà de huit, le disque et
/// la mémoire (une feuille décodée pèse 33 Mo) limitent plus que le processeur.
const LOAD_THREADS: usize = 8;
const SOUNDS: &str = "sons";
/// Images des calques vidéo (13k) : `videos/<id>/000001.jpg`…
const VIDEOS: &str = "videos";
/// Modèles 3D de repère (2d), dans le format de Pelure.
const MODELS: &str = "modeles";
/// Textures des traits vectoriels texturés (7b) : `pointe_<clé>.png`, `papier_<clé>.png`, la clé
/// venant du contenu (une texture ne change jamais sous la même clé).
const TEXTURES: &str = "textures";
/// Formats de son acceptés à l'import.
pub const SOUND_EXTS: [&str; 5] = ["wav", "mp3", "flac", "ogg", "oga"];

#[derive(Serialize, Deserialize)]
pub(crate) struct Manifest {
    format: String,
    version: u32,
    /// Taille de la feuille de dessin (et de chaque fichier de `dessins/`).
    largeur: usize,
    hauteur: usize,
    /// Cadre de la caméra = taille de l'image finale. Absent (format 3) : la feuille était le
    /// cadre ; elle est alors doublée à l'ouverture, dessin au centre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cadre: Option<[usize; 2]>,
    cadence: u32,
    papier: [u8; 3],
    sequences: Vec<SequenceEntry>,
    /// Fichiers son, copiés tels quels dans `sons/` (ajouté en 1b ; absent = aucun son).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sons: Vec<SoundEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pistes_son: Vec<TrackEntry>,
    /// Guides de perspective (ajouté en 1c).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    guides: Vec<Guide>,
    /// Notes de la planche (format 8) : traits de la vue Vignettes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    planche: Vec<NoteEntry>,
    /// Textes de la planche (format 9).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    planche_textes: Vec<NoteTextEntry>,
    /// Groupes de couleur des calques (format 13 ; absents : ceux d'un projet neuf).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    groupes: Vec<GroupEntry>,
    /// Numéro de la première image (format 16 ; absent : 1).
    #[serde(default = "one_usize", skip_serializing_if = "is_one_usize")]
    image_de_depart: usize,
    /// Arrière-plan (format 17 ; absent : la couleur du papier).
    #[serde(default, skip_serializing_if = "BackgroundEntry::is_default")]
    fond: BackgroundEntry,
    /// Volume général du son, piste maîtresse (format 17 ; absent : 1).
    #[serde(default = "one", skip_serializing_if = "is_one_f32")]
    volume_general: f32,
    /// Storyboard verrouillé (format 17 ; absent : non).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    storyboard_verrouille: bool,
    /// Numérotation des plans verrouillée (`numerotation.rs` ; absent : suivie, sans changer `VERSION`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    numerotation_verrouillee: bool,
    /// Propriétés du projet (format 17 ; absentes : vides).
    #[serde(default, skip_serializing_if = "PropertiesEntry::is_empty")]
    proprietes: PropertiesEntry,
    /// Dessins bitmap qui ont quelque chose hors de la feuille (format 30, toile infinie) : leur atlas
    /// `dessins/<id>_hors.png` et son index `<id>_hors.json` ; absent : aucun.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dessins_hors: Vec<u64>,
    /// Projet partagé (format 34, `projet_partage.rs`) : le manifeste n'est que la structure, les plans sont dans
    /// `plans/`. Absent : projet classique.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    partage: Option<partage_io::BlocPartage>,
}

/// Propriétés du projet (format 17). Dates en secondes depuis 1970.
#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
struct PropertiesEntry {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    titre: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    informations: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    auteur: String,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    cree: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    modifie: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    notes: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    copyright: String,
    /// Couleur des fonds sous le trait qu'on allume (format 23) ; absente : blanc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fond: Option<[u8; 3]>,
    /// Production (format 45) : la série, l'épisode et le modèle de noms des plans (`nommage.rs`) ; absents : vides.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    serie: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    episode: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    modele_noms: String,
    /// Budget de durée (format 46, `budget.rs`) : la cible du film et celle de chaque séquence, en millisecondes ; absentes : aucune.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    budget_ms: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    budgets_sequences: Vec<BudgetSequenceEntry>,
}

/// La cible de durée d'une séquence (format 46).
#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct BudgetSequenceEntry {
    sequence: u64,
    ms: u32,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl PropertiesEntry {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

fn is_one_f32(v: &f32) -> bool {
    *v == 1.0
}

/// Arrière-plan (format 17) : sorte et couleur B du damier (la couleur A est `papier`).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
struct BackgroundEntry {
    #[serde(default)]
    sorte: crate::document::BackgroundKind,
    #[serde(default = "default_paper_b")]
    couleur_b: [u8; 3],
}

impl Default for BackgroundEntry {
    fn default() -> Self {
        Self { sorte: Default::default(), couleur_b: crate::document::DEFAULT_PAPER_B }
    }
}

impl BackgroundEntry {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

fn default_paper_b() -> [u8; 3] {
    crate::document::DEFAULT_PAPER_B
}

fn one_usize() -> usize {
    1
}

fn is_one_usize(v: &usize) -> bool {
    *v == 1
}

/// Un groupe de couleur des calques (format 13), dans l'ordre des numéros de groupe.
#[derive(Serialize, Deserialize)]
struct GroupEntry {
    nom: String,
    couleur: [u8; 3],
    #[serde(default = "yes")]
    montre: bool,
}

/// Un trait de note : couleur et points (x, y, rayon), en coordonnées de la planche, ou, s'il
/// est attaché à une vignette (format 9, `attache` : id du panneau), relatifs à elle (en
/// largeurs de vignette).
#[derive(Serialize, Deserialize)]
struct NoteEntry {
    couleur: [u8; 3],
    points: Vec<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attache: Option<u64>,
}

/// Un texte de note (format 9) : coin haut gauche, taille, mêmes règles d'attache.
#[derive(Serialize, Deserialize)]
struct NoteTextEntry {
    texte: String,
    position: [f32; 2],
    taille: f32,
    couleur: [u8; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attache: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct SoundEntry {
    id: u64,
    /// Nom du fichier d'origine (affiché).
    #[serde(default)]
    nom: String,
    /// Fichier dans `sons/`.
    fichier: String,
}

#[derive(Serialize, Deserialize)]
struct TrackEntry {
    id: u64,
    #[serde(default)]
    nom: String,
    #[serde(default)]
    muette: bool,
    #[serde(default = "one")]
    volume: f32,
    #[serde(default)]
    extraits: Vec<ClipEntry>,
    /// Piste verrouillée (format 17 ; absent : non).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    verrouillee: bool,
}

/// Un extrait de son, en images du montage.
#[derive(Serialize, Deserialize)]
struct ClipEntry {
    id: u64,
    son: u64,
    debut: usize,
    /// Images sautées au début du son.
    #[serde(default)]
    decalage: usize,
    duree: usize,
}

#[derive(Serialize, Deserialize)]
struct SequenceEntry {
    id: u64,
    #[serde(default)]
    nom: String,
    /// Numéro propre, en numérotation verrouillée (`numerotation.rs` ; absent : le rang).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    numero: String,
    scenes: Vec<SceneEntry>,
}

#[derive(Serialize, Deserialize)]
struct SceneEntry {
    id: u64,
    #[serde(default)]
    nom: String,
    /// Numéro propre du plan, en numérotation verrouillée (absent : le rang).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    numero: String,
    panneaux: Vec<PanelEntry>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct PanelEntry {
    id: u64,
    #[serde(default)]
    duree: usize,
    #[serde(default)]
    action: String,
    #[serde(default)]
    dialogue: String,
    #[serde(default)]
    notes: String,
    calques: Vec<LayerEntry>,
    /// Images clés de la caméra (absent : caméra au centre de la feuille).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    camera: Vec<CameraEntry>,
    /// Postérisation temporelle de la caméra (format 37 ; absente : éteinte, cadence 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    saccade: Option<SaccadeEntry>,
    /// Marques d'image du plan (format 12 ; absent : aucune).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    marques: Vec<MarkEntry>,
    /// Signets, points d'entrée et de sortie (format 16 ; absents : aucun).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    signets: Vec<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    entree: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sortie: Option<usize>,
    /// Couleur de repère du plan, rang dans `marks::COLORS` (format 24 ; absente : aucune).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    couleur: Option<u8>,
    /// Notes dessinées sur la timeline (format 17 ; absentes : aucune).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notes_dessinees: Vec<TimeNoteEntry>,
}

/// Postérisation temporelle de la caméra d'un plan (format 37) : allumée ou non, et « toutes les N images ».
#[derive(Serialize, Deserialize)]
struct SaccadeEntry {
    #[serde(default)]
    actif: bool,
    cadence: usize,
}

/// Une note dessinée (format 17) : couleur (0 bleu, 1 rouge, 2 noir), épaisseur, points
/// (image, hauteur de 0 à 1).
#[derive(Serialize, Deserialize)]
struct TimeNoteEntry {
    couleur: u8,
    epaisseur: f32,
    points: Vec<[f32; 2]>,
}

/// Une marque d'image (13a) : une image de storyboard à l'image `image` du plan, avec ses textes.
#[derive(Serialize, Deserialize)]
struct MarkEntry {
    id: u64,
    image: usize,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    couleur: u8,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    action: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    dialogue: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    notes: String,
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

#[derive(Serialize, Deserialize)]
struct TransformEntry {
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
    #[serde(default = "one")]
    echelle: f32,
    #[serde(default)]
    rotation: f32,
    /// Étirement en largeur et en hauteur (22 ; absent = aucun).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    etirement_x: f32,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    etirement_y: f32,
    /// Inclinaison de la carte dans l'Espace 3D, en degrés (35 ; absente = à plat).
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    inclinaison_x: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    inclinaison_y: f32,
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

/// Une image clé de caméra : à l'image `image` du panneau, centre (`x`, `y`) en pixels de la
/// feuille, zoom, rotation en degrés.
#[derive(Serialize, Deserialize)]
struct CameraEntry {
    image: usize,
    x: f32,
    y: f32,
    #[serde(default = "one")]
    zoom: f32,
    #[serde(default)]
    rotation: f32,
    /// Mouvement adouci à cette clé.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    doux: bool,
    /// Avancée de la caméra vers la feuille, en pixels (format 10 ; absent : 0).
    #[serde(default, skip_serializing_if = "is_zero")]
    avance: f32,
    /// Adoucis d'entrée et de sortie, en images (format 10 ; absent : 0).
    #[serde(default, skip_serializing_if = "is_zero")]
    adoucir_entree: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    adoucir_sortie: f32,
    /// Bascule (vers le haut) et panoramique (vers la droite), en degrés (format 11 ; absents : 0).
    #[serde(default, skip_serializing_if = "is_zero")]
    bascule: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    panoramique: f32,
}

#[derive(Serialize, Deserialize)]
struct LayerEntry {
    nom: String,
    /// « vectoriel » ou « bitmap ».
    #[serde(rename = "type")]
    kind: String,
    #[serde(default = "yes")]
    visible: bool,
    #[serde(default = "one")]
    opacite: f32,
    /// Ajouté en 1e (absent = non verrouillé).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    verrouille: bool,
    /// Transformation du calque (2c ; absent = aucune) : décalage en pixels de la feuille,
    /// échelle, rotation en degrés, autour du centre de la feuille.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transformation: Option<TransformEntry>,
    /// Clés de position (format 33, `poskeys.rs`) : la pose du calque à certaines images du plan, qui
    /// l'animent ; absentes : le calque a sa pose fixe (`transformation`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cles_position: Vec<crate::poskeys::Entree>,
    /// Trajectoire (format 49, `trajectoire.rs`) : le chemin dessiné et ses marques, qui posent les clés de position ; absente : les clés
    /// sont à la main.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    trajectoire: Option<crate::trajectoire::Trajectoire>,
    /// Clés d'opacité (format 43, `opacitecles.rs`) : l'opacité du calque à certaines images du plan ; absentes : `opacite` fixe.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cles_opacite: Vec<crate::opacitecles::Entree>,
    /// Couleur unie du calque (format 43, `couleurunie.rs`) : le fond d'une seule couleur, sous les dessins du calque ; absente : calque ordinaire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    couleur_unie: Option<[u8; 3]>,
    /// Profondeur multiplan (2c ; absent = 0, plan de référence).
    #[serde(default, skip_serializing_if = "is_zero")]
    profondeur: f32,
    /// Calque de toute la scène (6f ; absent = calque du seul panneau) : ses exemplaires, un par
    /// panneau de la scène, portent le même numéro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    toute_la_scene: Option<u64>,
    /// La planche-mère (format 51, `partage.rs` ; absent : calque du seul plan) : les exemplaires d'un même calque partagé entre
    /// plusieurs plans portent le même numéro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    partage: Option<u64>,
    /// Mode de fusion (format 9 ; absent : normal).
    #[serde(default, skip_serializing_if = "is_normal")]
    fusion: crate::blend::BlendMode,
    /// Réglages de l'en-tête du calque (format 13 ; absents : éteints, groupe 0) : pelure
    /// d'oignon du calque, préserver la transparence, pochoir (et négatif), position
    /// verrouillée, groupe de couleur, ligne réduite dans la timeline.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pelure: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    transparence: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pochoir: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pochoir_negatif: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    position_verrouillee: bool,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    groupe: u8,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    replie: bool,
    /// Dossier de calques (format 14 ; absent : calque ordinaire), et rang dans le plan du
    /// dossier qui range ce calque (absent : hors dossier).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    dossier: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dans_dossier: Option<usize>,
    /// Pré / post-comportement (format 15 ; absents : rien).
    #[serde(default, skip_serializing_if = "is_nothing")]
    avant: Behaviour,
    #[serde(default, skip_serializing_if = "is_nothing")]
    apres: Behaviour,
    /// Cassure automatique d'instance (absente : non) et création automatique d'image
    /// (absente : oui), format 15.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    cassure_auto: bool,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    creation_auto: bool,
    /// Noms d'instance (format 15) : numéro de dessin → nom.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    noms: std::collections::BTreeMap<u64, String>,
    /// Marques de couleur (format 16) : numéro de dessin → rang de la couleur.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    marques_couleur: std::collections::BTreeMap<u64, u8>,
    /// Rôles des images (format 18) : image du calque → rôle.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    roles: crate::roles::Roles,
    /// Pile d'effets (format 19) : sorte, actif, valeurs, clés (image, valeurs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    effets: Vec<crate::fx::Effect>,
    /// Interrupteur « fx » du calque (format 33) : vrai, la pile `effets` est éteinte d'un coup sans être
    /// vidée. Absent : allumée.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    effets_eteints: bool,
    /// Calque CTG (format 20 à 35, **lu seulement** : le format 36 ne l'écrit plus) : rangs de ses calques sources dans le
    /// plan (aucun : tous les autres calques de dessin) ; absent : calque ordinaire. Un calque qui en a un devient un
    /// calque de couleur à la lecture (`griffonnage::migrer_ctg`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ctg: Option<CtgEntry>,
    /// Calque vidéo (format 21) ; absent : calque ordinaire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    video: Option<VideoEntry>,
    /// Fond sous le trait (format 23) : couleur, débord, fermeture des trous, bord ; absent : pas
    /// de fond. Ce qu'on a peint dans le fond d'un dessin : `dessins/<id>_fond.png`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fond: Option<crate::fond::FondReglages>,
    /// Calque d'effet (format 27) : ajustement ou incrustation ; absent : calque ordinaire. Ses
    /// expositions sont les images où il agit (un dessin vide tenu).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    calque_effet: Option<crate::calqueffet::Entree>,
    /// Calque de couleur (format 30) : il colore le calque de trait de ce rang ; absent : calque
    /// ordinaire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    calque_couleur: Option<CouleurEntry>,
    /// Calque Décor du plan (`decor.rs`) : un par plan, en bas, tenu sur toutes les images ; absent : calque ordinaire. Sans changer
    /// `VERSION` (une version d'avant l'ouvre comme un calque ordinaire nommé « Décor »).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    decor: bool,
    /// Le personnage du calque (`calquespastilles.rs` : le numéro d'un personnage de `personnages.json`) ; absent : calque ordinaire. Sans changer
    /// `VERSION` (une version d'avant l'ouvre comme un calque vectoriel ordinaire, au nom du personnage).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    personnage: Option<u64>,
    /// Calque d'annotation du plan (`calqueannot.rs`) : des notes, jamais dans les exports ; absent : calque ordinaire. Sans changer `VERSION`
    /// (une version d'avant l'ouvre comme un calque vectoriel ordinaire, avec ses notes).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    annotation: bool,
    /// Les expositions dans l'ordre : quel dessin (fichier `dessins/<id>.*`, ou aucun) et pendant
    /// combien d'images.
    expositions: Vec<Exposure>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct VideoEntry {
    id: u64,
    nom: String,
    images: usize,
    ips: f64,
    /// Image du plan où commence la vidéo (négative : le début du plan a été rogné).
    #[serde(default)]
    debut: i64,
    ext: String,
}

/// Le lien d'un calque de couleur avec son calque de trait (format 30), et ses réglages.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct CouleurEntry {
    /// Rang, dans le plan, du calque de trait ; absent (format 36) : calque de couleur libre, sans calque de trait propre.
    #[serde(rename = "trait", default, skip_serializing_if = "Option::is_none")]
    ligne: Option<usize>,
    /// Les couples (dessin du trait, dessin de couleur qui lui répond).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dessins: Vec<[u64; 2]>,
    /// Fermeture des trous (pixels de chaque côté du trou).
    #[serde(default = "default_couleur_trous")]
    trous: u8,
    /// Un trait de pinceau reste dans la zone fermée où il commence.
    #[serde(default = "yes")]
    zone: bool,
    /// Mode « Griffonner » (format 36).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    griffonner: bool,
    /// Le calque a des griffonnages, donc un remplissage (format 36).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    remplissage: bool,
    /// Les couples (dessin de peinture, dessin de griffonnages) (format 36) : ce sont des dessins ordinaires de `dessins/`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    griffonnages: Vec<[u64; 2]>,
    /// Rangs des calques qui ferment aussi les zones du remplissage (format 36).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sources: Vec<usize>,
}

fn default_couleur_trous() -> u8 {
    crate::calquecouleur::TROUS
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct CtgEntry {
    #[serde(default)]
    sources: Vec<usize>,
    /// Fermeture automatique des trous, en pixels (absente : celle d'un calque de couleur neuf).
    #[serde(default = "default_couleur_trous")]
    trou: u8,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
struct Exposure {
    dessin: Option<u64>,
    images: usize,
}

fn to_exposures(frames: &[Option<u64>]) -> Vec<Exposure> {
    let mut out: Vec<Exposure> = Vec::new();
    for &f in frames {
        match out.last_mut() {
            Some(last) if last.dessin == f => last.images += 1,
            _ => out.push(Exposure { dessin: f, images: 1 }),
        }
    }
    out
}

fn from_exposures(runs: &[Exposure]) -> Vec<Option<u64>> {
    // Au plus la durée d'un plan (`rognage::MAX_PLAN`) : un manifeste abîmé qui annonce des milliards
    // d'images remplirait la mémoire.
    runs.iter()
        .flat_map(|r| std::iter::repeat_n(r.dessin, r.images.min(crate::rognage::MAX_PLAN)))
        .take(crate::rognage::MAX_PLAN)
        .collect()
}

fn yes() -> bool {
    true
}

fn one() -> f32 {
    1.0
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn is_true(b: &bool) -> bool {
    *b
}

fn is_nothing(b: &Behaviour) -> bool {
    *b == Behaviour::Nothing
}

fn is_normal(m: &crate::blend::BlendMode) -> bool {
    *m == crate::blend::BlendMode::Normal
}

/// L'extension du manifeste d'un projet (Anthony, 2026-10-23 : le logiciel s'appelle SHOT). Un projet enregistré
/// s'écrit `MonFilm/MonFilm.shot`.
pub const EXTENSION: &str = "shot";
/// L'extension d'avant (`MonFilm/MonFilm.pelure`) : toujours ouverte. Un projet classique enregistré passe à `.shot`
/// (`App::save_then`) ; un film partagé la garde (`target_for_partage`).
pub const ANCIENNE_EXTENSION: &str = "pelure";
/// Les extensions qu'« Ouvrir », le glisser-déposer et la ligne de commande acceptent comme projet SHOT.
pub const EXTENSIONS: [&str; 2] = [EXTENSION, ANCIENNE_EXTENSION];

fn extension_est(path: &Path, ext: &str) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// Un manifeste de projet SHOT (`.shot`, ou l'ancien `.pelure`) ?
pub fn est_un_projet(path: &Path) -> bool {
    EXTENSIONS.iter().any(|e| extension_est(path, e))
}

/// Un manifeste à l'ancienne extension (`.pelure`) ?
pub fn est_ancien_manifeste(path: &Path) -> bool {
    extension_est(path, ANCIENNE_EXTENSION)
}

/// Le manifeste `.shot` d'un projet : `Bus/Bus.pelure` → `Bus/Bus.shot` (même dossier ; un `.shot` reste tel quel).
pub fn manifeste_shot(path: &Path) -> PathBuf {
    if est_ancien_manifeste(path) {
        path.with_extension(EXTENSION)
    } else {
        path.to_path_buf()
    }
}

/// L'autre nom du même manifeste : `Bus.shot` ↔ `Bus.pelure` (rien pour un autre fichier).
pub fn manifeste_jumeau(path: &Path) -> Option<PathBuf> {
    if est_ancien_manifeste(path) {
        Some(path.with_extension(EXTENSION))
    } else if extension_est(path, EXTENSION) {
        Some(path.with_extension(ANCIENNE_EXTENSION))
    } else {
        None
    }
}

/// Le manifeste à ouvrir quand on demande `path`. Un projet converti (`.pelure` → `.shot`) peut être demandé sous son
/// ancien nom (récents, séance d'avant, raccourci) : c'est son `.shot`. Un plantage entre l'écriture du `.shot` et
/// l'effacement du `.pelure` laisse les deux : le plus récent est ouvert (le `.shot` à égalité). Lit le disque : pas sur
/// le fil de l'interface pour un disque qui peut ne pas répondre (l'ouverture, elle, le lit de toute façon).
pub fn manifeste_a_ouvrir(path: &Path) -> PathBuf {
    let Some(jumeau) = manifeste_jumeau(path) else { return path.to_path_buf() };
    let date = |p: &Path| fs::metadata(p).ok().filter(fs::Metadata::is_file).map(|m| m.modified().unwrap_or(std::time::UNIX_EPOCH));
    match (date(path), date(&jumeau)) {
        (Some(_), None) | (None, None) => path.to_path_buf(),
        (None, Some(_)) => jumeau,
        (Some(a), Some(b)) => {
            let (shot, pelure, t_shot, t_pelure) = if est_ancien_manifeste(path) { (jumeau, path.to_path_buf(), b, a) } else { (path.to_path_buf(), jumeau, a, b) };
            if t_pelure > t_shot {
                pelure
            } else {
                shot
            }
        }
    }
}

/// Le projet `path` est-il là (sous ce nom ou sous son jumeau, `manifeste_jumeau`) ?
pub fn manifeste_present(path: &Path) -> bool {
    path.exists() || manifeste_jumeau(path).is_some_and(|j| j.is_file())
}

/// La clé d'un projet pour le reconnaître sous ses deux noms (`Bus.shot`, `Bus.pelure`) et en ignorant la casse, comme
/// Windows : la liste des récents et les onglets ne le montrent qu'une fois.
pub fn cle_de_projet(path: &Path) -> String {
    let p = if est_un_projet(path) { path.with_extension("") } else { path.to_path_buf() };
    p.to_string_lossy().to_lowercase()
}

/// Le manifeste `ancien` est-il la structure d'un film partagé (`projet_partage.rs`) ? Un poste à une version d'avant la
/// lit sous ce nom : elle n'est jamais effacée par une conversion.
pub(crate) fn est_une_structure_partagee(ancien: &Path) -> bool {
    #[derive(serde::Deserialize)]
    struct Bloc {
        #[serde(default)]
        partage: Option<serde::de::IgnoredAny>,
    }
    let Ok(octets) = fs::read(ancien) else { return false };
    let texte = String::from_utf8_lossy(&octets);
    // (Illisible : on ne sait pas ce que c'est ; il reste, comme une structure.)
    serde_json::from_str::<Bloc>(texte.trim_start_matches('\u{feff}')).map_or(true, |b| b.partage.is_some())
}

/// Après l'écriture du manifeste `nouveau` (`.shot`, écrit et synchronisé sur le disque) : l'ancien `ancien` (`.pelure`
/// du même dossier) part. Rien ne se passe si le nouveau manque, si c'est le même fichier, si l'ancien n'est pas dans le
/// même dossier ou s'il est la structure d'un film partagé. Rend vrai si l'ancien a été effacé. Un échec (fichier tenu
/// par un autre programme) laisse les deux : l'ouverture prend le plus récent (`manifeste_a_ouvrir`), le prochain
/// enregistrement réessaie.
pub(crate) fn retirer_ancien_manifeste(nouveau: &Path, ancien: &Path) -> bool {
    let meme_dossier = nouveau.parent() == ancien.parent();
    if !meme_dossier || !est_ancien_manifeste(ancien) || est_ancien_manifeste(nouveau) || cle_de_projet(nouveau) != cle_de_projet(ancien) {
        return false;
    }
    if !nouveau.is_file() || !ancien.is_file() || est_une_structure_partagee(ancien) {
        return false;
    }
    fs::remove_file(ancien).is_ok()
}

/// Dossier et manifeste correspondant au nom choisi dans la boîte « Enregistrer sous ».
/// « C:\Films\Bus.shot » → dossier « C:\Films\Bus\ », manifeste « C:\Films\Bus\Bus.shot »
/// (ou directement dans « C:\Films\Bus\ » si l'on a choisi un fichier qui s'y trouve déjà). Le manifeste est
/// toujours un `.shot`, quelle que soit l'extension choisie.
pub fn target_for(chosen: &Path) -> (PathBuf, PathBuf) {
    target_for_ext(chosen, EXTENSION)
}

/// Comme `target_for`, pour un film partagé : sa structure garde le nom `.pelure` (des équipiers peuvent avoir une
/// version de SHOT d'avant `.shot`, qui ne cherche que celui-là ; `docs/COOPERATION.md`).
pub fn target_for_partage(chosen: &Path) -> (PathBuf, PathBuf) {
    target_for_ext(chosen, ANCIENNE_EXTENSION)
}

fn target_for_ext(chosen: &Path, ext: &str) -> (PathBuf, PathBuf) {
    let stem = chosen
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "projet".into());
    let parent = chosen.parent().map(Path::to_path_buf).unwrap_or_default();
    let dir = if parent.file_name().is_some_and(|n| n.to_string_lossy() == stem) {
        parent
    } else {
        parent.join(&stem)
    };
    let manifest = dir.join(format!("{stem}.{ext}"));
    (dir, manifest)
}

/// Écrire là est-il sans danger ? (pas d'écrasement d'un autre projet ni de données étrangères)
pub fn check_target(manifest: &Path, current: Option<&Path>) -> Result<(), String> {
    if current == Some(manifest) {
        return Ok(());
    }
    let dir = manifest.parent().unwrap_or(Path::new("."));
    // (Le même projet sous l'autre nom, `Bus.pelure` à côté de `Bus.shot`, est aussi un projet qui existe.)
    for m in std::iter::once(manifest.to_path_buf()).chain(manifeste_jumeau(manifest)) {
        if m.exists() {
            return Err(format!("un projet existe déjà à cet endroit ({}). Choisis un autre nom.", m.display()));
        }
    }
    for sub in [DRAWINGS, SOUNDS, MODELS, VIDEOS, crate::references::DIR] {
        if dir.join(sub).exists() {
            return Err(format!(
                "le dossier {} contient déjà un dossier « {sub} ». Choisis un autre nom.",
                dir.display()
            ));
        }
    }
    Ok(())
}

/// Écrit `bytes` dans `path` sans jamais laisser un fichier à moitié écrit : tout va d'abord dans
/// `<path>.tmp`, qui prend la place de `path` d'un seul geste (renommage, atomique sur Windows).
/// Un échec (disque plein, droits) ou un plantage au milieu laisse `path` tel qu'il était.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_atomic_with(path, bytes, false)
}

/// Comme `write_atomic`, en attendant que le disque ait reçu les octets avant le renommage : pour
/// le manifeste et les petits fichiers qui le suivent (une coupure de courant ne laisse pas un
/// fichier vide à la place d'un bon).
pub(crate) fn write_atomic_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_atomic_with(path, bytes, true)
}

/// Une texture de trait (pointe ou papier) en PNG de niveaux de gris, comme `textures/` la garde.
pub(crate) fn texture_png(img: &GrayImage) -> Result<Vec<u8>, String> {
    let gray = image::GrayImage::from_raw(img.width as u32, img.height as u32, crate::brushtex::quantized(img)).ok_or("texture de taille invalide")?;
    let mut bytes = Vec::new();
    gray.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(bytes)
}

fn write_atomic_with(path: &Path, bytes: &[u8], sync: bool) -> Result<(), String> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let written = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&tmp)?;
        std::io::Write::write_all(&mut file, bytes)?;
        if sync {
            file.sync_all()?;
        }
        Ok(())
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(format!("{} : {e}", tmp.display()));
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("{} : {e}", path.display())
    })
}

/// La peinture de la strate Fond du dessin `id` (format 23), à côté de lui.
fn fond_path(dir: &Path, id: CelId) -> PathBuf {
    dir.join(DRAWINGS).join(format!("{id}_fond.png"))
}

/// La coupe de la strate Fond du dessin `id` (où le fond est retiré à la gomme).
fn coupe_path(dir: &Path, id: CelId) -> PathBuf {
    dir.join(DRAWINGS).join(format!("{id}_coupe.png"))
}

/// Ce que le dessin bitmap `id` a hors de la feuille (toile infinie, format 30) : un PNG qui range
/// ses tuiles côte à côte (« atlas »), et leurs coordonnées de tuile dans un petit fichier JSON.
fn hors_png_path(dir: &Path, id: CelId) -> PathBuf {
    dir.join(DRAWINGS).join(format!("{id}_hors.png"))
}

fn hors_json_path(dir: &Path, id: CelId) -> PathBuf {
    dir.join(DRAWINGS).join(format!("{id}_hors.json"))
}

/// Les coordonnées des tuiles de l'atlas hors-feuille, dans l'ordre de l'atlas (de gauche à droite,
/// puis de haut en bas), et sa largeur en tuiles.
#[derive(Serialize, Deserialize)]
struct HorsIndex {
    colonnes: usize,
    tuiles: Vec<[i32; 2]>,
}

/// Le plus de tuiles qu'un atlas hors-feuille accepte à la lecture (un fichier abîmé qui en annonce
/// des millions ne doit pas remplir la mémoire).
const MAX_HORS_TUILES: usize = 60_000;

/// L'atlas des tuiles `tiles`, et son index.
fn hors_atlas(tiles: &[((i32, i32), crate::raster::Tile)]) -> (Raster, HorsIndex) {
    let cols = (tiles.len() as f64).sqrt().ceil().max(1.0) as usize;
    let rows = tiles.len().div_ceil(cols).max(1);
    let mut atlas = Raster::new(cols * crate::raster::TILE, rows * crate::raster::TILE);
    for (i, (_, tile)) in tiles.iter().enumerate() {
        let id = (i / cols) * atlas.tiles_x + i % cols;
        atlas.tiles[id] = Some(tile.clone());
    }
    (atlas, HorsIndex { colonnes: cols, tuiles: tiles.iter().map(|((x, y), _)| [*x, *y]).collect() })
}

/// Remet dans le dessin `cel` ce qu'il avait hors de la feuille (rien, sans bruit, si les fichiers
/// sont abîmés : un avertissement le dit).
fn read_hors(dir: &Path, id: CelId, cel: &mut Cel) -> Result<(), String> {
    let text = fs::read_to_string(hors_json_path(dir, id)).map_err(|e| format!("dessin {id} : ce qu'il a hors de la feuille est illisible ({e})"))?;
    let index: HorsIndex = serde_json::from_str(&text).map_err(|e| format!("dessin {id} : ce qu'il a hors de la feuille est illisible ({e})"))?;
    if index.tuiles.len() > MAX_HORS_TUILES || index.colonnes == 0 || index.colonnes > 4096 {
        return Err(format!("dessin {id} : ce qu'il a hors de la feuille est démesuré"));
    }
    let tile = crate::raster::TILE;
    let atlas = read_rgba(&hors_png_path(dir, id), |pw, ph, data| {
        (pw % tile == 0 && ph % tile == 0 && pw / tile >= index.colonnes && (ph / tile) * index.colonnes >= index.tuiles.len()).then(|| Raster::from_rgba(pw, ph, data))
    })
    .map_err(|e| format!("dessin {id} : ce qu'il a hors de la feuille est illisible ({e})"))?
    .ok_or_else(|| format!("dessin {id} : l'image de ce qu'il a hors de la feuille ne correspond pas à son index"))?;
    for (i, [tx, ty]) in index.tuiles.iter().enumerate() {
        let slot = (i / index.colonnes) * atlas.tiles_x + i % index.colonnes;
        if let Some(Some(t)) = atlas.tiles.get(slot) {
            cel.raster.absorb_beyond(*tx, *ty, t.clone());
        }
    }
    Ok(())
}

fn drawing_path(dir: &Path, id: CelId, vector: bool) -> PathBuf {
    dir.join(DRAWINGS).join(format!("{id}.{}", if vector { "json" } else { "png" }))
}

fn sound_file(sound: &Sound) -> String {
    format!("{}.{}", sound.id, sound.ext)
}

/// Sons utilisés par au moins un extrait.
fn used_sounds(doc: &Document) -> BTreeSet<u64> {
    doc.audio_tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(|c| c.sound))
        .filter(|id| doc.sounds.contains_key(id))
        .collect()
}

/// Ce que l'enregistrement retient d'un dessin (`Document::memo_enregistrement`) : a-t-il quelque chose hors de la
/// feuille (il faut lire les pixels du bord pour le savoir), quelles textures ses traits citent (il faut parcourir tous
/// ses traits). Valable tant que sa clé est celle du dessin : sa version (elle change à chaque modification, et
/// l'enregistrement s'y fie déjà pour ne pas réécrire un dessin), la taille de sa feuille et sa sorte.
#[derive(Clone)]
pub(crate) struct MemoDessin {
    cle: (u64, usize, usize, bool),
    hors: bool,
    textures: Vec<(TextureKind, u32)>,
    /// Les images collées que ses éléments citent (`imagevec.rs`).
    images: Vec<u64>,
}

#[cfg(test)]
thread_local! {
    /// Essais : les annexes et le manifeste sont réécrits même identiques, comme avant (banc avant / après).
    static TOUJOURS_ECRIRE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Essais : réécrire les fichiers identiques sur ce fil-ci (`true`), comme avant, ou non.
#[cfg(test)]
pub(crate) fn ecrire_toujours(oui: bool) {
    TOUJOURS_ECRIRE.with(|t| t.set(oui));
}

fn toujours_ecrire() -> bool {
    #[cfg(test)]
    return TOUJOURS_ECRIRE.with(|t| t.get());
    #[cfg(not(test))]
    false
}

/// Le hachage des clés du mémo (des numéros de dessins) : une multiplication, au lieu du SipHash de la bibliothèque
/// standard (le cliché d'un film de 600 plans consulte le mémo près de dix mille fois, sur le fil de l'interface).
#[derive(Default)]
pub(crate) struct HacheNumero(u64);

impl std::hash::Hasher for HacheNumero {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0.rotate_left(8) ^ u64::from(*b)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0 ^ n).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

/// Le mémo de l'enregistrement, par dessin (`Document::memo_enregistrement`).
pub(crate) type MemoDessins = HashMap<CelId, MemoDessin, std::hash::BuildHasherDefault<HacheNumero>>;

fn cle_memo(cel: &Cel) -> (u64, usize, usize, bool) {
    (cel.version, cel.raster.width, cel.raster.height, cel.vector.is_some())
}

fn memo_de(cel: &Cel) -> MemoDessin {
    let mut textures = BTreeSet::new();
    for e in cel.vector.iter().flat_map(|v| &v.elements) {
        if let crate::vector::Element::Stroke { brush: Some(b), .. } = e {
            textures.extend(b.tip.map(|k| (TextureKind::Tip, k)));
            textures.extend(b.paper.map(|k| (TextureKind::Paper, k)));
        }
    }
    let images = crate::imagevec::citees(cel.vector.iter().flat_map(|v| &v.elements));
    MemoDessin { cle: cle_memo(cel), hors: cel.vector.is_none() && cel.raster.has_beyond(), textures: textures.into_iter().collect(), images: images.into_iter().collect() }
}

#[cfg(test)]
thread_local! {
    /// Essais : le mémo est recalculé à chaque fois, comme avant lui (banc avant / après dans le même processus).
    static SANS_MEMO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
thread_local! {
    /// Essais : l'écriture s'arrête juste après le nouveau manifeste, avant l'effacement de l'ancien (un plantage là).
    pub(crate) static PLANTER_AVANT_RETRAIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Le mémo du dessin `id`, refait s'il ne correspond plus au dessin.
fn memo_a_jour<'m>(memo: &'m mut MemoDessins, id: CelId, cel: &Cel) -> &'m MemoDessin {
    let cle = cle_memo(cel);
    #[cfg(test)]
    if SANS_MEMO.with(|s| s.get()) {
        memo.remove(&id);
    }
    let m = memo.entry(id).or_insert_with(|| memo_de(cel));
    if m.cle != cle {
        *m = memo_de(cel);
    }
    m
}

impl Document {
    /// Le mémo de l'enregistrement pour tous les dessins (à l'ouverture, sur le fil d'ouverture : le premier
    /// enregistrement ne relit pas les pixels du bord de chaque dessin sur le fil de l'interface).
    pub(crate) fn memoriser_dessins(&self) {
        let mut memo = self.memo_enregistrement.lock().unwrap_or_else(|e| e.into_inner());
        for (&id, cel) in &self.cels {
            memo_a_jour(&mut memo, id, cel);
        }
    }
}

/// Les dessins bitmap parmi `used` qui ont quelque chose hors de la feuille (toile infinie). Un dessin
/// vectoriel ne garde rien à part : ses traits se rendent à la lecture, hors de la feuille aussi.
fn hors_of(doc: &Document, used: &BTreeSet<CelId>) -> BTreeSet<CelId> {
    let mut memo = doc.memo_enregistrement.lock().unwrap_or_else(|e| e.into_inner());
    used.iter().copied().filter(|id| doc.cels.get(id).is_some_and(|c| c.vector.is_none() && memo_a_jour(&mut memo, *id, c).hors)).collect()
}

/// Textures citées par les traits vectoriels texturés des dessins `used` : (sorte, clé).
fn used_textures(doc: &Document, used: &BTreeSet<CelId>) -> BTreeSet<(TextureKind, u32)> {
    let mut memo = doc.memo_enregistrement.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = BTreeSet::new();
    for &id in used {
        if let Some(cel) = doc.cels.get(&id).filter(|c| c.vector.is_some()) {
            out.extend(memo_a_jour(&mut memo, id, cel).textures.iter().copied());
        }
    }
    out
}

/// Images collées citées par les éléments des dessins vectoriels `used`.
fn used_images(doc: &Document, used: &BTreeSet<CelId>) -> BTreeSet<u64> {
    let mut memo = doc.memo_enregistrement.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = BTreeSet::new();
    for &id in used {
        if let Some(cel) = doc.cels.get(&id).filter(|c| c.vector.is_some()) {
            out.extend(memo_a_jour(&mut memo, id, cel).images.iter().copied());
        }
    }
    out
}

/// Les images citées qui manquent dans `images/` du dossier `dir` (et dont on a les pixels) : à écrire.
pub(crate) fn images_a_ecrire(dir: &Path, used: &BTreeSet<u64>) -> Vec<(PathBuf, Arc<crate::imagevec::Pixels>)> {
    used.iter()
        .map(|&key| (crate::imagevec::fichier(dir, key), key))
        .filter(|(path, _)| !path.exists())
        .filter_map(|(path, key)| crate::imagevec::obtenir(key).map(|px| (path, px)))
        .collect()
}

fn texture_file(dir: &Path, kind: TextureKind, key: u32) -> PathBuf {
    dir.join(TEXTURES).join(format!("{}_{key}.png", kind.file_word()))
}

/// Lit les textures du projet et les met à la disposition des traits vectoriels.
fn load_textures(dir: &Path, warnings: &mut Vec<String>, nouvelles_seulement: bool) {
    let Ok(entries) = fs::read_dir(dir.join(TEXTURES)) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some((word, key)) = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.split_once('_')) else { continue };
        let kind = match word {
            "pointe" => TextureKind::Tip,
            "papier" => TextureKind::Paper,
            _ => continue,
        };
        let Ok(key) = key.parse::<u32>() else { continue };
        // (Une texture ne change jamais sous la même clé : un plan relu seul ne relit pas celles qu'il a déjà.)
        if nouvelles_seulement && crate::brushtex::has_shared(kind, key) {
            continue;
        }
        match image::open(&path) {
            Ok(img) => {
                let gray = img.to_luma8();
                let values = gray.as_raw().iter().map(|&v| v as f32 / 255.0).collect();
                crate::brushtex::share_texture(kind, key, GrayImage::new(gray.width() as usize, gray.height() as usize, values));
            }
            Err(e) => warnings.push(format!("texture {} illisible ({e}) : traits lisses à la place", path.display())),
        }
    }
}

/// Efface les textures du dossier qui ne servent plus.
fn remove_unused_textures(dir: &Path, used: &BTreeSet<(TextureKind, u32)>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(TEXTURES)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some((word, key)) = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.split_once('_')) else { continue };
        let kind = match word {
            "pointe" => TextureKind::Tip,
            "papier" => TextureKind::Paper,
            _ => continue,
        };
        let is_png = path.extension().and_then(|e| e.to_str()) == Some("png");
        if let (true, Ok(key)) = (is_png, key.parse::<u32>()) {
            if !used.contains(&(kind, key)) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

pub struct SaveReport {
    pub written: usize,
    pub removed: usize,
    /// Les instantanés d'envois écrits (`envois.rs`) : l'application libère leur mémoire.
    pub snapshots: Vec<u64>,
    /// Projet partagé : ce qui a été écrit (`projet_partage.rs`).
    pub partage: Option<partage_io::ResultatPartage>,
    /// Des dessins que `written` disait écrits dans le dossier n'y étaient plus (effacés à côté de SHOT) : le cliché
    /// n'avait pas leurs pixels, ils n'ont pas été écrits. À retirer de `written` et à enregistrer de nouveau tout de
    /// suite (`App::finish_save`, `project::save`, `Recovery::tick`) : c'est ce que l'enregistrement d'avant faisait
    /// d'un seul coup, quand il regardait le dossier sur le fil de l'interface.
    pub manquants: Vec<CelId>,
}

/// Enregistre le projet. `written` retient la version de chaque dessin déjà écrit dans ce dossier.
/// En cas d'échec, `written` est vidé : le prochain enregistrement réécrira tout.
pub fn save(doc: &Document, manifest_path: &Path, written: &mut HashMap<CelId, u64>) -> Result<SaveReport, String> {
    let job = prepare_save(doc, manifest_path, written, None)?;
    let mut report = job.run().inspect_err(|_| written.clear())?;
    // Des dessins manquaient dans le dossier : ils s'écrivent maintenant (ils sont dans le cliché suivant).
    if !report.manquants.is_empty() {
        for id in &report.manquants {
            written.remove(id);
        }
        let suite = prepare_save(doc, manifest_path, written, None)?.run().inspect_err(|_| written.clear())?;
        report.written += suite.written;
        report.removed += suite.removed;
        report.snapshots.extend(suite.snapshots);
        report.manquants = suite.manquants;
    }
    Ok(report)
}

/// Un dessin à écrire : déjà converti (vectoriel), copié tel quel (bitmap, converti en PNG à
/// l'écriture, la partie lente), ou à recopier d'un autre dossier où il est déjà écrit.
enum DrawingData {
    Bytes(Vec<u8>),
    Raster(Raster),
    /// Les tuiles d'un dessin hors de la feuille (toile infinie), rangées en atlas à l'écriture.
    Atlas(Vec<((i32, i32), crate::raster::Tile)>),
    CopyFrom(PathBuf),
}

/// Dossier d'un projet déjà enregistré, et versions des dessins qui y sont écrits : un dessin
/// inchangé depuis s'y recopie au lieu d'être réencodé.
pub struct Reuse<'a> {
    pub manifest_path: &'a Path,
    pub written: &'a HashMap<CelId, u64>,
}

/// Ce qu'un enregistrement doit écrire, préparé sur le fil de l'interface (copie des dessins
/// modifiés seulement) : l'écriture, elle, peut se faire sur un autre fil (sauvegarde de
/// secours) pendant qu'on continue à dessiner.
pub struct SaveJob {
    manifest_path: PathBuf,
    drawings: Vec<(PathBuf, DrawingData)>,
    sounds: Vec<(PathBuf, Arc<Vec<u8>>)>,
    textures: Vec<(PathBuf, GrayImage)>,
    /// Images collées dans des dessins vectoriels (`imagevec.rs`) qui manquent dans `images/` : leur PNG s'encode à l'écriture.
    images: Vec<(PathBuf, Arc<crate::imagevec::Pixels>)>,
    /// Images collées citées par les dessins du projet : celles de `images/` qui n'y sont pas ne servent plus.
    used_images: BTreeSet<u64>,
    /// Images des calques vidéo à recopier dans le dossier du projet (13k) : d'où, vers où.
    videos: Vec<(crate::video::VideoClip, PathBuf, PathBuf)>,
    manifest: Vec<u8>,
    /// Des dessins du dossier n'ont pas pu être lus à l'ouverture : leurs fichiers (et les textures
    /// qu'ils pourraient utiliser) ne sont pas effacés.
    has_unreadable: bool,
    /// Fichiers de sons et de modèles 3D du dossier que cette version n'a pas pu lire à l'ouverture :
    /// l'enregistrement ne les efface pas (ils ne sont plus dans le projet, mais ils sont à
    /// Anthony : une autre version les lira, ou il les réparera).
    kept: BTreeSet<PathBuf>,
    /// Dessins à garder dans le dossier : ceux du projet, et les illisibles.
    used: BTreeSet<CelId>,
    /// Dessins qui ont une strate Fond peinte (format 23).
    fonds: BTreeSet<CelId>,
    /// Dessins bitmap qui ont quelque chose hors de la feuille (toile infinie, format 30).
    hors: BTreeSet<CelId>,
    used_sounds: BTreeSet<u64>,
    used_textures: BTreeSet<(TextureKind, u32)>,
    /// Le fichier des commentaires (`commentaires.json`) ; rien : il n'y en a plus, le fichier part.
    comments: Option<Vec<u8>>,
    /// Le fichier du contenu rogné (`rognages.json`) ; rien : il n'y en a plus, le fichier part.
    reserves: Option<Vec<u8>>,
    /// Le fichier des transitions (`transitions.json`) ; rien : il n'y en a plus, le fichier part.
    transitions: Option<Vec<u8>>,
    /// Le fichier des effets des sons (`effets_son.json`) ; rien : il n'y en a plus, le fichier part.
    sound_fx: Option<Vec<u8>>,
    /// Le fichier des personnages (`personnages.json`) ; rien : il n'y en a plus, le fichier part.
    personnages: Option<Vec<u8>>,
    /// Le fichier des annotations de la vue Vignettes (`annotations.json`) ; rien : il n'y en a plus, le fichier part.
    annotations: Option<Vec<u8>>,
    /// Le fichier du scénario gardé (`scenario.json`) ; rien : il n'y en a plus (un scénario lisible n'est jamais effacé : il passe à côté).
    scenario: Option<Vec<u8>>,
    /// Le fichier des sons proposés (`sons_proposes.json`) ; rien : il n'y en a plus, le fichier part.
    sons_proposes: Option<Vec<u8>>,
    /// Les références visuelles : fichiers à recopier dans `references/`, `references.json`, et ce
    /// qui peut s'effacer (`references.rs`).
    references: crate::references::SavePlan,
    /// Le fichier des envois (`envois.json`) ; rien : il n'y en a pas (un fichier illisible reste là).
    envois: Option<Vec<u8>>,
    /// Le fichier des autres versions des plans (`plans_versions.json`) ; rien : il n'y en a pas.
    plans_versions: Option<Vec<u8>>,
    /// Le fichier des variantes rangées des séquences (`variantes.json`) ; rien : il n'y en a pas.
    variantes: Option<Vec<u8>>,
    /// Projet partagé dont des dessins de variantes manquaient à la lecture : le fichier n'est pas réécrit.
    variantes_gardees: bool,
    /// Le fichier de la corbeille (`corbeille.json`) ; rien : elle est vide, le fichier part.
    corbeille: Option<Vec<u8>>,
    /// Le fichier des plans annulés (`plans_annules.json`) ; rien : aucun, le fichier part.
    annules: Option<Vec<u8>>,
    /// Dessins de la corbeille hors mémoire à recopier d'un autre dossier : (dossier d'où, numéros).
    froids_a_copier: Option<(PathBuf, BTreeSet<CelId>)>,
    /// Un `plans_versions.json` que cette version ne sait pas lire est là : ses dessins sont dans `dessins/`
    /// et rien ne les cite, l'enregistrement n'en efface aucun.
    keep_drawings: bool,
    /// Les instantanés d'envois à écrire dans `versions/`, les dessins de `dessins/` qui ont déjà
    /// leur contenu, et l'ancien dossier d'où recopier `versions/` (« Enregistrer sous », `envois.rs`).
    snapshots: Vec<Arc<crate::envois::Instantane>>,
    snapshot_reuse: HashMap<u64, PathBuf>,
    versions_from: Option<PathBuf>,
    /// Projet partagé : ce que je tiens, écrit à la place du manifeste unique (`projet_partage.rs`). Rien : projet classique.
    partage: Option<partage_io::TravailPartage>,
    /// Ce que le fil de l'interface a laissé au fil d'écriture (`prepare_save`) : tant qu'il est là, les champs
    /// ci-dessus qui en dépendent (manifeste, annexes, dessins, sons, textures, ce qu'il faut garder) sont vides ;
    /// `run_with` les remplit d'abord (`SaveJob::resoudre`).
    differe: Option<Box<Differe>>,
    /// Les dessins déjà écrits dont les fichiers manquaient (`SaveReport::manquants`).
    manquants: Vec<CelId>,
    /// L'ancien manifeste du projet (`Bus.pelure`, à côté du `Bus.shot` qu'on écrit) : effacé une fois le nouveau écrit et
    /// synchronisé (`retirer_ancien_manifeste`). Jamais pour un film partagé.
    ancien_manifeste: Option<PathBuf>,
}

impl SaveJob {
    /// Après l'écriture du manifeste, l'ancien `ancien` (le `.pelure` du même projet) sera effacé, s'il est là
    /// (`retirer_ancien_manifeste` dit quand ce n'est pas le cas). Sans effet pour un film partagé.
    pub(crate) fn retirer_apres(mut self, ancien: PathBuf) -> SaveJob {
        if self.partage.is_none() {
            self.ancien_manifeste = Some(ancien);
        }
        self
    }
}

/// Les dessins de `used` à écrire dans `dir/dessins/` : ceux dont la version n'est pas celle déjà écrite (`written`) ou dont le
/// fichier manque, avec leur atlas hors-feuille et leur strate Fond. `written` est mis à jour comme si l'écriture allait
/// réussir. Rend aussi les dessins qui ont une strate Fond et ceux qui ont un hors-feuille.
#[allow(clippy::type_complexity)]
fn dessins_a_ecrire(
    doc: &Document,
    dir: &Path,
    used: &BTreeSet<CelId>,
    written: &mut HashMap<CelId, u64>,
    reuse: Option<&Reuse>,
) -> Result<(Vec<(PathBuf, DrawingData)>, BTreeSet<CelId>, BTreeSet<CelId>, BTreeSet<CelId>), String> {
    let reuse_dir = reuse.and_then(|r| r.manifest_path.parent());
    let mut drawings = Vec::new();
    let mut reecrits = BTreeSet::new();
    // Les dessins qui ont une strate Fond peinte (format 23) : les autres `<id>_fond.png` partent.
    let fonds: BTreeSet<CelId> = used.iter().copied().filter(|id| doc.cels.get(id).and_then(|c| c.fond.as_ref()).is_some_and(|f| !f.is_empty())).collect();
    let hors = hors_of(doc, used);
    // Les fichiers déjà là, d'une seule lecture du dossier : un `exists` par dessin coûtait 0,15 ms
    // chacun sur le fil de l'interface (450 ms pour un enregistrement de 3 000 dessins où rien
    // n'avait changé).
    let present: std::collections::HashSet<std::ffi::OsString> =
        fs::read_dir(dir.join(DRAWINGS)).map(|entries| entries.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
    let there = |p: &Path| p.file_name().is_some_and(|n| present.contains(n));
    for &id in used {
        let Some(cel) = doc.cels.get(&id) else { continue };
        let vector = cel.vector.is_some();
        let path = drawing_path(dir, id, vector);
        if written.get(&id) == Some(&cel.version)
            && there(&path)
            && (!fonds.contains(&id) || there(&fond_path(dir, id)))
            && (!hors.contains(&id) || (there(&hors_png_path(dir, id)) && there(&hors_json_path(dir, id))))
        {
            continue;
        }
        reecrits.insert(id);
        let same_elsewhere = reuse_dir.zip(reuse).filter(|(_, r)| r.written.get(&id) == Some(&cel.version)).map(|(d, _)| d);
        let elsewhere = same_elsewhere.map(|d| drawing_path(d, id, vector)).filter(|p| p.exists());
        let data = match (&cel.vector, elsewhere) {
            (_, Some(from)) => DrawingData::CopyFrom(from),
            (Some(v), None) => DrawingData::Bytes(serde_json::to_vec(v).map_err(|e| e.to_string())?),
            (None, None) => DrawingData::Raster(cel.raster.clone()),
        };
        drawings.push((path, data));
        // Ce que le dessin a hors de la feuille (toile infinie) : son atlas et son index, à côté.
        if hors.contains(&id) {
            let elsewhere = |path_of: fn(&Path, CelId) -> PathBuf| same_elsewhere.map(|d| path_of(d, id)).filter(|p| p.exists());
            match (elsewhere(hors_png_path), elsewhere(hors_json_path)) {
                (Some(png), Some(json)) => {
                    drawings.push((hors_png_path(dir, id), DrawingData::CopyFrom(png)));
                    drawings.push((hors_json_path(dir, id), DrawingData::CopyFrom(json)));
                }
                _ => {
                    let tiles = cel.raster.beyond_sheet_tiles();
                    let index = HorsIndex { colonnes: (tiles.len() as f64).sqrt().ceil().max(1.0) as usize, tuiles: tiles.iter().map(|((x, y), _)| [*x, *y]).collect() };
                    drawings.push((hors_json_path(dir, id), DrawingData::Bytes(serde_json::to_vec(&index).map_err(|e| e.to_string())?)));
                    drawings.push((hors_png_path(dir, id), DrawingData::Atlas(tiles)));
                }
            }
        }
        // La strate Fond : sa peinture et sa coupe, à côté du dessin.
        if let (true, Some(s)) = (fonds.contains(&id), cel.fond.as_ref()) {
            let parts: [(fn(&Path, CelId) -> PathBuf, &Raster); 2] = [(fond_path, &s.peinture.raster), (coupe_path, &s.coupe)];
            for (path_of, raster) in parts {
                let data = match same_elsewhere.map(|d| path_of(d, id)).filter(|p| p.exists()) {
                    Some(from) => DrawingData::CopyFrom(from),
                    None => DrawingData::Raster(raster.clone()),
                };
                drawings.push((path_of(dir, id), data));
            }
        }
        written.insert(id, cel.version);
    }
    Ok((drawings, fonds, hors, reecrits))
}

/// Le contenu de `variantes.json` (`variantes.rs`) : les dessins qui ont quelque chose hors de la feuille y sont notés.
pub(crate) fn variantes_octets(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    if doc.variantes.is_empty() {
        return Ok(None);
    }
    let cels = doc.cels_des_variantes();
    crate::variantes::to_bytes(doc, &hors_of(doc, &cels))
}

/// Un dessin dans le cliché d'un enregistrement (`prepare_save`).
struct DessinCliche {
    id: CelId,
    vector: bool,
    /// Il a une strate Fond peinte (format 23).
    fond: bool,
    /// Il a quelque chose hors de la feuille (toile infinie, format 30).
    hors: bool,
    /// Ce qu'il faut écrire. Rien : il est déjà écrit dans le dossier à cette version ; le fil d'écriture vérifie que
    /// ses fichiers y sont (`SaveReport::manquants`).
    donnees: Option<Box<DonneesCliche>>,
}

/// Le contenu d'un dessin à écrire, copié sur le fil de l'interface : ses traits, ou ses pixels (tuiles partagées,
/// `Arc` : rien n'est recopié), et sa strate Fond.
struct DonneesCliche {
    vecteur: Option<crate::vector::VectorDrawing>,
    raster: Option<Raster>,
    /// La strate Fond : peinture et coupe.
    fond: Option<(Raster, Raster)>,
    /// Un autre dossier où cette version du dessin est déjà écrite (la copie de secours d'un projet enregistré) : ses
    /// fichiers s'y recopient au lieu d'être encodés, s'ils y sont.
    ailleurs: Option<PathBuf>,
}

/// Ce que le fil de l'interface laisse au fil d'écriture (`prepare_save`, `SaveJob::resoudre`) : le document sans ses
/// dessins, les dessins du cliché, et ce qu'il en a déjà déduit.
struct Differe {
    doc: Document,
    used: BTreeSet<CelId>,
    dessins: Vec<DessinCliche>,
    /// Les dessins des variantes rangées qui ont quelque chose hors de la feuille (`variantes.json`).
    hors_variantes: BTreeSet<CelId>,
    /// Le dossier `versions/` d'un autre dossier du projet (« Enregistrer sous »), à reprendre s'il existe.
    versions_from: Option<PathBuf>,
    /// Les dessins de la corbeille que la séance n'a pas en mémoire (relus à la demande) : leurs fichiers restent, et sont
    /// recopiés d'où ils étaient quand le dossier change (« Enregistrer sous », copie de secours).
    froids: BTreeSet<CelId>,
    froids_depuis: Option<PathBuf>,
}

/// Le cliché des dessins `used` : rien que la mémoire pour ceux qui sont déjà écrits à leur version, et le contenu des
/// autres ; et les textures que leurs traits citent. Aucun accès au disque. `written` ne garde que les dessins de `used`,
/// et il est mis à jour comme si l'écriture allait réussir.
fn cliche_dessins(
    doc: &Document,
    used: &BTreeSet<CelId>,
    written: &mut HashMap<CelId, u64>,
    reuse: Option<&Reuse>,
) -> (Vec<DessinCliche>, BTreeSet<(TextureKind, u32)>, BTreeSet<u64>) {
    let reuse_dir = reuse.and_then(|r| r.manifest_path.parent());
    let mut memo = doc.memo_enregistrement.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = Vec::with_capacity(used.len());
    let mut textures = BTreeSet::new();
    let mut images = BTreeSet::new();
    // (`written` ne retient que des dessins de `used` : s'ils y sont tous, rien à retirer.)
    let (avant, mut retenus) = (written.len(), 0);
    for &id in used {
        let deja = written.get(&id).copied();
        retenus += usize::from(deja.is_some());
        let Some(cel) = doc.cels.get(&id) else { continue };
        let vector = cel.vector.is_some();
        let fond = cel.fond.as_ref().is_some_and(|f| !f.is_empty());
        let m = memo_a_jour(&mut memo, id, cel);
        let hors = !vector && m.hors;
        textures.extend(m.textures.iter().copied());
        images.extend(m.images.iter().copied());
        let donnees = if deja == Some(cel.version) {
            None
        } else {
            written.insert(id, cel.version);
            Some(Box::new(DonneesCliche {
                vecteur: cel.vector.clone(),
                raster: (!vector).then(|| cel.raster.clone()),
                fond: cel.fond.as_ref().filter(|_| fond).map(|s| (s.peinture.raster.clone(), s.coupe.clone())),
                ailleurs: reuse_dir.zip(reuse).filter(|(_, r)| r.written.get(&id) == Some(&cel.version)).map(|(d, _)| d.to_path_buf()),
            }))
        };
        out.push(DessinCliche { id, vector, fond, hors, donnees });
    }
    if retenus < avant {
        written.retain(|id, _| used.contains(id));
    }
    (out, textures, images)
}

/// Prépare l'enregistrement de `doc` dans `manifest_path`. `written` est mis à jour comme si
/// l'écriture allait réussir (à vider si elle échoue).
///
/// C'est la part du fil de l'interface (Ctrl+S, copie de secours) : seulement ce qui demande le document vivant, et
/// rien du disque. Il copie le document sans ses dessins (`Document::copie_sans_dessins`) et le contenu des seuls
/// dessins modifiés depuis le dernier enregistrement ; le manifeste et les fichiers annexes se mettent en JSON, le
/// dossier se lit (dessins déjà là, sons, textures, fichiers illisibles à garder, `references.json`) sur le fil
/// d'écriture, au début de `SaveJob::run_with`. Avant : 8 à 58 ms sur le fil de l'interface pour un film où rien
/// n'avait changé (100 à 600 plans), le manifeste à lui seul 10 à 24 ms.
pub fn prepare_save(
    doc: &Document,
    manifest_path: &Path,
    written: &mut HashMap<CelId, u64>,
    reuse: Option<Reuse>,
) -> Result<SaveJob, String> {
    let dir = manifest_path.parent().ok_or("chemin de projet invalide")?;
    let mut used = doc.used_cels();
    // La corbeille garde ses dessins : ceux qui sont en mémoire s'écrivent, les autres ont déjà leur fichier (`froids`).
    let froids: BTreeSet<CelId> = doc.corbeille.cels().filter(|id| !doc.cels.contains_key(id)).collect();
    used.extend(doc.corbeille.cels().filter(|id| doc.cels.contains_key(id)));
    let froids_depuis = if froids.is_empty() { None } else { doc.envois.dossier.as_ref().filter(|d| d.as_path() != dir).map(|d| d.join(DRAWINGS)) };
    // (Les textures citées par les traits : le mémo de chaque dessin les connaît ; écrites si elles manquent.)
    let (dessins, mut used_textures, used_images) = cliche_dessins(doc, &used, written, reuse.as_ref());
    // (Et celles des pinceaux du dessin hors des cases, dans les annotations de la planche : `horschamptex.rs`.)
    used_textures.extend(doc.annotations.textures_citees());
    let mut job = SaveJob::vide(manifest_path);
    job.used_textures = used_textures;
    job.used_images = used_images;
    // Images des calques vidéo : recopiées si elles ne sont pas déjà dans ce dossier.
    for v in doc.panels.iter().chain(doc.plans_ranges()).flat_map(|p| &p.layers).filter_map(|l| l.video.as_ref()) {
        let to = dir.join(VIDEOS).join(v.id.to_string());
        if let Some(from) = crate::video::dir_of(v.id).filter(|from| *from != to) {
            if !job.videos.iter().any(|(w, _, _)| w.id == v.id) {
                job.videos.push((v.clone(), from, to));
            }
        }
    }
    // Les instantanés d'envois pas encore écrits (`envois.rs`) : un dessin du projet qui a déjà leur contenu
    // dans `dessins/` est recopié (les dessins s'écrivent avant eux), pas encodé de nouveau ; un autre
    // dossier que celui du projet (« Enregistrer sous ») reçoit aussi les instantanés déjà écrits.
    job.snapshots = doc.envois.pending.clone();
    job.snapshot_reuse = crate::envois::dessins_reutilisables(&job.snapshots, written, &|id| Some(drawing_path(dir, id, false)));
    let versions_from = if reuse.is_none() { doc.envois.dossier.as_ref().filter(|d| d.as_path() != dir).map(|d| d.join(crate::envois::DIR)) } else { None };
    let hors_variantes = if doc.variantes.is_empty() { BTreeSet::new() } else { hors_of(doc, &doc.cels_des_variantes()) };
    job.differe = Some(Box::new(Differe { doc: doc.copie_sans_dessins(), used, dessins, hors_variantes, versions_from, froids, froids_depuis }));
    Ok(job)
}

impl SaveJob {
    /// La part de `prepare_save` laissée au fil d'écriture : ce que le dossier contient déjà, puis le manifeste et les
    /// fichiers annexes en JSON, à partir du cliché. Mêmes octets, mêmes décisions que lorsque tout se faisait sur le
    /// fil de l'interface ; un dessin que `written` disait écrit et dont un fichier manque est noté dans `manquants`.
    fn resoudre(&mut self) -> Result<(), String> {
        let Some(differe) = self.differe.take() else { return Ok(()) };
        let Differe { doc, used, dessins, hors_variantes, versions_from, froids, froids_depuis } = *differe;
        let dir = self.manifest_path.parent().ok_or("chemin de projet invalide")?.to_path_buf();
        let dir = dir.as_path();
        // Les fichiers déjà là, d'une seule lecture du dossier (un `exists` par dessin coûtait 0,15 ms chacun).
        let present: std::collections::HashSet<std::ffi::OsString> =
            fs::read_dir(dir.join(DRAWINGS)).map(|entries| entries.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
        let there = |p: &Path| p.file_name().is_some_and(|n| present.contains(n));
        let (mut fonds, mut hors) = (BTreeSet::new(), BTreeSet::new());
        for c in dessins {
            let id = c.id;
            if c.fond {
                fonds.insert(id);
            }
            if c.hors {
                hors.insert(id);
            }
            let Some(DonneesCliche { vecteur, raster, fond, ailleurs }) = c.donnees.map(|d| *d) else {
                let complet = there(&drawing_path(dir, id, c.vector))
                    && (!c.fond || there(&fond_path(dir, id)))
                    && (!c.hors || (there(&hors_png_path(dir, id)) && there(&hors_json_path(dir, id))));
                if !complet {
                    self.manquants.push(id);
                }
                continue;
            };
            let copie = |path_of: &dyn Fn(&Path) -> PathBuf| ailleurs.as_deref().map(path_of).filter(|p| p.exists());
            // Ce que le dessin a hors de la feuille : recopié d'ailleurs si ses deux fichiers y sont, sinon ses tuiles.
            let hors_ailleurs = c.hors.then(|| (copie(&|d| hors_png_path(d, id)), copie(&|d| hors_json_path(d, id))));
            let tuiles = match (&hors_ailleurs, &raster) {
                (Some((Some(_), Some(_))), _) | (None, _) => None,
                (Some(_), r) => Some(r.as_ref().map(Raster::beyond_sheet_tiles).unwrap_or_default()),
            };
            let data = match (copie(&|d| drawing_path(d, id, c.vector)), vecteur, raster) {
                (Some(from), _, _) => DrawingData::CopyFrom(from),
                (None, Some(v), _) => DrawingData::Bytes(serde_json::to_vec(&v).map_err(|e| e.to_string())?),
                (None, None, Some(r)) => DrawingData::Raster(r),
                (None, None, None) => return Err(format!("dessin {id} : contenu absent du cliché")),
            };
            self.drawings.push((drawing_path(dir, id, c.vector), data));
            if let Some(ailleurs_hors) = hors_ailleurs {
                match (ailleurs_hors, tuiles) {
                    ((Some(png), Some(json)), _) => {
                        self.drawings.push((hors_png_path(dir, id), DrawingData::CopyFrom(png)));
                        self.drawings.push((hors_json_path(dir, id), DrawingData::CopyFrom(json)));
                    }
                    (_, tiles) => {
                        let tiles = tiles.unwrap_or_default();
                        let index = HorsIndex { colonnes: (tiles.len() as f64).sqrt().ceil().max(1.0) as usize, tuiles: tiles.iter().map(|((x, y), _)| [*x, *y]).collect() };
                        self.drawings.push((hors_json_path(dir, id), DrawingData::Bytes(serde_json::to_vec(&index).map_err(|e| e.to_string())?)));
                        self.drawings.push((hors_png_path(dir, id), DrawingData::Atlas(tiles)));
                    }
                }
            }
            // La strate Fond : sa peinture et sa coupe, à côté du dessin.
            if let (true, Some((peinture, coupe))) = (c.fond, fond) {
                let parts: [(fn(&Path, CelId) -> PathBuf, Raster); 2] = [(fond_path, peinture), (coupe_path, coupe)];
                for (path_of, raster) in parts {
                    let data = match copie(&|d| path_of(d, id)) {
                        Some(from) => DrawingData::CopyFrom(from),
                        None => DrawingData::Raster(raster),
                    };
                    self.drawings.push((path_of(dir, id), data));
                }
            }
        }
        // Un son ne change jamais : on ne l'écrit que s'il n'est pas déjà dans le dossier.
        self.used_sounds = used_sounds(&doc);
        self.sounds = self.used_sounds.iter().map(|id| &doc.sounds[id]).map(|s| (dir.join(SOUNDS).join(sound_file(s)), s.bytes.clone())).filter(|(path, _)| !path.exists()).collect();
        // Une texture non plus (sa clé vient de son contenu) : écrite seulement si elle manque.
        self.textures = self
            .used_textures
            .iter()
            .map(|&(kind, key)| (texture_file(dir, kind, key), kind, key))
            .filter(|(path, _, _)| !path.exists())
            .filter_map(|(path, kind, key)| crate::brushtex::shared_image(kind, key).map(|img| (path, img)))
            .collect();
        // Une image collée non plus : écrite seulement si elle manque.
        self.images = images_a_ecrire(dir, &self.used_images);
        self.manifest = serde_json::to_vec_pretty(&manifest_de(&doc, true, &used, &hors)).map_err(|e| e.to_string())?;
        let unreadable = unreadable_in(dir);
        self.keep_drawings = crate::planvers::illisible(dir) || crate::variantes::illisible(dir) || crate::corbeille::illisible(dir) || crate::planannule::illisible(dir) || !doc.annules.illisibles.is_empty();
        // Les dessins de la corbeille hors mémoire sont traités comme les illisibles : leurs fichiers (et leurs textures, leurs images
        // collées) ne sont jamais effacés ; recopiés d'ailleurs si le dossier a changé.
        let unreadable: BTreeSet<CelId> = unreadable.union(&froids).copied().collect();
        self.has_unreadable = !unreadable.is_empty() || self.keep_drawings;
        self.kept = kept_files_in(dir);
        self.used = used.union(&unreadable).copied().collect();
        self.fonds = fonds.union(&unreadable).copied().collect();
        self.hors = hors.union(&unreadable).copied().collect();
        self.froids_a_copier = froids_depuis.map(|d| (d, froids.clone())).filter(|(_, f)| !f.is_empty());
        self.corbeille = crate::corbeille::to_bytes(&doc)?;
        self.annules = crate::planannule::to_bytes(&doc)?;
        self.comments = crate::commentaires::to_bytes(&doc)?;
        self.reserves = crate::rognage::to_bytes(&doc)?;
        self.transitions = crate::transitions::to_bytes(&doc)?;
        self.sound_fx = crate::audiofx::to_bytes(&doc)?;
        self.personnages = crate::personnages::to_bytes(&doc)?;
        self.annotations = crate::annotations::to_bytes(&doc)?;
        self.scenario = crate::scenariogarde::to_bytes(&doc)?;
        self.sons_proposes = crate::indications::to_bytes(&doc)?;
        self.references = crate::references::plan_save(&doc, dir)?;
        self.envois = crate::envois::to_bytes(&doc)?;
        self.plans_versions = crate::planvers::to_bytes(&doc)?;
        self.variantes = if doc.variantes.is_empty() { None } else { crate::variantes::to_bytes(&doc, &hors_variantes)? };
        self.versions_from = versions_from.filter(|v| v.is_dir());
        Ok(())
    }

    /// Essais : la part du fil d'écriture faite tout de suite, sur ce fil-ci (la mesurer à part).
    #[cfg(test)]
    pub(crate) fn resoudre_ici(&mut self) -> Result<(), String> {
        self.resoudre()
    }
}

/// Essais : `prepare_save` tel qu'il était avant le cliché léger (tout sur le fil de l'interface : manifeste et
/// annexes en JSON, lecture du dossier, pixels du bord de chaque dessin relus deux fois), pour mesurer l'avant et
/// l'après dans le même processus.
#[cfg(test)]
pub(crate) fn prepare_save_avant(doc: &Document, manifest_path: &Path, written: &mut HashMap<CelId, u64>) -> Result<SaveJob, String> {
    SANS_MEMO.with(|s| s.set(true));
    let job = (|| {
        let dir = manifest_path.parent().ok_or("chemin de projet invalide")?;
        let used = doc.used_cels();
        written.retain(|id, _| used.contains(id));
        let (drawings, fonds, hors, _) = dessins_a_ecrire(doc, dir, &used, written, None)?;
        let mut job = SaveJob::vide(manifest_path);
        job.used_sounds = used_sounds(doc);
        job.sounds = job.used_sounds.iter().map(|id| &doc.sounds[id]).map(|s| (dir.join(SOUNDS).join(sound_file(s)), s.bytes.clone())).filter(|(path, _)| !path.exists()).collect();
        job.used_images = used_images(doc, &used);
        job.images = images_a_ecrire(dir, &job.used_images);
        job.used_textures = used_textures(doc, &used);
        job.textures = job
            .used_textures
            .iter()
            .map(|&(kind, key)| (texture_file(dir, kind, key), kind, key))
            .filter(|(path, _, _)| !path.exists())
            .filter_map(|(path, kind, key)| crate::brushtex::shared_image(kind, key).map(|img| (path, img)))
            .collect();
        for v in doc.panels.iter().chain(doc.plans_ranges()).flat_map(|p| &p.layers).filter_map(|l| l.video.as_ref()) {
            let to = dir.join(VIDEOS).join(v.id.to_string());
            if let Some(from) = crate::video::dir_of(v.id).filter(|from| *from != to) {
                if !job.videos.iter().any(|(w, _, _)| w.id == v.id) {
                    job.videos.push((v.clone(), from, to));
                }
            }
        }
        let drawn = doc.used_cels();
        job.manifest = serde_json::to_vec_pretty(&manifest_de(doc, true, &drawn, &hors_of(doc, &drawn))).map_err(|e| e.to_string())?;
        let unreadable = unreadable_in(dir);
        job.keep_drawings = crate::planvers::illisible(dir) || crate::variantes::illisible(dir) || crate::corbeille::illisible(dir) || crate::planannule::illisible(dir) || !doc.annules.illisibles.is_empty();
        job.has_unreadable = !unreadable.is_empty() || job.keep_drawings;
        job.kept = kept_files_in(dir);
        job.drawings = drawings;
        job.used = used.union(&unreadable).copied().collect();
        job.fonds = fonds.union(&unreadable).copied().collect();
        job.hors = hors.union(&unreadable).copied().collect();
        job.snapshots = doc.envois.pending.clone();
        job.snapshot_reuse = crate::envois::dessins_reutilisables(&job.snapshots, written, &|id| Some(drawing_path(dir, id, false)));
        job.versions_from = doc.envois.dossier.as_ref().filter(|d| d.as_path() != dir).map(|d| d.join(crate::envois::DIR)).filter(|v| v.is_dir());
        job.comments = crate::commentaires::to_bytes(doc)?;
        job.reserves = crate::rognage::to_bytes(doc)?;
        job.transitions = crate::transitions::to_bytes(doc)?;
        job.sound_fx = crate::audiofx::to_bytes(doc)?;
        job.personnages = crate::personnages::to_bytes(doc)?;
        job.annotations = crate::annotations::to_bytes(doc)?;
        job.scenario = crate::scenariogarde::to_bytes(doc)?;
        job.sons_proposes = crate::indications::to_bytes(doc)?;
        job.references = crate::references::plan_save(doc, dir)?;
        job.envois = crate::envois::to_bytes(doc)?;
        job.plans_versions = crate::planvers::to_bytes(doc)?;
        job.variantes = variantes_octets(doc)?;
        Ok(job)
    })();
    SANS_MEMO.with(|s| s.set(false));
    job
}

/// Essais : le manifeste du document vivant, calculé comme avant le cliché léger (comparer les octets).
#[cfg(test)]
pub(crate) fn manifeste_vivant(doc: &Document) -> Vec<u8> {
    let drawn = doc.used_cels();
    serde_json::to_vec_pretty(&manifest_de(doc, true, &drawn, &hors_of(doc, &drawn))).expect("manifeste")
}

/// Essais : le manifeste qu'un enregistrement préparé écrira.
#[cfg(test)]
pub(crate) fn manifeste_du_travail(job: &mut SaveJob) -> Vec<u8> {
    job.resoudre().expect("cliché");
    job.manifest.clone()
}

/// Les sons proposés, lus dans `sons_proposes.json` (absent : aucun). Un fichier illisible ou d'une
/// version plus récente n'empêche pas d'ouvrir le projet : on le dit, sans propositions, et il reste
/// en place (`write_sidecar`).
fn load_sons_proposes(dir: &Path, warnings: &mut Vec<String>) -> crate::indications::SonsProposes {
    let path = dir.join(crate::indications::FILE);
    match fs::read_to_string(&path) {
        Ok(text) => match crate::indications::parse(&text) {
            Ok(mut s) => {
                crate::indications::verifier_fichiers(&mut s);
                s
            }
            Err(e) => {
                warnings.push(format!("{} : {e} ; les sons proposés ne sont pas chargés.", crate::indications::FILE));
                Default::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => {
            warnings.push(format!("{} : {e} ; les sons proposés ne sont pas chargés.", path.display()));
            Default::default()
        }
    }
}

/// Les personnages du projet, lus dans `personnages.json` (absent : aucun). Un fichier illisible ou d'une
/// version plus récente n'empêche pas d'ouvrir le projet : on le dit, sans personnages, et il reste en
/// place (`write_sidecar`).
fn load_personnages(dir: &Path, warnings: &mut Vec<String>) -> Vec<crate::personnages::Personnage> {
    let path = dir.join(crate::personnages::FILE);
    match fs::read_to_string(&path) {
        Ok(text) => crate::personnages::parse(&text).unwrap_or_else(|e| {
            warnings.push(format!("{} : {e} ; les personnages ne sont pas chargés.", crate::personnages::FILE));
            Vec::new()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            warnings.push(format!("{} : {e} ; les personnages ne sont pas chargés.", path.display()));
            Vec::new()
        }
    }
}

/// Les annotations de la vue Vignettes, lues dans `annotations.json` (absent : aucune). Un fichier illisible ou
/// d'une version plus récente n'empêche pas d'ouvrir le projet : on le dit, sans annotations, et il reste en
/// place (`write_sidecar`).
fn load_annotations(dir: &Path, warnings: &mut Vec<String>) -> crate::annotations::Annotations {
    let path = dir.join(crate::annotations::FILE);
    match fs::read_to_string(&path) {
        Ok(text) => crate::annotations::parse(&text).unwrap_or_else(|e| {
            warnings.push(format!("{} : {e} ; les annotations ne sont pas chargées (le fichier reste en place).", crate::annotations::FILE));
            Default::default()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => {
            warnings.push(format!("{} : {e} ; les annotations ne sont pas chargées.", path.display()));
            Default::default()
        }
    }
}

/// Les effets des sons, lus dans `effets_son.json` (absent : aucun). Un fichier illisible ou d'une
/// version plus récente n'empêche pas d'ouvrir le projet : on le dit, sans effets, et il reste en
/// place (`write_sidecar`).
fn load_sound_fx(dir: &Path, warnings: &mut Vec<String>) -> Option<crate::audiofx::Loaded> {
    let path = dir.join(crate::audiofx::FILE);
    match fs::read_to_string(&path) {
        Ok(text) => match crate::audiofx::parse(&text) {
            Ok(l) => Some(l),
            Err(e) => {
                warnings.push(format!("{} : {e} ; les effets des sons ne sont pas chargés.", crate::audiofx::FILE));
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            warnings.push(format!("{} : {e} ; les effets des sons ne sont pas chargés.", path.display()));
            None
        }
    }
}

/// Les transitions du montage, lues dans `transitions.json` (absent : aucune). Un fichier illisible
/// ou d'une version plus récente n'empêche pas d'ouvrir le projet : on le dit, sans transitions, et
/// il reste en place (`write_sidecar`).
fn load_transitions(dir: &Path, warnings: &mut Vec<String>) -> Vec<crate::transitions::Transition> {
    let path = dir.join(crate::transitions::FILE);
    match fs::read_to_string(&path) {
        Ok(text) => crate::transitions::parse(&text).unwrap_or_else(|e| {
            warnings.push(format!("{} : {e} ; les transitions ne sont pas chargées.", crate::transitions::FILE));
            Vec::new()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            warnings.push(format!("{} : {e} ; les transitions ne sont pas chargées.", path.display()));
            Vec::new()
        }
    }
}

/// Le contenu rogné des plans, lu dans `rognages.json` (absent : rien n'est rogné). Un fichier
/// illisible n'empêche pas d'ouvrir le projet : on le dit, sans contenu rogné.
fn load_reserves(dir: &Path, warnings: &mut Vec<String>) -> Option<crate::rognage::Loaded> {
    let path = dir.join(crate::rognage::FILE);
    match fs::read_to_string(&path) {
        Ok(text) => match crate::rognage::parse(&text) {
            Ok(r) => Some(r),
            Err(e) => {
                warnings.push(format!("{} : {e} ; le contenu rogné des plans n'est pas chargé.", crate::rognage::FILE));
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            warnings.push(format!("{} : {e} ; le contenu rogné des plans n'est pas chargé.", path.display()));
            None
        }
    }
}

/// Les commentaires du projet, lus dans `commentaires.json` (absent : aucun commentaire). Un
/// fichier illisible n'empêche pas d'ouvrir le projet : on le dit, sans commentaires.
fn load_comments(dir: &Path, warnings: &mut Vec<String>) -> Vec<crate::commentaires::Comment> {
    let path = dir.join(crate::commentaires::FILE);
    match fs::read_to_string(&path) {
        Ok(text) => crate::commentaires::parse(&text).unwrap_or_else(|e| {
            warnings.push(format!("{} : {e} ; les commentaires ne sont pas chargés.", crate::commentaires::FILE));
            Vec::new()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            warnings.push(format!("{} : {e} ; les commentaires ne sont pas chargés.", path.display()));
            Vec::new()
        }
    }
}

/// Écrit le fichier annexe `path` (`bytes`), ou le retire s'il n'y a plus rien à y mettre — sans
/// jamais détruire un fichier que cette version ne sait pas lire (`lisible` en juge). Un
/// `commentaires.json` écrit par une version plus récente, ou abîmé, s'ouvre sans ses commentaires
/// (avec un avertissement) : « aucun commentaire » ne veut alors pas dire « fichier à effacer ». Il
/// reste en place s'il n'y a rien à écrire ; sinon il est mis de côté sous le nom `<fichier>.illisible`
/// avant que le neuf prenne sa place.
///
/// Un fichier qui a déjà exactement ces octets n'est pas réécrit (ni synchronisé sur le disque) : c'est un succès. Un
/// dossier partagé par Dropbox ou Drive renvoyait sinon chaque annexe à chaque enregistrement.
pub(crate) fn write_sidecar(path: &Path, bytes: Option<&[u8]>, lisible: impl Fn(&str) -> bool) -> Result<(), String> {
    let lu = fs::read(path);
    if let (Some(b), Ok(actuel)) = (bytes, &lu) {
        if actuel.as_slice() == b && !toujours_ecrire() {
            return Ok(());
        }
    }
    let existant = match lu {
        Ok(b) => Some(String::from_utf8(b).ok().filter(|t| lisible(t.trim_start_matches('\u{feff}'))).is_some()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => Some(false),
    };
    match (bytes, existant) {
        (Some(b), Some(false)) => {
            let mut aside = None;
            for k in 1..100 {
                let mut name = path.as_os_str().to_owned();
                name.push(if k == 1 { ".illisible".to_string() } else { format!(".illisible{k}") });
                let candidate = PathBuf::from(name);
                if !candidate.exists() {
                    aside = Some(candidate);
                    break;
                }
            }
            if let Some(to) = aside {
                fs::rename(path, &to).map_err(|e| format!("{} : {e}", path.display()))?;
            }
            write_atomic_synced(path, b)
        }
        (Some(b), _) => write_atomic_synced(path, b),
        (None, Some(false)) => Ok(()),
        (None, _) => {
            let _ = fs::remove_file(path);
            Ok(())
        }
    }
}

impl SaveJob {
    /// Écrit les dessins, puis les sons, puis le manifeste (en dernier : un manifeste ne cite
    /// jamais un dessin pas encore écrit), puis efface ce qui ne sert plus.
    pub fn run(self) -> Result<SaveReport, String> {
        self.run_with(None, LOAD_THREADS)
    }

    /// Comme `run`, sur au plus `threads` fils, en comptant dans `progress` les dessins écrits (un
    /// autre fil l'affiche pendant que celui-ci écrit : `enregistrement.rs`).
    pub fn run_with(mut self, progress: Option<&Progress>, threads: usize) -> Result<SaveReport, String> {
        // Ce que le fil de l'interface a laissé (le dossier lu, le manifeste et les annexes en JSON) : avant toute écriture.
        self.resoudre()?;
        if self.partage.is_some() {
            return self.run_partage(progress, threads);
        }
        let dir = self.manifest_path.parent().ok_or("chemin de projet invalide")?;
        fs::create_dir_all(dir.join(DRAWINGS)).map_err(|e| format!("{} : {e}", dir.display()))?;
        let mut count = 0;
        if let Some(p) = progress {
            p.total.store(self.drawings.len(), std::sync::atomic::Ordering::Relaxed);
        }
        // Les dessins s'encodent et s'écrivent en parallèle (11 ms de PNG chacun) ; la première
        // erreur, dans l'ordre des dessins, est celle qui est rendue.
        let written = crate::parallele::par_map(&self.drawings, threads, |_, (path, data)| -> Result<(), String> {
            let bytes: std::borrow::Cow<[u8]> = match data {
                DrawingData::Bytes(b) => std::borrow::Cow::Borrowed(b),
                DrawingData::Raster(r) => std::borrow::Cow::Owned(encode_png(&r.to_rgba(), r.width, r.height)?),
                DrawingData::Atlas(tiles) => {
                    let (atlas, _) = hors_atlas(tiles);
                    std::borrow::Cow::Owned(encode_png(&atlas.to_rgba(), atlas.width, atlas.height)?)
                }
                DrawingData::CopyFrom(from) => std::borrow::Cow::Owned(fs::read(from).map_err(|e| format!("{} : {e}", from.display()))?),
            };
            let result = write_atomic(path, &bytes);
            if let Some(p) = progress {
                p.done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            result
        });
        for result in written {
            result?;
            count += 1;
        }
        if !self.sounds.is_empty() {
            fs::create_dir_all(dir.join(SOUNDS)).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        for (path, bytes) in &self.sounds {
            write_atomic(path, bytes)?;
            count += 1;
        }
        if !self.textures.is_empty() {
            fs::create_dir_all(dir.join(TEXTURES)).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        for (path, img) in &self.textures {
            write_atomic(path, &texture_png(img)?)?;
            count += 1;
        }
        if !self.images.is_empty() {
            fs::create_dir_all(dir.join(crate::imagevec::DOSSIER)).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        for (path, px) in &self.images {
            write_atomic(path, &crate::imagevec::png(px)?)?;
            count += 1;
        }
        for (clip, from, to) in &self.videos {
            count += crate::video::copy_frames(clip, from, to)?;
        }
        // Les commentaires : leur fichier à côté du manifeste (sans commentaire, il part).
        // Ni l'un ni l'autre ne détruit un fichier que cette version ne sait pas lire (`write_sidecar`),
        // et les deux s'écrivent atomiquement et sur le disque (`write_atomic_synced`).
        write_sidecar(&dir.join(crate::commentaires::FILE), self.comments.as_deref(), |t| crate::commentaires::parse(t).is_ok())?;
        // Le contenu rogné des plans : son fichier à côté du manifeste (rien de rogné, il part).
        write_sidecar(&dir.join(crate::rognage::FILE), self.reserves.as_deref(), |t| crate::rognage::parse(t).is_ok())?;
        // Les transitions du montage, de même.
        write_sidecar(&dir.join(crate::transitions::FILE), self.transitions.as_deref(), |t| crate::transitions::parse(t).is_ok())?;
        // Et les effets des sons.
        write_sidecar(&dir.join(crate::audiofx::FILE), self.sound_fx.as_deref(), |t| crate::audiofx::parse(t).is_ok())?;
        // Les personnages de la brosse par personnage, de même.
        write_sidecar(&dir.join(crate::personnages::FILE), self.personnages.as_deref(), |t| crate::personnages::parse(t).is_ok())?;
        // Les annotations de la vue Vignettes, de même.
        write_sidecar(&dir.join(crate::annotations::FILE), self.annotations.as_deref(), |t| crate::annotations::parse(t).is_ok())?;
        // Le scénario gardé : l'ancien (d'un autre scénario) passe à côté, jamais effacé.
        crate::scenariogarde::mettre_de_cote(&dir.join(crate::scenariogarde::FILE), self.scenario.as_deref());
        write_sidecar(&dir.join(crate::scenariogarde::FILE), self.scenario.as_deref(), |t| crate::scenariogarde::parse(t).is_ok())?;
        // Et les sons proposés par la lecture du storyboard.
        write_sidecar(&dir.join(crate::indications::FILE), self.sons_proposes.as_deref(), |t| crate::indications::parse(t).is_ok())?;
        // Les références visuelles : leurs fichiers dans `references/`, puis `references.json`.
        count += self.references.write(dir)?;
        // Les envois : les instantanés d'abord (dessins puis fichier), puis la liste qui les cite.
        count += crate::envois::ecrire(dir, &self.snapshots, &self.snapshot_reuse, self.versions_from.as_deref())?;
        write_sidecar(&dir.join(crate::envois::FILE), self.envois.as_deref(), |t| crate::envois::parse(t).is_ok())?;
        // Les autres versions des plans (leurs dessins sont ceux de `dessins/`, déjà écrits).
        write_sidecar(&dir.join(crate::planvers::FILE), self.plans_versions.as_deref(), |t| crate::planvers::parse(t).is_ok())?;
        // Les variantes rangées des séquences, de même (`variantes.rs`).
        write_sidecar(&dir.join(crate::variantes::FILE), self.variantes.as_deref(), |t| crate::variantes::parse(t).is_ok())?;
        // La corbeille : ses dessins hors mémoire d'abord (recopiés d'un autre dossier s'il a changé), puis son fichier.
        if let Some((depuis, ids)) = &self.froids_a_copier {
            count += copier_dessins_de(depuis, &dir.join(DRAWINGS), ids);
            // Leurs textures et leurs images collées (que les dessins hors mémoire citent sans que la séance les connaisse) suivent : ce
            // qui manque dans le nouveau dossier est recopié, rien d'écrasé.
            if let Some(source) = depuis.parent() {
                count += copier_manquants(&source.join(TEXTURES), &dir.join(TEXTURES));
                count += copier_manquants(&source.join(crate::imagevec::DOSSIER), &dir.join(crate::imagevec::DOSSIER));
            }
        }
        write_sidecar(&dir.join(crate::corbeille::FILE), self.corbeille.as_deref(), |t| crate::corbeille::parse(t).is_ok())?;
        write_sidecar(&dir.join(crate::planannule::FILE), self.annules.as_deref(), |t| crate::planannule::parse(t).is_ok())?;
        // Le manifeste en dernier ; s'il a déjà ces octets-là (une copie de secours où rien n'a changé), il reste tel quel.
        if toujours_ecrire() || fs::read(&self.manifest_path).ok().as_deref() != Some(self.manifest.as_slice()) {
            write_atomic_synced(&self.manifest_path, &self.manifest)?;
        }
        // Le nouveau manifeste est sur le disque : l'ancien nom du projet (`.pelure`) peut partir. Un plantage avant laisse
        // les deux fichiers, et l'ouverture prend le plus récent (`manifeste_a_ouvrir`).
        if let Some(ancien) = self.ancien_manifeste.take() {
            #[cfg(test)]
            if PLANTER_AVANT_RETRAIT.with(|p| p.get()) {
                return Err("plantage simulé entre l'écriture du .shot et l'effacement du .pelure".into());
            }
            retirer_ancien_manifeste(&self.manifest_path, &ancien);
        }
        let removed = if self.keep_drawings { 0 } else { remove_unused(dir, &self.used, &self.fonds, &self.hors) }
            + remove_unused_sounds(dir, &self.used_sounds, &self.kept)
            + if self.has_unreadable { 0 } else { remove_unused_textures(dir, &self.used_textures) + crate::imagevec::effacer_inutiles(dir, &self.used_images) }
            + self.references.cleanup(dir);
        Ok(SaveReport { written: count, removed, snapshots: self.snapshots.iter().map(|s| s.id).collect(), partage: None, manquants: std::mem::take(&mut self.manquants) })
    }
}

/// Efface les sons du dossier qui ne servent plus (seulement les fichiers « <nombre>.<ext> »).
fn remove_unused_sounds(dir: &Path, used: &BTreeSet<u64>, kept: &BTreeSet<PathBuf>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(SOUNDS)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if kept.contains(&path) {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let id = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u64>().ok());
        if let Some(id) = id {
            if SOUND_EXTS.contains(&ext.as_str()) && !used.contains(&id) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

/// Le numéro de dessin d'un fichier de `dessins/` (« <nombre>.png », « <nombre>_fond.png », « <nombre>_hors.json »…).
fn numero_de_fichier(path: &Path) -> Option<CelId> {
    let stem = path.file_stem().and_then(|s| s.to_str())?;
    let stem = stem.strip_suffix("_hors").or_else(|| stem.strip_suffix("_fond")).or_else(|| stem.strip_suffix("_coupe")).unwrap_or(stem);
    stem.parse::<CelId>().ok()
}

/// Recopie de `depuis` (un dossier `dessins/`) vers `vers` les fichiers des dessins `ids` qui n'y sont pas encore (la corbeille d'un
/// projet enregistré ailleurs, ou une copie de secours : ses dessins ne sont pas en mémoire). Rend le nombre de fichiers copiés.
fn copier_dessins_de(depuis: &Path, vers: &Path, ids: &BTreeSet<CelId>) -> usize {
    let Ok(entries) = fs::read_dir(depuis) else { return 0 };
    let _ = fs::create_dir_all(vers);
    let mut n = 0;
    for e in entries.flatten() {
        let path = e.path();
        let Some(id) = numero_de_fichier(&path).filter(|id| ids.contains(id)) else { continue };
        let _ = id;
        let to = vers.join(e.file_name());
        if !to.exists() && fs::copy(&path, &to).is_ok() {
            n += 1;
        }
    }
    n
}

/// Recopie de `depuis` vers `vers` les fichiers qui n'y sont pas encore (un dossier à plat : `textures/`, `images/`). Rend le nombre copié.
fn copier_manquants(depuis: &Path, vers: &Path) -> usize {
    let Ok(entries) = fs::read_dir(depuis) else { return 0 };
    let mut n = 0;
    for e in entries.flatten() {
        let from = e.path();
        if !from.is_file() {
            continue;
        }
        let to = vers.join(e.file_name());
        if !to.exists() {
            let _ = fs::create_dir_all(vers);
            if fs::copy(&from, &to).is_ok() {
                n += 1;
            }
        }
    }
    n
}

/// La place que prennent sur le disque les fichiers des dessins `ids` dans `dir/dessins/` (la fenêtre de la corbeille la dit).
pub(crate) fn taille_des_dessins(dir: &Path, ids: &BTreeSet<CelId>) -> u64 {
    let Ok(entries) = fs::read_dir(dir.join(DRAWINGS)) else { return 0 };
    entries.flatten().filter(|e| numero_de_fichier(&e.path()).is_some_and(|id| ids.contains(&id))).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()
}

/// Relit un seul dessin de `dir/dessins/` (la corbeille ne charge rien à l'ouverture : elle relit à la demande). `rapide` : rendu
/// approché des traits (une vignette).
pub(crate) fn charger_un_dessin_avec(dir: &Path, id: CelId, w: usize, h: usize, rapide: bool) -> Result<Cel, String> {
    let hors = hors_json_path(dir, id).exists();
    load_drawing(dir, id, w, h, rapide, hors).map(|(c, _, _)| c)
}

/// Relit un seul dessin pour le remettre dans le film (rendu exact).
pub(crate) fn charger_un_dessin(dir: &Path, id: CelId, w: usize, h: usize) -> Result<Cel, String> {
    charger_un_dessin_avec(dir, id, w, h, false)
}

/// Efface les dessins du dossier qui ne servent plus (seulement les fichiers « <nombre>.png/json »),
/// et les strates Fond « <nombre>_fond.png » et « <nombre>_coupe.png » des dessins qui n'en ont plus.
fn remove_unused(dir: &Path, used: &BTreeSet<CelId>, fonds: &BTreeSet<CelId>, hors: &BTreeSet<CelId>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(DRAWINGS)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        // Ce qu'un dessin a hors de la feuille : « <nombre>_hors.png » et « <nombre>_hors.json ».
        if let (Some(id), "png" | "json") = (stem.strip_suffix("_hors").and_then(|s| s.parse::<CelId>().ok()), ext) {
            if !hors.contains(&id) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
            continue;
        }
        if let (Some(id), "png") = (stem.strip_suffix("_fond").or_else(|| stem.strip_suffix("_coupe")).and_then(|s| s.parse::<CelId>().ok()), ext) {
            if !fonds.contains(&id) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
            continue;
        }
        let id = stem.parse::<CelId>().ok();
        if let (Some(id), "png" | "json") = (id, ext) {
            if !used.contains(&id) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

/// L'entrée du manifeste pour un plan : tout ce que le fichier garde de lui. `key` donne le numéro sous
/// lequel chaque dessin y est cité (le même, pour un enregistrement ; l'empreinte du dessin, pour un
/// instantané d'envoi, qui se lit alors sans rien devoir aux numéros de la séance).
///
/// `digest` : ne garde que ce qui compte quand on se demande si le plan **a changé** (`envois.rs`) — ce
/// qui se voit et ce qui dure (dessins, calques réglés, effets, caméra, textes du storyboard, marques
/// d'image, durée) ; pas ce qui aide à travailler (couleur de repère du plan, signets, entrée et
/// sortie, notes dessinées sur la timeline, repère 3D, noms et marques de couleur des dessins, rôles
/// des images, verrous, calque replié, groupe de couleur, pelure d'oignon et pochoir, nom du calque).
fn panel_entry(panel: &Panel, key: &dyn Fn(CelId) -> CelId, garde: &dyn Fn(CelId) -> bool, digest: bool) -> PanelEntry {
    let mut entry = PanelEntry {
        id: panel.id,
        duree: panel.length,
        action: panel.action.clone(),
        dialogue: panel.dialogue.clone(),
        notes: panel.notes.clone(),
        calques: panel
            .layers
            .iter()
            .map(|l| LayerEntry {
                nom: l.name.clone(),
                kind: match l.kind {
                    LayerKind::Vector => "vectoriel".into(),
                    LayerKind::Bitmap => "bitmap".into(),
                },
                visible: l.visible,
                opacite: l.opacity,
                verrouille: l.locked,
                transformation: (!l.transform.is_identity()).then(|| TransformEntry {
                    x: l.transform.x,
                    y: l.transform.y,
                    echelle: l.transform.scale,
                    rotation: l.transform.angle,
                    etirement_x: l.transform.stretch_x,
                    etirement_y: l.transform.stretch_y,
                    inclinaison_x: l.transform.tilt_x,
                    inclinaison_y: l.transform.tilt_y,
                }),
                cles_position: l.pos_keys.iter().map(crate::poskeys::Entree::from).collect(),
                trajectoire: l.trajectoire.clone(),
                cles_opacite: l.opacity_keys.iter().map(crate::opacitecles::Entree::from).collect(),
                couleur_unie: l.unie,
                // (Un calque fixe n'a pas d'écart de profondeur : s'il en reste un, il est écrit dans la profondeur.)
                profondeur: if l.pos_keys.is_empty() { l.depth + l.transform.dz } else { l.depth },
                toute_la_scene: l.scene_wide.map(|w| w.key),
                partage: l.partage,
                fusion: l.blend,
                pelure: l.onion,
                transparence: l.keep_alpha,
                pochoir: l.stencil,
                pochoir_negatif: l.stencil_negative,
                position_verrouillee: l.position_locked,
                groupe: l.group,
                replie: l.collapsed,
                dossier: l.folder,
                dans_dossier: l.parent.and_then(|p| panel.layers.iter().position(|f| f.folder && f.id == p)),
                avant: l.pre,
                apres: l.post,
                cassure_auto: l.auto_break,
                creation_auto: l.auto_create,
                // Seulement les noms des dessins encore dans la timeline du calque.
                noms: l.names.iter().filter(|(id, _)| l.frames.contains(&Some(**id))).map(|(id, n)| (key(*id), n.clone())).collect(),
                marques_couleur: l.color_marks.iter().filter(|(id, _)| l.frames.contains(&Some(**id))).map(|(id, k)| (key(*id), *k)).collect(),
                roles: l.roles.clone(),
                effets: l.fx.clone(),
                effets_eteints: !l.fx_enabled,
                // (Le calque CTG n'existe plus : il n'est pas écrit.)
                ctg: None,
                video: l.video.as_ref().map(|v| VideoEntry { id: v.id, nom: v.name.clone(), images: v.frames, ips: v.fps, debut: v.start, ext: v.ext.clone() }),
                fond: l.fond,
                calque_effet: l.effet.as_ref().map(crate::calqueffet::Entree::from),
                calque_couleur: l.couleur.as_ref().and_then(|c| {
                    // (Libre : pas de trait. Un trait qui manque, sans griffonnages à garder : plus de lien.)
                    let ligne = if c.trait_id == 0 { None } else { panel.layers.iter().position(|s| s.id == c.trait_id) };
                    if c.trait_id != 0 && ligne.is_none() && !c.remplissage {
                        return None;
                    }
                    Some(CouleurEntry {
                        ligne,
                        dessins: c.dessins.iter().filter(|(t, k)| garde(**t) && garde(**k)).map(|(t, k)| [key(*t), key(*k)]).collect(),
                        trous: c.trous,
                        zone: c.zone,
                        griffonner: c.griffonner,
                        remplissage: c.remplissage,
                        griffonnages: c.griffonnages.iter().filter(|(p, g)| garde(**p) && garde(**g)).map(|(p, g)| [key(*p), key(*g)]).collect(),
                        sources: c.sources.iter().filter_map(|id| panel.layers.iter().position(|s| s.id == *id)).collect(),
                    })
                }),
                decor: l.decor,
                personnage: l.personnage,
                annotation: l.annotation,
                expositions: to_exposures(&l.frames.iter().map(|f| f.map(key)).collect::<Vec<_>>()),
            })
            .collect(),
        camera: panel
            .camera
            .iter()
            .map(|k| CameraEntry {
                image: k.frame,
                x: k.cam.x,
                y: k.cam.y,
                zoom: k.cam.zoom,
                rotation: k.cam.angle,
                doux: k.smooth,
                avance: k.cam.dolly,
                adoucir_entree: k.ease_in,
                adoucir_sortie: k.ease_out,
                bascule: k.cam.tilt,
                panoramique: k.cam.pan,
            })
            .collect(),
        saccade: (panel.saccade != crate::camera::Saccade::default()).then(|| SaccadeEntry { actif: panel.saccade.actif, cadence: panel.saccade.cadence }),
        marques: panel
            .marks
            .iter()
            .map(|m| MarkEntry {
                id: m.id,
                image: m.frame,
                couleur: m.color,
                action: m.action.clone(),
                dialogue: m.dialogue.clone(),
                notes: m.notes.clone(),
            })
            .collect(),
        signets: panel.cues.bookmarks.clone(),
        entree: panel.cues.in_point,
        sortie: panel.cues.out_point,
        couleur: panel.color,
        notes_dessinees: panel
            .cues
            .notes
            .iter()
            .map(|n| TimeNoteEntry { couleur: n.color, epaisseur: n.width, points: n.points.clone() })
            .collect(),
    };
    if digest {
        entry.signets.clear();
        (entry.entree, entry.sortie, entry.couleur) = (None, None, None);
        entry.notes_dessinees.clear();
        // Les numéros du plan et de ses marques disent qui il est, pas ce qu'il contient : deux versions d'un
        // même plan (`planvers.rs`) se comparent sans eux.
        entry.id = 0;
        for m in &mut entry.marques {
            m.id = 0;
        }
        // Les numéros de calques de toute la scène sont neufs à chaque séance : seule leur présence compte.
        for l in &mut entry.calques {
            (l.nom, l.verrouille, l.pelure, l.transparence, l.pochoir, l.pochoir_negatif) = (String::new(), false, false, false, false, false);
            (l.position_verrouillee, l.groupe, l.replie, l.cassure_auto, l.creation_auto) = (false, 0, false, false, true);
            l.noms.clear();
            l.marques_couleur.clear();
            l.roles.clear();
            l.toute_la_scene = l.toute_la_scene.map(|_| 1);
            l.partage = l.partage.map(|_| 1);
        }
    }
    entry
}

/// Les octets qui disent ce qu'est un plan, pour l'empreinte « le plan a-t-il changé ? » (`envois.rs`) :
/// son entrée du manifeste réduite à ce qui compte (`panel_entry`), les dessins cités par leur
/// empreinte (`key`).
pub(crate) fn digest_bytes(panel: &Panel, key: &dyn Fn(CelId) -> CelId) -> Vec<u8> {
    serde_json::to_vec(&panel_entry(panel, key, &|c| dans_le_plan(panel, c), true)).unwrap_or_default()
}

/// L'entrée complète d'un plan pour un instantané (envoi, version) : le même JSON que dans le
/// manifeste, les dessins cités par `key`.
pub(crate) fn snapshot_value(panel: &Panel, key: &dyn Fn(CelId) -> CelId) -> Result<serde_json::Value, String> {
    serde_json::to_value(panel_entry(panel, key, &|c| dans_le_plan(panel, c), false)).map_err(|e| e.to_string())
}

/// Un instantané ne garde des couples trait / couleur (`calque_couleur`) que ceux dont les deux dessins
/// sont dans une timeline du plan : ce sont les seuls dessins qu'il emporte.
fn dans_le_plan(panel: &Panel, cel: CelId) -> bool {
    // (Les griffonnages d'un calque de couleur sont des dessins du plan, même s'ils ne sont dans aucune timeline.)
    panel.layers.iter().any(|l| l.frames.contains(&Some(cel)) || l.couleur.as_ref().is_some_and(|c| c.dessins_griffonnages().any(|g| g == cel)))
}

/// En-tête d'un instantané : la feuille, le cadre, la cadence et le papier du projet au moment où il a été
/// pris (les dessins qu'il cite ont cette taille, même si le projet a changé de résolution depuis).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Tete {
    pub largeur: usize,
    pub hauteur: usize,
    pub cadre: [usize; 2],
    pub cadence: u32,
    pub papier: [u8; 3],
}

impl Tete {
    pub fn de(doc: &Document) -> Tete {
        Tete { largeur: doc.width, hauteur: doc.height, cadre: doc.frame, cadence: doc.fps, papier: doc.paper }
    }
}

/// Relit des plans d'un instantané : `entrees` (les valeurs de `snapshot_value`) deviennent un petit projet
/// dont les dessins sont ceux de `dir/dessins/<empreinte>.png|json`. C'est l'ouverture ordinaire d'un
/// projet (`load_manifest`), donc avec ses protections : un dessin manquant ou abîmé est un
/// avertissement, le plan s'ouvre quand même. `scene` : le numéro de scène que prend le plan.
pub(crate) fn charger_plans(dir: &Path, tete: &Tete, scene: u64, entrees: Vec<serde_json::Value>, quick: bool) -> Result<Loaded, String> {
    let panneaux: Vec<PanelEntry> = entrees
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| format!("plan illisible : {e}")))
        .collect::<Result<_, _>>()?;
    // Une vidéo de l'instantané ne doit pas faire perdre au projet en cours le dossier de la sienne
    // (`video::register_dir` est commun à tout le programme).
    let videos: Vec<(u64, Option<PathBuf>)> =
        panneaux.iter().flat_map(|p| &p.calques).filter_map(|l| l.video.as_ref()).map(|v| (v.id, crate::video::dir_of(v.id))).collect();
    let manifest = Manifest {
        format: FORMAT.into(),
        version: VERSION,
        largeur: tete.largeur,
        hauteur: tete.hauteur,
        cadre: Some(tete.cadre),
        cadence: tete.cadence.max(1),
        papier: tete.papier,
        sequences: vec![SequenceEntry { id: 1, nom: String::new(), numero: String::new(), scenes: vec![SceneEntry { id: scene.max(1), nom: String::new(), numero: String::new(), panneaux }] }],
        sons: Vec::new(),
        pistes_son: Vec::new(),
        guides: Vec::new(),
        planche: Vec::new(),
        planche_textes: Vec::new(),
        groupes: Vec::new(),
        image_de_depart: 1,
        fond: BackgroundEntry::default(),
        volume_general: 1.0,
        storyboard_verrouille: false,
        numerotation_verrouillee: false,
        proprietes: PropertiesEntry::default(),
        // Les instantanés ne gardent que la feuille de leurs dessins (pas leur hors-feuille, `monde.rs`).
        dessins_hors: Vec::new(),
        partage: None,
    };
    let loaded = load_manifest(manifest, dir, quick, None);
    for (id, previous) in videos {
        if let Some(p) = previous {
            crate::video::register_dir(id, p);
        }
    }
    loaded
}

/// Le manifeste de `doc`. Sans `avec_plans` (la structure d'un projet partagé), les scènes n'ont pas de panneaux : chaque
/// plan est dans son propre fichier (`projet_partage.rs`). Avec : `drawn`, les dessins qui seront enregistrés
/// (`Document::used_cels`, pris sur le document vivant : le cliché de `prepare_save` n'a pas les dessins), et `hors`,
/// ceux qui ont quelque chose hors de la feuille.
fn manifest_de(doc: &Document, avec_plans: bool, drawn: &BTreeSet<CelId>, hors: &BTreeSet<CelId>) -> Manifest {
    let mut sequences: Vec<SequenceEntry> = Vec::new();
    // Les dessins qui seront enregistrés : un calque de couleur n'écrit que les couples dont les deux
    // dessins le sont.
    let vide = BTreeSet::new();
    let drawn = if avec_plans { drawn } else { &vide };
    for (i, panel) in doc.panels.iter().enumerate() {
        let seq = doc.sequence_of(i);
        if sequences.last().is_none_or(|s| s.id != seq) {
            let nom = doc.sequences.iter().find(|s| s.id == seq).map(|s| s.name.clone()).unwrap_or_default();
            let numero = doc.sequences.iter().find(|s| s.id == seq).map(|s| s.number.clone()).unwrap_or_default();
            sequences.push(SequenceEntry {
                id: seq,
                nom,
                numero,
                scenes: Vec::new(),
            });
        }
        let scenes = &mut sequences.last_mut().expect("séquence ajoutée juste avant").scenes;
        if scenes.last().is_none_or(|s| s.id != panel.scene) {
            let nom = doc.scene(panel.scene).map(|s| s.name.clone()).unwrap_or_default();
            let numero = doc.scene(panel.scene).map(|s| s.number.clone()).unwrap_or_default();
            scenes.push(SceneEntry {
                id: panel.scene,
                nom,
                numero,
                panneaux: Vec::new(),
            });
        }
        if avec_plans {
            scenes.last_mut().expect("scène ajoutée juste avant").panneaux.push(panel_entry(panel, &|c| c, &|c| drawn.contains(&c), false));
        }
    }
    let used = used_sounds(doc);
    Manifest {
        format: FORMAT.into(),
        version: VERSION,
        largeur: doc.width,
        hauteur: doc.height,
        cadre: Some(doc.frame),
        cadence: doc.fps,
        papier: doc.paper,
        sequences,
        sons: used
            .iter()
            .map(|id| {
                let s = &doc.sounds[id];
                SoundEntry {
                    id: s.id,
                    nom: s.name.clone(),
                    fichier: sound_file(s),
                }
            })
            .collect(),
        pistes_son: doc
            .audio_tracks
            .iter()
            .map(|t| TrackEntry {
                id: t.id,
                nom: t.name.clone(),
                muette: t.muted,
                volume: t.volume,
                verrouillee: t.locked,
                extraits: t
                    .clips
                    .iter()
                    .filter(|c| used.contains(&c.sound))
                    .map(|c| ClipEntry {
                        id: c.id,
                        son: c.sound,
                        debut: c.start,
                        decalage: c.offset,
                        duree: c.length,
                    })
                    .collect(),
            })
            .collect(),
        guides: doc.guides.clone(),
        planche: doc
            .board
            .strokes
            .iter()
            .map(|s| NoteEntry {
                couleur: s.color,
                points: s.points.iter().map(|p| [p.x, p.y, p.r]).collect(),
                attache: s.anchor,
            })
            .collect(),
        planche_textes: doc
            .board
            .texts
            .iter()
            .map(|t| NoteTextEntry {
                texte: t.text.clone(),
                position: [t.pos.x, t.pos.y],
                taille: t.size,
                couleur: t.color,
                attache: t.anchor,
            })
            .collect(),
        groupes: if doc.groups == crate::groups::defaults() {
            Vec::new()
        } else {
            doc.groups
                .iter()
                .map(|g| GroupEntry { nom: g.name.clone(), couleur: g.color, montre: g.shown })
                .collect()
        },
        image_de_depart: doc.start_frame,
        fond: BackgroundEntry { sorte: doc.background, couleur_b: doc.paper_b },
        volume_general: doc.master_volume,
        storyboard_verrouille: doc.storyboard_locked,
        numerotation_verrouillee: doc.numbering_locked,
        proprietes: {
            let p = &doc.properties;
            PropertiesEntry {
                titre: p.title.clone(),
                informations: p.info.clone(),
                auteur: p.author.clone(),
                cree: p.created,
                modifie: p.modified,
                notes: p.notes.clone(),
                copyright: p.copyright.clone(),
                fond: p.fond,
                serie: p.serie.clone(),
                episode: p.episode.clone(),
                modele_noms: p.modele_noms.clone(),
                budget_ms: p.budget_ms,
                budgets_sequences: p.budgets_sequences.iter().filter(|(_, ms)| **ms > 0).map(|(s, ms)| BudgetSequenceEntry { sequence: *s, ms: *ms }).collect(),
            }
        },
        dessins_hors: if avec_plans { hors.iter().filter(|id| drawn.contains(id)).copied().collect() } else { Vec::new() },
        partage: None,
    }
}

pub struct Loaded {
    pub doc: Document,
    /// Dessins présents dans le dossier, à la version chargée (rien à réécrire au prochain enregistrement).
    pub written: HashMap<CelId, u64>,
    /// Problèmes non bloquants (dessin manquant ou illisible…).
    pub warnings: Vec<String>,
    /// Ancien format (3) converti à l'ouverture : feuille doublée autour du cadre.
    pub converted: bool,
    /// Dessins vectoriels affichés en rendu rapide (`load_quick`) : leur rendu exact est à
    /// demander au rejeu en arrière-plan (`rejeu.rs`).
    pub approx: Vec<CelId>,
    /// Dessins cités par le projet que le dossier n'a pas pu donner (absents, pas encore arrivés d'un dossier synchronisé,
    /// illisibles). Un plan partagé qui en a un est « en attente » : on ne le prend pas, on ne l'enregistre pas.
    pub manquants: BTreeSet<CelId>,
    /// Projet partagé : ce que la session a besoin de savoir de l'ouverture (`projet_partage.rs`).
    pub(crate) partage: Option<partage_io::Ouverture>,
    /// Projet partagé : la session, démarrée sur le fil d'ouverture (le dossier partagé n'est jamais lu sur celui de l'interface),
    /// ou pourquoi elle n'a pas pu l'être (`cooperationui::rejoindre_dans_le_fil`).
    pub(crate) session: Option<crate::cooperation::Session>,
    pub(crate) erreur_equipe: Option<String>,
}

/// Le plus grand numéro de séquence, de scène, de plan, de marque, de piste, d'extrait ou de son du manifeste.
fn top_id(m: &Manifest) -> u64 {
    let plans = m.sequences.iter().flat_map(|q| std::iter::once(q.id).chain(q.scenes.iter().flat_map(|s| std::iter::once(s.id).chain(s.panneaux.iter().flat_map(|p| std::iter::once(p.id).chain(p.marques.iter().map(|k| k.id)))))));
    let sons = m.pistes_son.iter().flat_map(|t| std::iter::once(t.id).chain(t.extraits.iter().map(|c| c.id))).chain(m.sons.iter().map(|s| s.id));
    plans.chain(sons).max().unwrap_or(0)
}

/// Un projet enregistré avec des identifiants en double (deux plans collés qui gardaient le même,
/// une scène reprise deux fois…) se relit avec tous ses plans : chaque entrée du manifeste est un
/// plan, une scène, une séquence à part entière. Le premier garde son numéro, les suivants en
/// reçoivent un neuf (au-dessus de `floor` et de tout ce que le manifeste contient). Renvoie le
/// nombre d'identifiants changés.
fn unique_ids(manifest: &mut Manifest, floor: u64) -> usize {
    let mut top = floor;
    for q in &manifest.sequences {
        top = top.max(q.id);
        for s in &q.scenes {
            top = top.max(s.id);
            for p in &s.panneaux {
                top = top.max(p.id).max(p.marques.iter().map(|m| m.id).max().unwrap_or(0));
            }
        }
    }
    top = top.max(manifest.pistes_son.iter().flat_map(|t| std::iter::once(t.id).chain(t.extraits.iter().map(|c| c.id))).max().unwrap_or(0));
    top = top.max(manifest.sons.iter().map(|s| s.id).max().unwrap_or(0));
    let mut changed = 0;
    let mut take = |seen: &mut BTreeSet<u64>, id: &mut u64| {
        if !seen.insert(*id) {
            top += 1;
            *id = top;
            seen.insert(top);
            changed += 1;
        }
    };
    let (mut seqs, mut scenes, mut cards) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for q in &mut manifest.sequences {
        take(&mut seqs, &mut q.id);
        for s in &mut q.scenes {
            take(&mut scenes, &mut s.id);
            for p in &mut s.panneaux {
                take(&mut cards, &mut p.id);
                for m in &mut p.marques {
                    take(&mut cards, &mut m.id);
                }
            }
        }
    }
    changed
}

/// Ouvre le projet, dessins vectoriels rendus exactement (le moteur rejoué sur chaque trait : ce
/// que l'application affiche une fois le rejeu terminé).
pub fn load(manifest_path: &Path) -> Result<Loaded, String> {
    load_with(manifest_path, false, None)
}

/// Comme `load_quick_progress`, sans suivre l'avancement (essais).
#[cfg(test)]
pub fn load_quick(manifest_path: &Path) -> Result<Loaded, String> {
    load_with(manifest_path, true, None)
}

/// Ouvre le projet tout de suite : les dessins vectoriels texturés sont affichés en rendu rapide
/// (traits lisses, sans rejeu du moteur) et listés dans `approx`, pour un rendu exact en
/// arrière-plan. Règle d'Anthony : rien ne bloque l'interface. `progress` tient le compte des
/// dessins lus / à lire : l'interface l'affiche pendant qu'un autre fil ouvre le projet
/// (`ouverture.rs`).
pub fn load_quick_progress(manifest_path: &Path, progress: &Progress) -> Result<Loaded, String> {
    load_with(manifest_path, true, Some(progress))
}

/// Avancement d'une ouverture, lu par l'interface pendant qu'un autre fil travaille.
#[derive(Default)]
pub struct Progress {
    pub done: std::sync::atomic::AtomicUsize,
    pub total: std::sync::atomic::AtomicUsize,
}

fn load_with(manifest_path: &Path, quick: bool, progress: Option<&Progress>) -> Result<Loaded, String> {
    // (Des messages en français qui disent quel fichier : « stream did not contain valid UTF-8 »
    // ou « Le chemin d'accès spécifié est introuvable » ne disent pas ce qui ne va pas.)
    let marque_structure = fs::metadata(manifest_path).ok().map(|m| crate::depot::marque_de(&m));
    let octets = fs::read(manifest_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("projet introuvable : {}", manifest_path.display())
        } else {
            format!("manifeste illisible : {} ({e})", manifest_path.display())
        }
    })?;
    let text = String::from_utf8(octets).map_err(|_| "manifeste illisible : le fichier n'est pas du texte (UTF-8), il est abîmé".to_string())?;
    let manifest: Manifest =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("manifeste illisible : {e}"))?;
    if manifest.format != FORMAT {
        return Err("ce fichier n'est pas un projet FEANOR".into());
    }
    if manifest.version > version_partage_ecrite() || (manifest.version > VERSION && manifest.partage.is_none()) {
        return Err(format!(
            "projet créé par une version plus récente de FEANOR (format {})",
            manifest.version
        ));
    }
    let dir = manifest_path.parent().unwrap_or(Path::new("."));
    if manifest.partage.is_some() {
        // Projet partagé : la structure est ici, les plans dans `plans/` (`projet_partage.rs`).
        let mut loaded = partage_io::ouvrir(manifest, dir, quick, progress)?;
        if let Some(o) = loaded.partage.as_mut() {
            o.nom_structure = manifest_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            o.hash_fichier_structure = crate::empreinte::de_octets(text.as_bytes());
            o.marque_structure = marque_structure;
        }
        return Ok(loaded);
    }
    load_manifest(manifest, dir, quick, progress)
}

/// La suite de l'ouverture, une fois le manifeste lu : les dessins de `dir/dessins/`, les sons, les
/// annexes, puis le document. (Les instantanés des envois passent aussi par là, `charger_plans`.)
fn load_manifest(manifest: Manifest, dir: &Path, quick: bool, progress: Option<&Progress>) -> Result<Loaded, String> {
    load_manifest_avec(manifest, dir, quick, progress, Annexes::default())
}

/// Ce que l'ouverture prend ailleurs que dans les fichiers annexes du dossier, ou ne lit pas : un projet partagé donne
/// lui-même les versions, le contenu rogné, les scènes 3D de ses plans et les commentaires (fusionnés de tous les
/// auteurs) ; un plan relu seul ne lit rien d'autre que ses dessins (`projet_partage.rs`). `Annexes::default()` est
/// l'ouverture d'un projet classique : tout vient du dossier.
#[derive(Default)]
pub(crate) struct Annexes {
    /// Les quatre annexes ci-dessous remplacent la lecture de leur fichier (même vides).
    pub fournies: bool,
    /// Un plan relu seul : ni transitions, ni références, ni effets de son, ni envois… et les textures déjà connues
    /// ne sont pas relues.
    pub plans_seuls: bool,
    pub versions: Option<crate::planvers::Fichier>,
    pub reserves: Option<crate::rognage::Loaded>,
    pub commentaires: Vec<crate::commentaires::Comment>,
    /// Sons déjà en mémoire, par numéro : ils ne sont pas relus du dossier (relecture de la structure).
    pub sons_connus: HashMap<u64, Sound>,
}

fn load_manifest_avec(mut manifest: Manifest, dir: &Path, quick: bool, progress: Option<&Progress>, mut annexes: Annexes) -> Result<Loaded, String> {
    let (w, h) = (manifest.largeur, manifest.hauteur);
    if w == 0 || h == 0 {
        return Err("taille d'image invalide dans le manifeste".into());
    }
    // (Un manifeste abîmé qui annonce une feuille de milliards de pixels ferait demander à SHOT des
    // milliards de tuiles : il tomberait sans un mot.)
    if w > crate::vector::MAX_SIDE * 2 || h > crate::vector::MAX_SIDE * 2 {
        return Err(format!("taille d'image démesurée dans le manifeste ({w} × {h})"));
    }
    let mut warnings = Vec::new();
    // Les textures d'abord : les dessins vectoriels texturés en ont besoin pour leur rendu.
    load_textures(dir, &mut warnings, annexes.plans_seuls);
    crate::imagevec::charger_dossier(dir, &mut warnings, annexes.plans_seuls);
    // Les autres versions des plans (`plans_versions.json`) : leurs plans passent par la même lecture que
    // les autres, dans une séquence à part qu'on retire ensuite du film (`planvers.rs`).
    let mut versions = if annexes.fournies { annexes.versions.take() } else { crate::planvers::lire(dir, &mut warnings) };
    let mut a_ranger: Option<(u64, u64, Vec<(u64, u32)>)> = None;
    if let Some(f) = versions.as_mut() {
        let (mut cles, mut panneaux) = (Vec::new(), Vec::new());
        for p in &mut f.plans {
            for v in &mut p.autres {
                match serde_json::from_value::<PanelEntry>(std::mem::take(&mut v.entree)) {
                    Ok(e) => {
                        cles.push((p.plan, v.numero));
                        panneaux.push(e);
                    }
                    Err(e) => warnings.push(format!("version {} d'un plan : illisible ({e}), ignorée.", crate::planvers::etiquette(v.numero))),
                }
            }
        }
        if !panneaux.is_empty() {
            let top = top_id(&manifest);
            manifest.sequences.push(SequenceEntry { id: top + 1, nom: String::new(), numero: String::new(), scenes: vec![SceneEntry { id: top + 2, nom: String::new(), numero: String::new(), panneaux }] });
            a_ranger = Some((top + 1, top + 2, cles));
        }
    }
    // Les variantes rangées des séquences (`variantes.json`) : leurs plans, et ceux de leurs versions, passent aussi par
    // la lecture ordinaire, dans une séquence cachée qu'on retire ensuite du film (`variantes.rs`). Un plan relu seul
    // ne lit rien de tout cela.
    let mut variantes = if annexes.plans_seuls { None } else { crate::variantes::lire(dir, &mut warnings) };
    let mut rognages_variantes: HashMap<crate::variantes::Cle, crate::rognage::Loaded> = HashMap::new();
    let cachees = variantes.as_mut().and_then(|f| cacher_variantes(&mut manifest, f, &mut warnings, &mut rognages_variantes));
    // Les plans annulés (`plans_annules.json`) passent par la même lecture, dans une séquence cachée qu'on retire ensuite du film.
    let mut annules_lus = if annexes.plans_seuls || annexes.fournies { None } else { crate::planannule::lire(dir, &mut warnings) };
    let (annules_cachees, mut rognages_annules, annules_illisibles) = match annules_lus.as_mut() {
        Some(f) => cacher_annules(&mut manifest, f, &mut warnings),
        None => (None, Vec::new(), Vec::new()),
    };
    // Les dessins que les variantes citent : s'il en manque, la lecture les retire des plans, et il faut s'en souvenir.
    let cels_variantes: BTreeSet<CelId> = cachees
        .as_ref()
        .map(|c| {
            manifest
                .sequences
                .iter()
                .filter(|s| s.id == c.sequence)
                .flat_map(|s| &s.scenes)
                .flat_map(|s| &s.panneaux)
                .flat_map(|p| &p.calques)
                .flat_map(|l| l.expositions.iter().filter_map(|r| r.dessin).chain(l.calque_couleur.iter().flat_map(|c| c.griffonnages.iter().map(|g| g[1]))))
                .collect()
        })
        .unwrap_or_default();
    let mut cels: HashMap<CelId, Cel> = HashMap::new();
    let mut missing: BTreeSet<CelId> = BTreeSet::new();
    // Le contenu rogné des plans (`rognages.json`) : ses dessins se chargent avec les autres.
    let reserves = if annexes.fournies { annexes.reserves.take() } else { load_reserves(dir, &mut warnings) };
    let mut referenced: BTreeSet<CelId> = manifest
        .sequences
        .iter()
        .flat_map(|s| &s.scenes)
        .flat_map(|s| &s.panneaux)
        .flat_map(|p| &p.calques)
        .flat_map(|l| {
            // (Les griffonnages d'un calque de couleur sont des dessins du plan qu'aucune timeline ne cite.)
            let griffonnages = l.calque_couleur.iter().flat_map(|c| c.griffonnages.iter().map(|g| g[1]));
            l.expositions.iter().filter_map(|r| r.dessin).chain(griffonnages)
        })
        .collect();
    if let Some(r) = &reserves {
        referenced.extend(r.cels());
    }
    for r in rognages_variantes.values() {
        referenced.extend(r.cels());
    }
    for r in rognages_annules.iter().flatten() {
        referenced.extend(r.cels());
    }
    let mut approx = Vec::new();
    // Les dessins se lisent en parallèle (décodage PNG ou rendu des traits : 25 ms chacun, un par
    // cœur) ; les résultats reviennent dans l'ordre des identifiants, comme en une seule boucle.
    let referenced: Vec<CelId> = referenced.into_iter().collect();
    if let Some(p) = progress {
        p.total.store(referenced.len(), std::sync::atomic::Ordering::Relaxed);
    }
    let hors: BTreeSet<CelId> = manifest.dessins_hors.iter().copied().collect();
    let loaded = crate::parallele::par_map(&referenced, LOAD_THREADS, |_, &id| {
        let r = load_drawing(dir, id, w, h, quick, hors.contains(&id));
        if let Some(p) = progress {
            p.done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        r
    });
    for (id, result) in referenced.into_iter().zip(loaded) {
        match result {
            Ok((cel, approximate, warning)) => {
                if approximate {
                    approx.push(id);
                }
                warnings.extend(warning);
                cels.insert(id, cel);
            }
            Err(e) => {
                warnings.push(e);
                missing.insert(id);
            }
        }
    }

    let mut sounds: HashMap<u64, Sound> = HashMap::new();
    // Fichiers de sons et de modèles illisibles : l'enregistrement ne les efface pas.
    let mut unreadable_files: BTreeSet<PathBuf> = BTreeSet::new();
    for entry in &manifest.sons {
        if let Some(known) = annexes.sons_connus.get(&entry.id) {
            sounds.insert(entry.id, known.clone());
            continue;
        }
        match load_sound(dir, entry) {
            Ok(sound) => {
                sounds.insert(sound.id, sound);
            }
            Err(e) => {
                if Path::new(&entry.fichier).components().count() == 1 {
                    unreadable_files.insert(dir.join(SOUNDS).join(&entry.fichier));
                }
                warnings.push(e);
            }
        }
    }
    let audio_tracks = manifest
        .pistes_son
        .iter()
        .map(|t| AudioTrack {
            id: t.id,
            name: t.nom.clone(),
            muted: t.muette,
            // (Le solo vient de `effets_son.json`.)
            solo: false,
            volume: t.volume.clamp(0.0, 4.0),
            clips: t
                .extraits
                .iter()
                .filter(|c| sounds.contains_key(&c.son) && c.duree > 0)
                .map(|c| SoundClip::new(c.id, c.son, c.debut, c.decalage, c.duree))
                .collect(),
            locked: t.verrouillee,
        })
        .collect();

    let renumbered = unique_ids(&mut manifest, cels.keys().chain(sounds.keys()).copied().max().unwrap_or(0));
    if renumbered > 0 {
        warnings.push(format!("{renumbered} identifiant(s) de plan, de scène ou de séquence en double ont été renumérotés : aucun plan n'est perdu."));
    }
    let mut structure = Structure {
        sequences: Vec::new(),
        scenes: Vec::new(),
        panels: Vec::new(),
        audio_tracks,
        guides: manifest
            .guides
            .into_iter()
            .filter(|g| (1..=3).contains(&g.points.len()) && g.points.iter().flatten().all(|v| v.is_finite()))
            .collect(),
        paper: manifest.papier,
        fps: manifest.cadence.max(1),
        board: crate::board::Board {
            strokes: manifest
                .planche
                .into_iter()
                .map(|n| crate::board::NoteStroke {
                    color: n.couleur,
                    points: n
                        .points
                        .into_iter()
                        .filter(|p| p.iter().all(|v| v.is_finite()))
                        .map(|[x, y, r]| crate::vector::VPoint { x, y, r: r.clamp(1e-4, 200.0) })
                        .collect(),
                    anchor: n.attache,
                })
                .filter(|s| !s.points.is_empty())
                .collect(),
            texts: manifest
                .planche_textes
                .into_iter()
                .filter(|t| t.position.iter().all(|v| v.is_finite()) && t.taille.is_finite() && t.taille > 0.0)
                .map(|t| crate::board::NoteText {
                    pos: eframe::egui::pos2(t.position[0], t.position[1]),
                    text: t.texte,
                    size: t.taille.min(500.0),
                    color: t.couleur,
                    anchor: t.attache,
                })
                .collect(),
        },
        groups: crate::groups::completed(
            manifest
                .groupes
                .into_iter()
                .map(|g| crate::groups::LayerGroup { name: g.nom, color: g.couleur, shown: g.montre })
                .collect(),
        ),
        start_frame: manifest.image_de_depart.min(1_000_000),
        background: manifest.fond.sorte,
        paper_b: manifest.fond.couleur_b,
        master_volume: if manifest.volume_general.is_finite() { manifest.volume_general.clamp(0.0, 4.0) } else { 1.0 },
        storyboard_locked: manifest.storyboard_verrouille,
        numbering_locked: manifest.numerotation_verrouillee,
        properties: {
            let p = manifest.proprietes;
            crate::document::Properties {
                title: p.titre,
                info: p.informations,
                author: p.auteur,
                created: p.cree,
                modified: p.modifie,
                notes: p.notes,
                copyright: p.copyright,
                fond: p.fond,
                serie: p.serie,
                episode: p.episode,
                modele_noms: p.modele_noms,
                budget_ms: p.budget_ms.min(crate::budget::MAX_MS),
                // (Une cible d'une séquence qui n'existe plus est lue comme les autres, `Document::cible_de_sequence` l'ignore.)
                budgets_sequences: p.budgets_sequences.into_iter().filter(|e| e.ms > 0).map(|e| (e.sequence, e.ms.min(crate::budget::MAX_MS))).collect(),
            }
        },
        // Les commentaires viennent de leur fichier, après les plans (`load_comments`).
        comments: Vec::new(),
        // La réserve du rognage aussi (`rognages.json`, `load_reserves`).
        reserves: Default::default(),
        // Les transitions : `transitions.json`, `load_transitions`.
        transitions: Vec::new(),
        // Les références visuelles aussi (`references.json`, `references::load`).
        references: Vec::new(),
        // Les personnages : `personnages.json`, `load_personnages`.
        personnages: Vec::new(),
        // Les versions des plans : `plans_versions.json`, `planvers::installer`.
        versions: Default::default(),
        // Les variantes rangées des séquences : `variantes.json`, `variantes::installer`.
        variantes: Default::default(),
        corbeille: Default::default(),
        // Les plans annulés : `plans_annules.json`, `planannule::installer`.
        annules: Default::default(),

        // Les annotations de la vue Vignettes : `annotations.json`, `load_annotations`.
        annotations: Default::default(),
        // Le scénario gardé : `scenario.json`, `scenariogarde::load`.
        scenario: None,
        frames_rangees: None,
    };
    // Les calques CTG des projets d'avant : convertis en calques de couleur quand le document existe (`migrer_ctg`).
    let mut anciens_ctg: Vec<crate::griffonnage::AncienCtg> = Vec::new();
    // Numéros des partages (la planche-mère, `partage.rs`) : neufs pour la session, les mêmes pour tous les exemplaires d'une clé.
    let mut partages: HashMap<u64, u64> = HashMap::new();
    for seq in manifest.sequences {
        structure.sequences.push(Sequence {
            id: seq.id,
            name: seq.nom,
            number: seq.numero,
        });
        for scene in seq.scenes {
            structure.scenes.push(Scene {
                id: scene.id,
                name: scene.nom,
                number: scene.numero,
                sequence: seq.id,
            });
            // Numéros des calques de toute la scène : neufs pour la session.
            let mut keys: HashMap<u64, u64> = HashMap::new();
            for p in scene.panneaux {
                let parents: Vec<Option<usize>> = p.calques.iter().map(|l| l.dans_dossier).collect();
                let ctg_sources: Vec<Option<(Vec<usize>, u8)>> = p.calques.iter().map(|l| l.ctg.as_ref().map(|c| (c.sources.clone(), c.trou))).collect();
                let couleurs: Vec<Option<CouleurEntry>> = p.calques.iter().map(|l| l.calque_couleur.clone()).collect();
                let mut layers: Vec<Layer> = p
                    .calques
                    .into_iter()
                    .map(|l| Layer {
                        blend: l.fusion,
                        scene_wide: l.toute_la_scene.map(|k| SceneWide {
                            key: *keys.entry(k).or_insert_with(fresh_layer_id),
                            scene: scene.id,
                        }),
                        partage: l.partage.map(|k| *partages.entry(k).or_insert_with(fresh_layer_id)),
                        id: fresh_layer_id(),
                        name: l.nom,
                        kind: if l.kind == "bitmap" {
                            LayerKind::Bitmap
                        } else {
                            LayerKind::Vector
                        },
                        visible: l.visible,
                        opacity: l.opacite.clamp(0.0, 1.0),
                        locked: l.verrouille,
                        transform: l
                            .transformation
                            .as_ref()
                            .filter(|t| [t.x, t.y, t.echelle, t.rotation, t.etirement_x, t.etirement_y, t.inclinaison_x, t.inclinaison_y].iter().all(|v| v.is_finite()))
                            .map(|t| LayerTransform {
                                x: t.x,
                                y: t.y,
                                scale: t.echelle.clamp(0.01, 100.0),
                                angle: t.rotation,
                                stretch_x: LayerTransform::clamp_stretch(t.etirement_x),
                                stretch_y: LayerTransform::clamp_stretch(t.etirement_y),
                                tilt_x: LayerTransform::clamp_tilt(t.inclinaison_x),
                                tilt_y: LayerTransform::clamp_tilt(t.inclinaison_y),
                                dz: 0.0,
                            })
                            .unwrap_or_default(),
                        pos_keys: crate::poskeys::from_entries(l.cles_position),
                        trajectoire: l.trajectoire.and_then(crate::trajectoire::Trajectoire::assainie),
                        opacity_keys: crate::opacitecles::from_entries(l.cles_opacite),
                        unie: l.couleur_unie,
                        depth: if l.profondeur.is_finite() {
                            l.profondeur.clamp(Layer::MIN_DEPTH, 1000.0)
                        } else {
                            0.0
                        },
                        frames: from_exposures(&l.expositions)
                            .into_iter()
                            .map(|f| f.filter(|id| !missing.contains(id)))
                            .collect(),
                        onion: l.pelure,
                        keep_alpha: l.transparence,
                        stencil: l.pochoir,
                        stencil_negative: l.pochoir_negatif,
                        position_locked: l.position_verrouillee,
                        group: if (l.groupe as usize) < crate::groups::COUNT { l.groupe } else { 0 },
                        collapsed: l.replie,
                        folder: l.dossier,
                        parent: None,
                        pre: l.avant,
                        post: l.apres,
                        auto_break: l.cassure_auto,
                        auto_create: l.creation_auto,
                        names: l.noms,
                        color_marks: l.marques_couleur.into_iter().filter(|(_, k)| (*k as usize) < crate::marks::COLORS.len()).collect(),
                        roles: l.roles,
                        fx: l.effets,
                        fx_enabled: !l.effets_eteints,
                        // Calque vidéo (format 21) : ses images dans `videos/<id>/`.
                        video: l.video.filter(|v| v.images > 0 && v.ips > 0.0 && matches!(v.ext.as_str(), "jpg" | "png")).map(|v| {
                            crate::video::register_dir(v.id, dir.join(VIDEOS).join(v.id.to_string()));
                            crate::video::VideoClip { id: v.id, name: v.nom, frames: v.images, fps: v.ips, start: v.debut, ext: v.ext }
                        }),
                        // Fond sous le trait (format 23).
                        fond: l.fond,
                        // Calque d'effet (format 27).
                        effet: l.calque_effet.map(crate::calqueffet::Entree::into_model),
                        // Calque de couleur (format 30) : le lien vient après (rang du trait → son id).
                        couleur: None,
                        decor: l.decor,
                        personnage: l.personnage,
                        annotation: l.annotation,
                    })
                    .collect();
                // Dossiers (format 14) : le rang du dossier devient l'id de son calque.
                for (i, fi) in parents.into_iter().enumerate() {
                    if let Some(id) = fi.and_then(|fi| layers.get(fi)).filter(|f| f.folder).map(|f| f.id) {
                        layers[i].parent = Some(id);
                    }
                }
                // Calques CTG (formats 20 à 35, plus écrits) : les rangs des sources deviennent leurs id ; le calque
                // devient un calque de couleur une fois les dessins chargés (`griffonnage::migrer_ctg`).
                for (i, entry) in ctg_sources.into_iter().enumerate() {
                    if let Some((sources, gap)) = entry {
                        let ids = sources.into_iter().filter(|&k| k != i).filter_map(|k| layers.get(k).map(|l| l.id)).collect();
                        anciens_ctg.push(crate::griffonnage::AncienCtg { calque: layers[i].id, sources: ids, trou: gap });
                    }
                }
                // Calques de couleur (format 30) : le rang du calque de trait devient son id, les
                // couples de dessins sont gardés si leurs deux dessins sont là. Format 36 : libre (sans trait), griffonnages,
                // sources supplémentaires, mode.
                for (i, entry) in couleurs.into_iter().enumerate() {
                    let Some(e) = entry else { continue };
                    let line = match e.ligne {
                        None => 0,
                        Some(rang) => match layers.get(rang).filter(|_| rang != i) {
                            Some(l) => l.id,
                            // (Le trait est introuvable : un calque qui n'a pas de griffonnages redevient ordinaire.)
                            None if e.remplissage => 0,
                            None => continue,
                        },
                    };
                    let mut c = crate::calquecouleur::CalqueCouleur::new(line);
                    c.dessins = e.dessins.into_iter().filter(|[t, k]| cels.contains_key(t) && cels.contains_key(k)).map(|[t, k]| (t, k)).collect();
                    c.trous = e.trous.min(50);
                    c.zone = e.zone;
                    c.griffonner = e.griffonner;
                    c.remplissage = e.remplissage;
                    c.griffonnages = e.griffonnages.into_iter().filter(|[p, g]| cels.contains_key(p) && cels.contains_key(g)).map(|[p, g]| (p, g)).collect();
                    c.sources = e.sources.into_iter().filter(|&k| k != i).filter_map(|k| layers.get(k).map(|l| l.id)).collect();
                    layers[i].couleur = Some(c);
                }
                crate::calquecouleur::reparer(&mut layers);
                crate::folders::normalize(&mut layers);
                if layers.is_empty() {
                    layers.push(Layer::new("Dessin", LayerKind::Vector));
                }
                let mut camera: Vec<CameraKey> = Vec::new();
                for k in &p.camera {
                    let finite =
                        [k.x, k.y, k.zoom, k.rotation, k.avance, k.adoucir_entree, k.adoucir_sortie, k.bascule, k.panoramique];
                    if finite.iter().all(|v| v.is_finite()) {
                        let cam = Camera {
                            x: k.x,
                            y: k.y,
                            zoom: k.zoom.clamp(0.05, 50.0),
                            angle: k.rotation,
                            dolly: k.avance,
                            tilt: k.bascule.clamp(-85.0, 85.0),
                            pan: k.panoramique.clamp(-85.0, 85.0),
                        };
                        crate::camera::set_key(&mut camera, k.image, cam);
                        if let Some(x) = camera.iter_mut().find(|x| x.frame == k.image) {
                            x.smooth = k.doux;
                            x.ease_in = k.adoucir_entree.max(0.0);
                            x.ease_out = k.adoucir_sortie.max(0.0);
                        }
                    }
                }
                structure.panels.push(Panel {
                    id: p.id,
                    scene: scene.id,
                    length: p.duree.min(crate::rognage::MAX_PLAN),
                    layers,
                    action: p.action,
                    dialogue: p.dialogue,
                    notes: p.notes,
                    camera,
                    saccade: p.saccade.map_or_else(crate::camera::Saccade::default, |s| crate::camera::Saccade { actif: s.actif, cadence: s.cadence }.normalise()),
                    marks: marks_of(p.marques),
                    cues: crate::document::Cues {
                        bookmarks: {
                            let mut b = p.signets;
                            b.sort_unstable();
                            b.dedup();
                            b
                        },
                        in_point: p.entree,
                        out_point: p.sortie,
                        notes: p
                            .notes_dessinees
                            .into_iter()
                            .filter(|n| n.epaisseur.is_finite() && n.points.iter().flatten().all(|v| v.is_finite()))
                            .map(|n| crate::document::TimeNote {
                                color: n.couleur.min(2),
                                width: n.epaisseur.clamp(0.5, 40.0),
                                points: n.points.into_iter().map(|[x, y]| [x, y.clamp(0.0, 1.0)]).collect(),
                            })
                            .collect(),
                    },
                    color: p.couleur.filter(|k| (*k as usize) < crate::marks::COLORS.len()),
                });
            }
        }
    }
    // Les plans rangés des versions ne sont pas des plans du film : mis de côté pour `planvers::installer`.
    let mut rangees: Vec<Panel> = Vec::new();
    if let Some((sequence, scene, _)) = &a_ranger {
        let (gardes, extra): (Vec<Panel>, Vec<Panel>) = std::mem::take(&mut structure.panels).into_iter().partition(|p| p.scene != *scene);
        structure.panels = gardes;
        rangees = extra;
        structure.scenes.retain(|s| s.id != *scene);
        structure.sequences.retain(|s| s.id != *sequence);
    }
    // De même les plans des variantes rangées.
    let mut panneaux_variantes: HashMap<crate::variantes::Cle, Panel> = HashMap::new();
    if let Some(c) = &cachees {
        let (gardes, extra): (Vec<Panel>, Vec<Panel>) = std::mem::take(&mut structure.panels).into_iter().partition(|p| !c.scenes.contains_key(&p.scene));
        structure.panels = gardes;
        for p in extra {
            if let Some(cle) = c.scenes.get(&p.scene) {
                panneaux_variantes.insert(*cle, p);
            }
        }
        structure.scenes.retain(|s| !c.scenes.contains_key(&s.id));
        structure.sequences.retain(|s| s.id != c.sequence);
    }
    // De même les plans annulés.
    let mut panneaux_annules: Vec<Option<Panel>> = Vec::new();
    let mut versions_annules: Vec<Vec<Panel>> = Vec::new();
    if let Some(c) = &annules_cachees {
        panneaux_annules = (0..c.plans).map(|_| None).collect();
        versions_annules = (0..c.plans).map(|_| Vec::new()).collect();
        let (gardes, extra): (Vec<Panel>, Vec<Panel>) = std::mem::take(&mut structure.panels).into_iter().partition(|p| !c.scenes.contains_key(&p.scene));
        structure.panels = gardes;
        for mut p in extra {
            p.id = c.numeros.get(&p.id).copied().unwrap_or(p.id);
            for m in &mut p.marks {
                m.id = c.numeros.get(&m.id).copied().unwrap_or(m.id);
            }
            if let Some(&(i, version)) = c.scenes.get(&p.scene) {
                if version {
                    versions_annules[i].push(p);
                } else {
                    panneaux_annules[i] = Some(p);
                }
            }
        }
        structure.scenes.retain(|s| !c.scenes.contains_key(&s.id));
        structure.sequences.retain(|s| s.id != c.sequence);
    }
    if structure.panels.is_empty() {
        return Err("le projet ne contient aucun plan".into());
    }
    // Les commentaires, dans leur fichier à côté du manifeste (absent : aucun commentaire).
    structure.comments = if annexes.fournies { std::mem::take(&mut annexes.commentaires) } else { load_comments(dir, &mut warnings) };
    if !annexes.plans_seuls {
        // Les transitions du montage, de même (`transitions.json`).
        structure.transitions = load_transitions(dir, &mut warnings);
        // Les références visuelles : `references.json` et le dossier `references/` (absents : aucune).
        structure.references = crate::references::load(dir, &mut warnings);
        // Les personnages de la brosse par personnage (`personnages.json`, absent : aucun).
        structure.personnages = load_personnages(dir, &mut warnings);
        // Les annotations de la vue Vignettes (`annotations.json`, absent : aucune).
        structure.annotations = load_annotations(dir, &mut warnings);
        // Le scénario gardé dans le projet (`scenario.json`, absent : aucun).
        structure.scenario = crate::scenariogarde::load(dir, &mut warnings);
    }
    let frame = manifest.cadre;
    let mut written: HashMap<CelId, u64> = cels.keys().map(|&id| (id, 0)).collect();
    let mut doc = Document::from_parts(w, h, structure, cels, sounds);
    if !annexes.plans_seuls {
        // Les effets des sons (fondus, volume, égaliseur, écho, solo : `effets_son.json`).
        if let Some(fx) = load_sound_fx(dir, &mut warnings) {
            fx.install(&mut doc);
        }
        // Les sons proposés (`sons_proposes.json`).
        doc.sons_proposes = load_sons_proposes(dir, &mut warnings);
    }
    if let Some(r) = reserves {
        r.install(&mut doc, &missing);
    }
    if !annexes.plans_seuls {
        // L'ancienne planche de crayonnés (supprimée) : si elle gardait des croquis, on le dit ; le fichier reste en place.
        crate::crayonnesheritage::signaler(dir, &mut warnings);
        // Les envois (`envois.json`, absent : aucun) ; leurs instantanés ne se lisent qu'à la demande.
        doc.envois = crate::envois::load(dir, &mut warnings);
    }
    // La corbeille (`corbeille.json`) : ses entrées seulement ; ses dessins restent dans `dessins/`, relus à la demande. Ses numéros
    // sont pris (aucun numéro neuf ne tombe sur un fichier qu'elle garde).
    if !annexes.plans_seuls && !annexes.fournies {
        doc.corbeille = crate::corbeille::lire(dir, &mut warnings);
        let haut = doc.corbeille.entrees.iter().map(|e| e.id_max).max().unwrap_or(0);
        doc.reserve_id(haut);
    }
    // Les plans annulés : hors du film, à leur place sur la planche.
    if let Some(f) = annules_lus {
        crate::planannule::installer(&mut doc, f, panneaux_annules, versions_annules, std::mem::take(&mut rognages_annules), annules_illisibles, &missing);
    }
    // Les autres versions des plans.
    if let (Some(f), Some((_, _, cles))) = (versions, a_ranger) {
        crate::planvers::installer(&mut doc, f, cles, rangees, &mut warnings);
    }
    // Les variantes rangées des séquences.
    if let Some(f) = variantes {
        let manquants: BTreeSet<CelId> = cels_variantes.intersection(&missing).copied().collect();
        crate::variantes::installer(&mut doc, f, panneaux_variantes, rognages_variantes, &missing, !manquants.is_empty(), &mut warnings);
    }
    // Les calques CTG d'un projet d'avant deviennent des calques de couleur qui montrent la même image (plans et autres
    // versions des plans).
    let convertis = crate::griffonnage::migrer_ctg_document(&mut doc, &anciens_ctg);
    if convertis > 0 {
        warnings.push(format!(
            "{convertis} calque{} CTG converti{} en calque de couleur : même image, griffonnages gardés (l'aplat se refait tout seul quand les traits changent).",
            if convertis > 1 { "s" } else { "" },
            if convertis > 1 { "s" } else { "" }
        ));
    }
    // Projet d'avant les plans (13a) : chaque scène devient un plan, ses panneaux bout à bout.
    if !doc.is_plans() {
        let n = doc.panels.len();
        warnings.extend(doc.into_plans());
        warnings.push(format!(
            "Projet converti en plans : {n} panneaux mis bout à bout en {} plans ; le début de chaque ancien panneau est une marque d'image.",
            doc.panels.len()
        ));
    }
    let converted = match frame {
        Some([fw, fh]) if fw > 0 && fh > 0 && fw <= w && fh <= h => {
            doc.frame = [fw, fh];
            false
        }
        // Format 3 : la feuille était le cadre. On la double, dessin au centre ; les fichiers de
        // dessins seront tous réécrits à la nouvelle taille au prochain enregistrement.
        _ => {
            doc.enlarge_stage(w * 2, h * 2);
            doc.frame = [w, h];
            written.clear();
            true
        }
    };
    // Les dessins illisibles ne sont plus dans le projet, mais leurs fichiers restent : un
    // enregistrement ne les efface jamais (on pourra les récupérer ou les rouvrir avec une
    // version qui les lit).
    // (Chacun a déjà son avertissement, plus haut.)
    // (Un projet partagé n'efface jamais rien : ces listes ne servent pas, et un plan relu seul ne doit pas écraser celles
    // du projet.)
    if !annexes.fournies {
        keep_unreadable(dir, &missing);
        keep_files(dir, unreadable_files);
    }
    // Ce que l'enregistrement doit savoir de chaque dessin (hors de la feuille, textures), appris ici, sur le fil
    // d'ouverture : le premier Ctrl+S ne relit pas les pixels du bord de tout le film sur celui de l'interface.
    doc.memoriser_dessins();
    Ok(Loaded {
        doc,
        written,
        warnings,
        converted,
        approx,
        manquants: missing,
        partage: None,
        session: None,
        erreur_equipe: None,
    })
}

/// Les plans annulés, cachés dans le manifeste le temps de la lecture (`cacher_annules`).
struct AnnulesCaches {
    sequence: u64,
    /// Le nombre de plans du fichier.
    plans: usize,
    /// Pour chaque scène cachée, le rang du plan dans le fichier et si c'est une de ses versions rangées.
    scenes: HashMap<u64, (usize, bool)>,
    /// Le vrai numéro de chaque plan et de chaque marque : le temps de la lecture ils en ont un neuf (le même plan peut être aussi au garage d'une
    /// autre version de séquence, `unique_ids` le renumérotait).
    numeros: HashMap<u64, u64>,
}

/// Fait entrer les plans annulés de `f` (et leurs versions rangées) dans `manifest`, un par scène d'une séquence cachée : la lecture ordinaire charge
/// leurs dessins, hors-feuille compris. Rend aussi le contenu rogné lu de chaque plan, et les entrées abîmées, telles quelles : **une entrée dont une part
/// ne se lit pas (le plan, une de ses versions, son contenu rogné) attend entière** dans le fichier, plutôt que de revenir incomplète ou décalée.
#[allow(clippy::type_complexity)]
fn cacher_annules(manifest: &mut Manifest, f: &mut crate::planannule::Fichier, warnings: &mut Vec<String>) -> (Option<AnnulesCaches>, Vec<Option<crate::rognage::Loaded>>, Vec<serde_json::Value>) {
    let top = top_id(manifest);
    let sequence = top + 1;
    let mut suivant = top + 2;
    let mut scenes: Vec<SceneEntry> = Vec::new();
    let mut cles: HashMap<u64, (usize, bool)> = HashMap::new();
    let mut numeros: HashMap<u64, u64> = HashMap::new();
    let mut rognages = Vec::with_capacity(f.plans.len());
    let mut illisibles = Vec::new();
    for (i, p) in f.plans.iter_mut().enumerate() {
        let brut = serde_json::to_value(&*p).unwrap_or(serde_json::Value::Null);
        // Tout se lit d'abord : une part abîmée, et l'entrée entière attend.
        let mut valeurs = vec![(std::mem::take(&mut p.entree), false)];
        if let Some(v) = p.versions.as_mut() {
            valeurs.extend(v.autres.iter_mut().map(|a| (std::mem::take(&mut a.entree), true)));
        }
        let lues: Result<Vec<(PanelEntry, bool)>, String> = valeurs.into_iter().map(|(v, version)| serde_json::from_value::<PanelEntry>(v).map(|e| (e, version)).map_err(|e| e.to_string())).collect();
        let rognage: Result<Option<crate::rognage::Loaded>, String> = match p.rognage.take() {
            Some(v) => crate::rognage::parse(&v.to_string()).map(Some),
            None => Ok(None),
        };
        match (lues, rognage) {
            (Ok(lues), Ok(rognage)) => {
                for (mut e, version) in lues {
                    let scene = suivant;
                    suivant += 1;
                    for id in std::iter::once(&mut e.id).chain(e.marques.iter_mut().map(|m| &mut m.id)) {
                        numeros.insert(suivant, *id);
                        *id = suivant;
                        suivant += 1;
                    }
                    scenes.push(SceneEntry { id: scene, nom: String::new(), numero: String::new(), panneaux: vec![e] });
                    cles.insert(scene, (i, version));
                }
                rognages.push(rognage);
            }
            (lues, rognage) => {
                let e = lues.err().or(rognage.err()).unwrap_or_default();
                warnings.push(format!("Un plan écarté est illisible ({e}) : il ne se montre pas, il reste dans le fichier et ses dessins sont gardés."));
                illisibles.push(brut);
                rognages.push(None);
            }
        }
    }
    if scenes.is_empty() {
        return (None, rognages, illisibles);
    }
    let plans = f.plans.len();
    manifest.sequences.push(SequenceEntry { id: sequence, nom: String::new(), numero: String::new(), scenes });
    (Some(AnnulesCaches { sequence, plans, scenes: cles, numeros }), rognages, illisibles)
}

/// Les plans des variantes rangées, cachés dans le manifeste le temps de la lecture (`cacher_variantes`).
struct VariantesCachees {
    /// La séquence cachée qui les porte.
    sequence: u64,
    /// Pour chaque scène cachée (un plan chacune), où ce plan est dans le fichier.
    scenes: HashMap<u64, crate::variantes::Cle>,
}

/// Fait entrer les plans des variantes rangées de `f` (et ceux de leurs versions) dans `manifest`, un par scène d'une
/// séquence cachée, pour que la lecture ordinaire charge leurs dessins ; garde pour chaque plan son contenu rogné lu
/// (`rognages`). Rien si le fichier n'a aucun plan lisible.
fn cacher_variantes(
    manifest: &mut Manifest,
    f: &mut crate::variantes::Fichier,
    warnings: &mut Vec<String>,
    rognages: &mut HashMap<crate::variantes::Cle, crate::rognage::Loaded>,
) -> Option<VariantesCachees> {
    use crate::variantes::Cle;
    let top = top_id(manifest);
    let sequence = top + 1;
    let mut suivant = top + 2;
    let mut scenes: Vec<SceneEntry> = Vec::new();
    let mut cles: HashMap<u64, Cle> = HashMap::new();
    let mut poser = |valeur: serde_json::Value, cle: Cle, scenes: &mut Vec<SceneEntry>| -> Result<(), String> {
        let e = serde_json::from_value::<PanelEntry>(valeur).map_err(|e| e.to_string())?;
        scenes.push(SceneEntry { id: suivant, nom: String::new(), numero: String::new(), panneaux: vec![e] });
        cles.insert(suivant, cle);
        suivant += 1;
        Ok(())
    };
    for (si, s) in f.sequences.iter_mut().enumerate() {
        // Les plans du garage (format 2), puis ceux des copies complètes d'un fichier du format 1.
        for (pi, p) in s.garage.iter_mut().enumerate() {
            if let Err(e) = poser(std::mem::take(&mut p.entree), Cle::Parque(si, pi), &mut scenes) {
                warnings.push(format!("version de séquence : un plan illisible ({e}), ignoré."));
                continue;
            }
            if let Some(vf) = p.versions.as_mut() {
                for (ri, r) in vf.autres.iter_mut().enumerate() {
                    if let Err(e) = poser(std::mem::take(&mut r.entree), Cle::ParqueVersion(si, pi, ri), &mut scenes) {
                        warnings.push(format!("version de séquence : une version de plan illisible ({e}), ignorée."));
                    }
                }
            }
            if let Some(val) = &p.rognage {
                match crate::rognage::parse(&val.to_string()) {
                    Ok(l) => {
                        rognages.insert(Cle::Parque(si, pi), l);
                    }
                    Err(e) => warnings.push(format!("version de séquence : contenu rogné illisible ({e}).")),
                }
            }
        }
        for (vi, v) in s.autres.iter_mut().enumerate() {
            for (pi, p) in v.plans.iter_mut().enumerate() {
                if let Err(e) = poser(std::mem::take(&mut p.entree), Cle::Plan(si, vi, pi), &mut scenes) {
                    warnings.push(format!("version {} d'une séquence : un plan illisible ({e}), ignoré.", crate::variantes::lettre(v.numero)));
                    continue;
                }
                if let Some(vf) = p.versions.as_mut() {
                    for (ri, r) in vf.autres.iter_mut().enumerate() {
                        if let Err(e) = poser(std::mem::take(&mut r.entree), Cle::Version(si, vi, pi, ri), &mut scenes) {
                            warnings.push(format!("version {} d'une séquence : une version de plan illisible ({e}), ignorée.", crate::variantes::lettre(v.numero)));
                        }
                    }
                }
                if let Some(val) = &p.rognage {
                    match crate::rognage::parse(&val.to_string()) {
                        Ok(l) => {
                            rognages.insert(Cle::Plan(si, vi, pi), l);
                        }
                        Err(e) => warnings.push(format!("version {} d'une séquence : contenu rogné illisible ({e}).", crate::variantes::lettre(v.numero))),
                    }
                }
            }
        }
    }
    manifest.dessins_hors.extend(f.hors.iter().copied());
    if scenes.is_empty() {
        return None;
    }
    manifest.sequences.push(SequenceEntry { id: sequence, nom: String::new(), numero: String::new(), scenes });
    Some(VariantesCachees { sequence, scenes: cles })
}

/// Dessins dont le fichier n'a pas pu être lu à l'ouverture, par dossier de projet : le prochain
/// enregistrement ne les efface pas.
static UNREADABLE: std::sync::Mutex<BTreeMap<PathBuf, BTreeSet<CelId>>> = std::sync::Mutex::new(BTreeMap::new());

fn keep_unreadable(dir: &Path, ids: &BTreeSet<CelId>) {
    let mut m = UNREADABLE.lock().unwrap_or_else(|e| e.into_inner());
    if ids.is_empty() {
        m.remove(dir);
    } else {
        m.insert(dir.to_path_buf(), ids.clone());
    }
}

/// Fichiers de sons et de modèles 3D illisibles à l'ouverture, par dossier de projet.
static KEPT_FILES: std::sync::Mutex<BTreeMap<PathBuf, BTreeSet<PathBuf>>> = std::sync::Mutex::new(BTreeMap::new());

fn keep_files(dir: &Path, files: BTreeSet<PathBuf>) {
    let mut m = KEPT_FILES.lock().unwrap_or_else(|e| e.into_inner());
    if files.is_empty() {
        m.remove(dir);
    } else {
        m.insert(dir.to_path_buf(), files);
    }
}

fn kept_files_in(dir: &Path) -> BTreeSet<PathBuf> {
    KEPT_FILES.lock().unwrap_or_else(|e| e.into_inner()).get(dir).cloned().unwrap_or_default()
}

fn unreadable_in(dir: &Path) -> BTreeSet<CelId> {
    UNREADABLE.lock().unwrap_or_else(|e| e.into_inner()).get(dir).cloned().unwrap_or_default()
}

/// Marques d'image relues du manifeste : triées, après la première image, une seule par image.
fn marks_of(entries: Vec<MarkEntry>) -> Vec<crate::document::ImageMark> {
    let mut marks: Vec<crate::document::ImageMark> = entries
        .into_iter()
        .filter(|m| m.image > 0)
        .map(|m| crate::document::ImageMark {
            id: m.id,
            frame: m.image,
            color: m.couleur,
            action: m.action,
            dialogue: m.dialogue,
            notes: m.notes,
        })
        .collect();
    marks.sort_by_key(|m| m.frame);
    marks.dedup_by_key(|m| m.frame);
    marks
}

fn load_sound(dir: &Path, entry: &SoundEntry) -> Result<Sound, String> {
    let shown = if entry.nom.is_empty() { &entry.fichier } else { &entry.nom };
    // Le nom de fichier vient du manifeste : on refuse tout ce qui sortirait du dossier `sons`.
    let file = Path::new(&entry.fichier);
    if file.components().count() != 1 || file.file_name().is_none() {
        return Err(format!("son « {shown} » : nom de fichier invalide"));
    }
    let bytes = fs::read(dir.join(SOUNDS).join(file)).map_err(|e| format!("son « {shown} » introuvable ({e})"))?;
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let pcm = crate::audio::decode(&bytes, &ext).map_err(|e| format!("son « {shown} » : {e}"))?;
    Ok(Sound {
        id: entry.id,
        name: entry.nom.clone(),
        ext,
        bytes: Arc::new(bytes),
        pcm: Arc::new(pcm),
    })
}

/// Le dessin `id`, avec sa strate Fond s'il en a une (format 23 ; illisible ou d'une autre taille :
/// ignorée, le dessin reste) ; et s'il est affiché en rendu rapide (`quick`, dessin vectoriel
/// texturé).
fn load_drawing(dir: &Path, id: CelId, w: usize, h: usize, quick: bool, hors: bool) -> Result<(Cel, bool, Option<String>), String> {
    let (mut cel, approximate) = load_line(dir, id, w, h, quick)?;
    // Ce que le dessin a hors de la feuille (toile infinie) : un fichier illisible n'empêche pas d'ouvrir
    // le dessin, il perd cette part et le dit.
    let warning = if hors && cel.vector.is_none() { read_hors(dir, id, &mut cel).err() } else { None };
    let read = |path: PathBuf| -> Option<Raster> {
        // (Pas même un `exists` d'avance : presque aucun dessin n'a de strate Fond.)
        read_rgba(&path, |pw, ph, data| (pw == w && ph == h).then(|| Raster::from_rgba(w, h, data))).ok().flatten()
    };
    let (paint, cut) = (read(fond_path(dir, id)), read(coupe_path(dir, id)));
    if paint.is_some() || cut.is_some() {
        let mut s = crate::document::Strate::new(w, h);
        if let Some(r) = paint {
            s.peinture.raster = r;
        }
        if let Some(r) = cut {
            s.coupe = r;
        }
        cel.fond = Some(Box::new(s));
    }
    // Les pixels se rangent dès la lecture (`Tile`, chantier « mémoire d'un épisode ») : un film de 300 plans ne passe pas par
    // 10 Go de tuiles à l'ouverture. La première lecture d'un dessin rouvre ses tuiles.
    cel.geler();
    Ok((cel, approximate, warning))
}

/// Coordonnée la plus lointaine qu'un élément vectoriel garde (une feuille fait 16 384 pixels au
/// plus ; un trait traîné bien hors du dessin reste loin en deçà), et rayon le plus gros.
const MAX_COORD: f32 = 1.0e5;
const MAX_RADIUS: f32 = 5_000.0;

/// Le rang du premier élément du dessin dont les valeurs sont absurdes (non finies, ou hors de ce
/// qu'un geste peut produire), s'il y en a un. Un fichier abîmé qui annonce un trait de 3,4e38 pixels
/// de long bloquait la lecture à le rejouer ; mieux vaut dire que le dessin est illisible (son fichier
/// reste tel quel, comme tout dessin illisible).
fn implausible(d: &VectorDrawing) -> Option<usize> {
    use crate::vector::Element;
    let coord = |v: f32| v.is_finite() && v.abs() <= MAX_COORD;
    d.elements.iter().position(|e| match e {
        Element::Stroke { points, opacity, hardness, .. } => {
            !points.iter().all(|p| coord(p.x) && coord(p.y) && p.r.is_finite() && (0.0..=MAX_RADIUS).contains(&p.r)) || !opacity.is_finite() || !hardness.is_finite()
        }
        Element::Fill { contours, opacity, gradient, .. } => {
            !contours.iter().flatten().all(|p| coord(p[0]) && coord(p[1])) || !opacity.is_finite() || gradient.as_ref().is_some_and(|g| !g.is_sane() || !g.depart.iter().chain(g.arrivee.iter()).all(|v| coord(*v)))
        }
        Element::Text { size, place, opacity, .. } => !(size.is_finite() && *size > 0.0 && *size <= MAX_RADIUS) || !place.iter().all(|v| v.is_finite() && v.abs() <= MAX_COORD * 10.0) || !opacity.is_finite(),
        Element::Image { size, place, opacity, .. } => size[0] == 0 || size[1] == 0 || size[0] > 65_536 || size[1] > 65_536 || !place.iter().all(|v| v.is_finite() && v.abs() <= MAX_COORD * 10.0) || !opacity.is_finite(),
    })
}

fn load_line(dir: &Path, id: CelId, w: usize, h: usize, quick: bool) -> Result<(Cel, bool), String> {
    let vector_path = drawing_path(dir, id, true);
    if vector_path.exists() {
        let text = fs::read_to_string(&vector_path).map_err(|e| format!("dessin {id} illisible : {e}"))?;
        let drawing: VectorDrawing =
            serde_json::from_str(&text).map_err(|e| format!("dessin {id} illisible : {e}"))?;
        if let Some(k) = implausible(&drawing) {
            return Err(format!("dessin {id} illisible : valeurs hors limites dans l'élément {} ; son fichier n'est pas modifié", k + 1));
        }
        let textured = drawing.elements.iter().any(|e| matches!(e, crate::vector::Element::Stroke { brush: Some(_), .. }));
        let approximate = quick && textured;
        let raster = if approximate { drawing.render_quick(w, h) } else { drawing.render(w, h) };
        return Ok((
            Cel {
                raster,
                vector: Some(drawing),
                version: 0,
                fond: None,
            },
            approximate,
        ));
    }
    let png_path = drawing_path(dir, id, false);
    if png_path.exists() {
        let raster = read_rgba(&png_path, |pw, ph, data| (pw == w && ph == h).then(|| Raster::from_rgba(w, h, data)))
            .map_err(|e| format!("dessin {id} illisible : {e}"))?;
        return match raster {
            Some(r) => Ok((Cel::bitmap(r), false)),
            None => Err(format!("dessin {id} : taille différente de celle du projet, ignoré")),
        };
    }
    Err(format!("dessin {id} introuvable dans « {DRAWINGS} »"))
}

thread_local! {
    /// Tampon de décodage, gardé par fil : une feuille décodée pèse 33 Mo, et en allouer (et
    /// remplir de zéros) une à chaque dessin coûtait autant que le décodage lui-même quand
    /// plusieurs fils lisent à la fois.
    static DECODE_BUF: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Lit une image PNG et la donne à `use_it` en octets RGBA (largeur, hauteur, octets). Un PNG RGBA 8
/// bits, comme ceux de SHOT, est décodé dans le tampon du fil ; tout autre PNG passe par
/// `image::open` comme avant.
fn read_rgba<R>(path: &Path, use_it: impl FnOnce(usize, usize, &[u8]) -> R) -> Result<R, String> {
    use image::ImageDecoder;
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if let Ok(decoder) = image::codecs::png::PngDecoder::new(std::io::Cursor::new(&bytes)) {
        if decoder.color_type() == image::ColorType::Rgba8 {
            let (w, h) = decoder.dimensions();
            // L'en-tête d'un fichier abîmé peut annoncer des dimensions démesurées : le tampon
            // demandé (des dizaines de Go) ferait tomber SHOT sans un mot (échec d'allocation).
            // Une feuille ne dépasse pas `MAX_SIDE` de côté (8 192).
            if w as usize > crate::vector::MAX_SIDE * 2 || h as usize > crate::vector::MAX_SIDE * 2 || decoder.total_bytes() > (1 << 30) {
                return Err(format!("image démesurée ({w} × {h})"));
            }
            let len = decoder.total_bytes() as usize;
            return DECODE_BUF.with(|buf| {
                let mut buf = buf.borrow_mut();
                buf.resize(len, 0);
                decoder.read_image(&mut buf[..len]).map_err(|e| e.to_string())?;
                let result = use_it(w as usize, h as usize, &buf[..len]);
                // Une feuille démesurée (image importée) ne reste pas en mémoire.
                if buf.capacity() > 64 << 20 {
                    *buf = Vec::new();
                }
                Ok(result)
            });
        }
    }
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?.into_rgba8();
    Ok(use_it(img.width() as usize, img.height() as usize, img.as_raw()))
}
