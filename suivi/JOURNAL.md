# Journal de Claude

Ce que Claude a fait ou proposé, par session, le plus récent en haut. Une session = une date.
Pour le détail technique, voir l'historique git et docs/INTERFACE.md (une partie numérotée par chantier).

## 2026-11-20

- **Les couleurs d'identification du décor 3D, et les aspects Couleurs et Argile** (partie 199, étape 4 sur 5 de la maquette,
  branche `couleurs-3d` partie de main d9c6788, format 61 inchangé) : une couleur par objet du décor (dix de la maquette ou
  « Aucune »), posée par la puce de sa ligne dans Scène 3D ou en haut de Propriétés > Objet, une étape d'annulation, enregistrée,
  gardée par la copie et la duplication ; l'aspect de la vue (Matières · Couleurs · Argile, barre de la vue de l'onglet 3D, un
  réglage retenu, pour toutes les vues 3D, jamais les cartes) ; « Dans les plans » (Propriétés > Décor et une bascule dans la barre
  au-dessus du dessin du Storyboard), dans la clé de l'image, les vignettes et l'export ; l'œil et le cadenas d'un objet enregistrés
  dans le projet et annulables. Aucun maillage refait. Essais : `couleurs3d_tests.rs` (8). **Rien regardé à l'écran, pas de
  commit** (liste 199.4).
- **L'onglet 3D** (partie 198, étape 3 sur 5 de la maquette `onglet-3d-v8`, branche `onglet-3d` partie de main 25c1084, format 61
  inchangé) : un onglet « 3D » entre Storyboard et Montage, la vue Espace 3D d'aujourd'hui en grand sur la séquence courante, sa
  barre (cinq points de vue, 1, 2 ou 4 vues, ombres, grille, cartes, caméras des plans, soleil, recadrer), la boussole cliquable,
  les outils de la vue seuls à gauche, la pile Scène 3D (le sommaire de la séquence) et Propriétés (Objet, Caméra, Lumières, Monde,
  Décor, qui suit ce qui est choisi), la timeline de la séquence et la banque 3D en bas. Essais : `onglet3d_tests.rs` (10).
  **Rien regardé à l'écran, pas de commit** (liste 198.4). Puis, vu à l'écran : les boutons de la bande (Timeline de la séquence,
  Banque 3D) ; 198.5 : « Tree oak » (196 triangles) jugé lourd parce que chronométré pendant l'étalonnage (la lecture l'attend, et un
  modèle loin sous le budget n'est plus jugé par un temps), les aperçus de la banque cadrés, les noms entiers de l'onglet Monde.
- **Le panneau Scène du Storyboard** (partie 197, étape 2 sur 5 de la maquette `onglet-3d-v8`, branche `scene-storyboard` partie de
  main 62620e6, format 61 inchangé) : un panneau de la pile, en haut de celle d'origine du Storyboard (Scène, Couleur, Propriétés),
  « + Panneau » dans l'Animation ; « Depuis : Film · Séquence · Plan » et le fil d'Ariane ; l'arbre de la colonne de la timeline
  sans le temps (dossiers, pastille, signe, nom, cadenas, œil par le code de la timeline), le calque courant déplié en la ligne
  complète de la timeline elle-même, le menu du clic droit de la timeline, le glisser de la timeline ; seules les lignes visibles
  peintes, l'arbre refait seulement quand il change. Essais : `scene_tests.rs` (10). **Rien regardé à l'écran, pas de commit**
  (liste 197.4).
- **Deuxième tour des retouches vues à l'écran** (partie 196, même branche `colonne-panneaux`, format 61 inchangé) : l'aperçu des
  Préréglages un tiers plus petit, au milieu, et fixé en haut pendant que la liste défile ; la ligne Séquence de la timeline du
  Storyboard deux fois plus fine (10 points) et sans icônes, la ligne Caméra sans icônes, leurs commandes au clic droit de la ligne ;
  les lignes Séquence et Plans restent en haut de la bande quand les pistes défilent ; la barre de la vue commence par la vue
  caméra, puis « 2D » et « 3D » en mots. Essais : `retouches196_tests.rs` (6), huit fichiers d'essais changés exprès (liste 196.2).
  **Rien regardé à l'écran, pas de commit** (liste 196.4). Puis 196.5 (vu par le coordinateur à 1384 × 868, 90 %) : la Couleur
  n'est plus écrasée dans la pile ; un panneau garde la hauteur de son contenu, la colonne défile quand tout ne tient pas,
  Propriétés a le reste (120 points au moins), une liste ajoutée commence à 230 et ne cède que pour ces 120.
- **Retouches de la partie 194 vues à l'écran** (partie 195, même branche `colonne-panneaux`, format 61 inchangé) : barres de la vue aux
  icônes de la timeline, 16 points d'origine pour les deux (« Taille d'origine 16pt ») avec les mêmes marges, interface à 90 % d'origine (« Par défaut »), Préréglages en un aperçu et une liste de noms,
  règle des temps et compteur toujours visibles quand la bande défile, Ctrl + clic sur le compteur (temps ou images, retenu), plus
  de vidéos d'explication hors du menu Aide, Couleur sans « Avant · maintenant », nouvelle icône du passe-partout, zoom et
  inclinaison en curseurs sous le dessin, curseurs plus fins, « Pinceau » et « calque vectoriel » séparés, Propriétés gardant le
  tiers de la colonne, Ctrl + clic qui choisit l'objet (stylet qui tremble, encre d'un autre calque) avec tous les outils de dessin.
  Essais : `retouches195_tests.rs` (11), Ctrl + clic dans `dessin_notes_tests` ; 20 essais de l'aide animée ignorés exprès ; la suite
  complète a les 90 échecs connus du Mac, aucun neuf. **Rien regardé à l'écran, pas de commit** (liste 195.4).
- **La colonne en pile de panneaux, façon Blender** (partie 194, étape 1 sur 5 de la maquette `onglet-3d-v8` validée par
  Anthony ; branche `colonne-panneaux` partie de `main` 831e997, format 61 inchangé, disposition des réglages en version 10). La
  colonne de droite est une seule pile, dans les quatre espaces : chaque panneau a son en-tête (poignée ⠿ à glisser, chevron et
  titre qui replient, ▲ ▼ ✕), un bord du bas à tirer, un seul grandit (Propriétés, sinon le dernier ouvert) ; « + Panneau » en
  haut ; **Propriétés** montre les panneaux de réglages en onglets d'icônes verticaux, ↗ sort l'onglet devant en panneau, ✕ l'y
  remet. La barre des panneaux du bord droit et son action `vue.barre_panneaux` sont retirées ; Affichage > Panneaux et la
  palette ouvrent toujours chaque panneau. Chaque espace garde sa pile ; la bande du bas a toute la largeur sous la colonne ; les
  dispositions de la version 9 se convertissent sans perdre un panneau. egui_dock garde la racine (dessin | colonne, bande), la
  colonne est un seul panneau neuf, `Pane::Colonne`, qui dessine sa pile (`colonne.rs`, `colonneui.rs`) et appelle le contenu
  d'avant tel quel. Essais : `colonne_tests.rs` (14) ; essais changés exprès listés dans la partie 194.3 ; la suite complète a
  les 90 échecs connus du Mac, aucun neuf ; la version optimisée se construit. **Rien regardé à l'écran, pas de commit**
  (liste 194.5 ; garder une copie de `reglages.toml` avant l'essai).
- **Les bords doux entre le décor 3D et les cartes** (partie 193, branche `bords-doux` partie de `main` 41fa82f, format 61
  inchangé), le choix d'Anthony (« Bords d'occlusion doux »), puis allégée sur son « Allège ». Le bord d'un objet du décor qui passe
  devant une carte, la ligne où une carte inclinée entre dans un objet et le bord du décor contre le ciel ne font plus d'escaliers :
  **lissés sur un pixel**, à l'écran, dans les exports (carte prêtée et composition par le processeur, d'accord à un niveau près), les
  vignettes, la pipette et la vue Espace 3D ; l'intérieur tout caché ou tout visible comme avant, la tolérance de la partie 183 la
  même, « Toujours devant » et accroché jamais cachés, les ombres des cartes exactement celles d'avant. La profondeur du décor est
  dessinée en plus en **quatre échantillons par pixel** (une passe de la profondeur seule à quatre échantillons après la passe de
  couleur, inchangée), **seulement quand une carte du plan la partage et que le décor tient dans le budget avec elle** : sinon le
  chemin d'avant, au coût d'avant. Une carte qui décoche « Toujours devant » ne fait faire que cette passe, sans redessiner le décor ;
  qui la recoche, rien. L'étalonnage mesure son surcoût à part : le budget d'un décor reste celui d'avant (0,9 à 1 M triangles sur le
  Mac), les bords doux jusqu'à 0,7 M, au-delà des bords nets ; le panneau dit les deux ; un budget gardé se remesure une fois. Coût
  des bords doux : + 0,3 à 0,6 ms par rendu du décor à la feuille 4K, la composition inchangée, l'export + 1 à 2 ms par image. Toute
  la passe lissée a été essayée puis écartée (2,5 à 5 fois plus chère avec les ombres). Essais : 12 nouveaux ; les 90 échecs connus du
  Mac, aucun neuf ; la version optimisée se construit. Compilé et essayé sur le Mac seulement ; **rien regardé à l'écran, pas de
  commit** (liste 193.6).
- **La timeline resserrée** (partie 192 dans INTERFACE.md, appelée 179 dans les commits de `timeline-serree`) fusionnée dans
  `main` puis poussée (9b63079), après trois intégrations du `main` de GitHub (parties 180 à 191 de la session du décor 3D, format 61) :
  barre de lecture tout en haut, vraie ligne Séquence, commandes des calques justifiées à droite sur « Calques », tous les calques
  sur deux rangées d'origine (« Replier les calques » en option), une seule taille d'icônes, Préférences > Tailles (taille de
  l'interface, des icônes, hauteur et couleur des lignes Séquence / Plans / Caméra / Calques, en listes déroulantes alignées),
  groupe / sens du pochoir / position verrouillée passés au clic droit > Apparence. « Dupliquer la séquence » ajouté au ⋯ de la
  ligne Séquence. Vérificateur complet OK sur l'intégration des parties 180 à 190 (seul l'avertissement `decor3d_objets_vises`,
  venu de GitHub, tu ensuite) ; essais ciblés OK après la partie 191. FEANOR habituel reconstruit (0 h 26), contrôle de lancement OK.
- En attente d'Anthony : son essai à l'écran (checklist 192.x), le premier clic perdu dans les Préférences (vu sur le banc seulement).

## 2026-11-19

- **Dupliquer la séquence** (partie 191, branche `dupliquer-sequence` partie de `main` b27db77, format 61 inchangé), le choix
  d'Anthony (« Dupliquer la séquence »). Une commande neuve, `sequence.dupliquer`, sans touche d'origine (aucune n'est d'usage) :
  menu ⋯ de la timeline du Storyboard, clic droit sur la bande d'une séquence du montage et menu ⋯ de son en-tête, titre et liste
  des séquences de la planche, menu « Séquence » d'une case, menu Storyboard, palette Ctrl+K. **Juste après l'originale**, une
  séquence neuve qui en est une **copie indépendante** : mêmes plans (durées, noms, calques, dessins au pixel près, caméra, textes,
  couleurs), avec leurs **versions** (partie 168), commentaires, contenu rogné, transitions internes, liens des références, notes et
  annotations de la planche ; **la coupe de sortie** garde sa transition (posée à la fin de la copie), la coupe originale → copie est
  franche ; les **autres versions de la séquence** (A, B…) avec un garage à elle ; **le décor 3D** copié par
  `copier_decor_de_sequence` (partie 190), les modèles partagés ; la planche-mère liée entre copies sous une clé neuve, jamais avec
  l'original. Nom « Poursuite (copie) », puis « (copie 2) »… ; sans nom, sans nom. Une étape d'annulation ; la copie devient la
  séquence courante. Pas copiés : le son (en temps du montage), les plans écartés. Essais : 13 nouveaux ; les 90 échecs connus du Mac, aucun neuf ; la version optimisée se construit. Compilé et
  essayé sur le Mac seulement ; **rien regardé à l'écran, pas de commit** (liste 191.5).

