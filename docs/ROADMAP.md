# Feuille de route

Légende : ✅ fait · 🔧 en cours · ⏳ à faire · ⏸ reporté

## Avant le plan (prototype)

✅ Montage de plans (vignettes, durées à la souris, vue planches), animation image par image
(expositions, pelure, lecture), pinceau à pression, calques bitmap et vectoriels (lasso, poignées,
gomme qui coupe, remplissage), annuler/rétablir, fichier `.pelure`, exports PNG et HTML.

## Phase 0 — Préparation ✅

- ✅ Suivi de versions (git, MinGit portable).
- ✅ `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`.
- ✅ `keymap.toml` : raccourcis par défaut + 8 perso, réserve « non
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
  Maj+G), enregistrés dans le projet.
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
  Pelure (la sélection de plusieurs panneaux n'est pas encore possible
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
    (sans touche), adoucir le mouvement à une clé (Pelure) ; clés
    visibles dans la timeline d'animation (ligne « Caméra ») et dans la bande de montage ; lecture,
    vue caméra et export suivent le mouvement. Reste : déplacer une clé à la souris dans la
    timeline, copier la caméra d'un panneau à l'autre, chemin de caméra affiché sur la feuille.
- ✅ 2c. Profondeur Z des calques (multiplan), vue de contrôle (vue de dessus).
  - ✅ 2c-1. Transformer un calque entier : outil Transformer (déplacer, échelle par un coin,
    rotation par la poignée), valeurs exactes, Ctrl+R pour annuler (jusque-là en réserve) ; on dessine normalement sur un calque transformé ; affichage, export et
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
- ✅ 2d. Import glTF de repère, en fil de fer (choix d'Anthony). **Retiré le 2026-10-21** (« on va rester simple ») : voir tout en bas, « Espace 3D simplifié ».
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
  la fenêtre reste vivante ; depuis 2026-10-20 (branche `exports-fond`) le document n'est plus bloqué : l'export copie le projet
  et calcule ses images sur d'autres fils (voir plus bas). ffmpeg : celui trouvé sur
  le poste (Krita, PATH, winget) ou choisi à la main (retenu dans les réglages). Image filmée
  4 à 8 fois plus rapide (zone filmée seulement, plusieurs cœurs) et images tenues pas
  recalculées : montage lourd de 960 images 1080p exporté en 23 s (avant : ~85 s de calcul,
  fenêtre figée). Couleurs vérifiées (BT.709). Pas encore : choix de la qualité, images JPEG, export EDL/AAF/XML.
- ✅ 3c. PDF de storyboard (Fichier > Exporter le storyboard (PDF), ou dans la fenêtre
  d'export) : A4 paysage, 6 panneaux par page (2 rangées de 3), image de début de panneau vue
  par la caméra, titre et durée, puis action, dialogue, notes sous l'image (« … » si le texte
  ne tient pas) ; nouvelle page à chaque séquence ; en-tête : projet, séquence, page n / N.
  Montage, scène ou panneau. Fabriqué sans bibliothèque (police standard Helvetica, accents
  français ; images JPEG) ; vérifié avec un lecteur PDF tiers (PDF.js). Pas encore : autres mises en page, police au
  choix, caractères hors de l'alphabet latin (remplacés par « ? »).
- ✅ 3d. Animation à la TVPaint (rendu visé : TVPaint — choix d'Anthony).
  - ✅ 3d-1. Pelure d'oignon : valeurs par défaut (3 avant en rouge, 3 après en vert, teintées, sous le dessin) ; compter par dessin
    ou par image ; jusqu'à 12 de chaque côté ; opacité du plus proche, puis de plus en plus pâle ;
    teinte ou vraies couleurs ; couleurs au choix ; réglages retenus d'une séance à l'autre.
    Pas encore : pelure à travers les panneaux, sur tous les calques.
  - ✅ 3d-2. Pinceaux texturés : grain du papier fixe sur le dessin (appuyer remplit les creux),
    pointes en image (avec réductions pour les petites tailles), pointe aplatie / tournée,
    variations au hasard (angle, taille), taille et opacité mini sous la pression. Réglages
    nommés dans le panneau Outils : 6 de Pelure (pinceau rond, crayon à papier, craie grasse,
    encre, gomme, gomme à grain) ; les pinceaux importés sont gardés dans
    %APPDATA%\Pelure\pinceaux. Rendu de Pelure (visé : TVPaint).
    Pas encore : mélange des couleurs (peinture humide), double pointe, inclinaison du stylet,
    grain sur les calques vectoriels.


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
  (réglable dans le menu
  Fichier, 0 = jamais), si le projet a changé, sans figer la fenêtre (écriture sur un autre
  fil). La copie va dans `%LOCALAPPDATA%\Pelure\secours`, jamais dans le projet lui-même : seul
  « Enregistrer » l'écrit (choix prudent). Fermeture normale : copie effacée. Après un plantage : fenêtre « Pelure s'est fermé
  sans enregistrer » (ouvrir la copie, la supprimer, ou plus tard), avec un avertissement si le
  projet a été enregistré après la copie. Barre d'état : « Secours il y a … ». Vérifié par un
  vrai plantage (processus tué) : copie retrouvée et rouverte, traits compris.
- ✅ 4b. Espaces de travail (Affichage > Espaces de travail) : quatre dispositions de Pelure
  (Standard ; Dessin, Timeline et Vue d'ensemble, sans touche, avec un contenu
  propre à Pelure) ; « Enregistrer la disposition… » sous un nom, et supprimer ; panneaux
  Outils, fiche et calques, montage / timeline à afficher ou masquer (Affichage > Panneaux) ;
  la disposition est retenue d'une séance à l'autre (AUTO_SAVE_LAYOUT = Y dans les préférences
  d'Anthony). Une disposition = panneaux affichés, leurs tailles, vue vignettes, vue de dessus.
  Pas encore : fenêtres détachées, plusieurs vues du même dessin.

## Phase 5 — Interface et nouveaux outils ✅

Demandée par Anthony après la phase 4. Choix faits : icônes Phosphor, fenêtres modulables dans
la fenêtre de Pelure (egui_dock), tampon **et** tampon de duplication,
perspective = déformer en perspective.

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
  Polyligne (sans touche). Courbe : tracer, puis glisser
  pour courber (la courbe passe par le pointeur) ; en S, deux glissers. Polyligne : un clic par
  point, double-clic ou Entrée pour finir, clic sur le premier point pour fermer.
- ✅ 5d. Texte : polices du poste, taille, couleur ; sur un calque
  vectoriel le texte reste modifiable (clic dessus), la Sélection le déplace, l'agrandit, le
  tourne ; sur un calque bitmap il devient des pixels. Police absente : celle de Pelure (signalé).
  Pas encore : crénage, alignement centré / à droite, gras ou italique autrement que par la police.
  Format de projet 6.
- ✅ 5e. Tampon (taille max et min,
  espacement, angle, rondeur, variations au hasard, miroirs, plusieurs images en cycle) avec six
  tampons de Pelure et les images importées ; tampon de duplication (Ctrl + clic pour la source,
  aligné ou non, calque courant ou tous les calques). Calques bitmap.
- ✅ 5f. Déformer en perspective : les 4 coins du dessin
  (ou de la sélection vectorielle), grille de perspective ; chaque geste repart du dessin d'origine.
  Pas encore : sur une sélection bitmap.
- ✅ 5g. Panneau dans une fenêtre Windows à part (clic droit sur son onglet), à poser sur un autre
  écran ; la fermer le remet à sa place ; sa place est retenue. Les raccourcis clavier restent ceux
  de la fenêtre principale.

## Phase 6 — Une interface de storyboard ✅

Demandée par Anthony (capture annotée de Pelure) : barre d'outils
verticale, dessiner directement dans la vignette, replier les textes des cartes, calques dans le
montage, calque de fond étendu à toute la scène, et la disposition générale.

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

Demandée par Anthony (capture d'une page de vignettes) : des plans qui se suivent sur la même ligne, un nombre de vignettes réglable
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
- ✅ 9e. Plateau en 3D (bouton 3D ; **repris et remplacé par l'Espace 3D** le 2026-10-20, voir plus bas) : plans des calques à leur profondeur, texturés par leur
  dessin ; caméra en pyramide de vision (et sa course si elle bouge) ; sol quadrillé. Outils :
  tourner autour, glisser la vue, avancer ; Déplacer un calque dans son plan (Maj : en
  profondeur) ou la caméra par son œil (Maj : avancée). Vues de face, de dessus, de côté, par la
  caméra. Même géométrie que le multiplan (testée).
  Pas encore : modes de fusion dans la vue 3D (tout y est en Normal), repère 3D (glTF) dans la
  vue 3D, échelle et rotation d'un calque dans la vue 3D (outil Transformer en 2D).
  Placer des objets 3D (formes, mannequins) et en faire un calque de fond : voir plus bas « Scène 3D de
  blocage » (qui fut une vue à part ; l'Espace 3D les a réunies).
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
  (2026-10-16). **Retiré le 2026-10-21 : fondu dans le calque de couleur** (« On fusionne », voir la fin de ce fichier) ;
  le texte qui suit est l'historique.
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

## Phase 14 — Échange avec un logiciel de storyboard tiers : abandonnée (2026-10-30)

Décision d'Anthony (2026-10-30) : SHOT n'ouvre plus de fichiers d'un autre logiciel de storyboard, n'en écrit plus et
n'importe plus ses fichiers de pinceaux (« si je veux commercialiser SHOT, ça va me poser des problèmes ; ça va alléger
le soft »). Le code, les menus, les préférences, les essais et les documents correspondants sont supprimés ; l'historique git garde les anciens états. Les projets SHOT déjà importés s'ouvrent comme avant et les
pinceaux déjà gardés restent utilisables. Voir `docs/INTERFACE.md`, partie 71.
## Propagation façon EbSynth (menée dans une session parallèle)

- ✅ Étapes a à d : la caisse `synthese/` (synthèse par l'exemple, flux optique, mélange des clés,
  carte graphique).
- ✅ Étape e (fusionnée le 2026-10-16, cdb93d2) : « Propager la peinture sur la vidéo… », au clic
  droit sur la ligne d'un calque peint au-dessus d'un calque vidéo (commande `calque.propager`).
  Tout est décrit dans `docs/PROPAGATION.md`.

## Coloriage par zones (2026-10-19, branche `coloriage-zones`)

Propager des aplats de couleur le long d'une animation en retrouvant les mêmes zones fermées ; distinct
de la propagation EbSynth (rien de généré, de vrais aplats modifiables sous le trait intact).

- ✅ `zones.rs` : découpe en zones fermées (fermeture des trous comme l'aplat sous le trait, petites
  zones gardées, bord de l'image fermant, zone sous le trait), voisines, invariants de Hu, cache par
  version du dessin.
- ✅ `coloriage.rs` : couleur dominante des zones d'une clé, appariement (hongrois, coûts de position,
  aire, recouvrement, forme, voisines ; coupures et réunions), propagation depuis la clé d'avant et celle
  d'après, confiance, zones à vérifier, aplats vectoriels (formes pleines) et bitmap.
- ✅ `coloriageui.rs` : « Colorier par zones… » (Animation, clic droit des calques, Ctrl+K), fenêtre,
  tâche en arrière-plan avec « Arrêter », une étape d'annulation, liste des zones à vérifier.
- ✅ Accroche du « conseiller de couleur » (`ConseillerCouleur`, sans avis aujourd'hui) pour un modèle
  génératif, plus tard, comme simple indice.
- ⬜ Aperçu avant de poser (l'annulation en tient lieu), mouvement estimé par flux optique pour les très
  grands déplacements, transformation de calque différente entre trait et couleur, zones qui fuient par un
  trou plus large que la fermeture : voir « Limites » dans `docs/COLORIAGE.md`.

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
        indique où elle va venir se déposer. Un repère indique avant ou
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
        passe de F5 à F6. Puis (Anthony : « quand une clé est
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
        PgSuiv, qui passent à « plan précédent / suivant »). En
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
        Sélection par couleur (`outil.selection_couleur`, sans touche) : bitmap, zone de la couleur cliquée (pixels
        voisins ou toute l'image, tolérance) ; vectoriel, tous les éléments de cette couleur ;
        Pour déplacer à la souris la zone choisie :
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
        croisent ») : `outil.cutter`, Alt+T, case
        après la gomme. Glisser en travers des bouts de traits (ou un clic) : chaque trait touché
        est coupé à ses croisements avec les autres traits et avec lui-même, les morceaux touchés
        partent ; un trait qui ne croise rien part en entier (mon choix) ; zones et textes ne
        bougent pas. Vérifié en essai (dépassement d'un T coupé net) ;
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
        l'en-tête de la timeline. Projet enregistré en dossier : gardé (choix d'Anthony) ;
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
      - **ouverture en plein écran** (Anthony, 2026-10-18) : fenêtre maximisée à l'ouverture
        (F11 / `vue.plein_ecran` reste le vrai plein écran). Sous Windows, la
        fenêtre créée cachée était marquée maximisée sans en prendre la taille : on la remet à sa
        taille puis on la maximise, quelques images après l'ouverture ;
      - **Transformer : étirement** (Anthony : « beaucoup plus de transformations comme l'étirement latéral ») :
        poignées au milieu des côtés (étirer dans un seul sens, le côté opposé reste en place),
        coins en échelle libre (Maj : proportions), largeur et hauteur en chiffres.
        `LayerTransform::stretch_x` / `stretch_y`, format de projet 22. Reste : les poignées de Bézier des coins avec Ctrl ;
      - **retourner** (Anthony : « créer la possibilité de flipper un plan, flipper un calque et
        flipper une sélection ») : la sélection l'était déjà (Alt+H / Alt+V, maintenant aussi dans
        Édition) ; le calque, sur place, par un étirement négatif (`calque.miroir_h` / `_v`, menu
        Calques) ; les plans choisis de gauche à droite autour du milieu du cadre, calques et
        caméra (`plan.retourner`, sans touche ;
        menus Storyboard et « … »). Mes choix : le calque autour du milieu de son contenu ; les
        calques de toute la scène pas retournés (partagés avec les autres plans) ;
      - **passe sur les raccourcis** (Anthony : « regarde s'il existe des raccourcis prévus pour des
        fonctions qui n'existent pas dans Pelure ») : 22 actions sans fonction, dont 19 avec une
        touche (presque toutes gardées en réserve). Puis (« retire ce qui
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
        la tourne (ses poignées), l'enveloppe (Ctrl + coin, **remplacé le 2026-10-18 par l'outil
        Déformer** : voir « Boîte de sélection ») ; les flèches la poussent (`Tool::keeps_zone`) ;
        8, panneaux qui s'ajustent (Anthony : « que les panneaux s'adaptent d'eux-mêmes en fonction
        de ce qu'ils affichent ») : quand deux panneaux sont l'un sur l'autre, les réglages de
        l'outil, les guides, la fiche, les effets et les icônes des outils prennent la hauteur de
        leur contenu (mesurée à chaque image) ; la couleur, les pinceaux, les calques gagnent le
        reste ; quand les deux débordent, chacun selon son contenu (la couleur est mesurée aussi).
        Le bord glisse en quelques images. Un bord tiré à la main est retenu dans `reglages.toml`
        (`panneaux_tires_a_la_main`) et n'est plus touché, jusqu'à Affichage > Panneaux >
        Réajuster les panneaux (`vue.reajuster_panneaux`, sans touche) ; case « Ajuster les
        panneaux à leur contenu » (`panneaux_ajustes`). `ajuste.rs` ; 7, panneau Historique
        (Édition > Historique…, `vue.historique`, sans touche ; fermé par défaut, en onglet avec
        les calques) : chaque étape d'annulation a un nom, celui de la commande lancée, sinon de
        l'outil (dessin) ou du panneau où elle a été faite (`History::context`, posé par
        `run_action` et `pane_ui` : aucun des endroits qui enregistrent n'a changé), sinon la
        sorte de modification ; clic sur une étape : annuler ou rétablir jusque-là ; les étapes
        annulées restent en gris dessous. `historyui.rs` ;
      - **aplat sous le trait** (appelé « fond sous le trait » jusqu'au 2026-10-21 ; Anthony : « une tâche fastidieuse du storyboard consiste à remplir
        les personnages d'un fond blanc » ; ses choix : « Dans le même calque », « Option du
        pinceau », puis depuis le calque, avec sa sous-ligne : voir plus bas) : option du pinceau et du crayon `pinceau.fond` (le calque courant prend un
        aplat ; aujourd'hui : bouton du calque) ; la forme se déduit du trait de chaque dessin à chaque changement (zones fermées,
        trous fermés jusqu'à 2 × `trous`, débord, bord adouci, bord de l'image qui ferme la
        forme ; `fond::silhouette`, distances exactes, limitée au rectangle du trait : 36 ms pour
        un grand personnage en 1920 × 1080) ; strate Fond par dessin (`Cel::fond`, bitmap) où
        peignent pinceau, crayon et gomme quand « Peindre dans : Fond » (`pinceau.strate_fond`,
        aussi sur la ligne du calque), gardée dans la forme par un pochoir ; affichée sous le trait
        à l'écran, à l'export, dans les vignettes (`render::fond_item`, `FondPart`) ; annulation
        `Edit::FondTiles` ; format 23 (`fond` du calque, `dessins/<id>_fond.png`). Sans touche.
        Puis (Anthony : « gomme qui coupe le fond ») : la strate devient `Strate { peinture, coupe }`,
        la gomme sur l'aplat peint la coupe (le fond disparaît là), le pinceau l'efface (il le
        recoud) ; un second trait suit le même geste sur la coupe ; le fond affiché est refait tuile
        par tuile pendant le geste (`fx::fond_patch`, forme gardée dans `COVERS`) au lieu d'un
        élément « peinture » à part ; coupe dans `dessins/<id>_coupe.png`. Puis (ses suggestions 2
        et 3 retenues) : `fond.calques_choisis`, `fond.tout_le_plan` (sauf les calques de toute la
        scène, le décor), `fond.retirer_choisis`, `fond.couleur_partout` (menu Calques > Fond sous le
        trait, sans touche) ; couleur des fonds neufs par projet (`Properties::fond`, manifeste
        `proprietes.fond`, Réglages du projet). Puis (« le pot de peinture dans le fond, pareil
        pour le cutter dans le fond qui coupe des zones fermées ») : sur l'aplat, un clic du pot
        remplit la zone fermée par le trait (`fond::zone` : trous fermés comme le fond, bande
        regagnée, `UNDER` pixels sous le trait, une zone qui fuit rejoint le dehors) de la couleur
        courante et y recoud le fond ; un clic du cutter retire la zone du fond (sur un calque
        bitmap aussi) ; une étape d'annulation ; aide des touches adaptée. ;
      - **couleur de repère des plans** (Anthony : « changer la couleur des shots dans la timeline,
        pour les utiliser comme repère », par le clic droit seulement) : `Panel::color` (rang dans
        `marks::COLORS`), clic droit sur un plan du montage ou une vignette > « Couleur du plan »
        (pastilles + « Aucune »), sur tous les plans choisis, une étape d'annulation
        (`App::set_plan_color`, `Document::set_panels_color`) ; bloc teinté + bande pleine dans le
        montage, liseré dans la vignette ; copiée avec le plan (duplication, copier-coller, nouveau
        projet des plans choisis, coupe) ; format 24 (`couleur`). Sans touche ni action ;
      - **écran d'accueil** (sa suggestion 1 retenue) : sans projet ouvert (lancement, « Fermer le
        projet »), une carte au milieu : nouveau projet, ouvrir, projets
        récents (huit), rouvrir le dernier au lancement, « Commencer sans projet ». Elle part dès
        qu'un projet s'ouvre ou se crée, ou au premier trait (`accueil.rs`, `App::welcome`) ;
      - **fenêtre « Nouveau projet »** (Anthony, d'après l'organisation d'une fenêtre « Nouveau »
        d'un logiciel de dessin, en version animation), ouverte par l'accueil et par Fichier >
        Nouveau (`fichier.nouveau`) : nom et dossier (le projet est enregistré tout de suite dans
        `dossier\nom\nom.pelure`, sans nom : « Enregistrer » demandera), préréglages groupés (Vidéo :
        HD 1080, HD 720, UHD 4K, 2K DCI, Scope 2,39, SD PAL 4:3 en pixels carrés ; Réseaux : 9:16,
        1:1, 4:5 ; Papier : A4, A3, A5, US Letter en paysage, calculés à la résolution), chacun avec
        sa cadence (24 i/s, 25 pour PAL), « Personnalisé » dès qu'une valeur est changée à la main,
        enregistrer / supprimer ses propres préréglages ; unités px, mm, cm, in (le projet garde des
        pixels), orientation, proportions reconnues (16:9, 9:16, 2,39:1…), résolution (dpi),
        cadence, couleur du papier (ou transparent), couleur des fonds sous le trait, durée du
        premier plan, aperçu ; Entrée = OK, Échap = Annuler. Dernières valeurs, dernier dossier et
        préréglages dans `reglages.toml` (`[nouveau_projet]`), la résolution jamais dans le projet :
        format du projet inchangé. Un papier trop grand (A3 à 300 dpi) baisse la résolution pour
        tenir dans 4096 px. Pas de 23,976 (la cadence est entière) (`nouveau.rs`, `nouveauui.rs`) ;
      - **menus courts** (Anthony : « certains menus contiennent trop de choses et noient les
        informations principales » ; « suis tes recommandations ») : barre de menus de 135 lignes
        de premier niveau à environ 75 (Fichier 25 → 12, Édition 13 → 10, Storyboard 22 → 9,
        Calques 28 → 12, Affichage 25 → 9) ; Fichier > Exporter… et Préférences… (`prefsui.rs`,
        actions `fichier.exporter` et `fichier.preferences`, sans touche) ; sous-menus Importer,
        Plans, Aller à, Caméra, Plateau, Guides, Ordre, Presse-papiers, Plus ; le sous-menu
        Couleur de l'Édition retiré (tout est dans le panneau Couleur). Clic droit : dessin 16 → 8,
        calque au montage 28 → 13 (Options du calque, Animation, Effets en sous-menus), calque du
        panneau 22 → 8, en-tête de timeline 14 → 10, case de timeline 21 → 12 (Insérer ou retirer,
        Plus). Rien n'est perdu : sous-menus, boutons, Ctrl+K ;
      - **barre des panneaux** (Anthony, d'après Photoshop : une fine bande d'icônes sur le côté, une
        par panneau de côté ; on ne prend que l'organisation) : un `SidePanel::right` de 30 points,
        posé avant la zone des panneaux (elle n'est pas un panneau de l'espace de travail), neuf
        icônes de `Pane::icon` en trois groupes (réglages, pinceaux, couleur, guides ; calques,
        effets, fiche, feuille d'exposition ; historique). Règle du clic (`regle_clic`, pure) :
        fermé, il s'ouvre devant ses onglets ; ouvert derrière un autre onglet, il passe devant ;
        ouvert et visible, il se ferme. Icône enfoncée quand le panneau est dans l'espace de
        travail. Chaque icône est une action (`vue.reglages`, `vue.pinceaux`, `vue.couleur`,
        `vue.guides`, `vue.calques`, `vue.fiche`, plus `fx.panneau`, `vue.feuille_exposition` et
        `vue.historique` qui existaient : leurs touches suivent maintenant la même règle), donc
        bulle de 1 s, clic droit « Assigner un raccourci… » et palette. Affichage > Panneaux >
        « Barre des panneaux » (`vue.barre_panneaux`, `reglages.toml` : `barre_panneaux`, montrée
        d'office). Rien par image au-delà du dessin des icônes ; la place d'un panneau n'est lue
        qu'au clic. Le menu Panneaux et les panneaux qui s'ajustent n'ont pas changé. `barrepanneaux.rs` ;
      - **feuille d'exposition en animation seulement** (Anthony : « Exposition n'est disponible que
        dans la partie animation ») : `Pane::animation_only`. En storyboard : son icône de la barre
        et son entrée d'Affichage > Panneaux sont masquées, `vue.feuille_exposition` répond d'un mot
        dans la barre d'état (`refuse_hors_animation`), et une disposition qui l'ouvre (démarrage,
        espace de travail) la garde de côté (`App::exposition_masquee`, `set_dock_layout`). Le
        retour en animation (`masquer_exposition`, appelée par `set_mode`) rend la disposition
        d'avant à l'identique si rien n'a bougé entre-temps, sinon rouvre la feuille à sa place
        d'ordinaire (`DockLayout::retour_animation`, pure). La disposition retenue à la fermeture la
        garde (`current_layout`). Une seule disposition pour les deux modes, pas de version de
        disposition de plus ;
      - **nouvelle clé = choix** (Anthony : la nouvelle clé doit être sélectionnée ; avant, le cadre
        blanc restait autour du bloc entier) : toute commande qui ajoute une clé ou une image
        (F4 / F5, double-clic et menu de la piste, Nouveau dessin, Dupliquer le dessin, Insérer une
        image) donne le choix à la nouvelle seule (`choice_for_new_key`, `select_new_key`) : dans le
        montage, la clé est cerclée de blanc et le cadre du calque courant s'efface (il garde un
        filet pâle) ; dans la timeline, ses images sont cadrées de blanc. Suppr et Ctrl+C la
        visent. `montagekeys.rs` ;
      - **clés d'effets dans le montage et la timeline** (Anthony : les voir, les déplacer, les
        copier-coller et les supprimer). Cause : elles ne se dessinaient que dans la timeline de
        l'animation, en petits losanges (7 points) collés au bord des dessins, une seule se prenait
        à la fois, et le montage, où il travaille, n'en montrait aucune ; la case du panneau Effets
        était cochée d'office, ce n'était pas elle. Maintenant (`fxkeys.rs`) : un losange violet
        par image clée en haut de la piste, dans le montage comme dans la timeline (point sombre
        s'il porte les clés de plusieurs effets) ; clic = choix (Ctrl ajoute ou retire, Maj prend
        les images d'entre), glisser = tout le choix glisse du même nombre d'images (une clé qui
        tombe sur une autre du même effet la remplace, rien avant l'image 0 ni après la fin du
        plan), Échap abandonne ; Ctrl+C / Ctrl+V (collées à l'image courante du calque courant, sur
        l'effet de même sorte et de même rang, les clés sans effet correspondant sont laissées de
        côté et dites) / Suppr (rien sans choix), clic droit ; actions `fx.cles_copier`,
        `fx.cles_coller`, `fx.cles_supprimer`. « Créer une clé » choisit la clé créée. Le choix
        est lâché par un clic ailleurs, une annulation, un changement d'ordre de la pile. Étapes
        d'annulation nommées « Déplacer / Coller / Supprimer des clés d'effet ». Logique en
        fonctions pures testées (`move_keys`, `paste_keys`, `clamp_delta`…) ;
      - **caméra : clés choisies, effets, focales, aides au cadrage** (Anthony, 2026-10-19). Branche
        `camera-presets`. (1) Clés de caméra (`camkeys.rs`, comme `fxkeys.rs`) : clic = choix (Ctrl,
        Maj, d'un plan à l'autre), glisser = tout le choix (une clé ne passe pas par-dessus une clé
        non choisie : c'était la règle de la clé seule, `camera::move_key` est remplacée par
        `clamp_delta`), Ctrl+C / Ctrl+V à la tête de lecture avec l'écart entre les clés (collage
        d'un plan à l'autre dans le montage, dans le plan courant en animation ; un plan sans clé
        qui en reçoit après son début garde sa caméra de continuité par une clé au début, comme
        « Clé ici »), Suppr (rien sans choix), clic droit ; étapes nommées « Déplacer / Coller /
        Supprimer des clés de caméra » ; actions `camera.cles_copier`, `camera.cles_coller`,
        `camera.cles_supprimer`, `camera.cles_tout_choisir`. (2) Effets (`camfx.rs`, `camfxui.rs`) :
        clés ordinaires ajoutées à la caméra de chaque image (décalages en part de la largeur du
        cadre filmé, bruit de sinus à phases tirées de la graine, secousses amorties) ; caméra à
        l'épaule : dérive 0,9 Hz de ±1,2 % × ±0,9 % et ±0,7°, secousses de ±2,5 % et ±1,2° toutes les
        0,9 à 2,4 s, une clé sur deux ; tremblement léger : ±0,4 % et ±0,15° à 7 Hz, une clé par
        image ; explosion : ±4,5 % et ±2,5° à 12 Hz, décroissance exponentielle (τ = durée / 4),
        coup de zoom +6 %, une clé par image ; secousse : ±3 % et ±1,5° à 10 Hz (τ = durée / 3) ;
        zoom avant / arrière : deux clés, adoucies, facteur 1,5 par défaut ; zoom choc : deux clés,
        départ brutal, facteur 3 ; vertigo : la distance suit la focale, 25 clés pour que la taille
        du sujet au plan de référence reste à 0,1 % près (la distance se mêle en ligne droite, le
        zoom en proportion). Plafond de 1000 clés par effet (au-delà, les clés s'espacent). Action
        `camera.effet_appliquer`. Les valeurs sont des choix de SHOT, à régler à l'œil ; rien de
        TVPaint n'est copié. (3) Focales
        (`camera::FOCAL_PRESETS`) : 14, 24, 35, 50, 85, 135, 200 mm et anamorphique (40 mm + cadre
        2,39:1 d'affichage, sans déformer le dessin) ; angle de champ horizontal, capteur de 36 mm.
        (4) Aides au cadrage (`cadrage.rs`, `cadrageui.rs`) : tiers, grille d'or (lignes à
        1/2,618 et 1,618/2,618, points aux intersections), spirale d'or étirée aux proportions du
        cadre (8 orientations), diagonales et réciproques, croix, zones de sécurité (action 93 % /
        titres 90 %, ou 90 % / 80 % à vérifier), cadre de format ; actions `cadrage.tiers`,
        `cadrage.grille_or`, `cadrage.spirale`, `cadrage.spirale_orientation`, `cadrage.diagonales`,
        `cadrage.croix`, `cadrage.aucune`, `cadrage.format_suivant`. Réglages dans `reglages.toml`
        (`[aides_cadrage]`, valeurs par défaut serde, sans changer la version du projet) ; le cadre
        de format et les zones de sécurité restent dans la disposition. Rien ne passe dans les
        exports ;
      - **clés : décaler, déplacer, insérer, coller sur la tête** (Anthony, 2026-10-19 : « quand je
        tire une image clé, les autres clés suivent ; avec Maj, seule l'image clé se déplace ;
        Ctrl+V colle après la clé de la tête de lecture ; Maj+Ctrl+V sur la tête »). Branche
        `cles-regles` (sur `retouches`). Mêmes règles pour les clés de caméra (`camkeys.rs`), les
        clés d'effets (`fxkeys.rs`) et les clés de dessins (`handles.rs`, `montagekeys.rs`).
        (1) Glisser : par défaut la clé prise (la première des choisies) et toutes les clés
        d'après, dans le même plan (caméra), le même effet (effets) ou la même piste (dessins),
        glissent du même nombre d'images (« Décaler les clés… ») ; celles d'avant ne bougent pas, la
        première s'arrête avant la clé d'avant (`clamp_ripple`, `ripple_frames`, `ripple_keys`,
        `ripple_at`). Maj tenue, même en cours de geste : seules les clés choisies bougent
        (« Déplacer une clé… »), sans passer par-dessus une clé non choisie pour la caméra
        (`clamp_delta`) et les dessins ; pour les effets, comme avant, une clé qui tombe sur une
        autre la remplace. Maj + clic sans mouvement reste le choix d'une plage (le seuil de
        glissement d'egui, environ 6 points, sépare le clic du glisser). (2) Ctrl+V insère
        (`paste_insert`, `paste_insert_keys`, `insert_point` + `open_gap`) : la première clé collée
        va à l'image qui suit la clé sur laquelle est la tête (sinon la dernière clé d'avant la
        tête ; sans clé avant, sur la tête) ; les écarts du bloc sont gardés et tout ce qui suit est
        repoussé de la longueur du bloc (écart de la dernière clé collée + 1 ; pour les dessins,
        écart + durée), donc rien n'est écrasé. Caméra : chaque plan touché fait la place de ce qu'il
        reçoit (le collage passe d'un plan à l'autre dans le montage). Dessins : l'ancre est le
        calque courant, même point d'insertion pour tous les calques collés. (3) Maj+Ctrl+V,
        action `edition.coller_a_la_tete` : la première clé exactement sur la tête, sans rien
        repousser, remplace ce qu'elle rencontre (c'est l'ancien Ctrl+V) ; hors clés, il fait ce
        que faisait Ctrl+Maj+V avant. Conflit : Ctrl+Maj+V était `edition.coller_sur_place`
        (alias de Ctrl+V dans SHOT) : la touche passe à
        `edition.coller_a_la_tete`, « Coller sur place » reste une commande sans touche par défaut
        (la ligne du `keymap.toml` garde sa source). (4) Plan : jamais allongé. Glisser des dessins :
        un calque tenu jusqu'à la fin du plan le reste (sa dernière exposition absorbe, avec une image
        au moins), sinon le contenu s'arrête à la fin du plan ; insérer : la queue de la dernière
        exposition est coupée, et rien n'est collé (message) si le début d'une clé repoussée sortirait
        du plan. Caméra et effets : même refus. La limite entre deux dessins (montage) et le bord
        droit d'un dessin (timeline) suivent la règle (par défaut les clés d'après suivent, Maj : cette
        limite seule) ; Alt tenue sur un dessin du montage rend l'ancien geste (la clé prend sa place
        parmi les autres, `shift_runs`, « Réordonner les clés »). Verrou du storyboard : refus comme
        avant (`content_refuses`). Étapes d'annulation nommées « Décaler les clés », « Déplacer une
        clé », « Insérer des clés », « Coller les clés à la tête de lecture » (« … de caméra », « …
        d'effet »). Les règles (cascade par défaut, Maj = seule, insertion après la clé de la tête)
        sont des choix d'Anthony ; leurs équivalents dans TVPaint sont à vérifier.
        Fonctions pures testées (`camkeys_tests.rs`, `fxkeys_tests.rs`, `handles_tests.rs`,
        `keymap_tests.rs`) ;
      - **clés d'effets : seulement celles de l'effet choisi** (Anthony : « elles n'apparaissent
        que lorsque l'effet est sélectionné »). L'effet choisi (`App::fx_choice` : plan, calque,
        rang) est celui du panneau Effets : un clic sur son nom le choisit (cadre de la couleur de
        sélection, nom sur toute la largeur), un second le lâche ; « Créer une clé », un curseur,
        « Ajouter un effet » ou cocher un effet du calque le choisissent aussi ; action
        `fx.choisir_aucun`. Seules ses clés se dessinent et se prennent, sur son calque, dans le
        montage et la timeline ; aucun effet choisi, aucun losange ; la case du panneau reste
        l'interrupteur général. Plus de point sombre (un losange = une clé). Copier, coller (sur
        l'effet choisi, de la même sorte que le copié, sinon dit et rien collé), supprimer
        agissent sur ces clés ; changer d'effet choisi, ouvrir un projet ou vider la pile lâche
        le choix des clés ; le rang suit un échange ou un retrait de la pile (`rank_after_swap`,
        `rank_after_remove`, `shown`, testées) ;
      - **enveloppe de Bézier** (**depuis le 2026-10-18, c'est un mode de l'outil Déformer, plus un
        Ctrl + coin** : voir « Boîte de sélection ») (Anthony : « maintenir Ctrl
        permet de saisir une poignée de Bézier
        des coins pour transformer la forme » ; son choix : « B, comme l'outil Perspective ») : dans
        l'outil Transformer, Ctrl + glisser un coin du cadre ouvre une enveloppe sur le dessin
        courant (4 coins, 2 poignées par coin ; surface de Coons), appliquée à chaque lâcher depuis
        le dessin d'origine (une étape d'annulation par geste), bitmap ou vectoriel (traits rendus
        plus denses pour se courber). Avec cet outil, Ctrl ne prend plus le lasso. Puis (« l'enveloppe doit aussi pouvoir ne déformer que
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
        `remplir_la_forme`) ; ;
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
- ✅ Sensation des brosses (étude de Fable du 2026-10-18, `docs/SENSATION-BROSSES.md` ; Anthony :
  « règle le point 10 en suivant ses recommandations ») :
  - **Toucher du stylet**, commun à tous les pinceaux (section repliée au bas des réglages du
    pinceau, du crayon et de la gomme ; gardé dans `reglages.toml`) : courbe de pression globale
    (Droite, Douce, Ferme ou à la main), plancher de
    taille 10 % (avant 3 %), seuil d'appui 3 % sous lequel le stylet ne pose rien (ni début
    fantôme ni pointe au levé), traits plus fins qu'un pixel gardés pleins (avant : éclaircis,
    gris), bord des empreintes en une seule chute douce d'au moins un pixel (le « pixel feather »
    de libmypaint ; une pointe dure fait maintenant sa taille vraie, un demi-pixel de plus
    qu'avant). « Ancien toucher » remet tout comme avant, pour comparer.
  - **Pinceau rond** : taille mini 18 % (tes outils TVPaint ont 10 %), au lieu de 0.
  - **Crayon** anticrénelé (tes outils TVPaint ont `aliasing=0`), 2,5 px, opacité selon l'appui
    (mini 30 %), lissage léger ; « Pixels nets » rend l'ancien crayon.
  - **Horodatage par événement** : les échantillons du stylet d'une image d'écran sont étalés
    entre l'image d'avant et celle-ci (egui les datait tous de la même heure) : la vitesse des
    connexions n'est plus en escalier.
  - **Lissage** (options par pinceau, sous le type) : Suivi selon le temps (constante de temps,
    cadence de référence 200 Hz — à vérifier à la tablette), corde et distance en pixels d'écran
    (le « Scalable Distance » de Krita), rattrapage de la pointe au levé en courbe, droit (comme
    avant) ou aucun, et **lissage après le trait** comme TVPaint avec Real Time décoché (ton
    réglage) : brut pendant le geste, rejoué lissé au levé. Ce dernier reste décoché par défaut.
  - **Latence** : `thread::sleep` mesuré sur ce poste (test ignoré `frame_pacing_sleep_is_precise`) :
    0,25 ms de dépassement moyen sur 2 ms, 0,8 au pire ; le rythme des images est bon, rien à
    changer.
  - Pas fait : la couverture par aire sous 2 px (le bord doux suffit pour l'instant),
    l'inclinaison et l'heure vraie des échantillons (lecture directe de WM_POINTER, avec Anthony),
    les traits vectoriels rejoués avec l'opacité selon l'appui (calque par défaut : bitmap) ; le
    trait vectoriel garde l'ancien bord et peut paraître, au lâcher, un demi-pixel plus fin que
    l'aperçu bitmap.
  - Tout est à essayer à la tablette : la liste des A/B est à la fin de
    `docs/SENSATION-BROSSES.md`. Le banc de traits (étape 0) n'a encore aucun trait d'Anthony.
- ✅ Audit du rendu des brosses (Fable, 2026-10-18, `docs/AUDIT-RENDU-BROSSES.md` ; Anthony :
  « les brosses ne simulent toujours pas cet effet de dessin ou peinture traditionnelle », pareil
  en bitmap) : un essai ignoré (`src/rendu_audit_tests.rs`) rend des planches PNG dans
  `audit-rendu/` (réglages livrés, essais, outil TVPaint d'Anthony et panneaux TVPaint 12 importés,
  Krita / MyPaint, papiers, pointes, bitmap contre vectoriel). Diagnostic : rubans lisses (empreinte
  ronde dense à opacité pleine, rien qui s'accumule), papiers livrés qui sont du bruit par pixel,
  grain à seuil binaire, aucune pointe en image livrée, grain propre du crayon TVPaint absent ; le
  chantier « sensation » ne touchait pas la matière. Le crayon 2B de TVPaint importé prouve que le
  moteur sait faire. Plan : pointes et papiers vrais, débit qui s'accumule, grain doux, grain du
  crayon, mode Multiplier, puis peinture. Corrigé en passant : import TVPaint (`[ToolBox]` des
  exports, `step` des outils de base en % — à vérifier —, grain du crayon noté), papiers importés
  répartis par rang (`GrayImage::equalized`).
- ✅ Rendu des brosses, étapes 1 à 3 (Fable, 2026-10-18, branche `rendu` ; Anthony : « Fusionne et
  commence les étapes 1 à 3 » ; `docs/AUDIT-RENDU-BROSSES.md` §6 bis) :
  - **pointes et papiers fabriqués par Pelure** au démarrage (Graphite, Fusain, Craie, Poils
    plats, Poils ronds ; Papier à dessin, Toile, Vergé), rien de copié d'un autre logiciel ;
  - **brosses traditionnelles** après le Pinceau rond (inchangé, toujours le départ de l'outil
    Pinceau) : Crayon graphite (le départ de l'outil Crayon), Crayon de couleur, Encre, Feutre,
    Fusain, Craie, Gouache ; l'ancien crayon reste « Crayon net » ; « Mes brosses » intact ;
  - **douceur du grain** (0,12 pour les anciens réglages, 0,3 pour les nouvelles brosses), débit
    qui s'accumule sous un plafond sur les outils secs (le débit tient compte du dépôt moyen de la
    pointe), appui → débit ; imports MyPaint / Krita en dépôt accumulé ;
  - **grain du crayon** (Soie, Doux, Moyen, Dur), collé à la feuille même sans papier ; le
    `grain` du `tv_pencil` importé y passe (ordre deviné, à vérifier) ; option « le grain suit le
    trait » ;
  - en vectoriel, l'appui d'un point est retrouvé pour toute connexion de taille
    (`vector::pressure_for_radius`).
  - Choix par défaut à confirmer à la tablette : grain collé à la feuille, crayon plafonné à 90 %,
    tailles pour 1920×1080 vu à 50 %. Planches avant / après : `audit-rendu/70_…` à `75_…`.
- ✅ Retour d'Anthony sur les brosses et étape 5 du rendu (Fable, 2026-10-18, branche `vecto` ;
  `docs/AUDIT-RENDU-BROSSES.md` §6 ter, `docs/VECTORIEL-COMME-BITMAP.md` §3 bis) :
  - le graphite et le crayon de couleur **mordent dès l'effleurement** (courbe appui → débit) et
    ont un grain **« sable »**, franc (`GRAINS_SAND`), papier à dessin plus fin ; planches `76_…` ;
  - **le trait vectoriel rend trait pour trait comme le bitmap** : il garde sa trace (appui et
    instant par point, toucher, couleur B, `vector::Trace`) et son réglage complet, et rejoue le
    moteur bitmap ; écart 0,0 sur toutes les brosses et le Trait TVPaint importé ; format 24 ;
  - **cache de pixels par trait** (`StrokeCache`, ≈ 100 Ko par trait de crayon, plafond 8 Mo par
    trait) : déplacer une sélection vectorielle pose les pixels sans rejeu — 50 traits de
    graphite : 103,9 → 3,9 ms par image ; rendu exact refait une fois au lâcher.
  - Reste : la peinture humide ne se mêle pas d'un trait vectoriel à l'autre ; rotation / échelle
    rééchantillonnées pendant le geste ; morceaux coupés repartent d'un état neuf.
- ✅ Revue de l'étape 5 et règle « sans latence » (Fable, 2026-10-18, branche `vecto` ;
  `docs/VECTORIEL-COMME-BITMAP.md` §3 ter, `revue_tests.rs`) : les 23 constats de la revue
  corrigés — gomme / cutter / enveloppe / intervalles gardent les appuis des morceaux et ne touchent
  plus aux traits voisins ; toute transformation hors aperçu vide le cache ; cache en tuiles creuses
  (plafond 1024 tuiles, budget global 512 Mo, partagé par l'historique) ; glissé arrondi au pixel ;
  boîte des pointes tournées ; fichier quantifié ; papier des crayons à part (`PAPER_GRAPHITE`) ;
  doc du format 24 corrigée. Puis, Anthony (« un vrai souci de ralentissement avec les brosses
  vecto quand elles sont enveloppées et qu'on veut les déplacer ; ma priorité : sans latence ») :
  **jamais de rejeu du moteur sur le fil de l'interface pendant un geste** — enveloppe et
  perspective déforment les pixels en cache par maillage (`envelope::warp_raster_fast`), le rendu
  exact arrive d'un fil de travail (`rejeu.rs`, `App::rejeu_cel` / `poll_rejeu`), ouverture d'un
  projet en rendu rapide puis exact (`project::load_quick`). Mesures (51 traits, 1920×1080) :
  enveloppe 8,6 ms / image (rien n'était affiché avant), lâcher 113 → 0,5 ms, glissé après enveloppe
  44,9 → 1,6 ms / image, lâcher 118 → 0,1 ms ; chargement 24 dessins affiché en 0,28 s au lieu de
  1,6 s.
- ✅ Essai d'Anthony, enveloppe et grosse sélection (2026-10-18, nuit, `docs/VECTORIEL-COMME-BITMAP.md`
  §3 ter, `warp.rs`, `contours.rs`) : (1) enveloppe et perspective vectorielles passent par la
  feuille flottante : seul un maillage change par image, dessiné par la carte graphique (`gpu.rs`
  `vs_mesh`, secours processeur dans `render.rs`), feuille avec marge ; 10 traits 9,9 → 0,14 ms par
  image, 40 traits 37 → 0,14 ms (8 Mo de tuiles → 221 Ko de sommets) ; la feuille déformée reste
  jusqu'au rendu exact. (2) Les contours bleus d'une grosse sélection sont gardés d'une image à
  l'autre, simplifiés à l'écran (300 traits de 500 points : 13,5 → 0,6 ms au repos, 1,2 en geste ;
  300 de 2000 points : 54 → 0,6 ms) ; saisie de 300 traits par bandes sur tous les cœurs
  (112 → 25 ms). Essais `warp_tests.rs`, `contours_tests.rs`, `gpu_tests.rs` (carte graphique
  réelle contre le calcul de secours).
- ✅ Essai d'Anthony sur la feuille flottante (2026-10-18, soir, `docs/VECTORIEL-COMME-BITMAP.md`
  §3 ter, `affichage_tests.rs`) : (1) des morceaux du dessin manquaient, coupés en ligne droite,
  quand on déplaçait vers l'intérieur des traits qui dépassaient de la feuille (cache et feuille
  flottante à la taille de la feuille) — les traits gardent maintenant ce qui dépasse (marge de
  1024 px, grain lu comme sur la feuille), la feuille flottante est élargie (saisie 11,0 ms contre
  10,2 ms) ; terminer un geste lâché trop tôt ne laisse plus la sélection en double. (2) La latence
  au dessin d'un trait texturé était la pose au lever du stylet, un rejeu complet sur le fil de
  l'interface (44 ms à 600 points, 174 ms à 2400) : 0,07 ms maintenant, le rendu exact vient du fil
  de travail, qui ne rejoue plus que les traits sans pixels exacts. Les traits manquants du dessin
  regardé sont réchauffés en arrière-plan (saisie sans cache : 0,8 s pour 40 traits larges).
- ✅ Latences sur la branche ecto (Anthony, 2026-10-18 : « sans latence »). **Feuille
  flottante** (lottante.rs) : déplacer, tourner ou agrandir une sélection vectorielle ne refait
  plus rien par image — à la première image, le dessin sans la sélection et la sélection seule
  (deux images, depuis les caches), puis le GPU n'a que la transformation du geste à changer
  (PlanItem::pre) ; au lâcher la géométrie seule (0,1 ms ; la feuille reste affichée jusqu'à l'arrivée du rejeu exact, ou jusqu'à la prochaine action, qui termine à l'ancienne). 40 traits de Crayon
  net sur toute la feuille 1920×1080 : translation 1,0 ms et 6,1 Mo de tuiles par image → 0,001 ms
  et 0 octet, rotation 28,4 ms et 8,3 Mo → 0 (`floating_selection_gesture_is_cheap_per_frame`).
  **Aplat sous le trait en arrière-plan** (ondfil.rs, x::differe) : la silhouette et la
  composition se calculent d'un fil de travail ; à l'écran, le fond d'avant reste jusqu'à
  l'arrivée du nouveau (demande caduque jetée) ; export et enregistrement restent exacts. Rendu du
  plan après le 60e trait : 91 → 6 ms, glissé de 50 traits 71 → 1,6 ms par image
  (`fond_background_keeps_the_ui_thread_free`). **Perte de données** corrigée : les traces en
  flottants de d01fffe se relisent, un dessin illisible n'est plus effacé à l'enregistrement. **Rejeu** (contre-vérification de 55b35a3, 
ejeu_tests.rs) : Ctrl+Z n'annule plus que les dessins qu'il a refaits (N1), un rendu dépassé est redemandé et les demandes interactives passent devant la file du chargement (N2), pas d'adoption pendant un geste sur le dessin ni si sa version a changé (N3), Ctrl+Z du second geste d'une enveloppe ou d'une perspective bitmap revient au premier (N4), l'adoption ne fait plus réécrire tous les dessins au premier enregistrement (N7), un panic du rendu est signalé sans tuer le fil (N9).- ✅ Étude « vectoriel comme bitmap » (Fable, 2026-10-18, `docs/VECTORIEL-COMME-BITMAP.md`) :
  l'existant en images (le trait vectoriel perd au lâcher opacité ↔ appui, connexions, humide,
  accumulation, et garde l'ancien bord), toutes les pistes (rejeu du moteur depuis le chemin avec
  appui, instant, réglage complet et graine ; cache par trait ; GPU ; état de départ des morceaux ;
  transformation ; tween ; format), les autres logiciels, et la recommandation : garder l'aperçu
  au lâcher puis rejouer le moteur bitmap — rien n'est commencé.
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

## Panneau Pinceaux refait (demandé par Anthony le 2026-10-18) ✅

Référence d'organisation : un panneau de brosses de logiciel de retouche (rangement seulement, ni
apparence ni textes). Demande : « une organisation des brosses plus simple, et la taille du pinceau
mise en avant ».

- ✅ Taille de l'outil courant en haut du panneau (grand curseur logarithmique + champ px), même
  valeur que les touches ^ et $ (largeur pour les formes).
- ✅ Onglet « Pinceaux » : recherche par nom (sans accents ni majuscules), liste (pastille +
  taille, aperçu du trait, nom) ou vignettes, dossiers repliables avec leur nombre (Crayons, Encres
  et feutres, Peinture, Craie et fusain, Gommes, Mes brosses, un dossier par fichier importé ; les
  réglages de Pelure rangés d'après leur nom), trois icônes en bas (importer, nouvelle brosse,
  supprimer). Menu du clic droit et enregistrement inchangés. Les lignes hors écran ne font ni
  aperçu ni texte (centaines de brosses importées).
- ✅ Onglet « Réglages du pinceau » : sept catégories (la case allume ou éteint la dispersion, le
  grain et papier, la couleur et mélange, le lissage ; les valeurs sont gardées pour la rallumer),
  aperçu du trait en haut sous les onglets (2026-10-18, demande d'Anthony ; il était en bas),
  liste déroulante si le panneau est étroit. Les réglages sont ceux de l'ancien panneau Réglages de
  l'outil (`dynui.rs` découpé par catégorie, `brush_ui` déplacé), qui n'en garde qu'un résumé : nom,
  taille, opacité (ses deux boutons vers le panneau Pinceaux retirés le 2026-10-18, « inutile »).
- ✅ Le même soir, l'onglet est retiré (Anthony : « les réglages de pinceaux font doublon avec le
  panneau Réglages, autant tout mettre dans le panneau Réglages ») : aperçu et catégories sous le
  résumé du panneau Réglages de l'outil (`brush_settings_ui`) ; le panneau Pinceaux garde la
  taille et la bibliothèque. `Onglet` reste dans `pinceaux.rs` pour relire les réglages écrits.
- ✅ Puis (Anthony : « la taille apparaît dans deux panneaux ; garder dans Réglages, en premier, en
  haut ») : la taille n'est plus que dans le panneau Réglages, en tête, en grand ; peint derrière,
  remplir la forme et l'aplat sous le trait passent en bas des réglages de la brosse
  (`brush_extras_ui`). Pour le pot et le cutter, les options du fond restent en haut.
- ✅ Retenus dans `reglages.toml` (`panneau_pinceaux`, `#[serde(default)]`) : onglet, liste ou
  vignettes, catégorie, dossiers ouverts ou repliés.
- ✅ Logique sans écran dans `pinceaux.rs` (rangement, recherche, état, catégories), testée dans
  `pinceaux_tests.rs` ; affichage dans `pinceauxui.rs`.
- Pas vu à l'écran (pas de capture de l'application dans cette séance) : à essayer par Anthony.
- Pas faits :
  - le crayon n'a pas de réglages nommés propres : choisir un réglage prend le pinceau, comme avant ;
  - pas de dossiers que l'utilisateur crée lui-même, ni de glisser-déposer d'une brosse vers un
    dossier ; pas de menu sur l'en-tête d'un dossier ;
  - la case d'une catégorie ne se rappelle ses valeurs que pendant la séance ;
  - « Supprimer la brosse » ne demande pas de confirmation (comme le menu du clic droit).

## Pelure devient SHOT (2026-10-18) ✅

Le logiciel s'appelle SHOT, « Frame the story » ; son logo (`assets/shot-logo.svg`) est celui
d'Anthony.
- Nom visible : titre de fenêtre (« Sans titre — SHOT »), accueil, boîtes de dialogue, messages,
  bulles, rapports d'erreur, La « pelure d'oignon »
  garde son nom (terme de dessin).
- Accueil : le logo remplace le titre en texte (`logo.rs` le trace en vecteur avec tiny-skia, une
  fois, à la taille de l'écran ; la devise est dans le dessin). Icône de fenêtre : le « S » entre
  les deux angles du logo (`logo::icone`). Icône et version dans l'exécutable : `build.rs`
  compile un fichier de ressources avec `rc.exe` du Windows SDK (sans lui, l'exécutable se
  construit quand même, sans icône dans l'Explorateur).
- Exécutable : `target\release\shot.exe`. La bibliothèque garde le nom `pelure`.
- Données : `%APPDATA%\SHOT` et `%LOCALAPPDATA%\SHOT` (`donnees.rs`). Au premier lancement, le
  contenu de l'ancien dossier `Pelure` y est copié s'il n'y a pas encore de dossier SHOT ; l'ancien
  n'est ni modifié ni supprimé.
- Projets : rien ne change (dossier du projet, fichier `.pelure`, format `pelure-projet`,
  version du format, modèles 3D `.pelure3d`) ; les anciens s'ouvrent comme avant.
- Gardés sous l'ancien nom, sans effet pour l'utilisateur : les variables `PELURE_*`, l'étiquette
  interne des brosses livrées, les dossiers temporaires `pelure-*`.

## Thème gris et couleurs réglables (2026-10-18) ✅

Demande d'Anthony : « changer l'interface vers un gris sombre/clair », couleurs « paramétrables dans les préférences » (seulement l'idée de gris
moyens, rien de copié).
- `theme.rs` : une palette centrale (`Palette`), déduite de neuf couleurs (`Couleurs`), lue à chaque
  image par de petites fonctions (`theme::fond()`, `theme::texte()`…) ; les `Visuals` d'egui ne sont
  posés qu'au changement de thème. Trois thèmes livrés : Gris sombre (défaut), Gris clair, Sombre
  (l'ancien) ; Personnalisé part du thème courant dès qu'une couleur est changée.
- Fichier > Préférences… (`prefsui.rs`) : liste des thèmes, neuf sélecteurs de couleur, Rétablir ;
  tout est appliqué en direct, enregistré dans `reglages.toml` (`[theme]`, vieux fichier = Gris
  sombre) une fois le sélecteur relâché.
- Les couleurs écrites en dur dans l'interface (timeline, montage, feuille d'exposition, calques,
  son, rôles, clés d'effets, messages, avertissements, fond du plan de planches, séparations de
  barres…) lisent le thème. Un thème clair prend des repères plus foncés et des blocs pâles à texte
  sombre ; le contour des éléments choisis passe du blanc au presque noir.
- Logo : variante claire sur un fond de panneau clair (accueil et « À propos de SHOT »).
- Tests (`theme_tests.rs`) : contraste du texte (≥ 7:1 sur les panneaux, ≥ 4,5:1 ailleurs), de
  l'accent (≥ 4,5:1 sur le panneau pour les traits, texte lisible sur l'accent), des repères de la
  timeline (≥ 3:1), réglages relus, Personnalisé.
- Pas fait : les vues 3D (plateau, vue de dessus) gardent leur fond sombre propre ; les poignées des
  outils dessinées sur le dessin (bleu vif, jaune de la caméra) gardent leurs couleurs.
- Aide > À propos de SHOT… (`aide.a_propos`, `aproposui.rs`) : fenêtre modale avec le logo, la
  version de Cargo.toml et « Anciennement Pelure » ; Échap, un clic à côté ou Fermer la ferment.

## Effets de calque : vitesse (demandé par Anthony le 2026-10-18) ✅

Anthony : « trois clés d'animation sur un calque vecto, un effet animé de flou ; par-dessus, un
calque bitmap avec un effet bruit animé et une option de fusion […] ça rame à mort. After Effects
n'aurait pas ce problème pour si peu. » Règle de la maison : jamais de calcul lourd sur le fil de
l'interface pendant qu'on s'en sert.

Scène mesurée : son projet copié (`TEST PELURE_effets`, dessins réels) : feuille 3840 × 2160, cadre
1920 × 1080, 24 images/s. « Dessin » (vecto, 67 et 108 tuiles) : flou gaussien 8 → 50 → 130 → 70 →
120 aux images 1, 7, 22, 31, 36, puis luminosité / contraste. « Calque 2 » (bitmap de 859 tuiles,
opacité 0,6, fusion Densité linéaire +) : bruit 0 → 45 %, puis teinte / saturation. Machine : Core
i7-10700K (8 cœurs, 16 fils), partagée avec d'autres programmes (± 15 % d'une série à l'autre).
Mesures : `cargo test --release --offline --lib fxperf -- --ignored --nocapture --test-threads=1`
(`fxperf_tests.rs`, le projet de l'utilisateur est lu en lecture seule ; absent, des dessins
fabriqués de même ampleur le remplacent).

**Où allait le temps** (160 ms par image sur le fil de l'interface, à chaque image de la lecture,
du défilement, à chaque cran d'un curseur, pour chaque vignette) :
- les effets par pixel (bruit, teinte / saturation, luminosité, niveaux, netteté, ombre, lueur,
  nuages…) tournaient sur un seul cœur : la teinte / saturation seule, 130 ms ; le bruit 36 ms
  (`fxfilters.rs`, `fxwarp.rs::shade`) ;
- le flou transposait l'image en écrivant en travers du cache, deux fois (`fx.rs`, `transpose`),
  et lisait la zone pixel par pixel (`region_of`) ;
- le calcul se faisait sur le fil de l'interface (`gpu.rs`, `sync` → `fx::shown`), et tout le
  dessin dérivé repartait ensuite à la carte graphique ;
- le cache ne gardait que 48 dessins dérivés (`CACHE_MAX`) : 47 images × 2 calques le vidaient à
  chaque tour de lecture (le deuxième tour coûtait autant que le premier) ;
- une vignette de la bande cherchait le dessin dérivé (hachage, verrou, copie des effets) pour
  chacun de ses 83 000 pixels lus (`render::thumbnail` → `composite_pixel`).
Ce n'était pas : le mode de fusion (fait sur la carte graphique à l'écran ; 28 ms sur processeur à
l'export) ni le bruit regénéré (le hasard est une fonction du pixel, sans état), ni le flou (déjà
en trois boîtes, coût constant par pixel, quel que soit le rayon).

**Ce qui a changé** (code d'avant : commit 14233e9) :
1. Calculs sur tous les cœurs (`render::par_chunks`), flou à transposition par bandes de 16
   colonnes, division par multiplication : **le même résultat à l'octet près** (empreinte de
   chacun des 21 effets comparée avec l'ancien code : `fxexact_tests.rs`).
2. Cache en octets (768 Mo) au lieu de 48 entrées, calcul unique même si plusieurs fils le
   demandent (`fxcache.rs`).
3. Calcul de fond : `fx::display` ne calcule rien ; il demande au fil de fond (deux cœurs laissés
   à l'interface), montre le dernier résultat du même dessin (ou le dessin sans effet) et l'écran se
   redessine à l'arrivée. En lecture, les 24 images qui viennent sont demandées d'avance (la plus
   proche d'abord), puis envoyées d'avance à la carte graphique ; les vignettes passent après,
   et attendent la fin d'un geste ou de la lecture.
4. Aperçu réduit (`fxproxy.rs`) en lecture, au défilement et pendant un curseur d'effet : à
   la taille de l'écran (moitié sous un zoom de 80 %, quart sous 35 %), une pleine résolution
   revient 250 ms après le dernier mouvement.
5. Vignettes : les dessins sont cherchés une fois par vignette, pas une fois par pixel.
6. Ouvrir un autre projet oublie les dessins dérivés de l'ancien (mêmes numéros, mêmes versions :
   un flou à réglages par défaut aurait pu réafficher l'ancien dessin — défaut d'avant).

**Avant / après** (médianes ; fil de l'interface = ce que l'écran attend) :

| Mesure | Avant | Après |
|---|---|---|
| Une image à froid, composée comme à l'export (caméra 1920 × 1080) | 225 ms | 92 ms |
| Même image, effets déjà calculés | 28 ms | 29–35 ms (la composition seule) |
| Lecture : fil de l'interface, par image d'écran | 161 ms (au mieux 6 images/s, interface figée) ; 2ᵉ tour 159 ms (cache vidé) | 0,08 ms (4,8 ms au pire : copie du dessin d'origine, une fois) ; 24 images/s tenues, 223 images d'écran sur 227 avec leurs effets prêts dès le 1ᵉʳ tour, toutes au 2ᵉ |
| Défilement (sauts dans le désordre) : fil de l'interface | 164 ms | 0,1 ms ; l'image se complète après ~40 ms (pleine) ou ~15 ms (aperçu) |
| Curseur du flou (rayon change, image fixe) | 25 ms | 0,1 ms ; résultat après 12 ms (pleine) ou 6,5 ms (aperçu) |
| Curseur du bruit | 153 ms | 0,0 ms ; résultat après 31 ms (pleine) ou 9 ms (aperçu) |
| Vignette 192 × 108, effets calculés | 46 ms | 7,7 ms |

Un effet à la fois sur le grand bitmap (3840 × 2160, 859 tuiles ; ms, avant → après) :
flou gaussien 40 px 81 → 35 ; luminosité / contraste 34 → 12 ; niveaux 154 → 29 ; teinte /
saturation 129 → 24 ; noir et blanc 34 → 13 ; négatif 33 → 12 ; postérisation 43 → 14 ; seuil
24 → 9 ; netteté 107 → 36 ; bruit 35 → 10 ; mosaïque 26 → 10 ; ombre portée 134 → 54 ; lueur
120 → 45 ; nuages 191 → 30 ; éclairage 88 → 15 ; dégradé 75 → 13 ; onde 88 → 79 ; ondulation
91 → 85 ; tourbillon 77 → 71 ; pincement 76 → 70 ; flou de mouvement 165 → 178 (inchangé).

**Ce que ça change à l'œil** (rien à l'export ni dans le dessin dérivé en pleine résolution) :
- à l'ouverture ou au changement d'un réglage, le calque s'affiche un instant (de l'ordre de 30 à
  100 ms) avec son dernier résultat, ou sans effet la toute première fois, puis l'effet arrive ;
- en lecture, au défilement et pendant un curseur, l'image est un aperçu à la taille de l'écran :
  écart mesuré avec la pleine résolution vue à moitié 0,2/255 en moyenne sur un dégradé net (flou,
  luminosité, teinte, lueur) ; un trait très fin peut y paraître un peu plus doux. Le grain du
  bruit est celui que la pleine résolution donne vue à moitié (moyenne du hasard de 2 × 2
  pixels), pas un bruit à gros grains ;
- quand le mouvement s'arrête, la pleine résolution remplace l'aperçu d'un coup (netteté qui
  revient) ;
- un calque déplacé ou tourné par la caméra (multiplan), la mosaïque, les déformations et le rendu
  restent en pleine résolution, et attendent leur calcul de fond (dernier résultat montré).

**Reste pour une passe sur la carte graphique** *(fait le 2026-10-19, branche `effets-gpu` : voir « Effets de
calque sur la carte graphique » plus bas ; ce qui suit est le plan d'origine, 8 à 10 jours estimés)* (pas fait au
moment de cette passe : le calcul de fond et l'aperçu tiennent la scène d'Anthony en temps réel sur processeur ; la
carte graphique le ferait sans copier le dessin d'origine ni envoyer le résultat) :
- fragment unique pour les réglages de couleur et le bruit (luminosité, niveaux, teinte, noir et
  blanc, négatif, postérisation, seuil, bruit : même hachage en WGSL), appliqué dans `fs_compose`
  avant l'opacité — 1 à 2 jours, sans cache ni texture en plus ;
- flou en boîte (trois passes en lignes puis en colonnes) par calcul à somme glissante, une
  rangée ou colonne par groupe, textures de travail à la taille de la zone dessinée — 2 à 3 jours ;
  netteté, lueur et ombre en découlent (1 jour) ;
- flou de mouvement, déformations (lecture inverse bilinéaire), nuages, éclairage, dégradé en
  fragment — 2 jours ; ce sont les plus lents aujourd'hui (flou de mouvement 178 ms, déformations
  70–85 ms pour une feuille 4K) ;
- tests : un banc d'essai sans écran (adaptateur wgpu) qui compare la carte graphique au calcul
  sur processeur à ±2/255, et le repli sur processeur (`PELURE_COMPOSITION=processeur`, export)
  inchangé — 2 jours.
Total estimé : 8 à 10 jours. À ce prix, plus de copie du dessin d'origine ni de cache de pixels sur
processeur pour l'écran (l'export garde le calcul exact d'aujourd'hui).

Autres restes : le flou de mouvement (64 échantillons bilinéaires par pixel) et les déformations
travaillent sur toute la feuille au lieu de la zone dessinée plus le déplacement maximal ; la copie
du dessin d'origine pour le fil de fond (3 à 8 ms, une fois par version, à la fin d'un trait sur un
calque à effets) se fait sur le fil de l'interface — il faudrait des tuiles partagées (`Arc`).
Pas vu à l'écran (pas de lancement de l'application dans cette séance) : la composition par la
carte graphique, le va-et-vient aperçu / pleine résolution et les vignettes en attente sont à
essayer par Anthony.

## Passe de performance (2026-10-19, branche `perf`) ✅

Demande d'Anthony : « avancer sur l'optimisation du soft et le rendre le plus performant possible
rapidement. C'est prioritaire. » Détail, mesures avant / après et leviers restants :
`docs/PERFORMANCE.md`.
- Mesurer tout le logiciel : superposition « Performance » (`vue.performance`, Ctrl+K ou menu
  Affichage : images par seconde, temps par panneau, réveils, arrière-plan) et banc sans fenêtre
  (`perf_tests.rs`, application complète et carte graphique, essais `--ignored`).
- Ouvrir un projet de 100 plans : 85 s → 7 s, sur un autre fil avec avancement ; enregistrer 81 → 9 s ;
  timeline zoomée 76 → 4 ms par image ; timeline de 2 000 images 20 → 5 ms ; montage de 600 plans
  32 → 4 ms ; vignettes ÷5 à ÷12 ; démarrage avec des centaines de pinceaux importés 5,2 → 0,4 s.
- Compilation : `target-cpu=x86-64-v3` (interface -20 %, PNG -26 %) ; LTO et une seule unité de
  génération écartés (aucun gain, compilation ×2 à ×3,5) ; `panic = "abort"` écarté (SHOT compte sur
  les panics rattrapés).
- Reste (classé dans le document) : décodage PNG en flux, effets sur la carte graphique.
  (Enregistrer en arrière-plan, textures à la taille du contenu, image composée gardée et vignettes hors
  du fil de l'interface : faits, ci-dessous.)

## Deuxième passe de performance (2026-10-19, branche `perf2`) ✅

Suite de la passe précédente, leviers 1 à 3 de `docs/PERFORMANCE.md` (section 9).
- **Enregistrer en arrière-plan** (`enregistrement.rs`) : Ctrl+S, Enregistrer sous et le bouton
  Enregistrer de la boîte « Modifications non enregistrées » ne figent plus l'interface (50 dessins
  modifiés : 270 → 1 ms ; premier enregistrement de 500 dessins : 2,8 s → 4 ms). Cliché sur le fil de
  l'interface (tuiles partagées à la copie), écriture sur un autre fil, `modified` remis à vrai par
  toute modification qui suit ou par un échec ; Quitter, Fermer, Nouveau, Ouvrir attendent la fin
  (fenêtre d'avancement au-delà de 200 ms) ; échec : fenêtre qui dit pourquoi, rien de perdu ;
  écriture atomique (fichier provisoire puis renommage, manifeste en dernier). « Enregistrement… n / N »
  dans la barre d'état.
- **Textures de dessin à la taille du contenu** (`gpu.rs`) : 1 Mo au lieu de 33 par dessin au trait,
  1 093 dessins dans le cache de 1 Go au lieu de 32 ; lecture d'un plan de 15 calques 7,9 → 3,9 ms,
  retour sur des dessins déjà vus 14 → 3 ms. Mêmes pixels qu'avant (essais contre la texture de la
  feuille entière et contre le calcul de secours : calques transformés, modes de fusion, aperçus réduits
  d'effets, feuilles flottantes déformées, dessin qui grandit pendant un trait, recyclage).
- **Image composée gardée** : la pile de calques n'est recomposée que si un dessin, une place, une
  opacité, un mode ou un maillage change (carte graphique 2,4 → 0,45 ms par image avec 15 calques).
- À essayer par Anthony : Ctrl+S dans un vrai gros projet (barre d'état, point • du titre, Quitter
  pendant l'écriture), un long trait qui traverse la feuille, des calques transformés, une
  enveloppe, la lecture d'un plan chargé.

## Troisième passe de performance : vignettes hors du fil de l'interface (2026-10-19, branche `vignettes-fond`) ✅

Levier 4 de `docs/PERFORMANCE.md` (section 11). Les vignettes des plans (bande du montage, planche) et les
miniatures des dessins (timeline, panneau Calques) ne se composent plus sur le fil de l'interface.
- Le fil de l'interface calcule la clé de chaque vignette (composition du plan, versions des dessins,
  caméra, cadre), prend un cliché du plan (un projet minuscule dont les tuiles sont partagées par `Arc`,
  presque gratuit depuis `perf2`) et le met en file ; il envoie les images finies à la carte graphique
  (1 ms au plus par image d'écran). Les fils (jusqu'à quatre, deux cœurs restent à l'interface) composent
  avec le code d'avant (`render::thumbnail_until`) : mêmes pixels, effets de calque, calques d'effets,
  fonds sous le trait, vidéo et CTG compris.
- Une demande par vignette : une nouvelle remplace l'ancienne et arrête son calcul ; les vignettes
  qu'on ne renouvelle plus (hors de l'écran) sont abandonnées ; les dernières demandées passent d'abord.
  L'ancienne image reste à l'écran en attendant la neuve.
- Montage de 600 plans, vignettes à froid : image d'écran 6,8 → 2,2 ms à la molette, 11,1 → 3,4 ms en sautant
  de plan en plan (pire image 13,2 → 4,6 ms) ; section Montage 9,9 → 1,8 ms.
- Essais : `thumbs_tests.rs` (mêmes pixels que l'ancien chemin sur le projet d'essai complet, mise à jour
  après onze sortes de changements, abandon des demandes périmées, rien composé sur le fil de l'interface).
- À essayer par Anthony : défiler et zoomer un grand montage, sauter de plan en plan, dessiner montage visible,
  changer un effet ou un calque d'effet d'un plan, la planche en vue Plans, la timeline zoomée.

## Effets de calque sur la carte graphique (2026-10-19, branche `effets-gpu`) ✅

Demande d'Anthony : les effets de calque sur la carte graphique, en temps réel, à pleine résolution pendant la lecture et
les curseurs ; le calcul sur processeur reste la référence de l'export. Détail, écarts et mesures :
`docs/PERFORMANCE.md`, section 12.
- **Chaînes de passes** (`gpufx.rs`, `gpufx.wgsl`, branchées dans `gpu.rs`) : la pile d'effets d'un calque se dessine
  à la composition sur des textures de travail en 8 bits (va-et-vient, réserve recyclée), les réglages déjà interpolés
  entre les clés partent en uniformes à chaque image. La texture résultat est gardée et redessinée seulement quand sa
  signature change. 28 effets sur 29 : réglages de couleur, flou gaussien (trois boîtes, comme le processeur), flou de
  mouvement, netteté, bruit, mosaïque, ombre portée, lueur, onde, ondulation, tourbillon, pincement, nuages, éclairage,
  dégradé, flash, image d'impact, lueur diffuse, secousse, vignettage, grain, aberration chromatique. Seul le tremblement du
  trait reste sur le processeur ; une pile qui en contient un (ou un calque CTG, un fond, une vidéo) est calculée tout
  entière sur le processeur. *(Mise à jour du 2026-10-20 : le tremblement a sa passe, `fs_boil`, 29 effets sur 29 ; seuls
  les effets du trait, section « Effets du trait » plus bas, restent un calcul de fond, et une pile qui en mêle à des effets
  de la carte est coupée en deux.)*
- **Calques d'ajustement hors dossier** : leurs effets passent sur l'image composée jusque-là (papier compris), résultat
  posé avec l'opacité et le mode du calque. Dans un dossier : calcul de fond comme avant.
- **Même hasard, mêmes arrondis** : écart de 0 à 2 niveaux avec le processeur sur tous les effets (4 au plus sur la
  composition entière avec un mode de fusion), flou et déformations compris (`gpufx_tests.rs`, 19 essais, passés
  s'il n'y a pas de carte ; les mesures sont dans `gpufxperf_tests.rs`, essais `#[ignore]`).
- **Réglage « Effets sur la carte graphique »** (allumé par défaut, Préférences > Performance, onzième réglage du
  diagnostic ; Équilibré l'éteint sans carte réelle, Performance hors carte dédiée) ; repli sur le processeur si le shader
  est refusé.
- **Mesures** (RTX 2080 SUPER, feuille 3840 × 2160, pleine résolution) : scène d'Anthony 2,9 ms par image en lecture,
  curseur du flou 3,5 ms, curseur du bruit 0,9 ms ; scène impact + lueur + secousse 1,5 ms par image (avant : quart de
  résolution, 33 à 37 ms par image sur le fil de fond, pleine résolution 308 ms au repos) ; pire cas d'une feuille pleine avec
  six effets lourds : 22 ms.
- À essayer par Anthony (rien n'a été lancé à l'écran) : `docs/PERFORMANCE.md` 12.7 (lecture et curseurs de la scène, les
  sept effets Cinéma sur un calque et en calque d'ajustement, tremblement + flou, calque d'ajustement dans un dossier,
  réglage éteint en lecture, mémoire vidéo).
- Reste : calques d'ajustement dans un dossier (composer le dossier à part), le tremblement du trait, un mélange carte /
  processeur dans une même pile, un aperçu réduit sur la carte pour les cartes faibles, les vignettes de la timeline (elles
  passent encore par le calcul de fond).

## Diagnostic des performances (2026-10-19, branche `diagnostic`) ✅

Demande d'Anthony : « dans Aide, ajouter une option qui détecte le matériel et propose des
ajustements de performances qui vont jusqu'à éteindre certaines fonctions gourmandes. »
- Aide > Diagnostic des performances… (`aide.diagnostic`, aussi dans Ctrl+K) : détecte le processeur,
  la mémoire, la carte graphique (et sa mémoire vidéo), l'écran ; mesure 13 secondes sur un autre fil
  (disque, composition, trait texturé, flou plein et réduit, lecture, envoi à la carte) avec avancement
  et annulation ; propose Qualité, Équilibré ou Performance avec ses raisons et ses réglages à cocher ;
  Appliquer / Revenir comme avant ; rapport copié ou enregistré (`materiel.rs`, `diagbanc.rs`,
  `diagnostic.rs`, `diagnosticui.rs`).
- Dix vrais réglages, tous dans Fichier > Préférences… (section Performance) : aperçu réduit des effets,
  rendu des traits texturés (exact / rapide jusqu'au repos / rapide partout), tremblement sans « Animer à
  l'arrêt », plafond de pelure, caches carte graphique / traits / effets, fils de calcul, résolution des
  vignettes, interface sans animations. Ajoutés : aperçu réduit même zoomé, rendu des traits à l'ouverture,
  tremblement coupé, plafond de pelure, taille réglable des trois caches, plafond de fils, vignettes réduites,
  animations coupées ; les valeurs par défaut sont le comportement d'avant.
- Proposé une fois au premier lancement sur une machine sans diagnostic (« Lancer le diagnostic » /
  « Plus tard »), jamais appliqué sans accord.
- À essayer par Anthony : la fenêtre à l'écran, l'effet de « Performance » sur un vrai projet, le rendu
  des traits texturés en « rapide partout » (l'export demande d'abord le rendu exact).

## Raccourcis proposés, à valider

Raccourcis proposés par SHOT (source « proposition » dans `keymap.toml`) :

| Action | Proposé | Note |
|---|---|---|
| Enregistrer sous | Ctrl+Maj+S | aucune touche d'origine |
| Rétablir (second) | Ctrl+Y | habituel sous Windows |
| Supprimer (second) | Retour arrière | |
| Ajuster à la fenêtre | Ctrl+0 | changé en 1a : Maj+F sert désormais à « scène suivante » |
| Nouvelle scène | Maj+P | P = nouveau panneau (preset) |
| Scène précédente / suivante | Ctrl+Pg préc. / Ctrl+Pg suiv. | ✅ validé (1d). Maj+A appartient aux guides (« aligner sur le guide ») |
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

- ✅ Perte de plan au collage ? Non reproduite : coller des plans (paste_plans) donne des ids neufs et survit à l'enregistrement (tests). L'écart « 5 à l'écriture, 4 à la relecture » venait de paste_panels (panneaux gardés dans la scène, mis bout à bout au chargement). Le chargement renumérote désormais les ids de plan, de scène et de séquence en double du manifeste, pour ne rien perdre.

## Commentaires sur la timeline (demandés par Anthony le 2026-10-18) ✅

La demande : « la possibilité de créer des commentaires sur la timeline. Un petit curseur apparaît là
où un commentaire est laissé. Une boîte de dialogue apparaît quand on survole le curseur. Une
fenêtre dans les panneaux permet au commentateur d'écrire le commentaire, de choisir la couleur du
curseur, etc. Le curseur sur la timeline est déplaçable. » Menée sur la branche `commentaires`.

- ✅ Modèle (`commentaires.rs`) : un commentaire tient à un **plan** (son id) et à une **image de ce
  plan** (depuis 0), pas à une image du film. Le montage le montre à début du plan + image, la
  timeline du plan à l'image du plan : il suit son plan quand on avance, recule ou réunit des plans,
  et ses fanions restent à la même image quand un plan d'avant s'allonge. Couper un plan emmène avec
  la suite les commentaires d'après la coupe ; réunir deux plans ramène ceux du second au bout du
  premier ; retirer un plan emporte les siens (l'annulation les rend). Si le plan devient plus court
  que l'image voulue, le fanion se montre sur sa dernière image et la reprend si le plan s'allonge.
  Champs : texte, auteur, couleur (RVB : la palette des marques de couleur, un jaune, ou n'importe
  quelle couleur du sélecteur), date de création, « résolu ». Longueur (plage) : pas faite.
- ✅ Annulation : `Document::comments` et `Structure::comments` ; toute modification est une étape
  d'`Edit::Structure` avec son nom (« Nouveau commentaire », « Déplacer le commentaire », « Texte du
  commentaire », « Auteur du commentaire », « Couleur du commentaire », « Commentaire résolu »,
  « Supprimer le commentaire ») ; le projet est marqué modifié.
- ✅ Fichier : `commentaires.json` à côté du manifeste (champs en français, `format`
  `shot-commentaires`, `version` 1), **sans changer `project::VERSION`** (24). Écrit avant le
  manifeste, atomiquement ; retiré quand il n'y a plus de commentaire ; absent : aucun commentaire ;
  illisible ou d'une version plus récente : le projet s'ouvre, sans commentaires, avec un
  avertissement. Une version qui ne le connaît pas le laisse en place (elle n'efface que les
  dessins, sons, modèles et textures inutilisés) : le fichier peut seulement vieillir si on
  enregistre avec elle. Les copies de secours l'écrivent aussi.
- ✅ Fanions (`commentairesui.rs`) dans la règle du montage (global) et de la timeline (ceux du plan
  courant) : mât et fanion de la couleur du commentaire, estompé s'il est résolu, cerclé de blanc
  s'il est choisi. Survol : bulle immédiate (auteur, date, plan, time code, début du texte ; jusqu'à
  trois si plusieurs se superposent). Clic : le choisit et y met la tête de lecture (en storyboard,
  le plan devient courant). Glisser : le déplace image par image, d'un plan à l'autre dans le
  montage, une seule étape d'annulation ; Échap abandonne. Double-clic dessus : l'éditer ; double-clic
  dans la règle : poser un commentaire. Clic droit : Modifier, Couleur ▸, Marquer résolu, Supprimer.
  Bouton « Nouveau commentaire » dans les barres du montage et de la timeline, entrée « Commentaire
  ici » du menu de la règle.
- ✅ Panneau **Commentaires** (`Pane::Comments`, colonne de droite, fermé par défaut, icône dans la
  barre des panneaux, `vue.commentaires`) : liste dans l'ordre du film (pastille, time code, plan,
  auteur, première ligne, case « résolu »), seules les lignes visibles sont dessinées ; un clic va au
  commentaire ; éditeur du commentaire choisi (texte sur plusieurs lignes, auteur, pastilles et
  sélecteur de couleur, résolu, supprimer) ; « Nouveau commentaire » ; case « Montrer les résolus »
  (liste et règles).
- ✅ Actions : `vue.commentaires`, `commentaire.ajouter`, `commentaire.resoudre`,
  `commentaire.supprimer`, `commentaire.precedent`, `commentaire.suivant` (aucune touche par
  défaut). Suppr supprime le commentaire choisi seulement quand le dernier clic l'a visé (fanion ou
  panneau) ; sinon elle suit sa règle : rien sans sélection.
- ✅ Auteur : Fichier > Préférences… > Commentaires (`auteur_commentaires` dans `reglages.toml`) ;
  vide : l'utilisateur Windows.
- ✅ Tests dans `commentaires_tests.rs` (fichier aller-retour, absent, abîmé, fichiers étrangers
  gardés, déplacer / modifier / supprimer avec annulation et rétablissement, plans réordonnés, coupés,
  réunis, retirés, zone de prise d'un fanion, Suppr sans focus). Pas vu à l'écran dans cette séance :
  à essayer par Anthony.
- Pas faits : longueur d'un commentaire (une plage dans la règle) ; réponses à un commentaire ; les
  commentaires ne sont pas copiés avec des plans copiés ou dupliqués. (Le **PDF** les écrit
  depuis la relecture par le réalisateur, 2026-10-20 : option « Avec les commentaires ».)

## Références visuelles (fenêtre « Références », 2026-10-19) ✅

Un tableau d'images et de vidéos à regarder pendant qu'on dessine, détachable sur un deuxième écran.
Menée sur la branche `references`.

- ✅ Panneau `Pane::References` (colonne de droite, fermé par défaut, icône dans la barre des
  panneaux, `vue.references`), détachable (« Dans une fenêtre à part ») avec « Toujours au-dessus »
  (`ViewportBuilder::with_always_on_top`, puis `ViewportCommand::WindowLevel`).
- ✅ Ajouter : images PNG / JPEG (copiées telles quelles dans `references/`, sur un autre fil),
  vidéos (ffmpeg, chemin de `video::start_import`, 1280×720 au plus, images JPEG dans
  `references/<n>/`), fichiers déposés sur le panneau (`soundtrack::handle_dropped_files` les lui
  donne s'ils tombent dessus ; fenêtre détachée : ses propres fichiers lâchés), Ctrl+V (réutilise
  `pasteimage::clipboard_image`), boutons et Fichier > Importer.
- ✅ Tableau libre : déplacer, zoomer (molette, autour de la souris), déplacer le fond, glisser,
  agrandir par les coins, tourner par la poignée ronde (Maj : 15°), choix multiple (Maj+clic), grille et
  liste, recherche (accents et majuscules ignorés) et filtre par étiquette.
- ✅ Outils de la référence choisie : ajuster, 100 %, miroirs H / V, niveaux de gris (Rec. 709), nom,
  échelle, rotation, étiquettes ; Alt+clic = pipette vers la couleur courante de SHOT ; vidéo :
  lecture, image par image, curseur, entrée / sortie, boucle, vitesse, « suivre la tête de lecture »
  avec décalage (`Clip::follow_index`).
- ✅ Plans : « Lier au plan courant » (`references.lier_plan`), filtre « références du plan courant »
  (`references.filtre_plan`). Un plan retiré n'est plus cité ; deux plans réunis réunissent leurs liens.
- ✅ Rangement sans changer `project::VERSION` (26) : `references.json` (`format` `shot-references`,
  `version` 1) et dossier `references/`, par `Document::references` / `Structure::references` et
  `SaveJob` (`references::plan_save` au cliché, `SavePlan::write` / `cleanup` sur le fil d'écriture).
  Mêmes protections que `commentaires.json` : fichier illisible ou plus récent jamais effacé
  (`write_sidecar`, `.illisible`), `references/` intact si le fichier n'a pas pu être lu, et un fichier
  touché pendant la séance n'est jamais effacé (annuler / rétablir une suppression après
  l'enregistrement le retrouve). Avant le premier enregistrement : dossier temporaire
  (`Reference::home`), recopié à l'enregistrement et aux copies de secours.
- ✅ Rapidité : décodage sur deux fils à la définition demandée (128 à 4096 pixels), cache de textures
  LRU (`references::Lru`) borné à un huitième du « cache de la carte graphique » du diagnostic des
  performances, définition qui suit le réglage « vignettes » ; trois textures téléversées au plus par
  image d'écran ; au repos, aucun repaint.
- ✅ Annulation (`Edit::Structure`) avec des noms français : « Ajouter une référence », « Supprimer la
  référence », « Déplacer les références », « Redimensionner la référence », « Tourner la référence »,
  « Retourner la référence », « Lier la référence au plan »…
- ✅ Tests dans `references_tests.rs` et `fiabilite_annexes_tests.rs` (aller-retour du fichier,
  fichiers abîmés ou plus récents, nettoyage, import, transformations, plans et filtre, image de
  vidéo selon tête de lecture / décalage / vitesse / boucle, couleur prise, cache, annulation, panneau
  dessiné dans ses trois modes). Pas vu à l'écran dans cette séance : à essayer par Anthony.
- Pas faits : WebP (pas de décodeur dans le programme), GIF animé, séquence d'images comme référence,
  son des vidéos de référence, texte et flèches sur le tableau, export du tableau, copie des
  références avec des plans copiés ; un fichier lâché sur la fenêtre principale est une référence si
  la dernière position de souris connue est sur le panneau (à vérifier à l'écran : si Windows ne la
  donne pas pendant le glisser, les boutons restent) ; l'extraction d'une vidéo longue garde toutes
  ses images (de l'espace disque).

## Boîte de sélection (demandée par Anthony le 2026-10-18) ✅

La demande : « revoir le système de transformation après sélection. On peut enlever dans SHOT notre système de
transformation avec poignées de Bézier (ou en faire un outil spécifique). » Elle s'appuyait sur une capture : un cadre aux
petites poignées carrées (coins et milieux des côtés), un point rond de pivot au centre, et des
flèches rouges qui montrent les gestes. **Tout le reste (touches, détails) est un choix de SHOT.**

- ✅ **La boîte** (`selbox.rs`, géométrie pure ; `seltools.rs`, dans l'application) : après un choix
  (traits vectoriels, zone bitmap), l'outil Sélection montre le cadre, huit poignées carrées et le
  pivot rond. Les gestes, avec leur curseur :
  - glisser dedans : **déplacer** ;
  - un coin : **mettre à l'échelle** dans les deux sens ; le milieu d'un côté : dans un seul sens ;
  - juste à l'extérieur d'un coin : **tourner** autour du pivot ;
  - un bord, entre les poignées : **incliner** (cisaillement le long de ce bord, le bord opposé reste) ;
  - le point rond : **déplacer le pivot** (il se remet au centre quand on le lâche près de lui).
  Tailles de prise en pixels d'écran (donc les mêmes à tout zoom) : poignées 9, pivot 8, bande d'un
  bord 5 de chaque côté, couronne de rotation jusqu'à 26 du coin ; elles rétrécissent (jusqu'à
  45 %) sur une boîte de moins de 40 px, pour que chaque partie reste atteignable.
- ✅ **La boîte reste tournée** : après une rotation ou une inclinaison, ses poignées restent sur les
  côtés du dessin tourné (échelle et inclinaison suivent les axes de la boîte, pas ceux de l'écran).
  Elle repart droite quand la sélection ou les éléments changent par ailleurs (annuler, miroir,
  autre choix) ; les flèches la déplacent sans la défaire. Une zone bitmap suit la transformation de
  ses pixels soulevés.
- ✅ **Touches** (choix de SHOT, à vérifier) : Maj sur un coin ou un côté, proportions gardées ; Maj en
  déplaçant, un seul axe ; Maj en tournant, par pas de 15° ; **Ctrl** en tirant une poignée ou un
  bord, autour du pivot au lieu du côté opposé (Alt est pris par la pipette). Maj + clic dedans
  ajoute au choix au lieu de déplacer. Le réglage « Garder les proportions » inverse l'effet de Maj
  sur les poignées. Sans pivot déplacé, le pivot est le centre de la boîte ; l'échelle d'une poignée
  se fait depuis le côté opposé (à vérifier : la capture dit « centre de rotation et d'échelle »,
  Ctrl donne l'échelle autour du pivot).
- ✅ **Sans latence** : les gestes passent tous par une `Affine` (échelle, rotation, cisaillement,
  déplacement) que la feuille flottante (`flottante.rs`) pose comme un déplacement : la carte
  graphique dessine un parallélogramme (quatre coins), le processeur inverse la matrice entière ;
  le contour bleu (`contours.rs`) suit déjà toute transformation affine. Aucun rejeu du moteur sur le
  fil de l'interface pendant l'inclinaison (essai `#[ignore]` `floating_selection_*` étendu :
  « inclinaison », 0 rejeu, 0 tuile renvoyée, 1,5 ms de saisie, 0,14 ms au lâcher).
- ✅ **Annulation** : un geste, une étape, à son nom (panneau Historique) : « Déplacer », « Mettre à
  l'échelle », « Tourner », « Incliner » ; les flèches « Déplacer ». Une zone bitmap se repose en une
  étape au nom du geste (« Transformer la sélection » s'il y en a eu plusieurs).
- ✅ **Réglages** (outil Sélection, calque vectoriel ou bitmap) : champs chiffrés de la boîte — position
  x / y du pivot, largeur et hauteur (le long des axes de la boîte), angle, inclinaison —,
  interrupteur « Garder les proportions » (largeur et hauteur ensemble ; inverse Maj sur les
  poignées), « Réinitialiser le pivot ». Un champ qu'on tire ou qu'on tape passe par le même chemin
  qu'un glissé (la feuille flottante suit, une seule étape d'annulation à la fin de la saisie).
  Actions : `selection.reinit_pivot`, `selection.proportions` (aucune touche par défaut).
- ✅ **Enveloppe et perspective deviennent l'outil Déformer** (même case de la barre d'outils que
  Transformer, même action `outil.perspective`, nouveau nom « Déformer ») avec deux modes dans Réglages :
  Enveloppe | Perspective (actions `deformer.enveloppe`, `deformer.perspective`). Plus de Ctrl + coin après
  une sélection, ni avec Transformer. L'enveloppe ne déforme que les éléments choisis (vectoriel), la
  zone choisie (bitmap), sinon tout le dessin ; elle ne perd rien (même moteur : `envelopetools.rs`).
  La perspective ne prend pas une zone bitmap (comme avant) ; il faut l'enveloppe pour cela. Une zone
  choisie survit au passage sur Déformer (`Tool::keeps_zone`).
- ✅ **Bitmap** : même boîte (déplacer, échelle, rotation, **inclinaison**, pivot) par les pixels
  soulevés (`pixsel::stamp`, qui prenait déjà une affine quelconque) ; Transformer sur une zone choisie
  garde la boîte.
- ✅ Tests : `selbox_tests.rs` (prise des parties à cinq zooms et sur une boîte tournée, petites
  boîtes, échelle autour d'un coin et du pivot, un seul sens, proportions, rotation et Maj,
  inclinaison, pivot, champs chiffrés aller-retour, miroir, annulation et rétablissement de chaque
  geste) et un test de cisaillement dans `flottante_tests.rs`. Pas vu à l'écran dans cette séance :
  à essayer par Anthony.
- Pas faits : le pivot n'est pas enregistré avec le projet ni annulable ; pas d'inclinaison de la
  perspective ; Échap n'abandonne pas un geste en cours (comme avant).

## Tremblement du trait, façon WigglyPaint (demandé par Anthony le 2026-10-19) ✅

La demande : « une option uniquement pour le vectoriel de tremblement du trait, avec des paramètres
possibles pour modifier l'intensité du tremblement, comme dans WigglyPaint ». Branche `tremblement`
(`src/tremblement.rs`, essais `tremblement_tests.rs`). Pas vu à l'écran dans cette séance : à essayer
par Anthony.

- ✅ **Où** : un effet de la pile du calque (`EffectKind::Boil`, famille Déformation), réservé aux
  calques vectoriels (`fx::layer_steps`, `EffectKind::vector_only` ; absent des menus d'un calque
  bitmap). Interrupteur dans le menu du calque (« Tremblement du trait », « Animer à l'arrêt »),
  réglages dans le panneau Effets, actions `fx.tremblement` et `fx.tremblement_repos`. Il hérite des
  effets : enregistrement dans le projet, clés (intensité, fréquence… animables), calcul de fond avec
  la règle « sans latence », cache des dessins dérivés par jeu de réglages, aperçu réduit pendant la
  lecture, envoi d'avance à la carte graphique, export et vignettes.
- ✅ **Réglages** : Intensité 0–20 px (3), Fréquence 0–100 % (55 ; longueur d'onde de 160 px à 4 px,
  en échelle exponentielle), Variantes 2–6 (3), Vitesse 1–12 images par variante (2 : 12 changements
  par seconde à 24 ips), Épaisseur 0–100 % (0), Graine 1–999, Animer à l'arrêt (non).
- ✅ **Déplacement déterministe** : un champ de bruit de valeurs en 2D, maille = demi-longueur d'onde,
  nœuds tirés d'un hachage entier de (graine, variante, nœud) ; chaque nœud porte un vecteur de norme au
  plus 1 et l'interpolation lissée (3t² − 2t³) est une combinaison convexe : continu, de classe C¹,
  jamais plus que l'intensité, et la même image donne toujours les mêmes pixels. Variante d'une image =
  `(image / vitesse) % variantes`.
- ✅ **Choix de rendu (et pourquoi pas un rejeu par variante)** : chaque variante est le dessin déjà
  rendu (les pixels exacts des traits en cache) relu à travers le champ — une relecture bilinéaire, pas
  un rejeu du moteur de brosses. Rejouer le moteur pour chaque variante aurait coûté N fois les 2 à 20 ms
  de chaque trait texturé, N fois la mémoire de chaque trait, et un déplacement par trait (pas par
  champ). Le champ donne le même caractère pour toutes les brosses, texturées ou non, sans toucher au
  moteur, et les variantes se calculent en arrière-plan comme n'importe quel effet. Le prix : une
  relecture bilinéaire adoucit très légèrement le trait (au plus un demi-pixel), et deux traits proches
  tremblent ensemble (le champ est continu) — c'est aussi le caractère de WigglyPaint. Le maillage
  `warp.rs` sur la carte graphique a été écarté : il aurait demandé un maillage de plusieurs dizaines de
  milliers de cases (longueur d'onde de 4 px sur 1920 px), renvoyé à chaque image, et un calcul de
  secours sur processeur pour l'export, plus lent qu'une variante gardée.
- ✅ **Épaisseur variable** : la couverture des pixels de bord grimpe ou descend selon une valeur lisse du
  même champ (puissance de la couverture, 2^(±1,5 × réglage)) : le cœur du trait ne bouge pas, les bords
  s'épaississent et s'amincissent d'une fraction de pixel.
- ✅ **Sans latence** : l'écran ne calcule jamais une variante (`fxcache.rs`, fil de fond). En lecture, les
  variantes des 24 images qui viennent sont demandées d'avance, la plus proche d'abord, et envoyées à la
  carte graphique ; en attendant une variante, l'écran montre la variante d'avant du même dessin (ou le
  dessin tel quel si le dessin a changé depuis : jamais un tremblement d'avant le dernier trait posé,
  `fx::display_with`). Aucun rejeu du moteur, nulle part, y compris sur le fil de l'interface.
- ✅ **À l'arrêt** : « Animer à l'arrêt » éteint par défaut (une passe de performance est en cours et
  une image fixe doit coûter 0 % de processeur) ; l'image fixe montre la variante de l'image courante.
  Allumé, le calque suit l'horloge (`tremblement::rest_frame`) et seul le prochain changement de variante
  réveille l'écran (`rest_wake` → `request_repaint_after`, 83 ms à 24 ips avec une vitesse de 2 : 12 Hz,
  pas 60) ; les autres variantes sont demandées et envoyées d'avance (`other_variants`).
- ✅ **Édition** : sur le calque courant, un geste de dessin, de gomme, de sélection (tant qu'un choix
  existe) ou de transformation éteint son tremblement (`App::boil_editing`) : on voit et on prend les
  traits à leur vraie place. Les dessins ne changent jamais : la prise, la sélection, le cutter et
  les exports lisent la vraie géométrie. L'export d'images, de film, de PDF et les vignettes montrent
  la variante de chaque image.
- ✅ **Format** : `project::VERSION` 25 → 26 (la sorte d'effet `tremblement` est inconnue des versions
  d'avant, qui refusent le projet au lieu de le montrer sans son tremblement).
- ✅ **Mesures** (`tremblement_tests::boiling_playback_cost`, `#[ignore]`, 1920 × 1080, 60 traits texturés,
  3 variantes, une toutes les deux images, 24 ips en temps réel) : lecture de 96 images, **0 rejeu du
  moteur sur le fil de l'interface**, interface **0,04 ms par image en moyenne** (1,2 ms pour la première
  image, qui demande les variantes), 95 images servies avec la variante prête, une seule en attente ;
  une variante en arrière-plan : **8 à 10 ms** pleine résolution (3,4 Mo), **4,7 ms** en aperçu réduit de
  moitié (1,1 Mo), 20 ms au pire (feuille couverte de bout en bout, 334 tuiles), 10 ms avec l'épaisseur
  variable.
- Pas faits : le tremblement ne se règle pas par trait mais par calque ; il adoucit un peu le trait
  (lecture bilinéaire) ; l'écran n'a pas été regardé (pas de capture dans cette séance) ; la perte du
  rendu exact d'un trait sous le tremblement n'est pas un défaut de la plume mais de la relecture —
  pour un rendu net, éteindre le tremblement à l'export final ou demander un rejeu par variante (piste
  écartée plus haut).

## Effets du trait : écriture, crayonné, respiration, ondulation, pointillés, désagrégation (demandés par Anthony le 2026-10-20) ✅

La demande : « le tremblement, j'adore » — d'autres effets qui agissent sur le trait lui-même (« effets de matière » plus
tard, pas maintenant), et le tremblement sur la carte graphique si c'est faisable. Branche `effets-trait` (partie de `main`
7562800 ; `src/fxtrait.rs`, essais `fxtrait_tests.rs`, `gpufx_tests.rs`). Pas vu à l'écran dans cette séance (aucune
application lancée, consigne) : liste à regarder dans `docs/INTERFACE.md` (section 20).

- ✅ **Où** : six effets de la pile du calque, famille **Trait**, **calques vectoriels seulement** (grisés avec la raison sur
  un calque bitmap, dans le panneau Effets, les menus et la rangée d'icônes : aucun repli raisonnable sur des pixels, ils
  lisent l'ordre, la longueur et la forme des traits). Même interface que le tremblement : panneau Effets, clés (losanges
  de la timeline et du montage), copier-coller (une seule fois par calque et par sorte), enregistrement dans le projet.
  Une **rangée d'icônes** en tête du panneau Effets coche et décoche chacun (bulle : nom, raccourci, description ;
  `bulles::etat_si`) ; un menu baguette pose trois **préréglages** (« Apparition écrite », « Croquis nerveux »,
  « Poussière ») avec leurs clés à partir de la tête de lecture, en une étape d'annulation ; neuf actions `fx.*` sans touche
  (Ctrl+K, clic droit sur une icône pour assigner un raccourci).
- ✅ **Écriture du trait** : avancement 0–100 % (animable), effacement, tous les traits ensemble, durée selon la longueur,
  douceur du rythme, bord doux (pointe qui se fond sur N pixels), zones remplies avec le trait. Ordre = ordre de dessin.
  Chaque pixel d'un trait a une abscisse curviligne (le point de l'axe le plus proche) donc un instant de tracé ; 0 % rend
  un dessin vide, 100 % exactement le dessin (rien à calculer), et à 50 % « l'un après l'autre » un seul trait est
  vraiment transformé (les tracés sont posés depuis leur cache, les autres sautés).
- ✅ **Crayonné multiple** : 1 à 4 passes, décalage, opacité des passes, nervosité (maille du champ de déplacement), variantes
  et vitesse (retirage toutes les N images, comme le tremblement), graine ; le trait d'origine reste dessus, intact. Une
  passe = le trait relu à travers une translation de travers (d'un côté puis de l'autre) plus un champ de déplacement doux
  (le champ du tremblement, un par trait, passe et variante) ; jamais plus loin que le décalage.
- ✅ **Respiration** : épaisseur de base (clés), amplitude, période, vague le long du trait. Chaque pixel est remis à
  l'échelle autour de l'axe (l'épaisseur étire la texture de la brosse, elle ne la rejoue pas).
- ✅ **Ondulation du trait** : amplitude, longueur d'onde, vitesse (négative : l'autre sens), bouts fixes ; déplacement le long
  de la normale, période exacte (vitesse × images = longueur d'onde : même image).
- ✅ **Pointillés** : tiret, intervalle, vitesse, bord doux ; le motif de l'image k est celui de l'image 0 décalé de vitesse × k.
- ✅ **Désagrégation** : avancement (animable), taille des grains, dispersion, sens du trait, direction et force du vent,
  graine, zones remplies. Un grain (carré de la grille de la feuille) part à son heure, dans une direction tirée par hachage,
  ses pixels se séparent en route, il s'efface en fin de course ; 100 % : plus rien. Mêmes réglages, même poussière.
- ✅ **Le tremblement du trait sur la carte graphique** : `fs_boil` reproduit le champ (mêmes nœuds, même interpolation, même
  épaisseur) ; écart avec le processeur : moyenne 0,000, maximum 1 niveau. C'était le seul effet qui obligeait une pile à
  rester sur le processeur. Choix : le tremblement relit les pixels déjà rendus, il ne touche pas la géométrie vectorielle ;
  il n'a donc pas besoin d'un calcul de fond de la géométrie, et le shader est l'exacte copie du champ.
- ✅ **Règle de mélange réduite** : les effets du trait sont les seuls sans passe ; une pile qui en mêle à d'autres effets est
  coupée en deux (`fx::hybrid_split`) : le fond calcule le dessin dérivé par eux, la carte passe le reste dessus.
- ✅ **Sans latence, déterministe** : jamais sur le fil de l'interface (copie du dessin vectoriel 0,5 à 1 ms par retouche,
  puis 0,000 ms par image) ; le calcul de fond transforme les traits en parallèle et les pose par bandes de tuiles dans
  l'ordre : même image à chaque fois, sur un fil ou plusieurs ; export et vignettes = écran. Mesures sur 2 000 traits (60 ms
  pour le dessin seul) : écriture 34 à 85 ms, crayonné 174 ms, respiration 114 ms, ondulation 139 ms, pointillés 68 ms,
  désagrégation 123 ms ; sur 200 traits de 5 à 50 ms, tout tient à 24 images par seconde (`docs/PERFORMANCE.md`, section 13).
- ✅ **Format** : `project::VERSION` 27 → **31** (les sorties 28 à 30 sont réservées à d'autres chantiers) ; les projets d'avant
  s'ouvrent tels quels, une version précédente refuse le projet (elle ne connaît pas ces sortes d'effet).
- ✅ **Essais** (`fxtrait_tests.rs`, 33 dont deux à lancer à la main — mesures et planche de contact en PNG ; `gpufx_tests.rs`, un de plus) : écriture à 0, 50 et 100 %, sens, ensemble, durées égales, bord doux,
  zones remplies, clés, déterminisme (un fil ou plusieurs), enregistrement et projets d'avant, calque vectoriel seulement,
  export = vignettes = écran, dessin dérivé, traits texturés sans rejeu, lecture sans rejeu sur le fil de l'interface,
  actions, préréglages, copier-coller, accord carte / processeur du tremblement et de la pile mêlée.
- Pas faits : un « animer à l'arrêt » pour le crayonné (seul le tremblement l'a ; les autres suivent l'image courante) ; un
  aperçu réduit de ces effets (longueurs en pixels de la feuille) ; l'effet par trait choisi (ils s'appliquent à tout le
  calque : un calque par groupe de traits) ; les zones remplies et les textes ne sont pas des traits (réglage dédié pour
  l'écriture et la désagrégation) ; un aplat sous le trait reste fixe ; 2 000 traits à 24 images par seconde en temps réel
  pour les effets qui changent à chaque image (pistes : carte des points les plus proches gardée par version du dessin,
  version sur la carte de l'écriture) ; les « effets de matière » (prochaine série). Comparaisons avec d'autres logiciels :
  aucune vérifiée (**à vérifier**).

## Effets de matière : liseré, trait sec, encre qui bave, dégradé le long du trait, changer de plume (demandés par Anthony le 2026-10-20) ✅

La demande : la deuxième série d'effets du trait, ceux de matière. Branche `effets-matiere` (partie de `main` da13a79 ;
`src/fxmatiere.rs`, essais `fxmatiere_tests.rs`). Pas vu à l'écran dans cette séance (aucune application lancée, consigne) :
liste à regarder dans `docs/INTERFACE.md` (section 21) ; planche de contact PNG par un essai à lancer à la main.

- ✅ **Où** : cinq effets de la pile du calque, famille **Matière**, dans une **deuxième rangée d'icônes** sous celle du Trait
  (plutôt qu'une rangée de onze icônes : deux familles, deux rangées, une baguette chacune) ; mêmes clés, copier-coller,
  enregistrement. Le liseré, le trait sec et l'encre qui bave vont **sur les deux sortes de calques** ; le dégradé et la
  plume sont **vectoriels seulement** (grisés avec la raison dans la bulle : ils lisent l'abscisse et le tracé des traits).
- ✅ **Liseré** : distance euclidienne **exacte** au bord de l'encre (transformée de Felzenszwalb, deux passes séparables
  réparties sur les cœurs ; bord = contour à 25 % de couverture), pas un flou seuillé : un pixel d'encre donne un disque exact
  (essai : tous les pixels à moins de la largeur sont pleins, à plus d'un pixel au-delà vides). Derrière le trait, ou en anneau
  (rien sous l'encre, même transparente). Sur les pixels : vectoriel et bitmap donnent la même image.
- ✅ **Trait sec** : vectoriel, des trous le long de chaque trait (une case de 75 / fréquence pixels, trois fois sur quatre un
  trou de longueur tirée au hasard, où passent des poils) et un bord qui s'effrite en stries parallèles au trait (bruit étiré
  le long de l'abscisse, rayon du trait) ; retirage toutes les N images. Bitmap, **forme simplifiée décidée** : plages creuses
  d'un bruit de valeur à basse fréquence (la taille d'une plage = longueur des trous, la part creuse = trous × longueur) et
  bord effrité d'après la distance à l'encre ; pas de notion de « trait » donc pas de trous le long de lui.
- ✅ **Encre qui bave** : halo irrégulier sous le trait, de la couleur de l'encre la plus proche (fbm de la taille du grain du
  papier, portée entre étalement × (1 − irrégularité) et × (1 + irrégularité)), bord plus foncé et un peu effrité sur 2 à
  3 pixels, cœur intact. Sur les pixels : les deux sortes de calques.
- ✅ **Dégradé le long du trait** : 2 à 4 couleurs, par trait ou sur tout le dessin dans l'ordre, mélange, défilement (un
  dessin dérivé par image seulement quand il défile), boucle. Sélecteur de couleur dans le panneau (nouveauté de l'interface :
  un réglage couleur = trois réglages à la suite, clés comprises).
- ✅ **Changer de plume** : chaque trait est refait avec un réglage de pinceau (désigné par un numéro de 24 bits tiré de
  son **nom**, pas de son rang : ranger ou importer des brosses ne change pas le choix ; publié par la bibliothèque à
  chaque `share()`), tracé et appuis gardés, **rayons de ses points refaits d'après la plume** (la boîte du trait en dépend :
  sans cela la plume était coupée). Les traits redessinés sont gardés (un rejeu par trait et par plume, pas par image).
  **« Appliquer pour de bon »** : remplace les traits de tous les dessins du calque, rendu rapide puis exact en arrière-plan,
  une étape nommée annulable (structure + dessins), l'effet est retiré.
