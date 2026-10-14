# Feuille de route

Légende : ✅ fait · 🔧 en cours · ⏳ à faire · ⏸ reporté

## Avant le plan (prototype)

✅ Montage de plans (vignettes, durées à la souris, vue planches), animation image par image
(expositions, pelure, lecture), pinceau à pression, calques bitmap et vectoriels (lasso, poignées,
gomme qui coupe, remplissage), annuler/rétablir, fichier `.pelure`, exports PNG et HTML.

## Phase 0 — Préparation ✅

- ✅ Suivi de versions (git, MinGit portable).
- ✅ `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`, `docs/reference/sbpro_presets/`.
- ✅ `keymap.toml` : 65 raccourcis par défaut de Storyboard Pro 27 + 8 perso, réserve « non
  implémenté », propositions marquées ; plus aucun raccourci codé en dur ; menus et aide (F1)
  générés depuis le fichier ; « Aide > Modifier / Recharger les raccourcis ».
- ✅ Nouvelles petites actions pour coller au preset : zoom par pas (1/2), plein écran, rotation
  par pas (V/C), allonger/raccourcir le panneau (+/−), première/dernière image du panneau, quitter.

## Phase 1 — Fondations storyboard ✅

- ✅ 1a. Modèle Séquence > Scène > Panneau (l'ancien « plan » est devenu le panneau) ; numérotation
  automatique et noms personnalisés ; barres de séquences et de scènes dans la bande de montage ;
  vue Planches rangée par scène ; nouveau panneau / scène / séquence, dupliquer et supprimer un
  panneau ou une scène, déplacer un panneau d'une scène à l'autre ; projet en dossier (format 3,
  enregistrement incrémental) ; ouverture des anciens `.pelure` ; export HTML rangé par séquence.
- ✅ 1b. Vues « Dessin » / « Vignettes » + bande de montage servant de timeline ; pistes son sous
  les panneaux : import WAV / MP3 / FLAC / OGG (bouton, menu, ou fichier lâché sur la fenêtre),
  forme d'onde, déplacer un extrait (aussi d'une piste à l'autre), raccourcir par les bords, couper,
  supprimer, piste muette, volume, nom ; lecture calée sur le son ; son au défilement ; son visible
  dans la timeline d'animation ; sons enregistrés dans `sons/` du projet.
  Reste pour plus tard : lier un son à un panneau (Ctrl+L, en réserve), fondus, export du son
  (phase 3, avec la vidéo), formats AAC/M4A.
- ✅ 1c. Sélection bitmap (lasso / rectangle, déplacer, échelle, rotation, miroir, effacer dedans /
  autour, copier / couper / coller) ; Maj+Suppr (effacer hors sélection) aussi en vectoriel ; table
  lumineuse (autres calques estompés) ; guides de perspective à 1, 2 ou 3 points (G, Maj+A, Maj+L,
  Maj+G, comme le module Guides de Storyboard Pro 27), enregistrés dans le projet.
  Reste pour plus tard : peinture limitée à la sélection, déformation en perspective d'une
  sélection (outil Perspective), autres types de guides, annulation des réglages de guide.
- ✅ 1d. Annuler / rétablir sur tout le document : textes des panneaux, noms de séquence, de scène,
  de calque et de piste, opacité et visibilité des calques, piste muette et volume, guides (ajout,
  suppression, nom, points de fuite), couleur du papier, cadence. Une étape par geste (une saisie,
  un glissé de curseur ou de point). Reste : regrouper les appuis répétés d'une même touche
  (+ / − sur la durée font une étape par appui).
- ✅ 1e. Couper le panneau à l'image courante (F5) et réunir un panneau avec le suivant ; couper
  une scène au panneau courant et la joindre à la précédente ; copier / coller un calque d'un
  panneau à l'autre (W / X) ; verrouiller un calque (Alt+L) ; fusionner avec le calque du dessous
  (Ctrl+E). « Réunir des panneaux » et « joindre à la scène précédente » sont des commandes de
  Pelure (Storyboard Pro 27 passe par une sélection de plusieurs panneaux, pas encore possible
  ici). Reste : sélection de plusieurs panneaux ou calques à la fois.

## Phase 2 — Caméra et 3D ⏳

- ✅ Prérequis : outils de compilation Microsoft (Build Tools 2022) installés, projet compilé avec
  la toolchain MSVC (`rust-toolchain.toml`) ; tests et lecture avec son vérifiés.

- ⏳ 2a. Affichage sur la carte graphique (wgpu).
- ⏳ 2b. Caméra : cadre, panoramique, zoom, rotation, images clés et courbes, visibles dans la timeline.
- ⏳ 2c. Profondeur Z des calques (multiplan), vue 3D de contrôle.
- ⏳ 2d. Import glTF de repère.

## Phase 3 — Animatique et animation ⏳

- ⏳ Lecture fluide (cache d'images), pinceaux texturés (essai d'import des 9 pinceaux perso).
- ⏳ Exports : séquence d'images, vidéo (via ffmpeg), PDF de planches.
- ⏸ Export XML Toon Boom : non prévu (format propriétaire non documenté).

## Phase 4 — Confort ⏳

- ⏳ Espaces de travail, sauvegarde automatique, récupération après plantage.

## Raccourcis proposés, à valider

Ces raccourcis ne viennent pas de Storyboard Pro (source « proposition » dans `keymap.toml`) :

| Action | Proposé | Note |
|---|---|---|
| Enregistrer sous | Ctrl+Maj+S | aucune touche par défaut dans Storyboard Pro |
| Rétablir (second) | Ctrl+Y | habituel sous Windows |
| Supprimer (second) | Retour arrière | |
| Ajuster à la fenêtre | Ctrl+0 | changé en 1a : Maj+F sert désormais à « scène suivante » |
| Nouvelle scène | Maj+P | NEW_SCENE_KEY n'a pas de touche par défaut ; P = nouveau panneau (preset) |
| Scène précédente / suivante | Ctrl+Pg préc. / Ctrl+Pg suiv. | ✅ validé (1d). PREV_SHOT_KEY / NEXT_SHOT_KEY sans touche ; Maj+A appartient au module Guides de Storyboard Pro (« aligner sur le guide ») |
| Miroir de la vue | M | |
| Storyboard ⇄ animation | Tab | |
| Vue dessin / vignettes | Maj+Tab | |
| Aide raccourcis | F1 | |
| Outil crayon | Alt+P | |
| Outil formes | Alt+R | |
| Outil main | H | |
| Image précédente / suivante (second) | Flèches gauche / droite | |
| Début / fin du montage | Ctrl+Origine / Ctrl+Fin | Origine / Fin = panneau (preset) |
| Panneau précédent / suivant (second) | Pg préc. / Pg suiv. | |
| Nouveau panneau (second, storyboard) | Inser | |
| Supprimer le panneau (storyboard) | Ctrl+Suppr | |
| Nouveau dessin (animation) | Inser | |
| Dupliquer le dessin | Ctrl+D | |
| Retirer le dessin (animation) | Ctrl+Suppr | |
| Allonger / raccourcir l'exposition | Alt++ / Alt+− | + / − = durée du panneau (preset) |
| Nouveau calque bitmap | Maj+B | Maj+S (perso) = vectoriel |
| Calque du dessus / dessous | Flèches haut / bas | |