- **Copier les objets du décor 3D** (partie 190, branche `copier-decor` partie de `main` 4481e66, format 61 inchangé), le choix
  d'Anthony (« Copier les objets du décor »). Après un clic dans la vue Espace 3D ou dans le décor du panneau, **Ctrl+C, Ctrl+X,
  Ctrl+V** (les commandes de toujours) visent les objets du décor choisis : collés dans la même séquence, **à côté** ; ailleurs (une
  autre séquence, un autre projet ouvert, après Ctrl+X), **à la même place** ; l'aimant repose ensuite les objets et les cartes
  aimantées ; une étape d'annulation ; plusieurs objets à la fois (Maj+clic dans la vue, Ctrl/Maj+clic dans la liste). Le modèle est
  partagé (ni relu ni renvoyé à la carte) ; dans un autre projet il entre et s'enregistre sous son empreinte, ou reprend le modèle de
  même contenu qui y est déjà. Le panneau : Copier, Couper, Coller, **« Copier vers la séquence… »** et **« Envoyer vers la
  séquence… »**. Ailleurs (dessin, calques, effets, plans, clés), Ctrl+C / Ctrl+V gardent leur sens ; la dernière copie est celle qui
  se colle. FEANOR n'a pas de commande qui duplique une séquence : les deux qui en font naître une des plans d'une autre (« Couper la
  séquence ici », « Nouvelle séquence avec les plans choisis ») lui donnent maintenant **une copie indépendante du décor**. Un collage
  qui ferait dépasser le budget de la machine est refusé. Essais : 13 nouveaux ; les 90 échecs connus du Mac, aucun neuf ; la version optimisée se construit. Compilé et essayé sur le Mac
  seulement ; **rien regardé à l'écran, pas de commit** (liste 190.5).

- **Les ombres des cartes et du décor 3D dans la vue Espace 3D** (partie 189, branche `ombres-espace3d` partie de `main` 9c33ec9,
  format 61 inchangé), le choix d'Anthony (« Ombres dans l'Espace 3D »). La vue Espace 3D (plein écran, à côté, vues multiples, y
  compris orthographiques) montre maintenant les ombres de la partie 185 : **une carte porte l'ombre de son encre sur le décor**,
  **le décor et les autres cartes ombrent les cartes**, sous le même soleil, avec les mêmes cases et les mêmes calques sans ombre,
  justes de tous les côtés (les ombres du soleil sont dans le monde). Les cartes de la vue (leurs coins, leur texture d'egui) sont
  vues du soleil dans une carte d'ombres partagée par les vues et gardée tant que rien ne bouge ; le décor de la vue la lit (un
  pipeline de plus, le même `eclairer`) ; une carte qui reçoit est repassée juste après son dessin, sur le même maillage, et l'ombre
  est retranchée de l'écran. Le calcul est celui de l'image du plan, le même texte de shader (`gpu::OMBRES_COMMUN`, sorti de la
  composition sans la changer). Sans les ombres du décor : la vue d'avant au pixel près, rien de plus à faire. Mesures (Mac, huit
  personnages, 180 000 triangles, vue de 1533 × 559) : +0,2 ms, +0,3 à 0,4 ms quand les cartes bougent (une carte grossière évite au
  décor son filtre là où aucune carte ne peut l'ombrer : sans elle, +2,8 ms en hautes). Essais : 10 nouveaux (dont un de mesure, ignoré) ;
  les 90 échecs connus du Mac, aucun neuf. Compilé et essayé sur le Mac seulement ; **rien regardé à l'écran, pas de commit** (liste 189.6).

- **Les branches rangées dans `main`** (sur son « Ranger les branches ») : le `main` de GitHub (fa23204, la timeline du
  Storyboard faite sur le PC, partie 178) réuni dans le `main` local (c9c24f6 : 162, 163, 178 la plage de lecture, 179), puis
  `banque-3d` (parties 180 à 185 : décor 3D, banque, origine et aimant, profondeur partagée, poignées dans la vue, ombres). Seuls
  INTERFACE.md et ce journal se contredisaient : les deux côtés gardés. Numéros en double renumérotés, déplacés à la fin
  d'INTERFACE.md : 162 les raccourcis qui manquaient → **186**, 163 l'Espace 3D à côté → **187**, 178 la plage de lecture →
  **188** (le 178 reste à la timeline du Storyboard, déjà sur GitHub). Essais sur le Mac seulement ; rien regardé à l'écran.
- **Les ombres des cartes et du décor 3D** (partie 185, branche `ombres-cartes` partie de `banque-3d` 9c7d444, format 61 inchangé),
  le choix d'Anthony (« Ombres cartes / décor »). Sous le soleil du décor : **une carte porte l'ombre de son encre** (pas de son
  rectangle) sur le sol, les dalles, les objets et les autres cartes ; **le décor ombre les cartes** (un personnage s'assombrit à
  l'ombre d'un arbre) ; une carte n'est jamais dans sa propre ombre. Le rendu du décor reste gardé : sa passe garde en plus la part
  du soleil à chaque pixel (une troisième cible, seulement avec ses ombres), les cartes qui portent une ombre sont dessinées vues du
  soleil dans une carte d'ombres à part, et tout se fait à la composition (sur le décor, retirer le soleil comme il le retire de ses
  faces ; sur une carte, lire les deux cartes d'ombres le long de son plan). Image du plan à l'écran et exports (le décor rendu par
  la carte prêtée ; le processeur fait le même calcul) ; pas dans la vue Espace 3D, ni dans le rendu de secours sans carte (qui n'a
  pas d'ombres du tout). Deux cases par calque, **cochées d'office** : « Porte une ombre », « Reçoit les ombres » (menu du calque,
  panneau de la carte, palette ; écrites seulement décochées, `VERSION` inchangée) ; jamais d'ombre pour un calque accroché, d'effet,
  d'annotation ; « Toujours devant le décor 3D » garde ses ombres. Sans décor ou sans ses ombres, les mêmes images par les mêmes
  chemins. Mesures (Mac, feuille 4K, huit cartes) : rien de mesurable pour huit personnages, +1,5 à 2,5 ms pour huit cartes pleines
  (le pire cas) ; le filtre des cartes a été réduit à une lecture filtrée sur ces mesures. Essais : 13 nouveaux (dont un de mesure,
  ignoré) ; les 90 échecs connus du Mac, aucun neuf. Compilé et essayé sur le Mac seulement ; **rien regardé à l'écran, pas de
  commit** (liste 185.6).

- **Les poignées dans la vue** (partie 184, branche `poignees-3d` partie de `banque-3d` 343a061, format 61 inchangé), le choix
  d'Anthony. Dans la vue Espace 3D, un **objet du décor 3D se choisit au clic** (sur ses vrais triangles ; le plus proche de l'œil,
  carte ou objet, l'emporte ; Maj mêle cartes, objets et caméra), choisi aussi dans la liste du panneau et l'inverse, et **se manie
  avec le même manipulateur** autour de son origine : déplacer, tourner (autour de la verticale seulement), mettre à l'échelle (d'un
  bloc), ou le tirer lui-même sur le sol ; **une étape d'annulation par geste**, par le chemin de la retouche du panneau (les
  cartes aimantées posées dessus suivent, Ctrl+Z ramène les deux). Un objet aimanté garde la hauteur de l'aimant : sa flèche Y est
  grise, comme le champ du panneau. **« Modifier l'origine »** (une case de la colonne des outils de la vue,
  `espace3d.modifier_origine` sans touche) : le manipulateur déplace l'origine seule d'un calque (dans le plan de sa carte, l'image
  ne bouge à aucune clé) ou d'un objet (dans l'espace, l'objet ne bouge pas), Maj l'accroche aux points de la boîte, l'aimant
  repose ensuite par la nouvelle origine. Poignées refusées grises, la barre d'état dit pourquoi. Suppr n'enlève pas un objet dans
  la vue (elle n'enlève pas non plus une carte : le bouton « Enlever » du panneau reste le chemin). Essais : 15 nouveaux ; les 90
  échecs connus du Mac, aucun neuf. Compilé et essayé sur le Mac seulement ; **rien regardé à l'écran, pas de commit** (liste
  184.5).

