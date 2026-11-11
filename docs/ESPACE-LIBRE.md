# Plan en Espace 3D : dessiner dans un décor que la caméra traverse (étape A)

Branche `espace-libre` (depuis `main` 7d8a7d2), 2026-11-11. **Plan validé par Anthony le 2026-11-11 pour l'étape A0
(vérifications), faite le même jour : résultats en partie 6. Les décisions de la partie 7 restent à trancher avant A1.
Étape A1 (la case « Plan en Espace 3D », sans effet sur l'image) faite le 2026-11-11 à la demande d'Anthony, avec les
propositions de la partie 7 prises par défaut.**

Sa demande : pousser dans FEANOR la possibilité de dessiner dans un environnement 3D, à la manière d'un add-on de
storyboard 3D pour Blender (StoryLiner), sans rien casser de ce que FEANOR propose.

Ce document couvre l'**étape A** seulement : un plan dont la caméra est libre. Les étapes suivantes (B : un décor partagé
par plusieurs plans d'une séquence ; C : décors glTF et mannequin, depuis le code retiré le 2026-10-21) auront leur propre
plan. Rien de l'étape A ne les empêche, rien ne les prépare en avance.

Écrit d'après une lecture du code de `main` (`espace3d.rs`, `camera.rs`, `render.rs`, `warp.rs`, `document.rs`,
`app.rs`, `docs/ESPACE3D.md`, `docs/ARCHITECTURE.md`). Aucun essai lancé, rien regardé à l'écran.

## 1. Pour Anthony : ce que ça change

Aujourd'hui, dans l'Espace 3D, la caméra regarde toujours plus ou moins la feuille de face : elle peut glisser, avancer,
pencher un peu, mais pas se retourner ni passer derrière un dessin. Les cartes penchent au plus de 85°.

Avec un **plan en Espace 3D** :

- la caméra va où elle veut : elle tourne sur elle-même, fait le tour d'un personnage, passe entre deux cartes, regarde
  derrière elle ;
- une carte peut être couchée au sol ou dressée comme un mur ;
- ce qui est devant cache ce qui est derrière, sans avoir à ranger la pile des calques ;
- on crée d'un geste une surface à dessiner « face à la caméra, à telle distance », « au sol » ou « en mur » ;
- on dessine en regardant par la caméra, et le trait tombe sur la carte choisie, quelle que soit son orientation.

Ce qui ne change pas : un plan ordinaire reste un plan ordinaire. Le plan en Espace 3D est une case à cocher, plan par plan.
Les projets existants s'ouvrent comme avant, avec les mêmes pixels.

**Coût** : 8 étapes (partie 6), chacune avec ses essais et fusionnable seule. Ordre de grandeur, à affiner après l'étape
A0 : deux à trois semaines de séances.
**Risque principal** : le nombre d'endroits du logiciel qui demandent « que filme ce plan ? » (écran, lecture, vignettes,
exports, planche). Tous doivent répondre pareil pour un plan en Espace 3D. L'étape A0 les compte avant tout code.
**Réversibilité** : complète. Décocher la case rend le plan ordinaire ; retirer la fonction ne touche pas aux plans
ordinaires.

## 2. L'idée : la caméra ne bouge pas, c'est le monde qui tourne devant elle

Le rendu actuel se fait en deux temps (`docs/ESPACE3D.md`, 5) : les cartes se posent sur la feuille, vues par l'œil de la
caméra, puis la caméra cadre la feuille. Il faut donc que la caméra voie encore la feuille (`Camera::limited`,
`MIN_GRAZING` = 5°) : c'est de là que viennent toutes les limites.

Pour un plan en Espace 3D, on ne construit **pas de second moteur d'image**. On change de repère :

1. on calcule le déplacement `C` qui amène la caméra « de référence » (au centre de la feuille, droite, à la distance
   `d0`) sur la caméra réelle du plan (position, panoramique, bascule, rotation) ;
2. on applique l'inverse de `C` à chaque carte : la carte est exprimée dans le repère de la caméra ;
3. on pose ces cartes sur la feuille avec le code d'aujourd'hui (`Monde::to_sheet`, `card_warp`, `Warp::partial`), l'œil
   étant celui de la caméra de référence ;
4. on cadre la feuille avec la caméra de référence, qui la voit toujours de face.

Conséquences :

- les maillages (`warp.rs`), la composition par le processeur (`render.rs`) et par la carte graphique (`gpu.rs`), les
  effets, l'opacité, les modes de fusion et les calques d'ajustement servent tels quels ;
- la limite des 5° disparaît d'elle-même pour ces plans (la caméra de référence voit toujours la feuille) ; une carte ou
  un morceau de carte derrière la caméra est déjà écarté par `Warp::partial` ;
- caméra droite et cartes à plat : `C` est l'identité, le plan passe par l'affinité d'aujourd'hui, **mêmes pixels**.

La focale (le zoom) est comptée dans la projection de l'étape 3, pour que la zone filmée reste le cadre au centre de la
feuille, quelle que soit la focale (voir A0, point 2).

