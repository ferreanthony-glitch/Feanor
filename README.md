# Pelure

Logiciel d'animation 2D écrit en Rust avec egui : storyboard en séquences et plans (chaque plan,
une timeline de calques façon TVPaint, ses images de storyboard marquées), animation image par
image, et dessin vectoriel modifiable.

## Lancer

```
cargo run --release
```

L'exécutable est ensuite dans `target\release\pelure.exe`. On peut lui passer un projet :
`pelure.exe "C:\Films\Bus\Bus.pelure"` (et donc associer les fichiers `.pelure` à Pelure pour les
ouvrir d'un double-clic).

## Deux niveaux : storyboard et animation

- **Storyboard** (au démarrage) : le film est une suite de **séquences**, découpées en **plans**
  (13a : plus de panneaux). Chaque plan a une seule timeline de calques, comme un plan de TVPaint.
  En bas, la bande de montage montre les plans (vignette, largeur proportionnelle à la durée) sous
  la barre des séquences.
  - **Images de storyboard** : le début d'un plan est sa première image ; une **marque d'image**
    (drapeau orange, bouton de la timeline ou menu Storyboard) en pose une de plus à l'image
    courante, avec ses propres textes Action / Dialogue / Notes. Vignettes et PDF montrent une
    image par marque (une par plan sans marque).
  - **Projets d'avant les plans** : à l'ouverture, chaque ancienne scène devient un plan, ses
    panneaux mis bout à bout ; le début de chaque ancien panneau devient une marque d'image avec
    ses textes ; la caméra ne bouge pas aux jointures ; les calques se suivent par nom ; un fond
    « toute la scène » devient un calque ordinaire tenu sur tout le plan. Rien ne change à l'écran
    ni dans le temps (le son reste calé). Le fichier n'est réécrit qu'à l'enregistrement (format
    12, qu'une version plus ancienne de Pelure n'ouvre pas : garde une copie de tes projets).
  - Nouveau plan (P), nouvelle séquence : icônes de la barre du montage ; le menu « … » à côté
    duplique, copie, coupe ou réunit des plans. Les barres du montage et de la timeline tiennent
    sur une ligne : des icônes, dont le nom et le raccourci s'affichent au survol.
  - F5 coupe le plan à l'image courante (la suite devient un nouveau plan, avec ses propres
    copies des dessins ; une marque à cette image devient le début du nouveau plan) ; « Réunir le
    plan et le suivant » fait l'inverse (le début du suivant devient une marque d'image).
  - On réordonne les plans en les faisant glisser ; un plan reste un plan (il rejoint la séquence
    où on le pose). Le bord droit d'un plan règle sa durée.
  - Plusieurs plans : Ctrl + clic en ajoute ou en retire un, Maj + clic choisit une suite ;
    glisser l'un d'eux déplace tout le groupe. Après un clic dans le montage (ou sur une
    vignette), Ctrl+C copie les plans choisis, Ctrl+V les colle après le plan courant (avec
    leurs propres copies des dessins), Ctrl+X les coupe et Suppr les supprime ; un clic dans le
    dessin rend ces touches au dessin. Mêmes commandes : icônes de la barre du montage, clic droit,
    menu Storyboard. Ctrl+Z annule.
  - Numérotation automatique : plans 1, 2, 3… dans l'ordre du film. Un plan ou une séquence peut
    recevoir un nom ou un numéro à soi (fiche à droite).
  - La fiche montre les textes de l'image de storyboard où est la tête de lecture.
  - La lecture du montage (▶ de sa barre) enchaîne tout (animatique), en boucle si on veut, avec le son.
  - Vue « Vignettes » (Maj+Tab) : une **planche** de pages blanches où les images de storyboard
    se suivent, numérotées, de gauche à droite. En haut : taille des vignettes, colonnes (6 au
    plus), lignes par page (« toutes » : une seule page), écart, « Plan à la ligne », « Attacher
    les notes ».
    - On dessine directement dans la vignette de l'image courante ; avec l'outil Caméra, elle
      montre la feuille et le cadre pour recadrer.
    - Notes, partout sur la planche : pinceau ou crayon, un trait ; outil Texte, un texte (Ctrl +
      Entrée pour le poser, double-clic pour le modifier) ; gomme, efface les traits touchés ;
      outil Sélection, choisir des notes (clic, Ctrl + clic, lasso) et les glisser. Une note libre
      reste où on l'a posée ; une note attachée suit sa vignette et grandit avec elle (clic droit
      sur des notes : attacher, détacher, couleur courante, taille du texte, supprimer ; Suppr).
    - Molette : défiler ; Ctrl + molette : zoom ; Espace, bouton du milieu ou outil Main :
      déplacer la planche ; le bouton du zoom met la page à la largeur de la vue.
    - Clic : choisir une image (et son plan) ; Ctrl + clic : ajouter un plan ; Maj + clic : une
      suite ; outil Sélection : lasso. Glisser des plans choisis (le courant par son titre) les
      place avant ou après d'autres plans. Clic droit : animer, nouveau plan, dupliquer, copier,
      coller, supprimer, faire d'une image un nouveau plan, réunir avec le plan précédent,
      recadrer.
  - Sous la bande : une piste par calque (le calque de chaque plan à ce rang, ses dessins
    exposés), et au-dessus des plans la piste Caméra (clés, mouvements). Un clic sur un calque,
    dans sa piste ou dans les noms à gauche, le rend courant.
  - On dessine directement sur le plan dans la vue « Dessin » : un nouveau dessin y est tenu
    jusqu'à la fin du plan.- **Son** : sous les plans, des pistes son.
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
  - Le son est en temps de montage : déplacer ou rallonger un plan ne déplace pas le son.
