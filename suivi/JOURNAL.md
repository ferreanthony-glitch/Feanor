# Journal de Claude

Ce que Claude a fait ou proposé, par session, le plus récent en haut. Une session = une date.
Pour le détail technique, voir l'historique git et docs/INTERFACE.md (une partie numérotée par chantier).

## 2026-11-12

- **Un panneau « Espace 3D » : Réglages montre toujours l'outil choisi** (partie 150, branche `trajectoire-espace`, toujours le
  format 55). Anthony : « même dans l'espace 3D, le panneau réglages affiche les réglages de l'outil sélectionné. Les réglages de
  l'espace 3D basculent dans un panneau spécifique », puis, pour sa place : « Sa place par défaut : en dessous de réglages, à la
  place de la couleur. » Réglages montre l'outil en main, vue Espace 3D ouverte ou non : le pinceau, le crayon, la gomme quand ils
  dessinent sur une carte, et les réglages de la Trajectoire dans l'espace (ils ne sont plus en tête de ceux de la vue). Les
  réglages de la vue (options, « Plan en Espace 3D », « Nouvelle surface », cartes, clés, caméra du plan) sont dans un panneau
  « Espace 3D » (cube), un onglet de plus à côté de la couleur, sous Réglages, dans le Storyboard, l'Animation et les dispositions
  standard. Il passe devant la couleur quand la vue s'ouvre et lui rend sa place quand elle se ferme ; fermé, la vue le remet à sa
  place ; la disposition retenue garde la couleur devant. Vue fermée : une phrase et le bouton « Ouvrir la vue Espace 3D ». Les
  dispositions enregistrées avant lui le reçoivent une fois, sans que rien d'autre y bouge (Storyboard, Animation, espaces
  enregistrés ; pas les Vignettes ni le Montage). Commande `vue.espace3d_panneau` (sans touche), icône dans la barre des panneaux,
  case dans Affichage > Panneaux. Les messages qui disaient « dans Réglages » disent « dans le panneau Espace 3D ». Défaut corrigé :
  avec l'outil Trajectoire, Ctrl+clic sur la caméra choisit la caméra, plus la carte qui passe dessous. 14 essais de plus ; deux
  essais des boutons de clés et une étape de l'essai de fumée lisent maintenant le panneau Espace 3D. Vérificateur : 4 593
  réussis, 0 échec, dix groupes OK, le seul avertissement connu. Pas fait : le clic simple de l'outil « choisir » prend encore la carte avant la caméra ; le panneau ne repasse pas devant tout seul
  après un clic sur l'onglet Couleur. À savoir : un fichier de réglages écrit par cette branche nomme le panneau, une version
  d'avant ne le lit pas (réglages à part pour l'exe de la branche). Rien regardé à l'écran, exe non reconstruit. Rien fusionné,
  rien poussé.
- **Parties 148 et 149 regardées à l'écran par Claude** (22 h 15 à 22 h 20, souris simulée, exe de la branche construit à 22 h 14,
  copie du projet `afa`, réglages à part). Partie 148 : la carte rouge du plan 6 mise à Z 8 m par son champ (profondeur −1,37,
  derrière la caméra d'origine) : acceptée, elle sort de l'image du plan ; annulé. Remarque : son champ « Échelle » affiche alors
  1000 %, la valeur interne. Partie 149 : outil Trajectoire dans la vue Espace 3D, « Clés d'interpolation » ; un chemin tracé au sol
  avec la carte rouge choisie : elle le suit, sa profondeur passe de 0 à −0,82 ; un petit trait en travers pose une marque (trois
  jalons sur la timeline). Caméra choisie par le bouton « choisir » des Réglages, chemin tracé au sol : elle le suit, « Regarder
  par la caméra » le montre, aucune clé de plus sur sa piste ; Ctrl+Z retire le chemin en une étape, Ctrl+Y le rend. Fermé puis
  rouvert : les deux chemins sont là (copie au format 55). Défaut vu : Ctrl+clic sur la caméra, avec l'outil, a choisi la surface
  au sol qui était dessous, pas la caméra. Pas essayé à l'écran : tordre ou déplacer le chemin, glisser ou retirer une marque,
  plusieurs cartes à la fois, carte et caméra ensemble, un chemin sur un mur, les « Clés d'images », le stylet.
- **La Trajectoire dans l'Espace 3D** (partie 149, branche `trajectoire-espace`, toujours le format 55). Anthony : « Je voudrais
  qu'on essaye d'intégrer l'outil trajectoire dans l'environnement 3D, afin de générer des mouvements dans l'espace », puis « Un
  calque ou la caméra, les deux. Ce qui est sélectionné est sujet à la trajectoire. » Dans la vue Espace 3D d'un plan en Espace 3D,
  l'outil Trajectoire trace un chemin posé sur une surface (une carte non choisie, sinon le sol, sinon un plan face à la vue), et
  tout ce qui est choisi le suit : chaque carte choisie, la caméra si elle l'est ; rien de choisi, la carte du calque courant.
  Mêmes gestes que sur la feuille (trait long, marque en travers, glisser ou arracher une marque, tirer, Maj pour déplacer, second
  trait, Échap), pour tout ce qui est choisi, en une étape d'annulation. Une carte reçoit des clés de position (une par image, ou
  une tenue par marque) et garde sa taille et son orientation dans l'espace ; la caméra ne reçoit aucune clé, son chemin s'ajoute à
  ses clés quand on l'évalue (image, exports, « Regarder par la caméra », vue de dessus), et on peut toujours la régler à toute
  image. Réglages en tête du panneau de l'Espace 3D : départ, fin, temps égaux, sortes de clés, détacher, effacer, pour tout ce qui
  est choisi. Montage : raccourcir le plan par la fin ne change pas le mouvement de la caméra ; couper, réunir, ou rogner le début
  au-delà du départ écrit son chemin en clés sans changer l'image. Fichier : deux champs facultatifs, pas de nouveau numéro.
  Décision prise en route : le sol l'emporte sur une carte quand il est devant elle (une carte descend sous le sol). Trois défauts
  de la partie 148 corrigés : la caméra retournée d'un plan en Espace 3D est relue telle quelle (plus ramenée à 85°, y compris
  dans le contenu rogné) ; le message « En construction » de la case est remplacé ; les silhouettes de la trajectoire de la feuille
  suivent la caméra du plan. 17 essais de plus. Vérificateur : 4 579 réussis, 0 échec, dix groupes OK, le seul avertissement connu (celui des fonctions jamais appelées de `espace3d.rs` ; il n'en nomme plus que cinq, `Monde::to_frame` sert maintenant). Pas fait : la trajectoire de la caméra ne se voit pas sur la
  timeline ; pas de silhouettes dans la vue ; un chemin tient sur une seule surface ; le miroir d'un calque ne retourne pas son
  chemin dans l'espace ; un plan sans clé de caméra qui suit un plan à trajectoire de caméra ne reprend pas la caméra là où le
  chemin l'a laissée. Rien regardé à l'écran, exe non reconstruit. Rien fusionné, rien poussé.
- **Une carte passe derrière la caméra d'origine du plan** (partie 148, branche `trajectoire-espace`, **format 55**). À ma phrase
  « Une carte ne peut pas passer derrière la caméra d'origine du plan. Un personnage qui vient vers nous s'arrêtera donc avant de
  nous dépasser », Anthony : « Alors dépassons cette limite absurde. » La profondeur d'un calque s'arrêtait à −0,9 parce que la
  taille d'une carte dans l'espace vaut « taille dessinée × (1 + profondeur) ». Dans un plan en Espace 3D seulement, elle est
  maintenant libre : sous −0,9 cette règle reste à sa valeur de la borne et la carte ne fait plus qu'avancer, jusqu'à la caméra
  d'origine et derrière elle. Rien de neuf dans le fichier, mais un nouveau numéro de format : une version d'avant ramènerait ces
  cartes devant la caméra. Suivent : les poignées de la vue Espace 3D, les champs « Position » et « Profondeur », « Nouvelle
  surface » (plus de refus derrière la caméra d'origine), le pinceau, la suppression des clés de position, l'outil Transformer.
  Sous la caméra d'origine, un plan dont une carte a passé la borne est composé par maillage : la carte passée derrière n'est plus
  filmée. Décocher « Plan en Espace 3D » ramène ces cartes à −0,9, le dit, et un Ctrl+Z rend tout. Trouvé en chemin : un champ de
  nombre borné écrit sa borne rien qu'en se montrant ; les deux champs « Profondeur » (Réglages de l'Espace 3D, menu du calque)
  descendent donc à −1000 dans ces plans. 9 essais de plus, 1 réécrit (la surface refusée), le numéro de format dans six fichiers
  d'essais. Vérificateur : 4 562 réussis, 0 échec, dix groupes OK, le seul avertissement connu. Pas fait : entre deux clés posées à
  des profondeurs différentes, la carte ne garde pas sa taille dans l'espace (défaut d'avant, à régler avec la trajectoire de
  l'Espace 3D). Vu en passant, pas corrigé : la caméra d'un plan en Espace 3D est relue bornée à ±85° (une caméra retournée revient
  à 85° à la réouverture) ; le message « En construction » de la case est périmé. Rien regardé à l'écran, exe non reconstruit.
  Rien fusionné, rien poussé.
- **Fin de la liste de la partie 143, faite par Claude sur la fenêtre d'essai** (17 h à 17 h 16, souris simulée, sur son « continue »),
  copie du projet `afa`. Point 2 (plan 6, trois calques de couleur pleine : plan ordinaire, la pile décide ; coché, le plus proche
  passe devant ; à profondeur égale la pile départage) : bon. Point 3 (caméra passée derrière les cartes, non bridée ; la feuille et
  « Regarder par la caméra » montrent la carte la plus proche devant, en miroir) : bon. Point 4, lecture d'un mouvement de caméra à
  trois clés : bon. Point 8 (sol dessiné, caméra au-dessus, −89° puis −90° : plongée totale, pas d'image qui tourne d'un coup) : bon.
  Point 12, vidéo (MP4 du plan, image 24 comparée à l'export en image : ressemblance 0,98 à 0,99 ; couleurs un peu décalées, encodeur
  H.264 de Windows faute de ffmpeg complet à côté de l'exe d'essai, cause non vérifiée) : bon pour l'image. Point 13, refus par le
  FEANOR habituel (format 53) d'une copie au format 54 : la copie n'est pas touchée ; texte du refus pas lu à l'écran. Point 14 (plan
  ordinaire : bascule bridée à −72,9° pour −90° tapé, vue de dessus présente) : bon pour ce qui a été regardé. Point 15 (rotations Y
  et X tapées de 85 à 95°, anneau tiré de 80 à 125° par pas de 15°) : pas de saut. Restent à son œil : le stylet, la bulle et la
  lisibilité des icônes, le « sans à-coup » du point 7. Rien changé au code.
- **Retour d'Anthony sur la partie 147** (vers 15 h, autre PC : i7-9700K, 16 Go, RTX 2060, seconde version portable d'essai) :
  « Non, plus de plantage ». Juste avant : « Pas de ralentissement ici avec une machine sous cette config ». Rien changé pour les
  ralentissements du matin, dont la cause reste inconnue. Rien fusionné, rien poussé.
