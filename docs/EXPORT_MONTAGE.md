# Export pour le montage (EDL, Final Cut XML, FCPXML, OpenTimelineIO) et échange (calques séparés, clés de caméra)

Demandé par Anthony le 2026-10-20 (branche `numerotation-export`) ; complété le 2026-10-30 (branche `exports-v1` : FCPXML, son dans
l'EDL, marqueurs, son NTSC sans dérive, calques séparés, clés de caméra : voir les deux dernières sections de ce document et
`docs/INTERFACE.md` §84). Le code : `src/editorial.rs` (ce qu'on écrit, sans
interface), `src/editorialjob.rs` (la tâche), `src/editorialui.rs` (la fenêtre), `src/editorial_tests.rs`.
Les numéros de plans qui nomment les clips : `src/numerotation.rs` (voir `docs/INTERFACE.md`, « Numérotation des plans »).

> **Rien de ce qui suit n'a été ouvert dans Premiere Pro, DaVinci Resolve ni Avid.** Les fichiers sont relus et vérifiés
> par les essais (structure, entrées et sorties à l'image, time codes, noms, chemins, pistes son), mais leur import dans un
> vrai logiciel de montage est **à vérifier par Anthony** (liste en fin de document). Ce document dit sur quoi on s'est
> appuyé pour chaque format ; ce qu'on ne sait pas, il le dit au lieu de le deviner.

## Lancer

Fichier > Exporter… > **Pour le montage (EDL / XML / OTIO)**, ou Ctrl+K « Exporter pour le montage », ou le bouton
« Exporter pour le montage » sous « Plus… » de la barre du montage (action `fichier.exporter_montage`).

| Réglage de la fenêtre | Effet |
|---|---|
| Fichiers de montage | EDL CMX 3600, Final Cut Pro 7 XML, Final Cut Pro X XML (FCPXML 1.9 ou 1.10), OpenTimelineIO : chacun se coche |
| Dossier, titre | l'export crée `<dossier>/<Titre>_montage/` ; s'il existe, les fichiers du même nom sont remplacés |
| Étendue | tout le film, ou la séquence courante (la ligne de temps commence alors à son premier plan) |
| Time code de départ | `01:00:00:00` d'office ; refusé s'il n'est pas lisible (`HH:MM:SS:FF`, FF < cadence) |
| NTSC | pour 24, 30 et 60 i/s : cadence réelle × 1000/1001 (23,976 / 29,97 / 59,94) |
| Vidéos | ProRes 422 HQ (`.mov`, d'office si ffmpeg sait l'encoder) ou H.264 (`.mp4`) ; à moitié de la taille |
| Son | les pistes son, un fichier par extrait (EDL, XML, FCPXML et OTIO) |
| Marqueurs | le numéro de chaque plan et les commentaires non résolus du projet (XML, FCPXML et OTIO) ; coché d'office |
| Chemins | relatifs au dossier (d'office), ou absolus (`file://localhost/…`) dans le XML et l'OTIO ; le FCPXML est toujours absolu |

## Ce qui est écrit

```text
Mon_film_montage/
  Mon_film.edl            EDL CMX 3600 (l'image, puis le son piste par piste)
  Mon_film.xml            Final Cut Pro 7 XML (xmeml 4)
  Mon_film.otio           OpenTimelineIO (JSON)
  Mon_film.fcpxml         Final Cut Pro X XML (1.9 ou 1.10), si on le coche
  media/SQ01_PL001.mov    un fichier vidéo par plan
  media/SQ01_PL002A.mov
  audio/A1_Dialogues_01.wav   un fichier son par extrait (stéréo, 48 kHz, 16 bits)
  LISEZ-MOI.txt           ce qui a été écrit, ce qui ne l'a pas été, ce qui reste à vérifier
```

### Les noms

Un clip se nomme `SQ<séquence>_PL<plan>` avec les **numéros de la numérotation des plans** : `SQ01_PL012A` = séquence 1, plan
12A (base sur 2 chiffres pour la séquence, 3 pour le plan, lettre ou suite en plus : `SQ01_PL002A`, `SQ01_PL001.5`).
En numérotation **verrouillée** ces noms ne bougent plus quand on insère, supprime ou déplace un plan (le plan inséré
entre le 12 et le 13 est le 12A) ; en numérotation **suivie** ils suivent le rang et la fenêtre le dit. Dans un nom de
fichier le point devient `_` (`SQ01_PL001_5.mov`) ; le nom du clip dans les fichiers de montage garde le point. Un nom à soi
donné à un plan (fiche) n'entre pas dans ces noms.

### Les vidéos

Une vidéo par plan, par `export::ExportJob::video` (le même chemin que l'export du film : calcul des images, une file bornée
de 8 images, ffmpeg en processus à côté) avec un ffmpeg par plan. Couleurs BT.709, tailles paires (comme le film). Sans
transitions dans les images : elles sont dans les fichiers de montage. En NTSC, ffmpeg reçoit `-framerate 24000/1001`
(`export::video_args_rate`). Les fichiers n'ont pas de time code incorporé : leur time code source commence à
`00:00:00:00`.

### Poignées

Dans SHOT une transition ne rallonge aucun plan : ce qui manque à un plan est **tenu** (`transitions.rs`). Pour qu'un monteur
retrouve la même image, la vidéo d'un plan qui participe à un fondu enchaîné contient des **poignées** : sa première image
répétée avant (le fondu en lit `cut - début` images) et sa dernière après (`fin - cut`). Le clip de la ligne de temps ne
montre que les images du plan (entrée = poignée avant, sortie = poignée avant + durée du plan) ; le fondu lit dans les
poignées. Sans fondu, aucune poignée. Fondu « centré » : la moitié avant la coupe, la moitié après ; « finit à la coupe » :
poignée avant seulement ; « commence à la coupe » : poignée après seulement.

### Les transitions

| Transition de SHOT | EDL | Final Cut XML | OTIO |
|---|---|---|---|
| Fondu enchaîné entre deux plans (centré, avant, après) | `D nnn`, événement à deux lignes | `transitionitem` « Cross Dissolve » (`start`, `end`, `alignment` center / end / start) | `Transition.1` `SMPTE_Dissolve` (`in_offset` = images avant la coupe, `out_offset` = après) |
| Fondu au noir, début ou fin du film | dissolve depuis / vers la bobine `BL` | coupe (dite) | coupe (dite) |
| Fondu au blanc, volet, noir ; fondu par le noir entre deux plans ; blanc au bord du film | coupe (dite) | coupe (dite) | coupe (dite) |

« Coupe (dite) » : le plan est mis bout à bout avec le suivant, et la transition est listée dans `LISEZ-MOI.txt`, en
commentaire de l'EDL, et dans la fenêtre. On n'a pas écrit les autres (fondu par le noir ou le blanc entre deux plans, volet) :
chaque logiciel les nomme à sa façon, et on n'invente pas ce que Premiere ou Resolve en ferait.

### Le son

Un fichier WAV par extrait, avec ce que SHOT entend de cet extrait : volume de l'extrait et de la piste, volume général,
fondus, clés de volume, égaliseur, écho (`audiofx::bake_sound`, sur un fil à part). Pas écrit : les fondus enchaînés **entre
deux extraits** (chaque extrait est écrit seul), les pistes muettes ou exclues par un solo (dites dans `LISEZ-MOI.txt`).
L'extrait coupé par l'étendue exportée (séquence courante) garde les fondus de l'extrait entier. L'EDL porte le son depuis le
2026-10-30 (voir « Le son dans l'EDL »).

Dans le XML, chaque piste de SHOT devient **deux** pistes (voie gauche, voie droite) qui pointent le même fichier stéréo ; dans
l'OTIO, une piste `Audio` par piste de SHOT, avec des `Gap.1` pour les vides.

### Temps, time code et NTSC

- Tout est compté en **images du projet** (cadence entière). Le time code de départ est ajouté à la ligne de temps ; time code
  **sans saut d'images** (NDF) : `HH:MM:SS:FF`, FF jusqu'à cadence - 1, à 24, 25, 30 et toute autre cadence entière.
- **NTSC** (option) : la cadence réelle est × 1000/1001. Les numéros d'images ne changent pas, seuls changent : `<ntsc>TRUE`
  du XML, la cadence de l'OTIO (`24000/1001` = 23,976…) et celle des vidéos. Le time code reste NDF. **Pas écrit** : le time
  code à sauts d'images (DF) du 29,97.
- **Son et NTSC, sans dérive** (2026-10-30) : en NTSC les images durent 1,001 fois plus longtemps, alors que leurs numéros sont ceux
  du projet ; un son écrit à sa durée exacte dériverait de 3,6 s par heure. Chaque fichier son est donc **ralenti de 0,1 %**
  (`editorial::slow_for_ntsc` : durée × 1001/1000, interpolation cubique, toujours écrit à 48 kHz ; la hauteur baisse de 1,7 centième
  de ton, comme le ralenti NTSC habituel). Un extrait de N images dure alors N × 1001/24000 s, comme ses N images : un essai le
  vérifie à l'échantillon près. Hors NTSC, rien ne change. À vérifier dans le logiciel : le son d'un film long reste-t-il calé sur
  l'image jusqu'à la fin ?
- L'EDL CMX 3600 est définie pour 24, 25 et 30 i/s ; pour une autre cadence, l'arithmétique est la même (FF jusqu'à
  cadence - 1) et un commentaire de l'EDL le dit.

### Le son dans l'EDL

Après les événements image, **un événement par extrait son**, piste par piste : un commentaire `* PISTE 1 : Voix`, puis les
extraits de la piste. Le canal est `A` pour la piste 1, `A2` pour la 2, puis `A3`… `A8` (une piste de SHOT est une paire
stéréo ; l'EDL n'a pas de vraies pistes, le logiciel range chaque canal sur une piste son). Source : toujours `00:00:00:00` à la
durée de l'extrait (le fichier son commence à zéro) ; enregistrement : sa place dans la ligne de temps ; bobine `A1_001`
(8 caractères au plus) ; `* FROM CLIP NAME: A1_Voix_01`. Chaque extrait est un fichier à part (`audio/`), effets compris.
**À vérifier** : un logiciel qui place tous les canaux `A` sur la même piste mélange-t-il les extraits qui se recouvrent ?

### Les marqueurs

Option « Marqueurs » (cochée d'office) : un marqueur au début de chaque clip, nommé comme le plan (`SQ01_PL012A`), et un marqueur
par **commentaire non résolu** du projet, à l'image du plan où il tombe (nom « Commentaire (Auteur) », texte du commentaire). Les
commentaires résolus ne sont pas écrits. Où :

- **XML (xmeml)** : `marker` {`name`, `comment`, `in` en images du fichier (poignée comprise), `out` -1} dans le `clipitem` ;
- **FCPXML** : `marker` {`start` en temps du média, `duration` une image, `value`, `note`} dans l'`asset-clip` ;
- **OTIO** : `Marker.2` {`name`, `color` GREEN pour un numéro de plan et RED pour un commentaire, `comment`, `marked_range`} dans
  le `markers` du clip ;
- **EDL** : pas de marqueurs (le format n'en a pas).

### Final Cut Pro X XML (FCPXML 1.9 ou 1.10)

Écrit du **même plan d'export** (`editorial::Plan`) que l'EDL, le XML et l'OTIO : mêmes clips, mêmes entrées et sorties, mêmes
poignées, mêmes fondus, même son. `fcpxml` > `resources` {`format` (taille, `frameDuration` `1/24s` ou `1001/24000s` en NTSC,
Rec. 709), un `asset` par plan (`hasVideo`), un `asset` par extrait son (`hasAudio`, 2 voies, 48 kHz), l'`effect` « Cross
Dissolve » s'il y a un fondu} > `library` > `event` > `project` > `sequence` {`duration`, `tcStart` = time code de départ, NDF} >
`spine` > un `asset-clip` par plan (`offset` = time code de départ + place, `start` = poignée avant, `duration`) et un `transition`
« Cross Dissolve » (`offset`, `duration`, `filter-video`) après le clip sortant. Les temps sont des fractions de seconde
(`N/Ds`). Le son est **accroché au clip image sous lequel il commence** (`asset-clip` avec `lane` négatif = le rang de la piste,
`audioRole` dialogue, `offset` en temps du média du clip parent). Les marqueurs sont dans le clip.

- 1.9 : l'adresse du média est l'attribut `src` de l'`asset` ; 1.10 : un élément `media-rep` (`kind` `original-media`).
- Les chemins sont **toujours absolus** (`file:///C:/…`, caractères protégés en `%XX`) : Final Cut Pro X ne retrouve pas un média par
  un chemin relatif. Si le dossier de l'export bouge, refaire l'export.
- Un essai relit le FCPXML avec un analyseur XML et le recoupe avec l'EDL (mêmes clips, mêmes entrées et sorties pour les coupes
  franches), avec le plan (fondu, son, marqueurs) et dans les deux versions et en NTSC.
- **Rien n'a été ouvert dans Final Cut Pro X.** À vérifier : l'import (un FCPXML dont une valeur déplaît est refusé en bloc), la
  place du son accroché, l'effet « Cross Dissolve » (identifiant `FxPlug:4731E73A-8DAC-4113-9A30-AE85B1761265`, écrit de mémoire),
  les marqueurs.

## Ce sur quoi on s'est appuyé, format par format

Écrit de mémoire des spécifications publiques ; rien n'a pu être recoupé avec un logiciel ici.

**EDL CMX 3600** (`write_edl`) : en-tête `TITLE:` et `FCM: NON-DROP FRAME` ; un événement par ligne :
`NNN  BOBINE   V     C        src_in src_out rec_in rec_out`, champs séparés par des blancs ; nom de bobine de 8 caractères au
plus (`PL012A`, `PL001_5`, ou `R00001` à défaut) ; ligne `* FROM CLIP NAME: SQ01_PL012A` après chaque événement.
Fondu enchaîné : deux lignes de **même numéro d'événement** : le plan sortant à longueur nulle (`C`, source et enregistrement
à l'image où le fondu commence), puis le plan entrant avec `D nnn` (`D    012`), son entrée en source reculée dans sa poignée.
Fondu depuis / vers le noir : la même chose avec la bobine `BL`.

```text
TITLE: Mon film
FCM: NON-DROP FRAME

001  PL001    V     C        00:00:00:00 00:00:00:18 01:00:00:00 01:00:00:18
* FROM CLIP NAME: SQ01_PL001
002  PL001    V     C        00:00:00:18 00:00:00:18 01:00:00:18 01:00:00:18
* FROM CLIP NAME: SQ01_PL001
002  PL002    V     D    012 00:00:00:00 00:00:00:18 01:00:00:18 01:00:01:12
* FROM CLIP NAME: SQ01_PL002
003  PL003    V     C        00:00:00:00 00:00:01:12 01:00:01:12 01:00:03:00
```

(Plan 1 de 24 images, plan 2 de 12 images, fondu centré de 12 images : il commence à l'image 18, six images avant la coupe.)

**Final Cut Pro 7 XML, xmeml version 4** (`write_xml`) : `xmeml` > `sequence` (`name`, `duration`, `rate` {`timebase`, `ntsc`},
`timecode` {`string`, `frame`, `displayformat` NDF}) > `media` > `video` > `format`/`samplecharacteristics`, `track` > `clipitem`
(`name`, `duration` du fichier, `start` / `end` dans la séquence, `in` / `out` dans le fichier, `file` {`name`, `pathurl`,
`duration`, `timecode`, `media`}, `sourcetrack`) et `transitionitem` (`start`, `end`, `alignment`, `effect` {`name`,
`effectid`, `effectcategory`, `effecttype`, `mediatype`}) entre les deux `clipitem` qu'il lie ; `audio` > `track` (une par voie)
> `clipitem` > `sourcetrack` {`mediatype` audio, `trackindex` 1 ou 2}. Le `pathurl` est relatif au dossier
(`media/SQ01_PL001.mov`) ou `file://localhost/C:/…` (caractères protégés en `%XX`).

**OpenTimelineIO** (`write_otio`) : JSON, schémas `Timeline.1` (`name`, `global_start_time` = time code de départ,
`tracks`), `Stack.1`, `Track.1` (`kind` Video / Audio), `Clip.1` (`source_range`, `media_reference`), `ExternalReference.1`
(`target_url`, `available_range`), `Gap.1`, `Transition.1` (`transition_type` `SMPTE_Dissolve`, `in_offset`, `out_offset`),
`TimeRange.1` et `RationalTime.1`. `Clip.1` est choisi parce que toutes les versions le lisent ; on ne sait pas que les versions
récentes le convertissent sans perte en `Clip.2` : à vérifier.

## Comment ça tourne (règle « sans latence »)

- Les **images** ne sont plus calculées sur le fil de l'interface (2026-10-20, branche `exports-fond`, `docs/PERFORMANCE.md` §17) :
  l'export copie le projet au départ (quelques images d'écran, pendant lesquelles il ne bouge pas : `EditorialWindow::rendering`),
  une seule fois pour tous les plans, puis calcule les images de chaque plan sur d'autres fils, comme pour l'export du film.
  Le projet est libre ensuite, et l'export continue si l'on change d'onglet. Il utilise le projet tel qu'il était au départ.
- ffmpeg (un processus par plan), l'écriture du **son** (un fil, avec son compteur) et l'écriture des fichiers sont à côté. Le
  son est écrit en même temps que les images.
- La fenêtre montre le plan en cours, une barre d'images (poignées comprises), une barre du son, le temps restant.
- **Arrêter** (ou fermer SHOT) arrête ffmpeg et le fil du son et **efface les fichiers déjà écrits par cet export** (et le
  dossier de l'export, s'il l'a créé).

## Ce qui a été vérifié (essais `editorial_tests.rs`)

- Time codes à 24, 25 et 30 i/s (lecture, écriture, refus de FF trop grand, départ choisi), NTSC (cadence réelle, `24000/1001`,
  drapeau XML, cadence OTIO, ffmpeg : `29.97 fps` lu dans le fichier produit), noms de fichiers sûrs.
- L'EDL relue : colonnes, numéros d'événements, bobines, noms de clips, entrées et sorties **à l'image** (source commence à
  zéro, enregistrement continu de 01:00:00:00 à la fin), fondu à deux lignes avec poignées, fondus depuis / vers `BL`, coupes
  dites en commentaire.
- Le XML relu par un analyseur XML : structure, `start` / `end` / `in` / `out` cohérents, durées de fichiers poignées
  comprises, `transitionitem` aux bonnes images et au bon alignement, caractères spéciaux du titre, chemins relatifs ou
  absolus, pistes son (deux voies par piste, fichier décrit une fois).
- L'OTIO relue par un analyseur JSON : schémas, `source_range`, `available_range`, `Transition.1` (offsets), `global_start_time`,
  pistes son avec leurs vides.
- Un export **réel** avec ffmpeg (ProRes, puis H.264 en 29,97 avec chemins absolus) : chaque fichier existe, chaque vidéo a
  exactement `poignée + plan + poignée` images, chaque son exactement la durée de son extrait, tous les chemins du XML mènent à
  un fichier ; annulation : plus aucun fichier, plus de dossier.

## À vérifier par Anthony

À faire dans les vrais logiciels, avec un projet de quelques plans, un fondu enchaîné et une piste son :

1. **Premiere Pro** : Fichier > Importer le `.xml` (ou l'EDL). Les clips sont-ils à la bonne place, à la bonne durée, avec leurs
   noms ? Le time code de départ ? Les médias sont-ils retrouvés par les chemins relatifs (sinon : réexporter avec « Chemins
   absolus ») ? Le fondu est-il centré sur la coupe, de 12 images ? Le son apparaît-il (deux pistes mono par piste de SHOT) ?
2. **DaVinci Resolve** : Fichier > Importer > Timeline… sur le `.xml`, puis sur le `.otio` et sur l'EDL. Mêmes questions ; les
   noms de clips de l'EDL (Resolve les lit en commentaire) ; l'OTIO s'ouvre-t-il (`Clip.1`) ?
3. **Avid** : l'EDL (bobines de 8 caractères). Le XML et l'OTIO ne sont pas visés ici.
4. Les **poignées** : en tirant le bord d'un clip dans le logiciel, voit-on l'image tenue ? Rien d'autre n'est caché derrière.
5. Le **NTSC** : la vidéo à 23,976 / 29,97 est-elle lue à la bonne cadence ? Le son dérive-t-il (voir plus haut) ?
6. Un fondu au noir au début du film : l'EDL le porte ; le XML et l'OTIO le laissent en coupe (dit).
7. Le **time code de départ** : `01:00:00:00` s'affiche-t-il comme la première image de la ligne de temps ?

## Limites

- L'export utilise le projet tel qu'il était au départ : ce qu'on retouche pendant qu'il tourne n'y est pas.
- Pas de time code à sauts d'images (29,97 DF), pas d'AAF, pas de fondus enchaînés entre extraits son.
- Transitions écrites : le fondu enchaîné entre deux plans (partout) et le fondu depuis / vers le noir (EDL). Le reste est une coupe, dite.
- Les plans sont rendus sans leurs transitions, mais avec tout le reste de l'export du film (incrustations gravées, calques
  d'effets, caméra, papier).
- Les contenus rognés (réserve de `rognage.rs`) ne sont pas lus pour les poignées : elles sont des images tenues.

## Calques séparés (pour un logiciel de compositing)

Fichier > Exporter… > **Calques séparés** (action `fichier.exporter_calques`). Pour l'étendue choisie (film, séquence, plan, plans
choisis) : **un dossier par plan**, nommé comme le plan (`SQ01_PL012A`), et dedans **une suite d'images PNG transparentes par
calque visible**, `01_Fond_0001.png`, `02_Personnage_0001.png`… (rang du calque de bas en haut, son nom, le numéro d'image du
plan sur 4 chiffres, depuis 1). Code : `src/exportlayers.rs`, `render::panel_layer_plans`.

- **Caméra appliquée** (d'office) : chaque image est ce que la caméra filme, à la taille de l'image finale, comme dans le film.
  Posées l'une sur l'autre dans l'ordre des numéros, en mode normal, les images d'un plan **redonnent l'image du film pixel pour
  pixel** (essai `separate_layers_recompose_the_mixed_image_pixel_for_pixel`, avec un calque transformé et à moitié transparent).
  **Sans caméra** (« feuille entière ») : la feuille entière de chaque calque, à la taille de la feuille.
- Un calque qui n'a rien à une image écrit une **image transparente** : la suite reste continue du début à la fin du plan.
- **Pas écrits** : les dossiers de calques, les calques cachés (eux ou leur dossier), les **calques d'effet** (un calque d'effet agit
  sur ce qui est dessous : il n'a pas d'image à lui, et son effet n'est donc pas dans les images des calques qu'il couvre ; la fenêtre
  dit combien ont été laissés de côté). Les calques d'annotation ne le sont que si « Avec les annotations » est coché. Les
  incrustations gravées (time code…) n'y sont pas.
- Un calque en mode de fusion autre que Normal est écrit tel qu'il se verrait seul sur du transparent ; le recomposer en Normal ne
  redonne pas le mode.
- Tâche de fond comme tous les exports (une copie du projet, des fils de rendu à priorité basse) ; annuler efface les fichiers
  écrits.

## Clés de caméra (JSON et CSV)

Fichier > Exporter… > **Caméra** (action `fichier.exporter_camera`). Pour chaque plan de l'étendue : les **clés** de la caméra et,
au choix, la caméra de **chaque image** (déjà interpolée : un script n'a pas à refaire les adoucis). On donne le nom du `.json` ;
les CSV s'écrivent à côté (`<nom>_cles.csv`, une ligne par clé ; `<nom>_images.csv`, une ligne par image). Code : `src/exportcamera.rs`.
Aucun script d'un autre logiciel n'est livré : voici le format, à relire par un script After Effects, Blender ou autre.

```json
{
  "format": "shot-camera", "version": 1, "projet": "Mon film", "cadence": 24,
  "cadre":   { "largeur": 1920, "hauteur": 1080 },
  "feuille": { "largeur": 3840, "hauteur": 2160 },
  "unites":  { "x": "...", "zoom": "...", "rotation": "...", "interpolation": "..." },
  "plans": [ {
    "plan": "SQ01_PL012A", "titre": "Séquence 1 — plan 12A",
    "image_debut": 0, "images": 24,
    "cles": [
      { "image": 0,  "x": 1920.0, "y": 1080.0, "zoom": 1.0, "rotation": 0.0, "avancee": 0.0, "bascule": 0.0, "panoramique": 0.0,
        "interpolation": "lineaire", "adouci_entree": 0.0, "adouci_sortie": 0.0, "adouci_ancien": false },
      { "image": 12, "x": 2020.0, "y": 1060.0, "zoom": 1.5, "rotation": 7.5, "avancee": 12.0, "bascule": -3.0, "panoramique": 4.0,
        "interpolation": "adoucie", "adouci_entree": 4.0, "adouci_sortie": 2.0, "adouci_ancien": false }
    ],
    "par_image": [ { "image": 0, "x": 1920.0, "y": 1080.0, "zoom": 1.0, "rotation": 0.0, "avancee": 0.0, "bascule": 0.0, "panoramique": 0.0 } ]
  } ]
}
```

| Champ | Sens |
|---|---|
| `x`, `y` | centre du cadre, en **pixels de la feuille** ; origine en haut à gauche de la feuille, `y` vers le bas |
| `zoom` | 1 : le cadre fait la taille de l'image finale ; 2 : deux fois plus serré (focale doublée) |
| `rotation` | degrés, sens horaire à l'écran |
| `avancee` | pixels, travelling avant (négatif : recul) |
| `bascule` | degrés ; positif : la caméra regarde vers le haut |
| `panoramique` | degrés ; positif : la caméra regarde vers la droite |
| `image` | numéro d'image **depuis 0, dans le plan** (`image_debut` : rang de la première image du plan dans le film) |
| `interpolation` | `lineaire` : vitesse constante jusqu'à la clé suivante ; `adoucie` : la caméra accélère et ralentit, sur `adouci_sortie` images après la clé et `adouci_entree` images avant la suivante (`adouci_ancien` : l'ancien réglage, un demi-intervalle de chaque côté) |

Un plan sans clé est filmé par la caméra du projet : `cles` est vide, `par_image` (si demandé) la donne. Les clés d'un plan sont
triées par image. Les nombres sont écrits avec un point décimal (le CSV aussi : les scripts, pas le tableur français). Le CSV des
clés a pour colonnes `plan,titre,image,x,y,zoom,rotation,avancee,bascule,panoramique,interpolation,adouci_entree,adouci_sortie` ;
celui des images `plan,image_du_plan,image_du_film,x,y,zoom,rotation,avancee,bascule,panoramique`.