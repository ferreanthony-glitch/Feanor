# Pelure

Logiciel d'animation 2D écrit en Rust avec egui : storyboard en séquences, scènes et panneaux façon
Storyboard Pro, animation image par image façon TVPaint, et dessin vectoriel modifiable.

## Lancer

```
cargo run --release
```

L'exécutable est ensuite dans `target\release\pelure.exe`. On peut lui passer un projet :
`pelure.exe "C:\Films\Bus\Bus.pelure"` (et donc associer les fichiers `.pelure` à Pelure pour les
ouvrir d'un double-clic).

## Deux niveaux : storyboard et animation

- **Storyboard** (au démarrage) : le film est une suite de **séquences**, découpées en **scènes**,
  découpées en **panneaux**. En bas, la bande de montage montre les panneaux (vignette, largeur
  proportionnelle à la durée) sous deux barres : séquences et scènes.
  - Nouveau panneau (P), nouvelle scène (Maj+P), nouvelle séquence : icônes de la barre du montage ; le menu « … » à côté duplique ou
    supprime un panneau ou une scène entière. Les barres du montage et de la timeline tiennent sur
    une ligne : des icônes, dont le nom et le raccourci s'affichent au survol.
  - F5 coupe le panneau à l'image courante (la suite devient un nouveau panneau, avec ses propres
    copies des dessins) ; « Réunir le panneau et le suivant » fait l'inverse. Une scène se coupe
    au panneau courant, ou se joint à la précédente (menu Storyboard, ou « … » du montage).
  - On réordonne les panneaux en les faisant glisser. Un panneau posé dans une autre scène la
    rejoint (à la frontière entre deux scènes : celle qui suit). Le bord droit d'un panneau règle
    sa durée.
  - Plusieurs panneaux : Ctrl + clic en ajoute ou en retire un, Maj + clic choisit une suite ;
    glisser l'un d'eux déplace tout le groupe. Après un clic dans le montage (ou sur une
    vignette), Ctrl+C copie les panneaux choisis, Ctrl+V les colle après le panneau courant (avec
    leurs propres copies des dessins), Ctrl+X les coupe et Suppr les supprime ; un clic dans le
    dessin rend ces touches au dessin. Mêmes commandes : icônes de la barre du montage, clic droit,
    menu Storyboard. Ctrl+Z annule.
  - Numérotation automatique : scènes 1, 2, 3… dans l'ordre du film, panneaux 1, 2… dans chaque
    scène. Une scène ou une séquence peut recevoir un nom ou un numéro à soi (fiche à droite).
  - Chaque panneau a sa durée et ses champs Action / Dialogue / Notes.
  - La lecture du montage (▶ de sa barre) enchaîne tout (animatique), en boucle si on veut, avec le son.
  - Vue « Vignettes » (Maj+Tab) : une **planche** de pages blanches où les panneaux se suivent,
    numérotés, de gauche à droite. En haut : taille des vignettes, colonnes (6 au plus), lignes
    par page (« toutes » : une seule page), écart, « Scène à la ligne », « Attacher les notes ».
    - On dessine directement dans la vignette du panneau courant ; avec l'outil Caméra, elle
      montre la feuille et le cadre pour recadrer.
    - Notes, partout sur la planche : pinceau ou crayon, un trait ; outil Texte, un texte (Ctrl +
      Entrée pour le poser, double-clic pour le modifier) ; gomme, efface les traits touchés ;
      outil Sélection, choisir des notes (clic, Ctrl + clic, lasso) et les glisser. Une note libre
      reste où on l'a posée ; une note attachée suit sa vignette et grandit avec elle (clic droit
      sur des notes : attacher, détacher, couleur courante, taille du texte, supprimer ; Suppr).
    - Molette : défiler ; Ctrl + molette : zoom ; Espace, bouton du milieu ou outil Main :
      déplacer la planche ; le bouton du zoom met la page à la largeur de la vue.
    - Clic : choisir un panneau ; Ctrl + clic : en ajouter ; Maj + clic : une suite ; outil
      Sélection : lasso. Glisser les panneaux choisis (le courant par son titre) les place avant
      ou après d'autres, dans une autre scène aussi. Clic droit : animer, nouveau panneau,
      dupliquer, supprimer les panneaux choisis, nouvelle scène, recadrer.
  - Sous la bande : une piste par calque (le calque de chaque panneau à ce rang, ses dessins
    exposés), une piste « Fond » par scène, et au-dessus des panneaux la piste Caméra (clés,
    mouvements). Un clic sur un calque, dans sa piste ou dans les noms à gauche, le rend courant.
  - On dessine directement sur le panneau dans la vue « Dessin » : un nouveau dessin y est tenu
    sur toute la durée du panneau.
