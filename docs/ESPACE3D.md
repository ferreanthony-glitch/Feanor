# Espace 3D : les calques et la caméra, rien d'autre

Branche `espace3d-simple` (depuis `integration14` 5f3fda7), 2026-10-21. Décision d'Anthony, le directeur :

> « On va rester simple et focus sur le dessin. Un espace 3D où je peux bouger les dessins dans l'environnement suffit.
> On peut supprimer les objets 3D comme les cubes, cylindres, personnages… ça alourdit le logiciel et l'interface pour pas
> grand-chose. Revenons à quelque chose de simple. »

Et, dans la foulée : le **repère 3D** (un modèle glTF en fil de fer montré par-dessus le dessin) part aussi, ainsi que le bouton « Nouveau calque de fond »\n(un calque bitmap ordinaire dont un seul dessin est tenu sur tout le plan : n'importe quel calque bitmap fait la même chose).

Ce document décrit ce qui reste. L'historique (la conception de l'Espace 3D unifié du 2026-10-20, avec ses objets, son
rasteriseur et son rendu dans l'image) est dans l'historique git (`espace-3d`, `5f3fda7`) et dans `docs/ROADMAP.md`.

Les mots du produit : **Espace 3D** (la vue), **carte** (un calque du plan posé dans l'espace), **caméra** (celle du plan,
une seule).

## 1. Ce qui reste, ce qui part

| Reste | Part (supprimé du code, de l'interface, des tests) |
|---|---|
| La vue **Espace 3D** : les calques du plan comme cartes, la caméra du plan (pyramide, cadre), un sol quadrillé pour l'orientation | Les objets 3D : cube, plan, cylindre, sphère, cône, escalier, mannequin, modèles glTF comme objets |
| Déplacer, tourner (inclinaison), mettre à l'échelle, étirer une carte : poignées dans la vue et champs chiffrés dans Réglages | Matières et couleurs d'objets, lumière, styles de rendu, groupes d'objets, décor de séquence |
| Plusieurs cartes choisies à la fois, aimantation (grille, paliers) | « Objets dans l'image » (rendu des objets sous les calques), « Figer les objets dans un calque de fond » |
| Choisir, déplacer, tourner la caméra du plan ; la regarder ; « Regarder par la caméra » | Le rasteriseur, les maillages, la carte graphique de la vue d'édition (`scene3d*.rs`, `espace3dobjets.rs`) |
| Clés de position des cartes (inclinaison et écart de profondeur compris) et clés de caméra | Le **repère 3D** : l'outil, l'import glTF (Fichier > Importer), le lecteur glTF, la vue en orbite, le fil de fer |
| La parallaxe : les cartes en profondeur défilent selon la caméra, à l'écran, à l'export, dans les vignettes | `Tool::Ref3d`, `model3d.rs`, `view3d.rs`, `ref3dtool.rs` |
| « Ordonner la pile selon la profondeur », « Remettre à plat » | Les commandes `scene3d.ajouter_*`, `importer_modele`, `dupliquer`, `supprimer`, `grouper`, `degrouper`, `envoyer`, `camera_depuis_plan`, `decor`, `outil.repere3d`, `fichier.importer_modele3d` |

Pourquoi : l'Espace 3D unifié du 2026-10-20 avait fait de SHOT un petit logiciel 3D (objets, mannequin, éclairage,
rasteriseur) à côté d'un logiciel de dessin. Ce que le film demande, c'est de bouger les dessins dans un environnement pour
créer de la profondeur et de la parallaxe ; les objets alourdissaient l'interface et le code pour pas grand-chose.

## 2. Un seul monde, deux sortes d'habitants

- **mètres**, Y vers le haut, X vers la droite, Z vers la caméra ;
- **l'origine est le centre de la feuille** ; le plan z = 0 est la feuille (le « plan de référence » de la caméra 2D) ;
- l'échelle est celle de `espace3d::PLATE_WIDTH_M` : le cadre de l'image finale mesure 6 m de large à la distance de
  référence, donc 1 pixel de la feuille vaut `k = 6 / largeur du cadre` mètres ;
- la distance de référence (`camera::ref_distance`, 35 mm voient exactement le cadre) vaut `d0 = 5,83 m`.

Les habitants : les **cartes** (un calque du plan = un rectangle de la taille de la feuille, texturé par son dessin) et la
**caméra** du plan, avec ses clés (dolly, bascule, panoramique, zoom, rotation).

## 3. La carte : un calque est un rectangle posé dans l'espace

### 3.1 Pose d'une carte

Position X/Y/Z, rotation X/Y/Z, échelle uniforme et par axe (`Xf`). La carte tourne autour de son centre — **de son origine**
depuis la partie 182, quand le calque en a une (`Layer::origine`, partie 15 ci-dessous ; sans origine, l'origine est le centre de
la feuille, donc de la carte : comme avant). Rotation : l'ordre
`Ry · Rx · Rz` ; Z est le tour dans le plan de la carte (le « angle » du calque), X et Y sont **l'inclinaison**. Inclinaison
X positive : le haut de la carte vient vers la caméra. Inclinaison Y positive : le bord droit s'éloigne.

### 3.2 Correspondance avec les champs du calque (rien n'est perdu, les anciens projets sont identiques)

Les champs du calque (`LayerTransform` : x, y, échelle, angle, étirements, `tilt_x`, `tilt_y`, `dz` ; `Layer::depth`) restent
la seule source. La pose 3D est *calculée* à partir d'eux (`Monde::card_xf`), et les poignées *écrivent* ces champs
(`Monde::card_from_xf`, `apply_card_xf`). Tout l'existant (outil Transformer, multisélection, clés de position, vue de
dessus, propagation) lit les mêmes champs ; un projet d'avant a inclinaison 0 et écart 0, et son rendu
passe par **le même code** qu'avant.

Avec `d` la profondeur du calque (`depth` + `dz`, au plus −0,9), `s = 1 + d`, `W×H` la feuille en pixels. (Dans un plan
en Espace 3D, depuis le 2026-11-12, `d` descend sous −0,9 : `s` reste alors 0,1 et seule la position Z continue,
`docs/INTERFACE.md`, partie 148.)

| Carte (monde) | depuis le calque |
|---|---|
| position X | `x · s · k` |
| position Y | `−y · s · k` |
| position Z | `−d0 · d` |
| rotation Z | `−angle` (l'angle du calque est horaire à l'écran) |
| rotation X, Y | `tilt_x`, `tilt_y` |
| largeur | `W · k · s · échelle · étirement_x` |
| hauteur | `H · k · s · échelle · étirement_y` |

Dans l'Espace 3D, une carte que l'on recule avec la poignée rapetisse à l'écran (sa taille dans le monde est gardée) ; le champ
« Profondeur » du panneau Calques garde son sens d'avant (la taille vue reste la même). Les deux écrivent les mêmes champs.
Un calque verrouillé, ou de position verrouillée, ne bouge pas.

### 3.3 Clés

Les clés de position (`poskeys.rs`) portent x, y, échelle, angle, étirements, `tilt_x`, `tilt_y` et `dz`, adoucis compris. Tirer
une carte animée à l'image `f` met à jour (ou crée) la clé de l'image `f` (`Layer::set_pose`).

