# Pelure — audit du rendu des brosses : pourquoi ça ne ressemble pas à du dessin traditionnel

Audit du 2026-10-18 (branche `audit-rendu`), après le chantier « sensation des brosses »
(`docs/SENSATION-BROSSES.md`). Anthony : « pour moi rien ne change. Le dessin est là, il n'est pas
bloquant, mais les brosses ne simulent toujours pas cet effet de dessin ou peinture traditionnelle. »
Il l'a aussi constaté sur un calque bitmap (Maj+B) : ce n'est donc pas une affaire de calque
vectoriel.

Méthode : un essai ignoré, `src/rendu_audit_tests.rs`, fait tracer par le vrai moteur de Pelure
quatre gestes (rampe de pression, hachures, courbe en S, zone remplie par passages croisés) avec
chaque réglage, en bitmap et en vectoriel, à 100 % et en gros plan ×4, sur papier blanc. Même chose
avec l'outil TVPaint d'Anthony (`raccourcis outils.tvpx`), une sélection des panneaux livrés avec
TVPaint 12 (crayons, craies, plumes, peinture) et des réglages Krita / MyPaint du poste, tous importés
par Pelure. Rien n'a été exécuté dans TVPaint : ses fichiers ont été lus, pas son programme, et ses
formules ne sont pas connues (ce qui est deviné est marqué **à vérifier**).

Pour refaire les planches (6 secondes) :

```
cargo test --release --offline --target-dir target\audit planches_rendu_audit -- --ignored --nocapture
```

Elles vont dans `audit-rendu/` (hors git), avec `LISEZMOI.txt` (comment les lire) et `mesures.txt`
(réglages exacts et chiffres). Les chiffres cités ci-dessous en viennent.

## 1. Résumé

1. Les brosses livrées (Pinceau rond, Crayon de la barre, Encre) sont des **rubans lisses et
   uniformes** : une empreinte ronde parfaite, posée tous les 8 % du diamètre, à opacité pleine, sans
   rien qui varie le long du trait sauf l'épaisseur. C'est la définition même d'un trait numérique.
   Planches `01`, `02`, `06`.
2. Les deux brosses « à grain » (Crayon à papier, Craie grasse) s'appuient sur un **papier qui n'est
   pas un papier** : un bruit par pixel (« Grain fin ») ou des taches de 2–4 pixels (« Grain
   gros »), binaire. Ça fait du pointillé sale ou un pochoir à trous, pas du graphite ni de la craie.
   Planches `04`, `05`, `10`.
3. Le moteur sait pourtant faire : le crayon 2B de TVPaint importé dans Pelure (pointe en image
   irrégulière en 12 cases, appui sur la taille et l'opacité) donne déjà un trait qui ressemble à
   du graphite — **planche `43`**, à comparer à `01`. Ce qui manque n'est pas le moteur, ce sont
   des **pointes en image**, de **vrais papiers**, et des **réglages de débit** (la peinture qui
   s'accumule au lieu d'un aplat au maximum).
4. L'outil TVPaint d'Anthony (« Trait » : `tv_pencil` 9,5 px, appui → taille et Power, grain
   « Soft ») n'importait rien dans Pelure (`[ToolBox]` majuscule, corrigé). Importé, il donne un
   ruban gris → noir lisse (planche `30`) parce que le **grain propre du crayon TVPaint** (Silk /
   Soft / Medium / Hard, indépendant du papier) n'existe pas dans Pelure.
5. Le chantier « sensation » (bord doux, plancher, seuil, lissage) ne change presque rien à
   l'image — planche `00` : les rangées « ancien » et « nouveau » sont quasi identiques. Il réglait
   des détails sous le pixel ; la matière n'y était pas. D'où « rien ne change ».

Le reste de ce rapport : les causes une à une avec leur planche (§2), ce qui manque par famille
d'outils avec des réglages chiffrés (§3), le cas vectoriel (§4, détaillé dans
`docs/VECTORIEL-COMME-BITMAP.md`), un plan par étapes (§5), ce qui a été corrigé pendant l'audit
(§6), les questions pour Anthony (§7), et les détails techniques à part (§8).

## 2. Le diagnostic, cause par cause

Chaque cause est prouvée par une planche de `audit-rendu/`. Lecture d'une planche : colonnes A
(rampe de pression, de 2 % à 100 % de gauche à droite), B (hachures), C (courbe en S), D (passages
croisés : zigzag noir puis zigzag bleu sur la moitié droite) ; rangées bitmap et vectoriel à 100 %,
puis les mêmes en gros plan ×4.