- ✅ **Préréglages** : Encre de Chine, Feutre usé, Contour blanc, Arc-en-ciel, Plume calligraphique (une plume plate inclinée de
  SHOT, toujours disponible).
- ✅ **Carte graphique : pas de passe** (décision documentée, `docs/PERFORMANCE.md` §14) : une dilatation par un disque n'est
  pas séparable, et rester à moins de deux niveaux du processeur demandait le même algorithme des deux côtés (propagation de
  distances en plusieurs passes) ; avec 19 ms (2 000 traits) à 77 ms (4K) en calcul de fond, le gain ne justifiait pas la
  deuxième implémentation. Une pile qui contient le liseré ou l'encre reste un calcul de fond ; le dégradé, le trait sec et la
  plume passent d'abord (`hybrid_split`) et la carte fait le reste.
- ✅ **Format** : `project::VERSION` 31 → **32** ; les projets d'avant s'ouvrent tels quels, une version précédente refuse le projet.
- ✅ **Essais** (`fxmatiere_tests.rs`, 30 dont deux à lancer à la main) : distance exacte contre force brute, liseré (rayon exact,
  couleur, opacité, douceur, derrière / anneau, vectoriel = bitmap), bavure (portée, irrégularité, graine, cœur intact, bord
  foncé), trait sec (trous, stries, retirage, bitmap), dégradé (début, fin, tout le dessin, défilement, boucle), plume (tracé,
  pression, couleur, taille, introuvable, rejeu unique, toutes les brosses de la bibliothèque, remplir pour de bon + annuler +
  rétablir), déterminisme (un fil ou plusieurs), réglages extrêmes et cassés, enregistrement et projets d'avant, calque
  bitmap (trois oui, deux grisés), dessin dérivé, écran = export = vignettes, règle de mélange, copier-coller, actions,
  préréglages, panneau Effets dessiné.