## 4. Une seule caméra

La caméra du plan (`camera::Camera`) *est* la caméra 3D : `Monde::scene_cam` en donne la prise de vue (position de l'œil,
panoramique, bascule, rotation, focale) et `camera_from_scene` fait le chemin inverse. Elle a ses clés (`camkeys.rs`) : la
tirer dans l'Espace 3D règle la caméra de l'image courante, ou la clé de cette image si le plan en a plusieurs
(`App::set_current_camera`), comme l'outil Caméra. « Regarder par la caméra » montre ce que le plan filme, au format du cadre ;
glisser la vue, la tourner ou avancer y déplace alors la caméra elle-même.

## 5. Ce que donne l'image finale

La chaîne de rendu est en deux temps : (1) les cartes se composent sur la feuille, vues par l'**œil** de la caméra (position
seulement : c'est le multiplan) ; (2) la caméra (zoom, rotation, bascule, panoramique) prend cette feuille en homographie.

- **Carte à plat** (inclinaison 0) : exactement le rendu d'avant (`Layer::transform_seen_at`, une affinité). Mêmes pixels.
- **Carte inclinée** : projetée, point par point, de l'œil sur le plan z = 0 (une homographie), posée sur la feuille par un
  maillage (`warp.rs`, 64 × 64 cases). Les effets, l'opacité, le mode de fusion, les clés s'appliquent *avant*, sur l'image de
  la carte à plat.
- **Écran, lecture, vignettes, export** : un seul chemin de composition (`render::panel_plan_at`) ; le processeur est la
  référence et la carte graphique la reproduit à ≤ 2 niveaux près.

**Ordre** : les cartes se composent dans l'ordre de la pile des calques, pas par distance (modes de fusion, calques
d'ajustement et dossiers travaillent sur « ce qui est dessous »). « Ordonner la pile selon la profondeur » met les calques du
plus loin au plus près (une étape d'annulation) quand on veut que les deux coïncident.

Limites connues : une carte inclinée est coupée aux bords de la feuille ; une partie de carte derrière l'œil n'est pas dessinée ;
le panoramique / la bascule restent à plus de 5° de l'horizon (`Camera::limited`) ; l'aperçu réduit des effets n'est pas utilisé
pour une carte inclinée.

## 6. Format et anciens projets (rien ne casse, rien n'est effacé)

Aucun changement de format (`project::VERSION` reste 35) : les objets et le repère disparaissent par *oubli à la lecture*.

- **Calque « Fond 3D »** d'un ancien projet (le rendu des objets figé par la scène de blocage) : un calque bitmap ordinaire,
  verrouillé, tout en bas. Il garde ses pixels ; il ne se met plus à jour tout seul (plus de scène derrière).
- **`scenes3d.json`** (objets, décors, caméra de blocage) : plus lu, plus écrit, **jamais effacé ni réécrit** — l'enregistrement
  (classique, en arrière-plan, projets partagés) le laisse tel quel, même abîmé ; plus d'avertissement non plus. Il est dans
  le dossier du projet si une version plus ancienne doit le relire.
- **Repère 3D** : le champ `repere3d` des plans et la liste `modeles` du manifeste sont ignorés à la lecture et ne sont plus
  écrits ; les fichiers `modeles/<id>.pelure3d` restent dans le dossier du projet (l'enregistrement ne nettoie plus
  `modeles/`). Un projet enregistré par cette version ne dit donc plus à une version plus ancienne qu'il avait un repère 3D.
- **Projets partagés** : un fichier de plan qui porte une `scene3d` l'ignore à la lecture (le champ n'est pas récrit quand le
  plan est ré-enregistré par cette version) ; `scenes3d.json` du dossier partagé reste en place.
- Anciens identifiants de commandes : `vue.3d`, `scene3d.basculer`, `vue3d.recadrer`, `scene3d.cadrer`, `outil3d.*`,
  `scene3d.outil_*`, `scene3d.vue_camera`, `scene3d.aimanter`, `scene3d.grille` restent des doubles (`keymap::ALIASES`) des
  commandes `espace3d.*` : un raccourci posé dessus marche toujours. Ceux des objets et du repère n'existent plus (un raccourci
  posé dessus ne fait rien).

## 7. Interface

- Un seul bouton « Espace 3D » (icône cube) dans la barre sous le dessin, une seule entrée du menu Affichage, une seule
  commande `espace3d.basculer`.
- Depuis le 2026-11-16, un second bouton à côté du cube, « Espace 3D à côté » (`espace3d.a_cote`) : la vue 3D à droite de
  la feuille, pendant qu'on dessine (partie 13).
- **Une seule rangée d'outils** (barre au-dessus de la vue) : Choisir · Déplacer · Tourner · Mettre à l'échelle · Caméra ·
  Recadrer · Regarder par la caméra. Chacun : icône, bulle d'une seconde (nom, raccourci, description) par `bulles.rs` ; l'outil
  actif en `bouton_etat`.
- **Gestes** : clic = choisir une carte ou la caméra (Maj / Ctrl : ajouter) ; glisser une carte la déplace dans son plan (Maj :
  en profondeur) ; poignées (déplacer, tourner, échelle ; axes du monde ou de la carte ; aimantation) pour tout ce qui est
  choisi ; clic droit ou Alt + glisser tourne la vue, bouton du milieu la glisse, molette avance.
- **Une étape d'annulation par geste**, au nom français : « Déplacer dans l'espace 3D », « Tourner dans l'espace 3D », « Mettre
  à l'échelle dans l'espace 3D », « Déplacer la caméra », « Ordonner la pile selon la profondeur »…
