# Les Vignettes, vues par un storyboardeur : analyse (2026-11-06, branche `vignettes-simplicite`)

Demande d'Anthony : « analyser le fonctionnement de Vignettes et l'améliorer pour le rendre plus simple, plus intuitif et ergonomique ;
rendre cette partie et le pont avec le storyboard le plus simple, sans bugs et sans pertes ». Référence : `suivi/V1.md` (narration pure,
outils limités, pont sans friction, compréhension intuitive).

Comment j'ai regardé : le code de la planche (`boardui.rs`, `cases.rs`, `horschamp.rs`, `debord.rs`, `planchesequencesui.rs`,
`vignette*.rs`, `toolbars.rs`, `workspace*.rs`), les parties 11, 42, 44, 46, 76, 77, 79, 101, 106, 110 et 111 de `docs/INTERFACE.md`,
puis l'application pilotée sans fenêtre (`Banc`) : des essais qui refont les gestes, et quatorze captures d'écran que j'ai lues
(`src/vignettes_simplicite_tests.rs`, essai `vignettes_simplicite_captures`). Je ne dis ici que ce que j'ai vérifié ; ce que je n'ai pas pu
voir est dit tel quel à la fin.

**Ce qui est corrigé dans cette passe** est marqué « corrigé (113) » : voir la partie 113 de `docs/INTERFACE.md`. Le reste est classé, puis
revient à Anthony en décisions ou en propositions.

Gravité : **G** (perte, piège, ou mensonge à l'écran), **M** (friction qui coûte du temps ou une hésitation à chaque fois), **P** (détail).
Gain : ce que l'utilisateur y gagne, en une phrase.

---

## 1. Première ouverture

**Ce qu'on voit.** Le projet s'ouvre sur l'espace choisi dans la fenêtre Nouveau projet (Vignettes ou Storyboard). Dans les Vignettes :
le papier blanc, les cases pâles (trois rangées au moins), deux rangées de six cases dans la vue (réglable), les numéros dans la marge, aucune
case choisie (pas de cadre noir), la colonne de gauche à quatre outils (pinceau, crayon, gomme, caméra), à droite le panneau « Planche »
(sens, textes, numéros), les Réglages de l'outil, la Couleur, le Scénario. En haut, à 30 % d'opacité, deux réglages (taille des cases, cases
par ligne) ; en bas, la barre du passe-partout et du zoom.

**Ce qu'il faut savoir sans que rien ne le dise.**
- Que l'on dessine directement dans une case, sans la choisir d'abord (la ligne d'état le dit une fois : « Vignettes : dessine dans les
  cases. ») : c'est bien.
- Que la **taille des cases** (barre du haut) et le **zoom** (barre du bas) sont deux choses : les deux agrandissent les cases à l'écran, mais
  seule la première change la planche (et son export). Deux commandes pour un même effet visible : **M**, décision 3.
- Que « Cases par ligne » (barre du haut) et « 6 × 2 » (barre du bas, le cadrage d'ouverture) sont aussi deux réglages voisins : **P**.

**Réglages en trop ou mal rangés.** Préférences > Vignettes compte plus de vingt réglages (débord, aimant, couleur des cadres, report du
décor, calque décor bitmap, taille des numéros, fond des numéros, numéros visibles, pastille de version, durée, trois lignes de texte d'un
projet neuf, trois couleurs de lignes, cases par rangée et rangées à l'ouverture, épaisseur et couleur du filet, opacité hors champ,
passe-partout, son opacité, passe-partout à l'export). Pour un logiciel « le plus simple possible », c'est beaucoup ; aucun ne gêne tant
qu'on ne va pas dans les Préférences, et chacun a été demandé. Je ne retire rien : décision 4 (en ranger une partie sous « Plus de
réglages »).

**Ce qui ment.** Rien à l'ouverture.

## 2. Dessiner et enchaîner des cases

**Ce qui marche bien.** Un trait part de n'importe où, entre dans le plan de la case où il tombe, crée le plan d'une case vide, le reste
va sur la planche ; une seule étape d'annulation ; aucune case choisie après le trait. Vérifié par les essais de la partie 101 (toujours
verts) et à l'écran (capture 02 : un trait du papier à travers une case jusque dans une case vide, entier, de la même couleur partout).

**Gestes cachés ou en conflit.**
- **Un outil pris par sa touche échappe à la colonne.** Alt+F (remplissage), S (sélection), T (texte), les formes… : l'outil est pris, la
  colonne des Vignettes ne montre plus aucun outil enfoncé, et rien ne dit lequel est en main. Le pot de peinture sur le papier ne faisait
  rien, en silence. **M**, corrigé (113) : la ligne d'état dit l'outil pris et qu'il n'est pas dans la colonne ; le pot et le cutter sur le
  papier disent où ils agissent. La vraie limite (refuser ces outils dans les Vignettes) est une décision : décision 2.
- **Ctrl + glisser = lasso** dans une case, **Maj + clic = choisir**, **Maj + appui sur le filet = déplacer le plan**, **Maj + Alt = sa
  copie**, **Ctrl + Maj = caméra**, **T tenue = caméra**, **Espace = main**, **N = numéros**. Huit gestes à touche, dont quatre sur Maj.
  Les deux plus utiles ont un retour visible avant l'appui (main sur le filet avec Maj, petite caméra avec Ctrl + Maj) ; les autres n'en ont
  pas. L'onglet « Vignettes » les énumère dans sa bulle, l'aide-mémoire (R) aussi. **M**, pas de correction sans décision : proposition A
  (moins de touches) et décision 5.
- **La gomme hors des cases efface un trait « Hors cadres » entier**, pas un morceau : documenté (partie 76), surprenant quand on frotte
  un bout. **P**.

**Ce qui peut surprendre.**
- **Un trait qui frôle une case pâle lointaine fait naître des plans vides.** La capture 02 le montre : un trait parti du papier sous la
  case 3 a laissé un bout dans la marge (12 %) de la **quatrième** case pâle de la rangée suivante ; cette case a fait son plan (plan 8),
  et **les trois cases pâles d'avant sont devenues les plans vides 5, 6 et 7**, comme le veut la règle « les cases vides sautées deviennent
  des plans vides, pour garder la place » (partie 11, pour un trait tracé dans une case pâle lointaine : un choix voulu). La ligne d'état dit
  « Nouveau plan : dessine-le. », au singulier ; le montage (capture 08) montre bien quatre plans de plus. Un seul Ctrl+Z défait tout. **M**
  (pas une perte, mais quatre plans qu'on n'a pas voulus) : décision 8.

**Ce qui ment à l'écran.**
- Les cases montrent le dessin **12 % au-delà du cadre** (la marge de débord), et le Storyboard, vu par la caméra, le coupe au cadre : un
  trait posé dans cette marge se voit dans les Vignettes et pas dans le film. Le passe-partout des cases (barre du bas) le cache ; il est
  éteint d'origine. **P** (c'est la maquette validée) ; proposition D remet ce point sur la table avec le « cadre vivant ».