- **Animation** (Tab, ou double-clic sur un plan) : on entre dans le plan, avec sa timeline,
  ses calques, ses expositions, la pelure d'oignon et la lecture en boucle. Tab ramène au storyboard.

## Calques vectoriels et bitmap

Chaque plan a ses calques, de deux sortes :

- **Vectoriel** (V, par défaut, façon Storyboard Pro) : les traits restent des objets modifiables.
  - Pinceau et crayon tracent des traits à épaisseur variable (pression du stylet comprise).
  - La gomme coupe les traits, ou retire des traits entiers en mode « traits entiers ».
  - Le pot de peinture crée une zone remplie qui passe sous les traits. Sur un trait ou une zone
    déjà peinte, il en change la couleur.
  - Les formes (ligne, rectangle, ellipse, pleines ou non) se tracent avec l'outil Formes (Alt+R).
  - L'outil Sélection (Alt+S) prend au clic ou au lasso (Maj pour ajouter). Ses poignées servent à
    déplacer, mettre à l'échelle (Maj : proportionnel) et tourner (Maj : par 15°). On peut aussi
    appliquer la couleur courante, épaissir, affiner, faire un miroir, passer devant ou derrière,
    supprimer. Copier, couper et coller (Ctrl+C/X/V) marchent d'un plan à l'autre.
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
plan (avec ses dessins et sa timeline, ajustée à la durée du plan). Alt+L verrouille le
calque (cadenas) : on ne peut plus dessiner dessus. Ctrl+E le fusionne avec le calque du dessous :
deux calques vectoriels restent vectoriels, sinon le résultat est bitmap ; les deux calques doivent
être visibles et non verrouillés.

Le panneau Calques montre la vignette de chaque calque à l'image courante. En haut, le **mode de
fusion** du calque et son opacité : les 27 modes des logiciels de retouche (Fondu, Produit,
Densité couleur +, Superposition, Lumière vive, Différence, Soustraction, Division, Teinte,
Luminosité…), rangés par famille ; le calque se mélange à ce qui est dessous, à l'écran comme à
l'export. Fusionner un tel calque avec celui du dessous applique le mode aux pixels. Double-clic
sur le nom, ou clic droit > « Renommer… », pour renommer ; le clic droit donne aussi Profondeur,
Dupliquer, Verrouiller, Supprimer. **Nouveau calque de fond** (icône des montagnes) crée, tout en
bas, un calque bitmap avec un seul dessin tenu sur tout le plan (un décor).

**En-tête des calques de la timeline** (13b, comme dans TVPaint). Chaque ligne montre :
- la vignette, le nom et le menu ⋮ ;
- en icônes : afficher, verrouiller, pelure d'oignon du calque, préserver la transparence (le
  pinceau ne colore que ce qui est déjà dessiné), pochoir et son sens + / − (le dessin du
  calque limite où l'on peint ailleurs), position verrouillée (ni Transformer, ni 3D, ni
  dessin), groupe de couleur ;