- Pas faits : un liseré par trait ou par groupe de traits (il entoure tout le calque) ; un dégradé et une plume sur un
  calque bitmap (aucun trait) ; le trait sec bitmap en stries le long des traits ; des plumes qui suivent des clés (le choix de
  la plume est un choix, il ne s'anime pas) ; les textes et zones remplies ne sont pas des traits (la plume les laisse) ;
  un « animer à l'arrêt » pour le trait sec. Comparaisons avec d'autres logiciels : aucune vérifiée (**à vérifier**).

## Retouches du 2026-10-19 (aides au cadrage, choix au rectangle, tremblement, commentaires) ✅

Quatre retours d'Anthony, branche `retouches` (pas vus à l'écran dans cette séance : à regarder).

- ✅ **Aides au cadrage plus discrètes et au bon endroit** : « un peu trop épaisses, elles doivent
  apparaître au-dessus de la zone de dessin, pas dans les réglages de caméra ». Défaut 1 px et 70 %
  (avant 1,3 px et 85 %), le liseré sombre dessous plus fin. La section « Aides au cadrage » quitte les
  réglages de la caméra : tout est dans le menu **Cadrage** (icône de règle et son nom) de la barre au-dessus
  du dessin, à côté du menu de format, allumé dès qu'une aide, les zones de sécurité ou un format sont
  tracés. Le menu : tiers, grille d'or, spirale + orientation, diagonales, croix, zones de sécurité (et
  leur choix), format (sous-menu), couleur (six pastilles + sélecteur en sous-menu), opacité, épaisseur,
  tout éteindre. Sous-menus et choix en ligne plutôt que listes déroulantes : un clic hors du menu le
  ferme, et une liste déroulante s'ouvre hors de lui. Migration : `[aides_cadrage]` porte `version_aspect`
  (absent = 0) ; à 0, une épaisseur de 1,3 ou une opacité de 0,85 (l'ancien défaut) passe au nouveau, une
  valeur choisie reste (`FramingAids::sanitized`). Le format du projet ne change pas.
- ✅ **Clés de caméra au rectangle** (`camkeys.rs`, `CamRect`) : un glisser qui part d'un espace vide de la
  piste Caméra du montage ou de la rangée Caméra de la timeline trace un rectangle (dessiné) et choisit
  les clés qu'il touche ; Maj ajoute au choix, Ctrl bascule celles qu'il touche, Échap rend le choix
  d'avant, un clic dans le vide lâche le choix. Seule la largeur compte (la piste est basse). Il ne
  gêne pas : un losange pris emmène toujours le choix, la règle et la fin du plan gardent leur geste, et
  le geste ne fait plus déplacer la tête de lecture depuis la piste Caméra. Les clés d'effets n'ont pas
  le rectangle : leurs losanges sont dans les pistes de calques, où le rectangle des clés de dessin existe
  déjà, ce serait deux rectangles pour le même geste.
- ✅ **Tremblement du trait plus facile à trouver** : l'entrée était cachée sur un calque bitmap et absente
  du menu de la timeline. Maintenant : en tête de « Ajouter un effet » (déjà là, il est choisi dans la
  pile), dans le clic droit d'un calque du panneau Calques, du montage **et** de l'en-tête de la timeline
  (« Effets » + la case « Tremblement du trait » à découvert), dans les réglages du pinceau et du crayon sur
  un calque vectoriel (icône en vague), Ctrl+K « tremblement » (ou « boil »). Sur un calque bitmap :
  grisé avec la raison dans la bulle (`fxui::offer`).
- ✅ **Panneau Commentaires sans superposition** : la liste gardait au moins 64 points et l'éditeur était
  collé au bas du panneau ; dans un panneau court la liste passait sous l'éditeur. Maintenant la liste
  (cadre, défilement propre, au moins trois lignes) puis l'éditeur dessous, dans un panneau qui défile s'il
  est plus court que les deux (`list_height`) ; chaque ligne tient sur une ligne, coupée par « … »
  (`one_line`), l'heure et le plan ne se chevauchent plus.

## Copier-coller des effets d'un calque à l'autre (demandé par Anthony le 2026-10-19) ✅

La demande : « J'aimerais qu'on puisse copier-coller un effet d'un calque à un autre. » Branche
`copier-effets` (`src/fxcopie.rs`, essais `fxcopie_tests.rs`). Pas vu à l'écran dans cette séance.

- ✅ **Copier** : l'icône « Copier l'effet » de chaque effet du panneau Effets, son clic droit sur le
  nom, le menu « Copier / coller » de la rangée du haut ; « Copier tous les effets du calque » aussi dans
  « Effets » du clic droit d'un calque (panneau Calques, montage, timeline). On copie la sorte, l'état actif,
  les valeurs et les clés : toute l'animation. Le presse-papiers d'effets est en mémoire (il survit à
  l'ouverture d'un autre projet) et distinct de ceux des clés, des images et des plans.
- ✅ **Coller** : « Coller l'effet » / « Coller les effets » ajoutent à la fin de la pile (comme « Ajouter un
  effet ») ; « Remplacer les effets » remplace toute la pile. Sur le calque courant, ou sur tous les calques
  choisis (Ctrl / Maj + clic) ; une seule étape d'annulation. Les clés gardent leurs images ; celles d'après
  la fin du plan sont laissées de côté (si toutes le sont, l'effet garde fixes les valeurs de sa première
  clé). Le tremblement du trait est laissé de côté sur un calque bitmap, et s'il est déjà sur le calque ; les
  dossiers sont ignorés. La barre d'état dit ce qui est collé où (« Flou gaussien collé sur 3 calques ») et
  ce qui est laissé de côté.