- **Son** : sous les panneaux, des pistes son.
  - « Importer un son » (bouton des notes de musique, ou un fichier lâché sur la fenêtre) importe un WAV, MP3, FLAC ou OGG à l'image
    courante, sur la première piste libre.
  - Chaque extrait montre sa forme d'onde. On le déplace en le faisant glisser (aussi d'une piste à
    l'autre) ; ses bords cachent ou montrent le début ou la fin du son.
  - Clic droit : couper le son à cet endroit, supprimer l'extrait (ou Suppr), importer ici, régler
    la piste. Le bouton M d'une piste la rend muette.
  - La lecture suit le son : l'image reste synchronisée. En changeant d'image hors lecture, on
    entend une image de son (« son au défilement », menu Son).
  - Pendant la lecture, la barre d'état indique la cadence vraiment affichée (« Lecture 24,0 i/s ») ;
    elle passe en orange si la machine saute des images pour rester calée sur le son.
  - Le son est en temps de montage : déplacer ou rallonger un panneau ne déplace pas le son.
- **Animation** (Tab, ou double-clic sur un panneau) : on entre dans le panneau, avec sa timeline,
  ses calques, ses expositions, la pelure d'oignon et la lecture en boucle. Tab ramène au storyboard.

## Calques vectoriels et bitmap

Chaque panneau a ses calques, de deux sortes :

- **Vectoriel** (V, par défaut, façon Storyboard Pro) : les traits restent des objets modifiables.
  - Pinceau et crayon tracent des traits à épaisseur variable (pression du stylet comprise).
  - La gomme coupe les traits, ou retire des traits entiers en mode « traits entiers ».
  - Le pot de peinture crée une zone remplie qui passe sous les traits. Sur un trait ou une zone
    déjà peinte, il en change la couleur.
  - Les formes (ligne, rectangle, ellipse, pleines ou non) se tracent avec l'outil Formes (Alt+R).
  - L'outil Sélection (Alt+S) prend au clic ou au lasso (Maj pour ajouter). Ses poignées servent à
    déplacer, mettre à l'échelle (Maj : proportionnel) et tourner (Maj : par 15°). On peut aussi
    appliquer la couleur courante, épaissir, affiner, faire un miroir, passer devant ou derrière,
    supprimer. Copier, couper et coller (Ctrl+C/X/V) marchent d'un panneau à l'autre.
- **Bitmap** (B, façon TVPaint) : peinture en pixels, avec le pinceau, le crayon à pixels nets,
  la gomme, le remplissage avec débord sous le trait et l'import d'images PNG/JPEG.
  - L'outil Sélection y découpe une zone au lasso ou au rectangle (Maj : ajouter). On la déplace
    en la faisant glisser (au pixel près, sans flou), ses poignées la mettent à l'échelle ou la
    tournent, Alt+H / Alt+V la retournent. Tant que la zone reste sélectionnée, chaque
    transformation repart de l'image d'origine : tourner deux fois ne floute pas deux fois.
  - Suppr efface dedans, Maj+Suppr efface autour, Ctrl+C / X / V copient, coupent, collent
    (la zone collée arrive à sa place, prête à être déplacée). Échap ou un autre outil la pose ;
    Ctrl+Z annule toute la transformation d'un coup.

