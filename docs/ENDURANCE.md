# Endurance d'un épisode (2026-10-31, branche `endurance`, partie de `main` e87f313)

Priorité n°3 de l'audit externe : tenir un épisode réaliste, pas un plan d'essai. Pas d'épisode réel sous la main : un générateur
en fabrique un (dessins tracés par le vrai moteur de pinceaux, aucune image d'IA), puis des bancs mesurent l'ouverture, la
navigation, la lecture, l'édition, l'enregistrement, les envois et les exports. Rien de tout cela n'est dans l'interface livrée.

## 1. Méthode

- **Machine** : Core i7-10700K (8 cœurs, 16 fils), 32 Go de mémoire, GeForce RTX 2080 SUPER, Windows 11, version release,
  tas segmenté (`docs/PERFORMANCE.md` §26). **La machine n'était pas seule** : quatre autres chantiers compilaient (jusqu'à
  trois `rustc` de 2 à 6 Go) ; Windows réduisait l'ensemble de travail de SHOT en cours de route (la « mémoire privée » est le
  chiffre fiable, pas le RSS). Les temps varient d'un passage à l'autre (jusqu'à × 2) ; seuls les écarts nets comptent.
- **L'épisode** (`endurance_generer`, `src/endurance_tests.rs`, qui réutilise le générateur du film lourd, `lourd_tests.rs`, en
  allongeant les plans) : **200 plans, 8 séquences, 681 cases, 819 calques, 22 270 images = 15,5 min à 24 i/s**, 4 429 dessins
  (1 656 bitmap = 2 Go de pixels, 2 402 vectoriels, 71 651 traits), 15 plans animés, 36 plans à caméra, 3 pistes de son
  (musique, dialogues, bruitages ; 179 extraits), notes, commentaires, 14 versions de plans ; deux envois sont marqués pendant
  l'essai. Fabrication 6,5 s ; sur le disque : 691 Mo (4 440 fichiers).
- **Les bancs** (tous `#[ignore]`, un processus chacun ; `ENDU_DIR` = dossier de travail, `%TEMP%\shot-endurance`) :

```
cargo test --release --offline --target-dir target\msvc endurance_generer -- --ignored --nocapture --test-threads=1
cargo test --release --offline --target-dir target\msvc endurance_naviguer -- --ignored --nocapture --test-threads=1
cargo test --release --offline --target-dir target\msvc endurance_editer   -- --ignored --nocapture --test-threads=1
cargo test --release --offline --target-dir target\msvc endurance_exports  -- --ignored --nocapture --test-threads=1
cargo test --release --offline --target-dir target\msvc endurance_profil   -- --ignored --nocapture --test-threads=1
cargo test --release --offline --target-dir target\msvc endurance_fuite    -- --ignored --nocapture --test-threads=1
```

  Les lignes de résultat commencent par `ENDU`. `ENDU_PLANS`, `ENDU_DUREE` (durée des plans, 250 %), `ENDU_OPS` (2 000),
  `ENDU_CYCLES` (400). Sans fenêtre : `App::frame` complet à 1920 × 1080 et vraie carte graphique, **attendue jusqu'au bout**
  (la colonne « carte » des images n'existe pas à l'écran : ce qui compte pour le fil de l'interface est la part
  « interface »). L'ouverture et les exports passent par le vrai code (`open_path`, `ExportJob`, `PaquetJob`, `EditorialJob`).
  Les ffmpeg et les copies de secours sont écrits dans des dossiers temporaires, jamais ceux d'Anthony.
- Limites : pas de stylet ni d'écran réel ; la mémoire d'ffmpeg n'est pas comptée ; le ffmpeg est celui de Krita (encodeur
  H.264 de Windows).

## 2. Chiffres (avant / après)

« Après » = priorité basse de l'écriture, de la copie de secours et des envois (seul changement de code de la passe). Ce qui n'a
pas de « après » n'a pas été touché.