- **Une part de trait refusée est invisible.** Dans `horschamp.rs`, une part qui tombe dans une case dont le calque est verrouillé, masqué,
  d'une autre sorte, d'une autre caméra ou d'un mouvement de caméra reste « sur la planche » (sans perte), mais le hors-cadres est peint
  **sous** les cases : cette part disparaît sous la case blanche, et aucune ligne d'état ne le dit (la partie 101 annonce « signalé »,
  je n'ai pas trouvé le message). **G** par le mensonge, pas par la perte. Branche `vignettes-traits-continus` : signalé, non touché ici.

## 3. Ajouter, déplacer, retirer

- **Ajouter** : dessiner dans une case pâle (le plan naît avec le trait ; une seule étape, un clic sans trait ne laisse rien) ; « Nouveau plan »
  (clic droit, anneau, touche) : il naît **après le plan courant**. Or, dans les Vignettes, le plan courant est invisible (aucun cadre après
  un trait) : le plan neuf apparaissait là où on ne regardait pas, sans cadre, et « Plan suivant » / « Plan précédent » (dans l'anneau
  d'origine, à droite et à gauche !) ne changeaient rien de visible. **G** (plan courant qui change sans qu'on le voie), corrigé (113) : une
  commande qui choisit un plan le montre (cadre noir) et fait défiler la planche jusqu'à sa case ; dessiner le libère comme avant.
- **Déplacer** : Maj + appui sur le filet, puis glisser ; repère entre deux plans ; Échap ; une étape. Le geste est bon, mais le filet fait
  un point : à 60 % de zoom il faut viser à cinq points près. **M**, proposition A.
