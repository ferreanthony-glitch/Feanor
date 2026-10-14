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

- ✅ 2a. Affichage sur la carte graphique (wgpu).
  - ✅ 2a-1. Moteur d'affichage de la fenêtre : wgpu (Vulkan / Direct3D 12), repli automatique sur
    OpenGL ; carte utilisée indiquée dans le menu Aide.
  - ✅ 2a-2. Superposition des calques par la carte graphique (opacité, pelure, table lumineuse) :
    image composée à la taille du projet, puis affichée avec la vue. Vérifié identique au calcul sur
    processeur (0 pixel différent sur les essais : montage, pelure + table lumineuse, trait en
    cours). Le calcul sur processeur reste pour l'export, et en secours
    (`PELURE_COMPOSITION=processeur`).
- ✅ 2b. Caméra : cadre, panoramique, zoom, rotation, images clés et courbes, visibles dans la timeline.
  - ✅ 2b-1. Feuille de dessin deux fois plus grande que le cadre (validé : option A, marge du
    double), cadre « Caméra » tracé, hors-champ estompé ; export et vignettes = ce que voit la
    caméra ; anciens projets convertis à l'ouverture (même cadrage), format de projet 4.
  - ✅ 2b-2. Caméra par panneau : outil Caméra (déplacer, zoomer par un coin, tourner par la
    poignée), réglages chiffrés dans la fiche du panneau, réinitialisation ; vue à travers la caméra ;
    hors-champ estompé (désactivable) ; export, vignettes et planches à travers la caméra ; la
    caméra suit la coupe et la réunion de panneaux. Modèle d'images clés déjà en place.
  - ✅ 2b-3. Mouvements de caméra par images clés : clé au début / ici / à la fin, retirer
    (commandes de Storyboard Pro 27, sans touche), adoucir le mouvement à une clé (Pelure) ; clés
    visibles dans la timeline d'animation (ligne « Caméra ») et dans la bande de montage ; lecture,
    vue caméra et export suivent le mouvement. Reste : déplacer une clé à la souris dans la
    timeline, copier la caméra d'un panneau à l'autre, chemin de caméra affiché sur la feuille.
- ✅ 2c. Profondeur Z des calques (multiplan), vue de contrôle (vue de dessus).
  - ✅ 2c-1. Transformer un calque entier : outil Transformer (déplacer, échelle par un coin,
    rotation par la poignée), valeurs exactes, Ctrl+R pour annuler (raccourci de Storyboard Pro 27,
    jusque-là en réserve) ; on dessine normalement sur un calque transformé ; affichage, export et
    vignettes en tiennent compte. Pas encore : transformation animée, miroir, fusion d'un calque
    transformé (refusée avec un message).
  - ✅ 2c-2. Profondeur des calques : réglage « Profondeur » dans les calques (0 = plan de
    référence, 1 = deux fois plus loin, négatif = premier plan) ; quand la caméra se déplace, les
    plans lointains défilent moins vite et les premiers plans plus vite, à l'écran, en vue caméra,
    à l'export et dans les vignettes. Le zoom de caméra reste un zoom d'objectif (sans effet de
    profondeur) ; les calques de profondeurs différentes ne se fusionnent pas.
  - ✅ 2c-3. Vue de dessus (menu Affichage, sans touche) : la caméra et son champ (début et fin
    du mouvement en pointillés), chaque calque à sa distance avec sa largeur réelle ; glisser un
    calque vers le haut / le bas change sa profondeur (Maj : par pas de 0,1), annulable en une
    étape. Pas encore : déplacer la caméra depuis cette vue, vue de côté, vraie vue 3D.
- ⏳ 2d. Import glTF de repère.

## Phase 3 — Animatique et animation ✅

- ✅ 3a. Lecture fluide. Mesuré sur un montage lourd (20 panneaux, 3 calques bitmap chargés,
  cadre 1920×1080) : chaque changement de panneau figeait l'image 45 à 75 ms (envoi des dessins
  à la carte graphique). Désormais seules les tuiles dessinées partent, en une copie groupée, et
  les dessins de la seconde suivante sont envoyés d'avance par petits morceaux : plus aucune image
  au-dessus de 20 ms de calcul, ni en lecture ni en défilement. Vignettes : quelques ms par image
  au lieu de 4 d'un coup. Barre d'état : cadence réellement affichée pendant la lecture (orange
  si des images sont sautées pour rester calé sur le son).
- ✅ 3b. Exports (Fichier > Exporter…) : film MP4 H.264 + son AAC (regarder, envoyer), film
  MOV ProRes 422 HQ + son PCM (montage), images PNG, son seul WAV ; tout le montage, la scène ou
  le panneau ; vidéo pleine taille ou moitié. Fenêtre de progression (temps restant, annuler) :
  la fenêtre reste vivante, le document est bloqué pendant l'export. ffmpeg : celui trouvé sur
  le poste (Krita, PATH, winget) ou choisi à la main (retenu dans les réglages). Image filmée
  4 à 8 fois plus rapide (zone filmée seulement, plusieurs cœurs) et images tenues pas
  recalculées : montage lourd de 960 images 1080p exporté en 23 s (avant : ~85 s de calcul,
  fenêtre figée). Couleurs vérifiées (BT.709). Commandes de Storyboard Pro 27 reprises sans
  touche : MOVIE_EXPORT_KEY, BITMAP_EXPORT_KEY, SOUNDTRACK_EXPORT_KEY, EXPORT_CURRENT_IMAGE_KEY.
  Pas encore : choix de la qualité, images JPEG, export EDL/AAF/XML.