| Mesure (épisode de 200 plans) | Avant | Après |
|---|---|---|
| Ouvrir : lecture sur un fil de fond / image d'adoption | 10,5 s / 7 ms | idem |
| Ouvrir : jusqu'au calme (vignettes 1 s, rejeux exacts finis) | 20,9 s (60 s de processeur) | idem |
| Mémoire après l'ouverture (privée) | 6,8 Go | idem |
| Mémoire après navigation, lecture, séance (privée) | 9,3 à 10,2 Go | 10,2 Go |
| Enregistrer, premier enregistrement : appel sur le fil de l'interface | 236 ms | 114 ms (bruit) |
| Enregistrer, premier enregistrement : durée / pire image pendant | 12,4 s / **201 ms** | 28,6 s (machine chargée) / **81 ms** |
| Ctrl+S après une séance de 2 000 gestes (1 361 dessins changés) : appel / durée / pire image | 84 ms / 5,7 s / **120 ms** | 61 ms / 8,3 s / **40 ms** |
| Ctrl+S sans changement / après un trait | 4 ms / 4 ms | 3 ms / 3 ms |
| Copie de secours (554 Mo) : image moyenne / pire pendant | **40 ms** / 215 ms | **15 ms** / 145 ms |
| Envoi n°1 : copie des plans sur le fil de l'interface / total / pire image | 109 ms / 8,1 s / 50 ms | 93 ms / 15,6 s / 57 ms |
| Envoi n°2 | 69 ms / 0,7 s | 87 ms / 1,2 s |
| Séance de 2 000 gestes : durée / gestes > 50 ms (geste ou 3 images d'après) | 313 s / 439 | 271 s / 252 |
| Gestes de structure au début (nouveau, supprimer 3 ou 40, annuler, déplacer 5, dupliquer, allonger), 4 onglets | 0 à 7 ms | idem |
| Annuler / rétablir 200 étapes | 0,3 s / 0,0 s | idem |
| Plantages, incohérences (plans, calques, marques) sur 2 000 gestes | 0, 0 | 0, 0 |
| Changer de plan : interface (Storyboard) / image avec la carte attendue | 10 à 17 ms / moy 41, p95 97 ms | idem |
| Vignettes, tous les plans à l'écran (zoom 0,2) : image moyenne / pire | 10 / 25 ms | idem |
| Lecture de tout l'épisode (22 270 images) : moy / p95 / > 100 ms | 18,9 / 53 ms / 169 | non refait (machine trop chargée) |
| Projet enregistré (tout écrire) | 554 Mo, 3 542 fichiers | idem ; avec deux envois : 904 Mo |

Exports (une seule passe, avant le changement ; le changement ne touche pas leur calcul, déjà sous la priorité basse) :

| Export | Durée | Taille | Mémoire de SHOT (max) | Interface pendant : moy / p95 / pire |
|---|---|---|---|---|
| MP4 de tout l'épisode, 1080p, avec le son | 20,5 min | 2,7 Go | 6,6 Go (pas de croissance) | 15 / 34 / 1 090 ms (64 images > 100 ms) |
| Storyboard PDF (200 plans) | 55 s | 33 Mo | 6,7 Go | 12,6 / 21 / 39 ms |
| Liste des plans (CSV + JSON) | 0,1 s (8 ms de calcul) | 3 fichiers | — | 8 ms |
| Vignettes par plan | 11,5 s | 248 Mo | 6,4 Go | 9 / 13 / 26 ms |
| Vidéos par plan (H.264, 200 fichiers) | 14,3 min | 2,6 Go | 6,5 Go | 12 / 28 / 131 ms |
| Paquet de livraison complet | 16,2 min | 5,6 Go, 404 fichiers | 6,6 Go | 8,7 / 12 / 52 ms |
| Montage EDL/FCPXML (+ 63 fichiers) | 92 s | 138 Mo | 6,4 Go | 8 / 11 / 18 ms |

## 3. Ce qui a été corrigé

- **L'écriture, la copie de secours et les envois travaillent sous la priorité de l'interface** (`enregistrement.rs`,
  `recovery.rs`, `envoisui.rs`, `parallele::fil_et_ses_fils_sous_la_normale`, comme les exports). Avant : huit fils d'encodage
  PNG au niveau normal prenaient les cœurs de l'interface (images de 40 ms en moyenne pendant une copie de secours,
  pointes de 200 ms ; Ctrl+S après une séance : pointe de 120 ms). Compromis : sur une machine déjà chargée, l'écriture dure
  plus longtemps (12 s → 28 s pour un premier enregistrement pendant une compilation) ; elle va aussi vite qu'avant dès que
  des cœurs sont libres. Essai : `the_save_and_the_recovery_copy_run_below_the_interface_priority`.