## 3. Ce qui est ajouté aux données

- `Panel::espace: bool` (faux par défaut). Manifeste : `"espace": true` par plan, absent si faux. Projets partagés : même
  champ dans le fichier du plan.
- `project::VERSION` 53 → 54 (tranché en A0, point 4) : une version d'avant ignorerait le champ sans rien dire et
  montrerait le plan à plat, avec une autre image ; elle doit refuser le projet, comme pour tout format qui change l'image.
- La caméra reste `camera::Camera` (x, y, avancée, panoramique, bascule, rotation, zoom) : elle sait déjà tout décrire.
  Pour un plan en Espace 3D, on ne passe plus par `limited` ni par la borne de `Camera::distance` (la caméra peut passer
  derrière le plan z = 0). Les clés (`camkeys.rs`), la saccade et les effets vivants (`camfx.rs`) lisent les mêmes champs.
- L'inclinaison d'une carte (`LayerTransform::clamp_tilt`, ±85°) est libre jusqu'à ±90° dans un plan en Espace 3D.
- Aucun nouveau type de calque, aucun objet 3D, aucun fichier en plus.

## 4. Ce qui ne change pas (les garde-fous)

- Un plan avec `espace = false` ne passe par **aucune** ligne nouvelle : tous les nouveaux chemins sont derrière ce test.
- Les essais existants (`espace3d_tests.rs`, `espace3dmath_tests.rs`, `exportrender_tests.rs`, `gpu_tests.rs`,
  `vignettes_camera_tests.rs`…) doivent passer sans être modifiés. Un essai qu'il faudrait modifier est un signal d'arrêt.
- Cocher la case sur un plan existant ne change pas son image tant que rien n'est déplacé (essai A3) : à profondeur
  égale, l'ordre de la pile est gardé.
- Décocher la case : la caméra et les cartes repassent par les limites d'aujourd'hui ; l'image peut changer, c'est une
  étape d'annulation, et un message le dit.

## 5. Ce que l'étape A ne fait pas

| Hors de l'étape A | Pourquoi |
|---|---|
| Décor partagé entre plusieurs plans | étape B (touche au format et aux projets partagés) |
| Objets 3D, mannequin, import glTF | étape C (code retiré le 2026-10-21, `git` `5f3fda7`) |
| Cartes qui se traversent | l'ordre se fait carte par carte (du plus loin au plus près), pas point par point |
| Outil Transformer, boîte de sélection et multisélection sur une carte inclinée | limite déjà connue (`ESPACE3D.md`, 11.4), inchangée |
| Vue de dessus (`topview.rs`), export de la caméra (`exportcamera.rs`), mouvements de caméra des Vignettes (`vignettecam.rs`) pour un plan en Espace 3D | désactivés pour ces plans, avec un mot d'explication ; à reprendre après |
| Netteté d'une carte au sol vue de très près | une carte garde la définition de son dessin ; à mesurer, pas à résoudre ici |

## 6. Les étapes

Chaque étape : une branche courte depuis `espace-libre`, ses essais, la mise à jour de ce document, un commit. Aucune
n'est visible de l'utilisateur avant A1.

### A0. Vérifier avant de coder (lecture et essais seulement, aucun changement de comportement)

1. **Compter les endroits qui évaluent la caméra ou composent un plan** et les lister ici. Relevé du 2026-11-11 (hors
   essais) : `panel_plan_at` dans 14 fichiers, `camera_image` dans 7, `transform_seen_at` dans 7, `camera_at` dans 3,
   `Camera::limited` dans 3 (`camtools.rs`, `camfx.rs`, `vignettecam.rs`), `espace3d::place` dans 2. Les points d'entrée
   `Document::panel_camera` et `Document::camera_seen` semblent centraux : le confirmer.
2. **La zone filmée tient-elle dans ce qui se compose ?** `render::plan_bounds` ne compte que la feuille et l'encre, et
   un dessin posé par un maillage y vaut « la feuille entière ». Vérifier que le cadre est toujours dans la feuille
   (`Document::frame` ≤ `width` × `height`) ; sinon, prévoir l'agrandissement de la zone pour les plans en Espace 3D.
3. **Dossiers, calques de couleur, fonds sous le trait, griffonnages** : relever comment `panel_plan_at` les enchaîne,
   pour savoir ce qui doit rester groupé quand on ordonne par distance (A4).
