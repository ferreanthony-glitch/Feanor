# Coloriage par zones

Demande d'Anthony (2026-10-19) : pour l'animation à aplats (« cel »), colorier quelques images clés et
laisser SHOT reporter les couleurs sur les autres images en retrouvant **les mêmes zones fermées**. Le
résultat reste de vrais aplats nets et modifiables sous le trait intact, jamais des pixels flous
générés. Ce n'est pas la propagation façon EbSynth (`docs/PROPAGATION.md`, `propager*.rs`,
`synthese/`), qui synthétise de la peinture sur une vidéo : les deux se complètent.

Fichiers : `src/zones.rs` (découpe), `src/coloriage.rs` (appariement, couleurs, aplats),
`src/coloriageui.rs` (commande, fenêtre, tâche, liste à vérifier). Essais : `zones_tests.rs`,
`coloriage_tests.rs`.

## Usage

Animation > « Colorier par zones… », ou clic droit sur le calque de trait (montage, timeline, panneau
Calques), ou Ctrl+K « Colorier par zones ». Action `calque.colorier_zones` (sans touche).

1. Un calque de **trait** (celui qu'on clique), un calque de **couleur** dessous où l'on pose des aplats
   dans les zones fermées, sur quelques **clés**.
2. Dans la fenêtre : le calque de trait ; le calque des clés (« Clés peintes sur ») ; les clés (les images
   où ce calque est peint, ou des images tapées : `1, 5-8, 12`) ; où écrire les aplats (dans le calque
   des clés, dans un nouveau calque « Couleur » vectoriel ou bitmap juste sous le trait, ou dans un autre
   calque) ; la plage (tout le plan, ou de telle à telle image) ; la fermeture des trous du trait ; le
   comportement des zones incertaines (les laisser sans couleur, ou les marquer en magenta) ; la
   confiance minimale.
3. « Colorier » lance la tâche sur un autre fil (progression, « Arrêter »). À la fin : une seule étape
   d'annulation, « Colorier par zones », et la liste des **zones à vérifier** (image, zone, raison) ; un
   clic va à l'image.

Les clés lues sont les images **marquées « Clé »** du calque des clés qui ont un dessin, sinon chaque
image où un dessin commence (comme la propagation). Après un coloriage, les clés sont marquées « Clé »
(retoucher une clé puis relancer suffit). Si les aplats vont dans un autre calque que les clés, les clés
y sont réécrites aussi (les clés bitmap deviennent des formes pleines vectorielles, par exemple).

## Méthode

### 1. Les zones (`zones.rs`)

Le trait est le dessin du calque de trait (pour un calque vectoriel, son rendu) : un pixel est du trait
si son opacité est au moins 96/255, comme pour l'aplat sous le trait et le pot de peinture. Un dessin
est découpé en zones fermées avec la même fermeture des trous que `fond.rs` (trous jusqu'à
`2 × trous` pixels, `trous` = 6 d'office) :

1. le **cœur** d'une zone est ce qui reste à plus de `trous` pixels du trait (transformée en distance
   exacte, `fond::edt`) ; les cœurs sont séparés par les trous fermés ; les îlots de moins de 8 pixels
   (deux pointes de cœur qui se frôlent au plus étroit d'un trou) sont ignorés ;
2. chaque cœur reprend son bord jusqu'au trait, par une file qui avance d'un pixel à la fois (deux
   cœurs se partagent un trou à égale distance) ;
3. une zone plus petite que la fermeture (une pupille, une dent) n'a pas de cœur : si elle est bien
   fermée, elle reste une zone à elle ;
4. chaque zone passe ensuite de `debord` pixels (3 d'office) **sous le trait**, pour qu'un aplat posé
   dessous ne laisse aucun jour.

Le **dehors** est la zone qui touche un côté de la boîte du trait qui n'est pas le bord de l'image ;
il n'est jamais colorié (et ne passe pas sous le trait). Le bord de l'image ferme les zones (un
personnage coupé par le cadre a son intérieur). Les zones de moins de 10 pixels sont du bruit.

Chaque zone a : numéro, aire, centre, boîte, périmètre, voisines (avec la longueur du bord commun, à
travers un trait de 14 pixels au plus), et deux invariants de Hu (identiques par déplacement, rotation,
changement d'échelle). La découpe est gardée par (dessin, version du dessin, réglages) dans un
`ZoneCache` borné à 384 Mo : relancer après avoir retouché des couleurs ne redécoupe rien.

### 2. Les couleurs des clés

`read_colours` : la couleur (RVB et opacité, exactes) qui couvre le plus de pixels de la zone, si elle
en couvre au moins un tiers ; sinon la zone est « sans couleur » (transparent = pas coloriée). Un
calque de couleur vectoriel est lu par son rendu.

### 3. L'appariement de deux images qui se suivent (`match_zones`)

- Le dessin entier a bougé d'un déplacement global `t` : celui qui superpose le mieux les
  silhouettes (départ au déplacement des centres, puis une recherche locale de plus en plus fine).
- Pour chaque zone d'après, les zones d'avant candidates : les huit plus proches de l'endroit attendu,
  quelques-unes proches d'un décor fixe (déplacement nul), et les trois qui lui ressemblent le plus par
  l'aire et la forme.
- Coût d'une paire : écart du centre au centre attendu (relatif à la taille), logarithme du rapport des
  aires, 1 − recouvrement (le recouvrement est mesuré après avoir glissé la zone d'avant), écart des
  invariants de Hu, cohérence des voisines (les voisines déjà appariées doivent l'être à des voisines de
  la zone). On essaie aussi la paire « alignée » (centres confondus), moyennant un écart au mouvement
  attendu, pour les déplacements rapides.
- Deux rondes : la seconde prévoit le mouvement de chaque zone d'après son appariement s'il est bon,
  sinon d'après celui de ses voisines.
- Algorithme **hongrois** (une zone peut rester seule, coût fixe) ; au-delà de 500 zones (somme des deux
  images), un choix glouton des meilleures paires.
- **Confiance** : fonction du coût (0 à 1), diminuée quand un autre choix était presque aussi bon (appariement
  « ambigu »).
- **Coupures** : une zone d'après sans pareille (ou à l'appariement peu sûr) qui était, pour
  l'essentiel (60 %), dans une zone d'avant, en reprend la couleur (confiance 0,55 à 0,65).
  **Réunions** : si des zones d'avant de couleurs différentes se sont réunies, la zone d'après est
  incertaine.

### 4. La propagation

De chaque clé vers l'avant, et de la clé suivante vers l'arrière ; avant la première clé et après la
dernière, un seul sens. La confiance d'une chaîne est celle de son maillon le plus faible. Quand les
deux clés parlent : même couleur → la confiance s'additionne ; couleurs différentes → la plus sûre
l'emporte (à égalité, la clé la plus proche), et la contradiction baisse la confiance. Une zone sans
pareille dans l'image d'avant est « nouvelle », sans couleur. Les images qui montrent le même dessin
de trait (exposition) n'en font qu'une et partagent leur dessin de couleur.

### 5. Les zones incertaines

Une zone sous la confiance minimale (0,5 d'office), ou nouvelle, n'est pas coloriée (« les laisser »)
ou prend le magenta (255, 0, 255) « à vérifier ». Elle est listée (plan, image, zone, raison) si elle
fait au moins 40 pixels. Raisons : nouvelle zone, appariement ambigu ou douteux, zone coupée, zones
réunies de couleurs différentes, clés qui se contredisent.

### 6. Les aplats

- **Calque bitmap** : chaque pixel de la zone, sous le trait compris, prend la couleur unie (aucun pixel
  de demi-teinte).
- **Calque vectoriel** : une forme pleine (`Element::Fill`, comme le pot de peinture et « Remplir la
  forme ») par zone, dont le contour (un par trou) est vectorisé et simplifié (`trace_contours`, 0,7 px)
  après l'avoir agrandie d'un pixel sous le trait et sur les zones voisines (pas de liseré entre deux
  aplats). Modifiable comme les autres formes.
- Le calque de trait n'est jamais modifié. Les nouveaux dessins sont ajoutés aux calques, en une
  étape d'annulation (instantané de la structure, comme la propagation).

## Le conseiller de couleur (pour plus tard)

`coloriage::ConseillerCouleur` : un avis sur la couleur d'une zone que l'appariement ne tranche pas
(zone nouvelle, confiance sous le seuil). Il reçoit une `Question` (rang de l'image, la découpe, la zone
et ses descripteurs, la couleur proposée et sa confiance) et peut rendre un `Conseil` (couleur,
confiance). S'il en rend un au-dessus de la confiance minimale, la zone prend cette couleur
(raison « avis du conseiller »). **Le résultat reste un aplat** de la couleur conseillée : un modèle
génératif ne fournirait qu'un indice. Aujourd'hui `SansConseil` ne dit rien ; `Coloriage::set_advisor`
(`coloriageui.rs`) branche un autre conseiller. Un essai (`the_colour_advisor_can_settle_a_new_zone`)
vérifie l'accroche.

## Limites

- La découpe lit les pixels du dessin de trait tel qu'il est dans le calque : ni la transformation du
  calque, ni ses effets (tremblement du trait…), ni la caméra. La fenêtre prévient si le calque de trait
  et celui des clés n'ont pas la même transformation. Les nouveaux calques « Couleur » copient celle du
  trait.
- Une zone qui fuit par un trou plus large que `2 × trous` fait partie du dehors : elle n'est pas
  coloriée (l'augmenter dans la fenêtre). Une petite zone ouverte par un trou se fond dans sa voisine.
- Le mouvement entre deux images successives doit rester raisonnable (de l'ordre de la taille des zones)
  ; pas de flux optique. Beaucoup de zones identiques qui échangent leurs places d'une image à
  l'autre sont indiscernables (la couleur reste à la place), le résultat est celui de la position.
- Une zone qui apparaît sans clé après elle reste sans couleur ; deux clés qui se contredisent font une
  zone à vérifier.
- Les zones se trouvent par fermeture : un trait qui se chevauche en laissant un jour très fin peut faire
  fuir une zone ; un trait épais (plus de `2 × débord`) laisse un fil non coloré au milieu, caché sous le
  trait.
- Les clés doivent être aux images où le calque de trait a un dessin ; une clé sans trait est ignorée.
- Pas d'aperçu avant de poser les aplats (l'annulation en tient lieu).

## Vitesse

Mesures (`cargo test --release --offline --target-dir target\msvc perf_coloriage -- --ignored
--nocapture`, Core i7-10700K, 16 fils), feuille 1920 × 1080, 50 zones (grille de rectangles) :

| Étape | Temps |
|---|---|
| découpe d'une image (un fil) | 74 ms |
| appariement de deux images (un fil) | 24 ms |
| aplats en pixels d'une image | 14 ms |
| aplats vectoriels : contours, puis rendu de la forme | 13 ms, 26 ms |
| 12 images, une clé (découpe et appariement en parallèle) | 0,49 s, soit 41 ms par image |
| 99 images, une clé, aplats bitmap | 3,5 s |

Sur le fil de l'interface : le relevé du plan (100 images) prend 0,5 ms (les dessins sont partagés à la
copie, rien n'est recopié) et le démarrage de la tâche 0,5 ms ; la fenêtre ne relève que les numéros
(sans les dessins) à chaque image d'affichage. Tout le calcul est sur un autre fil.
