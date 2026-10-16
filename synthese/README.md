# synthese : synthèse d'image par l'exemple

La propagation façon EbSynth : on peint une image clé, et elle est recopiée par morceaux sur les
autres images de la vidéo en suivant des guides. Étape a : la synthèse guidée (ci-dessous).
Étape b : le flux optique et la propagation d'une clé sur une vidéo. Étape c : plusieurs clés et
leur mélange (plus bas). Ce dossier est un projet Cargo à part (comme `src-tauri/`), sans
dépendance ; Pelure l'utilisera par son chemin à l'étape e.

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

## Étape b : propager une clé sur une vidéo

```rust
use synthese::propagation::{propagate, PropagateParams};

// load(i) lit l'image i de la vidéo ; each(i, image) reçoit chaque image fabriquée (false = arrêter).
propagate(load, cle, &peinture, 0..nombre, &PropagateParams::default(), |i, im| { …; true })?;
```

On part de la clé et on avance, puis on recule, d'une image à la fois. Chaque image est
synthétisée depuis la clé avec trois guides (« Stylizing Video by Example », Jamriška et al.,
SIGGRAPH 2019). Leurs poids par défaut sont ceux d'EbSynth :

| Guide | Source (la clé) | Cible (l'image à fabriquer) | Poids |
|---|---|---|---|
| vidéo | l'image de la vidéo sous la clé | l'image de la vidéo | 4 |
| position (« Mapping ») | x, y de chaque pixel | d'où vient le pixel dans la clé, suivi par le flux | 10 |
| temporel (« De-flicker ») | la clé peinte | l'image fabriquée juste avant, déplacée par le flux | 1 |

Le style lui-même (la clé) a le poids 1.

**Flux optique** (`flow.rs`) : méthode DIS (Kroeger et al., ECCV 2016), celle d'OpenCV, sans son
raffinement variationnel.
- Pyramide filtrée par [1 3 3 1] / 8. Une moyenne 2×2 laissait des fausses ondes qui faisaient
  rater les grands mouvements.
- Patchs 8×8 tous les 4 pixels, Lucas-Kanade inverse compositionnel (16 itérations).
- Passe aller, puis passe retour qui reprend les quatre voisins, toutes bandes de fils
  confondues. Sans elle, des rangées entières restaient fausses aux frontières entre fils.
- Moyenne pondérée des patchs qui couvrent chaque pixel, puis filtre médian 5×5 à chaque niveau.

Erreur sur des mouvements connus (tests) : moins de 0,1 pixel pour une translation de 4 px, moins
de 0,3 pour 25 px ou pour une rotation de 3° avec zoom de 4 %.

**Outils** :
- `propager.exe -images <dossier> -cle <n> <cle.png> -sortie <dossier> [-de <n>] [-a <n>]`
  propage une clé. Les images sont celles d'un dossier numéroté, comme celui que Pelure remplit
  en important une vidéo (`videos/<id>/000001.jpg`…). Les numéros commencent à 1. Options :
  `-poids <clé> <vidéo> <position> <temporel>`, `-flux <dossier>` (le flux en couleurs).
- `scintillement.exe <vidéo> <fabriquées> <de> <à>` mesure la stabilité. Chaque image fabriquée
  est comparée à la précédente, déplacée par le flux. La même mesure sur la vidéo donne ce que le
  flux n'explique pas.

**Essai sur une vraie vidéo (16 octobre 2026).** Images 30 à 60 de l'animatique BRAWLIDAYS
(`BS_BLD_Seq01_V01.mov`) en 960×540 : un travelling latéral sur des piles de livres, avec de la
parallaxe et du flou de mise au point. La clé est l'image 30 « peinte » par la synthèse elle-même :
la peinture à l'huile de l'exemple facestyle, guidée par la luminosité.
- 31 images en 2 min 23 s, environ 4,6 s par image.
- La peinture suit le mouvement 30 images durant, et les traits restent nets. Les zones qui entrent
  dans le champ (fenêtre, personnage) reçoivent une peinture plausible.
- Le premier plan flou s'étire un peu à l'horizontale.
- Scintillement moyen 1,71, contre 1,46 pour la vidéo. Sans le guide temporel (images 30 à 42) :
  2,05 contre 1,76 avec, pour une vidéo à 1,17.

## Étape c : plusieurs clés

```rust
use synthese::propagation::{propagate_keys, Key, PropagateParams};

let cles = [Key { frame: 29, style: &peinture_30 }, Key { frame: 59, style: &peinture_60 }];
propagate_keys(load, &cles, 0..nombre, &PropagateParams::default(), |i, im| { …; true })?;
```

- Avant la première clé et après la dernière : une seule séquence, comme à l'étape b.
- Entre deux clés : une séquence depuis chacune (A depuis la clé d'avant, B depuis celle
  d'après), puis un mélange image par image (`blend.rs`).
- Chaque image sort une fois. Une séquence A est gardée en mémoire le temps de faire B.

**Mélange** (« Stylizing Video by Example », section sur le mélange des clés). Un fondu enchaîné
(1 − α)·A + α·B double les traits et délave la texture là où A et B diffèrent. On mélange plutôt
dans le domaine des gradients :
- La synthèse rend son erreur par pixel (`Output::error`), moyennée par carrés de 8 pixels.
- Chaque pixel prend ses détails de A tant que la part d'erreur de A, ea / (ea + eb), reste sous
  1 − α. C'est donc d'abord là où B est meilleure qu'on passe à B.
- La bascule est étalée (`softness`, 0,5 par défaut) pour éviter un saut de texture d'une image à
  l'autre.
- Les couleurs d'ensemble suivent le fondu (attache `screen` = 0,1 : les formes de plus d'environ
  20 pixels).
- L'image est la solution de l'équation de Poisson écrantée, trouvée par gradient conjugué (au
  plus 300 itérations, un fil par canal).

Mesures sur l'animatique (clés « peintes » 40 et 50, images 40 à 50 ; la vidéo seule est à 1,77) :

| Mélange | Scintillement | Détail à l'image 45 |
|---|---|---|
| bascule nette (`softness` 0) | 2,33 | 4,63 |
| `softness` 0,3 | 2,25 | 4,61 |
| `softness` 0,6 | 2,19 | 4,59 |
| fondu enchaîné | 2,01 | 4,46 |

Le « détail » est l'écart moyen entre pixels voisins (`ecart -detail`). Il est dominé par les
traits noirs, communs à A et B. À l'œil, le fondu dédouble un peu les contours, et la bascule
nette garde la touche la plus franche. Plus la bascule est douce, moins ça scintille, et moins il
reste de détail.

Sur les images 30 à 60, avec des clés en 30 et 60 peintes séparément, comme le ferait un artiste :
- 31 images en 5 min 15 s : entre deux clés, chaque image est synthétisée deux fois.
- Près de l'image 60, la seconde clé remplace la peinture étirée du premier plan.

| Version | Scintillement (vidéo 1,46) | Détail à l'image 45 |
|---|---|---|
| une clé (30) | 1,71 | 4,71 |
| deux clés, bascule nette | 1,88 | 4,64 |
| deux clés, `softness` 0,5 (par défaut) | 1,84 | 4,62 |
| deux clés, fondu enchaîné | 1,66 | 4,51 |

**Outils** : `propager` accepte plusieurs `-cle <n> <cle.png>`, plus `-douceur <x>`, `-attache <x>`
et `-fondu` (simple fondu enchaîné, pour comparer). `ecart -detail a.png …` mesure le détail.

## Suite

- d. Calcul sur la carte graphique (wgpu).
- e. Dans Pelure : un calque vidéo, des clés, « Propager », des cels en sortie (annulables).