- **Retirer** : clic droit > « Supprimer le plan », ou Suppr après Maj + clic : le plan va à la corbeille (Fichier > Corbeille…), Ctrl+Z le
  rend, la ligne d'état le dit. Un plan supprimé disparaît de la planche ; son numéro saute, les suivants ne sont renumérotés que si la
  numérotation n'est pas verrouillée. **P**. Le chantier « annuler un plan par clic sur son numéro » arrive : je ne touche pas au clic sur le
  numéro.
- **Ce qui peut se perdre** : rien de trouvé. Supprimer, couper, réunir passent par la corbeille ou les garde-fous des versions (parties
  102 et 104, essais verts).

## 4. Continuité entre deux cases

- La barre entre deux cases : grise = même plan, noire = coupe, un clic bascule ; devant la première case pâle, un clic fait aussitôt la case
  de continuité avec la copie du dessin. Bulles claires au survol (« Même plan », « Coupe », « Prochaine case : nouveau plan »). Bon.
- **Caché** : rien ne dit, avant de survoler, qu'une barre se clique. À 100 %, la barre grise fait 1,5 point sur 230 de gris : on ne la voit
  pas sur le papier blanc (capture 01 : il faut savoir qu'elle est là). **M**, proposition B.
- **Un appui du stylet dans l'écart, à la hauteur de la barre, réunit ou coupe les plans sans dessiner** (fait reproduit par le chantier des
  traits continus ; dans le code : `board::hit` rend la barre dès que l'appui tombe dans sa zone, élargie à 14 points, et `board_toggle_cut`
  agit **à l'appui**, avant tout mouvement). Un trait qui part de l'écart entre deux cases, là où tout le reste du papier se dessine, change
  donc le film au lieu de tracer : c'est le contraire du dessin libre. Une étape d'annulation le rend, mais la coupe pose parfois la question
  des versions, et la réunion copie des dessins. **G**. Pas de correctif sûr tout de suite : la barre doit répondre **au lâcher, sans
  mouvement** (un clic), et un appui qui bouge doit devenir un trait **dès son premier point** (règle « sans latence », rien de perdu) ; cela
  demande de garder les points reçus entre l'appui et le seuil de mouvement et de les donner au trait quand il part, un mécanisme qui
  n'existe pas encore sur la planche. Décision 11.
- **Couper un plan à versions** pose une question (« avec quelle partie ? ») ; **réunir** demande parfois le nom des calques
  (`fusionnoms.rs`). Deux fenêtres qui surgissent au milieu d'un geste de planche : nécessaires, mais elles arrivent sans qu'on s'y attende.
  **P** (déjà décidé par Anthony pour la coupe).
- **Ce qui peut se perdre** : la continuité copie le dessin ; réunir deux plans garde les dessins des deux (essais `cases_tests`,
  `versions_fiables_tests`). Rien trouvé.

## 5. Caméra

- L'outil Caméra de la colonne : survol, les cadres se signalent ; clic, la caméra est choisie ; glissé, le cadre bouge ; coin, le zoom ;
  une étape par geste ; les dessins ne bougent jamais (essais verts, capture 10). Le geste Ctrl + Maj et T restent.
- **Caché** : « Ajouter un cadre » (C, D…) n'est qu'au clic droit ou en Ctrl + Alt ; « Retirer ce cadre » au clic droit sur le cadre.
  Avec l'outil Caméra en main, un second glissé depuis le dernier cadre pourrait créer le suivant : non fait (cela changerait le geste
  actuel : décision 6).
- **Trop de réglages avec l'outil Caméra** : dans les Vignettes, prendre l'outil Caméra ouvre dans « Réglages » tout le panneau de la caméra
  du Storyboard (clés, adoucis, position X Y Z, focale, rotation, bascule, panoramique, paliers, effets : capture 10). Pour tirer un cadre A
  vers B, rien de tout cela ne sert. **M**, décision 9.
- **Ment un peu** : un plan d'une seule image refuse le mouvement (« allonge-le d'abord ») : juste, mais l'utilisateur ne sait pas pourquoi
  une image n'a pas de durée. **P**.
- Le passe-partout suit la zone balayée, les numéros ne couvrent plus rien, la flèche est fine (partie 110, essais verts ; capture 09).

## 6. Séquences

- Titre au-dessus de chaque séquence (plusieurs séquences seulement), double-clic ou clic droit pour renommer, panneau Planche > Séquences
  pour montrer, cacher, ordonner. Clair (capture 11).
- **Caché** : une séquence cachée de la planche a toujours ses plans dans le film ; la ligne d'état le dit au moment où l'on cache, puis
  rien ne le rappelle sur la planche. **P**.
