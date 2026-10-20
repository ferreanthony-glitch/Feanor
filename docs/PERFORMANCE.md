# Performance de SHOT

Passe de performance du 2026-10-19 (branche `perf`, partie de `main` c693643). Demande d'Anthony :
« avancer sur l'optimisation du soft et le rendre le plus performant possible rapidement », avec la
règle qui prime sur tout : le logiciel répond toujours instantanément, sans latence.

Mise à jour du 2026-10-19 : la branche a été fusionnée avec `main` 31aef6e (effets de calque en calcul
de fond, commentaires, boîte de sélection, caméra, tremblement du trait, thèmes) dans `perf-integre` ;
voir « Après la fusion » plus bas. Les chiffres des tableaux ci-dessous sont ceux d'avant la fusion.

Mise à jour du 2026-10-19 (branche `perf2`, partie de 8b49066) : les leviers 1, 2 et 3 de la liste
(section 6) sont faits — **enregistrer en arrière-plan**, **textures de dessin à la taille du contenu**,
**image composée gardée** ; voir la section 9 (mesures avant / après, conception, risques). Les
chiffres des tableaux 2 et suivants restent ceux de la première passe.

Mise à jour du 2026-10-19 (branche `vignettes-fond`, partie de `main` 31b86f1) : le levier 4,
**les vignettes hors du fil de l'interface** (montage, planche, miniatures de la timeline et du panneau
Calques), est fait ; voir la section 11 (conception, mesures avant / après, risques, à regarder à l'écran).

Mise à jour du 2026-10-19 (branche `effets-gpu`, partie de 31b86f1) : les leviers 6 et 7 (**effets sur la
carte graphique**) sont faits pour tous les effets sauf le tremblement du trait ; voir la section 12
(conception, écarts avec le processeur, mesures avant / après, risques). Les effets d'un calque et les
calques d'ajustement se dessinent à pleine résolution, à chaque image, sur la carte ; le calcul de fond
sur processeur reste pour l'export, les vignettes, le repli sans carte et le réglage éteint.

Mise à jour du 2026-10-20 (branche `effets-trait`, partie de `main` 7562800) : **le tremblement du trait passe sur la
carte graphique** (plus aucun effet « ordinaire » ne force une pile sur le processeur ; sections 12.3 et 13.6), et six
**effets du trait** (écriture, crayonné multiple, respiration, ondulation, pointillés, désagrégation) lisent la
géométrie des traits en calcul de fond ; voir la section 13 (conception, mesures sur 2 000 traits, règle de mélange
réduite, risques, à regarder à l'écran).

Mise à jour du 2026-10-20 (branche `exports-fond`, partie de `main` f8b9222) : **les exports ne calculent plus aucune image sur le fil de
l'interface** et ne verrouillent plus le projet (film, GIF, images, image courante, PDF, page HTML, son, montage, relecture, fiche
technique, Storyboard Pro) ; voir la section 17 (conception, mesures avant / après, garde-fous, risques, à regarder à l'écran).

Ce document dit comment mesurer, ce qui a été mesuré avant et après, ce qui a été changé, ce qui reste
(classé), et ce qu'il faut regarder à l'écran.

## 1. Mesurer

### La superposition « Performance »

`Ctrl+K`, taper « Performance » (action `vue.performance`), ou **Affichage > Performance (temps de
l'interface)**. Un cadre noir semi-transparent apparaît en haut à gauche du dessin ; il ne prend aucun
clic. Éteint (même action), le profileur ne coûte qu'un test de booléen par section.

Il montre, sur les 60 dernières images d'écran :

- le nombre d'images par seconde (**0 au repos** : SHOT ne repeint pas pour rien) ;
- le temps de construction de l'interface (`App::frame`), moyenne et pire ;
- l'intervalle entre deux images et ce qui reste (découpage en triangles, peinture par la carte
  graphique, attente), quand SHOT repeint sans cesse ;
- l'arrière-plan en attente (rejeux de traits, fonds sous le trait, vignettes) ;
- **les sections**, du plus au moins coûteux, en ms (moyenne / pire). Elles s'emboîtent : un temps est
  *inclusif* (« Dessin » contient « composition : envoi à la carte graphique », elle-même contenue
  dans « panneaux (dock, total) »). Une section par panneau (Dessin, Montage, Timeline, Calques,
  Couleur, Pinceaux…), les barres, les fenêtres, la disposition, et, dans le dessin, l'envoi à la
  carte graphique (`gpu : tuiles copiées`, `tuiles envoyées`, `textures créées`) ;
- **les réveils** : fichier et ligne de chaque `request_repaint` qui a provoqué une image, pour
  trouver pourquoi SHOT repeint au repos. « entrée » veut dire souris, clavier ou fenêtre. La
  superposition elle-même se réveille deux fois par seconde (`perf.rs`), ce n'est pas un défaut.

Pour ajouter une section : `let _t = crate::perf::section("nom");` (une constante), jusqu'à la fin de
la portée.

### Le banc sans fenêtre (`src/perf_tests.rs`)

`App::sans_fenetre` construit l'application complète, `App::frame` construit toute l'interface comme
à l'écran (1920 × 1080), egui la découpe, et un périphérique wgpu sans fenêtre (la carte de la
machine) reçoit les textures et peint l'image : le dessin est composé par `gpu.rs`, comme à l'écran.
Les réglages d'un essai s'écrivent dans un dossier temporaire (`donnees::bac_a_sable`), jamais dans
ceux d'Anthony. Les essais `#[ignore]` s'affichent avec :

```
cargo test --release --offline --target-dir target\msvc perf_ -- --ignored --nocapture --test-threads=1
```

(ou un seul : `perf_long_project`, `perf_heavy_plan`, `perf_panels`, `perf_scale`,
`perf_vector_drawing`, `perf_project_open_save`, `perf_cel_io_steps`, `perf_thumbnail_cost`,
`perf_brush_library_startup`, `perf_cpu_bench`, et, depuis `perf2`, `perf_save_ui_thread` (Ctrl+S) et
`perf_gpu_textures` (mémoire de la carte, défilement, lecture, repos), et, depuis `vignettes-fond`,
`perf_montage_thumbnails_ui_thread` (le fil de l'interface pendant que les vignettes d'un montage de 600 plans
se font) et `perf_thumbnail_request_cost` (ce que coûte une vignette à l'interface). Un essai d'inactivité tourne toujours avec la suite :
au repos, après une lecture, après un trait, SHOT ne demande aucune image de plus.

Limites : projets synthétiques (dessins bitmap de 40 tuiles de 64 × 64 sur une feuille de
3840 × 2160, soit un dessin au trait qui occupe le quart de la feuille) ; pas de stylet, de synchro
d'écran ni de composition par Windows ; la carte est attendue jusqu'au bout (`attente`), ce que
l'écran ne fait pas. Les chiffres comparent deux états du code sur la même machine ; ils ne
promettent pas une image par image identique à l'écran. Écart d'une passe à l'autre : environ ±5 %
sur les petits temps.

## 2. Mesures avant / après

Machine : Core i7-10700K (8 cœurs, 16 fils), GeForce RTX 2080 SUPER, Windows 11, version release.
**Avant** : `main` c693643 plus le banc (commit 9a6976a, qui ne change aucun calcul), compilé sans
`target-cpu`. **Après** : la branche `perf`. Les deux séries ont été lancées l'une après l'autre sur
la machine au repos, avec les mêmes essais.

Temps par image d'écran en ms (interface + découpage + carte graphique attendue), moyenne ; entre
parenthèses, la plus lente image.

### Projet : ouvrir, enregistrer, démarrer

| Scénario | Avant | Après |
|---|---|---|
| Ouvrir 100 plans × 3 calques × 48 images (3 100 dessins de 3840 × 2160) | **85,4 s**, interface figée | **7,3 s**, sur un autre fil, avancement affiché |
| Enregistrer ce projet (première fois, tout écrire) | 81,4 s | 9,3 s |
| Enregistrer, rien de modifié (préparation sur le fil de l'interface) | 720 ms | 5 ms |
| Lire la bibliothèque de pinceaux au démarrage (300 pointes, 100 papiers importés) | 5 221 ms | 371 ms |
| Construire l'application (polices, thème, raccourcis) | 334 ms | 126 ms |

Un dessin coûtait 25 ms à lire et 28 ms à écrire, un par un : PNG de la feuille entière (33 Mo), 15 ms
de conversion en octets, 18 ms de décodage. Maintenant : sur 8 fils, conversion 3 ms, tampon de
décodage gardé par fil.

### Long projet : 100 plans × 3 calques × 48 images (storyboard, montage sous les yeux)

| Scénario | Avant | Après |
|---|---|---|
| Ouverture : images avant que les vignettes soient toutes là | 93 images, 977 ms | 7 images, 20 ms |
| Souris qui bouge (60 images/s) | 2,85 | 2,64 |
| Tête de lecture qui saute de plan en plan | 17,4 (32,0) | 5,9 (30,4) |
| Zoom du montage | 3,38 | 2,85 |
| Molette sur le montage | 3,32 | 2,98 |
| Lecture du montage à 24 images/s | 3,01 | 2,52 |
| Dessin au pinceau, montage visible | 3,10 | 2,79 |
| Planche (100 vignettes) : ouverture | 16 images, 261 ms | 6 images, 61 ms |
| Planche : molette | 12,1 | 4,5 |

### Plan chargé : 15 calques × 200 images (animation, timeline sous les yeux)

| Scénario | Avant | Après |
|---|---|---|
| Souris qui bouge | 4,89 | 4,58 |
| Tête de lecture à chaque image (15 dessins neufs par image) | 29,8 (49,4) | 13,8 (27,3) |
| Timeline zoomée (miniatures) : ouverture | 165 ms | 17 ms |
| Timeline zoomée : souris qui bouge | **76,4** | **4,2** |
| Timeline zoomée : tête de lecture | 87,6 | 13,3 |
| **Lecture à 24 images/s** (budget 41,7 ms) | 14,9 (33,5) | 8,2 (14,1) |
| Dessin au pinceau sur le calque 8 (8 points/image) | 6,19 | 4,99 |
| Dessin vectoriel texturé, 30 traits déjà posés (gouache, feutre, fusain) | 2,7 / 1,9 / 2,1 | 2,6 / 1,7 / 2,8 (mêmes, au bruit près) |

Le trait lui-même coûte 0,3 ms dans le dessin (section « Dessin ») ; le reste de l'image, c'est
l'interface (2 à 3 ms) et la composition de 15 calques par la carte graphique (2,2 ms).

### Panneaux (plan de 60 calques, 480 réglages de pinceau)

| Panneau ouvert | Avant | Après | Section du panneau |
|---|---|---|---|
| Calques (60) | 10,6 | 9,6 | 0,45 ms |
| Pinceaux (480 réglages) | 11,2 | 10,1 | 0,46 ms (premier affichage : 14 images, 151 ms) |
| Couleur | 10,1 | 9,6 | 0,12 ms |
| Réglages, Effets, Historique | 9,9 / 9,7 / 9,2 | 9,6 / 9,6 / 9,1 | < 0,1 ms |

Presque tout est la composition de 60 calques par la carte (5,7 ms) : voir le levier 3 plus bas.
Les panneaux eux-mêmes sont légers : les aperçus de pinceaux sont gardés par réglage, les
vignettes de calque par dessin et version, la roue des couleurs est un maillage (aucune texture
refaite).

### Ce qui grandit avec le projet

| Scénario | Avant | Après |
|---|---|---|
| Timeline de 2 000 images × 15 calques : souris | 20,5 | 4,7 |
| Même timeline : tête de lecture | 43,1 | 6,3 |
| Montage de 600 plans × 3 calques : souris | 32,4 | 3,6 |
| Même montage : tête de lecture | 46,3 (98) | 11,4 (31,8) |
| Étape d'annulation de structure, 600 plans | 0,96 ms | inchangé |

### Calculs de processeur

| Calcul | Avant | Après |
|---|---|---|
| Vignette du montage 192 × 108, 3 calques | 7,3 ms | 1,6 ms |
| Même vignette, 15 calques | 34,0 ms | 3,1 ms |
| Feuille → octets RGBA (enregistrement) | 13,6 ms | 3,3 ms |
| Octets → tuiles (ouverture) | 6,2 ms | 5,5 ms |
| Construire l'interface, 100 images, 100 plans | 153 ms | 131 ms |
| PNG : coder une feuille | 22,2 ms | 16,7 ms |

### Après la fusion avec `main` (31aef6e)

Mêmes essais, même machine, relancés sur `perf-integre` ; la machine était partagée avec d'autres
programmes (une série sur trois a été perturbée, ± 15 %), les chiffres sont ceux d'une série au repos.
Aucune régression : tout est dans le bruit de la colonne « Après » ci-dessus.

| Scénario | Après (perf) | Après la fusion |
|---|---|---|
| Ouvrir 100 plans × 3 calques × 48 images | 7,3 s | 7,9 s |
| Enregistrer ce projet (première fois) / rien de modifié | 9,3 s / 5 ms | 9,6 s / 5 ms |
| Plan chargé : souris / lecture à 24 images/s / tête de lecture | 4,58 / 8,2 / 13,8 | 4,50 / 8,0 / 14,2 |
| Timeline zoomée : souris | 4,2 | 4,2 |
| Long projet (100 plans) : souris / tête de lecture / lecture du montage | 2,64 / 5,9 / 2,52 | 2,64 / 5,6 / 2,35 |
| Timeline de 2 000 images : souris / tête de lecture | 4,7 / 6,3 | 4,9 / 6,7 |
| Montage de 600 plans : souris / tête de lecture | 3,6 / 11,4 | 3,7 / 11,6 |
| Panneaux (60 calques) Calques / Pinceaux / Couleur | 9,6 / 10,1 / 9,6 | 10,1 / 10,3 / 10,1 |
| Panneau Commentaires (300 commentaires, nouveau) | | 9,7 (la composition des 60 calques en fait 5,8) |
| Vignette 192 × 108, 3 / 15 calques | 1,6 / 3,1 ms | 1,5 / 2,9 ms |
| Démarrage : bibliothèque de pinceaux / construction de l'application | 371 / 126 ms | 356 / 134 ms |

Les effets de calque (`fxperf_tests.rs`) gardent leurs chiffres : lecture pleine résolution 0,06 ms
par image d'écran sur le fil de l'interface (0,08 avant), curseur du flou et du bruit 0,1 et 0,0 ms,
image à froid à l'export 64 ms, et la vignette à effets 5,7 ms (7,7 avant : le dessin des vignettes
est cherché une fois par vignette des deux côtés, le code de la fusion garde la lecture à l'échelle
d'un dessin plus petit que la feuille, `render::Resolved`). Tremblement du trait
(`boiling_playback_cost`) : 0,040 ms par image d'écran en moyenne, 1,2 ms au pire, 60 traits
texturés, 96 images.

Ce que la fusion a décidé :
- **Cartes graphiques** (`gpu.rs`) : `sync` cherche chaque dessin affiché une fois (`fx::display`),
  marque leurs textures comme celles de l'image avant d'en créer une, puis réutilise la texture
  la plus ancienne **de même taille** (perf) ; l'éviction se compte en octets, pas en nombre de
  textures, puisqu'un aperçu d'effet pèse le quart d'un dessin entier (effets).
- **Ouverture sur un autre fil** : `App::finish_open` passe par `App::load_doc`, donc le fichier
  `commentaires.json`, le format 26, l'oubli des dessins dérivés des effets (`fx::forget_all`) et la
  remise à zéro des clés choisies sont ceux d'avant. L'enregistrement parallèle écrit le fichier
  des commentaires, la copie de secours aussi (`parallele_tests.rs`, `recovery_tests.rs`).
- **Dessin des seuls blocs visibles** : les plans colorés, les marques, la bande des séquences, les
  clés de caméra choisies, les clés d'effets et les fanions de commentaires sont ceux de `main` ; seuls
  les plans et les colonnes hors de la zone visible ne sont plus peints.

## 3. Ce qui a été changé

| Commit | Levier | Gain mesuré |
|---|---|---|
| 9a6976a | Profileur intégré, superposition, banc sans fenêtre (`perf.rs`, `perf_tests.rs`, `App::frame`, `App::sans_fenetre`) | de quoi mesurer |
| e9e6fb3 | Dessins lus et écrits sur 8 fils (`parallele.rs`, `project.rs`) ; `to_rgba` / `from_rgba` sur les tuiles dessinées ; liste du dossier au lieu d'un `exists` par dessin ; ouverture sur un autre fil avec avancement (`ouverture.rs`) | ouvrir 85 → 7 s, enregistrer 81 → 9 s, rien de modifié 720 → 5 ms |
| ec5f9ef | Miniatures de la timeline seulement pour les blocs visibles, cache à 1 500 qui lâche les plus anciennes ; vignettes : dessins cherchés une fois ; textures de dessin recyclées par le cache de la carte | timeline zoomée 76 → 4 ms ; vignette ÷5 à ÷12 ; défilement ÷2 |
| 5ae4d78 | `target-cpu=x86-64-v3` (`.cargo/config.toml`) ; montage : plans visibles seulement | interface -19 %, PNG -26 % |
| f940688 | Timeline : colonnes visibles seulement ; numérotation des scènes et séquences en une passe (plus de carré) | 2 000 images 20 → 5 ms ; 600 plans 32 → 3,6 ms |
| af048b0 | Bibliothèque de pinceaux lue sur plusieurs cœurs ; rang du papier par histogramme | démarrage 5,2 s → 0,4 s |
| fd6dc47 | Présence des projets récents vérifiée sur un autre fil | plus de gel sur un serveur éteint |

Détail des garde-fous :

- **Mêmes fichiers, mêmes pixels.** Les PNG écrits par le chemin parallèle sont identiques à ceux
  de l'ancienne boucle (`parallele_tests.rs`) ; `to_rgba` / `from_rgba` sont comparés à l'ancien calcul
  sur des tailles quelconques ; la vignette est comparée à la composition entière échantillonnée aux
  mêmes points (opacité, teinte, mode de fusion, transformation) ; l'histogramme du papier donne les
  valeurs exactes du tri ; la bibliothèque de pinceaux se relit à l'identique. Le format du projet
  n'a pas changé.
- **Caches qui s'invalident.** Textures recyclées : la carte montre exactement le dessin, sans reste du
  précédent, 45 passages sur 15 dessins avec un cache minuscule (`gpu_tests.rs`) ; miniatures : gardées
  d'une image à l'autre sans renvoi (`perf_tests.rs`) ; présence des récents : inconnue = présente,
  puis la réponse, pas de nouvelle vérification avant 3 s.
- **Rien de lourd sur le fil de l'interface en plus.** L'ouverture d'un projet en sort ; les autres
  levées s'ajoutent à du travail déjà fait là (vignettes : 6 ms par image au plus, comme avant).

## 4. Compilation

Quatre variantes compilées à froid dans des dossiers à part, puis `perf_cpu_bench` (le meilleur de
cinq passages, deux à trois séries) :

| Variante | Compilation à froid | Après une modification | Interface (100 images) | Coder un PNG | Image filmée | 30 traits texturés |
|---|---|---|---|---|---|---|
| Réglage d'origine (opt 3, 16 unités) | 233 s | 132 s | 150–162 ms | 20,6–21,0 ms | 15,3–17,6 ms | 1 273–1 307 ms |
| LTO fin | 170 s\* | 130 s | 150–155 | 21,1–21,3 | 13,7–16,9 | 1 276–1 310 |
| LTO complet + 1 unité | 492 s | 461 s | 142–145 | 20,6–22,0 | 15,4–16,4 | 1 338–1 341 |
| LTO complet + 1 unité + `native` | 539 s | 436 s | 139–143 | 15,2–15,4 | 12,2–14,1 | 1 289–1 293 |
| **`x86-64-v3` seul** (retenu) | 222 s | ≈ 132 s | **116–123** | **15,4–15,7** | 13,3–14,9 | 1 237–1 255 |

\* La construction d'origine a été lancée en premier, sur une machine pas tout à fait au repos : les
170 s du LTO fin ne sont pas un gain réel.

- **LTO (fin ou complet) et une seule unité de génération** : aucun gain d'exécution mesurable (au
  mieux ±3 %, au pire +5 % sur le rendu des traits), pour un temps de compilation de ×2 à ×3,5 (7 à 8
  minutes à chaque modification). Écartés.
- **`target-cpu=native`** : le gain vient des instructions AVX2 (coder le PNG, l'interface, l'export),
  pas du LTO. `x86-64-v3` (AVX2, FMA, BMI2) donne le même gain et reste valable sur tout processeur
  depuis Haswell (2013) et Zen. **Appliqué** dans `.cargo/config.toml`. Coût : un processeur plus
  ancien ne lancerait pas SHOT (instruction illégale au démarrage) ; pour compiler pour une telle
  machine, retirer le fichier ou lancer `cargo` avec `RUSTFLAGS=""`. Le premier `cargo build` qui
  suit recompile tout une fois (3 à 4 minutes), comme à chaque changement d'options. SHOT ne tourne
  que sur le poste d'Anthony (i7-10700K, AVX2 et FMA présents).
- **`panic = "abort"`** : **écarté**. Le rapport de plantage (`plantage.rs`) passerait, car le crochet
  s'exécute avant l'arrêt ; mais SHOT compte sur les panics rattrapés : `rejeu.rs` (`catch_unwind`, un
  rendu qui plante ne ferme pas SHOT), les `join()` qui rendent une erreur (copie de secours, import
  vidéo, propagation, export). Avec `abort`, chacun deviendrait un plantage complet. Le gain attendu
  (taille du binaire, un peu de vitesse) ne le vaut pas.

## 5. Constats

- **Repos** : SHOT ne demande aucune image de plus quand rien ne change (après l'ouverture, une
  lecture, un trait, avec un montage de 100 plans ou une timeline de 200 images). Essai permanent :
  `an_idle_application_does_not_repaint`. Les seuls réveils sont ceux qui ont une cause : vignettes
  qui attendent (6 ms par image), rejeux en arrière-plan (16 ms), lecture, notifications (250 ms),
  copie de secours (≤ 10 s quand le projet est modifié).
- **Lecture** (état de la première passe ; depuis `perf2`, voir la section 9) : pas de cache en mémoire
  d'images composées. Chaque image est composée par la carte graphique à partir d'une texture par
  dessin ; les dessins de la seconde qui suit partent d'avance (4 Mo par image d'écran). Une texture
  de dessin était de la taille de la **feuille** (33 Mo pour 3840 × 2160), le cache en gardait
  environ 30 (1 Go) : avec 15 calques animés à 2 images, la lecture renvoyait à la carte les dessins
  neufs de chaque image. Tenait : 8,2 ms en moyenne, 14,1 ms au pire pour un budget de 41,7 ms. Les
  calques à effets (flou, ombre, CTG) se calculent sur le processeur (hors de cette passe, voir
  l'agent des effets).
- **Dessin dans un plan lourd** (état de la première passe ; levier 3 fait depuis) : la pile de
  calques était recomposée entièrement à chaque image d'écran par la carte : 2,2 ms pour 15 calques,
  5,7 ms pour 60. Sans effet sur la latence du trait (le CPU fait 2 à 3 ms, la carte travaille en
  parallèle), mais c'était de la chaleur et du ventilateur gratuits.
- **Trait vectoriel texturé** : déjà sans latence (rejeu en arrière-plan) ; 2 à 3 ms par image avec
  30 traits larges sur le calque.
- **Ce que le banc ne voit pas** : le stylet (cadence réelle des événements), l'affichage (synchro,
  Windows), la mémoire de la carte pleine, un vrai projet d'Anthony. À refaire avec la superposition
  sur un vrai projet.

## 6. Leviers restants, classés

| # | Levier | Gain estimé | Effort |
|---|---|---|---|
| 1 | ~~**Enregistrer en arrière-plan** (Ctrl+S) : l'écriture sort du fil de l'interface~~ **fait** (`perf2`, section 9.1) | Ctrl+S avec 50 dessins modifiés : 270 → 1,1 ms sur le fil de l'interface ; premier enregistrement de 500 dessins : 2 813 → 4 ms | fait |
| 2 | ~~**Textures de dessin à la taille du contenu** (boîte des tuiles dessinées) au lieu de la feuille~~ **fait** (`perf2`, section 9.2) | 32 textures de 33 Mo → 1 093 de 1 Mo dans le même Go ; lecture 7,9 → 3,9 ms ; plan revu 14,0 → 3,1 ms | fait |
| 3 | ~~**Image composée gardée** (dessous / dessus) tant que le plan et les versions ne changent pas~~ **fait** (`perf2`, section 9.3) | carte, 15 calques, souris qui bouge : 2,3 → 0,45 ms par image | fait |
| 4 | ~~**Vignettes du montage hors du fil de l'interface**~~ **fait** (`vignettes-fond`, section 11) | plus de pic de 6 ms en défilant un grand montage (600 plans, tête de lecture : interface 11,1 → 3,4 ms ; pire image 13,2 → 4,6 ms) | fait (les tuiles `Arc` de `perf2` rendaient le cliché presque gratuit) |
| 5 | **Décodage PNG en flux vers les tuiles** (crate `png` en dépendance directe, déjà dans `Cargo.lock`) | ouverture 7,3 → ≈ 4 s, mémoire ÷30 par fil | S à M |
| 6 | ~~**Cache d'images composées en mémoire** pour la lecture des plans à effets~~ **sans objet** : la carte dessine les effets à chaque image (section 11), plus rien à garder | 24 images/s garanties avec 10 effets ou plus : 1,5 à 22 ms par image, pleine résolution | fait |
| 7 | ~~**Effets sur la carte graphique** (flou, ombre, bruit, mosaïque)~~ **fait** (`effets-gpu`, section 12) : 28 effets sur 29 (29 sur 29 depuis le 2026-10-20, section 13.4), calques d'ajustement hors dossier | scène d'Anthony : 2,9 ms par image en pleine résolution (avant : aperçu au quart, pleine résolution au repos seulement) ; curseur du flou 3,5 ms | fait |
| 8 | ~~**Taille mémoire de la carte réglable** (`CACHE_BYTES` = 1 Go, sur 8 Go)~~ **fait** par le diagnostic (section 7) : réglage « Cache de la carte graphique » dans les Préférences, qui monte à 2 Go sur une carte de 8 Go (profil Qualité) | ×2 de dessins en cache à coût nul | fait |
| 9 | **Résolution d'affichage** (réduite pour la vue éloignée) | composition ÷4 en zoom arrière | M à L |
| 10 | **PGO** (optimisation guidée par un profil) | 5 à 15 % sur le rendu des traits (non mesuré) | M. Demande `llvm-profdata` et une procédure hors ligne. |

## 7. Diagnostic des performances (Aide > Diagnostic des performances…)

Action `aide.diagnostic` (menu Aide, Ctrl+K). Demande d'Anthony, 2026-10-19 : une option qui détecte le
matériel et propose des ajustements qui vont jusqu'à éteindre certaines fonctions gourmandes. Code :
`materiel.rs` (détection), `diagbanc.rs` (banc), `diagnostic.rs` (réglages, profils, rapport),
`diagnosticui.rs` (fenêtre, Préférences, pose des réglages), essais `diagnostic_tests.rs`.

**Détecté** (rien de téléchargé, aucune caisse de plus) : processeur (nom lu dans le registre de Windows,
fils par la bibliothèque standard, cœurs physiques par `GetLogicalProcessorInformation`, AVX2 / FMA / BMI2
par `is_x86_feature_detected`) ; mémoire vive totale et disponible (`GlobalMemoryStatusEx`) ; carte
graphique (adaptateur wgpu : nom, API, pilote, genre) et sa mémoire vidéo, lue dans le registre
(`HardwareInformation.qwMemorySize` de la carte du même nom), « inconnue » si Windows ne la donne pas ;
écran (moniteur, pixels par point) ; disque (64 Mo écrits puis relus dans le dossier temporaire,
effacés ensuite). Les appels système sont déclarés dans `materiel.rs` (kernel32 et advapi32, déjà liés).

**Mesuré** sur un autre fil, 13 secondes (plan synthétique de douze calques, aucun projet ouvert ; le
fil laisse un cœur à l'interface ; Annuler arrête dans la seconde) :

| Étape | Durée visée | Ce qui est chronométré (médiane) |
|---|---|---|
| Disque | ≈ 0,5 s | écriture + `sync_all`, lecture de 64 Mo |
| Composition | 2,5 s | `render::composite_full`, 12 calques de 3840 × 2160 |
| Trait texturé | 2,5 s | rejeu d'un trait large de gouache par le moteur (`vector::render_element`) |
| Flou plein | 2 s | `fx::apply_scaled` niveau 0, feuille de 3840 × 2160 |
| Flou réduit | 1,5 s | `fxproxy::downscale` puis `apply_scaled` niveau 1 |
| Lecture | 4 s | `render::camera_image` image après image : moyenne et pire |
| Carte graphique | 1,5 s | écriture d'une texture de 1920 × 1080 et attente de la carte (Mo/s) |

Poste de référence (Core i7-10700K, RTX 2080 SUPER ; deux séries, 2026-10-19) : composition 19,5 à 36 ms,
trait 106 à 155 ms, flou 43 à 63 ms (réduit 14 à 20 ms), lecture 12,7 à 17 ms par image (pire 17 à 24 ms),
carte 6 800 Mo/s, disque 100 à 1 400 Mo/s. Les constantes `REF_*` de `diagnostic.rs` sont ces temps arrondis
au-dessus de la meilleure série.

**Règles du profil** (`choisir_profil`) : chaque constat a un poids. Poids 2 → Performance : moins de
8 Go de mémoire vive ou moins de 2 Go disponibles, moins de 4 fils, pas de carte graphique ou rendu
logiciel, moins de 2 Go de mémoire vidéo, une mesure plus de 4,5 fois plus lente que la référence. Poids 1
→ Équilibré : moins de 16 Go ou moins de 4 Go disponibles, moins de 8 fils, graphisme intégré ou carte
virtuelle, moins de 4 Go de mémoire vidéo, une mesure plus de 2,2 fois plus lente. Sinon Qualité. Un disque
lent se dit mais ne change rien. La phrase d'explication cite les constats avec leurs chiffres.

**Les onze réglages** (`PerfReglages`, `[performance]` dans reglages.toml ; tous modifiables dans
Fichier > Préférences… > Performance ; la valeur par défaut est le comportement d'avant) :

| Réglage | Branché sur | Qualité | Équilibré | Performance | Prise d'effet |
|---|---|---|---|---|---|
| Effets en aperçu réduit | `fx::draft_level` (niveau minimal 1, même zoomé) | non | oui | oui | tout de suite |
| Traits vectoriels texturés | `App::finish_open`, `rejeu_idle_tick` | exact aussitôt | rapide jusqu'au repos (2 s de calme) | rapide partout | à l'ouverture d'un projet |
| Tremblement : couper « Animer à l'arrêt » | `tremblement::Settings::of` | non | non | oui | tout de suite |
| Pelure : images au plus | `onion::neighbours` | sans plafond | 6 | 2 | tout de suite |
| Cache de la carte graphique | `GpuCanvas::set_cache_bytes` | 1 Go (2 Go si 8 Go de mémoire vidéo) | 0,5 à 1 Go | 256 à 512 Mo | tout de suite |
| Cache des traits | `vector::set_cache_budget` | 512 Mo (1 Go si 32 Go de mémoire) | 512 (384 si < 16 Go) | 256 Mo | tout de suite |
| Cache des effets | `fxcache::set_limit` | 768 Mo (1,5 Go si 32 Go) | 768 (512 si < 16 Go) | 384 (256 si < 8 Go) | tout de suite |
| Fils de calcul | `parallele::coeurs` (tous les calculs répartis, fil des effets) | automatique | automatique | un cœur libre sur 3 à 4 fils | tout de suite |
| Vignettes | `thumbs::thumb_size`, vignettes de calque | normales | normales | moitié (quart si faible) | tout de suite, vignettes refaites |
| Animations | `egui::Style::animation_time`, `ajuste::ease` | non | non | oui | tout de suite |
| Effets sur la carte graphique | `GpuCanvas::set_effects_gpu` (`gpufx.rs`) | oui | oui (non sur un rendu logiciel) | oui seulement sur une carte dédiée | tout de suite |

« Rapide partout » : le rendu exact des traits texturés ne vient que pour l'image affichée à l'arrêt
(`warm_current_cel`) ; avant un export (images, film, PDF, image courante, Storyboard Pro) les autres
sont rendus exactement d'un coup, sur plusieurs cœurs (`App::exactify_idle`) : c'est le seul endroit où
ce réglage fait attendre, à la demande de l'utilisateur.

**Appliquer / revenir.** « Appliquer » garde les valeurs d'avant (`EtatDiagnostic`, `[diagnostic]`), pose
les réglages cochés ; « Revenir comme avant » remet ceux-là, exactement (un autre réglage changé à la main
entre-temps reste). **Premier lancement** : sans trace d'un diagnostic dans les réglages, un message propose
« Lancer le diagnostic » / « Plus tard » ; la réponse est gardée, il ne revient pas, rien n'est appliqué.

**Essais** : `cargo test --release --offline --target-dir target\msvc diagnostic_tests` (profils d'après des
mesures inventées, chaque réglage branché sur ce qu'il commande, appliquer puis revenir, anciens fichiers
de réglages, premier lancement, fenêtre et Préférences, annulation du banc). À la main :
`... diag_bench -- --ignored --nocapture` (le banc réel et ses chiffres) et `... diag_capture -- --ignored`
(images PNG de la fenêtre dans `SHOT_CAPTURE_DIR`).

## 8. À regarder à l'écran

- **Diagnostic des performances** : Aide > Diagnostic des performances… (ou `Ctrl+K`, « Diagnostic »).
  La fenêtre s'ouvre en haut au centre, la machine détectée dessous (processeur, mémoire, carte et sa
  mémoire vidéo) ; « Lancer le diagnostic » : barre d'avancement et Annuler, l'interface reste vivante ;
  puis mesures, profil conseillé avec ses raisons, trois profils à comparer, réglages à cocher, boutons
  Appliquer / Revenir comme avant / Relancer / Copier le rapport / Enregistrer le rapport…, toujours
  visibles en bas. Essayer « Performance » sur un vrai projet (lecture, pelure, traits texturés) puis
  « Revenir comme avant » ; vérifier la section Performance de Fichier > Préférences… avant et après.
- **Superposition** : `Ctrl+K` → « Performance » (ou Affichage > Performance). Le cadre est lisible
  sur les trois thèmes, ne gêne pas le dessin (il ne prend pas les clics), et se ferme avec la même
  commande. Regarder « Images » au repos : 0/s ; et les réveils.
- **Ouvrir un gros projet** : la fenêtre « Ouverture de ‹nom› » avec sa barre de progression ; la
  fenêtre de SHOT apparaît tout de suite au lancement ; l'interface reste vivante (menus, déplacement
  de la fenêtre) ; les projets récents introuvables sont grisés après quelques secondes.
- **Timeline** : zoomée, les miniatures des blocs apparaissent en défilant (aucun bloc manquant au
  bord de la zone visible, graduations et numéros, comportements avant / après de calque, signets,
  points d'entrée et de sortie).
- **Montage** : plans, titres, barres de séquences en défilant et en zoomant ; un plan coupé par le
  bord de la bande ; la lecture du montage.
- **Textures recyclées** : défiler vite entre des dessins très différents (lecture, tête de lecture
  qui saute) sans voir de reste du dessin précédent (tuiles fantômes).
- **Démarrage avec les brosses importées** : la liste des pinceaux, leurs grains et pointes comme avant.
- **Compilation `x86-64-v3`** : si `shot.exe` ne démarre pas sur une autre machine, retirer
  `.cargo/config.toml`.

## 8. Fiabilité : le temps jusqu'à la première image

Voir `docs/FIABILITE.md`. La fenêtre n'apparaît qu'une fois la première image peinte, donc tout `App::new` se paie
en fenêtre noire. Lire les pinceaux importés sur le fil de l'interface coûtait 90 % de ce temps (160 pointes + 60
papiers : 205 ms au repos, 738 ms avec un calcul par cœur en parallèle, **3 695 ms** avec deux par cœur) ; sur un
autre fil : 95 / 192 / 324 ms (banc `perf_startup_under_load`). `SHOT_DEMARRAGE=1` écrit le minutage étape par étape
dans `%LOCALAPPDATA%\SHOT\demarrage.log`. Le banc des essais compose maintenant aussi à une taille d'écran choisie
(`banc_taille`), et les essais d'effets partagent l'état commun du cache en lecteurs / écrivains (`fxcache::monde`).

## 9. Deuxième passe (branche `perf2`) : enregistrer en arrière-plan, textures au contenu, image composée gardée

Leviers 1, 2 et 3 de la section 6. Même machine, version release. « Avant » : 8b49066 pour
l'enregistrement ; pour la carte graphique, le comportement d'avant reproduit par deux réglages
d'essai (`GpuCanvas::set_whole_textures`, `set_fresh_staging`) dans le même processus, l'un après
l'autre. Écart d'une série à l'autre : ± 10 % (la machine était partagée avec d'autres programmes).

### 9.1 Enregistrer en arrière-plan (`enregistrement.rs`)

Banc `perf_save_ui_thread` : 500 dessins de 8 × 5 tuiles (320 Mo), projet déjà enregistré une fois.

| Scénario | Avant (sur le fil de l'interface) | Après : appel sur le fil de l'interface | Après : fin de l'écriture (autre fil) |
|---|---|---|---|
| Ctrl+S, 50 dessins modifiés | 269–279 ms de gel | **1,1–1,7 ms** | 177–190 ms |
| Premier enregistrement (500 dessins) | 2 813 ms de gel | **4 ms** | 1 680 ms |
| Rien de modifié | 7,2 ms | 0,9 ms | 8 ms |
| Image d'écran la plus lente pendant l'écriture | (aucune : gel) | 2,3–3,1 ms | |

**Conception.**
- *Cliché* : le fil de l'interface ne fait que `project::prepare_save` (manifeste JSON, fichiers
  annexes `commentaires.json` et `rognages.json`, liste des dessins modifiés depuis le dernier
  enregistrement : ceux dont la version n'est pas dans `written`). La copie des pixels ne coûte
  presque rien : `raster::Tile` est maintenant une tuile **partagée à la copie** (`Arc`, copie à la
  première écriture, `Arc::make_mut`), cloner un `Raster` n'en recopie aucune. Les écritures par
  `tuile[i] = c` se comportent comme avant ; trois boucles par pixel (`from_rgba`, `fxproxy`,
  `tremblement`) prennent le tableau une fois. Les étapes d'annulation y gagnent aussi.
- *Écriture* : un fil lance `SaveJob::run_with` (PNG des dessins sur la moitié des cœurs, 2 à 8 fils,
  puis sons, modèles, textures, vidéos, annexes, manifeste en dernier). La barre d'état montre
  « Enregistrement… n / N ».
- *Compteur de modifications* : `modified` passe à faux **au cliché** ; toute modification qui suit
  le remet à vrai. À la fin de l'écriture on n'y touche pas : s'il est vrai, c'est une modification
  postérieure au cliché que le fichier n'a pas, le projet reste « modifié ». Un échec le remet à vrai
  (et `written` est vidé : tout sera réécrit). `App::doc_epoch` (changé par `load_doc`) empêche
  qu'un enregistrement fini après l'ouverture d'un autre projet lui donne son chemin ou son état.
- *Suites* : `App::request` (Quitter, Fermer, Nouveau, Ouvrir, projet récent, import SBP) retient la
  demande pendant l'écriture (`after_save`) et la refait à la fin : modifié depuis → boîte
  « Modifications non enregistrées », sinon l'action. Au-delà de 200 ms d'attente, une fenêtre montre
  l'avancement. Fermer la fenêtre pendant l'écriture est refusé de même ; `fermeture` attend le fil
  au cas où. Le bouton « Enregistrer » de la boîte lance l'écriture avec l'action en suite. Un
  enregistrement demandé pendant un autre se met en file. Un échec annule toutes les suites et
  ouvre « Enregistrement impossible » (message, Réessayer, Fermer).
- *Atomicité* : `project::write_atomic` écrit `<nom>.tmp` puis renomme (atomique sur Windows) ; en cas
  d'échec le fichier provisoire est effacé et l'ancien fichier reste. Le manifeste est écrit en
  dernier, après les dessins ; le manifeste et les annexes sont en plus synchronisés sur le disque
  (`sync_all`) avant le renommage. La copie de secours (`recovery.rs`) passe par le même `SaveJob`
  et ne démarre pas pendant un enregistrement.
- *Protections de la fiabilité (`docs/FIABILITE.md`)* : elles vivent dans `SaveJob`, donc le fil
  d'écriture les applique comme l'ancien enregistrement synchrone. `prepare_save` prend le cliché des
  fichiers abîmés à garder (dessins, sons, modèles 3D illisibles à l'ouverture) ; `run_with` écrit
  les annexes par `write_sidecar` (un `commentaires.json` ou un `rognages.json` abîmé ou d'une version
  plus récente n'est jamais effacé, et est mis de côté sous `.illisible` si un fichier neuf doit
  prendre sa place), n'efface ni les sons ni les modèles gardés, et garde les textures si un dessin
  est illisible. `project::save` (prépare puis exécute le même `SaveJob`) sert aux essais ;
  `settings.toml` s'écrit lui aussi par `write_atomic`.
- Essais : `enregistrement_tests.rs` (13 : l'enregistrement de l'interface garde les annexes,
  un son et un modèle illisibles ; `modified` juste sous modifications concurrentes, file
  d'attente, Quitter attend et redemande si le projet a changé, la fermeture attend, échec qui garde
  `modified` et annule la suite, atomicité d'une écriture ratée et d'un enregistrement coupé avant le
  manifeste, autre projet ouvert pendant l'écriture, repos après l'écriture).

### 9.2 Textures de dessin à la taille du contenu (`gpu.rs`)

Banc `perf_gpu_textures` : un plan de 15 calques × 200 images (un dessin neuf toutes les 2 images),
feuille de 3840 × 2160, deux jeux de dessins : *éparpillés* (40 tuiles sur toute la feuille, la
boîte du contenu est presque la feuille : le pire cas) et *d'un bloc* (8 × 5 tuiles, un vrai dessin
au trait).

Dessins d'un bloc :

| | Avant | Après |
|---|---|---|
| Poids d'une texture | 33,2 Mo | **0,98 Mo** |
| Dessins gardés dans le cache de 1 Go | 32 | **1 093** |
| Défilement, 15 dessins neufs par image (ms par image d'écran) | 14,6 | 9,1 |
| Même parcours, dessins déjà vus | 14,0 | **3,1** |
| Lecture à 24 images/s (moyenne, ms) | 7,9 | **3,9** |
| Souris qui bouge sur un plan de 15 calques | 5,2 | 3,0 (2,7 avec l'image gardée, 9.3) |

Dessins éparpillés : 32 → 57 textures de 18,8 Mo en moyenne ; défilement 16,7 → 11,8, même parcours
16,1 → 10,4, lecture 8,7 → 4,9 ms. Le gain de ce jeu vient surtout du tampon d'envoi gardé (ci-dessous).

Le défilement à froid est le travail des pixels neufs : 15 dessins × 40 tuiles = 600 tuiles (9,8 Mo)
copiées et envoyées par image. Le « ≈ 6 ms » espéré par la liste ne vient qu'au deuxième passage (3,1 ms) :
ce qui reste à froid est la copie vers la carte (2,1 ms « tuiles envoyées » + la copie en tampon).
**Tampon d'envoi gardé** d'une image à l'autre (avant : 10 Mo de mémoire neuve à chaque image, dont les
fautes de page de Windows coûtaient 2 à 4 ms) : à lui seul 14,6 → 13,1 à froid et 14,0 → 10,7 sur
les dessins déjà vus, avec les textures de la feuille entière.

**Conception.**
- Chaque `CelTexture` a un `rect` (x, y, largeur, hauteur en pixels du dessin) et des `dims` (taille
  de l'image du dessin : feuille, aperçu réduit d'un effet, feuille flottante agrandie). `texture_rect`
  prend la boîte des tuiles non vides, ajoute **un pixel transparent de marge** du côté de chaque
  bord qui n'est pas celui de la feuille (le lissage d'un calque transformé y lit du transparent,
  comme dans la feuille entière ; au bord de la feuille, il lit le bord, comme avant), arrondit la
  taille à 128 pixels du côté libre, sans sortir de la feuille. Dessin vide : un pixel.
- Les tuiles s'envoient au décalage de la boîte. `paint` place les quatre coins du rectangle comme
  l'image entière l'aurait été : calque transformé, feuille flottante (`Pre { aff, dims }`), aperçu
  réduit d'effet (image plus petite montrée sur toute la feuille), et ramène les coordonnées de texture
  des maillages (enveloppe, perspective) à la texture ; la lecture hors de la texture tombe sur la marge
  transparente. Un dessin dont la texture est la feuille entière suit le chemin d'avant, bit pour bit.
- Un dessin qui sort de sa boîte (trait qui s'allonge, nouvelle version) refait sa texture avec de la
  marge (le quart du contenu, au moins deux tuiles : la texture ne se refait pas à chaque tuile) et
  renvoie le dessin entier. Les tuiles signalées pendant un trait (dont `fx::fond_patch`) sont
  vérifiées avant d'être posées ; un pixel peint hors de la boîte ne peut pas être rogné.
- Recyclage : quand le budget en octets est dépassé, la texture de même taille la plus ancienne qui ne
  sert pas à l'image est reprise, et les tuiles de son ancien dessin sont effacées d'abord (rectangles
  de tuile à zéro dans le même envoi). Le budget se compte en octets (le réglage « Cache de la carte
  graphique » des Préférences vaut pour des dessins 30 fois plus nombreux), `ahead_capacity` compte le
  poids moyen des textures gardées : la lecture envoie bien plus de dessins d'avance.
- Le calcul de secours (`render.rs`), l'export, les vignettes ne changent pas.
- Essais : `gpu_contenu_tests.rs` (8) : géométrie des boîtes (toutes positions et tailles, marge, reste
  dans la feuille) ; **mêmes pixels** (à un niveau près, et l'essai en trouve zéro) qu'avec la texture
  de la feuille entière pour un bloc, plusieurs, un dessin plein, des calques transformés (rotation,
  échelle, demi-pixel), des modes de fusion et opacités, un aperçu réduit (droit et transformé), une
  feuille flottante déplacée et en enveloppe ; un dessin qui grandit (nouvelle version, tuiles signalées
  une à une, effacé, vidé, rempli ailleurs) comparé au calcul de secours à chaque étape ; recyclage entre
  boîtes de même taille et d'origines différentes ; le cache garde 8 fois plus de dessins ; rien n'est
  renvoyé quand rien ne change.

### 9.3 Image composée gardée

`CanvasPaint::prepare` compare une clé (`ItemKey` : dessin, génération de sa texture, coins, couleur,
réglages, mode de fusion, empreinte du maillage de chaque élément, dans l'ordre) à celle de l'image
composée qu'il garde (`Frame::composed`). Identique : la passe de composition est sautée, seul l'affichage
(vue, zoom) est refait. La génération d'une texture est l'image d'écran où elle a reçu des pixels ou
a été refaite. Mesuré, souris qui bouge sur 15 calques, part de la carte graphique par image d'écran :
2,4 ms (avant) → 0,6–0,9 (textures au contenu) → **0,40–0,45 (image gardée)** ; dessins éparpillés,
dont les textures restent grandes : 2,3 → 1,6 → 0,40. Total de l'image d'écran : 5,2 → 2,7 ms. Pour
60 calques (5,7 ms de carte avant) le gain attendu est de l'ordre de 5 ms (non mesuré).
Essai : composée une fois pour cinq images identiques, refaite pour une opacité, une transformation,
un mode de fusion, une tuile signalée, une version, un dessin de plus, une autre taille de feuille.

### 9.4 Risques

- **Perte de données à l'enregistrement** : le seul risque sérieux. Garde-fous : `modified` remis à
  vrai par tout ce qui suit le cliché (et par tout échec), suites retenues jusqu'à la fin, fermeture
  qui attend, atomicité. Un `modified = true` oublié par un outil (une modification qui ne le pose
  pas) était déjà un risque avant : le cliché ne l'aggrave pas. Sons, modèles et textures ne
  s'écrivent que s'ils manquent (ils ne changent jamais sous le même nom) ; un dessin vectoriel est
  sérialisé en JSON sur le fil de l'interface (non mesuré : un dessin avec des milliers de traits peut
  y coûter quelques ms).
- Premier enregistrement d'un très gros projet : 8 fils d'encodage tournent en même temps que
  l'interface (essayé en priorité basse : sous charge, six fois plus lent, abandonné).
- **Tuiles partagées** (`Arc`) : une écriture fait la copie de 16 Ko si la tuile est encore partagée (avec
  un cliché en cours d'écriture, ou une étape d'annulation) ; une écriture par pixel directement par
  la tuile coûte un test atomique de plus (trois boucles chaudes corrigées ; `perf_cpu_bench` : 30 traits
  texturés 1 275 ms contre 1 237–1 255, dans le bruit).
- **Textures au contenu** : le rognage d'un pixel hors de la boîte et la lecture d'un voisin au bord
  sont les deux risques ; couverts par la marge d'un pixel et `covers`. Une texture dont le contenu a
  fini plus petit que la boîte ne rétrécit pas avant d'être chassée. L'arrondi à 128 pixels gaspille jusqu'à
  25 % sur un dessin de 8 × 5 tuiles (960 Ko au lieu de 750) pour des tailles qui se recyclent.
- **Image composée gardée** : une clé incomplète montrerait une image périmée. Tout ce qui entre dans le
  tampon des réglages et les sommets y est, avec la génération des textures. Les envois d'avance (dessins
  qui ne sont pas à l'écran) ne changent aucune clé.

### 9.5 À regarder à l'écran

- **Ctrl+S dans un gros projet** : rien ne se fige ; « Enregistrement… n / N » dans la barre d'état ;
  le point • du titre reste jusqu'à la fin ; dessiner pendant l'écriture laisse le projet « modifié »
  après. Quitter pendant l'écriture : la fenêtre « Enregistrement » apparaît après 200 ms, puis SHOT se
  ferme. Un disque plein ou un dossier en lecture seule : « Enregistrement impossible », rien de perdu.
- **Dessin et défilement** : un trait qui sort loin de son contenu (de haut en bas de la feuille) est
  entier, sans bord rogné ; un calque transformé (échelle, rotation) a les mêmes bords qu'avant ; une
  feuille flottante déformée (Enveloppe, Perspective) aussi ; un fond sous le trait qui se peint ; des
  pelures d'oignon teintées ; un aperçu réduit d'effet qui revient en pleine résolution ; un tremblement
  du trait animé à l'arrêt. Aller et venir très vite dans la timeline sans reste des dessins précédents.
- La **lecture** d'un plan chargé reste régulière (superposition « Performance » : « Dessin » et
  « composition : envoi à la carte graphique » plus bas qu'avant).

## 10. Calques d'effets : la lecture d'un plan avec impact, lueur, tremblement et timecode (2026-10-19)

Règle : un calque d'effet (`calqueffet.rs`) passe par le chemin des effets d'un calque (`fxcache.rs`, fil de fond,
aperçu réduit) ; le fil de l'interface ne calcule jamais rien. Mesures (`effetsperf_tests.rs`, tests `#[ignore]` :
`cargo test --release --offline --target-dir target\msvc --lib effetsperf -- --ignored --nocapture --test-threads=1`),
sur la scène de la partie 2 (feuille 3840 × 2160, cadre 1920 × 1080, 24 i/s, 48 images, deux calques de dessin) avec
quatre calques d'effets par-dessus : une image d'impact (images 12 et 13 : négatif teinté, rayons, lignes de vitesse),
une lueur qui pulse (tout le plan), un tremblement de l'image avec flou de bougé (images 20 à 43), un timecode.
Machine de la séance : 16 cœurs logiques, fil de fond sur 14.

| Mesure | Résultat |
|---|---|
| Fil de l'interface, une image dont les dessins dérivés sont prêts (plan, hachage, recherche) | **0,00 ms** (pire 0,01) |
| Débit du fil de fond, aperçu servi au quart (zoom moyen, faible ou élevé en lecture), 48 images demandées d'un coup | **51 images/s** (19,5 ms par image) |
| Idem, les images chargées (tremblement + lueur + timecode, images 20 à 43) | 36 images/s (27 ms par image) |
| Une image à la moitié de la résolution (tremblement + lueur) | 67 ms |
| Une image en pleine résolution, au repos (fil de fond) | 227 ms |
| Export exact d'une image (calcul bloquant, pleine résolution, caméra comprise) | 270 ms |

Ce qui tient les 24 images par seconde : en aperçu, un calque d'effet est calculé au plus à `PROXY_PIXELS` = 1 million
de pixels (`calqueffet::proxy_level` : une feuille 4K en aperçu est servie au quart de sa résolution, une feuille de
2560 × 1440 à la moitié), un cran plus bas que les effets d'un calque ordinaire ; au repos (un quart de seconde sans
mouvement), la pleine résolution prend la place et l'image montre le dernier résultat en attendant. Les images qui
viennent sont demandées d'avance pendant la lecture (`playback::upcoming_derived`). Sans l'abaissement, la moitié de la
résolution donne 4,4 images/s au premier jet (225 ms par image : le tremblement avec flou de bougé et la lueur
dominent), 15 images/s après optimisation (67 ms par image : lecture directe des pixels dans un tableau à plat, six
lectures au plus pour le flou de bougé, halo de la lueur lu à sa taille réduite) : insuffisant, le quart de résolution
est le réglage retenu.

Reste : la pleine résolution (227 ms) n'est pas fluide, c'est le repos ; l'export de 1 000 images coûterait environ
4 min 30 pour les images chargées (une secousse de pixels de la feuille entière, caméra hors de la feuille comprise :
calculer seulement dans le cadre de la caméra serait un gain d'un facteur 3 à 4, non fait — les effets lisent des
positions de la feuille entière). *(Mise à jour : sur la carte graphique, section 12, la pleine résolution tient en
lecture : 1,5 à 1,8 ms par image sur cette scène.)*

## 11. Vignettes hors du fil de l'interface (branche `vignettes-fond`, levier 4)

Avant : le montage, la planche et les miniatures de calques se composaient sur le fil de l'interface
(`render::thumbnail`), 6 ms par image d'écran au plus (`THUMB_TIME`) et au moins une vignette par image :
en défilant un montage de 600 plans ou en sautant de plan en plan, chaque image d'écran portait 4 à 7 vignettes
(1,5 ms chacune pour 3 calques, 5 ms pour 15). Depuis `perf2`, copier un `Raster` ne copie aucune tuile
(`Arc`) : le cliché d'un plan est presque gratuit, et la composition peut partir sur d'autres fils.

### Conception (`thumbs.rs`, `layersui.rs`)

- **Le fil de l'interface** (`Thumbs::texture_at`) calcule la clé de la vignette (composition du plan : dessins,
  opacités, teintes, transformations, modes, effets, calques d'effets ; version de chaque dessin ; caméra ;
  fonds arrivés ; taille du cadre) et la compare à celle de la vignette gardée. Elle a changé : un **cliché** (un
  `Document` minuscule qui ne porte que les dessins du plan, ceux de ses sources CTG et leur strate Fond, sans
  le vecteur, tuiles partagées) part dans la file. Il rend tout de suite l'image gardée, même ancienne, ou celle
  d'une autre taille du même plan ; rien si la vignette n'a jamais été faite (le bloc du montage reste vide un
  instant). `Thumbs::begin_frame` envoie les images finies à la carte graphique, 1 ms au plus par image d'écran
  (`UPLOAD_TIME`), et le reste à l'image suivante.
- **Les fils** (`Pool`, un à quatre, deux cœurs restent à l'interface ; endormis sans réveil périodique) prennent
  une demande, la composent avec **le code d'avant**, `render::thumbnail_until` (`thumbnail` plus un test
  d'abandon entre deux lignes), dans le mode « doux » d'avant (`fx::differe` + `fxcache::soft_scope`) : les
  effets de calque pas encore calculés sont demandés au fil des effets et remplacés par leur dernier résultat ou
  le dessin sans effet, les calques d'effets (`calqueffet`) se résolvent de même, le fond sous le trait montre
  celui d'avant jusqu'à l'arrivée du nouveau (alors seules les vignettes des plans qui montrent un fond se
  refont : le compte des fonds arrivés, `fx::fond_epoch`, n'est dans la clé que pour eux), les images de calque vidéo se lisent sur le fil de fond (elles se
  lisaient sur l'interface). Une vignette incomplète se refait quand un calcul de fond finit
  (`fxcache::generation`), comme avant, et aussi quand une demande d'effet est abandonnée parce qu'elle a vieilli
  (fil des effets occupé plus de six images d'écran) : sans cela, elle restait incomplète pour toujours. Un fil qui plante rend une vignette vide au lieu de fermer SHOT.
- **File d'attente.** Une demande par vignette : la nouvelle **remplace** celle qui attend et **arrête** celle qui
  se calcule (un indicateur testé entre deux lignes). Une demande que l'écran **ne renouvelle plus** depuis
  3 images d'écran (la vignette n'est plus visible) est **abandonnée** ; parmi celles qui restent, la plus
  récemment demandée passe la première, et dans une même image, dans l'ordre où l'écran les a demandées. Une
  réponse plus ancienne que celle qu'on montre déjà ne la remplace pas (deux fils finissent dans le désordre).
  Une vignette finie pendant l'image d'écran, pas encore envoyée, est la réponse : l'écran ne la redemande pas
  (elle était composée deux fois quand le fil finissait entre `begin_frame` et la demande), sauf si une demande
  plus récente pour la même vignette l'a suivie. Autre projet, nouvelle résolution : `clear` abandonne tout (file, calculs, images finies).
- **Miniatures de dessins** (timeline, panneau Calques, en-têtes de calques, 64 × 36) : la même file (deux
  fils), le calcul est `thumbs::cel_thumbnail`, déplacé tel quel de `App::layer_thumb`. L'ancienne miniature reste
  en attendant la neuve ; un dessin qui n'en a pas montre un cadre vide. (Au passage : l'ouverture d'un autre
  projet vide aussi ces miniatures, ce qu'elle ne faisait pas : des numéros et des versions de dessin
  identiques d'un projet à l'autre en auraient montré de fausses.)
- **Ce qui ne change pas** : les vignettes de l'export, de l'API locale (`noyau.rs`) et de la lecture du
  storyboard par un modèle (`lecturesb.rs`, déjà bornée par `BUDGET_VIGNETTES`) restent des appels bloquants ;
  la planche de crayonnés ne compose rien (elle envoie des tuiles) ; les références visuelles se décodent déjà
  sur un autre fil ; le calque « Fond 3D » est un calque bitmap ordinaire, rendu en arrière-plan depuis
  toujours. Les vignettes ne contiennent ni incrustations ni transitions (elles se posent à l'export), avant
  comme après. Le format du projet n'a pas changé.

### Mesures

Même machine (Core i7-10700K, RTX 2080 SUPER), version release, essais `--ignored`, l'ancien et le nouveau
exécutables lancés l'un après l'autre, deux séries. La machine était partagée avec un autre travail de
compilation : une série sur trois a été jetée (temps de carte × 10). Temps de **l'interface** (`App::frame`,
ms par image d'écran), moyenne et, entre parenthèses, la pire image. Banc : `perf_montage_thumbnails_ui_thread`
(montage de 600 plans × 3 calques, vignettes toutes à refaire) et `perf_scale`.

| Scénario | Avant | Après |
|---|---|---|
| Montage de 600 plans, vignettes à froid, molette (120 images) | 6,6–7,9 (9,6–11,3) | **2,3** (2,7–3,2) |
| Montage de 600 plans, vignettes à froid, la tête de lecture saute de plan en plan | 10,2–11,0 (11,6–13,2) | **3,7–4,1** (5,2–5,9) |
| `perf_scale` : montage de 600 plans, tête de lecture (section « Montage » : 9,9 → 1,8 ms) | 9,9 (12,1) | **3,2–3,3** (4,7) |
| `perf_scale` : montage de 600 plans, souris qui bouge | 2,1 (2,3) | 2,1–2,3 (2,2–3,2), pas de vignette à refaire |
| Dessin au pinceau, montage visible (60 images) | 2,4–2,9 (2,9–4,4) | 2,4–2,8 (3,3–3,9) : la vignette ne change qu'à la fin du trait |
| Timeline de 2 000 images × 15 calques, tête de lecture (pas de miniature à cette échelle) | 3,7 (4,1) | 3,4–3,6 (4,0–4,7), inchangé |
| Plan de 15 calques × 200 images, timeline zoomée, tête de lecture (15 miniatures neuves par image) | 12,4–14,8 (22,7–28,9) | 7,2 (8,7), moins sûr : la série suivante a été troublée |
| Attente avant que les vignettes visibles soient toutes là (après un défilement / des sauts) | ≈ 70–95 ms / 250 ms | ≈ 65–76 ms / 250 ms (les 250 ms sont le délai de l'aperçu, `fx::DRAFT_MS`) |

Coût d'une vignette pour le fil de l'interface (`perf_thumbnail_request_cost`, fils en pause) :

| Plan | Demande à froid (clé, cliché, mise en file) | Vignette à jour | La composer sur ce fil (avant) |
|---|---|---|---|
| 3 calques | **36 µs** (dont cliché 12) | 1 µs | 1,99 ms |
| 15 calques | **145 µs** (dont cliché 65) | 5 µs | 5,00 ms |

Les 2 ms de `THUMB_TIME` ne servent plus que de garde-fou (plus de 50 demandes à froid dans une image
d'écran) ; `pending` ne demande plus à repeindre à chaque image tant que les fils travaillent : ce sont eux
qui réveillent l'écran à la fin de chaque vignette (egui regroupe les réveils).

### Garde-fous

- **Mêmes pixels** (`thumbs_tests.rs`) : sur le projet d'essai complet de la fiabilité (effets de calque à clés,
  tremblement du trait, fond sous le trait, calque CTG, calque vidéo, dossier, mode de fusion, transformation,
  caméra à clés, calque d'effet d'ajustement, incrustation ; quatre images par plan), chaque vignette faite par
  les fils est identique, pixel pour pixel, à celle de l'ancien chemin ; idem pour les miniatures de dessins
  (l'ancien calcul est recopié dans l'essai) ; un cliché se compose comme le projet, et ne bouge pas quand on
  dessine pendant la composition (copie à la première écriture).
- **Invalidation** : onze sortes de changement refont la bonne vignette, et seulement elle (un trait, l'opacité
  d'un calque, un calque caché, le mode de fusion, la transformation d'un calque, un effet de calque, une clé de
  caméra, la durée du plan d'avant — qui change la caméra du plan d'après —, le papier, le dessin d'une image,
  un calque de moins) ; l'ancienne image reste à l'écran jusqu'à l'arrivée de la neuve ; un dessin modifié
  refait sa miniature.
- **Abandon** : une demande remplacée n'est jamais composée (huit états successifs, une composition) ; un calcul
  en cours est arrêté quand sa vignette change encore ; une demande qu'on ne renouvelle plus n'est jamais
  composée ; la plus récente passe la première ; autre projet : tout est jeté.
- **Rien sur le fil de l'interface** : `render::THUMBNAILS_HERE` et `thumbs::CEL_THUMBNAILS_HERE` comptent par
  fil les compositions ; l'essai fait défiler un montage, sauter la tête de lecture, dessiner, avec le montage
  puis la timeline zoomée sous les yeux, et exige zéro sur le fil de l'interface (et des compositions sur les
  fils de fond) ; 200 demandes à froid en moins de 250 ms.
- Le banc des essais attend les fils de vignettes (`Banc::attendre_vignettes`, `App::vignettes_en_cours`) : un
  réveil qui tombe au hasard ne trouble ni « au repos, SHOT ne repeint pas » ni les essais de fiabilité. Il
  attend aussi les demandes remises à l'image suivante (`Thumbs::pending` : sur une machine chargée, les 2 ms
  de `THUMB_TIME` passent avant la première demande, et rien n'est encore en file), et ne s'arrête qu'après
  trois images de suite sans demande ni calcul (juste après un changement d'onglet, rien n'est encore demandé).
  Son dossier de données (`shot-perf-bac-<processus>-<fil>`) est vidé au premier banc du fil : Windows
  réutilise les numéros de processus, et un ancien essai y avait laissé une copie de secours.
  `scripts\verifier.ps1 -Repetitions 2` : OK (1 502 essais deux fois de suite, aucun intermittent).

### Risques

- Une vignette qui n'a jamais été faite s'affiche vide (un instant, 5 à 100 ms) au lieu d'apparaître dans
  l'image d'écran où elle est demandée ; en défilant très vite, les vignettes arrivent par vagues derrière le
  défilement.
- Mémoire : le cliché garde les tuiles d'un dessin vivantes tant que sa vignette se compose. Si on dessine
  dans ce dessin pendant ce temps, les tuiles touchées sont copiées (16 Ko) : négligeable.
- Les fils des vignettes s'ajoutent à ceux des effets et de la carte : jusqu'à six fils au repos de plus, aucun
  réveil périodique ; sous charge (un export), ils se disputent les cœurs comme les autres fils de fond.
- Pendant un long trait sur un plan dont la vignette met plus longtemps qu'une image à se composer, chaque
  version remplace la précédente : la vignette du plan courant se met à jour à la fin du trait, pas pendant.

### À regarder à l'écran

- **Montage** : défiler à la molette, zoomer, sauter de plan en plan (flèches, clic sur la règle) : les vignettes
  apparaissent sans saccade derrière le mouvement, aucune vignette fausse (celle d'un autre plan), aucun bloc
  qui reste vide à l'arrêt.
- **Planche, vue Plans** : ouverture, zoom, défilement ; les marques d'image (une vignette par marque).
- **Changements** : dessiner un plan et regarder sa vignette dans le montage (elle se met à jour à la fin du
  trait) ; changer l'opacité, la visibilité, le mode de fusion, une transformation, un effet, un calque d'effet
  (impact, lueur), le fond sous le trait, une clé de caméra, la durée du plan d'avant : la vignette suit.
- **Timeline zoomée** et **panneau Calques** : les miniatures des dessins arrivent en défilant, le dessin
  modifié refait la sienne.
- **Autre projet** : ouvrir un projet puis un autre : aucune vignette ni miniature de l'ancien.
- **Diagnostic des performances** : profil « Performance » (vignettes en moitié de définition) puis « Revenir
  comme avant » : les vignettes se refont à la bonne taille.

## 12. Effets sur la carte graphique (2026-10-19, branche `effets-gpu`, partie de 31b86f1)

Demande d'Anthony : des effets en temps réel, à pleine résolution, pendant la lecture et pendant qu'on tire un
curseur ; le calcul sur processeur reste la référence exacte de l'export. Code : `gpufx.rs` (chaînes de passes,
textures de travail, pipelines), `gpufx.wgsl` (les 28 passes, dont `fs_boil`), `gpu.rs` (branchement), essais `gpufx_tests.rs` (carte
contre processeur), mesures `gpufxperf_tests.rs`.

### 12.1 Comment ça marche

- **Un calque dont toute la pile d'effets passe par la carte n'est plus calculé par le fil de fond.** `GpuCanvas::sync`
  n'envoie que son dessin d'origine (texture à la taille du contenu, comme tout dessin) ; la pile de ses effets à cette
  image (`fx::steps_at` : réglages déjà interpolés entre les clés, sur le processeur, très peu coûteux) devient une
  **chaîne de passes** (`gpufx::plan_chain`) : chaque effet est une à huit passes de dessin (un réglage de couleur une,
  le flou six, la lueur sept, l'ombre portée et la lueur diffuse huit) sur des textures de travail en 8 bits, avec
  va-et-vient entre elles (une réserve les recycle). La dernière passe écrit la **texture résultat** du calque, composée ensuite comme n'importe quel dessin :
  transformation du calque, opacité, mode de fusion, pelure d'oignon, maillage d'une enveloppe, tout est inchangé.
- **Une texture par calque à effets**, à la taille de ce que les effets touchent (le contenu, plus la portée du flou, de
  l'ombre ou du flou de mouvement ; la feuille entière pour ce qui remplit l'image : flash, vignettage, secousse,
  image d'impact, lueur diffuse, déformations), arrondie à 128 pixels. Elle est gardée d'une image à l'autre et
  **redessinée seulement si sa signature change** (effets et réglages, version du dessin d'origine) : une image fixe, un
  plan arrêté, une souris qui bouge ne dessinent aucune passe (essai `only_changed_effects_are_redrawn`).
- **Calques d'ajustement** (`calqueffet.rs`) hors dossier : le repère reste dans le plan ; à sa place dans la pile, la
  composition passe en lecture, la chaîne de passes s'applique à l'image composée jusque-là (papier compris), et le
  résultat est posé dessus avec l'opacité et le mode du calque (`compose_span`). Plus de dessin dérivé, plus de
  copie, plus d'attente : l'image n'est plus montrée « sans l'effet » en attendant.
- **Tout est à pleine résolution** : l'aperçu réduit (`fxproxy.rs`) ne sert plus aux piles qui passent par la carte.
- **Les passes sont exactes** : mêmes formules, mêmes arrondis (textures de travail en 8 bits comme les dessins,
  sommes entières pour les flous et la mosaïque), même hasard (bruit, grain, rayons et lignes de l'image d'impact : la
  même fonction de hachage en WGSL, `hash01`, `noise_at`, `value_noise`). Le **flou gaussien** est celui du processeur :
  trois flous en boîte en lignes, trois en colonnes, avec la même perte d'énergie au bord de la feuille (un noyau unique
  — la convolution des trois boîtes — avait donné jusqu'à 40 niveaux d'écart sur les bords de la feuille, où le
  processeur coupe chaque passe à la limite de la zone).

### 12.2 Écart avec le processeur (essais, feuille 640 × 360 : disque, aplats, trait fin, bords collés à la feuille)

Niveaux de 0 à 255, sur les pixels qui portent quelque chose d'un côté ou de l'autre. Seuil des essais : moyenne ≤ 0,5,
maximum ≤ 4 (sauf la composition entière avec transformation, 1 % de pixels au-delà de 2).

| Effet | Moyenne | Maximum |
|---|---|---|
| flou gaussien (4 à 60 px), lueur, ombre portée, aberration chromatique, négatif, seuil, éclairage, dégradé, mosaïque | 0 | 0 à 1 |
| luminosité / contraste, niveaux, teinte / saturation, noir et blanc | 0,003 à 0,09 | 1 |
| bruit, grain de film (même hasard) | 0,02 | 1 |
| netteté, flou de mouvement, secousse (avec flou de bougé), vignettage | < 0,01 | 1 |
| image d'impact (négatif, noir et blanc, inversé, postérisé ; éclat, lignes) | 0,001 à 0,035 | 1 |
| flash en fondu / en écran | 0 / 0,32 | 0 / 1 |
| postérisation | 0,2 | 2 |
| lueur diffuse (6 à 110 px) | 0,005 à 0,01 | 1 à 2 |
| onde, ondulation, tourbillon, pincement, nuages | 0 | 1 |
| piles (flou + bruit + teinte ; impact + lueur + secousse ; secousse + grain + vignettage) | 0,013 à 0,042 | 1 à 2 |
| calque d'ajustement (lueur + vignettage ; flou + grain à 60 % ; flash en Produit ; secousse + aberration), composition entière | 0,017 à 0,032 | 1 à 4 (79 pixels sur 230 400 au-delà de 2 pour la fusion Produit) |
| pile avec opacité 170, Produit et transformation (rotation 6°, échelle 0,9) | 0,039 | 26 sur 189 pixels (lissage de la carte aux bords du calque tourné, pas l'effet) |
| **les deux scènes d'Anthony en 3840 × 2160**, composition entière contre `render::composite_full` (images 1, 22, 40 de la scène 1 ; 12, 25, 38 de la scène 2), 8 294 400 pixels | 0 à 0,007 (0,18 sur l'image 12, l'image d'impact) | 1 à 2, aucun pixel à plus de 2 niveaux |

*Piste non terminée* : composer un calque d'ajustement **dans un dossier** sur la carte (calques du dossier dans une image
à part, effets, pose en mode normal). Un essai donnait, sous charge (suite complète en parallèle), des écarts intermittents
énormes avec le processeur de référence (cause non trouvée : état commun du cache des effets entre essais, ou course) ; non
livré, ces calques restent résolus sur le processeur.

*Cause trouvée (2026-10-20, branche `gpufx-essai`)* : c'est le processeur de référence qui perdait l'image, pas la carte.
`calqueffet::resolve` rangeait le dessin dérivé d'un calque d'ajustement dans la réserve des effets (`fxcache`) et n'en
rendait que le numéro ; la composition (`render::composite_full`, l'export, les vignettes) le relisait ensuite, tuile par
tuile, par ce numéro. Si d'autres calculs remplissaient la réserve entre-temps (les essais voisins du même processus, le fil
de fond, un autre onglet), le dessin en sortait et le calque disparaissait de l'image avec tout ce qu'il couvre :
`the_users_scenes_match_the_cpu_at_full_size` échouait alors sur l'image 12 de la scène 2 (moyenne 94,7, les 8 294 400
pixels à plus de 2 niveaux), reproduit à coup sûr avec un fil qui remplit la réserve pendant l'essai. Le plan résolu tient
maintenant ses dessins dérivés jusqu'à la fin de sa composition (`calqueffet::Resolu`, `Tenus` ; `derived` les y trouve) ;
essai `calqueffet_tests::a_resolved_plan_keeps_its_derived_drawings_when_the_cache_lets_them_go`. La piste du dossier
sur la carte peut reprendre sur cette base.

Les écarts de 1 niveau sont les arrondis de la carte (le rond « au plus proche » du 8 bits) contre ceux du
processeur, aux demi-niveaux exacts.

### 12.3 Ce qui reste sur le processeur, et la règle de mélange

- **Le tremblement du trait** (`tremblement.rs`, calque vectoriel seulement) *était* l'unique effet sans passe : il
  redessine le dessin déjà rendu à travers un champ de déplacement, il n'a donc besoin d'aucune géométrie et a sa passe
  depuis le 2026-10-20 (`fs_boil`, section 13.6). **Il ne force plus la pile sur le processeur.**
- **Une pile qui contient un effet sans passe est calculée sur le processeur.** L'ordre des effets compte et le calcul sur
  processeur ne sait pas partir d'une texture de la carte : mélanger (carte puis processeur) demanderait de relire
  l'image à chaque étape. Aujourd'hui les seuls effets sans passe sont **les effets du trait** (`fxtrait.rs`, section 13),
  et ils agissent toujours les premiers, sur le dessin vectoriel : la pile est alors **coupée en deux**
  (`fx::hybrid_split`) — le fil de fond calcule le dessin dérivé par les effets du trait seuls, la carte passe tous les
  autres effets dessus, à pleine résolution. Plus aucune pile d'effets « ordinaires » n'est entièrement sur le processeur.
- Un calque rempli par CTG, un fond sous le trait, une image de vidéo, une feuille flottante en cours de geste :
  chemin d'avant (la carte n'a que leur dessin dérivé).
- **Un calque d'ajustement dans un dossier** (il ne couvre que les calques de son dossier), ou à côté d'un
  calque d'ajustement qui ne passe pas : tous les repères du plan sont résolus sur le processeur
  (`gpufx::adjustments_supported`), comme avant. Reste à faire si on en a besoin : composer le dossier à part.
- **L'export, les vignettes, la pipette, le pot de peinture** : calcul sur processeur, exact, inchangé (les vignettes
  enregistrées et l'export n'ont jamais passé par l'écran). Sans carte graphique (OpenGL de secours), ou si le shader est
  refusé au démarrage (`GpuFx::try_new`, message sur la sortie d'erreur), ou réglage éteint : le chemin d'avant,
  tel quel.

### 12.4 Réglage

« Effets sur la carte graphique » (Fichier > Préférences… > Performance, `effets_carte_graphique` dans
`reglages.toml`), allumé par défaut ; onzième réglage du diagnostic. Qualité : allumé. Équilibré : allumé sauf rendu
logiciel. Performance : allumé seulement sur une carte dédiée (un graphisme intégré ou virtuel n'a pas la puissance
pour 8 millions de pixels à chaque image ; le calcul réduit au quart peut faire mieux). La case est grisée
(« non disponible sur cette carte graphique ») si le shader n'a pas été accepté. Éteindre ou rallumer joue tout de suite.

### 12.5 Mesures

Poste de référence (Core i7-10700K, RTX 2080 SUPER, pilote d'origine), 2026-10-19, feuille 3840 × 2160, cadre 1920 × 1080,
24 images/s, dessins réels de la scène d'Anthony (`TEST PELURE_effets`, copie lue en lecture seule). Commandes :

```
cargo test --release --offline --target-dir target\msvc --lib gpufxperf -- --ignored --nocapture --test-threads=1
cargo test --release --offline --target-dir target\msvc --lib -- mesure_scene_calcul_de_fond mesure_calques_effets_en_lecture --ignored --nocapture --test-threads=1
```

Le temps de la carte est **mesuré en entier** : `sync` (envoi des tuiles, plan), passes d'effets, composition, affichage
d'un écran de 1920 × 1080, attente de la fin du travail de la carte. Pas de relecture de l'image.

| Mesure | Avant (calcul de fond, aperçu) | Après (carte graphique, pleine résolution) |
|---|---|---|
| **Scène 1** : flou animé 8 → 130 px + luminosité sur le vecto ; bruit animé + teinte / saturation + fusion sur le bitmap. Lecture, 47 images | aperçu réduit en lecture (1/2 sous 80 % de zoom, 1/4 sous 35 %) ; 1ᵉʳ tour à froid : aucune image avec ses effets (0 sur 47) ; 24 images/s tenues une fois le cache chaud | **2,9 ms par image** en moyenne (2,85 médiane, 5,1 p95, 5,7 pire) = 340 images/s ; toutes les images avec leurs effets dès le 1ᵉʳ tour (157 ms en tout au 1ᵉʳ tour : envoi des dessins et pipelines) |
| Scène 1, curseur du flou (rayon change, image fixe) | résultat après 17 ms (pleine) ou 11 ms (aperçu) | **3,5 ms** (pleine résolution) |
| Scène 1, curseur du bruit | résultat après 50 ms (pleine) ou 18 ms (aperçu) | **0,9 ms** |
| **Scène 2** : image d'impact, lueur qui pulse, secousse avec flou de bougé (images 20 à 43), timecode, en calques d'ajustement par-dessus deux dessins. Lecture, 48 images | débit du fil de fond, aperçu au quart : 27 à 31 images/s (33 à 37 ms par image ; 12 images/s à froid) ; pleine résolution au repos : 308 ms par image ; export exact : 310 ms | **1,5 ms par image** en moyenne (2,5 pire) ; images chargées (20 à 43) : 1,8 ms ; 680 images/s |
| Mémoire vidéo, scène 1 | dessins dérivés envoyés (jusqu'au cache de 1 Go) | dessins 82 Mo + effets 218 Mo (textures résultat et de travail) |
| Mémoire vidéo, scène 2 | idem | dessins 24 Mo + effets 37 Mo |

Un effet à la fois sur le grand bitmap de la scène 1 (859 tuiles ; ms par image pour redessiner l'effet, image fixe : 0,16
à 0,22 ms avec la composition gardée) :

| Effet | ms | Effet | ms |
|---|---|---|---|
| luminosité / contraste | 0,6 | netteté | 1,0 |
| niveaux | 0,6 | ombre portée (20, 12 px de flou) | 2,5 |
| teinte / saturation | 0,6 | lueur (20 px) | 3,1 |
| bruit | 0,6 | flou gaussien 40 px | 4,4 |
| image d'impact | 0,8 | flou de mouvement 120 px | 4,3 |
| aberration chromatique | 0,8 | lueur diffuse 60 px | 1,1 |
| secousse avec flou de bougé | 1,3 | flou gaussien 130 px | 11,7 |

**Pire cas** (un dessin opaque qui couvre toute la feuille de 3840 × 2160, effets redessinés à chaque image) :
flou 190 px : 24 ms ; flou 100 + lueur 60 + lueur diffuse 150 + secousse avec flou + teinte + bruit : 22 ms ;
flou de mouvement 300 + ombre portée + netteté : 15 ms. Les 24 images/s (41,7 ms) tiennent même là ; mémoire vidéo
jusqu'à 133 Mo de dessins + 200 Mo d'effets.

### 12.6 Risques et limites

- **Mémoire vidéo** : une texture résultat par calque à effets (jusqu'à 33 Mo pour une feuille 4K entière), comptée dans
  le cache de la carte (réglage « Cache de la carte graphique ») et chassée comme les autres, plus des textures de travail
  (256 Mo au plus, libérées après 8 s d'inutilité). Une animation image par image où chaque dessin est différent a une
  texture résultat par dessin affiché.
- **Le premier affichage d'un effet** fabrique son pipeline (5 à 30 ms) si le fil de préparation du démarrage n'a pas fini ;
  d'ordinaire déjà prêts.
- **Flou de 130 px et plus sur un très grand dessin** : jusqu'à 25 ms sur cette carte (une carte plus faible ferait moins de
  24 images/s) ; l'aperçu réduit sur processeur reste disponible en éteignant le réglage.
- **Les calques d'ajustement dans un dossier** retombent sur le calcul de fond (12.3).
- **Pas vu à l'écran** (aucune application lancée dans cette séance, les essais comparent des pixels) : voir 12.7.

### 12.7 À regarder à l'écran

- Une pile de la scène 1 (flou animé, bruit, teinte, fusion) en lecture : l'image doit être la même qu'avant au repos, sans
  l'instant « sans effet » quand on change de réglage ; un curseur de flou, de bruit, de lueur, de grain : suit la souris sans
  retard.
- Les sept effets du menu Cinéma sur un calque et en calque d'ajustement (impact, lueur, secousse avec flou, vignettage,
  grain, aberration, flash) : mêmes images qu'avant, un grain qui change à chaque image, une secousse qui suit ses clés.
- Une pile avec un tremblement du trait puis un flou : elle est désormais dessinée par la carte (section 13.6) ; ajouter ou
  retirer le tremblement ne change pas l'aspect au repos.
- Un calque d'ajustement dans un dossier : même rendu que hors dossier, un peu moins fluide.
- Éteindre « Effets sur la carte graphique » dans les Préférences, en lecture : retour au calcul de fond sans clignotement
  (l'image reste, l'aperçu réduit revient) ; le rallumer.
- Le diagnostic : la ligne « Effets sur la carte graphique » dans la liste des réglages et dans le rapport.
- Mémoire vidéo dans le Gestionnaire des tâches pendant une longue lecture d'un plan où chaque dessin a des effets.

## 13. Effets du trait, et le tremblement sur la carte graphique (2026-10-20, branche `effets-trait`)

Demande d'Anthony : « le tremblement, j'adore » — encore des effets qui agissent sur le trait lui-même (écriture, crayonné
multiple, respiration, ondulation le long du trait, pointillés qui défilent, désagrégation), et si possible le tremblement
sur la carte graphique (c'était le seul effet sans passe : il obligeait les piles qui le contenaient à rester sur le
processeur). Code : `fxtrait.rs` (effets, essais `fxtrait_tests.rs`), `gpufx.wgsl` (`fs_boil`), `fx.rs`
(`apply_with`, `hybrid_split`), `fxcache.rs` (le dessin vectoriel suit le travail de fond). La règle de la passe tient :
**le fil de l'interface ne calcule jamais un de ces effets** (ni ne rejoue le moteur de brosses).

### 13.1 Comment ça marche

- **Les effets du trait lisent la géométrie du dessin vectoriel**, pas ses pixels. Le fil de fond (`fxcache.rs`) reçoit
  avec le dessin d'origine une copie du `VectorDrawing` (`fxcache::vector_source`, une fois par version du dessin ; les
  pixels gardés de chaque trait sont partagés, la copie ne coûte que les points : 0,5 à 1 ms pour 2 000 traits, **sur le
  fil de l'interface, une fois par retouche du dessin**). `fx::apply_with` passe d'abord les effets du trait, quelle que
  soit leur place dans la pile, puis les autres effets sur l'image qu'ils ont faite.
- **Chaque trait est rendu seul depuis son cache** (les pixels exacts, `vector.rs`), puis transformé **en pixels** : le
  moteur de brosses n'est rejoué qu'une fois par trait (en arrière-plan, comme à l'ouverture d'un projet ; 1,1 s pour 200
  traits de gouache, fusain et feutre) et jamais à chaque image — une gouache qui coûte 2,5 s au rejeu coûte, à chaque
  image, la même chose qu'un trait lisse. La texture de la brosse suit la déformation.
- **L'axe du trait** (rééchantillonné tous les 2 px) et une carte « point de l'axe le plus proche » (calculée une fois par
  trait, en traçant le disque de chaque point) donnent à chaque pixel son abscisse curviligne, sa normale et le rayon.
- **Un trait que l'effet ne touche pas n'est pas calculé** : écriture « l'un après l'autre » à 50 %, 1 000 traits sont
  posés tels quels depuis leur cache, 1 000 sautés, un seul est transformé.
- **Réparti sur les cœurs, déterministe** : morceaux de 96 éléments transformés en parallèle (`parallele::par_map`, le
  fil de fond laisse deux cœurs à l'interface), puis posés **par bandes de tuiles** (`compose_banded`) : chaque pixel
  reçoit les couches dans l'ordre des traits, comme avec un seul fil (essai : un fil ou plusieurs, mêmes pixels). Le
  hasard (crayonné, poussière) vient d'un hachage entier de (graine, trait, passe, variante).
- **Dessins dérivés** : un par jeu de réglages ; les valeurs d'une image portent sa variante (crayonné) ou son numéro
  (respiration, ondulation, pointillés), donc chaque image est un dessin dérivé à part, gardé par le cache de 768 Mo,
  demandé d'avance en lecture (`playback::upcoming_derived`) et montré à l'identique à l'export et dans les vignettes.
  En attendant : le dernier résultat du même dessin, ou le dessin tel quel (jamais un résultat d'avant la dernière retouche).
- **Pas d'aperçu réduit** pour ces effets (`fxproxy::proxyable` : leurs longueurs sont en pixels de la feuille).
- **Pendant qu'on dessine** sur le calque (geste de dessin, de gomme, de sélection, de transformation), ses effets du trait
  et son tremblement se taisent (`fx::without_boil`) : on prend ce qu'on voit.

### 13.2 Mesures

`cargo test --release --offline --target-dir target\msvc --lib fxtrait_tests::measure -- --ignored --nocapture --test-threads=1`.
Poste de référence (Core i7-10700K, 16 fils, RTX 2080 SUPER, d'autres compilations tournaient en même temps : les
chiffres varient de ± 30 %), 2026-10-20. Calque vectoriel dense sur une feuille de 1920 × 1080 : **2 000 traits** (1 800
lisses, 200 de feutre, fusain et gouache de 120 à 400 px) ; le travail est celui du fil de fond, caches chauds, image par
image (valeurs d'une image différente à chaque fois). « Dessin sans effet » : `VectorDrawing::render`, 60 ms ; la première
fois 1,1 s (rejeu des 200 traits texturés).

| Effet (2 000 traits) | ms par image | 200 traits | Remarque |
|---|---|---|---|
| Écriture 50 %, un trait après l'autre | 34 | 5 | un seul trait transformé, le reste posé ou sauté |
| Écriture 50 %, tous ensemble | 85 | 10 | chaque trait est transformé |
| Écriture 100 % (rien à faire) | 59 | 8 | tout est posé tel quel : le coût du dessin |
| Crayonné, 2 passes en plus | 174 | 21 | 3 variantes seulement : les 3 images calculées, la lecture n'attend plus |
| Crayonné, 4 passes, décalage 8 | 345 | 49 | |
| Respiration (35 %) | 114 | 17 | |
| Ondulation du trait (4 px) | 139 | 24 | |
| Pointillés | 68 | 11 | |
| Désagrégation 40 % | 123 | 16 | |
| Désagrégation 40 %, selon le sens du trait, 300 px | 277 | 50 | les grains tombent sur beaucoup de tuiles |
| Écriture 80 % + respiration + ondulation + crayonné | 300 | 39 | tout à la fois |

**Fil de l'interface** (même dessin, 2 000 traits, calque à effet du trait) : copie du dessin vectoriel 0,5 à 1,1 ms,
une fois par version du dessin (une retouche) ; ensuite 0,000 ms ; demande d'un dessin dérivé 0,08 ms ; image dont le
dessin dérivé est prêt **0,000 ms (pire 0,002)**. Aucun rejeu du moteur sur ce fil (essai
`playing_a_written_plan_never_replays_the_engine_on_the_interface_thread`).

À lire ainsi : à 24 images par seconde il faut 41,7 ms par image. Sur un dessin de 200 traits (un storyboard ou un plan
d'animation ordinaire) tous les effets tiennent, y compris tous empilés (39 ms). Sur 2 000 traits, les effets dont chaque
image est un calcul à part (écriture, respiration, ondulation, pointillés, désagrégation) tournent à 4 à 30 images par
seconde en arrière-plan : la lecture du premier tour montre l'image d'avant en attendant, les images qui viennent sont
demandées d'avance, et le deuxième tour est servi par le cache. Le crayonné, lui, n'a que 3 variantes (réglable) : après
trois images il est fluide.

### 13.3 Règle de mélange

Les effets du trait sont les seuls effets sans passe sur la carte. Une pile qui en contient est **coupée en deux**
(`fx::hybrid_split`, `gpu.rs`) : le fil de fond calcule le dessin dérivé par les effets du trait seuls (envoyé à la carte
comme n'importe quel dessin dérivé, `playback` le demande d'avance), puis la carte passe les autres effets dessus à pleine
résolution, à chaque image. Essai `a_mixed_stack_gives_the_cpu_pixels_with_the_card_drawing_the_card_effects` : écriture
+ flou + tremblement, aucun pixel à plus de 2 niveaux de la référence sur processeur. Une pile sans autre effet que
ceux du trait reste un dessin dérivé (comme avant). Un calque CTG, un fond sous le trait, une vidéo, une feuille
flottante : chemin d'avant. L'export, les vignettes et le repli sans carte : calcul exact sur processeur.

### 13.4 Le tremblement du trait sur la carte graphique

`fs_boil` (`gpufx.wgsl`) : le même champ que `tremblement::Field` — mêmes nœuds (hachage entier de (graine de la
variante, nœud, canal), norme au plus 1), même interpolation 3t² − 2t³, même lecture bilinéaire des pixels du dessin, même
variation d'épaisseur (puissance de la couverture) — la graine (variante comprise) calculée par le processeur. Écart avec
le processeur (`gpufx_tests::the_boil_matches_the_cpu`, feuille 640 × 360) : **moyenne 0,000, maximum 1 niveau**, aucun
pixel à plus de 2 niveaux, pour trois variantes, intensités de 0 à 14 px, fréquences de 5 à 90 %, épaisseur de 0 à 100 %,
et en pile avec un flou et une teinte (moyenne 0,043, maximum 2).

| Mesure (2 000 traits, 1920 × 1080, 24 images, 3 variantes) | Avant (fil de fond, 16 fils) | Après (carte, pleine résolution) |
|---|---|---|
| Une variante | 11 à 12 ms (3,4 Mo d'images dérivées par variante, envoyées à la carte) | une passe de dessin |
| Lecture, moyenne sur 144 images (`gpufxperf_tests::mesure_tremblement_gpu_contre_cpu`) | variantes servies par le cache des dessins dérivés, en aperçu réduit en lecture ; pleine résolution au repos | **0,22 ms par image** (pire 1,5 ms) à pleine résolution ; mémoire vidéo : dessin 17 Mo + 8 Mo d'effets |
| « Animer à l'arrêt » | variantes demandées d'avance au fil de fond | rien à demander : la passe se redessine au changement de variante |

### 13.5 Risques et limites

- **Mémoire** : une image dérivée par effet du trait et par image, jusqu'au cache de 768 Mo (un dessin dense couvert d'encre
  pèse quelques Mo) ; la copie du dessin vectoriel pour le fil de fond (jusqu'à 8 copies gardées) pèse les points des
  traits, pas leurs pixels.
- **2 000 traits à 24 images par seconde en temps réel** : pas tenu par les effets qui changent à chaque image (calcul de
  fond de 70 à 300 ms par image) ; le plan et l'interface restent fluides, les images arrivent plus tard.
  Pistes : carte des points les plus proches gardée par version du dessin ; calcul du seul cadre de la caméra ; une version
  sur la carte graphique de l'écriture (masque par trait envoyé en texture).
- **Un trait que le moteur n'a jamais rendu** (projet qui vient de s'ouvrir) est rejoué par le fil de fond au premier calcul
  d'un effet du trait : 1,1 s pour 200 traits texturés, une seule fois.
- **L'abscisse d'un pixel est celle du point de l'axe le plus proche** : une brosse très large qui se recoupe elle-même
  (une boucle serrée) peut se démasquer dans un autre ordre que celui de la plume.
- **Pas vu à l'écran** dans cette séance (aucune application lancée, consigne) : voir 13.6.

### 13.6 À regarder à l'écran

- Un calque vectoriel avec un visage et un décor : « Écriture du trait » avec les clés 0 → 100 % sur une seconde (le
  préréglage « Apparition écrite ») : le dessin se trace trait par trait, la pointe se fond, l'image fixe à 100 % est le
  dessin d'avant ; avec « Tous les traits ensemble », tous avancent à la fois ; « Effacement » le défait dans l'ordre.
- Pendant qu'on dessine sur ce calque, le trait posé se voit en entier (l'effet se tait le temps du geste), puis l'effet
  revient sans clignotement.
- « Crayonné multiple » : des passes pâles de part et d'autre du trait, qui changent toutes les deux images ; « Variantes »
  et « Vitesse » comme le tremblement.
- « Respiration » (le trait pulse, avec ou sans vague le long du trait), « Ondulation du trait » (bouts fixes, vague qui
  avance), « Pointillés » (tirets qui défilent dans les deux sens), « Désagrégation » (poussière, sens du trait, vent) : en
  lecture, en défilement, au curseur.
- Le tremblement du trait sur un dessin chargé : même aspect qu'avant (au repos et en lecture), « Animer à l'arrêt » sans
  à-coup, une pile « tremblement + flou + lueur » sans l'instant « sans effet » quand on change un réglage.
- Une pile « écriture du trait + flou » : le flou se règle au curseur sans retard (la carte), l'écriture suit ses clés.
- Éteindre « Effets sur la carte graphique » (Préférences) : retour au calcul de fond, mêmes images.

## 14. Effets de matière (2026-10-20, branche `effets-matiere`)

Cinq effets de plus du trait : liseré, trait sec, encre qui bave, dégradé le long du trait, changer de plume
(`fxmatiere.rs`, essais `fxmatiere_tests.rs`). La règle de la passe tient : **le fil de l'interface ne calcule rien de ces
effets** ; ils passent par le calcul de fond et les dessins dérivés de la section 13.

### 14.1 Comment ça marche

- **Liseré, encre qui bave, trait sec sur un bitmap** : opérations **sur les pixels** du dessin (vectoriel ou bitmap : le même
  code, dans `fx::apply_with`, à leur place dans la pile). Elles lisent une **distance euclidienne exacte** aux pixels d'encre
  (couverture de 25 % ou plus) : transformée de Felzenszwalb et Huttenlocher, une passe par colonnes puis une par lignes,
  réparties sur les cœurs (colonnes par paquets de 32, lignes par paquets de 8 ; chaque ligne et chaque colonne se calcule
  seule, le résultat ne dépend pas du nombre de fils : essai contre une recherche par force brute). Seule la boîte de l'encre
  plus la portée de l'effet est traitée. Le hasard (bruit de valeur, grain) est un hachage entier de la position : aucune
  dépendance au découpage en tuiles.
- **Trait sec, dégradé, plume sur un calque vectoriel** : dans `fxtrait::render`, par trait, avec l'axe et la carte « point de
  l'axe le plus proche » de la section 13 (le rayon du trait au point s'y ajoute). Posés par bandes de tuiles, dans l'ordre.
- **Changer de plume** : chaque trait est redessiné avec le réglage choisi, **gardé** (clé = tracé, appuis, couleur, toucher ;
  réglage final) : le moteur de brosses est rejoué une fois par trait et par plume, jamais par image. Les rayons des points
  sont refaits d'après la plume (la boîte du trait en dépend). Les pixels sont gardés par le cache des traits (budget de
  `vector.rs`).
- **Règle de mélange** : le dégradé, le trait sec et la plume lisent les traits : ils passent d'abord (`hybrid_split`), la
  carte graphique fait le reste. Le liseré et l'encre gardent leur place dans la pile, donc une pile qui en contient est un
  **calcul de fond en entier** (l'ordre des effets compte : un flou avant ou après un liseré ne donne pas la même image).
- **Pas d'aperçu réduit** (`fxproxy::proxyable` : longueurs en pixels de la feuille).

### 14.2 Pourquoi pas de passe sur la carte graphique

Décision de cette série, mesures à l'appui (14.3) : une dilatation par un disque n'est pas séparable (une approximation par
un flou seuillé change la largeur), et tenir deux niveaux d'écart avec le calcul sur processeur demandait **le même
algorithme** des deux côtés — une propagation de distances en plusieurs passes (inondation par sauts) dont l'arrondi diffère
de la transformée exacte. Le calcul de fond coûte 19 à 77 ms (dessin au trait) : de l'ordre d'une image à 24 par seconde, pour
un réglage qui change rarement. Si les curseurs du liseré paraissent mous à l'écran, la piste est une version par inondation
sur la carte, comparée à la transformée exacte (non faite). Aucun accord carte / processeur n'est donc mesuré dans cette
série ; l'essai `a_stack_with_the_pixel_effects_stays_on_the_processor_and_the_vector_ones_split_off` fige la règle.

### 14.3 Mesures

`cargo test --release --offline --target-dir target\msvc --lib fxmatiere_tests::measure -- --ignored --nocapture --test-threads=1`.
Poste de référence (Core i7-10700K, 16 fils, RTX 2080 SUPER ; ± 30 %), 2026-10-20, chaque ligne : moyenne de 3 à 5 images.

Calque vectoriel de **2 000 traits** (1 800 lisses, 200 de feutre, fusain et gouache), feuille 1920 × 1080 (le dessin seul :
60 ms) :

| Effet | ms par image | Remarque |
|---|---|---|
| Liseré 4 px | 19 | sur les pixels du calque |
| Liseré 20 px, douceur 10 | 19 | la portée ne change pas le coût (transformée exacte) |
| Encre qui bave (2,5 px, bord foncé) | 40 | deux distances (dehors et dedans) |
| Trait sec, par trait | 78 | |
| Trait sec, retiré toutes les 2 images | 77 | seule la variante change |
| Dégradé, par trait | 71 | |
| Dégradé sur tout le dessin qui défile | 71 | un dessin dérivé par image |
| Changer de plume (Feutre) | 262 la première fois, 47 ensuite | rejeu des traits une seule fois |
| Trait sec + dégradé | 84 | |
| Trait sec + dégradé + liseré + encre | 135 | |

Calque bitmap **4K** (3840 × 2160), un seul calcul de fond :

| Contenu | Liseré 4 px | Liseré 20 px | Encre qui bave | Trait sec (bitmap) |
|---|---|---|---|---|
| Dessin au trait (2 000 traits) | 76 ms | 77 ms | 181 ms | 90 ms |
| Grande forme pleine | 42 ms | 43 ms | 71 ms | 76 ms |
| Feuille entière pleine | 77 ms | 77 ms | 96 ms | 124 ms |

**Fil de l'interface** : rien de ces effets n'y est calculé. Ce qui y reste : `fx::steps_at` ajoute la signature de la plume
(une recherche dans une courte liste) ou l'image du défilement ; la demande d'un dessin dérivé coûte 0,08 ms (section 13).
La **plume** garde ses traits redessinés dans une table (jusqu'à 12 000 traits ; leurs pixels sont sous le budget du cache des
traits) : la copie du dessin vectoriel pour le fond reste de 0,5 à 1 ms par retouche. « Appliquer pour de bon » ne rejoue pas
sur le fil de l'interface : les traits déjà redessinés par l'effet gardent leurs pixels, les autres sont rendus en rapide
(`render_quick`) puis exactement en arrière-plan (`rejeu.rs`).

À lire ainsi : à 24 images par seconde il faut 41,7 ms par image. Un dessin ordinaire (200 traits) tient avec tous ces effets.
À 2 000 traits, les effets qui changent à chaque image (dégradé qui défile) tournent à 12 images par seconde en arrière-plan,
les images arrivent un peu après la première fois et le deuxième tour est servi par le cache ; un liseré, une bavure ou un
trait sec fixes (une seule image à calculer) sont prêts en moins d'un quart de seconde, y compris sur une feuille 4K.

### 14.4 Risques et limites

- **Mémoire** : la transformée garde deux tableaux de 8 octets par pixel de la zone traitée (133 Mo pour une feuille 4K
  entièrement couverte, 17 Mo pour 1080p), libérés à la fin du calcul ; plus une image dérivée par effet et par image dans le
  cache de 768 Mo.
- **Bitmap** : le trait sec y est une version simplifiée (pas de trous le long d'un trait) ; le dégradé et la plume n'y sont pas
  (aucun trait).
- Le bord de l'encre est le contour à 25 % de couverture : un pinceau très doux (aérographe) a un liseré qui suit ce contour.
- **Pas vu à l'écran** dans cette séance (aucune application lancée, consigne) : voir `docs/INTERFACE.md` section 21 ; planche de
  contact : `$env:FXMATIERE_PNG = '...'` puis `fxmatiere_tests::dump -- --ignored`.

## 15. Onglets de projets (2026-10-20, branche `onglets`)

Plusieurs projets ouverts dans la même fenêtre (`docs/ARCHITECTURE.md`, « Onglets de projets »). Mesures de
`onglets_tests::perf_tabs` (`cargo test --release --offline --target-dir target\msvc perf_tabs -- --ignored --nocapture
--test-threads=1`, carte graphique réelle sans fenêtre, écran de 1920 × 1080, projets de 30 plans × 3 calques × 24 images,
6 tuiles par dessin) :

| Mesure | Résultat |
|---|---|
| Changer d'onglet (l'échange et le rangement des caches, hors image d'écran) | **7 à 14 µs** médiane, 19 à 37 µs au pire (60 changements entre 4 onglets) |
| L'image d'écran qui suit le changement (interface complète + composition par la carte) | **1,9 ms** médiane (p95 2,1 ms) contre 1,8 ms (p95 2,2 ms) pour une image d'écran ordinaire de ces projets, machine au repos ; sur une machine chargée : 5,1 contre 4,4 ms. Le changement ne coûte donc pas une image de plus |
| Lecture disque pendant un changement | aucune (essai `switching_does_not_read_the_disk_again` : le dossier du projet est effacé, les changements marchent) |
| Mémoire d'un projet de plus | **+54 Mo** par projet de ce genre (284 Mo avec un projet, 336 / 391 / 448 Mo avec 2 / 3 / 4 ; le processus de départ en compte 9) |
| Processeur au repos, quatre onglets ouverts | **0 ms** en 3 s sans image d'écran (aucun fil de fond ne travaille ; 16 ms une fois, juste après des calculs de fond encore en route) |
| Repeinture au repos, trois onglets | aucune : `reveil` ≥ 1 s à chaque image (essai `three_open_tabs_at_rest_ask_for_no_repaint`, y compris après six changements d'onglet) |
| Changer d'onglet sans interface, 480 dessins par projet (200 changements) | 0,001 ms en moyenne (borne de l'essai `switching_is_a_swap_not_a_reload` : 25 ms) |

Pourquoi c'est instantané : il n'y a ni relecture, ni copie de dessin, ni recalcul. Les 139 champs d'un projet changent de
place par `std::mem::swap` ; les dessins dérivés des effets (`fxcache::Reserve`) et les textures de la carte graphique
(`gpu::TexturesDeProjet`) sont rangés avec leur onglet (des `HashMap` qui changent de propriétaire) ; seule l'image composée
gardée est abandonnée, la première image d'écran du nouvel onglet la refait (une composition, comme à chaque changement de plan).

**Pour un onglet caché** : aucune image d'écran, aucun calcul sur le fil de l'interface. Ses vignettes en file sont abandonnées
(`Thumbs::abandonner`), sa lecture arrêtée. Il ne réveille l'application que pour une écriture ou une copie de secours en cours
(`tick_onglets`, 200 ms). Les calculs de fond qu'il avait demandés (effets, rejeu des traits) finissent seuls ou sont jetés à
leur arrivée (le « monde » des effets change à chaque changement d'onglet).

**Mémoire des onglets cachés** : la moitié de la limite des dessins dérivés (384 Mo) et la moitié du budget de textures de la
carte graphique (500 Mo), tous onglets cachés ensemble ; les plus anciennement vus sont lâchés d'abord (ils se recalculent au
retour). L'essai `the_caches_of_hidden_tabs_stay_within_the_effect_cache_limit` le vérifie avec une limite d'un mégaoctet.

À regarder à l'écran : ouvrir quatre projets lourds, passer de l'un à l'autre à la souris et à Ctrl+Tab : aucun à-coup, la
lecture et les clés reviennent là où elles étaient.

## 16. Notes de dessin : ce que coûtent les nouveaux gestes (2026-10-20, branche `notes-dessin`)

Règle « sans latence » : aucun de ces gestes ne fait de travail lourd sur le fil de l'interface PENDANT le geste.

| Geste | Pendant le geste | Au relâché / au clic |
|---|---|---|
| **Tracé droit** (Maj pendant un trait) | une ligne d'aperçu dessinée par image d'écran ; viser 200 points : **4 µs** (rien n'est calculé, la pointe n'est pas donnée au moteur) | le segment est posé en points réguliers (au plus 3000) : un trait ordinaire, **2,3 ms** pour 380 px au pinceau de 24 px (release, mesuré par l'essai) |
| **Choisir un objet** (Ctrl + clic) | rien (le lasso est celui d'avant) | un parcours des pixels dessinés qui se touchent, une fois, au clic : **96 ms** pour un objet de 1,5 million de pixels (un dessin presque entier ; un objet ordinaire coûte quelques ms) |
| **Remplir la sélection** (Maj+Suppr) | — | seules les tuiles de la zone sont lues et écrites, une étape d'annulation par tuiles |
| **Un pinceau par sorte de calque** | une comparaison de deux réglages par image d'écran (quelques centaines d'octets) ; les réglages sont enregistrés hors geste | — |
| **Clés de position** | la pose d'un calque à l'image demandée est une interpolation de six nombres (`poskeys::pose_at`) par calque et par image : aussi pour la lecture, l'export et les vignettes ; glisser un losange de la timeline ne refait que la liste des clés (pas de rendu avant le lâché) | — |

Le fond de l'aperçu du trait (réglages du pinceau) est une couleur calculée, pas une texture de plus.

## 17. Exports hors du fil de l'interface (2026-10-20, branche `exports-fond`)

Règle d'Anthony : « un logiciel qui réponde toujours très rapidement, sans latence ». Avant, le film, les images, le GIF, le PDF, le
montage, la relecture, la fiche et l'export vers Storyboard Pro calculaient leurs images **sur le fil de l'interface**, 40 ms par
image d'écran (le test se fait entre deux images : une image de 4K finit toujours), derrière une fenêtre modale qui verrouillait le
projet. La page HTML du storyboard, l'image courante et le son seul se calculaient d'un coup, en gelant SHOT.

### Conception (`exportrender.rs`, `export.rs`)

- **Le cliché** (`SnapshotBuilder`). Au départ de l'export, l'interface copie le projet : la **structure** d'un coup (plans,
  calques, caméras, transitions, pistes son ; les plans que l'export ne montre gardent leur durée et leur caméra mais pas leurs
  calques), les **dessins** des plans exportés par tranches de 8 ms au plus (`ExportJob::step`). Copier un dessin ne copie aucun pixel :
  les tuiles sont partagées par `Arc` (comme les vignettes, § 11) ; si on dessine ensuite, seule la tuile touchée est copiée (copie à
  la première écriture, essai `the_snapshot_keeps_the_structure_…`). L'export utilise donc le projet tel qu'il était au départ.
- **Les fils de rendu** (`Pipeline`) : `(cœurs − 2) / 3` fils (1 à 6), chacun répartissant sa composition sur 2 à 3 cœurs
  (`render::set_thread_cap`) ; deux cœurs restent à l'interface. Ils calculent chaque image avec **le code d'avant** (`panel_plan_at`,
  `camera_image`, effets et calques d'effets en calcul bloquant, incrustations gravées, transitions) : les mêmes pixels. Chaque fil
  a sa **réserve privée de dessins dérivés** (`fxcache::avec_privee`, limitée en octets, jamais vidée pendant l'image en cours) : un
  export ne chasse pas de l'écran les dessins que l'interface vient de calculer, et ne range rien dans la réserve commune. Une image
  tenue (même plan, même caméra) n'est calculée **qu'une fois pour tous les fils** (`ImageStore` : le deuxième fil attend le premier).
- **Priorité**. Les fils de rendu, ceux qu'ils lancent (`par_chunks`, `par_map`), les fils d'écriture **et ffmpeg**
  (`BELOW_NORMAL_PRIORITY_CLASS`) sont sous la priorité de l'interface. Mesuré : tout à la priorité normale, la pire image d'écran
  pendant un film 4K passe de 9 ms à 78 ms ; ffmpeg normal avec des fils de rendu bas affame les fils de rendu (43 s au lieu de 9 s
  une fois sur trois) : tous sous la normale, ensemble.
- **Le passeur** : un fil range les images dans l'ordre vers l'écriture (file de 4 images vers les fils PNG, le fil de ffmpeg, les
  fils de réduction du PDF / de la relecture / de la fiche / de la page HTML) ; c'est lui qui attend quand l'écriture est en retard,
  jamais l'interface. Les fils de rendu n'avancent pas de plus de `avance_permise` images devant lui (5 à 10 en 4K) : la mémoire en
  l'air est bornée par la taille d'une image (≈ 500 Mo de pixels en 4K).
- **L'interface** (`ExportJob::step`) : copie le projet au départ, puis lit un compteur toutes les 100 ms (la barre d'avancement).
  Le projet n'est retenu que le temps de la copie (`needs_project`, lu par `modal_open` et `onglet_occupe`) ; ensuite un export
  **continue quand on change d'onglet**, la fenêtre « Export en cours » (non modale) dit de quel projet il s'agit.
- **Le son** (film, WAV) est mixé sur un fil à part, sur le cliché (`mix_clips_blocking` : égaliseur et écho compris), avant que
  ffmpeg parte ; le montage le fait déjà sur un fil. Les **traits texturés restés en rendu rapide** sont refaits exacts dans le
  cliché, sur un autre fil (`SnapshotBuilder::with_exact`). **Storyboard Pro** : les images des calques se calculent par les mêmes fils
  (`Pipeline::start_with`), les sons (égaliseur, écho) sur le fil de Storyboard Pro.
- **Annuler** : le drapeau arrête le passeur tout de suite (il rend la file d'écriture), ffmpeg est tué, les fils de rendu finissent
  leur image en cours et s'en vont sans qu'on les attende (annuler rend la main en quelques millisecondes, même en 4K) ; ce qui n'est
  pas fini est effacé ; le PDF, la page HTML, l'image courante et le son passent par un fichier `.part` renommé à la fin : un fichier
  d'avant n'est jamais touché.

### Mesures

Même machine (Core i7-10700K, 8 cœurs / 16 fils, RTX 2080 SUPER, partagée avec un autre travail de compilation), version release,
`cargo test --release --offline --target-dir target\msvc perf_export -- --ignored --nocapture --test-threads=1` (un essai par
processus : la mémoire est le pic du processus). Projet synthétique en **4K** (`mesures::projet_4k`, feuille 7 680 × 4 320) : 6 calques de
150 tuiles, un flou gaussien, un réglage teinte-saturation, un calque tourné, une caméra qui avance, tourne et zoome ; 48 images, toutes
différentes. L'ancien chemin est reproduit à l'identique dans l'essai (`ancien`) : images sur le fil de l'interface, pas de 40 ms,
mêmes écrivains. Une image d'écran toutes les 16 ms au plus (synchronisation verticale), banc de `perf_tests.rs` avec carte
graphique. « Image d'écran » = construction de l'interface + découpage + carte (+ le pas d'export de l'ancien chemin). Trois séries.

| Export de 48 images 4K | Avant (fil de l'interface) | Après (fils de rendu) |
|---|---|---|
| **Suite de PNG** : image d'écran, moyenne | 105,5 – 106,5 ms | **3,6 – 3,7 ms** |
| — p95 / la pire image d'écran | 143 – 150 ms / 150 – 164 ms | 6,3 – 7,0 ms / **9,9 – 11,6 ms** |
| — durée de l'export | 5,2 s (9,3 images/s) | **4,2 s (11,5 images/s)**, 19 % plus court |
| — mémoire (pic du processus) | 994 – 1 020 Mo | 1 267 – 1 277 Mo (+ 270 Mo) |
| **Film MP4** (ffmpeg, H.264 d'OpenH264) : image d'écran, moyenne | 107,2 – 109,1 ms | **4,3 – 4,5 ms** |
| — p95 / la pire image d'écran | 153 – 154 ms / 158 – 207 ms | 7,3 – 8,5 ms / **10,0 – 10,2 ms** |
| — durée de l'export | 8,6 – 8,7 s (5,5 images/s) | 9,0 – 9,1 s (5,3 images/s) |
| — mémoire (pic du processus) | 956 – 957 Mo | 1 194 – 1 195 Mo (+ 238 Mo) |

L'interface passe d'**une image d'écran toutes les 105 ms (pire 150 à 207 ms) à 4 ms (pire 10 à 12 ms)** pendant l'export, soit celle
d'un SHOT au repos. Les PNG vont plus vite (le rendu et l'encodage des PNG se recouvrent sur plusieurs cœurs). Le film est limité par
ffmpeg (5,3 à 5,5 images/s tout seul) : il n'est pas plus rapide, et **3 à 4 % plus long** — la première image arrive 0,3 s plus tard
(quatre images se calculent côte à côte au lieu d'une sur tous les cœurs) et ffmpeg passe sous la priorité normale. Le prix de la
mémoire (+ 240 à 270 Mo en 4K) est celui des images en l'air (quelques-unes de plus : 33 Mo chacune).

Le cliché (`perf_export_snapshot_cost`, ce que l'interface paie au départ, une fois) :

| Projet | Structure (d'un coup) | Dessins (au total, par tranches) | Plus longue tranche |
|---|---|---|---|
| 30 plans × 3 calques × 24 images, tout le montage | 0,13 ms | 480 dessins en 2,7 ms | 3,0 ms |
| 600 plans × 3 calques × 24 images, tout le montage | 2,0 ms | 9 600 dessins en 54 ms | 8,3 ms |
| 600 plans × 8 calques × 48 images, tout le montage | 8,8 ms (pire 14,8) | 26 400 dessins en 144 ms | 8,4 ms |
| un seul plan de ces projets | 0,01 – 0,17 ms | 0,03 – 0,16 ms | 0,23 ms |

La structure reste sous 15 ms même pour 600 plans de 8 calques de 48 images (4 800 calques) ; les dessins, par tranches de 8 ms, ne
retiennent jamais l'interface plus de 9 ms d'un coup.

### Garde-fous (`exportrender_tests.rs`, 46 essais et 6 mesures ignorées ; la suite complète : 2 072 essais, deux fois de suite, aucun intermittent)

- **Mêmes pixels** : sur le projet d'essai complet de la fiabilité (effets de calque à clés, tremblement du trait, fond sous le trait,
  CTG, vidéo, dossier, fusion, transformation, caméra à clés, calques d'effets d'ajustement, incrustation, damier), chaque image du
  nouvel export est celle de l'ancien chemin, pixel pour pixel, pour les trois papiers, à plusieurs images de chaque plan ; la toile
  infinie et les clés de position, les incrustations gravées, les transitions (un plan seul et tout le montage) aussi.
- **Chaque sorte d'export** : suite de PNG (et PNG sans papier), image courante, film et GIF (un ffmpeg qui recopie ses images brutes :
  l'entrée du film est l'ancien chemin, dans l'ordre), images reçues par `collect` (PDF, relecture, fiche), page HTML (planches
  identiques à l'octet à l'ancien export synchrone), médias du montage (poignées comprises), son (le même WAV que l'ancien mixage).
- **Le projet reste libre** : retoucher tous les dessins, les calques, les caméras et le nombre de plans pendant l'export ne change pas
  un pixel du résultat ; le compteur par fil (`IMAGES_ICI`, comme `render::THUMBNAILS_HERE`) reste à zéro sur le fil de l'interface ;
  une image tenue n'est composée qu'une fois, même demandée par quatre fils ensemble ; l'application revient au repos après l'export
  (`se_reposer`).
- **Annulation** : suite de PNG sans `.part`, film effacé (ffmpeg tué), PDF d'avant intact, planches de la page effacées, film d'avant
  intact si ffmpeg ne démarre pas, lâcher un export en cours l'annule, annuler rend la main en moins de deux secondes même quand
  l'écriture ne répond plus ; une image qui plante arrête l'export avec un message.
- **Onglets** : un export continue pendant qu'un autre onglet est affiché, répond à son projet (`export_projet`), ne retient l'onglet que
  le temps de la copie ; « Quitter » prévient une fois.
- **Mémoire** : l'avance des fils de rendu est bornée par la taille d'une image (`the_memory_in_flight_stays_bounded_…`).

### Risques et limites

- **Mémoire** : + 240 à 270 Mo en 4K par rapport à l'ancien chemin (quelques images de plus en l'air) ; en HD, l'avance autorisée est
  plus grande mais les images sont quatre fois plus petites.
- **Le film n'est pas plus rapide** (ffmpeg limite) et peut être 3 à 4 % plus long ; les exports d'images, de PDF, de pages, de fiches
  gagnent avec les cœurs.
- **Un seul export** (film, images, GIF, PDF, page, image, son) à la fois ; le montage, la fiche, la relecture et Storyboard Pro ont
  chacun le leur.
- **Ce qu'on retouche pendant l'export n'y entre pas** (c'est voulu, la fenêtre le dit). Si le projet est remplacé dans l'onglet pendant
  la copie (quelques images d'écran), la copie serait mélangée : `modal_open` suspend alors les raccourcis et `onglet_occupe` refuse le
  changement d'onglet, mais ouvrir un projet par un menu dans ces 10 à 100 ms n'est pas écarté.
- L'API locale (`noyau.rs`) et la lecture du storyboard par un modèle (`lecturesb.rs`) calculent encore des images sur leurs fils.
- Le film s'écrit directement dans le fichier choisi (pas de `.part` : ffmpeg doit connaître l'extension) ; un film inachevé est effacé.

### À regarder à l'écran

La liste de `docs/INTERFACE.md` § 28 : film 4K avec une interface qui répond, le film est celui du départ (un trait dessiné pendant
l'export n'y est pas), annuler, deux projets, quitter pendant l'export, image courante / page HTML / son sans gel, montage / fiche /
relecture / Storyboard Pro, mémoire.

## 18. Coller depuis un projet d'une autre taille (2026-10-20, branche `coller-adapter`)

Rééchantillonner les dessins d'une copie (`src/adaptation.rs`, `reechantillonne`) est lourd : il se fait sur des fils de fond
(`src/collage.rs`), jamais sur le fil de l'interface. Mesures (`collage_tests`, carte graphique absente, machine de 16 fils) :

| Mesure | Résultat |
|---|---|
| Un dessin **plein** sur toute la feuille 4K (7 680 × 4 320, 8 160 tuiles) vers 1080 | **257 ms** sur un fil, **33 ms** sur 15 fils |
| Le même, 1080 vers 4K (agrandi, 8 160 tuiles d'arrivée) | 617 ms sur un fil, 90 ms sur 15 fils |
| Le même, 4K vers 720 | 206 ms sur un fil, 25 ms sur 15 fils |
| Plan 4K de 3 calques × 24 dessins (fond rectangulaire, trait vectoriel, peinture sur le quart d'une zone de 3 840 × 1 920), collé en 1080 (`a_big_paste_never_blocks_the_interface_thread`) | lancer le collage : **2,9 ms** ; calcul de fond : **0,54 s** ; **pire image d'écran pendant le calcul : 7,7 ms** |
| Même plan avec 24 dessins pleins, si c'était sur le fil de l'interface (estimation : 24 × 257 ms) | de l'ordre de 6 s : c'est ce que la règle « sans latence » interdit |

Seuil de décision : tout ce qui porte des dessins entiers (plans, calque, images, clés) passe par un fil de fond ; une zone de
pixels ou une sélection sur plusieurs calques le fait au-delà de `ZONE_LOURDE` (1 million de pixels) ; les clés de caméra et
d'effets, les effets et les éléments vectoriels (des nombres) sont adaptés tout de suite. Le calcul laisse un cœur à l'interface
(`fils_disponibles` : tous sauf un, de 1 à 8) ; les dessins se répartissent sur les fils, le reste des fils sert chaque dessin.
Mémoire : les tuiles de la copie sont partagées (`Arc`), seules les tuiles d'arrivée sont nouvelles ; une copie qui ne change pas
d'échelle (cadence seule) ne rééchantillonne rien et partage ses tuiles.

Pour refaire les mesures : `cargo test --release --offline --target-dir target\msvc perf_collage -- --ignored --nocapture`
(dessins pleins) et `a_big_paste_never_blocks_the_interface_thread -- --nocapture` (le plan 4K).

## 19. Coopération : ce que coûte un film partagé (2026-10-20, branche `cooperatif`)

Mesures de `cooperation_mesures_tests.rs` (`cargo test --release cooperation_mesures -- --nocapture --include-ignored`), dossier local,
film de 600 plans (un de 8 calques × 12 dessins). **Rien de ce qui touche au dossier partagé ne se fait sur le fil de l'interface** ;
le tableau dit sur quel fil chaque coût tombe.

| Quoi | Fil | Coût |
|---|---|---|
| balayage du dossier, rien de changé (600 plans) | session | 1,4 ms (premier : 1,7 ms) |
| un tour complet du moteur, rien de neuf | session | 1,7 ms toutes les 3 s |
| `Session::sonder`, au repos | **interface** | 0,04 µs par image d'écran |
| lire et décoder un plan de 96 dessins | session | 110 ms |
| poser ce plan dans le document | **interface** | 0,07 ms |
| relire la structure de 600 plans | session | 3,0 ms |
| poser la structure | **interface** | 0,5 ms |
| cliché d'un enregistrement (50 plans tenus sur 600) | **interface** | 2,3 ms |
| écrire ces 50 plans | écriture | 270 ms |
| enregistrer un plan de 96 dessins dont un a changé | écriture | 12 ms |
| ouvrir le film de 600 plans (lecture exacte) | ouverture | 2,1 s |
| créer le film partagé à partir d'un projet de 600 plans | partage | 4,3 s |

- Au repos (rien ne change dans le dossier), le fil ne demande **aucune** repeinture ; la fenêtre Équipe ouverte non plus.
- Le coût réel d'un dossier synchronisé est celui du client (Dropbox, Drive) : SHOT n'écrit qu'un petit fichier de fiche toutes les
  30 s par fenêtre ouverte, et les plans que l'on a vraiment changés.
- Les dessins ne sont jamais relus à chaque balayage : seulement quand le fichier de leur plan change.