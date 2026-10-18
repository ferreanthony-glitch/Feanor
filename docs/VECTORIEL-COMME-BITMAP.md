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

Réponse d'Anthony (2026-10-18) : **il ne modifie pas ses traits vectoriels après coup**. Cela
simplifie tout : l'étape 1 ci-dessous (garder les pixels de l'aperçu au lâcher) donne déjà le
trait pour trait sur son calque par défaut, sans rejeu ; le rejeu (étape 2) ne sert plus qu'aux
rares éditions, à l'interpolation et à la mise à l'échelle, et peut attendre. Fait en passant avec
les étapes 1 à 3 du rendu : `vector::pressure_for_radius` retrouve l'appui d'un point pour toute
connexion de taille (dichotomie), si bien que les brosses à connexion (son « Trait » importé)
rendent déjà pareil en bitmap et en vectoriel quand elles sont texturées (planche `30`).

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

Format : `VERSION` 24, `#[serde(default)]` partout, vieux dessins inchangés à l'écran.

## 3 bis. Fait (branche `vecto`, 2026-10-18)

Anthony : « ensuite, l'étape 5 », avec en priorité le déplacement d'une sélection vectorielle, qui
« ralentit énormément » (mesuré à 107–109 ms par image pour 12 traits de graphite).

### Ce qui est fait

- **La trace** (`vector::Trace`, dans `Element::Stroke { trace }`, `#[serde(default)]`) : l'appui
  (après la courbe de pression) et l'instant de chaque point du chemin, le toucher du moment et la
  couleur B. Le moteur les enregistre pendant le geste (`StrokeState::path_pressure`,
  `path_time`) ; `commit_vector_stroke` les garde avec le réglage complet (`for_vector` rend
  maintenant toujours le réglage, texturé ou non) et **ne simplifie plus le chemin** (la
  simplification déplaçait les empreintes). Pas de graine à garder : elle est fixe (`RNG_SEED`).
- **Le rendu par rejeu** (`render_stroke_alone`) : un `StrokeState` sans lissage reçoit les points
  du chemin — qui sont déjà les cibles lissées que le moteur a reçues pendant le geste — avec leur
  appui et leur instant, le toucher (sa courbe et son seuil ont déjà joué), la couleur B. Mêmes
  empreintes, même ordre, même hasard : **trait pour trait**. `mesures.txt` : écart bitmap /
  vectoriel **0,0** sur toutes les brosses livrées, sur le « Trait » TVPaint d'Anthony et sur les
  panneaux TVPaint importés (planches `01` à `11`, `30`, `40` à `57`). Un trait sans appuis (vieux
  fichier, trait coupé, interpolé) retrouve l'appui d'après ses rayons (`pressure_for_radius`) ;
  un trait sans réglage (vieux fichier, formes) garde le trait lisse d'avant.
- **Le cache de pixels** (`StrokeCache`, `CachedPixels`) : au premier rendu, le trait rendu seul
  est gardé en **tuiles creuses de 64 × 64** (celles qu'il touche : un trait fin en diagonale de la
  feuille coûte ses tuiles, pas sa boîte), avec une signature de ce qui fait ses pixels (couleur,
  opacité, dureté, réglage, trace) ; les rendus suivants posent ces pixels (`blit_cache`). Le cache
  est partagé entre les copies du trait (annulation, aperçu en cours) ; un trait dont la signature
  a changé est rerendu ; `set_color`, `scale_width`, `invalidate_cache` et **toute transformation**
  (`Element::transformed`) le vident. Plafond : 1024 tuiles (16 Mo) par trait, au-delà le trait est
  rejoué à chaque rendu ; **budget global** 512 Mo (`CACHE_BUDGET_BYTES`), au-delà les pixels les
  moins récemment posés sont rendus (registre des caches vivants, `Registry`). Pas dans le fichier.
- **L'aperçu d'un geste** : `Element::transformed_preview` garde le cache et compose la
  transformation (`xf`) ; une translation est **arrondie au pixel** (copie nette, 8 fois moins
  chère que le rééchantillonnage), échelle et rotation sont rééchantillonnées depuis le cache (un peu
  floues pendant le geste). Au lâcher, l'aperçu reste affiché et le **rendu exact arrive d'un fil de
  travail** (`rejeu.rs`), voir §3 ter.