### 2.1 Rubans lisses : l'empreinte est un disque parfait, posée dense, à opacité pleine

- **Planches** `02_pelure_pinceau-rond.png`, `06_pelure_encre.png`, `01_pelure_crayon-barre.png`,
  `11_pointes.png`.
- **Ce qu'on voit** : le Pinceau rond fait un ruban noir à bord net dont seule l'épaisseur change
  (rampe : alpha 255 partout, de 3 à 7 px de large) ; 64 % des pixels du S sont pleins. Le Crayon
  de la barre (2,5 px, opacité ↔ appui) fait un fil gris uniforme (rampe : alpha 83 → 238) ; ses
  hachures sont des traits gris pâle tous pareils. Au gros plan ×4, le bord est une rampe propre d'un
  pixel : rien ne bouge le long du trait.
- **Pourquoi** : `BrushSettings::brush()` — empreinte ronde (pas de pointe en image), dureté 0,85,
  pas 8 % (12 empreintes par diamètre : le recouvrement gomme toute irrégularité), opacité 1,
  débit 1, pas d'accumulation, aucun hasard (taille, angle, dispersion à 0). Le crayon
  (`pencil()`) ne fait que relier l'opacité à l'appui : un gris uniforme à chaque appui.
- **Ce que fait TVPaint** : ses crayons livrés sont des *Custom Brushes* à pointe en image
  (`11_pointes.png` : le 2B est un amas de grains de 29×25 px en 12 images, le HB 150 images de
  10×10), tournée au hasard (`c_angle2=8`), à opacité 60–100 % mais **Power** réduit ou connecté à
  l'appui, avec un pas de 12,5 % par défaut, souvent avec un papier.

### 2.2 Le papier de Pelure est un bruit, pas un papier

- **Planches** `10_papiers.png` (relief à 1:1, ×4, et le grain laissé à 20 / 50 / 90 % d'appui),
  `04_pelure_crayon-a-papier.png`, `05_pelure_craie-grasse.png`.
- **Ce qu'on voit** : « Grain fin » est un bruit blanc à un texel par pixel (au ×4 : un damier
  aléatoire) ; « Grain gros » (flou 1) fait des taches de 2–4 px. Les papiers de TVPaint (Paper 33,
  cement_02, noisy_009…, 256×256) ont une structure : fibres, grumeaux, directions, un grain de 3 à
  10 px. Avec le Crayon à papier, le trait devient un pointillé sale (rampe : alpha 63 à 12 %
  d'appui, 236 à 90 %, mais en points isolés) ; avec la Craie grasse, un pochoir à trous noirs et
  blancs.
- **Pourquoi** : `procedural_paper(256, 0, 11)` et `(256, 1, 29)` dans `BrushLibrary::builtin` :
  du bruit par pixel, flouté de 0 ou 1 texel, réparti uniformément. Et `grain_factor` applique un
  **seuil presque binaire** (smoothstep large de ±0,12 sur un relief de 0 à 1) : un pixel est
  « pris » ou « pas pris ». Un vrai grain répond en douceur (le dépôt suit la hauteur relative du
  papier, sur une large plage).
- **Second défaut, corrigé pendant l'audit** : les papiers importés (TVPaint, PNG) étaient étirés
  de 0 à 1 mais pas répartis ; or le seuil suppose une répartition uniforme (c'est ce que fait
  `procedural_paper`). Un papier scanné a presque tout son relief dans les gris moyens : « Paper
  04 » passait de « rien » à « tout noir » entre 20 et 50 % d'appui (`10_papiers.png`, rangées
  « étiré (avant) » contre « réparti par rang (après) »). Voir §6.

### 2.3 Rien ne s'accumule : le trait garde le maximum, la Puissance est à 100 %

- **Planches** : colonne D de toutes les planches Pelure ; `20_essai_graphite.png` pour la
  comparaison.
- **Ce qu'on voit** : dans un même trait, repasser ne fonce pas (le zigzag noir est uniforme). D'un
  trait à l'autre, ça fonce par simple superposition : rapport croisé / un passage ≈ 1,8–1,9 pour
  tous les réglages, le même pour un crayon et pour un pinceau. Un crayon ou une craie réels
  foncent à chaque passage, et une seule couche n'est jamais noire.
- **Pourquoi** : `build_up: false` et `power: 1` par défaut (`brush.rs`) : le masque du trait garde
  `max(couverture)`, l'opacité est atteinte dès la première empreinte. Les briques existent (débit
  + « Accumuler », étape 4 du moteur) mais aucun réglage livré ne s'en sert.
