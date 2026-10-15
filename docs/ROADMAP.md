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
- Décision d'Anthony (après ce bilan) : **Pelure egui reste le logiciel principal** (toile
  native : latence, carte graphique partagée) ; Pelure Studio sert de banc d'essai d'interface.

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