- **Format** : `project::VERSION` 24 ; un fichier d'avant se relit (trace vide) ; une version
  d'avant refuse un fichier d'après (comme pour tout nouveau format). Dans le fichier, `appui` est
  en entiers sur 16 bits et `instants` en dixièmes de milliseconde depuis le premier ; un trait
  sans trace n'écrit pas le champ.
- Gomme vectorielle, cutter, enveloppe, interpolation : les morceaux et les traits refaits
  **gardent leurs appuis et leurs instants, interpolés** aux coupes et aux points ajoutés
  (`Trace::sample`, `densify_traced`, `piece_traced`) ; les traits non touchés ne bougent pas d'un
  point. Perspective, export Storyboard Pro : la trace suit (`without_cache`). Copier-coller et
  enregistrement : la trace suit.

### Mesures (`vecto_tests::vector_selection_drag_is_fast_with_the_pixel_cache`, essai ignoré)

50 traits de Crayon graphite sur 1920×1080, tous sélectionnés et déplacés image après image :
- avant (chaque image rejoue le moteur, comme `select_drag` le faisait) : **103,9 ms par image** ;
- après (pixels en cache copiés) : **3,9 ms par image** (1,9 ms avec le cache en tuiles, §3 ter) ;
- premier rendu des 50 traits (rejeu, une fois) : 101 ms ;
- mémoire du cache : **101 Ko par trait**, 5,0 Mo pour les 50 (131 Ko par trait en tuiles).

Tests : `vecto_tests.rs` — rendu vectoriel identique au bitmap pour graphite, encre, gouache,
feutre, fusain, pinceau rond, brosse à deux couleurs (et depuis le cache) ; un trait déplacé est
son image décalée pendant le geste, rejoué exact au lâcher ; les anciens traits, les morceaux
coupés, le relire après enregistrement ; un trait recoloré oublie son cache. 586 tests passent.

### Ce qui reste

- **Peinture humide entre traits** : chaque trait est rejoué seul, la gouache ne se mêle plus à ce
  que les traits d'avant ont posé sur le calque (écart 4 d'alpha sur 11 % des pixels aux
  croisements des hachures, planche `10_pelure_gouache.png`). Pour l'avoir, il faudrait rejouer
  sur le composite des traits d'avant — incompatible avec un cache par trait ; à décider si la
  gouache compte sur un calque vectoriel.
- **Connexion à la vitesse** : rejouée depuis les positions lissées, pas brutes (vitesse un peu
  différente) ; aucune brosse livrée ne s'en sert.
- **Rotation et échelle pendant le geste** : rééchantillonnées (floues) jusqu'au lâcher ; le
  rendu exact revient quelques instants après le lâcher (fil de travail).
- **Morceaux** (gomme, cutter) : gardent leurs appuis, mais repartent d'un état neuf (phase des
  empreintes, charge) — P5 du plan, pas fait. Pendant le geste de la gomme, les morceaux coupés
  sont rejoués sur le fil de l'interface (quelques traits, ≈ 2 ms chacun) : à passer en aperçu
  (cache du trait parent moins le disque) si ça se sent.
- **Relecture** : les appuis au 1/65535 et les instants au dixième de milliseconde font, à la
  relecture, un écart d'un niveau sur quelques pixels isolés (invisible).

## 3 ter. Revue, et règle « sans latence » (branche `vecto`, 2026-10-18)

Une revue de la branche a confirmé 23 constats (cache jamais invalidé hors du glissé, gomme qui
simplifiait les traits voisins et faisait sauter l'opacité des morceaux, mémoire sans limite,
chargement ralenti, glissé non entier huit fois plus lent, empreintes hors de la boîte, traits de
plus de 2 M px sans cache, fichier gonflé, papier partagé changé, doc du format fausse). Puis
Anthony, après essai : « on a un vrai souci de ralentissement avec les brosses vecto quand elles
sont enveloppées et qu'on veut les déplacer. Ma priorité est d'avoir un logiciel qui réponde
toujours très rapidement, sans latence ».

**Règle de conception (appliquée partout)** : pendant un geste (glisser, enveloppe, perspective,
échelle, rotation, flèches), **jamais de rejeu du moteur sur le fil de l'interface**. On montre les
pixels en cache déformés à la volée ; le rendu exact (le rejeu) se fait dans un fil de travail
(`rejeu.rs`) et remplace l'aperçu quand il est prêt — au lâcher non plus, rien ne bloque.