- ✅ **Clavier** : actions `fx.copier`, `fx.copier_tout`, `fx.coller`, `fx.remplacer` (Ctrl+K, raccourcis à
  donner soi-même, aucun par défaut). Ctrl+C / Ctrl+V visent les effets quand le panneau Effets a la main
  (son dernier clic ; cadre d'accent discret) et passent avant les autres presse-papiers. Un clic ailleurs
  rend la main ; « Créer une clé » la rend aux clés d'effets (Ctrl+C les copie comme avant).
  Sans effet dans la pile, Ctrl+C suit les règles d'avant. Les essais ne lisent plus le presse-papiers de
  Windows (`pasteimage::clipboard_image`).

## Outils de montage : rogner les plans (demandé par Anthony le 2026-10-19) ✅

La demande : « J'ai besoin de plus d'outils de montage pour la timeline. Je ne peux pas saisir le
début d'un shot pour le raccourcir en l'état. » Branche `outils-montage` (`src/rognage.rs`,
`src/rognageui.rs`, essais `rognage_tests.rs`). Pas vu à l'écran dans cette séance : à essayer par
Anthony.

- Avant : seule la fin d'un plan se tirait (`set_length`, qui jetait ce qu'on coupait : un plan n'a
  pas d'autre notion d'« entrée / sortie » que les limites de lecture de 13d, qui ne rognent rien),
  les plans suivants suivant d'autant ; on pouvait aussi déplacer, couper, dupliquer, réunir un plan
  et taper sa durée dans la fiche.