Les traits vectoriels sont rendus à la résolution du projet : en zoomant très fort, on voit les pixels
(comme à l'export), mais le trait reste modifiable.

L'outil **Transformer** déplace, agrandit ou tourne un calque entier (tous ses dessins) : glisser
dans la boîte, tirer un coin, tirer la poignée ronde (Maj : contraintes). Les dessins ne sont pas
modifiés, Ctrl+R annule la transformation, et on peut continuer à dessiner sur le calque.

**Multiplan** : le réglage « Profondeur » d'un calque (clic droit sur le calque) fixe sa distance. Quand la
caméra se déplace, un décor à 1 défile deux fois moins vite, un premier plan à −0,3 défile plus
vite ; à 0, rien ne change. Visible à l'écran, en vue caméra, à l'export et dans les vignettes.
Affichage > Vue de dessus montre la caméra, son champ et chaque calque à sa distance : glisser un
calque vers le haut l'éloigne, vers le bas le rapproche (Maj : par pas de 0,1).

Calques (menu Calques, panneau de droite) : W copie le calque courant et X le colle dans un autre
panneau (avec ses dessins et sa timeline, ajustée à la durée du panneau). Alt+L verrouille le
calque (cadenas) : on ne peut plus dessiner dessus. Ctrl+E le fusionne avec le calque du dessous :
deux calques vectoriels restent vectoriels, sinon le résultat est bitmap ; les deux calques doivent
être visibles et non verrouillés.

Le panneau Calques montre la vignette de chaque calque à l'image courante. En haut, le **mode de
fusion** du calque et son opacité : les 27 modes des logiciels de retouche (Fondu, Produit,
Densité couleur +, Superposition, Lumière vive, Différence, Soustraction, Division, Teinte,
Luminosité…), rangés par famille ; le calque se mélange à ce qui est dessous, à l'écran comme à
l'export. Fusionner un tel calque avec celui du dessous applique le mode aux pixels. Double-clic
sur le nom, ou clic droit > « Renommer… », pour renommer ; le clic droit donne aussi Toute la
scène, Profondeur, Dupliquer, Verrouiller, Supprimer. **Nouveau calque de fond** (icône des montagnes) crée,
tout en bas, un calque bitmap commun à toute la scène (un décor) : ce qu'on y dessine se voit dans
chaque panneau de la scène, pendant tout le panneau. « Toute la scène » étend n'importe
quel calque de la même façon, ou le rend à son panneau (chaque panneau garde alors sa copie).
Couper ou copier une scène lui donne son propre fond ; un panneau déplacé dans une autre scène
prend le fond de celle-ci.

## Le reste

- **Tablette** : la pression module la taille et/ou l'opacité. Elle passe par Windows Ink : sur une
  Wacom, l'option « Utiliser Windows Ink » doit être cochée dans les propriétés de la tablette.
  Sur le dessin, la croix de Windows marque la pointe sans retard, le cercle autour montre la
  taille du pinceau (Affichage > « Croix du système sur le pinceau » pour n'avoir que le cercle).
- **Panneaux** : chaque panneau (Outils, Couleur, Pinceaux, Réglages, Vue, Guides, Fiche,
  Calques, Montage ou Timeline) est un onglet. Glisse un onglet : des boutons apparaissent au milieu du panneau
  survolé, lâche-le sur l'un d'eux pour l'aimanter (à gauche, à droite, dessus, dessous, ou en
  onglet), ou ailleurs pour qu'il flotte. Tire les bords entre panneaux pour les redimensionner.
  La croix d'un onglet le ferme ; Affichage > Panneaux le fait revenir à sa place. Clic droit
  sur un onglet : « Dans une fenêtre à part », pour le poser sur un autre écran (le fermer le
  remet à sa place). Les commandes sont des icônes, partout où c'est possible : la souris posée
  une seconde sur l'une d'elles ouvre une bulle avec son nom, son raccourci et ce qu'elle fait.
  En haut, une barre de commandes ; à gauche, la barre d'outils verticale (Affichage > Panneaux
  pour les masquer). Couleur : teinte, saturation, valeur et nuancier. Réglages du pinceau : une
  grille de réglages, chacun avec l'aperçu de son trait et sa taille. Panneau **Pinceaux** (onglet
  à côté de Couleur) : les mêmes réglages en liste, avec un grand aperçu du trait, la taille, et
  « texture » / « importé » ; ceux de la gomme quand la gomme est choisie ; l'icône d'import pour
  les pinceaux Toon Boom. Les aperçus montrent le trait dans la couleur courante, sur le papier.
  L'interface est sombre et dense (gris et bleu d'accent repris de Pelure Studio, police Segoe
  UI). Les pinceaux texturés
  (pointe en image, grain du papier, pinceaux importés) marchent aussi sur un calque vectoriel :
  le trait reste modifiable, et ses textures sont enregistrées avec le projet.