- **Générateur d'épisode** et bancs (`endurance_tests.rs`), essai `the_episode_generator_makes_a_project_that_saves_and_reloads`.

Constat important : **aucun plantage, aucune incohérence** sur 2 000 gestes dont 70 % visent le début de l'épisode (supprimer,
nouveau plan, déplacer, dupliquer, couper, coller, durées, clés de caméra, versions, traits, lecture, changements d'onglet
et annulations) : le motif connu « rang de plan resté après un changement de structure » ne se reproduit pas.

## 4. Ce qui reste, classé

1. **Mémoire : 6,8 Go à l'ouverture, 9 à 10 Go après une séance** pour 691 Mo sur le disque. Répartition mesurée : 2,1 Go de
   tuiles bitmap, 2,2 Go de tuiles de rendu des 2 536 dessins vectoriels (132 570 tuiles), le reste en structures et caches ;
   la mémoire de la carte graphique est bornée (≈ 1 Go, §27). Un épisode de 300 plans dépasserait 12 Go : prévoir des tuiles
   de rendu vectoriel recalculables (les lâcher hors de l'écran) avant de monter plus haut.
2. **Les dessins des plans supprimés restent en mémoire jusqu'à la réouverture** : `doc.cels` n'est jamais élagué (l'annulation
   en a besoin ; une fois l'historique de 200 étapes dépassé, plus personne). `endurance_fuite` : 400 cycles (plan neuf, trait,
   copie, suppression) → +800 dessins retenus, privée de 6,6 à 8,7 Go puis plateau (+0,4 Mo par cycle) ; vider l'historique
   ne rend presque rien (8,76 → 8,30 Go). Un élagage doit tenir compte de l'historique, des variantes, versions et envois :
   pas fait ici, risque de perte de données.
3. **Envoi : 90 à 110 ms sur le fil de l'interface** (copie de tous les dessins, `envois_marquer`), pour une commande explicite ;
   à passer par tranches comme le cliché des exports.
4. **Ctrl+S : 60 à 85 ms sur le fil de l'interface** quand 1 400 dessins ont changé depuis l'enregistrement précédent (60 µs par dessin
   vectoriel cloné) ; 114 à 236 ms pour un enregistrement complet (Enregistrer sous). Rien au-dessus de 5 ms dans l'usage normal
   (un trait : 3 ms). Un cliché à partage (`Arc`) des éléments vectoriels le ramènerait sous 10 ms.
5. **MP4 : 2,7 Go pour 15,5 minutes** (débit de l'encodeur H.264 de Windows réglé par défaut) et 20 min d'encodage sur une
   machine chargée ; envisager un débit cible pour l'animatique.
6. Ouvrir : 10 s de lecture et 8 s de rejeux exacts (60 s de processeur) ; l'interface reste libre (adoption 7 ms).
7. Lecture de tout l'épisode avec la carte attendue : 169 images > 100 ms sur 22 270, concentrées dans deux dixièmes (machine
   chargée, ensemble de travail réduit par Windows) ; à refaire sur une machine calme.
8. Pas reproduit : une image de 1,9 s après un changement de durée (vue une seule fois, pendant que Windows réduisait la
   mémoire de SHOT ; 40 changements de durée relancés : pire image 46 ms).

## 5. Rejouer

Générer, puis lancer les bancs dans l'ordre ci-dessus (≈ 25 min pour `naviguer`, 10 pour `editer`, 1 h pour `exports`).
Comparer deux états du code avec les mêmes dossiers : `ENDU_DIR` garde l'épisode et l'ouvre en lecture seule (les bancs qui
enregistrent travaillent sur une copie).

## 6. Mémoire d'un épisode (2026-10-31, branche `memoire-episode`, partie de `main` 492090a)

Objectif : un épisode de 300 plans sous environ 4 Go, sans perte de données, sans latence au dessin. Mémoire = « mémoire privée » du
processus d'essai (la mesure fiable, §1). Nouveaux bancs : `endurance_memoire` (ouvrir, parcourir 120 plans, expériences),
`endurance_tuiles` (taux de rangement sur les vrais dessins) ; les anciens affichent en plus le **tas vivant** (compteur autour de
l'allocateur des essais) et les **plages** de mémoire engagée. `SHOT_SANS_GEL=1` remet l'ancien comportement dans le même exécutable :
toutes les mesures « avant » ci-dessous (colonne A) viennent de là, sur la même machine et dans la même heure que les « après » (B).
**Mesurer pendant qu'on compile fausse tout** : un premier passage de B fait pendant une compilation montrait des images de 300 ms qui
n'existent pas (refait seul : identique à A).

### 6.1 Où allait la mémoire (200 plans, état de `main`, à l'ouverture : 6,6 à 6,9 Go privée, tas vivant 5,3 à 5,5 Go)

| Poste | Mémoire |
|---|---|
| Tuiles des 1 893 dessins bitmap (127 940 tuiles de 16 Ko) | 2 096 Mo |
| Tuiles de rendu des 2 536 dessins vectoriels (132 570 tuiles) | 2 172 Mo |
| Caches de pixels des traits (budget 512 Mo, plein) | 531 Mo |
| Reste du tas (52 429 éléments, 3,3 M points, structures, sons, annulation…) | ≈ 0,5 Go |
| Hors tas : exécutable, carte graphique (pilote), piles | ≈ 1,3 Go à l'ouverture |
| Après avoir parcouru 120 plans (A) : tas +0,1 Go, **hors tas +2,1 Go** (privée 8,8 Go) | pilote Vulkan |

Ce qui ne pèse presque rien : vignettes (≤ 128 Mo sur la carte, 16 Mo en mémoire), historique (200 étapes : 1 à 3 Mo), dessins des plans
supprimés (400 cycles : +800 dessins, +0,4 Mo par cycle). Le hors tas : réduire le cache de la carte à 64 Mo n'en rend que 60 Mo,
`HeapCompact` rien ; avec Direct3D 12 à la place de Vulkan, il tombe de ≈ 2,6 Go à ≈ 0,5 Go (§6.5).

### 6.2 Ce qui a été fait

Pas de déchargement sur le disque (l'étiquette `archive/memoire-dessins` a été lue, pas fusionnée) : **les pixels des dessins qu'on ne
regarde pas sont rangés en mémoire en tampons exacts** (`tuilepack.rs`, séries de pixels identiques et littéraux, sans perte). Mesuré
sur l'épisode : ×12,7 (bitmap) et ×11,2 (vectoriel), 2,6 à 3,2 µs pour ranger et 1,1 à 1,5 µs pour rouvrir une tuile. Raison du choix :
aucun chemin de perte de données (un dessin modifié et pas enregistré se range comme les autres), rien à vérifier sur disque, aucun
lecteur à modifier (305 sites lisent `cel.raster`). `Tile` (`raster.rs`) est maintenant des pixels et/ou un tampon, rouvert tout seul à la
première lecture (`Deref`) ; une écriture (`DerefMut`) remet les pixels et oublie le tampon.

1. **À l'ouverture** (`project::load_drawing`, rejeu du chargement) : les dessins se rangent dès leur lecture ; le pic n'est plus de 7 Go.
2. **`gel.rs`**, un tour par image d'écran (0,27 ms en moyenne, 0,5 au pire) : hors dessins chauds (plan courant ±2, 4 derniers plans visités,
   3 s de lecture à venir), le fil « tuiles-rangees » (sous la priorité de l'interface) fait les tampons, puis les pixels partent des tuiles.
   Une tuile tenue aussi par un autre dessin (plan copié, version, variante), une étape d'annulation ou un fil de fond (vignette, export,
   enregistrement) : la copie du dessin passe à une tuile à elle faite du seul tampon, les pixels partent avec le dernier. Les projets des
   autres onglets sont tenus de même.
3. **Plans supprimés** : leurs dessins restent dans `doc.cels` (l'annulation) mais se rangent comme les autres : un dixième de la taille.
   Rien n'est élagué (risque de perte pour l'historique, les versions, les variantes, les envois).
4. **Historique d'annulation** : au-delà des 8 dernières étapes, les tuiles se rangent (150 tuiles toutes les 400 ms) ; elles se rouvrent
   à l'application.
5. **Caches de traits** : ceux qu'aucun rendu n'a touchés depuis 3 000 usages se rangent (fil de fond ; le budget compte toujours 16 Ko).
6. **Marquer un envoi** : plans et dessins se copient par morceaux de 3 ms, image après image (`envoisui.rs`) ; si le projet change
   pendant la copie, elle recommence.

### 6.3 Chiffres avant (A) / après (B), épisode de 200 plans (privée, Mo)

| Mesure | A | B |
|---|---|---|
| Ouvrir : mémoire à l'ouverture (tas vivant) | 6 872 (5 502) | 2 002 (1 336) |
| Ouvrir : de la demande au calme | 15,5 s | 16,3 s |
| Après navigation (Vignettes, Storyboard, Montage, tête de lecture) | 9 514 | 4 715 |
| Après la lecture de tout l'épisode (22 270 images) | 9 461 | 4 656 |
| 120 changements de plan (`endurance_memoire`) | 8 781 | 3 900 |
| Séance de 2 000 gestes : début → fin | 7 150 → 10 100 | 2 334 → 5 652 (tas vivant 3,3 Go dont 1,15 de tuiles) |
| 400 cycles de plans neufs supprimés (`endurance_fuite`) : début → fin ; historique vidé | 6 581 → 8 858 ; 8 381 | 1 743 → 3 766 ; 3 307 |
| Lecture de tout l'épisode : images moy / p95 / pire / > 33 ms | 8,2 / 21,5 / 37,9 / 10 | 8,1 / 21,5 / 38,5 / 9 |
| Changer de plan, image moy / p95 / pire : Storyboard | 29,7 / 52,6 / 95,6 | 30,8 / 49,2 / 96,6 |
| idem Montage | 27,9 / 44,6 / 105,3 | 30,4 / 46,2 / 82,2 |
| idem Vignettes | 5,0 / 7,0 / 27,7 | 4,9 / 6,7 / 24,5 |
| Tête qui saute (120 sauts) moy / p95 | 27,0 / 47,7 | 30,3 / 50,8 |
| Gestes de structure : pire image après | 64,7 ms | 46,0 ms |
| Séance : durée, gestes > 50 ms | 114 s, 7 | 124 s, 6 |
| Ctrl+S après la séance : appel / fin | 23,9 ms / 3,9 s | 22,6 ms / 3,8 s |
| Marquer un envoi : appel / plus long morceau de copie (avant ce chantier : 93 à 109 ms d'un coup) | 5,9 ms / 3 ms | 5,3 ms / 3 ms |

Le prix : passer à un plan jamais regardé rouvre ses tuiles sur le fil de l'interface (1 à 3 ms) : + 3 à 12 % sur les images de saut de plan,
rien sur la lecture ni sur les gestes.

Épisode de **300 plans** (même générateur : 6 156 dessins, 369 000 tuiles = 6,0 Go de pixels ; `ENDU_PLANS=300`) :

| Mesure | A | B |
|---|---|---|
| Mémoire à l'ouverture (tas vivant) | 9 031 (7 337) | 2 289 (1 580) |
| Après navigation / après la lecture (34 112 images) | 11 925 / 11 533 | 5 317 (tas 2 170) / 4 909 |
| 120 changements de plan | non mesuré | 4 074 (tas 1 485) |
| Lecture : moy / p95 / pire | 8,4 / 21,6 / 51,7 | 8,4 / 21,6 / 55,0 |
| Séance de 2 000 gestes (le projet passe à 451 plans) | non mesuré | 2 707 → 6 470 (tas 4,1 Go dont 1,56 de tuiles) |
| Gestes > 50 ms ; pire image après un geste de structure | | 6 ; 68 ms |
| Envoi, projet de 451 plans : appel / plus long morceau | | 7,9 ms / 4,2 ms (avant le découpage des plans : 50 ms) |

Plafond du tas, 300 plans : **2,2 Go après navigation** ; le reste de la mémoire privée (≈ 3,1 Go) est le pilote Vulkan de la carte.

### 6.4 Sûreté des données et essais (`gel_tests.rs`, `tuilepack_tests.rs`)

Un tampon est toujours la copie exacte des pixels (il est refait après toute écriture ; une tuile qui ne se range pas, plus de 12 Ko, reste en
pixels). Essais : tuile rangée puis rouverte identique et écriture qui oublie le tampon ; deux dessins sur une tuile ; une copie en vol (fil de fond) qui lit
ses pixels pendant que le dessin range la sienne ; un dessin modifié après rangement jamais perdu, même après plusieurs tours ; annulation
d'une suppression de plan après rangement : dessins exacts ; 24 étapes annulées puis rétablies, chacune identique au pixel près après
rangement de l'historique ; enregistrement d'un projet rangé : mêmes fichiers octet pour octet qu'un projet non rangé, relecture identique
(hors-feuille compris) ; copie de secours d'un projet rangé relue identique ; export d'image et vignette de dessins rangés identiques ;
les dessins d'un autre onglet se rangent et reviennent exacts ; un marquage d'envoi recommence si le projet change pendant sa copie.
Suite complète : voir le rapport. Pas de changement de format du projet (46).

### 6.5 Vulkan ou Direct3D 12 (non changé par défaut)

Sur cette machine (RTX 2080 SUPER) wgpu choisit Vulkan, dont le pilote garde 2,6 à 3,4 Go de mémoire vive de plus. Même banc avec
`WGPU_BACKEND=dx12` : 200 plans, ouverture 1 377 Mo, après 120 changements de plan 2 310, après navigation complète 2 215, lecture de tout
l'épisode **2,4 ms** en moyenne (8,1 en Vulkan) ; 300 plans, 1 654 puis 2 514. Le vrai `shot.exe` (copie renommée, `APPDATA` isolé, copie de
l'épisode, arrêté par son Id) s'ouvre et dessine la planche pareil dans les deux cas (capture de la fenêtre comparée) : 1 991 Mo en Vulkan,
1 630 Mo en Direct3D 12 à l'ouverture. **Non adopté par défaut** : l'affichage dans une vraie fenêtre (images successives, présentation sans
synchro verticale) n'a pas été éprouvé avec Direct3D 12 ; c'est la décision à prendre avec l'oeil d'Anthony (`WGPU_BACKEND=dx12` suffit
pour l'essayer, sans recompiler). Avec lui, 300 plans passent sous 4 Go partout ; sans lui, on y est à l'ouverture (2,3 Go) mais pas
après une longue navigation (5,3 Go dont 3,1 de pilote).

**Correction (2026-11-01).** Ces chiffres « Direct3D 12 » ne sont pas fiables. La fonction `dx12` de wgpu n'est pas compilée
(Cargo.toml : eframe sans fonctions par défaut, et les crates `gpu-allocator` et `range-alloc` absents du registre hors ligne). Avec
`WGPU_BACKEND=dx12`, le vrai `shot.exe` écrit « Failed to create surface for any enabled backend: {} » et se rabat sur OpenGL (glow).
Les 1 630 Mo mesurés « en Direct3D 12 » sont donc ceux d'OpenGL. Le banc hors fenêtre reste à revérifier. Pour essayer vraiment
Direct3D 12, il faut activer la fonction `dx12` de wgpu, ce qui demande de télécharger ces crates, avec l'accord d'Anthony.

### 6.6 Ce qui reste

- Pilote Vulkan (ci-dessus). Marquer un envoi laisse une image de 70 à 160 ms à la fin (l'enregistrement qui suit copie 3 000 dessins modifiés
  sur le fil de l'interface : `prepare_save`, point 4 du §4) ; Ctrl+S après séance : 23 ms (200 plans), 52 ms (450 plans).
- Après une séance, le tas garde 3 à 4 Go dont un tiers de tuiles : le reste (étapes d'annulation « Structure » complètes, éléments vectoriels,
  copies d'envois en attente) n'a pas été analysé poste par poste.
- Les dessins du banc se rangent ×11 à ×13 (traits au crayon, aplats) ; un dessin de pixels très variés (photo, grain de papier) ne se range pas
  et reste à 16 Ko par tuile : jamais de perte, mais moins de gain. À mesurer sur un vrai épisode.
- Les dessins de plans supprimés restent dans le projet ouvert (un dixième de la taille, mais jamais élagués).
- Non mesuré : exports complets (vidéo, PDF, paquet) après ce chantier : ils rouvrent les tuiles qu'ils lisent, que la tenue range ensuite.
- Un projet qui reste ouvert très longtemps avec beaucoup de gestes sur des plans copiés peut voir des tuiles redevenir partagées puis
  se re-séparer (un millier d'octets par copie) : sans effet visible.
