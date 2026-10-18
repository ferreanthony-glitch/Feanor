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

## Phase 2 — Caméra et 3D ✅

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
- ✅ 2d. Import glTF de repère, en fil de fer (choix d'Anthony).
  - Fichier > Importer un modèle 3D (glTF) : `.glb` ou `.gltf` (tampons intégrés ou à côté),
    lecteur écrit pour Pelure. Sommets soudés, seules les arêtes utiles sont tracées (bords,
    cassures de plus de 25°), arêtes cachées masquées. Le modèle est gardé dans le projet
    (`modeles/<id>.pelure3d`, format de projet 5).
  - Un repère par panneau (modèle, point de vue, couleur, opacité), posé sur toute la feuille comme
    un guide : il ne s'exporte pas et n'apparaît pas dans les vignettes. Outil « Repère 3D » :
    glisser pour tourner autour, Maj pour déplacer, Ctrl pour avancer ou reculer ; azimut, hauteur,
    distance et focale (mm) chiffrés ; « Cadrer le modèle », « Comme le panneau précédent ».
  - « Dessiner dans un calque » : le fil de fer devient un calque bitmap tout en bas, qui s'exporte.
  - Mesuré sur un vrai décor (1,77 million de triangles, 313 000 arêtes) : import 0,4 s, vue
    complète recalculée en 0,15 s sur un autre fil ; pendant qu'on tourne, aperçu immédiat (les
    6 000 plus longues arêtes) ; l'affichage du repère ne coûte qu'une image au pinceau.
  - Pas encore : matières et textures (fil de fer seulement), plusieurs modèles dans un panneau,
    repère animé dans le panneau, export avec le repère visible, formats autres que glTF (FBX, OBJ :
    passer par Blender), modèles compressés (Draco, quantifiés : refusés avec un message).

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
le pinceau » (menu Affichage, activé par défaut).

Second retour : « rien ne doit traîner derrière la croix, le cercle et le trait doivent y rester
collés ». Affichage sans attente de la synchro de l'écran (chaque mouvement du stylet est
affiché aussitôt, rythme limité : rien au repos, ~17 % d'un cœur en lecture) ; bout du trait
relié à la pointe du stylet à l'écran pendant le geste (le lissage ne le fait plus traîner).
L'écart restant est le temps de calcul et d'affichage d'une image (quelques millisecondes) :
aucun logiciel ne dessine dans sa fenêtre aussi vite que Windows déplace son curseur. Piste si
c'est encore visible : prédire la position du stylet d'une image. Effet secondaire : la lecture
n'est plus bridée quand la fenêtre est derrière une autre.

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

## Phase 5 — Interface et nouveaux outils ✅

Demandée par Anthony après la phase 4. Choix faits : icônes Phosphor, fenêtres modulables dans
la fenêtre de Pelure (egui_dock), tampon comme Storyboard Pro **et** tampon de duplication,
perspective = déformer en perspective (outil Perspective de Storyboard Pro).