- ✅ **Rognage sans rien détruire** (choix de conception) : un plan commence à l'image 0 de ses calques,
  de sa caméra, de ses marques… ; décaler tout cela pour un « point d'entrée » aurait touché tout le
  code. Ce qui sort du plan, au début ou à la fin, va donc dans une **réserve** rangée par numéro de
  plan (`Document::reserves`), et le plan vivant reste complet et cohérent (écran, export, PDF). Elle
  revient en rallongeant le côté. Dans la réserve : dessins, rôles et clés d'effets de chaque calque
  (par numéro de calque), clés de caméra, marques d'image, signets, entrée / sortie et commentaires de
  ces images. L'annulation la rétablit avec le reste (`Structure::reserves`).
- ✅ **Mouvements continus** : rogner pose au bord une clé de caméra (et d'effet) qui garde la valeur de
  la courbe à cet endroit, notée dans la réserve ; elle repart quand le contenu revient, donc rien ne
  s'accumule (essais : rogner puis rendre redonne le plan à l'identique). Avec des adoucis de caméra,
  la forme du mouvement près de la coupure est approchée, comme pour « Couper le plan » (F6).
- ✅ **Gestes** : bord gauche = début, bord droit = fin ; Alt + bord = roll ; Alt dans un plan = glisser
  le contenu (limité à la réserve : rien à faire glisser sans rognage). Ripple par défaut. Aimant
  (`MAGNET`, action `montage.aimanter`) : tête de lecture, coupes, clés de caméra et d'effets, à six
  points d'écran. Clic sur un bord + flèches : une image ; Alt + clic : la coupe. Menu du plan :
  « Durée… » (images, `1:12`, `1s12`, `+n`, `-n`), « Raccourcir le début / la fin jusqu'à la tête de
  lecture », « Rétablir » et « Oublier le contenu rogné ». Mêmes commandes dans le menu Storyboard >
  Plans et dans la règle de la timeline, dont la poignée de fin ne détruit plus rien.
