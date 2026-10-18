# Pelure — un trait vectoriel qui ressemble trait pour trait au bitmap

Étude du 2026-10-18 (branche `audit-rendu`), à la suite de l'audit du rendu
(`docs/AUDIT-RENDU-BROSSES.md`). Demande d'Anthony : « trouver comment retrouver un résultat
bitmap également sur le vectoriel ; élaborer toutes les pistes possibles pour avoir un vectoriel qui
ressemble trait pour trait à du bitmap ».

Les planches citées sont celles de l'audit (`audit-rendu/`, refaites par
`cargo test --release --offline --target-dir target\audit planches_rendu_audit -- --ignored --nocapture`) :
chaque planche a une rangée « bitmap » et une rangée « vectoriel » pour le même geste, et
`mesures.txt` donne l'écart entre les deux (alpha moyen |Δ|, et part des pixels qui diffèrent de
plus de 16/255).

## 1. L'existant, vérifié

### 1.1 Ce qu'un trait vectoriel garde

`vector::Element::Stroke { points: Vec<VPoint { x, y, r }>, color, opacity, hardness, brush:
Option<Box<BrushSettings>> }`.

- `points` : la **ligne médiane déjà lissée** (ce que le lissage a produit pendant le geste, point
  par point : `StrokeState::record` à chaque `move_to`) avec, en chaque point, le **rayon** (taille
  × facteur d'appui, plancher compris). L'appui lui-même n'est pas gardé, ni l'instant, ni la
  vitesse. Au lâcher, `commit_vector_stroke` simplifie à 0,25 px (Douglas-Peucker en x, y, r).
- `brush` : le réglage complet, **seulement** si le trait est texturé (pointe en image, papier
  avec grain, ou profil : `BrushLibrary::for_vector`). Sinon rien : le Pinceau rond, l'Encre, le
  Crayon de la barre, l'outil TVPaint d'Anthony, les brosses humides ou à accumulation sans papier
  ne gardent que couleur, opacité et dureté.
- Pas de graine du hasard, pas de toucher (`Toucher`), pas de couleur B, pas de zoom.

### 1.2 Comment il est rendu

- Sans `brush` : `Mask::stamp` le long des segments, un disque tous les 0,25 × rayon, couverture
  `coverage` (l'**ancienne** : rampe d'un pixel coupée par la dureté, voir
  `SENSATION-BROSSES.md` §2.4), `max` dans un masque, puis `paint` à l'opacité du trait.
- Avec `brush` : `paint_textured` — un `StrokeState` neuf sur une feuille pleine taille, sans
  instants (vitesse = 0), sans lissage, toucher par défaut, appui **reconstitué d'après le rayon**
  (`(r / (taille/2) − mini) / (1 − mini)`) seulement si « taille selon l'appui » est la connexion
  simple ; avec une connexion de la dynamique (toutes les brosses importées), l'appui vaut 1. Puis
  la feuille est posée (`over`) dans la région demandée.
- Où : `Cel::rerender` (tout le dessin), `rerender_region` (les tuiles d'une zone, éléments dans
  l'ordre), `render_on_top` (le dernier élément posé sur les tuiles d'avant l'aperçu — c'est le
  chemin du lâcher ordinaire). Pendant le geste, l'aperçu est un **vrai trait bitmap** (même
  `StrokeState` que sur un calque bitmap, avec toucher, zoom, couleur B, options) ; au lâcher, ses
  tuiles sont remises et l'élément vectoriel est rendu par-dessus.
- Édition : `transformed` (affine : positions, rayons × √|det|, `brush.size` × idem) ;
  `erase_circle` (densifie à 0,3 × rayon, coupe, garde `brush`) ; cutter (coupe aux croisements) ;
  `tween_vector` / `morph` (rééchantillonne 2 à 512 points, interpole x, y, r, couleur, opacité,
  dureté, garde le `brush` du premier) ; `scale_width`.