- **Ce que fait TVPaint** : Power = « amount of paint that gets laid down », Opacity = plafond
  (doc et `english.tvpaint.loc`). Les crayons livrés ont Power ou Opacity réduits et connectés à
  l'appui ; l'outil d'Anthony relie **Power** à l'appui (`cpower "10;…"`).

### 2.4 Le crayon TVPaint a un grain propre, pas Pelure

- **Planche** `30_tvpx_anthony_trait.png`.
- **Ce qu'on voit** : l'outil « Trait » d'Anthony importé (9,5 px, profil à 3 points, appui →
  taille et opacité) donne un ruban gris → noir propre (rampe : 24 → 224), lisse, sans grain.
- **Pourquoi** : la commande `tv_pencil … grain 1` : le crayon de TVPaint a un menu « Grain »
  (Silk, Soft, Medium, Hard — `english.tvpaint.loc` lignes 6923–6927 ; doc : « to get a
  softer-grained or a rougher-grained brush »), indépendant du papier (`tv_paper Active 0` chez
  Anthony). Le loc décrit l'outil : « Pencil … Like a charcoal pencil ». Pelure n'a rien de tel ;
  l'importeur le note maintenant dans son compte rendu. Ce qu'est exactement ce grain (bruit fixe
  sur la feuille ? sur la pointe ? quelle taille ?) : **à vérifier**, Pelure ne peut que le refaire
  à sa façon.

### 2.5 Le chantier « sensation » ne touchait pas la matière

- **Planches** `00_toucher_crayon.png`, `00_toucher_pinceau-rond.png`.
- **Ce qu'on voit** : « toucher ancien » et « toucher nouveau » sont quasi identiques à 100 % ; au
  ×4, le nouveau bord est un rien plus doux, le trait fin un rien plus plein. La seule différence
  visible est le crayon crénelé d'avant (troisième rangée).
- **Conclusion** : c'était le bon chantier pour les bouts de trait et le lissage, pas pour le
  rendu. Anthony a raison : à l'image, rien ne change.

### 2.6 Ce qui tient déjà debout (vérifié sur les planches)

- Les **pointes en image** (multi-cases, à la suite / au hasard / selon l'appui / la direction,
  retournées, réduites par niveaux) : les crayons TVPaint importés (`43`, `42`, `48`) ont de la
  matière.
- Les **connexions** (appui, vitesse, direction, fondu, hasard, courbes) : la rampe du 2B
  (`43`) s'affine et s'éclaircit comme attendu.
- Le **papier ancré à la feuille** (repasser retrouve les mêmes creux), la **dispersion**, le
  **nombre d'empreintes**, le **profil**, la **peinture humide** (mélange de la couleur dessous,
  charge : `22_essai_gouache.png`, colonne D, le bleu se mêle au noir).
- Le **toucher** (bord doux, trait fin plein) et le **lissage** : propres.

### 2.7 Ce qui est mal réglé ou mal fait dans le moteur (détails §8)

| # | Brique | Problème | Planche | Gravité |
|---|---|---|---|---|
| a | Papiers livrés | bruit par pixel, pas de structure | `10`, `04`, `05` | forte |
| b | `grain_factor` | seuil presque binaire (±0,12) ; pas de « dureté » du papier | `05` ×4 | forte |
| c | Papiers importés | étirés, pas répartis → tout ou rien (**corrigé**) | `10` avant/après | forte |
| d | Réglages livrés | ni pointe en image, ni débit < 1, ni accumulation, ni hasard | `01`, `02`, `06` | forte |
| e | Import TVPaint | `[ToolBox]` ignoré (**corrigé**) ; `step` des outils de base lu en pixels → chapelet (**corrigé**, à vérifier) ; grain du crayon absent (noté) | `30` | moyenne |
| f | Import MyPaint / Krita | `opaque` 0,15 pris comme plafond d'opacité au lieu d'un dépôt qui s'accumule : le Pencil 2B importé est quasi invisible (alpha max 35 à 90 % d'appui) | `60` | moyenne |
| g | Modèle « max » + pointe grise dense | une pointe en demi-teintes posée tous les 8 % devient un aplat uniforme (Watercolour 1 : alpha 127 partout) | `55` | moyenne |
| h | Pointes `.dip` animées | traînées horizontales dans certaines images (Chinese Brush, Watercolour 1, images 2–3) : décodage des différences `DBAN` **à vérifier** | `11`, `50` (vectoriel B) | faible |
| i | Peinture humide, charge | la charge s'épuise vite et uniformément (essai gouache : 237 → 140 sur 230 px) ; pas de « séchage » (Drying) entre traits ; pas d'étalement (Shift) | `22` | faible |
| j | Vectoriel | perd opacité ↔ appui, connexions, humide, accumulation au lâcher | `01`, `30`, `43`, `22` | voir §4 |

