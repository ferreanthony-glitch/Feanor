# Pelure

Logiciel d'animation 2D écrit en Rust avec egui : montage de plans façon storyboard,
animation image par image façon TVPaint, et dessin vectoriel modifiable façon Storyboard Pro.

## Lancer

```
cargo run --release
```

L'exécutable est ensuite dans `target\release\pelure.exe`. On peut lui passer un projet :
`pelure.exe mon_film.pelure` (et donc l'associer aux fichiers `.pelure` pour les ouvrir d'un double-clic).

## Deux niveaux : storyboard et animation

- **Storyboard** (au démarrage) : le film est une suite de plans. En bas, la bande de montage montre
  chaque plan avec sa vignette, sa largeur proportionnelle à sa durée.
  - Créer, dupliquer, supprimer un plan (P ou Inser, Ctrl+Suppr). On réordonne les plans en les faisant
    glisser dans la bande, et on règle la durée en tirant le bord droit d'un plan.
  - « Lire le montage » enchaîne tous les plans (animatique), en boucle si on veut.
  - Chaque plan a un titre, une durée, et des champs Action / Dialogue / Notes (panneau de droite).
  - Vue « Planches » (Maj+Tab) : la grille des plans avec leurs indications, comme une planche de storyboard.
  - On peut dessiner directement sur le plan dans la vue « Scène » : en storyboard, un nouveau dessin
    est tenu sur toute la durée du plan.
- **Animation** (Tab, ou double-clic sur un plan) : on entre dans le plan. On retrouve sa timeline
  avec calques, expositions, pelure d'oignon et lecture en boucle du plan seul. Tab ramène au storyboard.

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

Les traits vectoriels sont rendus à la résolution du projet : en zoomant très fort, on voit les pixels
(comme à l'export), mais le trait reste modifiable.

## Le reste

- **Tablette** : la pression module la taille et/ou l'opacité. Elle passe par Windows Ink : sur une
  Wacom, l'option « Utiliser Windows Ink » doit être cochée dans les propriétés de la tablette.
- **Pelure d'oignon** : dessins précédents en rouge, suivants en vert.
- **Vue** : zoom à la molette (ou 1 / 2), rotation (Maj + molette, ou V / C), miroir (M),
  déplacement (Espace + glisser).
- **Annuler / rétablir** : 200 étapes, qui couvrent traits, vectoriel, plans, montage et calques.
- **Export** : image courante, séquence PNG du plan ou de tout le montage, storyboard en page HTML
  imprimable (une planche par plan avec action, dialogue et notes ; à imprimer en PDF depuis le navigateur).

## Raccourcis clavier

Ils sont repris de Storyboard Pro 27 (raccourcis par défaut) et de tes réglages personnels, et
rangés dans `keymap.toml`. F1 les affiche tous, avec leur origine. Pour les changer sans recompiler :
« Aide > Modifier les raccourcis », puis « Aide > Recharger les raccourcis ». Détails et choix pour
le clavier AZERTY : `docs/reference/sbpro_presets/README.md`.

Documentation technique : `docs/ARCHITECTURE.md` ; avancement : `docs/ROADMAP.md`.

## Format `.pelure`

Un seul fichier : un en-tête, un manifeste JSON lisible (plans, calques, expositions, indications,
dessins vectoriels), puis les dessins bitmap encodés en PNG. Les projets de la première version
(un seul plan) s'ouvrent toujours. L'enregistrement passe par un fichier temporaire : un plantage
pendant l'écriture n'abîme pas la version précédente.

## Organisation du code

| Fichier | Rôle |
|---|---|
| `document.rs` | Projet, plans, calques, expositions, dessins |
| `raster.rs` | Images en tuiles de 64×64 allouées à la demande |
| `vector.rs` | Éléments vectoriels : rendu, sélection, transformations, gomme, contours de remplissage |
| `brush.rs` | Moteur de pinceau : empreintes, pression, lissage |
| `fill.rs` | Remplissage par zone et débord |
| `render.rs` | Composition des calques et de la pelure, mise à jour partielle de l'affichage, vignettes |
| `thumbs.rs` | Cache des vignettes de plans |
| `history.rs` | Annuler / rétablir |
| `io.rs` | Lecture / écriture `.pelure`, export PNG |
| `app.rs` | État de l'application, actions, raccourcis |
| `vtools.rs` | Outils des calques vectoriels |
| `storyboard.rs` | Bande de montage, vue planches, fiche du plan |
| `canvas.rs`, `panels.rs`, `timeline.rs` | Interface |

`cargo test --release` lance les tests (pinceau, vectoriel, remplissage, plans, annulation,
aller-retour fichier).

## Particularités de compilation (Windows, toolchain GNU)

La toolchain `x86_64-pc-windows-gnu` installée par rustup n'embarque qu'un MinGW minimal :

- `Cargo.lock` fige `parking_lot` 0.12.3, `parking_lot_core` 0.9.10 et `libloading` 0.8.6. Les
  versions plus récentes passent par `windows-link` (« raw-dylib »), qui exige un `dlltool`
  complet absent de cette toolchain. **Ne pas lancer `cargo update` sans revérifier
  `cargo tree -i windows-link --target x86_64-pc-windows-gnu`.**
- `link/libshlwapi.a` (copiée de `winapi-x86_64-pc-windows-gnu`) fournit la bibliothèque d'import
  de `shlwapi`, demandée par une dépendance d'egui. `build.rs` l'ajoute au chemin d'édition de liens.

Avec la toolchain MSVC (Visual Studio Build Tools), rien de tout cela n'est nécessaire.

## Pistes pour la suite

- Piste son dans le montage (indispensable pour une vraie animatique).
- Caméra par plan (cadrage, travelling, zoom) et transitions.
- Édition des points d'un trait vectoriel, gomme qui coupe aussi les zones remplies.
- Rendu vectoriel net à tous les zooms (tessellation GPU).
- Export vidéo via ffmpeg, export PDF direct du storyboard.