- la barre d'opacité à tirer, et le mode de fusion avec ◀ ▶.

Le triangle réduit la ligne. Glisser une ligne change le rang du calque ; double-clic sur le nom
pour le renommer. Ctrl + clic choisit d'autres calques, Maj + clic une suite (Maj + Haut / Bas au
clavier). Le menu ☰ (ou le menu Calques) agit sur les calques choisis : dupliquer, supprimer,
fusionner (les choisis, les visibles, tous). On y trouve aussi : choisir par groupe, tout
afficher, le calque courant seul, supprimer les calques vides ou cachés, éteindre ou inverser les
pochoirs, couper, coller à la place. Ctrl + Haut / Bas monte ou descend le calque. **Nouveau
calque…** ouvre une fenêtre : nom, sorte, groupe, mode de fusion, et place dans la pile.

**Groupes de couleur** (icône des nuanciers dans l'en-tête, ou menu Calques > Groupes > Éditer
les groupes…). Chacun des 13 groupes a un nom et une couleur, modifiables. On peut le cacher de la
timeline : ses calques restent dans l'image, seules leurs lignes disparaissent. Si le calque
courant est dans un groupe caché, l'en-tête l'annonce, et un clic sur la couleur remontre le
groupe. Le menu Groupes agit en une fois sur tous les calques, ceux du groupe courant ou ceux des
autres groupes : pelure, verrou, transparence, affichage, pochoir, fusion, et focus sur le groupe.