## 3. Par famille d'outils : ce qui manque, et les réglages proposés

Les valeurs sont celles des champs de `BrushSettings` (voir `mesures.txt` pour les réglages
actuels). « Brique » = ce qui n'existe pas encore dans le moteur. Chaque proposition est à juger à
la tablette, planche à l'appui.

### 3.1 Crayon graphite (l'outil de storyboard d'Anthony)

- **Ce qu'il faut** : un dépôt granuleux qui fonce avec l'appui et les passages, jamais tout à fait
  noir, un bord irrégulier, un grain fixe sur la feuille.
- **Manque** : une pointe en image (amas de grains, 8–12 cases, tournée au hasard), un papier à
  structure, un grain à réponse douce, un débit < 1 qui s'accumule. Brique : grain propre du crayon
  (§2.4), transition douce du grain (§8.2).
- **Réglage proposé « Graphite »** : taille 4, pointe en image (graphite), `tip_cells` 8–12,
  `tip_order` hasard, `angle_jitter` 360, `flip_x/y`, pas 0,12, débit **0,45** avec **Accumuler**,
  opacité **0,9**, appui → taille (mini 0,3) et → débit (courbe douce `[0,0.1] [0.5,0.45] [1,1]`),
  grain 0,7, papier structuré (taille 1,5), `size_jitter` 0,1, `scatter` 0,03, lissage 0,15.
- **Vérification A/B** : hachures légères (le grain doit apparaître, pas un gris plat) ; trois
  passages au même endroit (ça fonce à chaque fois) ; appui à fond (le noir reste un peu
  granuleux) ; comparer au 2B TVPaint importé (`43`).

### 3.2 Crayon de couleur

- Comme le graphite, avec opacité plafond 0,8, débit 0,3, grain 0,8, et des couleurs qui se
  **multiplient** au croisement (jaune sur bleu = vert).
- **Brique** : un mode de fusion par trait (Multiplier) dans `paint` et dans le rendu vectoriel.
  Aujourd'hui tout est « normal » (superposition).

### 3.3 Encre au pinceau

- L'encre est le seul médium qui *est* un ruban plein : le Pinceau rond et « Encre » ne sont pas
  loin. Ce qui manque : des bouts effilés (courbe de taille ferme, mini 4 %), un bord vivant
  (très léger hasard), et le **pinceau sec** quand on va vite (opacité ↔ vitesse inversée, pointe à
  poils).
- **Réglage proposé « Encre »** : taille 7, dureté 1, pas 0,05, `dynamics.size` appui courbe
  `[0,0] [0.4,0.15] [0.8,0.7] [1,1]` mini 0,04, `size_jitter` 0,03, lissage corde 6 px d'écran.
  Variante « pinceau sec » : pointe à poils (image), angle = direction, `dynamics.opacity` vitesse
  inversée. Planche `23_essai_encre.png` : les bouts sont déjà meilleurs que `06`.
