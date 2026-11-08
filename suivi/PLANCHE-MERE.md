# La planche-mère : un grand dessin, plusieurs cadres, plusieurs plans (conception, 2026-11-08)

Idée 4 d'Anthony (7 novembre) : « un grand dessin, plusieurs cadres caméra, donc plusieurs plans partageant ce dessin ; un plan se détache à la
demande ». Sa consigne du 8 novembre : « Ce doit être une option de plan (un plan réutilisé) ou de calque (un décor réutilisé). » Règle posée le
même jour dans les Vignettes : **les cases et les caméras ne coupent jamais le dessin**. Rien n'est codé : ce document est à valider.

## Ce qui existe déjà et sert de base

- **Le calque de toute la scène** (`Layer::scene_wide`, partie 6f, `Document::sync_scene_layers`) : chaque panneau d'une scène a son exemplaire
  d'un calque, réglé pareil, avec **les mêmes dessins** (les mêmes numéros de dessin : retoucher l'un, c'est retoucher tous), coupés à la durée
  du panneau, le dernier tenu. `set_scene_wide(false)` **détache** : chaque panneau garde une copie à lui. C'est exactement « un dessin partagé par
  plusieurs plans », limité aujourd'hui aux panneaux d'une même scène, et sans interface visible.
- **Le calque Décor des Vignettes** (`decor.rs`) : un dessin tenu sur tout le plan, donc partagé par ses cases ; porté au plan suivant en
  continuité par copie. Un seul plan.
- **La caméra est à chaque plan** (`Panel::camera`) : deux plans qui partagent un dessin le cadrent chacun à leur façon, fixes ou en mouvement.
  C'est ce qui fait « plusieurs cadres caméra ».

## Ce que je propose

Un seul mécanisme, deux portes d'entrée (les deux options d'Anthony).

### Le mécanisme : le partage d'un calque entre des plans choisis

- Un calque porte une **clé de partage** (`Layer::partage: Option<u64>`, qui généralise `scene_wide` en enlevant la limite de la scène). Tous
  les calques du film qui portent la même clé montrent **les mêmes dessins** (mêmes numéros de dessin), réglés pareil (nom, sorte, opacité,
  mode de fusion, effets), chacun **dans son plan, sous la caméra de son plan**. Un dessin de la toile infinie (plus grand que la feuille) est
  donc cadré autrement par chaque plan : c'est la planche-mère.