- **La profondeur partagée entre le décor 3D et les cartes** (partie 183, branche `profondeur-partagee` partie de `banque-3d`
  d3b7ab0, format 61 inchangé), le chantier choisi par Anthony. Les cartes et les objets du décor se cachent selon leur vraie
  distance à la caméra : un personnage passe derrière un arbre et devant un banc, une carte inclinée qui traverse un objet en sort,
  rien n'est rogné aux pieds d'une carte aimantée sur une dalle. Le décor garde sa profondeur (une seconde cible de sa passe sur la
  carte graphique, son tampon de profondeur sur le processeur et à l'export) ; chaque carte porte son plan, et un pixel de carte est
  caché où le décor est plus près (une lecture par pixel d'encre). Image du plan à l'écran, exports, rendu de secours, et vue
  Espace 3D (un voile du décor posé après chaque carte). Jamais cachés : calques accrochés à la caméra, d'effet, d'annotation, et
  la case **« Toujours devant le décor 3D »** (menu du calque, panneau de la carte, palette ; `devant_decor_3d`, sans changer
  `VERSION` : 61 n'est encore sur aucune version distribuée). Trouvé en route : la vue de dessin ne montrait pas le décor (seuls
  les sorties et la vue Espace 3D) ; il y est maintenant. Sans décor, les mêmes images par les mêmes chemins. Essais : 13 nouveaux
  (et un de mesure, ignoré) ; les 90 échecs connus du Mac, aucun neuf. Mesures (Mac) : la passe du décor avec sa seconde cible, comme avant dans le bruit ; la composition, +1,0 à 1,4 ms
  pour huit cartes pleines d'encre sur une feuille 4K (le pire cas), rien de mesurable pour huit cartes de personnage ; à refaire
  sur son PC. Compilé et essayé sur le Mac seulement ;
  **rien regardé à l'écran, pas de commit** (liste 183.6).

- **L'origine d'un calque, son vrai pivot** (partie 182, branche `origine-calques` partie de `decor-3d` 319e817, **format 61**), sur
  le plan qu'Anthony a validé. Un point du dessin que l'on place (outil Transformer : le cercle à croix, Maj s'aimante à la boîte ;
  champs x, y ; menu « Origine » ; Plus > Origine) : la pose fixe et chaque clé tournent, grandissent et s'étirent autour de lui ;
  le déplacer ne déplace jamais le dessin (sur la feuille, et dans l'Espace 3D pour une carte inclinée). Ancrage « L'origine du
  calque » des trajectoires, d'office pour les nouvelles (sur la feuille et dans l'espace) ; le manipulateur de l'Espace 3D tourne
  autour de l'origine ; « Aimanté au sol » pour les cartes, et sur sa règle : décoché, la carte reste où l'aimant l'a mise.
  Sans origine, les mêmes images au bit près. Compilé et essayé sur le Mac seulement ; **rien regardé à l'écran, pas de commit**
  (liste 182.4).

- **La banque 3D dans la Bibliothèque, et un décor fait de plusieurs objets** (partie 181, branche `banque-3d` partie de
  `decor-3d` 319e817, format 60 inchangé, aucune dépendance neuve). Le décor d'une séquence devient une liste d'objets (importer
  **ajoute**) ; la Bibliothèque a un onglet **3D** : les 45 modèles Kenney (CC0) par famille, recherche, bulle ; clic ou
  glisser (dessin : devant la caméra ; vue Espace 3D : au point du sol) les pose par le même chemin qu'un fichier ; budget de la
  machine pour la somme des objets (fenêtre « Ce décor est lourd » ou refus, jamais de ralenti). Les objets d'un même contenu
  partagent un fichier (`decors/<empreinte>.glb`) et un envoi à la carte ; `decors3d.json` en version 2, un fichier de la partie
  180 se lit comme un objet à la même place. Sur ses demandes en cours de route : une **origine** par objet (changer l'origine ne
  bouge pas l'objet) et **« Aimanté au sol »** (posé sur le sol ou sur les objets dessous, sans pénétration, calculé aux retouches
  seulement ; décoché, il reste exactement où l'aimant l'avait mis, même après réouverture). Banque rangée dans le programme
  (`include_bytes!`, 1,22 Mio de données, +1,47 Mio sur l'exécutable du Mac) ; essais : les 90 échecs connus, aucun neuf ; crédit Kenney dans « À propos ». Compilé et essayé sur le Mac seulement ; **rien regardé à
  l'écran, pas de commit** (liste 181.5).

- **Le décor 3D d'une séquence** (partie 180, branche `decor-3d` partie de `main` c9c24f6, format 60 inchangé, aucune dépendance
  neuve), première étape du moteur de prévisualisation qu'Anthony a choisi. Un glTF (`.glb`, `.gltf` empaqueté) importé pour une
  séquence : tous ses plans le montrent, derrière les calques, filmé par leur caméra (vue Espace 3D, image du plan, exports) ;
  aspect low poly doux (facettes, lumière hémisphère, ciel, brume, ombres douces) ; `decors3d.json` et `decors/<id>.glb` à côté du
  manifeste ; une étape d'annulation. Sur ses demandes en cours de route : barrière à l'entrée (« Ce décor est lourd » : alléger
  sans toucher la forme, simplifier en le disant), budget **mesuré sur la machine** (étalonnage gardé dans les réglages avec la
  clé de la carte et du processeur, « Remesurer » dans Préférences > Performance), règle « aucun ralentissement » (à l'écran, la
  carte graphique dessine le décor ; jamais d'attente sur le fil de l'interface), aucun plantage sur un fichier abîmé.
  Compilé et essayé sur le Mac seulement ; **rien regardé à l'écran, pas de commit** (liste 180.6). Budgets à valider sur son PC
  (`decor3d_perf`, ignoré).
- **`main` du 19 novembre (9c422b9 : parties 168 à 177, dont la colonne des calques de la partie 174) fusionné dans
  `timeline-claire`.** Première tentative annulée sans rien écrire : la colonne des calques de la 174 (en-tête, sept colonnes sur tous
  les calques, calque courant sur trois rangées) et celle de la timeline plus claire (lignes égales, barre du calque, exception
  seule) ne tenaient pas ensemble. Trois maquettes cliquables publiées (A : la timeline plus claire ; B : la 174 ; C : la 174 plus ce
  qui ne la contredit pas) ; Anthony : « C, le mélange ». Gardé de la timeline plus claire : la couleur du rangement sur toute la
  ligne (colonne et grille), les dossiers en titres, les clés des calques sur un dossier fermé. Retirés : la barre du calque, les
  lignes « exception seule », les lignes à 28 points ; la colonne est celle de la 174. La partie devient la **179** (168 : les versions
  d'un plan ; 178 : la plage de lecture du film, branche `plage-lecture`). Essais : ceux de la colonne sont ceux du `main` ;
  `timeline_claire_tests` garde la teinte et vérifie la couleur d'un calque rangé. Compilé et essayé sur le Mac seulement.
- **`timeline-storyboard` fusionnée dans main (ecaeeb5), poussée, et son FEANOR habituel reconstruit** (exe de 3 h 51), sur
  son « fusionne la timeline ». Partie 178 : la timeline du Storyboard dans l'organisation qu'il a dessinée (Séquence, Plans
  avec transitions sur les coupes et icônes, Caméra repliable avec la rangée des préréglages, Calques repliables, coupes qui
  traversent). Main n'avait pas bougé (9a748bb, ni ici ni sur GitHub) : l'arbre fusionné est celui vérifié (sans
  avertissement, 4 923 réussis, 0 échec, deux passages). Son FEANOR n'était pas ouvert. Contrôle de lancement sur une copie de
  « retakes brawlydays » (Tests\lancement main ecaeeb5, faux réglages) : vivant après 20 s, aucun rapport de plantage. Format
  inchangé (60). **Pas regardé sur son écran** : liste 178.4. Un fichier vide `storyboard.rs` créé par erreur par l'agent dans
  son dossier BRAWLSTAR\BRAWLIDAYS\3D lui a été signalé (suppression refusée à l'agent, laissée à lui). Gardé, retrait proposé :
  `Chantiers\timeline-storyboard`.
- **La timeline du Storyboard dans son organisation** (partie 178, branche `timeline-storyboard` partie de main 9a748bb, format
  60 inchangé), d'après son dessin et la maquette qu'il a validée : de haut en bas Séquence (16 points), Plans (26, le ⋈ des
  transitions sur les coupes, un clic ouvre leur menu), ⊕/⊖ Caméra (22 ; dépliée, la rangée Préréglages de 24 où l'on pose,
  glisse, étire les préréglages de caméra comme dans l'Animation), ⊕/⊖ Calques (repliée : une ligne de résumé). Chaque rangée a
  ses icônes d'en-tête ; les coupes des plans traversent tout. L'espace Montage garde sa disposition. **Pas fusionné, pas
  regardé sur son écran** : liste 178.4 d'INTERFACE.md. Captures dans le dossier `vues178` de la session.
- **`piste-mode-liste` fusionnée dans main (9c422b9), poussée, et son FEANOR habituel reconstruit** (exe de 2 h 22), sur
  son « fusionne et pousse sur github ». Parties 173 à 174 : la piste du calque actif du Storyboard (liste « Normal ▾ »,
  colonne réglable 250–520 points, toutes les options du calque) et la colonne des calques de la timeline dans la
  disposition qu'il a validée (en-tête des réglages en colonnes pour tous les calques, calque actif sur trois rangées, tout
  replier Maj+Alt+C, comportement avant et après les images). Main avait reçu 175 à 177 entre-temps : fusionnées d'abord
  dans la branche (12e5e45, seul conflit INTERFACE.md, les deux sections gardées), puis vérificateur complet sur l'arbre
  fusionné : sans avertissement, 4 914 réussis, 0 échec, deux passages, aucun intermittent. Son FEANOR n'était pas ouvert.
  Contrôle de lancement sur une copie de « retakes brawlydays » (Tests\lancement main 9c422b9, faux réglages) : vivant après
  15 s, aucun rapport de plantage. Format inchangé (60). **Pas regardé sur son écran** : liste 174 d'INTERFACE.md. Gardé, retrait
  proposé (sa décision) : le dossier `Chantiers\piste-mode`. Il enverra ce commit à la session du Mac.
- Sur son « Oui, retire-les » : le dossier `Chantiers\prereglages-reponses` et sa branche locale retirés (elle n'avait jamais
  été poussée ; tout est dans main, sur GitHub), `Tests\essai 177` mis à la corbeille (il n'avait pas servi). `Chantiers\piste-mode`
  et `Tests\essai 173`, à l'autre session, pas touchés.
- **La plage de lecture du film** (partie 178, branche `plage-lecture` partie de main 6b1b99c, format 60 inchangé), sur son
  « Plage de lecture » parmi trois écarts trouvés en comparant la timeline à celle d'un autre logiciel. La lecture du montage reste
  entre deux images de plans différents, en boucle ou non ; les bouts sont accrochés à leurs plans ; clic droit sur la règle du
  montage ; réglage de la séance, rangé avec l'onglet. 9 essais (`plagefilm_tests.rs`). Compilé et essayé sur le Mac seulement
  (les mêmes 90 échecs que main sur ce Mac, aucun de plus). Commit 04599ac, poussé sur GitHub sur son « Oui, pousse ». **Pas
  fusionné dans main, pas essayé sur son PC** (liste 178.4).
- **`prereglages-reponses` fusionnée dans main, poussée, et son FEANOR habituel reconstruit** (e859d56, exe de 1 h 10), sur
  son « Fusionne maintenant » (sans essai sur son écran avant). Main n'avait pas bougé depuis la branche (e561eff), ni ici ni
  sur GitHub : l'arbre fusionné est exactement celui vérifié (sans avertissement, 4 897 réussis, 0 échec, les dix groupes OK) ;
  le vérificateur n'a pas été relancé. Son FEANOR était ouvert (depuis 0 h 09) : arrêté sans enregistrer, l'exe recompilé
  dans Source. Contrôle de lancement avec ce nouvel exe sur une copie de « dd » (format 60) et de faux réglages : vivant après
  30 s, aucun rapport de plantage, son vrai « dd » intact. Il n'a pas été rouvert à sa place. Le format des projets ne change
  pas (60). **Pas regardé sur son écran** : liste 177.4. Gardés, retrait proposé (sa décision) : le dossier
  `Chantiers\prereglages-reponses` et `Tests\essai 177`.
- **Préréglages : ses neuf réponses** (partie 177, branche `prereglages-reponses` partie de main e561eff, format 60
  inchangé), sur son « Oui ok pour tout » aux neuf numéros restés sans réponse. Trois transitions de plus : l'iris (T1, sur un
  centre choisi au clic droit ; il s'ouvre au début du film, se ferme à la fin), le volet en horloge (T2) et le zoom traversant
  (T5) ; le volet, la poussée et le raccord filé viennent de chacun des quatre côtés (« Sens » au clic droit). Six plans types
  de plus : titre de séquence (P4), plan manquant (P9), ardoise d'en-tête (P10), compte à rebours avec son bip (P11), mire de
  couleur et son 1 kHz (P12), fin d'acte (P15) ; les incrustations libres connaissent `{duree}`, `{duree_film}`, `{serie}`,
  `{episode}`. `transitions.json` passe à la version 3 seulement quand il le faut. Vérificateur : sans avertissement, 4 897
  réussis, 0 échec, les dix groupes OK (l'essai sur la carte graphique avait trouvé, avant, le volet en horloge pris par le
  flou du raccord filé : corrigé). Contrôle de lancement de l'exe du chantier sur une copie de « dd » (format 60) avec de faux
  réglages : vivant 30 s, aucun plantage, son vrai « dd » intact (son FEANOR habituel, ouvert depuis 0 h 09, pas touché).
  Dossier d'essai pour lui : `Tests\essai 177` (copie de « dd », copie de ses réglages, `Lancer essai 177.cmd`). **Pas
  fusionné, pas essayé sur son écran** (liste 177.4).
- Sur son « Oui, retire-les » : le dossier `Chantiers\prereglages-montage` et sa branche locale retirés (elle n'avait jamais
  été poussée ; tout est dans main, sur GitHub), `Tests\essai 175` mis à la corbeille (il n'avait pas servi : la copie de
  « dd » était encore au format 59). `Chantiers\piste-mode` et `Tests\essai 173`, à l'autre session, pas touchés.
- **`prereglages-montage` fusionnée dans main, poussée, et son FEANOR habituel reconstruit** (23efd4c, exe du 19 novembre à
  0 h 01), sur son « Fusionne maintenant » (sans essai sur son écran avant). Main n'avait pas bougé depuis la branche (93ad4fd),
  ni ici ni sur GitHub : l'arbre fusionné est exactement celui vérifié (sans avertissement, 4 880 réussis, 0 échec, les dix
  groupes OK) ; le vérificateur n'a pas été relancé. Son FEANOR était ouvert (depuis 20 h 39) : arrêté sans enregistrer, l'exe
  recompilé dans Source. Contrôle de lancement avec ce nouvel exe sur une copie de « dd » (format 59) et de faux réglages :
  vivant après 30 s, aucun rapport de plantage, son vrai « dd » intact. Il n'a pas été rouvert à sa place. **Ses projets
  passent au format 60 quand il les ouvre** (une version précédente les refuserait). **Pas regardé sur son écran** : liste
  176.4. Gardés, retrait proposé (sa décision) : le dossier `Chantiers\prereglages-montage` et `Tests\essai 175`.

## 2026-11-18

- **`main` (95dc93b : vue nette lissée 166.8, `avant-apres-reel` 165.3 à 165.8, nouveau logo 167) fusionné dans
  `timeline-claire`.** Première tentative annulée sans rien écrire : le `main` avait pris l'autre chemin pour le calque courant (sa
  ligne complète toujours visible, 165.4). Deux maquettes côte à côte (A : la ligne complète ; B : la barre du calque, lignes toutes
  fines) ; Anthony : « B mais la taille d'une ligne de calque est plus épaisse ». Les lignes passent de 22 à 28 points ; les boutons
  de la timeline restent en haut de la colonne (165.6), la barre du calque au-dessus de la timeline, avec « 100 % » et « Normal ▾ »
  en texte (165.3) ; le calque courant garde sa ligne même dans un groupe caché (165.4) mais elle ne flotte plus. La timeline plus
  claire devient la partie **168** (le logo a pris la 167). Deux essais du `main` adaptés, un retiré (la ligne flottante). Suite sur
  le Mac : les mêmes 89 échecs propres au Mac qu'avant. Pas regardé à l'écran.
- **Préréglages dans la timeline, lot 4 : les transitions et les plans types** (parties 175 et 176, branche
  `prereglages-montage` partie de main 93ad4fd, **format 60**), sur son « Ok continue ». Cinq transitions de plus (poussée T3,
  raccord filé T4, flash de coupe T6, fondu par une couleur T7, volet par un objet T8), à l'écran comme à l'export ; le panneau
  Préréglages a deux onglets, Caméra et Montage ; dans Montage, les dix transitions (glissées sur une coupe) et dix-huit plans
  types (P1, P2, P3, P5, P6, P7, P8, P13, P14, P16, P17, P18) qui posent un plan neuf tout prêt entre deux plans, fait de calques
  ordinaires ; le lieu et la date (une incrustation qui s'écrit lettre à lettre, réglage neuf « Lettre à lettre ») et le
  générique de début se posent sur un plan. Numérotées 175 et 176 : la session de la branche `piste-mode-liste` a pris la partie
  173 et `Tests\essai 173` (rien touché de son côté). Vérificateur : sans avertissement, 4 880 réussis, 0 échec, les dix groupes OK (un premier passage avait 8 échecs : le format 60 dans d'anciens essais, les commandes neuves absentes de keymap.toml, le menu d'une transition trop long, ses sortes passées dans un sous-menu). Contrôle de lancement de l'exe du chantier sur une copie de « dd » avec de faux réglages : vivant 30 s, aucun plantage, son vrai « dd » intact (son FEANOR habituel, ouvert depuis 20 h 39, pas touché). Dossier
  d'essai pour lui : `Tests\essai 175` (copie de « dd » au format 59, copie de ses réglages, `Lancer essai 175.cmd`). **Pas
  fusionné, pas essayé sur son écran** (liste 176.4). Restent sans réponse : T1, T2, T5, P4, P9, P10, P11, P12, P15.
- Sur son « Oui, retire-les » : le dossier `Chantiers\prereglages-travellings` (environ 2,5 Go) et sa branche locale retirés
  (elle n'avait jamais été poussée ; tout est dans main, sur GitHub), `Tests\essai 172` mis à la corbeille (il n'avait pas
  servi : la copie de « dd » était encore au format 58).
- **`prereglages-travellings` fusionnée dans main, poussée, et son FEANOR habituel reconstruit** (cd23311, exe de 19 h 21),
  sur son « Fusionne maintenant » (sans essai sur son écran avant). Main n'avait pas bougé depuis la branche (603851a), ni ici
  ni sur GitHub : l'arbre fusionné est exactement celui vérifié (sans avertissement, 4 855 réussis, 0 échec, les dix groupes
  OK) ; le vérificateur n'a pas été relancé. Son FEANOR était ouvert (depuis 15 h 17) : arrêté sans enregistrer, l'exe
  recompilé dans Source. Contrôle de lancement avec ce nouvel exe sur une copie de « dd » (format 58) et de faux réglages :
  vivant après 30 s, aucun rapport de plantage, son vrai « dd » intact. Il n'a pas été rouvert à sa place. **Ses projets
  passent au format 59 quand il les ouvre** (une version précédente les refuserait). **Pas regardé sur son écran** : liste
  172.4. Gardés, retrait proposé (sa décision) : le dossier `Chantiers\prereglages-travellings` et `Tests\essai 172`.
- **Préréglages dans la timeline, lot 3 : la caméra dans l'espace** (partie 172, branche `prereglages-travellings` partie de
  main 603851a, **format 59**), sur son « Oui, le lot 3 ». Quatre préréglages de plus en blocs vivants, qui déplacent vraiment
  la caméra (travelling latéral C13, orbite C14, grue C15, survol C16, famille « Travellings ») : l'œil bouge, chaque calque
  glisse à sa vitesse, la caméra peut passer à côté, au-dessus, derrière ; le plan retient que sa caméra est libre. L'orbite et
  la grue gardent leur sujet à sa place dans l'image (viseur « Sujet » avec l'outil Caméra). Les trajectoires toutes faites
  (C17 : ligne droite, arc, spirale, en S) deviennent le chemin de la caméra du plan, qui garde son sujet dans le cadre et se
  retouche avec l'outil Trajectoire dans la vue Espace 3D ; le chemin a sa barre dans la timeline. Les vignettes du panneau
  Préréglages sont filmées par la vraie caméra. « Convertir en clés » donne la même image pour les trente-quatre (essai).
  **À juger** : le sujet est pris au plan de référence, pas à la profondeur d'un calque (172.3). Vérificateur : sans avertissement, 4 855 réussis, 0 échec, les dix groupes OK.
  Contrôle de lancement sur une copie de « dd » (format 58) avec de faux réglages : vivant 30 s, aucun plantage, son vrai « dd » intact (son FEANOR habituel, ouvert sur « dd », n'a pas été touché). Dossier d'essai pour lui : `Tests\essai 172` (copie de « dd » au format 58, copie de ses
  réglages, `Lancer essai 172.cmd`). **Pas fusionné, pas essayé sur son écran** (liste 172.4).
- Sur son « Oui, retire-les » : le dossier `Chantiers\prereglages-camera` (environ 2,5 Go) et sa branche locale retirés (elle
  n'avait jamais été poussée ; tout est dans main, sur GitHub), `Tests\essai 171` mis à la corbeille (il n'avait pas servi).
- **`prereglages-camera` fusionnée dans main, poussée, et son FEANOR habituel reconstruit** (ac390e6, exe de 13 h 56), sur son
  « Fusionne maintenant » (sans essai sur son écran avant). Main n'avait pas bougé depuis la branche (1070912), ni ici ni sur
  GitHub : l'arbre fusionné est exactement celui vérifié (sans avertissement, 4 846 réussis, 0 échec, les dix groupes OK) ; le
  vérificateur n'a pas été relancé. Son FEANOR était ouvert sur « dd » (depuis 11 h 54) : l'exe, déjà à jour dans le dossier du
  chantier, a été copié après l'arrêt de son FEANOR sans enregistrer (même empreinte aux deux chemins). Contrôle de lancement
  avec ce nouvel exe sur une copie de « dd » (format 57) et de faux réglages : vivant après 30 s, aucun rapport de plantage, son
  vrai « dd » intact. Il n'a pas été rouvert à sa place. **Ses projets passent au format 58 quand il les ouvre** (une version
  précédente les refuserait). **Pas regardé sur son écran** : liste 171.4. Gardés, retrait proposé (sa décision) : le dossier
  `Chantiers\prereglages-camera` et `Tests\essai 171`.
- **Préréglages dans la timeline, lot 2 : la caméra 2D** (partie 171, branche `prereglages-camera` partie de main 1070912,
  **format 58**), sur son « Oui, le lot 2 ». Vingt-deux préréglages de plus en blocs vivants (C1 à C10, C12, C18 à C27 ; C5 en
  avant et arrière), rangés en cinq familles (panneau Préréglages, liste du panneau Caméra, menu « Ajouter un effet »). La flèche
  ou la cible se tire dans le cadre avec l'outil Caméra, et dans un petit cadre du panneau Caméra ; champs Angle, Paliers,
  Avancée, Pas, Rythme. « Convertir en clés » donne la même image pour les trente (essai). **Pas faits** : C11 (suivi d'un
  calque) et le flou de vitesse du zip pan (171.3). Vérificateur : sans avertissement, 4 846 réussis, 0 échec, les dix groupes
  OK. Contrôle de lancement sur une copie de « dd » avec de faux réglages : vivant 30 s, aucun plantage, son vrai « dd » (57)
  intact. Dossier d'essai pour lui : `Tests\essai 171` (copie de « dd », lanceur aux réglages à part). **Pas fusionné, pas essayé
  sur son écran** (liste 171.4).
- Sur son « Oui, retire-les » : le dossier `Chantiers\prereglages-socle` (environ 2 Go) et sa branche locale retirés (elle n'avait
  jamais été poussée ; tout est dans main, sur GitHub), `Tests\essai 170` mis à la corbeille.
- **`vue-reduite` fusionnée dans main, poussée, et son FEANOR habituel reconstruit** (baf6476, exe de 11 h 39), sur son
  « Oui, fusionne » (écrit dans la session principale, qui l'a transmis mot pour mot ; relu dans son fil avant d'agir ; sans essai
  sur son écran avant). Main n'avait pas bougé depuis la vérification de 10 h 14 (fc908db) : l'arbre fusionné est exactement
  celui vérifié (sans avertissement, 4 840 réussis, 0 échec, les dix groupes OK) ; le vérificateur n'a pas été relancé. Son
  FEANOR était ouvert sur « dd » (dernière écriture du projet à 11 h 30) : la compilation s'est faite pendant qu'il restait
  ouvert (4 min 40, sans avertissement), puis il a été arrêté sans enregistrer à 11 h 39, pour les quelques secondes du
  remplacement du fichier. Contrôle de lancement sur des copies de « dd » (format 56) et de « afa » (format 52), avec de faux
  réglages, fenêtre réduite : vivant et répondant après 30 s, titres « dd.shot — FEANOR » et « afa.shot — FEANOR », aucun rapport
  de plantage ; ses vrais réglages et ses vrais projets n'ont pas été touchés. Il n'a pas été rouvert à sa place. **Pas regardé
  sur son écran** : à lui de juger (listes « À vérifier à l'écran » des sections 166.9 et 166.10). Le dossier
  `Chantiers\vue-reduite` et les dossiers `Tests\essai vue-reduite` et `Tests\vue-reduite` sont gardés (retrait proposé, sa
  décision) ; la branche d'essai `vue-reduite-essai` ne se fusionne pas.
- **La vue réduite : son verdict, son choix, et les vignettes** (branche `vue-reduite`, sections 166.9 et 166.10). L'image de
  démonstration de la veille ne l'a pas convaincu : « sur l'image de démonstration la pixélisation est la même. Pire, l'image
  d'après semble encore plus détériorée. » Il avait raison sur l'image (agrandie neuf fois, et pas la même image du plan des deux
  côtés) ; refaite à l'identique, la comparaison montrait des traits continus mais plus pâles. Rien n'a été fusionné. **L'essai à
  trois façons** (branche `vue-reduite-essai`, pas pour main ; F8 dans la vue du dessin ; lanceur dans `Tests\essai vue-reduite`) :
  comme avant, lissée trait pâle, lissée trait plein. **Son choix, l'essai ouvert : « La 3, trait plein », puis « Pardon, la 2 »** :
  la 166.9 telle qu'elle était. (Claude recommandait la 3 : c'est son œil, sur son écran, qui a tranché.) **Les vignettes ensuite**
  (166.10) : les cases des Vignettes qu'on ne dessine pas et les vignettes de plans, calculées par le processeur, lisaient 4 à 16
  points par pixel pour 25 à 100 pixels couverts ; l'encre d'un trait fin y allait de 0 à 1,8 fois la normale d'une ligne à
  l'autre. Maintenant : la moyenne exacte de tous les pixels couverts (caméra droite, jusqu'à 12 pixels par axe : les cases à six
  par ligne), des lectures en grille sinon ; un raccourci saute les pixels sans encre. Même espace de moyenne qu'avant (lumière
  linéaire pour les cases, comme la case qu'on dessine). **Coût mesuré** : cases à six par ligne, de 0,6 à 1,6 fois le temps
  d'avant selon le projet ; le plan le plus lourd, 0,3 s au lieu de 0,1 ; sur les fils de fond. Un essai existant vérifiait
  « quatre points par pixel » (l'ancien défaut) : son attendu est maintenant la moyenne de tous les pixels couverts. Capture de
  contrôle de l'onglet Vignettes sur une copie de « dd », regardée : dans une case voisine les traits fins sont continus, du même
  poids que dans la case qu'on dessine. Pas regardé sur son écran. **Vérificateur : aucun passage entièrement vert**, trois autres
  sessions compilant sans arrêt (processeur à 100 %, moins de 1,2 Go de mémoire libre). Premier passage : sans avertissement,
  suite complète 4 801 réussis et 0 échec, neuf groupes OK, deux essais de temps d'image du groupe Espace 3D en échec. Second : sans
  avertissement, les dix groupes OK, 4 800 réussis et 1 échec dans la suite (un essai d'aplat calculé en fond). Relancés seuls, ces
  essais passent quand le processeur n'est pas saturé ; aucun n'appelle le code changé. **Refait après avoir repris main dans la
  branche** (87db1b0, le panneau Pinceau ; f603c82) : à 3 h 44, la machine plus calme, compilation sans avertissement, **4 802
  réussis, 0 échec, les dix groupes OK**. Et encore après la trajectoire (60f002e ; d55ffd5), six essais à la fois : à 4 h 17,
  sans avertissement, **4 806 réussis, 0 échec, les dix groupes OK**. Main a encore avancé dans la nuit (versions de plan, puis
  Cutter) : repris à chaque fois. Avec les versions de plan, à 4 h 44, sous la charge, un échec dans la suite : l'essai de la
  planche des Vignettes à trente plans, qui touche à ce chantier ; relancé seul et au milieu de ses voisins, il passe à chaque
  fois (huit passages), sans que le message de l'échec ait pu être retrouvé. Avec le Cutter (bed74af ; a57da20), à 9 h 55, la
  machine calme : sans avertissement, **4 830 réussis, 0 échec, les dix groupes OK**. Puis avec les préréglages (fc908db,
  format 57 ; a4c6368), à 10 h 14 : sans avertissement, **4 840 réussis, 0 échec, les dix groupes OK**. Pas fusionnée : sa
  décision.
- **`prereglages-socle` fusionnée dans main, poussée, et son FEANOR habituel reconstruit** (d43ea27, exe de 09 h 50), sur
  son « Fusionne maintenant » (sans essai sur son écran avant). Main avait avancé deux fois pendant le chantier
  (trajectoire-exposition et versions-plan, puis cutter-reglages) : main ramené dans la branche à chaque fois, seuls
  INTERFACE.md et le journal en conflit (les deux parties gardées), puis vérificateur complet sur l'ensemble : compilation sans
  avertissement, **4 833 réussis, 0 échec**, les groupes de fiabilité OK. Son FEANOR était ouvert (depuis 09 h 48) : arrêté
  sans enregistrer juste avant de remplacer le fichier (la nouvelle version avait été construite et contrôlée à côté). Contrôle de
  lancement sur une copie de « dd » avec de faux réglages : vivant, aucun plantage ; son vrai « dd » est resté au format 56. Il
  n'a pas été rouvert à sa place. **Ses projets passent au format 57 dès qu'il les ouvre** avec ce FEANOR. Worktree et branche
  gardés (retrait proposé, sa décision).
- **`cutter-reglages` fusionnée dans main, poussée, et son FEANOR habituel reconstruit** (bed74af, exe de 04 h 53), sur son
  « fusionne le cutter ». Partie 169 de docs/INTERFACE.md (numérotée 168 sur la branche ; les versions de plan avaient pris le
  168) : le Cutter a cinq réglages, réglages du poste (clé `[cutter]` de reglages.toml), format inchangé (56) : écart toléré (0
  d'origine, comme avant), bouts effilés (0 = net), mode « Dépassements » (bouts libres plus courts que 12 px d'origine ; jamais un
  morceau entre deux croisements), portée « Calques visibles » (les autres calques coupent sans être modifiés), geste « Lasso » ;
  plusieurs calques ou clés choisis : tous coupés, une seule étape d'annulation. Panneau sobre validé par lui (segments texte,
  dessins-curseurs pour l'écart et l'effilé, curseur des dépassements, Précis), après deux essais refusés (tuiles « trop grosses
  et peu professionnelles »). main fusionnée dans la branche avec --no-ff, le conflit de docs/INTERFACE.md résolu en gardant les
  deux parties ; les accents de `cutter_tests.rs`, abîmés par une réécriture sous PowerShell, réparés. Vérificateur sur l'arbre
  fusionné : compilation sans avertissement, passage 2 : 4 823 réussis, 0 échec ; passage 1 : un échec intermittent connu
  (`camera_traits_tests::a_stroke_stays_whole_at_rest_while_another_is_drawn_and_after_it_is_released`, relancé seul trois fois,
  trois réussites) ; dix groupes de fiabilité OK. Son FEANOR (lancé depuis Source) n'était pas ouvert : rien à arrêter. Contrôle de
  lancement : l'exe sur une copie de « retakes brawlydays » (`Tests\lancement main bed74af`) avec de faux réglages, vivant après
  22 s, aucun plantage nouveau, puis arrêté. À juger par lui : l'écart d'origine (0 ou 2 à 3 px), les 12 px des dépassements, la
  pointe effilée (un trait effilé perd ses appuis enregistrés). Worktree `trajectoire-3d` et branche gardés.
- **`versions-plan` fusionnée dans main, poussée, et son FEANOR habituel reconstruit** (b3059ec, exe de 04 h 17), sur son
  « fusionne les versions de plan ». Partie 168 de docs/INTERFACE.md (numérotée 167 sur la branche ; le nouveau logo avait pris le
  167) : sur le bloc d'un plan, les autres versions en étiquettes « V2 ▸ » ; clic sur « V2 » = passer à la version ; ▸ déplie la
  version à côté, en lecture seule, à l'écran seulement ; le son gardé de chaque version est rejoué sous elle (le son du film ne
  bouge pas quand on change de version ; fondus et volume ne sont pas gardés) ; clic droit = dix teintes douces. Format inchangé (56).
  main avait avancé deux fois (panneau Pinceau et logo, puis trajectoire-exposition) : fusionnée deux fois dans la branche avec
  --no-ff, un conflit de docs/INTERFACE.md résolu en gardant les deux parties. Vérificateur sur l'arbre fusionné : compilation sans
  avertissement, 4 812 réussis au passage 1 ; un échec intermittent au passage 2 (`espace3d_tests::the_view_with_ten_cards_stays_within_the_frame_budget`,
  relancé seul trois fois, trois réussites). Au passage d'avant, deux essais de temps de l'Espace 3D avaient échoué pendant que le
  poste était chargé : mesurés tour à tour sur main et sur la branche, mêmes temps. Son FEANOR (lancé depuis Source) n'était pas
  ouvert : rien à arrêter. Contrôle de lancement : l'exe sur une copie de « retakes brawlydays » (`Tests\lancement main b3059ec`)
  avec de faux réglages, vivant après 22 s, aucun plantage nouveau, puis arrêté. Le message du commit `ab1c03f` est inexact (les
  teintes étaient déjà dans `b0f7b4f`). Worktree `aide-animee` et branche gardés.