- **La vue Espace 3D ne ferme plus FEANOR quand une carte passe derrière le regard** (partie 147, même branche). Anthony, après la
  version portable d'essai : « il y a beaucoup de crash du logiciel quand on se balade dans l'espace 3D ». Rapport du 12 novembre,
  0 h 27 : « Buffer slices can not be empty » dans egui-wgpu. Cause : `card_mesh` rendait un maillage avec des sommets et aucun
  triangle quand il ne restait pas un carreau entier de la carte devant l'œil ; egui le laisse passer, wgpu le refuse. Le défaut est
  aussi dans `main`, rare tant que les cartes font face à la caméra ; fréquent avec un sol ou un mur. Corrigé dans la vue
  (`espace3dview.rs`) et dans le canevas (`canvas.rs`, même maillage). Essai dirigé rouge puis vert (`espace3dvue_tests.rs`), une
  promenade de 360 images en plus (1,14 ms par image en médiane sur ce PC). Vérificateur : 4 552 réussis, 0 échec, dix groupes OK,
  le seul avertissement connu. Les ralentissements dont il parle ne sont pas retrouvés ici : rien changé pour eux, question posée
  (quelle machine, quel geste). Mesure geste par geste ajoutée ensuite (troisième essai, lancé seul, carte graphique comprise) :
  1,5 ms par image pour tourner, glisser, avancer ; 2,2 à 2,3 ms quand c'est la caméra du plan qui bouge ; pire image 4,95 ms.
  Exe reconstruit (13 h 16) ; deuxième version portable d'essai `FEANOR-portable-2026-11-12b-ESSAI-espace-3d` (dossier et zip),
  dézippée et ouverte ici par son lanceur sur une copie : fenêtre ouverte, aucun plantage, vrais réglages inchangés. La fenêtre
  d'essai de ce PC est rouverte sur le nouvel exe.