4. **Version du format** : ce que fait une version d'avant devant un champ inconnu d'un plan (partie 3).
5. **Budget** : mesurer `place` pour 20 cartes inclinées (aujourd'hui 0,15 à 0,25 ms par carte).

Sortie : cette partie complétée, les décisions de la partie 7 tranchées avec Anthony.

#### Résultats de A0 (2026-11-11, lecture de `main` 7d8a7d2 ; aucun changement de comportement)

**1. Qui pose les calques, qui cadre.**

- *Poser les calques (temps 1)* : deux constructeurs de plan seulement, `render::plan_with_camera_layers` (lecture,
  vignettes, exports, par `panel_plan_at` / `stored_panel_plan` / `panel_layer_plans`) et `App::build_plan_avec`
  (l'écran, avec la pelure d'oignon). Les deux parcourent `calqueannot::ordre` et appellent `espace3d::place`. La caméra
  du temps 1 (`CameraSeen`) n'est fabriquée qu'à deux endroits, `Document::camera_seen` et `camera_seen_du_plan_range`.
  **Le changement de repère et l'ordre par distance tiennent donc en deux endroits.**
- *Cadrer (temps 2)* : `render::camera_image` (7 fichiers : `exportrender`, `exportlayers`, `exportcles`, `noyau`,
  `propager`, `diagbanc`, `render`), les vignettes `thumbnail*` (`thumbs`, `plancheui`, `comparer`), `frame_map`
  (`prereglagesui`, `relectureui`), l'homographie de l'écran (`camtools`, `horschamp`). Tous reçoivent une `Camera` qui
  vient de `Document::panel_camera`, appelé dans 23 fichiers, pour filmer **et** pour l'interface (outil Caméra, vue de
  dessus, fiche, export de caméra, Espace 3D).