- ✅ 5a. Icônes des outils (jeu d'icônes Phosphor, licence MIT) : une icône par outil, nom et
  raccourci au survol ; icônes aussi dans les onglets des panneaux.
- ✅ 5b. Fenêtres modulables et aimantées : neuf panneaux (Dessin, Outils, Couleur, Réglages,
  Vue, Guides, Fiche, Calques, Montage / timeline) en onglets ; on glisse un onglet sur un des
  boutons qui apparaissent au milieu d'un panneau pour l'y aimanter (à gauche, à droite, dessus,
  dessous, ou en onglet), ou on le lâche ailleurs pour le faire flotter ; bords entre panneaux à
  tirer. Affichage > Panneaux : afficher / masquer chaque panneau (il revient à sa place). Les
  espaces de travail enregistrent tout cela (y compris les panneaux flottants et leur place) ;
  les anciennes dispositions sont converties. Le dessin ne se ferme pas et ne flotte pas.
- ✅ 5c. Formes en outils séparés : Ligne (droite, courbe, courbe en S), Rectangle, Ellipse,
  Polyligne (outils LINE / RECTANGLE / ELLIPSE / POLYLINE de Storyboard Pro 27, sans touche ; leur
  maniement exact n'est pas vérifié, celui-ci est propre à Pelure). Courbe : tracer, puis glisser
  pour courber (la courbe passe par le pointeur) ; en S, deux glissers. Polyligne : un clic par
  point, double-clic ou Entrée pour finir, clic sur le premier point pour fermer.
- ✅ 5d. Texte (TEXT_TOOL de Storyboard Pro 27) : polices du poste, taille, couleur ; sur un calque
  vectoriel le texte reste modifiable (clic dessus), la Sélection le déplace, l'agrandit, le
  tourne ; sur un calque bitmap il devient des pixels. Police absente : celle de Pelure (signalé).
  Pas encore : crénage, alignement centré / à droite, gras ou italique autrement que par la police.
  Format de projet 6.
- ✅ 5e. Tampon comme Storyboard Pro (STAMP_TOOL, d'après ses réglages : taille max et min,
  espacement, angle, rondeur, variations au hasard, miroirs, plusieurs images en cycle) avec six
  tampons de Pelure et les images importées ; tampon de duplication (Ctrl + clic pour la source,
  aligné ou non, calque courant ou tous les calques). Calques bitmap.
- ✅ 5f. Déformer en perspective (PERSPECTIVE_TOOL de Storyboard Pro 27) : les 4 coins du dessin
  (ou de la sélection vectorielle), grille de perspective ; chaque geste repart du dessin d'origine.
  Pas encore : sur une sélection bitmap.
- ✅ 5g. Panneau dans une fenêtre Windows à part (clic droit sur son onglet), à poser sur un autre
  écran ; la fermer le remet à sa place ; sa place est retenue. Les raccourcis clavier restent ceux
  de la fenêtre principale.

## Phase 6 — Une interface proche de Storyboard Pro ✅

Demandée par Anthony (capture annotée de Pelure et capture de Storyboard Pro) : barre d'outils
verticale, dessiner directement dans la vignette, replier les textes des cartes, calques dans le
montage, calque de fond étendu à toute la scène, et la disposition générale de Storyboard Pro
(inspiration fonctionnelle seulement : ni nom ni image de Toon Boom).

- ✅ 6a. Barre de commandes en haut (fichier, édition, panneaux, vue, lecture) et barre d'outils
  verticale à gauche, outils groupés ; nouvelle disposition : dessin | Calques et Fiche | Réglages,
  Vue, Guides et Couleur ; montage dessous. Une disposition enregistrée avant 6a passe une fois à
  la nouvelle. Affichage > Panneaux : chaque barre s'affiche ou se masque.
- ✅ 6b. Panneau Couleur : teinte, saturation et valeur en curseurs dégradés, code hexadécimal,
  nuancier.
- ✅ 6c. Panneau Calques : vignette de chaque calque à l'image courante, œil et cadenas en icônes,
  opacité en haut, commandes en icônes en bas.
- ✅ 6d. Réglages des pinceaux en grille : chaque case montre le trait du réglage (tracé par le vrai
  moteur) et sa taille ; nom et provenance au survol.
- ✅ 6e. Vue Vignettes : la vignette du panneau choisi est le dessin lui-même, vu par la caméra, où
  l'on dessine directement ; textes sous les vignettes repliables (« Textes »). L'affichage par la
  carte graphique moyenne plusieurs lectures quand il réduit fort : les traits fins ne disparaissent
  plus en vignette ou en vue très éloignée.
- ✅ 6f. Calque de toute la scène (« + Fond », case « Toute la scène ») : chaque panneau de la
  scène en a un exemplaire, réglé pareil, qui montre le même dessin pendant tout le panneau. Couper
  ou copier une scène lui donne son propre fond ; un panneau qui change de scène prend le fond de
  la nouvelle ; « rendre au panneau » laisse à chaque panneau sa copie. Format de projet 7.
  Montage : piste Caméra au-dessus des panneaux, une piste par calque dessous (le r-ième calque de
  chaque panneau, ses expositions), une piste Fond par scène, noms des pistes à gauche ; un clic
  sur un calque le rend courant.
  Pas encore : glisser des expositions dans les pistes du montage, replier les pistes.

## Phase 7 — Planche de travail et pinceaux vectoriels texturés ✅

Demandée par Anthony (capture annotée de la vue Vignettes) : « un vrai espace de travail où
dessiner, noter, sélectionner des shots et les déplacer, un maximum de souplesse » ; et des
textures pour les pinceaux vectoriels. Choix : tout le fond de la planche est un canevas libre
(les notes restent où on les dessine) ; « cadrage » = la caméra du panneau.

- ✅ 7a. Planche : fond gris clair où l'on note (pinceau, crayon, formes : un trait de note ;
  gomme : efface les notes touchées), zones de texte sous les vignettes retirées, titre au-dessus.
  Molette, Ctrl + molette (zoom), Espace / bouton du milieu / Main (déplacer), taille des
  vignettes. Clic, Ctrl + clic, Maj + clic, lasso de l'outil Sélection : choisir des panneaux ; les
  glisser (le panneau courant par son titre) pour les placer avant ou après d'autres, dans une
  autre scène aussi ; clic droit : menus (animer, nouveau, dupliquer, supprimer la sélection,
  nouvelle scène, recadrer). La vignette du panneau courant est le dessin ; avec l'outil Caméra
  elle montre la feuille et le cadre. Format de projet 8 (`planche`).
  Pas encore : déplacer ou recolorer une note, texte tapé sur la planche.
- ✅ 7b. Pinceaux texturés sur les calques vectoriels : mêmes réglages (grille, papier, grain,
  pointes importées) ; le trait reste modifiable. Textures enregistrées avec le projet.
  Pas encore : l'opacité selon la pression n'est pas gardée par le trait vectoriel texturé.

## Phase 8 — Planche en pages, notes souples, modes de fusion ✅

Demandée par Anthony (capture d'une page de vignettes de Storyboard Pro, en inspiration
fonctionnelle) : des plans qui se suivent sur la même ligne, un nombre de vignettes réglable
(6 au plus par ligne) ; choisir, déplacer, écrire et attacher les notes ; des modes de fusion
pour les calques, comme dans les logiciels de retouche.

- ✅ 8a. Planche en pages blanches numérotées : colonnes (1 à 6), lignes par page (ou une seule
  page), écart, « Scène à la ligne » au choix ; cases vides numérotées au bout de la dernière
  page ; la disposition ne dépend plus de la taille de la fenêtre (les notes restent à leur
  place). Notes : outil Sélection pour les choisir (clic, Ctrl + clic, lasso) et les glisser ;
  outil Texte pour écrire (double-clic : modifier) ; « Attacher les notes » (ou clic droit sur des
  notes) : une note suit sa vignette et grandit avec elle ; détacher, couleur courante, taille du
  texte, Suppr. Format de projet 9.
- ✅ 8b. Modes de fusion des calques (menu en haut du panneau Calques) : Normal, Produit,
  Superposition, Incrustation, Obscurcir, Éclaircir, Densité couleur + / −, Densité linéaire −,
  Lumière crue, Lumière tamisée, Différence, Exclusion, Teinte, Saturation, Couleur, Luminosité.
  Même image à l'écran (carte graphique) et à l'export (vérifié pixel à pixel à 100 %).
  Pas encore : les modes Fondu, Lumière vive / linéaire / ponctuelle, Soustraction, Division
  (faits en 9a).

## Phase 9 — Fusion complète, caméra continue, plateau en 3D ✅

Demandée par Anthony (quatre captures annotées) : les modes de fusion manquants ; un panneau
Calques plus compact (renommer au clic droit) ; la fiche dans les onglets des réglages ; une
caméra qui ne se réinitialise plus d'un panneau à l'autre, avec une fenêtre de réglages (clés,
adoucis, position x/y/z, focale, rotation, durées) ; des onglets vue du plateau / vue caméra ; des
cadres de sécurité et un cadre réglable (9:16, 4:5, 1:1, 16:9, personnalisé) ; un bouton 3D pour
voir le plateau dans l'espace, naviguer et y déplacer les objets.

- ✅ 9a. Les 27 modes de fusion (ajout de Fondu, Densité linéaire +, Couleur plus foncée / claire,
  Lumière vive, linéaire, ponctuelle, Mélange maximal, Soustraction, Division), rangés par
  famille. Carte graphique = processeur (vérifié pixel à pixel, Fondu compris).
- ✅ 9b. Panneau Calques compact : fusion et opacité en haut, la zone de réglages du bas retirée ;
  double-clic ou clic droit → Renommer, et le menu du calque (toute la scène, profondeur,
  dupliquer, verrouiller, supprimer). Fiche dans les onglets Réglages / Vue / Guides (les
  dispositions enregistrées sont mises à jour).
- ✅ 9c. Caméra continue : un panneau sans clé reprend la caméra du panneau d'avant dans la scène
  (sinon celle du suivant). Adoucir l'entrée / la sortie en images ; avancée (Z) dans le décor ;
  focale en mm. Fenêtre de la caméra (outil Caméra) : clés début / ici / fin, clé précédente /
  suivante, adoucis, position X Y Z, focale, rotation, vue, durées de la scène et du panneau.
  Format de projet 10.
  Bascule et panoramique : faits en 9f.
- ✅ 9d. Barre au-dessus du dessin : onglets Plateau (la feuille, la vue y est retrouvée telle
  qu'on l'a laissée) et Caméra ; zones de sécurité (action 93 %, titres 90 %, EBU R 95) ; cadre de
  format 16:9, 9:16, 4:5, 1:1 ou personnalisé, centré dans le cadre de la caméra, hors-format
  estompé. Retenus avec la disposition.
  Pas encore : décaler le cadre de format dans le cadre de la caméra.
- ✅ 9e. Plateau en 3D (bouton 3D) : plans des calques à leur profondeur, texturés par leur
  dessin ; caméra en pyramide de vision (et sa course si elle bouge) ; sol quadrillé. Outils :
  tourner autour, glisser la vue, avancer ; Déplacer un calque dans son plan (Maj : en
  profondeur) ou la caméra par son œil (Maj : avancée). Vues de face, de dessus, de côté, par la
  caméra. Même géométrie que le multiplan (testée).
  Pas encore : modes de fusion dans la vue 3D (tout y est en Normal), repère 3D (glTF) dans la
  vue 3D, échelle et rotation d'un calque dans la vue 3D (outil Transformer en 2D).
- ✅ 9f. Bascule et panoramique de la caméra (demandés par Anthony) : la caméra tourne sur
  elle-même. Fenêtre de la caméra (Bascule, Panoramique, interpolés entre les clés), vue 3D
  (outil Déplacer, Ctrl + glisser l'œil). Vue caméra, export et vignettes en perspective ; sur le
  plateau, le cadre filmé est un trapèze ; zones de sécurité et cadre de format suivent ; la vue
  de dessus tourne le champ avec le panoramique. Bornés pour que le cadre ne filme jamais
  l'horizon (au moins 5° sous lui). Format de projet 11.
  Pas encore : dessiner en vue caméra tournée marche (la souris passe par la perspective), mais
  la taille du pinceau ne suit pas la perspective ; en secours sans carte graphique, l'image en
  perspective est approchée (maillage), à l'export elle est exacte.

## Phase 10 — Appli compagnon Tauri

Demandée par Anthony : intégrer Tauri 2 pour une interface moderne (Vite, React, Tailwind),
exposer les fonctions Rust par des commandes. Choix d'Anthony : une appli **compagnon** qui
partage le noyau de Pelure (le dessin reste dans Pelure) ; installations validées (Node.js LTS,
crates Tauri, paquets npm).

- ✅ 10a. Pelure en bibliothèque (`lib.rs`, `pelure::lancer`) et noyau (`noyau.rs`) testé.
- ✅ 10b. `src-tauri/` (commandes, état, configuration, icônes) et `src-frontend/` (planche par
  scène, vignettes, fiche du panneau : textes, durée, ajouter, dupliquer, déplacer, supprimer ;
  enregistrer ; PDF). Lancement : `docs/COMPAGNON.md`.
  Pas encore : « Enregistrer sous » d'un projet neuf, annuler / rétablir, son, notes de la
  planche, ouvrir le panneau dans Pelure.

## Phase 11 — Moteur de pinceau partagé

Demandée par Anthony (option A) : brancher le moteur de pinceau de Pelure derrière le stylet de
Pelure Studio (`..\pelure-studio`).

- ✅ 11a. `moteur.rs` : bibliothèque de pinceaux, dessins en tuiles, traits, annulation, aperçus
  PNG ; testé.
- ✅ 11b. Branché dans Pelure Studio (commit a9efdbb de son dépôt) : chaque paquet du stylet est
  peint par ce moteur, les tuiles changées repartent vers la toile ; annuler / rétablir ; panneau
  Pinceaux. Aller-retour mesuré : 1,6 à 5,6 ms (plus le compositeur de WebView2).
- ✅ 11c. Repris du Studio : panneau **Pinceaux** (onglet avec Couleur) — les réglages en liste,
  chacun avec un grand aperçu de son trait (moteur de Pelure), son nom, sa taille, « texture »,
  « importé » ; les gommes avec la gomme ; Importer… ; même choix que la grille des Réglages.
  Disposition version 3 : les dispositions enregistrées reçoivent le panneau.
- Décision d'Anthony (après ce bilan) : **Pelure egui reste le logiciel principal** (toile
  native : latence, carte graphique partagée) ; Pelure Studio sert de banc d'essai d'interface.

## Phase 12 — Thème du Studio dans Pelure

Demandée par Anthony : reprendre dans Pelure le thème et la densité de Pelure Studio, et les
aperçus de pinceaux dans la couleur courante.

- ✅ 12a. `theme.rs` : gris sombres et bleu d'accent du thème sombre de Spectrum (couleurs
  seulement), interface dense (espacements, tailles de texte), Segoe UI si Windows l'a ; onglets
  des panneaux assortis ; fond autour de la feuille, barre de la vue et masque de la vue caméra
  aux couleurs du thème. Contrastes testés (texte ≥ 7:1, texte secondaire et accent ≥ 4,5:1 ;
  le test a fait passer le texte sur l'accent en blanc pur).
- ✅ 12b. Aperçus des pinceaux (grille des Réglages et panneau Pinceaux) : le trait dans la
  couleur courante, sur le papier du projet ; une gomme en gris.
- ✅ 12c. Timeline et montage harmonisés : mêmes couleurs dans les deux (règle, rangées, calques
  vectoriels en bleu et bitmap en vert, image courante, clés de caméra, extraits son, barres de
  séquence et de scène), prises dans `theme.rs`. Barres de la timeline et du montage sur une
  seule ligne : commandes en icônes (nom et raccourci du fichier de raccourcis au survol), la
  ligne défile si la fenêtre est trop étroite. Contrastes testés (noms sur les extraits et les
  calques ≥ 4,5:1, repères sur les rangées ≥ 3:1 : le test a éclairci le « dessin vide »).
  Le calque courant écrit en blanc dans la colonne des noms.
- ✅ 12d. Icônes partout où c'est possible, demandé par Anthony : barres (commandes, outils,
  timeline, montage, vue, plateau 3D), panneaux (Calques, Couleur, Vue, Outils, Pinceaux) et
  réglages des outils (sélection, formes, tampons, texte, perspective, transformer, caméra,
  guides, repère 3D, planche des vignettes). Chaque bouton a une bulle — nom, raccourci du
  fichier de raccourcis, petite description — qui ne s'ouvre que si la souris reste une seconde
  dessus. Les interrupteurs des réglages (pression, pixels nets, forme pleine…) passent au bout
  du curseur qu'ils concernent.
  Restent en texte, volontairement : les menus et menus contextuels, les fenêtres de dialogue
  (export, enregistrer, nouveau projet, secours), les noms de réglages devant les curseurs, et
  « Storyboard » / « Animation » en haut à droite (on voit d'un coup d'œil où l'on est) ;
  Anthony a confirmé de les garder en texte.
- ✅ 12e. Demandé par Anthony : copier, coller, supprimer des panneaux dans le montage, et
  déplacer les clés de caméra à la souris.
  - Choix de plusieurs panneaux dans la bande (Ctrl + clic, Maj + clic, comme sur la planche) ;
    glisser l'un d'eux déplace le groupe.
  - Après un clic dans le montage ou sur une vignette : Ctrl+C copie les panneaux choisis
    (avec une copie de leurs dessins), Ctrl+V les colle après le panneau courant, Ctrl+X les
    coupe, Suppr les supprime ; un clic dans le dessin rend ces touches au dessin. Icônes de la
    barre du montage, clic droit, menu Storyboard (`panneau.copier`, `panneau.coller`, sans
    touche à eux ; `panneau.supprimer` vise désormais les panneaux choisis).
  - Clés de caméra : on les fait glisser dans la rangée Caméra de la timeline et la piste
    Caméra du montage ; une clé ne passe pas par-dessus sa voisine ; une étape d'annulation par
    glisser.

## Phase 13 — Séquences et plans, timeline à la TVPaint

Demandée par Anthony (2026-10-15) : plus de panneaux, seulement des séquences et des plans ; une
timeline inspirée de TVPaint, avec le réglage des calques depuis la timeline et des icônes à
gauche, et toutes les possibilités de la timeline de TVPaint. Référence des fonctions (tirée des
textes de TVPaint 12, sans rien inventer) : `docs/TIMELINE-TVPAINT.md`.

Décisions d'Anthony :
- un plan = une scène d'avant ; ses panneaux sont mis bout à bout (une seule timeline) ;
- la suppression des panneaux reste directe, sans confirmation ;
- en Storyboard, le bas de l'écran montre la vue Projet (les plans côte à côte par séquence) ;
  en Animation, la timeline complète du plan ;
- les dessins des anciens panneaux restent des images de storyboard : chaque début d'ancien
  panneau devient une marque d'image du plan, et vignettes / PDF montrent une image par marque
  (une par plan s'il n'y en a pas) ;
- raccourcis des nouvelles fonctions : ceux de sa configuration TVPaint 12 quand ils ne cassent
  pas un raccourci de Pelure, sinon proposés à valider ;
- pré / post-comportement : avant la première image et après la dernière, le calque montre rien,
  la boucle de ses images, un va-et-vient, ou sa première / dernière image maintenue.

Étapes :
- ✅ 13a. Plans : un panneau par scène (le plan) ; conversion des projets (format 12) sans
  rien perdre ni changer à l'écran (caméra tenue aux jointures, calques par nom, débuts des
  anciens panneaux en marques d'image avec leurs textes, notes ré-attachées) ; marques d'image
  (drapeau orange : poser / retirer) ; opérations de plan (nouveau, dupliquer, copier / coller,
  couper à l'image, réunir, déplacer) ; vignettes, fiche, PDF et page HTML par image de
  storyboard ; barre de séquence seule au-dessus du montage ; « Plan » partout à l'écran ; calque
  de fond ordinaire (un dessin tenu sur tout le plan) ; export par séquence ou par plan.
- 13b. En-têtes de calques et opérations sur les calques dans la timeline, dossiers, groupes.
  - ✅ 13b-1. Réglages des calques et leurs effets (format 13) : pelure d'oignon par calque,
    préserver la transparence, pochoir positif / négatif (calques visibles, pinceau, crayon et
    gomme bitmap ; la gomme aussi est retenue par le pochoir), position verrouillée (ni
    Transformer, ni profondeur, ni 3D, ni dessin), groupe de couleur (13 groupes du projet),
    ligne réduite.
  - ✅ 13b-2. En-tête des calques dans la timeline (`layerheads.rs`) : ligne complète (vignette,
    nom, ⋮ options, 8 réglages en icônes, barre d'opacité, mode avec précédent / suivant) ou
    réduite, tout réduire / agrandir, glisser une ligne pour changer le rang, double-clic sur le
    nom pour renommer, menu de groupe.
  - ✅ 13b-3. Opérations sur les calques (`layerops.rs`) : calques choisis (Ctrl / Maj + clic,
    Maj + Haut / Bas), dupliquer / supprimer les choisis, fusionner les choisis / visibles /
    tous, choisir (tous, visibles, même groupe, autres groupes, aucun), tout afficher / cacher,
    calque courant seul, supprimer les vides / cachés, pochoirs éteints / inversés, couper,
    coller à la place, monter / descendre (Ctrl + Haut / Bas), premier / dernier, pelure du
    calque ; menu ☰ de l'en-tête = menu Calques.
  - ✅ 13b-3b. Fenêtre « Nouveau calque… » (`newlayer.rs`) : nom, sorte, groupe (ou celui du
    calque courant), mode, place (sur / sous le calque courant, tout en haut / en bas, sur /
    sous le groupe du calque courant). Le pré / post-comportement viendra en 13c.
  - ✅ 13b-4. Groupes (`groupsui.rs`) : fenêtre des 13 groupes (couleur, nom, montré dans la
    timeline, nombre de calques, appliquer au calque, choisir / verrouiller ses calques, cacher
    les calques des autres groupes, activer / désactiver les autres, seulement les couleurs
    utilisées) ; filtre de la timeline (`rows_of`) et « Calque courant caché » avec sa couleur à
    cliquer ; menu Groupes en masse (tous / groupe courant / autres groupes : pelure, verrou,
    transparence, affichage, pochoir, fusion ; afficher / cacher les groupes, focus). Le mot de
    passe de groupe de TVPaint n'est pas repris.
  - ✅ 13b-5. Dossiers (`folders.rs`, format 14), choix d'Anthony du 2026-10-15 : un seul
    niveau ; pas de mode de fusion de dossier (l'opacité du dossier multiplie celle de ses
    calques, un dossier caché les cache, un dossier verrouillé les verrouille) ; « joindre »
    met les calques des dossiers choisis dans le dossier courant et efface les autres. Nouveau
    dossier, placer / déposer / sortir les calques choisis, glisser une ligne dedans ou dehors,
    ouvrir / fermer (lignes des calques), supprimer le dossier seul ou avec ses calques,
    dupliquer / copier / coller un dossier avec ses calques, fusionner un dossier = fusionner
    ses calques ; aucun outil de dessin sur un dossier.
- 13c. Images et expositions (choix, glisser, insérer / supprimer avec modes de trou, couper /
  copier / coller, inverser, instances, noms, pré / post-comportement, faux-fixe, étirer).
  Choix d'Anthony (2026-10-15) : modes de trou = laisser vide, combler depuis la gauche (le
  dessin d'avant est tenu), combler depuis la droite (la suite recule), garder en l'état ;
  faux-fixe = répéter les images choisies à la suite, en boucle, en va-et-vient ou au hasard ;
  cassure automatique = dessiner sur une image tenue en crée une nouvelle.
  - ✅ 13c-1. Pré / post-comportement par calque (`Layer::shown_at`, format 15) : rendu, export,
    lecture, pochoirs, plateau 3D ; blocs pâles dans la timeline ; menu ⋮ et fenêtre « Nouveau
    calque ». Les outils de dessin restent sur les vraies images (`cel_at`).
  - ✅ 13c-2. Images choisies (Maj + clic / glisser dans la grille, sur les calques choisis ;
    Ctrl+X / C / V et Suppr les visent ensuite) et opérations (`images.rs`) : supprimer avec
    un mode de trou (laisser vide, combler depuis la gauche, combler depuis la droite),
    insérer, couper / copier / coller en insérant ou en remplaçant, dupliquer, inverser, tout
    choisir ; sur tout le plan avec tes touches TVPaint I / Maj+I / Maj+F5 (la durée du plan,
    ses clés de caméra et ses marques suivent). « Garder en l'état » attend sa description.
  - ✅ 13c-3a. Instances : précédente / suivante (Ctrl + Gauche / Droite, tes touches
    TVPaint ; « a » / « f » restent panneau précédent / suivant), tête / queue, casser à
    l'image courante, une exposition de plus / de moins à chaque instance choisie, nouveau
    dessin avant l'instance ; par calque, cassure automatique (dessiner sur une image tenue
    fait une copie jusqu'à la fin de l'exposition) et création automatique (format 15).
  - ✅ 13c-3b. Faux-fixe (les instances choisies rejouées N fois en tout : en boucle, en
    va-et-vient, au hasard sans deux fois de suite la même ; chacune garde son exposition),
    étirer le calque jusqu'à la fin du plan (tenir la dernière image, boucle, va-et-vient) ou
    le couper après l'image courante, glisser-déposer des images choisies (sans touche :
    déplacer ; Ctrl : copier, avec des copies des dessins ; Maj : insérer).
  - ✅ 13c-4. Noms d'instance (`Layer::names`, par dessin, format 15) : double-clic sur un
    dessin de la timeline pour le renommer, effacer, tout renommer (1, 2, 3…), renommage
    intelligent (préfixe / suffixe sur les instances sans nom, nommées par un nombre ou par un
    texte) ; retourner les images choisies (horizontalement, verticalement, les deux : une
    copie retournée de chaque dessin, l'original reste ailleurs).
  - Restent de 13c : « Garder en l'état » (mode de trou, à décrire), la copie « images de
    taille différente » (Pelure n'a qu'une taille de feuille), Ctrl pendant le tracé (chaque
    touche du pinceau sur l'image suivante), interpolation d'images.
- 13d. Repères : marques, signets, entrée / sortie, boucle, image de départ, time code, zoom.
  - ✅ 13d-1 (`cues.rs`, format 16) : signets (Ctrl+Maj+Haut / Gauche / Droite, tes touches
    TVPaint ; enlever tous ou ceux des images choisies), points d'entrée et de sortie (la
    lecture du plan reste entre eux, en boucle ou non ; refus si les deux tombent sur la même
    image), boucle entre les images choisies, image de départ du projet, numéros en image,
    time code ou trame, champ d'image courante où l'on tape un numéro ou un time code.
    Signets et points suivent les images insérées / supprimées dans tout le plan.
  - ✅ 13d-2. Zoom de la timeline (Ctrl + molette sur la grille, curseur ; les numéros
    s'espacent), « Adapter » au plan, au calque, aux images choisies, à l'image, aux clés de
    caméra, tout voir ; croix sur les instances vides ; miniatures des dessins dans les blocs
    des lignes complètes (deux cases à cocher du menu loupe).
  - ✅ 13d-3. Marques de couleur sur les dessins d'un calque (`marks.rs`, les « marques
    d'image » de TVPaint, renommées pour ne pas les confondre avec les images de storyboard
    de 13a) : 12 couleurs, poser / retirer sur l'instance courante, sur toutes les instances
    ou sur les non marquées, retirer celles du calque ou du plan, précédente / suivante
    (toutes ou de la couleur choisie), « Répéter », « Continuer sur le calque suivant »,
    seulement les couleurs utilisées. Une instance vide ne porte pas de marque (une marque
    tient à un dessin).
- 13e. Onglets du bas et vue Projet : notes dessinées, son par plan, arrière-plan, propriétés.
  - ✅ 13e-1 / 13e-2 (`bottomtabs.rs`, `notes.rs`, format 17) : onglets du bas de la timeline
    (Notes, Son, Groupes, Arrière-plan, Image de départ ; un clic ouvre la bande, un second la
    ferme). Arrière-plan : aucun (noir à l'écran, transparent à l'export), couleur (le papier,
    exporté), damier A / B (transparent à l'export). Notes dessinées dans une rangée de la
    grille alignée sur les images : crayon bleu / rouge / noir, gomme de la couleur, gomme
    globale, teinte, position, découper, effacer / teinter une couleur, tout effacer, grille
    toutes les n images ; montrées par-dessus l'image pendant la lecture (en haut, en bas, au
    centre, taille) ; jamais exportées ; elles suivent les images insérées / supprimées.
  - ✅ 13e-3. Son : piste verrouillée (ses extraits ne se déplacent, ne se coupent ni ne se
    suppriment plus), dupliquer une piste, volume général (piste maîtresse, format 17),
    Ctrl + glisser l'onde dans la timeline pour décaler l'extrait sous la souris ; onglet Son
    (son à la lecture, au défilement, pistes).
  - ✅ 13e-4 (`projetui.rs`, format 17). Vue Projet : nouveau plan avant le plan courant ;
    storyboard verrouillé (cadenas du montage, menu Storyboard) qui refuse ce qui change l'ordre,
    le nombre ou la durée des plans (nouveau, dupliquer, coller, supprimer, déplacer, couper,
    réunir, durée, glissés du montage et de la planche) et, choix d'Anthony, ce qui change le
    contenu des plans : dessin (tous les outils, effacer, transformer, déplacer en 3D), commandes
    d'images et d'expositions, ajout / suppression / fusion / collage de calques. Restent permis
    l'affichage (visibilité, pelure, opacité), les repères, notes, noms et marques de couleur. Le
    mot de passe du verrou de TVPaint n'est pas repris. Propriétés du projet (Fichier) :
    titre, information, auteur, notes, copyright ; date de création (projet neuf) et de dernière
    modification (à chaque enregistrement), montrées en temps universel. Nouveau projet à partir
    des plans choisis (Fichier) : il remplace le projet ouvert (après « enregistrer ? »), garde
    feuille, cadre, cadence, papier, arrière-plan, groupes, guides, séquences et notes de planche
    attachées ; le son n'est pas repris.
- ✅ 13f (`onion.rs`, `onionui.rs`). Table lumineuse complète (la pelure d'oignon de Pelure ;
  textes TVPaint 000050050–124) : panneau (engrenage de la timeline, clic droit sur la pelure
  d'un calque) ; colorisation avant / après chacune à part ; chaque image montrée ou cachée et
  son opacité ; opacité globale ; curseurs liés (opacité linéaire) ; « Devant » ; modes
  instance, image, signet, marque de couleur ; activer / désactiver tout ; réglages rangés sous
  un nom (le Bin) ; décaler temporairement les images de la pelure en glissant sur le dessin,
  puis « Remettre ». Réglages de l'application (pas du projet) ; les anciens sont repris.
  « Dégradé » (nom seulement dans les textes de TVPaint) : au choix d'Anthony, celui de
  Photoshop, deux couleurs de l'image la plus proche à la plus lointaine. Pas repris : le mode
  « clé de rig » (il n'y a pas de calque Puppet).
- 13g. Feuille d'exposition (« Timesheet » de TVPaint 12 ; inventaire dans
  `docs/TIMELINE-TVPAINT.md`).
  - ✅ 13g-1 (`xsheet.rs`, `xsheetui.rs`). Panneau « Feuille d'exposition » (Affichage >
    Panneaux, ouvert à la demande, en onglet à côté des calques), affichage occidental, une
    ligne par image du plan courant. Colonnes :
    - Image : numéro selon l'affichage choisi, signets, entrée / sortie ;
    - Son : niveau par image ;
    - une colonne par calque, dans l'ordre de la timeline : le nom ou numéro du dessin à sa
      première image, un trait tant qu'il est tenu, une croix au début des images vides, la
      marque de couleur, ce que montrent les pré- et post-comportements ;
    - Caméra : clés et mouvement ;
    - Action et Dialogues / Notes : les textes de chaque image de storyboard.

    Le reste du panneau :
    - « Infos affichées » : chaque colonne, calques cachés, comportements, image courante,
      repères ;
    - un trait épais chaque seconde ou toutes les N images ;
    - un clic va à l'image et au calque, et la feuille suit l'image courante ;
    - les réglages sont retenus.
  - ✅ 13g-2 (`roles.rs`, `rolesui.rs`, format 18). Rôles des images, ceux de TVPaint : clé,
    breakdown, intervalle à faire / complété, intervalle spécial à faire / complété.
    - Choix d'Anthony : un rôle se pose sur une case (calque, image), même vide, et ne change
      que si on le change.
    - On le pose par le clic droit dans la timeline (menu Rôle, puis le menu Images) ou dans
      la feuille, ou par les actions `role.*` (sans touche). Il vise les images choisies des
      calques choisis, sinon la case cliquée.
    - Chaque rôle a son symbole (propre à Pelure) : disque, triangle, cercle vide / pointé,
      carré vide / plein.
    - Dans la timeline : montrer / cacher les rôles, sur le calque courant seulement, opacité.
    - Les rôles suivent les images insérées, supprimées, inversées, dupliquées, déplacées,
      copiées / collées, et les plans coupés ou réunis. « Une exposition de plus / de moins à
      chaque instance » ne les déplace pas.
    - Permis sous le storyboard verrouillé, comme les marques de couleur.
  - Pas repris, choix d'Anthony : mémo et export PDF de la feuille, affichage japonais, vue
    par pages, XDTS, textes Action / Dialogue image par image. Restent [N] : associer marques
    et rôles. Les flips par rôle iront avec les flips.
- ✅ 13h (`flips.rs`, `flipsui.rs`). Flips, ceux de TVPaint 12 (textes 011074000–063, flips
  par rôle 012104150–159) : feuilleter vite des images pour juger un mouvement.
  - Panneau « Flips » (icône livre de la timeline, action `flip.panneau`) : choisir, créer,
    renommer, supprimer un flip (il en reste toujours un).
  - Chaque flip a une sorte (basique, directionnel, manuel), des limites (plan, calque
    courant, entrée / sortie, signets, marques de couleur, un rôle, ou N images de chaque
    côté), des checkpoints (toutes les images, les dessins du calque, signets, marques de
    couleur, un rôle). S'y ajoutent les flips par seconde (basique), la direction, le sens et
    la distance en pixels (directionnel), et deux options : boucler entre les limites, revenir
    à l'image de départ.
  - Basique et directionnel durent tant qu'on tient le bouton du panneau ou la touche de
    `flip.jouer`. Le manuel avance par `flip.precedent` / `flip.suivant` ou les flèches du
    panneau.
  - Flips par rôle (clés, breakdowns, intervalles et spéciaux à faire / complétés) : tenir
    leur bouton ou leur touche.
  - Réglage de l'application, comme chez TVPaint. Le flip par défaut est celui de la
    configuration d'Anthony : manuel, calque courant, de dessin en dessin, 30 par seconde,
    retour au départ. Aucune touche : sa configuration TVPaint n'en a pas.
  - Mécanique des trois sortes : notre lecture des réglages décrits par les textes. Pas
    repris : « En dehors des tenons » et « Multiple » (noms seulement).
- ✅ 13i (`fx.rs`, `fxui.rs`, format 19). Pile d'effets d'un calque et ses clés, la « pile FX »
  de TVPaint (000025314–316, 000025345).
  - Choix d'Anthony (2026-10-16) : des effets vivants, appliqués à l'affichage et à l'export,
    sans toucher aux dessins. Premiers effets : flou gaussien (rayon) et flou de mouvement
    (longueur, angle).
  - Panneau « Pile d'effets » (baguette de la timeline, action `fx.panneau`) : ajouter, activer,
    ordonner, retirer ; tout réinitialiser, tout supprimer.
  - Clés : créer, supprimer, aller à la précédente / suivante, création automatique. Avec des
    clés, changer un paramètre pose la clé de l'image courante ; entre deux clés, les
    paramètres vont en ligne droite.
  - Les clés se voient dans la timeline (losanges violets, à montrer / cacher). Elles suivent
    les images insérées / supprimées et les plans coupés.
  - Refusé sous le storyboard verrouillé.
  - Pas de « appliquer la pile » (inscrire les effets dans les dessins), puisque les effets
    sont vivants.
- ✅ 13j (`ctg.rs`, `ctgui.rs`, format 20). Calque CTG, principe confirmé par Anthony
  (2026-10-16).
  - On griffonne des couleurs (graffitis) sur le calque CTG. Chaque zone fermée par les traits
    de ses calques sources, touchée par un graffiti, se remplit de sa couleur (la plus
    présente), sous les traits.
  - Remplissage vivant (choix d'Anthony) : il suit toute retouche des traits ou des graffitis,
    à l'affichage comme à l'export.
  - Calque CTG courant : ses graffitis restent visibles, pâles, par-dessus (à l'écran
    seulement).
  - Calques > Calque CTG : nouveau calque CTG (sous le calque courant, qui devient sa
    source), transformer le calque courant, désactiver.
  - Menu de la ligne du calque CTG : choisir ses sources (aucune : tous les autres calques de
    dessin) ; « Charger la structure à partir des sources » (et « (effacer) »), seule commande
    retenue par Anthony.
  - Fermeture automatique des trous (demandée par Anthony) : un trou des traits jusqu'à
    8 pixels par défaut (réglable par calque CTG, menu de sa ligne) ne laisse pas fuir la
    couleur.
  - Pas repris (choix d'Anthony) : graffitis répétés sur les instances vides ou de même nom,
    extraire les couleurs, réglages d'affichage des graffitis. Pas fait non plus :
    « Appliquer les changements » (le remplissage est toujours à jour).
- ✅ 13k (`video.rs`, `videoui.rs`, format 21). Calque vidéo (TVPaint 000020117–119,
  000024902, 011085035).
  - Choix d'Anthony : vidéo lue depuis le fichier, légère en mémoire, plus séquences d'images
    et son de la vidéo.
  - Import (Fichier > Importer une vidéo / une séquence d'images, ou vidéo lâchée sur la
    fenêtre) : ffmpeg (celui de l'export) décode la vidéo une fois en images JPEG à la taille
    du cadre, sur un autre fil (la barre d'état suit). Une séquence est mise à la taille du
    cadre en PNG.
  - Les images sont rangées avec le projet (`videos/<id>/`) à l'enregistrement. Le calque lit
    à chaque image du plan l'image de la vidéo qui tombe là (cadences de la vidéo et du
    projet), à la demande, avec un petit cache.
  - Le son de la vidéo va sur une piste son, calé sur le calque. Le plan s'allonge à la durée
    de la vidéo (sauf storyboard verrouillé).
  - Menu de la ligne du calque : début du calque ; « Transformer en calque d'animation »
    (chaque image devient un dessin, pour dessiner dessus). On ne dessine pas sur un calque
    vidéo.
  - Bande de film dans la timeline.
  - Pas fait : entrée vidéo (capture), le calque préchargé.
- ✅ 13l (`tween.rs`, `tweenui.rs`). Interpolation d'images (TVPaint 000020927–929,
  000025580, 000022168). Choix d'Anthony (2026-10-16) : les deux.
  - Calque vectoriel : de vrais intervalles. Les traits et remplissages qui se correspondent
    (même rang, même sorte) passent d'une forme à l'autre ; un trait dessiné dans l'autre sens
    est retourné ; les autres éléments se fondent.
  - Calque bitmap : un fondu, comme TVPaint.
  - Commandes (menu Images > Interpolation), sur les calques choisis :
    - insérer une / des images intermédiaires entre le dessin courant et le suivant
      (exposées selon « en N ») ;
    - remplir les images vides entre deux dessins par des intervalles (images choisies, sinon
      autour de l'image courante) ;
    - étirer des images à une nouvelle durée en interpolant (ou en tenant) ;
    - changer la cadence du projet en gardant les durées. Images, durées des plans, clés de
      caméra, marques, repères, notes, rôles, clés d'effets, vidéos et sons suivent.
- 13m et suivantes : autres effets (au choix d'Anthony), laissés pour plus tard : Anthony est
  passé à la phase 14 le 2026-10-16. Pas de calque Puppet (choix d'Anthony, 2026-10-16).
Le faux-fixe n'a que son nom dans les textes de TVPaint : à faire décrire par Anthony avant de
l'implanter (le pochoir, la transparence préservée et la position verrouillée ont été décrits
le 2026-10-15, voir `docs/TIMELINE-TVPAINT.md`).

## Phase 14 — Échange avec Storyboard Pro (après la phase 13)

Demandé par Anthony (2026-10-15) : ouvrir un projet Storyboard Pro dans Pelure et en ressortir
quelque chose que Storyboard Pro ouvre. Constat (projet `BRAWLIDAYS_predec PART 1`, lu sans le
modifier) : le `.sboard` est du XML lisible (séquences, plans, panneaux, durées, calques, textes,
caméra, son) ; les dessins sont des `.tvg`, format de Toon Boom non documenté, compressé et marqué
d'un certificat de licence Toon Boom : ni leur lecture ni une écriture acceptée par Storyboard Pro
ne sont garanties.

Choix d'Anthony : passer par les imports officiels de Storyboard Pro.
- Vers Pelure : lire le `.sboard` (structure, durées, textes, caméra, son) ; les dessins, à partir
  des images que Storyboard Pro exporte (« Export: Bitmap »).
- Vers Storyboard Pro : un dossier que ses propres commandes réimportent — images par plan et par
  calque (« Import Images as Shots », « Import Images as Layers »), textes (« Update Captions from
  CSV »), son (« Import AudioTracks »), montage (EDL / AAF / XML, « Conformation: Import Animatic
  Project ») — à vérifier commande par commande dans Storyboard Pro 27 avant d'écrire le code.

Relevé dans Storyboard Pro 27 (lu, jamais lancé) : le format CSV des légendes et les règles de
nommage de « Images as Shots » ne sont pas décrits ; son interface de script (Qt Script,
ECMAScript 3), elle, est documentée (aide `help/storyboard`). Choix d'Anthony (2026-10-16) :
- vers Storyboard Pro : un script Storyboard Pro écrit par Pelure (plans, panneaux, durées, un
  calque par calque, légendes, sons) ;
- depuis Storyboard Pro : un script d'export des dessins écrit par Pelure ; Pelure lit le
  `.sboard` pour le reste ;
- d'abord le sens Storyboard Pro → Pelure ; la caméra plus tard (unités Toon Boom à convertir).

- ✅ 14a. Importer un projet Storyboard Pro (Fichier, ou glisser le `.sboard` sur la fenêtre).
  - Le `.sboard` est lu (jamais modifié) : chaque plan devient un plan, dans une séquence du même
    nom ; ses panneaux, mis bout à bout, deviennent ses images de storyboard avec leurs légendes
    (Dialog → dialogue, Action Notes → action, les autres → notes, préfixées de leur nom) ;
    cadence et taille du projet ; calques par leur nom, du fond au premier plan.
  - Dessins : « Créer le script… » écrit `export_pelure.js` dans un dossier choisi et le copie ;
    dans Storyboard Pro (vue Script Editor, Sandbox, coller, Run), il ne laisse voir qu'un calque
    à la fois, exporte chaque panneau en PNG transparent, remet les calques comme avant (et annule
    tout à la fin), puis écrit la liste `pelure.txt`. Pelure range ces images sur les calques
    (deux images identiques ne font qu'un dessin ; une image à une autre résolution, de mêmes
    proportions, est remise à la taille du cadre). L'opacité des calques est dans les images.
  - Son : les pistes et leurs extraits (début, fin, découpe) ; un extrait au volume, au tempo
    (« WSOLATempo », vitesse sans changer la hauteur) ou à la vitesse (« Resample ») changés
    devient son propre son, calculé à l'import. Enveloppes de volume, réverbération et changement
    de hauteur ne sont pas repris (signalés dans le compte rendu).
  - Vérifié sur les projets BRAWLIDAYS (partie 1 : 12 plans, 39 panneaux, 834 images ; partie 2 :
    45 plans, 322 panneaux, 3 323 images, lue en 0,2 s). Le script n'a pas pu être essayé ici
    (Storyboard Pro n'est jamais lancé par Claude) : à essayer par Anthony.
- ✅ 14a-2. Ouvrir sans rien demander (Anthony : « il faut automatiser le script à l'ouverture du
  fichier… c'est induit ») et garder les calques vectoriels.
  - **Ouverture** : Fichier > Ouvrir accepte les `.sboard`, avec une entrée dédiée sous « Ouvrir… ».
  - **Export automatique** : Pelure copie le projet dans un dossier temporaire (sans `logs/` ni
    verrous), y lance Storyboard Pro en batch avec le script (syntaxe de sa documentation), suit
    l'avancement, puis ouvre le projet. « Arrêter » ouvre sans les dessins.
  - **Traits vectoriels** : le script lit ceux de chaque calque vectoriel avec l'interface de
    script étendue de Storyboard Pro (`Drawing.query.getStrokes`, couleurs par
    `ColorManager.getColorDataWithId`) et les écrit en JSON.
  - **Reconstruction** : Pelure refait les zones colorées (bordées par les traits qui n'ont la
    couleur que d'un côté) et les traits de crayon, puis les cale sur l'image du même calque.
    Le calque reste vectoriel si le recouvrement atteint 60 % ; sinon il garde l'image.
  - **Calques vides** : un calque vectoriel sans dessin reste vectoriel.
  - **Non vérifié ici** : le mode batch et la lecture des traits n'ont pas pu l'être (Claude ne
    lance jamais Storyboard Pro). La façon de désigner le dessin d'un calque n'est décrite que pour
    Harmony : le script en essaie plusieurs et note dans la liste celle qui marche.
  - **Corrigé après l'essai d'Anthony (2026-10-17)**, avec un script d'essai lancé par lui sur
    son projet NAJI (`Documents\Work\analyse-sbpro`, analyse statique de Storyboard Pro faite
    par un agent, sans le lancer) :
    - le dessin d'un calque se désigne par l'élément du calque et le nom de son dessin
      (« 27_1 »), en `exposure`, traduit par `Drawing.Key`. En `drawingId`, le nom désigne un
      autre dessin (l'identifiant interne) : le script ne l'essaie plus, il aurait pu importer
      le mauvais dessin sans rien dire ;
    - le dessin est lu d'un coup par `Drawing.query.getData` : zones (une boucle fermée et sa
      couleur), traits de crayon. Les dessins d'Anthony sont faits de zones (traits de brosse),
      presque toutes d'une couleur texture ;
    - les couleurs vont une fois dans `couleurs.json` ; une texture donne sa couleur moyenne
      (pondérée par l'opacité de ses pixels, et pour opacité leur couverture) ;
    - le `sequenceName` « 0 » des plans n'est plus une séquence quand la méta du projet
      `sequenceExists` est vide (projets NAJI, BRAWLIDAYS partie 1) ;
    - vérifié : les dessins lus par l'essai sont refaits en zones et rendus justes (essai ignoré
      `real_getdata_drawings_from_the_test_script`, images avec `PELURE_SB_PNG`).
  - **Essai d'Anthony de l'ouverture automatique (2026-10-17) : échoue.** Storyboard Pro démarre
    en batch (son journal s'arrête après l'initialisation de ses greffons) mais le script ne
    tourne pas : `pelure.txt` n'est jamais écrit. Mis de côté à sa demande (« laisse tomber »).
    Pistes, non vérifiées : la ligne de commande batch, ou un script qu'on ne peut pas lancer
    ainsi. Le script à la main (Script Editor, Sandbox, coller, puis le bouton Run, une icône
    « ƒ » sans texte) a marché pour l'essai de lecture.
- ✅ 14b-1. Exporter vers Storyboard Pro (Fichier > « Exporter vers Storyboard Pro… »). Choix
  d'Anthony : partir de son projet modèle (choisi une fois, gardé dans les réglages), dessins en
  vecteurs si possible ; 14b-1 les envoie en images, les vecteurs viennent en 14b-2.
  - **Pelure prépare** dans un dossier temporaire une image PNG par calque et par image de
    storyboard : le cadre vu par la caméra au centre, à la première image de l'image de
    storyboard, à pleine opacité. Deux images identiques ne font qu'un fichier. Il écrit aussi un
    WAV par extrait de son, au volume de sa piste, et le script de construction, données
    comprises.
  - **Storyboard Pro construit** (batch, sur une copie du modèle) :
    - un plan par plan, un panneau par image de storyboard (durée, légendes Dialog / Action Notes
      / Notes, ajoutées si le modèle ne les a pas) ;
    - les calques, du fond au premier plan : image importée, ou calque vide du même genre ;
      nom, visibilité, opacité ; les calques vides du panneau neuf sont enlevés ;
    - les séquences (s'il y en a plusieurs, ou une qui a un nom) et les sons ;
    - puis il enlève les plans et les pistes du modèle et enregistre le projet (« Enregistrer
      sous » dans le dossier choisi ; à défaut, Pelure y copie le projet enregistré sur place).
  - **Compte rendu** de ce qui n'est pas repris tel quel : l'animation dans une image de
    storyboard (premier dessin seulement), la caméra, les modes de fusion, les dossiers (mis à
    plat), les pistes coupées (exportées quand même).
  - **Vérifié** : la préparation (tests) ; le script, exécuté sous Node contre une imitation de
    l'interface de script écrite d'après sa documentation ; sa syntaxe ES3 (JScript) ; la fenêtre
    avec un faux Storyboard Pro. **Non vérifié** : Storyboard Pro lui-même (jamais lancé par
    Claude) — à essayer par Anthony. Points incertains, documentation muette : taille donnée par
    Storyboard Pro à une image importée, nom des légendes en HTML ou en texte, nom du `.sboard`
    après « Enregistrer sous ».
  - L'import note maintenant la mesure du calage des vecteurs (pixels par unité, sens de l'axe,
    origine) : elle servira à 14b-2.
- 14b-2. Les calques vectoriels en vecteurs (`DrawingTools.createLayers`), quand l'essai de
  l'import aura dit quelle façon de désigner un dessin marche et l'échelle mesurée.
- Caméra de Storyboard Pro (plus tard).

## Propagation façon EbSynth (menée dans une session parallèle)

- ✅ Étapes a à d : la caisse `synthese/` (synthèse par l'exemple, flux optique, mélange des clés,
  carte graphique).
- ✅ Étape e (fusionnée le 2026-10-16, cdb93d2) : « Propager la peinture sur la vidéo… », au clic
  droit sur la ligne d'un calque peint au-dessus d'un calque vidéo (commande `calque.propager`).
  Tout est décrit dans `docs/PROPAGATION.md`.

## Passe interface (demandée par Anthony le 2026-10-16, avant 14b)

Simplifier l'interface et combler ce qui manque :
- poignées pour glisser les clés dans la timeline, étirer les calques, les monter ou les
  descendre en les glissant ;
- clic droit partout où c'est possible, et sur chaque icône, « Assigner un raccourci » ;
- dans Fichier, la résolution du projet et une fenêtre des raccourcis.

- ✅ Raccourcis :
  - clic droit sur une icône, un interrupteur ou une entrée de menu → « Assigner un
    raccourci… » : on tape la combinaison, Pelure signale celle qui est déjà prise, puis
    Remplacer, Ajouter, Retirer ou Par défaut ;
  - Fichier > Raccourcis clavier… (F1) : toutes les commandes, avec une recherche ;
  - fichier personnel écrit par Pelure ;
  - nouvelles actions pour des icônes qui n'en avaient pas : lecture en boucle, avancer / reculer
    le plan.
- ✅ Fichier > Réglages du projet :
  - onglet Réglages : les propriétés, plus le numéro de la première image ;
  - onglet Résolution : préréglages, largeur et hauteur liées, dessins à l'échelle ou cadre seul
    (choix d'Anthony : les deux), cadence en gardant les secondes ou les images.
- ✅ Montage : la case « Storyboard » devient la colonne du projet (choix d'Anthony : « seulement
  les boutons », comme la colonne de gauche du panneau Projet de TVPaint) : nouveau plan, plan
  avant, dupliquer, supprimer, nouvelle séquence, verrou, réglages, nom du projet.
- ✅ Poignées (glisser, une étape d'annulation par geste, Échap annule) :
  - dans la timeline du plan : les marques d'image, les signets, l'entrée et la sortie, les clés
    d'effets ; le bord droit d'un dessin étire ou raccourcit son exposition (ce qui suit glisse,
    rôles et clés d'effets compris) ;
  - dans le montage : les marques d'image ;
  - les clés de caméra se prenaient déjà (12e).
- ✅ Monter ou descendre un calque en le glissant :
  - dans le panneau Calques et dans les noms des pistes du montage (un trait montre où il se
    pose) ;
  - les en-têtes de la timeline le faisaient déjà (13b).
- ✅ Clic droit :
  - **timeline** : règle (signet, marque, entrée et sortie, cadrage), caméra, clé d'effet, son,
    noms des lignes ;
  - **montage** : pistes de calques, caméra, barre de séquence, règle ;
  - **panneau Calques** : toute la ligne et le vide (nouveaux calques) ;
  - **dessin** : annuler, presse-papiers, sélection, vue, pelure, table lumineuse, caméra, mode.

  Chaque entrée est une commande : un clic droit dessus lui donne un raccourci.
- Pas faits :
  - glisser le début d'un calque vidéo (son menu a un champ) ;
  - ~~étirer une exposition dans les pistes du montage~~ (fait au chantier 5) ;
  - les rôles, qui suivent leurs images.

## Audit du cœur par Fable et chantiers de la timeline (2026-10-16)

Anthony : « J'aimerais avancer plus sur le corps du logiciel plutôt que les exports. » Fable a
audité le cœur, en lecture seule. Constat : rien d'annoncé sans effet, aucun plantage trouvé, mais
le geste de base de TVPaint manquait : prendre le bloc d'un calque pour le déplacer, le rogner ou
l'allonger. Fable a proposé dix chantiers ; Anthony a lancé les quatre premiers. Ses choix :
allonger tient le dernier dessin ; une poignée de calque s'arrête à la fin du plan.

- ✅ 1. Poignées de calque dans la timeline :
  - **barre sous les dessins** d'un calque : la glisser décale tout le calque, avec ses rôles et
    ses clés d'effets ;
  - **début** : plus tôt, le premier dessin est tenu ; plus tard, les images d'avant se vident,
    la suite ne bouge pas ;
  - **fin** : plus loin, le dernier dessin est tenu ; plus près, la fin est coupée ;
  - le bord droit d'un dessin du milieu étire son exposition, et la suite glisse.

  Aucune de ces poignées ne change la durée du plan : elles s'arrêtent à sa fin, et un calque
  raccourci laisse au plan sa durée. Un aperçu en direct, une étape d'annulation par geste.
- ✅ 2. Échap ou Ctrl+Z pendant n'importe quel glisser l'abandonnent : poignée, clé de caméra,
  durée ou place d'un plan, son, images, choix au rectangle, ligne de calque. Tout revient comme
  avant. Avant, les raccourcis étaient coupés pendant un glisser du montage.
- ✅ 3. Choisir des images :
  - glisser sur les cases choisit les images et les calques couverts (rectangle) ; glisser dans la
    règle fait toujours défiler ;
  - Ctrl+A dans la timeline choisit toutes les images de tous les calques ;
  - Suppr après un clic dans la grille supprime les images visées (l'image courante s'il n'y a pas
    de choix) au lieu de vider le dessin ;
  - le choix d'images est vidé au changement de plan (il visait l'autre plan) ;
  - les gestes de la timeline sont dans la liste des gestes de la souris (F1).
- ✅ 4. Durée du plan :
  - une prise au bout de la règle (et le trait de fin dans la ligne de la caméra) change la durée
    du plan ;
  - quand une durée change, que ce soit par la poignée, le montage, la touche +, un dessin posé
    après la fin ou un collage, et que du son suit, la barre d'état prévient : les plans suivants
    se décalent par rapport au son, qui reste en place. Le point d'entrée ne peut plus être posé
    sur le point de sortie par sa poignée.
- ✅ 6. Timeline et montage visibles ensemble (lancé par Anthony le 2026-10-17) :
  - deux panneaux, **Montage** et **Timeline**, dans les deux modes ; avant, la bande montrait le
    montage en storyboard et la timeline en animation. Chacun se ferme, se déplace ou se
    détache comme les autres panneaux ;
  - **revu le même jour** (Anthony : « on a perdu pas mal de choses en ajoutant cette timeline
    sous la partie montage ») : l'un sous l'autre, chacun n'avait que la moitié de la hauteur
    (2 pistes de calques sur 10 visibles dans le montage, 2 calques dans la timeline, le dessin
    plus petit). Ils sont maintenant **en onglets** dans la bande, à la hauteur d'avant
    (290 points) : l'onglet montré **suit le mode** (montage en storyboard, timeline en
    animation). On peut sortir un onglet pour les voir ensemble. Une disposition enregistrée
    l'un sous l'autre repasse en onglets ;
  - **puis retiré** (Anthony, le même jour : « je ne veux pas de double timeline. La timeline
    principale est celle du montage […]. La timeline d'animation est visible quand on double-clique
    sur le shot comme avant ») : **une seule bande**, le montage en storyboard, la timeline en
    animation, comme avant le chantier 6. Restent du chantier : la main au dernier clic (et son
    trait), la lecture du film ou du plan selon le bouton, les raccourcis selon la main ;
  - Copier, Coller, Suppr visent le dernier des deux où l'on a cliqué, quel que soit le mode ;
    un trait de couleur en haut du panneau le montre (un clic dans le dessin le retire) ;
  - le bouton Lecture du montage lit tout le film, celui de la timeline le plan ; la touche
    Lecture garde son sens (le film en storyboard, le plan en animation) ;
  - le bouton de mode de chaque barre fait entrer dans le plan ou revenir au storyboard ;
  - les deux panneaux suivent l'image courante.

  - **les raccourcis suivent le panneau qui a la main** (choix d'Anthony, 2026-10-17) : après
    un clic dans le montage, ceux du storyboard (Insert : nouveau plan, flèches d'un plan à
    l'autre, lecture du film) ; après un clic dans la timeline, ceux de l'animation (Insert :
    nouveau dessin, flèches dans le plan, lecture du plan) ; après un clic dans le dessin, ceux
    du mode du dessin, comme avant. Tab et la vue Vignettes changent le dessin et lui rendent la
    main. La barre d'état dit « raccourcis du montage » ou « raccourcis de la timeline ».
- ✅ 5. Pistes du montage modifiables (lancé par Anthony le 2026-10-17) :
  - **une piste par nom de calque** : le calque « Perso » de chaque plan est sur la même piste,
    où qu'il soit dans sa pile ; un plan qui n'en a pas y laisse un trou ; deux calques du même
    nom dans un plan ont chacun leur piste (« Perso (2) »). Avant : une rangée par rang, 8 au
    plus ;
  - **toutes les pistes** sont montrées (la bande défile) ;
  - **les gestes de la timeline** dans les pistes : le bord droit d'un dessin étire son
    exposition (la suite glisse), la barre sous les dessins décale le calque, ses bouts le
    rognent ou l'allongent. Le plan et le calque deviennent courants ; une étape d'annulation
    par geste ; Échap pendant le geste l'abandonne ; refusé sous le verrou du storyboard.

  Pas fait : la piste « Fond (scène) » reste en lecture seule (un même calque sur plusieurs
  plans) ; un nom de piste sans calque dans le plan courant ne se clique pas.
- ✅ 5 bis. Le montage comme banc principal (Anthony, 2026-10-17 : « La timeline principale est
  celle du montage avec séquence, shot et calques d'animation visibles. […] Tout doit pouvoir
  être fait depuis le banc de montage. La timeline d'animation sert à rentrer dans la
  précision ») ; ses choix : une image clé est un nouveau dessin, étirer un calque change sa
  durée.
  - **nouveau calque** : un bouton dans la barre du montage (calque vectoriel dans le plan
    courant : une piste de plus) ;
  - **nouveau dessin (clé)** : double-clic sur la piste d'un calque, ou clic droit « Nouveau
    dessin ici » ; au milieu d'une exposition elle est coupée là, sur une image vide le dessin
    tient jusqu'au suivant (ou la fin du plan). Le plan, le calque et l'image deviennent
    courants : on dessine la clé tout de suite ;
  - **étirer un calque** : le bout de sa barre (déjà là, chantier 5) ;
  - revu le même jour (Anthony : « faire glisser une clé avant ou après une autre. Permet
    d'étirer une clé pour changer sa durée d'exposition, même si ça change la durée
    d'exposition de la précédente ou la suivante ») :
    - **pistes plus hautes** (26 points au lieu de 20, puis 40 avec une barre plus épaisse et
      des noms plus grands : « la taille des calques n'a pas changé ») ;
    - **une clé prise par son milieu se place avant ou après les autres** : elle passe une
      voisine quand son milieu passe celui de la voisine ; chaque dessin garde sa durée, la
      durée du calque et du plan ne change pas ;
    - **étirer une clé** : la limite entre deux dessins se prend et se déplace ; l'un s'allonge,
      l'autre raccourcit d'autant (au moins une image), la suite ne bouge pas. (Dans la
      timeline, le bord droit d'un dessin garde son geste : la suite glisse.)
    - puis (Anthony, le même jour) : **copier / coller une clé** (Ctrl+C, Ctrl+V après un clic
      sur la piste d'un calque, ou clic droit) : elle se colle à l'image courante, avec sa
      durée, à la place de ce qui y était (un nouveau dessin, copie du sien) ; **supprimer une
      clé** (Suppr, ou clic droit) : le dessin d'avant la remplace en tenant plus longtemps,
      sinon celui d'après ; **supprimer en laissant un vide** (clic droit) : ses images restent
      vides, les autres clés ne bougent pas. Après un clic sur une piste, les raccourcis sont
      ceux de l'animation (la barre d'état dit « raccourcis des clés »).
    - puis (Anthony, le même jour : « choix et sélection de plusieurs clés à la fois, dupliquer
      une clé sur place, même geste sur la piste fond ») :
      - **plusieurs clés** : clic, elle seule ; Ctrl + clic, une de plus ou de moins ; Maj +
        clic, toutes celles de la piste entre la dernière cliquée et celle-ci, d'un plan à
        l'autre ; Ctrl+A, toutes celles du calque courant. Copier, coller (écarts et durées
        gardés), supprimer, dupliquer visent les clés choisies (cerclées de la couleur du choix) ;
      - **dupliquer sur place** (Ctrl+D, ou clic droit) : la copie prend la seconde moitié de
        l'exposition, la suite ne bouge pas (une clé d'une image : rien) ;
      - **la piste « Fond »** : chaque calque de toute la scène a sa piste, teintée « fond », avec
        les mêmes gestes ; il garde ses clés d'un plan à l'autre de la scène (avant : un seul
        dessin tenu, et une barre en lecture seule).

      - puis **glisser plusieurs clés à la fois** (Anthony : « Maj pour sélectionner les clés
        voulues à la suite, Ctrl pour des clés qui ne se suivent pas forcément », ce que fait le
        choix) : une clé choisie prise par son milieu emmène les autres ; toutes glissent du même
        nombre d'images, des clés choisies qui se suivent restent un bloc, chaque bloc (ou clé
        seule) prend sa place parmi les autres selon son milieu, toutes gardent leur durée ; sur
        plusieurs calques et plusieurs plans, chacun dans son calque. Échap pendant le geste rend
        tout comme avant.
      - **repère de pose** (Anthony : « quand on déplace une clé, on ne voit rien à l'écran qui
        indique où elle va venir se déposer. Un système sur Storyboard Pro indique avant ou
        après quelle clé elle vient se déposer ») : pendant qu'on glisse une ou plusieurs clés,
        rien ne bouge ; un fantôme suit la souris et un repère (trait blanc à deux pointes) montre
        la limite entre deux clés où elles se poseront ; elles se posent au lâcher ;
      - **choix au rectangle** : un glisser qui part d'un espace vide des pistes de calques (ou,
        avec Ctrl ou Maj, de n'importe où dans les pistes) choisit les clés que le rectangle
        touche, sur les pistes et les images couvertes ; Ctrl ou Maj les ajoute au choix ; Échap
        rend le choix d'avant. (Glisser dans la règle fait toujours défiler.)
      - **zoom de la bande** (Anthony : « il existe déjà, il faut le mettre en bas de la
        timeline, pas en haut, et le rendre plus petit. Des raccourcis clavier seraient
        pertinents ») : le curseur du montage passe dans une petite barre en bas (zoom arrière,
        curseur, zoom avant, tout voir) ; Ctrl + molette sur la bande ; raccourcis proposés
        Alt+1 (arrière), Alt+2 (avant), Alt+3 (tout voir), comme 1 et 2 pour le dessin : ils
        visent le montage en storyboard, la timeline en animation ;
      - **F5 : nouveau dessin clé** à l'image courante du calque courant, partout (Anthony :
        « pour moi F5 », puis « F5 pose une clé partout. F6 coupe le plan ») ; « couper le plan »
        passe de F5 (son raccourci de Storyboard Pro) à F6. Puis (Anthony : « quand une clé est
        créée avec F5 sur un calque déjà dessiné, la clé ne doit pas être vide » ; « l'option
        d'une clé vide doit cependant exister, […] comme F4 ») : F5 et le double-clic sur une
        piste copient le dessin montré à l'image (sinon le dernier d'avant, mon choix) ; F4 pose
        une clé vide ; les deux au clic droit d'une piste ;
      - **outils à ressort** (Anthony : « chaque outil peut être utilisé par pression continue
        d'une touche, au relâché on revient sur l'outil précédent ») : la touche d'un outil tenue
        (0,35 s au moins, ou l'outil utilisé pendant qu'on la tient) rend l'outil d'avant au
        relâché ; une pression brève change d'outil comme avant. **Ctrl tenu seul sur le
        dessin : le lasso** (outil Sélection) le temps de le tenir ; Ctrl + une autre touche ou
        la molette n'y touche pas ; le tampon de duplication garde son Ctrl + clic. Le retour
        attend que la souris soit relâchée ;
      - **ordre des calques propre à chaque plan** (Anthony : « l'organisation et l'ordre des
        calques indépendants d'un shot à un autre ») : la rangée r montre, sous chaque plan, son
        r-ième calque en partant du haut de sa pile ; plus de pistes par nom ni de trous ; le nom
        de chaque calque est dans sa case, la colonne de gauche montre ceux du plan courant ;
        monter ou descendre un calque ne change que son plan. Maj + clic et le rectangle vont
        par rangée ;
      - **collage multi-calques** : des clés copiées sur plusieurs calques se collent chacune sur
        le calque de même nom du plan courant (créé s'il manque, à la même place dans la pile),
        avec leurs écarts et leurs durées ; venues d'un seul calque, sur le calque courant ;
      - **colonne de gauche étirable et réglages des calques** (Anthony : « ajoute la
        possibilité d'étirer la fenêtre caméra et calques en ajoutant […] les options de
        calques, options de fusion, opacité, option d'animation en boucle, etc. En clic droit
        tout ce qui a trait aux calques ») : une barre au bord droit de la colonne l'élargit
        (largeur retenue ; double-clic : tout montrer ou revenir étroit). Selon la place, chaque
        rangée de calque montre afficher, verrouiller, pelure d'oignon, le menu ⋮, l'opacité, le
        mode de fusion et « après la dernière image » (rien, boucle, va-et-vient, maintenir) ;
        la boucle se voit en pâle dans les cases du montage. Clic droit (ou ⋮) : tout ce qui
        touche au calque, puis toutes les commandes des calques ; double-clic sur le nom : le
        renommer sur place ;
      - **premier dessin sur un calque vide : tout le plan** (Anthony : « quand je dessine sur
        un plan vide, le calque se remplit du début à la fin du plan et pas à partir de la
        position de la tête de lecture ») : en storyboard ; un calque qui a déjà un dessin
        garde la règle d'avant (de la tête de lecture au dessin suivant ou la fin du plan) ; en
        animation, toujours « en N » images à la tête de lecture, comme TVPaint (mon choix) ;
      - **clé précédente / suivante, tous les calques** (Anthony : « passer d'une clé à une
        autre, tous calques compris ») : A / F (choix d'Anthony ; d'abord proposés sur PgPréc /
        PgSuiv, qui passent à « plan précédent / suivant », A / F de Storyboard Pro). En
        storyboard, dans tout le montage ; en animation, dans le plan. Aussi dans le menu des
        instances ;
      - **tête de lecture de la timeline** (Anthony : « elle s'arrête quand je la fais glisser
        sur l'entrée ou la sortie d'un plan ») : posée sur l'entrée, la sortie, une marque ou la
        fin du plan, sa case dans la règle prenait la prise au lieu de la tête ; la case de la
        tête passe maintenant avant. Dans le montage, deux essais (règle, piste Caméra) passent
        d'un plan à l'autre sans s'arrêter.
        Puis (« de nouveau […] la tête de lecture qui s'arrête à l'entrée et sortie de chaque
        plan ») : reproduit sur une copie de son projet. C'était une régression de la colonne
        étirable (d09e2f3) : ses boutons, plus ou moins nombreux selon les calques du plan
        courant, changeaient l'identité egui de la bande ; en entrant dans un plan qui n'avait
        pas le même nombre de calques, le glisser ne la reconnaissait plus. La colonne est
        maintenant dessinée dans sa propre zone (`new_child`) ;
      - **déplacer la sélection aux flèches** (Anthony : « déplacement au pixel, déplacement par
        10 pixels quand Maj est maintenu ») : traits vectoriels choisis ou zone bitmap, le
        dessin ayant la main ; Ctrl / Alt + flèches gardent leurs raccourcis ; sans sélection,
        les flèches changent d'image ou de calque comme avant. Corrigé après son essai (« ça ne
        marche pas ») : choisis au lasso de Ctrl, Ctrl relâché, les traits restaient choisis
        avec le pinceau revenu, mais les flèches n'agissaient qu'avec l'outil Sélection ; elles
        agissent maintenant quel que soit l'outil. Une zone bitmap choisie au lasso de Ctrl
        disparaissait au relâché : l'outil Sélection reste alors (mon choix) ;
      - **panneau Calques** (Anthony : « la zone [des boutons] doit rester en haut. [Le verrou]
        ne fonctionne pas. Le clic droit sur les calques n'est pas disponible ») : les commandes
        passent sous le mode et l'opacité, seule la liste défile ; l'œil, le verrou, la vignette,
        le nom et le clic droit ne marchaient plus depuis la « passe interface » (la zone de glisser
        de chaque rang, posée par-dessus, prenait leurs clics) ; le clic droit donne le menu
        complet du calque. Puis (« ne doit pas faire doublon avec les options déjà disponibles
        dans la fenêtre de calques ») : dans le panneau, ce menu n'a plus afficher, opacité, mode,
        dupliquer, supprimer, verrouiller, ni dans « Autres commandes » ce que font ses boutons ;
        ailleurs, le sous-menu des commandes ne répète plus ce que le menu du calque a déjà
        (`layerops::OpsFrom`). Puis (« en premier les fonctions simples, puis les fonctions
        d'état, ensuite les fonctions d'animation ») : renommer, dupliquer, verrouiller,
        supprimer ; afficher, pelure, transparence, pochoir, position, opacité, mode, profondeur,
        groupe ; avant / après ses images, cassure et création automatiques ; puis le reste
        (`rename_item`, `depth_item`, `duplicate_lock_delete_items` sortis de `layer_menu_where`) ;
      - **partie Effets** (Anthony : « ajoute une partie effets [avec] les flous, les niveaux,
        contrastes, etc. Les effets s'inspirent de Photoshop et sont non destructifs […] Une
        fenêtre effets peut être prévue ») : au clic droit sur un calque, après l'animation :
        opacité, mode de fusion, puis Flou, Réglages, Netteté, Bruit, Pixéliser, Styliser en
        sous-menus de cases à cocher, « Fenêtre Effets… » (la pile d'effets de 13i, renommée,
        son « Ajouter » rangé par familles), et la propagation de la peinture. Douze effets
        nouveaux (`fxfilters.rs`), avec les clés et le cache des flous. Puis (« ajoute
        Déformation et Rendu ») : onde, ondulation, tourbillon, pincement ; nuages, éclairage,
        dégradé (`fxwarp.rs`) ; le rendu texture ou éclaire le dessin au lieu de remplir la
        feuille (mon choix). Pas faits : les galeries (de filtres, de flous), la vidéo de
        Photoshop, Liquéfier. Puis (« la partie effets doit être une fenêtre dans l'espace de
        travail ») : panneau Effets (`Pane::Effects`, disposition version 7) sous les onglets des
        réglages, au lieu de la fenêtre flottante ; « Fenêtre Effets… » retiré du clic droit (à
        sa demande) ; la baguette de la timeline et Affichage > Panneaux l'ouvrent ;
      - **panneaux rétrécis toujours utilisables** (Anthony : « ces fenêtres ne sont pas
        optimisées pour rester lisibles et accessibles lorsqu'on réduit la taille ») : dans les
        panneaux de côté, curseurs et listes à la largeur du panneau (`dockui::fit_to_pane`,
        `fit_w`) ; onglets qui débordent : une barre de défilement ; Calques : mode et opacité à
        la ligne, vignette plus petite ; Couleur : réglages de la roue et choix des curseurs sur
        deux lignes au lieu de se chevaucher ; Effets : commandes à la ligne. Vérifié à 1050 ×
        720. Puis l'espace de travail réorganisé (choix d'Anthony : « A, avec les Effets sous les
        Calques ») : à droite du dessin, les calques avec les effets dessous, puis les réglages
        (vue, guides, fiche en onglets) avec la couleur et les pinceaux dessous ; disposition 8,
        qui remplace celles de ce jour (6, 7) ; les espaces de travail enregistrés ne changent pas ;
      - **outils de la vue sous le dessin, guides réparés** (Anthony : « ajoute les outils de la
        fenêtre vue sous l'espace de travail. La fenêtre vue devient inutile. Les guides ne
        fonctionnent pas, les traits ne s'alignent pas sur le guide quand l'aimant est
        activé ») : une barre sous le dessin (ajuster, 100 %, miroir, rotation, table lumineuse
        et sa force), sauf dans la vue 3D ; le panneau Vue disparaît des réglages et des
        dispositions enregistrées. Aimant : le début du trait oubliait le guide juste après l'avoir
        retenu ; vérifié en essai (trait ondulé redressé à l'horizontale) ;
      - **tampons retirés** (Anthony : « supprime les outils tampon et tampon de duplication ») :
        outils, réglages, actions `outil.tampon` / `outil.tampon_duplication` (un fichier de
        raccourcis qui les nomme encore les ignore avec un avertissement) ;
      - **cases d'outils groupés, sélection par couleur** (Anthony : « les outils lignes,
        rectangle, ellipse et polyligne sont dans un seul groupe qui s'affiche quand le clic est
        maintenu sur l'icône de base (le rectangle). Ajouter l'outil sélection au-dessus du
        pinceau. Dans le même groupe, l'outil de sélection par la couleur ») : une case de la
        barre d'outils montre le dernier outil pris parmi les siens (mon choix, comme dans les
        logiciels de dessin ; au début le rectangle, la sélection), un petit coin, et la liste au
        clic maintenu (0,35 s ; pas au clic droit, qui sert à donner un raccourci). Outil
        Sélection par couleur (`outil.selection_couleur`, sans touche comme dans Storyboard
        Pro 27 : DRAWING_SELECT_TOOL_BY_COLOR_KEY) : bitmap, zone de la couleur cliquée (pixels
        voisins ou toute l'image, tolérance) ; vectoriel, tous les éléments de cette couleur ;
        comportement exact de Storyboard Pro à vérifier. Pour déplacer à la souris la zone choisie :
        flèches, poignées ou outil Sélection ;
      - **enregistrement « introuvable », propagation « disparue »** (Anthony : « quand
        j'enregistre un fichier Pelure, il n'apparaît pas dans l'explorateur Windows » ; « je ne
        trouve plus la propagation de couleur ») : rien n'était perdu. Le projet est un dossier à
        son nom (`Work\TEST PELURE\TEST PELURE.pelure`), la barre d'état le dit maintenant, et
        Fichier > « Afficher le projet dans l'Explorateur » (`fichier.afficher_explorateur`) le
        montre. La propagation n'était proposée que sur un calque bitmap ayant un autre calque à
        suivre ; le calque par défaut est vectoriel. L'entrée est maintenant toujours là (menus du
        calque, menu Calques), grisée avec la raison ; le fichier `propagerui.rs` (autre session)
        n'est pas touché ;
      - **cutter** (Anthony : « ajoute l'outil cutter qui coupe les lignes vectorielles qui se
        croisent ») : `outil.cutter`, Alt+T (touche de Storyboard Pro 27, déjà réservée), case
        après la gomme. Glisser en travers des bouts de traits (ou un clic) : chaque trait touché
        est coupé à ses croisements avec les autres traits et avec lui-même, les morceaux touchés
        partent ; un trait qui ne croise rien part en entier (mon choix) ; zones et textes ne
        bougent pas. Comportement exact du Cutter de Storyboard Pro (lasso, rectangle…) à
        vérifier. Vérifié en essai (dépassement d'un T coupé net) ;
      - **coller une image du presse-papiers de Windows** (Anthony : « ajoute la possibilité de
        coller une image copiée [dans] le presse-papiers de Windows, issue d'internet ou autre ») :
        Ctrl+V / Édition > Coller prend l'image du presse-papiers si elle ne vient pas de Pelure
        (les copies de Pelure y écrivent « Pelure : … », maintenant aussi celles des plans, des
        images et des clés). Calque bitmap courant, sinon nouveau calque bitmap « Image collée » ;
        centrée dans le cadre, réduite pour y tenir, sélectionnée. egui avale Ctrl+V quand le
        presse-papiers n'a pas de texte : le relâché de V avec Ctrl en tient lieu. `arboard`
        (déjà dans Cargo.lock, rien téléchargé). Vérifié en essai avec une image fabriquée (le
        presse-papiers d'Anthony n'est pas touché) ; la copie depuis un navigateur est à essayer ;
      - **code couleur des sortes de calques** (Anthony, capture annotée : « code couleur selon
        type de calque : bitmap, vecto, CTG ») : bitmap vert, vectoriel bleu (les couleurs que
        le montage et la timeline avaient déjà), CTG rose (nouveau). Panneau Calques : le rang
        courant (ou choisi) teinté de sa couleur au lieu du bleu d'accent, une bande à gauche de
        chaque rang, « bitmap / vectoriel / CTG » écrit dans sa couleur ; en-tête du montage : la
        même bande ; dessins et barres du montage et de la timeline, lettre de la sorte dans
        l'en-tête de la timeline. Projet enregistré en dossier : gardé (choix d'Anthony, comme
        Storyboard Pro) ;
      - **journal de plantage** (Anthony : « le logiciel crashe très vite lors de l'utilisation de
        l'outil de propagation de la couleur ») : rien dans le journal de Windows, donc des erreurs
        de programme (panic) qui fermaient Pelure sans trace. `plantage.rs` : le message, l'endroit
        du code, le fil et la pile dans `%LOCALAPPDATA%\Pelure\plantages\`, signalé dans la barre
        d'état à la séance suivante ; numéros de ligne dans la pile (`debug = "line-tables-only"`,
        dans le .pdb). Une propagation simple (47 images) passe en essai sans planter : cause à
        trouver avec le premier rapport ;
      - **commandes de la vue sous le dessin** (Anthony, capture : « change la disposition comme
        ceci ») : planche, caméra, 3D, ajuster, zooms, rotations, miroir, pelure d'oignon, table
        lumineuse quittent la barre du haut pour la barre sous le dessin (avec le 100 %, le
        curseur de rotation et la force de la table lumineuse) ; en 3D, la barre ne garde que les
        vues, pour en sortir ; panneau étroit : curseurs plus courts ;
      - **propagation : cadence** (Anthony : « l'option de créer des images uniquement sur les clés
        existantes ou de créer des images à différentes cadences d'images à choisir, avec l'option
        de suivre les clés existantes ou non ») : une image toutes les N images, ou seulement aux
        clés du guide ; « suivre les clés du guide » relance la cadence à chaque clé. Les autres
        images tiennent l'image d'avant. « Les clés existantes » = celles du guide (l'animation
        suivie), les clés peintes restant les modèles (confirmé par Anthony le 2026-10-18). Fait dans
        `propager.rs` / `propagerui.rs` (session Propagation prévenue) ;
      - **propagation plus solide** (relecture du code, aucun plantage reproduit en essai) : la
        synthèse avait lieu sur le périphérique de la carte de l'affichage, et wgpu fait de toute
        erreur de la carte (manque de mémoire, perte du périphérique) un plantage de tout Pelure.
        Elle a maintenant son propre périphérique (celui de l'affichage seulement s'il ne s'ouvre
        pas) : une erreur arrête la propagation (« rien n'a changé ») sans fermer Pelure. Les
        erreurs de la carte de l'affichage sont notées dans un rapport au lieu de tout fermer.
        L'aperçu de progression est borné en hauteur. Reste à voir : la mémoire (toutes les images
        fabriquées gardées entières jusqu'à la fin, dessins jamais purgés), le travail final sur le
        fil de l'interface (longs plans : « ne répond pas »), un calque peint transformé (décalage) ;
      - **mémoire de la propagation** (Anthony, 2026-10-18 : « allège la mémoire de la
        propagation ») : la synthèse garde la séquence depuis la première clé sans ses carreaux
        nuls (`synthese::propagation::Sparse`, sans perte) et libère chaque image au mélange ; Pelure
        convertit chaque image gardée en dessin à tuiles dès qu'elle arrive (bruit sous 1 %
        d'opacité ignoré) et ne garde que la vignette d'aperçu. Vérifié : une image fabriquée de 2
        millions de pixels n'en a que 2 199 de peints. Mesure (48 images, deux clés, 1080p) : le pic
        ne baisse pas (1,7 Go, 456 Mo au repos) ; il vient du calcul de chaque image (environ 0,8 Go
        dès la première, puis +0,4 Go en cours de route, dans les deux versions). Prochaine piste :
        la mémoire de travail de la synthèse (tampons refaits à chaque image) ;
      - **ouverture en plein écran** (Anthony, 2026-10-18) : fenêtre maximisée à l'ouverture,
        comme Storyboard Pro (F11 / `vue.plein_ecran` reste le vrai plein écran). Sous Windows, la
        fenêtre créée cachée était marquée maximisée sans en prendre la taille : on la remet à sa
        taille puis on la maximise, quelques images après l'ouverture ;
      - **Transformer : étirement** (Anthony : « que l'outil de transformation ressemble plus à celui
        de Storyboard Pro qui permet beaucoup plus de transformations comme l'étirement latéral ») :
        poignées au milieu des côtés (étirer dans un seul sens, le côté opposé reste en place),
        coins en échelle libre (Maj : proportions), largeur et hauteur en chiffres.
        `LayerTransform::stretch_x` / `stretch_y`, format de projet 22. Comportement exact de
        l'outil de Storyboard Pro à vérifier. Reste : les poignées de Bézier des coins avec Ctrl ;
      - **retourner** (Anthony : « créer la possibilité de flipper un plan, flipper un calque et
        flipper une sélection ») : la sélection l'était déjà (Alt+H / Alt+V, maintenant aussi dans
        Édition) ; le calque, sur place, par un étirement négatif (`calque.miroir_h` / `_v`, menu
        Calques) ; les plans choisis de gauche à droite autour du milieu du cadre, calques et
        caméra (`plan.retourner`, « Flip Selected Shots » de Storyboard Pro 27, sans touche ;
        menus Storyboard et « … »). Mes choix : le calque autour du milieu de son contenu ; les
        calques de toute la scène pas retournés (partagés avec les autres plans) ;
      - **passe sur les raccourcis** (Anthony : « regarde s'il existe des raccourcis prévus pour des
        fonctions qui n'existent pas dans Pelure ») : 22 actions sans fonction, dont 19 avec une
        touche (presque toutes de Storyboard Pro 27, gardées en réserve). Puis (« retire ce qui
        est devenu inutile et fais les 5 rapides ») : retirées, pinceau pochoir, calque de toute la
        scène, lier les clips au panneau ; faites, Ctrl+W fermer le projet (un projet vide du même
        format, après « Enregistrer ? »), Ctrl+Maj+V coller sur place (le collage pose déjà à la
        place d'origine), Alt+Entrée propriétés du plan (la fiche devant), Ctrl+F aller à un plan
        (nom, séquence/plan ou rang, `aller.rs`), Maj+D flou du calque (l'effet Flou gaussien coché
        ou décoché). Restent en réserve : grille, outil zoom, zoom et rotation temporaires, peindre
        les zones non peintes, remplissage automatique, chercher dans les légendes, supprimer et
        resserrer, choisir tous les dessins du plan, épaisseur du crayon, aimanter au contour,
        aplatir, décharger la référence ;
      - **axes d'ergonomie** (Anthony : « ok pour changement de 1 à 9 ») : 9, Transformer et
        Perspective dans une même case ; 1, projets récents (Fichier > Projets récents, dix au
        plus, `reglages.toml` : `projets_recents`) et dernier projet rouvert au lancement
        (`rouvrir_dernier_projet`, pas si une séance interrompue attend) ; 2, palette de commandes
        (Ctrl+K, « proposition » : `palette.rs`, accents et casse ignorés, la commande elle-même
        puis un mot placé tôt dans le nom) ; 3, messages qui restent (historique au clic sur le
        message, texte entier au survol, notifications pour les avis importants : `messages.rs`) ;
        4, ce que font les touches avec l'outil courant, au bout de la barre sous le dessin
        (`viewbar::tool_hint`) ; 5, le panneau qui a la main encadré en bleu d'accent (montage,
        timeline ; le dessin, plus discret) au lieu d'un fin trait blanc en haut ; 6, une zone bitmap
        choisie survit au passage sur Transformer, qui la déplace (glisser dedans), l'agrandit ou
        la tourne (ses poignées), l'enveloppe (Ctrl + coin) ; les flèches la poussent (`Tool::keeps_zone`) ;
      - **enveloppe de Bézier** (Anthony : « maintenir Ctrl permet de saisir une poignée de Bézier
        des coins pour transformer la forme » ; son choix : « B, comme l'outil Perspective ») : dans
        l'outil Transformer, Ctrl + glisser un coin du cadre ouvre une enveloppe sur le dessin
        courant (4 coins, 2 poignées par coin ; surface de Coons), appliquée à chaque lâcher depuis
        le dessin d'origine (une étape d'annulation par geste), bitmap ou vectoriel (traits rendus
        plus denses pour se courber). Avec cet outil, Ctrl ne prend plus le lasso. Comportement
        exact de Storyboard Pro à vérifier. Puis (« l'enveloppe doit aussi pouvoir ne déformer que
        la sélection, comme l'outil Perspective le fait en vectoriel ») : Ctrl + un coin du cadre de
        la sélection vectorielle ne déforme que les éléments choisis. Puis (« Ajoute ») : une zone
        bitmap choisie aussi (seuls ses pixels, bords adoucis compris, `warp_raster_masked`) ;
        Ctrl + coin du cadre de la sélection marche aussi avec l'outil Sélection (la zone bitmap
        n'existe qu'avec lui ; une zone déjà déplacée est d'abord posée) ;
      - **peint derrière, remplir la forme** (Anthony : « pour le pinceau j'ai besoin de deux
        options : peint derrière et remplir la forme. Remplir la forme remplit par la couleur une
        forme fermée ») : deux interrupteurs en haut des réglages du pinceau et du crayon,
        bitmap et vectoriel. Fermé : le bout revient à 15 points d'écran du début, ou le trait se
        recoupe (sa boucle se remplit). Le remplissage est sous le trait, par-dessus le dessin
        d'avant (sous tout s'il peint derrière). Ils valent aussi pour le crayon et sont retenus
        d'une séance à l'autre (choix d'Anthony, `reglages.toml` : `peint_derriere`,
        `remplir_la_forme`) ; « Draw Behind » / « Auto Fill » de Storyboard
        Pro : comportement exact à vérifier ;
      - **fusionner les plans choisis, les calques choisis** (Anthony : « quand deux plans ou
        plus sont sélectionnés, l'option de fusionner les plans doit être possible. De même pour
        les calques ») : plans choisis par Ctrl ou Maj + clic dans le montage, qui se suivent
        dans une même séquence : « Fusionner les N plans choisis » (clic droit sur un plan, menu
        « … », menu Storyboard), sans repère aux jointures (Anthony : « fusionner les deux plans
        laisse un repère sur la timeline, tu peux les enlever ? ») : les textes et notes de la
        planche des plans fusionnés passent à l'image de storyboard d'avant
        (`Document::join_plans`, `drop_joint_mark`) ; « Réunir le plan et le suivant » et
        « Réunir avec le plan précédent » non plus (choix d'Anthony ; `join_next_plan_without_mark`,
        `join_next_plan` du modèle garde la marque). Calques : Ctrl ou Maj + clic dans le panneau Calques et dans la
        colonne du montage (comme dans la timeline) ; le bouton Fusionner du panneau fusionne les
        choisis quand il y en a plusieurs, le menu de la colonne du montage aussi.

  Pas fait : ces gestes sur la piste « Fond ».
- Restent (ordre de Fable) : navigation de calques à la TVPaint, fenêtre « Agrandir / rétrécir
  le calque », finitions.

## Moteur de dessin façon TVPaint, brosses façon Procreate, import de brosses (2026-10-16)

Anthony : « revois les outils de dessin pour avoir une sensation de dessin équivalente à celle de
TVPaint […] Ajoute des paramètres supplémentaires aux brosses […] comme dans Procreate […]
importer des brush Photoshop, Procreate ou autre. »

Fable a fait l'étude, en lecture seule. Ses sources :
- les fichiers de TVPaint 12 (`english.tvpaint.loc`, `defaulttools.conf`, 175 réglages de brosses
  `.ini`, papiers) ;
- la documentation officielle de TVPaint et sa référence George ;
- le manuel de Procreate.

Ses constats :
- **Connexions** : les sources pression, vitesse, direction, orientation, fondu, hasard,
  inclinaison et molette, chacune avec sa courbe. Les codes viennent de George : 10 pression,
  4 vitesse, 5 direction, 6 orientation, 7 fondu, 8 hasard, 11 altitude, 12 azimut.
- **Power** (débit qui s'accumule), plafonné par l'opacité.
- **Séchage**, étalement et décalage de la couleur, profil de pointe.
- **Papier** : dureté, taille, angle, inversion, double papier.
- **Lissage** de trois types.
- **Fichiers** : `.tvpx` est une archive tar, `.dip` une image IFF DEEP.

Choix d'Anthony :
- reproduire les quatre familles d'outils : crayons, Pen Brush, Custom Brush, peinture ;
- Power et Opacité réglables par brosse, le Pinceau rond inchangé ;
- la sensation d'abord, puis l'éditeur de brosses, puis les imports.

Étapes : 1 banc de traits ; 2 connexions ; 3 profil de pointe ; 4 Power ; 5 lissage ;
6 dispersion ; 7 papier ; 8 pointes multiples et animées ; 9 éditeur de brosses ; 10 imports
(TVPaint, `.abr`, Krita et MyPaint, Procreate) ; puis peinture humide et inclinaison du stylet.

- ✅ 2. Connexions :
  - **réglages connectés** : taille, opacité, angle et rondeur de la pointe ;
  - **sources** : pression, vitesse du trait, direction, fondu sur la longueur tracée, hasard ;
  - **chaque connexion** a une courbe de réponse de 2 à 8 points, « Inverser » et une réponse
    minimale ;
  - **vitesse** : mesurée d'une image d'écran à l'autre, avec une vitesse max réglable ;
  - **fondu** : longueur en pixels.

  Ce sont les « connexions » de TVPaint et la « dynamique » de Procreate.
  - Les anciens réglages (taille et opacité selon la pression, avec leur minimum) donnent
    exactement le même trait ; une connexion choisie les remplace.
  - L'aperçu des brosses trace lentement aux bouts et vite au milieu.
  - Pas encore : l'orientation et l'inclinaison du stylet, que Pelure ne lit pas. Sur un calque
    vectoriel, le trait gardé ne retient que la taille.
- ✅ 3. Profil de la pointe (le « Profile » de TVPaint) : une courbe de la couverture, du centre au
  bord, à la place de la dureté.
  - Il part de la dureté réglée, quand on l'allume ; préréglages Dur, Doux, Diffus.
  - Il se garde aussi sur un trait vectoriel (rejoué comme un pinceau texturé).
- ✅ 5. Lissage : les types du « Line Smoothing » de TVPaint, avec un calcul propre à Pelure :
  - Suivi : le lissage d'avant, par défaut ;
  - Corde tirée (longueur en pixels) ;
  - Moyenne sur des points, ou sur une distance.

  À la fin du trait, il rattrape la pointe. Pas encore : le rattrapage pendant une pause, et
  l'affichage de la corde.
- ✅ 10 (TVPaint). Import des brosses TVPaint : un export de panneau `.tvpx`, un panneau `.bin`
  avec son dossier `.data`, ou un réglage `.ini` seul (bouton « Importer des pinceaux »).
  - **Pointes `.dip`** : lues d'après les fichiers, sans spécification publiée. Le format est
    IFF DEEP ; la première image est compressée par plages de pixels, les suivantes d'une
    brosse animée sont des différences. Les 17 pointes essayées de TVPaint 12 se décodent
    exactement, jusqu'à 180 images. Une brosse animée devient une pointe en plusieurs cases.
  - **Réglages** : taille (% de la pointe), opacité, pas, dispersion, connexions et leurs
    courbes (codes de la référence George), profil des outils de base, débit (Power).
  - **Papier** : retrouvé par son nom dans les papiers de l'installation de TVPaint, en lecture
    seule.
  - **Compte rendu** de ce qui n'est pas repris : estompe, décalage de couleur, peinture qui ne
    sèche pas, inclinaison et azimut, modes de dessin. Il s'affiche dans une fenêtre.
  - Essayé sur les panneaux livrés : crayons (23 brosses, 21 pointes, 10 papiers), craies,
    feutres.
  - **Devinés, à comparer dans TVPaint** : le sens d'un code négatif, l'ordre des modes
    d'animation, l'échelle de la dispersion et celle du papier.
- ✅ Deux couleurs (choix d'Anthony le 2026-10-17 : la couleur d'arrière-plan sert au mélange de
  deux couleurs) : la connexion « Couleur A→B » mêle la couleur d'arrière-plan à celle
  d'avant-plan, selon l'appui, la vitesse, la direction, le fondu ou le hasard. Elle vaut aussi
  pour la peinture humide (la brosse se recharge de la couleur mêlée). L'option « Mix » des
  réglages Krita s'y importe. Sur un calque vectoriel, le trait gardé a une seule couleur. Pas
  de touche pour intervertir les couleurs, et les nuanciers restent hors de Ctrl+Z (réponses
  d'Anthony).
- ✅ 1. Banc de traits : Aide > « Enregistrer mes traits » garde chaque trait tel que le stylet le
  donne (positions, appui, instants) avec son réglage, dans
  `%APPDATA%\Pelure\traits\traits.jsonl`. Un essai ignoré (`PELURE_TRAITS`) les rejoue dans le
  moteur et les écrit en images : de quoi régler la sensation sur les vrais gestes d'Anthony et
  comparer avec TVPaint. À faire par Anthony : quelques traits types (hachures, boucle rapide,
  appui progressif, encrage), avec les mêmes gestes dans TVPaint.
- ✅ 11. Peinture humide, première version, avec un modèle de Pelure (la formule de TVPaint n'est
  pas publiée) :
  - **Mélange** : à chaque empreinte, la brosse prend la couleur moyenne sous elle, la mêle à la
    sienne et pose le mélange directement ; repasser mêle encore ;
  - **Charge** : la peinture s'épuise selon la distance parcourue ; vide, la brosse ne fait
    plus qu'étaler la couleur prise.

  À 0 de mélange et charge pleine, le trait d'avant. Les imports s'en servent, en le notant dans
  le compte rendu : l'estompe des brosses TVPaint, le smudge de MyPaint, le Wet Mix et la
  charge de Procreate.

  À comparer avec les outils de peinture de TVPaint (Oil, Wet, Water Color, Gouache). Pas encore :
  le séchage entre deux traits (Drying), le décalage de couleur (Shift).
- ✅ 10 (Procreate). Import des brosses `.brush` et des jeux `.brushset`. L'archive est un zip ;
  `Brush.archive` est une liste de propriétés binaire d'Apple au format NSKeyedArchiver, lue
  d'après le format publié par Apple.
  - Repris : `Shape.png` pour la pointe, `Grain.png` pour le papier ; le nom, la taille,
    l'opacité, l'espacement, la dispersion, le nombre et la rondeur ; l'appui et la vitesse
    sur la taille et l'opacité ; la rotation qui suit le trait.
  - Les échelles sont devinées (aucune documentation de Procreate). Le mélange humide et la
    double brosse ne sont pas repris.
  - Testé sur des fichiers fabriqués seulement : aucune brosse Procreate n'était sur le poste.
- ✅ 10 (Photoshop). Import des brosses `.abr`, d'après la structure que lisent GIMP et Krita :
  - versions 1 et 2, pointes échantillonnées ;
  - versions 6 et plus : les pointes (`samp`) et, au mieux, les préréglages (`desc` : nom,
    diamètre, dureté, angle, rondeur, espacement, pointe liée, appui sur la taille et
    l'opacité).

  Testé sur des fichiers fabriqués seulement : aucune brosse Photoshop n'était sur le poste.
  À essayer sur une vraie.
- ✅ 10 (Krita, MyPaint). Import des réglages Krita (`.kpp`, et les paquets `.bundle` entiers) et
  MyPaint (`.myb`), plusieurs fichiers à la fois :
  - **Krita** : le réglage XML compressé dans le PNG ; seul le moteur « paintbrush » est repris.
    La pointe est ronde (diamètre, rapport, fondu, espacement, angle) ou en image (`.gbr`,
    `.png`), cherchée dans le réglage, dans le dossier `brushes`, puis dans les paquets. Les
    capteurs pression, vitesse, angle du trait, fondu et hasard deviennent des connexions avec
    leurs courbes.
  - **MyPaint** : taille (rayon logarithmique et appui), opacité, dureté, pas, ellipse,
    dispersion.
  - Essayé sur les réglages de Krita de ce poste : 22 réglages sur 23 (le dernier est un autre
    moteur), et les paquets par défaut (64 et 82 réglages, pointes comprises).
  - Lecteur zip (`zipread.rs`, flate2), qui servira aussi pour Procreate.
- ✅ 9 (première partie). « Mes brosses » :
  - l'icône disquette (grille des Réglages et panneau Pinceaux) garde le réglage courant sous un
    nom ;
  - clic droit sur un réglage : renommer, dupliquer dans « Mes brosses », remplacer par le
    réglage courant, supprimer (ceux de Pelure se dupliquent seulement) ;
  - les réglages restent d'une séance à l'autre, avec les importés.

  Les sections des réglages (Forme et dispersion, Connexions, profil, lissage, débit, papier,
  pointe) font déjà l'éditeur de brosse. Reste à voir avec Anthony s'il veut un « atelier » à
  part, façon Procreate.
- ✅ 8. Pointes en plusieurs cases (les brosses animées de TVPaint, les `.gih` de GIMP) : l'image
  de la pointe porte plusieurs dessins côte à côte. Chaque empreinte en prend un : à la suite, au
  hasard, selon l'appui ou selon la direction. Une image peut être importée comme pointe : sa
  transparence, ou son noir sur fond blanc. C'est la base des futurs imports de pointes
  (`.gih`, brosses TVPaint).
- ✅ 7. Papier : taille du grain, angle, inversion (la « Hardness » de TVPaint est le curseur
  Grain), et n'importe quelle image importée comme papier (son gris est le relief, réduite à
  1024 pixels). Pelure ne copie aucun papier de TVPaint d'office : Anthony peut choisir lui-même
  les PNG de son installation. Le papier tel quel suit exactement le même calcul qu'avant. Pas
  encore : le double papier.
- ✅ 6. Forme et dispersion :
  - dispersion des empreintes en travers et le long du trait (le « Jitter » de TVPaint, les
    « Jitter Lateral / Linear » de Procreate) ;
  - nombre d'empreintes par pas (le « Count » de Procreate) ;
  - retournements au hasard d'une pointe en image.

  Une section « Forme et dispersion » montre aussi la rondeur, l'angle et les hasards de taille et
  d'angle, qui n'avaient pas de curseur (seuls les imports les remplissaient). Le hasard n'est
  tiré que pour ce qui est demandé : les anciennes brosses tracent le même trait.
- ✅ 4. Débit (« Power » de TVPaint), à côté de l'opacité, qui en reste le plafond :
  - « Accumuler » éteint (par défaut, le Pinceau rond inchangé) : le trait reste uniforme, au débit
    fois l'opacité ;
  - allumé : un passage dépose à peu près le débit, et repasser dans le même trait fonce jusqu'à
    l'opacité.

  Le dépôt de chaque empreinte est réglé d'après leur nombre au même point (diamètre ÷ pas) : un
  choix de Pelure, à comparer avec TVPaint, dont la formule n'est pas publiée. Autre point à
  comparer : d'après sa documentation, un Power à 0 ne rend pas la brosse transparente.
## Couleur (demandée par Anthony le 2026-10-16) ✅

La demande : « Le nuancier seul ne suffit pas, il me faut également la possibilité de passer
par une roue chromatique, de créer ses propres nuanciers, et tout ce qui est nécessaire pour
paramétrer et choisir ses couleurs. La possibilité de choisir deux couleurs que l'on peut
intervertir comme dans Photoshop est également une bonne idée. »

Menée dans une session parallèle, branche `couleur` (à fusionner par la session principale).

- ✅ Deux couleurs, avant-plan et arrière-plan, comme dans Photoshop : deux carrés qui se
  chevauchent (panneau Couleur et pied de la barre d'outils) ; un clic sur celui de derrière les
  échange ; « Intervertir les couleurs » et « Couleurs par défaut » (noir, blanc) ; les outils
  dessinent avec l'avant-plan. D = couleurs par défaut (touche de Photoshop, libre dans Pelure,
  à valider) ; intervertir n'a pas de touche (X, celle de Photoshop, colle le calque).
- ✅ Roue chromatique : anneau des teintes, et dedans un carré ou un triangle saturation /
  valeur, au choix ; clic ou glissé ; elle suit la couleur changée ailleurs.
- ✅ Réglages : curseurs TSV, RVB ou TSL avec leurs chiffres, code hexadécimal (#RRGGBB, #RGB),
  opacité du pinceau ou du crayon (la couleur elle-même n'en a pas), couleurs récentes (une
  rangée des dernières couleurs avec lesquelles on a dessiné).
- ✅ Nuanciers personnels : plusieurs, nommés ; créer, renommer, dupliquer, supprimer ; ajouter,
  retirer, nommer, remplacer, ranger en glissant ; choisir celui qui est montré ; import et
  export GIMP (`.gpl`) et Photoshop (`.aco`). Gardés dans `reglages.toml`, pas dans le projet.
- ✅ Clic droit partout dans le panneau (carrés, roue, curseurs, code, récentes, pastilles, nom
  du nuancier), chaque entrée étant une commande à laquelle on peut donner un raccourci ;
  Édition > Couleur.
- Vérifié par les tests (conversions, roue dans egui sans fenêtre, nuanciers, formats sur des
  exemples écrits à la main, réglages, raccourcis). Pas vu à l'écran : à essayer par Anthony.
- Pas faits :
  - la couleur d'arrière-plan ne sert qu'à l'échange : aucun outil ne l'utilise encore ;
  - la pipette prend toujours l'avant-plan (pas de touche pour prendre l'arrière-plan) ;
  - les notes de la planche et le tampon ne nourrissent pas les couleurs récentes ;
  - Ctrl+Z ne défait pas les changements de nuancier (la suppression est confirmée) ;
  - pas d'import Adobe Swatch Exchange (`.ase`) ; `.aco` en CMJN ou Lab : sans profil de couleur.

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
| Début / fin du montage | Ctrl+Origine / Ctrl+Fin | Origine / Fin = panneau (preset) ; en storyboard seulement depuis 13b |
| Panneau précédent / suivant (second) | Pg préc. / Pg suiv. | |
| Nouveau panneau (second, storyboard) | Inser | |
| Supprimer le panneau (storyboard) | Ctrl+Suppr | |
| Nouveau dessin (animation) | Inser | |
| Dupliquer le dessin | Ctrl+D | |
| Retirer le dessin (animation) | Ctrl+Suppr | |
| Allonger / raccourcir l'exposition | Alt++ / Alt+− | + / − = durée du panneau (preset) |
| Nouveau calque bitmap | Maj+B | Maj+S (perso) = vectoriel |
| Calque du dessus / dessous | Flèches haut / bas | ✅ ce sont aussi tes touches TVPaint 12 (LayerUp / LayerDown) |
| Couleurs par défaut (noir, blanc) | D | touche de Photoshop, libre dans les deux modes (passe couleur) ; « Intervertir les couleurs » reste sans touche : X, celle de Photoshop, est « coller le calque » (perso) |

### Touches TVPaint 12 (13b) qui prendraient celle d'une fonction de Pelure

Tes raccourcis TVPaint 12 libres dans Pelure sont posés (Maj + Haut / Bas : choisir en montant ou
descendant ; Ctrl + Haut / Bas : monter / descendre le calque). Pour les autres, décision
d'Anthony du 2026-10-15 :

| Action | Ta touche TVPaint | Déjà prise dans Pelure par | Décision |
|---|---|---|---|
| Pelure d'oignon du calque | Ctrl+L | « Lier les clips au panneau » (en réserve, sans objet depuis les plans) | ✅ Ctrl+L, la réserve n'a plus de touche |
| Premier / dernier calque | Ctrl+Origine / Ctrl+Fin | Début / fin du montage | ✅ en Animation ; le montage les garde en Storyboard |
| Afficher tous les calques | = | + (ajouter une image au plan) : même touche en AZERTY | sans touche |
| Nouveau calque d'animation | Ctrl+N | Nouveau projet | sans touche |
