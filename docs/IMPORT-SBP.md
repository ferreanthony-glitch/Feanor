# Import Storyboard Pro dans SHOT (import 71, 2026-10-29, branche `import-sbp`)

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
- Palette : `TCSC` (RVBA) + `TCID` (nom UTF-16, identifiant u64).
- Bitmap : tuiles `TBBM` : zone utile `TBBA` (x, y, largeur, hauteur) et PNG ; toile de 3840×2160
  centrée sur 0.
- Unités : **576 unités de dessin par champ**, un champ = 1/9 de la hauteur du cadre, y vers le haut :
  vérifié en recalant des rendus sur les vignettes de SBP (`.sboard.thumbnails`).
- Non décodé : le sens exact de `TGCO` (matrice ? épaisseur ?), des zones sans couleur
  (`TGSD` de 13 octets : comptées et signalées), de `TGBM` (matrice du bitmap), de `TGTL`.

## 3. Traduction SBP → SHOT

| SBP | SHOT | État |
|---|---|---|
| Séquence (`sequenceName`, si `sequenceExists`) | Séquence | fait |
| Plan (scène de panneaux) | Plan | fait |
| **Panneau** | **une clé de dessin par calque** du Plan, à son image de départ, avec sa durée ; **une marque d'image** (case des Vignettes) par panneau | fait : `into_plans` puis `cles_de_case` (un dessin tenu d'un panneau à l'autre reçoit une copie : une clé propre) |
| Calques (`READ`), ordre (premier = devant), nom | Calques, du fond au premier plan | fait |
| Visibilité (`disabled`), verrou, opacité 0-100 | idem | fait |
| Mode de fusion | — | SBP n'en écrit pas (aucun attribut trouvé) |
| Calque vectoriel | Calque vectoriel : une zone remplie (pair-impair) par suite de zones de même couleur d'un calque plan ; couleur et opacité de la palette du dessin | fait |
| Calque bitmap | Calque bitmap : tuiles assemblées, 1 pixel de toile = 1 pixel de la feuille (la feuille de SHOT a aussi 2× le cadre) **(à vérifier : l'échelle)** | fait |
| Placement du calque (décalage en champs, échelle, rotation) | cuit dans les points (vecteur) ; décalage seul pour le bitmap | fait ; animé par courbe ou incliné : valeur fixe + remarque |
| Traits de crayon à épaisseur | — | aucun trouvé dans les projets d'Anthony ; zones sans couleur : comptées, signalées |
| Caméra (`PEG` « Camera… », chemin 3D de clés `lockedInTime`) | clés de caméra du Plan : unités du chemin = 0,366 champ (x), 0,274 champ (y) ; zoom = 12 / (12 − 0,52·z) ; calées à l'œil sur deux vignettes de SBP (plan 1 de PART_1), **à confirmer** sur d'autres plans ; la feuille de SHOT est 6 fois le cadre (les décors font ±25 champs) ; la vue suit la caméra à l'ouverture | fait ; interpolation linéaire (la courbe de vitesse, les rotations quaternion, le Z-pivot ne sont pas repris) |
| Son (pistes, extraits, volume, tempo, vitesse) | Pistes et extraits | fait (comme 14a) ; enveloppes et autres effets : signalés |
| Légendes (Dialog, Action Notes, autres) | dialogue, action, notes | fait (comme 14a) |
| Effets de calque (flou gaussien) | — | signalés (« non repris ») |
| Groupes (`PEG` « Group ») | — | les calques sont repris à plat |
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

Flou et autres effets ; groupes ; animation des calques par courbes ; rotations quaternion de la
caméra ; courbes de vitesse de la caméra ; zones sans couleur ; textures de crayon (la couleur
moyenne n'est pas lue : la couleur pleine de la palette est prise) ; échelle des bitmaps et unité de z
de la caméra à confirmer à l'écran ; légendes-images ; `.sbpz`. Tout cela est dit dans la fenêtre de
compte rendu de l'import (« remarques »).