- **A/B** : boucle rapide (les bouts s'effilent), trait lent appuyé (plein, noir), trait rapide
  léger (sec, cassé).

### 3.4 Feutre

- Un feutre est un aplat à 60–80 % qui **se multiplie** en repassant, avec un biseau (pointe
  aplatie à angle fixe) et un bord à peine doux.
- **Réglage** : taille 10, `roundness` 0,55, `angle` 40 fixe, dureté 0,9, pas 0,05, opacité 0,7,
  débit 1, pas d'accumulation dans le trait (un feutre ne fonce pas en un passage), **Multiplier**
  entre traits. Brique : le mode de fusion par trait (3.2).
- **A/B** : deux traits croisés (le croisement est plus foncé, de la même teinte) ; un aller-retour
  sans lever (uniforme).

### 3.5 Gouache et huile

- **Ce qu'il faut** : des poils (stries le long du trait), une matière opaque qui se mélange un peu
  à ce qui est dessous, une brosse qui se vide.
- **Existant** : mélange (`mix`), charge, pointe en image, angle = direction (planche `22` : le
  mélange marche, la charge aussi, mais sans poils ça reste un ruban gris).
- **Manque** : pointes à poils livrées (ou importées des panneaux TVPaint « PaintBrushes » :
  `53_tvp_oil-1.png` montre des stries, mais aussi le motif répété de la pointe, à casser par
  `angle_jitter` 10 et `scatter` 0,05) ; une charge qui s'épuise moins vite (§8.4) ; le « séchage »
  (Drying) : avec séchage, le trait suivant ne se mélange pas au précédent ; sans, si. Pelure
  mélange toujours.
- **Réglage proposé « Gouache »** : taille 16, pointe poils plats, angle = direction,
  `angle_jitter` 8, pas 0,06, opacité 1, débit 0,9, `mix` 0,25, `charge` 0,95, dureté 0,6.
- **A/B** : un trait bleu sur un trait jaune frais (le bleu entraîne du jaune sur quelques
  centimètres), puis le même après « séchage ».

### 3.6 Aquarelle

- **Ce qu'il faut** : une transparence qui s'accumule (lavis), un **bord plus foncé** (le pigment
  se dépose au bord de la flaque), de la granulation dans le grain du papier, de la diffusion dans
  l'humide.
- **Existant** : lavis possible (opacité 0,35, dureté 0,3, pas 0,05, grain 0,4 papier taille 2,
  `mix` 0,2) ; granulation par le papier (une fois les papiers vrais).
- **Briques manquantes** : le bord foncé (un post-traitement au lâcher : assombrir le contour du
  masque du trait — 1 à 2 jours) ; la diffusion (une simulation d'eau sur la feuille — gros
  chantier, à ne pas commencer sans accord).
- Les « Watercolour » de TVPaint importées (`55`, `56`, `57`) sont des pointes en demi-teintes
  posées dense : un aplat gris (§2.7 g). TVPaint les rend avec son « Wet profile » (Alpha /
  Alpha Max, doc) dont la formule n'est pas publiée.

## 4. Vectoriel contre bitmap

Confirmé par Anthony et par les planches : le ressenti est le même en bitmap, le vectoriel n'est pas
la cause. Mais, au lâcher, le trait vectoriel **ne garde pas** ce que le trait bitmap montre pendant
le geste dès que le réglage sort du pinceau rond : opacité ↔ appui (Crayon de la barre, planche
`01` : 74 % des pixels diffèrent de plus de 16/255), connexions (l'outil d'Anthony, planche `30` :
92 à 100 % des pixels ; 2B TVPaint, `43`), peinture humide (`22` : 91 %), et même le bord du
Pinceau rond (`12_vecto_bord.png`, ancienne couverture contre bord doux : 33 % des pixels). Seuls
les traits à papier ou pointe en image sont rejoués par le moteur bitmap (écart ≈ 0 : `04`, `05`,
`20`, `21`).

**Recommandation** : le calque par défaut peut rester vectoriel (les traits modifiables et
l'interpolation sont voulus), à condition de rendre le trait vectoriel **par le moteur bitmap
rejoué** : garder dans l'élément le chemin avec l'appui et l'instant, le réglage complet et la
graine, et rejouer les empreintes. C'est exact trait pour trait, c'est le moins coûteux, et c'est
réversible. Coût, risques, pistes alternatives et plan : `docs/VECTORIEL-COMME-BITMAP.md`.
En attendant : pour juger les nouvelles brosses à la tablette, dessiner sur un calque bitmap (ce que
le trait vectoriel fera ensuite dépend de ce chantier).

## 5. Plan par étapes, de la plus rentable à la plus coûteuse

Chaque étape se voit à la tablette et se vérifie par un A/B ; les planches de l'audit servent de
témoin « avant ». Les durées sont des ordres de grandeur.

| # | Étape | Durée | Ce qu'Anthony verra | A/B à la tablette |
|---|---|---|---|---|
| 0 | **Fait** : import TVPaint réparé (`[ToolBox]`, `step` en %, grain noté), papiers importés répartis par rang (§6) | fait | ses panneaux TVPaint s'importent ; un papier TVPaint importé a du grain à tout appui | importer `raccourcis outils.tvpx` et `Resources\custompanels\Pencils_v1.bin` par « Importer des pinceaux », dessiner avec « Trait » et « 2B Pencil » sur un calque bitmap |
| 1 | **Pointes et papiers vrais, livrés avec Pelure** : 5–6 pointes fabriquées dans Pelure (amas de grains pour graphite et fusain, craie, biseau de feutre, poils plats et ronds — par procédure ou dessinées, pas copiées de TVPaint), 3 papiers à structure (bruit à plusieurs échelles 2–8 px, « toile », « vergé ») ; les réglages livrés refaits avec (§3) | 1–2 j | le crayon fait du grain, la craie de la craie ; les aperçus de la grille changent | hachures légères, aplat appuyé, à comparer au 2B TVPaint importé et aux planches `01`/`04` |
| 2 | **Grain à réponse douce et dépôt qui s'accumule** : largeur de transition du grain réglable (« dureté du papier », défaut ±0,3), débit 0,3–0,5 avec Accumuler sur les outils secs, opacité plafond 0,85–0,9, appui → débit ; import MyPaint/Krita : `opaque`/Flow → débit accumulé | 1 j | repasser fonce ; les hachures légères sont grises et granuleuses ; le Pencil 2B MyPaint devient visible | trois passages au même endroit ; rampe d'appui ; planche `60` avant/après |
| 3 | **Grain propre du crayon** (Silk / Soft / Medium / Hard) : un bruit fin ancré à la feuille, appliqué même sans papier, pour l'outil Crayon et l'import `tv_pencil` | 1 j | l'outil « Trait » importé retrouve une texture ; le Crayon de la barre aussi | son Trait dans TVPaint et dans Pelure, même geste, PNG côte à côte |
| 4 | **Mode de fusion par trait** (Multiplier, puis Écran…) pour feutre et crayon de couleur, bitmap et vectoriel | 1 j | deux feutres croisés font une troisième couleur | croisement jaune/bleu |
| 5 | **Vectoriel rejoué par le moteur bitmap** (`docs/VECTORIEL-COMME-BITMAP.md`) | 3–5 j | le trait ne change plus au lâcher sur un calque vectoriel | même geste bitmap / vectoriel, écart 0 |
| 6 | **Peinture** : poils, charge plus lente, séchage on/off, puis bord foncé de l'aquarelle | 2–3 j (+1–2 j aquarelle) | gouache qui s'entraîne, lavis à bord foncé | bleu sur jaune frais / sec ; lavis superposés |
| 7 | **Inclinaison du stylet** (les « Tilt crayons » de TVPaint, `c_size2=11`) et heure vraie des échantillons (WM_POINTER) | avec Anthony | le côté de la mine | à la tablette |
| 8 | Diffusion de l'aquarelle, relief (empâtement) | plus tard | — | — |

Les étapes 1 à 4 se font sans toucher au format des projets ni au moteur de trait (que des réglages,
des textures livrées, une fonction de grain et un mode de fusion), chacune réversible par un
réglage. Rien de tout cela n'a été commencé : l'audit s'arrête au diagnostic et aux petites
corrections du §6, comme convenu.

## 6. Corrections faites pendant l'audit (branche `audit-rendu`)

Petites, sûres, testées, chacune réversible en une ligne.

1. **Import TVPaint, sections sans casse** (`tvpbrush::read_ini`) : un `.tvpx` exporté par
   TVPaint 12 écrit `[ToolBox]`, les panneaux livrés `[toolbox]` ; l'export d'Anthony n'importait
   aucun outil (`mesures.txt` : « 0 réglages »). Test `an_exported_panel_with_capitalized_section_is_read`.
2. **Import TVPaint, `step` des outils de base en pour cent** (`tvpbrush::base_tool`) : lu en
   pixels, le crayon 9,5 px d'Anthony à `step 12.5` posait une empreinte par diamètre (pas borné à
   1,0 : un chapelet). Pris comme le `stepval` des brosses perso (12,5 par défaut dans les deux
   cas). **À vérifier** dans TVPaint (la bulle dit « number of pixels »). Et le `grain` du
   `tv_pencil` est noté dans le compte rendu d'import.
3. **Papiers importés répartis par rang** (`GrayImage::equalized`, `BrushLibrary::add_paper` et
   `load`) : le seuil du grain suppose une répartition uniforme du relief (comme
   `procedural_paper`) ; un papier scanné ne l'a pas. Planche `10` avant/après. Test
   `imported_papers_are_rank_equalized`. Conséquence : les papiers déjà importés dans
   `%APPDATA%\Pelure\pinceaux` changent de rendu au prochain lancement (plus de grain à appui
   moyen) — c'est voulu ; l'ancien comportement est `normalized()`.
4. `.gitignore` : `audit-rendu/` (planches refaites par l'essai).

Les 574 tests d'avant passent, plus les 2 nouveaux (576), et l'essai ignoré `planches_rendu_audit`.

## 7. Questions pour Anthony

1. Dans TVPaint, pour le storyboard, est-ce bien « Trait » (ton `tv_pencil` 9,5 px, grain Soft,
   sans papier) que tu utilises le plus ? Ou aussi les crayons du panneau Pencils (2B, HB) ? Et
   lequel aimerais-tu retrouver en premier dans Pelure ?
2. Tes panneaux TVPaint importés dans Pelure (« Importer des pinceaux » : ton `.tvpx`, et
   `C:\Program Files\TVPaint Developpement\TVPaint Animation 12 Pro\Resources\custompanels\Pencils_v1.bin`)
   te vont-ils comme réglages de départ, ou veux-tu des brosses propres à Pelure (qui ne
   copieraient rien de TVPaint) ?
3. Le grain doit-il rester fixé à la feuille (TVPaint : repasser retrouve les mêmes creux) ou
   suivre le trait (Procreate « grain moving ») ? Pelure sait faire le premier.
4. Acceptes-tu qu'un crayon ne fasse jamais du noir pur (opacité plafond ≈ 90 %) et qu'il faille
   repasser pour foncer ? C'est le prix de la matière.
5. Quelle taille de projet (1920×1080 ? plus) et quel zoom de travail ? La taille du grain et des
   pointes en dépend ; les planches sont à 100 %.
6. Pour comparer sans se tromper : refaire dans TVPaint les quatre gestes des planches (rampe
   d'appui, hachures, S, zone croisée) avec « Trait » et le 2B, exporter en PNG à 100 %, et les
   poser à côté de `30_tvpx_anthony_trait.png` et `43_tvp_2b-pencil.png`. C'est l'A/B le plus
   parlant, et il dira ce que vaut le `step` deviné (§6.2).
7. L'ordre du plan te va-t-il (pointes et papiers d'abord, vectoriel ensuite), ou le vectoriel
   avant tout parce que c'est ton calque par défaut ?

## 8. Détails techniques

### 8.1 Le modèle d'empreinte de Pelure (`brush.rs`, `dab`)

Pour chaque empreinte : rayon = taille/2 × facteur d'appui (plancher du toucher), couverture d'un
pixel = pointe en image (bilinéaire, réduite par niveaux) ou disque (`coverage_soft` : smoothstep
de rayon × dureté au bord + 0,5 px) ou profil ; `v = couverture × débit-appui × grain × pochoir ×
dépôt`. Sans accumulation : `masque = max(masque, v)` ; avec : `masque += (1 − masque) × v`, où
`dépôt = 1 − (1 − débit)^(1/n)`, n = diamètre ÷ pas. Pixel = `avant` peint de `masque × opacité`.
Peinture humide : pas de masque, lerp prémultiplié vers la couleur de la brosse (moyenne pondérée
sous l'empreinte mêlée à la sienne), couverture `charge + (1 − charge) × couverture prise × mélange`.

Pourquoi ça fait « numérique » : sans accumulation, 12 empreintes par diamètre et une couverture
pleine, le masque vaut 1 partout sous le trait dès le premier passage — un aplat, quel que soit le
hasard. Avec une pointe en image en demi-teintes, le `max` des empreintes qui se recouvrent tend
vers le maximum de la pointe : le motif de la pointe est écrasé (`55`). La texture revient quand
le dépôt par empreinte est petit et s'accumule (`20_essai_graphite.png`, S : écart-type intérieur
2,1, un ton sans aplat… mais sans grain à cause du papier).

### 8.2 Le grain (`brushtex::grain_factor`)

`t = 0,15 + 0,6 × (1 − appui)` ; `rempli = smoothstep(t − 0,12, t + 0,12, relief)` ;
`facteur = 1 − grain × (1 − rempli)`. À appui 1, tout relief > 0,27 est plein ; à appui 0, seul le
relief > 0,87 l'est. La transition de ±0,12 fait un contour binaire, visible sur la craie (`05`
×4). Proposition : largeur réglable (`paper_hardness`, défaut 0,3 ; TVPaint appelle « Hardness »
le curseur que Pelure a pris pour `grain`, **à vérifier** lequel des deux il est), et une
répartition par rang des papiers (faite) pour que `t` soit une fraction de creux.

Les papiers livrés : `procedural_paper(256, flou 0 ou 1)` ; remplacer par un bruit à plusieurs
échelles (somme de bruits floutés de 1, 2, 4, 8 texels, pondérés), toujours réparti par rang, ou
par des images dessinées dans Pelure. Taille d'un texel : 1 px du dessin à 1920×1080 ; prévoir
`paper_scale` 1,5–2 par défaut pour les crayons.

### 8.3 Les pointes

`11_pointes.png` : TVPaint livre des pointes de 10×10 à 727×415 px, de 1 à 150 images, en
`.dip`. Pelure les lit (`dip.rs`), avec un doute : des traînées horizontales dans certaines images
des brosses animées (Chinese Brush images 2–3, Watercolour 1 image 3). Le trait bitmap n'en souffre
pas visiblement, le vectoriel du Chinese Brush (`50`, colonne B) si (les cases diffèrent entre
aperçu et rendu : l'ordre « à la suite » dépend du nombre d'empreintes posées). **À vérifier** sur
le décodage `DBAN` (sauts `0x80 k`). Pelure ne livre aucune pointe ; il faut en fabriquer (une
pointe « amas de grains » se fabrique par procédure : N disques doux de rayons aléatoires dans un
cercle, 8–12 tirages).

### 8.4 La charge humide

`wet_load *= exp(−(1 − charge) × 0,3 × pas)` par empreinte, le pas en diamètres : à `charge`
0,85 et pas 0,08, 0,996 par empreinte, 0,64 après 100 empreintes (130 px à 16 px de diamètre).
L'essai gouache (`22`) passe de 237 à 140 sur la rampe. Pour une gouache, une charge qui dure 3 à
5 fois plus (coefficient 0,1 au lieu de 0,3, ou charge 0,95) ; et une reprise de charge quand la
brosse repasse dans de la peinture fraîche (déjà partiellement fait par le mélange).

### 8.5 Imports : ce qui a été lu

- `raccourcis outils.tvpx` (tar) : `[ToolBox]`, 3 boutons : « Trait » = `Tool:tv_pencil mode
  "color" size 9.5 power 100 opacity 100 aspect 100 angle 35 step 12.5 dry 1 gradient 0 grain 1
  csize "10;2 0 1 0 0 1 1" cpower "10;2 0 1 0 0 1 1" profile "3 1 1 0 0 0.0746 0.737 1 1"` +
  `tv_paper Active 0` ; « Gomme » et « Remplir » = `tv_setactiveshape freehandfill` (ignorés).
- Panneaux livrés : Pencils (23 brosses : 20 custom à pointe `.dip`, 10 papiers, 2 airbrush),
  Chalks (5), Pens (11), PaintBrushes (27). Dans leurs `.ini` : `opacity` 10–100, `stepval` 1,5–49,
  `c_size2`/`c_opacity2` à l'appui (10), au hasard (8), à l'altitude (11, non lu), `anim` 3–15,
  `stamp` 1 (alpha), 5–7 (estompe → `mix`), `dry` 0/1, `smear` 0–100, `shift` 0–100, `mode` 0/1/38.
- MyPaint `c)_Pencil_2b` : `opaque 0.15` + `opaque_multiply` ↔ appui, `hardness 0.2`,
  `dabs_per_actual_radius 4`, `offset_by_random 0.5`. Pelure en fait opacité 0,15 plafond ; MyPaint
  compose chaque empreinte « par-dessus » (son `opaque` est un dépôt) : il faudrait `power` 0,15 +
  Accumuler, opacité 1. Même chose pour le Flow de Krita.

### 8.6 Sources

- Code lu : `brush.rs`, `brushtex.rs`, `vector.rs`, `vtools.rs`, `tvpbrush.rs`, `dip.rs`,
  `kritabrush.rs`, `newlayer.rs`, `document.rs`, `app.rs`, `brushui.rs`, `banc.rs`.
- TVPaint 12, lecture seule : `Resources\custompanels\*.bin` et `.data\*.ini`, `.dip`,
  `Resources\papers\Legacy\*.dip`, `Resources\data\english.tvpaint.loc` (lignes 6907–6994).
  Doc publique : doc.tvpaint.com, « Brush engine overview » (Power « controls the amount of color
  to be applied », Opacity « transparency level », Drying « When off, brush strokes will blend
  together », Grain « softer-grained or a rougher-grained brush »), « Settings Cheat Sheet »,
  « Custom brushes » (Stamp Origin / Alpha / Alpha Max / Luma / Hue / Smear ; Wet profile),
  « Drawing modes ».
- Krita du poste : `%APPDATA%\krita\paintoppresets\*.myb`, `*.kpp` ; libmypaint (doc des réglages).
- Rien n'a été copié : les pointes et papiers de TVPaint ne sont utilisés que pour les planches,
  lus dans l'installation ; Pelure ne doit livrer que ses propres textures.
