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