- **Dessiner** dans l'un des plans, c'est dessiner dans tous (c'est le même dessin) ; les vignettes des autres plans se refont par la version du
  dessin, comme le Décor.
- **Détacher** un plan : son calque garde une **copie** des dessins et perd la clé ; les autres ne changent pas. Détacher le dernier lié : la
  clé disparaît d'elle-même.
- **La durée** : les dessins partagés suivent les clés du modèle (le plan qu'on édite), coupés à la durée de chaque plan, le dernier tenu
  (comme le calque de toute la scène). Dans l'usage de la planche-mère, c'est un seul dessin tenu : rien à régler.
- **Ce qui ne bouge pas** : la caméra, la durée, les marques (cases), les notes, l'action et le dialogue, les clés de position et d'opacité
  **du calque** restent à chaque plan (un plan peut faire glisser son exemplaire sans toucher aux autres : le dessin est partagé, pas sa pose).
- **Le fichier** : un champ `partage` de plus sur le calque ; format de projet 51 (une version d'avant perdrait le lien à l'enregistrement, elle
  refuse). Couper un plan garde le lien des deux parties ; réunir deux plans liés garde le lien ; la corbeille, les versions et les plans
  écartés gardent la clé avec le calque (restaurer rend le lien si d'autres l'ont encore).

### Porte 1 — option de calque : « Décor réutilisé »

Dans le panneau Calques (Storyboard et Animation), clic droit sur un calque : **« Réutiliser ce calque dans… »** → **le plan suivant**, **les
plans choisis…** (une petite liste à cocher), **tous les plans de la séquence**. Le calque reçoit un **maillon** dans sa ligne (bulle : « Dessin
partagé avec les plans 12, 13 et 15 ») ; les plans visés reçoivent leur exemplaire (en bas de leur pile, comme un décor, ou au rang du modèle).
Clic droit sur un exemplaire : **« Détacher (garder une copie) »**. Le calque Décor des Vignettes peut l'être aussi : c'est alors le décor
d'une suite de plans sans coupe franche.

### Porte 2 — option de plan : « Plan réutilisé »

Dans la liste des plans du Storyboard (clic droit, Fiche du plan) et plus tard dans les Vignettes : **« Réutiliser ce plan »** crée juste après
un **nouveau plan dont tous les calques de dessin sont liés** à ceux du plan d'origine (même mécanisme, un partage par calque), avec **sa propre
caméra** (au départ, celle de l'origine : on la déplace ensuite), sa durée, ses textes vides. On fait ça trois fois, on bouge trois caméras : quatre
plans sur une seule planche-mère. **« Détacher ce plan »** détache tous ses calques d'un coup (le plan devient un plan ordinaire avec des copies).
Un plan réutilisé porte le maillon à côté de son numéro (Storyboard, Vignettes).

### Dans les Vignettes (après la fusion de `vignettes-selection`)

- Les cases de plans liés portent le maillon ; un **trait dans une case d'un plan lié va entièrement au dessin partagé** (il n'est pas réparti
  aux cases voisines par `horschamp`) : la règle « les cases ne coupent jamais le dessin » vaut d'autant plus que le dessin est, par construction,
  plus grand que chaque case.
- Une case de plan lié montre le dessin **à travers sa caméra** ; le **passe-partout** laisse voir ce qui déborde, comme pour un mouvement.
- Plus tard, si Anthony le veut : **dessiner la planche-mère d'un coup** dans les Vignettes (une case « mère » plus large, puis poser des cadres
  dedans qui deviennent des plans), ce que fait déjà en partie une case de mouvement de caméra (cadres A, B…) : « un cadre = un plan » serait
  une commande sur une case de mouvement : « Faire un plan par cadre ».

## Ce que ça ne fait pas (pour rester petit)

- Pas d'objet « planche » à part dans le projet : le dessin partagé vit dans ses calques, comme aujourd'hui pour la scène. Rien ne change au
  rendu, aux exports, au film.
- Pas de partage de la pose (clés de position) ni de la caméra : chaque plan cadre et anime à sa guise.
- Pas de partage entre deux projets.

## Ordre de travail proposé

1. Le modèle et sa synchronisation (généraliser `scene_wide`), détacher, enregistrer et relire (format 51), couper et réunir, corbeille ; essais.
2. Porte 1 (clic droit du calque, maillon, liste des plans à cocher) et porte 2 (« Réutiliser ce plan », « Détacher ce plan »), Storyboard et
   Animation ; essais à l'écran.
3. Vignettes : maillon sur les cases, trait entier au dessin partagé, passe-partout — après la fusion de `vignettes-selection` (fichiers
   `boardui.rs`, `horschamp.rs`, `annotations.rs` tenus par l'autre session jusque-là).
4. Plus tard : « Faire un plan par cadre » sur une case de mouvement.

## Les réponses d'Anthony (2026-11-08)

1. « Réutiliser ce plan » lie **tout le plan** (décor, personnages, traits) ; on détache ensuite ce qu'on veut.
2. La planche-mère se dessine d'abord dans le **Storyboard** (toile infinie, caméra reculée) ; les Vignettes viennent à l'étape 3.
3. **« Réutiliser »** dans les commandes. Le mot « planche-mère » ne sert que dans les bulles des commandes et la ligne d'état, pour nommer
   la chose partagée : **rien de nouveau dans le menu Aide**, déjà bien rempli (Anthony).