- « Nouvelle séquence » (menu du fond) crée **un plan vide à la fin du film** : d'où il faut le trouver ; avec la correction (113) le plan
  neuf est choisi et la planche défile jusqu'à lui.

## 7. Textes

- Action, Dialogue, Notes : cochées dans le panneau Planche, une ligne chacune sous la case (ou à droite), étiquette colorée, se cochent
  seules quand un projet a du texte. La note s'écrit d'un clic sur sa ligne ou sur l'icône crayon (visible au survol). Action et Dialogue
  s'écrivent dans la fiche du plan, qui n'est pas dans l'espace Vignettes : il faut aller au Storyboard pour taper un dialogue. **M** :
  proposition C (écrire les trois lignes en place).
- Les « notes libres » de la planche (outil Texte, outil Sélection, « Attacher les nouvelles notes », « Effacer toutes les notes ») sont une
  seconde famille de textes, d'avant les lignes sous les cases, qui n'est ni dans les plans ni dans le film. Deux systèmes de texte sur la
  même planche : **M**, décision 7 (les garder hors du chemin).

## 8. Passer au Storyboard et revenir

- Les onglets Vignettes / Storyboard / Montage : un clic, instantané, même plan, même image. Bon.
- **Tab** : des Vignettes, Tab mène au Storyboard ; du Storyboard, Tab mène à l'Animation, jamais aux Vignettes (V1.md l'a noté). **M**,
  décision 1.
- **Maj+Tab (« vue dessin / vue vignettes »)** depuis les Vignettes **piégeait** : la planche cédait la place au dessin seul sous l'onglet
  Vignettes ; l'onglet, déjà allumé, ne faisait rien ; et la disposition retenue rouvrait les Vignettes sur le dessin à la séance suivante
  (vérifié par essai : `shift_tab_from_the_vignettes…`, rouge avant la correction). **G**, corrigé (113) : Maj+Tab depuis les Vignettes va au
  dessin du Storyboard, et l'onglet Vignettes rouvre toujours la planche, quelle que soit la disposition retenue.
- **Le retour de l'animation** : « Animer cette image » (clic droit) entre dans le plan ; Tab ramenait aux Vignettes, mais le bouton « Retour
  au storyboard » ramenait au Storyboard. Deux chemins de retour, deux arrivées. **G** (on perd sa place), corrigé (113) : entré depuis les
  Vignettes, le bouton dit « Retour aux Vignettes » et y ramène, comme Tab ; entré depuis le Storyboard ou le Montage, rien ne change.

## 9. Retrouver le même dessin

- Le plan, l'image (marque d'image) et le calque courants sont les mêmes dans les trois espaces ; la case d'un plan à plusieurs images
  montre l'image où elle commence, et un trait y entre (essai `a_stroke_across_cases_of_a_plan_with_several_keys…`, vert). Captures 07 et
  08 : le même plan en Vignettes puis en Storyboard, même dessin, même cadre.
- **Différences voulues** : la case ne montre pas les calques d'annotation du plan ; elle montre la marge de 12 % que la vue caméra du
  Storyboard coupe (voir 2). **P**.

## 10. Annuler, enregistrer, rouvrir

- Chaque geste de planche est une étape nommée (« Nouveau plan dessiné », « Couper le plan entre deux vignettes », « Continuité avec le
  plan précédent », « Déplacer le plan », « Mouvement de caméra »…). Les noms sont lisibles dans l'historique. Bon.
- **Ce qui n'est pas dans le projet** : le sens de lecture, les lignes cochées, la taille des cases, le zoom, le passe-partout sont des
  réglages du poste (`reglages.toml`) ; l'affichage des séquences est dans le projet (`annotations.json`). Un collègue qui ouvre le projet
  ne voit pas la même planche. Décidé (partie 44), **P**.
- Rouvrir : la planche cadre 6 × 2 sur la séquence du plan courant. Bon.

## 11. Un projet long (300 plans)