- **Formes** : Ligne (droite, courbe : tracer puis glisser pour courber ; courbe en S : deux
  glissers), Rectangle, Ellipse (Maj : carré, cercle), Polyligne (un clic par point, double-clic
  ou Entrée pour finir, clic sur le premier point pour fermer).
- **Texte** : clic pour poser un texte, Ctrl+Entrée pour le valider ; police et taille dans
  Réglages. Sur un calque vectoriel il reste modifiable (clic dessus avec l'outil Texte).
- **Tampon** (calque bitmap) : des images posées le long du trait (feuille, herbe, caillou…, ou
  tes images importées), avec variations au hasard ; Ctrl + clic sur plusieurs tampons pour les
  alterner. **Tampon de duplication** : Ctrl + clic pour la source, puis peindre.
- **Perspective** : clic sur le dessin, puis tirer ses coins (Ctrl+Z annule chaque geste).
- **Pinceaux texturés** (calque bitmap, panneau Réglages) : liste de réglages (pinceau rond, crayon à
  papier, craie grasse, encre, gommes) ; « Papier » et « Grain » donnent un grain fixe sur le
  dessin, que la couleur remplit quand on appuie. L'icône d'import lit un fichier de pinceaux Toon
  Boom (`.xml`, par exemple `STB_TextureBrushes_preset.xml`) : ses pinceaux, pointes et papiers
  s'ajoutent à la liste et restent pour les fois suivantes.
- **Pelure d'oignon** (Alt+O) : par défaut 3 dessins avant en rouge et 3 après en vert, de plus en
  plus pâles. Dans la timeline : nombre avant / après, et l'engrenage (compter par dessin ou par
  image, opacité, teinte ou vraies couleurs, couleurs). Les réglages sont retenus pour la prochaine
  fois.
- **Table lumineuse** (panneau Vue, ou menu Affichage) : les autres calques du panneau sont
  estompés, le calque sur lequel on dessine reste net.