- **`trajectoire-exposition` fusionnée dans main, poussée, et son FEANOR habituel reconstruit** (223054b, exe de 03 h 29), sur son
  « Oui, fusionne » à la question « Je fusionne ce changement de la trajectoire et je reconstruis ton FEANOR habituel ? ». main
  n'avait pas bougé : la fusion a l'arbre vérifié (4 799 réussis, 0 échec, compilation sans avertissement). Son FEANOR n'était pas
  ouvert : rien n'a été arrêté. Contrôle de lancement : l'exe ouvert sur une copie de « dd » avec de faux réglages, vivant après
  22 s, aucun plantage, puis arrêté ; ses vrais réglages et son vrai projet n'ont pas changé. Son FEANOR n'a pas été rouvert à sa
  place. Worktree et branche gardés (retrait proposé, sa décision).
- **Trajectoire : une marque qu'on glisse emporte son temps** (branche `trajectoire-exposition`, la section du 2026-11-18 de la
  partie 119). Sa demande : « L'outil trajectoire doit ajuster l'exposition des images dans la timeline quand on décale les clés
  sur la trajectoire. » Son plan 10 de « dd » (lu sans y toucher) : sept marques serrées au départ, chaque dessin exposé 8 ou 9
  images. **Ce qui est fait**, en clés d'images : glisser une marque le long du chemin lui fait prendre l'image de sa place entre
  ses deux voisines (le dessin d'avant tient plus longtemps quand elle avance, moins quand elle recule ; les autres ne bougent pas ;
  la barre d'état dit l'image) ; une marque posée quand des temps sont déjà posés prend le temps de sa place ; un bouton « Temps
  selon le chemin » met toutes les marques d'un coup au temps de leur place (son plan 10 : 3, 2, 2, 8, 11, 19, 5 et 20 images).
  Sur la feuille et pour les cartes de l'Espace 3D. **Ce qui ne change pas** : les clés d'interpolation (les marques y sont
  l'espacement), la caméra, les projets à l'ouverture, le format. **Ma lecture, à confirmer par lui** : une marque glissée ne
  recale qu'elle (le bouton recale tout) ; une marque posée sur un chemin en parts égales repartage le temps comme avant. Quatre
  essais d'avant, qui décrivaient l'ancien comportement, mis à jour. Vérificateur complet (03 h 17) : compilation sans
  avertissement, **4 799 réussis, 0 échec**, dix groupes OK, au troisième passage : les deux premiers avaient échoué sur des essais
  chronométrés sans rapport, la machine étant saturée par les essais d'autres sessions (ils passent seuls). Image de contrôle de la
  timeline faite sur une copie. Pas regardé sur son écran. Pas fusionnée : sa décision.
- **Préréglages dans la timeline, lot 1 : le socle** (branche `prereglages-socle`, partie 170, format 57). Sa demande : « des
  présets d'animation de caméra à intégrer directement dans la timeline » (crash zoom, zip pan, panoramique, travelling) et des
  présets de plan (color card, titre, crédits). Une liste numérotée lui a été faite, puis une page où chaque préréglage joue en
  boucle (https://claude.ai/artifact/RaFTcminkjKeDWxMUFVi6J, ses choix gardés dans la page) : 64 Oui, 9 sans réponse (P4, P9,
  P10, P11, P12, P15, T1, T2, T5). À l'ordre en lots, il a répondu « Oui, le lot 1 ». **Ce qui est fait** : un plan porte
  plusieurs effets de caméra vivants, des blocs jaunes sous les clés de caméra de la timeline, qui s'ajoutent l'un à l'autre ; on
  les glisse, on tire leurs bords pour leur durée, Suppr enlève le bloc choisi ; le panneau Préréglages (vignettes qui jouent le
  vrai mouvement au survol) se glisse sur la timeline ou se clique pour poser à la tête de lecture ; « Convertir en clés » cuit
  tous les blocs d'un coup, avec la même image. Les huit préréglages de caméra existants seulement : les autres sont les lots
  suivants.

- **`panneau-pinceau` fusionnée dans main, poussée, et son FEANOR habituel reconstruit** (907c92a, exe de 01 h 27), sur son
  « Oui, fusionne » à la question « Je fusionne ce nouvel ordre du panneau et je reconstruis ton FEANOR habituel ? ». Le main de
  `Source` était en retard sur GitHub (fd35d4f) : avancé d'abord jusqu'à origin/main (95dc93b, le nouveau logo), puis la fusion,
  qui a l'arbre vérifié sur la branche (4 795 réussis, 0 échec, compilation sans avertissement). Son FEANOR n'était pas ouvert :
  rien n'a été arrêté. Contrôle de lancement : l'exe ouvert sur une copie de « dd » avec de faux réglages, vivant après 22 s,
  aucun plantage, puis arrêté ; ses vrais réglages et son vrai projet n'ont pas changé. Son FEANOR n'a pas été rouvert à sa place.
  Worktree et branche gardés (retrait proposé, sa décision).
- **Le panneau Pinceau : les réglages d'abord, la liste des brosses tout en bas** (branche `panneau-pinceau`, la section du
  2026-11-18 de la partie 45). Sa demande : « revoir la disposition du panneau », sur une capture annotée de sa main : 1 les
  réglages, 2 les trois boutons, 3 la liste, les réglages remontés sous l'aperçu, la liste descendue en bas. **Ce qui est fait** :
  sous l'aperçu du trait, « Peindre derrière », « Peint sur l'aplat », l'aplat du calque et « Plus de réglages » ; puis la rangée
  importer / nouvelle brosse / supprimer ; puis la liste dans son cadre, dernière chose du panneau (pinceau, crayon, gomme). Dépliée,
  la liste ne repousse plus les réglages loin de l'aperçu. Rien d'autre ne change (le bouton « livres », la hauteur de la liste, ce
  qu'elle offre). Un seul texte visible changé : la fin de la bulle du bouton (« en bas du panneau »). Essais du panneau : 8 réussis,
  dont un nouveau sur l'ordre ; vérificateur complet (01 h 17) : compilation sans avertissement, **4 795 réussis, 0 échec**, les dix
  groupes OK. Image de contrôle faite sur le banc (le panneau, replié et déplié), regardée : l'ordre du schéma. Pas regardé sur son
  écran. Pas fusionnée : sa décision. (Le chantier de la vue réduite est passé le même soir à une session à part.)

## 2026-11-17

- **La vue réduite : tous les pixels du dessin comptent** (branche `vue-reduite`, section 166.9). Son retour : « Est-ce qu'on peut
  rajouter une passe d'antialiasing dans le logiciel ? Je vois toujours de la pixelisation de l'image, même avec le vecto propre. »
  Sa fenêtre lue telle quelle : la vue à 37 %, sa vue ordinaire (le cadre ne tient pas dans la zone du dessin de son écran), où la
  vue nette ne fait rien. **La cause** : réduite, l'image était lue par points (le filtre de grossissement de l'affichage, qui ne
  lisse pas) : 4 points pour plus de 7 pixels couverts à 37 %, un seul de 67 à 100 %. Les traits fins se cassaient en pointillés.
  **Ce qui est fait** : réduite, chaque pixel d'écran est la moyenne exacte de tout ce qu'il couvre du dessin (vue droite), ou de
  n × n lectures lissées (vue tournée, perspective) ; tournée à 100 %, l'image est lissée ; à 100 % vue droite et au-delà, rien ne
  change. Tout sur la carte graphique, rien à attendre. **Pas fait** : les cases des Vignettes qu'on ne dessine pas et les
  vignettes de plans (calcul du processeur, même faiblesse, chantier à part) ; le hors-feuille ; l'affichage sans carte graphique.
  Mesuré par un essai écrit avant la correction et vu échouer (à 80 %, l'encre d'un trait fin de 0 à 1,25 fois la normale ; après,
  de 0,94 à 1,04 à tous les zooms). Image de contrôle faite sur une copie de « dd » (plan 10 à 37 %), regardée agrandie : traits
  continus. Pas regardé sur son écran. Vérificateur complet (20 h 49) : compilation sans avertissement, **4 800 réussis, 0 échec**,
  les dix groupes de fiabilité OK. Pas fusionnée : sa décision.