- **Réglages** (panneau Réglages tant que l'Espace 3D est ouvert) : manipulateur dans le repère de la carte, aimantation et
  pas, grille ; la liste des cartes (calques) ; pour une carte : position X/Y/Z (m), rotation X/Y/Z (°), échelle (%), étirement
  X/Y (%), profondeur multiplan, « Remettre à plat » ; pour la caméra : position, panoramique, bascule, rotation, focale.
- Barre d'état : « Espace 3D : clic sur une carte (un calque) ou sur la caméra pour la choisir, glisser pour la déplacer… ».

## 8. Performance (règle « sans latence »)

- Tourner la vue, tirer une carte : rien ne passe par le processeur lourd. Les cartes sont des maillages egui de 16 × 16 cases,
  leurs textures (≤ 1024 px) sont refaites quand le dessin change seulement ; le quadrillage est tracé par egui (une quarantaine
  de segments). Plus de rendu d'objets, plus de fil de fond, plus de carte graphique dédiée à la vue.
- 10 cartes : ≈ 1,6 ms par image d'interface, carte graphique (essai de budget `the_view_with_ten_cards_stays_within_the_frame_budget`) ;
  choisir à la souris < 0,01 ms ; plan d'une image avec 10 cartes inclinées ≈ 1,3 ms.
- Le maillage d'une carte inclinée est refait à la construction du plan d'une image (≈ 0,15 à 0,25 ms par carte).
- Aucun repaint au repos (essai `the_space_toolbar_is_one_row_of_tools_and_the_application_rests_with_it_open`).
- Taille de `shot.exe` (release) : voir `docs/PERFORMANCE.md` (« Espace 3D simplifié »).

## 9. Essais

`espace3d_tests.rs` (cartes, caméra, rendu, parallaxe, clés, format, interface, anciens projets) et `espace3dmath_tests.rs`
(maths, rayons, poignées, aimantation, caméra d'orbite, pyramide). Le script `scripts/verifier.ps1` les nomme dans la famille
« Espace 3D ».

- **Pose d'une carte** : aller-retour monde ↔ champs du calque ; reculer une carte garde sa taille et sa position dans le monde ;
  échelle sur un axe = étirement, uniforme = échelle ; la carte est le calque du multiplan vu de l'œil (à 0,05 px près).
- **Caméra** : la caméra du plan ↔ la prise de vue de l'espace dans les deux sens.
- **Rendu** : carte à plat = plan d'avant ; carte inclinée = projection de la géométrie ; opacité et fusion gardées ; carte
  derrière l'œil non dessinée ; vignette = composition ; processeur contre carte graphique ≤ 2 niveaux ; l'export compose la
  même image que l'écran (cartes inclinées, caméra tournée).
- **Parallaxe** : une carte plus loin bouge moins (rapport 6 à ±0,6, à plat et incliné).
- **Ordre** : la pile l'emporte sur la distance ; « Ordonner la pile » (une étape, refusé avec des dossiers).
- **Anciens projets** : un projet avec objets, décor, repère 3D, modèle et calque « Fond 3D » s'ouvre sans avertissement avec les
  mêmes pixels ; `scenes3d.json` (même abîmé) et le modèle sont intacts après l'enregistrement classique et en arrière-plan ;
  le manifeste n'écrit plus `repere3d` ni `modeles`.
- **Interface** : plus aucune commande d'objet ni de repère (actions, palette, keymap) ; anciens noms en doubles ; choisir au
  rayon et à la souris ; déplacer, incliner, mettre à l'échelle par les poignées, une étape d'annulation par geste ; calque
  verrouillé immobile ; caméra du plan déplacée (clé de l'image si animée) ; pinceau sur une carte inclinée (aller-retour) ;
  l'application finit par se reposer, Espace 3D ouvert.

## 10. Liste de contrôle à l'écran (rien n'a été regardé : consigne de ne pas lancer SHOT)

À voir une fois, dans les trois thèmes :

1. Barre du bas : un seul bouton cube ; l'ouvrir : la rangée d'outils (Choisir, Déplacer, Tourner, Échelle, Caméra | Recadrer,
   Regarder par la caméra) avec des icônes lisibles et des bulles à 1 s ; l'outil actif est en couleur d'accent.
2. Cartes : chaque calque est un rectangle texturé, contour d'accent quand il est choisi, nom et profondeur en légende ; clic
   sur un calque du panneau Calques = carte choisie ; clic sur une carte = calque courant.
3. Poignées : flèches, carrés de plan, cercles, cubes d'échelle ; couleurs rouge / vert / bleu ; survol en jaune.
4. Caméra : pyramide, cadre, triangle du haut de l'image ; « Caméra » la choisit ; « Regarder par la caméra » montre le cadre.
5. Réglages : liste des cartes, champs de position / rotation / échelle / étirement / profondeur, « Remettre à plat », champs de la
   caméra ; aucune mention d'objet, de décor, de lumière.
6. Barre d'état : le message d'aide ne parle que de cartes et de caméra.
7. Menu Fichier > Importer : plus de « modèle 3D (glTF) comme repère » ; barre d'outils de dessin : plus d'outil « Repère 3D ».
8. Ouvrir une copie d'un ancien projet qui avait des objets et un calque « Fond 3D » : le calque est là, mêmes pixels, verrouillé ;
   l'enregistrer ; `scenes3d.json` est toujours dans le dossier.

## 11. Ce qui reste à faire (et ne le sera pas, sauf demande)

1. Objet devant une carte, cartes qui se traversent, tri des cartes par profondeur comme réglage de plan : sans objet, plus
   d'objet ; le tri reste incompatible avec les modes de fusion et les calques d'ajustement.
2. Cartes inclinées plus grandes que la feuille (toile infinie) : coupées aux bords de la feuille.
3. Vue d'édition : les effets, modes de fusion et opacités des calques n'y sont pas montrés (le dessin brut).
4. Outils 2D sur une carte inclinée : le pinceau suit l'inclinaison, pas la boîte de sélection, le Transformer ni la
   multisélection .
5. Inclinaison limitée à ±85° ; la pose est donnée en angles d'Euler (ordre Y · X · Z), la poignée de rotation peut sembler
   sauter près de ±90°.
6. Dossiers : un dossier n'est pas une carte ; sa pose ne place pas ses calques dans l'espace.

## 12. Plan en Espace 3D (2026-11-12, branche `espace-libre`)

Tout ce qui précède décrit un plan ordinaire, et reste vrai pour lui. Un plan coché « Plan en Espace 3D » (`Panel::espace`,
format 54) lève plusieurs de ces limites : sa caméra est libre (plus de `Camera::limited`, bascule jusqu'à 90°), ses cartes
sont posées dans le repère de la caméra (`Monde::repere_camera`, `espace3d::place_dans`) et ordonnées par distance
(`espace3d::ordre_par_distance`), leur inclinaison va jusqu'au demi-tour, et on y pose des surfaces face à la caméra, au
sol ou en mur. Les points 11.1 (tri par profondeur) et 11.5 (inclinaison limitée à ±85°) ne valent plus pour ces plans ;
11.2 (carte coupée aux bords de la feuille), 11.3, 11.4 et 11.6 valent toujours. Conception, essais et limites :
`docs/ESPACE-LIBRE.md` ; interface et liste de contrôle : `docs/INTERFACE.md`, partie 143.

`SceneCam::view` tire maintenant la droite de la caméra de son panoramique (mêmes axes qu'avant sous 90° de bascule) et
`espace3d::MAX_TILT` vaut 90° : la caméra d'un plan ordinaire reste bridée par `Camera::limited`.

## 13. Espace 3D à côté du dessin (2026-11-16, branche `espace-a-cote`, depuis `espace-libre`)

Demandé par Anthony : garder la vue 3D ouverte à côté de la feuille pendant qu'on dessine. Plan validé par lui le
2026-11-16 (partage de la zone du dessin plutôt qu'un panneau ; un geste va à la vue où il commence ; Réglages de la
dernière vue cliquée ; branche partie d'`espace-libre`, puis remise sur `main` e1f18b2 le même jour). Code : `espace3dcote.rs` ; essais : `espace3dcote_tests.rs` ;
interface et liste de contrôle : `docs/INTERFACE.md`, partie 187 (numérotée 163 sur sa branche). **Rien n'a été regardé à l'écran.**

- **Le partage** (`espace3dcote::partage`) : la feuille à gauche, une barre de 6 points à tirer, la vue 3D à droite
  (20 à 80 % de la largeur, 50 % par défaut, `Layout::part_espace3d`, retenue d'une séance et d'un espace de travail à
  l'autre). Chaque vue garde au moins 160 points ; plus étroit, la feuille garde toute la zone. Pas dans les vignettes.
- **L'ordre dans l'image d'écran** : la feuille d'abord (`canvas_feuille`, l'ancien corps de `canvas_panel`), la vue 3D
  ensuite : ce qu'un trait change à cette image est déjà dans les cartes.
- **Le plein écran d'avant** (`espace3d.basculer`, `espace3d_plein()`) est inchangé et l'emporte tant qu'il est ouvert ;
  le refermer rend le côte à côte (`a_cote` reste vrai). Les outils (`espace3d.outil_*`, Recadrer, Regarder par la
  caméra) règlent la vue d'à côté sans passer en plein écran (`montrer_espace3d`). Le côte à côte n'arrête ni le trait ni
  la lecture.
- **`espace3d_fit`** cadre dans la zone de la vue (`Espace3dState::rect`, posée par `espace3d_panel`), plus dans
  `canvas_rect`, qui reste la feuille.
- **Gestes** : chaque vue a sa propre zone egui ; `canvas_input` ne prend que les appuis sur la feuille ; un geste
  commencé dans une vue y reste jusqu'au relâché. `clic_3d` (dernier appui dans la vue 3D) décidait du panneau Réglages ; depuis la remise sur `main` (e1f18b2), Réglages
  montre toujours l'outil (partie 150) et les réglages de la vue d'à côté sont dans l'onglet « Espace 3D » (`espace3dpanneau.rs`),
  qui ne passe pas devant quand on ouvre le côte à côte.

**Sans latence.** Trois mécanismes :

1. *L'image d'une carte par morceaux* (`changement`, `render::cel_image_region`). On garde les tuiles du dessin au moment
   où l'image a été faite (copies partagées, `Cliche`) ; une écriture copie la tuile, donc une tuile qui n'est plus la
   même (`Tile::same_as`) a changé. On ne refait que le morceau d'image qu'elle touche, avec un texel de marge pour les
   4 échantillons bilinéaires, et on l'envoie seul (`TextureHandle::set_partial`). Un changement hors de la feuille, de
   taille, ou de plus de la moitié de l'image refait tout. Les clichés ne sont gardés que pour les cartes tracées à la
   dernière image (`e3_lacher_les_cliches`) : sinon ils retiendraient de vieilles tuiles et empêcheraient les dessins
   qu'on ne regarde plus de se ranger (`tuilepack.rs`). Vaut aussi pour le plein écran.
2. *Un budget par image d'écran* : au plus 600 000 texels de cartes (une carte de 1024 × 576), toujours au moins une
   carte. Une carte qui attend montre son dessin d'avant (`vu_par_calque`) et demande une image de plus ; la vue n'est
   alors pas gardée.
3. *Le rythme* (`VueGardee`) : pendant un geste sur la feuille, la vue 3D se refait au plus 30 fois par seconde ; entre
   deux, elle repeint ce qu'elle a tracé (les formes d'egui de sa couche, recopiées). En lecture, elle se refait quand
   l'image du film change. Au repos, rien ne se redessine.

**Mesures sur le Mac** (Apple Silicon, 2026-11-16, essai `a_stroke_with_the_side_view_open_stays_within_the_frame_budget`,
feuille 1920 × 1080, 10 calques, trait de 120 images à 240 images par seconde, carte graphique) : vue fermée 2,07 ms par
image en médiane, ouverte 2,25 ms (interface 0,53 → 0,51 ms, découpage 0,15 → 0,21 ms, carte graphique 1,84 → 2,11 ms).
Remesuré après la remise sur `main` (e1f18b2, interface des parties 145 à 161, plus lourde dans les deux cas) : fermée
5,6 à 5,75 ms en médiane, ouverte 5,9 à 6,3 ms (trois passages, essai lancé seul ; lancé en même temps que d'autres essais,
il peut dépasser son seuil de 8 ms).
L'image d'une carte 1920 × 1080 refaite en entier : 3,2 ms ; le morceau d'un bout de trait : 0,11 ms. À refaire sur le
poste Windows, la référence : `cargo test --lib side_view_open_stays -- --nocapture`, et noter ici les lignes `PERF`.

**Essais** (`espace3dcote_tests.rs`) : image refaite par morceaux = image entière, pixel pour pixel (quatre tailles de
feuille, bords de tuiles et de feuille) ; hors feuille ou plus de la moitié : tout ; partage et bornes ; ouvrir, fermer,
agrandir, revenir à côté ; les outils restent à côté ; la part retenue et bornée (réglages d'avant sans le champ) ; un
appui dans la vue 3D choisit la carte et son calque sans dessiner, un trait commencé sur la feuille qui file dans la vue
dessine et ne choisit rien ; un trait d'un quart de seconde refait la vue 4 à 10 fois, chaque fois avec le trait du
moment ; la lecture continue et la carte suit les images ; 8 dessins neufs d'un coup se répartissent sur plusieurs images
sans qu'aucun ne se perde ; l'application se repose vue ouverte ; la mesure ci-dessus (seuil 8 ms par image).

**Limites.**

1. Le partage est seulement gauche / droite ; une seule vue 3D (pas de vues de dessus et de côté en plus, comme Storyboard
   Pro en montre : ce n'est donc pas une exclusivité).
2. Pendant un trait, la carte suit avec au plus 1/30 s de retard ; un pot de peinture ou une gomme sur toute la feuille
   refait la carte entière (3 ms environ pour une feuille 1920 × 1080) à cette image-là.
3. La vue 3D montre les dessins bruts (sans effets, modes de fusion), comme en plein écran (point 11.3).
4. Le côte à côte n'est pas retenu d'une séance à l'autre (seule sa largeur l'est).

## 14. Le décor 3D d'une séquence (2026-11-19, branche `decor-3d`) : ce qui revient, et pourquoi

Le 2026-10-21, Anthony avait choisi de rester simple (partie 1 : plus d'objets 3D, plus de repère, plus de rasteriseur). Le
2026-11-19, il choisit d'aller **vers un moteur de prévisualisation** : décors, puis personnages, filmés par les caméras des plans.
L'étude se fait par étapes ; **l'étape 1** est le décor d'une séquence (`docs/INTERFACE.md`, partie 180 ; code `decor3d*.rs`).

Ce qui revient du code d'avant (le sien, repris) : le lecteur glTF de `model3d.rs` (étendu : normales, coordonnées de texture,
couleur de base et texture, plafonds de sûreté), le rasteriseur par bandes de `scene3drender.rs` (étendu : texture juste en
perspective, facettes, lumière hémisphère, ciel, brume ; il ne sert plus qu'à l'export sans carte graphique et à l'affichage de
secours), la passe wgpu de `scene3dgpu.rs` (refaite : maillages texturés, ombres douces, une prise par vue), et l'idée
d'`espace3dobjets.rs` (l'image des objets, élément le plus bas du plan, vue de l'œil de la caméra et alignée sur la feuille).
Ce qui ne revient pas : les objets simples (cube, mannequin…), les styles de rendu, le « Figer dans un calque de fond ».

Ce qui est neuf : le décor est **dessiné par la carte graphique à l'écran** (dans la composition de l'image du plan, et dans la
vue), jamais refait par le processeur quand la caméra bouge ; un **budget mesuré sur la machine** (`decor3dbudget.rs`) décide
ce qu'un décor peut peser sans ralentir, et un décor trop lourd est allégé ou simplifié (la copie du projet seulement), jamais
accepté pour ralentir FEANOR. V1 : le décor est **derrière toutes les cartes** ; la profondeur partagée décor / cartes (faite :
partie 16 ci-dessous), les ombres portées par les cartes, puis les personnages (squelettes, animations : l'étape 2) viendront
ensuite.

**Partie 181 (2026-11-19, branche `banque-3d`)** : le décor d'une séquence est une **liste d'objets** (`Objet3d` : un modèle, une
place, une **origine** — le point que la position place, autour duquel il tourne et change d'échelle, d'office le centre du dessous
de sa boîte — et « Aimanté au sol », `decor3daimant.rs` : la hauteur calculée, posée sur le sol ou sur les objets sous lui, sans
pénétration). Importer un fichier ou prendre un modèle de la **banque 3D** (45 modèles CC0 de Kenney, rangés dans le programme,
onglet « 3D » de la Bibliothèque) ajoute un objet, par le même chemin (budget de la machine pour **la somme** des objets, fenêtre
« Ce décor est lourd », ou refus). Les objets d'un même contenu partagent un modèle : un fichier `decors/<empreinte>.glb`, une
lecture, un envoi à la carte. Tous les objets sont dans le même tampon de profondeur (justes entre eux) ; derrière les cartes
jusqu'à la partie 183, qui partage cette profondeur avec elles (partie 16 ci-dessous). `decors3d.json` passe en version 2 ; un fichier de la partie 180 se lit comme un objet, à la même place. L'origine
modifiable et l'aimant viendront aussi aux calques dessinés (chantier suivant, avec un repère d'origine dans la vue partagé par les
deux) ; `docs/INTERFACE.md`, partie 181.

## 15. L'origine des cartes et l'aimant au sol (2026-11-19, branche `origine-calques`, partie 182 de `docs/INTERFACE.md`)

**L'origine** d'un calque (`Layer::origine`, en pixels de son dessin, `origine.rs`) est le point de la carte qui porte sa place.
Avec `e` l'écart de l'origine au centre de la feuille (`Monde::ecart`), les champs du calque donnent (`Monde::card_xf_o`) :

| Carte (monde) | depuis le calque, avec une origine |
|---|---|
| place de l'origine | `((e.x + x) · s · k, −(e.y + y) · s · k, −d0 · d)` |
| rotation, taille | comme au point 3.2, autour de l'origine |
| centre de la carte (`Xf::pos`) | la place de l'origine moins `R · (taille ⊙ e / (W, H))` |

La profondeur `d` est donc celle de **l'origine** (pour une carte à plat, c'est aussi celle de son centre). Sans origine, `e` est
nul et `card_xf_o` est exactement `card_xf`, au bit près ; `card_from_xf_libre_o` en est l'inverse exacte (un essai fait l'aller
et retour sur des cartes inclinées). Une carte à plat posée par l'affinité de la feuille (`Layer::transform_seen_at`) a la même
image : la pose y est rapportée au centre de la feuille (`LayerTransform::au_centre`, x' = x + e − A·e).

- **Déplacer l'origine ne déplace pas la carte** (`origine::deplacer_origine`) : à plat, la pose est recalculée sur la feuille
  (xy2 = xy1 + A·Δ − Δ) ; **inclinée**, son placement 3D est recalculé (`card_xf_o` avec l'ancienne origine, puis
  `card_from_xf_libre_o` avec la nouvelle) : la profondeur, celle de l'origine, peut changer, et l'échelle avec elle (la taille
  dans le monde est gardée) ; les quatre coins de la carte restent au même endroit de l'espace, à la pose fixe et à chaque clé.
- **Le manipulateur** (`App::e3_pivot`) : une carte seule tourne et grandit autour de son origine ; plusieurs, autour de la
  moyenne de leurs origines (la caméra comptant pour sa place) ; aucune carte choisie n'a d'origine : le centre de la boîte des
  positions, comme avant. Étirer une carte sur un de ses axes garde son origine en place. Le champ « Position » du panneau
  (Précis) est la place de l'origine ; la rotation saisie, et « Comment il est posé », tournent la carte autour d'elle. La carte
  choisie montre son origine, un petit cercle à croix.
- **La trajectoire dans l'espace** (`trajectoire3d::cles_dans_l_espace`) : avec l'ancrage `Origine`, l'origine de la carte est posée
  sur le chemin à chaque image ou jalon (au lieu du déplacement relatif d'avant, gardé pour `Depart`). La carte n'y tourne pas avec
  le chemin (vue par la tranche, elle disparaîtrait). Une trajectoire neuve tracée dans la vue part avec l'ancrage `Origine`.
- **Aimanté au sol** (`Layer::aimante_au_sol`, `origine::aimanter_au_sol`) : l'origine de la carte repose sur le sol, à sa pose
  fixe et à chacune de ses clés : seul `y` change, `y = −sol / (s · k) − e.y`, à la profondeur de la pose. Le sol se lit par **une
  seule fonction**, `origine::hauteur_du_sol_sous(monde, decor, x, z, ici)` : le plan `Monde::sol_y`, ou le dessus d'un objet du
  décor de la séquence (`decor3daimant::hauteur_sous` : la verticale de l'origine traverse les triangles des objets ; un niveau
  dedans un objet est écarté ; le plus proche de la hauteur où est l'origine l'emporte). Toute retouche du décor repose les
  cartes aimantées de la séquence (`App::reposer_les_cartes_aimantees`). L'aimant est refait **après chaque modification** (manipulateur, champs, réglette de
  distance, taille, clé posée, trajectoire recalculée, origine déplacée, profondeur dans le panneau Calques ou la vue de dessus),
  jamais à la lecture d'un fichier ni pendant la lecture du film. Coché, la hauteur (Y) est grisée dans le panneau. Décoché,
  rien ne bouge : la carte reste où l'aimant l'a mise, maintenant et après enregistrement. Un calque accroché à la caméra ne se
  pose pas au sol.
- **Aux pieds d'une carte aimantée** sur un objet du décor, rien n'est caché depuis la profondeur partagée (partie 16) : la
  tolérance (0,4 % de la distance plus 1 cm) couvre le millimètre de l'aimant ; ce qui du dessin descend sous l'origine, dans
  l'objet, est caché par lui.

## 16. La profondeur partagée entre le décor et les cartes (2026-11-19, branche `profondeur-partagee`, partie 183 de `docs/INTERFACE.md`)

Le décor n'est plus toujours derrière : **une carte et un objet du décor se cachent selon leur vraie distance** à l'œil de la caméra
du plan. Un personnage passe derrière un arbre et devant un banc. Code : `profondeur.rs`.

- **Le décor garde sa profondeur.** Il est rendu une fois par image, comme avant, et reste l'élément le plus bas de l'image du plan ;
  son rendu garde, à chaque pixel de la feuille, l'inverse de la distance à l'œil (le long de son axe) de ce qu'il y montre, 0 là
  où il n'y a rien (le ciel ne cache rien). Carte graphique : une seconde cible de la passe de l'image du plan (`decor3dgpu::PROF`,
  `R16Float`, à 0,05 % près), une texture de plus à côté de celle du décor (`gpu.rs`, `CelTexture::prof`), dessinée en même temps qu'elle, donc
  seulement quand la clé du décor change. Processeur et exports : `decor3drendu::rendre_objets_prof` (le plus proche des
  échantillons qui couvrent la moitié du pixel), ou relue de la carte graphique (`rendre_hors_ecran_prof`) ; gardée avec le rendu
  (`decor3d::profondeur_du_rendu`, par le numéro du rendu).
- **Le plan de chaque carte.** L'élément d'une carte porte le plan où elle est posée, vu du même œil que le décor
  (`profondeur::plan_du_calque`, comme `espace3d::place_dans` : l'œil de la caméra, ou l'œil de référence et le changement de
  repère d'un plan en Espace 3D) : trois nombres (a, b, c) tels que l'inverse de la distance du point de la carte vu au pixel
  (x, y) de la feuille soit a·x + b·y + c. Pour un plan, l'inverse de la profondeur est linéaire sur l'image : une carte inclinée
  est juste en chaque pixel, sans maillage de plus.
- **À la composition**, un pixel de carte est caché si le décor y est plus près qu'elle d'au moins 0,4 % de sa distance plus 1 cm
  (`profondeur::cache`) ; le décor, dessous, se voit alors. Une lecture de plus par pixel de carte (`fs_compose_p`, `fs_blend_p` ;
  `render::composite_tile_into`, `composite_world_tile`, `pixel_of`). Entre elles, les cartes gardent l'ordre de toujours.
- **Jamais cachés** (`profondeur::partage`) : dossiers, calques d'effet (ajustements, incrustations de texte), calques d'annotation,
  calques accrochés à la caméra, et tout calque marqué **« Toujours devant le décor 3D »** (`Layer::devant_decor`, écrit
  `devant_decor_3d` sous le format 61 sans changer `VERSION`) ; ni la couleur unie d'un calque, ni les pelures d'oignon, ni les
  silhouettes de la trajectoire.
- **La vue Espace 3D** : ses cartes sont dessinées par egui, par-dessus le décor ; juste après chaque carte qui partage la
  profondeur, un **voile** (`decor3dgpu::VoileCarte`) remet sur son quadrilatère le décor qui est plus près qu'elle, lu dans les
  cibles de la passe du décor de cette vue (couleur lissée, tampon de profondeur à quatre échantillons, gardé pour cela).
- **Sans décor**, aucun élément ne porte de plan : la composition est celle d'avant, par les mêmes pipelines et les mêmes
  boucles.


## 17. Les poignées sur les objets du décor, et l'origine dans la vue (2026-11-19, branche `poignees-3d`, partie 184 de `docs/INTERFACE.md`)

Le titre de ce document (« les calques et la caméra, rien d'autre ») vaut toujours pour ce qui se dessine : le décor 3D reste
celui de la séquence (parties 14 à 16). Mais **ses objets se choisissent et se manient dans la vue** comme les cartes. Code :
`poignees3d.rs`.

- **Un troisième habitant de la sélection** : `Espace3dState::sel` tient des cartes (`espace3d::card_selection`, marque `1 << 62`),
  des objets du décor (`poignees3d::objet_selection`, marque `1 << 61`, l'`Objet3d::id`) et la caméra (`CAMERA_ID`) ; ils se
  mêlent (Maj+clic). L'objet choisi dans la vue l'est dans la liste du panneau (`decor3d.choisi`), et l'inverse.
- **Le choix** : le plus proche de l'œil l'emporte, entre la carte dont le dessin est touché et l'objet dont un triangle l'est
  (`App::e3_pick_tout`, `poignees3d::toucher_objet` : le rayon dans le repère du modèle, sa boîte puis ses triangles ; la boîte
  seule au-delà de 2 millions de triangles) ; une carte « Toujours devant le décor 3D » passe devant. L'outil Trajectoire et le
  dessin dans la vue ne prennent toujours que des cartes (`e3_pick`).
- **Un objet vu comme une carte** par le manipulateur : `poignees3d::xf_de_l_objet` (la place de l'origine, `[0, rotation_y, 0]`,
  l'échelle sur les trois axes), dont la matrice est `Place3d::matrice`. Le pivot est l'origine de l'objet (`e3_pivot`). Un objet ne
  tourne qu'autour de la verticale et grandit d'un bloc : les anneaux X et Z sont refusés, l'échelle est uniforme ; aimanté, sa
  flèche Y est refusée (`App::e3_poignee_refusee`, poignées grises, la barre d'état dit pourquoi).
- **Le geste** passe par le cœur de la retouche du panneau (`decor3dui::decor3d_appliquer` : l'aimant des objets, puis les cartes
  aimantées de la séquence) ; l'instantané du projet pris au début du geste en fait **une étape d'annulation** (`commit_named`).
- **« Modifier l'origine »** (`Espace3dState::origine`, `espace3d.modifier_origine`) : le manipulateur se pose sur l'origine de ce
  qui est choisi seul et la déplace seule. Une carte : dans son plan, par `origine::deplacer_origine` (le point du monde ramené au
  carré unité de la carte, puis aux pixels du dessin) ; un objet : dans l'espace, par `Objet3d::changer_origine`. Maj accroche aux
  points de la boîte (`origine::aimanter_a_la_boite` ; `poignees3d::points_de_la_boite`). Au lâcher, l'aimant repose par la
  nouvelle origine.
- Rien ne change dans l'image du plan, les exports ni le fichier.


## 18. Les ombres des cartes et du décor (2026-11-19, branche `ombres-cartes`, partie 185 de `docs/INTERFACE.md`)

Sous le même soleil que le décor : **une carte porte l'ombre de son encre** sur le décor et sur les autres cartes, et **le décor
ombre les cartes**. Code : `ombres.rs`.

- **Le décor reste gardé** (`DecorCle`) ; les cartes n'y entrent pas, tout se fait à la composition de l'image du plan. Quand le décor
  a ses ombres, sa passe garde à chaque pixel **la part du soleil** qu'il y a mise et le mélange de l'hémisphère (une troisième cible,
  `decor3dgpu::SOLEIL`) : l'ombre d'une carte retire ce soleil-là exactement, brume comprise (`Lumiere::assombrir_decor`).
- **La carte d'ombres des cartes** : les cartes qui portent une ombre, vues du soleil avec la matrice de la carte d'ombres du décor
  (`decor3dgpu::eclairage`), relevées sur leur plan (`PlanItem::ombre`, le plan de la partie 16, aussi pour une carte « Toujours
  devant le décor 3D »), là où leur dessin est au moins à moitié opaque.
- **Le décor** (`fs_decor_o`) : son point du monde vient de sa profondeur (partie 16) ; comparé à la carte d'ombres des cartes avec
  le filtre du décor. **Une carte** (`fs_compose_p`, `fs_blend_p`) : son point du monde vient de son plan ; comparée à la carte
  d'ombres du décor et à celle des cartes, chaque lecture à la profondeur de son propre plan (`Lumiere::gradient` : jamais dans sa
  propre ombre) ; assombrie de ce que perd une face du décor tournée vers le soleil.
- **Ni porter ni recevoir** (`ombres::concerne`, et les cases `Layer::porte_ombre`, `Layer::recoit_ombres`, écrites
  `porte_ombre`, `recoit_ombres` quand elles sont décochées, sous le format 61) : dossiers, calques d'effet, d'annotation, accrochés
  à la caméra ; ni la couleur unie d'un calque, ni les pelures d'oignon.
- **À l'export** avec une carte graphique, la même chose par le processeur (`ombres::Contexte`) avec ce que le rendu du décor a relu
  (`decor3dgpu::SortieOmbres`) ; sans carte graphique, ni ombres du décor ni ombres de cartes. La vue Espace 3D les montre aussi
  depuis la partie 189 (partie 19 ci-dessous).
- **Sans décor, ou sans ses ombres**, aucun élément ne porte d'ombre : la composition d'avant, par les mêmes chemins.


## 19. Les ombres des cartes dans la vue Espace 3D (2026-11-19, branche `ombres-espace3d`, partie 189 de `docs/INTERFACE.md`)

La vue Espace 3D (plein écran, à côté, vues multiples) montre les ombres de la partie 18 : **une carte porte l'ombre de son encre**
sur le décor de la vue, **le décor et les autres cartes ombrent une carte**, sous le même soleil, avec les mêmes cases et les mêmes
exclusions ; justes **de tous les côtés**, puisque les ombres du soleil sont dans le monde. Code : `ombres3d.rs`, et le moteur du décor
(`decor3dgpu.rs`).

- **La carte d'ombres des cartes de la vue** : les cartes telles que la vue les pose (leurs quatre coins dans le monde, leur dessin :
  la texture d'egui), vues du soleil avec la matrice de la carte d'ombres du décor (`eclairage`, qui ne dépend pas du point de vue).
  Une pour toutes les vues, refaite seulement quand une carte, son dessin ou le soleil change (`CartesVue::cle`) ; avec une carte
  grossière (une profondeur par case de 8 × 8 texels) qui épargne au décor son filtre là où aucune carte ne peut l'ombrer.
- **Le décor de la vue** (sa passe à quatre échantillons) : son soleil multiplié par ce que les cartes lui en laissent, au point du
  monde du fragment, avec le filtre du décor (`soleil_des_cartes_sur_le_decor`) : le calcul de l'image du plan.
- **Une carte de la vue** : ses cartes étant dessinées par egui, un rappel de la carte graphique (`RecueCarte`) repasse juste après
  elle sur le même maillage et **retranche de l'écran** ce que l'ombre ôte à sa couleur (`soleil_vu_par_la_carte`, `assombrir_carte`,
  la pente de la carte : jamais dans sa propre ombre), avant son voile (partie 16).
- **Un seul calcul** : le texte des ombres de la composition (`gpu::LUMIERE`, `gpu::OMBRES_COMMUN`) est inclus tel quel par les
  shaders de la vue.
- **Sans les ombres du décor**, ou aucune carte qui porte ou reçoit : rien n'est préparé, la vue d'avant au pixel près. Coût mesuré
  (huit personnages, 180 000 triangles, vue de 1533 × 559 pixels) : +0,2 ms, +0,4 ms quand les cartes bougent (`docs/INTERFACE.md`,
  189.3).


## 20. Copier les objets du décor (2026-11-19, branche `copier-decor`, partie 190 de `docs/INTERFACE.md`)

Les objets du décor 3D d'une séquence se **copient, se coupent et se collent** (Ctrl+C / X / V après un clic dans la vue Espace 3D ou
dans le décor du panneau ; boutons du panneau), **se copient ou s'envoient vers une autre séquence**, passent **d'un projet ouvert à
l'autre** ; une séquence née d'une autre (« Couper la séquence ici », « Nouvelle séquence avec les plans choisis ») **emporte une
copie de son décor**. Code : `copiedecor.rs`.

- **Le presse-papiers** (`App::copie3d`) garde des copies des objets avec leur `Arc<Modele3d>` : coller ne relit rien et n'envoie
  rien de plus à la carte graphique (le même `Modele3d::uid`). Dans un autre projet, le modèle de même empreinte qu'il a déjà est
  repris, sinon celui de la copie y entre et s'enregistre avec lui (`decor3d::plan_save`, par l'empreinte).
- **Où** : dans la même séquence, à côté (la largeur du groupe, comme « Dupliquer ») tant que les originaux y sont ; ailleurs, à la
  même place du monde. Puis l'aimant des objets (`decor3daimant::aimanter`) et des cartes (`reposer_les_cartes_aimantees`) ; une
  étape d'annulation (`App::decor3d_transferer`, le cœur commun).
- **Une séquence née d'une autre** : `Document::copier_decor_de_sequence` (un décor neuf, `uid` neuf, objets aux numéros neufs, mêmes
  modèles et mêmes places), indépendant ensuite.
- Le fichier ne change pas (format 61, `decors3d.json` version 2).

## 21. Les bords doux entre le décor et les cartes (2026-11-20, branche `bords-doux`, partie 193 de `docs/INTERFACE.md`)

Depuis la partie 183 (section 16), une carte était cachée **tout ou rien** à chaque pixel de la feuille : le bord d'un objet du
décor qui passe devant une carte, et la ligne où une carte inclinée entre dans un objet, faisaient des escaliers. Ils sont
maintenant **lissés sur un pixel**, partout où la profondeur est partagée. Code : `profondeur.rs` (`ProfDecor`, `part_cachee`,
`Masque::visible`).

- **La profondeur du décor en quatre échantillons par pixel, seulement quand une carte la lit.** La passe de l'image du plan est celle
  d'avant (sa couleur à un échantillon, sa profondeur en une valeur, `PROF1`, que les ombres lisent toujours). Quand une carte du plan
  partage la profondeur et que le décor tient dans le budget avec les bords doux (`decor3d::bords_doux`,
  `BudgetMachine::bords_doux_pour` : l'étalonnage mesure leur surcoût à part, `Chronos::prof4` ; le budget d'un décor reste celui
  d'avant), une passe de la profondeur seule à quatre échantillons (`Moteur::prof4`, sans lumière ni fragment), puis une petite passe
  (`decor3dgpu::PROF_DES_ECHANTILLONS`) qui ramène ses quatre échantillons à l'inverse de leur distance à l'œil, un par canal d'une
  seconde texture `Rgba16Float` (`decor3dgpu::PROF`, `CelTexture::prof4`), aux places standard du lissage à quatre échantillons
  (`profondeur::MOTIF_CARTE`). Une carte qui se met à partager la profondeur ne fait faire que cette passe (`profondeur_quatre`), la
  couleur du décor n'est pas redessinée ; une carte qui cesse ne fait rien faire. Sinon, les bords nets de la partie 183, au coût
  d'avant. Toute la passe à quatre échantillons coûtait trop avec les ombres du décor (2,5 à 5 fois plus). Processeur :
  `rendre_objets_prof` garde, avec la valeur de chaque pixel, les quatre échantillons de son sur-échantillonnage de deux par côté
  (`MOTIF_GRILLE`).
- **À la composition**, avec les quatre échantillons (le drapeau 4 des composeurs), la carte est évaluée à la place de chacun (son
  plan est linéaire sur la feuille) ; la part où le décor est plus près qu'elle (la tolérance de la partie 183, inchangée) est retirée de son opacité : 0, ¼, ½, ¾ ou
  tout. Carte graphique : `visible` de `fs_compose_p` / `fs_blend_p` ; processeur : `opacite_visible` dans `composite_tile_into`,
  `composite_world_tile` et `pixel_of`. L'intérieur est tout caché ou tout visible, exactement comme avant.
- **Les ombres des cartes** (sections 18 et 19) lisent la profondeur en une valeur d'avant (`PROF1`, `ProfDecor::un`) : elles ne
  changent pas.
- **La vue Espace 3D** : le voile d'une carte comptait déjà les quatre échantillons du décor de la vue (le bord d'un objet était
  doux), mais prenait la distance de la carte au centre du pixel ; il la prend maintenant à la place de chaque échantillon, d'après
  sa pente à l'écran : la ligne où une carte inclinée entre dans un objet est douce aussi.
- **Sans décor**, rien n'est lu, rien ne change, au bit près.

## 22. L'onglet 3D (2026-11-20, branche `onglet-3d`, partie 198 de `docs/INTERFACE.md`)

L'onglet « 3D », entre le Storyboard et le Montage, montre **la vue Espace 3D d'aujourd'hui, en grand**, sur la séquence courante :
pas de nouveau moteur. Ce qu'il ajoute vit dans `onglet3d.rs` et `onglet3dui.rs` : sa barre de vue (point de vue, 1, 2 ou 4 vues :
`espace3dvues::Disposition`, une vue orthographique seule étant la vue agrandie des quatre), la boussole cliquable, les caméras des
autres plans et le soleil peints par-dessus la vue, le sommaire « Scène 3D », les onglets de Propriétés (le réglage du décor découpé
par `decor3d_reglages_de`), la timeline de la séquence et la banque 3D dans la bande du bas. Les règles « sans latence » tiennent :
rien ne se redessine au repos ; seule l'animation de la boussole (un quart de seconde) demande des images. Un objet caché ou
verrouillé dans « Scène 3D » l'est pour la séance (`Decor3d::caches`, `Decor3d::verrous`, pas dans le fichier) : caché, ni la vue ni
l'image ne le dessinent (`Decor3d::vus`) ; verrouillé, il ne se prend plus dans la vue.

## 23. Les couleurs d'identification et les aspects du décor (2026-11-20, branche `couleurs-3d`, partie 199 de `docs/INTERFACE.md`)

Chaque objet du décor peut avoir une couleur d'identification (`Objet3d::couleur`, la palette de `couleurs3d.rs`), et le décor se
montre en trois aspects (`couleurs3d::Rendu`) : Matières (le rendu d'avant), Couleurs (la couleur de chaque objet, éclairée à plat),
Argile. L'aspect de la vue est un réglage du poste (`Settings::aspect_vue_3d`) pour toutes les vues 3D ; celui des images des
plans est celui du décor de la séquence (`Aspect::rendu`), dans la clé de l'image (`DecorCle::bits`, et les couleurs dans `contenu`
pour l'aspect Couleurs). Aucun maillage n'est refait : une teinte par objet (`couleurs3d::Teinte`, `Dessin::teinte`) remplace la
couleur de base de ses matières dans l'uniforme de la carte graphique (les drapeaux `z` et `w`), et dans le rendu du processeur
(`rendre_objets_prof_teintes`). Les règles sans latence tiennent : un changement redessine une fois. L'œil et le cadenas d'un objet
(section 22) sont maintenant dans le fichier (`Objet3d::cache`, `Objet3d::verrou`), plus pour la séance seulement.

## 24. La mise en scène : lampes, objets animés, objets cachés dans un plan (2026-11-20, branche `mise-en-scene-3d`, partie 200 de `docs/INTERFACE.md`)

Un décor de séquence a des **lampes** (`Decor3d::lampes`, `miseenscene3d::Lampe3d`, huit au plus) : des lumières ponctuelles, sans
ombres, qui éclairent le décor (pas les cartes). Elles passent au rendu par l'aspect (`Aspect::lampes`, `LampesVues`, jamais écrit avec
lui) : les réglages d'une passe de la carte graphique (640 octets, `LAMPES_AU`) et `Aspect::ombrer_au` du processeur, avec les mêmes
formules. Un objet a des **clés** (`Objet3d::cles`) : sa place à une image de la séquence, adoucie entre deux ; et des plans dont
l'image ne le montre pas (`Objet3d::caches_dans`). L'image gardée du décor (`DecorCle`) compte les poses à l'image, les objets cachés
dans le plan et les lampes **seulement quand il y en a** (`contenu_a`) : un décor immobile garde exactement sa clé d'avant, un décor
animé en fait une par image, redessinée une fois dans la même texture. Sans latence : 0,4 ms par passe pour 40 objets en 1920 × 1080,
huit lampes comprises (mesures de la partie 200).

## 25. Finitions : la timeline de la séquence, les objets d'une séquence à l'autre, qui a la main, un calque neuf devant la caméra (2026-11-20, branche `finitions-3d`, partie 201 de `docs/INTERFACE.md`)

La timeline de la séquence de l'onglet 3D se zoome et défile (`timelineseq.rs`, `Fenetre`). Les objets du décor se copient ou se
déplacent vers une autre séquence par le menu d'une ligne de « Scène 3D » ou en la glissant sur le sélecteur de séquence
(`seqdecor3d.rs`, par `decor3d_vers_sequence` de la partie 190). Chaque vue 3D dit qui a la main, le décor ou une carte
(`quialamain.rs`). Un calque neuf se pose devant une caméra que le chemin de toujours ne filme pas (`calquedevant.rs`, le repère de la
partie 159). Les lampes éclairent les cartes qui reçoivent les ombres (201.4) : `ombres::Lumiere` porte les lampes, la lumière des lampes seules
existe sans les ombres du décor, le drapeau 8 des composeurs de la carte graphique et le contexte du processeur ajoutent
`c × (1 + lampes)` en lumière linéaire ; une carte hors de portée est inchangée au bit près.

## 26. Les cyclos (2026-11-21, branche `cyclos`, partie 202 de `docs/INTERFACE.md`)

Un cyclo par séquence (`cyclos.rs`, `Decor3d::cyclo`), même sans objet : le ciel à l'infini, l'horizon et ses silhouettes, et sa
lumière, dessinés par FEANOR (neuf choix, puis l'heure, les nuages, la brume, la rotation, la graine, une couleur). Son image
équirectangulaire (2048 × 1024) est faite une fois hors du fil de l'interface et gardée par ses réglages ; le ciel du décor la lit
(processeur et carte graphique), le soleil du décor suit le sien (`Aspect::soleil`). Les cartes qui « prennent la lumière du cyclo »
reçoivent `c × (ambiante + ambiante_soleil × n·soleil)` en lumière linéaire (le drapeau 16 des composeurs, le contexte des ombres du
processeur), avec les ombres et les lampes ; Jour clair vaut 1.