- **Guides de perspective** (panneau Guides) : 1, 2 ou 3 points de fuite, que l'on déplace à la
  souris. G les affiche, Maj+A aligne les traits (le trait suit la fuyante, la verticale ou
  l'horizontale la plus proche de son départ), Maj+L verrouille les points, Maj+G montre des lignes
  de référence. Les guides sont enregistrés avec le projet.
- **Vue** : zoom à la molette (ou 1 / 2), rotation (Maj + molette, ou V / C), miroir (M),
  déplacement (Espace + glisser).
- **Annuler / rétablir** (Ctrl+Z / Ctrl+Maj+Z) : 200 étapes, sur tout le projet : traits, vectoriel,
  sélections, panneaux, scènes, séquences, calques, sons, textes, noms, réglages des calques et des
  pistes, guides, papier, cadence. Une saisie ou un glissé de curseur compte pour une seule étape.
- **Espaces de travail** (Affichage > Espaces de travail) : Standard, Dessin (plus de place pour
  dessiner), Timeline (bande du bas plus haute), Vue d'ensemble (vignettes) ; « Enregistrer la
  disposition… » garde la tienne sous un nom (panneaux aimantés et flottants compris).
  Affichage > Panneaux masque ou affiche chaque panneau. Pelure rouvre avec la disposition de la
  dernière fois.
- **Copie de secours** : toutes les minutes (menu Fichier pour changer, 0 = jamais), Pelure copie
  le projet modifié à part, sans toucher au projet lui-même. Après un plantage, au démarrage
  suivant, Pelure propose d'ouvrir la copie ; « Enregistrer » la remet dans le projet.
- **Export** (menu Fichier) :
  - « Exporter le film / des images / le son… » ouvre une fenêtre : film MP4 (pour regarder,
    envoyer) ou MOV ProRes (pour un logiciel de montage), images PNG, ou son seul (WAV) ; tout le
    montage, la scène ou le panneau courant ; vidéo en taille réelle ou moitié, avec ou sans son.
    Une barre de progression indique le temps restant ; « Annuler » arrête et efface la vidéo
    inachevée. La vidéo est fabriquée par **ffmpeg**, que Pelure cherche sur le poste (il est
    installé avec Krita, par exemple) ; « Choisir un autre ffmpeg… » sinon.
  - « Exporter le storyboard (PDF)… » : planches A4 paysage, 6 panneaux par page, avec titre,
    durée, action, dialogue et notes ; une nouvelle page à chaque séquence.
  - Image courante (PNG), et storyboard en page HTML (une planche par panneau).

## Appli compagnon (Tauri)

`src-tauri/` + `src-frontend/` : une seconde appli (Tauri 2, React, Tailwind) sur le même noyau
que Pelure, pour le storyboard (panneaux, textes, durées, ordre, PDF) ; le dessin reste dans
Pelure. Lancer : `cd src-frontend` puis `npm run tauri dev`. Détails : `docs/COMPAGNON.md`.

## Raccourcis clavier

Ils sont repris de Storyboard Pro 27 (raccourcis par défaut) et de tes réglages personnels, et
rangés dans `keymap.toml`. F1 les affiche tous, avec leur origine. Pour les changer sans recompiler :
« Aide > Modifier les raccourcis », puis « Aide > Recharger les raccourcis ». Détails et choix pour
le clavier AZERTY : `docs/reference/sbpro_presets/README.md`.

Documentation technique : `docs/ARCHITECTURE.md` ; avancement : `docs/ROADMAP.md`.

## Projet en dossier

« Enregistrer » crée un dossier au nom du projet :

```
Bus/
  Bus.pelure        manifeste lisible (réglages, séquences > scènes > panneaux > calques)
  dessins/12.png    un fichier par dessin bitmap
  dessins/13.json   un fichier par dessin vectoriel
  sons/14.mp3       les sons importés, copiés tels quels
  modeles/15.pelure3d  les modèles 3D de repère
```

On ouvre le projet en choisissant `Bus.pelure`. Un enregistrement n'écrit que les dessins modifiés,
et chaque fichier est écrit à côté puis renommé : un plantage pendant l'enregistrement n'abîme pas la
version précédente. Les anciens fichiers `.pelure` (un seul fichier) s'ouvrent toujours : chaque
ancien « plan » devient une scène d'un panneau, et le premier « Enregistrer » crée le dossier.

**Cadre et feuille** : la feuille de dessin est deux fois plus grande que le cadre de la caméra (la
taille de l'image finale, choisie à la création du projet). Le cadre est tracé (« Caméra »), le
hors-champ est estompé autour ; on peut y dessiner pour préparer des panoramiques. Les exports et
les vignettes montrent ce que voit la caméra. Un projet enregistré avant cette version est converti
à l'ouverture (feuille agrandie, même cadrage) et passe au nouveau format au premier
« Enregistrer » ; une version plus ancienne de Pelure ne pourra plus l'ouvrir.

**Caméra** : chaque panneau a sa caméra. Un panneau sans image clé garde la caméra du panneau
d'avant dans la scène (elle ne revient plus au centre en changeant de panneau). Avec l'outil
Caméra, on glisse dans le cadre pour le déplacer, on tire un coin pour zoomer, la poignée ronde
pour tourner (Maj : par 15°). Le panneau Réglages de l'outil Caméra est sa fenêtre : images clés
(début, ici, fin, précédente, suivante, retirer), « Adoucir l'entrée / la sortie » en images,
position X, Y et Z (Z : distance au plan de référence, pour avancer dans le décor), focale en mm,
rotation, bascule (regarder vers le haut ou le bas) et panoramique (vers la gauche ou la
droite), vue, durées de la scène et du panneau. Tournée, la caméra filme la feuille en
perspective : sur le plateau, son cadre devient un trapèze ; la vue caméra, l'export et les
vignettes montrent l'image en perspective.

**Mouvements de caméra** : « Clé début », « Clé ici », « Clé fin » (timeline d'animation ou outil
Caméra) posent des images clés ; on règle la caméra sur chacune, elle va de l'une à l'autre. Les
adoucis la font accélérer en partant et freiner en arrivant. Avancer (Z) agrandit chaque calque
selon sa profondeur : le premier plan vient plus vite que le décor. Les clés apparaissent dans la
ligne « Caméra » de la timeline et en petits losanges dans la bande de montage ; on les y déplace
en les faisant glisser (une clé ne passe pas par-dessus sa voisine ; Ctrl+Z annule).

**Plateau, caméra, 3D** (barre au-dessus du dessin) : « Plateau » montre toute la feuille avec le
cadre de la caméra ; « Caméra » montre ce qui sera filmé (on peut y dessiner) ; en revenant au
plateau, la vue est celle qu'on avait laissée. « Sécurité » trace les zones de sécurité (action
93 %, titres 90 %) ; « Format » trace un cadre 16:9, 9:16, 4:5, 1:1 ou personnalisé dans le cadre
de la caméra, le reste estompé, pour composer une version pour les réseaux. Ces cadres suivent la
caméra et ne s'exportent pas.

**Plateau en 3D** (bouton 3D, barre de commandes ou barre de la vue) : les calques du panneau
sont des plans posés à leur profondeur, la caméra une pyramide de vision (avec sa course si elle
bouge), sur un sol quadrillé. Outils : tourner autour, glisser la vue, avancer / reculer ; partout,
molette = avancer, bouton du milieu = glisser, clic droit ou Alt + glisser = tourner. Outil
Déplacer : glisser un calque le déplace dans son plan, Maj + glisser change sa profondeur ; prendre
l'œil de la caméra la déplace (Maj : avancée ; Ctrl : la tourner, panoramique et bascule). Boutons Face, Dessus, Côté, « Par la caméra », et
recadrer. Pour dessiner, on revient au plateau à plat (bouton 3D).

**Repère 3D** : exporte ton décor en glTF depuis Blender (Fichier > Exporter > glTF 2.0, `.glb`),
puis Fichier > « Importer un modèle 3D (glTF) comme repère… ». Il apparaît en fil de fer sur la
feuille, comme un guide pour dessiner (il ne s'exporte pas). Outil « Repère 3D » : glisser pour
tourner autour, Maj + glisser pour déplacer, Ctrl + glisser pour avancer ou reculer ; azimut,
hauteur, distance et focale se règlent aussi en chiffres. Chaque panneau a son point de vue
(« Comme le panneau précédent » le recopie). « Dessiner dans un calque » transforme le fil de fer
en calque bitmap, tout en bas, qui lui s'exporte. Le modèle est gardé dans le projet (`modeles/`).

## Organisation du code

| Fichier | Rôle |
|---|---|
| `lib.rs`, `main.rs` | Bibliothèque (tous les modules, `lancer`) ; l'exécutable ne fait que la lancer |
| `noyau.rs` | Noyau pour l'appli compagnon : projet, résumé, vignettes PNG, panneaux, textes, PDF |
| `document.rs` | Séquences, scènes, panneaux, calques, expositions, dessins ; opérations de montage |
| `raster.rs` | Images en tuiles de 64×64 allouées à la demande |
| `vector.rs` | Éléments vectoriels : rendu, sélection, transformations, gomme, contours de remplissage |
| `pixsel.rs`, `pixtools.rs` | Sélection bitmap : zones, pixels soulevés, transformations ; outil |
| `guides.rs` | Guides de perspective : géométrie, affichage, traits alignés |
| `brush.rs` | Moteur de pinceau : empreintes, pression, lissage, pointe en image, grain |
| `brushtex.rs`, `brushui.rs` | Pointes, papiers, réglages nommés ; leur panneau |
| `tbbrush.rs` | Import des pinceaux Toon Boom (XML) |
| `onion.rs` | Pelure d'oignon |
| `fill.rs` | Remplissage par zone et débord |
| `render.rs` | Composition sur processeur (export, vignettes, secours), image filmée par la caméra |
| `gpu.rs` | Composition et affichage par la carte graphique, envoi des dessins par tuiles |
| `playback.rs` | Lecture fluide : dessins envoyés d'avance, cadence affichée |
| `export.rs`, `exportui.rs` | Exports : film (ffmpeg), images, son, PDF ; fenêtres de choix et de progression |
| `pdf.rs` | Écriture du PDF de storyboard : texte, images, mise en page |
| `settings.rs` | Réglages de l'application (emplacement de ffmpeg) |
| `camera.rs`, `camtools.rs` | Caméra du panneau : images clés, adoucis, avancée, focale ; outil Caméra, sa fenêtre, vue caméra |
| `viewbar.rs`, `frames.rs` | Barre au-dessus du dessin (plateau / caméra / 3D) ; zones de sécurité, cadre de format |
| `stage3d.rs`, `stage3dui.rs` | Plateau en 3D : géométrie (plans, caméra, rayons) ; affichage, navigation, outil Déplacer |
| `layertools.rs` | Outil Transformer (calque entier) |
| `topview.rs` | Vue de dessus (profondeur des calques) |
| `model3d.rs`, `view3d.rs`, `ref3dtool.rs` | Repère 3D : lecture glTF, arêtes utiles ; caméra en orbite, fil de fer, arêtes cachées ; outil et affichage |
| `thumbs.rs` | Cache des vignettes de panneaux |
| `history.rs` | Annuler / rétablir |
| `project.rs` | Projet en dossier (format 11), enregistrement préparé puis écrit |
| `blend.rs` | Modes de fusion des calques (formules, composition d'un pixel) |
| `board.rs`, `boardui.rs` | Planche de la vue Vignettes : disposition, notes, gestes, menus |
| `recovery.rs`, `recoveryui.rs` | Copie de secours, récupération après plantage |
| `workspace.rs`, `workspaceui.rs` | Espaces de travail, disposition retenue |
| `dock.rs`, `dockui.rs` | Panneaux modulables : disposition (format de Pelure), contenu des onglets, fenêtres à part |
| `shapes.rs`, `shapetools.rs` | Formes : ligne (courbes), rectangle, ellipse, polyligne |
| `text.rs`, `texttools.rs` | Texte : polices du poste, mise en page, outil |
| `stamp.rs`, `stamptools.rs` | Tampon et tampon de duplication |
| `persp.rs`, `perstools.rs` | Déformer en perspective |
| `settings.rs` | Réglages de l'application (ffmpeg, pelure, copie de secours, dispositions) |
| `io.rs` | Anciens fichiers `.pelure`, écriture d'images |
| `audio.rs` | Décodage des sons, formes d'onde, mixage et sortie audio |
| `keymap.rs` | Raccourcis : actions, lecture de `keymap.toml` |
| `app.rs` | État de l'application, actions, raccourcis |
| `vtools.rs` | Outils des calques vectoriels |
| `storyboard.rs` | Bande de montage, vue vignettes, fiche du panneau |
| `montagerows.rs` | Pistes du montage : caméra, calques, fond de scène, noms des pistes |
| `toolbars.rs`, `colorui.rs`, `layersui.rs`, `brushui.rs` | Barres de commandes et d'outils, panneau Couleur, panneau Calques, grille des pinceaux |
| `soundtrack.rs` | Pistes son : import, lecture synchronisée, affichage, glisser des extraits |
| `canvas.rs`, `panels.rs`, `timeline.rs` | Interface |

`cargo test --release` lance les tests (pinceau, vectoriel, remplissage, séquences/scènes/panneaux,
annulation, projet en dossier, anciens fichiers, raccourcis, son, caméra, transformation,
profondeur, vue de dessus, lecture, repère 3D, formes, texte, tampons, perspective, couleur,
panneaux modulables, calques de toute la scène, planche, modes de fusion, cadres de composition,
plateau en 3D).

## Compilation (Windows)

Depuis la phase 2, le projet se compile avec la toolchain Rust **MSVC**
(`stable-x86_64-pc-windows-msvc`, fixée par `rust-toolchain.toml`). Elle demande les outils de
compilation Microsoft : « Visual Studio Build Tools 2022 », charge de travail C++
(`winget install Microsoft.VisualStudio.2022.BuildTools`, avec `Microsoft.VisualStudio.Workload.VCTools`).

### Ancienne toolchain GNU (gardée en secours)

Avant la phase 2, le projet utilisait `x86_64-pc-windows-gnu`, qui n'embarque qu'un MinGW minimal.
Les contournements sont encore là, pour pouvoir y revenir en supprimant `rust-toolchain.toml` :

- `Cargo.lock` fige `parking_lot` 0.12.3, `parking_lot_core` 0.9.10 et `libloading` 0.8.6 (les
  versions récentes exigent un `dlltool` complet).
- `link/libshlwapi.a` + `build.rs` fournissent une bibliothèque d'import manquante (sans effet avec
  MSVC).

Ils pourront être retirés quand on n'aura plus besoin de ce secours (les versions figées seront
alors libérées lors d'une mise à jour des bibliothèques).