- **`avant-apres-reel` fusionnée dans main, poussée, et son FEANOR habituel reconstruit** (0073f65, exe de 20 h 15), sur son
  « vas y ». Contenu : partie 165.6 (deux lignes de boutons en tête de la colonne des calques de l'Animation, colonne à 410 points
  au lieu de 757, règle et en-tête à 58 points), 165.7 (le mode « Trois calques » retiré : toute la pile, toujours ; un réglage
  « trois » enregistré s'ouvre en toute la pile, format 56 inchangé), 165.8 (Alt + clic dans la vue de dessin choisit le calque du
  dessin cliqué ; ce n'est plus la pipette). Fusion de main dans la branche (conflits INTERFACE.md et calques_timeline_tests.rs
  résolus), puis avance rapide de main. L'avertissement des méthodes inutilisées d'`espace3d.rs` était déjà corrigé par main.
  Vérificateur complet : compilation sans avertissement, 4 796 réussis aux deux passages, 0 échec, aucun intermittent, dix groupes
  OK. Son FEANOR (lancé par la jonction Work\tvpaint-rs) a été arrêté sans enregistrer avant la reconstruction. Contrôle de
  lancement : l'exe ouvert sur une copie de « retakes brawlydays » (Tests\lancement main 0073f65) avec de faux réglages, vivant
  après 22 s, aucun plantage, puis arrêté. Son FEANOR n'a pas été rouvert à sa place. Worktree et branche gardés.