- **Deux versions portables**, demandées par Anthony (« sors moi une nouvelle version portable pour pc »), dans `Logiciel\Portable`,
  dossier et zip. `FEANOR-portable-2026-11-12` : le FEANOR habituel (`main` 00b5e44, format 53). `FEANOR-portable-2026-11-12-ESSAI-espace-3d` :
  cette branche (3f4e3e9, format 54), l'exe rangé dans `programme\` et lancé par « Lancer FEANOR essai Espace 3D.cmd », qui lui donne
  ses propres réglages dans `reglages\` à côté de lui : elle ne lit pas les préférences habituelles, donc ne rouvre et ne réécrit
  aucun vrai projet au format 54 (raccourcis et brosses d'origine). Rien construit : les exe existaient. Chaque zip dézippé et ouvert
  ici sur une copie du projet d'essai : fenêtre ouverte, aucun plantage, vrais réglages et vrai projet inchangés.
- **Sur la feuille d'un plan en Espace 3D, plus de trait que la carte ne montre pas** (partie 146.1, même branche ; correction de
  l'étape A5 de la session du Mac). Le défaut vu à l'écran ce jour, point 9 de la liste de la partie 143 : caméra dos aux cartes
  (panoramique 180°), pinceau sur « Surface au sol », sur la feuille : rien ne se voyait, mais un trait était enregistré hors de la
  surface (y = 2 541 à 3 153 pour une feuille de 2 160), sans un mot. Son « Oui » à « Je corrige le défaut du point 9, le trait
  invisible hors de la carte ? ». Cause : `App::to_layer_opt` acceptait un point jusqu'à une feuille au-delà des bords de la carte, alors
  qu'une carte posée par un maillage ne montre que sa feuille (`Monde::card_warp_m`). Règle : dans un plan en Espace 3D, un point n'est
  accepté que dans les bords de la carte (`espace3ddessin::carte_montre`, la même condition que dans la vue Espace 3D, une seule
  fonction) ; un trait commencé au-delà ne commence pas et la barre d'état le dit (message neuf pour le bord de la carte,
  `App::rien_a_peindre`) ; pendant un trait les points qui sortent sont laissés. Rien ne change sous la caméra de référence avec une carte
  à plat (la toile infinie reste peinte), ni dans un plan ordinaire. Essai neuf
  `on_the_sheet_of_a_plan_in_3d_space_no_stroke_is_recorded_where_the_card_will_not_show_it` (le cas vu à l'écran, le bord de la carte
  caméra tournée, le trait qui sort de la carte, la caméra de référence, le plan ordinaire). Un essai existant de l'étape A5 touché :
  `near_the_horizon_of_a_card_on_the_ground_the_brush_stops_instead_of_running_away` (`espace3d_tests.rs`) vérifiait « jamais à plus
  d'une feuille des bords » ; il vérifie maintenant « jamais hors des bords » (il passait aussi sans ce changement). Pas regardé à
  l'écran. Limite laissée : une carte inclinée d'un plan ordinaire accepte encore un trait hors de ses bords, qui ne se voit pas.
- **Dessiner dans la vue Espace 3D** (partie 146, branche locale `espace-libre-cles`, à la suite des boutons de clés). Sa demande,
  pendant le même essai : « il est impossible de dessiner dans l'espace 3D pour l'instant. C'est quelque
  chose qui doit être faisable en permanence, le trait s'appliquant sur la carte », et « Oui, ici » à « je le code ici ? ». La règle :
  **l'outil décide**. Dans la vue (orbite ou « Regarder par la caméra », plan ordinaire ou plan en Espace 3D), le pinceau, le crayon, la
  gomme ou le mélangeur (case, touche, ou le bouton neuf « Dessiner sur la carte », commande `espace3d.dessiner`, sans touche) font
  dessiner le pointeur sur la carte du calque courant ; un outil de la vue (choisir, déplacer, tourner, taille, caméra) lui rend les
  cartes ; Alt+glisser, clic droit, milieu et molette restent à la vue ; Ctrl+clic prend une autre carte. Le trait passe par le moteur
  de toujours (`begin_paint`, `stroke_point`, `end_stroke`) : seule change la conversion écran → dessin, par le regard d'UNE vue
  (`espace3ddessin::Regard3d`, `CarteVue`, prévu pour les vues côte à côte de `vues-3d`) ; `layer_to_screen` et `layer_zoom` y passent
  tant qu'on dessine dans la vue. Le trait se voit pendant le geste sans rejeu du moteur : la texture de la carte dessinée est refaite
  par morceaux, les tuiles changées seulement (`render::cel_image_zone`). Le cercle du pinceau est posé sur la carte (ellipse sur une
  carte penchée). Refus avec un mot : hors de la carte, de dos, par la tranche, derrière l'œil, cartes cachées, calque verrouillé ou caché.
  Jamais d'encre qu'on ne voit pas : un trait ne commence et ne reçoit de points que dans les bords de la carte.
  Fichiers : `src/espace3ddessin.rs` et `src/espace3ddessin_tests.rs` (neufs), quelques lignes dans `espace3dview.rs`, `espace3dui.rs`,
  `app.rs` (`set_tool`, `layer_to_screen`, `layer_zoom`), `canvas.rs`, `tracedroit.rs`, `render.rs`, `onglets.rs`, `keymap.rs`,
  `keymap.toml`, `lib.rs`. Hors de cette étape : pot de peinture, dégradé, formes, sélection, Transformer, coller une image dans la vue.
  Essais : 9 neufs (`espace3ddessin_tests.rs` : de vrais traits de souris dans la vue d'orbite et par la caméra, carte de face, penchée,
  surface au sol ; les refus ; l'outil qui décide ; Alt+glisser et Ctrl+clic ; la taille du pinceau ; la texture par morceaux et le
  retour au repos ; calque vectoriel et segment droit ; la feuille d'un plan en Espace 3D, puce ci-dessus), et les familles voisines,
  0 échec (les nombres sont dans le dernier commit). Commits sur `espace-libre-cles` : 7b6800e et 072cbda (en cours), e6755ff (la règle
  des bords dans la vue), puis celui de la partie 146.1. Limites : pot de peinture, dégradé, formes, sélection, Transformer absents de
  la vue ; à l'entrée dans la vue le pointeur manipule et la barre d'outils de gauche garde son outil allumé ; carte vue de dos refusée
  dans la vue. **Rien n'a été regardé à l'écran ni essayé au stylet** (liste en 11 points, partie 146) ; la suite de référence entière
  n'a pas été relancée ; l'exe n'est pas construit (son FEANOR d'essai tournait) ; pas poussé, pas fusionné.
- Pendant son essai à l'écran de la branche `espace-libre` (plan en Espace 3D), sa remarque : « les clés de caméras et de transformation
  des objets ont disparu avec la vue 3D », puis « j'aimerais qu'un bouton y apparaisse » (vue Espace 3D ouverte, le panneau Réglages
  n'avait aucun bouton de clés, sur `main` non plus). Chantier `espace-libre-cles` (partie 145), branche locale du même nom depuis
  `espace-libre` e5dc84e, dans `Chantiers\calques-place` ; `espace-libre` elle-même ne reçoit aucun commit. Ajouté dans les Réglages de
  l'Espace 3D, avec les icônes, les bulles et les commandes des panneaux 2D (aucune commande neuve) : sur la ligne « Caméra du plan », le
  losange « Clé de caméra ici » (allumé quand l'image courante porte une clé) et la corbeille qui la retire ; sur la ligne de la carte
  choisie, ou « N cartes choisies », le losange « Clé de position » (allumé quand toutes les cartes choisies ont une clé ici) et sa
  corbeille. Sa règle « la sélection agit sur tout ce qui est choisi » : vue Espace 3D ouverte, les commandes de clés de position visent
  les cartes choisies et non le seul calque courant, en une étape d'annulation ; une carte verrouillée est laissée de côté, avec un mot.
  Essais : 6 neufs (`espace3dcles_tests.rs`, dont un vrai clic dans le panneau et une surface au sol qui garde son inclinaison), et les
  familles voisines (Espace 3D 86, caméra 248, clés 102, clés de position 18, trajectoire 29, Transformer 31, bulles, raccourcis, fumée) :
  0 échec ; `cargo check --release` : le seul avertissement déjà connu de `espace3d.rs`. Pas fait : la suite de référence entière ; l'exe
  n'a pas été construit (son FEANOR d'essai tournait) ; **rien n'a été regardé à l'écran** (liste en 7 points, partie 145). Pas poussé,
  pas fusionné.
- **plantage-carte-vide fusionné dans `main`** (41fe5a9) sur le « Oui, corrige-le » d'Anthony, poussé sur GitHub. Exe principal
  reconstruit (18 h 37, aucun avertissement) et ouvert deux fois sur une copie de son projet `afa`, réglages à part : pas de
  plantage, la copie reste au format 52. La promenade dans la vue Espace 3D n'a pas été refaite à l'écran avec cet exe : les essais
  la couvrent, et Anthony l'a faite sur un autre PC avec la même correction. Pas de version portable (sa consigne). À tester par
  Anthony : la liste de la partie 147.
- **La vue Espace 3D ne ferme plus FEANOR quand une carte passe derrière le regard** (partie 147, branche `plantage-carte-vide`).
  Anthony, pendant l'essai de « Plan en Espace 3D » : « il y a beaucoup de crash du logiciel quand on se balade dans l'espace 3D ».
  Son rapport du 12 novembre, 0 h 27 : « Buffer slices can not be empty » dans egui-wgpu. Cause : `card_mesh` rendait un maillage
  avec des sommets et aucun triangle quand il ne restait pas un carreau entier de la carte devant l'œil ; egui le laisse passer,
  wgpu le refuse. Corrigé d'abord sur la branche d'essai `espace-libre-cles` (il a confirmé sur un autre PC : « Non, plus de
  plantage »), puis porté ici à sa demande (« Oui, corrige-le ») : la vue (`espace3dview.rs`) et le canevas (`canvas.rs`, même
  maillage), ligne pour ligne la même correction. Essai dirigé rouge sur le code d'avant, vert après (`espace3dvue_tests.rs`) ; une
  promenade de 360 images à travers les cartes en plus (0,94 ms par image en médiane). Vérificateur : 4 502 réussis, 0 échec, aucun
  avertissement, dix groupes OK. Le Plan en Espace 3D, les boutons de clés et le dessin dans la vue Espace 3D ne sont pas dans
  `main` : ils restent sur `espace-libre-cles`, envoyée sur GitHub le même jour à sa demande. Format inchangé : 53.

## 2026-11-11

- Calques dans la timeline, suite : ses deux idées (« trois calques maximum affichés », ceux du dessus et du dessous venant quand on
  monte ou descend ; la rangée des plans trop épaisse) numérotées 39 et 40 et maquettées sur la page des propositions. Avis donné :
  oui aux deux ; le calque actif toujours au milieu, deux fines bandes qui comptent les autres, un bouton « Toute la pile » ; la
  hauteur des plans réglable en tirant son bord, jusqu'au bandeau, retenue par onglet (`montage_rows` donne aujourd'hui aux plans la
  place qui reste, entre 56 et 170 points). La 39 remplace la 37. Rien de codé : attend son accord.
- Ses décisions après avoir vu la maquette : « Toute la pile » gardée (comparer deux calques éloignés, en choisir plusieurs) ; plans
  en taille moyenne ou en bandeau seulement ; l'affichage des calques réglable depuis la barre du bas de la timeline et depuis les
  Préférences ; **un dossier n'est jamais traité comme un calque dans la timeline : toujours plus petit, il se plie et se déplie** ;
  les touches de zoom de la timeline sont celles de la zone de dessin (41 : chez lui Maj+1 et Maj+2 ; la timeline a aujourd'hui
  `bande.zoom_*` sur Alt+1 et Alt+2). Maquette refaite (version 3 de la page) : fenêtre de trois calques avec le calque actif au
  milieu, ligne fine de dossier qui prend sa place à la fine bande, barre du bas, Préférences. Mes choix signalés comme à
  contredire : les flèches sautent les dossiers, boutons de plans réduits à « Nouveau plan », réglages et « ⋯ », un seul réglage
  pour Storyboard et Montage, fin du réglage « Calques non actifs ». Toujours rien de codé.
- Troisième tour : sa règle, à la place de mon « dossier fermé traversé d'un pas » : **un dossier fermé se traverse avec « calque du
  dessus / du dessous » et se referme quand on en sort**. Sa question : d'accord pour les couleurs par dossier, mais risque de
  confusion avec les couleurs par type de calque (partie 98). Réponse proposée et maquettée (version 4 de la page) : jamais les deux
  sens au même endroit ; la bande garde la couleur du type ; le dossier a un filet de couleur dans la colonne de gauche, pris dans
  les treize groupes de couleur existants (`groups.rs`, `Layer::group`, déjà peints en filet dans la timeline de l'Animation, jamais
  utilisés dans ses projets), attribué tout seul à la création du dossier. Attend son avis et son accord pour coder.
- Sur son « code, pas besoin de version portable jusqu'à nouvel ordre » : **les calques dans la timeline** (partie 137, branche
  `calques-timeline`, chantier `Chantiers\calques-timeline`, depuis main 8da4f1d). Trois calques à la fois (le calque actif au milieu,
  toujours à la même place ; deux fines bandes comptent les autres) ou toute la pile ; un dossier est une ligne fine qui se ferme
  et s'ouvre, jamais un calque ; un dossier fermé se traverse avec « calque du dessus / du dessous » et se referme à la sortie,
  sans rien écrire ; la bande garde la couleur du type, le dossier a son filet à gauche (groupes de couleur, d'office par
  dossier) ; plans de hauteur moyenne ou en bandeau ; réglages dans la barre du bas de la bande et dans Préférences > Montage ;
  les touches du zoom de l'image zooment la bande quand le pointeur est dessus. Annoncé puis corrigé auprès de lui : la bande du
  bas ne rétrécit pas toute seule (sa hauteur est le bord tiré à la main), le chantier ne rend donc pas de place à l'image. Trouvé
  en passant : la fenêtre des Préférences s'élargissait de 9 points par image pendant une recherche (défaut d'avant, corrigé) ;
  `ahead_capacity` s'arrêtait net sur une feuille de moins de 64 Ko (corrigé). Un travers du banc d'essai noté : le premier clic
  après l'apparition du pointeur se perd. Captures sur une copie de « retakes brawlydays ». Vérificateur sur la branche : 4 444
  essais réussis, 0 échec, aucun avertissement. Pas fait : les propositions 34, 35, 36 et 38 ; la bande qui prendrait la hauteur
  de son contenu. Pas fusionné : attend son essai ou son accord.
- Fusion de `calques-timeline` dans main (c410030, sans avance rapide) sur son « Fusionne directement », sans qu'il ait essayé l'exe
  de la branche ; pas de version portable (sa consigne du jour). `main` n'avait pas bougé depuis le départ de la branche : aucun
  conflit, même contenu que la branche. L'autre session prévenue avant (message déposé, pas de réponse attendue) ; son chantier
  `cles-glisser` (`Chantiers\multi-calques`, non validé) touche les mêmes fichiers et devra ramener main avant d'être fusionné.
  Vérificateur sur main : 4 444 essais réussis, 0 échec, aucun avertissement, tous les groupes de fiabilité OK (712 s). Poussé sur
  GitHub. Exe principal reconstruit (`target\release\feanor.exe`, 11 h 53) et ouvert deux fois sur une copie de « retakes brawlydays »
  avec de faux réglages : pas de plantage. Pas vérifié : la timeline manipulée à la main dans le vrai exe (sa liste 137.8). Le
  dossier de chantier `Chantiers\calques-timeline` attend son accord pour être retiré.
- Son essai de l'exe de midi, quatre retours : « Ajoute les bandes de couleur du dossier dans le panneau calque également » ;
  « Modifier la durée des images clés et la durée des plans n'est pas intégré » ; « Sélectionner plusieurs calques et créer une
  nouvelle image clé (vides également) n'a pas été intégré » ; « Déplacer une image clé avant ou après la suivante n'a pas été
  intégré ». Ce que j'ai trouvé : la durée des clés et des plans et les clés glissées avant ou après une autre étaient codées par
  une autre session depuis le 10 au soir, mais restées **non validées** dans `Chantiers\multi-calques` (branche `cles-glisser`,
  16 fichiers modifiés, 3 neufs, dernière écriture le 11 à 0 h 03), jamais vérifiées ni fusionnées ; aucune session ne s'en réclame
  (l'une hors ligne, l'autre sans chantier en cours). Repris : validé tel que trouvé (4bc6fbf), main ramené dedans (d8aaf8f ; un
  seul conflit, `docs/INTERFACE.md` : sa partie devient la 138), ses 16 essais passent. La nouvelle clé sur plusieurs calques, elle,
  **était dans main** (partie 132) mais trois gestes courants la réduisaient à un seul calque : un clic dans la piste d'un des
  calques choisis (le calque courant n'était pas inscrit dans le choix), des clés prises sur plusieurs pistes sans cliquer de nom,
  et le double-clic sur une piste (qui défaisait en plus le choix) ; corrigés (partie 137.11). Son geste exact n'est pas connu : son
  compteur d'usage dit seulement F5 au clavier. Le filet du dossier dans le panneau Calques : tout à gauche du rang, la bande de la
  sorte juste après, rien de changé dans un plan sans dossier ; regardé sur une capture d'une copie de « retakes brawlydays ».
  Vérificateur sur la branche : 4 464 essais réussis, 0 échec, aucun avertissement (669 s). Sur son second « Fusionne directement »
  (donné pendant le travail) : fusion dans main à suivre.
- Fusion de `cles-glisser` dans main (805b8e3, sans avance rapide) : `main` n'avait pas bougé, même contenu que la branche vérifiée
  (le vérificateur n'a donc pas été relancé sur main). L'autre session prévenue par deux messages, sans réponse. Poussé sur GitHub.
  Exe principal reconstruit (`target\release\feanor.exe`, 13 h 54) et ouvert deux fois sur une copie de « retakes brawlydays » avec
  de faux réglages : pas de plantage. Pas de version portable. Pas vérifié : aucun geste fait à la main dans le vrai exe (ses listes
  137.11 et 138) ; la partie 138 n'a été ni écrite ni essayée par moi au-delà de ses 16 essais et du vérificateur. Les dossiers de
  chantier `Chantiers\calques-timeline` et `Chantiers\multi-calques` attendent son accord pour être retirés.
- Sur son « Oui, retire-les » : les deux dossiers de chantier retirés (propres, leurs branches entièrement dans main ; 21 Go de
  fichiers de construction rendus au disque). Les branches `calques-timeline` (7e89e7b) et `cles-glisser` (49472b8) restent. Il ne
  reste que `Source`.
- Deux nouveaux retours de son essai de l'exe de 13 h 54 (partie 139, branche `dossier-glisser`, chantier
  `Chantiers\dossier-glisser`, depuis main 8c2417d ; le dossier de construction est celui de `Source`, pour ne pas tout
  recompiler). **« glisser des calques dans un dossier ne fonctionne pas »** (capture : trois calques choisis sous un dossier vide) :
  quatre causes dans le code — pris par son nom, un calque ne partait pas du panneau Calques (le texte du nom se sélectionnait) ; le
  lâcher sur le rang d'un dossier ne rangeait pas dedans ; le calque posé juste sous un dossier vide ne pouvait pas y entrer ; un
  seul calque partait, et rien n'entrait dans un dossier fermé. Corrigé aux trois endroits (panneau Calques, colonne de la bande,
  en-têtes de l'Animation) : lâchés sur la ligne d'un dossier, les calques choisis y entrent ensemble (`folders::poser`,
  `layerheads::cible_du_glisse`). **« le ctrl+c/ctrl+v ne fonctionne plus pour copier/coller plusieurs lignes vectorielles »** :
  avec deux clés restées choisies dans la timeline (ce que laisse une nouvelle clé posée sur deux calques) et un seul calque choisi,
  la copie partait dans le presse-papiers « plusieurs dessins » que Ctrl+V ne lisait qu'avec deux calques choisis : rien ne se
  collait. Ctrl+V colle maintenant la dernière copie quel que soit le nombre de calques choisis, et un clic sur le nom d'un calque
  lâche les clés choisies sur les autres. Son geste exact n'est pas connu : c'est le seul chemin trouvé où le collage ne fait rien,
  reproduit par un essai (écrit après la correction : je ne l'ai pas vu échouer avant). Vérificateur sur la branche : 4 472 essais
  réussis, 0 échec, aucun avertissement. Exe de la branche (`Source\target\msvc\release\feanor.exe`, 14 h 41) ouvert deux fois sur
  une copie de « retakes brawlydays » avec de faux réglages : pas de plantage. Pas fusionné : attend son accord.
- Fusion de `dossier-glisser` dans main (b78ea38, sans avance rapide) sur son « Fusionne directement » : `main` n'avait pas bougé,
  même contenu que la branche vérifiée (vérificateur non relancé sur main). L'autre session prévenue (message déposé, sans réponse).
  Poussé sur GitHub. Son FEANOR, ouvert depuis 13 h 59, fermé sans enregistrer pour reconstruire (il était prévenu ; l'écriture est
  continue). Exe principal reconstruit (`target\release\feanor.exe`, 14 h 48) et ouvert deux fois sur une copie de « retakes
  brawlydays » avec de faux réglages : pas de plantage. Pas de version portable. Pas vérifié : aucun geste fait à la main dans le
  vrai exe (sa liste 139.3). Le dossier de chantier `Chantiers\dossier-glisser` attend son accord pour être retiré.
- Sur son « Oui, retire-le » : le dossier de chantier `Chantiers\dossier-glisser` retiré (propre, sa branche entièrement dans main).
  La branche `dossier-glisser` (0293edd) reste. Il ne reste que `Source`.
- Nouveau retour, deux captures à l'appui : « Sélectionner 3 calques et cliquer sur F5 ne crée une clé d'image que sur un seul
  calque au lieu des trois ». Sur ses captures, les pistes de deux des trois calques choisis sont ternes : c'est ainsi que la bande
  peint un calque masqué. Son projet le confirme (lu sans y toucher) : « Vectoriel 14 » et « Vectoriel 15 » ont l'œil fermé. La
  nouvelle clé sur plusieurs calques (partie 132) laissait de côté les calques masqués, en le disant seulement dans la barre
  d'état, alors qu'un calque masqué seul la reçoit. Corrigé (partie 139.5, branche `cle-masques`, chantier `Chantiers\cle-masques`,
  depuis main 4395c46) : un calque masqué reçoit sa clé ; les calques verrouillés restent de côté (à décider par lui). Vérifié sur
  une copie fraîche de son projet : les trois calques de ses captures choisis, F5, « Nouvelle clé sur 3 calques ». Vérificateur sur
  la branche : 4 473 essais réussis, 0 échec, aucun avertissement. Exe de la branche (`Source\target\msvc\release\feanor.exe`,
  15 h 59) ouvert deux fois sur une copie de projet avec de faux réglages : pas de plantage. Pas fusionné : attend son accord.
  Ma correction du matin (partie 137.11) visait d'autres gestes : elle n'était pas la cause de son cas.
- Fusion de `cle-masques` dans main (02c1ceb, sans avance rapide) sur son « Fusionne directement » : `main` n'avait pas bougé, même
  contenu que la branche vérifiée. L'autre session prévenue (message déposé, sans réponse). Poussé sur GitHub. Son FEANOR, ouvert
  depuis 15 h 35, fermé sans enregistrer pour reconstruire (il était prévenu). Exe principal reconstruit (`target\release\feanor.exe`,
  16 h 05) et ouvert deux fois sur une copie de projet avec de faux réglages : pas de plantage. Pas de version portable. Sa question
  ouverte : la clé sur les calques verrouillés. Le dossier de chantier `Chantiers\cle-masques` attend son accord pour être retiré.
- Sa demande, capture à l'appui (la bande tirée très haut, à moitié vide) : « que la taille des calques dans la timeline s'adapte
  aussi à la taille du panneau. La taille la plus grande reste la même mais quand il y a plus de place […] les calques les plus
  proches du calque sélectionné prennent aussi la taille la plus grande », puis « il faut que ce soit toujours le plus confortable
  possible à utiliser ». Codé (partie 140, branche `calques-place`, chantier `Chantiers\calques-place`) : quand la bande est plus
  haute que ses pistes, les calques grandissent un à un autour du calque actif (le dessus, le dessous, puis les suivants) tant
  qu'il reste de la place ; dans « Trois calques », les deux voisins ; pas assez de place : rien ne grandit, la bande défile comme
  avant. Une case dans Préférences > Montage, allumée d'office (éteinte dans les essais). Captures sur une copie fraîche de son
  projet, regardées et envoyées. Vérificateur sur la branche : 4 478 essais réussis, 0 échec, aucun avertissement. Exe de la
  branche (16 h 27) ouvert deux fois sur une copie de projet avec de faux réglages : pas de plantage. Son « Fusionne directement »
  est arrivé pendant le vérificateur : fusion dans main à suivre. Limite dite : deux hauteurs seulement (grande ou en bande).
- Fusion de `calques-place` dans main (7e96224, sans avance rapide) : `main` n'avait pas bougé, même contenu que la branche
  vérifiée. L'autre session prévenue (message déposé, sans réponse). Poussé sur GitHub. Son FEANOR n'était pas ouvert. Exe principal
  reconstruit (`target\release\feanor.exe`, 16 h 34) et ouvert deux fois sur une copie de projet avec de faux réglages : pas de
  plantage. Pas de version portable. Pas vérifié : la bande tirée à la main dans le vrai exe (sa liste 140.4). Les dossiers de
  chantier `Chantiers\cle-masques` et `Chantiers\calques-place` attendent son accord pour être retirés.
- Sa demande du soir : « une option de calque qui permet d'accrocher le calque à la caméra. Quand la caméra bouge, le calque suit
  naturellement car il y est accroché ». Chantier `calque-camera` (partie 141 de l'interface), sur une branche du même nom dans le
  dossier `Chantiers\calques-place` réutilisé. La règle : un calque accroché garde sa place dans l'image filmée, quoi que fasse la
  caméra (déplacement, zoom, rotation, avancée) ; l'accrocher ou le décrocher ne le déplace pas à l'image courante ; un calque animé
  garde son mouvement ; l'outil Transformer travaille sur ce qu'on voit ; les calques choisis s'accrochent ensemble, un dossier compte
  pour ses calques, un calque de couleur suit son trait. Un petit bouton caméra par calque dans le panneau Calques, la même icône
  dans l'en-tête de la timeline, Plus > Réglages avancés, une commande sans touche. Format de projet 53 (un projet ouvert par cette
  version ne s'ouvre plus avec une version d'avant). Vérificateur sur la branche : 4 489 essais réussis, 0 échec, aucun
  avertissement (un premier passage avait 7 échecs : des essais anciens qui comptaient sur le numéro de format 52, mis à jour).
  Quatre captures d'un plan d'essai regardées et envoyées. Exe de la branche (19 h 26) ouvert deux fois sur une copie fraîche de
  son projet avec de faux réglages, dont une fois avec un calque accroché sous sa caméra à deux clés : pas de plantage. Pas vérifié :
  aucun geste fait à la main dans le vrai exe. Limites dites : la bascule et le panoramique de la caméra ne sont pas suivis ;
  l'Espace 3D et la vue de dessus montrent le calque à sa pose dans le cadre ; pendant un geste du Transformer, la ligne d'état donne
  les valeurs vues sur la feuille. Les dossiers `Chantiers\cle-masques` et `Chantiers\calques-place` (qui porte ce chantier) restent.
  Fusion dans main : à son accord.
- Fusion de `calque-camera` dans main (c6cca20, sans avance rapide) sur son « Fusionne directement » : `main` n'avait pas bougé,
  même contenu que la branche vérifiée. L'autre session prévenue (message déposé, sans réponse). Poussé sur GitHub. Son FEANOR
  était ouvert : fermé sans rien demander avant la construction. Exe principal reconstruit (`target\release\feanor.exe`, 19 h 44) et
  ouvert deux fois sur une copie fraîche de son projet avec de faux réglages, dont une fois avec un calque accroché sous sa caméra à
  deux clés : pas de plantage. Pas de version portable. Pas vérifié : aucun geste fait à la main dans le vrai exe (sa liste 141.6).
  Les dossiers de chantier `Chantiers\cle-masques` et `Chantiers\calques-place` restent ; le second est repris pour le sujet suivant
  (un mouvement de caméra B vers C dans les Vignettes).
- Son sujet suivant, avec une capture annotée : « ajouter un mouvement de caméra dans Vignettes après une continuité de vignettes qui a
  déjà un mouvement A vers B. Ajouter possibilité de B vers C etc. » Chantier `camera-suite` (partie 142 de l'interface), branche du
  même nom dans le dossier `Chantiers\calques-place` réutilisé. Avant, un plan ne portait qu'un mouvement, sur sa première case, et une
  case de continuité refusait la caméra. Maintenant une case de continuité porte son propre mouvement : il part du cadre qu'elle montre
  (B) et va vers un nouveau cadre (C) sur la durée de la case ; la case d'après montre C et peut porter C vers D. Mêmes gestes que
  d'habitude, faits sur la case (Ctrl + Maj + glisser, le « + » de Maj, « Ajouter un cadre »). Les lettres courent d'un bout à l'autre du
  plan. Le premier cadre d'un mouvement de continuité est la case : il ne se déplace pas. Déplacer B emmène le départ de B vers C. Un
  mouvement créé sur une case du milieu met les cases de derrière dans un plan à part (la règle du 22 octobre). Le format du projet ne
  change pas : tout se lit dans les clés de caméra. Vérificateur sur la branche : 4 497 essais réussis, 0 échec, aucun avertissement.
  Captures de la planche regardées et envoyées. Exe de la branche (20 h 32) ouvert deux fois sur une copie fraîche de son projet avec de
  faux réglages : pas de plantage (une première construction avait échoué sur un caractère parasite dans un commentaire, corrigé ; les
  essais de la caméra repassés ensuite). Pas vérifié : aucun geste fait à la main dans le vrai exe ; la lecture à la japonaise
  et l'export de la planche n'ont pas d'essai propre à ce cas. Fusion dans main : à son accord.
- Fusion de `camera-suite` dans main (2066ecf, sans avance rapide) sur son « Fusionne directement » : `main` n'avait pas bougé,
  même contenu que la branche vérifiée. L'autre session prévenue (message déposé, sans réponse). Poussé sur GitHub. Son FEANOR
  était ouvert : fermé sans rien demander avant la construction. Exe principal reconstruit (`target\release\feanor.exe`, 20 h 57) et
  ouvert deux fois sur une copie fraîche de son projet avec de faux réglages : pas de plantage. Pas de version portable. Pas
  vérifié : aucun geste fait à la main dans le vrai exe (sa liste 142.4) ; la copie de projet ouverte n'a pas de case de continuité
  qui porte un mouvement. Les dossiers de chantier `Chantiers\cle-masques` et `Chantiers\calques-place` restent.
- Son retour après l'essai du mouvement B vers C, avec deux captures : « sur le plan 5 par exemple, je ne peux pas créer de mouvement de
  caméra depuis la deuxième vignette, il s'applique automatiquement à la première », et sa règle : « un mouvement de caméra peut démarrer
  d'une vignette même si la vignette n'est pas la première vignette du plan. Le mouvement de caméra démarre alors du début de la clé
  d'image et termine à la fin de la clé d'image ». J'avais gardé l'ancienne règle pour un plan qui ne bouge pas encore (le premier
  mouvement naissait sur la première case) : elle est retirée. Chantier `camera-case` (partie 142.6), branche du même nom dans
  `Chantiers\calques-place`. Le mouvement naît sur la case où on le demande, de sa première à sa dernière image, que le plan bouge déjà
  ou non ; le « + » de Maj sert toujours la case survolée ; un mouvement vit sur la case où il commence. Une case qu'un mouvement
  commencé ailleurs traverse refuse le geste et dit de quelle image à quelle image. Trouvé dans son projet (lu seulement) : la clé B de
  son plan 3 est à l'image 108, au-delà de sa première case (elle était à l'image 48 à 18 h 43) ; c'est pour cela que sa dernière case ne
  prenait pas de mouvement tant qu'il n'avait pas ajouté un cadre sur la première. Sur une copie fraîche de son projet : son plan 5 prend
  son mouvement sur la deuxième case (clés aux images 49 et 96), son plan 3 dit pourquoi. Vérificateur sur la branche : 4 500 essais
  réussis, 0 échec, aucun avertissement (un premier passage, la machine chargée, avait un échec sur un essai de capture d'écran qui passe
  seul et au second passage). Fusion dans main : à son accord.
- Fusion de `camera-case` dans main (2777b27, sans avance rapide) sur son « Fusionne directement » : `main` n'avait pas bougé,
  même contenu que la branche vérifiée. L'autre session prévenue (message déposé, sans réponse). Poussé sur GitHub. Son FEANOR
  était ouvert : fermé sans rien demander avant la construction. Exe principal reconstruit (`target\release\feanor.exe`, 22 h 26) et
  ouvert deux fois sur une copie fraîche de son projet avec de faux réglages : pas de plantage. Pas de version portable. Pas
  vérifié : aucun geste fait à la main dans le vrai exe (sa liste 142.6). À lui de voir : la clé B de son plan 3 est à l'image 108,
  au-delà de sa première case. Les dossiers de chantier `Chantiers\cle-masques` et `Chantiers\calques-place` restent.

## 2026-11-10

- Fusion de `camera-souvient` dans main (35a6e42, sans avance rapide), après l'essai d'Anthony dans la version portable du 9 novembre et
  son accord (« ok tu peux fusionner avec le reste ») ; l'autre session prévenue avant. `main` n'avait pas bougé depuis le départ de la
  branche : aucun conflit. Vérificateur sur main : 4 329 essais réussis, 0 échec, aucun avertissement, tous les groupes de fiabilité OK
  (828 s). Poussé sur GitHub. Exe principal reconstruit (`target\release\feanor.exe`) et ouvert sur une copie de projet avec de faux
  réglages : pas de plantage. Le dossier de chantier `Chantiers\camera-souvient` attend son accord pour être retiré.
- Son retour sur « afa » (« la planche ne prend plus le trait… le trait disparaît au lâcher ») : la cause était l'interrupteur « Peint
  sur l'aplat » resté allumé (vu dans sa vidéo d'écran), pas la fusion ; ses trois symptômes rejoués sur une copie de son projet. Sur son
  « Oui, corrige » : partie 133, branche `souvient-retours` (chantier `Chantiers\souvient-retours`, depuis main 3e2b090). Un trait posé
  sur le papier va au papier ; un trait qui ne laisse rien ne crée ni aplat, ni dessin vide, ni étape, ni plan vide ; la barre d'état
  dit pourquoi ; la barre du bas de la planche montre l'interrupteur allumé, un clic l'éteint. Vérificateur sur la branche : 4 339
  essais réussis, 0 échec, aucun avertissement, fiabilité OK. Exe de branche ouvert sur une copie de son projet avec de faux réglages :
  pas de plantage. Pas fusionné : attend son essai et son accord.
- L'autre session a fusionné `multi-calques` dans main (00a047e, partie 132) et l'a demandé : main fusionné dans `souvient-retours`
  (c7addc7), seul conflit dans docs/INTERFACE.md (132 puis 133, les deux gardées). Vérificateur : un premier passage sous forte charge
  (1 010 s) a vu échouer `camera_traits_tests::scrubbing_far_away_never_cuts_the_stroke_again` (aperçu réduit des effets fait en
  arrière-plan, sensible au temps) ; seul, il passe trois fois sur trois ; second passage complet : 4 385 réussis, 0 échec, aucun
  avertissement, fiabilité OK (416 s). Exe de branche reconstruit et ouvert sur une copie d'afa : pas de plantage. Ctrl+A des Vignettes
  laissé à l'autre session (vignettesel.rs, partie 134).
- Sur son « Fusionne directement » : `souvient-retours` fusionné dans main (cc9e505, sans avance rapide), l'autre session prévenue
  avant ; main n'avait pas bougé depuis 00a047e : aucun conflit. Vérificateur sur main : 4 385 essais réussis, 0 échec, aucun
  avertissement, fiabilité OK (985 s). Poussé sur GitHub. Exe principal reconstruit (`target\release\feanor.exe`, FEANOR n'était pas
  ouvert) et ouvert sur une copie d'afa avec de faux réglages : pas de plantage. Le dossier `Chantiers\souvient-retours` retiré avec
  son accord (« Oui, retire-le ») ; la branche est gardée.
- Audit à sa demande (« audit sévère s'il faut »), à partir de son compteur d'habitudes et de ses projets : `suivi/AUDIT-2026-11-10.md`,
  cinq constats, propositions 21 à 29, puis la 30 (« La feuille par axe ») en réponse à « si tu repartais de zéro ». Rien de codé.
- Sur son « fais la proposition 25 » : **on n'enregistre plus** (partie 135, branche `sans-enregistrer`, chantier
  `Chantiers\sans-enregistrer`, depuis main a17ad33). FEANOR écrit le projet tout seul dès que le stylet se lève (l'enregistrement
  de toujours, déclenché par `continu.rs`, silencieux) ; fermer et quitter ne demandent plus rien ; Ctrl+S pose un jalon ; un jalon
  automatique à chaque ouverture remplace « Ne pas enregistrer » ; Fichier > Jalons… pour nommer, revenir, supprimer. Un jalon est
  un projet complet en liens physiques (`jalons.rs`), qui ne prend presque pas de place. Pas pour un film partagé ni un projet sans
  dossier. Réglage « Enregistrer en continu » dans les Préférences. Le format ne change pas (52). Trouvé en relisant : chaque
  écriture réécrivait les réglages (liste des récents) et le titre de la fenêtre aurait clignoté ; corrigé. Vérificateur sur la
  branche : 4 406 essais réussis, 0 échec, aucun avertissement, fiabilité OK (975 s). Exe de branche lancé sur une copie d'« afa »
  avec de faux réglages : jalon d'ouverture posé en 1,5 s (300 fichiers, en liens), aucun fichier du projet changé, pas de second
  jalon au second lancement, arrêt brutal sans reste. Pas fait : le fichier de projet unique (étape à part), l'annulation qui survit
  à la fermeture. Pas fusionné : attend son essai et son accord.
- L'autre session a fusionné dans main `vignettes-ctrl-a` (d07678e, partie 134) puis `selection-tout-2` (d3662ef, partie 136) et l'a
  rappelé : main fusionné dans `sans-enregistrer` (d7f1761), seul conflit dans docs/INTERFACE.md (parties rangées 134, 135, 136).
  Vérifié dans le code reçu : ni fil, ni canal, ni travail de fond nouveau, donc rien à ajouter à `App::continu_retenu`. Vérificateur
  sur la branche : 4 437 essais réussis, 0 échec, aucun avertissement, fiabilité OK (785 s). L'exe de la branche n'a pas été
  reconstruit : la version portable du soir (dac56c7) reste celle de son essai. Toujours pas fusionné : attend son essai et son accord.
- Sur son « Fusionne directement » : `sans-enregistrer` fusionné dans main (6093d46, sans avance rapide), l'autre session prévenue
  avant ; main n'avait pas bougé depuis d3662ef : aucun conflit, contenu identique à la branche vérifiée. Vérificateur sur main :
  4 437 essais réussis, 0 échec, aucun avertissement, fiabilité OK (732 s). Poussé sur GitHub. Exe principal reconstruit
  (`target\release\feanor.exe`, FEANOR n'était pas ouvert) et ouvert deux fois sur une copie d'afa avec de faux réglages : pas de
  plantage, jalon d'ouverture posé au premier lancement (300 fichiers), aucun fichier du projet changé, pas de second jalon au
  second lancement, arrêt brutal sans reste. Il n'a pas essayé la version portable avant : le stylet pendant l'écriture continue
  reste à juger par lui, dans son FEANOR habituel. Le dossier `Chantiers\sans-enregistrer` retiré avec son accord (« Oui,
  retire-le ») ; la branche est gardée.
- Sa remarque sur les calques dans la timeline (« un peu bordélique », les bandes repliées « manquent parfois de lisibilité ») : relevé
  du code (parties 29 et 68, `pistes.rs`, `montagerows.rs`) et de ses projets en lecture seule (13 calques dans le plan le plus chargé
  de « retakes brawlydays », dix bandes de la même couleur, aucun nom donné par lui, 172 montées et descentes de calque au clavier).
  Huit propositions numérotées 31 à 38 (couleur par dossier, bandes et noms plus lisibles, dossiers visibles, calques cachés en une
  ligne, noms sans les taper, retrouver un calque par son dessin, pile qui ne saute plus, un plan à la fois) et une page de maquettes
  cliquables. Rien de codé : attend son choix par numéros.

## 2026-11-09

- La caméra se souvient (partie 131, branche `camera-souvient`, chantier `Chantiers\camera-souvient`) : à sa demande de repenser les
  Vignettes « quitte à repartir de zéro », cinq approches présentées (A à E) ; son choix : « Ok on essaye A ». **Un trait appartient tout
  entier à la case où le stylet s'est posé ; hors de toute case, il appartient au papier** : plus de découpe du trait au bord des cases.
  La planche montre chaque trait en entier, par-dessus les cases qu'il traverse (« la feuille ») ; le passe-partout montre le film (chaque
  case son plan) ; le plan, le montage et les exports du film restent coupés au cadre. La vignette d'une case s'étend à son encre (quatre
  cadres par côté au plus). La sélection de la planche (vectorielle et bitmap), le collage et la case qui naît reposent le trait entier là
  où il commence ; un vieux trait coupé redevient entier quand on le déplace. Le bout d'un trait ne fait plus naître la case pâle
  suivante ; la marge de la première case pâle la fait naître. La gomme de la planche efface aussi ce que les cases ont dessiné hors de
  leur cadre (`gommedebord.rs`). Le format du projet ne change pas. Mesure demandée par l'autre session (60 cases, 2 040 traits) : une
  image d'écran en 0,89 ms. Onze essais neufs (`camera_souvient_tests.rs`), les essais d'avant réécrits pour dire la règle neuve. Limites
  notées : caméra en perspective (le trait qui dépasse s'amincit), quatre cadres au plus, un débord ne se gomme pas depuis la case d'une
  autre. **Relecture indépendante** du code avant toute proposition de fusion (demandée par l'autre session) : quinze remarques, dont
  plusieurs sérieuses sur la gomme toute neuve (elle prenait un trait resté dans son cadre, ignorait le verrou du storyboard et le
  passe-partout) et sur les dessins montrés par plusieurs cases. Corrigé : ces dessins-là (le Décor, une planche-mère, un calque de toute
  la scène) gardent la règle d'avant, pour ne pas s'étaler autour de chaque case qui les montre ; un trait bitmap parti du papier n'emporte
  plus un double du dessin qu'il traverse ; la marge ordinaire d'une case ne disparaît plus dans un grand export. Pas fusionné : exe de
  branche à essayer par Anthony.
- À sa demande (« fusionner sur une version portable seulement sans altérer la version principale ») : version portable d'essai
  `Logiciel\Portable\FEANOR-portable-2026-11-09-camera-souvient` (dossier et zip), faite de l'exe de la branche (a9f3540). `main` n'avait
  pas bougé depuis le départ de la branche (6f27220) : cet exe est déjà le FEANOR habituel plus la partie 131, sans aucune fusion. Ouverte
  sur une copie de projet avec de faux réglages : pas de plantage. `main`, `Source` et son exe ne sont pas touchés.

## 2026-11-08

- Dépôt GitHub de sauvegarde, à la demande d'Anthony (« afin de ne rien perdre ») : il a créé le dépôt privé
  https://github.com/ferreanthony-glitch/Feanor lui-même ; `origin` est branché dessus dans `Logiciel\Source`, et tout l'historique est
  poussé (main, les 118 branches, le tag `archive/memoire-dessins`). Son premier commit fait en ligne (un README de deux lignes) est repris
  dans l'historique, le README détaillé du dépôt est gardé. La connexion passe par le gestionnaire d'identifiants de Windows, validée par lui
  dans le navigateur ; `gh` (l'outil GitHub en ligne de commande) est installé mais pas connecté. Avant de pousser : aucun secret dans les
  fichiers suivis (`clessecretes.rs` range les clés dans Windows, jamais dans un fichier).
- La case qui naît prend le papier (partie 128, branche `cases-prennent`, chantier `Chantiers\cases-prennent`) : réponse à ses deux captures
  (« les caméras non activées au début cachent le trait quand elles s'activent »). Une case pâle qui devient un plan (un trait dedans, la barre
  de continuité) prend les traits du papier (« Hors cadres ») qui passent dans son champ : la part dans le champ entre dans le dessin du plan,
  le reste reste sur le papier, par la repose de la sélection de la planche à l'identique (`caseprend.rs`, sans toucher aux fichiers de l'autre
  session) ; une seule étape d'annulation avec la création. Suites : les pixels bitmap du papier (après `vignettes-bitmap`), la case née par le
  bout d'un trait (`horschamp_creer_cases`, fichier de l'autre session), un plan bitmap qui naît. Sa réponse sur l'architecture : pas de « planche
  = dessin, case = caméra » (un plan ne doit montrer que ce qui a été dessiné dans son champ, pas toute la planche en baladant la caméra), et le
  Décor reste tel quel. Trouvé en passant et corrigé (`app.rs`, `clamp_nav`) : après Ctrl+Z d'une continuité, l'image courante restait au-delà
  de la fin du plan, et un trait dans la case des Vignettes partait dans une image neuve au bout du plan (qui s'allongeait), la vignette du
  montage restant vide. Quatre essais (`caseprend_tests.rs`). Fusionné dans main (b334410) sur son « Oui, fusionne » après son essai de l'exe
  de branche ; l'exe de main est reconstruit (son FEANOR ouvert arrêté sans enregistrer, comme convenu).
- Fusion dans main (aa03f7f) de `planche-mere`, à la demande d'Anthony (« Oui, fusionne »), après la fusion de `vignettes-selection` par l'autre
  session (6bcade3) : la planche-mère (partie 126, format de projet 51), sans sa partie Vignettes, qui vient dans un chantier suivant. Essai
  ajouté à la demande de l'autre session : un trait d'un dessin partagé montré par deux cases est pris une seule fois, le déplacer le déplace dans
  les deux cases, rien n'est dupliqué (limite : le lasso ne le trouve que dans la première case). Vérificateur sur la branche à jour de main :
  4259 réussis sur 4259, aucun avertissement. `feanor.exe` de main reconstruit ; le FEANOR d'Anthony, ouvert à 17:53, a été fermé sans
  enregistrer pour cela (sa règle). L'autre session fabrique le zip portable sur cet exe.
- La planche-mère (idée 4 d'Anthony ; branche `planche-mere`, chantier `Chantiers\planche-mere`, partie 126 ; conception validée par ses trois
  réponses : tout le plan, Storyboard d'abord, « Réutiliser » dans les commandes et rien de plus dans Aide) : un calque porte une clé de
  partage, ses exemplaires dans plusieurs plans montrent les mêmes dessins, chacun sous la caméra de son plan (`partage.rs`,
  `sync_partages` à chaque image comme les calques de toute la scène) ; « Réutiliser dans… » (plan suivant, précédent, séquence, plans
  choisis) et « Détacher » dans le menu d'un calque, maillon dans sa ligne ; « Réutiliser le plan » (un plan lié dans une scène neuve, même
  dessin, à cadrer) et « Détacher le plan » dans le menu Storyboard > Plans (les menus du montage restent à leur dizaine d'entrées). Couper
  garde le lien, réunir détache le second, dupliquer et coller ne lient pas. Format de projet 51. Fusion dans main après celle de
  `vignettes-selection` (6bcade3), à la demande d'Anthony (« Oui, fusionne »), sans la partie Vignettes (maillon sur les cases, trait entier au
  dessin partagé), qui vient dans un chantier suivant, à coordonner avec l'autre session (ses fichiers).
- Fusion dans main (7c7df04) de `finitions`, à la demande d'Anthony (« Fusionne finitions ») après son essai de la Trajectoire (« parfait ») :
  la trajectoire suit les images insérées ou retirées et peut faire tourner le dessin (partie 119, format 50), la licence de ffmpeg dans
  À propos et les trois touches (partie 124). Vérificateur sur la branche : 4235 réussis sur 4235, aucun avertissement. `feanor.exe` de main
  reconstruit. L'autre session (`vignettes-selection`) est prévenue. La planche-mère (branche `planche-mere`, partie 126) reste à part.
- Finitions (branche `finitions`, chantier `Chantiers\finitions`, à la demande d'Anthony) : la Trajectoire d'abord (partie 119) — **insérer ou
  retirer des images décale la trajectoire** (tout ce qui passe par `Layer::shift_frames` : le départ, la fin et chaque marque restent sur
  leur image, comme les clés et les dessins ; la fin est posée dès le tracé sur la dernière image du plan) et **« Le dessin tourne avec le
  chemin »** (case des réglages : il pivote autour du point d'ancrage en gardant son orientation du départ ; clés d'interpolation : l'angle
  sur chaque clé ; clés d'images : chaque copie tournée, bitmap rééchantillonné par `Raster::transformee`). Format de projet 50. Puis le
  petit lot V1 (partie 124) : la **licence de ffmpeg dans À propos** (ligne LGPL, dépliant avec le texte de la licence, la configuration
  exacte, le ffmpeg du poste, le dossier `ffmpeg-licence` de la version portable, les sources) ; **trois touches** proposées : Ctrl+Maj+D
  duplique le plan, Maj+C l'outil Caméra, Maj+K la clé de caméra ; les versions rangées d'une séquence vidée étaient déjà protégées
  (refus avec la raison, chantier `versions-fiables` du 5 novembre, partie 102) : rien à refaire. Remarque : la version portable du
  8 novembre ne joint pas l'archive des sources de ffmpeg que son `CONFIGURATION.txt` annonce. Non fusionné : attend son essai et son
  « Fusionne ».
- Vignettes : traits coupés autour des cases (capture d'Anthony). Cause : « Peindre derrière » allumé dans ses réglages ; depuis 6d89a00 (2026-10-30) un trait peint derrière n'était pas réparti et restait dans le champ de sa case (formes aussi). Pas une correction perdue en fusion (six fusions vérifiées fichier par fichier). Corrigé (a06d4c5, fusion 90f4db4), essai `vignettes_dessin_tests` rouge puis vert, vérificateur 4228 réussis deux passages. Portable `FEANOR-portable-2026-11-08.zip`.

## 2026-11-07

- Fusion dans main (760cba4) de `duree-cles` : effets au-delà de la feuille (partie 121), traits cachés (122), durée d'un plan et clés alternées (123), planche en tuiles exactes, Annuler sans gel (15 s → 0,2 s sur un gros cas, en essais automatiques), réparation de plusieurs causes d'essais instables. Relectures : gel à l'annulation, décalage à tort sur les anciens projets (marqueur `cles_v2`, essai sur un vrai fichier ancien) et trajectoire qui ne suivait pas Ctrl : réels, corrigés. Vérification finale 4226 réussis, deux passages. Portable `FEANOR-portable-2026-11-07-soir.zip`, ouvert sur une copie de projet. Incident sans perte : un script de renumérotation (alias `Ren` de PowerShell) a vidé 13 fichiers du worktree, restaurés depuis l'index avant commit.
- Fusion dans main (3a6cfc0) à la demande d'Anthony, après deux essais (« ça marche très bien ») : l'outil Trajectoire (partie 119,
  format de projet 49) et les poignées de lissage de la caméra au repos (partie 120). Suite complète sur la branche : 4133 réussis sur
  4134, un essai de temps différent à chaque passage, réussi seul. `feanor.exe` reconstruit. Le chantier `trajectoire` peut partir avec
  son accord.
- Les poignées de lissage de la caméra attendent à côté de chaque clé dès qu'un mouvement existe, au repos (à 0), et se tirent ; ramenées
  près de la clé, elles reviennent au repos (partie 120, branche `trajectoire`). Rien ne change dans le projet.
- L'outil Trajectoire (partie 119, branche `trajectoire`), l'idée d'Anthony : un trait trace le chemin du calque, de petits
  traits en travers posent les marques d'espacement comme sur une feuille de timing, et les clés de position se posent toutes seules, une par
  image, le long de la courbe. Marques glissées ou arrachées, chemin déplacé ou retracé, silhouettes pâles aux jalons, pas des marques (durée
  du plan partagée), détachement quand on retouche une clé à la main. Format de projet 49. Puis, à sa demande : la clé de départ démarre
  sur la tête de lecture, deux sortes de clés (interpolées, ou par images). Après son essai : les « clés d'images » sont de vrais dessins,
  un à chaque marque, copie du dessin du départ déplacée sur le chemin (la timeline se scinde) ; glisser une clé sur la timeline recale la
  trajectoire (fin, départ, temps d'une marque) au lieu de la détacher ; une fin explicite ; tirer sur le chemin le tord comme une corde
  (Maj : en bloc). Puis, « ça marche très bien » : les clés d'images deviennent le réglage d'origine, et « ce qui suit le chemin » se
  choisit (là où part le trait, le bas du dessin, le centre, le haut, un bord, ou un point cliqué). Et la trajectoire ne s'en va plus
  d'elle-même (une clé supprimée la détachait) : Suppr retire une marque, poser ou lisser une clé à la main est refusé, Transformer
  déplace le calque et son chemin ensemble. 25 essais. Reste son essai.
- Idées retenues par Anthony dans les vingt proposées : 1 + 8 (dessiner pendant la lecture, la page de relecture existant déjà), 3, 4, 7, 11,
  14, 15, 16, 18, 20 ; sa trajectoire remplace 13 ; 2, 5, 6, 9, 10 écartées. Rien d'autre n'est codé.
- Fusion dans main (6a06b1c) : plan écarté (partie 115, format 48), « + » de la peau de Maj pour un cadre de plus (116), poignées de lissage
  sur la piste caméra (117), traits coupés au bord de la feuille (118 : les effets, dont le tremblement par défaut, ne rendaient que la feuille).
  Relectures indépendantes : 8 défauts corrigés sur le plan écarté, 3 points corrigés sur les traits. Vérificateur : 4109 réussis, 0 échec, deux passages.
  `feanor.exe` reconstruit, ouvert sur une copie de projet. Rien d'essayé au stylet. Limites : secousse, onde, vignettage et effets du trait
  s'arrêtent au bord de la feuille ; colonne un peu plus claire au raccord sur carte graphique (4,5 % à l'intensité 0,5).
- Incident : un agent a écrit `camera_traits_tests.rs` dans Work\BRAWLSTAR\BRAWLIDAYS\3D par un chemin relatif. Signalé à Anthony, suppression à sa décision.
- Mac : reporté par Anthony tant que la version PC n'est pas peaufinée. Rien n'est envoyé sur GitHub.

- Fusion dans main (9819707) à la demande d'Anthony : le bouton du stylet est le clic droit, le stylet retourné prend la Gomme (partie 114,
  branche `stylet-branche`). `feanor.exe` reconstruit. Reste son essai à la tablette ; l'inclinaison attend.
- Le stylet branché (partie 114, branche `stylet-branche`) : d'après son relevé (bouton vu pointe posée, gomme vue, et « clic droit : 0 »
  côté FEANOR, winit jetant le bouton), le bouton du stylet devient le clic droit (réglages de l'outil, anneau) et le stylet retourné prend
  la Gomme. Réglage dans Préférences > Dessin. L'inclinaison attend.
- Fusion dans main (547b51f) à la demande d'Anthony : Mes habitudes et notes de friction (partie 112), banc du stylet (partie 114), après
  fusion de main dans la branche. Vérificateur sur le résultat : 4003 réussis, 0 échec, aucun avertissement. `feanor.exe` reconstruit et ouvert
  sans erreur sur une copie de TEST PELURE_effets. Reste son essai : F9, Aide > Mes habitudes, et le relevé du stylet à la tablette.

## 2026-11-06

- Le banc du stylet (INTERFACE partie 114, branche `stylet-entier`, posée sur `usage-frictions`, pas fusionnée) : Aide > Ce que le stylet
  transmet… écoute le stylet auprès de Windows (bouton, stylet retourné, gomme, inclinaison, rotation) sans rien changer au dessin, et le
  montre à côté de ce que FEANOR reçoit aujourd'hui. L'écoute n'est posée qu'à l'ouverture du banc. Reste l'essai d'Anthony à la tablette :
  son relevé (`%LOCALAPPDATA%\SHOT\stylet-releve.txt`) décide de ce qu'on branche (gomme au dos, inclinaison, bouton tenu).
- « Tout au stylet » : page de propositions publiée (trois règles, 23 gestes, six décisions à prendre). Rien de codé avant ses réponses.
- Mes habitudes et notes de friction (INTERFACE partie 112, branche `usage-frictions`, pas fusionnée) : un compteur local des commandes par
  chemin (clavier, icône, menu, anneau, palette), la fenêtre Aide > Mes habitudes, et F9 pour noter une friction avec une capture et les
  dernières commandes, dans `Documents\SHOT\Frictions`. Vérificateur : 3985 réussis, aucun avertissement. Reste son essai, puis la fusion.
- Traits continus (partie 111, c11f743) et Vignettes simplifiées par Fable (analyse `suivi/ANALYSE-VIGNETTES.md`, partie 113) fusionnés (f5caddd).
  Vérification complète : 3981 réussis, 0 échec, deux passages, aucun avertissement. Ouvert sans plantage sur une copie. Version FEANOR-portable-2026-11-06c.
  Reste : décision 6, plan annulé, essai au stylet.
- L'anneau sous le stylet est fusionné dans main à la demande d'Anthony (ef983ac) : `feanor.exe` reconstruit, ouvert sans erreur sur une
  copie de TEST PELURE_effets. Le contenu est celui que le vérificateur a passé sur la branche (3954 réussis, aucun avertissement).
  Reste son essai à la tablette ; le dossier de chantier `vignettes-passepartout` (2,4 Go) peut partir avec son accord.
- L'anneau sous le stylet (INTERFACE partie 111, branche `anneau-stylet`) : après la maquette essayée par Anthony au stylet,
  huit commandes autour de la pointe par le bouton du stylet, second anneau des pastilles, onglet Préférences > Anneau (commandes, délai),
  « Envoyer dans l'anneau » au clic droit d'une icône. Joue sur le dessin et sur la planche des Vignettes ; le clic bref garde les réglages
  de l'outil ou le menu de la case. 27 essais, rendu vu sur cinq captures. Non vérifié sans lui : que le bouton de son stylet arrive
  comme un clic droit tenu.
- Fusionnés aussi : passe-partout, numéros, flèche (partie 110) ; corbeille, format 47, gestes de montage sans casse, J/K/L (partie 104, 7a0c6ad).
  Vérification complète au calme, avec le ffmpeg allégé : 3927 réussis, 0 échec, deux passages, aucun avertissement, aucun plantage.
  Une relecture a trouvé deux pertes de dessins dans la corbeille avant fusion, corrigées. Version portable FEANOR-portable-2026-11-06b.
- Fusionnés dans main (7cac5ed, INTERFACE parties 101 à 103 et 105 à 109) : canevas libre et outil Caméra, versions, aplat, textes et séquences,
  brosses dédiées, références, exports à la clé, son par plan, nomenclature, color script. Suite complète : 3866 réussis, 1 échec instable connu
  (`vignettes_retouches_tests`, réussit 3 fois sur 3 seul). Reste : avertissements à corriger, coupe des captures 102 et 103 non reproduite,
  corbeille (format 47) et passe-partout en cours. Version portable FEANOR-portable-2026-11-06.

## 2026-11-05

- Plus d'onglet Animation : double clic sur un plan, bouton « Retour au storyboard » (fusion 9a9c77c, INTERFACE partie 100).
  ffmpeg allégé compilé dans `Logiciel\ffmpeg\allege` (8.1.3, 9 Mo, LGPL, sans OpenH264) et posé à côté de `feanor.exe` ; l'export pour le
  montage explique une taille refusée (80738d2). 3751 tests réussis en deux passages. Version portable FEANOR-portable-2026-11-05c.
  Reste : mention de licence dans À propos, double clic et bouton à essayer à la vraie souris.
- Contenu de la V1 écrit à partir des mots d'Anthony (`suivi/V1.md`), avec les écarts relevés dans le code. À corriger par lui ;
  il signale encore de la friction entre vignettes et storyboard, à détailler.
- ffmpeg 8.1 LGPL installé sur le PC d'Anthony, à sa demande : `Logiciel\ffmpeg\` et copie à côté de `target\release\feanor.exe`
  (FEANOR utilisait jusqu'ici celui de Krita). Essais d'encodage H.264, ProRes, DNxHR et GIF réussis ; 3745 tests réussis avec lui.
  Pas encore distribuable : 126 Mo, et OpenH264 inclus. Il faudra une version allégée sans OpenH264, ses sources et la mention de licence.
- Le logiciel s'appelle FEANOR : fusion fbd6a98 (nom, couleurs Atelier et Papier, Joyau, animation d'ouverture une fois,
  polices Big Shoulders et IBM Plex, barres irisées, rouge réservé aux erreurs bloquantes, point ambre pour les avertissements).
  L'exécutable devient `target\release\feanor.exe`. L'extension `.shot` et les dossiers `SHOT` restent.
- Mémoire d'une longue séance : fusion 9c3e516. 300 plans après 2000 gestes : de 6,5 à 5,8 Go (mesure de banc).
  Les dessins que plus rien n'utilise quittent la mémoire ; une relecture n'a trouvé aucun scénario de perte.
- Essais : 3743 réussis, 0 échec, deux passages sur la branche dont la fusion est identique. Projet d'essai rouvert avec feanor.exe.
- En attente de toi : tests à l'écran (INTERFACE parties 96, 97 et 98).
- Pas fait : focus clavier jade sur les boutons. Inexpliqué : 2,6 à 3 Go hors du tas, sans doute le pilote graphique.
- Sortes de calques en couleurs FEANOR, vectoriel en jade sombre (INTERFACE partie 98). Nuancier « FEANOR », texte copié « FEANOR : », ligne « Anciennement » retirée.
- Dossiers de travail `feanor` et `memoire-seance` et leurs branches supprimés, avec ton accord.
- Version portable FEANOR-portable-2026-11-05 (52a25c9), sans ffmpeg ; zip rouvert et lancé ici sur une copie du projet d'essai.
- Gris sombre redevient le thème de base, avec un trait jade sous l'espace actif, une pastille jade sur l'onglet non enregistré et les couleurs FEANOR des calques (INTERFACE partie 99). 3745 essais réussis, un passage complet. Portable FEANOR-portable-2026-11-05b.

## 2026-10-30

- Créé le dossier suivi/ : ETAT.md (état réel du projet), NOTES.md (ta boîte de notes), JOURNAL.md (ce fichier).
- Lancé cargo check et cargo test sur main (b201dc1) : résultats dans ETAT.md, section 1.
- Aucun fichier source modifié, aucun commit.
- Décision d'Anthony : SHOT abandonne tout lien avec Storyboard Pro et Toon Boom (import, export, import de
  pinceaux, raccourcis, références), pour pouvoir le commercialiser. Chantier lancé sur la branche `sans-sbp`.
- Engrenage de l'accueil gardé (choix d'Anthony). Dossier de travail `demarrage` supprimé ; `importsbp2` le sera
  après le chantier `sans-sbp`, qui l'utilise.
- Fusionné 17dda2a : SHOT sans Storyboard Pro ni Toon Boom. 3288 essais passent, shot.exe reconstruit
  (40,08 → 39,21 Mo, ~7 600 lignes de code en moins). Projet d'essai rouvert sans problème. Dossier `importsbp2` supprimé.
- Lancé : panneau « Scénario » (le scénario importé est gardé dans le projet et lisible, lié aux plans),
  branche `panneau-scenario`.
- Fusionné 32dd5f5 : panneau Scénario (3308 essais passent, shot.exe reconstruit, projet d'essai rouvert).
- Fusionné 5335c60 : ajustements des vignettes (3326 essais passent, shot.exe reconstruit, projet d'essai rouvert).
  Lancé : vignettes par séquences (branche `vignettes-sequences`).
- `vignettes-ajustements` rendu ; renvoyé pour 3 défauts vus sur capture (cadrage d'ouverture, zoom affiché
  deux fois, réglage « lignes d'un projet neuf » sans effet). Rotation du canevas des vignettes non faite : question à Anthony.
- Avant : `vignettes-ajustements` (continuité, planche dessinable, numéros, lignes de texte, réglages,
  passe-partout des cases, barre du bas). Suivant : vignettes par séquences.
- Décisions d'Anthony : pas de rotation du canevas des vignettes ; dossier de travail `scenario` supprimé ;
  recherche (Ctrl+F) dans le panneau Scénario lancée (branche `scenario-recherche`, dossier `Chantiers\recherche`).
- Fusionné 7e3f7e8 : vignettes par séquences (titres, choix et ordre des séquences affichées, export qui suit
  l'affichage ; 3350 essais passent sur la branche).
- Fusionné 4d8677d : recherche Ctrl+F dans le panneau Scénario (3360 essais passent, shot.exe reconstruit, projet
  d'essai rouvert). Ctrl+F reste « Aller à un plan » hors du panneau Scénario.
- Dossier de travail `recherche` supprimé (accord d'Anthony).
- Lancé : hors-cadres texturé + passe-partout à l'export (branche `vignettes-texture-export`).
- Fusionné e546fc7 : hors-cadres texturé + passe-partout à l'export avec sa propre case (3372 essais passent,
  shot.exe reconstruit, projet d'essai rouvert). À vérifier : l'export de la planche ne montre, pour une case à
  mouvement de caméra, que le cadre A (le dessin hors de A n'y est pas) ; préexistant, pas dû à ce chantier.
- Fusionné 3774514 : outil Dégradé plus complet (format du projet 43 → 44 ; 3400 essais passent). Renvoyé une fois
  avant fusion : un changement de calque en pleine retouche pouvait perdre le dégradé (corrigé, essais ajoutés).
- Fusionné 1df74a1 : planche de références (3425 essais passent, shot.exe reconstruit, projet d'essai rouvert).
- Décisions d'Anthony : post-it écrits directement sur la planche (chantier lancé, branche `postit-en-place`, dossier
  `Chantiers\references` réutilisé) ; cadre autour des éléments choisis : OK ; dossier `degrade` supprimé.
- Lancé : planche de références (branche `planche-references`, dossier `Chantiers\references`).
- Lancé : outil Dégradé plus complet (branche `degrade-avance`, dossier `Chantiers\degrade`).
- Point global (relevé en lecture seule) : solide = tests (3425), enregistrement sûr, compatibilité des projets,
  exports en fond, aucun plantage depuis le 22 oct. À améliorer, dans l'ordre : sept chantiers jamais vus à l'écran
  par Anthony ; ffmpeg non livré ; exports montage jamais ouverts dans un logiciel de montage ; limites d'export
  (cadre A seul, « ? » du PDF, son NTSC) ; menu Exporter chargé ; vocabulaire (Fiche / Propriétés du plan, Arrière-plan).
- Décisions d'Anthony : exports — les 4 familles (vidéo pro, images par plan + PDF, son par piste, montage +
  compositing), menu Exporter regroupé par usage (chantier `exports-v1` lancé) ; ffmpeg livré avec SHOT après
  vérification de la licence ; `memoire-dessins` abandonnée (étiquette `archive/memoire-dessins`).
- Lancé : panneau Bibliothèque (`panneau-bibliotheque`), fusion et menu des calques (`calques-menu`). Fait : logo
  retiré de la barre de menus (`sans-logo-barre`, 9c1d1d1, à fusionner).
- Fusionné 012c17a (post-it : écrire directement sur la planche) et 1c5a102 (logo retiré de la barre de menus) :
  3439 essais passent, shot.exe reconstruit, projet d'essai rouvert (Fichier est bien le premier menu).
- Fusionné bab0087 : numérotation verrouillée pour les projets neufs (décision d'Anthony, case dans Préférences), montage redimensionnable (poignée visible, défilement, hauteur des pistes son). 3564 essais passent, shot.exe reconstruit, projet d'essai rouvert. Public visé (Anthony) : storyboarders d'animation indépendants ou en studio, dans un pipeline de production (exports multiples, ponts vers Kitsu / ShotGrid).
- Fusionné abadcb0 : stabilité et retouches de l'audit (annulation de l'export des calques sans fichier tardif ; marquer un envoi ne renomme plus un .pelure ; bouton « Transformer en calque d'animation » ; Échap ferme Exporter ; fin d'export qui s'efface seule ; crayonnes.json « Mettre de côté »). 3550 essais passent (3 passages de suite chez l'agent, 1 chez moi), shot.exe reconstruit, projet d'essai rouvert.
- Fusionnés : exports V1 (c1594e9 : menu en 4 familles, vidéo pro, images par plan, PDF au choix avec accents, son par piste, FCPXML, calques séparés, caméra) et calques (1e6ff18 : fusion fidèle, en tâche de fond si longue, menu clic droit unique). shot.exe reconstruit, projet d'essai rouvert. Suite complète : 3539 à 3541 essais verts sur 3542 selon les passages ; deux essais instables sous charge (annulation de l'export des calques, enregistrement en tâche de fond) confiés au chantier `stabilite-essais` avec les petits bugs de l'audit pratique (envoi qui renomme, calque vidéo, Échap, messages d'export, crayonnes.json).
- Audit pratique à la souris : résumé ajouté à suivi/AUDIT-2026-10-30.md.
- Fusionné : panneau Bibliothèque (3476 essais passent, shot.exe reconstruit, projet d'essai rouvert sans plantage).
- Audit extérieur (storyboard série / long) rangé dans suivi/AUDIT-2026-10-30.md : 3/10 série, 4/10 long, 6/10
  indépendant ; priorités : valider les exports montage, PDF de studio, endurance sur un vrai épisode, équipe, modèle d'épisode.
- À retenir pour après la V1 (demande d'Anthony) : retravailler la partie collaborative (travail en équipe).
- Fusionnés (6040504) : fil de décision par plan (statut des notes, fil du plan, conflits, compteurs, note suivante,
  pastille, relecture n+1 avec réponses du relecteur, retakes + CSV) et premier contact (visite « Premier jour »
  en 5 étapes, Labo caché derrière « Afficher les fonctions expérimentales », Propager rangé dans Animation,
  vocabulaire : Fiche du plan, Fond de la feuille, Vectoriel, tutoiement). 3596 essais passent, shot.exe reconstruit,
  projet d'essai rouvert sans plantage. Format du projet inchangé (44).
- Fusionné 3cd07b7 : pipeline étape 1 (modèle de noms, série / épisode, liste des plans CSV/JSON profil Kitsu,
  paquet de livraison avec manifeste et empreintes, modèle d'épisode). Format du projet 44 → 45. 3627 essais passent,
  shot.exe reconstruit, projet d'essai rouvert. En-têtes Kitsu à confirmer sur une vraie instance.
- Lancés : petits livrables (`petits-livrables`), journal des différences entre envois (`journal-differences`),
  noms de marques retirés + « Papier » (`noms-papier`).
- Fusionné c442f0e + 46d46c4 : plus aucun nom de marque dans l'interface (noms de formats gardés), « Papier » partout
  (anciens noms trouvés par Ctrl+K). 3633 essais passent, shot.exe reconstruit, projet d'essai rouvert.
- Lancés (choix d'Anthony) : pont Kitsu étape 2 (`pont-kitsu`, faux serveur local) et test d'endurance (`endurance`).

## 2026-10-31

- Fusionnés (6fd6808, 634ffec, c0e8bba, 1353610, 7dfac45) : petits livrables (budget de durée, rapport de
  numérotation, planche PDF légendée, PDF de studio, fenêtres dans le dossier du projet, bobine EDL `S01P012A`),
  journal des différences entre envois (PDF + page HTML), pont Kitsu (publier, lire les retours ; adresses à confirmer
  sur une vraie instance), endurance (épisode de 200 plans mesuré, docs/ENDURANCE.md ; enregistrement et copie de
  secours sous la priorité de l'interface). Format du projet 45 → 46. 3701 essais passent, shot.exe reconstruit,
  projet d'essai rouvert sans plantage.
- Endurance, restes : mémoire 6,8 Go à l'ouverture d'un épisode de 200 plans (9–10 Go après une séance) ; dessins des
  plans supprimés gardés jusqu'à la réouverture ; envoi 90–110 ms sur le fil de l'interface ; MP4 lourd (2,7 Go / 15 min).
- Lancé (accord d'Anthony) : chantier mémoire des longs épisodes (branche `memoire-episode`, dossier
  `Chantiers\exports` réutilisé) ; but : 300 plans sous ~4 Go, sans perte ni latence.
- Fusionné 5f8c27a : mémoire des longs épisodes (dessins non regardés rangés en mémoire sans perte, aussi
  l'historique, les caches de traits, les plans supprimés, les onglets cachés ; marquer un envoi par morceaux).
  200 plans : 6,9 → 2,0 Go à l'ouverture, 10,1 → 5,7 Go après 2 000 gestes ; 300 plans : 9,0 → 2,3 Go à l'ouverture.
  Relecture ciblée (second agent) : aucune perte de dessin ; 4 remarques corrigées avant fusion (38528d9). Format
  inchangé (46). 3719 essais passent, shot.exe reconstruit, projet d'essai rouvert (même écran qu'avant).
  Essai intermittent `fermeture_tests::a_rich_project_opened_and_left_idle…` : panique audio (cpal, COM) quand la
  machine manque de mémoire, pas un défaut des dessins.
- À décider avec l'œil d'Anthony : affichage Direct3D 12 au lieu de Vulkan (−2,6 Go, lecture 3× plus rapide selon
  l'agent ; `WGPU_BACKEND=dx12`), jamais vérifié à l'écran.
- Dossiers de travail vignettes, bibliotheque, calques, references, exports supprimés et leurs 6 branches fusionnées
  effacées (accord d'Anthony).

## 2026-11-01

- Storyboard Pro fermé sans enregistrer (accord d'Anthony) : il occupait 14 Go, et Windows refusait d'ouvrir de nouvelles
  fenêtres SHOT.
- Affichage Direct3D 12 : il n'est pas compilé dans SHOT. `WGPU_BACKEND=dx12` fait passer à OpenGL, donc les chiffres
  « Direct3D 12 » du chantier mémoire n'étaient pas ceux de Direct3D 12. Docs corrigées. Les raccourcis de comparaison ont été
  retirés. Activer Direct3D 12 demanderait de télécharger deux crates.
- Téléchargé, avec l'accord d'Anthony : gpu-allocator, range-alloc et presser, depuis crates.io. Direct3D 12 est compilé
  (branche `affichage-dx12`).
- Vraie mesure sur l'épisode de 200 plans, après 120 changements de plan : 3,6 Go avec Direct3D 12, 3,9 Go avec Vulkan. Direct3D 12
  est un peu moins fluide. Vulkan reste par défaut ; Direct3D 12 sert de repli avant OpenGL.
- Fusionné 0bc89a7 : 3719 essais passent, shot.exe reconstruit, projet d'essai rouvert (même écran, Vulkan).
- Avec l'accord d'Anthony : dossier Chantiers\dx12 et branche affichage-dx12 supprimés.

## 2026-10-29

- Fusionné dans main : import Storyboard Pro (dessins, caméras, son, puis rendu et vitesse), clés d'opacité et calque
  « Couleur unie », démarrage sans projet ouvert, fenêtre d'accueil compacte.
- Projet Storyboard Pro lourd : la mémoire d'un animatic importé passe de 20,7 à 10,8 Go, l'import de 181 s à 17 s
  (mesures de l'agent, banc sans fenêtre). À confirmer par ton test sur ton fichier de 7 plans.
- Version portable SHOT-portable-2026-10-29b.
- En attente de toi : garder ou retirer l'engrenage de l'accueil ; suppression des dossiers de travail `demarrage`
  et `importsbp2`.
