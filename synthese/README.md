# synthese : synthèse d'image par l'exemple

Première brique de la propagation façon EbSynth (étape a) : on peint une image clé, la synthèse
la recopie par morceaux sur une autre image en suivant des guides. Ce dossier est un projet Cargo
à part (comme `src-tauri/`), sans dépendance ; Pelure l'utilisera par son chemin à l'étape e.

C'est un portage en Rust de la version processeur d'[ebsynth](https://github.com/jamriska/ebsynth)
(Ondřej Jamriška, domaine public) : pyramide d'images, PatchMatch avec uniformité, vote simple,
passe 3×3 en option. Même algorithme et mêmes réglages par défaut que la ligne de commande
d'ebsynth.

**Brevet.** PatchMatch est breveté par Adobe (brevet américain 8,861,869), comme le signale le
README d'ebsynth. Aucun souci pour expérimenter ; à faire vérifier avant de distribuer ou de vendre
Pelure.

## Utilisation

```rust
use synthese::{synthesize, GuidePair, Image, Params};

// Images 8 bits, canaux entrelacés. Image::from_rgba range une image RGBA sur 1 à 4 canaux.
let out = synthesize(&style, 1.0, &[GuidePair { source: &guide_src, target: &guide_cible, weight: 2.0 }],
                     &Params::default())?;
// out.image : la taille des guides cibles, les canaux du style.
// out.nnf[y * largeur + x] : le centre du patch du style recopié en (x, y).
```

`Params` : `uniformity` (3500), `patch_size` (5), `pyramid_levels` (le maximum possible),
`search_vote_iters` (6), `patchmatch_iters` (4), `extra_pass_3x3`, `threads` (0 = tous les cœurs),
`seed`.

## Différences avec l'original

- Le hasard vient d'une graine ; avec un seul fil, le résultat est reproductible.
- Chaque fil ne propage verticalement qu'à l'intérieur de sa bande de lignes. L'original lit les
  lignes des autres fils pendant qu'elles changent.
- Le vote est réparti sur tous les fils (séquentiel dans l'original).
- Pas de CUDA, pas de « modulation » des guides (la ligne de commande d'ebsynth ne s'en sert pas).

## Tester

Lancer cargo depuis ce dossier, pour que `../rust-toolchain.toml` choisisse MSVC :

```
cargo test
cargo build --release --examples
```

Deux outils en ligne de commande :

- `target/release/examples/ebsynth.exe` prend les mêmes options qu'ebsynth (`-style`, `-guide`,
  `-weight`, `-output`, `-uniformity`, `-patchsize`, `-pyramidlevels`, `-searchvoteiters`,
  `-patchmatchiters`, `-extrapass3x3`), plus `-threads` et `-seed`.
- `target/release/examples/ecart.exe a.png b.png` mesure l'écart entre deux résultats ;
  `ecart.exe -montage sortie.png a.png b.png …` met des images côte à côte.

## Comparaison avec l'original (16 octobre 2026)

L'ebsynth original a été compilé depuis ses sources (MSVC, OpenMP, processeur seul) et lancé sur
ses trois exemples avec les options de leurs `run.bat`. Le portage a été lancé sur les mêmes.
PatchMatch tire au hasard, donc deux passes ne donnent jamais la même image. La référence est
l'écart entre deux graines du portage.

Écart moyen par canal (0 à 255) :

| Exemple | Original / portage | Portage graine 0 / graine 7 |
|---|---|---|
| texbynum (320×462) | 10,7 | 10,6 |
| stylit (1200×912) | 10,4 | 10,0 |
| facestyle (810×1000) | 4,6 | 5,3 |

L'écart avec l'original est du même ordre que celui du portage avec lui-même : les deux font la
même chose. À l'œil, les images sont de même qualité.

Temps sur l'i7-10700K (8 cœurs, 16 fils) :

| Exemple | Original | Portage |
|---|---|---|
| texbynum | 0,8 s | 0,7 s |
| stylit | 14 à 16 s | 10 à 11 s |
| facestyle | 8 s | 6,3 à 6,8 s |

facestyle sur un seul fil : 43 s.

Un style de bruit pur avec un guide de position décalé fait échouer l'original comme le portage
(écart 82 pour les deux). Sur un style qui a des formes, les deux suivent le guide (écart 13).

## Suite

- b. Flux optique et guides vidéo : propagation d'une clé sur une plage d'images.
- c. Plusieurs clés et mélange entre elles.
- d. Calcul sur la carte graphique (wgpu).
- e. Dans Pelure : un calque vidéo, des clés, « Propager », des cels en sortie (annulables).