- **`vue-nette-lissage` fusionnée dans main, poussée, et son FEANOR habituel reconstruit** (e8537c3, exe de 16 h 29), sur son
  « Oui, fusionne » à la question « Je fusionne ce lissage et je reconstruis ton FEANOR habituel ? ». main n'avait pas bougé : la
  fusion a l'arbre vérifié (4 787 réussis, 0 échec, compilation sans avertissement). Il avait fermé son FEANOR lui-même : rien n'a
  été arrêté. Contrôle de lancement : l'exe ouvert sur une copie de « dd » avec de faux réglages, aucun plantage, puis refermé ;
  ses vrais réglages n'ont pas changé. Son FEANOR n'a pas été rouvert à sa place. La session de la timeline a le numéro du commit
  pour refaire sa fusion de main dans `timeline-claire`.
- **Lissé pendant que la vue bouge, et l'image nette refaite en route** (branche `vue-nette-lissage`, section 166.8). Son premier
  retour, la vue nette dans son FEANOR habituel : « On voit encore la pixelisation au moment du zoom ou quand on déplace la vue.
  Est-ce qu'on peut ajouter un système d'anti-aliasing pour régler ce problème ? ». La cause : l'image nette ne se refaisait qu'à
  l'arrêt ; en attendant, on voyait l'image de toujours, en marches. **Ce qui est fait** : tant qu'une image nette est montrée ou
  attendue, la carte graphique pose l'image de toujours lissée quand la vue la grossit (adoucie au lieu d'être en marches : la bande
  que la vue découvre, tout le dessin pendant un zoom, la zone d'un trait en cours) ; l'image nette se refait pendant le mouvement,
  une à la fois, sans attendre le repos ; déplacée seulement, elle est recalée sur les pixels de l'écran et reste nette. Un plan
  tout en bitmap et une zone sans rien de vectoriel gardent leurs pixels nets. La règle « sans latence » est précisée : rien ne se
  calcule pendant un geste de dessin (trait en cours, ou bouton tenu sans que la vue bouge) ; un geste de vue n'arrête rien.
  **Les essais** : onze dans `vuenette_tests.rs`, dont un sur la carte graphique qui lit l'écran peint à 400 % (en marches sans la
  vue nette, lissée quand une image nette est attendue, bord fin une fois arrivée). Vérificateur complet (15 h 24) : compilation
  sans avertissement, 4 787 réussis, 0 échec, dix groupes OK. **Pas regardé à
  l'écran** : aucune capture cette fois (il travaillait à son poste). Le dossier de travail `vectoriel-net` et le dossier d'essai
  avaient été retirés juste avant, à sa demande (« Oui, retire-les », branche gardée, 2,1 Go rendus).
- **Son FEANOR habituel reconstruit depuis main** (567c90c, exe de 14 h 39), sur son « Oui, reconstruis-le » : prévenu qu'il y
  perdait la timeline plus claire et la vue 3D à côté qu'il essayait (branche `timeline-claire`, non fusionnée, intacte), il a
  choisi d'avoir le trait vectoriel net tout de suite. Le dossier `Source` est repassé de `timeline-claire` à `main` (il était
  propre). Il avait fermé son FEANOR lui-même : rien n'a été arrêté. Ses réglages : `timeline-claire` n'y ajoute qu'un champ
  (`part_espace3d`), que main ignore ; une copie est dans `Documents\SHOT\Tests\reglages avant reconstruction 2026-11-17`.
  Contrôle de lancement : l'exe ouvert sur une copie de « dd » avec de faux réglages, aucun plantage, puis refermé ; son FEANOR
  n'a pas été rouvert à sa place (il travaillait dans Photoshop). La session de la timeline est prévenue : pour reprendre son
  essai, elle fusionnera main dans sa branche.
- **`vectoriel-net` fusionnée dans main et poussée** (7742e31, 14 h 25), sur son « Fusionne directement », sans son essai au stylet.
  Avant : le main du jour (633c450) fusionné dans la branche, sans conflit, et le vérificateur complet sur cet arbre : compilation
  sans avertissement, 4 784 réussis, 0 échec, les dix groupes OK. **Son FEANOR habituel n'est pas reconstruit** : il l'a construit
  lui-même ce matin depuis `timeline-claire` (c8fdfd5, le dossier `Source` est sur cette branche) et il est en train de l'essayer
  (liste 164.3) ; le reconstruire depuis main lui retirerait la timeline plus claire et la vue 3D à côté. La session de la timeline,
  consultée, fusionnera main dans sa branche après son retour : il aura alors les deux. En attendant, la vue nette s'essaie par le
  dossier `Documents\SHOT\Tests\essai vectoriel-net` (l'exe de 14 h 06, lancé une fois sur la copie : il s'ouvre, aucun plantage).
