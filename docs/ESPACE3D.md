# Espace 3D : un seul espace pour les calques, les objets et la caméra

Branche `espace-3d` (depuis `main` 1d1ccca), 2026-10-20. Demande d'Anthony : « Il y a quelque chose de très confus entre
ces deux espaces 3D. Il nous faut un seul espace 3D où les calques apparaissent et donnent la possibilité de les déplacer
dans l'espace pour créer un environnement 3D avec, et ainsi une parallaxe. Ça implique donc d'importer dans cet espace tous
les outils 3D et de pouvoir déplacer, pivoter, modifier les calques dans cet espace comme de vrais objets 3D. »

Ce document est la conception (écrite avant le code) puis, en dernière partie, l'état de ce qui est fait. Les mots du
produit : **Espace 3D** (la vue), **carte** (un calque du plan posé dans l'espace), **objet** (cube, plan, cylindre, sphère,
cône, escalier, mannequin, modèle glTF), **caméra** (celle du plan, une seule).

## 1. Ce qui existe, et pourquoi c'est confus

| | Plateau en 3D (`stage3d.rs`, `stage3dui.rs`) | Scène 3D de blocage (`scene3d*.rs`) |
|---|---|---|
| Ce qu'on y voit | les calques du plan, plats, à leur profondeur ; la caméra du plan (pyramide, cadre) ; un sol | cube, plan, cylindre, sphère, cône, escalier, mannequin, modèle glTF ; un sol quadrillé ; une « caméra du plateau » |
| Unités | pixels de la feuille | mètres |
| Ce qu'on y manie | un calque : glisser dans son plan, Maj : en profondeur ; la caméra par son œil | tout : manipulateurs (déplacer, tourner, échelle), axes locaux, aimantation, groupes |
| Caméra | celle du plan (clés `camkeys.rs`) | une autre, fixe, propre à la scène (`PlanScene::cam`) |
| Résultat dans l'image | oui, tout de suite : c'est le rendu 2D (multiplan) | non : il faut « envoyer vers un calque de fond » (une image figée, calque « Fond 3D ») |

Deux mondes, deux caméras, deux jeux d'outils, et un seul des deux change l'image. Un calque ne peut pas être tourné
dans l'espace ; un cube ne bouge pas avec la caméra du film.

## 2. Le principe : un monde, trois sortes d'habitants

Un seul monde, celui de la scène 3D de blocage (il est déjà le plus riche) :

- **mètres**, Y vers le haut, X vers la droite, Z vers la caméra ;
- **l'origine est le centre de la feuille** ; le plan z = 0 est la feuille (le « plan de référence » de la caméra 2D) ;
- l'échelle est celle de `scene3d::PLATE_WIDTH_M` : le cadre de l'image finale mesure 6 m de large à la distance de
  référence, donc 1 pixel de la feuille vaut `k = 6 / largeur du cadre` mètres (`SceneCam::from_plan_camera` le faisait déjà) ;
- la distance de référence (`camera::ref_distance`, 35 mm voient exactement le cadre) vaut `d0 = 5,83 m`.

Les habitants :

1. les **cartes** : un calque du plan = un rectangle de la taille de la feuille, texturé par son dessin ;
2. les **objets** : tout ce que la scène 3D contenait (par plan, plus le décor de la séquence) ;
3. la **caméra** : celle du plan, avec ses clés (dolly, bascule, panoramique, zoom, rotation).

Le plateau en 3D (`stage3d`) n'a plus de vue à lui : ses maths (`World`) restent pour les tests et la vue de dessus, son
interface (`stage3dui.rs`) disparaît ; ses gestes (glisser un calque, prendre la caméra par l'œil) sont ceux de l'Espace 3D.

## 3. La carte : un calque est un vrai objet 3D

### 3.1 Pose d'une carte

Position X/Y/Z, rotation X/Y/Z, échelle uniforme et par axe, comme un objet (`Xf` : position, rotation, taille). La carte
tourne autour de son centre. Rotation : l'ordre des objets (`Ry · Rx · Rz`) ; Z est le tour dans le plan de la carte (le
« angle » du calque), X et Y sont **l'inclinaison**. Inclinaison X positive : le haut de la carte vient vers la caméra.
Inclinaison Y positive : le bord droit s'éloigne.

### 3.2 Correspondance avec les champs existants (rien n'est perdu, les anciens projets sont identiques)

