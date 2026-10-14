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
  - « + Panneau » (P), « + Scène » (Maj+P), « + Séquence » ; le menu « Panneau… » duplique ou
    supprime un panneau ou une scène entière.
  - On réordonne les panneaux en les faisant glisser. Un panneau posé dans une autre scène la
    rejoint (à la frontière entre deux scènes : celle qui suit). Le bord droit d'un panneau règle
    sa durée.
  - Numérotation automatique : scènes 1, 2, 3… dans l'ordre du film, panneaux 1, 2… dans chaque
    scène. Une scène ou une séquence peut recevoir un nom ou un numéro à soi (fiche à droite).
  - Chaque panneau a sa durée et ses champs Action / Dialogue / Notes.
  - « Lire le montage » enchaîne tout (animatique), en boucle si on veut, avec le son.
  - Vue « Vignettes » (Maj+Tab) : les panneaux rangés par scène, avec leurs indications.
  - On dessine directement sur le panneau dans la vue « Dessin » : un nouveau dessin y est tenu
    sur toute la durée du panneau.
- **Son** : sous les panneaux, des pistes son.
  - « + Son… » (ou un fichier lâché sur la fenêtre) importe un WAV, MP3, FLAC ou OGG à l'image
    courante, sur la première piste libre.
  - Chaque extrait montre sa forme d'onde. On le déplace en le faisant glisser (aussi d'une piste à
    l'autre) ; ses bords cachent ou montrent le début ou la fin du son.
  - Clic droit : couper le son à cet endroit, supprimer l'extrait (ou Suppr), importer ici, régler
    la piste. Le bouton M d'une piste la rend muette.
  - La lecture suit le son : l'image reste synchronisée. En changeant d'image hors lecture, on
    entend une image de son (« son au défilement », menu Son).
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

## Le reste

- **Tablette** : la pression module la taille et/ou l'opacité. Elle passe par Windows Ink : sur une
  Wacom, l'option « Utiliser Windows Ink » doit être cochée dans les propriétés de la tablette.
- **Pelure d'oignon** : dessins précédents en rouge, suivants en vert.
- **Table lumineuse** (Outils > Vue, ou menu Affichage) : les autres calques du panneau sont
  estompés, le calque sur lequel on dessine reste net.
- **Guides de perspective** (Outils > Guides) : 1, 2 ou 3 points de fuite, que l'on déplace à la
  souris. G les affiche, Maj+A aligne les traits (le trait suit la fuyante, la verticale ou
  l'horizontale la plus proche de son départ), Maj+L verrouille les points, Maj+G montre des lignes
  de référence. Les guides sont enregistrés avec le projet.
- **Vue** : zoom à la molette (ou 1 / 2), rotation (Maj + molette, ou V / C), miroir (M),
  déplacement (Espace + glisser).
- **Annuler / rétablir** : 200 étapes, qui couvrent traits, vectoriel, panneaux, scènes, séquences et
  calques. Les textes (noms, action, dialogue…) ne s'annulent pas encore, mais une annulation ne les
  efface jamais.
- **Export** : image courante, séquence PNG du panneau ou de tout le montage, storyboard en page HTML
  imprimable (une planche par panneau, rangées par séquence ; à imprimer en PDF depuis le navigateur).

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
```

On ouvre le projet en choisissant `Bus.pelure`. Un enregistrement n'écrit que les dessins modifiés,
et chaque fichier est écrit à côté puis renommé : un plantage pendant l'enregistrement n'abîme pas la
version précédente. Les anciens fichiers `.pelure` (un seul fichier) s'ouvrent toujours : chaque
ancien « plan » devient une scène d'un panneau, et le premier « Enregistrer » crée le dossier.

## Organisation du code

| Fichier | Rôle |
|---|---|
| `document.rs` | Séquences, scènes, panneaux, calques, expositions, dessins ; opérations de montage |
| `raster.rs` | Images en tuiles de 64×64 allouées à la demande |
| `vector.rs` | Éléments vectoriels : rendu, sélection, transformations, gomme, contours de remplissage |
| `pixsel.rs`, `pixtools.rs` | Sélection bitmap : zones, pixels soulevés, transformations ; outil |
| `guides.rs` | Guides de perspective : géométrie, affichage, traits alignés |
| `brush.rs` | Moteur de pinceau : empreintes, pression, lissage |
| `fill.rs` | Remplissage par zone et débord |
| `render.rs` | Composition des calques et de la pelure, mise à jour partielle de l'affichage, vignettes |
| `thumbs.rs` | Cache des vignettes de panneaux |
| `history.rs` | Annuler / rétablir |
| `project.rs` | Projet en dossier (format 3) |
| `io.rs` | Anciens fichiers `.pelure`, écriture d'images |
| `audio.rs` | Décodage des sons, formes d'onde, mixage et sortie audio |
| `keymap.rs` | Raccourcis : actions, lecture de `keymap.toml` |
| `app.rs` | État de l'application, actions, raccourcis |
| `vtools.rs` | Outils des calques vectoriels |
| `storyboard.rs` | Bande de montage, vue vignettes, fiche du panneau |
| `soundtrack.rs` | Pistes son : import, lecture synchronisée, affichage, glisser des extraits |
| `canvas.rs`, `panels.rs`, `timeline.rs` | Interface |

`cargo test --release` lance les tests (pinceau, vectoriel, remplissage, séquences/scènes/panneaux,
annulation, projet en dossier, anciens fichiers, raccourcis, son).

## Particularités de compilation (Windows, toolchain GNU)

La toolchain `x86_64-pc-windows-gnu` installée par rustup n'embarque qu'un MinGW minimal :

- `Cargo.lock` fige `parking_lot` 0.12.3, `parking_lot_core` 0.9.10 et `libloading` 0.8.6. Les
  versions plus récentes passent par `windows-link` (« raw-dylib »), qui exige un `dlltool`
  complet absent de cette toolchain. **Ne pas lancer `cargo update` sans revérifier
  `cargo tree -i windows-link --target x86_64-pc-windows-gnu`.**
- `link/libshlwapi.a` (copiée de `winapi-x86_64-pc-windows-gnu`) fournit la bibliothèque d'import
  de `shlwapi`, demandée par une dépendance d'egui. `build.rs` l'ajoute au chemin d'édition de liens.

Le passage aux outils Microsoft (MSVC) est prévu avant la phase 2 et supprimera ces contournements.
