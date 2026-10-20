# SHOT — Frame the story

*Anciennement Pelure.* Le logiciel s'appelle SHOT depuis le 2026-10-18 ; son logo est dans
`assets/shot-logo.svg`.

Logiciel d'animation 2D écrit en Rust avec egui : storyboard en séquences et plans (chaque plan,
une timeline de calques façon TVPaint, ses images de storyboard marquées), animation image par
image, et dessin vectoriel modifiable.

Ce qui garde l'ancien nom, pour que rien d'existant ne casse : les projets (dossier du projet,
fichier `.pelure`, format `pelure-projet`), la bibliothèque Rust (`pelure::`, la caisse reste
`pelure`) et les variables d'environnement `PELURE_*`. La « pelure d'oignon » est un terme de
dessin, pas l'ancien nom du logiciel.

## Lancer

```
cargo run --release
```

L'exécutable est ensuite dans `target\release\shot.exe`. On peut lui passer un projet :
`shot.exe "C:\Films\Bus\Bus.pelure"` (et donc associer les fichiers `.pelure` à SHOT pour les
ouvrir d'un double-clic).

Les réglages, raccourcis, brosses importées, rapports d'erreur et copies de secours sont dans
`%APPDATA%\SHOT` et `%LOCALAPPDATA%\SHOT`. Au premier lancement, ce que l'ancien nom avait rangé
dans `...\Pelure` y est copié (jamais déplacé ni supprimé) s'il n'y a pas encore de dossier SHOT.

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
  - Nouveau plan (P), nouvelle séquence : icônes de la barre du montage ; le menu « Plans » (liste à
    puces) à côté duplique, copie, coupe ou réunit des plans. Les barres du montage et de la
    timeline tiennent sur une ligne qui ne défile pas : des icônes, dont le nom et le raccourci
    s'affichent au survol, et un bouton « Plus… » (trois points) pour le reste (voir « Barres
    minces et Plus… » plus bas).
  - F6 coupe le plan à l'image courante (la suite devient un nouveau plan, avec ses propres
    copies des dessins ; une marque à cette image devient le début du nouveau plan) ; « Réunir le
    plan et le suivant » fait l'inverse, sans laisser de repère (les textes du suivant rejoignent
    ceux de l'image d'avant).
  - On réordonne les plans en les faisant glisser ; un plan reste un plan (il rejoint la séquence
    où on le pose). Le bord droit d'un plan règle sa durée.
  - Plusieurs plans choisis (Ctrl ou Maj + clic) qui se suivent dans une même séquence :
    « Fusionner les N plans choisis » (clic droit, menu « … ») en fait un seul, sans repère aux
    jointures (leurs textes rejoignent ceux du plan). Plusieurs calques choisis (Ctrl ou Maj + clic dans le
    panneau Calques, la colonne du montage ou la timeline) : le bouton Fusionner les réunit.
  - La colonne de gauche du montage (Caméra, plans, calques) s'élargit en tirant son bord droit
    (double-clic : tout montrer, ou revenir étroite) ; sa largeur est retenue. Élargie, chaque
    calque y montre afficher, verrouiller, pelure d'oignon, son opacité, son mode de fusion et ce
    qu'il fait après sa dernière image (rien, boucle, va-et-vient, maintenir, en pâle dans ses
    cases). Clic droit sur un calque (ou ⋮) : tout ce qui le touche ; double-clic sur son nom :
    le renommer.
  - Le premier dessin sur un calque vide remplit tout le plan, du début à la fin, où que soit la
    tête de lecture (en animation : « en N » images à la tête de lecture, comme TVPaint).
  - A / F : clé précédente / suivante, tous les calques compris (dans tout le montage en
    storyboard, dans le plan en animation). Plan précédent / suivant : PgPréc / PgSuiv.
  - Plusieurs plans : Ctrl + clic en ajoute ou en retire un, Maj + clic choisit une suite ;
    glisser l'un d'eux déplace tout le groupe. Après un clic dans le montage (ou sur une
    vignette), Ctrl+C copie les plans choisis, Ctrl+V les colle après le plan courant (avec
    leurs propres copies des dessins), Ctrl+X les coupe et Suppr les supprime ; un clic dans le
    dessin rend ces touches au dessin. Mêmes commandes : icônes de la barre du montage, clic droit,
    menu Storyboard. Ctrl+Z annule.
  - Numérotation automatique : plans 1, 2, 3… dans l'ordre du film. Un plan ou une séquence peut
    recevoir un nom ou un numéro à soi (fiche à droite).
  - **Numérotation verrouillée** (Réglages du projet > Numérotation des plans) : chaque plan garde son numéro.
    Un plan inséré entre le 12 et le 13 devient le **12A** (puis 12B), un plan supprimé laisse un trou (son
    numéro n'est pas redonné), un plan déplacé garde son numéro. « Renuméroter les plans… » (menu Storyboard)
    remet 1, 2, 3… dans l'ordre du film, avec confirmation et une étape d'annulation. Détail :
    `docs/INTERFACE.md`.
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

- **Vectoriel** (Maj+S pour en créer un, par défaut, façon Storyboard Pro) : les traits restent des objets modifiables.
  - Pinceau et crayon tracent des traits à épaisseur variable (pression du stylet comprise).
    Deux interrupteurs en haut de leurs réglages, aussi sur un calque bitmap : **Peint derrière**
    (la peinture passe sous ce qui est déjà dessiné) et **Remplir la forme** (au lâcher, un trait
    fermé, dont le bout revient près du début ou qui se recoupe, remplit son intérieur de sa
    couleur, sous le trait ; en vectoriel, la zone reste modifiable).
  - La gomme coupe les traits, ou retire des traits entiers en mode « traits entiers ».
  - Le pot de peinture crée une zone remplie qui passe sous les traits. Sur un trait ou une zone
    déjà peinte, il en change la couleur.
  - Les formes (ligne, rectangle, ellipse, pleines ou non) se tracent avec l'outil Formes (Alt+R).
  - L'outil Sélection (Alt+S) prend au clic ou au lasso (Maj pour ajouter) et montre une **boîte façon
    Storyboard Pro** : un cadre, huit petites poignées carrées (coins et milieux des côtés) et un point
    rond de pivot au centre. Glisser dedans déplace ; un coin met à l'échelle dans les deux sens, le
    milieu d'un côté dans un seul ; juste à l'extérieur d'un coin, on tourne autour du pivot ; sur un
    bord, entre les poignées, on incline ; le point rond se tire ailleurs pour changer le pivot. Maj :
    proportions (échelle), un seul axe (déplacement), pas de 15° (rotation) ; Ctrl : autour du pivot au
    lieu du côté opposé (*touches à vérifier avec le vrai Storyboard Pro*). Le panneau Réglages donne
    les mêmes valeurs en chiffres (position, largeur, hauteur, angle, inclinaison), « Garder les
    proportions » et « Réinitialiser le pivot ». Chaque geste est une étape d'annulation à son nom
    (« Déplacer », « Mettre à l'échelle », « Tourner », « Incliner »). On peut aussi appliquer la couleur
    courante, épaissir, affiner, faire un miroir, passer devant ou derrière, supprimer. Copier, couper
    et coller (Ctrl+C/X/V) marchent d'un plan à l'autre.
  - **Plusieurs calques d'un coup** (2026-10-19, `multisel.rs`, `multitools.rs`) : on choisit les
    calques par Ctrl + clic (ajouter / retirer) ou Maj + clic (tous ceux entre le courant et le
    cliqué) sur leur ligne, dans le panneau Calques, la timeline ou la colonne du montage ; ils sont
    surlignés, le courant d'une cible (⌖), les autres d'une coche. Avec l'outil Sélection, le lasso, le
    rectangle et « Tout sélectionner » (Ctrl+A) prennent alors **sur tous les calques choisis** : les
    traits dans le lasso de chaque calque vectoriel, la zone de chaque calque bitmap, au dessin de
    l'image courante. Une seule boîte entoure le tout et chaque geste — déplacer, échelle, rotation,
    inclinaison, champs chiffrés, flèches, miroir (Alt+H / Alt+V) — s'applique à tous autour du
    même pivot, calque décalé, tourné ou en profondeur compris. Suppr, Ctrl+C / X / V marchent
    calque par calque ; un geste sur N calques est **une seule étape d'annulation**, à son nom. Sans
    latence : chaque calque a sa feuille flottante (jamais de rejeu du moteur pendant le geste ;
    3 calques de 30 traits texturés : 0,001 ms par image, saisie de 10 ms une fois). Les calques
    cachés, verrouillés, sans dessin à l'image, vidéo et les dossiers sont laissés de côté, la barre
    d'état et les Réglages disent lesquels et pourquoi (« 3 calques, 42 éléments »). L'outil
    **Transformer** déplace, tourne et agrandit (un coin) de même les calques choisis ensemble ;
    l'outil **Déformer** reste sur le calque courant seul (il le dit).
- **Bitmap** (Maj+B pour en créer un, façon TVPaint) : peinture en pixels, avec le pinceau, le crayon (anticrénelé ;
  « Pixels nets » pour le pixel art), la gomme, le remplissage avec débord sous le trait et
  l'import d'images PNG/JPEG.
  - L'outil Sélection y découpe une zone au lasso ou au rectangle (Maj : ajouter). On la déplace
    en la faisant glisser (au pixel près, sans flou) ou aux flèches (1 pixel, 10 avec Maj ; aussi
    pour des traits vectoriels choisis), sa boîte (mêmes gestes que sur un calque vectoriel : échelle,
    rotation, inclinaison, pivot) la transforme, Alt+H / Alt+V la retournent. Tant que la zone reste sélectionnée, chaque
    transformation repart de l'image d'origine : tourner deux fois ne floute pas deux fois.
  - Suppr efface dedans, **Maj+Suppr remplit la zone de la couleur courante** (une étape « Remplir la
    sélection » ; l'icône « Remplir » des réglages de l'outil fait pareil), **Maj+Retour arrière efface
    autour**, Ctrl+C / X / V copient, coupent, collent
    (la zone collée arrive à sa place, prête à être déplacée). Échap ou un autre outil la pose ;
    Ctrl+Z annule toute la transformation d'un coup. **Un clic (sans glisser) sur un dessin choisit la
    forme d'un seul tenant sous la souris** ; voir « Notes d'Anthony : dessin ».

Les traits vectoriels sont rendus à la résolution du projet : en zoomant très fort, on voit les pixels
(comme à l'export), mais le trait reste modifiable.

**Un trait vectoriel rend exactement comme le trait bitmap** (2026-10-18, étape 5 du rendu des
brosses, `docs/VECTORIEL-COMME-BITMAP.md`) : au lâcher, le trait garde son réglage complet et sa
trace (l'appui et l'instant de chaque point, le toucher, la couleur B) et rejoue le moteur des
pinceaux bitmap, empreinte par empreinte — crayon graphite, fusain, gouache, brosses TVPaint
importées, tout rend pareil sur les deux sortes de calques, et le trait ne change plus quand on lève
le stylet. Ses pixels sont gardés en mémoire en tuiles (environ 130 Ko par trait de crayon ; 16 Mo
au plus par trait, 512 Mo en tout, au-delà les moins récents sont oubliés et rejoués au besoin).
**Aucun geste n'attend le moteur** (règle d'Anthony, 2026-10-18 : « un logiciel qui réponde
toujours très rapidement, sans latence ») : déplacer, tourner, agrandir une sélection vectorielle,
la pousser aux flèches, la déformer à l'enveloppe ou en perspective pose ces pixels déformés à la
volée (une cinquantaine de traits texturés : 2 ms par image au glissé ; à l'enveloppe et en
perspective la carte graphique déforme la feuille par un maillage, 0,14 ms par image que ce
soit 10, 40 ou 300 traits, au lieu de 10 à 37 ms), et le rendu exact arrive quelques instants après le lâcher, d'un fil de travail (`rejeu.rs`), sans rien
bloquer ; au lever du stylet non plus, le trait n'est plus rejoué sur le fil de l'interface (174 ms
pour 2400 points de gouache avant). Un trait qui dépasse de la feuille garde ce qui dépasse :
déplacé vers l'intérieur, il se montre en entier. Les contours bleus d'une grosse sélection (des
centaines de traits) sont faits une fois, à la résolution de l'écran : 0,6 ms par image au lieu de
13,5 ms pour 300 traits de 500 points. À l'ouverture d'un projet, les dessins texturés
paraissent tout de suite en rendu rapide, puis exacts. Seule différence restante : la peinture humide (gouache) ne se mêle pas à ce que les
traits précédents ont posé, chaque trait étant rejoué seul. Les anciens dessins (sans trace)
rendent comme avant.

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

Le panneau Calques montre la vignette de chaque calque à l'image courante. Chaque sorte de calque a
sa couleur, partout (panneau Calques, montage, timeline) : bitmap vert, vectoriel bleu, CTG rose. En haut, le **mode de
fusion** du calque et son opacité : les 27 modes des logiciels de retouche (Fondu, Produit,
Densité couleur +, Superposition, Lumière vive, Différence, Soustraction, Division, Teinte,
Luminosité…), rangés par famille ; le calque se mélange à ce qui est dessous, à l'écran comme à
l'export. Fusionner un tel calque avec celui du dessous applique le mode aux pixels. Juste
dessous, les commandes en icônes restent en haut ; seule la liste défile. Double-clic sur le nom
pour renommer ; le clic droit sur un calque donne ce que le panneau ne montre pas déjà (pelure,
préserver la transparence, pochoir, position verrouillée, avant / après ses images, groupe,
renommer, profondeur, puis les autres commandes des calques). **Nouveau calque de fond** (icône des montagnes) crée, tout en
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

**Poignées de calque** (timeline). Sous les dessins de chaque calque, une barre :
- la glisser décale tout le calque dans le temps ;
- tirer son début ou sa fin rogne le calque, ou l'allonge en tenant son premier ou son dernier
  dessin ;
- le bord droit d'un dessin étire son exposition (la suite glisse).

Ces poignées s'arrêtent à la fin du plan : sa durée se change par la prise au bout de la règle
(ou par le champ de durée). Quand une durée change et que du son suit, la barre d'état prévient
que les plans suivants se décalent par rapport au son. Échap (ou Ctrl+Z) pendant un glisser
remet tout comme avant.

**Images** (bouton pellicule de la timeline, ou menu Animation > Images). Glisser sur les cases
choisit les images et les calques couverts. Maj + clic choisit les images entre l'image courante
et le clic, sur les calques choisis. Ctrl+A dans la timeline choisit tout. Ctrl+X / C / V et
Suppr les visent ensuite (Suppr seul, après un clic dans la grille, supprime l'image courante).
On peut :
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

**Effets** (un panneau de l'espace de travail, sous les calques ; la baguette de la timeline
l'ouvre ou le ferme) : des
effets sur le calque, vus à l'écran et à l'export, sans toucher aux dessins (non destructifs :
on les coche et décoche, le dessin reste tel quel).
- Par familles, comme le menu Filtre de Photoshop : **Flou** (gaussien, de mouvement),
  **Déformation** (onde, ondulation, tourbillon, pincement — centre réglable, animable),
  **Réglages** (luminosité / contraste, niveaux, teinte / saturation, noir et blanc, négatif,
  postérisation, seuil), **Bruit**, **Pixéliser** (mosaïque), **Rendu** (nuages, éclairage,
  dégradé : ils texturent ou éclairent le dessin), **Netteté**, **Styliser** (ombre portée,
  lueur).
- **Tremblement du trait** (famille Déformation, **calques vectoriels seulement**, comme dans
  WigglyPaint) : le dessin « bout », montré en quelques variantes légèrement déplacées qui se
  suivent à basse cadence, même sur une image fixe. Où le trouver : en tête de « Ajouter un effet »
  (panneau Effets), au clic droit sur un calque dans le panneau Calques, dans le montage et dans
  l'en-tête de la timeline (la case « Tremblement du trait » et « Animer à l'arrêt »), dans les
  réglages du pinceau et du crayon sur un calque vectoriel (l'icône en vague), et par Ctrl+K en
  tapant « tremblement ». Sur un calque bitmap, l'entrée est grisée et dit pourquoi (il ne va que
  sur un calque vectoriel). Les réglages sont dans le panneau Effets :
  **Intensité** (0 à 20 px, le déplacement maximal), **Fréquence** (0 à 100 % : de très doux le long
  du trait à nerveux), **Variantes** (2 à 6 dessins qui tournent), **Vitesse** (1 à 12 images du
  projet par variante : 2 images à 24 ips, 12 changements par seconde), **Épaisseur** (0 à 100 % :
  le trait s'épaissit et s'amincit un peu), **Graine** (pour que deux calques ne tremblent pas
  pareil), **Animer à l'arrêt** (éteint par défaut : l'image fixe montre la variante de l'image
  courante et ne coûte rien ; allumé, le dessin continue de trembler quand on ne lit pas, et seul
  cela redessine l'écran, à la cadence du tremblement, 12 fois par seconde et non 60). Tout se règle
  par clés comme les autres effets (sauf « Animer à l'arrêt »). Le même dessin donne toujours les
  mêmes pixels (lecture, export, défilement). Les dessins ne changent pas : tant que tu dessines, gommes,
  sélectionnes ou transformes sur le calque, il se montre sans tremblement, à la vraie place de ses
  traits. Actions : `fx.tremblement`, `fx.tremblement_repos`. Détails dans `src/tremblement.rs` et
  `docs/ROADMAP.md`. Le tremblement est dessiné **par la carte graphique** depuis le 2026-10-20 (même champ,
  écart d'un niveau au plus avec le processeur).
- **Effets du trait** (2026-10-20, famille **Trait**, **calques vectoriels seulement** : grisés avec la raison sur un
  calque bitmap ; `src/fxtrait.rs`). Six effets de plus de la pile, qui agissent sur les traits eux-mêmes ; mêmes
  réglages enregistrés, mêmes clés, même copier-coller que le reste. Un **bouton déroulant « Trait ▾ »** en tête du
  panneau Effets (depuis le 2026-10-20 : c'était une rangée d'icônes) liste les effets avec leur icône et leur nom, les
  coche et les décoche (bulle : nom, raccourci, description ; grisés avec la raison sur un bitmap) et, dans une section
  « Préréglages » du même menu, pose des **préréglages** :
  - **Écriture du trait** (icône signature) : le dessin se trace de lui-même, trait par trait dans l'ordre où ils ont été
    dessinés (ou tous ensemble), selon un **avancement** de 0 à 100 % animable (une clé à 0 %, une à 100 %).
    Réglages : effacement (le dessin se défait dans le même ordre), tous les traits ensemble, durée selon la longueur
    (un trait long prend plus de temps), douceur du rythme (départ et arrivée en douceur), bord doux de la pointe,
    zones remplies avec le trait (les aplats de couleur apparaissent en fondu à la fin).
  - **Crayonné multiple** (icône gribouillis) : chaque trait est redoublé de 1 à 4 passes décalées et plus pâles,
    comme un dessin repris au crayon, sous le trait d'origine resté intact ; décalage, opacité, nervosité, et
    **retirage toutes les N images** (variantes et vitesse, comme le tremblement).
  - **Respiration** (icône battement) : l'épaisseur du trait pulse (amplitude, période en images) autour d'une
    épaisseur de base animable ; une vague de pulsation peut courir le long du trait.
  - **Ondulation du trait** (icône vague) : une vague court le long de chaque trait (amplitude, longueur d'onde,
    vitesse, bouts fixes ou libres).
  - **Pointillés qui défilent** (icône points) : le trait devient des tirets (longueur, intervalle, vitesse dans un
    sens ou l'autre, bord doux).
  - **Désagrégation** (icône grains) : le trait se défait en poussière grain par grain, avancement animable : pour
    une disparition ou une transition (taille des grains, dispersion, sens du trait, direction et force du vent,
    graine ; mêmes réglages : même poussière).
  - **Préréglages du trait** (baguette) : « Apparition écrite » (le dessin s'écrit en une seconde), « Croquis nerveux »
    (crayonné et léger tremblement), « Poussière » (le dessin part en poussière en une seconde) ; les clés partent de
    la tête de lecture. Actions `fx.ecriture`, `fx.crayonne`, `fx.respiration`, `fx.ondulation_trait`,
    `fx.pointilles`, `fx.desagregation`, `fx.preset_apparition`, `fx.preset_croquis`, `fx.preset_poussiere` (Ctrl+K,
    sans touche par défaut ; clic droit sur une icône : lui assigner un raccourci).
  Chaque trait est transformé **en pixels**, depuis les pixels exacts que son cache garde : la texture de la brosse suit la
  déformation, et le moteur de brosses n'est jamais rejoué à chaque image. Calcul de fond, sans latence ; le même
  dessin donne toujours les mêmes pixels (lecture, export, vignettes). Tant que tu dessines sur le calque, ses effets du
  trait se taisent. Un fond sous le trait reste fixe. Mesures sur 2 000 traits : `docs/PERFORMANCE.md` (section 13) ;
  liste de ce qu'il faut regarder à l'écran : `docs/INTERFACE.md` (section 20).
- **Effets de matière** (2026-10-20, famille **Matière**, `src/fxmatiere.rs`, branche `effets-matiere`). Cinq effets de
  plus, dans un **bouton déroulant « Matière ▾ »** juste à côté de « Trait ▾ » (depuis le 2026-10-20 ; avant, une deuxième
  rangée d'icônes) : même section « Préréglages », mêmes clés, même copier-coller, une seule fois par calque et par sorte. Les trois premiers marchent sur **tous les calques** ;
  les deux derniers sur **les calques vectoriels seulement** (grisés avec la raison sur un bitmap) :
  - **Liseré** (icône sélection) : un contour d'une autre couleur autour du dessin, pour détacher un personnage du fond ;
    largeur (distance exacte au bord de l'encre, un vrai disque), douceur du bord, opacité, couleur, « derrière le trait »
    (sous un trait un peu transparent on voit le liseré) ou en anneau seulement. Sur les pixels du calque : vectoriel ou
    bitmap, même résultat.
  - **Trait sec** (icône pinceau large) : des trous le long de chaque trait (nombre pour 100 px, longueur, profondeur : des
    poils de brosse passent) et un bord qui s'effrite en stries parallèles au trait ; graine, et **retirage toutes les N
    images** (variantes, comme le tremblement). Sur un **bitmap**, une version simplifiée : plages creuses d'un bruit à
    basse fréquence et bord effrité d'après la distance à l'encre (pas de trait : pas de « le long du trait »).
  - **Encre qui bave** (icône goutte) : le bord du trait bave un peu, irrégulièrement, comme de l'encre sur du papier
    buvard (étalement, irrégularité, force, grain du papier, graine) et fonce à l'endroit où l'encre s'accumule (bord plus
    foncé et un peu effrité) ; la bavure est de la couleur de l'encre voisine, le cœur du trait reste intact et lisible.
  - **Dégradé le long du trait** (icône dégradé, vectoriel) : la couleur passe de la première à la dernière de 2 à 4
    couleurs du début à la fin de chaque trait, ou **sur tout le dessin dans l'ordre des traits** ; mélange avec la couleur
    d'origine, **défilement** animable (en % du dégradé par image), boucle. La forme et la texture ne changent pas.
  - **Changer de plume** (icône plume, vectoriel) : les traits sont redessinés avec **un autre réglage de pinceau** de la
    bibliothèque (encre, feutre, fusain, craie, gouache, un réglage importé…) ou la **plume calligraphique** de SHOT, en
    gardant leur tracé et leur pression ; taille de la plume en %. **Rien n'est modifié** : c'est un effet. Le bouton
    **« Appliquer pour de bon »** (et l'action `fx.plume_appliquer`) remplace vraiment les traits de tous les dessins du
    calque, en **une étape d'annulation nommée** « Plume appliquée pour de bon », et retire l'effet. Un trait sans appui
    enregistré garde son rayon (la plume ne le dépasse pas) ; une plume dont l'opacité suit l'appui (fusain) le montre alors
    plus pâle.
  - **Préréglages de la matière** (baguette) : « Encre de Chine », « Feutre usé » (plume Feutre et trait sec), « Contour
    blanc », « Arc-en-ciel » (dégradé sur tout le dessin), « Plume calligraphique ». Actions `fx.lisere`, `fx.trait_sec`,
    `fx.encre_bave`, `fx.degrade_trait`, `fx.plume`, `fx.plume_appliquer`, `fx.preset_encre_chine`, `fx.preset_feutre_use`,
    `fx.preset_contour_blanc`, `fx.preset_arc_en_ciel`, `fx.preset_plume_calligraphique` (Ctrl+K, sans touche par défaut).
  Tout est calculé en arrière-plan (dessins dérivés), jamais sur le fil de l'interface ; hasard par hachage (graine,
  position, trait, variante) : mêmes réglages, mêmes pixels, sur un fil ou plusieurs ; export, vignettes et écran
  identiques. **Pas de passe sur la carte graphique** (voir `docs/PERFORMANCE.md`, section 14, pour la raison et les
  mesures : 2 000 traits en 19 à 78 ms par image, une feuille 4K en 42 à 181 ms). Format de projet **32** : une version
  précédente refuse le projet. Liste à regarder à l'écran : `docs/INTERFACE.md` (section 21).
- Au clic droit sur un calque, la partie « Effets » : opacité, mode de fusion, chaque famille
  en cases à cocher ; on règle les paramètres dans le panneau Effets.
- **Copier-coller des effets d'un calque à l'autre** : l'icône « Copier l'effet » de chaque effet (ou
  son clic droit, ou le menu « Copier / coller » du panneau) copie l'effet avec ses réglages et toutes
  ses clés ; « Copier tous les effets du calque » copie la pile (aussi dans « Effets » du clic droit
  d'un calque). « Coller l'effet » / « Coller les effets » l'ajoutent à la fin de la pile du calque, ou
  de tous les calques choisis (Ctrl / Maj + clic) ; « Remplacer les effets » remplace leur pile. Une
  seule étape d'annulation. Les clés gardent leurs images (celles d'après la fin du plan sont laissées
  de côté) ; le tremblement du trait est laissé de côté sur un calque bitmap ; la barre d'état dit ce
  qui est collé où. Au clavier : quand le panneau Effets a la main (on y a cliqué en dernier), Ctrl+C
  copie l'effet choisi (ou le seul de la pile) et Ctrl+V colle, avant les autres presse-papiers ; un
  clic dans la timeline, le montage ou le dessin rend la main à ce qu'on y a choisi (les clés d'effets
  gardent leur Ctrl+C / Ctrl+V). Actions `fx.copier`, `fx.copier_tout`, `fx.coller`, `fx.remplacer`
  (Ctrl+K, sans raccourci par défaut). Détails dans `src/fxcopie.rs`.
- On les active, les ordonne ou les retire à tout moment.
- On peut les animer par clés : « Créer une clé » à une image, puis à une autre, et les
  paramètres vont de l'une à l'autre. Les clés de l'effet choisi dans le panneau Effets (clic sur
  son nom, il prend un cadre de la couleur de sélection ; un second clic le lâche) sont des
  losanges violets, en haut de la piste de son calque, dans la timeline comme dans la bande de
  montage ; aucun effet choisi, aucun losange (la case « Clés d'effets dans la timeline et le
  montage » est l'interrupteur général). Un clic en choisit une (Ctrl : en ajoute
  ou en retire ; Maj : toutes celles d'entre), la tête de lecture va dessus ; glisse-en une, elle
  **et toutes les clés d'après** du même effet glissent avec elle (Maj tenue : la clé choisie
  seule ; Échap abandonne le geste) ; Ctrl+C copie, Ctrl+V **insère** après la clé de la tête de
  lecture (sinon la dernière d'avant) et repousse les clés d'après, Maj+Ctrl+V colle exactement
  sur la tête de lecture sans rien repousser, sur l'effet choisi (de la même sorte que celui
  copié), Suppr supprime
  (rien si aucune n'est choisie) ; le clic droit propose les mêmes commandes. Changer d'effet
  choisi lâche le choix des clés. Une clé créée par
  « Créer une clé » est aussitôt choisie. Chaque geste est une étape d'annulation nommée
  (« Décaler / Déplacer / Insérer / Coller à la tête de lecture / Supprimer des clés d'effet »).
- **Caméra : clés, effets, focales, aides au cadrage** (2026-10-19). Les losanges jaunes de la
  piste Caméra (montage, et timeline) se choisissent comme les clés d'effets : clic (Ctrl : ajoute
  ou retire ; Maj : toutes celles d'entre, d'un plan à l'autre), la tête de lecture va dessus ;
  **glisser une clé emmène aussi toutes les clés d'après** de son plan (Maj tenue : seules les clés
  choisies bougent, sans passer par-dessus une clé non choisie ; Échap abandonne) ; **au
  rectangle** : un glisser qui part
  d'un espace vide de la piste Caméra (montage ou timeline) trace un rectangle et choisit les clés
  qu'il touche (Maj : les ajoute au choix ; Ctrl : bascule celles qu'il touche ; Échap rend le choix
  d'avant ; un clic dans le vide lâche le choix) ; Ctrl+C copie ; **Ctrl+V insère** les clés
  après la clé sur laquelle est la tête de lecture (sinon la dernière d'avant elle ; sans clé
  avant, sur la tête) en gardant l'écart entre elles : la première collée est à l'image d'après
  cette clé, et les clés d'après sont repoussées de la longueur du bloc (écart de la dernière clé
  collée + 1), donc rien n'est écrasé ; **Maj+Ctrl+V** (« Coller à la tête de lecture ») pose la
  première clé exactement sur la tête, sans rien repousser (les clés rencontrées sont remplacées).
  Dans le montage elles passent d'un plan à l'autre, en animation elles restent dans le plan
  courant, les clés qui tombent hors sont dites et laissées de côté ; si l'insertion repoussait une
  clé hors de son plan, rien n'est collé (le plan ne s'allonge pas, la barre d'état le dit) ;
  Suppr supprime (rien sans clé choisie) ; clic droit sur la piste Caméra. Une étape d'annulation
  nommée par geste (« Décaler / Déplacer / Insérer / Coller à la tête de lecture / Supprimer des
  clés de caméra »). Ces règles valent aussi pour les clés d'effets et les clés de dessins.
  Maj+Ctrl+V est la touche de l'ancien « Coller sur place » (un simple alias de Ctrl+V, resté sans
  touche) ; hors clés, il colle comme Ctrl+V. Maj+clic sans mouvement reste le choix d'une plage ;
  c'est le seuil de glissement d'egui (environ 6 points) qui sépare ce clic d'un Maj+glisser. Dans les réglages de l'outil Caméra, la section **Effets de caméra** pose des
  clés ordinaires à partir de la caméra actuelle (rien n'est remis à zéro) : caméra à l'épaule
  (found footage), tremblement léger, explosion / impact (avec coup de zoom), secousse rapide,
  zoom avant, zoom arrière, zoom choc, vertigo (dolly zoom) ; intensité, durée, fréquence, graine ;
  « Appliquer » (à l'image courante, ou sur les images choisies dans la timeline) est une seule
  étape d'annulation. À côté de la focale, la liste **Objectifs** (14, 24, 35, 50, 85, 135, 200 mm
  et l'anamorphique 40 mm, qui pose aussi le cadre 2,39:1) et l'angle de champ horizontal (capteur
  de 36 mm). Les **aides au cadrage** (menu « Cadrage », icône de règle, dans la barre au-dessus du
  dessin, à côté du menu de format ; il s'allume dès qu'une aide est tracée ; plus dans les réglages
  de la caméra) : tiers, grille d'or, spirale d'or (huit orientations), diagonales et
  réciproques, croix centrale, zones de sécurité, cadre de format (16:9, 9:16, 4:5, 1:1, 4:3,
  1,85:1, 2,39:1) ; couleur, opacité, épaisseur (par défaut 1 px et 70 % : discrets ; un ancien
  `reglages.toml` resté à l'ancien défaut 1,3 px / 85 % passe au nouveau, un réglage choisi est
  gardé) ; repères d'écran, jamais exportés.
- Vitesse (2026-10-18, scène de 3840 × 2160 : un calque vecto avec un flou animé, par-dessus un
  bitmap avec un bruit animé et une fusion). L'écran ne calcule jamais un effet : un fil de fond
  s'en charge sur tous les cœurs sauf deux (`fxcache.rs`), et l'écran montre en attendant le
  dernier résultat du même dessin (ou le dessin sans effet), puis se redessine à l'arrivée du
  nouveau. En lecture, les 24 images qui viennent sont calculées d'avance, la plus proche d'abord.
  Pendant la lecture, le défilement et les gestes sur un curseur d'effet, et si le zoom est sous
  80 % (sous 35 % : au quart), les effets sont calculés sur un dessin réduit de moitié, à la
  taille de l'écran, quatre fois plus vite (`fxproxy.rs`) ; un quart de seconde sans mouvement, la
  pleine résolution prend la place (la netteté revient d'un coup). Pas d'aperçu pour un calque
  déplacé ou tourné, la mosaïque, les déformations et le rendu. L'export, les vignettes
  enregistrées et le calcul sans écran donnent exactement les mêmes pixels qu'avant : seule la
  vitesse a changé. Mesures dans `docs/ROADMAP.md` (« Effets de calque : vitesse »), à relancer
  par `cargo test --release --lib fxperf -- --ignored --nocapture --test-threads=1`.
- **Effets sur la carte graphique** (2026-10-19, `src/gpufx.rs`, `src/gpufx.wgsl`). Les effets d'un calque
  et ceux des calques d'ajustement hors dossier se dessinent sur la carte graphique, **à pleine résolution, à
  chaque image** : plus d'aperçu réduit en lecture, plus d'attente quand on change un réglage, les curseurs
  suivent la souris. Les 29 effets ordinaires y passent (réglages de couleur, flous, netteté, bruit, grain, mosaïque,
  ombre, lueur, déformations, nuages, éclairage, dégradé, flash, image d'impact, lueur diffuse, secousse,
  vignettage, aberration, et depuis le 2026-10-20 le tremblement du trait) avec le même hasard et les mêmes arrondis
  que le calcul sur processeur (écart de 0 à 2 niveaux sur 255, flou exact, tremblement à 1 niveau). Scène d'Anthony
  en 3840 × 2160 : 2,9 ms par image en lecture (avant : aperçu au quart), 3,5 ms par cran du curseur du flou. **Les
  effets du trait** (famille Trait ci-dessus) lisent la géométrie des traits : ils restent un calcul de fond, et une
  pile qui en mêle à des effets de la carte est **coupée en deux** (le fond calcule le dessin dérivé par les effets du
  trait, la carte passe le reste dessus) ; un calque CTG, un fond sous le trait ou une vidéo restent sur le processeur,
  un calque d'ajustement dans un dossier aussi. **L'export, les vignettes et le repli
  sans carte graphique restent le calcul exact sur processeur.** Réglage : Fichier > Préférences… >
  Performance > « Effets sur la carte graphique » (allumé par défaut, grisé si la carte refuse le shader).
  Détails, écarts et mesures : `docs/PERFORMANCE.md` (section 12), à relancer par
  `cargo test --release --lib gpufxperf -- --ignored --nocapture --test-threads=1`.

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
  - **Toucher du stylet** (section repliée au bas des réglages du pinceau, du crayon et de la
    gomme ; vaut pour tous les pinceaux) : la courbe de pression (Droite, Douce, Ferme, ou à la
    main), le plancher de taille quand on effleure, le seuil d'appui sous lequel rien n'est posé
    (ni début fantôme ni pointe au levé), « Traits fins pleins » (un trait plus fin qu'un pixel
    reste un pixel plein, pas un fil gris) et « Bord doux » (bord des empreintes en une seule
    chute douce). « Ancien toucher » remet le comportement d'avant, pour comparer.
- **Couleur** (panneau Couleur ; au pied de la barre d'outils, un clic sur le carré de devant
  ouvre à côté la roue, les curseurs, les couleurs récentes et le nuancier ; menu Édition >
  Couleur) :
  - **deux couleurs**, comme dans Photoshop : l'avant-plan, avec laquelle on dessine, et
    l'arrière-plan, derrière. Un clic sur le carré de derrière (ou l'icône des flèches) les
    échange ; D remet le noir devant et le blanc derrière. Clic droit sur les carrés :
    intervertir, couleurs par défaut, copier le code, ajouter au nuancier ;
  - **roue chromatique** : l'anneau donne la teinte, le carré ou le triangle (au choix : icônes
    au-dessus de la roue, ou clic droit dessus) la saturation et la valeur. On clique ou on
    glisse ; le geste garde la partie où il a commencé. Elle suit la couleur prise ailleurs
    (pipette, nuancier, code). L'icône du demi-cercle la cache ou la montre ;
  - **curseurs** TSV, RVB ou TSL (teinte, saturation, luminosité) : boutons TSV / RVB / TSL, ou
    clic droit sur un curseur. Chacun a ses chiffres ; le code hexadécimal accepte #RRGGBB ou
    #RGB ;
  - **opacité** : la couleur n'en a pas ; c'est celle du pinceau ou du crayon, la même que dans
    leurs réglages (grisée pour les autres outils) ;
  - **couleurs récentes** : une rangée des dernières couleurs avec lesquelles on a dessiné
    (pinceau, crayon, remplissage, formes, texte). Clic : avant-plan ; clic droit : arrière-plan,
    ajouter au nuancier, effacer les récentes ;
  - **nuanciers** : autant qu'on veut, chacun nommé ; la liste choisit celui qui est montré.
    + ajoute la couleur d'avant-plan, − retire sa pastille. Le menu ⋯ (ou clic droit sur le nom)
    crée, renomme, duplique, supprime (après confirmation), importe ou exporte un nuancier aux
    formats de GIMP (`.gpl`) et de Photoshop (`.aco`) ; à l'export, l'extension choisit le
    format. Sur une pastille : clic, avant-plan ; glisser, la ranger ailleurs (Échap
    abandonne) ; la bulle donne son nom ; clic droit : arrière-plan, nommer, remplacer par
    l'avant-plan, retirer ;
  - les commandes de pastille visent celle dont le menu est ouvert, sinon celle sous la souris :
    un raccourci qu'on leur assigne agit sur la pastille survolée.

  Les couleurs, la roue, les curseurs, les récentes et les nuanciers sont retenus pour la
  prochaine fois (`%APPDATA%\SHOT\reglages.toml`), pas dans le projet. Ctrl+Z ne défait pas
  les changements de nuancier.
- **Panneaux** : chaque panneau (Outils, Couleur, Pinceaux, Réglages, Guides, Fiche, Effets,
  Calques, Montage ou Timeline) est un onglet. La bande du bas montre le montage en storyboard,
  la timeline du plan en animation (double-clic sur un plan pour y entrer). Le montage est le banc
  principal : sous chaque plan, ses calques dans son propre ordre (du haut de sa pile vers le
  bas), et l'essentiel s'y fait sans entrer dans le plan :
  nouveau calque (bouton de la barre), nouveau dessin à une image (double-clic sur la piste du
  calque, ou clic droit), **décaler une clé** (la prendre par son milieu ou par la limite entre
  deux dessins : elle **et toutes les clés d'après** sur ce calque glissent, chacune gardant sa
  durée, l'exposition d'avant s'allonge ou raccourcit ; **Maj tenue : la clé seule** bouge, la
  suite reste en place ; **Alt tenue : l'ancien geste**, la clé prend sa place avant ou après les
  autres dessins, chacun gardant sa durée), décaler un calque (sa barre), le rogner ou l'allonger
  (ses bouts). La durée du plan ne change jamais pendant ces gestes : un calque tenu jusqu'à la
  fin du plan le reste (sa dernière exposition absorbe le décalage, en gardant au moins une
  image), sinon le contenu s'arrête à la fin du plan (le bord droit d'un dessin dans la timeline
  suit la même règle : Maj tenue, seule cette limite bouge). Après un clic
  sur la piste d'un calque, Ctrl+C copie la clé (le dessin et sa durée), **Ctrl+V l'insère** après
  la clé sur laquelle est la tête de lecture, sur le calque courant, et repousse la suite de la
  piste de la longueur du bloc (sans allonger le plan : la queue de la dernière exposition est
  coupée ; rien n'est collé si une clé entière sortirait du plan) ; **Maj+Ctrl+V** (« Coller à
  la tête de lecture ») la pose exactement sur la tête, par-dessus ce qui y était, sans rien
  repousser. Suppr la supprime (sa voisine la remplace), Ctrl+D la duplique sur place ; le clic
  droit propose aussi de la supprimer en laissant un vide. Le storyboard verrouillé refuse tous
  ces gestes. Ctrl ou Maj + clic choisissent plusieurs
  clés à la fois (Ctrl+A : toutes celles du calque) : ces commandes les visent toutes, et une
  clé choisie prise par son milieu les emmène toutes ; des fantômes et un repère montrent
  où elles se poseront. Un glisser dans le vide des pistes choisit les clés au rectangle. Zoom
  de la bande : la petite barre en bas, Ctrl + molette, ou Alt+1 / Alt+2 / Alt+3 (tout voir). F5
  (ou double-clic sur la piste) pose une clé à l'image courante, copie du dessin montré (sinon du
  dernier d'avant) ; F4 pose une clé vide ; la clé posée est aussitôt la seule choisie (cerclée de
  blanc, le cadre du calque s'efface) : Suppr la supprime, Ctrl+C la copie. Il en va de même d'un
  nouveau dessin, d'un dessin dupliqué ou d'images insérées dans la timeline (cadrées de blanc).
  F6 coupe le plan. Outils : la touche d'un
  outil tenue le prend le temps de la
  tenir ; Ctrl tenu sur le dessin donne le lasso. Les
  calques de toute la scène (fond) ont leur piste, teintée, avec les mêmes gestes. La timeline sert à entrer dans le détail. Copier, Coller, Suppr et les raccourcis
  visent la bande quand on y a cliqué en dernier (un trait de couleur en haut, et la barre
  d'état le dit) ; un clic dans le dessin, Tab ou la vue Vignettes rendent la main au dessin. Glisse un onglet : des boutons apparaissent au milieu du panneau
  survolé, lâche-le sur l'un d'eux pour l'aimanter (à gauche, à droite, dessus, dessous, ou en
  onglet), ou ailleurs pour qu'il flotte. Tire les bords entre panneaux pour les redimensionner.
  La croix d'un onglet le ferme ; Affichage > Panneaux le fait revenir à sa place. Clic droit
  sur un onglet : « Dans une fenêtre à part », pour le poser sur un autre écran (le fermer le
  remet à sa place). Les commandes sont des icônes, partout où c'est possible : la souris posée
  une seconde sur l'une d'elles ouvre une bulle avec son nom, son raccourci et ce qu'elle fait.
  En haut, une barre de commandes ; à gauche, la barre d'outils verticale (Affichage > Panneaux
  pour les masquer). Une case à petit coin regroupe plusieurs outils : clic maintenu pour les
  voir et en prendre un, la case montre ensuite le dernier pris (Sélection et Sélection par
  couleur en haut ; Rectangle, Ligne, Ellipse et Polyligne dans une seule case). Couleur : avant-plan et arrière-plan, roue, curseurs, couleurs récentes et
  nuanciers (voir **Couleur** plus haut). Panneau **Pinceaux** (onglet à côté de Couleur) :
  la bibliothèque : une recherche par nom, une bascule liste / vignettes (liste : pastille de la
  pointe avec sa taille, aperçu du trait, nom), et les réglages rangés en **dossiers repliables
  avec leur nombre** : Crayons, Encres et feutres, Peinture, Craie et fusain, Gommes (avec la
  gomme), Mes brosses, puis un dossier par fichier importé (les importés démarrent repliés). Clic :
  choisir ; clic droit : renommer, dupliquer, remplacer, supprimer. En bas, trois icônes :
  importer des pinceaux, nouvelle brosse (garde le réglage courant dans Mes brosses), supprimer la
  brosse choisie (celles de SHOT restent). Les **réglages du pinceau** sont dans le panneau
  **Réglages** de l'outil (pinceau, crayon, gomme) : en premier la **taille** en grand curseur et
  en champ px (la même valeur que les touches ^ et $), l'opacité, le nom du réglage, l'aperçu du
  trait courant, puis à gauche les catégories (Forme de la pointe, Taille et appui, Dispersion,
  Grain et papier, Couleur et mélange, Lissage, Toucher du stylet), dont quatre ont une case qui
  les allume ou les éteint sans perdre leurs valeurs, et à droite les réglages de la catégorie.
  Panneau étroit : les catégories passent au-dessus, en liste déroulante. En bas : peint
  derrière, remplir la forme, le fond sous le trait et ses options.
  Liste ou vignettes, la catégorie et les dossiers ouverts sont retenus (`reglages.toml`,
  clé `panneau_pinceaux`). Les aperçus montrent le trait dans la couleur courante, sur le papier.
  L'interface est dense (police Segoe UI), en gris neutres, avec trois thèmes et un quatrième à
  régler soi-même : voir « Apparence » plus bas. Les pinceaux texturés
  (pointe en image, grain du papier, pinceaux importés) marchent aussi sur un calque vectoriel :
  le trait reste modifiable, et ses textures sont enregistrées avec le projet.
- **Formes** : Ligne (droite, courbe : tracer puis glisser pour courber ; courbe en S : deux
  glissers), Rectangle, Ellipse (Maj : carré, cercle), Polyligne (un clic par point, double-clic
  ou Entrée pour finir, clic sur le premier point pour fermer).
- **Coller une image** copiée ailleurs (navigateur, capture d'écran, autre logiciel) : Ctrl+V ou
  Édition > Coller. Sur un calque bitmap, dans son dessin ; sinon sur un nouveau calque bitmap
  « Image collée » au-dessus. Centrée dans le cadre (réduite pour y tenir), sélectionnée :
  déplacer, agrandir, tourner, puis Échap pour la poser. Après une copie dans Pelure, Ctrl+V colle
  de nouveau ce qui a été copié dans Pelure.
- **Retourner** : la sélection (Alt+H, Alt+V, ou Édition), le calque sur place (menu Calques >
  Retourner le calque, de gauche à droite ou de haut en bas ; ses dessins ne changent pas, Ctrl+R
  annule), les plans choisis de gauche à droite autour du milieu du cadre, caméra comprise (menu
  Storyboard ou « … » du montage ; les calques de toute la scène, partagés, ne sont pas retournés).
- **Transformer** (calque entier, ses dessins ne changent pas) : glisser dans la boîte pour
  déplacer, un coin pour agrandir (Maj : proportions), le milieu d'un côté pour étirer en largeur ou
  en hauteur, la poignée ronde pour tourner ; largeur et hauteur aussi en chiffres. Ctrl+R annule.
  L'enveloppe de Bézier n'est plus un Ctrl + coin : c'est un mode de l'outil **Déformer** (voir plus bas).
  Une zone bitmap choisie reste quand on passe sur Transformer : glisser dedans la
  déplace, sa boîte l'agrandit, la tourne ou l'incline ; ailleurs, Transformer agit sur le calque. Avec l'outil Transformer, Ctrl ne prend pas le lasso.
- **Cutter** (Alt+T, calque vectoriel) : glisser en travers des bouts de traits à enlever (ou
  cliquer sur l'un d'eux) ; chaque trait touché est coupé là où il croise les autres, seuls les
  morceaux touchés partent (un dépassement est coupé net au croisement). Un trait qui ne croise
  rien part en entier.
- **Sélection par couleur** (baguette, dans la case de la Sélection) : sur un calque bitmap, clic
  sur une couleur pour choisir sa zone (tolérance ; pixels voisins seulement, ou toute l'image) ;
  sur un calque vectoriel, tous les traits et zones de cette couleur. Maj : ajouter. Copier,
  couper, effacer, flèches et poignées comme avec la Sélection.
- **Texte** : clic pour poser un texte, Ctrl+Entrée pour le valider ; police et taille dans
  Réglages. Sur un calque vectoriel il reste modifiable (clic dessus avec l'outil Texte).
- **Déformer** (même case de la barre d'outils que Transformer ; ex-outil Perspective) : deux modes,
  choisis dans Réglages (ou par les actions « Outil déformer : enveloppe de Bézier » et « … :
  perspective »). *Perspective* : clic sur le dessin, puis tirer ses coins. *Enveloppe* : clic sur le
  dessin, ses coins et leurs poignées rondes courbent les côtés, l'intérieur la déplace ; Entrée, Échap
  ou un clic ailleurs terminent. Ctrl+Z annule chaque geste. Avec des éléments choisis (vectoriel) ou
  une zone choisie (bitmap, enveloppe seulement), seuls ceux-là sont déformés ; sinon tout le dessin.
- **Brosses traditionnelles** (2026-10-18, `docs/AUDIT-RENDU-BROSSES.md`) : dans la liste des
  réglages (grille des Réglages, panneau Pinceaux), après le Pinceau rond — **Crayon graphite**
  (le réglage de départ de l'outil Crayon : un amas de grains sur un papier à dessin, un dépôt qui
  s'accumule jusqu'à 90 % d'opacité, on fonce en repassant), **Crayon de couleur**, **Encre**
  (bouts effilés), **Feutre** (aplat à 70 %, biseau), **Fusain**, **Craie** (sur toile),
  **Gouache** (poils plats qui suivent le trait, peinture qui se mêle à ce qui est dessous), et
  **Crayon net** (le crayon d'avant, sans matière). Leurs pointes (« Graphite », « Fusain »,
  « Craie », « Poils plats », « Poils ronds ») et papiers (« Papier à dessin », « Toile »,
  « Vergé ») sont fabriqués par Pelure au démarrage, rien n'est copié d'un autre logiciel ; ils
  se choisissent dans les listes « Pointe » et « Papier » de n'importe quel réglage. Pour les
  essayer : outil Crayon sur un calque bitmap ou vectoriel, hachures légères (le grain), trois
  passages au même endroit (ça fonce), puis comparer à « Crayon net ». Tailles pensées pour un
  projet 1920×1080 vu à 50 %. Depuis le retour d'Anthony : le graphite et le crayon de couleur
  mordent dès l'effleurement (courbe appui → débit relevée) et leur pointe est un « sable » de
  grains francs plutôt qu'un nuage.
  - **Douceur du grain** (sous le papier) : petit = grain net et dur (les anciens réglages),
    grand = grain qui répond en douceur. **Grain du crayon** (Soie, Doux, Moyen, Dur) : un grain
    fin collé à la feuille, même sans papier — le « Grain » du crayon de TVPaint, refait par
    Pelure ; un crayon TVPaint importé le reçoit. **Le grain suit le trait** : le relief est lu
    depuis le début du trait au lieu de rester collé à la feuille.
  - Les brosses MyPaint et Krita importées (`opaque`, Flow) déposent maintenant une part par
    empreinte qui s'accumule dans le trait, au lieu d'un plafond d'opacité.
- **Pinceaux texturés** (panneau Réglages) : « Papier » et « Grain » donnent un grain fixe sur le
  dessin, que la couleur remplit quand on appuie. L'icône d'import lit un fichier de pinceaux Toon
  Boom (`.xml`, par exemple `STB_TextureBrushes_preset.xml`) : ses pinceaux, pointes et papiers
  s'ajoutent à la liste et restent pour les fois suivantes.
- **Brosses Procreate** (même bouton) : `.brush`, `.brushset`. Arrivent la forme (pointe), le
  grain (papier), la taille, l'opacité, l'espacement, la dispersion et l'appui. Pas encore
  essayé sur une vraie brosse ; les échelles sont à vérifier.
- **Brosses Photoshop** (même bouton) : `.abr`, pointes et préréglages (taille, dureté, angle,
  rondeur, espacement, appui). Pas encore essayé sur une vraie brosse.
- **Brosses Krita et MyPaint** (même bouton, plusieurs fichiers à la fois) : réglages Krita
  `.kpp`, paquets Krita `.bundle` entiers, réglages MyPaint `.myb`. Arrivent : la pointe
  (ronde ou en image), la taille, l'opacité, le débit, le pas, et les courbes de pression,
  vitesse, angle du trait, fondu et hasard.
- **Brosses TVPaint** (bouton « Importer des pinceaux ») : un export de panneau `.tvpx`, un
  panneau `.bin` (avec son dossier `.data`) ou un réglage `.ini`. Arrivent :
  - les pointes, y compris animées ;
  - la taille, l'opacité, le pas, les connexions et leurs courbes ;
  - le papier, retrouvé dans l'installation de TVPaint.

  Une fenêtre dit ce qui n'a pas pu être repris (estompe, inclinaison…).
- **Mes brosses** : l'icône disquette (grille des Réglages, panneau Pinceaux) garde le réglage
  courant sous un nom. Clic droit sur un réglage : renommer, dupliquer, remplacer par le réglage
  courant, supprimer. Elles restent pour les fois suivantes.
- **Pointe en image** (panneau Pinceaux, icône pinceau) : une image devient la pointe, par sa
  transparence ou son noir sur fond blanc. Plusieurs dessins côte à côte donnent une pointe en
  plusieurs cases (« Cases ») ; chaque empreinte en prend un, à la suite, au hasard, selon l'appui
  ou selon la direction.
- **Papier** (panneau Pinceaux) : taille du grain, angle, « Inverser » (les creux deviennent des
  sommets). L'icône image à côté de la liste prend n'importe quelle image (PNG, JPG…) comme papier,
  par exemple un papier de TVPaint ; il reste pour les fois suivantes.
- **Forme et dispersion** (réglages du pinceau) : rondeur et angle de la pointe, taille et angle
  au hasard, dispersion des empreintes en travers et le long du trait, nombre d'empreintes par
  pas, retournements au hasard d'une pointe en image (craies, matières).
- **Lissage** (réglages du pinceau) : Suivi (comme avant), Corde tirée (le trait suit la pointe
  au bout d'une corde : les petits tremblements ne le bougent pas, pour l'encrage), Moyenne sur
  des points ou sur une distance. Options, sous le type : « Après le trait » (le trait brut suit
  la pointe sans retard, puis est rejoué lissé au levé, comme TVPaint avec Real Time décoché),
  « Selon le temps » (le Suivi lisse pareil quelle que soit la cadence du stylet et de l'image),
  « Pixels d'écran » (la corde ou la distance se mesurent à l'écran : la même à tout zoom), et la
  fin du trait : rattrapage de la pointe en courbe, droit ou aucun.
- **Crayon** : anticrénelé, 2,5 px, qui mord plus ou moins selon l'appui ; « Pixels nets » (à
  côté de la dureté) rend le crayon à pixels francs du pixel art.
- **Profil de la pointe** (bouton courbe à côté de la Dureté) : la couverture de la pointe du
  centre au bord, en courbe, à la place de la dureté. Il part de la dureté réglée ; préréglages
  Dur, Doux, Diffus.
- **Banc de traits** (Labo > Enregistrer mes traits) : chaque trait est gardé tel que le stylet
  le donne, avec son réglage, pour régler le moteur sur tes vrais gestes. Le même menu l'arrête.
- **Peinture humide** (réglages du pinceau) : « Mélange » fait prendre à la brosse la couleur déjà
  posée, la mêler à la sienne et l'entraîner. « Charge » épuise la peinture le long du trait,
  après quoi la brosse ne fait plus qu'étaler. Calque bitmap.
- **Débit** (réglages du pinceau) : la peinture déposée à chaque empreinte (le « Power » de
  TVPaint) ; l'opacité en reste le plafond. Avec « Accumuler », repasser dans le même trait fonce,
  comme un crayon ou une craie ; sans, le trait reste uniforme.
- **Deux couleurs** : dans les Connexions, « Couleur A→B » mêle la couleur d'arrière-plan (B) à
  celle d'avant-plan (A) selon l'appui, la vitesse, le fondu, la direction ou le hasard.
- **Connexions** (bas des réglages du pinceau, du crayon et de la gomme) : la taille, l'opacité,
  l'angle et la rondeur de la pointe peuvent suivre l'appui, la vitesse du trait, sa direction,
  un fondu le long du trait ou le hasard.
  - Chaque connexion a sa courbe de réponse : glisser un point, double-clic pour en ajouter un,
    clic droit pour le retirer. Elle a aussi « Inverser » et une réponse minimale.
  - Exemples : une pointe plate dont l'angle suit la direction, un feutre qui s'affine quand on
    va vite (taille → vitesse, inversé), un trait qui s'éteint (taille → fondu, inversé).
  - L'aperçu des pinceaux trace lentement aux bouts et vite au milieu.
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
- **Table lumineuse** (barre sous le dessin, ou menu Affichage) : les autres calques du panneau sont
  estompés, le calque sur lequel on dessine reste net.
- **Guides de perspective** (panneau Guides) : 1, 2 ou 3 points de fuite, que l'on déplace à la
  souris. G les affiche, Maj+A aligne les traits (le trait suit la fuyante, la verticale ou
  l'horizontale la plus proche de son départ), Maj+L verrouille les points, Maj+G montre des lignes
  de référence. Les guides sont enregistrés avec le projet.
- **Vue** : zoom à la molette (ou 1 / 2), rotation (Maj + molette, ou V / C), miroir (M),
  déplacement (Espace + glisser). Toutes les commandes de la vue sont dans la barre sous le dessin :
  planche, caméra, 3D ; ajuster, 100 %, zoom ; rotation ; miroir ; pelure d'oignon, table lumineuse
  et sa force.
- **Annuler / rétablir** (Ctrl+Z / Ctrl+Maj+Z) : 200 étapes, sur tout le projet : traits, vectoriel,
  sélections, panneaux, scènes, séquences, calques, sons, textes, noms, réglages des calques et des
  pistes, guides, papier, cadence. Une saisie ou un glissé de curseur compte pour une seule étape.
- **Espaces de travail liés aux modes** (Affichage > Espaces de travail) : **Storyboard**, **Animation**
  et **Montage** retiennent chacun leur disposition des panneaux. Les boutons « Storyboard » et
  « Animation » (en haut à droite), Tab et « Animer ce plan » changent d'espace et posent tout de
  suite la disposition qu'on y avait ; ce qu'on change dans l'un (un panneau fermé, une colonne
  déplacée) ne touche pas les autres. La première fois, chacun a sa disposition d'origine :
  Storyboard (la disposition standard), Animation (la bande, c'est-à-dire la timeline, plus haute et
  les calques plus larges), Montage (la bande du montage en grand ; ni calques, ni effets, ni réglages
  de l'outil, ni couleur ; la fiche du plan, les bruitages et les suggestions sonores à droite).
  Montage n'est pas un troisième mode : il garde les outils, les raccourcis et le dessin du
  storyboard ; on y va par Affichage > Espaces de travail > Montage (ou Ctrl+K), et le bouton
  « Storyboard » (ou « Storyboard » du menu) en revient. Quitter l'animation ramène à celui des deux
  espaces du côté storyboard où l'on était. **Affichage > Espaces de travail > Rétablir la
  disposition** remet celle d'origine de l'espace en cours, sans toucher aux autres. Les panneaux
  dans leur propre fenêtre restent où ils sont (ils sont sur un autre écran), la feuille d'exposition
  n'existe qu'en animation, et les panneaux qui s'ajustent à leur contenu repartent de la disposition
  posée. Un ancien fichier de réglages garde sa disposition unique comme celle du Storyboard.
  Les quatre dispositions d'avant (Standard, Dessin, Timeline, Vue d'ensemble) s'appliquent à
  l'espace en cours ; « Enregistrer la disposition… » garde la tienne sous un nom (panneaux aimantés
  et flottants compris). Affichage > Panneaux masque ou affiche chaque panneau. SHOT rouvre dans
  l'espace du côté storyboard et la disposition de la dernière fois.
- **Barres minces et « Plus… »** : la barre de commandes du haut ne garde que 9 icônes (Nouveau,
  Ouvrir, Enregistrer, Exporter ; Annuler, Rétablir ; Couper, Copier, Coller) ; la barre du montage en
  garde une douzaine (lecture et boucle, nouveau plan, nouvelle séquence, menus Plans et Son,
  importer un son, animer ce plan, vue dessin / vignettes, planche de crayonnés) ; la barre de la
  timeline (plan, transport, repères, dessins, expositions, clés de caméra, pelure) ne défile plus.
  Le reste est sous le bouton « Plus… » (trois points) à droite de chaque barre : une ligne par
  commande avec son icône, son nom et son raccourci. L'**épingle** de chaque ligne (« Épingler dans
  la barre ») remet la commande dans la barre ; le sous-menu « Dans la barre » permet de retirer une
  commande montrée d'office, « Rétablir la barre d'origine » efface les choix. Le choix est retenu
  par barre (`reglages.toml`, table `barres`). Une barre trop étroite envoie d'elle-même ses
  dernières commandes sous « Plus… » (elles reviennent quand la fenêtre s'élargit). Rien n'est
  retiré : tout reste dans les menus, la palette Ctrl+K et les raccourcis. La barre des panneaux, à
  droite, ne change pas.
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

**Jeu de raccourcis** (Fichier > Préférences… ou en tête de la liste F1) : « SHOT (par défaut) » ou
« Façon Storyboard Pro ». Ce dernier ne change que ce que `keymap.toml` documente (Ctrl+Maj+V colle sur
place) ; A / F (clé précédente / suivante), F5 (nouvelle clé) et F6 (couper le plan) restent à tes
réglages, voulu ainsi ; le reste garde les touches de SHOT, et tes
raccourcis personnels restent par-dessus le jeu. La liste dit d'où vient chaque touche (SHOT, jeu, perso).
Tableau et sources : `docs/INTERFACE.md`, 6.2.

Les choix vont dans un fichier personnel (`%APPDATA%\SHOT\keymap.toml`), qui remplace, action par
action, le fichier par défaut (et le jeu choisi). On peut aussi l'écrire à la main (« Aide > Modifier le fichier de
raccourcis à la main », puis « Recharger »). Détails et choix pour le clavier AZERTY :
`docs/reference/sbpro_presets/README.md`.

Documentation technique : `docs/ARCHITECTURE.md` ; avancement : `docs/ROADMAP.md` ; mesures et
leviers de performance (superposition « Performance », banc `perf_tests.rs`, diagnostic des
performances) : `docs/PERFORMANCE.md`.

**Les vignettes ne ralentissent plus SHOT.** Les vignettes du montage et de la planche, et les
miniatures des dessins dans la timeline et le panneau Calques, se composent sur des fils de fond : en
défilant un montage de 600 plans ou en sautant de plan en plan, l'interface ne compose plus rien (chaque
image d'écran passe de 7–11 ms à 2–3 ms). Une vignette qui change garde son ancienne image jusqu'à
l'arrivée de la neuve ; celles qu'on voit passent d'abord, celles qu'on a quittées sont abandonnées.

**Enregistrer ne fige plus SHOT.** `Ctrl+S` (et Enregistrer sous) écrit le projet sur un autre fil :
la barre d'état dit « Enregistrement… n / N », le point • du titre reste jusqu'à la fin, et ce que tu
modifies pendant l'écriture reste « non enregistré ». Quitter, Fermer, Nouveau et Ouvrir attendent la
fin de l'écriture (une petite fenêtre montre l'avancement si elle dure) ; si l'enregistrement échoue
(disque plein, droits), une fenêtre le dit et le projet reste marqué comme modifié.

## Diagnostic des performances

**Aide > Diagnostic des performances…** (ou `Ctrl+K`, « Diagnostic ») règle SHOT pour la machine sur
laquelle il tourne, sans rien appliquer sans ton accord :

1. **Il détecte** le processeur (nom, cœurs, AVX2 / FMA / BMI2), la mémoire vive (totale et
   disponible), la carte graphique (nom, API, pilote, mémoire vidéo quand Windows la donne), l'écran.
2. **Il mesure**, une quinzaine de secondes sur un autre fil (barre d'avancement, bouton Annuler,
   l'interface reste vivante) : écriture et lecture de 64 Mo sur le disque, composition de douze
   calques, rejeu d'un trait texturé, un flou à pleine résolution et en aperçu réduit, une lecture
   d'un plan chargé, l'envoi de dessins à la carte graphique. Aucun projet n'est ouvert.
3. **Il propose un profil** : Qualité, Équilibré ou Performance, avec la raison (« 8 Go de mémoire
   vive seulement », « rejeu d'un trait texturé 5 fois plus lent que sur le poste de référence »…) et,
   pour chaque profil, la liste des réglages qu'il changerait, chacun à cocher ou décocher.
4. **Appliquer** pose les réglages cochés ; **Revenir comme avant** remet exactement les valeurs
   d'avant (elles sont gardées dans les réglages, même après un redémarrage). Le rapport (matériel,
   mesures, réglages) se copie ou s'enregistre en texte.

Au premier lancement sur une machine sans diagnostic, un petit message le propose une fois
(« Lancer le diagnostic » / « Plus tard ») ; il ne bloque rien et n'applique rien.

Les onze réglages sont de vrais réglages de SHOT, tous dans **Fichier > Préférences…, section
Performance**, et jouent tout de suite (sauf le rendu des traits texturés, à l'ouverture d'un projet) :
effets en aperçu réduit pendant la lecture et les gestes ; traits vectoriels texturés (rendu exact
aussitôt, rapide jusqu'au repos, ou rapide partout) ; tremblement du trait sans « Animer à l'arrêt » ;
pelure d'oignon plafonnée ; caches de la carte graphique, des traits et des effets ; fils de calcul ;
résolution des vignettes ; interface sans animations ; **effets sur la carte graphique** (allumé par
défaut ; le profil Équilibré l'éteint sur un rendu logiciel, le profil Performance hors d'une carte
dédiée, où le calcul réduit sur processeur peut faire mieux). Détails : `docs/PERFORMANCE.md`.

## Messages

Les messages de la barre d'état ne se perdent plus : survole-le pour le lire en entier, clique
pour voir les précédents. Les avis importants (enregistrement, ouverture, erreurs, fin de
propagation, export) restent aussi quelques secondes en bas à droite ; un clic les ferme.

## Fond sous le trait

Pour détacher un personnage du décor sans le remplir à la main : allume « Fond sous le trait » **sur le
calque** (depuis le 2026-10-20, un bouton de la ligne du calque dans le panneau Calques, de l'en-tête de la
timeline et des commandes du panneau Calques ; ce n'est plus une option des réglages du pinceau et du crayon :
c'est un réglage du calque, un bouton par calque de dessin). Chaque dessin du calque
prend alors un fond sous son trait : tout ce que le trait enferme, plus un léger débord autour,
en une seule forme qui se recalcule à chaque trait (en animation aussi). Réglages, juste dessous :
- couleur du fond (blanc d'office) ;
- débord (3 px) ;
- fermer les trous (6 px : un trou du trait jusqu'à 12 px ne laisse pas fuir le fond) ;
- le bord de l'image ferme la forme (un personnage coupé par le cadre a son fond jusqu'au bord).

« Peindre dans : Trait | Fond » (dans les réglages, et sur la ligne du calque) : en « Fond », le
pinceau et le crayon peignent dans le fond, sous le trait, sans jamais le toucher, et la peinture
reste dans la forme ; la gomme **coupe le fond** (une zone fermée qu'on veut laisser voir à travers),
et repeindre au même endroit le recoud. Tout se voit pendant le geste ; Ctrl+Z annule chaque coup.
Éteindre l'option retire le fond.

Toujours en « Fond », le pot de peinture et le cutter travaillent par zones fermées par le trait :
un clic du pot de peinture remplit la zone de la couleur courante, sous le trait (un t-shirt rouge,
un visage qui reste blanc) ; un clic du cutter retire la zone du fond (le trou entre le bras et le
corps). Les trous du trait sont fermés comme pour le fond.

Plusieurs calques d'un coup : Calques > Fond sous le trait (sur les calques choisis, sur tout le
plan, retirer), ou « Ce fond aussi sur » dans les réglages. « Tout le plan » laisse les calques de
fond du décor. La couleur d'un fond neuf se règle par projet (Fichier > Réglages du projet,
« Couleur des fonds sous le trait », par exemple un gris clair) ; « Aux fonds déjà posés » la
donne à tous les fonds du projet.

## Calque de couleur (la couleur sous le trait)

Pour colorier sans abîmer le trait : un **calque de couleur** est lié à un calque de dessin (le
« trait »), posé juste dessous, et il en suit les images. C'est l'idée « trait / couleur » de
Toon Boom ; la façon exacte de Storyboard Pro ou de Harmony n'a pas été vérifiée (à vérifier).

- **Créer** : l'icône palette du panneau Calques (« Nouveau calque de couleur »), Calques > Nouveau
  calque > « De couleur », clic droit sur un calque > « Calque de couleur » > « Ajouter… », ou Ctrl+K.
  Il se range sous le trait du calque courant, en retrait, avec la mention « sous le trait » ; un
  trait peut en porter plusieurs. Ctrl+Z annule d'un coup.
- **Les images suivent le trait** : pour chaque image où le trait montre un dessin, la couleur a le
  sien (vide au début). Poser, allonger, déplacer ou supprimer une clé du trait emmène sa couleur ;
  « Nouveau dessin en copie » emporte la couleur du dessin copié. On ne pose pas de clé sur la couleur
  elle-même (le message le dit : fais-le sur le trait). Le pré / post-comportement, la
  transformation et la profondeur du trait sont ceux de sa couleur.
- **Colorier** : le pinceau et le crayon peignent comme sur un calque bitmap ; par défaut, un trait
  de pinceau **reste dans la zone fermée** où il commence (et passe sous le trait). Le **pot de
  peinture** remplit la zone fermée par le trait où l'on clique, sous le trait (les trous du trait
  jusqu'à 8 px sont fermés, réglable dans le menu du calque). Un clic dehors colorie tout le dehors,
  et le message prévient quand la zone fuit par un trou. Le trait n'est jamais touché ; la gomme n'est
  pas limitée à la zone.
- **Trait vectoriel ou bitmap** : la couleur est toujours en pixels (un aplat n'a pas besoin d'être
  modifiable point par point) ; le pot de peinture lit les pixels rendus du trait, quel que soit le calque.
- **Ensemble** : l'œil du trait cache aussi sa couleur (l'œil de la couleur la cache seule) ; supprimer
  ou dupliquer le trait supprime ou duplique ses couleurs ; copier-coller le trait les emmène ; le
  monter ou le descendre déplace la couleur avec lui (la couleur seule ne se déplace pas). **Indépendants** : le
  verrou (on colorie sous un trait verrouillé), l'opacité, le mode de fusion, la pelure d'oignon et les
  effets de chaque calque. Un effet du trait (tremblement…) ne bouge donc pas la couleur : à vérifier
  si c'est ce que tu veux.
- **Détacher** (clic droit > Calque de couleur) : la couleur redevient un calque bitmap ordinaire, aux
  images à elle. Si le trait disparaît (fusion, suppression), le lien s'oublie tout seul et la couleur reste.
- **Fond sous le trait** (juste au-dessus) : l'autre façon d'avoir de la couleur sous le trait, dans le
  même calque. Les deux se cumulent (la strate du trait, puis la couleur, puis le trait) ; ce qui
  les distingue : le fond est déduit du trait (une seule forme, pas de choix de couleur par zone), la
  couleur se colorie zone par zone, avec ses propres images.
- Export, PDF, vignettes, montage, export Storyboard Pro : la couleur est un calque bitmap de plus
  (l'export Storyboard Pro le dit : le lien trait / couleur n'y existe pas, à vérifier).
- Projets : format **30** (un projet enregistré ainsi n'est pas lisible par une version d'avant) ; un projet
  d'avant s'ouvre tel quel. Détails : `src/calquecouleur.rs`, `docs/INTERFACE.md` partie 18.

## Toile infinie (dessiner hors de la feuille)

La feuille n'est plus une page cernée d'une ombre : on peut dessiner **n'importe où autour d'elle**, comme
dans Storyboard Pro (le fonctionnement exact de Storyboard Pro est **à vérifier**, SHOT reprend l'idée).

- **À l'écran** : le papier (ou le damier, ou le noir de l'arrière-plan) couvre tout ce que la vue regarde,
  plus d'ombre ni de zone hachurée autour de la feuille. Le **cadre de la caméra** est un trait fin et
  discret (1 pixel de l'écran, net, sur un liseré sombre qui le garde visible sur n'importe quel fond) ;
  « Estomper le hors-champ » (Caméra) voile, plus légèrement, tout ce qui est autour du cadre : l'opacité de ce
  **passe-partout** se règle dans Fichier > Préférences… (voir « Notes d'Anthony du 2026-10-20 » plus bas).
- **Dessiner** : le pinceau, le crayon, la gomme et les formes vont où on les mène, coordonnées négatives
  comprises (de −32 768 à 32 768 pixels autour de la feuille). Les traits vectoriels n'ont pas de limite
  de feuille. Une zone où l'on n'a rien dessiné ne coûte aucune mémoire (tuiles de 64 px à la demande).
- **Annuler / rétablir, enregistrer, ouvrir** : comme pour le reste. Ce qui est hors de la feuille est
  gardé dans deux petits fichiers par dessin (`<n>_hors.png` et `<n>_hors.json`), lus seulement s'il y en a ;
  un fichier abîmé ne coûte que le hors-feuille. Un projet d'avant s'ouvre tel quel, identique.
- **Caméra, export, PDF** : le cadre de la caméra peut regarder ce qui est hors de la feuille, l'image
  exportée le montre ; une caméra qui regarde un endroit vide donne du papier.
- **Pipette** : prend aussi hors de la feuille.
- **Ajuster à la fenêtre** (bouton de la barre sous le dessin) : cadre toujours la **caméra** (la feuille, pour un nouveau plan), avec
  un peu d'air autour, et non « tout ce qui est dessiné » (choix de SHOT ; le comportement de Storyboard Pro
  est à vérifier). Le zoom va de 2 % à 6 400 %, le défilement n'a pas de bord.
- **Limites connues** (à la feuille seulement) : le pot de peinture (il cherche ses contours sur la feuille ;
  hors d'elle il le dit et ne fait rien) ; les zones bitmap sélectionnées (lasso, baguette) ; les effets de
  calque, les calques d'ajustement, le fond sous le trait, les calques de couleur, les déformations
  (enveloppe, perspective) ; l'import et l'export Storyboard Pro ; la propagation. Ce qui est dessiné hors de
  la feuille s'y voit, s'y exporte, mais ces fonctions ne le touchent pas.
- **En dessous** : la feuille se compose comme avant (carte graphique) ; le reste du monde visible est une
  texture composée par le processeur (`src/monde.rs`), seulement là où il y a du dessin, par petits morceaux
  (5 ms par image au plus), de 4 096 px de côté au plus (réduite quand on dézoome beaucoup). Un trait loin de la
  feuille ne recompose que les tuiles qu'il touche. Mesure : 12 traits de 20 points à 12 000 px de la feuille,
  puis 40 défilements, pire image 2,4 ms. Détails : `docs/ARCHITECTURE.md` « Toile infinie ».
## Couleur des plans

Pour repérer les plans d'un coup d'œil : clic droit sur un plan de la bande du montage (ou sur
une vignette) > « Couleur du plan » > une pastille, ou « Aucune ». Ça vaut pour tous les plans
choisis (Ctrl ou Maj + clic). Le bloc prend la couleur (fond teinté et bande en haut), la
vignette un liseré ; la couleur suit le plan quand on le duplique, le copie ou le met dans un
nouveau projet. Ctrl+Z l'annule.

## Menus courts

Les menus montrent l'essentiel ; le reste est rangé dans un sous-menu (« Plans », « Importer »,
« Ordre », « Plus »…) ou une fenêtre :
- Fichier > Exporter… : film, images, image courante, PDF, page HTML, son, Storyboard Pro, et la
  case « PNG sans le papier » ;
- Fichier > Préférences… : l'apparence (voir ci-dessous), copie de secours, dernier projet rouvert,
  croix du système, panneaux ajustés, raccourcis clavier ;
- Aide > À propos de SHOT… : le logo, la version et le nom d'avant (Échap ferme la fenêtre).
- **Labo** (2026-10-20, demande d'Anthony) : les fonctions à l'essai, rangées ensemble — Suggestions sonores…,
  Lire le storyboard (proposer des sons), Propager la peinture du calque…, Enregistrer mes traits (banc
  d'essai du moteur). Le panneau Suggestions sonores n'a plus d'icône dans la barre des panneaux ni
  d'entrée dans le menu Son ; « Propager… » reste aussi au clic droit d'un calque bitmap qui s'y prête.

### Apparence

Fichier > Préférences…, en tête : le **thème** de l'interface, qui change tout de suite.
- **Gris sombre** (par défaut) : gris neutres moyens, plus proches de Storyboard Pro ou de Clip
  Studio Paint que du noir ; **Gris clair** (texte sombre, logo clair) ; **Sombre**, l'ancien thème
  presque noir ; **Personnalisé**.
- Neuf couleurs se règlent : fond des panneaux, barres, fenêtres et menus, boutons et curseurs,
  texte, accent et sélection, fond autour du dessin, timeline et montage, bordures. Changer l'une
  d'elles passe en « Personnalisé », en partant du thème qu'on avait. Le survol des boutons, le
  texte secondaire, les couleurs des dessins, des clés, des marques et du son sont déduits de ces
  neuf-là, assez contrastés pour rester lisibles (un thème clair prend des repères plus foncés).
- « Rétablir » revient à Gris sombre. Le choix est retenu dans `reglages.toml` (section `[theme]`) ;
  un ancien fichier sans cette section donne Gris sombre.
- Les couleurs du dessin (papier, traits, couleurs du projet) ne changent jamais avec le thème.

Ce qui a déjà un bouton sous le dessin (zoom, rotation, miroir, pelure, table lumineuse, caméra)
n'est plus répété dans Affichage ni dans le clic droit du dessin. La couleur du papier se règle
dans l'onglet du bas Arrière-plan. Toute commande reste trouvable avec Ctrl+K.

## Panneaux qui s'ajustent

Quand deux panneaux sont l'un sur l'autre, celui des réglages (réglages de l'outil, guides, fiche,
effets) prend la hauteur de ce qu'il montre : avec la main ou la pipette, il se réduit et la
couleur gagne la place ; avec le pinceau, il grandit (moitié-moitié si les deux débordent). Les
effets sous les calques font de même. Tire un bord à la main : il reste où tu l'as mis, même
après relance. Affichage > Panneaux > Réajuster les panneaux oublie ces bords ; la case « Ajuster
les panneaux à leur contenu » coupe le tout.

## Barre des panneaux

Une fine bande d'icônes sur le bord droit de la fenêtre, une par panneau de côté : réglages de
l'outil, pinceaux, couleur, guides ; calques, effets, fiche du plan, feuille d'exposition ;
commentaires, historique, références. Un clic sur une icône ouvre le panneau, devant les onglets de son groupe. S'il est
ouvert mais caché derrière un autre onglet, le clic le met devant ; s'il est visible, le clic le
ferme. L'icône est enfoncée tant que le panneau est dans l'espace de travail. La bulle d'une
icône donne son nom et son raccourci ; clic droit > « Assigner un raccourci… » (chaque icône est
aussi une commande de Ctrl+K, et son raccourci suit la même règle). Affichage > Panneaux >
« Barre des panneaux » la montre ou la cache.

La feuille d'exposition n'existe qu'en animation (Anthony : « Exposition n'est disponible que dans
la partie animation ») : en storyboard, son icône, son entrée du menu Affichage > Panneaux et son
action disparaissent ou répondent d'un mot dans la barre d'état, et si elle était ouverte elle est
mise de côté ; au retour en animation (Tab, double-clic sur un plan) elle revient où elle était.

## Historique

Édition > Historique… ouvre la liste des étapes d'annulation, à côté des calques : chacune porte
le nom de ce qui l'a faite (« Pinceau », « Retourner le calque de gauche à droite », « Calques »…).
Clique une étape pour revenir juste après elle ; les étapes annulées restent en gris dessous
(clic : les rétablir), jusqu'à la prochaine modification.

## Commentaires sur la timeline

Un commentaire est un petit fanion dans la règle du montage (tout le film) et de la timeline du
plan (ceux du plan). Il tient à une image d'un plan : il suit son plan quand on le déplace, le coupe
ou le réunit à un autre.

- Poser : le bouton « Nouveau commentaire » des barres du montage et de la timeline, un double-clic
  dans la règle, ou le bouton du panneau Commentaires. Le fanion se pose à l'image courante et le
  panneau s'ouvre, le texte prêt à être tapé.
- Survoler un fanion : une bulle donne l'auteur, la date, l'endroit et le début du texte.
- Clic : choisit le commentaire et y va. Glisser : le déplace image par image (d'un plan à l'autre
  dans le montage) ; Échap abandonne. Double-clic : l'éditer. Clic droit : Modifier, Couleur,
  Marquer résolu, Supprimer. Suppr supprime le commentaire choisi quand le dernier clic l'a visé.
- Panneau Commentaires (barre des panneaux, ou Storyboard > Commentaires…) : la liste dans l'ordre du
  film (pastille, time code, plan, auteur, première ligne, case « résolu »), « Montrer les résolus »,
  et l'éditeur du commentaire choisi : texte sur plusieurs lignes, auteur, couleur, résolu. La liste
  est dans un cadre qui défile seul (chaque ligne tient sur une ligne, coupée par « … »), l'éditeur
  dessous : ils ne se superposent à aucune taille (panneau trop court : le panneau défile).
- Un commentaire résolu est estompé. L'auteur par défaut est l'utilisateur Windows, ou le nom donné
  dans Fichier > Préférences… > Commentaires.
- Enregistrés dans `commentaires.json`, à côté du manifeste du projet (le format du projet ne
  change pas) ; chaque changement s'annule et se rétablit.
- **Dans le PDF** : Fichier > Exporter > Storyboard PDF écrit aussi les commentaires non résolus sous
  l'image où ils tombent (« Commentaire (Anthony) : … ») quand on coche « Avec les commentaires ».

## Relecture par le réalisateur

Le réalisateur ou le client **n'a pas SHOT**. SHOT lui fabrique une page, il renvoie ses notes, SHOT les pose en
commentaires sur les bons plans. Détail et liste de vérifications : `docs/INTERFACE.md` partie 22.

**Aller** : Fichier > Exporter > « Pour relecture (page HTML) » (`fichier.exporter_relecture`), l'icône « lunettes » de
chaque envoi dans la fenêtre Envois (les plans modifiés depuis cet envoi sont proposés d'emblée), « Plus… » de la barre du
montage, Ctrl+K. La fenêtre propose : le titre, l'envoi concerné (son nom est écrit dans la page), les plans (tout le film,
la séquence courante, les plans choisis, les plans modifiés depuis un envoi), et dans la page : la lecture animée de chaque
plan (images réduites, page plus lourde), les commentaires déjà posés (en lecture seule, les résolus exclus), et un PDF
à imprimer (3 plans par page, avec un cadre à lignes pour écrire à la main) écrit à côté de la page.

**La page** : un seul fichier `.html`, images dedans, qui s'ouvre hors ligne dans n'importe quel navigateur (ordinateur ou
téléphone, thème clair). Un plan par carte : numéro (celui de la numérotation du projet), « V2 » s'il y a des versions,
durée, textes d'action, dialogue, notes, une carte par image de storyboard (marque d'image). Pour chaque image : un champ de
note, et « Dessiner » (stylo à trois couleurs, gomme, annuler le trait, tout effacer) directement sur l'image, au doigt, au
stylet ou à la souris. Une note générale, le nom du relecteur, « Enregistrer mes notes » qui télécharge
`notes-<titre>-<date>.json` (à renvoyer par e-mail). Les notes en cours sont gardées dans le navigateur (s'il le
permet) et retrouvées en rouvrant la page.

**Retour** : Fichier > Importer > « Des notes de relecture (.json) » (`fichier.importer_notes`), ou le fichier lâché sur la
fenêtre. Le fichier est lu et vérifié sur un autre fil ; une fenêtre dit combien de notes (dont avec dessin) seront posées sur
combien de plans, lesquelles sont sans plan, lesquelles sont déjà importées ; « Importer » les pose en **une** étape
d'annulation (« Importer les notes de relecture »), puis le compte rendu : « 12 notes importées sur 9 plans, 1 note sans
plan (plan supprimé). » Chaque note devient un commentaire (fanion jaune, auteur = le relecteur, date de la note, sur
l'image de storyboard visée). Le dessin du relecteur s'affiche par-dessus l'image quand son commentaire est choisi ;
l'interrupteur du panneau Commentaires le cache (il reste dans le commentaire).

**Où va une note** : par l'**identifiant du plan** (stable aux déplacements et aux renumérotations) ; à défaut, par son
**numéro** ; à défaut, « sans plan » (listée, et on peut cocher « Les poser sur le plan courant » : elles y arrivent avec
leur provenance). L'image visée est retrouvée par l'identifiant de sa marque d'image, sinon le début du plan. La note
générale va sur le premier plan qui reçoit une note. **Réimporter le même fichier ne double rien** : la clé de la note est
gardée dans le commentaire (`commentaires.json`, champ `note`), donc aussi après avoir fermé et rouvert le projet. Un fichier
qui vient d'un autre projet (nom différent) demande une confirmation.

**Format du fichier de notes** (`shot-relecture`, version 1, JSON UTF-8) :
`{"format","version","paquet","projet","relecteur","cree"(secondes),"date","general","notes":[{"id","plan","image","numero","titre","texte","dessin"(data:image/png;base64,…)}]}`.
Le fichier du projet ne change pas de version : `commentaires.json` gagne seulement deux champs facultatifs (`note`,
`dessin`) ; un projet sans note de relecture s'écrit comme avant.

**Sécurité** : la page n'a ni lien, ni script, ni police, ni image du dehors, aucune requête (politique de contenu
`default-src 'none'` + un seul script, le même pour tous les projets, qui ne contient aucun texte du projet) ; tout texte du
projet (titres, dialogues, commentaires) est échappé. Le fichier de notes est traité comme **venant d'un inconnu** : 40 Mo
au plus, format et version vérifiés, 3000 notes au plus, textes nettoyés et coupés à 20 000 caractères, identifiants réduits
à lettres / chiffres / `-_.`, aucun nom de fichier du fichier n'est jamais utilisé, dessins PNG décodés sous limites (4096
points par côté, 8 millions de pixels, 64 Mo de mémoire), réduits à 1280 de large et réécrits proprement (seuls les pixels
restent). Un dessin refusé ne fait pas perdre sa note.

**Sans latence** : l'interface ne calcule que les images (40 ms par image d'écran) ; leur réduction en JPEG, la page et le
PDF sont écrits sur un autre fil, avec barre d'avancement, temps restant et « Arrêter » (rien n'est laissé : les fichiers
sont d'abord écrits en `.part`, une page exportée avant sous le même nom n'est jamais détruite). Le projet est verrouillé
tant que les images se calculent.

Code : `src/relecture.rs` (modèle, page, notes, placement), `src/relecturejob.rs` (tâche), `src/relectureui.rs`
(fenêtres, dessin sur l'image), `assets/relecture/page.js` et `page.css` (la page), `src/relecture_tests.rs`.

## Notes d'Anthony : dessin (2026-10-20, branche `notes-dessin`)

Huit notes du réalisateur sur le dessin. Le détail, les règles de touches et la liste à regarder à l'écran sont
dans `docs/INTERFACE.md`, partie 24.

- **Tracé droit** : **Maj tenue pendant un trait** (pinceau, crayon, gomme ; vectoriel ou bitmap), le trait
  s'arrête là où il était et un segment droit le relie à la pointe (aperçu en ligne à la largeur du pinceau) ;
  **Maj+Alt** : l'angle par pas de 15° (comme la Maj des formes) ; Maj relâchée ou bouton relâché, le segment est
  posé d'un coup. Maj tenue dès l'appui : du début du trait à la pointe. Un seul geste, une seule étape
  d'annulation. Un trait aligné sur un guide est déjà droit : Maj n'y change rien. Pas de « Maj + clic, clic »
  façon Photoshop.
- **Choisir un objet au clic** : **Ctrl + clic** (le lasso temporaire de Ctrl) choisit l'objet sous la souris :
  sur un calque vectoriel le trait ou la zone remplie (comme avant), sur un calque bitmap **la forme d'un seul
  tenant** (pixels dessinés qui se touchent, huit voisins ; un trait fin se vise à quatre pixels d'écran près).
  **Ctrl+Maj + clic** ajoute un autre objet à la sélection, sur n'importe quel calque, ou le retire s'il y était ;
  Ctrl+Maj + lasso ajoute tout ce qu'il prend. Un clic dans le vide désélectionne ; un glisser reste un lasso.
- **Un pinceau par sorte de calque** : SHOT retient le dernier pinceau utilisé sur un calque vectoriel et le
  dernier sur un calque bitmap, et rend celui de la sorte du calque quand on change de calque. Retenus dans les
  réglages de l'application, ils reviennent au lancement suivant.
- **Enregistrer la brosse** : une icône disquette à côté du nom du réglage (panneau Réglages de l'outil) :
  « Enregistrer dans la brosse » (remplace la brosse d'où vient le réglage ; rétablissable aussitôt),
  « Enregistrer comme nouvelle brosse… » (nom demandé sur place). Retouché à la main, le nom devient
  « (réglé à la main) d'après « Encre » ». Les brosses de SHOT ne se remplacent pas.
- **Réglages repliés** : tout ce qui est sous l'aperçu du trait (catégories et réglages) est replié derrière une
  petite flèche, l'état est retenu. **Fond de l'aperçu** : un gris un peu plus clair que le panneau (les trois
  thèmes), poussé vers le blanc ou le noir si le trait lui ressemble.
- **Remplir la sélection** : Maj+Suppr, sur une zone bitmap (voir plus haut).
- **Clés de position** (outil Transformer, réglages > « Clés de position ») : un bouton pose une clé avec la pose
  du calque à l'image courante ; un calque qui a des clés est animé, le déplacer / agrandir / tourner à une autre
  image pose ou met à jour la clé de cette image, et entre deux clés il va de l'une à l'autre (lecture, export,
  vignettes). **Lisser** : la clé arrive et repart lentement ; arrivée et départ se règlent en images, comme les
  clés de caméra (`camera::ease`). Dans la timeline : losanges verts sur la ligne du calque (ronds quand la clé est
  adoucie), clic pour choisir, glisser pour déplacer, Suppr, clic droit. Le format du projet passe à **33**
  (`cles_position` d'un calque).

Code : `src/tracedroit.rs`, `src/selobjet.rs`, `src/remplirsel.rs`, `src/poskeys.rs` (modèle), `src/poskeysui.rs`
(écran), `src/pinceaux.rs` / `src/pinceauxui.rs` / `src/brushui.rs` (brosses), `src/dessin_notes_tests.rs`.

## Références visuelles

Le panneau **Références** (icône dans la barre des panneaux, action `vue.references`, fermé par défaut)
est un tableau libre d'images et de vidéos à regarder en dessinant. Clic droit sur son onglet >
« Dans une fenêtre à part » : une vraie fenêtre, pour un deuxième écran (le bouton « Toujours
au-dessus » y garde la fenêtre devant les autres).

- **Ajouter** : les boutons du haut (images PNG ou JPEG, vidéo, coller), des fichiers déposés sur le
  panneau, ou Ctrl+V quand le panneau a la main (l'image du presse-papiers de Windows : navigateur,
  capture…). Fichier > Importer propose aussi les deux premiers. Les images sont copiées dans le projet
  sur un autre fil ; une vidéo est extraite en images par ffmpeg, en arrière-plan (le même chemin que
  les calques vidéo, limitée à 1280×720). WebP n'est pas lu (le programme n'a pas ce décodeur).
- **Tableau** : glisser le fond (ou le bouton du milieu) pour se déplacer, la molette pour zoomer,
  glisser une référence pour la placer, un coin pour l'agrandir, la poignée ronde pour la tourner
  (Maj : par 15°), Maj+clic pour en choisir plusieurs, double-clic pour la cadrer (sur le fond :
  tout voir). Les boutons de la ligne du dessous règlent la référence choisie : ajuster, 100 %,
  miroirs, niveaux de gris (les valeurs, Rec. 709), nom, échelle, rotation, étiquettes. Trois façons de
  parcourir : tableau, grille, liste. La recherche trouve les mots du nom ou des étiquettes sans tenir
  compte des accents.
- **Couleur** : Alt+clic (ou Alt+glisser) sur une référence prend la couleur vue sous la souris et en
  fait la couleur courante de SHOT, comme la pipette du dessin (en gris, la valeur du gris).
- **Vidéo** : lecture / pause, image par image, curseur, entrée et sortie, boucle, vitesse ; « suivre
  la tête de lecture » montre l'image de la vidéo qui tombe sur l'image courante de SHOT, avec un
  décalage en images.
- **Plans** : « Lier au plan courant » (une référence peut aller avec plusieurs plans) ; le bouton en
  entonnoir ne montre que les références du plan courant (ce qu'on ajoute alors est lié d'office).
- **Rangement** : `references.json` à côté du manifeste et le dossier `references/` (le format du
  projet ne change pas). Même protection que les commentaires : un fichier illisible ou d'une version
  plus récente n'est jamais effacé (il est mis de côté en `.illisible`), et rien n'est effacé de
  `references/` si `references.json` n'a pas pu être lu. Une image retirée pendant la séance reste dans
  le dossier (annuler la suppression, même après avoir enregistré, la retrouve). Avant le premier
  enregistrement, les fichiers attendent dans un dossier temporaire.
- **Rapidité** : les images se décodent sur deux fils, à la définition que l'écran demande, et restent
  en mémoire graphique dans un cache dont les moins récemment vues partent d'abord (un huitième du
  réglage « cache de la carte graphique » du diagnostic des performances, 32 à 512 Mo ; la définition
  suit le réglage « vignettes »). Au repos, le panneau ne repeint rien.
- **Annulation** : ajouter, supprimer, déplacer, redimensionner, tourner, retourner, lier, nommer…
  sont des étapes de l'historique (« Ajouter une référence », « Déplacer les références »…).
- Actions : `vue.references`, `references.importer_image`, `references.importer_video`,
  `references.coller`, `references.supprimer`, `references.lier_plan`, `references.filtre_plan`,
  `references.tout_voir` (aucune touche par défaut ; Ctrl+V et Suppr agissent quand le panneau a la main).

## Rogner les plans dans le montage

Un plan se raccourcit ou s'allonge par ses deux bords dans la bande de montage, sans rien détruire :
ce qui sort du plan est gardé en réserve et revient en le rallongeant (un petit triangle au bord dit
qu'il reste du contenu caché de ce côté).

| Geste | Effet |
|---|---|
| Glisser le **bord gauche** d'un plan | rogne ou rallonge son **début** (vers la droite : le plan perd ses premières images ; vers la gauche : elles reviennent de la réserve, puis le premier dessin est tenu) |
| Glisser le **bord droit** | rogne ou rallonge sa **fin** (c'était déjà là, mais détruisait ce qu'on coupait) |
| **Alt** + glisser un bord, entre deux plans | **roll** : la coupe se déplace, l'un raccourcit, l'autre s'allonge, le total ne change pas |
| **Alt** (ou Ctrl+Alt) + glisser dans un plan | **glisser le contenu** : le plan garde sa durée et sa place, c'est ce qu'il montre qui avance ou recule dans sa réserve (rogne d'abord le début ou la fin) |
| Clic sur un bord (Alt : la coupe), puis ← / → | déplace ce bord d'une image |
| Clic droit sur un plan > « Durée… » | tape la durée exacte (`36`, `1:12` ou `1s12`, `+12`, `-4`) |
| Clic droit > « Raccourcir le début / la fin jusqu'à la tête de lecture » | coupe au trait de la tête de lecture (le bord gauche de l'image courante) |
| Clic droit > « Rétablir le contenu rogné » / « Oublier le contenu rogné » | reprend toute la réserve du plan, ou la vide (le rognage devient définitif) |

- **Ripple** par défaut : les plans suivants se serrent ou s'écartent d'autant, sans trou. Le son ne
  bouge pas (la barre d'état prévient quand les plans se décalent par rapport à lui).
- **Aimant** (bouton sous « Plus… » dans la barre du montage) : un bord se colle, à six points d'écran près, à la tête
  de lecture, aux autres coupes et aux clés de caméra et d'effets. Éteint, il suit la souris à
  l'image près. Pendant le geste, un repère montre où la coupe tombe dans le film d'avant et une
  étiquette donne la nouvelle durée et ce qui est caché. Échap rend tout comme avant.
- Dans la timeline du plan, la poignée de fin de la règle et les menus de la règle font la même chose.
- Ce qui tient aux images suit le contenu : calques (dessins, rôles, clés d'effets), clés de caméra,
  marques d'image, signets, entrée / sortie, commentaires et calques vidéo. Rogner pose une clé
  au bord du plan (caméra, effets) pour que le mouvement continue tel qu'il était ; elle repart quand
  le contenu revient. Ce qui est caché disparaît des vignettes, de l'export et du PDF ; les
  commentaires cachés reviennent avec leurs images.
- Chaque geste est une étape d'annulation nommée : « Raccourcir le début », « Allonger le début »,
  « Raccourcir la fin », « Allonger la fin », « Déplacer la coupe », « Glisser le contenu ». Le
  storyboard verrouillé les refuse. Les commandes sont des actions (`montage.duree`,
  `montage.debut_a_tete`, `montage.fin_a_tete`, `montage.rognage_retablir`,
  `montage.rognage_oublier`, `montage.aimanter`), sans touche par défaut : clic droit sur l'entrée d'un
  menu pour leur en donner une, ou Ctrl+K.
- À vérifier face à Storyboard Pro et aux logiciels de montage : la touche Alt pour le roll et le
  glisser, la coupe « au trait de la tête de lecture » (l'image sous la tête sort de la fin), et le
  premier dessin tenu quand on rallonge au-delà de la réserve. Ce sont des choix de SHOT.
- Enregistré dans `rognages.json`, à côté du manifeste (le format du projet ne change pas ; une
  version qui ne le connaît pas ne voit que les plans, et peut effacer les dessins cachés). Les
  noms d'instance et marques de couleur d'un dessin caché ne sont pas gardés à l'enregistrement.
  Dupliquer ou copier un plan ne copie pas sa réserve ; couper un plan garde le début caché au premier
  plan, réunir deux plans ne garde que le début caché du premier.

## Envois, plans modifiés, versions et comparaison

Savoir ce qui a changé entre deux livraisons du storyboard (« v2 au réal »), garder l'ancienne version d'un plan,
et les regarder l'une contre l'autre. Rien de tout cela ne change le format du projet (27) : un projet sans envoi ni
version est exactement celui d'avant (aucun fichier, aucun dossier, aucun calcul de plus).

**Marquer un envoi** : barre du montage > « Plus… » > « Marquer un envoi… » (icône avion en papier), menu Storyboard >
Envois, ou Ctrl+K `envoi.marquer`. On donne un nom (« v2 au réal ») et une date (celle du jour) ; SHOT note l'état de
**chaque plan**, garde de quoi le revoir (dossier `versions/`) et enregistre le projet. Un dessin qui n'a pas changé entre
deux envois n'est écrit qu'une fois. La fenêtre « Envois » (icône `GIT_DIFF` sous « Plus… », `envoi.liste`) liste les
envois, combien de plans diffèrent de chacun, et permet de retirer un envoi de la liste (ses fichiers restent).

**Plans modifiés depuis l'envoi** : un plan qui n'est plus comme au dernier envoi porte une **pastille ambre** en haut
à droite de son bloc dans le montage et de sa première vignette dans la vue Vignettes : un crayon (modifié) ou un
plus (ajouté depuis). La bulle dit depuis quel envoi (« Modifié depuis l'envoi « v2 au réal » »). Un plan qu'on remet
dans l'état de l'envoi (annuler) perd sa pastille. Sur la bande zoomée très loin (blocs de moins de 44 points), une bande ambre le long du
bord haut du bloc remplace la pastille. Le « dernier envoi » est le dernier marqué (pas le plus tardif par sa date).
- *Ce qui compte comme un changement* : ce qui se voit et ce qui dure : les dessins (pixels, traits), les calques et
  leurs réglages (visibilité, opacité, mode de fusion, transformation, profondeur, effets, calques d'effets), la
  caméra, la durée, les textes d'action, de dialogue et de notes, les marques d'image.
- *Ce qui n'en est pas un* : ce qui aide à travailler : la couleur du plan, les signets et points d'entrée / sortie, les
  notes dessinées de la timeline, le repère 3D, le nom d'un calque, son verrou, sa ligne repliée, son groupe de
  couleur, la pelure d'oignon et le pochoir, les noms d'instance et marques de couleur des dessins, les rôles des
  images, le déplacement du plan dans le film. Les commentaires, les transitions, le son et les scènes 3D de blocage ne
  sont pas non plus dans l'empreinte d'un plan.
- *Parcourir* : « Plan modifié suivant / précédent » (barre du montage sous « Plus… », menu Storyboard > Envois, fenêtre
  « Envois ») va au plan modifié suivant en tournant ; « Choisir les plans modifiés » les choisit tous (pour les
  copier, les colorer, les exporter ensemble). Actions `envoi.modifie_suivant`, `envoi.modifie_precedent`,
  `envoi.modifies_choisir`, sans touche par défaut (clic droit sur l'entrée d'un menu pour leur en donner une).
- *Coût* : sans envoi, rien. Avec des envois, un relevé par image d'écran (le numéro de l'historique et la somme des
  versions des dessins : moins d'une microseconde pour 1 200 dessins) ; après 0,6 s de calme (souris relâchée, pas de
  lecture) les plans sont copiés (au plus 1,5 ms par image d'écran) et **un fil** hache les dessins. Marquer un envoi
  copie tout d'un coup : 15 à 32 ms pour 600 plans et 1 200 dessins bitmap (banc `envois_tests`), le reste sur un fil.

**Versions d'un plan** : « Nouvelle version du plan » (barre du montage sous « Plus… », menu Storyboard > Versions du
plan, fiche du plan, Ctrl+K `version.nouvelle`) garde le plan tel qu'il est comme version plus ancienne (V1) et
continue sur une copie (V2). Les versions se nomment **V1, V2, V3…** partout. **Dans le montage**, un plan qui a
plusieurs versions porte une petite étiquette « V2 » près du haut de son bloc : la **survoler** ouvre, au-dessus du
bloc, une carte par version (aperçu, nom, croix) ; un clic sur une carte fait passer le plan à cette version, la croix
la supprime (après confirmation sur la carte ; jamais la version affichée). Le **clic droit sur un plan** (bande ou
vignette) a « **Nouvelle version ›** » : *en dupliquant la version en cours* ou *en partant d'un plan vide* (même
durée, même cadre, même caméra, un calque de dessin vide ; `version.vide`). Storyboard > Versions du plan >
« **Supprimer les versions non affichées…** » (tous les plans, ou le plan courant) garde la version affichée de chaque
plan et supprime les autres, après confirmation (« 7 versions de 4 plans seront supprimées ») ; les états gardés par les
envois ne sont pas touchés. La **fiche du plan** a aussi une ligne « Version » : la version active, les boutons
« Nouvelle version », et pour chaque autre version de quoi **passer** à elle (les deux échangent leur contenu), la
**comparer**, la **restaurer** (une copie d'elle devient une nouvelle version active ; rien n'est perdu) ou la
**supprimer** (jamais l'active). Le titre du plan dit « Plan 3 · V2 » (bande, planche, fiche, timeline, barre d'état)
quand il a plusieurs versions ; `version.precedente` et `version.suivante` passent de l'une à l'autre au clavier. Dans
l'export pour le montage, le clip du plan s'appelle `SQ01_PL012A_V2` quand il a plusieurs versions. Détail et liste de
vérifications : `docs/INTERFACE.md`, partie 15.
- Chaque version rangée a ses propres dessins : retoucher l'une ne touche pas l'autre (les tuiles sont partagées
  jusqu'à la première retouche, la mémoire ne double pas).
- Un geste = une étape d'annulation nommée (« Nouvelle version du plan », « Passer à la version V1 », « Restaurer la
  version V2 », « Supprimer la version V1 », « Nouvelle version du plan, vide », « Supprimer les versions non affichées ») ; le storyboard verrouillé les refuse.
- Ce qui reste au **plan** et non à la version : sa place dans le film, son numéro, ses commentaires, ses transitions,
  son contenu rogné (réserve) ; la barre d'état le rappelle quand un plan a un contenu rogné. Dupliquer un plan ne copie
  pas ses versions.
- L'envoi note le numéro de la version partie ; la copie d'un plan dans un autre projet ne prend que la version active.

**Comparer** : icône « Comparer le plan » (barre du montage sous « Plus… »), fenêtre « Envois » (une icône par envoi),
ligne « Version » de la fiche (une icône par version), clic droit sur un plan > « Version du plan » > « Comparer le plan… »,
ou Ctrl+K `comparer.ouvrir`. Une fenêtre à deux côtés : *Aujourd'hui*, chaque version du plan, ou le plan **tel qu'il
était à un envoi** (relu dans `versions/`). Quatre manières de voir : **côte à côte**, **volet** (un curseur qu'on
glisse : un côté à gauche, l'autre à droite), **fondu**, et **superposition** (ce qui n'est que d'un côté en rouge,
que de l'autre en bleu, des deux en noir). Une glissière choisit l'image du plan, une icône échange les côtés, une
autre relit tout. En bas : la part de l'image qui diffère (« 3,2 % de l'image diffère » ou « Les deux images sont les
mêmes »). **Lecture seule** : la fenêtre ne garde que des copies, ne touche ni au plan, ni à l'historique, ni aux
fichiers. Les images se composent sur un fil de fond (effets compris), jamais sur le fil de l'interface.

**Fichiers** (tous écrits par l'enregistrement habituel, atomiquement, sans jamais effacer un fichier illisible ou plus
récent : il est mis de côté en `.illisible` si un neuf doit le remplacer) :

```
MonFilm/
  envois.json                      les envois : nom, date, et pour chaque plan l'empreinte de son contenu
  versions/
    envoi-3-9f2a41c0.json          l'instantané du 3e envoi (écrit une fois, jamais modifié)
    dessins/<empreinte>.png|json   les dessins des instantanés (un par contenu)
    textures/                      les textures des traits texturés de ces dessins
  plans_versions.json              les autres versions de chaque plan (leurs dessins sont dans dessins/)
```

Rien de `versions/` n'est jamais effacé (même quand un envoi est retiré de la liste). « Enregistrer sous » recopie
`versions/`. Un projet ouvert sans `versions/` (déplacé sans lui) s'ouvre ; la comparaison avec un envoi dit alors quel
fichier manque.

## Notes d'Anthony du 2026-10-20 : calques, effets, montage, export, caméra

Sept demandes, une branche (`notes-calques`). Pour chacune : ce qui existait déjà, ce qui a été fait. Liste de ce qu'il
faut regarder à l'écran : `docs/INTERFACE.md`, partie 25. **Format de projet 33** (voir plus bas).

1. **Effets sur le trait et sur la matière dans un menu déroulant chacun.** Les deux rangées d'icônes du panneau Effets
   sont remplacées par deux boutons, « Trait ▾ » et « Matière ▾ » (le nombre d'effets allumés s'affiche dans le
   titre : « Trait (2) ▾ »). Chaque menu liste les effets avec leur **icône et leur nom**, allumés quand ils sont sur le
   calque ; ceux qu'un calque bitmap refuse restent **grisés avec la raison dans la bulle**. Les préréglages sont dans une
   section « Préréglages » du même menu. Clic sur une entrée : même action qu'avant (`fx.ecriture`…), une étape
   d'annulation ; clic droit : lui assigner un raccourci. Code : `fxui.rs` (`trait_menu`, `matiere_menu`).
2. **Dossiers de calques.** Le modèle (`Layer::folder`, `parent`, format 14), la timeline (bouton « Nouveau dossier »,
   triangle, glisser dedans et dehors, œil, verrou, opacité) et le menu Calques > Dossiers existaient **déjà** ; ce qui
   manquait, c'était le panneau **Calques** : maintenant un bouton « Nouveau dossier » (et « Ranger les calques choisis
   dans un dossier » quand plusieurs sont choisis) dans ses commandes, « Nouveau dossier » au clic droit dans le vide de
   la liste, la ligne du dossier avec son **triangle** (ouvrir / fermer, comme dans la timeline : le réglage est le même),
   son icône à la place de la vignette et « dossier · 3 calques », les calques rangés **décalés** vers la droite, et les
   calques d'un dossier fermé sans ligne (le calque courant reste toujours listé). Glisser une ligne entre les calques d'un
   dossier l'y range, au-dessus du dossier l'en sort (`move_layer_to`, déjà là) ; monter ou descendre un dossier emmène ses
   calques ; son opacité (le champ en haut, quand le dossier est choisi) multiplie celle de ses calques.
3. **Fond sous le trait : un bouton du calque, plus une option du pinceau.** Le fond a toujours été un réglage du calque
   (`Layer::fond`) : l'ancien bouton des réglages du pinceau et du crayon n'enregistrait rien dans les réglages de
   pinceau. Il est **retiré** de ces réglages et devient un bouton (icône carré-utilisateur) sur **la ligne de chaque calque
   de dessin** du panneau Calques, dans ses commandes (pour le calque courant) ; allumé = le calque a un fond. L'action
   `pinceau.fond` (raccourci d'avant) fait la même chose pour le calque courant. Aucune conversion : un projet ou des
   réglages d'avant se lisent tels quels. Les réglages du fond (couleur, débord, trous) et la bascule Trait / Fond restent
   sous le pinceau, visibles quand le calque courant a un fond.
4. **Bouton « fx » du calque.** Un petit « fx » à côté de l'œil et du cadenas (ligne du panneau Calques, ligne de la
   timeline, et « Effets du calque allumés » dans les menus de calque, la colonne du montage n'ayant pas la place) éteint
   **toute la pile d'effets** du calque d'un coup, sans la toucher (réglages, clés et effets cochés restent). Il ne se montre
   que sur un calque qui a des effets (ou qui les a éteints : il faut pouvoir les rallumer) ; éteint il est barré, et le
   panneau Effets le dit avec un bouton « Rallumer ». Une étape d'annulation, action `calque.effets` (Ctrl+K, sans touche).
   Enregistré dans le projet (`effets_eteints`, format 33) ; l'écran, l'export, les vignettes et les comparaisons le suivent
   (un seul point, `fx::layer_steps`).
5. **Durées différentes selon la version d'un plan.** Le modèle les gardait déjà (passer à une version échange la durée avec
   le contenu : c'est la durée du plan dans le montage, donc les plans suivants reculent ou avancent comme pour tout
   changement de durée, le son restant en place). Ce qui manquait : la tête de lecture et les choix d'images ne restaient
   pas dans le plan plus court (corrigé), la barre d'état ne le disait pas (« Durée du plan : 2s 00 → 3s 00 ; les plans
   suivants reculent de 24 i, le son reste en place. »), et on ne voyait pas la durée de chaque version : elle est dans les
   **cartes de la pile** (« V2 3s 00 »), dans la bulle de la carte, dans la fiche du plan et le menu « Version du plan ».
   Passer à une version, la restaurer : toujours **une étape d'annulation**. L'export pour le montage prend la durée de la
   version active (et son nom : `…_V2`) ; l'empreinte des envois suit la version (un plan d'une autre durée est « modifié »,
   et ne l'est plus quand on revient à la version envoyée) ; « Comparer » reste dans les images de chaque côté. Limite : le
   contenu rogné (réserve) reste attaché au plan, pas à la version.
6. **Exporter un GIF animé.** Fichier > Exporter… > « Un GIF animé » (ou la fenêtre d'export, ou Ctrl+K
   `fichier.exporter_gif`) : le film, la séquence, le plan courant ou les **plans choisis** ; une **taille** (480 px, 720 px
   de large, 50 % ou image finale, jamais plus grand que l'image finale), une **cadence** (6 à 24 images par seconde, jamais
   plus que celle du projet), **boucle** (sans fin ou une fois) et **tramage** (dégradés plus doux, fichier plus lourd).
   Même tâche de fond que le film : progression, temps restant, Annuler (le GIF inachevé est effacé). ffmpeg
   (`palettegen` puis `paletteuse` : une palette de 256 couleurs calculée sur tout le GIF) ; **aucune caisse de plus** ;
   seules les images que le GIF garde sont calculées. Pas de son. Les transitions du montage peuvent y être.
7. **Cadre de la caméra plus fin, passe-partout réglable.** Le cadre est tracé à **1 pixel de l'écran** (1,5 avec l'outil
   Caméra), calé sur la grille des pixels, sur un liseré sombre d'un pixel de chaque côté qui le garde lisible sur fond clair
   comme sur fond sombre. Fichier > Préférences… > Dessin > « **Opacité du passe-partout** » (0 à 100 %, 27 % d'origine,
   « Rétablir ») règle le voile de « Estomper le hors-champ » ; sur la toile infinie il vaut 65 % de la valeur, et pendant la
   lecture un peu plus du double. Réglage de l'application (`opacite_passe_partout`), pas du projet.

**Format du projet : 33** (`src/project.rs`). Un seul champ nouveau, `effets_eteints` dans l'entrée d'un calque, écrit
seulement s'il est vrai (aucun calque éteint : le manifeste est celui d'avant, au numéro près). Une version précédente refuse
un projet 33 plutôt que de montrer les effets qu'on a éteints. Un projet de 32 ou moins s'ouvre tel quel, effets allumés.
Les durées par version n'ajoutent rien (c'est le contenu de `plans_versions.json` d'avant) ; le GIF et le passe-partout non plus.

## Transitions du montage

Une transition habille une **coupe** entre deux plans, ou le **début / la fin du film** : fondu enchaîné,
fondu au noir, fondu au blanc, volet, noir. Elle se voit dans la bande de montage comme un petit nœud posé
sur la coupe (papillon pour le fondu enchaîné, losange pour le noir ou le blanc, rampe au bord du film).

- **Poser** : clic droit sur une coupe > « Transition ▸ » (aussi dans le menu d'un plan : « Transition au
  début / à la fin du plan ») ; le bouton « Ajouter une transition » (sous « Plus… » dans la barre du montage) vise la coupe la
  plus proche de la tête de lecture (un fondu enchaîné entre deux plans, un fondu au noir au bord du film) ;
  ou une commande de Ctrl+K : `transition.ajouter`, `transition.fondu_enchaine`, `transition.fondu_noir`,
  `transition.fondu_blanc`, `transition.volet`, `transition.noir`, `transition.supprimer`,
  `transition.apercu`, sans touche par défaut (clic droit sur l'entrée d'un menu pour leur en donner une).
- **Durée** : 12 images par défaut. On tire l'un des bords du nœud, ou on tape la durée dans le menu du
  clic droit sur la transition (avec la sorte et le calage). Clic : la choisit, Suppr : la supprime.
- **Temps** (choix de SHOT, à confirmer à l'écran) : **le film ne change jamais de durée**. La transition est
  centrée sur la coupe (calage « Finit à la coupe » ou « Commence à la coupe » aussi) et utilise les images
  des deux plans autour d'elle ; l'image qui manque d'un côté est **tenue** (le plan sortant garde sa
  dernière image, l'entrant sa première). Elle ne sort jamais des deux plans qu'elle relie. Les poignées du
  rognage (la réserve) ne servent pas encore à avoir des images vivantes : à faire. Au début du film, le
  fondu part de la couleur (la première image est toute noire) ; à la fin, il y va (la dernière est toute noire) ;
  sur une coupe, il passe par la couleur.
- **Lecture et export** : la carte graphique pose le plan entrant sur le plan sortant pendant la lecture
  (chaque plan vu par sa propre caméra en vue caméra) ; à l'arrêt, le mélange se voit aussi
  (`transition.apercu` pour le couper si l'on dessine dans la zone). Le film et les images exportés
  contiennent les transitions, image par image (case « Avec les transitions du montage »). L'export vers
  Storyboard Pro ne les reprend pas (comment Storyboard Pro range les siennes : à vérifier) ; le compte
  rendu de l'export le signale.
- Enregistrées dans `transitions.json`, à côté du manifeste (le format du projet ne change pas) ; un
  fichier illisible ou d'une version plus récente ne bloque pas l'ouverture et n'est jamais effacé.
  Chaque changement est une étape d'annulation nommée (« Ajouter une transition : fondu enchaîné »,
  « Durée de la transition »…).
- Une transition tient à deux plans (par leur numéro) : si l'un est supprimé, elle part avec lui ; si les deux
  ne se suivent plus (plan déplacé, coupé ou réunion), elle est gardée sans jouer, et l'annulation la rend.

## Effets sonores

Sur les extraits de la bande de montage (l'extrait doit être choisi) :

- **Fondus** : deux petites poignées aux coins du haut de l'extrait, qu'on tire dans le temps ; le coin qui
  disparaît s'assombrit. Ou les commandes `son.fondu_entree` / `son.fondu_sortie`.
- **Fondu enchaîné** entre deux extraits **qui se touchent** sur la même piste (`son.enchaine`, ou la case
  du menu) : une croix sur la jonction dont on tire la poignée. Le fondu est centré sur la jonction, en
  puissance égale, et utilise le son que chaque extrait a de l'autre côté de sa coupe ; s'il n'y en a
  pas (extraits qui commencent au début de leur son), il se raccourcit jusqu'à la coupe sèche. Les extraits
  d'une piste ne se chevauchent toujours pas.
- **Volume** de l'extrait (curseur du menu, jusqu'à 400 %) et de la piste ; **muet**, **solo** (bouton S de
  l'en-tête de la piste : seules les pistes en solo s'entendent, même muettes).
- **Clés de volume** : double-clic sur l'extrait pose une clé là où l'on a cliqué, on la tire (temps et
  niveau, collé à 100 % près de l'unité), Suppr la retire. La courbe est linéaire entre les clés.
- **Égaliseur** à trois bandes (graves, médiums, aigus, ±12 dB) et **écho** (niveau, délai, retour) : dans
  le menu du clic droit sur l'extrait > « Effets du son ». Sans nouvelle bibliothèque : filtres du second
  ordre et ligne à retard. Ils se calculent sur un fil à part ; en attendant, on entend le son tel quel.
- Lecture, son au défilement, WAV du film, son des vidéos exportées et fichiers son de l'export vers
  Storyboard Pro passent tous par le même calcul (`audiofx::build_mix`) : ce qu'on entend est ce qu'on exporte.
- Enregistrés dans `effets_son.json` (même principe que `transitions.json`) ; une étape d'annulation nommée par réglage et par geste.

## Bruitages

Le panneau **Bruitages** (barre des panneaux, ou Son > Bruitages…) garde les dossiers de sons de
l'utilisateur : WAV, MP3, FLAC, OGG, sous-dossiers compris. Les dossiers sont un réglage de l'application
(`reglages.toml`), retenus d'un projet à l'autre ; la liste se lit sur un fil à part à la première ouverture du
panneau (« Actualiser » la relit).

- **Chercher** : tous les mots tapés, dans le nom et le sous-dossier. La durée de chaque son se lit à la
  demande, pour les seules lignes qu'on voit.
- **Écouter** : le bouton ▶ de la ligne (le même une seconde fois arrête) ; la lecture du projet s'arrête.
- **Poser** : glisser la ligne sur une piste son de la bande de montage (un repère montre l'image visée),
  ou sur la ligne du son de la timeline ; un double-clic la pose à la tête de lecture (sur la piste de
  l'extrait choisi, sinon la première libre). Le fichier est **copié dans le projet**, comme un son
  importé. `bruitage.inserer` et `bruitage.ecouter` agissent sur le son choisi.

## Suggestions sonores (phase 1)

Le panneau **Suggestions sonores** (menu **Labo** > Suggestions sonores…, `vue.suggestions_sonores`) lit
le storyboard et **propose des sons témoins** pour l'animatique : bruitages, ambiances, musique d'intention, voix
des dialogues. On les écoute, on les accepte, on les régénère ou on les supprime ; rien n'entre dans le montage
sans accord. Le détail, le contrat du serveur local, comment le lancer et ce qui manque encore sont dans
`docs/GENERATION_SON.md`.

- **Lire le storyboard** (`suggestion.lire`) : tout le film, la séquence courante, le plan courant ou une plage de
  plans. Chaque proposition a une sorte (icône), un plan et un moment, une description française, un prompt
  anglais pour le générateur (une réplique, un personnage et une voix pour une voix) et un état : proposée, en
  génération, prête, acceptée, rejetée. « Modifier » change tout cela ; une relecture ne ramène pas une
  proposition rejetée.
- **Marche tout de suite, sans modèle** : la lecture par **règles** (choix d'origine) lit l'action, les notes et le
  dialogue : chaque ligne de dialogue devient une voix, des mots-clés font des bruitages, des ambiances et une musique,
  et une ligne « SFX : porte qui grince, pas dans l'escalier » est reprise telle quelle. Elle ne génère pas de son.
- **Trois rôles, un fournisseur chacun** (Fichier > Préférences > Génération sonore) : lecture (règles, Qwen2.5-VL
  local ou Claude en ligne), bruitages et ambiances (Stable Audio Open local, ElevenLabs ou Stable Audio en ligne), voix
  (Kokoro-82M local, ElevenLabs en ligne). Le **serveur local** (`sidecar/serveur.py`, Python, 127.0.0.1, port
  réglable) est écrit : SHOT le lance tout seul (sans fenêtre) au premier « Générer » local et l'arrête en se
  fermant, ou « Démarrer le serveur local » / « Arrêter » / « Tester » dans les Préférences (dossier `SHOT-modeles`
  et Python réglables). **Les voix (Kokoro) et la lecture par Qwen2.5-VL marchent** ; les bruitages attendent
  Stable Audio Open, pas encore téléchargé (licence à accepter par l'utilisateur). Les requêtes des services en
  ligne sont prêtes, leur envoi HTTPS est **à brancher** (aucune caisse TLS hors ligne).
- **Accepter** copie le son dans le projet et le pose à la bonne image sur la piste « Sons proposés » (une étape
  d'annulation). Tout travail se fait sur un fil à part, avec son avancement, et s'annule.
- **En ligne** : le panneau dit ce qui part (vignettes, textes, descriptions, répliques) et une fenêtre demande une
  confirmation, une fois par projet, avant le premier envoi. Les **clés** sont dans le Gestionnaire d'identifiants de
  Windows, jamais dans un fichier. Les licences des modèles et des services sont à vérifier.
- Enregistré dans `sons_proposes.json`, à côté du manifeste (le format du projet ne change pas ; un fichier
  illisible ou plus récent ne bloque pas l'ouverture et n'est jamais effacé). Les sons générés attendent dans
  `%LOCALAPPDATA%\SHOT\sons_generes\`.

## Crayonnés rapides (la planche)

Une page de petites cases, toutes au rapport du cadre du projet, où l'on enchaîne des croquis très
vite (thumbnailing) ; chaque case devient ensuite un plan. Ce n'est pas une vue des plans existants :
c'est une surface de dessin. S'ouvre par Storyboard > Planche de crayonnés…, le bouton à côté de
« Vue vignettes » (barre du montage), ou Ctrl+K > « Planche de crayonnés » (`storyboard.crayonnes`) ; elle
prend la place du dessin en storyboard.

- **Dessiner** : directement sur la page, au crayon (trait vectoriel lisse, net à toute échelle),
  au pinceau du logiciel (bitmap) ou à la gomme (coupe les traits, efface la peinture) ; les touches
  des outils marchent. Sans latence, comme dans le dessin : l'aperçu du trait est dans l'image de la
  case à chaque point reçu, jamais de rejeu du moteur. **Un trait appartient à la case où il
  commence** et est coupé à elle (à l'écran comme dans le plan).
- **Barre** : crayon, pinceau, gomme · trois tailles · noir et deux gris · annuler, rétablir · vider la
  case, insérer une case, supprimer la case · page précédente / suivante, ajouter une page · grille
  (2×2, 3×2, 4×3 par défaut, 4×4, 6×4) · case suivante automatique (éteinte : après une pause d'une
  seconde, la case suivante) · toutes les cases dessinées · Créer les plans · fermer.
- **Aller de case en case** : Tab / Maj+Tab et les flèches (Page préc. / suiv. : la page) quand la planche
  a la main (le dernier clic y était). Chaque case a son numéro et, dessous, une ligne de légende (texte
  court) qui deviendra le texte d'action du plan.
- **Choisir** : clic sur le numéro d'une case (Maj : jusqu'à la dernière cliquée, Ctrl : ajouter ou
  retirer) ; Maj ou Ctrl + appui dans la case marchent aussi (sans tracer) ; « Toutes les cases dessinées ».
  **Changer l'ordre** : glisser le numéro d'une case sur une autre. Clic droit sur une case : choisir,
  créer son plan, insérer avant / après, reculer, avancer, vider, supprimer, cocher.
- **Créer les plans** : une fenêtre demande où (après le plan courant, ou dans une nouvelle séquence) et la
  durée de chaque plan (2 s). Un plan par case choisie, dans l'ordre de la planche ; le dessin de la case
  va sur un calque « Crayonné » (les traits restent vectoriels et sont mis à l'échelle du cadre ; la peinture
  du pinceau va sur un calque bitmap « Crayonné (pinceau) », rééchantillonnée) sous un calque « Dessin »
  vide ; la légende devient le texte d'action. **Une seule étape d'annulation** du projet. Les cases restent
  sur la page, avec une petite coche verte.
- **Annuler** : la planche a sa propre pile (traits, gomme, ordre, cases ajoutées ou retirées, vidées,
  légendes, grille). Ctrl+Z dans la planche défait aussi la création des plans quand c'est la dernière
  étape du projet. Suppr vide les cases choisies (ou la courante) ; Ctrl+A les choisit.
- **Enregistrée** dans `crayonnes.json`, à côté du manifeste (le format du projet ne change pas ; absent,
  il n'y a pas de planche ; une planche sans rien dedans n'écrit rien). Les protections de la
  fiabilité s'appliquent : un fichier illisible ou d'une version plus récente n'est jamais effacé ni
  réécrit (il est mis de côté sous `.illisible` si une planche neuve doit prendre sa place), et l'écriture se
  fait atomiquement sur le fil d'enregistrement.
- Limites : les traits d'une case sont lisses (pas de pinceau texturé dans les cases : leur rendu est exact et
  immédiat) ; une case garde la taille et le rapport qu'avait le cadre à sa création (un cadre d'un autre
  rapport : la case est mise à l'échelle sans déformation, centrée) ; le pinceau bitmap rééchantillonné d'une
  case de 640 px sur un cadre de 1 920 px est flou ; « Créer » rééchantillonne la peinture sur le fil de
  l'interface (une peinture de pleine page prend une fraction de seconde par case).

## Palette de commandes

Ctrl+K (ou Aide > Palette de commandes) : tape quelques lettres du nom d'une commande (« persp »,
« retourner », « flou »), la palette propose les commandes avec leur raccourci. Flèches pour
choisir, Entrée pour lancer, Échap pour fermer. Chaque commande y est une fois, sous son nom (les doubles
`scene.*` des commandes de plan n'y sont pas).

## Calques d'effets et préréglages d'effets (2026-10-19, branche `calques-effets`)

Demande d'Anthony : des calques d'effets posés par des préréglages d'animation (impact frame, glow,
tremblement de l'image, timecode…).

**Un calque d'effet** est un calque de la pile sans dessin à lui (icône étincelle et teinte cyan dans le
panneau Calques, le montage et la timeline) :
- **Ajustement** (comme un calque de réglage de Photoshop ou d'After Effects) : sa pile d'effets, avec ses clés,
  s'applique à **tout ce qui est dessous** dans le plan, papier compris, ou seulement dans son **dossier** s'il
  est rangé dans un dossier. Il agit aux images où il a une **exposition** (un dessin vide tenu : on l'étire, le
  coupe, le déplace dans la timeline comme un dessin) ; il a un œil, une opacité et un mode de fusion comme les
  autres calques. On ne dessine pas dessus.
- **Incrustation** : du texte posé sur l'image finale, dans le cadre de la caméra (il ne suit ni la caméra ni les
  calques) : **timecode HH:MM:SS:IM** à la cadence du projet (entière : pas de « drop frame »), numéro d'image,
  nom du plan, nom de la séquence, titre du projet, date ou texte libre (avec les mots `{timecode}` `{image}`
  `{plan}` `{sequence}` `{titre}` `{date}`). Neuf places, taille (% de la hauteur du cadre), couleur, boîte
  derrière le texte, marge, compté depuis le film ou depuis le plan, et **À l'écran et à l'export / Seulement à
  l'écran / Seulement à l'export**. Gravée dans toute image exportée (film, images, image courante) quand le
  calque est visible ; les vignettes, le PDF et le HTML de storyboard ne la portent pas. Se règle dans le panneau
  Effets quand le calque est choisi. Texte tracé avec la mise en page de l'outil Texte (`text.rs`).

**Où ajouter** : le bouton étincelle du panneau Calques, « Nouveau calque > Calque d'effet » du menu Calques et des
clics droits des calques, « Préréglages » du panneau Effets, et Ctrl+K (`effet.impact`, `effet.lueur`,
`effet.flash_blanc`, `effet.flash_couleur`, `effet.tremblement`, `effet.vignettage`, `effet.grain`,
`effet.aberration`, `effet.flou_vitesse`, `effet.timecode`, `effet.vide`). Chaque préréglage ouvre une petite
fenêtre (intensité, durée ou « jusqu'à la fin du plan », couleur, graine, et ses réglages à lui) qui **montre
l'effet en direct sur le dessin** pendant qu'on la règle (par le chemin du fil de fond, sans jamais bloquer) ;
« Appliquer » le pose, en **une étape d'annulation** (« Préréglage d'effet : Impact frame »). Les clés
partent de la tête de lecture et restent toutes modifiables dans le panneau Effets, la timeline et le montage.
« Sur le calque courant » ajoute les mêmes effets, avec leurs clés (et des clés « éteint » avant et après), à la pile
du calque courant au lieu de créer un calque. Le nouveau calque est posé tout au-dessus (au-dessus du calque courant
s'il est rangé dans un dossier). « Calque d'effet vide » couvre tout le plan, sans effet.

| Préréglage | Ce qu'il pose | Réglages |
|---|---|---|
| Image d'impact | effet **Image d'impact** : négatif, noir et blanc, noir et blanc inversé ou postérisé, teinte, éclat de rayons, lignes de vitesse (tracés au calcul, graine). Allumé et éteint d'un coup | durée (2 images), mode, teinte + couleur, éclat, lignes, graine |
| Lueur (glow) | effet **Lueur diffuse** : ce qui dépasse le seuil de brillance est flouté et ajouté en écran ; pulsation par clés | intensité, seuil, pulsation + période, durée (jusqu'à la fin) |
| Flash blanc, flash de couleur | effet **Flash** : monte vite, redescend | intensité, durée (8 images à 24 i/s), couleur |
| Tremblement de l'image | effet **Secousse de l'image** : un décalage et une rotation par image, tirés des **mêmes générateurs de bruit que les effets de caméra** (`camfx.rs` : léger, à l'épaule, explosion, secousse), bornés par l'intensité ; flou de bougé en option. Secoue l'image déjà dessinée, tout ce qui est dessous | style, intensité, fréquence, durée, flou de bougé, graine |
| Vignettage | effet **Vignettage** centré sur le cadre de la caméra | intensité |
| Grain de film | effet **Grain de film** (change à chaque image) | intensité, graine |
| Aberration chromatique | effet **Aberration chromatique** légère | intensité |
| Flou de vitesse | le **Flou de mouvement** existant, qui monte puis retombe | intensité, direction, durée |
| Timecode et incrustations | un calque d'**incrustation** | contenu, place, taille, couleur, boîte, compté depuis, visible |

Les sept effets nouveaux (famille **Cinéma** de « Ajouter un effet ») sont des effets ordinaires de la pile
(`src/fxcine.rs`) : ils servent aussi sur un calque de dessin. Valeurs en pixels proportionnelles à la hauteur du
cadre ; l'ampleur du tremblement est celle de `camfx.rs`, à régler à l'œil — aucun équivalent dans Storyboard Pro ni
dans TVPaint n'a été vérifié (à vérifier).

**Rendu** (`src/calqueffet.rs`) : un calque d'ajustement actif pose dans le plan un repère ; l'écran le remplace, avec ce
qu'il couvre, par UN dessin dérivé (la composition d'en dessous passée par les effets, puis fondue avec l'opacité
et le mode du calque), gardé par `fxcache.rs` comme ceux des effets d'un calque. À l'écran il est demandé au fil
de fond (jamais calculé sur le fil de l'interface), à la taille de l'aperçu pendant la lecture et les gestes, et les
images qui viennent sont préparées d'avance ; en attendant, l'image se montre sans l'effet. À l'export et dans les
essais il est calculé tout de suite, à pleine résolution ; la carte graphique et le processeur composent le même dessin.
Pendant le déplacement d'une sélection flottante sous un calque d'effet, l'effet attend la fin du geste. **Sur la carte
graphique** (réglage allumé par défaut, `src/gpufx.rs`), un calque d'ajustement hors dossier ne passe plus par ce dessin
dérivé : la carte applique ses effets à l'image composée jusque-là, à pleine résolution, à chaque image ; le chemin
ci-dessus sert pour un calque dans un dossier, avec le réglage éteint, sans carte graphique et à l'export. Mesure :
`cargo test --release --offline --target-dir target\msvc --lib effetsperf -- --ignored --nocapture --test-threads=1`
(voir `docs/PERFORMANCE.md`).

**Format 27** : champ `calque_effet` d'un calque (`ajustement` ou `incrustation` et ses réglages) et sept sortes d'effets
de plus ; un projet plus ancien s'ouvre tel quel, SHOT d'avant refuse un projet 27 (le film changerait sans prévenir).

## Passe d'interface (2026-10-19) : un mot par idée, les mêmes gestes pour les trois sortes de clés

Détail et propositions : `docs/INTERFACE.md`.

- **Vocabulaire** : plan (et non scène), clé / clé de caméra / clé d'effet (et non « image clé »),
  choisir (calques, images, clés, plans) et sélectionner (éléments d'un dessin), Réglages (et non
  options), Supprimer un objet / Retirer une propriété / Effacer du dessin, pelure d'oignon, aides au
  cadrage.
- **Clés de dessins, de caméra, d'effets** : le même menu du clic droit (Copier, Coller, Coller à la tête de
  lecture, Supprimer, Choisir toutes les clés), le même choix (clic, Ctrl + clic, Maj + clic, rectangle :
  Maj ajoute, Ctrl bascule), Ctrl+A pour toutes celles de la sorte qui a la main.
- **Plusieurs clés de plusieurs calques** (montage) : le rectangle couvre toutes les rangées qu'il touche ; Maj
  + clic prend le rectangle entre la clé de départ et celle cliquée ; glisser les emmène toutes, calque par
  calque ; Ctrl+V garde chaque clé sur son calque **relativement au plus haut calque copié** (celui-ci sur le
  calque courant, les autres dessous, calques créés au besoin) ; la barre d'état dit « 12 clés sur 3
  calques ».
- **Ctrl+Maj + outil Sélection** (lasso, rectangle ou clic sur le dessin) : prend sur **tous** les calques
  du plan à l'image courante (visibles, déverrouillés, non vidéo), comme dans Storyboard Pro ; leurs clés se
  choisissent aussi dans le montage. Ctrl seul et Maj seul gardent leur sens. **Depuis le 2026-10-20, Ctrl+Maj
  cumule** : un objet s'ajoute à la sélection (ou en est retiré s'il y était), un lasso ajoute ce qu'il prend
  (avant : la sélection était remplacée).
- **Zoom de la bande** (montage et timeline) : le curseur, les boutons, Alt+1 / Alt+2 et Ctrl + molette
  gardent la tête de lecture au même endroit de l'écran ; hors de la vue, elle vient d'abord au milieu.
- **Icônes allumées** : une icône de bouton allumé (boucle, outil actif, aimant, vue…) prend la couleur
  lisible sur l'accent, dans tous les thèmes.

## Projets récents

Fichier > Projets récents : les dix derniers projets ouverts ou enregistrés. Au lancement, SHOT
rouvre le dernier — et, si plusieurs onglets étaient ouverts à la fermeture, tous (voir « Onglets de projets »),
l'onglet affiché à la fin en dernier — sauf si tu en ouvres un autre, ou si une séance interrompue attend
d'être récupérée ; une case du même menu le désactive.

Sans projet ouvert (au lancement, ou après « Fermer le projet »), un écran d'accueil propose :
- un nouveau projet ;
- d'ouvrir un projet Pelure ou Storyboard Pro ;
- les projets récents.

Il s'en va dès qu'un projet s'ouvre ou se crée, au premier trait, ou par « Commencer sans projet ».

### Nouveau projet

« Nouveau projet… » (l'accueil, ou Fichier > Nouveau, Ctrl+N) ouvre une fenêtre :
- **Nom** et **dossier** : avec un nom, le projet est créé et enregistré tout de suite dans
  `dossier\nom\` ; sans nom, « Enregistrer » demandera où le ranger plus tard.
- **Préréglage**, groupé : Vidéo (HD 1080, HD 720, UHD 4K, 2K DCI, Scope 2,39, SD PAL 4:3 en pixels
  carrés), Réseaux (vertical 9:16, carré 1:1, portrait 4:5) et Papier (A4, A3, A5, US Letter,
  en paysage, calculés avec la résolution). Chacun porte sa cadence (24 i/s, 25 pour PAL). Dès
  qu'une valeur change à la main, la liste affiche « Personnalisé ». La disquette garde le format
  actuel sous un nom, la corbeille retire un préréglage à toi (pas ceux d'origine).
- **Format** : largeur et hauteur en px, mm, cm ou in (le projet garde des pixels), orientation,
  proportions reconnues, résolution (dpi, pour le papier et les unités ; elle n'est pas gardée dans
  le projet) et cadence.
- **Papier** (couleur, ou transparent), couleur des fonds sous le trait, durée du premier plan, et un
  aperçu du cadre.

Entrée valide, Échap annule. La fenêtre repart des dernières valeurs, du dernier dossier et de tes
préréglages, retenus dans `%APPDATA%\SHOT\reglages.toml`.

## Onglets de projets

Plusieurs projets ouverts dans la même fenêtre (demande d'Anthony, 2026-10-20). Une bande d'onglets sous la barre de menus : un
onglet par projet, avec son nom, une pastille « • » orange s'il n'est pas enregistré et une croix. Chaque onglet garde **son**
annulation, sa tête de lecture, sa sélection, sa vue (zoom, mode storyboard / animation) et ses caches ; passer de l'un à
l'autre est instantané (aucune relecture du disque : 7 à 14 µs, l'image d'écran suivante ne coûte pas plus qu'une autre).

- **Ouvrir** : Fichier > Ouvrir, Nouveau, un projet récent, un import Storyboard Pro ou de scénario, « nouveau projet à partir
  des plans choisis », ou un `.pelure` lâché sur la fenêtre prennent un **nouvel onglet** (Préférences > « Ouvrir dans un nouvel
  onglet », allumé ; éteint, ils remplacent le projet affiché après la question habituelle). Un projet déjà ouvert : son onglet
  passe à l'écran. L'accueil (un onglet « Nouvel onglet ») est réutilisé par le premier projet qu'on y ouvre.
- **Gestes** : clic = passer à l'onglet ; clic du milieu ou croix = fermer ; clic droit = Fermer, Fermer les autres, Afficher dans
  l'Explorateur ; glisser = déplacer ; « + » ou **Ctrl+T** = un onglet vierge. **Ctrl+W** ferme l'onglet affiché (le dernier est
  remplacé par un projet vide et l'accueil revient) ; **Ctrl+Tab** / **Ctrl+Maj+Tab** passent à l'onglet suivant / précédent.
  `Tab` et `Maj+Tab` gardent leur sens (storyboard / animation, planche).
- **Fermer un onglet modifié** : la boîte « Modifications non enregistrées » nomme le projet (Enregistrer / Ne pas enregistrer /
  Annuler). **Quitter** (ou « Fermer les autres ») avec plusieurs onglets modifiés : une seule boîte les liste.
- **Enregistrer** : chaque onglet s'écrit sur son fil ; on peut changer d'onglet pendant l'écriture, son résultat revient à son
  onglet (la pastille s'éteint, la barre d'état dit de quel projet elle parle). Une écriture n'est jamais coupée : fermer l'onglet
  ou quitter l'attend.
- **Copie de secours** : une par onglet (un dossier de séance chacun) ; après un plantage, la fenêtre de récupération liste chaque
  projet (« Tout ouvrir » : chacun dans son onglet).
- **Copier-coller** : plans, calques, images, clés de caméra / de montage / d'effets, effets d'un calque, zones bitmap et éléments
  vectoriels se collent d'un onglet à l'autre, avec leurs propres dessins. Une feuille d'une autre taille refuse (message avec les
  deux tailles) ; une autre cadence prévient (la durée des plans change).
- **Pendant un travail qui parle à l'interface** (export, propagation, coloriage, génération des sons proposés) on ne change pas
  d'onglet : la barre d'état dit pourquoi.
- **Au repos** : trois onglets ouverts ne demandent aucune image de plus, le processeur reste à zéro ; un onglet caché n'a pas de
  vignette en cours, pas de lecture, pas de calcul sur le fil de l'interface. Mémoire : environ 54 Mo de plus par projet de
  30 plans × 3 calques × 24 images (mesure de `docs/PERFORMANCE.md`, section 15).

Détails : `docs/ARCHITECTURE.md` (« Onglets de projets »), `docs/INTERFACE.md` partie 23 (vérifications à l'écran).
## Importer un scénario

Fichier > Importer > « Un scénario… » (action `fichier.importer_scenario`, donc aussi dans Ctrl+K) : on
choisit un fichier, SHOT le lit sur un autre fil (barre d'avancement), puis une fenêtre montre ce qui va
être créé avant que rien ne soit touché. Exemple prêt à essayer : `docs/exemples/Le-Reveil.fountain`.

**Formats lus** : Fountain (`.fountain`, `.spmd` : intitulés INT./EXT., personnages en majuscules,
parenthèses, V.O., transitions, dialogues simultanés `^`, notes `[[ ]]`, cimetière `/* */`, sections `#`,
synopsis `=`, centré `> <`, éléments forcés `.` `!` `@` `>` `~`, page de titre) ; texte brut (`.txt` et
le reste : mêmes repères déduits de la forme, « NOM : réplique », indentation, numéros de page et
« (SUITE) » d'un texte copié d'un PDF retirés) ; Final Draft (`.fdx`) ; Word (`.docx`, avec les styles
d'un logiciel de scénario si le document en a, sinon un paragraphe = une ligne). **Pas lus** : le PDF
(aucun lecteur de PDF hors connexion dans SHOT ; le message propose d'exporter en `.fountain` ou `.txt`
ou de copier le texte dans un `.txt`), l'ancien `.doc`, le `.rtf`, l'`.odt`.

**Ce qui est créé** :
- chaque **intitulé de scène** (« INT. CUISINE – NUIT ») devient une **séquence** du même nom (« Ouverture »
  pour ce qui précède le premier intitulé) ; ses **plans** viennent de l'action et des répliques ;
- **découpage** au choix : *un plan par scène* (les paragraphes et les répliques en sont les images de
  storyboard, avec leurs textes), *un plan par paragraphe* (par défaut : un plan par paragraphe d'action et un
  par échange, les répliques qui se suivent), *un plan par réplique* ;
- **action** → champ Action ; **répliques** → champ Dialogue, une ligne chacune, `MARIE (V.O.) (bas) : Viens
  ici.` (deux personnages qui parlent ensemble : deux lignes du même plan) ; **notes** → champ Notes : notes
  `[[ ]]` (« Note : … »), transitions (« Transition : CUT TO: »), synopsis, sections, et le décor
  (« Décor : INT. CUISINE – NUIT », case à décocher) sur le premier plan de la séquence. Ce sont les
  conventions du lecteur de sons par règles (`NOM : réplique`, lignes `SFX :`) ;
- **durée** estimée du texte, réglable (« Rythme ») : répliques à 2,5 mots par seconde, action à 1 seconde par
  ligne de 60 signes, au plus court 1 s, au plus long 30 s ; un plan dure la somme de ses textes ;
- les **personnages** qui parlent, avec leur nombre de répliques, sont écrits dans les notes du projet
  (Réglages du projet > Notes), sous « Personnages du scénario : » (`- MARIE (24 répliques)`) : un autre
  import s'ajoute à la liste. Les voix provisoires la liront.

**Destination** : un nouveau projet (au format du dernier « Nouveau projet », titre et auteur de la page de
titre ; SHOT propose d'enregistrer le projet ouvert), ou le projet ouvert, à la fin ou après la séquence du
plan courant (une séquence ne se coupe pas). Dans le projet ouvert, l'import est **une seule étape
d'annulation**, « Importer un scénario » ; un projet encore vierge (un seul plan vide) est remplacé. Les
derniers choix sont gardés le temps de la séance.

## Fiabilité

Un filet de sécurité contre « tous les essais passent mais SHOT ne s'ouvre plus » (`docs/FIABILITE.md`) :

- un **projet d'essai complet** fabriqué par le code (séquences, plans, calques bitmap / vectoriel / vidéo / CTG /
  dossier, traits texturés avec leur trace, fond sous le trait, effets à clés, caméra et effet de caméra, repère 3D,
  marques, commentaires, plan rogné avec sa réserve, son…), enregistré et rouvert par trois chemins (une boucle,
  plusieurs fils, copie de secours) et par l'application elle-même : le projet relu est le projet enregistré ;
- un **essai de fumée** : 195 images de l'interface sans fenêtre (accueil, tous les panneaux, les outils, lecture,
  sélection et déplacement, effet de caméra, annuler / rétablir), sur la carte graphique et sur le processeur ;
- des **fichiers abîmés** (manifeste tronqué ou d'une version plus récente, dessin manquant ou aux valeurs absurdes,
  `commentaires.json` / `rognages.json` d'une version plus récente) : un message clair, jamais de plantage ni de
  blocage, aucun fichier effacé — y compris par l'enregistrement qui suit ;
- un **démarrage** minuté (`SHOT_DEMARRAGE=1` écrit `%LOCALAPPDATA%\SHOT\demarrage.log`) : les pinceaux importés et la
  présence du dernier projet ne retardent plus la première image ;
- `.\scripts\verifier.ps1 [-Repetitions N]` : tout cela, la suite complète (N fois, pour débusquer un essai
  intermittent) et le contrôle des avertissements, avec un résumé OK / ÉCHEC. Il ne lance pas `shot.exe`.

## Plantage

Si Pelure se ferme sur une erreur, un rapport est écrit dans `%LOCALAPPDATA%\Pelure\plantages\`, et la
barre d'état le signale au démarrage suivant (envoie-le pour qu'on trouve la cause). Le projet est
rattrapé par la copie de secours.

## Projet en dossier

« Enregistrer » crée un dossier au nom du projet :

```
Bus/
  Bus.pelure        manifeste lisible (réglages, séquences > plans > calques, marques d'image)
  dessins/12.png    un fichier par dessin bitmap
  dessins/13.json   un fichier par dessin vectoriel
  sons/14.mp3       les sons importés, copiés tels quels
  modeles/15.pelure3d  les modèles 3D de repère
  commentaires.json    les commentaires de la timeline (absent : aucun commentaire)
  rognages.json        le contenu rogné des plans (absent : rien de rogné)
  references.json      les références visuelles (absent : aucune)
  references/16.png    leurs images, copiées telles quelles (une vidéo : un dossier references/17/ d'images 000001.jpg…)
  crayonnes.json       la planche de crayonnés rapides (absent : pas de planche)
  scenes3d.json        les scènes 3D de blocage (absent : aucune scène)
  envois.json          les envois (« v2 au réal ») et l'empreinte de chaque plan à chacun (absent : aucun envoi)
  versions/            les instantanés des envois et leurs dessins, rangés par contenu (absent : aucun envoi)
  plans_versions.json  les autres versions de chaque plan (absent : aucun plan n'a de version)
```

Si tu enregistres « Bus.pelure » dans `Films`, le fichier est donc dans `Films\Bus\Bus.pelure`, pas
directement dans `Films` : la barre d'état le dit après l'enregistrement, et Fichier > « Afficher le
projet dans l'Explorateur » ouvre l'Explorateur sur lui. On ouvre le projet en choisissant
`Bus.pelure`. Un enregistrement n'écrit que les dessins modifiés,
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

**Plateau en 3D** (bouton 3D de la barre sous le dessin, ou Affichage > Plateau) : les calques du plan
sont des plans posés à leur profondeur, la caméra une pyramide de vision (avec sa course si elle
bouge), sur un sol quadrillé. Outils : tourner autour, glisser la vue, avancer / reculer ; partout,
molette = avancer, bouton du milieu = glisser, clic droit ou Alt + glisser = tourner. Outil
Déplacer : glisser un calque le déplace dans son plan, Maj + glisser change sa profondeur ; prendre
l'œil de la caméra la déplace (Maj : avancée ; Ctrl : la tourner, panoramique et bascule). Boutons Face, Dessus, Côté, « Par la caméra », et
recadrer. Pour dessiner, on revient au plateau à plat (bouton 3D).

**Scène 3D de blocage** (bouton « Scène 3D » de la barre sous le dessin, ou Affichage > Plateau) :
une vue 3D pour poser des formes simples et des mannequins, placer la caméra, puis envoyer le résultat dans un
calque de fond sur lequel dessiner. Les réglages sont dans le panneau Réglages.
- **Formes** : cube, plan (sol, mur), cylindre, sphère, cône, escalier (nombre de marches), **mannequin**
  (silhouette de capsules articulées : taille réglable, poses debout / assis / marche / bras levés, et chaque
  articulation en chiffres) ; ou un modèle glTF importé. Choisir, Déplacer, Tourner, Échelle : flèches, cercles et
  cubes par-dessus l'objet ; glisser un objet le pose ailleurs sur le sol (Maj : en hauteur). Aimantation à la
  grille, par paliers de rotation, et au sol. Champs numériques, couleur ou gris par objet, grouper, dupliquer,
  supprimer ; chaque geste s'annule (« Déplacer dans la scène 3D »…).
- **Un décor par séquence, une scène par plan** : le décor (sol, murs, escalier) se place une fois et tous les plans
  de la séquence le montrent ; chaque plan y met ses personnages et sa caméra. « Modifier : le décor » change le
  décor pour tous.
- **Caméra du plateau** : posée dans la scène avec sa pyramide de vision, déplaçable avec les mêmes poignées ; focale
  en mm et objectifs de la caméra du plan ; « Vue par la caméra » montre l'image au format du cadre. « Caler sur la
  caméra du plan » la place là où la caméra 2D est à l'image courante. Le plan z = 0 de la scène est la feuille : le
  cadre par défaut y mesure 6 m. Les clés de caméra ne sont pas dans la scène : la caméra 2D continue de bouger par-dessus.
- **Dessiner par-dessus** : « Envoyer vers un calque de fond » rend la scène par la caméra du plateau dans le calque
  « Fond 3D » (en bas, verrouillé), en couleurs, **niveaux de gris** ou **contours**. Il se met à jour tout seul,
  en arrière-plan, quand la scène change ; jamais pendant un trait. C'est un calque ordinaire.
- Enregistré dans `scenes3d.json` (le format du projet ne change pas). Détails : `docs/ROADMAP.md`, « Scène 3D de
  blocage ».

**Repère 3D** : exporte ton décor en glTF depuis Blender (Fichier > Exporter > glTF 2.0, `.glb`),
puis Fichier > « Importer un modèle 3D (glTF) comme repère… ». Il apparaît en fil de fer sur la
feuille, comme un guide pour dessiner (il ne s'exporte pas). Outil « Repère 3D » : glisser pour
tourner autour, Maj + glisser pour déplacer, Ctrl + glisser pour avancer ou reculer ; azimut,
hauteur, distance et focale se règlent aussi en chiffres. Chaque plan a son point de vue
(« Comme le plan précédent » le recopie). « Dessiner dans un calque » transforme le fil de fer
en calque bitmap, tout en bas, qui lui s'exporte. Le modèle est gardé dans le projet (`modeles/`).

**Propager une peinture sur une vidéo** (façon EbSynth) : on peint quelques images clés sur un
calque **bitmap** posé au-dessus d'un calque vidéo (ou d'une animation), puis clic droit sur ce
calque (montage, timeline ou panneau Calques) ou menu Calques > « Propager la peinture… ». Sur un
calque vectoriel (comme le calque « Dessin » d'un nouveau projet), l'entrée est grisée et sa bulle
dit pourquoi. Les autres images suivent le mouvement de la vidéo. Détails :
`docs/PROPAGATION.md`.

**Colorier par zones** (animation « cel » à aplats) : on dessine le trait sur un calque, on pose les
aplats de couleur dans les zones fermées d'un calque de couleur **sous le trait**, sur quelques clés
(les images où ce calque est peint) ; menu Animation > « Colorier par zones… » ou clic droit sur le
calque de trait (montage, timeline, Calques), ou Ctrl+K. SHOT retrouve les mêmes zones d'un dessin
au suivant (centre, aire, recouvrement, forme, voisines ; une zone peut se couper ou se réunir),
depuis la clé d'avant et celle d'après (la plus sûre l'emporte), et écrit de **vrais aplats** : des
formes pleines vectorielles sur un calque vectoriel, des pixels unis sur un calque bitmap, un peu sous
le trait, qui n'est jamais touché. Dans la fenêtre : le calque de trait, celui des clés, où écrire (le
même, un nouveau calque « Couleur » sous le trait, ou un autre), les clés (peintes, ou des images
tapées), la plage (tout le plan ou de telle à telle image), la fermeture des trous du trait, et ce qu'on
fait des zones incertaines (les laisser vides, ou les marquer en magenta « à vérifier »). Les zones
incertaines (zone nouvelle, appariement ambigu, clés qui se contredisent) sont listées, un clic va à
l'image. Un calcul en arrière-plan avec progression et « Arrêter », une seule étape d'annulation. La
méthode, ses limites et l'accroche d'un futur conseiller de couleur : `docs/COLORIAGE.md`.

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

**Projet Storyboard Pro** : Fichier > Ouvrir (le choix accepte les `.sboard`), « Ouvrir un projet
Storyboard Pro… », ou glisser le `.sboard` sur la fenêtre. Le fichier est seulement lu.
- Ce qui arrive tel quel : plans, séquences, panneaux (devenus les images de storyboard de chaque
  plan, avec leurs légendes), durées, cadence, taille, calques (vectoriels ou bitmap) et son. La
  caméra n'est pas reprise.
- Les dessins, si Storyboard Pro est installé : Pelure le lance tout seul, sans fenêtre, sur une
  copie du projet (l'original n'est jamais touché). Il exporte chaque calque de chaque panneau, en
  image et, pour un calque vectoriel, en traits ; l'ouverture suit.
- Un calque vectoriel revient vectoriel (zones et traits modifiables) quand ses traits ont pu être
  lus et recouvrent bien son image. Sinon il garde l'image, et le compte rendu le dit.
- « Arrêter » ouvre le projet sans les dessins.
- À la main : « Dessins à la main » écrit le script à coller dans Storyboard Pro (vue Script
  Editor, Sandbox, Run).

**Exporter vers Storyboard Pro** : Fichier > « Exporter vers Storyboard Pro… ».
- Choisir une fois le projet modèle : un projet Storyboard Pro vide, à tes réglages (résolution,
  cadence, légendes). Puis choisir le dossier et le titre, et « Exporter ».
- Pelure lance Storyboard Pro tout seul, sans fenêtre, sur une copie du modèle (le modèle n'est
  pas touché). Il y construit le projet, l'enregistre dans le dossier sous le titre, et rend un
  compte rendu.
- Chaque plan devient un plan. Chaque image de storyboard devient un panneau (durée, légendes).
  Chaque calque devient un calque : son dessin en image, son nom, sa visibilité, son opacité.
  Les séquences et le son suivent.
- Pas encore : les calques vectoriels en traits (ils arrivent en images), la caméra, l'animation
  dans un panneau (premier dessin seulement).

**Exporter pour le montage** : Fichier > Exporter… > « Pour le montage (EDL / XML / OTIO) ».
- L'animatique découpée plan par plan pour un monteur : un fichier vidéo par plan (ProRes ou H.264)
  dans `<Titre>_montage/media/`, et le montage en **EDL CMX 3600**, **Final Cut Pro 7 XML**
  (Premiere Pro, DaVinci Resolve) et **OpenTimelineIO**, avec les durées à l'image, un time code de
  départ (01:00:00:00), les fondus enchaînés (avec leurs poignées dans les vidéos) et les pistes son
  (un fichier WAV par extrait, XML et OTIO).
- Les clips portent les numéros des plans : `SQ01_PL012A` (voir « Numérotation verrouillée » plus
  haut : les noms ne bougent plus quand un plan s'ajoute ou se déplace).
- Cases : formats, dossier, titre, étendue (tout le film ou la séquence courante), time code de départ,
  NTSC (23,976 / 29,97 / 59,94), ProRes ou H.264, moitié de taille, son, chemins relatifs ou absolus.
  Les images se calculent par petits morceaux, ffmpeg et le son travaillent à côté ; « Arrêter » efface
  ce qui a été écrit. Un `LISEZ-MOI.txt` dit ce qui a été écrit et ce qui reste à vérifier.
- **Jamais ouvert dans Premiere Pro, DaVinci Resolve ni Avid** : les fichiers sont relus par les
  essais, leur import est à vérifier. Détail et liste de contrôle : `docs/EXPORT_MONTAGE.md`.

## Organisation du code

| Fichier | Rôle |
|---|---|
| `lib.rs`, `main.rs` | Bibliothèque (tous les modules, `lancer`) ; l'exécutable ne fait que la lancer |
| `noyau.rs` | Noyau pour l'appli compagnon : projet, résumé, vignettes PNG, panneaux, textes, PDF |
| `document.rs` | Séquences, scènes, panneaux, calques, expositions, dessins ; opérations de montage |
| `raster.rs` | Images en tuiles de 64×64 allouées à la demande |
| `vector.rs` | Éléments vectoriels : rendu (rejeu du moteur, cache de pixels en tuiles), sélection, transformations, gomme, contours de remplissage |
| `rejeu.rs` | Rendu exact des dessins vectoriels en arrière-plan (fil de travail, demandes numérotées) |
| `pixsel.rs`, `pixtools.rs` | Sélection bitmap : zones, pixels soulevés, transformations ; outil |
| `guides.rs` | Guides de perspective : géométrie, affichage, traits alignés |
| `brush.rs` | Moteur de pinceau : empreintes, pression, lissage, pointe en image, grain |
| `brushtex.rs`, `brushui.rs` | Pointes, papiers, réglages nommés ; leur panneau |
| `tbbrush.rs` | Import des pinceaux Toon Boom (XML) |
| `tracedroit.rs` | Tracé droit (Maj pendant un trait) : segment en attente, aperçu, pose par le moteur au relâché |
| `selobjet.rs`, `remplirsel.rs` | Choisir un objet au clic (forme d'un seul tenant, Ctrl / Ctrl+Maj, cumul) ; remplir la zone bitmap (Maj+Suppr) |
| `poskeys.rs`, `poskeysui.rs` | Clés de position des calques : pose entre les clés, adoucis, coupe et réunion des plans, enregistrement ; réglages de l'outil Transformer, losanges de la timeline |
| `onion.rs` | Pelure d'oignon |
| `onionui.rs` | Panneau de la pelure d'oignon (13f) |
| `bottomtabs.rs`, `notes.rs` | Onglets du bas de la timeline, notes dessinées (13e) |
| `projetui.rs` | Vue Projet : plan avant, storyboard verrouillé, propriétés, projet fait de plans (13e) |
| `xsheet.rs`, `xsheetui.rs` | Feuille d'exposition : ce que montre chaque case, le panneau (13g) |
| `roles.rs`, `rolesui.rs` | Rôles des images : modèle, suivi des images, symboles, menu (13g-2) |
| `flips.rs`, `flipsui.rs` | Flips : limites, checkpoints, pas, panneau et flip en cours (13h) |
| `fx.rs`, `fxui.rs` | Pile d'effets : clés, flous, dessins dérivés, panneau (13i) |
| `fxcache.rs`, `fxproxy.rs` | Effets : cache en octets, calcul de fond, dernier résultat en attendant ; aperçu réduit pendant la lecture et les gestes |
| `tremblement.rs` | Tremblement du trait des calques vectoriels : champ de déplacement lisse, variantes, repos (effet de la pile) ; aussi sur la carte graphique (`fs_boil`) |
| `fxtrait.rs` | Effets du trait des calques vectoriels (écriture, crayonné multiple, respiration, ondulation, pointillés, désagrégation) : axe des traits, rendu en pixels par trait, bandes de tuiles, préréglages, bulles |
| `fxmatiere.rs` | Effets de matière (liseré, trait sec, encre qui bave, dégradé le long du trait, changer de plume) : distance exacte au bord, bruit par hachage, plumes publiées par la bibliothèque de pinceaux, préréglages, bulles |
| `calqueffet.rs`, `fxcine.rs` | Calques d'effets (ajustement, incrustation) : repère du plan, dessin dérivé de ce qui est dessous, enregistrement ; effets « Cinéma » (flash, impact, lueur diffuse, secousse, vignettage, grain, aberration) |
| `prereglages.rs`, `prereglagesui.rs`, `incrustation.rs` | Préréglages d'effets (clés posées à la tête de lecture), fenêtre à aperçu en direct et menus ; texte des incrustations (timecode…) et son tracé |
| `ctg.rs`, `ctgui.rs` | Calque CTG : remplissage par graffitis, sources, structure (13j) |
| `calquecouleur.rs`, `calquecouleurui.rs` | Calque de couleur sous le trait : lien, miroir des images du trait, murs et zones fermées (pot de peinture, pinceau dans la zone), commandes et menu |
| `video.rs`, `videoui.rs` | Calque vidéo : import par ffmpeg, images lues à la demande (13k) |
| `tween.rs`, `tweenui.rs` | Interpolation : intervalles vectoriels, fondus, étirement, cadence (13l) |
| `xml.rs`, `sboard.rs` | Lecture d'un XML en arbre ; lecture d'un projet Storyboard Pro (14a) |
| `sbscript.rs`, `sbimport.rs`, `sbui.rs` | Script d'export des dessins et sa liste ; projet Pelure construit ; fenêtre d'ouverture (14a) |
| `sbbatch.rs`, `sbvector.rs` | Storyboard Pro lancé en batch sur une copie du projet ; ses dessins vectoriels refaits en éléments Pelure (14a-2) |
| `scenario.rs`, `scenariofdx.rs` | Importer un scénario : lecture de Fountain et de texte brut ; de Final Draft (`.fdx`) et de Word (`.docx`) |
| `scenarioplans.rs`, `scenarioui.rs` | Importer un scénario : séquences, plans, durées et personnages ; la fenêtre d'aperçu et l'étape d'annulation |
| `procreate.rs` | Import des brosses Procreate (`.brush`, `.brushset`) : listes de propriétés binaires d'Apple |
| `abr.rs` | Import des brosses Photoshop (`.abr`) |
| `kritabrush.rs`, `zipread.rs` | Import des réglages Krita (`.kpp`, `.bundle`) et MyPaint (`.myb`) ; lecture des archives zip |
| `dip.rs`, `tvpbrush.rs` | Images `.dip` de TVPaint ; import de ses brosses (`.tvpx`, `.bin`, `.ini`) |
| `banc.rs` | Banc de traits : enregistrer les traits tels que reçus, les rejouer dans le moteur |
| `dynui.rs` | Connexions d'un pinceau : sources, courbes de réponse (moteur de dessin, étape 2) |
| `sbexport.rs`, `sbexportui.rs` | Export vers Storyboard Pro : images des calques, sons, script de construction ; sa fenêtre (14b-1) |
| `stretch.rs` | Vitesse d'un son : rééchantillonnage, tempo sans changer la hauteur (WSOLA) (14a) |
| `fill.rs` | Remplissage par zone et débord |
| `render.rs` | Composition sur processeur (export, vignettes, secours), image filmée par la caméra, tuiles du monde (toile infinie) |
| `monde.rs` | Toile infinie à l'écran : texture du monde hors de la feuille, composée par le processeur par petits morceaux |
| `gpu.rs` | Composition et affichage par la carte graphique, envoi des dessins par tuiles ; une texture par dessin à la taille de son contenu (pas de la feuille) et image composée gardée tant que rien ne change |
| `playback.rs` | Lecture fluide : dessins envoyés d'avance, cadence affichée |
| `export.rs`, `exportui.rs` | Exports : film (ffmpeg), GIF animé (ffmpeg, palette), images, son, PDF ; fenêtres de choix et de progression |
| `notes_calques_tests.rs` | Les notes d'Anthony du 2026-10-20 : menus Trait / Matière, dossiers du panneau Calques, fond et effets en boutons du calque, durées par version, GIF, cadre fin et passe-partout |
| `pdf.rs` | Écriture du PDF de storyboard : texte, images, mise en page |
| `settings.rs` | Réglages de l'application (emplacement de ffmpeg) |
| `camera.rs`, `camtools.rs` | Caméra du panneau : images clés, adoucis, avancée, focale ; outil Caméra, sa fenêtre, vue caméra |
| `clescommun.rs` | Ce que les clés de dessins, de caméra et d'effets ont en commun : menu du clic droit, règle de choix (clic, Ctrl, Maj, rectangle) |
| `zoombande.rs` | Zoom du montage et de la timeline calé sur la tête de lecture |
| `viewbar.rs`, `frames.rs` | Barre au-dessus du dessin (plateau / caméra / 3D) ; zones de sécurité, cadre de format |
| `stage3d.rs`, `stage3dui.rs` | Plateau en 3D : géométrie (plans, caméra, rayons) ; affichage, navigation, outil Déplacer |
| `scene3d.rs`, `scene3dmath.rs`, `scene3dmesh.rs` | Scène 3D de blocage : objets, mannequin, caméra du plateau, rangement par plan et par séquence ; maths (matrices, rayons, vue) ; maillages des formes et du mannequin |
| `scene3dgizmo.rs`, `scene3drender.rs`, `scene3dgpu.rs` | Manipulateurs et aimantation ; rendu par le processeur (envoi vers un calque, sur un autre fil) ; vue d'édition par la carte graphique (wgpu) |
| `scene3dstore.rs`, `scene3dui.rs`, `scene3dview.rs` | `scenes3d.json` (avec les protections des annexes) ; commandes, Réglages, calque « Fond 3D » ; la vue (souris, poignées, tracé) |
| `layertools.rs` | Outil Transformer (calque entier) |
| `selbox.rs`, `seltools.rs` | Boîte de sélection façon Storyboard Pro : parties sous le pointeur, gestes (échelle, rotation, inclinaison, pivot), valeurs chiffrées ; sa place dans l'application |
| `multisel.rs`, `multitools.rs` | Sélection sur plusieurs calques choisis : parts par calque, boîte commune sur la feuille, geste appliqué à chaque calque (`T⁻¹·A·T`), une feuille flottante par calque, une étape d'annulation ; souris, commandes, écran |
| `envelopetools.rs` | Outil Déformer : mode Enveloppe de Bézier (la perspective : `perstools.rs`) |
| `topview.rs` | Vue de dessus (profondeur des calques) |
| `model3d.rs`, `view3d.rs`, `ref3dtool.rs` | Repère 3D : lecture glTF, arêtes utiles ; caméra en orbite, fil de fer, arêtes cachées ; outil et affichage |
| `thumbs.rs` | Vignettes des plans (montage, planche) et miniatures des dessins (timeline, Calques) : composées sur des fils de fond, gardées par contenu, envoyées à la carte graphique par l'interface (voir `docs/PERFORMANCE.md`, section 11) |
| `perf.rs`, `perf_tests.rs` | Profileur intégré et superposition « Performance » (`vue.performance`) ; banc sans fenêtre (voir `docs/PERFORMANCE.md`) |
| `parallele.rs`, `ouverture.rs`, `enregistrement.rs` | Travail réparti sur les cœurs (`par_map`, plafond de fils) ; ouverture d'un projet sur un autre fil ; enregistrement sur un autre fil (cliché, suites qui attendent, échec, file d'attente) |
| `materiel.rs`, `diagbanc.rs`, `diagnostic.rs`, `diagnosticui.rs` | Diagnostic des performances (`aide.diagnostic`) : détection du matériel, banc de mesures sur un autre fil, profils et réglages (appliquer, revenir), fenêtre, proposition du premier lancement, section Performance des Préférences |
| `history.rs` | Annuler / rétablir |
| `project.rs` | Projet en dossier (format 11), enregistrement préparé puis écrit |
| `blend.rs` | Modes de fusion des calques (formules, composition d'un pixel) |
| `board.rs`, `boardui.rs` | Planche de la vue Vignettes : disposition, notes, gestes, menus |
| `recovery.rs`, `recoveryui.rs` | Copie de secours, récupération après plantage |
| `workspace.rs`, `workspaceui.rs` | Espaces de travail (Storyboard, Animation, Montage : `Espace`, une disposition retenue chacun), disposition retenue |
| `barres.rs`, `barres_tests.rs` | Barres de commandes minces et « Plus… » : une table de commandes par barre (commandes, montage, timeline), épingles retenues dans les réglages, passage automatique sous « Plus… » quand la barre est étroite |
| `dock.rs`, `dockui.rs` | Panneaux modulables : disposition (format de Pelure), contenu des onglets, fenêtres à part |
| `shapes.rs`, `shapetools.rs` | Formes : ligne (courbes), rectangle, ellipse, polyligne |
| `text.rs`, `texttools.rs` | Texte : polices du poste, mise en page, outil |
| `persp.rs`, `perstools.rs` | Déformer en perspective |
| `settings.rs` | Réglages de l'application (ffmpeg, pelure, copie de secours, dispositions, couleurs et nuanciers) |
| `io.rs` | Anciens fichiers `.pelure`, écriture d'images |
| `audio.rs` | Décodage des sons, formes d'onde, mixage et sortie audio |
| `keymap.rs` | Raccourcis : actions, lecture de `keymap.toml`, touche tapée, fichier personnel |
| `raccourcisui.rs` | Clic droit « Assigner un raccourci… », fenêtre d'assignation, liste des raccourcis |
| `propager.rs`, `propagerui.rs` | Propagation de la peinture sur une vidéo, façon EbSynth (voir `docs/PROPAGATION.md`) |
| `zones.rs` | Découpe d'un dessin au trait en zones fermées (cœur, trous fermés, sous le trait, voisines, forme), cache par version du dessin |
| `coloriage.rs`, `coloriageui.rs` | Colorier par zones : appariement des zones d'une image à l'autre, aplats vectoriels ou bitmap, fenêtre, tâche, liste à vérifier (voir `docs/COLORIAGE.md`) |
| `handles.rs` | Poignées de la timeline (marques, signets, entrée / sortie, expositions) et menus de la grille |
| `fxkeys.rs` | Clés d'effets dans le montage et la timeline : losanges, choix, déplacement, copier-coller, suppression |
| `fxcopie.rs` | Copier-coller des effets (pile) d'un calque à l'autre : presse-papiers, calques choisis, clés hors du plan, tremblement sur bitmap |
| `camkeys.rs` | Clés de caméra dans le montage et la timeline : choix, déplacement, copier-coller entre plans, suppression |
| `camfx.rs`, `camfxui.rs` | Effets de caméra en préréglages (tremblements, explosion, zooms, vertigo) et liste des focales |
| `cadrage.rs`, `cadrageui.rs` | Aides au cadrage (tiers, grille d'or, spirale, diagonales, croix) : géométrie, réglages, tracé |
| `commentaires.rs`, `commentairesui.rs` | Commentaires sur la timeline : modèle, fichier `commentaires.json`, positions ; fanions, bulle, menu, panneau |
| `references.rs`, `referencesui.rs` | Références visuelles : modèle, fichier `references.json` et dossier `references/`, vue du tableau, filtre, image d'une vidéo selon la tête de lecture, import, cache des images ; panneau (tableau, grille, liste, pipette, lecture) |
| `crayonnes.rs`, `crayonnesui.rs` | Crayonnés rapides : planche de cases (traits vectoriels, pinceau bitmap), appartenance et coupe des traits, ordre, pages, annulation, création des plans (mise à l'échelle), fichier `crayonnes.json` ; page, barre, gestes, numéros, légendes, fenêtre « Créer les plans » |
| `empreinte.rs`, `envois.rs`, `envoisui.rs` | Envois : empreintes de contenu stables d'une séance à l'autre (pixels tels que le PNG les garde) ; le modèle, `envois.json`, les instantanés de `versions/` écrits une fois par contenu, le calcul sur un fil ; marquer un envoi, le suivi des plans modifiés (relevé par image d'écran, copie par tranches, fil de fond), pastilles, navigation, fenêtres « Marquer un envoi » et « Envois » |
| `planvers.rs`, `planversui.rs` | Versions d'un plan (v1, v2…) : le modèle (nouvelle, passer, restaurer, supprimer), `plans_versions.json`, annulation ; la ligne « Version » de la fiche, le sous-menu du clic droit |
| `comparer.rs`, `comparerui.rs` | Comparer un plan avec lui-même (deux versions, ou aujourd'hui et un envoi) en lecture seule : composition sur un fil de fond, superposition colorée, différence ; la fenêtre (côte à côte, volet, fondu, superposition) |
| `rognage.rs`, `rognageui.rs` | Rogner les plans sans rien détruire : réserve du début et de la fin, ripple, roll, glisser, aimantation, durée tapée, fichier `rognages.json` ; bords à la souris, flèches, commandes, fenêtre « Durée… » |
| `app.rs` | État de l'application, actions, raccourcis |
| `vtools.rs` | Outils des calques vectoriels |
| `storyboard.rs` | Bande de montage, vue vignettes, fiche du panneau |
| `montagerows.rs` | Pistes du montage : caméra, calques, fond de scène, noms des pistes |
| `montagehead.rs` | Colonne de gauche du montage : largeur étirable, réglages des calques par rangée, menu de tout ce qui touche au calque |
| `toolbars.rs`, `colorui.rs`, `layersui.rs`, `brushui.rs` | Barres de commandes et d'outils, panneau Couleur, panneau Calques, case et menu d'un pinceau, import |
| `pinceaux.rs`, `pinceauxui.rs` | Panneau Pinceaux : dossiers, recherche, état retenu, catégories des réglages (logique) ; taille, onglets, liste et réglages (affichage) |
| `couleur.rs` | Couleurs : conversions RVB, TSV, TSL, Lab, code hexadécimal, couleurs récentes, réglages retenus |
| `roue.rs` | Roue chromatique : géométrie (anneau, carré, triangle) et maillages colorés |
| `nuancier.rs`, `nuancierui.rs` | Nuanciers personnels : modèle, formats GIMP (.gpl) et Photoshop (.aco) ; leur place dans le panneau Couleur |
| `soundtrack.rs` | Pistes son : import, lecture synchronisée, affichage, glisser des extraits |
| `transitions.rs`, `transitionsui.rs` | Transitions du montage : zones, mélange d'une image, fichier `transitions.json` ; nœud dans la bande, menus, commandes, aperçu |
| `audiofx.rs`, `audiofxui.rs` | Effets sonores : fondus, enchaîné, clés de volume, égaliseur, écho, solo, fichier `effets_son.json` ; poignées, ligne du volume, menus |
| `bruitages.rs`, `bruitagesui.rs` | Bibliothèque de bruitages : parcours des dossiers, recherche, durées, écoute ; panneau, glisser vers le montage |
| `indications.rs`, `lecturesb.rs`, `lecturerules.rs` | Sons proposés : la proposition et `sons_proposes.json` ; l'entrée de la lecture du storyboard ; la lecture par règles, sans modèle |
| `fournisseurs.rs`, `generation.rs`, `genreglages.rs` | Génération sonore : les trois rôles (traits), annulation et avancement, tâches sur des fils à part, réglages `[generation_son]` |
| `httplocal.rs`, `sidecar.rs`, `serveurlocal.rs`, `fournisseursapi.rs`, `clessecretes.rs`, `base64.rs` | Client HTTP de boucle locale et contrat du serveur local ; lancement, attente et arrêt du serveur Python (`sidecar/`) ; Claude, ElevenLabs, Stable Audio (requêtes prêtes, envoi à brancher) ; clés dans le Gestionnaire d'identifiants de Windows |
| `suggestionsui.rs`, `genprefsui.rs` | Panneau Suggestions sonores et ses fenêtres ; section « Génération sonore » des Préférences |
| `numerotation.rs`, `numerotationui.rs` | Numérotation des plans : suivie ou verrouillée (12, 12A, 13), renuméroter, noms `SQ01_PL012A` |
| `editorial.rs`, `editorialjob.rs`, `editorialui.rs` | Export pour le montage : le plan d'export, l'EDL CMX 3600, le Final Cut XML, l'OpenTimelineIO ; la tâche (vidéos par plan, son, annulation) ; la fenêtre |
| `canvas.rs`, `panels.rs`, `timeline.rs` | Interface |

`cargo test --release` lance les tests (pinceau, vectoriel, remplissage, séquences/scènes/panneaux,
annulation, projet en dossier, anciens fichiers, raccourcis, son, caméra, transformation,
profondeur, vue de dessus, lecture, repère 3D, formes, texte, perspective, couleur,
roue chromatique, nuanciers et leurs formats, panneaux modulables, calques de toute la scène,
planche, modes de fusion, cadres de composition, plateau en 3D).

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