- La mise en page de 300 plans est faite une fois et gardée (`CacheDisposition`) ; l'essai de 200 plans la mesure à 0,13 ms. Les vignettes
  se composent sur des fils de fond, bornées en mémoire (essais d'optimisation verts).
- Capture 12 (300 plans, ouverture) : sept rangées de six à 78 % (le cadrage « 6 × 2 » met six cases dans la largeur ; la hauteur en montre
  plus), lisible, les numéros gros. Bon.
- **« Cadrer tout » mentait** : sa bulle disait « Toute la page dans la vue », et le bouton ne mettait la page qu'à la **largeur** de la vue :
  sur 300 plans, la capture 13 (avant correction) montre exactement la même chose que la capture 12, les sept premières rangées. **G** (un
  bouton qui ne fait pas ce qu'il dit), corrigé (113) : « Cadrer tout » met toute la planche dans la vue, en largeur et en hauteur, jusqu'au
  zoom le plus petit (20 %) ; « Page à la largeur de la vue » (menu du fond) garde son sens.
- **« Plan suivant »** et « Aller à un plan » : avec la correction (113), la planche défile jusqu'à la case. Sans elle, à 300 plans, « Plan
  suivant » ne faisait rien de visible : **G**, corrigé.

---

## Classement par gravité et par gain

| Gravité | Quoi | Gain pour l'utilisateur | État |
|---|---|---|---|
| G | Maj+Tab piégeait l'espace Vignettes sur le dessin, et la disposition retenue le refaisait à chaque séance | Plus jamais de planche disparue | Corrigé (113) |
| G | Le bouton de retour de l'animation ramenait au Storyboard quand on venait des Vignettes (Tab, lui, aux Vignettes) | On retrouve sa place | Corrigé (113) |
| G | « Plan suivant », « Nouveau plan », « Dupliquer » changeaient le plan courant sans rien montrer ; à 300 plans, rien ne défilait | On voit où l'on est, et le plan neuf | Corrigé (113) |
| G | « Cadrer tout » ne cadrait que la largeur : sur 300 plans, sept rangées, et la bulle disait « toute la page » | Le bouton fait ce qu'il dit | Corrigé (113) |
| G | Un appui du stylet dans l'écart, à la hauteur de la barre, réunit ou coupe les plans au lieu de dessiner | Dessiner partout, pour de bon | Décision 11 |
| M | Un trait qui frôle une case pâle lointaine fait naître plusieurs plans vides, la ligne d'état en annonce un | Pas de plans fantômes | Décision 8 |
| M | Le cadre du plan choisi (gris 80, un point) se distingue à peine du filet (gris 154) | Voir d'un coup d'œil le plan choisi | Décision 10 |
| M | L'outil Caméra ouvre tout le panneau caméra du Storyboard dans les Vignettes | Rien que le cadre A, B, C | Décision 9 |
| G | Une part de trait refusée par une case (calque verrouillé, autre caméra, mouvement) disparaît sous la case, sans message | Ne plus croire un trait perdu | À faire sur `vignettes-traits-continus` (signalé) |
| M | Un outil pris par sa touche échappait à la colonne, en silence ; le pot sur le papier ne disait rien | On sait ce qu'on a en main | Corrigé (113) ; la vraie limite : décision 2 |
| M | Tab depuis le Storyboard ne revient jamais aux Vignettes | Un aller-retour au clavier | Décision 1 |
| M | Taille des cases et zoom : deux commandes pour un même effet visible | Un réglage de moins | Décision 3 |
| M | Huit gestes à touche sur la planche, quatre sur Maj | Moins à retenir | Proposition A, décision 5 |
| M | La barre grise (1,5 point, gris 230) est invisible sur le papier avant survol | Trouver la coupe sans la chercher | Proposition B |
| M | Action et Dialogue ne s'écrivent pas depuis les Vignettes | Raconter sans changer d'espace | Proposition C |
| M | Deux familles de textes (lignes des cases, notes libres) | Une seule idée de « texte » | Décision 7 |
| P | Vingt-trois réglages dans Préférences > Vignettes | Une page plus courte | Décision 4 |
| P | La marge de 12 % se voit en Vignettes, pas dans le film | — | Proposition D |
| P | « Ajouter un cadre » seulement au clic droit | — | Décision 6 |

---

## Décisions à prendre (avec ma recommandation)

1. **Tab depuis le Storyboard.** Aujourd'hui : Storyboard → Animation, et retour. Les Vignettes ne sont qu'à l'aller (Vignettes → Storyboard).
   Deux choix : (a) laisser Tab à l'animation et donner **Maj+Tab** au couple Vignettes ↔ Storyboard (aujourd'hui Maj+Tab, dans le
   Storyboard, montre l'ancienne « vue vignettes » dans la disposition du Storyboard, un doublon de l'espace Vignettes) ; (b) faire de Tab un
   cycle Vignettes → Storyboard → Animation → Vignettes. **Je recommande (a)** : Tab garde son sens (entrer dans le plan, en sortir) et
   Maj+Tab devient le pont, dans les deux sens ; l'ancienne vue vignettes du Storyboard disparaît (elle n'a plus de bouton depuis la partie
   11, seulement cette touche). Exemple : dans le Storyboard, Maj+Tab → la planche ; Maj+Tab → le dessin, même plan.