**Dossiers de calques** (icône du dossier + dans l'en-tête, ou ☰ > Dossiers). Un dossier range des
calques, sur un seul niveau. Son œil cache ses calques, son cadenas les verrouille, et son
opacité s'ajoute à la leur. Chaque calque garde son mode de fusion. On s'en sert ainsi :
- le triangle ferme le dossier : ses calques n'ont plus de ligne, mais restent à l'image ;
- « Placer les calques choisis dans un dossier » en crée un autour d'eux ;
- glisser une ligne entre les calques d'un dossier l'y fait entrer, la glisser ailleurs l'en
  fait sortir ;
- « Joindre » met les calques des dossiers choisis dans le dossier courant ;
- supprimer, dupliquer, copier ou coller un dossier emmène ses calques. « Supprimer uniquement
  le dossier » les garde, hors dossier.

On ne dessine pas sur un dossier.

**Pré / post-comportement** (menu ⋮ d'un calque, ou fenêtre « Nouveau calque… »). Avant la
première image dessinée du calque et après la dernière, il montre rien, la boucle de ses images,
un va-et-vient ou la première / dernière image maintenue. La timeline montre ces images en blocs
pâles ; on dessine toujours sur les vraies images.

**Images** (bouton pellicule de la timeline, ou menu Animation > Images). Maj + clic ou Maj +
glisser dans la grille choisit des images, sur les calques choisis ; Ctrl+X / C / V et Suppr
les visent ensuite. On peut :
- les supprimer, en choisissant le trou laissé : vide, comblé par le dessin d'avant, ou la suite
  qui recule ;
- les couper, les copier, les coller avant l'image courante ou à la place des images choisies ;
- les dupliquer, inverser leur ordre, insérer des images vides.

Sur tout le plan, ses clés de caméra et ses marques suivent : I ajoute une image après l'image
courante, Maj+I en insère une avant, et Maj+F5 supprime l'image.

Glisser les images choisies les déplace ; avec Ctrl elles sont copiées, avec Maj insérées. Le
même menu Images contient aussi :
- **Instances** (un dessin et ses expositions) : précédente / suivante (Ctrl + Gauche /
  Droite), tête / queue, casser à l'image courante, une exposition de plus ou de moins à chaque
  instance choisie ;
- **Faux-fixe** : rejouer les instances choisies N fois, en boucle, en va-et-vient ou au hasard ;
- **Étirer le calque** jusqu'à la fin du plan, ou le couper après l'image courante ;
- **Retourner** les images, horizontalement et / ou verticalement ;
- **Noms d'instance** : double-clic sur un dessin de la timeline pour le nommer, tout
  renommer, renommage intelligent.

Par calque (menu ⋮), deux réglages : la **cassure automatique**, où dessiner sur une image tenue
en fait un nouveau dessin, et la **création automatique** d'image.

**Repères** (bouton signet de la timeline) :
- **signets** : Ctrl+Maj+Haut en pose ou en retire un, Ctrl+Maj+Gauche / Droite va au
  précédent / suivant ;
- **points d'entrée et de sortie** : la lecture du plan reste entre eux, en boucle ou non.
  « Boucle entre les images choisies » les pose sur les images choisies ;
- **marques de couleur** sur les dessins d'un calque, pour s'y retrouver, avec précédente /
  suivante ;
- **numérotation** des images, en image, time code ou trame, à partir de l'image de départ du
  projet.

Le champ d'image courante de la barre accepte un numéro ou un time code. La loupe règle le
zoom de la timeline (aussi Ctrl + molette) et « Adapter » (au plan, au calque, aux images
choisies…). Elle allume aussi les croix sur les instances vides et les miniatures des dessins.

**Onglets du bas de la timeline** (un clic ouvre la bande, un second la ferme) :
- **Notes** : on dessine des notes dans une rangée alignée sur les images (crayon bleu, rouge ou
  noir, gommes, teinte, déplacer, découper). Elles suivent les images insérées ou supprimées et
  peuvent s'afficher par-dessus l'image pendant la lecture ; elles ne sont jamais exportées.
- **Son** : volume général, son à la lecture et au défilement. Par piste : muette, verrouillée,
  volume, dupliquer, supprimer. Ctrl + glisser l'onde dans la timeline décale l'extrait.
- **Groupes**.
- **Arrière-plan** : aucun, couleur (le papier, exporté) ou damier (pour voir les
  transparences ; l'export est alors transparent).
- **Image de départ** : le numéro de la première image.

**Feuille d'exposition** (Affichage > Panneaux > Feuille d'exposition) : le plan courant en
colonnes, une ligne par image. On y trouve :
- le numéro de l'image, les signets, l'entrée et la sortie ;
- le niveau du son ;
- une colonne par calque : le nom ou le numéro du dessin à sa première image, un trait tant
  qu'il est tenu, une croix au début des images vides ;
- les clés de caméra ;
- les textes Action et Dialogues / Notes du storyboard.

Un clic va à l'image et au calque. L'œil choisit les colonnes et le trait de séparation.

**Rôles des images** (clic droit sur une case de la timeline ou de la feuille > Rôle) :
- les rôles : clé (disque), breakdown (triangle), intervalle à faire / complété (cercle
  vide / pointé), intervalle spécial à faire / complété (carré vide / plein) ;
- ils se posent sur les images choisies des calques choisis, même vides ;
- ils suivent les images qu'on insère, supprime ou déplace ;
- le même menu montre ou cache les rôles dans la timeline, ou les limite au calque courant.

**Flips** (icône livre de la timeline) : feuilleter vite des images pour juger un mouvement.
Chaque flip a une sorte :
- basique : il défile tant qu'on tient le bouton ou la touche ;
- directionnel : on tient et on glisse la souris ;
- manuel : deux flèches pour l'image d'avant et d'après.

On règle aussi :
- ses limites : le plan, le calque, l'entrée / sortie, les signets, les marques, un rôle, ou
  N images autour ;
- ses checkpoints : toutes les images, les dessins, les signets, les marques, un rôle ;
- sa vitesse, la boucle, le retour à l'image de départ.

Les flips par rôle feuillettent les clés, les breakdowns ou les intervalles du calque.

**Pile d'effets** (baguette de la timeline) : des effets sur le calque courant, vus à l'écran et
à l'export, sans toucher aux dessins.
- Effets : flou gaussien (rayon) et flou de mouvement (longueur, angle).
- On les active, les ordonne ou les retire à tout moment.
- On peut les animer par clés : « Créer une clé » à une image, puis à une autre, et les
  paramètres vont de l'une à l'autre. Les clés sont des losanges violets dans la timeline.

**Calque CTG** (Calques > Calque CTG > Nouveau calque CTG) : un calque de couleur sous les
traits.
- Griffonne une couleur dans une zone fermée par les traits de ses sources : toute la zone se
  remplit, sous les traits.
- Retouche les traits ou les graffitis : le remplissage suit.
- Un petit trou dans les traits ne laisse pas fuir la couleur (jusqu'à 8 pixels, réglable).
- Le menu de la ligne du calque CTG règle la fermeture des trous, choisit ses sources, et
  offre « Charger la structure à partir des sources », qui donne un dessin de graffitis par
  dessin des traits.

**Calque vidéo** (Fichier > Importer une vidéo, ou une séquence d'images ; une vidéo peut
aussi être lâchée sur la fenêtre) : une référence pour la rotoscopie ou l'animatique.
- La vidéo est mise à la taille du cadre et rangée avec le projet. Elle se lit image par image,
  sans occuper la mémoire.
- Son son va sur une piste son.
- Le menu de la ligne du calque règle son début et « Transformer en calque d'animation » en
  fait des dessins. Il faut ffmpeg, le même que pour l'export.

**Interpolation** (menu Images > Interpolation) :
- Sur un calque vectoriel, de vrais intervalles : les traits passent d'une forme à l'autre.
  Sur un calque bitmap, un fondu.
- « Insérer une image intermédiaire » entre le dessin courant et le suivant.
- « Remplir les vides par des intervalles » entre deux clés.
- « Étirer en interpolant » donne une nouvelle durée aux images choisies.
- « Changer la cadence » passe le projet à une autre cadence en gardant les durées.

**Vue Projet** :
- **Nouveau plan avant le plan courant** (menu Storyboard, menu ⋯ du montage).
- **Cadenas du montage** : il verrouille le storyboard. L'ordre, le nombre et la durée des plans
  ne changent plus, et on n'y dessine plus : ni trait, ni commande d'images, ni calque ajouté
  ou supprimé. Restent permis l'affichage des calques, les repères, les notes et les noms.
- **Propriétés du projet** (Fichier) : titre, information, auteur, notes et copyright. On y voit
  aussi les dates de création et de dernière modification.
- **Nouveau projet à partir des plans choisis** (Fichier) : sans le son.

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
  plus pâles. Dans la timeline : nombre avant / après, et l'engrenage, qui ouvre le panneau de la
  pelure (aussi : clic droit sur la pelure d'un calque). On y trouve :
  - le mode : instance, image, signet ou marque de couleur ;
  - « Devant » : par-dessus le dessin courant ;
  - la colorisation avant et après, chacune à part, d'une couleur ou en dégradé de deux
    couleurs (de l'image la plus proche à la plus lointaine) ;
  - chaque image montrée ou cachée, avec son opacité, et l'opacité globale ;
  - les curseurs liés : l'opacité décroît en ligne droite ;
  - activer ou désactiver tout ;
  - des réglages rangés sous un nom ;
  - « Décaler » : glisser sur le dessin déplace les images de la pelure, sans toucher aux
    dessins, et « Remettre » les replace.

  Les réglages sont retenus pour la prochaine fois.
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
    montage, la séquence ou le plan courant ; vidéo en taille réelle ou moitié, avec ou sans son.
    Une barre de progression indique le temps restant ; « Annuler » arrête et efface la vidéo
    inachevée. La vidéo est fabriquée par **ffmpeg**, que Pelure cherche sur le poste (il est
    installé avec Krita, par exemple) ; « Choisir un autre ffmpeg… » sinon.
  - « Exporter le storyboard (PDF)… » : planches A4 paysage, 6 images de storyboard par page (le
    début de chaque plan, puis ses marques d'image), avec titre, durée, action, dialogue et notes ;
    une nouvelle page à chaque séquence.
  - Image courante (PNG), et storyboard en page HTML (une planche par image de storyboard).

## Appli compagnon (Tauri)

`src-tauri/` + `src-frontend/` : une seconde appli (Tauri 2, React, Tailwind) sur le même noyau
que Pelure, pour le storyboard (plans, textes, durées, ordre, PDF) ; le dessin reste dans
Pelure. Lancer : `cd src-frontend` puis `npm run tauri dev`. Détails : `docs/COMPAGNON.md`.

## Raccourcis clavier

Ils sont repris de Storyboard Pro 27 (raccourcis par défaut) et de tes réglages personnels, et
rangés dans `keymap.toml`. Pour en changer un :
- **clic droit** sur une icône ou une entrée de menu, « Assigner un raccourci… », puis taper la
  combinaison ;
- Pelure dit si elle sert déjà (l'autre commande la perd), puis propose « Remplacer » ou « Ajouter » ;
- la fenêtre a aussi « Retirer le raccourci » et « Raccourci par défaut ».

Fichier > Raccourcis clavier… (ou F1) liste toutes les commandes. On y trouve une recherche, les
mêmes boutons, et l'origine de chaque raccourci en bulle.

Les choix vont dans un fichier personnel (`%APPDATA%\Pelure\keymap.toml`), qui remplace, action par
action, le fichier par défaut. On peut aussi l'écrire à la main (« Aide > Modifier le fichier de
raccourcis à la main », puis « Recharger »). Détails et choix pour le clavier AZERTY :
`docs/reference/sbpro_presets/README.md`.

Documentation technique : `docs/ARCHITECTURE.md` ; avancement : `docs/ROADMAP.md`.

## Projet en dossier

« Enregistrer » crée un dossier au nom du projet :

```
Bus/
  Bus.pelure        manifeste lisible (réglages, séquences > plans > calques, marques d'image)
  dessins/12.png    un fichier par dessin bitmap
  dessins/13.json   un fichier par dessin vectoriel
  sons/14.mp3       les sons importés, copiés tels quels
  modeles/15.pelure3d  les modèles 3D de repère
```

On ouvre le projet en choisissant `Bus.pelure`. Un enregistrement n'écrit que les dessins modifiés,
et chaque fichier est écrit à côté puis renommé : un plantage pendant l'enregistrement n'abîme pas la
version précédente. Les anciens fichiers `.pelure` (un seul fichier) s'ouvrent toujours : chaque
ancien « plan » devient un plan de Pelure, et le premier « Enregistrer » crée le dossier.

**Cadre et feuille** : la feuille de dessin est deux fois plus grande que le cadre de la caméra (la
taille de l'image finale, choisie à la création du projet). Le cadre est tracé (« Caméra »), le
hors-champ est estompé autour ; on peut y dessiner pour préparer des panoramiques. Les exports et
les vignettes montrent ce que voit la caméra. Un projet enregistré avant cette version est converti
à l'ouverture (feuille agrandie, même cadrage) et passe au nouveau format au premier
« Enregistrer » ; une version plus ancienne de Pelure ne pourra plus l'ouvrir.

**Caméra** : chaque plan a sa caméra (sans image clé : au centre, cadre entier). Avec l'outil
Caméra, on glisse dans le cadre pour le déplacer, on tire un coin pour zoomer, la poignée ronde
pour tourner (Maj : par 15°). Le panneau Réglages de l'outil Caméra est sa fenêtre : images clés
(début, ici, fin, précédente, suivante, retirer), « Adoucir l'entrée / la sortie » en images,
position X, Y et Z (Z : distance au plan de référence, pour avancer dans le décor), focale en mm,
rotation, bascule (regarder vers le haut ou le bas) et panoramique (vers la gauche ou la
droite), vue, durée du plan. Tournée, la caméra filme la feuille en
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

**Plateau en 3D** (bouton 3D, barre de commandes ou barre de la vue) : les calques du plan
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
hauteur, distance et focale se règlent aussi en chiffres. Chaque plan a son point de vue
(« Comme le plan précédent » le recopie). « Dessiner dans un calque » transforme le fil de fer
en calque bitmap, tout en bas, qui lui s'exporte. Le modèle est gardé dans le projet (`modeles/`).

**Propager une peinture sur une vidéo** (façon EbSynth) : on peint quelques images clés sur un
calque posé au-dessus d'un calque vidéo, puis clic droit sur la ligne de ce calque > « Propager la
peinture sur la vidéo… ». Les autres images suivent le mouvement de la vidéo. Détails :
`docs/PROPAGATION.md`.

**Glisser et clic droit** (passe interface) :
- **Timeline du plan** : on prend à la souris les marques d'image (triangles de la règle), les
  signets, les crochets d'entrée et de sortie, les clés d'effets (losanges violets) et le bord droit
  d'un dessin, qui étire ou raccourcit son exposition (ce qui suit glisse d'autant).
- **Montage** : les marques d'image se glissent aussi dans les blocs des plans.
- **Calques** : on les monte ou les descend en les glissant, dans le panneau Calques, dans les noms
  des pistes du montage ou dans l'en-tête de la timeline.
- **Clic droit** : partout il ouvre un menu (règle, caméra, son, pistes, séquences, calques, dessin).
  Un clic droit sur une entrée de ces menus lui donne un raccourci.
- Échap abandonne un glisser en cours ; Ctrl+Z défait chaque geste en une fois.

**Réglages du projet** (Fichier) :
- **onglet Réglages** : titre, information, auteur, notes, copyright, numéro de la première
  image, dates ;
- **onglet Résolution** : des préréglages (HD 1080, HD 720, 4K UHD, 2K / 4K DCI, 1,85:1, Scope
  2,39:1, 4:3, carré, 4:5, vertical 9:16, PAL), la largeur et la hauteur (liées ou non), la
  cadence.

Une nouvelle taille met les dessins à l'échelle : dessins, caméra, guides et placement des
calques suivent, et avec d'autres proportions le dessin remplit le cadre sans être déformé. On
peut aussi les garder à leur taille en pixels : seul le cadre change, centré. Ce changement ne
s'annule pas : l'historique est vidé.

Une nouvelle cadence garde les durées en secondes (comme « Changer la cadence », avec ou sans
intervalles) ou garde les images.

**Projet Storyboard Pro** : Fichier > « Importer un projet Storyboard Pro (.sboard)… », ou glisser
le `.sboard` sur la fenêtre. Le fichier est seulement lu. Plans, séquences, panneaux (devenus les
images de storyboard de chaque plan, avec leurs légendes), durées, cadence, taille, noms des
calques et son arrivent tels quels ; la caméra n'est pas reprise. Pour les dessins : « Créer le
script… » écrit un script dans un dossier et le copie ; dans Storyboard Pro, projet ouvert, vue
Script Editor, choisir Sandbox, coller, puis Run : chaque calque de chaque panneau est exporté en
PNG dans ce dossier (le projet est remis comme avant). Revenir à Pelure, « Relire » (ou glisser
le `pelure.txt` du dossier sur la fenêtre), puis « Importer ».

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
| `onionui.rs` | Panneau de la pelure d'oignon (13f) |
| `bottomtabs.rs`, `notes.rs` | Onglets du bas de la timeline, notes dessinées (13e) |
| `projetui.rs` | Vue Projet : plan avant, storyboard verrouillé, propriétés, projet fait de plans (13e) |
| `xsheet.rs`, `xsheetui.rs` | Feuille d'exposition : ce que montre chaque case, le panneau (13g) |
| `roles.rs`, `rolesui.rs` | Rôles des images : modèle, suivi des images, symboles, menu (13g-2) |
| `flips.rs`, `flipsui.rs` | Flips : limites, checkpoints, pas, panneau et flip en cours (13h) |
| `fx.rs`, `fxui.rs` | Pile d'effets : clés, flous, dessins dérivés en cache, panneau (13i) |
| `ctg.rs`, `ctgui.rs` | Calque CTG : remplissage par graffitis, sources, structure (13j) |
| `video.rs`, `videoui.rs` | Calque vidéo : import par ffmpeg, images lues à la demande (13k) |
| `tween.rs`, `tweenui.rs` | Interpolation : intervalles vectoriels, fondus, étirement, cadence (13l) |
| `xml.rs`, `sboard.rs` | Lecture d'un XML en arbre ; lecture d'un projet Storyboard Pro (14a) |
| `sbscript.rs`, `sbimport.rs`, `sbui.rs` | Script d'export des dessins et sa liste ; projet Pelure construit ; fenêtre d'import (14a) |
| `stretch.rs` | Vitesse d'un son : rééchantillonnage, tempo sans changer la hauteur (WSOLA) (14a) |
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
| `keymap.rs` | Raccourcis : actions, lecture de `keymap.toml`, touche tapée, fichier personnel |
| `raccourcisui.rs` | Clic droit « Assigner un raccourci… », fenêtre d'assignation, liste des raccourcis |
| `propager.rs`, `propagerui.rs` | Propagation de la peinture sur une vidéo, façon EbSynth (voir `docs/PROPAGATION.md`) |
| `handles.rs` | Poignées de la timeline (marques, signets, entrée / sortie, clés d'effets, expositions) et menus de la grille |
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