Ce qui a été fait :
- **Gomme, cutter, enveloppe, intervalles** : les morceaux gardent leurs appuis et instants
  (interpolés) ; plus de `simplify_strokes_in` sur les traits voisins ; une brosse dont l'opacité
  seule suit l'appui garde son toucher léger sur ses morceaux.
- **Invalidation** : `Element::transformed` vide le cache (agrandir la feuille, retourner un dessin,
  changer la résolution, vieux projets) ; seul `transformed_preview` le garde (glissé, flèches).
- **Mémoire** : cache en tuiles creuses, plafond 1024 tuiles par trait, budget global 512 Mo
  (éviction du moins récent) ; les copies de l'historique partagent les caches (`Arc`).
- **Glissé** : translation arrondie au pixel ; échelle et rotation posées depuis le cache (la
  signature n'est pas vérifiée sur un aperçu, qui est par construction le trait rendu transformé).
- **Boîte** : `bounds()` compte √2 − 1 de rayon en plus pour une pointe en image (tournée) ; le
  rendu d'un trait est borné à `bounds() + 1,5` : refaire une zone = rendu complet.
- **Enveloppe et perspective** : pendant le geste, la feuille des éléments choisis (posée depuis
  les caches, `render_split`) est déformée par le maillage (`envelope::warp_raster_fast` :
  projection au plus proche voisin par blocs, grille de 8 px, coût selon les pixels dessinés) et
  posée sur le reste ; au lâcher, la géométrie seule et la demande de rejeu. Bitmap : même aperçu,
  résultat exact (bilinéaire) au lâcher.
- **Lâcher d'un glissé, flèches** : l'aperçu reste, le rejeu exact arrive en arrière-plan
  (`App::rejeu_cel`, `App::poll_rejeu` : adopté seulement si le dessin n'a pas changé entre-temps ;
  un rendu direct ou une annulation rend la demande caduque).
- **Chargement** : `project::load_quick` affiche tout de suite les dessins vectoriels texturés en
  rendu rapide (traits lisses) et les liste pour le rejeu en arrière-plan ; `project::load` reste
  exact (essais, outils).
- **Fichier** : `appui` en u16, `instants` en entiers relatifs ; champ `trace` omis s'il est vide.
- **Papier** : les crayons ont leur « Papier à dessin fin » (`PAPER_GRAPHITE` = 6) ; le « Papier à
  dessin » (3) a retrouvé son relief des étapes 1 à 3 (fusain, gomme à grain, réglages enregistrés).

### Mesures (`revue_tests.rs`, essais ignorés, 1920 × 1080, 51 traits de graphite)

Scénario d'Anthony (`envelope_then_drag_is_fast_on_the_ui_thread`), avant (d01fffe) → après :

| Étape | Avant | Après (fil de l'interface) | Arrière-plan |
|---|---|---|---|
| Enveloppe en cours (1575 × 867 px) | rien d'affiché | **8,6 ms / image** (exact bilinéaire : 65 ms) | — |
| Lâcher de l'enveloppe | **113 ms** (rejeu) | **0,5 ms** | 95–102 ms |
| Glissé de la sélection enveloppée (pas de 3,4 × 2,6 px) | **44,9 ms / image** (bilinéaire) | **1,6 ms / image** | — |
| Lâcher du glissé | **118 ms** (rejeu) | **0,1 ms** | 92 ms |
| Une flèche (1 px) | 3,8 ms | **1,9 ms** | 92 ms |

Glissé de 50 traits (`vecto_tests::vector_selection_drag_is_fast_with_the_pixel_cache`) : 1,9 ms
par image (105,7 sans cache) ; cache 131 Ko par trait (tuiles). Chargement
(`loading_a_big_project_is_measured`, 24 dessins × 30 traits, 8,7 Mo de JSON) : exact **1,60 s** ;
rapide **0,28 s** affiché, puis 1,50 s de rejeu en arrière-plan. Fichier
(`saved_traces_are_quantized_and_much_smaller`) : par trait de 200 points, points 8,7 Ko, trace
2,35 Ko (3,25 Ko avant ; les instants de ce geste de synthèse sont courts, l'écart réel est plus
grand), réglage 1,27 Ko — les points et le réglage restent les gros postes (à quantifier / dédoubler
plus tard si la taille compte). 597 tests passent.

### Essai d'Anthony sur la feuille flottante (2026-10-18, soir) : deux constats corrigés

**Déplacement : des morceaux du dessin manquaient, coupés en ligne droite** (`affichage_tests.rs`).
Le cache de pixels d'un trait, comme la feuille flottante, était à la taille de la feuille : ce qui
dépassait de la feuille n'existait pas. Un trait qui en sortait par le haut ou la gauche, déplacé
vers l'intérieur, montrait donc un bord droit là où la feuille s'arrêtait (21 % des pixels peints
manquaient dans l'essai). Les fines lignes vues par-dessus sont les contours de sélection
(`canvas.rs`, le trait médian de chaque trait choisi, 1 px) : ils continuaient là où les pixels
manquaient. Maintenant un trait qui dépasse est rendu sur une image plus grande (marge de 1024 px au
plus par côté, 8192 px de côté au plus : limite des textures), le grain du papier lu comme sur la
feuille (`StrokeState::with_grain_offset`) : sur la feuille, mêmes pixels qu'avant (un niveau d'écart
sur quelques pixels du bord, l'arrondi des positions décalées). La feuille flottante est élargie de
ce qui dépasse (`render_split_extended`, `PlanItem::pre` devient `Pre { aff, dims }`, textures de la
carte graphique à la taille de l'image du dessin). Coût mesuré (40 gros traits de gouache, feutre et
fusain, 1920 × 1080, 11 dépassent) : saisie 11,0 ms contre 10,2 ms, une fois au début du geste ;
feuille 2039 × 1455 (6,0 Mo de tuiles) ; 0,001 ms par image. Aussi corrigé : terminer un geste
lâché avant l'arrivée du rendu exact (`apply_preview`) ne refaisait que la nouvelle place de la
sélection et la laissait en double à l'ancienne. Limites : l'enveloppe et la perspective
(`render_split`) n'avaient pas encore cette marge (elles l'ont, voir plus bas) ; la saisie d'une sélection dont des traits n'ont pas
de pixels en cache (budget dépassé, projet qui vient de s'ouvrir) les rejoue sur le fil de
l'interface, 20 ms par trait large (0,8 s pour 40) — d'où `App::warm_current_cel`, qui demande au
fil de travail les traits manquants du dessin regardé, une fois par version.

**Dessin : de la latence avec une brosse vectorielle texturée.** Le moteur ne ralentit pas avec la
longueur du trait (`live_textured_stroke_cost_per_frame`, 4 points par image d'écran, de 400 à 2400
points : 0,15 à 1,4 ms par image selon la brosse et la taille, sans croissance). Ce qui croît avec
la longueur était la pose au lever du stylet : le trait était rejoué en entier sur le fil de
l'interface (`vector_stroke_begin_and_commit_cost`, gouache de 40 px : 44 ms pour 600 points, 174 ms
pour 2400). Maintenant l'aperçu du geste reste, le dessin est copié (0,07 ms) et le rendu exact
vient d'un fil de travail (181 ms pour 2400 points, sans rien bloquer). Le fil ne rejoue plus tout
le dessin à chaque demande : les traits aux pixels exacts en cache (signature juste) les gardent,
seuls les aperçus déformés sont oubliés (`Rejeu::submit_with`) — un trait de plus coûte un trait.

### Essai d'Anthony, enveloppe et grosse sélection (2026-10-18, nuit) : deux constats corrigés

Essai avec des traits rouges texturés : « l'outil Transformer en mode Enveloppe rame encore », et un
ralentissement quand une centaine de longs griffonnages sont choisis au lasso, au repos comme en geste.

**Enveloppe et perspective** (`envelopetools.rs`, `perstools.rs`). Il n'y avait déjà aucun rejeu du
moteur pendant le geste, mais chaque image copiait le reste du dessin, projetait un à un les pixels de
la feuille des traits choisis (`warp_raster_fast`, sur le fil de l'interface) et renvoyait 470 à 510
tuiles (8 Mo) à la carte graphique ; la feuille n'avait pas non plus de marge (ce qui dépasse de la
feuille était coupé). Maintenant c'est la feuille flottante des déplacements (`flottante.rs`) : saisie
une fois depuis les pixels en cache du dessin d'origine de la séance (`Flottante::lift_from`, marge
comprise), puis seul un **maillage** change à chaque image (`warp.rs` : au plus 48 × 48 cases, chaque
point avec sa place dans la feuille et sa place d'arrivée). `gpu.rs` le dessine en triangles
(`vs_mesh`, mêmes fragments, modes de fusion et opacité compris, transformation du calque appliquée
après) ; `render.rs` fait le même dessin sur processeur (secours sans carte graphique). Au lâcher :
la géométrie, le rejeu en arrière-plan, et la feuille déformée reste jusqu'à l'arrivée du rendu exact
(`warp_settle`) ; reprendre un coin ou finir par Entrée / Échap ne la termine pas, tout autre geste la
pose d'abord sur le dessin par le processeur (`Flottante::flatten_warp`). Le bitmap garde l'ancien
aperçu (il n'a pas de feuille de traits).

| 1920 × 1080, traits larges de gouache, feutre, fusain | Avant | Après |
|---|---|---|
| Enveloppe, 10 traits, par image | **9,9 ms** (+ 7,7 Mo de tuiles envoyées) | **0,14 ms** (221 Ko de sommets) |
| Enveloppe, 40 traits, par image | **37,0 ms** (+ 8,4 Mo) | **0,14 ms** (221 Ko) |
| Saisie, 10 / 40 traits (une fois, début du geste) | 2,6 / 18,0 ms | 2,8 / 12,2 ms |
| Enveloppe de 300 traits (boîte 3618 × 2872) | — | saisie 25 ms (112 avant les bandes), 0,16 ms par image |

La saisie d'une grosse sélection se fait par bandes de lignes sur tous les cœurs quand chaque trait a
ses pixels exacts en cache (`vector::render_in_bands`, `Element::cache_ready`) : 112 → 25 ms pour 300
traits, même image pixel pour pixel. Essais : `warp_tests.rs` (maillage contre la déformation exacte de
la feuille : 0 pixel de différence ; débord de la feuille ; calque transformé ; fin anticipée),
`gpu_tests.rs` (la carte graphique de la machine contre le secours : 0 pixel à plus de 3 niveaux, en
mode Produit, sous une transformation de calque), `affichage_tests::envelope_drag_frame_cost`.

**Beaucoup de traits choisis** (`contours.rs`, `canvas.rs`). Le contour bleu d'1 px de chaque trait
choisi recopiait tous ses points, les passait à l'écran et laissait egui tracer chaque ligne (4
sommets et 18 indices par point, renvoyés à la carte graphique) à chaque image d'écran, au repos
comme en geste : 300 traits de 500 points = 13,5 ms et 22,8 Mo par image, 300 de 2000 points = 54 ms
et 91 Mo. Ce qui ne coûtait presque rien : la boîte de la sélection (0,2 ms pour 150 000 points).
Maintenant les contours sont faits une fois par sélection et par vue (simplifiés à 0,45 px près
— plus gros jusqu'à 12 px au-delà de 40 000 points —, ruban de 3 sommets par point, un seul
maillage gardé) ; ensuite l'image ne coûte qu'une empreinte de la sélection (0,02 ms) et, quand la
vue ou le geste a déplacé la sélection, une transformation des sommets ; refaits seulement si la
sélection ou le dessin change, ou en dehors d'un geste si la vue change d'échelle de plus d'un quart.

| Sélection (griffonnages dans 1920 × 1080) | Avant, par image | Après, par image (au repos / en geste), egui compris | Construction (une fois) |
|---|---|---|---|
| 30 × 500 points | 1,2 ms, 2,3 Mo | 0,10 / 0,18 ms | 0,7 ms |
| 100 × 500 | 4,2 ms, 7,6 Mo | 0,33 / 0,64 ms | 2,5 ms |
| 300 × 500 (150 000 points → 31 746) | **13,5 ms, 22,8 Mo** | **0,64 / 1,2 ms** (3,4 Mo) | 8,1 ms |
| 300 × 2000 (600 000 points → 29 216, tolérance 6,4 px) | **53,8 ms, 91 Mo** | **0,55 / 1,0 ms** (3,1 Mo) | 36,6 ms |

(`affichage_tests::many_selected_strokes_frame_cost` et `many_selected_strokes_move_cost`, ignorés.)
Le contour garde la même encre qu'une ligne d'1 px d'egui (908 contre 911, `contours_tests.rs`) ; en
geste, une mise à l'échelle épaissit le trait avec le facteur (1 px × facteur) jusqu'au lâcher. Reste :
la saisie de 300 traits coûte encore 25 ms au début du geste (pose des pixels en cache) ; la faire
d'avance en arrière-plan dès que la sélection est faite l'effacerait.

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