- **Le trait vectoriel net au zoom** (branche `vectoriel-net`, partie 166 ; fusionnée ensuite, voir au-dessus). Anthony, avec une capture de son projet
  « dd » : « je trouve le trait très pixélisé alors que c'est du vectoriel, tu peux m'expliquer pourquoi et comment résoudre ce
  problème ? ». **La cause**, mesurée sur sa capture : la vue était à 225 % (un pixel du dessin = 2,25 pixels d'écran), et FEANOR
  montre un dessin vectoriel à travers une image en pixels à la taille de la feuille, grossie sans lissage. Rien n'était abîmé dans
  le dessin. Trois remèdes proposés ; il a choisi le premier : « Oui, construis-le ». **Ce qui est fait** : au-delà de 100 %, la zone
  que la vue regarde est composée à la taille de l'écran sur un autre fil, les dessins vectoriels rejoués à cette échelle par le moteur
  des pinceaux (ses traits gardent tous le réglage complet du pinceau), le bitmap laissé en pixels ; l'image est posée par-dessus
  celle de toujours. Rien ne se calcule pendant un trait ; la zone du trait est en pixels le temps du geste, puis redevient nette.
  Le dessin, le fichier, l'export ne changent pas ; aucun texte, aucun réglage, aucun format. **Les essais** : huit dans
  `vuenette_tests.rs` (le bord d'un trait tenu à 0,3 pixel de la feuille à 400 %, là où l'image grossie échoue ; le bitmap au pixel
  près ; aucun calcul pendant un geste ; un trait tracé au pointeur dans la vraie fenêtre sans écran, à 300 %, puis Ctrl+Z).
  Vérificateur complet sur le code final (14 h 05) : 4 784 réussis, 0 échec, dix groupes OK, le seul avertissement connu ; la
  première suite avait sept échecs, tous de mon ajout (le champ `nette` à déclarer dans `etat_projet!`, et la vue nette qui
  réveillait l'écran dans les essais de repos : elle est éteinte d'origine dans les essais, allumée dans les siens).
  **Regardé à l'écran par Claude** (12 h 53, exe de la branche, copie de « dd », réglages neufs) : plan 2
  grossi à environ 350 %, les traits nets, les bords au pixel d'écran (mesuré sur les pixels réels). **Pas regardé à l'écran** :
  tracer, déplacer et tourner la vue, annuler, plusieurs calques, la pelure, un calque bitmap : le contrôle a été arrêté parce
  qu'Anthony travaillait dans Photoshop et que les captures d'écran réduisaient ses fenêtres (rouvertes, Photoshop remis devant).
  **Ce qui n'est pas net** (l'image de toujours reste seule) : plan sans papier, calque à effets, carte inclinée de l'Espace 3D et
  caméra libre, vue en perspective, lecture, cases des Vignettes ; et la caméra qui cadre serré, l'export, les vignettes ne changent
  pas. **À juger par lui** : les traits très fins (plus fins et plus francs au zoom qu'à 100 %), les brosses à grain, les dessins
  très chargés à fort zoom, la zone en pixels pendant le trait.
- **Timeline plus claire** (INTERFACE partie 168, d'abord numérotée 164 puis 167, branche `timeline-claire`, partie d'`espace-a-cote`), à sa demande : repenser
  l'interface autour des clés pour 15 calques. Deux séries de dix pistes en maquettes (page « Timeline de Feanor, dix pistes ») ;
  la première écartée (« à côté du vrai problème », « trop compliquées »). Ses gênes : retrouver un calque, lire le timing, la
  place, les en-têtes chargés ; ses règles : ordre de la pile, temps horizontal, rien d'automatique. Gardées : l'exception seule
  sur les lignes, une barre de réglages du calque courant, la couleur du rangement sur toute la ligne, des dossiers-titres qui se
  replient en montrant les clés de leurs calques. Toutes les lignes à 22 points (fin de la ligne complète de la partie 160).
- **`main` (567c90c, parties 162 à 166 du `main`) fusionné dans `timeline-claire`**, à sa demande (« fusionne »). La partie 164 de
  la branche devient la **167** (le `main` avait déjà sa 164 : la Couleur, les Effets et la ligne du calque plus visuels). Conflit
  principal dans `layerheads.rs` : le `main` avait refait la ligne complète du calque courant, que cette branche supprime ; ses deux
  apports passent dans la barre du calque : l'opacité en glissant sur la miniature (un dossier et un calque Couleur unie gardent la
  barre) et le mode par ses pastilles. Deux essais de `visuels164_tests` adaptés : la miniature est dans la barre, à droite des
  icônes ; le pointeur se pose un instant avant d'appuyer (à sa première image, egui pose la bulle par-dessus son élément, et un
  appui immédiat allait à la bulle). Suite sur le Mac : les mêmes 89 échecs propres au Mac qu'avant, plus un essai de temps de la
  vue 3D qui passe seul (trois fois sur trois). Pas regardé à l'écran.
- Essais sur le Mac : `RUSTUP_TOOLCHAIN=stable`, `RUSTFLAGS="-Awarnings -L native=<remplaçants>"` ; les remplaçants vides de
  kernel32 et psapi (six fonctions de mesure) sont hors du dépôt, dans le dossier de travail de la session.

## 2026-11-16

- **Zoom 1 et 2 dans la vue 3D à côté** (à sa demande) : `raccourcis-muets` fusionnée dans `espace-a-cote` (le zoom 3D au
  clavier y est), puis la souris sur la vue d'à côté, 1 et 2 la zooment ; sur la feuille, la feuille. Essai
  `the_zoom_keys_zoom_the_side_view_under_the_mouse_and_the_sheet_elsewhere`. Fusionner `espace-a-cote` dans `main`
  y apportera aussi la partie 162. Mesure après remise sur `main` : trait 5,6 à 5,75 ms par image vue fermée, 5,9 à 6,3 ms
  ouverte (médianes, essai seul ; l'interface du `main` à jour coûte plus que celle d'`espace-libre`, 2,07 ms).
- **`espace-a-cote` remise sur le `main` à jour** (e1f18b2, parties 145 à 161), à sa demande : fusion du `main` dans la branche,
  neuf fichiers en conflit réglés à la main. La partie 144 d'INTERFACE devient la **163** (le `main` avait déjà sa partie
  144, les vues multiples). Le `main` refait déjà par morceaux la carte sur laquelle on dessine dans la 3D
  (`render::cel_image_zone`, `espace3ddessin.rs`) : la vue d'à côté passe d'abord par lui, puis par ses propres morceaux pour
  les dessins faits sur la feuille. Réglages montre toujours l'outil (sa décision de la partie 150) ; les réglages de la
  vue d'à côté sont dans l'onglet « Espace 3D », qui ne passe pas devant à l'ouverture. Un essai cliquait au centre de la
  vue, où la caméra du plan est désormais choisissable : il clique un peu à côté, sur la carte. Dessiner dans la 3D et tracer
  une Trajectoire restent réservés au plein écran (la vue d'à côté ne dessine jamais, comme prévu) ; plusieurs vues 3D
  (partie 144) s'affichent aussi dans la vue d'à côté, pas essayé.
- Vue 3D à côté du dessin (INTERFACE partie 163, d’abord numérotée 144 ; ESPACE3D partie 13), demandée depuis la session de la page de vente :
  aujourd'hui la vue Espace 3D remplace la feuille, la page de vente n'en parle donc pas tant que ce n'est pas fait et
  essayé. Plan écrit et validé par Anthony le jour même : partage de la zone du dessin (feuille à gauche, 3D à droite, barre
  à tirer) plutôt qu'un panneau ; un geste va à la vue où il commence ; Réglages de la dernière vue cliquée ; carte refaite
  par morceaux pendant un trait, vue 3D refaite au plus 30 fois par seconde. Branche `espace-a-cote` partie d'`espace-libre`
  (choix d'Anthony : elle se fusionne après elle), dans le dossier de travail `Feanor-a-cote` sur le Mac.
- Fait : commande `espace3d.a_cote` (bouton après le cube, menu Affichage > Plateau, palette, sans touche), plein écran
  inchangé (« Agrandir », et le refermer rend le côte à côte), lecture et trait non interrompus. Mesure sur le Mac
  (1920 × 1080, 10 calques, trait à 240 images/s) : 2,07 ms par image vue fermée, 2,25 ms ouverte (médianes) ; une carte
  refaite en entier 3,2 ms, un bout de trait 0,11 ms. À mesurer sur le PC Windows.
- Rien regardé à l'écran (FEANOR ne se lance pas sur ce Mac comme sur le PC) : liste de 12 points dans la partie 163.
- À savoir pour la page de vente : Storyboard Pro montre déjà ensemble une vue caméra et des vues de dessus et de côté de
  son espace 3D (non revérifié) ; la vue côte à côte n'est donc sans doute pas une exclusivité.
- **Les raccourcis qui manquaient parfois** (branche `raccourcis-muets`, partie 162, depuis `main` e1f18b2) : à sa remarque « les
  raccourcis manquent parfois, notamment le zoom », sur la feuille aussi. Pas reproduit ; trois causes trouvées en lisant le code :
  un glisser de la bande de montage dont le lâcher n'était pas vu restait en cours et mettait toutes les touches de côté (corrigé :
  fini comme au lâcher dès qu'aucun bouton n'est appuyé) ; un bouton qui avait le clavier ne le rendait jamais (le code prévu ne
  jouait pas, corrigé) ; et la barre d'état dit maintenant pourquoi une touche de raccourci n'a rien fait (glisser en cours, case où
  l'on écrit, fenêtre ouverte). Et dans l'Espace 3D, 1 et 2 zooment la vue 3D (avant : la feuille cachée). 7 essais, rouges
  avant pour les corrections ; suite complète sur le Mac : les mêmes 89 échecs que `main` (propres au Mac), aucun nouveau. Fait et essayé sur le Mac (bibliothèque seule,
  sans le vrai exe) : pas regardé à l'écran, pas de version portable de cette branche. Non fusionné : attend son essai.

## 2026-11-13

- **Ses deux décisions, regardées à l'écran par Claude, et un défaut corrigé** (20 h 55 à 21 h 15, souris simulée, exe de la
  branche construit à 20 h 54 puis à 21 h 10, copie du projet `afa`, réglages neufs, fenêtre rétrécie). **Le bouton
  « Caméra »** : outil Trajectoire en main dans la vue Espace 3D du plan 1, un clic sur la case : la caméra choisie seule (son
  dessin en bleu, sa piste allumée, plus aucun calque choisi), la case allumée, la légende de l'outil Trajectoire toujours en
  bas de la vue ; un trait long au sol : « Trajectoire tracée : la caméra, 0 marque, de l'image 31 à l'image 60. » **Les
  dessins posés à la main**, sur la feuille (plan 5, calque « Dessin », qui avait déjà deux dessins) : le chemin tracé en clés
  d'images laisse le second dessin à son image ; une marque posée, « Nouveau dessin » (qui insère une image), un trait de
  pinceau dessus, la marque glissée deux fois : les dessins restent à leurs images, aucune fenêtre ne s'ouvre. **Défaut vu
  à l'écran, corrigé** : la phrase « dessin posé à la main gardé », écrite à la suite du message du geste, ne se lisait
  pas : la barre d'état ne montre que la première phrase d'un message long, puis « … ». Elle passe maintenant devant, et sans
  parenthèse pour un seul calque ; lue à l'écran après correction : « Trajectoire : 2 dessins posés à la main gardés aux
  images 18 et 19, entre les marques… ». Les essais vérifient désormais ce que la barre montre (`messages::resume`), plus
  seulement le message entier. À savoir : un calque qui avait déjà plusieurs dessins avant le chemin les garde tous (ils
  comptent comme posés à la main), et ils ne suivent pas le chemin. Pas regardé à l'écran, couvert par les essais : le jalon
  posé à côté (point 28), les deux cartes dans l'espace (29), la suppression (30), le calque dupliqué (31). Filtre
  `trajectoire` : 67 réussis. Vérificateur complet sur ce code (21 h 11) : 4 620 réussis, 0 échec, dix groupes OK, le seul
  avertissement connu. La fenêtre d'essai (copie du projet, faux réglages) est rouverte pour lui avec cet exe.
- **Deux décisions d'Anthony sur ce qui restait imparfait** (branche `trajectoire-espace`, partie 152 ; regardé à l'écran le
  soir, voir au-dessus). **Le bouton « Caméra »** : « Le bouton « Caméra » de la barre de la vue fait quitter
  l'outil Trajectoire. On garde l'outil ». Avec l'outil Trajectoire dans la vue Espace 3D, la case « Caméra » choisit la caméra
  seule (Maj+clic : avec les cartes), reste allumée, et l'outil garde le pointeur : le trait suivant trace le chemin de la
  caméra. Avec un autre outil, la case fait ce qu'elle faisait. **Les dessins posés à la main** : « Une clé de dessin posée à
  la main entre deux marques est écrasée au recalcul suivant du chemin : une boîte de dialogue demande si on écrase le dessin ou
  non », puis « Je préfère qu'on évite au maximum la boîte de dialogue. Tout ce qui peut s'en passer s'en passera ». Ce cas
  s'en passe : en clés d'images, sur la feuille comme dans l'espace, la trajectoire ne remplace plus jamais une clé de dessin
  qu'elle n'a pas posée. Ta clé reste à son image avec son dessin, le dessin de la marque d'avant est exposé jusqu'à elle ; une
  marque qui tomberait sur son image est posée sur l'image libre d'à côté ; la barre d'état le dit une fois (« Trajectoire :
  1 dessin posé à la main gardé à l'image 18, entre les marques. »), aucune fenêtre ne s'ouvre. Rien de neuf dans le fichier
  (format 55). Aucun cas n'a demandé de question. Arrangé en chemin : un calque dupliqué exposait, à son premier calcul, les
  dessins de l'original (les deux calques partageaient leurs poses) ; il garde les siens. Vu en lisant le code, pas réglé :
  Échap pendant qu'on glisse sur la timeline la clé de position d'un jalon (carte en clés d'images) ne rend pas les clés de
  dessin. Vérificateur complet (10 h 07) : 4 619 réussis, 1 échec (un essai de repos des Vignettes sans rapport, réussi repassé
  seul le soir sur le même code), dix groupes OK, le seul avertissement connu. Le travail s'est arrêté le matin sur la limite
  d'usage, avant l'enregistrement ; relu, complété et enregistré le soir. À vérifier à l'écran par Anthony : points 26 à 31 de
  la partie 152.
- **Ce qui restait imparfait, regardé à l'écran par Claude** (4 h 20 à 4 h 40, souris simulée, exe de la branche construit à 4 h 38,
  copie du projet `afa`, réglages neufs, fenêtre rétrécie hors de la fenêtre invisible de Claude). Un clic simple sur la caméra avec
  l'outil de la vue la choisit seule, alors que des cartes sont derrière ; la rangée « Caméra » du montage est la rangée choisie et
  le panneau Calques ne montre plus aucun calque choisi ; un clic sur « Vectoriel 3 » dans Calques reprend le calque et lâche la
  caméra ; un clic sur le nom de la piste « Caméra » du montage reprend la caméra. Le panneau Espace 3D arrive en onglet de Réglages
  et passe devant ; vu trop bas (il gardait la hauteur de Réglages et coupait la liste des cartes) : réglé par Claude (238e6e1,
  `ajuste.rs`), il prend maintenant la hauteur de ses cartes et de la caméra, et Réglages reprend la sienne à la fermeture. Suite
  complète sur 238e6e1 : 4 614 réussis, 0 échec, le seul avertissement connu. Pas regardé à l'écran : une marque ajoutée au milieu
  d'un chemin en clés d'images (couvert par trois essais), le stylet.
