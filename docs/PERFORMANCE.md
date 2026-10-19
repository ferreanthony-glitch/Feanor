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
`perf_gpu_textures` (mémoire de la carte, défilement, lecture, repos). Un essai d'inactivité tourne toujours avec la suite :
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
| 4 | **Vignettes du montage hors du fil de l'interface** | plus de pic de 6 ms en défilant un grand montage (600 plans : tête 11,4 → ≈ 3 ms) | L. Il faut des tuiles partageables (`Arc<Tile>`) pour ne pas copier les dessins vers le fil ; touche le moteur de pinceau. |
| 5 | **Décodage PNG en flux vers les tuiles** (crate `png` en dépendance directe, déjà dans `Cargo.lock`) | ouverture 7,3 → ≈ 4 s, mémoire ÷30 par fil | S à M |
| 6 | **Cache d'images composées en mémoire** pour la lecture des plans à effets | 24 images/s garanties avec 10 effets ou plus (les effets se calculent sur le processeur) | L. Avec la carte graphique pour les effets (7). |
| 7 | **Effets sur la carte graphique** (flou, ombre, bruit, mosaïque) | enlève le processeur de la lecture des plans à effets | L. Mené par l'agent des effets. |
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

**Les dix réglages** (`PerfReglages`, `[performance]` dans reglages.toml ; tous modifiables dans
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