2. **La vraie limite des outils dans les Vignettes.** Aujourd'hui la colonne n'en montre que quatre, mais toute touche d'outil en prend un
   autre (la ligne d'état le dit désormais). Choix : (a) garder ainsi ; (b) refuser les autres outils dans les Vignettes (la touche ne change
   rien, la ligne d'état renvoie au Storyboard), en gardant Ctrl + glisser (lasso) qui est un geste et non un outil. **Je recommande (b)** :
   c'est la « narration pure » de V1.md, et le lasso, la main et la gomme restent. Risque : les formes (ligne, rectangle) servent parfois à
   un cadre vite tracé ; on peut les garder.
3. **Taille des cases ou zoom.** Les deux grossissent les cases à l'écran. La taille des cases change la planche (et l'export) ; le zoom
   ne change que la vue. **Je recommande** de retirer la taille des cases de la barre du haut (la garder dans Préférences > Vignettes et dans la
   fenêtre d'export, où elle a un sens), et de laisser le zoom seul, en bas. La barre du haut ne garderait que « Cases par ligne ».
4. **Préférences > Vignettes.** Garder en vue : débord, numéros (taille, visibles), lignes d'un projet neuf, cadrage d'ouverture, passe-partout.
   Ranger sous un « Plus de réglages » replié : aimant, couleur des cadres, report du décor, calque décor bitmap, fond des numéros, pastille de
   version, durée, couleurs des lignes, épaisseur et couleur du filet, opacité hors champ, passe-partout à l'export. Rien n'est perdu, la page
   fait deux fois moins long. **Je recommande** ce rangement.
5. **Maj + Alt + glisser (copie du plan)** et **Ctrl + Alt + glisser (cadre de plus)** : deux gestes à deux touches que rien ne montre. **Je
   recommande** de les retirer du geste et de ne garder que leurs commandes (« Dupliquer le plan », « Ajouter un cadre »), avec l'anneau et le
   clic droit ; Maj seule et Ctrl + Maj restent.
6. **Un cadre de plus avec l'outil Caméra.** Avec l'outil Caméra en main, un glissé qui part de l'intérieur du dernier cadre (B) pourrait
   créer C, comme le faisait la première version (partie 11). Aujourd'hui : « dedans B on déplace B ». **Je recommande** de ne pas changer
   (déplacer est plus fréquent que créer), et de garder « Ajouter un cadre » au clic droit et à l'anneau.
7. **Les notes libres de la planche** (outil Texte sur le papier, outil Sélection, « Attacher les notes », « Effacer toutes les notes ») : d'avant
   les lignes sous les cases et les calques d'annotation, elles font double emploi avec les deux. **Je recommande** de les retirer du menu du
   fond et de l'outil Texte dans les Vignettes (un projet qui en a les garde, lisibles et effaçables), pour qu'il n'y ait qu'un texte par
   case et des annotations par-dessus.
8. **Les plans vides d'un trait qui frôle une case pâle lointaine** (capture 02 : un bout de trait dans la marge de la quatrième case pâle a fait
   quatre plans, trois vides). Choix : (a) garder la règle (la place est gardée, un Ctrl+Z défait tout) ; (b) **seule la case pâle où le trait
   est tracé en entier** (où il commence) fait des plans vides devant elle ; un **bout** de trait venu d'ailleurs ne crée qu'un plan, à la fin de
   la séquence, sans plans vides ; (c) un bout de trait ne crée jamais de plan (il reste sur la planche). **Je recommande (b)** : dessiner
   dans une case lointaine garde sa place (son geste voulu), un bout de trait ne fait plus de fantômes ; et la ligne d'état dirait « 4 plans »
   quand il y en a quatre.
9. **L'outil Caméra des Vignettes et ses Réglages.** Dans les Vignettes, ne montrer dans « Réglages » que ce qui sert au cadre (rien, ou
   « Ajouter un cadre », « Retirer le dernier cadre », « Réinitialiser ») et laisser le panneau complet au Storyboard. **Je recommande** de
   le vider dans les Vignettes.