- ✅ **À vérifier** (conventions supposées) : Alt pour le roll et le
  glisser ; la coupe « jusqu'à la tête de lecture » au trait de la tête (le bord gauche de l'image
  courante : l'image sous la tête sort de la fin) ; le premier dessin tenu quand on rallonge le début
  au-delà de la réserve. Le son ne bouge pas (la barre d'état prévient du décalage des plans).
- ✅ **Fichier** : `rognages.json` à côté du manifeste, sans changer `VERSION` (comme
  `commentaires.json`) ; les dessins cachés restent dans le projet. Un calque vidéo dont le début est
  rogné commence avant le plan : `VideoClip::start` devient signé (le manifeste lit toujours les
  anciennes valeurs ; une version d'avant ne lirait pas un début négatif).
- ✅ **Couper / réunir** : couper un plan garde le début caché au premier (la fin cachée est
  oubliée) ; réunir ne garde que le début caché du premier ; dupliquer ou copier un plan n'emporte pas
  sa réserve.
- Pas faits : poignée de début dans la timeline du plan (la grille y est ancrée à l'image 0 : le contenu
  glisserait sous la souris) ; noms d'instance et marques de couleur des dessins cachés (perdus à
  l'enregistrement) ; roll de plusieurs plans choisis ; glisser le contenu sans réserve ; « Retourner
  les plans » ne retourne pas leur contenu caché ; « Changer la cadence » oublie les réserves.

## Sélection sur plusieurs calques (demandée par Anthony le 2026-10-19) ✅

La demande : « Ajoute la possibilité de sélectionner deux calques pour ensuite sélectionner les
deux dessins de chaque calque pour les transformer, les déplacer. » Branche `multi-calques`
(`src/multisel.rs` : le cœur, sans interface ; `src/multitools.rs` : la souris, les commandes,
l'écran ; essais `multisel_tests.rs`). Pas vu à l'écran dans cette séance : à essayer par Anthony.

- ✅ **Choisir des calques** : existait déjà (13b) — Ctrl + clic ajoute / retire, Maj + clic prend tous
  ceux entre le courant et le cliqué, dans le panneau Calques, la colonne du montage et la timeline.
  Ajouté : une marque distincte (⌖ pour le courant, coche pour les autres dans le panneau Calques ;
  un cadre à chaque ligne choisie ailleurs).
- ✅ **Sélectionner** : avec l'outil Sélection et deux calques choisis ou plus, le lasso, le
  rectangle, un clic sur un trait (Maj : ajouter / retirer) et « Tout sélectionner » prennent sur
  **tous les calques choisis** : les traits dans le lasso de chaque calque vectoriel, la zone de chaque
  calque bitmap (s'il y a des pixels dedans), au dessin de l'image courante. Les dossiers ne comptent
  que par leurs calques choisis un à un ; cachés, verrouillés (eux, leur dossier, position verrouillée),
  vidéo, sans dessin à l'image : laissés de côté, et le message dit lesquels et pourquoi.
- ✅ **Un seul geste** : la boîte (`selbox.rs`) est sur la feuille, autour de la réunion des parts ;
  déplacer, échelle, rotation, inclinaison, champs chiffrés, flèches, miroir s'appliquent à tous autour
  du même pivot. Le geste de la feuille `A` devient, dans le repère de chaque calque de transformation
  `T`, `T⁻¹·A·T` : un calque décalé, tourné, agrandi ou en profondeur suit là où on le voit. Suppr,
  copier, couper, coller marchent calque par calque (le collage remet chaque calque sur lui-même).
- ✅ **Sans latence** : une feuille flottante par calque vectoriel (`flottante.rs`, une « place » par
  calque : `slot_ids`) — jamais de rejeu du moteur pendant le geste, le rendu exact arrive d'un fil de
  travail au lâcher ; les zones bitmap soulèvent leurs pixels une fois (tourner deux fois ne floute pas
  deux fois). Mesure (`three_layers_thirty_strokes_move_at_one_ms_per_frame`, `--ignored`) : 3 calques ×
  30 traits texturés, 0,001 ms par image pour poser les trois feuilles et le plan, saisie de 10 ms une
  seule fois, lâcher 0,13 ms, 0 rejeu.
- ✅ **Annulation** : un geste sur N calques = une étape (`Edit::Group`), au nom du geste (« Déplacer »,
  « Mettre à l'échelle », « Tourner », « Incliner », « Retourner », « Supprimer », « Coller »). Une flèche tenue
  = une étape (`History::extend_last`). Annuler rend tous les calques.
- ✅ **Outil Transformer** : avec plusieurs calques choisis (non cachés, non verrouillés), il les
  déplace, les tourne et les agrandit ensemble (un coin, proportions gardées) autour de leur boîte
  commune, en une étape ; Ctrl+R les remet tous en place.
- ✅ **Indications** : « 3 calques, 42 éléments » dans la barre d'état et le panneau Réglages (avec le
  détail calque par calque), ligne d'aide sous le dessin pour les outils Sélection, Transformer et
  Déformer.
- Pas faits / limites : l'outil **Déformer** (enveloppe, perspective) reste sur le calque courant seul
  (il le dit) ; l'étirement par le milieu d'un côté de l'outil Transformer se fait sur un seul calque ;
  « Effacer hors de la sélection » aussi ; les zones bitmap ne vivent qu'avec l'outil Sélection (déjà
  posées à chaque geste, elles s'oublient en changeant d'outil), les traits vectoriels restent choisis ;
  la boîte est l'enveloppe droite des parts sur la feuille (un calque tourné la fait grossir).

## Passe d'interface : simplicité et cohérence (2026-10-19, branche `simplicite`) ✅

Relecture d'ensemble de l'interface (menus, barres, bulles, raccourcis, clic droit) ; le détail, les
tableaux et les propositions sont dans **`docs/INTERFACE.md`**. Pas vu à l'écran dans cette séance.

- ✅ **Raccourcis** : plus aucune touche partagée dans un même contexte, même « en réserve » (Suppr et
  Maj+Suppr volées par deux commandes pas encore faites) ; chaque commande a un bloc dans `keymap.toml` et un
  nom français net ; la palette Ctrl+K ne montre plus les doubles `scene.*`. Aucun raccourci d'Anthony changé.
- ✅ **Vocabulaire** : un glossaire (plan, clé / clé de caméra / clé d'effet, choisir / sélectionner, effet,
  Réglages, Supprimer / Retirer / Effacer, pelure d'oignon, aides au cadrage) appliqué aux étiquettes, aux
  bulles et aux messages ; chemins de menu périmés corrigés.
- ✅ **Bulles et couleurs** : les icônes sans bulle passent par `bulles.rs` ; les couleurs de surimpression
  (poignées, cadres, décor, caméra) sont écrites une fois dans `theme.rs`.
- ✅ **Icônes allumées lisibles** : un bouton d'état allumé dessine son icône avec `theme::sur_accent`
  dans tous les thèmes (`bulles::symbole_etat` / `encre_etat`), les noms en gras des choix aussi (nom
  d'un effet choisi, onglets, modes) ; un essai interdit de l'oublier.
- ✅ **Les trois sortes de clés** (`clescommun.rs`) : même menu du clic droit (Copier, Coller, Coller à la
  tête, Supprimer, Choisir toutes), même règle de choix (clic, Ctrl, Maj, rectangle), Ctrl+A pour les clés de
  caméra et d'effets.
- ✅ **Plusieurs clés de plusieurs calques** (demande d'Anthony) : rectangle sur toutes les rangées couvertes
  (Ctrl bascule, Maj ajoute), Maj + clic en rectangle entre la clé de départ et celle cliquée, collage relatif
  au calque courant (le plus haut calque copié va sur le calque courant, les autres dessous, calques créés
  si besoin), « 12 clés sur 3 calques » dans la barre d'état. **Ctrl+Maj + outil Sélection** sur le canevas :
  prend sur tous les calques du plan et choisit leurs clés dans le montage.
- ✅ **Zoom de la bande sur la tête de lecture** (`zoombande.rs`) : curseur, boutons, raccourcis et Ctrl +
  molette gardent la tête de lecture au même endroit de l'écran (hors vue : d'abord au milieu).
- Proposé (INTERFACE.md, partie 6) : les espaces de travail par tâche (6.1), les raccourcis (6.2) et les barres par défaut plus minces (6.3) sont **faits** (voir « Espaces de travail
  liés aux modes et barres minces » et le jeu de raccourcis, plus bas). Reste aussi : choisir des clés au rectangle sur la timeline
  de l'animation (l'unité y est l'image), un rectangle pour les clés d'effets, les 150 infobulles d'egui
  restantes, « Arrière-plan » / « Fond » / « Propriétés du plan ».

## Importer un scénario (demandé par Anthony le 2026-10-19, branche `import-scenario`) ✅

Fichier > Importer > Un scénario… (`fichier.importer_scenario`, aussi dans Ctrl+K) : un scénario devient des
séquences et des plans, avec leurs textes et une durée estimée. Détail : README, « Importer un scénario ».

- ✅ **Lecture** (`scenario.rs`, `scenariofdx.rs`) : Fountain (intitulés, personnages, parenthèses, V.O.,
  transitions, dialogues simultanés, notes, cimetière, sections, synopsis, centré, éléments forcés, page de
  titre), texte brut par heuristiques (majuscules, « NOM : réplique », indentation, numéros de page d'un PDF
  copié), Final Draft `.fdx` (quick-xml en flux : les espaces entre morceaux de texte comptent), Word `.docx`
  (`zipread.rs` : par styles de scénario s'il y en a, sinon un paragraphe par ligne). Fichier abîmé : une
  erreur en français, jamais un plantage (lecture rattrapée en cas de panique).
- ✅ **Structure** (`scenarioplans.rs`) : intitulé → séquence ; plans par scène (images de storyboard), par
  paragraphe ou par réplique ; action / dialogue (`NOM (V.O.) (jeu) : réplique`, comme le lit le lecteur de
  sons) / notes (notes, transitions, synopsis, sections, décor) ; durée = mots ÷ 2,5 par seconde + 1 seconde par
  ligne de 60 signes, entre 1 et 30 s, réglable ; personnages notés dans les notes du projet.
- ✅ **Fenêtre** (`scenarioui.rs`) : lecture sur un autre fil avec avancement, aperçu (séquences, plans, durée,
  personnages, remarques), destination (nouveau projet, fin du projet, après la séquence du plan courant),
  une seule étape d'annulation « Importer un scénario ».
- Pas fait : le **PDF** (aucun lecteur de PDF hors connexion ; copier le texte dans un `.txt`, ou exporter en
  `.fountain`), `.doc` / `.rtf` / `.odt`, glisser le fichier sur la fenêtre, un bouton sur l'accueil, les
  transitions de montage (autre branche : elles restent des notes), les voix provisoires (la liste des
  personnages est prête pour elles).

## Fiabilité : un filet contre « tous les essais passent, SHOT ne s'ouvre plus » (2026-10-19, branche `fiabilite`) ✅

Le point de départ : le 2026-10-18 SHOT ne s'ouvrait plus (`raster.rs:57`, l'aperçu réduit d'un effet lu comme une
feuille entière) alors que 768 essais passaient ; et sur une machine chargée, la fenêtre est restée noire plus de
14 s au lancement. Détail, chiffres et mode d'emploi : `docs/FIABILITE.md`.

- ✅ **Projet d'essai complet** (`fiabilite_fixture.rs`), aller-retour sur trois chemins d'enregistrement et par
  l'application, comparé champ par champ : aucun défaut d'enregistrement trouvé, mais le filet est là.
- ✅ **Fumée de toute l'application** sans fenêtre (195 images, GPU et processeur, annuler / rétablir) et incident du
  2026-10-18 refait (vignettes pendant le calcul des effets).
- ✅ **Fichiers abîmés** et essais à la louche (troncatures, valeurs changées au hasard, PNG aux bits changés). Quatre
  défauts réels corrigés : `Raster::get` et `composite_tile_into` hors du dessin ; **perte de données** (annexes, sons,
  modèles illisibles effacés à l'enregistrement) ; manifeste (message, feuille démesurée) ; dessin vectoriel à
  coordonnées absurdes (ouverture sans fin) et PNG à l'en-tête démesuré (arrêt brutal).
- ✅ **Démarrage** : lecture des pinceaux importés et présence du dernier projet sortis de la première image (jusqu'à
  3 695 → 324 ms sous charge) ; minutage dans `demarrage.log` (`SHOT_DEMARRAGE=1`).
- ✅ **Essais déterministes** : dossiers de données isolés par fil, état commun des effets en lecteurs / écrivains
  (`fxcache::monde`), attentes sur des conditions et horloge d'aperçu qu'on avance ; `scripts/verifier.ps1 -Repetitions N`.
- Pas fait : la première image dans la vraie fenêtre (eframe, carte graphique, pipelines : à lire dans `demarrage.log`) ;
  `BrushLibrary::builtin()` (74 ms) reste avant la première image ; un son ou un modèle illisible sort du projet et ses
  extraits avec lui à l'enregistrement.

## Raccourcis : un seul jeu (2026-10-19, simplifié le 2026-10-30)

Le choix entre deux jeux de raccourcis (SHOT et un jeu calqué sur un autre logiciel) a existé du 2026-10-19 au
2026-10-30 ; il est retiré (`docs/INTERFACE.md`, 6.2). Il reste `keymap.toml` (SHOT) et le fichier personnel, qui
joue par-dessus commande par commande ; un ancien réglage `jeu_raccourcis` est ignoré à la lecture.

## Transitions, effets sonores et bruitages (demandés par Anthony le 2026-10-19, branche `transitions-son`) ✅

Trois chantiers du montage ; le détail est dans le README (sections « Transitions du montage », « Effets
sonores », « Bruitages ») et le vocabulaire dans `docs/INTERFACE.md` (partie 8). **Rien n'a été regardé à l'écran dans
cette séance** (consigne : aucun lancement de SHOT) ; la liste « à vérifier » est en tête de la partie 8.

- ✅ **Transitions** (`transitions.rs`, `transitionsui.rs`) : fondu enchaîné, fondu au noir, fondu au blanc, volet,
  noir, sur une coupe ou au bord du film. Durée de 12 images, tirée aux bords du nœud ou tapée ; calage centré /
  finit à la coupe / commence à la coupe. **Temps : la durée du film ne change pas** (le son reste calé) ; les images
  qui manquent d'un côté sont tenues. Lecture sur la carte graphique (second plan composé à part, posé par
  l'affichage), export exact (mélange sur le processeur, image par image). `transitions.json`, sans nouvelle
  version du projet.
- ✅ **Effets sonores** (`audiofx.rs`, `audiofxui.rs`) : fondus par extrait (poignées aux coins du haut), fondu
  enchaîné de deux extraits qui se touchent (puissance égale, poignées de son de part et d'autre de la coupe),
  volume par extrait et par piste, solo, clés de volume (double-clic, glisser, Suppr), égaliseur trois bandes et
  écho, sans nouvelle bibliothèque. Un seul calcul (`build_mix`) pour la lecture, le défilement, le WAV, la vidéo
  ; le travail lourd hors des fils de l'interface et de la carte son.
  `effets_son.json`, sans nouvelle version du projet.
- ✅ **Bruitages** (`bruitages.rs`, `bruitagesui.rs`) : panneau fermé par défaut (`Pane::Bruitages`, icône de la
  barre des panneaux, `vue.bruitages`), dossiers de l'utilisateur dans `reglages.toml`, parcours et durées sur
  des fils à part, écoute au clic, glisser vers une piste du montage ou de la timeline, double-clic à la tête de
  lecture, son copié dans le projet.
- Pas fait / limites : les poignées du rognage (la réserve) ne fournissent pas d'images vivantes aux transitions
  (images tenues) ; pas d'aperçu des transitions sans carte graphique (l'export, lui, les contient) ; le volet de
  l'écran avance sur la largeur du cadre par défaut, pas du cadre d'une caméra qui bouge (l'export est exact) ;
  les fondus enchaînés entre extraits ne partent pas ailleurs ; l'égaliseur et l'écho refont le son
  de l'extrait entier (un son très long a un petit délai avant d'être entendu avec ses effets) ; un chevauchement
  d'extraits sur une même piste reste refusé (le fondu enchaîné passe par les poignées de son).
- À vérifier face aux logiciels de montage : le calage centré par défaut, le fondu au noir
  qui passe par la couleur sur une coupe, la courbe linéaire des fondus d'entrée et de sortie, la puissance
  égale du fondu enchaîné, le solo qui l'emporte sur le muet.

## Génération sonore à partir du storyboard, phase 1 (demandée par Anthony le 2026-10-19, branche `generation-son`)

SHOT lit le storyboard et propose des sons témoins (bruitages, ambiances, musique d'intention, voix des
dialogues) que l'on accepte, régénère ou supprime. Le détail, le contrat du serveur local et la liste des
téléchargements de la phase 2 sont dans `docs/GENERATION_SON.md` ; le vocabulaire et le « à vérifier à l'écran » dans
`docs/INTERFACE.md` (partie 9). **Rien n'a été regardé à l'écran** (consigne : aucun lancement de SHOT) ; aucun
téléchargement, aucun modèle, aucune requête réseau.

- ✅ **Le modèle et le fichier** (`indications.rs`) : proposition (plan, image, durée, sorte, description française,
  prompt anglais, réplique / personnage / voix, état, fichier, extrait posé), `sons_proposes.json` sans changer la version
  du projet, mêmes protections que les autres fichiers annexes (illisible ou plus récent jamais effacé, `.illisible`,
  écriture atomique), fusion d'une relecture (ne ramène pas une proposition rejetée), état « accepté » qui suit le son posé.
- ✅ **La lecture** : l'entrée commune (vignette PNG, action / dialogue / notes de la fiche et des images de storyboard,
  durée, transitions, commentaires) pour tous les fournisseurs (`lecturesb.rs`) ; **lecture par règles sans modèle**
  (`lecturerules.rs`) : répliques → voix, mots-clés → bruitages / ambiances / musique, consignes « SFX : … » reprises.
- ✅ **Les fournisseurs** : trois traits (`LecteurStoryboard`, `GenerateurSon`, `GenerateurVoix`), annulation et avancement
  (`Controle`), tâches sur des fils à part (`generation.rs`) ; client du serveur local en HTTP/1.1 minimal de boucle locale
  (`httplocal.rs`, `sidecar.rs`) ; requêtes et réponses de Claude, ElevenLabs et Stable Audio (`fournisseursapi.rs`).
- ✅ **Le panneau** `Pane::SuggestionsSonores` (fermé par défaut, icône de la barre, `vue.suggestions_sonores`,
  `suggestion.lire`) ; accepter pose le son à la bonne image sur la piste « Sons proposés » (annulable) ; confirmation
  d'envoi en ligne une fois par projet ; Préférences > Génération sonore (fournisseur de chaque rôle, port local,
  « Tester », clés, « Tester la connexion »).
- ✅ **Les clés** dans le Gestionnaire d'identifiants de Windows (`clessecretes.rs`), jamais dans un fichier.
- ✅ **Phase 2 (branche `sidecar-son`)** : le **serveur local** `sidecar/serveur.py` (FastAPI, `127.0.0.1`, le contrat du §6)
  avec Kokoro (voix) et Qwen2.5-VL AWQ chargé sans `gptqmodel` (`sidecar/awq.py`) ; lancé, attendu et arrêté par SHOT
  (`serveurlocal.rs`, sans console, objet de travail Windows) ; Préférences : dossier des modèles, Python, Démarrer / Arrêter /
  Tester, démarrage automatique. Essayé de bout en bout (voix → prête → accepter ; lecture Qwen). Mesures et manques dans
  `docs/GENERATION_SON.md` §8.
- **Pas fait / limites** : **Stable Audio Open n'est pas téléchargé** (licence à accepter par l'utilisateur, `t5-base` et
  `stable-audio-tools` aussi) : pas de bruitage généré ; Qwen lit à ~4 jetons/s (pas de noyau AWQ) ; l'**envoi HTTPS
  aux services en ligne est « à brancher »** (aucune caisse TLS hors ligne ; piste : WinHTTP déclaré à la main) ; les
  adresses et formats des services en ligne sont à vérifier avant le premier appel ; le cache des sons générés n'est pas
  nettoyé ; une seule génération à la fois ; pas de voix par personnage retenue d'un plan à l'autre (la voix se
  choisit par proposition ou dans les Préférences) ; la lecture par règles ne connaît que le français.
- **À vérifier** : les licences de chaque modèle et de chaque service (usage commercial des sons, données envoyées).

## Vignettes simples : la planche de crayonnés disparaît dans les Vignettes (Anthony, 2026-10-21, branche `vignettes-simples`) ✅

Remplace « Crayonnés rapides : la planche » (2026-10-19), supprimée : elle et la vue Vignettes faisaient deux fois le même
travail. Décisions prises sur des maquettes interactives. Mode d'emploi : README, « Vignettes simples » ; gestes, mots et
liste à vérifier à l'écran : `docs/INTERFACE.md`, partie 11.

- ✅ **La planche de crayonnés est retirée** : panneau, menu Storyboard, bouton de la barre du montage, onze commandes
  `crayonnes.*` et leurs blocs de `keymap.toml`, état dans les onglets, champ du document, écriture dans le projet (y
  compris le projet partagé), 38 essais, part du projet d'essai de fiabilité, `Document::insert_plans`. **Anciens
  projets** : `crayonnes.json` n'est plus lu ni écrit mais **jamais effacé** ; un avertissement, une fois à l'ouverture,
  dit combien de cases dessinées n'avaient pas servi (`crayonnesheritage.rs`). Choix : ne rien convertir tout seul (un
  croquis qui revient dans le film surprendrait) ; ce qu'on peut perdre : « Enregistrer sous » ne recopie pas le fichier.
- ✅ **Dessin direct** : un trait qui commence dans une image (pleine ou vide) la rend courante à l'appui et dessine dedans,
  sans clic avant, avec le même moteur que la vue Dessin (mesuré sans carte graphique : un trait de 60 points, image
  d'écran moyenne 0,9 ms dans une vignette, 1,2 ms dans la vue Dessin) ; un trait qui commence ailleurs ne trace rien.
  Ctrl / Maj + appui choisit des plans. Le trait coupé à l'affichage au bord de l'image, le dessin garde tout.
- ✅ **Cases vides** après la dernière image : dessiner dans l'une fait un plan à l'appui (les cases sautées deviennent des plans
  vides), ou une image de plus du dernier plan si la barre devant est grise ; création et trait = une étape d'annulation ;
  un clic sans trait ne laisse rien.
- ✅ **Épuré** (consigne d'Anthony : « rien de distraction, que du dessin ») : réglages petits et pâles (30 %) jusqu'à la souris, plus de pastilles d'envoi ni de film partagé, plus de compteurs ; « Attacher les notes » au clic droit sur le fond.
- ✅ **Look simple** : filet d'un point (plus foncé pour la courante), plus de bande de titre, de durée ni de « Plan » : seul le
  numéro du plan au-dessus de sa première image.
- ✅ **Barres** grise (même plan) / noire (coupe), un tiers de la hauteur, 1,5 point, aussi au bout de chaque ligne ; un clic
  coupe (`split_plan`) ou réunit (`join_next_plan`), une étape nommée ; écart de 10 / 30 points ; lignes à nombre fixe d'images.
- ✅ **Bandes de note** sous chaque image : notes du plan ou de la marque d'image, 11 → 10 → 9,5 points selon la longueur,
  trois lignes puis « … », la ligne suivante descend juste de ce qu'il faut ; saisie Entrée / Maj + Entrée / Échap ; une étape.
- Pas fait : réordonner en glissant une vignette (le glisser est celui du trait ; le montage et couper / coller restent) ;
  le trait qui déborde visible hors de l'image pendant qu'on le trace (il faudrait la toile infinie dans la vignette) ;
  l'étiquette de version « · V2 » dans les Vignettes (au clic droit) ; vérification à l'écran et à la tablette.
## Calques d'effets et préréglages d'effets (demandés par Anthony le 2026-10-19, branche `calques-effets`) ✅

« Création de calques d'effets après des préréglages d'animation d'effet comme des impact frames, glows, tremblements
de caméra effectifs, timecode… ». Mode d'emploi : README (section du même nom) ; vocabulaire : `docs/INTERFACE.md`,
partie 12 ; mesures : `docs/PERFORMANCE.md`, partie 10.

- ✅ **Calque d'effet** (`calqueffet.rs`, `Layer::effet`) : un calque de la pile sans dessin. **Ajustement** : sa pile
  d'effets (clés comprises) agit sur tout ce qui est dessous dans le plan, papier compris, ou dans son dossier ;
  aux images où il a une exposition (un dessin vide tenu : la timeline l'étire et le coupe comme un dessin) ; œil,
  opacité et mode de fusion du calque. **Incrustation** (`incrustation.rs`) : timecode HH:MM:SS:IM à la cadence du
  projet (entière), numéro d'image, nom du plan ou de la séquence, titre, date, texte libre ; neuf places, taille,
  couleur, boîte, « Seulement à l'écran / à l'export » ; gravée dans l'export, tracée avec les contours de l'outil Texte.
- ✅ **Rendu** : le plan porte un repère (`FondPart::Reglage`) que l'écran remplace, avec ce qu'il couvre, par un dessin
  dérivé (composition d'en dessous → effets → fusion), demandé au fil de fond (`fxcache::Job::base`) et jamais calculé
  sur le fil de l'interface ; calcul bloquant et exact à l'export ; carte graphique et processeur composent le même
  dessin. Aperçu d'un calque d'effet limité à 1 million de pixels (feuille 4K : le quart de la résolution).
- ✅ **Sept effets « Cinéma »** (`fxcine.rs`) dans la pile d'effets (aussi sur un calque de dessin) : Flash, Image
  d'impact (négatif, noir et blanc, postérisé, teinte, éclat de rayons et lignes de vitesse par graine), Lueur diffuse,
  Secousse de l'image (décalage, rotation, zoom, flou de bougé), Vignettage, Grain de film (change à chaque image),
  Aberration chromatique.
- ✅ **Dix préréglages** (`prereglages.rs`, `prereglagesui.rs`) : image d'impact, lueur (pulsation), flash blanc, flash de
  couleur, tremblement de l'image (mêmes générateurs de bruit que les effets de caméra, bornés par l'intensité), vignettage,
  grain, aberration chromatique, flou de vitesse, timecode et incrustations. Une fenêtre par préréglage (intensité,
  durée, couleur, graine, réglages propres) avec **aperçu en direct** sur le dessin, une étape d'annulation aux noms
  français, « sur le calque courant » ou « nouveau calque d'effet », « dans tous les plans » pour ce qui dure jusqu'à la fin.
- ✅ **Où** : bouton étincelle du panneau Calques, Calques > Nouveau calque > Calque d'effet (et clics droits), « Préréglages »
  du panneau Effets, Ctrl+K (13 actions `effet.*`, sans touche). Étincelle et cyan dans le panneau Calques, la timeline
  et le montage.
- ✅ **Format 27** (voir `project.rs`) : `calque_effet` d'un calque ; sept sortes d'effets de plus. Anciens projets lus
  tels quels. Projet d'essai de la fiabilité complété (un ajustement à clés, une incrustation réglée à fond).
- Vérifié par 64 essais de plus (`fxcine_tests`, `calqueffet_tests`, `incrustation_tests`, `prereglages_tests`,
  `prereglagesui_tests`) et deux mesures ignorées (`effetsperf_tests`) : 51 images/s au fil de fond en aperçu, 0,00 ms
  sur le fil de l'interface.
- Pas fait / à décider : calculer les effets dans le seul cadre de la caméra (gain d'un facteur 3 à 4 à l'export) ; fusionner
  les calques avec un calque d'effet (refusé, avec un message) ; incrustations dans le PDF, le HTML et les vignettes ;
  choix de la police de l'incrustation dans l'interface (le champ existe et s'enregistre) ; sélection flottante sous un
  calque d'effet (l'effet attend la fin du geste). À vérifier à l'écran : voir `docs/INTERFACE.md`, partie 12.

## Scène 3D de blocage (demandée par Anthony le 2026-10-19, branche `scene3d`) ✅ — devenue l'Espace 3D, puis **retirée le 2026-10-21** (objets 3D supprimés : voir la dernière partie)

Le choix d'Anthony : **blocage simple** — des formes (cube, plan, cylindre, sphère, escalier), les déplacer, les
tourner, les mettre à l'échelle ; la caméra du plan dans la scène ; des silhouettes de personnages ; puis dessiner
par-dessus. Pas d'édition de sommets. Vue **Scène 3D** (bouton de la barre au-dessus du dessin, menu Affichage >
Plateau, action `scene3d.basculer`), réglages dans le panneau Réglages. Code : `scene3d*.rs` (huit fichiers), essais
`scene3d_tests.rs`. Pas vu sur l'écran d'Anthony : captures faites avec le banc d'essai (voir plus bas), à regarder.

- ✅ **Le modèle** (`scene3d.rs`). Chaque **plan** a sa scène : ses objets (personnages, accessoires), sa caméra du
  plateau, sa lumière, son style de rendu. Chaque **séquence** peut avoir un **décor** : une liste d'objets que tous
  ses plans montrent (case « Montrer le décor de la séquence » ; le plan qu'on modifie estompe le décor, et
  « Modifier : le décor » inverse). On place un décor une fois ; chaque plan y pose ses personnages et sa caméra ;
  changer le décor le change dans tous les plans qui l'utilisent. Un modèle importé (`.pelure3d`, glTF) est un objet
  de plus (`Shape3d::Model`).
- ✅ **Formes** : cube, plan (sol ou mur), cylindre, sphère, cône, escalier (nombre de marches réglable, 2 à 40),
  **mannequin** (capsules articulées, 10 articulations : colonne, tête, épaules, coudes, hanches, genoux ; taille
  réglable ; poses *debout, assis, marche, bras levés* ; une articulation touchée rend la pose « libre » ; les pieds
  sont toujours au sol). Mètres, Y vers le haut, un personnage regarde vers +Z. Nom, couleur, ombrage gris par objet,
  cacher, verrouiller, grouper (un groupe se choisit et se manie d'un bloc), dupliquer, supprimer.
- ✅ **Vue d'édition sur la carte graphique** (`scene3dgpu.rs`, wgpu) : un appel de dessin par objet sur un maillage
  gardé dans la carte (déplacer un objet n'envoie qu'une matrice de 176 octets), tampon de profondeur, lissage 4×,
  coque de contour, arêtes vives, sol quadrillé, une lumière réglable (direction, force, ambiante). Tourner : clic
  droit ou Alt + glisser ; glisser la vue : bouton du milieu ; avancer : molette ; **Cadrer**.
- ✅ **Manipulateurs** (`scene3dgizmo.rs`) : Choisir, Déplacer (flèches et carrés de plan), Tourner (trois cercles),
  Échelle (cubes d'axe et cube central, uniforme si plusieurs objets). Repère du monde ou de l'objet. Un objet se
  tire directement sur le sol (Maj : en hauteur). Boîte de sélection en pointillés avec ses petits carrés d'angle,
  comme celle du dessin. **Aimantation** : grille (pas réglable, 0,25 m), rotations par paliers (15°), tailles, et
  pose sur le sol quand l'objet en est à moins de 15 cm. Champs numériques (position, rotation, taille, articulation)
  dans les Réglages. **Annulation** : chaque geste est une étape au nom français (« Déplacer dans la scène 3D »,
  « Ajouter : mannequin », « Grouper des objets 3D »…).
- ✅ **Caméra du plateau** (`SceneCam`) : une caméra posée dans la scène, avec sa pyramide de vision tracée ; on la
  choisit et on la déplace ou la tourne avec les mêmes poignées ; **Vue par la caméra** montre ce qu'elle filme au
  format du cadre (et déplace alors la caméra elle-même : milieu = glisser, clic droit = tourner, molette = avancer).
  Focale en mm sur une pellicule de 36 mm, avec la liste des objectifs de la caméra du plan (`FOCAL_PRESETS`), même
  angle de champ que `camera::angle_of_view`. **Choix : une caméra à part, liée à la caméra 2D par une conversion
  exacte, pas fusionnée avec elle.** Convention : le plan z = 0 de la scène est la feuille (le plan de référence) et
  le cadre par défaut y mesure 6 m de large (`PLATE_WIDTH_M`) ; « Caler sur la caméra du plan » (`SceneCam::
  from_plan_camera`) pose la scène-caméra là où la caméra 2D est à l'image courante (position, focale, panoramique,
  bascule, rotation) — essai : les coins du cadre vus par l'une retombent sur ceux de l'autre, caméras droite,
  zoomée, avancée, tournée, basculée, en panoramique. Pourquoi à part : la caméra 2D bouge pendant le plan (clés,
  adoucis) au-dessus d'un fond qui, lui, est une image fixe ; fusionner les deux ferait appliquer le mouvement deux
  fois. **Clés de caméra en 3D : pas faites** (la caméra du plateau est fixe pour tout le plan ; les clés de la
  caméra 2D continuent de la faire bouger par-dessus le fond rendu). À reprendre si Anthony veut des travellings
  filmés dans la scène : il faudrait rendre une image par image, ou la scène par la caméra 2D.
- ✅ **Dessiner par-dessus** (`scene3drender.rs`) : « Envoyer vers un calque de fond » rend la scène par la caméra du
  plateau dans un calque bitmap « Fond 3D », **tout en bas du plan et verrouillé**, à la taille de la feuille (le cadre
  seul, ou toute la feuille au choix : la caméra 2D peut alors s'éloigner sans sortir du rendu). Styles : couleurs,
  **niveaux de gris**, **contours** (aplats clairs et traits noirs sur silhouettes, cassures et profondeurs). Le
  calque suit la scène (« Suivre ») : rendu sur un **autre fil** (la moitié des cœurs, sur-échantillonnage 2 à 3), posé
  seulement quand on ne dessine pas (ni trait, ni geste dans la vue 3D, ni lecture), jeté s'il est périmé. Rien ne
  tourne au repos et aucune image n'est demandée. C'est un calque ordinaire : il s'enregistre en PNG, s'exporte, se
  copie. Le rendu vient du processeur (déterministe, indépendant de la carte graphique) avec les mêmes formules que la
  vue d'édition (essai : mêmes silhouettes à moins de 0,5 %).
- ✅ **Enregistrement** : `scenes3d.json` à côté du manifeste (`scene3dstore.rs`), écrit par le même `SaveJob`
  (atomique, sur le disque), **sans changer `project::VERSION`** (resté à 26) : une version qui ne le connaît pas ouvre
  le projet sans la scène, le calque « Fond 3D » étant un dessin ordinaire — au lieu de refuser le projet. Protections
  de `docs/FIABILITE.md` : fichier d'une version plus récente ou abîmé = avertissement, jamais effacé, mis de côté en
  `.illisible` si une scène neuve est à écrire ; valeurs bornées à la lecture (nombre d'objets, marches, tailles,
  articulations, numéros doubles renumérotés). Les modèles importés qu'une scène montre sont gardés dans `modeles/`.
  Le projet d'essai de la fiabilité contient une scène (aller-retour par les trois chemins d'enregistrement, fumée,
  fichiers coupés et mutés).
- ✅ **Essais** : 38 (`scene3d_tests.rs`, dont un qui passe par la carte graphique de la machine, sauté s'il n'y en a
  pas) et la suite complète passe (1114 essais, `scripts\verifier.ps1 -Repetitions 2` : OK, compilation sans
  avertissement) : nombre de sommets et normales des formes, mannequin (hauteur, pieds au sol,
  poses, articulations), transformations et manipulateurs, aimantation, caméra et cadrage face à la caméra 2D,
  taille et alignement du rendu vers un calque, aller-retour et protections du fichier, annulation, application sans
  fenêtre (envoi, suivi, attente quand on est occupé, repos), carte graphique contre processeur. Un banc `#[ignore]`
  (`perf_100_primitives`) et des captures (`images_of_the_scene_views`, `SHOT_SCENE3D_IMAGE=<dossier>`).
- ✅ **Mesures** (`perf_100_primitives`, 100 formes de toutes sortes, carte graphique de la machine de développement) :
  vue d'édition **1,9 ms** par image d'application entière (3,7 ms au plus), repos atteint en 6 images ; maillages
  construits en 0,5 ms ; rendu vers un calque 1920 × 1080 en **102 ms** (sur-échantillonnage 2), toute la feuille
  3840 × 2160 en **202 ms**, sur un autre fil.
- Pas fait : pas d'édition de sommets ni de modèle à plusieurs matières ; pas de clés de caméra 3D (voir plus haut) ;
  les scènes ne suivent pas un plan qu'on duplique, coupe ou assemble (la scène reste au plan d'origine) ; pas
  d'ombres portées ni de textures ; pas de poses animées du mannequin ; un modèle importé de plus de 400 000 triangles
  s'affiche en boîte dans la vue d'édition (le rendu vers un calque le trace en entier) ; pas de raccourci par défaut
  (aucune touche choisie ; W / E / R pour déplacer / tourner / échelle sont l'usage de plusieurs logiciels 3D, **à
  vérifier** avant de les adopter).
- Conventions empruntées à d'autres logiciels, **à vérifier** : rouge / vert / bleu pour X / Y / Z ; flèches pour
  déplacer, cercles pour tourner, cubes pour l'échelle ; tourner la vue au clic droit et la glisser au bouton du
  milieu (comme le plateau en 3D) ; Alt + glisser pour tourner ; rotation en degrés dans l'ordre lacet, tangage,
  roulis (Y, X, Z) ; focale équivalente sur pellicule de 36 mm (celle de la caméra 2D).

## Espaces de travail liés aux modes et barres minces (demandés par Anthony le 2026-10-19, branche `espaces-barres`) ✅

Les propositions 6.1 et 6.3 de `docs/INTERFACE.md`, faites ; détail dans la partie 14 de ce fichier. Pas vu à
l'écran dans cette séance.

- ✅ **Espaces de travail Storyboard, Animation, Montage** (`workspace::Espace`) : chacun retient sa disposition de
  panneaux ; changer de mode pose celle de l'espace d'arrivée, instantanément ; « Rétablir la disposition »
  (`espace.retablir`) remet celle d'origine de l'espace en cours. Montage est une disposition du mode Storyboard (pas un
  troisième mode) : la bande en grand, la fiche du plan, Bruitages et Suggestions sonores à droite.
- ✅ **Réglages** (`reglages.toml`) : `disposition` (Storyboard, c'est l'ancien champ : un ancien fichier est repris sans
  conversion), `disposition_animation`, `disposition_montage`, `espace_storyboard`, tous avec une valeur d'origine.
- ✅ **Compatible avec l'existant** : panneaux qui s'ajustent (`ajuste.rs`, ils repartent de la disposition posée),
  fenêtres détachées (elles restent où elles sont), feuille d'exposition (animation seule), cadres de composition et
  planche (réglages de la vue, pas d'un espace).
- ✅ **Barres minces** (`barres.rs`) : barre de commandes 19 → 9 icônes, montage 21 → 12, timeline sans défilement ;
  le reste sous « Plus… » (`DOTS_THREE`), chaque ligne avec icône, nom et raccourci ; épingle « Épingler dans la barre »
  par ligne, retenue par barre (`[barres.*]`) ; une barre trop étroite envoie d'elle-même ses dernières commandes
  sous « Plus… ». Rien n'est retiré (menus, Ctrl+K, raccourcis).
- ✅ **Arrivées de main rangées** : « Ajouter une transition » et « Importer une vidéo » sous « Plus… » du montage, la
  planche de crayonnés (supprimée le 2026-10-21) dans la barre à côté de « Vue vignettes » ; le cube de la scène 3D reste dans la barre de la vue.
- ✅ **Correction** : Historique, Commentaires, Bruitages, Suggestions sonores et Références s'ouvrent aussi quand les
  calques sont fermés (la colonne de droite se crée avec eux).
- ✅ **Essais** : 23 nouveaux (`barres_tests.rs`, `workspace_tests.rs`), la suite complète passe (1511 essais,
  `scripts\verifier.ps1 -Repetitions 2` : OK, compilation sans avertissement).
- Pas fait : un réglage « barres complètes » (épingler tout y revient) ; miroir et table lumineuse sous « Plus… » dans
  la barre sous le dessin (proposition 4 de la partie 5).

## Le troisième onglet Montage (demandé par Anthony le 2026-10-21, branche `espace-montage`) ✅

« En haut à droite, à côté de Storyboard et Animation, un troisième onglet Montage, avec une organisation des panneaux
propre au travail de montage. » Détail et liste de contrôle : `docs/INTERFACE.md` partie 14, « Le troisième onglet
Montage ». Pas vu à l'écran dans cette séance.

- ✅ **Trois onglets** Storyboard · Animation · Montage, en haut à droite, bulle à 1 s, **Alt+M** pour le Montage ; changer
  est instantané et ne touche pas au projet (plan, image, plans choisis). **Tab** depuis le Montage revient au Storyboard.
- ✅ **Montage = une disposition du mode Storyboard** (pas un troisième `Mode`) : mêmes outils et raccourcis.
- ✅ **Disposition propre au montage** : Lecteur en grand (vue caméra) en haut, bande à 440 avec toutes les pistes en
  bas, Fiche du plan et Bruitages / Suggestions sonores / Commentaires / Historique à droite, sans outils ni
  pinceaux ni calques ni effets ni barre verticale. Retenue, rétablissable, enregistrable comme les autres espaces.
- ✅ **Barre du Montage** (`Barre::MontageEdition`) : coupes, rognage, durée, avancer / reculer, aimant, transitions,
  sons, bruitages, pistes, relecture et exports visibles d'office ; le reste sous « Plus… » ; épingles à elle.
- Pas fait : des panneaux Son et Transitions à part (ils se règlent dans la bande) ; un lecteur qui n'est pas le dessin.

## Onglet Vignettes et mouvements de caméra dans les cases (demandés par Anthony le 2026-10-21, branche `vignettes-onglet-camera`) ✅

« La partie Vignettes doit être la plus épurée possible, rien de distraction, que du dessin et des références visuelles
éventuellement. » Détail et liste de contrôle : `docs/INTERFACE.md`, parties 11 (« Mouvements de caméra dessinés dans les
cases ») et 14 (« Le quatrième onglet Vignettes »). Pas vu à l'écran dans cette séance.

- ✅ **Quatrième onglet Vignettes**, avant Storyboard (Vignettes · Storyboard · Animation · Montage), Alt+Maj+V, bulle à 1 s,
  clic droit « assigner un raccourci », changement instantané qui ne touche pas au projet. Une disposition du mode Storyboard
  (comme le Montage) ; Tab en revient au Storyboard.
- ✅ **Disposition épurée** (`DockLayout::pour_les_vignettes`) : la planche, réglages du pinceau, couleur + pinceaux en onglets,
  barre des personnages au-dessus de la planche, colonne d'outils minimale (`TOOL_GROUPS_VIGNETTES`) ; retenue, rétablissable,
  enregistrable (`disposition_vignettes` dans les réglages).
- ✅ **Ctrl + glisser un cadre** : A reste, B naît (clés A au début, B à la fin du plan), C, D… réparties, coin = zoom, bord =
  déplacer la clé ; une étape d'annulation nommée par geste ; Échap abandonne ; Ctrl + clic sans bouger choisit toujours des plans.
- ✅ **La case grandit** (`vignettecam.rs`, `board.rs`) : taille déduite des clés (rien d'enregistré), cases entières, rangée
  de cases mise en page par `layout_with` (passage tout entier à la rangée ou page suivante), dessin sur toute la zone y compris
  hors de la feuille, vignette de zone sur un fil de fond (`Thumbs::texture_zone`).
- Limites : au plus 3 cases de haut ; les calques en profondeur (multiplan) de la grande case gardent leur place vue de A ;
  un plan d'une seule image ne peut pas porter de mouvement ; une caméra inclinée ou panoramiquée est comptée par son
  rectangle englobant (la zone n'est pas inclinée).

## Envois, versions de plans et comparaison (demandés par Anthony le 2026-10-20, branche `versions`) ✅

Le plus grand souci d'un storyboardeur : savoir ce qui a changé entre deux livraisons. Quatre morceaux, un commit chacun
(détail : `README.md`, « Envois, plans modifiés, versions et comparaison » ; `docs/INTERFACE.md`, partie 15 ; format :
`docs/ARCHITECTURE.md`). Le format du projet ne change pas (27). Rien n'a été regardé à l'écran dans cette séance
(aucun lancement de SHOT) : voir « À vérifier à l'écran ».

- ✅ **Envois** (`empreinte.rs`, `envois.rs`, `envoisui.rs`) : « Marquer un envoi… » (nom + date, « v2 au réal ») note
  l'empreinte du contenu de chaque plan et un instantané pour le revoir (`envois.json`, `versions/`). Un dessin qui n'a pas
  changé entre deux envois n'est écrit qu'une fois (lien dur depuis `dessins/` quand le disque le permet). Écrit par
  `prepare_save` / `SaveJob` / `write_sidecar`, avec les protections de `docs/FIABILITE.md` (rien d'effacé, `.illisible`).
- ✅ **Plans modifiés depuis l'envoi** : pastille ambre sur le bloc du montage et la vignette de la planche, bulle « Modifié
  depuis l'envoi « v2 au réal » », « Plan modifié suivant / précédent », « Choisir les plans modifiés ». Relevé par image
  d'écran = 0,4 à 0,6 µs pour 1 200 dessins ; copie par tranches de 1,5 ms ; un fil hache ; rien sans envoi.
- ✅ **Versions d'un plan** (`planvers.rs`, `planversui.rs`) : « Nouvelle version du plan », passer, restaurer, supprimer,
  chaque geste une étape d'annulation nommée ; ligne « Version » de la fiche, sous-menu du clic droit, « Plan 3 · v2 »,
  `plans_versions.json`.
- ✅ **Comparer** (`comparer.rs`, `comparerui.rs`) : deux versions, ou aujourd'hui et un envoi ; côte à côte, volet, fondu,
  superposition rouge / bleu ; part de l'image qui diffère ; lecture seule ; sur un fil de fond.
- ✅ **Essais** : 57 nouveaux (`envois_tests.rs` 28, `planvers_tests.rs` 17, `comparer_tests.rs` 12), la suite complète
  passe (1612 essais, `scripts\verifier.ps1 -Repetitions 2` : OK).
- **Choix de SHOT, à confirmer** : ce qui compte comme un changement (les textes du storyboard et les marques d'image oui ;
  la couleur du plan, les signets, les noms et verrous de calques, le repère 3D non) ; « Restaurer » fait une nouvelle version
  plutôt que de remplacer ; marquer un envoi enregistre aussitôt le projet ; la pastille est ambre (couleur d'avertissement
  du thème), pas une couleur propre.
- **Limites** : les commentaires, les transitions, le son et les scènes 3D ne comptent pas dans l'empreinte d'un plan ; le contenu
  rogné (réserve) reste attaché au plan, pas à la version ; un envoi ne note que la version active ; un envoi ne se renomme
  pas (il se retire de la liste) ; changer la résolution du projet rend les anciens instantanés illisibles (leurs dessins ont
  l'ancienne taille : la comparaison le dit) ; les versions rangées se lisent toutes à l'ouverture (comme le contenu rogné) ;
  aucune touche par défaut.

## Numérotation robuste des plans (demandée par Anthony le 2026-10-20, branche `numerotation-export`) ✅

Détail des règles dans `docs/INTERFACE.md` (« Numérotation des plans : suivie ou verrouillée ») et en tête de
`src/numerotation.rs`. Pas vu à l'écran dans cette séance.

- ✅ **Deux modes** dans Réglages du projet : « Suivie » (le rang, comme avant) et « Verrouillée » (numéros propres, 12,
  12A, 12B…, trous après une suppression, numéro gardé par un plan déplacé).
- ✅ **Renuméroter** : confirmation, une étape d'annulation nommée ; **Verrouiller** : une étape nommée aussi.
- ✅ **Partout** : tous les titres passent par `Document::scene_label` / `plan_code` ; les noms de clips et de fichiers
  des exports pour le montage par `plan_full_ref` (« SQ01_PL012A »).
- ✅ **Format** : champs facultatifs du manifeste, `VERSION` reste 27 (l'autre session peut prendre 28).
- ✅ **Essais** : `numerotation_tests.rs` (règle d'insertion, 1500 insertions au hasard ordonnées et uniques,
  insertion / suppression / déplacement / duplication / coupe / collage / réunion dans les deux modes, renuméroter et
  annuler, aller-retour d'enregistrement, ancien manifeste, projet suivi écrit comme avant) ; le projet d'essai de
  fiabilité est maintenant verrouillé (un plan 2A, un trou, une séquence 1A).
- Pas fait : pas de numérotation de plan automatique par séquence (SQ01 → PL001…) : un seul compte pour tout le film.
## Export pour le montage : EDL, Final Cut XML, OpenTimelineIO (demandé par Anthony le 2026-10-20, branche `numerotation-export`) ✅

Détail dans `docs/EXPORT_MONTAGE.md`. **Jamais ouvert dans Premiere Pro, DaVinci Resolve ni Avid** (à vérifier par Anthony).

- ✅ **Un dossier `<Titre>_montage/`** : `media/` (un fichier vidéo par plan, ProRes 422 HQ ou H.264, par le chemin de l'export du
  film), `audio/` (un WAV par extrait, effets compris), `.edl` (CMX 3600), `.xml` (Final Cut Pro 7 XML, xmeml 4), `.otio`
  (OpenTimelineIO), `LISEZ-MOI.txt`.
- ✅ **Noms de clips = numéros des plans** (`SQ01_PL012A`), chemins relatifs (ou absolus au choix), cadence du projet,
  time code de départ au choix (01:00:00:00), NTSC en option.
- ✅ **Transitions** : fondu enchaîné partout (avec poignées dans les vidéos : images tenues), fondu depuis / vers le noir dans
  l'EDL ; les autres sont des coupes, dites.
- ✅ **Fenêtre** `Fichier > Exporter > Pour le montage`, action `fichier.exporter_montage` ; tâche annulable qui efface ce
  qu'elle a écrit ; son écrit sur un fil à part.
- ✅ **Essais** : `editorial_tests.rs` relit chaque fichier produit (EDL, XML, OTIO) et le compare au plan à l'image près
  (24, 25, 30 i/s, NTSC), et fait un export réel avec ffmpeg (ProRes, H.264 en 29,97 avec chemins absolus : nombre d'images de
  chaque vidéo, durée de chaque son, chemins qui mènent aux fichiers, annulation).
- Pas fait : calcul des images sur un fil à part (elles se calculent sur le fil de l'interface, 40 ms par image d'écran, comme
  l'export du film) ; time code à sauts d'images ; AAF ; son dans l'EDL ; fondus par le noir ou le blanc entre deux plans et volets
  (coupes dites) ; poignées lues dans le contenu rogné (elles sont des images tenues).

## Calque de couleur : la couleur sous le trait (demandé par Anthony le 2026-10-20, branche `canevas-couleur`) ✅

- Un calque de couleur est lié à un calque de dessin (le trait) : juste dessous dans la pile, il en suit les images
  (miroir des clés, des expositions et des trous), on y colorie sans abîmer le trait. Pot de peinture borné par le trait
  (trous fermés comme l'aplat sous le trait et le calque CTG), pinceau dans la zone fermée (réglable). Créer, déplacer,
  cacher, supprimer, dupliquer, copier / coller avec le trait ; une annulation par commande ; format 30. Détails et
  vérifications : `docs/INTERFACE.md` partie 18, `README.md` « Calque de couleur ».
- SHOT reprend l'idée « trait / couleur ».
- Reste : réglage de la fermeture des trous par zone (aujourd'hui par calque) ; une couleur qui suit le tremblement du
  trait ; remettre sa couleur à une clé du trait coupée / collée ; un panneau de palette de couleurs par personnage.

## Toile infinie : dessiner hors de la feuille (demandé par Anthony le 2026-10-20, branche `canevas-couleur`) ✅

- Dessiner partout autour de la feuille (jusqu'à 32 768 px) : tuiles
  éparses à coordonnées signées (`Raster::outer`), numéros de tuiles stables pour l'historique, trait vectoriel
  sans limite de feuille, composition par tuiles du monde, caméra qui regarde hors de la feuille, papier partout,
  plus d'ombre sous la feuille, cadre de caméra discret. Enregistrement : fichiers `<n>_hors.png / .json`
  (format 30). Un projet d'avant s'ouvre tel quel.
- À l'écran, la feuille garde sa composition d'avant (carte graphique) et le reste du monde visible est une texture
  composée par le processeur (`monde.rs`), 5 ms par image au plus, mise à jour par tuiles. Pire image mesurée loin
  de l'origine : 2,4 ms. Détails : `docs/ARCHITECTURE.md` « Toile infinie », vérifications : `docs/INTERFACE.md`
  partie 19.
- Reste : pot de peinture, zones bitmap sélectionnées, effets de calque et d'ajustement, aplat sous le trait,
  calque de couleur, déformations (enveloppe, perspective) et propagation à la
  feuille seulement ; effets de la carte graphique sur le hors-feuille ; « Tout voir » (cadrer tout ce qui est
  dessiné) en plus de « Ajuster à la fenêtre » ; zoom arrière extrême plus net (tuiles réduites par niveaux).

## Versions d'un plan dans le montage (demandées par Anthony le 2026-10-20, branche `versions-montage`) ✅

- Nomenclature **V1, V2, V3…** partout. Une étiquette « V2 » sur le bloc d'un plan qui a plusieurs versions ; la survoler
  ouvre une pile de cartes au-dessus du bloc (aperçu par la file de fond, nom, croix) ; clic = passer à la version, croix
  = supprimer après confirmation. Clic droit sur le plan > Nouvelle version > en dupliquant / en partant d'un plan vide
  (même durée, même cadre, **même caméra**, un calque vide). Storyboard > Versions du plan > Supprimer les versions non
  affichées (film ou plan courant, confirmation avec le compte, une étape d'annulation). Clips de l'export pour le montage
  suffixés `_V2`. Aucun changement de format. Détails et vérifications : `docs/INTERFACE.md` partie 15.
- Reste : l'étiquette dans la vue Vignettes ; défilement des cartes au clavier ; aperçu animé (toutes les images) d'une
  version au survol.

## Onglets de projets (demandés par Anthony le 2026-10-20, branche `onglets`) ✅

« J'aimerais qu'on travaille sur la possibilité d'ouvrir plusieurs fichiers dans la même fenêtre avec un système d'onglets. »

- Une **bande d'onglets** sous les menus : un projet par onglet, « • » s'il n'est pas enregistré, croix, clic du milieu, clic droit
  (Fermer, Fermer les autres, Afficher dans l'Explorateur), glisser pour réordonner, « + » et Ctrl+T (accueil), Ctrl+W, Ctrl+Tab /
  Ctrl+Maj+Tab. Ouvrir, Nouveau, récents, imports et un `.pelure` lâché prennent un nouvel onglet (Préférences) ; un projet déjà
  ouvert donne la main à son onglet. Détails et vérifications : `docs/INTERFACE.md` partie 23, `README.md` « Onglets de projets ».
- **Architecture** : l'état d'un projet (139 champs d'`App`) est échangé avec celui de l'onglet qu'on affiche (`onglets.rs`,
  macro `etat_projet!`), plutôt que d'écrire `self.projet().doc` partout ; les caches indexés par numéro de dessin (effets,
  textures, vignettes) sont rangés avec leur onglet, les tâches de fond répondent à leur onglet (époques uniques,
  enregistrement adopté par l'onglet caché), une copie de secours par onglet. Un essai lit `app.rs` et refuse tout champ
  d'`App` non classé. `docs/ARCHITECTURE.md`, « Onglets de projets ».
- **Mesures** (`docs/PERFORMANCE.md`, section 15) : changer d'onglet 7 à 14 µs, l'image d'écran suivante ne coûte pas plus qu'une
  autre (1,9 contre 1,8 ms), +54 Mo par projet lourd de plus, 0 ms de processeur et aucune repeinture au repos avec 3-4 onglets.
- **Presse-papiers** communs : plans, calques, images, clés, effets, zones, éléments ; feuille d'une autre taille **adaptée**
  (voir « Coller depuis un projet d'une autre taille » plus bas), autre cadence signalée ou recalculée. Session : les onglets
  reviennent au lancement. Quitter / fermer les autres : une seule boîte.
- Reste : les travaux qui parlent à l'interface
  (export, propagation, sons proposés) suivraient leur onglet au lieu de refuser le changement ; ranger par onglet les
  commentaires, la scène 3D de blocage et les références liées aux plans copiés d'un projet à l'autre ; Alt+1…9 pour aller à un
  onglet ; un menu des onglets quand la bande déborde ; réouvrir le dernier onglet fermé ; plusieurs fenêtres (glisser un onglet
  hors de la bande).

## Relecture par le réalisateur (demandée par Anthony le 2026-10-20, branche `revue-realisateur`) ✅

Le réalisateur / le client n'a pas SHOT : une page HTML hors ligne à lui envoyer, ses notes qui reviennent en commentaires.
- ✅ **Export « Pour relecture »** (Fichier > Exporter, fenêtre Envois, « Plus… », Ctrl+K) : une page `.html` autonome
  (images en base64, aucune adresse du dehors, politique de contenu `default-src 'none'`), plan par plan (numéro de la
  numérotation, V2, durée, action / dialogue / notes, une carte par image de storyboard), champ de note et dessin (stylo
  trois couleurs, gomme, annuler, effacer ; doigt, stylet, souris) sur chaque image, note générale, nom du relecteur,
  « Enregistrer mes notes » (`notes-<titre>-<date>.json`), brouillon gardé dans le navigateur. Options : lecture animée de
  chaque plan (≤ 16 images échantillonnées, identiques fondues), commentaires existants en lecture seule, PDF à imprimer
  (3 plans par page, cadres à lignes). Étendue : film, séquence, plans choisis, plans modifiés depuis un envoi.
- ✅ **Import des notes** (Fichier > Importer, ou le `.json` lâché sur la fenêtre) : lecture et validation sur un autre fil ;
  placement par identifiant de plan, puis par numéro ; notes sans plan listées (option : les poser sur le plan courant) ; une
  étape d'annulation nommée ; compte rendu « 12 notes importées sur 9 plans, 1 note sans plan (plan supprimé). » ;
  **idempotent** (clé de la note gardée dans le commentaire) ; le dessin du relecteur par-dessus l'image quand son commentaire
  est choisi, interrupteur dans le panneau Commentaires.
- ✅ **Commentaires dans le PDF** : option « Avec les commentaires » (non résolus, sous l'image où ils tombent).
- ✅ **Sans latence** : images calculées 40 ms par image d'écran, JPEG, page et PDF sur un autre fil, barre et « Arrêter », fichiers
  `.part` renommés à la fin. **Fiabilité** : `commentaires.json` gagne deux champs facultatifs (`note`, `dessin`), **le format
  du projet ne change pas** (`VERSION` reste 32) ; écrit par `write_sidecar` comme avant ; le projet d'essai de fiabilité porte
  une note de relecture avec son dessin.
- ✅ **Sécurité** : page sans script ni donnée du projet dans le script, tout le texte échappé ; fichier de notes non cru
  (40 Mo, 3000 notes, textes, identifiants, dessins décodés sous limites et réécrits).
- Vérifié dans un vrai navigateur (Chromium du volet intégré) : dessin, gomme, annulation, enregistrement, brouillon retrouvé,
  lecture animée ; le fichier produit est relu par SHOT (`a_notes_file_written_by_a_real_browser_is_read`). Pas vu : un
  téléphone, Safari, Firefox ; l'écran de SHOT (aucun lancement dans cette séance).
- Pas fait / reste : une note modifiée par le relecteur puis renvoyée n'est pas mise à jour (la première version importée est
  gardée) ; pas de vidéo par plan dans la page (images échantillonnées seulement) ; une réponse du réalisateur sur un commentaire
  existant ; envoyer la page par e-mail depuis SHOT ; les notes sans plan ne sont pas mémorisées si on ferme la fenêtre sans les poser.


## Notes d'Anthony : calques, effets, montage, export, caméra (2026-10-20, branche `notes-calques`) ✅

Sept demandes d'une même note du réalisateur ; pour chacune, vérifier d'abord si cela existait. Détail : `README.md` (« Notes
d'Anthony du 2026-10-20 »), vérifications à l'écran : `docs/INTERFACE.md`, partie 25.
- ✅ **Effets : « Trait ▾ » et « Matière ▾ »** à la place des deux rangées d'icônes (icône + nom, préréglages en section,
  entrées grisées avec leur raison sur un bitmap, nombre d'effets allumés dans le titre).
- ✅ **Dossiers de calques** : le modèle, la timeline et le menu Calques > Dossiers existaient ; ajouté dans le panneau Calques
  le bouton Nouveau dossier (et Ranger les calques choisis), le triangle d'ouverture, l'icône et le compte, le décalage des
  calques rangés, les dossiers fermés sans ligne.
- ✅ **Aplat sous le trait** (ex-« Fond sous le trait ») : un bouton du **calque** (ligne du panneau Calques, commandes) ; retiré des réglages du pinceau.
  Aucune conversion à faire (c'était déjà `Layer::fond`). Sa sous-ligne dans le panneau Calques et la cible du dessin : voir la section « Aplat sous le trait, depuis le calque » plus bas.
- ✅ **Bouton « fx »** de la ligne du calque (panneau Calques, timeline, menus de calque) : éteint toute la pile sans la
  toucher ; une étape d'annulation ; **format 33** (`effets_eteints`).
- ✅ **Versions de plan de durées différentes** : le modèle les permettait ; corrigé la tête de lecture et les choix d'images
  dans un plan plus court, ajouté les durées dans les cartes, la fiche, le menu et la phrase de la barre d'état (les plans
  suivants avancent ou reculent, le son reste en place). Export pour le montage, empreintes des envois et comparaison vérifiés.
- ✅ **GIF animé** (Fichier > Exporter) : film, séquence, plan courant ou plans choisis ; taille, cadence ≤ celle du projet,
  boucle, tramage ; ffmpeg `palettegen` / `paletteuse`, même tâche de fond que le film ; seules les images gardées sont calculées.
- ✅ **Cadre de la caméra** à 1 pixel de l'écran, net, sur un liseré sombre ; **opacité du passe-partout** réglable dans les
  Préférences (0 à 100 %).
- Reste : l'étiquette « fx » dans la colonne du montage (pas de place : seulement dans les menus) ; un GIF avec des plans
  choisis qui ne se suivent pas n'a pas de son (le GIF n'en a jamais) ; la réserve de rognage reste attachée au plan et non à
  la version (une version plus longue ne ramène pas les images rognées de l'autre).

## Notes d'Anthony : dessin (demandées par Anthony le 2026-10-20, branche `notes-dessin`) ✅

Huit notes du réalisateur ; le détail, les règles de touches et la liste à regarder à l'écran sont dans
`docs/INTERFACE.md`, partie 24. Essais : `src/dessin_notes_tests.rs`.

- ✅ **Tracé droit** : Maj tenue pendant un trait (pinceau, crayon, gomme, vectoriel ou bitmap) : un segment droit du
  dernier point posé à la pointe, en aperçu, posé au relâché ; Maj+Alt : angle par 15° ; une étape d'annulation. Pas
  fait : Maj + clic, Maj + clic façon Photoshop ; aperçu en ligne, pas avec le vrai pinceau.
- ✅ **Choisir un objet au clic** : Ctrl + clic (existait en vectoriel) choisit aussi, sur un calque bitmap, la forme d'un
  seul tenant sous la souris ; Ctrl+Maj + clic ajoute ou retire un objet sur n'importe quel calque, Ctrl+Maj + lasso
  ajoute (avant : remplaçait) ; la sélection d'un seul calque est reprise dans le cumul ; Ctrl+Maj ne prend plus les
  poignées ; le lasso temporaire de Ctrl accepte Maj.
- ✅ **Un pinceau par sorte de calque** (vectoriel, bitmap), retenu dans les réglages de l'application.
- ✅ **Enregistrer la brosse** : disquette dans le panneau Réglages : dans la brosse d'où vient le réglage (rétablissable),
  ou comme nouvelle brosse ; « (réglé à la main) d'après « X » ». Existait caché : « Remplacer par le réglage courant »
  (clic droit de la bibliothèque).
- ✅ **Réglages repliés** sous l'aperçu du trait (flèche, état retenu) et **fond de l'aperçu** gris un peu plus clair que le
  panneau (trois thèmes, contraste gardé).
- ✅ **Remplir la sélection bitmap** : Maj+Suppr (prise à « effacer hors de la sélection », qui garde Maj+Retour arrière et
  l'ancien sens de Maj+Suppr sans zone bitmap) ; verrou, préserver la transparence, pochoirs respectés ; icône « Remplir ».
- ✅ **Clés de position** : outil Transformer > « Clés de position » : un calque animé va d'une clé à l'autre (lecture,
  export, vignettes, 3D), adoucis comme les clés de caméra (`camera::ease`), losanges verts dans la timeline (choisir,
  glisser, Suppr, clic droit), clés qui suivent les images insérées, la coupe et la réunion des plans, les miroirs.
  **Format 33** (`cles_position`). Pas fait : montage, copier / coller des clés, courbe de vitesse libre, clé d'opacité,
  propagation EbSynth (lit la pose fixe).

## Fiche technique : le découpage du film en tableur (demandée par Anthony le 2026-10-20, branche `fiche-technique`) ✅

Une fiche de découpage pour la production, le layout et le montage : une ligne par plan (ou par image de storyboard),
en classeur XLSX ou en CSV. Détail, règles du mouvement de caméra, limites et liste à regarder à l'écran :
`docs/INTERFACE.md`, partie 26 ; usage : `README.md`, « Fiche technique (tableur) ». **Aucun changement de format du
projet** (`VERSION` reste 33).

- ✅ **29 colonnes au choix et dans l'ordre voulu** (retenues dans les réglages) : séquence, n° de plan (`12A`), référence
  (`SQ01_PL012A_V2`), version, nom, durée en images / secondes / time code, TC entrée et sortie (départ réglable,
  01:00:00:00 d'office, time codes du film entier), cadrage (objectif ou focale), mouvement de caméra et son détail,
  images de storyboard, action, dialogue, notes, transitions d'entrée et de sortie, sons, commentaires ouverts et leur
  texte, modifié depuis l'envoi et son nom, couleur, calques, effets, vignette.
- ✅ **Mouvement de caméra déduit des clés** : fixe, travelling (X, Y, avancée), panoramique, bascule, zoom, rotation,
  seuls ou ensemble, avec le sens (« panoramique à droite + zoom avant ») ; aller-retour reconnu ; seuils nommés
  (`SEUIL_*`). À confirmer par Anthony : un glissement sur la feuille s'écrit « travelling », seule la rotation sur place
  est « panoramique ».
- ✅ **Formats** : XLSX écrit à la main (aucune caisse de plus ; `zipwrite.rs` écrit l'archive, `flate2` déjà là compresse) avec
  en-tête gras et figé, filtre, largeurs, nombres pour les durées, time codes en texte, **vignettes dans les cellules** ;
  CSV UTF-8 avec BOM, `;` ou `,`, guillemets et retours à la ligne gardés ; presse-papiers à tabulations (bouton, jamais
  automatique). Textes qui commencent par `=` `+` `-` `@` : apostrophe devant (CSV, presse-papiers) ; XLSX échappé ;
  texte coupé à 32 767 caractères dans le XLSX seulement, et dit.
- ✅ **Étendue** : tout le film, séquence courante, plans choisis, plans modifiés depuis un envoi (les choix de la
  relecture, `relectureui::plans_de`).
- ✅ **Tâche de fond** : vignettes dessinées 40 ms par image d'écran (`ExportJob::collect`), PNG et fichiers sur un fil à
  part, barre, temps restant, « Arrêter » (efface tout) ; écriture en `.part` puis renommage ; un fichier tenu ouvert par
  Excel est refusé tout de suite (« ouvert dans un autre programme ») et, s'il se verrouille en cours de route, la fiche
  d'avant reste intacte.
- ✅ **Vérifié dans Excel** (installé ici, ouverture automatisée) : pas de réparation, images dans leurs cellules, première
  ligne figée, filtre, en-tête gras, durées en nombres, `=1+1` resté du texte. Pas vérifié : LibreOffice, Google Sheets.
- ✅ **Essais** : 51 nouveaux (`fiche_tests.rs` : colonnes et réglages, numéros et versions, time codes, cadrage,
  classification du mouvement sur des clés synthétiques et seuils, une ligne par image, transitions, sons, commentaires,
  effets, envois ; CSV (BOM, séparateurs, guillemets, formules), presse-papiers, archive zip relue, classeur relu (parties,
  relations, XML, cellules, styles, volet figé, images), tâche (atomique, annulation, fichier verrouillé au début et en
  cours de route), fenêtre, étendues, onglets) ; la suite complète passe, `scripts\verifier.ps1 -Repetitions 2`.
- Pas fait : choix de l'image de la vignette (c'est la première du plan) ; mise en page à imprimer au-delà du paysage une
  page de large ; export Google Sheets direct ; colonnes calculées (formules) ; mouvement des calques (clés de position).

## Exports hors du fil de l'interface (2026-10-20, branche `exports-fond`)

Priorité d'Anthony : « un logiciel qui réponde toujours très rapidement, sans latence ». Avant, le film, les images, le GIF, le
PDF, le montage, la relecture, la fiche calculaient leurs images sur le fil de l'interface (40 ms par image
d'écran, projet verrouillé) ; la page HTML, l'image courante et le son seul gelaient SHOT.

- ✅ **Cliché du projet** (`exportrender.rs`, `SnapshotBuilder`) : structure copiée, dessins par tranches de 8 ms, tuiles partagées
  par `Arc` (rien n'est recopié en pixels) ; 2 ms pour la structure d'un montage de 600 plans, 9 à 15 ms pour 600 plans de 8
  calques de 48 images, jamais plus de 9 ms d'un coup. L'export utilise le projet tel qu'il était au départ.
- ✅ **Fils de rendu** (`Pipeline`) : le code d'avant (mêmes pixels, essais pixel pour pixel sur le projet d'essai de la
  fiabilité), mode exact, réserve privée de dessins dérivés par fil (`fxcache::avec_privee`), une image tenue calculée une fois,
  un passeur qui range les images dans l'ordre, mémoire bornée, priorité sous celle de l'interface (ffmpeg aussi), deux cœurs
  laissés à l'interface. Annuler rend la main en quelques millisecondes.
- ✅ **Tous les exports** : film, GIF, images PNG, image courante, PDF, page HTML, son seul, montage, relecture, fiche
  technique. Le son est mixé sur un fil à part. Les traits texturés restés en rendu rapide sont refaits exacts dans
  la copie (sur un autre fil).
- ✅ **Le projet est libre** : fenêtre « Export en cours » non modale ; on change d'onglet, l'export suit son projet ;
  « Quitter » prévient. PDF, page, image, son : `.part` puis renommage (annuler ne détruit pas le fichier d'avant).
- Restent : l'API locale (`noyau.rs`) et la lecture du storyboard par un modèle (`lecturesb.rs`) calculent encore des images
  d'export sur leur propre fil ; le film s'écrit directement (pas de `.part`) comme avant ; un seul export à la fois pour le
  film / les images / le PDF…

## Coller depuis un projet d'une autre taille (demandé par Anthony le 2026-10-20, branche `coller-adapter`) ✅

« Onglets : un plan collé dans un projet d'une autre taille de feuille doit s'adapter. » Avant, la copie était refusée.

- **La règle** (`src/adaptation.rs`, pure) : le cadre de la caméra d'origine est ajusté au cadre de la caméra d'arrivée, d'un même
  facteur en largeur et en hauteur, centre sur centre ; même cadre et même feuille : rien ne change. **Contenir** (d'office) ou
  **Remplir** quand les proportions diffèrent ; **garder les images** (d'office) ou **garder la durée en secondes** quand les
  cadences diffèrent (arrondi de « Changer la cadence », dont la règle est partagée : `retemps_reglages_calque`/`_plan`).
- **Mis à l'échelle** : bitmap (feuille et hors-feuille, filtre prémultiplié), vectoriel, aplat sous le trait, pose et clés de
  position, clés de caméra, effets en pixels (reconnus à leur unité `px` ; un essai garde la liste), fermetures de trous.
  Appliqué aux plans, calques, images, clés du montage, clés de caméra et d'effets, effets, zones, éléments et sélection sur
  plusieurs calques.
- **Parcours** (`src/collage.rs`) : petite fenêtre (proportions, cadence) seulement s'il y a un choix, choix retenus dans les
  réglages et dans Préférences > Projets ; calcul sur des fils de fond avec avancement ; une étape d'annulation nommée ; message
  d'état « Collé depuis un projet 3840×2160 : mis à l'échelle ×0,5. » ; le presse-papiers reste la copie d'origine.
- **Mesures** (`docs/PERFORMANCE.md`, section 18) : lancer le collage 3 ms, plan 4K de 72 dessins prêt en 0,5 s, pire image d'écran
  8 ms pendant le calcul. Aucun changement du format du projet (version 33).
- Reste : « Contenir / Remplir » pour les paramètres d'effets en pour cent d'une image (centre, rayon) quand les proportions
  diffèrent ; les calques vidéo (images de la vidéo, non copiées) ; une barre de progression plutôt que la barre d'état ; annuler
  le calcul en cours ; un aperçu du résultat dans la fenêtre.

## Travailler à plusieurs sur un seul film (demandé par Anthony le 2026-10-20, branche `cooperatif`) ✅ (cœur)

« Rendre le logiciel coopératif : possibilité de travailler à plusieurs sur un seul fichier. » Choix d'Anthony : **option A**, un dossier partagé
(Dropbox, Drive, NAS) où chacun **prend** ses plans, un à la fois ; deux personnes pour l'instant, au bureau et à distance ; pensé pour passer à un serveur.
Conception écrite d'abord : `docs/COOPERATION.md`.

- ✅ **Format partagé** (format 34, projet partagé seulement ; le classique reste 33) : structure sans plans, un fichier par plan avec l'empreinte de ses
  dessins, commentaires par auteur, fiches d'équipier ; numéros préfixés par équipier ; « Partager ce projet… » sans toucher l'original.
- ✅ **Le dépôt** : trait `Depot`, `DossierPartage` (balayage par date et taille, fichiers temporaires ignorés, copies en conflit repérées) ;
  la couture du futur `ServeurShot`.
- ✅ **Les prises** : fiches à un seul auteur, titulaire calculé de la même façon partout, bail de 10 min, reprise confirmée, course tranchée, brouillons,
  jeton du montage, séquence entière, plan créé = à soi.
- ✅ **Rafraîchir sans déranger** : fil de fond, plans lus et décodés hors de l'interface, posés en 0,07 ms ; ce qu'on tient n'est jamais remplacé ;
  annuler ne ressuscite pas un vieux plan ; notifications ; présence ; dossiers synchronisés lents ou dans le désordre (plan avant ses dessins, structure
  avant le plan, release avant le fichier).
- ✅ **Enregistrer** ce qu'on tient seulement, par le chemin sûr ; garde-fous (retouche sans prise, plan supprimé, jeton perdu) → brouillons.
- ✅ **Interface** : barrières de lecture seule, pastilles, fenêtre Équipe, menus, barre d'état, préférences, commandes, hors connexion.
- ✅ **Essais** : 62 (+1 mesure), famille ajoutée à `scripts\verifier.ps1` ; mesures dans `docs/PERFORMANCE.md`, 18.
- Pas fait : le serveur SHOT ; l'enregistrement automatique dans le dossier partagé ; le nettoyage du dossier ; importer un montage perdu ; retenir les copies
  en conflit ignorées ; deux porteurs écrivant le même dessin plus de huit secondes d'affilée laissent des « copies en conflit » de dessins (rien n'est perdu).

## Pistes repliables du montage (demandées par Anthony le 2026-10-20, branche `pistes-repliables`) ✅

« Un système de dépliage / repliage des calques dans la timeline pour gagner de la place tout en gardant cela lisible. » Fait dans le montage (storyboard) :
deux hauteurs de piste (40 et 16 points), un triangle par piste (Alt + clic : toutes), double-clic sur la colonne, replier / déplier tout (menu du calque, « Plus… » de la barre du montage,
actions `piste.*` sans touche), « Déplier le calque courant » (réglage, allumé d'office, jamais pendant un geste), les dossiers fermés cachent les pistes de leurs calques. L'état est
`Layer::collapsed`, déjà enregistré (`replie`) et déjà celui des lignes réduites de la timeline de l'animation : un seul système, format inchangé. Les gestes (choix, glisser, losanges, rectangle)
marchent sur une piste repliée. Détail et liste de contrôle : `docs/INTERFACE.md` § 29 ; code : `src/pistes.rs`.

À vérifier à l'écran : voir § 29.7. Pas fait : une touche par défaut (aucune de libre sans conflit), « Replier / Déplier » pour les pistes son, une disposition de pistes propre à chaque plan.

## Espace 3D (demandé par Anthony le 2026-10-20, branche `espace-3d`) ✅ (cœur)

Sa demande : « Il y a quelque chose de très confus entre ces deux espaces 3D [le plateau en 3D et la scène 3D de blocage]. Il nous
faut un seul espace 3D où les calques apparaissent et donnent la possibilité de les déplacer dans l'espace pour créer un
environnement 3D avec, et ainsi une parallaxe. Ça implique donc d'importer dans cet espace tous les outils 3D et de pouvoir
déplacer, pivoter, modifier les calques dans cet espace comme de vrais objets 3D. »

Fait (conception : `docs/ESPACE3D.md` ; interface et liste de contrôle : `docs/INTERFACE.md`, partie 13 ; code : `espace3d.rs`,
`espace3dobjets.rs`, `scene3dui.rs`, `scene3dview.rs`) :
- ✅ **Un seul espace** : un bouton (cube), une entrée de menu, une commande `espace3d.basculer` ; les anciens identifiants
  (`vue.3d`, `scene3d.basculer`, `vue3d.recadrer`, `outil3d.*`) restent en doubles. Le plateau en 3D est retiré.
- ✅ **Les calques sont des cartes** : déplacer / tourner / incliner / mettre à l'échelle avec les poignées des objets (axes
  locaux, aimantation, multi-choix), champs chiffrés, verrous, annulation par geste ; la profondeur du multiplan est la position Z.
- ✅ **Une seule caméra** : celle du plan, avec ses clés ; plus de caméra du plateau (l'ancienne reste pour le calque « Fond 3D »).
- ✅ **Parallaxe dans l'image** : cartes à plat = rendu d'avant (mêmes pixels) ; cartes inclinées en perspective (maillage, écran,
  vignettes, export, calques d'ajustement) ; carte graphique = processeur à 2 niveaux près ; objets 3D dans l'image, sous les cartes.
- ✅ **Clés 3D** (inclinaison, profondeur) comme les clés de position ; **format 35** (projets partagés : le plus grand des numéros) ;
  anciens projets identiques ; objets d'un ancien projet hors de l'image.
- ✅ Pinceau sur une carte inclinée ; « Ordonner la pile selon la profondeur » ; budget 10 cartes + 20 objets ≈ 2 ms par image.
- Pas encore (détail et raisons : `docs/ESPACE3D.md`, 11) : objet devant une carte (tampon de profondeur partagé), tri des cartes par
  profondeur, cartes inclinées au-delà de la feuille, effets et fusions dans la vue d'édition, objets animés, préchargement des
  objets en lecture, outils 2D sur une carte inclinée, inclinaison au-delà de 85°, dossiers comme groupes dans l'espace, et **tout ce qui se voit à l'écran** (aucun lancement de SHOT dans cette séance).

À vérifier à l'écran : voir § 29.7. Pas fait : une touche par défaut (aucune de libre sans conflit), « Replier / Déplier » pour les pistes son.

### Replis propres à chaque plan ou communs (demandé par Anthony le 2026-10-21, branche `pistes-par-plan`) ✅

Anthony garde les deux comportements : **Préférences > Montage > « Repliage des pistes »** (aussi au clic droit sur une piste et sous « Plus… » du montage, `piste.replis_par_plan`) entre
**commun à tous les plans** (d'origine) et **propre à chaque plan** (chaque plan retient ses replis ; hauteurs du plan courant, prises au relâchement de la souris ; triangle et « tout replier » sur
le plan courant seulement, « … de tous les plans » au clic droit). Réglage de l'application (`repliage_pistes`), rien de neuf dans le projet ; passage d'un mode à l'autre sans perte ; en mode par
plan, aucune écriture dans les plans qu'on ne tient pas (film partagé). Détail : `docs/INTERFACE.md` § 29.8, liste de contrôle § 29.9 ; code : `src/pistes.rs`.

## Espace 3D simplifié : plus d'objets 3D, plus de repère 3D, plus de calque de fond (demandé par Anthony le 2026-10-21, branche `espace3d-simple`) ✅

Anthony : « On va rester simple et focus sur le dessin. Un espace 3D où je peux bouger les dessins dans l'environnement suffit. On
peut supprimer les objets 3D comme les cubes, cylindres, personnages… ça alourdit le logiciel et l'interface pour pas grand-chose. »
Puis : le repère 3D (glTF) part aussi, et « les calques de fond qui ne servent à rien ».

- ✅ **Gardé** : la vue Espace 3D (les calques en cartes : déplacer, tourner / incliner, échelle, étirement par poignées et en chiffres ;
  plusieurs cartes ; aimantation ; sol quadrillé), la caméra du plan (la choisir, la déplacer, la regarder), les clés de position et de
  caméra, la parallaxe dans l'image (écran, lecture, vignettes, export), « Ordonner la pile selon la profondeur », « Remettre à plat ».
  Une seule rangée d'outils : Choisir · Déplacer · Tourner · Échelle · Caméra · Recadrer · Regarder par la caméra.
- ✅ **Supprimé** : objets 3D (cube, plan, cylindre, sphère, cône, escalier, mannequin, modèle glTF), matières et couleurs, lumière et
  styles de rendu, groupes, décor de séquence, « Objets dans l'image », « Figer les objets dans un calque de fond » (`scene3d.rs`,
  `scene3dmesh.rs`, `scene3drender.rs`, `scene3dgpu.rs`, `scene3dstore.rs`, `espace3dobjets.rs`) ; le repère 3D (`model3d.rs`,
  `view3d.rs`, `ref3dtool.rs`, `Tool::Ref3d`, Fichier > Importer > « Un modèle 3D (glTF) comme repère ») ; le bouton « Nouveau calque de
  fond » (icône des montagnes, `calque.nouveau_fond`) et ses menus.
- ✅ **Anciens projets sûrs** : un calque « Fond 3D » est un calque bitmap ordinaire (mêmes pixels) ; `scenes3d.json`, le champ `repere3d`
  et `modeles/*.pelure3d` ne sont plus lus ni écrits et jamais effacés ; plus d'avertissement pour un `scenes3d.json` abîmé ; aucun
  changement de `VERSION` (35). Un calque « de fond » d'un ancien projet était déjà un calque bitmap ordinaire.
- ✅ Les identifiants de commandes d'objets et du repère n'existent plus ; ceux qui gardent un sens (`vue.3d`, `scene3d.basculer`,
  `outil3d.*`, `scene3d.outil_*`, `scene3d.cadrer`, `scene3d.vue_camera`, `scene3d.aimanter`, `scene3d.grille`) sont des doubles des commandes
  `espace3d.*`.
- Essais : objets, mannequin, rasteriseur, rendu dans l'image, repère glTF retirés ; maths, poignées, caméra, cartes, parallaxe, export,
  format et migration gardés ; ajoutés : un ancien projet (objets, décor, repère, modèle, « Fond 3D ») s'ouvre sans avertissement avec les
  mêmes pixels et garde ses fichiers à l'enregistrement (classique et en arrière-plan) ; plus aucune commande d'objet ni de repère
  (actions, palette, keymap) ; le repos de l'application avec l'Espace 3D ouvert.
- Taille de `shot.exe` (release) et lignes : voir `docs/PERFORMANCE.md`.
- **Pas fait / à voir à l'écran** : la liste de contrôle de `docs/INTERFACE.md`, partie 13.

## Un seul calque de couleur : « On fusionne » (demandé par Anthony le 2026-10-21, branche `couleur-unique`) ✅

Anthony : « On fusionne » — le calque de couleur (peint à la main, lié à un calque de trait) et le calque CTG (on griffonne une
couleur, la zone se remplit) n'en font plus qu'un, puis le calque CTG disparaît.

- ✅ **Le calque de couleur remplit au griffonnage** (`griffonnage.rs`, `calquecouleur.rs`, `calquecouleurui.rs`) : mode **Griffonner**
  (icône boucle dans les réglages du pinceau, du crayon, de la gomme et du pot ; case du menu Calque de couleur ; commande
  `calque.griffonner`, sans touche). Les griffonnages sont un dessin à part par dessin de peinture (un vrai dessin du projet) ;
  chaque zone fermée par le trait (+ « sources supplémentaires ») qu'ils touchent se remplit, **sous** la peinture libre, avec la
  fermeture des trous (jusqu'à 8 px) du calque CTG (même algorithme, même image). La gomme efface les griffonnages en ce mode, la
  peinture libre sinon. Le pot pose un tampon.
- ✅ **Recalcul sans latence** : le remplissage est un élément du plan à part (dessin dérivé) calculé **en arrière-plan** à l'écran
  (`fxcache::Job::griffon`), l'écran garde le remplissage d'avant en attendant ; l'export et l'enregistrement le calculent tout de
  suite. Avant, le remplissage du calque CTG se calculait sur le fil de l'interface.
- ✅ **Calque libre** (sans calque de trait propre, images à lui) : pour les anciens calques CTG que leur trait ne suit pas, et pour
  un calque de couleur « détaché » qui a des griffonnages.
- ✅ **Calque CTG retiré** : commande « Nouveau calque CTG », menus « Calque CTG », « Transformer / Désactiver », « Charger la
  structure à partir des sources », sorte de calque rose (le rose est maintenant celui du calque de couleur), blocs de `keymap.toml`,
  `ctg.rs`, `ctgui.rs`, essais. (Reste : `crate::ctg::input`, vide, et le nom du champ `PlanItem::ctg`, que `propager.rs` écrit encore.)
- ✅ **Anciens projets** : format **36** ; un calque CTG s'ouvre converti en calque de couleur, **même image au pixel près**
  (empreintes prises avant la fusion), griffonnages gardés (mêmes fichiers), lié à sa source si elle est juste au-dessus et que les
  images se recouvrent, libre sinon ; le champ `ctg` est lu, jamais plus écrit ni effacé.
- ✅ **Trois façons de colorier sous le trait, devenues deux** : calque de couleur (peinture + griffonnage) et aplat sous le trait ; la
  brosse par personnage s'appuie sur l'aplat sous le trait et reste telle quelle (ordre sur un calque : remplissage au griffonnage,
  aplat, peinture libre, trait).
- **Pas fait / limites** : sélectionner, déplacer ou transformer des griffonnages (les outils agissent sur la peinture libre) ; le
  remplissage n'est pas dans les images guides de la propagation (`propager.rs`, qu'on ne touche pas ici) ; un effet de calque qui crée
  de la matière dans le vide s'applique au remplissage et à la peinture libre chacun de son côté.
- **À voir à l'écran** : la liste de contrôle de `docs/INTERFACE.md`, partie 18 (« À vérifier à l'écran (Griffonner) »).

## Brosses par personnage (demandées par Anthony le 2026-10-21, branche `brosses-personnages`) ✅

« La brosse par personnage » : on ajoute un personnage et on lui donne une couleur de trait et un aplat (la couleur sous le trait) ; le choisir règle le trait et l'aplat
tout de suite, pour reconnaître un personnage même dans un dessin très rough. Une barre fine de pastilles au-dessus du dessin (une par personnage, plus des couleurs
neutres), paramétrable, et son panneau « Personnages ». Mode d'emploi : README, « Brosses par personnage » ; conception, gestes et liste de contrôle :
`docs/INTERFACE.md` § 32 ; code : `src/personnages.rs`, `personnagesui.rs`, `persofond.rs`.

- ✅ **Personnages du projet** (nom, trait, aplat ou non, ordre) : dans `Structure` (annulables), fichier `personnages.json` à côté du manifeste (`write_sidecar` : jamais détruit s'il est
  illisible ou plus récent ; **`VERSION` inchangée**, projets partagés inclus). **Couleurs neutres** dans les réglages de l'application (noir sans aplat, gris sur gris clair, rouge de correction).
- ✅ **Barre** au-dessus du dessin, par espace de travail (visible d'office en Storyboard, cachée en Animation et en Montage, allumable ; l'espace Vignettes à venir la montrera : une ligne dans
  `Espace::barre_personnages_par_defaut`) ; clic = choisir (ou lâcher), clic droit = modifier, dupliquer, déplacer, supprimer. **Panneau Personnages** en onglet avec Couleur et Pinceaux,
  glisser pour ranger.
- ✅ **L'aplat de chaque trait** : le « Aplat sous le trait » est une forme par dessin, d'une couleur par calque ; plutôt que de changer ce format, les pixels que la forme gagne grâce à un trait de
  personnage sont peints dans la peinture de l'aplat, de la couleur d'aplat du personnage (fil de fond, sans latence ; une étape d'annulation avec le trait). Plusieurs personnages, un seul calque.
- ✅ Une couleur prise à la main (Couleur, pipette, nuancier, échange) lâche le personnage ; pinceau et crayon, bitmap et vectoriel ; la gomme n'est pas touchée ; le pot de peinture
  sur l'aplat prend l'aplat du personnage.
- Limites : un personnage dessiné dans l'aplat d'un autre garde cet aplat là où ils se chevauchent ; un trait ne se souvient pas de son personnage ; formes et texte ne portent que la couleur du trait.
- À faire plus tard : les raccourcis `perso.suivant` / `perso.precedent` n'ont pas de touche par défaut ; l'espace Vignettes (autre branche) montre la même barre (`personnages_barre`). La passe de
  vocabulaire « fond → aplat » a eu lieu le 2026-10-21 (section suivante).

## Aplat sous le trait, depuis le calque (2026-10-21, branche `aplat-souscalque`)

Demande d'Anthony : « Le fond derrière le trait s'active depuis le calque, plus depuis les réglages du pinceau. Une fois activé, un sous-calque apparaît dans le panneau de calques
(uniquement, pas sur la timeline), en petit. Quand on clique sur ce calque on peut gommer ou dessiner sur le fond uniquement. Plus simple, plus intuitif. » Et le nom : « Aplat ». Détail,
gestes, règles de cible et liste de contrôle : `docs/INTERFACE.md` § 33.

- ✅ **Activation seulement depuis le calque** : bouton de sa ligne (« Aplat sous le trait »), clic droit du calque (« Ajouter un aplat » / « Retirer l'aplat »), menu Calques ; `pinceau.fond` et
  `fond.*` restent comme alias (leurs raccourcis marchent). **Supprimés** : la bascule « Peindre dans : Trait | Fond » (réglages et ligne du calque), l'action `pinceau.strate_fond`, tous les
  réglages d'aplat sous le pinceau, le crayon, la gomme et le pot de peinture.
- ✅ **Sous-ligne fine** (16 px) sous chaque calque qui a un aplat, **dans le panneau Calques seulement** (ni timeline ni montage), qui suit son calque dans un dossier fermé.
- ✅ **Cible du dessin** (`App::aplat_cible`, propre au projet, jamais enregistrée) : un clic sur la sous-ligne dirige pinceau, crayon, gomme, pot de peinture et cutter vers l'aplat seul ; un clic sur le calque
  (ou un autre calque, ou retirer / annuler l'aplat) revient au trait. Barre de la vue : « Aplat de <calque> · … ». Choisir un personnage revient au trait (il dessine trait et aplat dans la même strate).
- ✅ **Réglages** de l'aplat dans Réglages (sous-ligne choisie) et au clic droit de la sous-ligne ; étapes d'annulation nommées « Aplat : ajouter / retirer / peinture / gomme / remplir / retirer la zone / réglages ».
- ✅ **Vocabulaire** : « Aplat » partout dans l'interface et les docs ; le code et le format gardent `fond` (aucun changement de format).
- Reste : « Cet aplat aussi sur » seulement dans Réglages ; formes et texte ne se posent pas dans l'aplat.

## Fusion des deux lignes de travail (2026-10-21, branche `integration17`)

Deux branches parties de `integration14` se rejoignent : **Espace 3D simplifié + un seul calque de couleur qui griffonne** (`couleur-unique`, format 36)
et **Préférences en onglets, onglet Montage, effets au clavier, Vignettes simples, brosses par personnage, sous-ligne « Aplat »** (`c3507d9`).

- ✅ Rien n'est perdu des deux côtés : plus d'objets 3D, de repère glTF, de calque de fond ni de calque CTG ; plus de planche de crayonnés ni de bascule
  « Peindre dans » ; le calque de couleur qui griffonne, les personnages, la sous-ligne d'aplat, les Préférences en onglets, l'onglet Montage, les Vignettes
  simples et les effets au clavier sont tous là.
- ✅ Format **36** (celui de `couleur-unique`) ; `personnages.json` (annexe sans changement de numéro), l'aplat (format 23) et les Vignettes (aucun fichier) s'y
  ajoutent sans rien changer ; `crayonnes.json` et `scenes3d.json` sont d'anciens fichiers jamais lus, jamais écrits, jamais effacés ; le champ `ctg` est lu
  seulement.
- ✅ Vocabulaire : le remplissage sous le trait s'appelle toujours « Aplat » ; le calque de couleur a « Griffonner » (plus de CTG).
- ✅ Ordre de composition d'un calque : remplissage au griffonnage, puis aplat (la strate Fond, où les personnages posent leur aplat), puis peinture libre, puis trait.
- ✅ Retouches des Vignettes (`vignettes-retouches` : débord du trait hors de l'image, Maj + cadre pour déplacer un plan, `src/debord.rs`, `src/boardmove.rs`) et isolation de l'essai des personnages, fusionnées par-dessus.

## Variantes de séquence (2026-10-21, branche `variantes-sequence`)

Anthony : essayer un autre découpage d'une séquence (A plus serrée, B avec un plan de plus) sans perdre le premier.

- ✅ `src/variantes.rs` : une séquence a une variante **choisie** (ses plans dans le film : montage, lecture, exports, numérotation, export pour le montage, relecture
  et fiche la suivent) et des variantes **rangées** (scènes, plans, versions de plan, contenu rogné, commentaires, transitions internes) dans `Document::variantes`
  (`Arc`, dans `Structure` : une étape d'annulation nommée par geste, sans rien recopier).
- ✅ Nouvelle variante en dupliquant (copie profonde rangée, le film ne bouge pas) ou en partant d'un plan vide ; passer à, supprimer (jamais la dernière), renommer
  (A, B, C, nom court) ; les transitions de bord suivent la coupe ; numérotation verrouillée sans doublon.
- ✅ Fichier : `variantes.json` à côté du manifeste, **format 36 inchangé** ; plans relus par la lecture ordinaire (séquence cachée, comme `plans_versions.json`) ;
  fichier illisible ou plus récent jamais effacé.
- ✅ Interface (`src/variantesui.rs`) : sélecteur discret en haut de la vue Vignettes, Storyboard > Variantes de la séquence, clic droit sur la barre de séquence du
  montage (« Séquence 2 · B »), Ctrl+K (7 commandes sans touche), confirmation de suppression, renommage.
- ✅ Comparer : côté « Variante B » (plan de même rang) dans la fenêtre Comparer. Envois : la variante choisie est notée, un plan copié se compare au plan dont il vient.
- ✅ Film partagé : jeton du montage, plans sortants à moi ou libres, annexe du montage `variantes.json`, jamais réécrite si des dessins manquent.
- ✅ 29 essais (`src/variantes_tests.rs`) : création, échange, annulation, enregistrement octet pour octet, fichier illisible, envois, comparaison, jeton, plan tenu par un
  autre, repos.
- À faire plus tard : comparer deux jeux de plans côte à côte ; dupliquer aussi le contenu rogné ; fusionner deux variantes ; liste des variantes dans la fiche technique.

## Vignettes : annotations, images déposées, marie-louise, exports de la planche et color script (demandés par Anthony le 2026-10-21, branche `vignettes-annot-exports`) ✅

Fait (non fusionné) : stylo et gomme d'annotation par-dessus la planche, attachés à la case la plus proche et suivant la planche (`annotations.rs`, `annotationsui.rs`, `annotations.json`) ;
marie-louise (`marie.rs`) ; images déposées depuis l'Explorateur et le panneau Références (`deposer.rs`) ; Fichier > Exporter > « Planche des vignettes » (`planche.rs`, `plancheui.rs`) et « Color script »
(`colorscript.rs`, `colorscriptui.rs`) en tâches de fond annulables. Mode d'emploi : README ; gestes, choix et liste à vérifier : `docs/INTERFACE.md`, partie 11.

Limites connues, à décider : la gomme d'annotation efface le trait entier (pas de coupe en morceaux) ; la marie-louise et la planche exportée ne gardent pas les cases vides ; un trait d'annotation
d'une case vide dépendant de la dernière vignette suit cette dernière si le nombre de colonnes change beaucoup ; le dépôt d'un fichier depuis l'Explorateur lit le curseur de l'écran (Windows ne donne pas la
place du lâcher à la fenêtre) ; l'export en pages A4 / A3 ne coupe qu'en hauteur ; la bande des couleurs dominantes est un histogramme (pas un partitionnement k-means) ; pas encore de réglages mémorisés d'une
séance à l'autre pour les deux fenêtres d'export, ni d'annotations dans les envois de relecture.

## Plan en Espace 3D : dessiner dans un décor que la caméra traverse (demandé par Anthony le 2026-11-11, branche `espace-libre`) — en cours

Sa demande : pousser dans FEANOR le dessin dans un environnement 3D (à la manière d'un add-on de storyboard 3D pour Blender), sans rien casser. Plan de l'étape A (un plan dont la
caméra est libre, cartes ordonnées par distance, surfaces posées face à la caméra, au sol ou en mur) : `docs/ESPACE-LIBRE.md`. Rien n'est codé. Étapes B (décor partagé par séquence)
et C (décors glTF, mannequin) à planifier ensuite. `suivi/V1.md` range le moteur 3D en V2.

- ✅ **A0** (2026-11-11) : vérifications par lecture ; le plan tient (deux constructeurs de plan seulement, zone filmée dans la feuille, dossiers en bloc, format 54).
- ✅ **A1** (2026-11-11) : la case `Panel::espace`, enregistrée (**format 54**), commande `espace3d.plan_en_espace`, bouton dans Réglages de l'Espace 3D, une étape d'annulation ;
  l'image n'en dépend pas encore. Pas regardé à l'écran.
- ✅ **A2** (2026-11-11) : les maths du changement de repère (`Monde::repere_camera`, variantes à matrice) ; rien ne les appelle encore. Une caméra dans les limites d'avant
  filme la même image par le repère, à 0,05 px près ; caméra retournée, derrière la feuille, carte au sol à 90° (essais).
- ✅ **A3** (2026-11-11) : le rendu. Les cartes d'un plan en Espace 3D sont posées dans le repère de sa caméra (`espace3d::place_dans`), `Document::panel_camera` rend la caméra de
  référence pour ces plans (`panel_camera_reelle` pour ce qui décrit la caméra) ; caméra non bridée ; le pinceau suit. L'export compose l'image de l'écran pixel pour pixel (essai).
  Pas regardé à l'écran ; limites dans `docs/ESPACE-LIBRE.md`.
- ✅ **A4** (2026-11-11) : ce qui est devant cache ce qui est derrière (`espace3d::ordre_par_distance`). Les cartes d'un même plan gardent l'ordre de la pile ; un dossier passe en
  bloc ; un calque d'ajustement agit sur ce qui est plus loin que lui (`calqueffet::Portees`). Pas regardé à l'écran.
- ✅ **A5** (2026-11-11) : dessiner par la caméra. Pas de trait là où le regard ne touche pas la carte (`App::to_layer_opt`), ni au-delà d'une feuille de ses bords près de
  l'horizon ; le curseur du pinceau a la taille du trait en perspective (`App::layer_zoom`). Pas regardé à l'écran.
- ✅ **A6** (2026-11-12) : poser une surface à dessiner face à la caméra, au sol ou en mur (`espace3d.surface_*`, `Monde::surface_xf`), à une distance choisie ; inclinaison libre
  jusqu'au demi-tour dans un plan en Espace 3D (85° ailleurs). Limite : pas de carte derrière la caméra d'origine du plan. Pas regardé à l'écran.
- À faire : A7 (vue Espace 3D), A8 (finitions).
