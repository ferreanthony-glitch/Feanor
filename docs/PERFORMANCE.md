# Performance de SHOT

Passe de performance du 2026-10-19 (branche `perf`, partie de `main` c693643). Demande d'Anthony :
« avancer sur l'optimisation du soft et le rendre le plus performant possible rapidement », avec la
règle qui prime sur tout : le logiciel répond toujours instantanément, sans latence.

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
`perf_brush_library_startup`, `perf_cpu_bench`). Un essai d'inactivité tourne toujours avec la suite :
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
- **Lecture** : pas de cache en mémoire d'images composées. Chaque image est composée par la carte
  graphique à partir d'une texture par dessin ; les dessins de la seconde qui suit partent d'avance
  (4 Mo par image d'écran). Une texture de dessin est de la taille de la **feuille** (33 Mo pour
  3840 × 2160), le cache en garde environ 30 (1 Go) : avec 15 calques animés à 2 images, la lecture
  renvoie à la carte les dessins neufs de chaque image. Tient : 8,2 ms en moyenne, 14,1 ms au pire
  pour un budget de 41,7 ms. Les calques à effets (flou, ombre, CTG) se calculent sur le processeur
  (hors de cette passe, voir l'agent des effets).
- **Dessin dans un plan lourd** : la pile de calques est recomposée entièrement à chaque image
  d'écran par la carte (pas de dessous / dessus gardés) : 2,2 ms pour 15 calques, 5,7 ms pour 60.
  Sans effet sur la latence du trait (le CPU fait 2 à 3 ms, la carte travaille en parallèle), mais
  c'est de la chaleur et du ventilateur gratuits (levier 3).
- **Trait vectoriel texturé** : déjà sans latence (rejeu en arrière-plan) ; 2 à 3 ms par image avec
  30 traits larges sur le calque.
- **Ce que le banc ne voit pas** : le stylet (cadence réelle des événements), l'affichage (synchro,
  Windows), la mémoire de la carte pleine, un vrai projet d'Anthony. À refaire avec la superposition
  sur un vrai projet.

## 6. Leviers restants, classés

| # | Levier | Gain estimé | Effort |
|---|---|---|---|
| 1 | **Enregistrer en arrière-plan** (Ctrl+S) : l'écriture sort du fil de l'interface | gel de ≈ 2 ms par dessin modifié (50 dessins : ≈ 100 ms ; premier enregistrement d'un gros projet : ≈ 9 s) → 0 | M. `save_to` rend un booléen qu'attendent Quitter, Fermer, Nouveau, Ouvrir un récent : il faut une suite (`SaveThen`), et un compteur de modifications pour ne pas effacer `modified` si l'on dessine pendant l'écriture. À faire avec soin : un `modified` effacé à tort perd du travail. |
| 2 | **Textures de dessin à la taille du contenu** (boîte des tuiles dessinées) au lieu de la feuille | mémoire de la carte ÷5 à ÷20 ; ≈ 10 fois plus de dessins en cache (30 → 300) ; défilement d'un plan lourd 13,8 → ≈ 6 ms ; lecture sans renvoi | L. `gpu.rs` : coins et UV de chaque élément, feuille flottante, maillages (enveloppe, perspective), agrandissement quand on dessine hors de la boîte, tests pixel contre le processeur. |
| 3 | **Image composée gardée** (dessous / dessus) tant que le plan et les versions ne changent pas | carte : 2,2 → ≈ 0,5 ms par image au repos de contenu (15 calques) ; 60 calques : 5,7 → ≈ 0,5 ms ; moins de chaleur | M. Clé de composition + `sync` qui dit si un envoi a eu lieu ; invalider à chaque changement de taille, mode de fusion, maillage. |
| 4 | **Vignettes du montage hors du fil de l'interface** | plus de pic de 6 ms en défilant un grand montage (600 plans : tête 11,4 → ≈ 3 ms) | L. Il faut des tuiles partageables (`Arc<Tile>`) pour ne pas copier les dessins vers le fil ; touche le moteur de pinceau. |
| 5 | **Décodage PNG en flux vers les tuiles** (crate `png` en dépendance directe, déjà dans `Cargo.lock`) | ouverture 7,3 → ≈ 4 s, mémoire ÷30 par fil | S à M |
| 6 | **Cache d'images composées en mémoire** pour la lecture des plans à effets | 24 images/s garanties avec 10 effets ou plus (les effets se calculent sur le processeur) | L. Avec la carte graphique pour les effets (7). |
| 7 | **Effets sur la carte graphique** (flou, ombre, bruit, mosaïque) | enlève le processeur de la lecture des plans à effets | L. Mené par l'agent des effets. |
| 8 | **Taille mémoire de la carte réglable** (`CACHE_BYTES` = 1 Go aujourd'hui, sur 8 Go) | ×3 de dessins en cache à coût nul | S. wgpu n'expose pas la mémoire : un réglage dans les Préférences. |
| 9 | **Résolution d'affichage** (réduite pour la vue éloignée) | composition ÷4 en zoom arrière | M à L |
| 10 | **PGO** (optimisation guidée par un profil) | 5 à 15 % sur le rendu des traits (non mesuré) | M. Demande `llvm-profdata` et une procédure hors ligne. |

## 7. À regarder à l'écran

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
