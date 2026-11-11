# Plan en espace : dessiner dans un décor que la caméra traverse (étape A)

Branche `espace-libre` (depuis `main` 7d8a7d2), 2026-11-11. **Plan validé par Anthony le 2026-11-11 pour l'étape A0
(vérifications), faite le même jour : résultats en partie 6. Les décisions de la partie 7 restent à trancher avant A1.
Étape A1 (la case « Plan en espace », sans effet sur l'image) faite le 2026-11-11 à la demande d'Anthony, avec les
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

Avec un **plan en espace** :

- la caméra va où elle veut : elle tourne sur elle-même, fait le tour d'un personnage, passe entre deux cartes, regarde
  derrière elle ;
- une carte peut être couchée au sol ou dressée comme un mur ;
- ce qui est devant cache ce qui est derrière, sans avoir à ranger la pile des calques ;
- on crée d'un geste une surface à dessiner « face à la caméra, à telle distance », « au sol » ou « en mur » ;
- on dessine en regardant par la caméra, et le trait tombe sur la carte choisie, quelle que soit son orientation.

Ce qui ne change pas : un plan ordinaire reste un plan ordinaire. Le plan en espace est une case à cocher, plan par plan.
Les projets existants s'ouvrent comme avant, avec les mêmes pixels.

**Coût** : 8 étapes (partie 6), chacune avec ses essais et fusionnable seule. Ordre de grandeur, à affiner après l'étape
A0 : deux à trois semaines de séances.
**Risque principal** : le nombre d'endroits du logiciel qui demandent « que filme ce plan ? » (écran, lecture, vignettes,
exports, planche). Tous doivent répondre pareil pour un plan en espace. L'étape A0 les compte avant tout code.
**Réversibilité** : complète. Décocher la case rend le plan ordinaire ; retirer la fonction ne touche pas aux plans
ordinaires.

## 2. L'idée : la caméra ne bouge pas, c'est le monde qui tourne devant elle

Le rendu actuel se fait en deux temps (`docs/ESPACE3D.md`, 5) : les cartes se posent sur la feuille, vues par l'œil de la
caméra, puis la caméra cadre la feuille. Il faut donc que la caméra voie encore la feuille (`Camera::limited`,
`MIN_GRAZING` = 5°) : c'est de là que viennent toutes les limites.

Pour un plan en espace, on ne construit **pas de second moteur d'image**. On change de repère :

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
  Pour un plan en espace, on ne passe plus par `limited` ni par la borne de `Camera::distance` (la caméra peut passer
  derrière le plan z = 0). Les clés (`camkeys.rs`), la saccade et les effets vivants (`camfx.rs`) lisent les mêmes champs.