- Format : `Element` en JSON avec `#[serde(tag = "type")]`, `brush` en `#[serde(default)]`,
  textures du projet dans `textures/` par clé de contenu ; `project::VERSION` 23.

### 1.3 Ce qui diffère, en images

| Ce qui change | Planche | Écart bitmap / vectoriel (`mesures.txt`) | Cause |
|---|---|---|---|
| Bord du trait | `12_vecto_bord.png`, `02` | pinceau rond : 27–33 d'alpha moyen, 31–39 % des pixels | ancienne `coverage` contre `coverage_soft` ; `thin_full`, plancher |
| Opacité selon l'appui | `01_pelure_crayon-barre.png` | 39–48 ; 72–80 % | perdue (pas de `brush`, le masque est à opacité pleine) |
| Connexions (appui → taille, opacité, hasard d'angle…) | `30_tvpx_anthony_trait.png`, `43_tvp_2b-pencil.png`, `48_tvp_pen-1.png` | 122–176 ; 92–100 % (Trait) ; 68–105 (2B) | appui reconstitué = 1 avec une dynamique ; hasard retiré à partir de la même graine mais pas le même nombre d'empreintes |
| Peinture humide (mélange, charge) | `22_essai_gouache.png` | 101–156 ; 88–94 % | perdue (pas de `brush`) |
| Débit et accumulation sans papier | (pas de planche : `20` a un papier, donc gardé) | — | perdus sans `brush` |
| Grain, papier, pointe en image, profil | `04`, `05`, `20`, `21`, `44` | 1–5 ; 0–8 % | gardés (`brush`), petit écart : `paint_textured` sans instants ni toucher du moment |
| Pointes animées « à la suite » | `50_tvp_chinese-brush.png` colonne B | 53–61 | le numéro de case dépend du nombre d'empreintes, différent au rejeu |
| Lissage | toutes | 0 | la ligne médiane gardée est déjà lissée |
| Simplification 0,25 px | `02`, `12` | noyé dans l'écart du bord | sous-pixel, invisible à 100 % ; visible au ×4 sur les rayons (marches de 0,25 px) |
| Vitesse (connexion « vitesse ») | aucune brosse livrée | — | perdue (pas d'instants) |

Ce que ça veut dire pour Anthony : sur son calque par défaut (vectoriel), un crayon ou une brosse
importée change d'aspect au moment où il lève le stylet. Ce n'est pas la cause du rendu
« numérique » (l'audit le montre en bitmap), mais ça empêchera de juger les nouvelles brosses sur
un calque vectoriel tant que ce n'est pas réglé.

## 2. Toutes les pistes

Pour chacune : le principe, ce qu'elle garde du vectoriel (trait modifiable, interpolation, mise à
l'échelle), ce qu'elle perd, coût, risque, réversibilité, et comment vérifier « trait pour trait »
(la mesure `ecart` de l'essai d'audit : alpha moyen |Δ| et part des pixels à plus de 16 d'écart,
qui doit tomber à 0 ; 0,0 pour la même image).

### P1. Rejouer le moteur bitmap depuis le chemin gardé, avec le réglage complet

- **Principe** : garder dans l'élément ce qui manque au rejeu exact : l'**appui** et l'**instant**
  à chaque point du chemin (`VPoint { x, y, r, p, t }`), le **réglage complet** toujours
  (`brush: BrushSettings`, pas seulement si texturé), la **graine** du hasard, le **toucher** du
  moment (plancher, bord, trait fin ; la courbe de pression et le seuil sont déjà appliqués avant
  `record`), la couleur B et les options (peint derrière). Rendre = un `StrokeState` qui reçoit les
  points du chemin **directement par `move_to`** (sans repasser par le lissage : le chemin gardé
  *est* la suite des cibles lissées que `move_to` a reçues pendant le geste), avec les instants
  pour la vitesse. Le moteur est déterministe (graine fixe `RNG_SEED`, grain ancré à la feuille,
  masque f32 recalculé depuis `before`) : le banc de traits le prouve déjà (`recorded_strokes_replay`
  rend la même image qu'à l'écran).
- **Garde** : tout du vectoriel — le chemin reste la ligne médiane (sélection, déplacement,
  rotation, gomme qui coupe, cutter, tween travaillent sur les mêmes points qu'aujourd'hui).
- **Perd** : rien de visible. La simplification 0,25 px est à retirer du chemin de rendu (elle
  déplace les empreintes) et à garder seulement pour la prise (`hit`) et l'affichage des
  poignées — ou à faire à 0,05 px.
- **Exactitude** : trait pour trait, par construction, dès que le rejeu passe par `move_to` avec
  le même état de départ. Deux exceptions à traiter : le **rattrapage** du levé (`catch_up`) produit
  des points qu'il faut garder dans le chemin (déjà fait : `record` est appelé) ; le **lissage
  différé** rejoue le journal brut — le chemin final est celui du rejeu, bon.
- **Coût** : 1 j pour le format et le rendu, 1 j pour les tests (l'essai d'audit a déjà la mesure).
  Mémoire : un point = 20 octets (x, y, r, p, t) au lieu de 12 ; un trait = 200–2 000 points ; un
  dessin de 500 traits ≈ 2–20 Mo de JSON en plus du réglage par trait (~ 1,5 Ko chacun, à
  dédoublonner par une table de réglages par dessin si besoin). Temps de rendu : celui du moteur
  bitmap, 1–5 ms par trait moyen, 20–50 ms pour une grosse brosse texturée ; un `rerender` complet
  de 500 traits = secondes → il faut P2 pour l'édition.
- **Risque** : moyen. Le rejeu doit reproduire l'**état** du trait (empreintes déjà posées,
  `since_dab`, charge humide, case de pointe animée, hasard) : un morceau coupé repart d'un état
  neuf (première empreinte posée pile au bout, charge pleine) — voir P4 pour l'état de départ.
- **Réversible** : oui. Les anciens traits (sans `p`, `t`, `brush`) restent rendus comme
  aujourd'hui ; `#[serde(default)]` ; `VERSION` 24 pour prévenir qu'une version d'avant ignorerait
  `p`/`t`.

### P2. Un cache bitmap par trait

- **Principe** : chaque élément garde en mémoire (pas dans le fichier) les tuiles qu'il a rendues
  (`Option<Vec<(usize, Tile)>>`, ou un masque de couverture u8 + couleur pour les traits sans
  peinture humide). Il est refait quand le trait change (édition, transformation, tween) ; la
  composition d'une zone = poser les caches des éléments dans l'ordre. `rerender_region` ne rend
  plus, il compose.
- **Le cas le plus rentable, à faire d'abord** : au lâcher, **garder les pixels de l'aperçu** —
  le trait bitmap dessiné pendant le geste est déjà là, exact ; `render_on_top` cesse de
  redessiner et l'élément prend ces tuiles comme cache. Trait pour trait au prix de zéro rendu.
- **Garde** : tout (c'est un accélérateur de P1, pas un format).
- **Perd** : de la mémoire : une tuile RGBA 64×64 = 16 Ko ; un trait de 300 px sur 10 px touche
  ~10 tuiles = 160 Ko ; 500 traits ≈ 80 Mo ; en masque u8 (4 Ko/tuile), 20 Mo. À plafonner (cache
  des N traits les plus récents / visibles, les autres refaits à la demande).
- **Coût** : 1–2 j. **Risque** : faible (cache invalidé par toute modification de l'élément, un
  test par opération). **Réversible** : oui (rien dans le fichier).
- **Vérification** : la mesure `ecart` à 0 après chaque opération (déplacer puis revenir, couper
  puis annuler) ; un test qui compare `rerender` complet et composition des caches.

### P3. Rendre les empreintes sur la carte graphique

- **Principe** : porter `dab` en WGSL (empreinte ronde / profil / pointe en image en texture,
  papier en texture, masque de trait en texture de stockage, `max` ou accumulation) ; un trait =
  un passage de calcul ; la peinture humide (lecture de la feuille sous l'empreinte) force
  l'ordre, donc des passages séquentiels.
- **Garde** : tout si le calque bitmap passe aussi par là ; sinon le vectoriel GPU diffère du bitmap
  CPU (arrondis flottants, interpolation des textures) : **pas** trait pour trait avec l'existant.
- **Coût** : 10 jours et plus (deux moteurs à tenir identiques, ou un seul à basculer entièrement) ;
  Pelure a déjà la composition des calques sur wgpu (`gpu.rs`), pas les empreintes.
- **Risque** : élevé ; **réversible** : difficile une fois les deux rendus divergents.
- À garder pour plus tard, quand P1 + P2 ne suffiront plus (très grands dessins, pinceaux énormes).

### P4. Variante de P1 : rejouer depuis les échantillons bruts

- **Principe** : garder le journal brut (`raw_log` : x, y, appui brut, instant) au lieu du chemin
  lissé, et rejouer tout le trait (toucher, lissage, rattrapage) au rendu.
- **Plus** : le lissage devient modifiable après coup (« lisser ce trait davantage ») ; le lissage
  différé n'a rien de spécial.
- **Moins** : le rendu dépend du zoom du moment (corde en pixels d'écran) et du toucher global —
  à geler dans l'élément ; la ligne médiane pour la prise et les poignées doit être recalculée (ou
  gardée en double) ; le tween entre deux journaux bruts n'a pas de sens direct (il faut
  interpoler les chemins lissés, puis rejouer les empreintes depuis le chemin — donc P1 quand même).
- **Verdict** : P1 avec, en option, le journal brut gardé *en plus* pour les traits qu'on voudra
  relisser ; pas comme base du rendu.

### P5. L'état de départ d'un morceau (gomme, cutter, tween)

- Un trait rejoué par `move_to` dépend de son état au départ : `since_dab` (phase des
  empreintes), nombre d'empreintes posées (case d'une pointe « à la suite »), charge humide,
  longueur parcourue (fondu), hasard. Quand la gomme ou le cutter coupe un trait en morceaux,
  chaque morceau peut **garder l'état du trait d'origine à son premier point** (un petit
  `Depart { dabs, since_dab, wet_load, travel, rng }`, obtenu en rejouant le trait d'origine jusqu'à
  la coupe, sans peindre — ou calculé à la coupe par le moteur en mode « à blanc »). Alors la
  coupe ne change pas un pixel ailleurs qu'au trou.
- Pour le **tween** : interpoler x, y, p, t point à point (comme `morph` aujourd'hui, en
  rééchantillonnant), garder la graine et le réglage du premier ; la phase des empreintes suit la
  longueur, donc le grain de la pointe « bouillonne » un peu d'une image à l'autre. Et le papier
  ancré à la feuille : un trait qui se déplace change de creux — c'est le comportement de TVPaint
  pour un calque déplacé, acceptable pour un storyboard ; sinon, option « grain lié au trait »
  (papier échantillonné dans le repère du trait : `paper_follow`), à proposer.
- Coût : 1 j ; risque faible ; réversible.

### P6. La transformation : mise à l'échelle, rotation

- Positions transformées, `brush.size × √|det|` (déjà fait), **angle de la pointe + angle de
  rotation** (à ajouter pour les pointes à angle fixe ; celles qui suivent la direction suivent
  d'elles-mêmes), `paper_angle` idem si « grain lié au trait », sinon le papier reste à la feuille.
  Le pas, la dispersion, le hasard de taille sont relatifs au diamètre : ils suivent. Une pointe en
  image agrandie au-delà de sa taille native s'interpole (flou) : c'est le prix de toute texture,
  le même en bitmap. Rien à perdre par rapport à aujourd'hui.

### P7. Le remplissage, « peint derrière », le pot, le cutter

- Le remplissage de forme (`brushfill`), le « peint derrière » (élément inséré en bas), le pot
  (`Element::Fill`), le texte : inchangés — ils ne touchent pas au rendu du trait.
- Le cutter et la gomme : P5. Le lasso et les poignées : le chemin reste la ligne médiane.
- Le trait « derrière » rejoué par `move_to` avec `behind` : `StrokeState::with_options` existe.

### P8. Compatibilité et format

- `VPoint` gagne `p` et `t` en `#[serde(default)]` (absents : `p` reconstitué comme aujourd'hui
  d'après `r`, `t` = −1 → vitesse nulle) ; `brush` toujours écrit (les anciens fichiers l'ont en
  option : absent → rendu d'avant, pas de changement d'aspect des vieux dessins) ; `seed`,
  `feel`, `secondary`, `behind` en `#[serde(default)]`. `project::VERSION` → 24, avec la ligne de
  journal d'usage dans `project.rs`. Le `Cel` bitmap, le fond, les exports SBP (`sbexport.rs`,
  qui lit `points` et la couleur) ne changent pas.
- Option de migration : « Moderniser les traits » (menu) qui donne aux anciens traits ronds le
  `brush` du Pinceau rond — à ne faire que sur demande, puisque ça change le bord (`12_vecto_bord`).

### P9. Les autres logiciels (public, « à vérifier » partout)

- **Toon Boom Storyboard Pro / Harmony** : la « textured vector brush » est un contour vectoriel
  rempli d'une **texture bitmap** à densité fixe (« the vector shapes your brush creates will be
  filled with a bitmap texture ») ; la densité se règle par calque (100 % par défaut, « set the
  pixel density of that layer to at least 400% before you start drawing ») ; modifier ou agrandir
  le trait **rééchantillonne** la texture (« Storyboard Pro will need to resample the textures,
  which will make it lose quality ») ; chaque trait texturé est stocké et composé à part, plus lourd
  qu'un bitmap ; le crayon texturé garde sa netteté à la transformation. C'est P2 (cache par trait)
  sans P1 (pas de rejeu exact) — et c'est ce qu'Anthony connaît dans SBP.
- **Clip Studio Paint** : le calque vectoriel garde des courbes (« recognizes each line you draw
  as a little equation ») ; l'outil Objet change après coup « the brush shape, color, size,
  texture, and blending mode » (donc le rendu est **rejoué** depuis le chemin avec la brosse, comme
  P1) ; certaines options ne marchent pas sur vectoriel (le bord d'aquarelle, la double brosse) ;
  à l'export, « vector layers will be considered rasters ».
- **TVPaint** : bitmap seulement, pas de calque vectoriel. **Procreate** : bitmap seulement.
  **Krita** : les calques vectoriels sont des formes SVG, sans moteur de brosse. **Adobe Animate** :
  les « art / pattern brushes » répètent un motif vectoriel le long du chemin, pas de texture
  bitmap par empreinte.
- Conclusion : personne ne fait « trait pour trait » par magie ; ceux qui approchent (Clip Studio)
  rejouent la brosse sur le chemin, ceux qui cachent (Toon Boom) acceptent le rééchantillonnage.
  P1 + P2 fait les deux.

### Tableau

| Piste | Trait pour trait | Modifiable / tween / échelle | Coût | Risque | Réversible |
|---|---|---|---|---|---|
| P1 rejeu depuis le chemin + réglage complet | oui | oui / oui / oui | 2 j | moyen (état de départ, P5) | oui |
| P2 cache par trait (aperçu gardé au lâcher) | oui | oui | 1–2 j | faible | oui |
| P3 empreintes sur GPU | non (sauf tout basculer) | oui | 10 j + | élevé | difficile |
| P4 rejeu depuis le brut | oui | tween difficile | 2 j + | moyen | oui |
| P5 état de départ des morceaux | (complète P1) | oui | 1 j | faible | oui |
| P6 transformation (angle de pointe) | (complète P1) | oui | 0,5 j | faible | oui |
| Calque bitmap par défaut (hors sujet mais possible) | oui | non / fondu seulement / non | 0 | nul | oui |

## 3. Recommandation et plan

**Faire P1 + P2 (dans cet ordre : P2-aperçu d'abord, c'est gratuit), puis P5 et P6.** Le vectoriel
de Pelure devient « un trait bitmap dont on garde la recette » : le moteur est le même, les pixels
sont les mêmes, et le chemin reste modifiable. C'est aussi ce qui rend l'audit du rendu utile sur le
calque par défaut.

| # | Étape | Durée | Ce qu'Anthony verra | Test A/B |
|---|---|---|---|---|
| 1 | **Au lâcher, garder l'aperçu** : `commit_vector_stroke` ne redessine plus l'élément ; les tuiles de l'aperçu deviennent son cache (P2-aperçu). L'élément garde dès maintenant `brush` complet, `p`, `t`, graine, toucher (P1 format) | 1 j | plus aucun changement au lever du stylet, quelle que soit la brosse | même geste sur calque bitmap et vectoriel : identiques (mesure `ecart` 0) ; un trait de crayon, un trait TVPaint importé, un trait humide |
| 2 | **Rendu par rejeu** (P1) : `render_element` rejoue `move_to` avec l'état gardé ; l'ancienne `Mask::stamp` ne sert plus qu'aux traits sans `brush` (vieux fichiers) ; simplification retirée du chemin de rendu | 1–2 j | un trait déplacé, agrandi, tourné, interpolé garde sa matière ; l'annulation/rétablissement aussi | déplacer un trait puis Ctrl+Z : identique au pixel ; tween entre deux dessins : le grain tient |
| 3 | **Cache par trait pour l'édition** (P2) : tuiles gardées par élément, composition dans `rerender_region`, plafond mémoire | 1–2 j | l'édition d'un gros dessin vectoriel reste fluide | 500 traits texturés : déplacer un trait < 1 image ; mémoire mesurée |
| 4 | **État de départ des morceaux** (P5) et angle de pointe à la rotation (P6) | 1 j | couper un trait ne change rien ailleurs qu'au trou | gomme sur un trait à pointe animée, cutter sur une craie : hors du trou, écart 0 |
| 5 | Option « grain lié au trait » pour le tween, et « Moderniser les traits » des vieux dessins | 0,5 j | au choix | tween d'un trait à papier : bouillonne ou non |
| 6 | Plus tard : empreintes sur GPU (P3) si la taille des projets l'exige | — | — | — |

Format : `VERSION` 24, `#[serde(default)]` partout, vieux dessins inchangés à l'écran. Rien de ce
plan n'est commencé sur la branche (étude seulement) ; l'essai d'audit fournit la mesure d'écart
qui servira de test de non-régression (« écart 0 »).

## 4. Questions pour Anthony

1. Édites-tu vraiment les traits vectoriels après coup (lasso, échelle, rotation, cutter), ou
   surtout gomme + annuler ? Si c'est surtout gomme + annuler, l'étape 1 suffit presque.
2. Interpolation (tween) entre deux dessins : avec des traits texturés aussi ? Acceptes-tu que le
   grain « bouillonne » légèrement pendant le mouvement, ou veux-tu le grain lié au trait ?
3. Un dessin vectoriel peut peser plus (appui + instant par point, réglage par trait, caches en
   mémoire) : une limite à respecter ? (Projet type : combien de panneaux, combien de traits par
   panneau ?)
4. Les vieux dessins doivent-ils garder leur aspect actuel (oui par défaut), ou veux-tu les
   « moderniser » au bord doux ?
5. Si l'étape 1 te convient, veux-tu qu'elle passe avant les pointes et papiers de l'audit (plan
   §5 de `AUDIT-RENDU-BROSSES.md`), pour juger les nouvelles brosses directement sur ton calque
   vectoriel ?