- Conséquence pour A3 : `panel_camera` reste la caméra réelle. Une fonction de plus, `Document::camera_filmee`, donne la
  caméra de référence pour un plan en Espace 3D et `panel_camera` sinon ; la douzaine de fichiers qui filment y passent. Un
  oubli se voit (l'image serait cadrée deux fois) : l'essai « chaque sortie = l'écran » de A3 parcourt toutes les sorties.

**2. La zone filmée tient dans ce qui se compose.** La feuille fait au moins le double du cadre à la création
(`Document::with_frame`) et après un changement de taille (`resize_frame`) ; un dessin posé par un maillage compte pour
« la feuille entière » dans `plan_ink_bounds_sauf`. Avec la focale comptée dans la projection, la zone filmée est le
cadre au centre de la feuille : rien à agrandir. Non vérifié : un très ancien projet dont la feuille serait plus petite
que le cadre ; la case « Plan en Espace 3D » est refusée dans ce cas (essai en A1).

**3. Ce qui doit rester groupé.**

- Le remplissage d'un calque de couleur (`griffon_item`) et le fond sous le trait (`fond_item`) sont posés par le calque
  du trait lui-même, juste sous lui : ils le suivent sans rien faire.
- Un dossier n'est pas composé à part : il multiplie l'opacité (`folders::effect_at`) et borne la portée d'un calque
  d'ajustement. Ordonner à l'intérieur de chaque dossier, et déplacer un dossier en bloc, suffit.
- **À corriger en A4** : un calque d'ajustement vise « depuis le premier calque de son dossier » par son rang dans la pile
  (`calqueffet::scope_low`, `starts[low]`). Une fois les calques ordonnés par distance, il faut « depuis le premier
  calque *posé* de son dossier ». Les plages de `panel_layer_plans` se calculent depuis l'ordre parcouru : rien à changer.
- L'ordre se décide donc **avant** la boucle (sur la liste des calques), jamais en triant les éléments après coup.

**4. Format.** La règle de la maison (`project.rs`, note du format 25) : tout ce qui change l'image prend un numéro, et
une version d'avant refuse le projet. `PanelEntry` ignore un champ inconnu sans rien dire : une version d'avant
montrerait un plan en Espace 3D à plat, avec une autre image. **Donc `VERSION` 53 → 54, dès A1.**

**5. Budget : tenu sur le Mac, à confirmer sur le poste Windows.** L'essai
`the_plan_of_an_image_with_twenty_tilted_cards_stays_within_the_frame_budget` (`espace3d_tests.rs` : 20 cartes
inclinées, cadre 1920 × 1080, seuil 8 ms) donne **1,14 ms pour les 20 cartes** (0,057 ms par carte) sur le Mac d'Anthony
(Apple Silicon, 2026-11-11). Le poste Windows est la référence du projet : y lancer
`cargo test --lib twenty_tilted_cards -- --nocapture` et noter ici la ligne `PERF`.

**6. Trouvé en chemin.**

- *Calque accroché à la caméra* (format 53, `accroche.rs`, du jour même) : dans un plan en Espace 3D, c'est une carte qui ne
  reçoit pas le changement de repère (elle reste dans le cadre). À prévoir en A3, avec son essai. Aujourd'hui une carte
  inclinée ne s'accroche pas : inchangé.
- *Compiler sur le Mac* : `rust-toolchain.toml` impose la chaîne Windows et `.cargo/config.toml` un réglage de
  processeur Intel ; `cargo` échoue tel quel sur ce Mac (Apple Silicon). En passant `cargo +stable-aarch64-apple-darwin`
  et `RUSTFLAGS="-Awarnings"`, sans toucher aux fichiers : l'application se vérifie sans erreur (`cargo check --lib`), mais
  les essais ne se compilaient pas, à cause de deux essais écrits pour Windows seulement (`exportrender_tests.rs`,
  `planchebitmap_tests.rs`) : ils sont maintenant gardés par `#[cfg(windows)]` (rien ne change sous Windows). Il reste
  des fonctions de Windows appelées par des essais de mesure (`lourd_tests.rs`, `exportrender_tests.rs` : mémoire et
  temps du processus) ; pour lier les essais sur le Mac, on leur donne des remplaçants vides **hors du dépôt**
  (`-L native=<dossier>` dans `RUSTFLAGS`). Les essais tournent alors sur le Mac ; une partie échoue pour des raisons
  propres au Mac, sans rapport avec ce chantier (voir A1). À régler pour de bon avant la bêta Mac.

### A1. La case « Plan en Espace 3D »

`Panel::espace`, lecture et écriture (classique, en arrière-plan, projets partagés), commande `plan.espace` (palette,
menu du plan, panneau Réglages de l'Espace 3D), une étape d'annulation « Plan en Espace 3D ». Aucun effet sur l'image.
Essais : aller-retour du fichier, octet pour octet quand la case est décochée ; un ancien projet inchangé ; annulation ;
couper, réunir, dupliquer un plan garde la case.

#### Fait (2026-11-11)

- **La case** : `Panel::espace` (`document.rs`), faux pour un plan neuf, importé ou rouvert d'un ancien projet. Elle
  suit le plan : les deux moitiés d'une coupe (`split_panel`), une copie (`duplicate_panel`), le plan léger des exports
  (`exportrender::plan_leger`), les plans annulés (`planannule.rs`). Entre deux versions d'un plan (`planvers::echanger`),
  elle va avec la caméra et les calques, donc avec la version.
- **Le fichier** : `"espace": true` dans l'entrée du plan (`project::PanelEntry`, la même pour le projet classique, la
  copie de secours et le fichier d'un plan de projet partagé), absent si faux. **Format 54** (`project::VERSION`) : une
  version d'avant refuse le projet. Si `main` prend le numéro 54 pour autre chose avant la fusion, celui-ci devient 55.
- **La commande** : `espace3d.plan_en_espace` (et non `plan.espace` : elle vit avec les autres commandes de l'Espace 3D),
  dans la palette et `keymap.toml` (sans touche), et un bouton à bascule dans Réglages de l'Espace 3D (icône cube
  transparent). Une étape d'annulation « Plan en Espace 3D ». Le message de la barre d'état et la bulle disent que l'image
  ne change pas encore.
- **Le refus** : `Document::accepte_plan_en_espace` (le cadre tient dans la feuille), avec un message.
- **Pas fait** : l'entrée dans le menu du plan (elle viendra avec A3, quand la case aura un effet visible).
- **Essais** (`espace3d_tests.rs`, 4 de plus) : le fichier (rien d'écrit pour un plan ordinaire, champ écrit et relu,
  projet du format 53 ouvert avec les mêmes pixels) ; une étape d'annulation et **le même plan d'image, les mêmes pixels**
  case cochée ; couper, dupliquer, échanger deux versions, plan neuf ; refus quand le cadre dépasse la feuille.
- **Essais existants touchés** : le numéro de format épinglé (53 → 54, et « plus récent » 54 → 55) dans
  `camera_retours_tests`, `corbeille_tests`, `partage_tests`, `petits_livrables_tests`, `pipeline_tests`,
  `planannule_tests`, comme à chaque nouveau format ; les deux gardes `#[cfg(windows)]` de A0, point 6.
- **Suite complète sur le Mac** (2026-11-11, avec les remplaçants de A0, point 6) : 4 412 essais passent, 86 échouent,
  164 sont ignorés. Le code d'avant A1, dans les mêmes conditions : 4 407 passent, 87 échouent. **Aucun essai n'échoue à
  cause de A1** : les 86 échecs sont les mêmes avant et après (propres au Mac) ; le 87e d'avant
  (`recolorsel_tests::engine_strokes_are_replayed_by_the_worker…`) passe après, sans rapport (un essai de minutage).
  La suite de référence reste celle du poste Windows (`scripts/verifier.ps1`), pas encore lancée sur cette branche.
- **Rien n'a été regardé à l'écran** (FEANOR n'a pas été lancé). À voir une fois : le bouton dans Réglages de l'Espace 3D,
  sa bulle, son état coché, le message de la barre d'état, Ctrl+Z.

### A2. Les maths

Dans `espace3d.rs` : le déplacement `C` d'une caméra (`Monde::repere_camera`), les variantes de `card_warp`,
`card_to_sheet`, `sheet_to_card` qui prennent une matrice au lieu d'un `Xf`, la caméra sans `limited` ni borne de
distance. Essais (`espace3dmath_tests.rs`) : caméra droite = identité ; aller-retour d'un point à 0,05 px ; caméra
tournée de 90° et de 180° ; caméra derrière le plan z = 0 ; carte derrière la caméra écartée ; carte à 90°.

#### Fait (2026-11-11)

Des maths seulement, dans `espace3d.rs` : **rien ne les appelle encore**, l'image et l'interface ne changent pas.

- `Monde::repere_camera(cam)` : la matrice du changement de repère (partie 2). La caméra est prise telle quelle, sans
  `limited` ni borne de distance ; mêmes conventions que `Camera::homography` (panoramique, bascule, rotation, œil de
  `Monde::eye`) ; le zoom y est compté. Caméra de référence : l'identité.
- `Monde::eye_reference`, `Monde::camera_reference` : l'œil et la caméra de référence (au centre de la feuille, de face).
- `Monde::carte_dans_le_repere(cam, xf)` : la matrice d'une carte dans ce repère ; `Monde::to_frame(cam, p)` : où un
  point du monde tombe dans l'image, caméra libre.
- `card_warp_m`, `card_to_sheet_m`, `sheet_to_card_m` : les variantes à matrice. Les fonctions d'avant (à `Xf`) les
  appellent : mêmes calculs, mêmes maillages (essai).
- **Essais** (`espace3d_tests.rs`, 5 de plus) : caméra de référence = identité ; **7 caméras dans les limites d'avant
  filment la même image par le repère que par le chemin d'avant, à 0,05 px près** (feuille, hors feuille, carte
  inclinée) ; caméra tournée de 90° et de 180°, basculée de 85°, passée derrière la feuille (vue en miroir), zoom ;
  aller-retour carte ↔ feuille pour une carte inclinée, **couchée au sol (90°)** et en mur ; carte derrière la caméra
  sans maillage, carte traversée par la caméra posée pour sa moitié visible.
- **Suite complète sur le Mac** : 4 417 passent, 86 échouent (les mêmes 86 qu'avant A2, propres au Mac).
- **Pas dans A2** : l'inclinaison d'une carte reste bornée à ±85° dans `LayerTransform` (les essais à 90° passent par
  un `Xf`) ; la borne sera levée pour les plans en Espace 3D en A6, avec les surfaces au sol et en mur.

### A3. Le rendu

`espace3d::place` et `render::panel_plan_at` prennent le chemin de la partie 2 quand `espace` est vrai. Une seule
fonction dit « ce que filme ce plan à cette image » (le `Plan` et la caméra de référence) ; l'écran, la lecture, les
vignettes et l'export l'appellent tous. Essais : case cochée sans rien bouger = mêmes pixels ; processeur contre carte
graphique ≤ 2 niveaux ; l'export compose la même image que l'écran ; vignette = composition ; parallaxe gardée.

#### Fait (2026-11-11)

**Cocher la case change maintenant l'image, dès que la caméra du plan n'est plus la caméra de référence.**

- **Poser les cartes** : `espace3d::place_dans(…, repere)` ; `place` l'appelle sans repère (inchangé). Avec un repère,
  chaque carte, à plat ou inclinée, est passée dans le repère de la caméra et posée par un maillage vu de l'œil de
  référence. Les deux constructeurs de plan (`render::plan_with_camera_layers`, `App::build_plan_avec`) lui passent
  `Document::repere_du_plan` (ou `repere_du_plan_range` pour la version rangée d'un plan).
- **Cadrer** : plutôt qu'une fonction de plus à appeler dans une douzaine de fichiers, c'est `Document::panel_camera`
  (« la caméra montrée : tout ce qui montre ou exporte passe par ici ») qui rend la caméra de référence pour un plan en
  Espace 3D, et `camera_du_plan_range` de même. Tout ce qui filme (exports, lecture, vignettes, planche, comparaison,
  image fixe) est donc juste sans y toucher, et le code à venir aussi. Ce qui **décrit** la caméra du plan lit
  `panel_camera_reelle` : la fiche technique, l'export de caméra, la vue de dessus, le tracé du chemin de la caméra dans
  l'Espace 3D, le point de départ d'un mouvement de caméra des Vignettes. Ce qui la **règle** (`panel_camera_douce`,
  `current_camera`) n'a pas changé.
- **L'écran** : `App::camera_montree` rend la caméra de référence (le cadre est au centre de la feuille, qui montre ce
  que la caméra filme). L'outil Caméra du canevas ne tire plus le cadre dans un plan en Espace 3D : un message renvoie à
  la vue Espace 3D. `set_current_camera` ne bride plus la caméra d'un plan en Espace 3D (`Camera::limited`).
- **Le pinceau** (`App::to_layer`, `layer_to_screen`) passe par le repère : le trait tombe sur la carte, caméra tournée
  (avancé de l'étape A5).
- **Calque accroché à la caméra** : il ne reçoit pas le changement de repère, il reste dans le cadre.
- **Garantie corrigée par rapport à la partie 4** : les pixels sont **identiques** quand la caméra du plan est la caméra
  de référence (pas de repère, le chemin de toujours). Avec une autre caméra, restée dans les limites d'avant, l'image
  a la **même géométrie** mais pas les mêmes pixels au bord des formes (une interpolation au lieu de deux) : 152 pixels
  sur 57 600 dans l'essai.
- **Essais** (`espace3d_tests.rs`, 6 de plus) : caméra qui cadre, caméra réelle et repère ; image gardée pour une caméra
  dans les limites d'avant (moins de 1 % de pixels à plus de 48 niveaux) ; caméra passée derrière la feuille, qui voit
  la carte de dos, en miroir, et quart de tour qui la sort du champ ; **l'export et un plan rangé composent la même image
  que l'écran, pixel pour pixel**, caméra tournée de 55° ; calque accroché ; plan de l'écran = plan des sorties, pinceau
  aller-retour à 0,05 px, caméra non bridée.
- **Suite complète sur le Mac** : 4 422 passent, 87 échouent. Les 86 d'avant, plus
  `horschamp_tests::a_stroke_outside_every_field…`, qui passe seul trois fois de suite (instable sous la charge de la
  suite entière, sans rapport).
- **Limites de A3, à lever ensuite** :
  - l'ordre reste celui de la pile (A4) ;
  - une carte ne montre que ce qui est dans la feuille : le dessin hors feuille (toile infinie) n'est pas filmé dans un
    plan en Espace 3D dont la caméra a bougé ;
  - les outils 2D (sélection, Transformer, gomme de débord, multisélection) posent leurs boîtes comme sous la caméra de
    référence : justes seulement tant que la caméra n'a pas bougé ;
  - les poignées de la vue Espace 3D gardent leur borne de bascule (±89°) ; les mouvements de caméra des Vignettes et la
    vue de dessus ne sont pas encore désactivés pour ces plans (A8) ;
  - processeur contre carte graphique : pas d'essai propre aux plans en Espace 3D (le chemin est celui des cartes
    inclinées, déjà couvert) ;
  - **rien n'a été regardé à l'écran**.

### A4. Ce qui est devant cache ce qui est derrière

Pour un plan en Espace 3D, les éléments du plan sont ordonnés du plus loin au plus près de la caméra, à chaque image ; à
distance égale, l'ordre de la pile. Un calque et ce qui lui est attaché (couleur, fond sous le trait) restent ensemble.
Un calque d'ajustement et un mode de fusion agissent sur ce qui est plus loin qu'eux. Essais : deux cartes qui échangent
leur place quand la caméra fait le tour ; pile gardée à égalité ; couleur attachée à son trait ; ajustement.

#### Fait (2026-11-11)

**Dans un plan en Espace 3D, ce qui est devant cache ce qui est derrière, sans ranger la pile.**

- `espace3d::ordre_par_distance` donne l'ordre de pose, du plus loin de la caméra au plus près ; les deux constructeurs
  de plan l'appellent quand `Panel::espace` est vrai (caméra de référence comprise), **avant** leur boucle. Un plan
  ordinaire ne passe pas par là.
- **La distance** d'un calque est celle du centre de sa carte, le long de l'axe de la caméra.
- **Même plan, ordre de la pile.** C'est plus fort que le « à distance égale » prévu : dès que la caméra tourne, les
  centres de deux cartes posées dans le même plan (même profondeur, même inclinaison) n'ont plus la même distance. Un
  tri sur le centre seul aurait fait passer la couleur devant son trait. Les cartes d'un même plan forment donc un
  groupe, qui a une seule distance (la moyenne) : elles gardent l'ordre de la pile, où que soit la caméra. Un calque de
  couleur suit ainsi son trait sans cas particulier (tant qu'ils ont la même pose).
- **Dossier** : d'un seul tenant, à la distance de son calque le plus loin ; ses calques ordonnés entre eux de même.
- **Calque d'ajustement** : il a sa place dans l'ordre (sa profondeur) et agit sur ce qui est plus loin que lui.
  `calqueffet::Portees` donne sa portée dans les deux constructeurs : « depuis le premier calque **posé** de son
  dossier » (hors dossier : depuis le début). `marker` est inchangé pour un plan ordinaire.
- **Annotations** : toujours posées après tout le reste.
- **Essais** (`espace3d_tests.rs`, 4 de plus) : la carte proche cache la carte loin quoi que dise la pile, et l'ordre
  s'inverse quand la caméra passe derrière (pixels) ; trois cartes d'un même plan gardent l'ordre de la pile caméra
  tournée à +40° et −40°, cartes inclinées pareil comprises ; dossier en bloc, annotation en dernier, plan ordinaire
  dans l'ordre de la pile ; calque d'ajustement à la racine (n'inverse que la carte derrière lui, pixels) et dans un
  dossier (portée depuis le premier calque posé). L'écran pose dans le même ordre que les sorties.
- **Suite complète sur le Mac** : 4 426 passent, 87 échouent : les 86 du Mac, plus
  `colorscript::tests_sortie::cancelling_leaves_no_file…`, qui passe seul trois fois de suite (instable sous charge ;
  ce n'est pas le même essai qu'à A3, qui repasse).
- **Limites** :
  - deux cartes qui se traversent, ou un personnage debout sur une carte au sol, ne sont pas départagés point par
    point : l'une passe entière devant l'autre (partie 5) ;
  - un dossier passe en bloc : une carte hors dossier située entre deux calques du dossier est devant ou derrière tout
    le dossier ;
  - la vue d'édition de l'Espace 3D trace encore les cartes dans l'ordre de la pile (A7) ; « Ordonner la pile selon la
    profondeur » reste utile pour un plan ordinaire, sans effet sur l'image d'un plan en Espace 3D ;
  - cocher la case change l'image d'un plan dont la pile et les profondeurs ne disent pas la même chose : c'est le but,
    mais ce n'est plus « rien ne change » ;
  - **rien n'a été regardé à l'écran**.

### A5. Dessiner par la caméra

`App::to_layer` (aujourd'hui `tilted_card` + `sheet_to_card`) passe par le repère de la caméra pour un plan en Espace 3D.
Dans un plan en Espace 3D, la vue de dessin est « Regarder par la caméra ». Essais : aller-retour du pinceau sur une carte
de face, inclinée, au sol, derrière une autre ; un trait sur une carte derrière la caméra est refusé.

#### Fait (2026-11-11)

Le passage du pinceau par le repère de la caméra est fait depuis A3 (`App::to_layer`). A5 ajoute ce qui manquait.

- **La vue de dessin** : rien à faire. Dans un plan en Espace 3D, la feuille montre déjà ce que la caméra filme, le
  cadre au centre (A3) : on y dessine comme dans un plan ordinaire. « Regarder par la caméra » de la vue Espace 3D reste
  une vue de réglage, pas de dessin.
- **Pas de trait là où il n'y a pas de carte** : `App::to_layer_opt` ne rend rien quand le regard ne touche pas la carte
  du calque courant (derrière la caméra, vue par la tranche). Le pinceau, le crayon, la gomme, le pot et le dégradé ne
  commencent pas (`canvas.rs`), avec un message ; pendant un trait, les points qui quittent la carte sont laissés.
  Avant, le trait partait à un endroit sans rapport.
- **L'horizon** : dans un plan en Espace 3D, `to_layer_opt` refuse aussi un point à plus d'une feuille des bords de la
  carte. Sans cela, près de l'horizon d'une carte au sol, un petit geste ferait un trait immense. Un plan ordinaire
  n'est pas borné (inchangé).
- **La taille du pinceau** : `App::layer_zoom` rend, pour un plan en Espace 3D, la taille qu'un pixel de la carte a à
  l'écran sous le pointeur (au centre de la carte sinon). Le curseur a la taille du trait qu'il va laisser : deux fois
  plus fin sur une carte deux fois plus loin. Les tolérances des autres outils (sélection de couleur, cutter, dégradé)
  lisent la même valeur.
- **Essais** (`espace3d_tests.rs`, 4 de plus) : aller-retour du pinceau à 0,05 px de face, sur une carte inclinée
  caméra tournée, sur une carte presque au sol (80°) ; **un vrai trait de souris laisse de l'encre** sur la carte caméra
  tournée de 30°, et **n'en laisse aucune** carte derrière la caméra, avec le message ; près de l'horizon, jamais plus
  d'une feuille au-delà des bords ; taille du pinceau divisée par deux à distance double, rétablie par un zoom double.
- **Suite complète sur le Mac** : 4 430 passent, 87 échouent : les 86 du Mac, plus
  `enregistrement_tests::the_application_is_idle_again_once_the_save_is_done`, qui passe seul trois fois de suite
  (instable sous charge, un troisième essai différent de ceux de A3 et A4, qui repassent).
- **Limites** :
  - les autres appels de `to_layer` (coller une image, formes, trait droit, trajectoire) gardent l'ancien repli quand le
    regard manque la carte ;
  - la carte au sol à 90° exactement attend l'étape A6 (borne des ±85°) ;
  - on peint sur le calque courant même s'il est caché derrière une autre carte : c'est voulu, mais rien ne le dit à
    l'écran ;
  - **rien n'a été regardé à l'écran**, en particulier le curseur du pinceau.

### A6. Poser une surface

Trois commandes, chacune crée un calque et le choisit, en une étape d'annulation : « Nouvelle surface face à la caméra »
(à la distance choisie, couvre le cadre avec une marge), « Nouvelle surface au sol », « Nouvelle surface en mur » (face
à la caméra, verticale, posée au sol). Inclinaison libre jusqu'à 90°. Bulles, palette, `keymap.toml` sans touche par
défaut. Essais : la surface face à la caméra remplit le cadre ; le sol passe par y = sol ; annulation.

#### Fait (2026-11-12)

- **Trois commandes**, chacune un calque neuf en une étape d'annulation : `espace3d.surface_face`, `espace3d.surface_sol`,
  `espace3d.surface_mur` (`App::espace3d_nouvelle_surface`). Le calque est créé juste au-dessus du calque courant, du
  même genre que lui (vectoriel ou bitmap), il devient le calque courant et la carte choisie. Noms : « Surface face à la
  caméra », « Surface au sol », « Surface en mur », numérotés s'il y en a déjà. Refusé hors d'un plan en Espace 3D (le
  message dit de cocher la case) et storyboard verrouillé.
- **Où elles se posent** (`Monde::surface_xf`, la caméra réelle du plan à l'image courante) : face à la caméra, sur son
  axe, avec sa bascule et sa rotation ; au sol, devant elle à l'horizontale, le haut de la carte vers le fond ; en mur,
  debout sur le sol, de face à l'horizontale. **Taille** : celle de la feuille, à l'échelle où un pixel du dessin fait
  un pixel de l'image à cette distance : de face, le cadre tombe en son centre avec la marge de la feuille autour.
- **La distance** : un champ en mètres à côté des trois boutons (Réglages de l'Espace 3D, montrés seulement dans un plan
  en Espace 3D) ; par défaut la distance d'origine de la caméra, celle de la feuille.
- **Le sol** (`Monde::sol_y`) : le bas du cadre de la caméra de référence. Le quadrillage de la vue Espace 3D est tracé
  à cette hauteur pour un plan en Espace 3D (il reste à la hauteur du centre de la feuille pour un plan ordinaire).
- **L'inclinaison est libre dans un plan en Espace 3D** : jusqu'au demi-tour (`MAX_TILT_LIBRE`, `clamp_tilt_libre`,
  `card_from_xf_libre`, `apply_card_xf_libre`) pour les surfaces, les poignées et les champs de Réglages. Un plan
  ordinaire garde ±85°. Ce qui se lit d'un fichier (pose, clés de position) est borné au demi-tour : un plan ordinaire
  n'écrit jamais plus de 85°, rien ne change pour lui.
- **Limite trouvée** : une carte ne peut pas être derrière la caméra d'origine du plan. La profondeur d'un calque
  s'arrête à −0,9 (`Layer::MIN_DEPTH`) et sa taille en dépend : la scène se construit dans le demi-espace devant la
  caméra de référence. La caméra, elle, va partout. Une surface qui tomberait derrière est refusée, avec un message.
  Lever cette limite demande de séparer la taille d'une carte de sa profondeur : à voir avec l'étape B.
- **Essais** (`espace3d_tests.rs`, 5 de plus) : de face à la caméra de référence, la surface est la feuille ; **le cadre
  d'une caméra tournée (35°, −50° avec zoom et rotation, 120° passée derrière la feuille) tombe aux quatre coins prévus
  de la surface, à 0,3 px** ; au sol : quatre coins à la hauteur du sol, couchée à 90°, haut vers le fond ; en mur :
  pied au sol, verticale, de face ; la commande (refus, calque courant, noms, annulation, **pinceau sur le sol à 90°**,
  enregistrement qui garde les 90°) ; borne à 85° dans un plan ordinaire, libre dans un plan en Espace 3D.
- **Suite complète sur le Mac** : 4 436 passent, 86 échouent, les 86 du Mac, deux passes de suite. (Une première passe
  a compté un échec de plus, `enregistrement_tests::the_application_is_idle_again…`, un essai de minutage qui passe seul
  et avec la famille Espace 3D en parallèle.)
- **Pas fait** : les trois commandes n'ont pas de touche ; pas d'entrée dans le menu des calques ; les poignées près de
  ±90° peuvent sauter (angles d'Euler, `ESPACE3D.md` 11.5) ; **rien n'a été regardé à l'écran**, les trois icônes
  (cadre, alignement bas, mur) comprises.

### A7. La vue Espace 3D

Pour un plan en Espace 3D : cartes tracées du plus loin au plus près de l'œil de la vue ; poignées de la caméra sans les
limites (`MAX_TILT`, `limited`) ; champs de Réglages ; message de la barre d'état. Essais : choix au rayon, caméra
tournée de 180° par la poignée, repos de l'application.

### A8. Finitions

Ce qui est désactivé pour un plan en Espace 3D (partie 5) dit pourquoi. Essai de budget (20 cartes, écran et lecture).
`docs/ESPACE3D.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `INTERFACE.md` (liste de contrôle à l'écran), README.

## 7. À trancher par Anthony

| Question | Ce que je propose |
|---|---|
| Le nom à l'écran | **Tranché par Anthony le 2026-11-11 : « Espace 3D ».** Le mode porte le nom de la vue ; pour ne pas confondre les deux, le bouton et les messages disent « Plan en Espace 3D » (le plan est en Espace 3D), la vue reste « Espace 3D ». Avant ce choix, les documents disaient « plan en espace ». |
| L'ordre par distance est-il automatique dans un plan en Espace 3D ? | Oui. C'est ce qui rend le dessin dans un décor naturel ; l'ordre de la pile ne départage plus que les égalités. |
| Les dossiers dans un plan en Espace 3D | Un dossier se déplace en bloc dans l'ordre (à la distance de sa carte la plus loin), ses calques ordonnés entre eux. Confirmé possible par A0, point 3. |
| Dessiner depuis la vue d'orbite (pas seulement par la caméra) | Pas dans l'étape A. |
| Avant ou après la V1 | `suivi/V1.md` range le moteur 3D en V2. A0 et A1 ne coûtent presque rien et ne montrent rien ; A2 à A8 peuvent attendre la V1. |

## 8. Licence

StoryLiner est sous licence GPL. On reprend des idées d'usage, aucun code, aucune icône, aucun visuel. Dans un texte
public, ne pas le nommer.