10. **Le cadre du plan choisi.** Aujourd'hui un filet d'un point en gris 80, contre gris 154 pour les autres : on le cherche (captures 03 et 04).
    Plusieurs plans choisis ont déjà un liseré d'accent de 1,5 point. **Je recommande** le même liseré d'accent pour un plan choisi seul :
    une seule façon de dire « choisi », visible de loin.
11. **La barre qui répond à l'appui.** Un appui dans l'écart, à la hauteur de la barre, coupe ou réunit sans dessiner (voir 4). Choix :
    (a) **la barre répond au lâcher d'un clic sans mouvement** ; un appui qui bouge devient un trait dès son premier point (les points reçus
    avant le seuil sont gardés et donnés au trait : rien n'est perdu, rien ne traîne) ; (b) la barre ne répond qu'avec **Maj** (proposition A :
    Maj tenue, la planche montre ses barres, ses poignées, ses filets ; sans Maj, tout est du papier) ; (c) rétrécir la zone de la barre à sa
    largeur réelle (elle est élargie à 14 points) : moins de faux appuis, mais pas zéro. **Je recommande (a) tout de suite** (un clic reste
    un clic, un trait reste un trait, comme un bouton qui réagit au relâché), et (b) avec la proposition A si elle est retenue. Le correctif
    (a) n'est pas fait ici : il touche la façon dont la case vive reçoit l'appui (`card_canvas`, `appui_bande`), le terrain de la branche
    `vignettes-traits-continus`, et il doit garder les premiers points du trait.

---

## Propositions (demande d'Anthony : « s'il trouve d'autres solutions, des idées, des manques, qu'il les propose »)

Peu d'idées, fortes, classées. Pour chacune : le problème, l'image du geste, le coût (petit, moyen, gros), ma recommandation. Rien n'est codé.

**A. Une seule touche pour tout ce qui n'est pas dessiner : Maj.** *Problème* : huit gestes à touche sur la planche (Maj, Maj+Alt, Ctrl,
Ctrl+Maj, Ctrl+Alt, T, Espace, N) ; on dessine avec un stylet, la main gauche n'en retient qu'une. *Image* : Maj tenue, la planche change de
peau : les cases montrent un filet plus fort et une poignée au coin, les cadres de caméra leurs poignées, les barres grossissent ; Maj + glisser
une case la déplace, Maj + glisser une poignée de caméra tire le cadre, Maj + clic choisit, Maj + clic sur une barre coupe ou réunit ; Maj
relâchée, tout redevient du papier. Ctrl garde le lasso (c'est celui du dessin), Espace la main. *Coût* : moyen (un état « Maj tenue » qui
remplace quatre gestes, et le dessin des poignées). *Recommandation* : oui, c'est la simplification qui rapporte le plus ; elle absorbe les
décisions 5 et 6.

**B. La barre de coupe qui se voit.** *Problème* : la barre grise (même plan) fait 1,5 point de gris très pâle : invisible avant survol ; la
coupe noire, 2 points. On ne sait pas, en regardant la planche, où l'on peut cliquer. *Image* : à la place de la barre, dans l'écart, un petit
signe toujours visible : un trait plein noir à une coupe (comme aujourd'hui), un **point** gris au milieu de l'écart entre deux images d'un même
plan ; au survol, le point devient la barre d'accent. *Coût* : petit (`paint_bars`). *Recommandation* : oui.

**C. Écrire Action et Dialogue en place.** *Problème* : les lignes Action et Dialogue sous la case se lisent dans les Vignettes mais s'écrivent
dans la fiche du plan, qui n'est pas dans cet espace ; seule la ligne Notes s'écrit d'un clic. *Image* : un clic sur la ligne Action ou Dialogue
l'ouvre comme la ligne Notes (le même éditeur, Entrée valide, Échap abandonne, une étape « Action de la vignette »). *Coût* : petit (le chemin
existe pour Notes). *Recommandation* : oui, c'est raconter sans quitter la planche. Elle revient sur un choix de la partie 44 (« les lignes
Action et Dialogue laissent dessiner ») : d'où la décision.

**D. Le cadre vivant : ce qui est dans le film est net, le reste est voilé.** *Problème* : la case montre 12 % au-delà du cadre, le film
coupe au cadre ; on dessine un détail dans la marge en croyant qu'il est dans le plan. *Image* : dans chaque case, ce qui est hors du cadre
de la caméra est peint à 40 % (un voile blanc), à l'écran seulement ; le passe-partout des cases reste pour couvrir tout à fait. *Coût* :
petit (une bande de voile par côté, dans `board_card`). *Recommandation* : oui, à essayer avec lui au stylet ; si le voile gêne le trait, le
réglage « opacité du dessin hors des cases » existe déjà pour le doser.

