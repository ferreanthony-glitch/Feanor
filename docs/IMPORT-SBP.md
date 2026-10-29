# Import Storyboard Pro dans SHOT (import 71, 2026-10-29, branches `import-sbp` puis `import-sbp-camera`)

SHOT lit un projet Storyboard Pro (SBP) **sans lancer Storyboard Pro** : le `.sboard` (XML) donne la
structure, la caméra, les calques et le son ; les dessins sont lus directement dans
`elements/<dossier>/<élément>-<dessin>.tvg`. Rien n'est écrit dans le dossier du projet. Le
format `.tvg` a été relevé en lisant les fichiers d'Anthony (interopérabilité ; aucun code, outil ni
bibliothèque de Toon Boom). Tout ce qui est déduit est marqué **(à vérifier)**.

## 1. Pourquoi les dessins ne s'affichaient pas

L'ancien import (14a-2) ne lisait pas les `.tvg` : il lançait Storyboard Pro en mode batch pour
exporter chaque calque en PNG, puis relisait ces images. Sans Storyboard Pro lancé (poste sans SBP,
`PELURE_SANS_STORYBOARD_PRO`, batch qui échoue ou qui n'exporte pas), les calques arrivaient **vides**
(« les dessins ne sont pas repris »). Ce n'était ni un problème de chemins ni d'échelle : les dessins
n'étaient simplement jamais décodés. Le `.tvg` est maintenant décodé par SHOT (`src/tvg.rs`) ; le
lancement de Storyboard Pro n'est plus fait à l'ouverture (la voie du script reste, à la main, dans
la fenêtre « Dessins à la main »).

## 2. Le format `.tvg` (relevé)

- Conteneur : `OTVGfull`, table `TTOC`, sections `balise mode taille données`. Sections `tUAA`, `tCAA`,
  `tLAA`, `tOAA` (arts Underlay, Color, Line, Overlay), `TLAB` (bitmap), `TPAL` (palette). Mode `UNCO`
  (brut) ou `ZLIB` (taille décompressée u64, puis flux zlib). Les blocs `CERT`/`SIGN` (signature) ne
  sont ni lus ni vérifiés.
- Blocs imbriqués : `balise(4) drapeau(1) longueur` ; la longueur a 1, 2 ou 3 octets selon le drapeau
  (1, 3, 7).
- Art : `TGLY` (calque plan) > `TGVS` (zone) > `TGSD` (attributs, dont `TGCO` : l'identifiant de
  couleur 64 bits à l'octet 49) + `TGBP` (nombre de points u32, masque « sur la courbe », puis des
  couples f32). Un point sur la courbe suivi de deux points hors courbe puis d'un point sur la courbe :
  Bézier cubique. Les zones sont des contours remplis (les pinceaux d'Anthony), pas des traits à
  épaisseur variable.
- **Zones sans couleur** (`TGSD` de 13 octets, sans `TGCO`) : toutes dans `tLAA`, entre les zones colorées.
  Les **fermées** sont les **trous** de la zone colorée qui les précède (le contour intérieur d'un anneau, l'étoile
  d'une pastille) : sans elles, un anneau devient un disque. Les **ouvertes** (une ou plusieurs Béziers, 4 à 40
  points aplanis) sont des lignes médianes sans épaisseur ni couleur ; elles n'apparaissent pas dans les vignettes
  de SBP aux endroits vérifiés : non reprises, comptées. PART_1 : 19 999 zones sans couleur, dont 3 942 fermées
  (reprises comme trous) et 16 057 ouvertes (tous les fichiers du dossier) ; l'import en compte 15 908 non reprises.
- Palette : `TCSC` (RVBA) + `TCID` (nom UTF-16, identifiant u64).
- Bitmap : tuiles `TBBM` (PNG RVBA 8 bits, 256 × 56) : zone utile `TBBA` (x, y, largeur, hauteur) ; toile de
  3840×2160 centrée sur 0.
- Unités : **576 unités de dessin par champ**, un champ = 1/9 de la hauteur du cadre, y vers le haut :
  vérifié en recalant des rendus sur les vignettes de SBP (`.sboard.thumbnails`).
- Non décodé : le sens exact de `TGCO` (matrice ? épaisseur ?), de `TGBM` (matrice du bitmap), de `TGTL`.

## 3. Traduction SBP → SHOT

| SBP | SHOT | État |
|---|---|---|
| Séquence (`sequenceName`, si `sequenceExists`) | Séquence | fait |
| Plan (scène de panneaux) | Plan | fait |
| **Panneau** | **une clé de dessin par calque** du Plan, à son image de départ, avec sa durée ; **une marque d'image** (case des Vignettes) par panneau | fait : `into_plans` puis `cles_de_case` (un dessin tenu d'un panneau à l'autre reçoit une copie : une clé propre) |
| Calques (`READ`), ordre (premier = devant), nom | Calques, du fond au premier plan | fait |
| Visibilité (`disabled`), verrou, opacité 0-100 | idem | fait |
| Mode de fusion | — | SBP n'en écrit pas (aucun attribut trouvé) |
| Calque vectoriel | Calque vectoriel : une zone remplie (pair-impair) par suite de zones de même couleur d'un calque plan ; trous = contours fermés sans couleur ; couleur et opacité de la palette du dessin | fait |
| Calque bitmap | Calque bitmap : tuiles assemblées, 1 pixel de toile = 1 pixel de la feuille ; ce qui dépasse la feuille va dans les tuiles hors-feuille (toile infinie) | fait ; **couleurs à revoir** (voir §5) |
| Placement du calque (décalage en champs, échelle, rotation, **z**) | x, y, échelle, rotation cuits dans les points (vecteur) ; décalage seul pour le bitmap ; **z** = profondeur multiplan SHOT : `depth = −z × 1,1 / 12`, dessin rapetissé de `1 / (1 + depth)` (SBP ne grossit pas un calque lointain) ; deux panneaux où un même calque a des z différents donnent deux calques | fait ; animé par courbe : valeur fixe + remarque |
| Zones sans couleur | fermées : trous ; ouvertes : non reprises, comptées | fait / signalé |
| Caméra (`PEG` « Camera… » : chemin 3D de position, chemin de rotation, courbe de vitesse) | clés de caméra du Plan. **Sténopé à 12 champs du plan z = 0** (`CAMERA` : z = 12, FOV vertical 41,11° = 9 champs de haut). x, y en champs : x × 0,366, y × 0,2745 ; z de la caméra × 0,45 champ de recul = `dolly` négatif de SHOT (zoom 1). Rotation du chemin (degrés) : x = bascule (`tilt`), y = panoramique (`pan`, signe changé), z = roulis (`angle`, signe changé). Adoucis d'entrée / de sortie = poignées plates de la courbe de vitesse (`rx − x`, `x − lx`) | fait, **calé sur les vignettes** (§6) ; non repris : poignées non plates (Bézier libre), chemin courbe entre clés, `scale` du peg de la caméra |
| Groupes (`PEG` parent d'un calque dans `linkedlist`, `options/disabled`) | Dossier de calques SHOT (nom ; désactivé = dossier caché) au-dessus de ses calques ; un seul niveau (règle de SHOT) | fait |
| Flou gaussien (`GAUSSIANBLUR-PLUGIN`, `blurriness`) | Effet « Flou gaussien » du calque : rayon = valeur × 10 px (calé à l'œil, **à confirmer**), première valeur de la courbe | fait ; autres effets : signalés (« non repris ») |
| Son (pistes, extraits, volume, tempo, vitesse) | Pistes et extraits | fait (comme 14a) ; enveloppes et autres effets : signalés |
| Légendes (Dialog, Action Notes, autres) | dialogue, action, notes | fait (comme 14a) |
| Transitions | — | aucune trouvée dans les projets d'Anthony |
| Durée, cadence | durée et `framerate` | égales à SBP (vérifié) |
| `.sbpz`, `files.vdb` | — | non pris en charge |

## 4. Vérifié sur les trois projets (copies)

| Projet | plans | panneaux = cases | images SBP = images SHOT | cases avec dessin visible | plans avec caméra | pistes / extraits son |
|---|---|---|---|---|---|---|
| BRAWLIDAYS_CLEAN_PART_1 | 8 | 31 | 526 = 526 | 31 / 31 | 7 | 8 / 12 |
| Brawlidays_PART2_extract_v02 | 27 | 194 | 1529 = 1529 | 193 / 194 | 19 | 8 / 0 (pas de dossier audio) |
| BRAWLIDAYS_animatic PART2 | 37 | 339 | 2586 = 2586 | 339 / 339 | 27 | 8 / 19 |

Temps (build release) : 7 s, 13 s, 123 s (1 Go) ; l'import tourne sur un autre fil avec sa barre de
progression. Essai : `SBP_ESSAI=<copie> cargo test --release --offline --lib sbnative::tests::projet_reel -- --ignored --nocapture`.

## 5. Ce qui reste partiel ou impossible

- **Bitmaps de PART_1 plan 003 (`BG2`, `BG3`)** : les couleurs décodées ne correspondent pas aux vignettes (arc jaune au lieu d'un halo
  blanc et d'une vignette sombre). Sur ces deux dessins, le canal « bleu » du PNG semble porter l'opacité et « rouge = vert » le gris :
  à confirmer sur d'autres bitmaps avant de corriger.
- Effets autres que le flou gaussien ; courbes de vitesse non plates ; chemins courbes entre clés ; `scale` du peg de la caméra.
- Animation des calques par courbe : opacité (2 calques dans l'animatic, 0 dans PART_1 ; SHOT n'a pas de clés d'opacité de calque, donc
  impossible aujourd'hui) ; décalage, rotation, z (2 calques dans l'animatic ; reprenables plus tard par les clés de position).
- Lignes médianes sans couleur ouvertes : non reprises.
- Flou : la profondeur de champ des vignettes est plus forte que le rayon repris ; l'unité (10 px par unité) est à confirmer.
- Papier : les vignettes de SBP ont un papier gris très clair, SHOT garde le blanc du projet.
- Textures de crayon (couleur pleine de la palette prise) ; légendes-images ; `.sbpz`.
- Tout cela est dit dans la fenêtre de compte rendu de l'import (« remarques »).

## 6. La vue de dessin blanche, et le calage de la caméra (2026-10-29)

**La vue blanche.** La feuille agrandie à 6 fois le cadre (11 520 × 6 480 px) dépassait la texture maximale de la carte graphique
(8 192) : le journal d'erreurs disait « Dimension X value 11520 exceeds the limit of 8192 » pour la texture « image composée », la vue
restait blanche, alors que la miniature, calculée sur le processeur, était juste. **Correction** : la feuille garde la taille du projet
(deux fois le cadre) ; ce qui la dépasse (les décors vont jusqu'à ±24 champs) va sur la **toile infinie** (tuiles hors-feuille des dessins
vectoriels et bitmap). **Garde générale** : `Document::enlarge_stage` ne dépasse jamais `MAX_SHEET_SIDE` (8 192) ; une feuille plus
grande que la texture maximale de la carte (projet ancien ou abîmé) n'est plus montrée en blanc en silence : le dessin dit la raison
(message sur fond sombre, et barre d'état). Un affichage par tuiles n'a pas été nécessaire.

**Le modèle de caméra** : sténopé à 12 champs du plan z = 0 ; les calques ont leur z (`offset/z`) ; la caméra a x, y, z et des
rotations. Dans SHOT : `Layer::depth` et `Camera::dolly` (multiplan de SHOT), zoom 1. Unités calées par
`sbnative::tests::calage_descente` (ignoré), qui rend les images dont on a la vignette (`*.sboard.thumbnails`, extraites) et minimise l'écart de
luminance par blocs de 6 px : x = 0,366 champ par unité (identique sur PART_1 et l'animatic), y = 0,2745 (les trois quarts de x ; 0,22 à
0,26 selon les plans), z caméra = 0,45 champ (0,39 à 0,50), z calques = 1,1 champ (1,1 à 1,4, peu discriminant). Vérifié à l'écran et par les paires
vignette / rendu (plans 001 aux images 1, 50 et 66 ; 003 ; travellings 22, 30 et 8 de l'animatic) : même cadrage à quelques pour cent près.

Outils d'essai (ignorés) : `sbnative::tests::calage_vignettes` (paires vignette / rendu), `calage_descente`, `liste_cameras`, `liste_calques`,
`tvg::tests::sonde`, `sonde_dossier`, `sonde_bitmap`.