- **Ce qui restait imparfait après la partie 152, réglé** (à sa demande : « règle ce qui reste imparfait » ; branche
  `trajectoire-espace`, quatre commits, rien fusionné, rien poussé, rien regardé à l'écran). (1) Avec les outils de la vue,
  un clic simple sur le dessin de la caméra la prend, même avec une carte derrière, et la tirer la déplace, elle. (2) Un clic
  sur la piste « Caméra » de la timeline ou du montage (son nom, le vide de sa rangée, une clé) choisit la caméra seule, la
  vue Espace 3D montrée ; vue fermée, rien de plus qu'avant. (3) La caméra choisie seule, le panneau Calques ne montre plus
  de calque choisi ; un clic sur un calque ou son aplat, un outil de dessin pris ou un trait dans la vue le rendent. (4) En
  « Clés d'images », chaque copie reste à son jalon, sur la feuille comme dans l'espace : une marque posée au milieu reçoit
  une copie du dessin du jalon d'avant, les poses redessinées ne se décalent plus ; rien de neuf dans le fichier. À lui de
  dire (partie 152, points 22 à 25) : si le dessin de la caméra prend le clic d'assez près, et si la piste « Caméra » devrait
  se montrer choisie vue fermée. Essais : 6 nouveaux, 1 adapté. Vérificateur, deux passages : 4 611 réussis et 2 échecs de
  rendu et de temps au premier (une autre session lançait ses essais en même temps ; réussis relancés seuls), puis 4 613
  réussis, 0 échec ; aucun nouvel avertissement.
- **Le panneau Espace 3D devient un onglet de Réglages** (partie 152, point 3, modifié après sa réponse ; branche
  `trajectoire-espace`, rien fusionné, rien regardé à l'écran). Entre « Colonne à part » et « Onglet de Réglages » (comme il
  l'avait rangé lui-même dans son essai), Anthony a choisi : « Onglet de Réglages ». La colonne est retirée ; le panneau
  arrive en onglet après Réglages et la fiche quand la vue s'ouvre, passe devant, Réglages revient devant à la fermeture, et
  l'onglet reste ; rangé, fermé ou glissé à la main, il est respecté. Une disposition retenue qui l'a en onglet de Réglages le
  garde. À lui de dire : si la hauteur du groupe suffit quand l'onglet Espace 3D est devant (le bord avec la couleur ne
  s'ajuste pas tout seul). Essais : 4 récrits, 4 adaptés, 1 nouveau. Vérificateur : 4 607 réussis, 0 échec, aucun nouvel
  avertissement.
- **Partie 152 regardée à l'écran par Claude** (1 h 46 à 1 h 57, souris simulée, exe de la branche construit à 1 h 45, copie du projet
  `afa`). Avec des réglages neufs : la vue Espace 3D ouverte, le panneau « Espace 3D » arrive dans une colonne à droite de Réglages,
  Réglages montre le Pinceau à côté ; un clic dans la liste des cartes de la colonne choisit la carte, ses champs apparaissent.
  Outil Trajectoire, Ctrl+clic sur la caméra : elle est choisie seule, la rangée « Caméra » du montage est la rangée choisie, aucune
  rangée de calque ne l'est ; Ctrl+clic sur une carte la reprend et lâche la caméra. « Clés d'images » : un chemin au sol et une
  marque sur la carte « Vectoriel 4 » donnent, dans la timeline, une clé de dessin à la marque (image 38) ; chemin retiré ensuite
  par « Effacer ». Outil Sélection dans la vue : un lasso autour du trait bleu de la carte sélectionne cinq éléments, la boîte et ses
  poignées sont posées sur la carte ; Échap la lâche. Dans les réglages de son propre essai, Anthony avait rangé le panneau
  « Espace 3D » en onglet à côté de « Réglages » et « Fiche » : il y est resté, et ses boutons répondent. Piège du banc d'essai : une
  fenêtre de l'application Claude, invisible sur les captures, couvrait la droite de l'écran et y prenait les clics ; vu avec
  `WindowFromPoint`, contourné en rétrécissant la fenêtre d'essai, puis remise en plein écran. Pas regardé à l'écran : le stylet,
  le déplacement de la sélection, couper / coller sur une carte, Maj+Ctrl+clic, l'espace Animation.
- **Retours d'Anthony sur la Trajectoire dans l'Espace 3D** (partie 152, branche `trajectoire-espace`, rien fusionné, rien regardé
  à l'écran). Ses quatre phrases après son essai des parties 149 et 150, et ce qui a changé. 1. « Un calque est toujours sélectionné
  en même temps » : choisir la caméra la choisit seule (Ctrl+clic avec l'outil Trajectoire, « choisir » du panneau Espace 3D ; Maj
  pour l'ajouter exprès aux cartes), et la timeline et le montage montrent alors la piste « Caméra » comme la rangée choisie, plus
  aucun calque ; choisir un calque la lâche. 2. « Clés d'images ne crée pas de clés d'images » : une carte en clés d'images a
  maintenant une clé de dessin à chaque jalon, copie du dessin du départ qu'on peut redessiner (la copie n'est pas déplacée : la
  carte porte le mouvement). 3. « Place-le à côté de réglages plutôt » : le panneau Espace 3D a une colonne à lui à droite de
  Réglages, 340 points pris au dessin, qui arrive quand la vue s'ouvre et repart quand elle se ferme ; rangé ou ouvert à la main,
  il reste où il est ; l'onglet que la partie 150 avait mis avec la couleur dans les dispositions retenues part une fois. 4. « Le
  lasso ne fonctionne pas dans l'espace 3D » : l'outil Sélection prend le pointeur dans la vue, sur la carte du calque courant
  (lasso, boîte, poignées, Suppr, couper, copier, coller, Échap, annulation) ; Ctrl+glisser trace un lasso depuis un outil de
  dessin, Ctrl+clic prend toujours une carte. À lui de dire : si la rangée « Caméra » se lit bien comme choisie, si la colonne du
  panneau (sa largeur, son va-et-vient avec la vue) convient, si le lasso et ses poignées tombent juste sur une carte en biais.
  Non fait : la sélection sur plusieurs calques dans la vue, le clic simple qui prend la caméra avant une carte. Essais : 12
  nouveaux (caméra seule 4, clés d'images 4, lasso 4) et 8 récrits pour la colonne du panneau. Vérificateur, deux passages : 4 605 réussis et 1 échec chacun, un essai de temps des
  Vignettes différent à chaque fois, vert relancé seul ; dix groupes OK ; aucun nouvel avertissement. Commits 6e72a99, af3d064,
  6ad1f1f, 9232a63, puis la documentation.

## 2026-11-12

- **Partie 150 regardée à l'écran par Claude** (23 h 33, souris simulée, exe de la branche construit à 23 h 32, copie du projet `afa`,
  réglages à part). Vue Espace 3D ouverte sur le plan 6 : « Réglages » montre le Pinceau (taille, opacité) ; dessous, le groupe de
  Couleur a trois onglets et « Espace 3D » est devant. Outil Trajectoire pris : « Réglages » montre « Trajectoire dans l'espace » et
  ses boutons. Ctrl+clic sur le symbole de la caméra, posé devant la surface au sol : c'est la caméra qui est choisie. Vue fermée :
  Couleur revient devant ; rouverte : « Espace 3D » repasse devant. Remarque : dans l'espace Storyboard, le panneau « Espace 3D » n'a
  que la hauteur du groupe de Couleur, une centaine de pixels ; la liste des cartes et les champs demandent de faire défiler ou de
  tirer la bordure. Pas essayé à l'écran : l'espace Animation, une disposition enregistrée avant ce changement, le pinceau qui
  dessine sur une carte avec ses réglages sous les yeux, le stylet.
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