- ✅ 3c. PDF de storyboard (Fichier > Exporter le storyboard (PDF), ou dans la fenêtre
  d'export) : A4 paysage, 6 panneaux par page (2 rangées de 3), image de début de panneau vue
  par la caméra, titre et durée, puis action, dialogue, notes sous l'image (« … » si le texte
  ne tient pas) ; nouvelle page à chaque séquence ; en-tête : projet, séquence, page n / N.
  Montage, scène ou panneau. Fabriqué sans bibliothèque (police standard Helvetica, accents
  français ; images JPEG) ; vérifié avec un lecteur PDF tiers (PDF.js). Commande de Storyboard
  Pro 27 reprise sans touche : PDF_EXPORT_KEY. Pas encore : autres mises en page, police au
  choix, caractères hors de l'alphabet latin (remplacés par « ? »).
- ✅ 3d. Animation à la TVPaint (rendu visé : TVPaint, pas Storyboard Pro — choix d'Anthony).
  - ✅ 3d-1. Pelure d'oignon : valeurs par défaut reprises des préférences Storyboard Pro
    d'Anthony (3 avant en rouge, 3 après en vert, teintées, sous le dessin) ; compter par dessin
    ou par image ; jusqu'à 12 de chaque côté ; opacité du plus proche, puis de plus en plus pâle ;
    teinte ou vraies couleurs ; couleurs au choix ; réglages retenus d'une séance à l'autre.
    Pas encore : pelure à travers les panneaux, sur tous les calques.
  - ✅ 3d-2. Pinceaux texturés : grain du papier fixe sur le dessin (appuyer remplit les creux),
    pointes en image (avec réductions pour les petites tailles), pointe aplatie / tournée,
    variations au hasard (angle, taille), taille et opacité mini sous la pression. Réglages
    nommés dans le panneau Outils : 6 de Pelure (pinceau rond, crayon à papier, craie grasse,
    encre, gomme, gomme à grain) et import d'un fichier de pinceaux Toon Boom (XML
    `penstyles`) : les 9 pinceaux perso d'Anthony passent (3 pointes, 3 papiers), gardés dans
    %APPDATA%\Pelure\pinceaux. Rendu de Pelure (visé : TVPaint), pas celui de Storyboard Pro.
    Hypothèses d'échelle non vérifiées (dureté sur 0–5, taille en pixels) : voir `tbbrush.rs`.
    Pas encore : mélange des couleurs (peinture humide), double pointe, inclinaison du stylet,
    grain sur les calques vectoriels.
- ⏸ Export XML Toon Boom : non prévu (format propriétaire non documenté).

## Correctif — latence du dessin ✅

Signalé par Anthony après la phase 4 : le dessin n'est plus fluide depuis la phase 2. Mesuré
(traits simulés dans l'entrée de l'application) : le calcul pendant un trait est léger
(< 1 ms par image), mais la fin d'un trait vectoriel redessinait tout le dessin, sur la feuille
4 fois plus grande que le cadre depuis la phase 2 : 1 ms au premier trait, 56 ms au 80ᵉ, et
plus encore sur une vraie planche. Désormais ~1 ms quel que soit le nombre de traits (trait posé
par-dessus l'image existante) ; gomme vectorielle et déplacement de sélection ne refont que leur
zone. File d'affichage ramenée de 2 images à 1. Reste à confirmer au stylet par Anthony (le
retard ressenti à l'écran ne se mesure pas d'ici).

Retour d'Anthony : « le curseur a un léger retard sur le stylet ». Cause : avec les pinceaux, Pelure
cachait le curseur de Windows et dessinait son propre cercle dans l'image (une à deux images de
retard). Désormais la croix du système reste affichée à la pointe (Windows la déplace sans
attendre l'image), avec le cercle de la taille du pinceau autour ; réglage « Croix du système sur
le pinceau » (menu Affichage, activé par défaut). Piste suivante si le trait lui-même semble en
retard : affichage sans attente de la synchro (risque de déchirement, à essayer).

## Phase 4 — Confort ✅

- ✅ 4a. Copie de secours et récupération après plantage : toutes les minutes par défaut
  (AUTO_SAVE_DELAY = 1 dans les préférences Storyboard Pro d'Anthony ; réglable dans le menu
  Fichier, 0 = jamais), si le projet a changé, sans figer la fenêtre (écriture sur un autre
  fil). La copie va dans `%LOCALAPPDATA%\Pelure\secours`, jamais dans le projet lui-même : seul
  « Enregistrer » l'écrit (choix prudent ; comment Storyboard Pro fait exactement n'est pas
  vérifié). Fermeture normale : copie effacée. Après un plantage : fenêtre « Pelure s'est fermé
  sans enregistrer » (ouvrir la copie, la supprimer, ou plus tard), avec un avertissement si le
  projet a été enregistré après la copie. Barre d'état : « Secours il y a … ». Vérifié par un
  vrai plantage (processus tué) : copie retrouvée et rouverte, traits compris.
- ✅ 4b. Espaces de travail (Affichage > Espaces de travail) : quatre dispositions de Pelure
  (Standard ; Dessin, Timeline et Vue d'ensemble, qui reprennent par le nom les commandes
  VIEW_LAYOUT_DRAWING / TIMELINE / OVERVIEW de Storyboard Pro 27, sans touche, avec un contenu
  propre à Pelure) ; « Enregistrer la disposition… » sous un nom, et supprimer ; panneaux
  Outils, fiche et calques, montage / timeline à afficher ou masquer (Affichage > Panneaux) ;
  la disposition est retenue d'une séance à l'autre (AUTO_SAVE_LAYOUT = Y dans les préférences
  d'Anthony). Une disposition = panneaux affichés, leurs tailles, vue vignettes, vue de dessus.
  Pas encore : fenêtres détachées, plusieurs vues du même dessin.

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
