# Espace 3D : les calques et la caméra, rien d'autre

Branche `espace3d-simple` (depuis `integration14` 5f3fda7), 2026-10-21. Décision d'Anthony, le directeur :

> « On va rester simple et focus sur le dessin. Un espace 3D où je peux bouger les dessins dans l'environnement suffit.
> On peut supprimer les objets 3D comme les cubes, cylindres, personnages… ça alourdit le logiciel et l'interface pour pas
> grand-chose. Revenons à quelque chose de simple. »

Et, dans la foulée : le **repère 3D** (un modèle glTF en fil de fer montré par-dessus le dessin) part aussi.

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

Position X/Y/Z, rotation X/Y/Z, échelle uniforme et par axe (`Xf`). La carte tourne autour de son centre. Rotation : l'ordre
`Ry · Rx · Rz` ; Z est le tour dans le plan de la carte (le « angle » du calque), X et Y sont **l'inclinaison**. Inclinaison
X positive : le haut de la carte vient vers la caméra. Inclinaison Y positive : le bord droit s'éloigne.

### 3.2 Correspondance avec les champs du calque (rien n'est perdu, les anciens projets sont identiques)

Les champs du calque (`LayerTransform` : x, y, échelle, angle, étirements, `tilt_x`, `tilt_y`, `dz` ; `Layer::depth`) restent
la seule source. La pose 3D est *calculée* à partir d'eux (`Monde::card_xf`), et les poignées *écrivent* ces champs
(`Monde::card_from_xf`, `apply_card_xf`). Tout l'existant (outil Transformer, multisélection, clés de position, vue de
dessus, export Storyboard Pro, propagation) lit les mêmes champs ; un projet d'avant a inclinaison 0 et écart 0, et son rendu
passe par **le même code** qu'avant.

Avec `d` la profondeur du calque (`depth` + `dz`, au plus −0,9), `s = 1 + d`, `W×H` la feuille en pixels :

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
   multisélection ; l'export vers Storyboard Pro ignore l'inclinaison.
5. Inclinaison limitée à ±85° ; la pose est donnée en angles d'Euler (ordre Y · X · Z), la poignée de rotation peut sembler
   sauter près de ±90°.
6. Dossiers : un dossier n'est pas une carte ; sa pose ne place pas ses calques dans l'espace.
7. Comparaison avec Storyboard Pro / Harmony : non faite (à vérifier).