On **garde** les champs du calque (`LayerTransform` : x, y, échelle, angle, étirements ; `Layer::depth`) et on y **ajoute**
l'inclinaison (`tilt_x`, `tilt_y`, en degrés) et un écart de profondeur de clé (`dz`). La pose 3D d'une carte est *calculée*
à partir d'eux (`espace3d::Monde::card_xf`), et les manipulateurs *écrivent* ces champs (`card_from_xf`). Avantage : tout
l'existant (outil Transformer, multisélection, clés de position, vue de dessus, export Storyboard Pro, propagation) continue
de lire les mêmes champs ; un projet d'avant a inclinaison 0 et écart 0, et son rendu passe par **le même code** qu'avant (pas
une réécriture qui donnerait « presque » les mêmes pixels).

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

Le facteur `s` est le « agrandi de (1 + d) » du multiplan : un calque reculé garde, vu par la caméra de référence, la
taille qu'il a à plat ; il ne défile moins vite que quand la caméra bouge. **Dans l'Espace 3D un calque que l'on recule avec le
manipulateur rapetisse à l'écran** (c'est un vrai objet : sa taille dans le monde est gardée, `échelle` et `x`, `y`
s'ajustent) ; le champ « Profondeur » du panneau Calques garde son sens d'avant (la taille vue reste la même). Les deux
écrivent les mêmes champs, aucun état en double.

Retour (manipulateur → calque) : `d = −Z / d0` ; `x = X / (s·k)`, `y = −Y / (s·k)` ; l'échelle et les étirements se déduisent
du rapport entre la nouvelle taille et celle que la carte aurait au nouveau `s` ; si les deux axes ont changé du même
rapport, c'est l'échelle uniforme qui bouge, sinon les étirements (un cube central = échelle uniforme, une poignée d'axe =
un étirement). Un calque dont la position est verrouillée, ou verrouillé, ne bouge pas.

### 3.3 Clés (déplacer dans le temps)

Les clés de position (`poskeys.rs`) portent déjà x, y, échelle, angle, étirements, adoucis. Elles portent maintenant aussi
`tilt_x`, `tilt_y` et `dz` : même interpolation (ligne droite, adoucis d'entrée et de sortie), mêmes losanges verts dans la
timeline et le montage. Un calque animé dont on tire la carte à l'image `f` met à jour (ou crée) sa clé de l'image `f`
(`Layer::set_pose`), comme l'outil Transformer. Un calque fixe garde l'inclinaison dans sa pose fixe ; son écart de profondeur
est ajouté à `depth` (jamais deux sources). Un projet d'avant a des clés sans ces valeurs : elles valent 0, il se lit tel quel.

## 4. Une seule caméra

La caméra du plan (`camera::Camera`) *est* la caméra 3D : position de l'œil (x, y de la feuille, avancée), panoramique,
bascule, rotation, zoom. `SceneCam::from_plan_camera` montre que la correspondance est exacte dans les deux sens : l'œil est à
`((cam.x − W/2)·k, −(cam.y − H/2)·k, (d0_px − dolly)·k)`. Elle a ses clés (`camkeys.rs`) : tirer la caméra dans l'Espace 3D
règle la caméra de l'image courante, ou la clé de cette image si le plan a plusieurs clés (`App::set_current_camera`), comme
l'outil Caméra. La pyramide (œil, cadre, haut du cadre) est dessinée à la place de la « caméra du plateau ». « Vue caméra »
de l'Espace 3D regarde par elle, au format du cadre.

**La caméra du plateau de la scène 3D disparaît de l'interface** (champs, bouton « choisir », « Caler sur la caméra du plan »).
Migration : son réglage reste enregistré (`PlanScene::cam`) parce que l'image du calque « Fond 3D » d'un ancien projet a été
rendue par elle et doit pouvoir être refaite à l'identique ; il ne sert plus qu'à cela. « Figer en calque de fond » (l'ancien
« Envoyer… ») le remplace d'abord par la caméra du plan à l'image courante.

## 5. Ce que donne l'image finale

La chaîne de rendu de SHOT est en deux temps, et on **la garde** : (1) tous les éléments se composent sur la feuille, vus
par l'**œil** de la caméra (position seulement : c'est le multiplan) ; (2) la caméra (zoom, rotation, bascule, panoramique)
prend cette feuille en homographie. Une rotation de la caméra autour de son œil ne crée pas de parallaxe : séparer les deux
temps est exact.

- **Carte à plat** (inclinaison 0) : exactement le rendu d'avant (`Layer::transform_seen_at`, une affinité). Même code, mêmes
  pixels.