- L'inclinaison d'une carte (`LayerTransform::clamp_tilt`, ±85°) est libre jusqu'à ±90° dans un plan en espace.
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
| Vue de dessus (`topview.rs`), export de la caméra (`exportcamera.rs`), mouvements de caméra des Vignettes (`vignettecam.rs`) pour un plan en espace | désactivés pour ces plans, avec un mot d'explication ; à reprendre après |
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
   (`Document::frame` ≤ `width` × `height`) ; sinon, prévoir l'agrandissement de la zone pour les plans en espace.
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
  caméra de référence pour un plan en espace et `panel_camera` sinon ; la douzaine de fichiers qui filment y passent. Un
  oubli se voit (l'image serait cadrée deux fois) : l'essai « chaque sortie = l'écran » de A3 parcourt toutes les sorties.

**2. La zone filmée tient dans ce qui se compose.** La feuille fait au moins le double du cadre à la création
(`Document::with_frame`) et après un changement de taille (`resize_frame`) ; un dessin posé par un maillage compte pour
« la feuille entière » dans `plan_ink_bounds_sauf`. Avec la focale comptée dans la projection, la zone filmée est le
cadre au centre de la feuille : rien à agrandir. Non vérifié : un très ancien projet dont la feuille serait plus petite
que le cadre ; la case « Plan en espace » est refusée dans ce cas (essai en A1).

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
montrerait un plan en espace à plat, avec une autre image. **Donc `VERSION` 53 → 54, dès A1.**

**5. Budget : tenu sur le Mac, à confirmer sur le poste Windows.** L'essai
`the_plan_of_an_image_with_twenty_tilted_cards_stays_within_the_frame_budget` (`espace3d_tests.rs` : 20 cartes
inclinées, cadre 1920 × 1080, seuil 8 ms) donne **1,14 ms pour les 20 cartes** (0,057 ms par carte) sur le Mac d'Anthony
(Apple Silicon, 2026-11-11). Le poste Windows est la référence du projet : y lancer
`cargo test --lib twenty_tilted_cards -- --nocapture` et noter ici la ligne `PERF`.

**6. Trouvé en chemin.**

- *Calque accroché à la caméra* (format 53, `accroche.rs`, du jour même) : dans un plan en espace, c'est une carte qui ne
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

### A1. La case « Plan en espace »

`Panel::espace`, lecture et écriture (classique, en arrière-plan, projets partagés), commande `plan.espace` (palette,
menu du plan, panneau Réglages de l'Espace 3D), une étape d'annulation « Plan en espace ». Aucun effet sur l'image.
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
  transparent). Une étape d'annulation « Plan en espace ». Le message de la barre d'état et la bulle disent que l'image
  ne change pas encore.
- **Le refus** : `Document::accepte_plan_en_espace` (le cadre tient dans la feuille), avec un message.
- **Pas fait** : l'entrée dans le menu du plan (elle viendra avec A3, quand la case aura un effet visible).
- **Essais** (`espace3d_tests.rs`, 4 de plus) : le fichier (rien d'écrit pour un plan ordinaire, champ écrit et relu,
  projet du format 53 ouvert avec les mêmes pixels) ; une étape d'annulation et **le même plan d'image, les mêmes pixels**
  case cochée ; couper, dupliquer, échanger deux versions, plan neuf ; refus quand le cadre dépasse la feuille.
- **Essais existants touchés** : le numéro de format épinglé (53 → 54, et « plus récent » 54 → 55) dans
  `camera_retours_tests`, `corbeille_tests`, `partage_tests`, `petits_livrables_tests`, `pipeline_tests`,
  `planannule_tests`, comme à chaque nouveau format ; les deux gardes `#[cfg(windows)]` de A0, point 6.
- **Rien n'a été regardé à l'écran** (FEANOR n'a pas été lancé). À voir une fois : le bouton dans Réglages de l'Espace 3D,
  sa bulle, son état coché, le message de la barre d'état, Ctrl+Z.

### A2. Les maths

Dans `espace3d.rs` : le déplacement `C` d'une caméra (`Monde::repere_camera`), les variantes de `card_warp`,
`card_to_sheet`, `sheet_to_card` qui prennent une matrice au lieu d'un `Xf`, la caméra sans `limited` ni borne de
distance. Essais (`espace3dmath_tests.rs`) : caméra droite = identité ; aller-retour d'un point à 0,05 px ; caméra
tournée de 90° et de 180° ; caméra derrière le plan z = 0 ; carte derrière la caméra écartée ; carte à 90°.

### A3. Le rendu

`espace3d::place` et `render::panel_plan_at` prennent le chemin de la partie 2 quand `espace` est vrai. Une seule
fonction dit « ce que filme ce plan à cette image » (le `Plan` et la caméra de référence) ; l'écran, la lecture, les
vignettes et l'export l'appellent tous. Essais : case cochée sans rien bouger = mêmes pixels ; processeur contre carte
graphique ≤ 2 niveaux ; l'export compose la même image que l'écran ; vignette = composition ; parallaxe gardée.

### A4. Ce qui est devant cache ce qui est derrière

Pour un plan en espace, les éléments du plan sont ordonnés du plus loin au plus près de la caméra, à chaque image ; à
distance égale, l'ordre de la pile. Un calque et ce qui lui est attaché (couleur, fond sous le trait) restent ensemble.
Un calque d'ajustement et un mode de fusion agissent sur ce qui est plus loin qu'eux. Essais : deux cartes qui échangent
leur place quand la caméra fait le tour ; pile gardée à égalité ; couleur attachée à son trait ; ajustement.

### A5. Dessiner par la caméra

`App::to_layer` (aujourd'hui `tilted_card` + `sheet_to_card`) passe par le repère de la caméra pour un plan en espace.
Dans un plan en espace, la vue de dessin est « Regarder par la caméra ». Essais : aller-retour du pinceau sur une carte
de face, inclinée, au sol, derrière une autre ; un trait sur une carte derrière la caméra est refusé.

### A6. Poser une surface

Trois commandes, chacune crée un calque et le choisit, en une étape d'annulation : « Nouvelle surface face à la caméra »
(à la distance choisie, couvre le cadre avec une marge), « Nouvelle surface au sol », « Nouvelle surface en mur » (face
à la caméra, verticale, posée au sol). Inclinaison libre jusqu'à 90°. Bulles, palette, `keymap.toml` sans touche par
défaut. Essais : la surface face à la caméra remplit le cadre ; le sol passe par y = sol ; annulation.

### A7. La vue Espace 3D

Pour un plan en espace : cartes tracées du plus loin au plus près de l'œil de la vue ; poignées de la caméra sans les
limites (`MAX_TILT`, `limited`) ; champs de Réglages ; message de la barre d'état. Essais : choix au rayon, caméra
tournée de 180° par la poignée, repos de l'application.

### A8. Finitions

Ce qui est désactivé pour un plan en espace (partie 5) dit pourquoi. Essai de budget (20 cartes, écran et lecture).
`docs/ESPACE3D.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `INTERFACE.md` (liste de contrôle à l'écran), README.

## 7. À trancher par Anthony

| Question | Ce que je propose |
|---|---|
| Le nom à l'écran | « Plan en espace » |
| L'ordre par distance est-il automatique dans un plan en espace ? | Oui. C'est ce qui rend le dessin dans un décor naturel ; l'ordre de la pile ne départage plus que les égalités. |
| Les dossiers dans un plan en espace | Un dossier se déplace en bloc dans l'ordre (à la distance de sa carte la plus loin), ses calques ordonnés entre eux. Confirmé possible par A0, point 3. |
| Dessiner depuis la vue d'orbite (pas seulement par la caméra) | Pas dans l'étape A. |
| Avant ou après la V1 | `suivi/V1.md` range le moteur 3D en V2. A0 et A1 ne coûtent presque rien et ne montrent rien ; A2 à A8 peuvent attendre la V1. |

## 8. Licence

StoryLiner est sous licence GPL. On reprend des idées d'usage, aucun code, aucune icône, aucun visuel. Dans un texte
public, ne pas le nommer.
