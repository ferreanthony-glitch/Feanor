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

## Phase 1 — Fondations storyboard 🔧

- ✅ 1a. Modèle Séquence > Scène > Panneau (l'ancien « plan » est devenu le panneau) ; numérotation
  automatique et noms personnalisés ; barres de séquences et de scènes dans la bande de montage ;
  vue Planches rangée par scène ; nouveau panneau / scène / séquence, dupliquer et supprimer un
  panneau ou une scène, déplacer un panneau d'une scène à l'autre ; projet en dossier (format 3,
  enregistrement incrémental) ; ouverture des anciens `.pelure` ; export HTML rangé par séquence.
- ⏳ 1b. Vues Vignettes / Scène / Timeline avec pistes son (WAV/MP3, forme d'onde, lecture synchronisée).
- ⏳ 1c. Sélection et transformation en bitmap, guides de perspective, table lumineuse.
- ⏳ 1d. Annuler/rétablir sur tout (textes, réglages de calque, sons), gestes regroupés.
- ⏳ 1e. Couper (F5) / fusionner des panneaux, copier/coller de calques (W/X), verrouillage (Alt+L),
  fusion (Ctrl+E).

## Phase 2 — Caméra et 3D ⏳

Prérequis : installer les outils de compilation Microsoft (validé, à faire au début de la phase).

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
| Scène précédente / suivante | Maj+A / Maj+F | PREV_SHOT_KEY / NEXT_SHOT_KEY sans touche ; A / F = panneau (preset) |
| Miroir de la vue | M | |
| Storyboard ⇄ animation | Tab | |
| Vue planches / scène | Maj+Tab | |
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