- **Carte inclinée** : la carte est projetée, point par point, de l'œil sur le plan z = 0 (une homographie). Elle est posée
  sur la feuille par un maillage (`warp.rs`, le même que l'enveloppe et la perspective du Déformer : 64 × 64 cases, des
  triangles que la carte graphique et le processeur dessinent identiquement). Les effets du calque, son opacité, son mode de
  fusion, ses clés s'appliquent *avant*, sur l'image de la carte à plat (les effets sont calculés sur le dessin du calque, puis
  le dessin est posé ; un calque d'ajustement au-dessus voit la carte posée).
- **Objets** : rendus par le rasteriseur à tampon de profondeur de la scène (`scene3drender.rs`), vus par le même œil, et posés
  comme une image de la feuille tout en bas du plan (voir 5.2).
- **Écran, lecture, vignettes, export** : un seul chemin de composition (`render::panel_plan_at`) ; le processeur est la
  référence (export) et la carte graphique la reproduit à ≤ 2 niveaux près, comme pour les feuilles déformées.

### 5.1 Ordre

**Les cartes se composent dans l'ordre de la pile des calques**, pas par distance. C'est ce qu'il faut pour les modes de
fusion, les calques d'ajustement et les dossiers (ce sont des opérations « sur ce qui est dessous »), et c'est le
comportement de tout le reste de SHOT. L'Espace 3D dessine donc les cartes dans le même ordre, pour que ce qu'on voit soit
ce qu'on filme. La commande **Ordonner la pile selon la profondeur** met les calques du plus loin au plus près (une étape
d'annulation) quand on veut que les deux coïncident. Pas de test de profondeur entre cartes qui se traversent : elles se
recouvrent dans l'ordre de la pile.

### 5.2 Les objets dans l'image

Les objets 3D (cube, mannequin, modèle…) sont rendus *par la caméra du film* : ils suivent ses clés (parallaxe), sans
« envoi » à refaire. Ils sont posés **sous toutes les cartes** (comme l'était le calque « Fond 3D »), donc une carte
recouvre un objet, jamais l'inverse. Règle d'ordre documentée faute de mieux : un objet opaque *devant* une carte (un pilier
devant un personnage) demanderait un tampon de profondeur partagé entre l'objet et les cartes dans la composition de la
carte graphique ; ce n'est pas fait (voir « Ce qui reste »). La vue d'édition de l'Espace 3D suit la même règle (objets
d'abord, cartes ensuite) pour ne pas promettre ce que l'image ne fait pas.

Un objet est dans l'image si son plan a **« Objets dans l'image » allumé**. Un plan d'un ancien projet l'a *éteint* : ses
objets étaient des repères de blocage (ils n'apparaissaient que dans le calque « Fond 3D »), les montrer ajouterait des
pixels. Un plan neuf l'a allumé.

### 5.3 Limites connues

- Une carte inclinée est coupée aux bords de la feuille (comme une feuille déformée). Les cartes à plat gardent la toile
  infinie.
- Une partie de carte qui passe derrière l'œil (ou à moins de 1 % de la distance de référence) n'est pas dessinée.
- Le panoramique / bascule ne peut pas aller jusqu'à l'horizon (`Camera::limited` : la feuille doit rester visible sous au
  moins 5°) : c'est la limite d'avant, elle vaut pour les cartes inclinées.
- L'aperçu réduit des effets (`fxproxy`) n'est pas utilisé pour une carte inclinée (elle est calculée en pleine résolution).

## 6. Format et migration

- **Projet** : `project::VERSION` passe de 33 à **35** (champs `inclinaison_x`, `inclinaison_y` de la pose d'un calque,
  et dans une clé de position `inclinaison_x`, `inclinaison_y`, `ecart_profondeur` ; tous omis quand ils valent 0). Une
  version précédente refuse le projet au lieu de montrer des cartes à plat. Règle des projets partagés : ils écrivent
  `max(VERSION_PARTAGE, VERSION)` — le numéro 34 des projets partagés n'était qu'un numéro plus grand que le classique ; il
  reste lisible, et un projet partagé s'écrit désormais 35. Le test « projet partagé > VERSION » devient « projet partagé ≥ VERSION
  et porte son bloc `partage` ».
- **Anciens projets** (≤ 34) : champs absents = 0 ; rendu identique, pixel pour pixel (essai : un plan avec profondeurs et un
  calque « Fond 3D » rendu avant / après).
- **Profondeur du plateau en 3D** : c'est déjà `Layer::depth`, devenue la position Z de la carte (`Z = −d0·d`). Aucune conversion
  à l'ouverture.
- **Scènes 3D** (`scenes3d.json`, sans toucher à son numéro de format) : un champ en plus par plan, `en_image` (absent = faux).
  Les objets restent là où ils sont, la caméra du plateau reste enregistrée (4). Le calque « Fond 3D » d'un ancien projet reste
  un calque bitmap ordinaire, verrouillé, tout en bas ; « Suivre » continue de le mettre à jour avec la même caméra.
- Une version précédente lit toujours `scenes3d.json` (elle ignore `en_image`).

## 7. Interface

- **Un seul bouton** « Espace 3D » (icône cube) dans la barre sous le dessin, une seule entrée du menu Affichage, une seule
  commande `espace3d.basculer`. `vue.3d` et `scene3d.basculer` en sont des doubles (`keymap::ALIASES`) : un raccourci posé
  sur l'un des deux marche. `vue3d.recadrer` double `scene3d.cadrer` ; `outil3d.deplacer` double `scene3d.outil_deplacer` ;
  `outil3d.orbite`, `outil3d.glisser`, `outil3d.avancer` doublent `scene3d.outil_choisir` (glisser dans le vide tourne la vue,
  bouton du milieu la glisse, molette avance : il n'y a plus d'outils de navigation à choisir).
- **Gestes** : clic = choisir (une carte, un objet ou la caméra ; Maj / Ctrl : ajouter) ; glisser un objet = le poser sur le
  sol ; poignées du manipulateur (déplacer, tourner, échelle ; axes du monde ou de l'objet ; aimantation) pour tout ce qui est
  choisi ; clic droit tourne la vue ; milieu la glisse ; molette avance. Une carte se tire dans son plan (plan z) ou, avec Maj,
  en profondeur.
- **Une étape d'annulation par geste**, au nom français : « Déplacer dans l'espace 3D », « Tourner dans l'espace 3D »,
  « Mettre à l'échelle dans l'espace 3D », « Déplacer la caméra », « Ordonner la pile selon la profondeur »…
- **Réglages** (panneau Réglages tant que l'Espace 3D est ouvert) : la liste des cartes (calques) et des objets, cliquables ;
  pour une carte : position X/Y/Z (m), rotation X/Y/Z (°), échelle (%), étirement X/Y (%), profondeur multiplan, inclinaison
  remise à plat ; pour la caméra : position, panoramique, bascule, rotation, focale.
- Icônes et bulles d'une seconde (nom, raccourci, description) par `bulles.rs` ; boutons à état par `bulles::bouton_etat`
  (`encre_etat` / `symbole_etat`) ; couleurs du thème (`theme::`).
- Toujours dans le Plateau à plat (vue normale) : dessiner sur une carte se fait sur sa texture, comme avant. Pour une carte
  inclinée, le pinceau suit l'inclinaison (inverse de l'homographie au point visé).

## 8. Performance (règle « sans latence »)

- Tourner la vue, tirer une carte ou un objet : rien ne passe par le processeur lourd. Les objets sont dessinés par la carte
  graphique (maillages gardés, une matrice de 176 octets par objet) ; les cartes sont des maillages egui de 16 × 16 cases,
  leurs textures (≤ 1024 px) sont refaites quand le dessin change seulement. Objectif : 60 images par seconde avec 10 cartes
  et 20 objets (essai de budget sur le calcul de l'image : sélection, projection, maillages).
- Le rendu des objets vers l'image se fait sur un autre fil, jamais pendant un trait ni un geste dans la vue (comme le
  calque « Fond 3D » avant).
- Le maillage d'une carte inclinée ne dépend que de la pose et de l'œil : il est refait à l'image, pas à chaque affichage,
  et reste une structure légère (4 225 points).
- Aucun repaint au repos.

## 9. Essais

Migration (profondeurs du plateau → Z, caméra du plateau, ancien calque « Fond 3D », projet d'avant identique pixel pour pixel,
numéros de format et projets partagés), pose d'une carte (aller-retour monde ↔ calque, manipulateurs déplacer / tourner /
échelle, annulation en une étape, verrous), clés 3D (interpolation, adoucis, écriture / lecture), parallaxe (une carte plus
loin bouge moins quand la caméra se déplace ; une carte inclinée aussi), rendu (processeur contre carte graphique ≤ 2 niveaux,
vignette = composition, export = écran), ordre, budget de temps, interface (une seule entrée, doubles, bulles).

## 10. Comparaison avec Storyboard Pro et Harmony (à vérifier)

Storyboard Pro a, d'après sa documentation, un « stage 3D » où les couches sont des plans que l'on place dans l'espace ; le
détail (inclinaison, ordre de rendu, caméra) n'a pas été comparé. Rien de ce document ne prétend reproduire un comportement
de Storyboard Pro ou de Harmony.

## 11. État (fin de séance)

Voir la section « État » en bas de ce fichier, remplie à la fin du travail.