**E. Le « film au doigt » : lire la planche.** *Problème* : pour sentir le rythme de ce qu'on vient de dessiner, il faut aller au Montage.
*Image* : la barre d'espace tenue (ou un bouton « lecture » dans la barre du bas) fait défiler les cases comme un animatique, à la durée des
plans, dans la planche même : la case courante s'allume l'une après l'autre, sans rien changer au film. *Coût* : moyen (une tête de lecture
sur la planche ; la lecture existe). *Recommandation* : après les quatre premières ; c'est le manque le plus net pour « raconter vite ».

---

## Ce que j'ai vu à l'écran (captures lues)

Quatorze PNG de 1920 × 1080, faits par l'application sans fenêtre (`vignettes_simplicite_captures`), lus un par un.

1. **Ouverture (4 plans)** : la planche à 78 %, six cases par rangée, deux rangées de cases pâles dessous ; les numéros 1 à 4 gros et nets ; aucun
   cadre noir ; les barres de coupe (noires) visibles, **les barres grises entre les cases d'un même plan invisibles** sur le papier ; la barre
   du haut (taille, cases par ligne) presque effacée à 30 % ; en bas, passe-partout, zoom 78 %, « 6 × 2 », « Cadrer tout ».
2. **Trait du papier à travers une case** : le trait entier, de la même couleur, dans le plan 3, le plan 4, sur la planche ; et les plans 5, 6, 7
   (vides) et 8 (le bout) nés d'un bout de trait dans la marge d'une case pâle (voir 2 et décision 8). « Hors cadres » apparaît dans le panneau Planche.
3. **Maj + clic** : le cadre de la case choisie passe du gris 154 au gris 80, un point d'épaisseur : on le voit si on le cherche (décision 10).
4. **« Plan suivant »** (après correction) : le cadre passe à la case suivante.
5. **Outil hors colonne** : la ligne d'état dit « Outil Remplir … » (coupée au premier point, le reste dans sa bulle) ; les Réglages montrent
   l'outil Remplir ; aucune case de la colonne enfoncée.
6. **Animation entrée depuis les Vignettes** : « Retour aux Vignettes » à gauche des onglets (après correction) ; le dessin du plan 3 à 488 %, vu
   par la caméra ; la timeline en bas.
7. **Retour aux Vignettes** : la planche, aucun cadre, le plan 3 toujours courant.
8. **Storyboard, même plan** : le même trait dans le même cadre ; le montage en bas montre les plans 5, 6, 7 vides.
9. **Mouvement de caméra** : A et B côte à côte, la fine flèche, la case élargie ; lisible.
10. **Outil Caméra** : les poignées des coins, la petite caméra ; mais les Réglages montrent tout le panneau caméra (décision 9).
11. **Deux séquences, trois lignes de texte** : titres « 1 · 3 plans », « 2 · 1 plan » ; sous chaque case, Action, Dialogue, Note avec leur
    couleur, le texte du plan 2 lisible à 81 %.
12. **300 plans, ouverture** : sept rangées à 78 %, numéros nets, le compte « 300 » dans le panneau Planche.
13. **300 plans, « Cadrer tout »** (avant correction) : la même vue que la 12, sept rangées : le bouton ne cadrait pas tout (corrigé).
14. **Préférences > Vignettes** : six groupes bien rangés (Planche, À l'ouverture, Numéros et textes, Aspect des cases, Cadres de caméra,
    Décor) ; la page dépasse la fenêtre d'un écran de 1080 : il faut défiler pour Décor. Moins gênant que le compte ne le laissait croire :
    décision 4 reste, en dernier.

## Ce que je n'ai pas vérifié

- Le stylet réel (pression, bouton latéral, anneau) : le banc envoie des souris et des pressions « tactiles » ; rien sur le pilote de la tablette.
- Les pixels composés d'une vignette en fond contre le dessin vivant : je n'ai pas comparé pixel à pixel une case et le Storyboard ; j'ai comparé
  les deux captures à l'œil (même dessin, même cadre).
- L'export de la planche et le PDF du storyboard après les corrections : non concernés par les changements (aucun ne touche aux données), non relancés.
- Le film partagé (plusieurs personnes) sur la planche.
