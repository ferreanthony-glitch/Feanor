# FEANOR : ce qui n'a jamais été vu à l'écran, à vérifier en premier

Relevé du 21 novembre 2026, sur `main` 23cb4e4 (« Partie 201 : finitions 3D », 20 novembre), en lecture seule : ni cargo
ni l'application n'ont été lancés.
Sources : docs/INTERFACE.md (parties 1 à 201), suivi/JOURNAL.md, ETAT.md, NOTES.md, V1.md, AUDIT-2026-10-30.md,
AUDIT-2026-11-10.md, docs/DOUBLONS.md, FIABILITE.md, ROADMAP.md, ESPACE3D.md, ESPACE-LIBRE.md, ENDURANCE.md,
AUDIT-RENDU-BROSSES.md. Chaque point a été recoupé avec le code (`src/`) et `git log`.

**Comment lire.**
- « SHOT » et « Pelure » sont d'anciens noms de FEANOR.
- Ce qui touche Storyboard Pro et Toon Boom (retirés le 30 octobre, 17dda2a) est écarté.
- Un point compte comme « jamais vu » sauf si une source plus récente dit qu'Anthony (ou le coordinateur, à l'écran) l'a
  vu. Si une partie n'a été vue qu'en partie, seul le reste figure ici.
- Les gestes sont écrits **pour Mac** : Cmd remplace Ctrl, Option remplace Alt. Le code lit bien `modifiers.command`, mais
  l'interface écrit encore « Ctrl » partout.
- Lignes « l. N » : ligne de docs/INTERFACE.md, sauf mention d'un autre fichier.

**Les deux audits hors dépôt** (/Users/anthony/CLAUDE/AUDIT-ARDA-2026-11-17.md et AUDIT-TABLEAUX-2026-11-19.md)
portent sur ARDA, une autre application (application web de carnet de références), pas sur FEANOR. Ils ne citent FEANOR
qu'une fois, pour le profil de raccourcis « Feanor » d'ARDA (AUDIT-TABLEAUX l. 248), et ce défaut est celui d'ARDA. Ils
ne fournissent donc rien à la partie 4. Celle-ci reprend les deux audits internes : AUDIT-2026-10-30 (constats B1 à B10)
et AUDIT-2026-11-10.

## Avant tout : FEANOR n'a jamais été vu tourner sur Mac

Le projet est fait pour Windows d'abord. Sur Mac, les sessions ont seulement compilé le programme et lancé les essais
automatiques : 86 à 90 essais y échouent, pour des raisons « propres au Mac » qui n'ont jamais été listées
(ESPACE-LIBRE.md A0 point 6, l. 181 ; JOURNAL l. 63). JOURNAL l. 615 dit : « FEANOR ne se lance pas sur ce Mac comme sur
le PC ». Vérifiés dans le code, cinq points peuvent bloquer le test dès la première minute :

1. **Rien n'est enregistré hors du projet.** `src/donnees.rs:22-29, 46` lit les variables `APPDATA` et `LOCALAPPDATA`,
   qui n'existent pas sur Mac. Conséquence : ni réglages, ni projets récents, ni raccourcis personnels, ni brosses, ni
   copie de secours, ni reprise après plantage (`recovery.rs:77`). Lancer FEANOR avec ces deux variables pointant vers un
   dossier d'essai, ou noter que ces fonctions sont hors d'état.
2. **ffmpeg n'est jamais trouvé seul.** `src/export.rs:352-384` ne cherche que `ffmpeg.exe`. Sans `PELURE_FFMPEG=/chemin/ffmpeg`,
   pas d'export vidéo, de GIF, d'import de vidéo ni d'animatique.
3. **La pression du stylet manque probablement.** Elle ne vient que de `Event::Touch { force }` (`src/canvas.rs:469`),
   que winit 0.30 ne produit pas, à notre connaissance, pour une tablette sous macOS. Le bouton du stylet, la gomme au dos
   et l'anneau sont propres à Windows (`stylet.rs:346, 543`). L'inclinaison n'est branchée nulle part.
4. **« Ouvrir le dossier » et « Montrer dans l'explorateur »** lancent `explorer.exe` (`src/exportui.rs:1082-1101`) :
   aucun effet sur Mac. Il faut retrouver les fichiers exportés à la main.
5. **Touches prises par macOS ou mal lues.** Cmd+Espace (zoom temporaire) est pris par Spotlight. Cmd+Tab (onglet de projet
   suivant) est pris par macOS. F11 (mode concentration) est une touche système. « Ctrl tenu = lasso » lit la vraie touche
   Control, pas Cmd (`springtools.rs:94`). Cmd+Q peut quitter sans passer par la fermeture de FEANOR. Les heures (jalons,
   projets remplacés) s'affichent en UTC (`remplacer.rs:95`).

La liste complète des dégradations propres au Mac est dans la partie 2, section « Spécifique Mac ».

## Les 15 vérifications à faire en premier (celles dont l'échec bloquerait un vrai storyboard)

1. **Les réglages et la copie de secours survivent à une relance** (voir ci-dessus, point 1). Changer un réglage, ouvrir
   un projet, quitter, relancer : le réglage et le projet récent doivent revenir. Dessiner, forcer à quitter
   (Cmd+Option+Échap), relancer : la reprise doit être proposée. Sources : src/donnees.rs ; INTERFACE 135 et 67.
2. **Le premier trait au stylet.** La pression doit faire varier l'épaisseur, la croix doit rester collée au trait, sans
   retard. Sources : ETAT §5.1 ; partie 114 ; src/canvas.rs:469.
3. **L'écriture continue et les jalons** (partie 135, qui remplace « Enregistrer »). Dessiner, forcer à quitter, rouvrir :
   le dernier trait doit être là. Poser un jalon avec Cmd+S, continuer, revenir au jalon : l'état du jalon doit revenir.
   Source : INTERFACE 135, l. 9910 (liste l. 9987).
4. **Le pont Vignettes → Storyboard.** Dessiner trois cases dans les Vignettes, puis passer au Storyboard : les dessins
   doivent être dans les plans, sans copie. Pour un trait qui déborde d'une case (règle A, partie 131, liste en 15 points),
   le trait doit rester entier et sa sélection au Cmd+lasso de la planche doit le prendre en entier (parties 125 et 127).
   Sources : V1.md « Pont » ; INTERFACE 131, 125, 127.
5. **Le calque Aplat**, à faire « parfaitement » selon V1.md. Activer « Aplat sous le trait », fermer un contour : l'aplat
   doit apparaître sous le trait, suivre une retouche du trait et passer à la clé suivante. Cmd+Z doit l'annuler en une
   étape. Sources : parties 33, 56, 63, 103, 133 ; V1.md Fiabilité.
6. **Le versionnage, et rien ne disparaît.** Créer une version de plan (partie 168, dépliée dans la timeline) et une version
   de séquence (partie 59). Écarter un plan (partie 115), en supprimer un puis le reprendre de la corbeille (partie 104).
   Il faut retrouver chaque dessin, et l'export ne doit sortir que la version choisie. V1.md signale deux pertes sans
   message : un plan sorti de sa séquence, et le dernier plan supprimé d'une séquence. Les vérifier en particulier.
7. **Les calques dans la timeline resserrée** (partie 192, la nouvelle disposition d'origine). Créer, renommer et ranger
   des calques, régler opacité, verrou et visibilité, poser des clés. Clic droit > Apparence : il doit aussi s'ouvrir sur
   un calque replié. Sources : l. 16815, 16906, 16976, 17042.
8. **La colonne de panneaux en pile** (194 à 196, qui remplacent toute la colonne de droite). Ouvrir, replier, glisser et
   fermer des panneaux, et « + Panneau ». En changeant d'espace, la disposition de chaque espace doit revenir. Au premier
   clic dans les Préférences, le clic ne doit pas être perdu (JOURNAL l. 99). Sources : l. 17396, 17517, 17617.
9. **La structure du film.** Nouveau plan, dupliquer, couper (F5), réunir, supprimer, dupliquer la séquence (191).
   La numérotation verrouillée (parties 16 et 86) ne doit jamais redonner un numéro. Ces gestes ne doivent pas planter
   pendant qu'un autre écran montre l'ancien plan : ce plantage est connu par motif et « semble traité » (ETAT l. 176,
   V1 l. 83). Dupliquer la séquence ne copie pas le son, et la suite du film se décale (191.4).
10. **La caméra du plan.** Poser deux clés, les adoucir, lire : le mouvement doit être fluide et se retrouver dans la
    vignette et dans l'export. Poser un préréglage de caméra depuis la rangée Préréglages du Storyboard (170 à 172, 178).
    Sources : parties 47, 51, 117, 129, 170-172.
11. **L'origine des calques** (partie 182, format 61). Elle change le pivot de toutes les rotations et mises à l'échelle.
    Avec Transformer, tourner un calque sans toucher à son origine : il doit tourner autour du même point qu'avant. Puis
    déplacer l'origine : le dessin ne doit pas bouger. Source : l. 15553.
12. **La lecture avec le son.** Importer un WAV, lire le montage : pas de décalage, pas de saccade. Régler la durée d'un plan
    à la poignée ; avec Cmd, les clés doivent suivre (partie 123). Lecture J/K/L (partie 104), plage de lecture (partie 188).
    Sources : ETAT §5.8 ; parties 104, 123, 188.
13. **Les sorties d'un storyboard.** PDF du storyboard (en-tête, pages n/N, 2/4/6/8 cases), planche PDF, MP4 et images par
    plan (84, 91, 105). Ouvrir chaque fichier pour vérifier les accents : les PDF n'embarquent aucune police sur Mac
    (pdffont.rs). Retrouver les fichiers sans « Ouvrir le dossier ». Sources : parties 84, 91, 105, 109 ; ETAT §5.9.
14. **Les exports pour le montage** (EDL, FCPXML, OTIO ; parties 17 et 84). Personne ne les a ouverts dans un logiciel de
    montage. Les importer dans DaVinci Resolve ou Final Cut : les plans doivent être dans l'ordre, aux bonnes durées, avec
    le son calé. Sources : V1.md l. 107 ; AUDIT-2026-10-30 H3.
15. **La relecture et les envois.** « Marquer l'envoi », modifier un plan : la pastille « modifié depuis l'envoi » doit
    apparaître. Exporter la relecture HTML (partie 22), y écrire une note dans un navigateur, la réimporter : la note doit
    arriver sur le bon plan. Faire aussi le journal des différences (partie 92). Sources : parties 15, 22, 88, 92.

---


## Partie 1 — Fonctions fusionnées jamais testées à l'écran (180 entrées)

Des plus récentes aux plus anciennes. Pour chaque entrée : la source, le statut (jamais vu ou vu en partie), puis les gestes à faire et le résultat attendu. Faire les essais sur une copie de projet.

### Parties 189 à 201 (lot 6)

#### Partie 201 — Finitions 3D (2026-11-20, 23cb4e4)
- Source : INTERFACE 201.1-201.5, l. 18261-18421 ; JOURNAL l. 8-14
- Statut : jamais vu, sauf le « ◆ » (201.6, voir C). Le JOURNAL dit « pas de commit », mais le commit 23cb4e4 existe (voir D).
- Manipulations :
  1. Onglet 3D, séquence de plusieurs plans. Sur la timeline de la séquence (en bas), Cmd + molette doit zoomer autour de la tête de
     lecture, la molette doit faire défiler, Option+1 et Option+2 aussi zoomer ; la règle doit passer des secondes aux images. La
     réglette et « Ajuster » doivent être en tête de bande, avec « Timeline de la séquence » et « Banque 3D » toujours à gauche. Zoomer
     fort puis lire : la bande doit tourner la page quand la tête de lecture atteint le bord.
  2. En haut de Scène 3D, ouvrir « Séquence N ▾ » et cliquer une autre séquence : on doit aller à son premier plan.
  3. Clic droit sur un objet de Scène 3D > « Déplacer vers la séquence ▸ » : l'objet doit partir, et Cmd+Z le ramener. Glisser une ligne
     d'objet : la liste doit s'ouvrir et la séquence sous le pointeur s'éclairer ; lâcher doit déplacer, Option + lâcher copier. Un
     objet coloré et animé doit arriver avec sa couleur et ses clés.
  4. Dans la vue, choisir un objet, puis une carte, puis la caméra : l'étiquette en haut à gauche (« Décor : … », « Carte : … »)
     doit prendre la couleur de ce qui est choisi, et le carré du manipulateur aussi. Sans rien choisir, le survol doit afficher
     « Un clic prend : … ». Même chose dans le Storyboard, vue Espace 3D à côté puis en plein écran.
  5. Tirer la caméra d'un plan haut dans la vue, en plongée, puis « + » (Nouveau calque) de la timeline : le calque doit être face à
     la caméra, au milieu du cadre, et un trait doit se voir dans le plan. Sur un plan dont la caméra n'a pas bougé, on doit obtenir
     le calque de toujours. Préférences > Caméra, décocher « Placer un nouveau calque devant la caméra » : on doit retrouver l'ancien
     comportement.
  6. Une lampe rouge près d'un personnage (carte avec « Reçoit les ombres » coché) : l'image du plan et l'export doivent teinter le
     personnage, mais pas la vue Espace 3D (limite). Décocher « Reçoit les ombres » : la teinte doit disparaître.

#### Partie 200 — Mise en scène 3D : lampes, objets animés, caché dans le plan, nouveau plan depuis la vue (2026-11-20, e016b52)
- Source : INTERFACE 200.1-200.5, l. 18046-18195 ; JOURNAL l. 15-24
- Statut : vu en partie. Le coordinateur a vu les textes « ◆ Clé » (en case vide, 201.6), donc la ligne de tête de la timeline et
  Propriétés > Objet > Animation. Anthony a vu le plan gris (200.7, en C). Les lampes, les clés en lecture, « Caché dans le plan » et
  « Nouveau plan depuis cette vue » n'ont jamais été vus.
- Manipulations :
  1. Onglet 3D, cliquer l'ampoule « Ajouter une lampe » dans la barre de gauche : une marque, son nom et un pointillé jusqu'au sol
     doivent apparaître, et Propriétés > Lumières passer devant. Régler Rouge, intensité 2, portée 3 m : le décor proche doit rougir,
     le décor lointain rester tel quel. En Argile puis en Couleurs, la lampe doit éclairer encore. Cmd+Z doit défaire chaque réglage.
  2. Tirer la marque de la lampe, puis outil Déplacer et ses flèches ; Tourner doit être refusé avec un message. Ajouter huit lampes :
     l'outil doit se griser.
  3. Choisir un objet, cliquer « ◆ Clé sur … » (en bas, au bout de la ligne de tête) à l'image 1, aller plus loin dans la séquence et
     déplacer l'objet : un second losange doit apparaître. À la lecture, l'objet doit glisser en démarrant et en s'arrêtant doucement,
     aussi dans le Storyboard et le Montage. Sur un losange : un clic doit y mener, un clic droit l'enlever, un glisser le déplacer.
  4. Propriétés > Objet > « Dans les plans », cocher « Caché dans le plan … » : le dessin et la vignette de ce plan ne doivent plus
     montrer l'objet, le plan suivant si. La perspective de l'onglet doit le montrer en pointillés, « Caméra du plan » non.
  5. Cadrer la perspective, puis le clap « Nouveau plan depuis cette vue » : un plan de 2 s doit arriver en fin de séquence, filmant ce
     que montrait la vue. Cmd+Z. Recommencer depuis « Dessus » : il doit prendre la perspective.
  6. Enregistrer, fermer, rouvrir : les lampes, les clés et les objets cachés doivent être là.

#### Partie 199 — Couleurs d'identification, aspects Matières / Couleurs / Argile (2026-11-20, 3446588)
- Source : INTERFACE 199.1-199.4, l. 17942-18044 ; JOURNAL l. 25-32
- Statut : jamais vu (199.3 ; les 55 % « à plat », le gris a3a3a3 et l'argile sont à juger à l'œil).
- Manipulations :
  1. Scène 3D : clic sur la puce d'un objet (le carré devant le cadenas), choisir Bleu : la puce doit devenir bleue. Cmd+Z doit la
     vider, Cmd+Maj+Z la remettre.
  2. Maj + clic sur deux objets, puis Propriétés > Objet > Couleur d'identification > Vert : les deux doivent changer en une étape.
  3. Barre de la vue de l'onglet 3D : « Couleurs » puis « Argile » ; les cartes ne doivent jamais être teintes. Dans le Storyboard, la
     vue Espace 3D et la vue à côté doivent avoir le même aspect, retenu après relance.
  4. Propriétés > Décor > « Dans les plans » > Couleurs : le dessin, les vignettes et l'export doivent être en couleurs, avec les ombres
     et les bords doux.
  5. Storyboard, barre au-dessus du dessin : « Décor » Matières · Couleurs · Argile (seulement si la séquence a un décor). Argile, puis
     Cmd+Z.
  6. Cacher un objet (œil) et en verrouiller un autre (cadenas), enregistrer et rouvrir : les deux états doivent être gardés. Copier
     un objet coloré vers une autre séquence, puis dupliquer la séquence : la couleur doit suivre.

#### Partie 198 — L'onglet 3D (2026-11-20, d9c6788)
- Source : INTERFACE 198.1-198.4, l. 17778-17899 ; JOURNAL l. 33-40
- Statut : vu en partie. Le coordinateur et Anthony ont ouvert l'onglet, vu la bande du bas et ses deux boutons (ajoutés après
  l'écran), cliqué Tree oak dans la Banque 3D et vu l'onglet Monde (198.5). Le reste (boussole, 1/2/4 vues, Scène 3D, Propriétés qui
  suit, timeline de la séquence, repos) n'a jamais été confirmé.
- Manipulations :
  1. Onglet « 3D » entre Storyboard et Montage : la vue en grand, avec à gauche seulement les outils de la vue (une touche d'outil de
     dessin ne doit pas dessiner sur une carte).
  2. Barre : Perspective, Dessus, Face, Côté, Caméra du plan, en 1 vue puis en 2 et 4. Les bascules Ombres, Grille, Cartes, Caméras
     (nom du plan sur chaque caméra), Lumières, puis Recadrer.
  3. Boussole en haut à droite de la perspective : cliquer X, Y, puis le bout pâle de Z. La vue doit tourner en un quart de seconde.
  4. Scène 3D : clic sur un objet (Propriétés doit passer à Objet), sur la caméra d'un autre plan (on doit aller au plan, Propriétés
     doit passer à Caméra), sur une carte. Taper « maison » dans la recherche. Replier « Caméras ».
  5. Timeline de la séquence : cliquer un plan, glisser sur la règle : la vue doit suivre. Glisser un modèle de la Banque 3D sur la vue :
     il doit se poser au sol sous le pointeur.
  6. Revenir au Storyboard : même plan, même calque, la vue Espace 3D refermée si elle l'était. Au repos dans l'onglet 3D, le
     processeur doit être inactif (Moniteur d'activité).

#### Partie 197 — Le panneau Scène du Storyboard (2026-11-20, 25c1084)
- Source : INTERFACE 197.1-197.4, l. 17663-17776 ; JOURNAL l. 41-47
- Statut : jamais vu (197.3).
- Manipulations (garder une copie de reglages.toml ; dans le Storyboard, Affichage > Espaces de travail > « Rétablir la disposition »,
  ou « + Panneau » > Scène) :
  1. En haut de la colonne : Scène, Couleur, Propriétés. « Depuis : Plan » : replier un dossier par son triangle doit le replier aussi
     dans la timeline.
  2. Clic sur un calque : il doit devenir courant et se déplier en ligne complète. Changer le mode, l'opacité et la pelure depuis le
     panneau, vérifier dans la timeline. L'œil et le cadenas doivent se suivre dans les deux sens.
  3. « Depuis : Séquence » : déplier un autre plan et cliquer un de ses calques : on doit aller à ce plan. L'œil d'un calque d'un autre
     plan ne doit pas faire changer de plan.
  4. Clic droit sur un calque : le menu de la timeline, puis « Dupliquer ». Glisser un calque entre deux autres, puis sur un dossier.
     Double-clic sur le nom pour renommer.
  5. Animation (double-clic sur un plan), « + Panneau » > Scène : la caméra doit être sous les calques. Sur un gros projet,
     « Depuis : Film » tout déplié, le panneau doit rester fluide.
  6. Vérifier la ligne complète du calque dans 290 points de large, et le fil d'Ariane quand il passe à la ligne.

#### Partie 196 — Deuxième tour des retouches (2026-11-20, 62620e6)
- Source : INTERFACE 196.1-196.5, l. 17542-17661 ; JOURNAL l. 48-55
- Statut : vu en partie. Le coordinateur a vu la pile Couleur / Propriétés / Préréglages après 196 (196.5), donc probablement
  l'aperçu réduit des Préréglages. Le correctif 196.5 et les points 3 à 6 de 196.1 n'ont jamais été vus (196.3 et 196.5 : « Pas
  regardé à l'écran »).
- Manipulations :
  1. Préréglages : descendre la liste à la molette. L'aperçu (deux tiers de la taille, centré), les onglets et la phrase doivent
     rester en haut.
  2. Storyboard : la ligne Séquence doit faire 10 points, avec le mot seul et des noms de séquences lisibles. Clic droit dessus :
     nouvelle séquence, renommer, versions, dupliquer, nouvelle séquence avec les plans choisis, couper ici.
  3. Ligne Caméra : seulement ⊕ et le mot. Clic droit : « Clé de caméra ici », « Préréglages de caméra… », « Remettre la caméra à zéro »,
     convertir, copier, coller.
  4. Beaucoup de calques et une bande basse, faire défiler : la règle, le compteur, la ligne Séquence et la ligne Plans doivent rester
     en haut. Cliquer un plan et « Nouveau plan » là-haut.
  5. Barre au-dessus du dessin : vue caméra, puis « 2D », puis « 3D ». Les bulles « Vue du plateau » et « Espace 3D », et la case
     allumée doit suivre.
  6. Fenêtre de 1384 × 868 à 90 %, Couleur, Propriétés et Préréglages ouverts : la roue doit être entière, et la molette sur la colonne
     doit descendre jusqu'au bas de Préréglages. Couleur repliée : tout doit tenir.

#### Partie 195 — Retouches de la partie 194 (2026-11-20, 62620e6)
- Source : INTERFACE 195.1-195.4, l. 17423-17540 ; JOURNAL l. 56-63
- Statut : vu en partie. Anthony a regardé ces retouches avant de demander la partie 196 (Préréglages, ligne Séquence et Caméra,
  barre de la vue). Le point 12 a été remplacé par 196.5. Aucune source ne confirme la taille d'origine 90 % / 16 pt, le zoom et
  l'inclinaison en curseurs, l'icône du passe-partout, les curseurs fins, le compteur Ctrl + clic, l'aide hors du menu, ni le
  Ctrl + clic qui choisit un objet.
- Manipulations :
  1. Sans réglage de taille, l'interface doit être à 90 %. Préférences > Apparence > Tailles : « 16 pt (d'origine) », et 13 pt doit
     rapetisser à la fois les barres du dessin et la colonne de la timeline. « Par défaut » doit remettre 90 % et 16.
  2. Sous le dessin : la nouvelle icône du passe-partout, sa flèche (l'opacité), le curseur de zoom (double-clic, taper 250),
     l'inclinaison, « Remettre la vue droite ».
  3. Cmd + clic sur le compteur de la timeline : « image 63 / 528 », puis retour ; le choix doit tenir après relance. Dans l'Animation :
     « / 23 » doit devenir le time code.
  4. Prendre un outil jamais pris : aucune vidéo ne doit s'ouvrir ; Aide > Registre des explications.
  5. Pinceau sur un calque vectoriel : Cmd + clic sur un trait doit le choisir (même au stylet), Cmd + glisser faire un lasso, Cmd + clic
     dans le vide désélectionner. Cmd + clic sur un trait d'un autre calque doit en faire le calque courant. Option + clic doit choisir
     le calque.
  6. Couleur : plus de « Avant · maintenant ». Réglages du Pinceau : « Pinceau » et « calque vectoriel » séparés.

#### Partie 194 — La colonne en pile de panneaux (2026-11-20, 62620e6)
- Source : INTERFACE 194.1-194.5, l. 17287-17421 ; JOURNAL l. 64-75
- Statut : vu en partie. Anthony a annoté des captures, puis essayé l'application (195 : « puis l'a essayée »), et le coordinateur a
  vu la pile (196.5). Rien ne dit que les gestes de la pile (poignée ⠿, ▲ ▼, ↗, bord du bas, fenêtre à part, flottant, piles par
  espace, conversion v9) ont été essayés.
- Manipulations (garder une copie de reglages.toml : la disposition passe en version 10) :
  1. Dans Propriétés, survoler les onglets d'icônes. ↗ doit sortir l'onglet devant en panneau, et son ✕ le remettre en onglet.
  2. ▲ ▼ sur la Couleur ; glisser sa poignée ⠿ vers le bas, une ligne d'accent doit montrer où elle tombe. Clic sur un titre pour
     replier, tirer le bord du bas, puis Affichage > Panneaux > Réajuster.
  3. « + Panneau » > Bibliothèque : elle doit arriver en bas, et les panneaux déjà là être grisés « (déjà là) ».
  4. Clic droit sur un en-tête > Dans une fenêtre à part, puis fermer cette fenêtre. Panneau flottant, puis clic droit sur sa poignée
     > Dans la colonne.
  5. Ouvrir la vue Espace 3D : l'onglet Espace 3D doit venir devant dans Propriétés et repartir à la fermeture. Cmd+K « Historique ».
  6. Changer la pile du Montage, revenir au Storyboard, quitter et relancer : chaque espace doit garder la sienne ; « Rétablir la
     disposition » doit ramener celle d'origine. Avec un reglages.toml d'avant (version 9), aucun panneau ne doit se perdre.

#### Partie 193 — Bords doux entre décor 3D et cartes (2026-11-20, 0335c3c, fusion 831e997)
- Source : INTERFACE 193.1-193.6, l. 17093-17285 ; JOURNAL l. 76-90
- Statut : jamais vu.
- Manipulations :
  1. Un arbre de la banque devant un personnage et un mur tourné. Zoom à 400 % sur l'image du plan, puis sur une image exportée : il
     ne doit plus y avoir d'escaliers le long du tronc ni du haut du mur.
  2. Une carte de sol inclinée qui entre dans une boîte : la ligne d'entrée doit être douce, aussi dans la vue Espace 3D en tournant
     autour.
  3. Animer la profondeur du personnage pour qu'il passe derrière l'arbre, lire avec J/K/L : le bord ne doit pas scintiller.
  4. « Toujours devant le décor 3D » et un calque accroché à la caméra ne doivent jamais être cachés, même au bord. Les pieds d'une
     carte aimantée doivent rester comme avant.
  5. Le panneau du décor doit dire les deux budgets (« … avec les bords doux des cartes »), et au premier lancement le budget doit se
     remesurer une fois. Cocher « Toujours devant » sur toutes les cartes, puis en décocher une : le décor ne doit pas clignoter.
  (Mesures à refaire sur le PC d'Anthony : hors audit Mac.)

#### Partie 192 — La timeline resserrée (2026-11-19/20, b8ceddb à 314248f, fusion f24dd5e puis 9b63079)
- Source : INTERFACE 192.4, 192.9, 192.12, 192.15, 192.16, 192.17, l. 16815-17090 ; JOURNAL l. 91-99
- Statut : vu en partie. Anthony a réagi sur des captures du banc (192.5, 192.10, 192.13, 192.16, 192.17), puis a vu la timeline dans
  l'application d'essai lors des parties 195 et 196. Le JOURNAL (l. 99) note toujours « En attente d'Anthony : son essai à l'écran
  (checklist 192.x) ». Plusieurs points ont été remplacés : icônes 13 pt devenues 16 (195), interface 100 % devenue 90 % (195), icônes
  de la ligne Séquence (192.11) et de la ligne Caméra enlevées en 196, Séquence à 10 pt (196), calque actif seul déplié remplacé par
  « tous épais » (192.13), indicateurs dossier, +/− et punaise retirés (192.16). Il reste à voir :
- Manipulations :
  1. Storyboard : tous les calques sur deux rangées. Changer de calque actif ne doit rien déplacer, l'actif doit seulement être
     surligné. Sur un calque non actif, régler mode et opacité ; avec deux calques choisis (Cmd + clic), les deux doivent changer.
  2. ⋯ de la ligne « Calques » > « Replier les calques » : tous les calques doivent devenir fins, et décocher doit les remettre. La même
     case est dans Préférences > Montage > Pistes du montage.
  3. Ligne « ⊖ Calques » : +, dossier, effet, dupliquer, supprimer, ⋯ collés à droite. Clic droit dans le vide : nouveau calque,
     coller. Colonne au plus étroit (286 pt) : rien ne doit déborder, les boutons doivent passer sous « ⋯ ».
  4. Préférences > Apparence > Tailles > Lignes de la timeline : les listes déroulantes doivent être alignées. Plans 40, Calques 36 :
     l'effet doit être immédiat. La pastille de couleur d'une ligne, puis « Par défaut ».
  5. Clic droit sur un calque > Apparence : « Groupe (couleur d'étiquette) », « Pochoir négatif » (grisé sans pochoir), « Verrouiller
     la position » ; Cmd+Z.
  6. Animation : tous les calques sur leur ligne complète, dossiers fins, barre sur deux lignes, ⊖/⊕ et Maj+Option+C (« Replier
     les calques »). Montage : rien ne doit avoir changé (pistes de 66 et 17).

#### Partie 191 — Dupliquer la séquence (2026-11-19, 41fa82f)
- Source : INTERFACE 191.1-191.5, l. 16583-16714 ; JOURNAL l. 103-114
- Statut : jamais vu. Les emplacements du menu ont bougé : la ligne Séquence n'a plus de « ⋯ » depuis 196. La commande est au clic
  droit de la ligne Séquence, au « ⋯ » de la ligne Plans (196.1), au clic droit de la bande du montage, dans la planche, le menu
  Storyboard et Cmd+K.
- Manipulations :
  1. Une séquence « Poursuite » (2 ou 3 plans dessinés, caméra animée, V2 sur un plan, commentaire, fondu enchaîné interne, fondu au
     noir en sortie, décor 3D de 2 objets). Clic droit sur la ligne Séquence > « Dupliquer la séquence » : « Poursuite (copie) » doit
     arriver juste après et devenir courante.
  2. Lire le film : la coupe entre l'originale et la copie doit être franche, et le fondu au noir à la fin de la copie.
  3. Dans la copie : un trait, l'opacité d'un calque, une clé de caméra, un objet du décor déplacé ; l'originale ne doit pas bouger, et
     inversement.
  4. Cmd+Z : la copie entière doit partir ; Cmd+Maj+Z doit la rendre.
  5. Planche des Vignettes : clic droit sur le titre de la séquence > Dupliquer (« (copie 2) »), puis depuis la liste et depuis le menu
     d'une case. Sur la V2 de la copie, passer en V1 : l'originale doit rester en V2. Avec des versions de séquence A/B : passer la
     copie en B ne doit pas toucher l'originale.
  6. Enregistrer et rouvrir : les deux séquences doivent être là, avec un seul fichier par modèle dans decors/.

#### Partie 190 — Copier les objets du décor 3D (2026-11-19, b27db77)
- Source : INTERFACE 190.1-190.5, l. 16441-16581 ; JOURNAL l. 116-127
- Statut : jamais vu. « Le décor du panneau Espace 3D » est maintenant un onglet de Propriétés (194), et dans l'onglet 3D c'est
  Propriétés > Décor (198). Le geste « Envoyer vers » est doublé par « Déplacer vers » de Scène 3D (201).
- Manipulations :
  1. Vue Espace 3D : clic sur une dalle, Cmd+C puis Cmd+V : une seconde dalle doit apparaître à côté, choisie. Cmd+V encore : plus loin.
     Cmd+Z deux fois.
  2. Maj + clic sur le banc posé sur la dalle, Cmd+C. Clic sur un plan d'une autre séquence (montage), clic dans le vide de la vue,
     Cmd+V : les deux objets doivent arriver à la même place, le banc sur la dalle, et un personnage aimanté doit monter dessus. Cmd+Z.
  3. Liste des objets du décor : Cmd + clic sur deux objets, Couper (ce qui reposait dessus doit redescendre), puis Coller.
  4. « Envoyer vers la séquence… » sur l'arbre, « Copier vers la séquence… » sur le banc, avec Cmd+Z à chaque fois.
  5. Ailleurs, Cmd+C / Cmd+V ne doivent pas changer de sens : un trait sur le dessin, un calque, un plan au montage, des clés dans la
     timeline, même avec un objet resté choisi dans la vue.
  6. Deux projets ouverts : copier un arbre dans l'un, le coller dans l'autre, enregistrer, rouvrir (le fichier doit être dans
     decors/). « Couper la séquence ici » sur une séquence qui a un décor : le décor doit être copié et indépendant.

#### Partie 189 — Ombres des cartes dans la vue Espace 3D (2026-11-19, 4481e66)
- Source : INTERFACE 189.1, 189.6, l. 16304-16439 ; JOURNAL l. 129-140 ; ESPACE3D §19
- Statut : jamais vu.
- Manipulations :
  1. Décor avec dalle et arbre, ombres de l'aspect actives, un personnage « Aimanté au sol ». Vue Espace 3D plein écran : son ombre
     doit être sur la dalle et rester au même endroit du sol en tournant autour (clic droit + glisser).
  2. Propriétés > Espace 3D, régler le soleil (autour, hauteur) : l'ombre doit tourner dans la vue et dans le dessin.
  3. Placer le personnage à l'ombre de l'arbre : il doit s'assombrir dans la vue. Deux personnages, l'un devant le soleil : il doit
     ombrer l'autre.
  4. Décocher « Porte une ombre », puis « Reçoit les ombres » : l'ombre doit disparaître de la vue, la carte ne plus s'assombrir.
     Cmd+Z.
  5. 4 vues, puis vue à côté du dessin : l'ombre doit se voir dans la vue de dessus et en perspective. Lecture à 24 i/s, vue à côté
     ouverte : pas de saccade (superposition Performance).
  6. Décocher « Ombres » dans l'aspect du décor : il ne doit plus y avoir aucune ombre, ni dans la vue ni dans le dessin.

### Parties 170 à 188 (lot 5)

#### Partie 188 — Plage de lecture du film (2026-11-19, 04599ac, fusion c9c24f6)
- Source : INTERFACE 188.1/188.4, l. 16255-16301 ; JOURNAL l. 261-266.
- Statut : jamais vu par Anthony. Selon 188.3, la palette, le crochet et le voile ont été vus « à l'écran sur le Mac » par la
  session, pas par un humain.
- Manipulations :
  1. Montage, clic droit sur la règle au milieu du plan 2 > « Début de la plage de lecture ici » ; au milieu du plan 5 > « Fin de la
     plage de lecture ici » → un crochet à chaque bout, le film hors de la plage assombri.
  2. Entrée (lecture) → la lecture part du début de la plage, traverse les plans et s'arrête à la fin. Avec la boucle allumée, elle
     reprend au début. J (en arrière) s'arrête au début de la plage.
  3. Allonger le plan 1 → la plage suit son contenu.
  4. Choisir trois plans (Maj+clic), clic droit sur la règle > « Plage de lecture : les 3 plans choisis ». Puis « Enlever la plage
     de lecture » → tout le film est lu.
  5. Poser une fin avant le début → le début est retiré et la barre d'état le dit. Cmd+K « plage » → les quatre commandes.

#### Partie 185 — Ombres des cartes et du décor 3D (2026-11-19, c00c84c)
- Source : INTERFACE 185.1/185.6, l. 15981-16142 ; ESPACE3D 18 ; JOURNAL l. 148-162.
- Statut : jamais vu. En partie prolongé par la 189 (les mêmes ombres dans la vue Espace 3D). Il reste à voir : l'image du plan,
  les exports et les deux cases.
- Manipulations :
  1. Une séquence avec une dalle et un arbre (Bibliothèque > 3D). Un personnage en carte, « Aimanté au sol » sur la dalle → son
     ombre part de ses pieds. Tourner le soleil (panneau Espace 3D, ou Propriétés > Lumières dans l'onglet 3D) → l'ombre tourne.
  2. Faire passer le personnage à l'ombre de l'arbre (deux clés de position), lire à 24 i/s puis J/K/L → il s'assombrit et
     ressort au soleil, sans saccade.
  3. Sur un fond peint : menu du calque > Plus > Réglages avancés, décocher « Porte une ombre » et « Reçoit les ombres » → plus
     d'ombre portée ni reçue ; Cmd+Z.
  4. Un titre accroché à la caméra → les deux cases sont grisées, avec la raison. Décocher « Ombres » dans l'aspect du décor → plus
     aucune ombre.
  5. Exporter une image et un film → les mêmes ombres qu'à l'écran. Enregistrer, rouvrir → les cases décochées le restent.

#### Partie 184 — Poignées dans la vue pour les objets du décor et l'origine (2026-11-19, 9c7d444)
- Source : INTERFACE 184.1/184.5, l. 15839-15978 ; ESPACE3D 17 ; JOURNAL l. 164-176.
- Statut : jamais vu. Repris par l'onglet 3D (198 : « Modifier l'origine » dans la barre de gauche ; 201 : l'étiquette « qui a la
  main »), mais le geste lui-même n'a été vu nulle part.
- Manipulations (vue Espace 3D du Storyboard ou onglet 3D) :
  1. Outil Choisir, clic sur un banc → sa boîte s'allume et il est choisi dans la liste du décor. Un clic entre les branches d'un
     arbre prend ce qui est derrière.
  2. Maj+clic sur une carte en plus → les deux sont choisis. Avec une carte devant le banc, le clic prend la carte ; derrière, il
     prend le banc.
  3. Outil Déplacer : la flèche rouge fait glisser le banc ; la flèche verte est grise s'il est aimanté, et la barre d'état dit
     pourquoi. Une carte aimantée posée sur une dalle suit la dalle ; un Cmd+Z ramène les deux.
  4. Outil Tourner : seul l'anneau vert agit (les autres sont gris). Mettre à l'échelle : un cube agrandit tout l'objet.
  5. « Modifier l'origine » (le viseur de la colonne d'outils) sur le banc : la flèche verte monte l'origine sans bouger le banc ;
     avec Maj, elle s'accroche au centre ou aux coins de la boîte. Sur une carte, l'origine glisse dans son plan et le dessin ne
     bouge pas.
  6. ⌫ sur un objet choisi dans la vue → rien, c'est voulu (« Enlever » du panneau).

#### Partie 183 — Profondeur partagée décor / cartes (2026-11-19, 343a061)
- Source : INTERFACE 183.1/183.6, l. 15665-15836 ; ESPACE3D 16 ; JOURNAL l. 178-191.
- Statut : jamais vu (le bord dur a été adouci par la 193, voir C).
- Manipulations :
  1. Un personnage en carte entre un arbre et un banc de la banque 3D → derrière l'arbre, le tronc le cache ; devant le banc, il le
     couvre. Vérifier que le dessin (pas seulement l'export) montre le décor sous les calques.
  2. Animer sa profondeur (deux clés) → il passe derrière puis devant ; lecture et J/K/L sans saccade.
  3. Aimanté sur une dalle → aucun bord rogné ni scintillement aux pieds quand la caméra bouge.
  4. Une carte inclinée (un sol, un mur) qui traverse un objet → la partie derrière est cachée.
  5. Menu du calque > Plus > Réglages avancés > « Toujours devant le décor 3D » → il repasse devant ; Cmd+Z. Un titre en
     incrustation et un calque accroché ne sont jamais cachés.
  6. Pipette sur le décor → elle prend sa couleur ; le pot de peinture l'ignore.

#### Partie 182 — Origine d'un calque, son vrai pivot ; aimant des cartes (2026-11-19, f6a7a46, format 61)
- Source : INTERFACE 182.1/182.4, l. 15553-15662 ; ESPACE3D 15 ; JOURNAL l. 193-200.
- Statut : jamais vu. C'est un changement de fond, qui touche tous les calques et toutes les trajectoires.
- Manipulations :
  1. Outil Transformer sur un personnage : le cercle à croix est au centre de la feuille. Le tirer aux pieds (Maj : milieu du bas)
     → rien ne bouge. Tourner par la poignée ronde → il tourne autour des pieds. Cmd+Z → l'origine revient.
  2. Deux clés d'angle sur une tête, l'origine au cou → entre les clés, le cou reste fixe.
  3. Menu du calque > Plus > Origine > « En bas au centre », puis « Au centre de la feuille » → le dessin ne bouge jamais.
  4. Nouvelle trajectoire → les pieds sont sur le chemin ; avec « Le dessin tourne avec le chemin », il tourne autour des pieds.
     Déplacer ensuite l'origine → le chemin suit.
  5. Espace 3D, panneau de la carte, « Aimanté au sol » → les pieds touchent le sol à chaque clé et le Y est grisé. Décocher → rien
     ne bouge ; enregistrer et rouvrir → toujours là.
  6. Un projet d'avant → les mêmes images (le projet passe au format 61, qu'une version précédente refuse).

#### Partie 181 — Banque 3D dans la Bibliothèque, décor fait de plusieurs objets, origine et aimant des objets (2026-11-19, ae7c5b3)
- Source : INTERFACE 181.1/181.5, l. 15385-15551 ; ESPACE3D 14 ; JOURNAL l. 202-212.
- Statut : vu en partie. La 198.5 (coordinateur et Anthony) a vu un clic sur « Tree oak » dans la Banque 3D de l'onglet 3D (fenêtre
  « lourd » par erreur, corrigée) et les aperçus de la banque. Il reste à voir l'onglet 3D de la Bibliothèque, la liste d'objets,
  l'origine, l'aimant, le partage de fichier, l'ancien projet et le budget.
- Manipulations :
  1. Storyboard, « + Panneau » > Bibliothèque > onglet « 3D » → cinq familles, recherche (« tree », « road »), bulle. Cliquer un
     arbre trois fois puis une route → quatre objets ; Cmd+Z les retire un par un.
  2. Glisser une maison sur la vue Espace 3D → elle se pose au sol sous le pointeur. Glissée sur le dessin → elle se pose devant la
     caméra.
  3. Panneau Espace 3D (ou Propriétés > Objet / Décor dans l'onglet 3D) : renommer, Dupliquer, Enlever ; menu ⌖ de l'Origine,
     « Au centre de la boîte », puis tourner → l'objet tourne sur lui-même. Changer l'origine ne le déplace jamais.
  4. Aimant : une dalle, puis un banc glissé dessus → le banc monte dessus. Déplacer la dalle → le banc redescend. Un feu de camp
     sous un arbre reste au sol. Décocher « Aimanté » sur le banc → il ne bouge pas, et c'est gardé après réouverture.
  5. Enregistrer → `decors/` n'a qu'un fichier pour trois arbres pareils. À propos → le crédit Kenney, qui ouvre le site.
  6. Poser des objets jusqu'au budget → la fenêtre « Ce décor est lourd » s'ouvre, puis le refus quand la séquence est pleine.
     Un projet de la partie 180 (le fichier d'Anthony) n'est pas testable sans lui.

#### Partie 180 — Décor 3D d'une séquence (glTF) (2026-11-19, 319e817)
- Source : INTERFACE 180.1/180.6, l. 15174-15383 ; ESPACE3D 14 ; JOURNAL l. 214-223.
- Statut : vu en partie. D'après la 198.5 et la 200.7 : le message de l'étalonnage, un décor dans l'image du plan et dans la vue à
  côté (projet d'Anthony, « Plan 5 »). Il reste à voir l'import par Fichier, l'aspect, l'allègement et la simplification, l'export,
  Préférences > Performance et le secours sans carte graphique.
- Manipulations :
  1. Fichier > Importer > « Objet 3D pour le décor de la séquence (glTF, .glb)… » (libellé de la 181) avec un `.glb` low poly → la
     barre d'état dit sa taille ; l'objet est sur le sol, derrière la feuille. Les plans de la séquence le montrent, ceux d'une autre
     séquence non.
  2. Bouger la caméra (déplacement, avancée, panoramique, zoom) → le décor défile avec la parallaxe des cartes ; lecture, J/K/L sans
     ralenti.
  3. Panneau Espace 3D : cocher et décocher Facettes, Ciel, Brume, Ombres, tourner le soleil → un Cmd+Z défait chaque retouche.
     Puis « Remettre au sol », « Enlever », Cmd+Z.
  4. Exporter une image et un film → le décor y est, ombres comprises. Enregistrer et rouvrir → `decors3d.json` et `decors/`.
  5. Un fichier lourd (textures 8K, des millions de triangles) → « Ce décor est lourd » : l'avant → après se calcule ; « Simplifier,
     alléger et importer » ; le fichier d'origine n'a pas changé. Un fichier Draco est refusé avec un message clair.
  6. Préférences > Performance → « Décor 3D sur cet ordinateur : jusqu'à N triangles… » et « Remesurer ». (Le secours OpenGL,
     `PELURE_AFFICHAGE=opengl`, demande une variable d'environnement : facultatif.)

#### Partie 178 — Timeline du Storyboard (Séquence, Plans, Caméra, Calques) (2026-11-19, e613b0c, fusion ecaeeb5)
- Source : INTERFACE 178.1/178.4, l. 15010-15088 ; JOURNAL l. 233-247.
- Statut : vu en partie. Anthony a annoté des captures de cette timeline (192, 192.5, 192.13), puis l'a vue dans l'application
  d'essai (195, 196 : il a fait retirer les icônes des lignes Séquence et Caméra). Les hauteurs et les en-têtes ont été refaits par
  la 192 et la 196. Les fonctions de la 178 n'ont jamais été vues en usage (il disait avant ne pas pouvoir poser de préréglages dans
  le Storyboard).
- Manipulations :
  1. Poser une transition (Préréglages, onglet Montage, glisser sur une coupe) → le ⋈ sur la coupe. Un clic sur ⋈ ouvre son menu ;
     changer la sorte ; Cmd+Z. Un clic juste à côté reste au plan.
  2. ⊕ de la ligne Caméra → la rangée Préréglages. Glisser « Zoom avant » sur un plan, puis « Tremblement léger » par-dessus → deux
     rangs bas.
  3. Tirer un bord, puis le corps d'un bloc ; clic puis ⌫ ; clic droit > « Convertir les effets en clés » → des losanges sur la
     ligne Caméra.
  4. ⊖ Caméra → la ligne seule, avec un trait jaune sous les préréglages. Y glisser un préréglage → il se pose quand même.
  5. ⊖ Calques (ou Maj+Option+C dans le Storyboard) → « Calques · N » et les clés en petits traits. ⊕ → les pistes. Relancer FEANOR
     → les deux états sont gardés. Un calque à 50 % → « 50 % » écrit sur sa ligne.
  6. La plus petite fenêtre → rien ne déborde.

#### Partie 177 — Préréglages : trois transitions et six plans types de plus (2026-11-19, 7a9b2bb, fusion e859d56)
- Source : INTERFACE 177.1/177.4, l. 14932-15008 ; JOURNAL l. 267-286.
- Statut : jamais vu (fusionné « sans essai sur son écran », JOURNAL l. 268).
- Manipulations (Préréglages, onglet Montage, la liste des noms depuis la 195) :
  1. Glisser « Iris » sur une coupe ; clic droit sur la transition, cliquer dans le petit cadre sur un visage ; lire. Au début du
     film l'iris s'ouvre seulement, à la fin il se ferme seulement.
  2. Un volet et une poussée : clic droit > Sens « du haut », « du bas » ; lire. « Volet en horloge », « Zoom traversant » avec son
     centre.
  3. « Titre de séquence » entre deux séquences ; renommer la séquence → le carton suit. « Plan manquant » : allonger le plan → la
     durée écrite suit.
  4. « Ardoise d'en-tête » : remplir Fichier > Propriétés (titre, série, épisode) ; la version se change dans les réglages du
     calque.
  5. « Compte à rebours » (bip sur le 2), « Mire de couleur » (1 kHz), deux « Fin d'acte » (1 puis 2). Exporter une vidéo → les
     mêmes transitions, le bip et la mire dans le son.

#### Partie 176 — Préréglages, lot 4 : plans types (2026-11-18, ebbd313, fusion 23efd4c, format 60)
- Source : INTERFACE 176.1/176.4, l. 14856-14930 ; JOURNAL l. 290-317.
- Statut : jamais vu (fusionné sans essai à l'écran).
- Manipulations :
  1. Glisser « Titre du film » au début du montage → un plan noir, le titre en fondu ; le retoucher à l'outil Texte.
  2. Cliquer « Générique de fin » → il monte ; allonger le plan → il monte moins vite.
  3. Glisser « Lieu et date » sur un plan → le texte s'écrit lettre à lettre ; le changer dans les réglages du calque (« Lettre à
     lettre »).
  4. « Écran partagé en quatre » → dessiner dans chaque case. « Précédemment » après trois plans → leurs images sous le carton.
  5. « Logo studio » → le choix d'une image s'ouvre ; elle apparaît en douceur. Cmd+Z retire le plan neuf.

#### Partie 175 — Préréglages, lot 4 : transitions (2026-11-18, ebbd313, fusion 23efd4c)
- Source : INTERFACE 175.1, l. 14800-14854 ; checklist en 176.4 (points 1 à 4 et 11).
- Statut : jamais vu.
- Manipulations :
  1. Glisser « Poussée » sur une coupe du montage → la coupe s'éclaire, en rouge au bord du film ; lire.
  2. Clic droit sur une transition > « Fondu par une couleur », puis sa case Couleur ; Cmd+Z.
  3. « Flash de coupe » et « Raccord filé » sur deux coupes, lus en boucle ; « Volet par un objet » (le tronc).
  4. Cmd+K « Transition : poussée » → posée. Exporter une vidéo → la même image qu'à l'écran.

#### Partie 174 — Colonne des calques de la timeline (Animation) (2026-11-18, ad92268, fusion 9c422b9)
- Source : INTERFACE 174.1/174.4, l. 14693-14798 ; JOURNAL l. 248-257.
- Statut : en partie remplacé par la 192. L'en-tête des colonnes a disparu (192.6), le calque courant est passé sur 2 rangées puis
  tous les calques sont épais (192.13), Maj+Option+C bascule maintenant « Replier les calques ». Il reste, jamais vu : les colonnes
  de réglages sur chaque ligne (petits points), les icônes avant / après dans la bande, et la marge de 20 points avant l'image 0.
- Manipulations (Animation, double-clic sur un plan) :
  1. Sur une ligne qui n'est pas le calque courant, cliquer la pelure, le damier, le pochoir, l'aplat, la caméra, le cadenas, l'œil
     → le réglage change sur ce calque ; Cmd+Z.
  2. Cmd+clic sur deux calques, puis choisir « Produit » dans « Normal ▾ » → les deux ; un seul Cmd+Z.
  3. Dans la bande, l'icône grise avant ou après les images d'un calque ; clic → « Aucun / Boucle / Va-et-vient / Maintenir ».
     « Boucle » après → les images reviennent en pâle ; Cmd+Z.
  4. Un calque qui commence à l'image 0 → son icône est dans la marge avant la grille. Les prises du début et de la fin se tirent
     encore.
  5. Rétrécir la timeline au plus étroit → rien ne déborde.

#### Partie 173 — Piste du calque actif : « Normal ▾ », colonne réglable (2026-11-18, bbcc219, 8b4600a, c78000b)
- Source : INTERFACE 173.1/173.3/173.5, l. 14570-14672.
- Statut : en partie remplacé par la 192. La barre est sur une ligne (plus deux) ; deux rangées, puis tous les calques épais ;
  l'espacement de la 173.6 est remplacé par « collé à droite » ; le groupe, le pochoir et la punaise sont au clic droit > Apparence
  (192.16) ; le minimum de la colonne est passé de 250 à 286 points. Il reste, jamais vu : la liste du mode, la pelure au clic droit,
  la largeur de la colonne.
- Manipulations (Storyboard) :
  1. Choisir « Superposition » dans « Normal ▾ » → raccourci « Superpos… ▾ », rien ne passe sur « 100 % » ; la bulle donne le nom
     entier.
  2. Cmd+clic sur deux autres calques, « Produit » → les trois ; un Cmd+Z les remet.
  3. Clic droit sur la piste > Apparence > « Pelure d'oignon du calque » (Cmd+L écrit à droite) ; « Panneau de la pelure d'oignon »
     l'ouvre.
  4. La colonne : 340 points d'origine ; la tirer de 286 à 520 ; double-clic → 340 ↔ 520. Relancer → la largeur est gardée ;
     l'espace Montage garde la sienne.

#### Partie 172 — Préréglages, lot 3 : la caméra dans l'espace (2026-11-18, dd6f3a9, fusion cd23311, format 59)
- Source : INTERFACE 172.1/172.4, l. 14465-14568 ; JOURNAL l. 322-340.
- Statut : vu en partie. Seul le panneau a été vu (refait par la 195 en un aperçu et une liste de noms, réduit par la 196, sur ses
  demandes) ; les préréglages eux-mêmes ne l'ont jamais été.
- Manipulations :
  1. « Travelling latéral » sur un plan qui a des calques en profondeur → le proche passe vite ; tirer la flèche (outil Caméra).
  2. « Orbite » : outil Caméra, tirer le viseur « Sujet » sur un visage ; Angle 90 → le visage reste à sa place.
  3. « Grue » Hauteur 2 puis −1 ; « Survol » : Hauteur, Bascule, Avancée.
  4. Glisser « Arc » (famille Trajectoires) sur la piste Caméra → la barre « Chemin de la caméra ». Dans la vue Espace 3D avec
     l'outil Trajectoire, Cmd+clic sur le chemin pose une poignée.
  5. Clic droit sur la barre du chemin : décocher « garde son sujet », puis « Retirer » ; Cmd+Z. « Convertir en clés » sur une
     orbite → l'image ne change pas.

#### Partie 171 — Préréglages, lot 2 : la caméra 2D (2026-11-18, 146fb76, fusion ac390e6, format 58)
- Source : INTERFACE 171.1/171.4, l. 14363-14463 ; JOURNAL l. 343-360.
- Statut : vu en partie (le panneau seulement, comme pour la 172).
- Manipulations :
  1. Poser « Panoramique à plat », prendre l'outil Caméra → la flèche dans le cadre ; tirer son bout (Maj : droite) → l'image suit ;
     Cmd+Z.
  2. « Zoom vers un point » : tirer la cible sur un visage → à la fin du bloc, il est au centre et serré.
  3. « Zoom par paliers » avec Paliers 2 puis 4 ; « Plan penché progressif » et le champ Angle.
  4. Les secousses : tremblement de terre, pas lourds (Pas 1), caméra qui marche, véhicule, houle, respiration.
  5. « Convertir en clés » sur un plan à plusieurs blocs → l'image ne change pas ; enregistrer, rouvrir → blocs et flèches reviennent.

#### Partie 170 — Préréglages, lot 1 : le socle (blocs jaunes dans la timeline) (2026-11-18, 2385959, fusion d43ea27, format 57)
- Source : INTERFACE 170.1/170.4, l. 14281-14361 ; JOURNAL l. 404-412, 461-470.
- Statut : vu en partie. Le panneau a été vu et remplacé (la grille de vignettes est devenue un aperçu et une liste à la 195 ; la
  barre des panneaux a été retirée à la 194). Les blocs jaunes n'ont jamais été vus.
- Manipulations (Animation, timeline du plan ; dans le Storyboard, voir la 178) :
  1. « + Panneau » > Préréglages ; survoler un nom → l'aperçu le joue.
  2. Glisser « Zoom avant » sur la rangée de la caméra → un repère jaune ; au lâcher, un bloc, et l'image zoome aussitôt.
     « Tremblement léger » par-dessus → un second rang, les effets s'ajoutent.
  3. Tirer le bord droit d'un bloc, puis son corps → un Cmd+Z par geste. Clic sur le bloc puis ⌫ → il part, les clés restent.
  4. Cliquer un nom → un bloc à la tête de lecture.
  5. Panneau Caméra > « Effets de caméra » : la liste des effets, l'intensité ; « Convertir en clés » → l'image ne change pas.

#### Trajectoire : une marque qu'on glisse emporte son temps (2026-11-18, 223054b) — section de la partie 119
- Source : INTERFACE l. 8476-8535 (hors de cette tranche) ; JOURNAL l. 441-460.
- Statut : jamais vu. La lecture « une marque glissée ne recale qu'elle » reste à confirmer par Anthony.
- Manipulations (le plan 10 de « dd » appartient à Anthony ; prendre n'importe quel calque animé en clés d'images avec une
  trajectoire à plusieurs marques) :
  1. Outil Trajectoire : glisser une marque vers la suivante → au lâcher, le dessin d'avant s'allonge dans la timeline ; la barre
     d'état dit « Marque à N % du chemin, image M ». La ramener → l'exposition revient ; Cmd+Z.
  2. Réglages de l'outil > « Temps selon le chemin » → les dessins serrés deviennent brefs ; « Temps égaux entre les marques » → tous
     pareils.
  3. En « Clés d'interpolation » → glisser une marque change la vitesse, pas les images.

#### Panneau Pinceau : réglages d'abord, liste en bas (2026-11-18, 907c92a) — section de la partie 45
- Source : INTERFACE l. 4232-4262 (hors de cette tranche) ; JOURNAL l. 472-488.
- Statut : jamais vu par Anthony (la session a regardé une image du banc).
- Manipulations : outil Pinceau, liste dépliée (bouton « livres ») → sous l'aperçu : les réglages et « Plus de réglages », puis
  importer / + / corbeille, puis la liste. Tourner un réglage → l'aperçu reste visible. « + » → la saisie du nom en tête de la liste.

#### Partie 187 — Espace 3D à côté du dessin (2026-11-16, fusion c9c24f6)
- Source : INTERFACE l. 16194-16253 ; ESPACE3D 13 ; JOURNAL l. 536, 546, 591-609.
- Statut : vu en partie. La 200.7 : Anthony, dans l'application d'essai, a eu la vue à côté ouverte et y a tiré la caméra (bug du
  plan gris, corrigé). Le reste n'a pas été vu.
- Manipulations :
  1. Le bouton « à côté » après « 3D » dans la barre au-dessus du dessin (196 : vue caméra, 2D, 3D, à côté…) → la vue 3D à droite.
     Allumé, il prend la couleur d'accent.
  2. Tirer la barre entre les deux → elle s'arrête à 20 % et 80 % ; double-clic → moitié-moitié ; relancer → la largeur est gardée
     (le côte à côte lui-même ne l'est pas).
  3. Dessiner sur la feuille → aucun retard du trait ; à droite, la carte suit (1/30 s au plus).
  4. Cliquer une carte à droite → son calque devient courant ; on dessine dessus à gauche.
  5. Touches 1 et 2 : souris sur la vue 3D, elle zoome seule ; souris sur la feuille, la feuille seule.
  6. « Agrandir » → plein écran ; le cube rend le côte à côte. « Fermer » → la feuille reprend toute la zone. Lire le plan vue
     ouverte → même fluidité.

#### Partie 186 — Raccourcis qui manquaient parfois (2026-11-16, fusion c9c24f6)
- Source : INTERFACE l. 16144-16192.
- Statut : jamais vu, et le défaut d'origine n'a pas été reproduit (trois causes trouvées en lisant le code).
- Manipulations :
  1. Déplacer un plan dans le montage, tirer un bord, puis taper 2 souris sur la feuille → le zoom répond.
  2. Pendant un glisser, taper 2 → la barre d'état dit « « Zoom avant » n'a pas été fait : un glisser est en cours. »
  3. Cliquer dans une case de texte, mettre la souris sur la feuille, taper 2 → le message « une case où l'on écrit a le clavier ».
  4. Espace 3D : 2 puis 1, la souris sur la vue puis sur un panneau. À quatre vues, seule la vue sous la souris zoome.
  5. En usage normal, noter tout raccourci qui ne fait rien et que la barre d'état n'explique pas (cause encore inconnue).

#### Partie 179 — Timeline plus claire, ce qui en reste (2026-11-17, c8fdfd5 puis bebafae)
- Source : INTERFACE 179.0, 179.1 points 4-5, 179.3, l. 15090-15172 ; JOURNAL l. 224-232, 301-308.
- Statut : jamais vu dans sa forme finale. Anthony a choisi la version C sur des maquettes, et avait construit la branche le 17
  (JOURNAL l. 546) sans en rendre compte. Il ne reste que la teinte du rangement, les dossiers en titres et les clés d'un dossier
  fermé.
- Manipulations :
  1. Un plan de quinze calques en dossiers et groupes de couleur (Animation) → les lignes d'un dossier ont sa couleur, et un calque
     d'un groupe celle de son groupe, dans la colonne comme dans la grille.
  2. Les dossiers : icône de leur couleur, nom en demi-gras et nombre de calques. Fermer un dossier → sa ligne montre les clés de ses
     calques en traits fins.
  3. Dans les trois thèmes → la teinte reste lisible (trop forte ou trop faible ?).

### Parties 150 à 169 (lot 4)

#### Partie 169 — Les réglages du Cutter (2026-11-18, 050cec7, fusion bed74af)
- Source : INTERFACE 169.1-169.3, l. 14198-14279 ; JOURNAL l. 413-427
- Statut : vu en partie. Anthony a validé le dessin du panneau (après deux essais refusés, 169.2) ; le comportement des cinq
  réglages n'a jamais été vu (Claude a vu le panneau tenir, pas le nettoyage : calque de la copie verrouillé).
- Manipulations :
  1. Outil Cutter, panneau : Mode « Couper | Dépassements », Geste « Glisser | Lasso », Portée « Calque actif | Calques visibles »,
     les dessins-curseurs Écart toléré et Bouts effilés, « Précis » → la ligne « Dépassements enlevés jusqu'à » n'apparaît qu'en
     mode Dépassements ; tout tient au-dessus de la Couleur sans défiler.
  2. Deux traits vectoriels qui ne se touchent pas (écart de 3 px) : écart toléré 0 → rien n'est coupé ; écart 4 px → l'autre trait
     est coupé au prolongement du bout.
  3. Bouts effilés 30 px, couper un trait texturé → le bout coupé s'affine ; regarder si le trait perd son aspect (appuis perdus).
  4. Un dièse dessiné avec des bouts qui dépassent, mode Dépassements, Lasso autour → les petits bouts partent, jamais un morceau
     entre deux croisements ni un trait isolé.
  5. Portée « Calques visibles », une ligne sur un autre calque → elle coupe sans être modifiée ; cachée, elle ne coupe plus.
  6. Trois calques choisis, un geste → trois traits coupés, un seul Cmd+Z, barre d'état « Cutter sur 3 calques… ». Relancer
     FEANOR : les réglages sont retrouvés.

#### Partie 168 — Les versions d'un plan dans la timeline (2026-11-17/18, 6336310, b0f7b4f, fusion b3059ec)
- Source : INTERFACE 168.1-168.7, l. 14070-14196 ; JOURNAL l. 428-440
- Statut : jamais vu par Anthony (vu par Claude le 17 au soir, 168.6). Ses réponses du 18 (168.7) portent sur les questions de
  168.5, pas sur un essai à l'écran ; le son par version, le repli et les teintes douces ajoutés ensuite n'ont été vus par personne.
- Manipulations :
  1. Storyboard, un plan à deux versions (Fiche > « + nouvelle ») : sur son bloc, « Plan 1 · V2 » puis « V1 ▸ ». Clic sur « V1 » →
     la V1 devient la version en cours, une étape (Cmd+Z revient), message « Le plan est maintenant en V1… ». Clic sur le « V » du
     nom → la pile des versions.
  2. Clic sur ▸ → un bloc hachuré « V1 ◂ dépliée » après le plan, les plans suivants glissent à l'écran ; clic dans le bloc → la vue
     montre la version avec « … dépliée · lecture seule · image N », on ne peut pas y dessiner ; ◂ replie.
  3. Lecture (Espace) traverse le plan, puis la version dépliée, puis le plan suivant ; le son sous la dépliée est celui posé sous
     cette version quand elle a été quittée, sinon le son du film repris au début du plan.
  4. Clic droit sur « V2 » → palette de dix teintes douces + « Aucune » ; une teinte colore l'étiquette, le bord du bloc déplié et
     le cadre de la vue ; une étape d'annulation ; elle suit le numéro de version.
  5. Clic sur « V2 » alors qu'elle est dépliée → elle devient la version en cours et se replie, aucune autre ne se déplie à sa place.
  6. Zoom arrière sur la bande : les étiquettes qui ne tiennent pas se résument en « +2 » (clic : pile ; clic droit : menu). Exporter
     pendant qu'une version est dépliée → l'export ne la contient pas ; fermer / rouvrir → rien n'est déplié.

#### Partie 167 — Le nouveau logo (2026-11-17, fusion 1d30449)
- Source : INTERFACE 167, l. 14041-14068
- Statut : jamais vu (aucune mention d'un essai).
- Manipulations : 1. Lancer FEANOR sans projet → « FEAN », l'étoile à la place du O, « R », la devise ; l'étoile s'ouvre une fois puis
  reste figée. 2. Aide > À propos de FEANOR… → l'étoile devant le mot. 3. Thème Papier : accueil et À propos lisibles.
  (Point 3 de la doc, icône de `feanor.exe` dans l'Explorateur : Windows ; sur Mac regarder l'icône du Dock.)

#### Partie 166 — La vue nette, la vue réduite, les vignettes (2026-11-17/18, 1362bf6, bcc3e02, 19f8b18, 24b1d44 ; fusions 7742e31, e8537c3, baf6476)
- Source : INTERFACE 166.2, 166.7, 166.8, 166.9, 166.10 (l. 13730-13747, 13837-13850, 13852-13890, 13892-13961, 13963-14001) ;
  JOURNAL l. 363-403, 492-571
- Statut : vu en partie. Anthony a vu la vue nette au-delà de 100 % dans son FEANOR (son retour « on voit encore la pixelisation
  au moment du zoom », 166.8) et a choisi la façon 2 de la vue réduite sur la branche d'essai à bascule (166.9, « Pardon, la 2 ») :
  ces deux points sont vus. Restent jamais vus : le lissage en mouvement fusionné (166.8), la vue réduite fusionnée sur main, et
  toute la 166.10 (vignettes).
- Manipulations :
  1. Vue de dessin à 225 % ou plus sur un dessin vectoriel : traits nets ; puis tracer un trait au stylet → aucun ralentissement, la
     zone du trait adoucie pendant le geste, nette au lâcher.
  2. Déplacer (Espace + glisser), zoomer, tourner la vue rapidement → l'image adoucie puis nette, vue fluide.
  3. Un calque bitmap dans le même plan → toujours en pixels nets, à sa place dans la pile ; Cmd+Z, changer d'image, de calque →
     l'image nette revient.
  4. Vue cadrée d'origine (35-50 %), traits fins et hachures → traits continus, sans pointillés ni trous ; vue tournée aussi.
  5. Onglet Vignettes, six cases par ligne : les cases qu'on ne dessine pas ont des traits continus, du même poids que la case
     qu'on dessine ; pas de saut d'aspect en passant d'une case à l'autre ; vignettes de plans de la timeline idem.
  6. Ouvrir un projet de plusieurs plans et zoomer la planche → noter le temps de remplissage (coût mesuré jusqu'à 1,6 fois).
  (Le plan 2 / plan 10 de « dd » cités dans la doc sont des fichiers d'Anthony : prendre n'importe quel dessin vectoriel fin.)

#### Partie 165 — Avant | après du vrai calque, Alt + clic, Trois calques retiré, boutons de la timeline (2026-11-16/17, 899de9c, 8de9324, e1f2cf0, b1b9686, fc460d8 ; fusion 0073f65)
- Source : INTERFACE 165.1, 165.3, 165.6, 165.7, 165.8 (l. 13488-13510, 13517-13534, 13625-13653, 13655-13683, 13685-13709) ;
  JOURNAL l. 505-514
- Statut : en partie remplacé. 165.2 annulé (165.3). 165.4 (ligne flottante) remplacé par 179.4 (« elle ne flotte plus »).
  165.5 (Storyboard) remplacé par 174 puis 192.1 (une ligne). 165.3 demandé par Anthony sur capture. Restent jamais vus : 165.1,
  165.6, 165.7, 165.8.
- Manipulations :
  1. Panneau Effets (onglet de Propriétés), calque dessiné, vignette Lueur, puis tirer la barre de l'avant | après → à gauche le
     calque sans l'effet, à droite avec ; l'image arrive après le geste ; une étape d'annulation ; calque vide → schéma.
  2. Animation (double-clic sur un plan), fenêtre la plus petite : les boutons de la timeline sur deux lignes en haut de la colonne
     des calques ; « Image suivante » (ligne 1) et « Clé de caméra ici » (ligne 2) répondent.
  3. Clic droit sur une piste du Storyboard : plus de choix « Trois calques / Toute la pile » ; toute la pile est montrée.
  4. Pinceau en main, Option + clic sur l'encre d'un calque du dessous → il devient le calque actif (barre d'état « Calque actif :
     … »), aucune étape d'annulation, la couleur ne change pas ; Option + clic dans le vide → « Aucun dessin sous le curseur ».
  5. Outil Pipette : prend toujours la couleur ; Dégradé + Option : « tout le calque » inchangé ; dans la vue Espace 3D,
     Option + glisser tourne la vue.

#### Partie 164 — La Couleur et les Effets plus visuels (2026-11-15, f3827b2, d04a23c)
- Source : INTERFACE 164.1, 164.2, 164.7, l. 13335-13376, 13459-13466
- Statut : en partie remplacé. 164.3 (ligne du calque en pastilles) retiré par 165.3 ; « Avant · maintenant » de 164.1 retiré par
  195.7. La roue en tête de la Couleur a été vue par Anthony (captures annotées de la partie 194/195). Restent jamais vus : les
  vignettes d'effets et la barre de force (164.2), « Précis » de la Couleur.
- Manipulations :
  1. Effets : cliquer les vignettes Lueur, Flou, Grain… → l'effet s'ajoute (une étape) ; recliquer une vignette allumée → l'effet
     est choisi, pas ajouté deux fois ; sur un calque bitmap, pas de vignette Tremblement.
  2. Tirer la barre avant | après d'un flou → le rayon change, une étape ; valeur 80 px tapée sous Précis → la barre s'élargit.
  3. Effets > Précis : « Ajouter un effet » par familles, préréglages, Tout supprimer, monter / descendre un effet.
  4. Couleur > Précis : nuancier (+, −, …), choix roue / curseurs, carré / triangle, TSV / RVB / TSL, code, récentes, mélange.

#### Partie 163 — Fiche du plan, mouvements de caméra des Vignettes, planche, exports (2026-11-15, 812ec35, a78ee79)
- Source : INTERFACE 163.1-163.4, 163.7, l. 13175-13252, 13315-13324
- Statut : jamais vu (captures de contrôle par Claude seulement).
- Manipulations :
  1. Fiche du plan : tirer la poignée de la barre de durée → le plan s'allonge (une étape « Allonger la fin ») ; Cmd tenu pendant le
     geste → les clés suivent ; Échap abandonne.
  2. Vignettes des versions : « + nouvelle », clic sur V1 puis V2 ; clic droit → comparer, restaurer, supprimer.
  3. Onglet Vignettes, case sans mouvement : partie « Mouvement de caméra · A → B », clic Zoom avant → cadres A et B sur la case ;
     puis Doux, Brusque → les poignées d'adoucis de la piste caméra suivent ; au bout du panoramique, la barre d'état le dit.
  4. Planche : les deux dessins « En lignes » / « À la japonaise », tirer le bord de la mini-planche (1 à 6 cases), cocher Action,
     Dialogue, Notes.
  5. Fichier > Exporter en image > Storyboard en PDF : quatre vignettes de famille, aperçu de la première page, gros bouton
     « Exporter le storyboard… » ; « Pour le montage » ouvre sa propre fenêtre à la place ; Précis : tous les réglages d'avant.

#### Partie 162 — Le panneau Espace 3D en graphiques (2026-11-15, 55e959f)
- Source : INTERFACE 162.1-162.5, l. 13038-13161
- Statut : jamais vu. Le panneau est aujourd'hui un onglet de Propriétés (194.7) ; dans l'onglet 3D ses graphiques sont repris par
  Propriétés > Caméra / Objet (198.1.4), à tester avec le lot de la partie 198.
- Manipulations (Storyboard, cube ouvert) :
  1. Rien de choisi : « Ajouter à la scène » Surface, Sol, Mur, Ciel → chacun pose son calque ; « Le plateau, vu de dessus » :
     glisser un trait vers le haut → le calque recule (une étape « Distance du calque ») ; clic sur un trait → le calque est choisi.
  2. Calque choisi : Face, Au sol, Mur g., Mur d. (une étape chacun), la réglette de distance, le curseur de taille, « ▸ Précis ».
  3. Clic sur le triangle de la caméra du plateau : vignettes 18/35/50/85/135, glisser le cône, monter la caméra à côté du
     personnage, tirer la visée, tourner le cadran, pencher l'horizon → chaque geste = un Cmd+Z.
  4. Barre au-dessus de la vue : Affichage (grille du sol, montrer les calques, environnement : fond, maillage) et Aimant.
  5. « Le plus proche passe devant » sur un plan dont la pile est dans le désordre, puis Cmd+Z.

#### Partie 160 — Panneaux fins : ce qui en reste (2026-11-14, 216cb89, b4d2fd5)
- Source : INTERFACE 160.1 points 2, 7, 8, 9 et 160.3, l. 12883-12916, 12931-12949
- Statut : en grande partie remplacé : poignées et barre des panneaux par 194 ; ligne du calque courant par 179/192 ; barre
  d'actions des calques par 192.1.2 ; barres de la vue vues par Anthony (captures annotées, 195.1). Restent jamais vus :
- Manipulations :
  1. Storyboard, ouvrir le cube : les outils de la vue (Choisir, Déplacer, Tourner, Échelle, Caméra, Dessiner sur la carte) en tête
     de la barre d'outils de gauche ; refermer → la barre d'outils d'avant.
  2. Commande « Calques » (Cmd+K, taper Calques) : la bande revient si elle était cachée ; clic dans la colonne des calques puis
     Cmd+C / Cmd+V → copie et colle le calque.
  3. Vignettes : la ligne « Annotations » (œil, clic → stylo d'annotation) et « Dessin de la case » ; clic droit → les couches
     (viser, renommer, monter, descendre, nouvelle couche).
  4. Clic droit sur un calque qui a un aplat : « Réglages de l'aplat » sous « Retirer l'aplat ».

#### Partie 159 — Tous les plans dans l'espace, la 3D jamais sentie à plat (2026-11-14, c0e3df2, format 56)
- Source : INTERFACE 159.1-159.8, l. 12673-12867
- Statut : jamais vu tel quel (les projets d'Anthony ont été migrés sans contrôle d'image ; un défaut issu de 159 a été vu par
  Anthony et corrigé en 200.7, voir C). Renommage « Devant cache derrière » → « Le plus proche passe devant » (162). L'icône
  « Espace 3D » de la barre des panneaux n'existe plus (194).
- Manipulations :
  1. Copie d'un projet d'avant le format 56 : aucune fenêtre ; exporter l'image d'un plan à caméra animée et comparer à un export
     fait avant → identique.
  2. À plat, sans ouvrir le cube : menus, Cmd+K, « + » des calques, bulles → nulle part « 3D », « Espace 3D », « carte », « scène »
     (sauf la porte : le cube, Affichage > Plateau > Espace 3D, la ligne de palette, l'onglet « 3D » depuis 198).
  3. Cube sur un plan dont la caméra bouge → aucun refus ; poser un Sol → il se pose ; fermer → barre d'état « Retour au dessin. ».
  4. Dans le cube, panoramique de la caméra à 150°, fermer → la feuille montre ce qu'elle filme ; outil Caméra : glisser, zoomer,
     tourner marchent encore ; Vignettes : « Pas de cadre sur cette vignette… ».
  5. Plan neuf : outil Caméra, vue de dessus, cadres des Vignettes comme avant.

#### Partie 158 — Le maillage tout près de la scène (2026-11-14, 47addb0)
- Source : INTERFACE 158.1, 158.5 points 2 et 5, l. 12551-12568, 12662-12671
- Statut : 158.2 remplacé par 159 (plus de passage « tout seul »). Reste 158.1, correctif d'un bug vu par Anthony, jamais revu.
- Manipulations : 1. Cube, maillage au pas fixe 0,6 m (Affichage > Environnement), molette jusque tout près du sol → le sol reste
  quadrillé jusque sous la vue, en 1 vue comme en 4 ; reculer → rien ne saute.

#### Partie 157 — Environnement, calques Sol / Mur / Ciel et leur grille (2026-11-14, 007daca, fusion 85596fa)
- Source : INTERFACE 157.1-157.6, l. 12458-12542
- Statut : vu en partie : Anthony a vu l'environnement et le maillage (ses deux retours de la partie 158). L'environnement est passé
  dans la barre de la vue > Affichage (162) et dans Propriétés > Monde de l'onglet 3D (198) ; le refus hors plan en Espace 3D a
  disparu (158/159). Restent jamais vus : la grille de repère, le Ciel, la mémoire par projet.
- Manipulations :
  1. Cube ouvert, « + » des calques > Calques 3D > Sol, Mur, Ciel → le Ciel tout au fond, centré sur l'horizon, remplit le cadre ;
     Cmd+Z enlève chacun d'un coup ; cube fermé, le sous-menu Calques 3D n'est pas là (159).
  2. Calque Sol choisi, « Précis » du calque (ou menu du calque > Plus > Réglages avancés > « Grille de repère ») : allumer la
     grille → traits bleus sur la carte ; dessiner, gommer → la gomme n'efface jamais la grille.
  3. Fermer la vue : la grille reste visible tant que le Sol est le calque courant ; exporter une image → pas de grille.
  4. Fond bleu nuit et pas 0,5 m, enregistrer, fermer, rouvrir → gardés ; un autre projet garde le gris d'origine.

#### Partie 154 — Les poignées de la trajectoire et leurs flèches x, y, z (2026-11-13, dbe873b, fusion 934b13d)
- Source : INTERFACE 154, l. 12198-12331
- Statut : jamais vu (journal : aucune entrée pour 154).
- Manipulations :
  1. Feuille, outil Trajectoire, un chemin : Cmd+clic sur le chemin → un rond jade ; Cmd+clic ailleurs → le lasso ; Cmd+clic sur le
     rond → il part.
  2. Deux poignées, glisser la première → le chemin se tord en douceur, la seconde et les bouts fixes ; Échap au milieu → tout
     revient ; Cmd+Z → un seul pas.
  3. Cube, une carte avec un chemin au sol : Cmd+clic sur le chemin → poignée et flèches x rouge, y vert, z bleu ; tirer y → le chemin
     monte ; lire le plan → la carte monte et redescend.
  4. Caméra choisie seule, poignée sur son chemin, tirer y → la caméra monte sans clé.
  5. « Vues » 4 vues, tirer y dans la vue de face → le chemin monte dans les quatre ; enregistrer, rouvrir → poignées et bosse là.

#### Partie 152 — Trajectoire 3D : caméra seule, clés d'images, dessins posés à la main, lasso dans la vue (2026-11-13, 6e72a99 … 9ea2375, fusion 934b13d)
- Source : INTERFACE 152.1, 152.2, 152.4 et listes l. 11999-12057 et 12157-12196 (points 22 à 31, rangés à la fin de 153) ;
  JOURNAL l. 629-721
- Statut : jamais vu par Anthony ; Claude a vu à l'écran (13 nov.) : Cmd+clic caméra, rangée « Caméra » choisie, une clé de
  dessin à une marque, un lasso de cinq éléments, le bouton « Caméra », deux dessins à la main gardés. 152.3 (place du panneau)
  remplacé par 159 puis 194.7 ; tout ce qui parle du panneau Calques (points 3, 24) est sans objet depuis 160.
- Manipulations (Storyboard, cube ouvert) :
  1. Outil Trajectoire, après avoir dessiné sur une carte : Cmd+clic sur la caméra → elle seule est choisie, barre d'état « La
     caméra est choisie, seule… », la piste « Caméra » de la timeline est la rangée choisie ; tracer un chemin → aucun calque ne
     bouge ni ne reçoit de clé. Bouton « Caméra » de la barre de la vue → même chose, l'outil Trajectoire reste en main.
  2. Clic sur un calque de la timeline (même le courant), ou prendre le pinceau → la caméra est lâchée. Maj+Cmd+clic sur une carte
     → elle s'ajoute à la caméra.
  3. Carte dessinée, « Clés d'images », chemin + deux marques → clé de dessin au départ, à chaque marque, à la fin ; redessiner une
     pose, glisser la marque, poser une marque avant / après, Cmd+Z, enregistrer / rouvrir → chaque pose reste à sa marque.
  4. Sur la feuille, clé posée à la main (« Nouvelle clé ») entre départ et marque, dessinée : glisser la marque, « Temps égaux »,
     retracer, passer en clés d'interpolation et revenir → ton dessin reste, aucune fenêtre, barre d'état « Trajectoire : 1 dessin
     posé à la main gardé à l'image N… ».
  5. Outil Sélection dans la vue : lasso sur une carte penchée → contour et boîte en perspective ; glisser, poignée de coin,
     extérieur d'un coin ; Suppr, Cmd+X / C / V, Échap, Cmd+Z ; pinceau en main, Cmd+glisser → lasso, l'outil revient au relâché.
  6. Outils de la vue, clic simple sur le dessin de la caméra passée devant une carte → la caméra (pas la carte) ; la tirer → elle
     glisse seule. Clic sur la piste « Caméra » vue ouverte → caméra seule ; vue fermée → rien d'autre qu'avant.

#### Partie 151 — Lisser une trajectoire, la continuer avec Maj, vues multiples (2026-11-12, b22611d, 78ffda3, fusion 934b13d)
- Source : INTERFACE 151.1-151.3 et liste l. 11528-11542
- Statut : jamais vu (« Rien n'a été regardé à l'écran », l. 11520).
- Manipulations :
  1. Feuille, trajectoire en zigzag, tirer « Lissage » dans Réglages → la courbe s'adoucit en direct, bouts fixes ; 0 % → le zigzag
     revient ; Cmd+Z annule le geste entier ; deux calques choisis → les deux lissés.
  2. Clés d'images : pendant le geste seul le chemin bouge ; les dessins se reposent au lâcher.
  3. Maj + trait depuis la fin d'un chemin → il continue ; depuis le début → il s'allonge devant ; Maj au milieu → déplacement en
     bloc ; Échap pendant le trait → rien.
  4. Même chose dans le cube, sur un chemin au sol ; « Vues » 4 vues : un chemin tracé dans la vue de dessus se voit dans toutes,
     le pinceau tombe sous le pointeur dans chaque vue ; se promener longtemps → pas de plantage.

### Parties 120 à 149 (lot 3)

#### Partie 149 — Trajectoire dans l'Espace 3D (2026-11-12, 651dfb0)
- Source : INTERFACE 149, l. 11099 (liste l. 11246) ; JOURNAL l. 751-779, 705-721.
- Statut : vu en partie. Anthony l'a essayé (parties 149-150) : ses quatre remarques (caméra choisie avec un calque, clés d'images
  sans dessin, place du panneau, lasso) ont donné la partie 152. Claude a vu à l'écran (souris simulée) : chemin au sol suivi par une
  carte, une marque, caméra sur chemin sans clé, Ctrl+Z/Ctrl+Y, réouverture. Jamais vu par un humain : tordre/déplacer le chemin,
  glisser/arracher une marque, deux cartes, carte + caméra, chemin sur un mur, réglages, recalage sur la timeline, montage.
  Point 14 (plan ordinaire refusé) caduc (159). Ctrl+clic = choisir seul, Maj+Ctrl+clic = ajouter (152).
- Manipulations :
  1. Vue Espace 3D (cube), outil Trajectoire, deux petits traits en travers d'un chemin tracé au sol, serrés près du départ → la
     carte part lentement ; glisser une marque, puis la tirer loin du chemin → elle disparaît ; Cmd+Z = un pas par geste.
  2. Tirer le chemin au milieu → il se tord ; Maj+glisser → il se déplace en bloc ; Échap pendant le geste → rien ne change.
  3. Maj+Cmd+clic sur une seconde carte puis un trait → les deux suivent ; un seul Cmd+Z retire les deux chemins.
  4. Tracer sur une carte debout non choisie (mur) → le chemin est sur le mur, la carte choisie monte/descend sans changer de distance.
  5. Timeline : glisser la clé d'une marque d'une carte → mouvement recalé ; raccourcir puis rallonger le plan → mouvement de caméra
     intact ; couper le plan au milieu → les deux moitiés gardent leur image.
  6. Caméra en panoramique 180°, fermer, rouvrir → toujours retournée.

#### Partie 148 — Une carte passe derrière la caméra d'origine (2026-11-12, f2860cd, format 55)
- Source : INTERFACE 148, l. 10932 (liste l. 11075) ; JOURNAL l. 751-760, 780-796.
- Statut : jamais vu par un humain. Claude a seulement vu une carte mise à Z 8 m sortir de l'image (et noté « Échelle » à 1000 %).
  Points 9 et 11 (décocher « Plan en Espace 3D », plan ordinaire) caducs (159).
- Manipulations :
  1. Vue Espace 3D, tirer une carte par la flèche de profondeur vers la caméra d'origine → elle la traverse, continue derrière, garde
     sa taille dans l'espace.
  2. Quitter la vue, caméra non déplacée → la carte passée derrière n'est plus dans l'image ; « Profondeur » −0,95 → elle remplit
     l'image.
  3. Reculer la caméra derrière la carte → l'image la montre ; dessiner dessus → le trait tombe sous le stylet.
  4. Une clé de position loin devant, une derrière la caméra, lire → dire si la taille « respire » en chemin (défaut connu).
  5. Supprimer les clés de position de cette carte → elle ne bouge pas ; enregistrer, rouvrir → toujours derrière.

#### Partie 146 (et 146.1) — Dessiner dans la vue Espace 3D (2026-11-12, 7b6800e, 3f4e3e9)
- Source : INTERFACE 146, l. 10684 (liste l. 10782), 146.1 l. 10803 (l. 10843) ; JOURNAL l. 830-868.
- Statut : jamais vu par un humain (« rien regardé à l'écran ni au stylet »). Le défaut corrigé en 146.1 avait été vu par Anthony
  (point 9 de 143) ; la correction n'a pas été regardée. 160 : « Dessiner sur la carte » et les outils de la vue sont en tête de la
  barre d'outils de gauche ; 152 : Sélection et Cmd+glisser (lasso) ajoutés dans la vue.
- Manipulations :
  1. Vue Espace 3D, outil Pinceau (ou « Dessiner sur la carte ») → la carte du calque courant est entourée ; dessiner de face → le
     trait suit sans retard ni décalage, ne change pas au relâché.
  2. Tourner la vue (Option+glisser ou clic droit), dessiner de trois quarts → le cercle du pinceau est une ellipse couchée sur la
     carte, le trait tombe sous la pointe.
  3. Cmd+clic sur une autre carte → elle devient courante, le trait suivant y va ; outil Déplacer de la vue → le même geste déplace la carte.
  4. Pointe hors de la carte, carte vue de dos ou par la tranche → curseur interdit, rien tracé, barre d'état dit pourquoi.
  5. Cmd+Z après un trait → il part ; quitter la vue → le trait est sur la feuille au même endroit.
  6. 146.1 : caméra dos aux cartes (panoramique 180°), pinceau sur la feuille hors vue → rien ne se trace, message, Cmd+Z n'a rien à défaire.
  (Point « au stylet » : nécessite la tablette, hors souris.)

#### Partie 145 — Boutons de clés dans les réglages de l'Espace 3D (2026-11-12, 5eb26a4)
- Source : INTERFACE 145, l. 10615 (liste l. 10670) ; JOURNAL l. 869-882.
- Statut : jamais vu (« Rien de cette partie n'a été regardé à l'écran »). Les boutons sont maintenant dans le panneau Espace 3D
  (150, 11345), refait en graphiques par 162 : recouper avec le lot qui couvre 162.
- Manipulations :
  1. Vue Espace 3D, panneau Espace 3D, ligne « Caméra du plan » : losange + corbeille ; tête de lecture au milieu, clic sur le
     losange → il s'allume, la piste Caméra montre deux clés.
  2. Sur une clé de caméra : corbeille → la clé part ; Cmd+Z → elle revient.
  3. Une carte choisie : losange à côté de son nom → losange vert sur sa piste ; changer d'image, déplacer la carte → elle s'anime.
  4. Deux cartes (Maj+clic) → boutons sur « 2 cartes choisies », une clé sur chacune, un seul Cmd+Z.
  5. Carte verrouillée → rien posé, message.

#### Partie 144 — Plusieurs vues dans l'Espace 3D (2026-11-12, 16f24d9, fusion e82d047)
- Source : INTERFACE 144, l. 10549 (liste l. 10587).
- Statut : jamais vu par un humain. Le bouton « Vues » est devenu « 1/2/4 vues » dans la barre du haut (160) ; l'onglet 3D (198) a
  sa propre barre Perspective · Dessus · Face · Côté · Caméra et 1 · 2 · 4.
- Manipulations :
  1. Vue Espace 3D, 4 vues → Dessus, Face, Droite, Perspective, chacune avec grille et repère d'axes.
  2. Molette, bouton du milieu (sur trackpad : à défaut, Option+glisser) dans une vue → seule celle-ci bouge.
  3. Clic droit + glisser dans la vue de face → elle passe en perspective, barre d'état le dit ; son menu la remet de face.
  4. Tirer une carte dans la vue de face → elle bouge dans les quatre ; dans la vue de dessus, clic sur son trait → choisie.
  5. Glisser la bordure centrale ; double-clic sur le menu d'une vue → plein cadre, encore → retour ; enregistrer, rouvrir → les 4 vues reviennent.

#### Partie 143 — Plan en Espace 3D (2026-11-12, e5dc84e ; ESPACE-LIBRE.md A0-A8)
- Source : INTERFACE 143, l. 10484 (liste l. 10524) ; ESPACE-LIBRE.md l. 183-490 ; JOURNAL l. 797-807, 830-833.
- Statut : vu en partie. Anthony a essayé la branche à l'écran (12/11) : il a vu le point 9 (trait invisible enregistré → 146.1) et
  remarqué l'absence de clés (→145) et l'impossibilité de dessiner dans la vue (→146). Claude a fait les points 2, 3, 4, 8, 12, 13,
  14, 15 (souris simulée) : jamais vus par un humain. Points 1, 13, 14 caducs (159 : plus de case, plus deux sortes de plans ;
  bouton remplacé par « Devant cache derrière »). Non vus du tout : 5, 6, 7, 10, 11.
- Manipulations :
  1. Panneau Espace 3D, « Nouvelle surface » : face, sol, mur → les trois icônes lisibles ; au sol à plat sur le quadrillage, mur debout.
  2. Sur le sol, dessiner vers l'horizon (hors vue) → le trait s'arrête avant l'infini, sans à-coup.
  3. Caméra tournée, dessiner sur une carte → sans décalage, cercle du pinceau à la largeur du trait, plus petit sur une carte loin.
  4. Trois calques à des profondeurs différentes, pile à l'envers, « Devant cache derrière » allumé → le plus proche est devant.
  5. Caméra à −90° au-dessus d'un sol → plongée totale, pas de rotation brusque ; exporter le plan en vidéo → même image (Claude a vu
     des couleurs un peu décalées avec l'encodeur Windows : à regarder sur Mac).

#### Partie 142 (et 142.6) — Mouvement de caméra B vers C sur une case de continuité (2026-11-11, 2066ecf ; 142.6 2777b27)
- Source : INTERFACE 142, l. 10413 (liste l. 10446), 142.6 l. 10460 (liste l. 10478) ; JOURNAL l. 1040-1077.
- Statut : vu en partie : Anthony a essayé B vers C (son retour a donné 142.6). 142.6 (le mouvement naît sur la case demandée)
  jamais vu ; vérifié seulement par Claude sur la copie de son projet.
- Manipulations (Vignettes) :
  1. Plan à deux cases sans caméra : Maj sur la deuxième case, clic sur le « + » → le cadre B est sur la deuxième, la première ne bouge pas ; lire → arrêt puis A vers B.
  2. Plan A→B + deux cases de continuité : Maj sur la dernière, clic sur « + » → C ; lire → A→B, arrêt sur B, B→C.
  3. Déplacer B sur la première case → le départ de B→C suit ; clic droit sur C > « Retirer ce cadre » → arrêt sur B.
  4. Case traversée par un mouvement venu d'ailleurs → refus, la barre d'état donne les images à ramener.
  5. Cmd+Z après chaque geste → une seule étape.

#### Partie 141 — Calque accroché à la caméra (2026-11-11, c6cca20, format 53)
- Source : INTERFACE 141, l. 10357 (liste l. 10399) ; JOURNAL l. 1018-1037.
- Statut : jamais vu (« aucun geste fait à la main dans le vrai exe »). Le bouton n'est plus dans le panneau Calques (160) : icône
  « accroche à la caméra » sur la piste du calque actif, dans la timeline.
- Manipulations :
  1. Plan avec deux clés de caméra ; cliquer l'icône caméra du calque → rien ne bouge à l'image courante.
  2. Lire le plan → le calque reste à la même place dans le cadre, les autres défilent.
  3. À une image où la caméra est zoomée ailleurs, dessiner sur le calque → le trait tombe sous la pointe ; Transformer → la boîte suit la main.
  4. Recliquer → décroché sans bouger ; Cmd+Z ; enregistrer, rouvrir → toujours accroché.

#### Partie 140 — Les calques de la timeline prennent la place (2026-11-11, 7e96224)
- Source : INTERFACE 140, l. 10316 (liste l. 10345) ; JOURNAL l. 1003-1017.
- Statut : en grande partie remplacé par 192 (calque actif replié d'origine ; « Agrandir les calques proches » ne joue que si le
  calque actif se déplie, l. 16928). Reste : le réglage `calques_grands_si_place` (settings.rs:183) et le comportement une fois le
  calque déplié. « Trois calques » (points 4) caduc (165.7).
- Manipulations : déplier le calque actif, tirer le bord de la bande vers le haut → les calques voisins grandissent un à un, pas de
  grand vide ; tirer vers le bas → ils repassent en bandes, les plus loin d'abord ; Préférences > Montage : la case « Agrandir les
  calques proches quand il y a de la place ».

#### Partie 139 — Glisser des calques dans un dossier ; Ctrl+V de la dernière copie (2026-11-11, b78ea38 ; 139.5 02c1ceb)
- Source : INTERFACE 139, l. 10243 (liste l. 10284, 139.5 l. 10312) ; JOURNAL l. 965-1002.
- Statut : jamais vu par Anthony (son retour suivant portait sur F5 → 139.5, vérifié par Claude seulement). Le panneau Calques
  n'existe plus (160) : faire les gestes dans la colonne des calques de la timeline.
- Manipulations :
  1. Colonne des calques : trois calques choisis (Cmd+clic), en prendre un par son nom, le lâcher sur la ligne fine d'un dossier
     vide → la ligne s'encadre, les trois entrent dans l'ordre ; un Cmd+Z les ressort.
  2. Lâcher un calque sur un dossier fermé → le compte augmente, barre d'état « … rangés dans … ».
  3. F5 sur deux calques, clic sur le nom d'un des deux, lasso sur des traits, Cmd+C, Cmd+V → collés et choisis.
  4. Trois calques choisis dont deux masqués, fn+F5 → trois clés, « Nouvelle clé sur 3 calques ».

#### Partie 138 — Clés glissées avant/après, étirer un plan, durée des clés et des plans (2026-11-10/11, 805b8e3)
- Source : INTERFACE 138, l. 10153 (liste l. 10230) ; JOURNAL l. 941-961.
- Statut : jamais confirmé : Anthony a essayé l'exe qui le contenait (titre de 139 : « après l'essai des parties 137 et 138 ») sans
  rien dire de ces gestes ; « aucun geste fait à la main dans le vrai exe » côté Claude.
- Manipulations :
  1. Timeline du Storyboard : prendre une clé par son milieu, la lâcher sur la seconde moitié de sa voisine → trait d'insertion
     pendant le geste, la clé passe après ; sur la première moitié de la première → en tête.
  2. Lâcher une clé sur des images vides → elle glisse avec la suite (ancien geste).
  3. Plan à trois calques de longueurs différentes, tirer la poignée de durée → les trois tiennent leur dernier dessin jusqu'au bout.
  4. Clic droit sur des clés > « Modifier la durée des clés d'images… », 12, Entrée ; puis une valeur qui dépasse la fin → le plan
     s'allonge, le suivant se décale ; Cmd+Z rend tout.
  5. Trois plans de durées différentes > « Changer la durée des plans choisis… » → champ vide « durées différentes », taper 2:12.

#### Partie 137 — Calques dans la timeline : dossiers en lignes fines, plans moins hauts, zoom (2026-11-11, c410030 ; 137.11 49472b8)
- Source : INTERFACE 137, l. 10048 (liste 137.8 l. 10099, 137.11 l. 10143) ; JOURNAL l. 920-955.
- Statut : vu en partie : Anthony a utilisé l'exe de midi le 11/11 (quatre retours, aucun sur l'affichage lui-même). 137.1 et 137.5
  « trois calques » retirés (165.7) ; colonne refaite par 160/174/178/192 ; filet dans le panneau Calques (137.11) caduc (160).
  Restent à voir : dossier fermé traversé par « calque du dessus/dessous » puis refermé, clic sur la ligne fine d'un dossier, filet de
  couleur du dossier distinct de la couleur du type, plans moyens/en bandeau, zoom de la bande sous le pointeur, F5 multi (137.11).
- Manipulations :
  1. « Calque du dessus » répété jusqu'à un dossier fermé → il s'ouvre « de passage », se referme à la sortie, rien d'enregistré.
  2. Clic sur la ligne fine d'un dossier → il se ferme/s'ouvre, ne devient jamais le calque actif.
  3. Pointeur sur la timeline, touches 2 et 1 (zoom de la vue) → la timeline zoome ; pointeur sur l'image → l'image zoome.
  4. Cmd+clic sur un second nom, clic dans la piste de l'un, fn+F5 → « Nouvelle clé sur 2 calques » ; double-clic dans la piste d'un
     calque choisi → une clé sur chacun.

#### Partie 136 — Réglages, effets et clés d'effets sur tous les calques choisis (2026-11-10, d3662ef)
- Source : INTERFACE 136, l. 10001 (liste l. 10024).
- Statut : jamais vu. Panneau Calques retiré (160) : œil, cadenas, pelure, pochoir, mode sont sur les lignes de la timeline.
- Manipulations :
  1. Trois calques choisis, clic sur l'œil de l'un → les trois se cachent ; sur un calque hors du choix → lui seul. Pareil cadenas, pelure, pochoir, mode.
  2. Un vectoriel et un bitmap choisis, Tremblement du trait → barre d'état : bitmap laissé de côté.
  3. « Flou sur le calque » puis un réglage dans le panneau Effets → les deux changent ; « Créer une clé » → une sur chacun.
  4. Un calque à trajectoire + un autre, Retourner → chemin et mouvement retournés ensemble.

#### Partie 135 — Écriture continue et jalons (2026-11-10, 6093d46)
- Source : INTERFACE 135, l. 9910 (liste l. 9987) ; JOURNAL l. 1106-1129.
- Statut : jamais vu par Anthony (« Il n'a pas essayé la version portable avant » ; seul Claude a lancé l'exe sur une copie).
- Manipulations (sur une copie de projet) :
  1. Dessiner, lever le stylet → « Enregistré » en bas à droite ; fermer la fenêtre (rouge) → aucune question ; rouvrir → le trait est là.
  2. Dessiner puis Forcer à quitter (Cmd+Option+Échap) → rouvrir : tout est là sauf au plus le dernier instant.
  3. Cmd+S → « Jalon posé à … », « Nommer… », taper un nom, Entrée.
  4. Gribouiller, Fichier > Jalons…, ton jalon, « Revenir à ce jalon… », « Revenir » → projet comme au jalon ; « Avant le retour à … » garde le gribouillage.
  5. Hachurer vite 20 s → aucune saccade ; Préférences > En cas de plantage, décocher « Enregistrer en continu » → Cmd+S enregistre, fermer redemande.

#### Partie 134 — Ctrl+A dans les Vignettes (2026-11-10, d07678e)
- Source : INTERFACE 134, l. 9850 (liste l. 9881).
- Statut : jamais vu.
- Manipulations : Vignettes, plan à trois calques, Cmd+A → seuls les traits du calque courant et du papier, la boîte entoure tout ;
  un trait case → papier → case voisine : Cmd+A puis Cmd+glisser dans la boîte → il bouge entier, un Cmd+Z ; calque verrouillé → la
  barre d'état le dit ; calque bitmap → les pixels sur toute la planche.

#### Partie 133 — « Peint sur l'aplat » ne piège plus la planche (2026-11-10, cc9e505)
- Source : INTERFACE 133, l. 9784 (liste l. 9839) ; JOURNAL l. 1086-1103.
- Statut : défaut vu par Anthony (sa vidéo), correction jamais revue par lui.
- Manipulations : allumer « Peint sur l'aplat » (2e bouton sous l'aperçu du pinceau) → pastille dans la barre du bas de la planche,
  un clic l'éteint ; rallumé : dessiner sur le papier → le trait reste ; dans une case pâle → aucun plan ne naît, message ; dans une
  case hors de la forme → rien ne reste, Cmd+Z n'a rien à défaire ; dans la forme d'un personnage → la peinture va dans son aplat.

#### Partie 132 — Sélection, Gomme, Déformer, Transformer sur plusieurs calques et clés (2026-11-10, 00a047e)
- Source : INTERFACE 132, l. 9563 (liste l. 9757).
- Statut : jamais vu (F5 multi a été essayé par Anthony et a échoué, corrigé en 137.11 et 139.5, non revu). Panneau Calques retiré (160).
- Manipulations :
  1. Un vectoriel et un bitmap choisis (Cmd+clic dans la colonne), Gomme en travers → les deux gommés ; un Cmd+Z rend les deux ; l'un verrouillé → nommé, intact.
  2. Déformer, mode Perspective, deux calques → un seul cadre ; tirer un coin → les deux suivent ; pareil en Enveloppe.
  3. Transformer, deux calques, tirer le milieu du côté droit → les deux s'élargissent ; l'un tourné → refus motivé.
  4. Rectangle sur plusieurs clés de deux calques, gommer → barre d'état « Gomme sur N dessins » ; plus de 24 clés → bandeau rouge pendant le geste.
  5. Clés choisies, Suppr → dessins vidés, clés gardées ; clic droit > « Supprimer les clés » → elles partent.
  6. Trois calques choisis : opacité glissée → les trois ; Vider le dessin, Retourner le calque → les trois.

#### Partie 131 — La caméra se souvient (règle A) (2026-11-09/10, 35a6e42)
- Source : INTERFACE 131, l. 9406 (liste l. 9537) ; JOURNAL l. 1081-1085, 1139-1159 ; suivi/CAMERA-SOUVIENT.md.
- Statut : vu en partie : Anthony a essayé la version portable du 9/11 et donné son accord de fusion (JOURNAL l. 1081) ; son retour
  du lendemain (133) prouve qu'il a dessiné sur la planche. Les 15 points de la liste ne sont pas dits vus un par un.
- Manipulations (Vignettes) :
  1. Grand trait de la case 1 à la case 4 → entier sur la planche ; plan 1 au Storyboard : le trait, coupé au cadre ; plans 2-4 vides.
  2. Passe-partout → chaque case ne montre que son plan ; l'enlever → le trait revient.
  3. Cmd+lasso là où le trait traverse la case 3 → pris entier ; le glisser d'une case → il passe au plan où son début arrive ; sur le papier → au papier.
  4. Gomme posée sur le papier, sur le bout qui dépasse → vectoriel : le trait part entier ; bitmap : seulement sous la gomme ; un Cmd+Z.
  5. Trait dans la marge de la première case pâle → elle naît ; bout d'un trait qui finit dans une case pâle → elle ne naît pas.
  6. Export de la planche (PDF/PNG) avec et sans passe-partout → comme à l'écran ; Décor à travers une continuité → chaque case reçoit sa part, rien ne s'étale.

#### Partie 130 — Menus plus courts : exports en quatre sous-menus (2026-11-08, bcd4ff7)
- Source : INTERFACE 130, l. 9265.
- Statut : jamais vu par un humain (seulement « captures à regarder » d'un essai). Les choix de rangement étaient « à valider ».
- Manipulations : Fichier > Exporter en vidéo / image / le son / pour la production → chaque entrée ouvre sa boîte directement ;
  Aide n'a plus « Raccourcis clavier… » (F1 ouvre toujours les raccourcis) ; Préférences > Dessin > « Stylet à ma main » et le banc du
  stylet ; Réglages > Transformer : x, y, l, h, angle sur une ligne, bulles des poignées après 1 s.

#### Partie 129 — La clé de caméra qu'on pousse ; effets de caméra vivants (2026-11-08, 008de95, format 52)
- Source : INTERFACE 129, l. 9305 (liste l. 9389).
- Statut : jamais vu (« Pas vérifié : à l'écran, à la main »). La partie 2 (effet vivant, un par plan, liste de l'outil Caméra) est
  remplacée par les blocs des préréglages (170-172 : plusieurs blocs, visibles dans la timeline) ; reste à voir surtout 129.1.
- Manipulations :
  1. Plan avec une clé sur sa dernière image : tirer la clé d'avant vers la droite → elle avance, la clé de fin reste au bout.
  2. Plusieurs clés après elle, tirer loin → elles se tassent contre la fin, une image chacune ; barre d'état « la fin du plan l'arrête ».
  3. Maj+tirer une clé par-dessus une autre → elle passe, ne s'arrête jamais dessus ; Cmd+Z après chaque geste.
  4. Un plan à effet de caméra : le couper en deux, le réunir au suivant, rogner le début avec Cmd → rien ne bouge à l'image ; Cmd+Z rend l'effet vivant.
  5. Outil Caméra sur un plan à forte secousse → le coin du cadre s'attrape là où on le voit.

#### Partie 127 — Couche bitmap de la planche, sélection bitmap du trait entier (2026-11-08, 6f27220)
- Source : INTERFACE 127, l. 9051 (liste l. 9183).
- Statut : en partie remplacé par 131 (plus de répartition : un trait bitmap d'une case garde ses pixels dans le dessin du plan).
  Reste, jamais vu : la couche bitmap du papier (trait commencé hors des cases), Cmd+lasso/clic de la planche en bitmap, Suppr/C/X/V,
  la limite de 7 680 tuiles (vignetteselbmp.rs:40) et son message, l'export de la planche.
- Manipulations :
  1. Calque bitmap, trait commencé sur le papier à travers des cases → même grain partout, rien ne disparaît au lâcher.
  2. Cmd+lasso autour du bout sur le papier → boîte autour du trait entier ; Cmd+glisser → aucun trou, rien à l'ancienne place ; un Cmd+Z.
  3. Coin de la boîte (échelle), juste hors d'un coin (rotation) → trait entier, un peu plus doux.
  4. Suppr → tout part ; Cmd+C, Suppr, Cmd+V → il revient à sa place.
  5. Très grande sélection (beaucoup de cases pleines) → « Sélection trop grande… : seule la part de cette case est prise » ; dire si l'attente au lâcher (jusqu'à ~3-5 s) va.
  (Point 9, ouvrir dans une version d'avant, et 12, film partagé à plusieurs postes : hors portée du testeur seul.)

#### Partie 125 — La sélection de la planche prend le trait entier (vectoriel) (2026-11-08, 6bcade3)
- Source : INTERFACE 125, l. 8964 (liste l. 9039).
- Statut : en partie remplacé par 131 (plus de parts ni de numéro pour les traits neufs). Reste, jamais vu : Cmd+lasso/Cmd+clic de
  la planche en vectoriel, gestes de la boîte, Suppr/C/X/V/recolorer, refus au-dessus d'un calque verrouillé, anciens traits coupés.
  Les points 1-6 recoupent la liste de 131 (point 5).
- Manipulations : Cmd+clic sur un trait → pris ; Suppr → tout part, Cmd+Z ; Cmd+C, Suppr, Cmd+V → il revient ; déplacer un trait vers
  la case d'un plan au calque verrouillé → il reste par-dessus, la ligne d'état le dit ; ancien projet → ses traits déjà coupés se
  prennent ensemble et redeviennent entiers une fois déplacés.

#### Partie 124 — Licence ffmpeg dans À propos, trois touches (2026-11-08, 7c7df04)
- Source : INTERFACE 124, l. 8918 (liste l. 8948) ; JOURNAL l. 1195-1209.
- Statut : jamais vu (fusionné sur « Fusionne finitions » après l'essai de la Trajectoire, sans essai de ces points). Touches
  toujours dans keymap.toml : Ctrl+Maj+D (l. 795-797), Maj+C (l. 1908-1910), Maj+K (l. 1934-1936).
- Manipulations :
  1. Menu FEANOR/Aide > À propos de FEANOR… → ligne ffmpeg sous les polices ; déplier « Licence et sources de ffmpeg » → le ffmpeg du poste, la licence qui défile, la configuration.
  2. Cmd+Maj+D sur un plan → dupliqué ; Maj+C → outil Caméra ; Maj+K → losange sur la piste caméra à la tête de lecture.
  3. Séquence d'un seul plan avec une version de séquence rangée : Suppr sur le plan → refusé, la ligne d'état dit pourquoi.
  (Bouton « Ouvrir le dossier ffmpeg-licence » : version portable Windows seulement.)

#### Partie 123 — Durée d'un plan : Ctrl fait suivre les clés ; clés alternées (2026-11-07, 760cba4)
- Source : INTERFACE 123, l. 8842 (liste l. 8896) ; ETAT.md l. 4.
- Statut : jamais vu (« Fusionnés, en attente du test d'Anthony », ETAT l. 4). Sans touche, « étirer » tient maintenant le dernier
  dessin de chaque calque (138.2).
- Manipulations :
  1. Plan de 4 s, clés à 0, 1, 2, 3 s : tirer le bord droit jusqu'à 8 s en tenant Cmd → clés à 4, 5, 6, 7 s, la barre d'état le dit.
  2. Pendant le geste, lâcher puis reprendre Cmd → les clés vont et viennent ; revenir au point de départ → rien dans l'historique.
  3. Avec Cmd, raccourcir → premières clés en réserve (« N images cachées au début ») ; rallonger avec Cmd → elles reviennent.
  4. Cmd+Z → une étape ; Cmd+Maj+Z (ou Cmd+Y) → l'après ; clés de position et d'opacité suivent, courbe de caméra sans saut.
  5. Pistes de calques (thèmes clair et sombre) → chaque clé de dessin se distingue de sa voisine (teinte alternée), stable au défilement.

#### Partie 122 — Le trait qui traverse une case qui ne peut pas le recevoir (2026-11-07, 87abe71)
- Source : INTERFACE 122, l. 8755 (liste l. 8831) ; ETAT.md l. 4.
- Statut : jamais vu ; en grande partie remplacé par 131 (un trait ne va plus dans les cases qu'il traverse). Restent : la part
  menée par la sélection au-dessus d'une case refusée (131 l. 9468), la gomme sur cette part, « Tous les plans sont écartés »,
  l'export des plans à cadres tournés.
- Manipulations :
  1. Écarter tous les plans → la timeline et les panneaux disent « Tous les plans sont écartés. » ; dessiner au Storyboard → rien ;
     changer la résolution du projet → marche ; tracer dans une case pâle → un plan naît.
  2. Écarter un plan, déplacer au lasso un trait sur sa case, reprendre le plan → le trait reste visible par-dessus ; export : absent tant que le plan est écarté.
  3. Plan à cadres A/B tournés, export PNG/PDF de la planche → la case montre la feuille comme l'écran.

#### Partie 121 — Les effets continuent au-delà du bord de la feuille (2026-11-07, 8478aec)
- Source : INTERFACE 121, l. 8614 (liste l. 8742) ; ETAT.md l. 4.
- Statut : jamais vu (« en attente du test d'Anthony »).
- Manipulations :
  1. Longs traits qui passent le bord de la feuille ; poser Onde, Secousse, Tourbillon, Vignettage, Aberration ; choisir un autre
     calque ; regarder la verticale du bord → ni saut ni changement de couleur.
  2. Pointillés, Crayonné, Respiration ; Écriture à 70 % puis 100 % → le trait s'écrit jusqu'au bout, au-delà de la feuille.
  3. Mosaïque en grande taille → carrés continus à travers le bord ; tremblement 0,5 puis 3, zoom fort sur le bord → plus de colonne claire.
  4. Tirer le curseur d'un flou sur un calque qui dépasse → pas d'accroc, l'effet hors feuille arrive juste après.
  5. Lire un plan animé dont les dessins dépassent, avec flou → pas de clignotement hors feuille ; exporter film, images, planche → effet complet.

---

### Parties 76 à 119 (lot 2)

#### Partie 126 — Planche-mère : calque partagé entre plans (2026-11-08, aa03f7f)
- Source : INTERFACE partie 126, ligne 7070 ; JOURNAL l. 1180-1194
- Statut : jamais vu (fusionné sur « Oui, fusionne » sans essai décrit). Le maillon est maintenant sur la ligne du calque de la
  timeline (160, 174 : plus de panneau Calques). Partie Vignettes (maillon sur les cases) annoncée « chantier suivant ».
- Manipulations :
  1. Un plan avec un grand décor qui déborde de la feuille ; menu Storyboard > Plans > « Réutiliser le plan » → un plan neuf juste
     après, même dessin ; déplacer sa caméra ; dessiner un détail dans l'un → il apparaît dans l'autre.
  2. Clic droit sur la ligne d'un calque dans la timeline > « Réutiliser dans… » > « Le plan suivant », « Tous les plans de la
     séquence », « Des plans choisis… » (cocher, Appliquer) → maillon sur la ligne, bulle qui nomme les plans.
  3. « Détacher (garder une copie) » puis dessiner : les autres plans ne changent plus ; « Détacher le plan » ; Cmd+Z à chaque fois.
  4. Transformer un exemplaire : il ne bouge que dans ce plan ; allonger le plan : le dessin tient jusqu'au bout.
  5. Couper un plan lié (les deux parties restent liées), réunir (le second se détache), dupliquer (copie non liée).
  6. Sur un dossier ou un calque de couleur, « Réutiliser dans… » dit pourquoi non.

#### Partie 119 — Outil Trajectoire (2026-11-07, 3a6cfc0 ; ajouts 2026-11-08 7c7df04 et 2026-11-18 223054b)
- Source : INTERFACE partie 119, ligne 8364 ; section « Une marque qu'on glisse emporte son temps » ligne 8476 ; JOURNAL l. 1195, 1215, 441
- Statut : vu en partie. Vu et validé par Anthony : tracé, marques, clés d'images, ce qui suit le chemin, recalage sur la timeline,
  rotation avec le chemin et décalage à l'insertion d'images (« parfait » avant la fusion de `finitions`), puis retours de la partie
  152. Reste jamais vu : la section du 18 novembre (« Pas regardé sur son écran », ligne 8534).
- Manipulations (section du 18 novembre) :
  1. Calque dessiné, outil Trajectoire, clés d'images : glisser une marque le long du chemin vers la suivante → au lâcher, dans la
     timeline, le dessin d'avant s'allonge et le sien raccourcit ; la barre d'état dit « Marque à N % du chemin, image M » ; la
     ramener rend l'exposition ; Cmd+Z.
  2. Réglages de l'outil : « Temps selon le chemin » → expositions proportionnelles à l'espacement ; « Temps égaux entre les
     marques » → toutes pareilles.
  3. Poser une nouvelle marque quand des temps sont déjà posés → elle prend le temps de sa place.
  4. En clés d'interpolation : glisser une marque ne change pas les images des jalons.
  (Le projet « dd » cité est sur le PC d'Anthony.)

#### Partie 118 — Traits coupés au bord de la feuille, plan à mouvement de caméra (2026-11-07, 6a06b1c)
- Source : INTERFACE partie 118, ligne 8312
- Statut : jamais vu ; « Le stylet réel n'a pas été essayé » (l. 8362). En partie remplacé par 121 (`fxmonde.rs` remplace la bande
  de 4 tuiles) ; reste le résultat visible à contrôler.
- Manipulations :
  1. Plan à mouvement, cadre B poussé loin à droite ; longs traits à travers A et B jusqu'au bout de B → au repos rien n'est coupé,
     dans les Vignettes et au Storyboard.
  2. Tracer dans la zone puis lever le stylet → rien ne disparaît après le lâcher.
  3. Refaire avec B à gauche, en haut, en bas, avec un cadre C ; Préférences > Dessin « tremblement du trait » allumé.
  4. Export vidéo, images, planche : le cadre B montre le trait.

#### Partie 117 — Poignées de lissage de la caméra sur la piste (2026-11-06, 6a06b1c)
- Source : INTERFACE partie 117, ligne 8253
- Statut : jamais vu (« non vérifié avec un vrai stylet », l. 8304). En partie remplacé par 120 (poignées au repos dès qu'un
  mouvement existe, plus besoin d'« Adoucir » avant).
- Manipulations :
  1. Plan à deux clés de caméra : deux petits ronds pâles à côté des clés dans la timeline → tirer celui de droite de la 1re clé :
     la caméra ralentit au départ, en direct ; la barre d'état dit la durée en images.
  2. Tirer jusqu'à la clé voisine → arrêt avant elle ; ramener sur la clé → retour au repos ; Échap pendant le geste → tout revient.
  3. Bouton « Adoucir » de la barre caméra : pose les deux côtés ; second clic : retire.
  4. Faible zoom de timeline : un appui sur la clé prend la clé, pas la poignée. Même chose dans le Montage.

#### Partie 116 — Le « + » de la peau de Maj : un cadre de plus (2026-11-06, 6a06b1c)
- Source : INTERFACE partie 116, ligne 8194
- Statut : jamais vu explicitement. Règle de la case précisée par 142.6 (le « + » sert la case survolée, un mouvement vit sur la case
  où il commence) ; Anthony a fait des mouvements B vers C ensuite (JOURNAL l. 1065), sans dire par quel geste.
- Manipulations :
  1. Vignettes, tenir Maj sur une case → petit « + » à côté du dernier cadre ; relâcher Maj → plus rien. Pareil avec T tenue et
     l'outil Caméra (Maj+C).
  2. Clic sur le « + » → cadre B (puis C, D) ; Cmd+Z le retire en une étape.
  3. Glisser depuis le « + » → le cadre naît sous le pointeur ; Échap abandonne ; revenir au départ = clic.
  4. Vérifier que glisser dans B déplace B, Maj+glisser une case déplace le plan, Maj+clic sur une barre coupe.

#### Partie 115 — Le plan écarté (2026-11-06, 6a06b1c, format 48)
- Source : INTERFACE partie 115, ligne 8101
- Statut : jamais vu (JOURNAL l. 1263 « Reste : … plan annulé »).
- Manipulations :
  1. Vignettes : clic sur le numéro d'un plan → case pâlie, croix rouge, numéros inchangés ; Storyboard et Montage : le plan
     n'y est plus ; lecture : il est sauté ; reclic → il revient.
  2. Poser le stylet sur le numéro et tracer → un trait, rien n'écarté ; une pose nette → écarté sans point de trait.
  3. Touche N (numéros cachés), puis Maj tenue → numéros visibles ; Maj+clic sur le numéro → écarte ; Maj+clic ailleurs → choisit.
  4. Écarter tous les plans → le Storyboard dit « Tous les plans sont écartés. », Fichier > Exporter… refuse ; un trait dans une
     case pâle refait un plan.
  5. Écarter un plan au milieu d'une piste son → l'image saute, le son reste en place.
  6. Clic droit sur une case > Plus > « Écarter ce plan » / « Reprendre ce plan ».

#### Partie 114 — Le stylet : banc « Ce que le stylet transmet » et stylet branché (2026-11-06/07, 547b51f et 9819707)
- Source : INTERFACE partie 114, lignes 8000 et 8069 ; JOURNAL l. 1242-1249
- Statut : banc vu (relevé d'Anthony, 28 513 messages) ; « branché » (bouton = clic droit, stylet retourné = Gomme) jamais vu
  (« Reste son essai à la tablette »). **Inapplicable sur Mac** : l'écoute est en `cfg(all(windows, 64 bits))` (`src/stylet.rs` l. 369,
  543) ; le banc s'ouvre et dit que l'écoute est refusée. Le banc est maintenant dans Préférences > Dessin > « Stylet à ma main » (130).
- Manipulations (Mac) :
  1. Préférences > Dessin > Stylet à ma main > « Ce que le stylet transmet… » → la fenêtre dit que l'écoute est refusée, sans planter.
  2. Avec une tablette sur Mac : bouton latéral réglé en « clic droit » dans le pilote → clic bref = réglages de l'outil, glisser = anneau.
  3. Retourner le stylet (gomme au dos) → noter ce qui se passe (rien n'est prévu sous macOS).

#### Partie 113 — Vignettes plus simples (2026-11-06, f5caddd)
- Source : INTERFACE partie 113, ligne 7848 ; suivi/ANALYSE-VIGNETTES.md
- Statut : décisions validées par Anthony sur l'analyse (pas à l'écran) ; jamais vu à l'écran. En partie remplacé par 131 (point 12
  « un bout de trait ne fait que la case suivante » : c'est maintenant la marge de la 1re case pâle qui la fait naître), par 160
  (panneau Planche) et 198 (onglet 3D).
- Manipulations :
  1. Pont : Vignettes, Maj+Tab → dessin du Storyboard, même plan ; Maj+Tab → la planche. Vignettes, Tab → animation du plan,
     bouton « Retour aux Vignettes » ; Tab → retour.
  2. Outils refusés : dans les Vignettes, touches d'outils (pot, sélection, texte, formes), anneau vers « Remplir » → rien ne change,
     la ligne d'état dit « pinceau, crayon, gomme et caméra » ; Cmd+glisser dans une case → le lasso.
  3. Maj tenue : filets bleus et poignées aux coins, barres entières ; Maj+glisser depuis le milieu d'une case → le plan bouge ;
     Maj+coin → la caméra.
  4. Barre entre deux cases : point gris ; poser le stylet sur la barre et tracer → un trait continu ; clic → coupe sans point de trait.
  5. Cocher Action et Dialogue ; clic sur la ligne Dialogue sous une case → s'écrit, Entrée ; Cmd+Z.
  6. Long projet : « Cadrer tout » (barre du bas, à droite) → toute la planche (jusqu'à 20 %) ; « Plan suivant » → la planche suit,
     liseré d'accent sur le plan choisi.

#### Partie 112 — Mes habitudes et notes de friction F9 (2026-11-06, 547b51f)
- Source : INTERFACE partie 112, ligne 7785 ; AUDIT-2026-11-10 (« F9 lancé trois fois le 8 novembre, dossier Frictions vide »)
- Statut : compteur vu en partie (ses données lues pour l'audit) ; fenêtre Aide > Mes habitudes et notes F9 jamais confirmées, F9
  peut-être défaillant (voir B).
- Manipulations :
  1. Travailler quelques minutes, Aide > Mes habitudes… → commandes avec leur chemin (clavier, icône, anneau) ; tenir Cmd+Z → +1 par appui.
  2. Clic droit sur une ligne → « Assigner un raccourci… », « Envoyer dans l'anneau ».
  3. En plein dessin, F9 (Fn+F9) → champ « Qu'est-ce qui t'a freiné ? » en haut ; taper, Entrée → la barre d'état donne le fichier.
     Vérifier que `Documents/SHOT/Frictions/frictions.md` et une PNG existent vraiment (Finder).
  4. F9 puis Échap → rien de neuf ; F9 puis Entrée sans texte → note « (sans texte) ».
  5. Aide > Mes frictions notées… → sur Mac n'ouvre rien (voir B).

#### Partie 111 (anneau) — L'anneau sous le stylet (2026-11-06, ef983ac)
- Source : INTERFACE partie 111 « L'anneau », ligne 7623 ; AUDIT-2026-11-10 (anneau : 16 commandes)
- Statut : vu en partie (utilisé 16 fois par Anthony). Second anneau des pastilles, Préférences > Anneau, « Envoyer dans l'anneau »
  jamais confirmés.
- Manipulations (souris : clic droit tenu) :
  1. Clic droit bref sur le dessin → réglages de l'outil ; avec la Sélection → son menu.
  2. Clic droit tenu, glisser vers le bas, relâcher vite → Gomme, nom affiché un instant ; haut → Crayon ; droite/gauche → plan suivant/précédent.
  3. Clic droit tenu sans bouger → anneau après 180 ms ; relâcher au centre → réglages ; Échap → rien.
  4. Vers haut gauche puis plus loin → second anneau des pastilles ; relâcher → pastille choisie.
  5. Préférences > Anneau : changer une direction, choisir une commande déjà présente (échange), délai 0 puis 1000 ms ; décocher.
  6. Clic droit sur une icône d'outil > « Envoyer dans l'anneau » > Haut → la barre d'état le dit.

#### Partie 110 — Passe-partout qui suit la caméra, numéros dans la marge, fine flèche (2026-11-06, fcd894d)
- Source : INTERFACE partie 110, ligne 7516 ; partie 118 l. 8352 (badge 12)
- Statut : vu en partie (numéros dans la marge visibles sur sa capture, partie 118). Le passe-partout a changé de sens en 131 (« il
  montre le film : chaque case son plan ») ; la zone balayée reste à voir.
- Manipulations :
  1. Plan A→B en diagonale, passe-partout allumé (barre du bas) → tout l'espace entre A et B ouvert, dans le vrai dessin ; A, B, C
     en L → le coin jamais filmé reste couvert.
  2. Numéros à 28 %, 50 %, 100 % → jamais sur une image, un cadre ou la zone balayée d'un voisin.
  3. Touche N → cache/remontre ; relancer → retenu ; export de la planche sans numéros quand cachés.
  4. Flèche fine A→B→C, couleur des cadres, lisible sur blanc et gris.
  5. Fichier > Exporter en image > Planche des vignettes, « Passe-partout » coché puis décoché.

#### Partie 109 — Sortie dédiée au color script (2026-11-06, 7cac5ed)
- Source : INTERFACE partie 109, ligne 7483
- Statut : jamais vu.
- Manipulations :
  1. Fichier > Exporter en image > Color script : présentations Planche, Bande continue, Bande de couleurs seule.
  2. Image d'un plan : première, milieu, moyenne, première clé → quatre rendus différents sur un plan qui bouge ; « Arrêter » ne laisse rien.
  3. Sorties : un PNG, un PNG par séquence (dossier), PDF A3 puis A4 (bloc de titre, version, date, accents).
  4. Repères (numéros, minutage) ; Couleurs en CSV puis JSON (ouvrir dans un tableur / un éditeur).
  5. Paquet de livraison : case « Le color script (PNG et couleurs en CSV) » décochée d'origine ; cochée → deux fichiers et manifeste.

#### Partie 108 — Planche de références : canevas blanc et crayon (2026-11-06, 163356d)
- Source : INTERFACE partie 108, ligne 7411
- Statut : jamais vu.
- Manipulations :
  1. Ouvrir le panneau Références (Affichage > Panneaux ou « + Panneau » de la colonne, la barre des panneaux n'existe plus depuis 194) →
     fond blanc dans chaque thème, détaché et plein écran.
  2. Icône crayon : tracer à la souris puis au stylet (la pression amincit), changer d'encre et de taille ; molette, Espace.
  3. Gomme sur des traits qui croisent images et post-it ; Cmd+Z / Cmd+Maj+Z.
  4. Sélection : cliquer un trait, le déplacer, changer son encre, Suppr ; Échap rend la sélection.

#### Partie 107 — Brosses dédiées : nom et « avec quoi elles dessinent » (2026-11-05, cfb5e85)
- Source : INTERFACE partie 107, ligne 7352
- Statut : jamais vu.
- Manipulations :
  1. Affichage > Panneaux, Cmd+K : « Brosses dédiées », plus de « perso ».
  2. Régler le pinceau (feutre, 30 px), choisir une pastille Perso 1 → le pinceau ne change pas ; lâcher → intact.
  3. Mettre un décor en « Brosse spécifique » (menu de brosses, Pinceau|Crayon, taille, texture) → petit signe sur la pastille,
     bulle « Dessine avec : … ».
  4. Choisir ce décor, dessiner, le lâcher → le pinceau d'avant revient tel quel ; changer sa taille aux crochets → gardée pour lui seul.
  5. Personnage en « Brosse spécifique », fermer, rouvrir → gardé ; Cmd+Z défait le changement de mode.

#### Partie 106 — Textes sous les cases, renommer les séquences, séquences nommées d'après le scénario (2026-11-05, 9169a0e)
- Source : INTERFACE partie 106, ligne 7308
- Statut : jamais vu.
- Manipulations :
  1. Cocher Dialogue et Notes (panneau Planche), aller au Montage, quitter, relancer → lignes encore sous les cases.
  2. Taper un premier dialogue dans la fiche d'un plan → la ligne se coche, le texte est sous la case.
  3. Plusieurs séquences : survol du titre (bulle), double-clic, clic droit > « Renommer la séquence… », Entrée, Cmd+Z.
  4. Une seule séquence : Cmd+K « Renommer la séquence » → petite fenêtre ; idem depuis la barre des séquences du Montage.
  5. Importer un .fountain puis « Créer les plans depuis le scénario… » → séquences « 12. INT. JOUR - CUISINE ».

#### Partie 105 — Images à la clé, son par plan, nomenclature (2026-11-05, c9e076f)
- Source : INTERFACE partie 105, ligne 7433
- Statut : jamais vu.
- Manipulations :
  1. Fichier > Exporter en image > « une image par dessin clé » sur un plan à dessins tenus → « N clés dans M plans », fichiers `…_K01`.
  2. Cocher « Une image par calque » → JPEG grisé, fichiers `…_K03_Calque.png` transparents.
  3. Fichier > Exporter le son > un son par plan, WAV puis AIFF 24 bits ; deux fichiers voisins bout à bout sans trou.
  4. Nomenclature : cliquer chaque forme → aperçu ; « Personnalisée » `{plan:03}_{cle}_{calque}` puis retirer `{cle}` → message rouge,
     « Exporter… » grisé.
  5. « Arrêter » en route → dossier vide.

#### Partie 104 — Montage qui ne casse rien, corbeille, J K L (2026-11-05/06, 7a0c6ad, format 47)
- Source : INTERFACE partie 104, ligne 7561 ; AUDIT-2026-11-10 (montage jamais touché)
- Statut : jamais vu.
- Manipulations :
  1. Montage : L → lecture, L encore → x2 (barre d'état), K → arrêt ; J → lecture arrière jusqu'au début du film.
  2. Avec une piste son : x1 avec son, x2 sans son, retour à x1 → le son reprend.
  3. K tenue + L, L, J → une image par appui. Taper un nom de plan contenant J, K, L → rien ne se lance. Storyboard : L = Sélection.
  4. Supprimer un plan puis un calque → « envoyé à la corbeille (N dessins) » ; Cmd+Z rend sans double.
  5. Fichier > Corbeille… → Restaurer (à sa place), Supprimer… et Vider demandent confirmation et ne s'annulent pas ; relancer → la
     corbeille est là.
  6. Option+glisser une clé sur un autre dessin → le dessin recouvert va à la corbeille.

#### Partie 103 — L'aplat fiable (2026-11-05, c49d65b)
- Source : INTERFACE partie 103, ligne 7279
- Statut : jamais vu.
- Manipulations :
  1. Calque à aplat, pastille de personnage : très gros trait puis un autre aussitôt, Cmd+Z deux fois → plus d'aplat ; Cmd+Maj+Z deux
     fois → les deux reviennent.
  2. Trait puis Cmd+Z immédiat → le trait part ; rétablir → il revient avec son aplat.
  3. Option « La clé » (changer la couleur d'aplat d'une clé), retoucher sans pastille → même couleur.
  4. Copier une clé du trait, la coller plus loin (Cmd+V) → la couleur suit.
  5. Trait avec pastille puis aussitôt couper le plan (F5) ou « Nouvelle version de séquence » → l'aplat est dans les deux.

#### Partie 102 — Versionnage fiable (2026-11-05, 45a2146)
- Source : INTERFACE partie 102, ligne 7245 ; AUDIT-2026-11-10 (versions jamais touchées)
- Statut : jamais vu.
- Manipulations :
  1. Séquence à deux versions A, B, 4 plans ; dans B, « Nouvelle séquence avec les plans choisis » sur deux plans, passer à A → A a
     ses 4 plans.
  2. Séquence d'un plan avec version B : Suppr sur ce plan → refusé, la ligne d'état dit pourquoi.
  3. Plan avec V1 et V2 : F5 au milieu → fenêtre « Ce plan a 1 version rangée. Elle va avec : » ; « La seconde partie » ; Cmd+Z ;
     puis Annuler et Échap.
  4. Réunir deux plans qui ont chacun des versions → toutes gardées, sans numéro en double (bulle V2).
  5. Déposer une image entre deux images d'un plan à versions → même fenêtre, rien posé avant la réponse.

#### Partie 101 — Canevas libre des Vignettes et outil Caméra (2026-11-05, 92ab7a8)
- Source : INTERFACE partie 101, ligne 7226 ; JOURNAL l. 1277 (« coupe des captures 102 et 103 non reproduite »)
- Statut : en partie remplacé par 131 (le trait n'est plus partagé au lâcher entre cases : il appartient entier à la case du premier
  point, vu par Anthony). Reste jamais vu : aucun plan choisi d'office, outil Caméra de la colonne, couleur du débord.
- Manipulations :
  1. Ouvrir un projet → aucune case n'a de cadre noir ; dessiner → toujours rien.
  2. Maj+clic sur une case → cadre ; Échap ou clic sur le papier → il part.
  3. Outil Caméra (Maj+C) : survoler une case (cadres, poignées), cliquer (plan courant), glisser un cadre sans clic préalable, Cmd+Z ;
     reprendre le pinceau → plus de sélection.
  4. Un trait parti d'une case qui déborde → même couleur et opacité pendant et après le geste.

#### Partie 100 — Plus d'onglet Animation (2026-11-05, 9a9c77c)
- Source : INTERFACE partie 100, ligne 7207 ; JOURNAL l. 1285 (« double clic et bouton à essayer à la vraie souris »)
- Statut : jamais vu. Les onglets sont maintenant Vignettes, Storyboard, 3D, Montage (198, `sansonglet_tests.rs` l. 59).
- Manipulations :
  1. En haut à droite : pas d'onglet Animation.
  2. Double-clic sur un plan de la bande du Storyboard puis du Montage → animation de ce plan.
  3. Bouton « Retour au storyboard » à gauche des onglets → Storyboard, même plan, même venu du Montage.
  4. Double-clic sur un libellé de plan, une transition, une clé → leur geste, pas d'entrée en animation.
  5. Fermer et rouvrir un projet laissé en animation → bouton de retour présent.

#### Partie 99 — Gris sombre, thème de base, rehauts jade (2026-11-05)
- Source : INTERFACE partie 99, ligne 7195
- Statut : demandé sur capture d'Anthony ; le résultat jamais vu.
- Manipulations : lancer sans réglages (thème Gris sombre) ; regarder le trait jade sous l'espace actif ; Préférences > Apparence >
  Thème ; cliquer chaque sorte de calque dans la timeline (couleurs de 98). La pastille jade « non enregistré » est probablement rare
  depuis l'écriture continue (voir D).

#### Partie 98 — Sortes de calques en couleurs FEANOR (2026-11-05)
- Source : INTERFACE partie 98, ligne 7162
- Statut : vu en partie (couleurs gardées par Anthony sur sa capture, partie 99). Reste : nuancier « FEANOR », presse-papiers, À propos.
  « Panneau Calques » de la checklist : n'existe plus (160), regarder la timeline.
- Manipulations : panneau Couleur → nuancier nommé FEANOR ; Aide > À propos → plus de ligne « Anciennement » ; copier une zone, coller
  → c'est bien elle.

#### Partie 97 — Mémoire d'une longue séance (2026-11-05, 9c3e516)
- Source : INTERFACE partie 97, ligne 7060, suite et checklist déplacées l. 7140-7160 (sous le titre 126)
- Statut : jamais vu (JOURNAL l. 1297 « en attente de toi »).
- Manipulations (Moniteur d'activité à la place du Gestionnaire des tâches) :
  1. Ouvrir une copie d'un gros projet, noter la mémoire de FEANOR.
  2. Longue séance (dessiner dans dix plans, supprimer, dupliquer, annuler/rétablir) → rien ne disparaît, pas de saccade.
  3. Supprimer un plan puis annuler, aussi après vingt gestes → le plan revient avec ses dessins.
  4. Annuler cinquante fois d'un coup puis tout rétablir → état exact.
  5. Pause de dix secondes → mémoire pas plus haute ; rien ne disparaît.

#### Partie 96 — FEANOR : nom, couleurs, barres (2026-11-05, fbd6a98)
- Source : INTERFACE partie 96, ligne 7040
- Statut : en partie remplacé : logo Joyau remplacé par l'étoile (167) ; thème Atelier vu par Anthony (il a demandé le retour au gris,
  99). Reste : thème Papier, rouge réservé aux erreurs, barres d'avancement irisées.
- Manipulations : Préférences > Apparence > Thème « Papier » → lisible ; exporter un film → barre fine, colorée, qui glisse et s'arrête ;
  rien de rouge hors des erreurs.

#### Partie 95 — Épisode de 300 plans en mémoire (2026-10-31, 5f8c27a)
- Source : INTERFACE partie 95, ligne 6993
- Statut : jamais vu. Les remarques Vulkan / Direct3D ne valent pas sur Mac (Metal).
- Manipulations :
  1. Ouvrir une copie d'un gros projet → tous les plans ont vignettes et dessins ; mémoire dans le Moniteur d'activité.
  2. Sauter loin d'un plan à l'autre (Storyboard, Montage, Vignettes) → rien de vide, même un instant.
  3. Dessiner, changer de plan, revenir, annuler/rétablir → traits identiques ; supprimer un plan, attendre dix secondes, annuler.
  4. Marquer un envoi sur un gros film → la fenêtre reste utilisable, « état des plans en cours de relevé… ».
  5. Aide > Régler FEANOR pour cette machine… → ligne « Dessins en mémoire ».

#### Partie 93 — Pont Kitsu (2026-10-31, c0e8bba)
- Source : INTERFACE partie 93, ligne 6931 ; AUDIT-2026-11-10 (« Kitsu : un essai »)
- Statut : jamais vu sur une vraie instance (adresses de l'API « À CONFIRMER », l. 6948). Fenêtre ouverte une fois par Anthony.
  Demande un compte Kitsu et une instance de test (Docker local possible) : hors de portée d'un testeur sans instance.
- Manipulations (si instance de test) :
  1. Fichier > Exporter pour la production > Kitsu ; cocher « Activer le pont Kitsu », adresse, e-mail, mot de passe ; « Tester la
     connexion ». Sur Mac « Rester connecté » échoue (voir B).
  2. Production : « Lire les listes », projet, tâche Storyboard, statuts.
  3. « Aperçu » puis « Publier » ; republier → « Aperçu » à 0 partout.
  4. Commenter dans Kitsu, « Lire les retours » → note dans Commentaires (source Kitsu) ; relire sans doublon ; Cmd+Z.

#### Partie 92 — Journal des différences entre deux envois (2026-10-31, 634ffec)
- Source : INTERFACE partie 92, ligne 6866
- Statut : jamais vu (envois jamais touchés, AUDIT-2026-11-10).
- Manipulations :
  1. Copie d'un projet à deux envois : Cmd+K « journal » → côtés proposés ; changer De / À → résumé à jour.
  2. Retoucher un plan, attendre une demi-seconde → « État actuel » le liste modifié ; ajouter, supprimer, déplacer un plan.
  3. « Exporter le journal… » → barre, on peut dessiner ; « Arrêter » ne laisse rien.
  4. Ouvrir la page HTML et le PDF (accents, images avant/après).
  5. Fenêtre « Envois » : l'icône de chaque ligne ouvre le journal.

#### Partie 91 — Petits livrables (2026-10-31, 6fd6808, format 46)
- Source : INTERFACE partie 91, ligne 6809
- Statut : jamais vu.
- Manipulations :
  1. Réglages du projet : « Durée cible du film » 11:30, une cible par séquence ; Cmd+Z ; dépassement en rouge.
  2. Montage : barre de séquence « 0:42 / 0:40 (+2s 00) » qui suit quand on allonge un plan ; fiche du plan « Cible séquence ».
  3. Marquer un envoi, ajouter/supprimer/déplacer des plans, Fichier > Exporter pour la production > Rapport de numérotation → CSV et .txt.
  4. Storyboard PDF > Production : en-tête de studio, plans modifiés en rouge, page « Notes de révision ».
  5. Planche des vignettes en PDF : légendes et bloc de titre.
  6. N'importe quel export, projet ouvert → la boîte « Enregistrer » s'ouvre dans le dossier du projet.

#### Partie 90 — Noms de marques retirés, « Papier » (2026-10-30, c442f0e)
- Source : INTERFACE partie 90, ligne 6780
- Statut : jamais vu.
- Manipulations : onglet « Papier » sous la timeline ; Cmd+K « fond de la feuille » → « Papier : … » ; fenêtres d'import de brosses
  et de nuancier sans nom de logiciel ; Préférences > Raccourcis, survol → « Origine : … ».

#### Partie 89 — Premier contact : visite, Labo, vocabulaire (2026-10-30, 6040504)
- Source : INTERFACE partie 89, ligne 6654
- Statut : jamais vu. Un Mac neuf (aucun réglage FEANOR) est le bon cas pour la visite.
- Manipulations :
  1. Premier lancement sans réglages → à côté de « Rouvrir le dernier projet » : « Première fois sur FEANOR ? Visite rapide (2 min) ·
     Plus tard · Ne plus proposer » ; aucune autre bulle.
  2. « Visite rapide » → étape 1 sur 5 ; créer le projet → étape 2 ; boutons Vignettes, Montage, lecture, Exporter ; dernière étape
     « Régler FEANOR pour cette machine… ».
  3. Ouvrir les Préférences pendant la visite → la carte s'efface puis revient.
  4. « Plus tard » deux fois sur deux lancements → ne revient plus ; Aide > Visite rapide « Premier jour »… la relance.
  5. Pas de menu Labo ; Préférences > Général > « Afficher les fonctions expérimentales » → Labo apparaît.
  6. Cmd+K « propriétés du plan », « labo », « diagnostic » → trouvent les nouvelles commandes.

#### Partie 88 — Fil de décision par plan (2026-10-30, 6040504)
- Source : INTERFACE partie 88, ligne 6583
- Statut : jamais vu.
- Manipulations :
  1. Panneau Commentaires : compteurs « À faire », « Fait »… ; clic → cache ces notes dans la liste et sur la règle ; « Tout » remet.
  2. Statut « Refusé » → champ de raison obligatoire, « Refuser la note » ; Cmd+Z d'un coup.
  3. Onglet « Fil du plan » : notes V1 / V2 enchaînées ; répondre (Entrée).
  4. Deux notes « En conflit avec… » → éclair rouge, pastille du plan rouge.
  5. Cmd+K « Note suivante à traiter » → va au plan, boucle, « Plus aucune note à traiter ».
  6. Retake sur deux notes, « Liste des retakes (CSV) » ouverte dans un tableur.

#### Partie 87 — Pipeline étape 1 : noms, liste des plans, paquet, modèle d'épisode (2026-10-30, 3cd07b7, format 45)
- Source : INTERFACE partie 87, ligne 6739
- Statut : jamais vu ; en-têtes Kitsu « à confirmer sur une instance ».
- Manipulations :
  1. Fichier > Exporter pour la production > « Noms des plans, série et épisode » : taper un modèle, aperçu, modèle faux, doublon ; Cmd+Z.
  2. Liste des plans : Kitsu/Générique, CSV/JSON, 1001/1/0, vignettes ; ouvrir le CSV.
  3. Paquet de livraison : tout cocher, lancer ; « Arrêter » ne laisse rien ; relancer sans « Remplacer » → refus ; `manifest.json`.
  4. Enregistrer comme modèle d'épisode ; Fichier > Nouveau → « Modèle d'épisode », épisode 2 : mêmes calques, personnages, noms.

#### Partie 86 — Numérotation verrouillée d'origine, montage redimensionnable (2026-10-30, bab0087)
- Source : INTERFACE partie 86, ligne 6567
- Statut : jamais vu. La timeline a été refaite ensuite (137, 178, 179, 192) : les boutons de hauteur de pistes son sont à revérifier.
- Manipulations :
  1. Nouveau projet, trois plans, nouveau plan après le 2 → « Plan 2A » ; décocher Préférences > Montage > Plans neufs → « Suivie ».
  2. Tirer le bord entre dessin et montage → pastille au survol, minimum 200 points, hauteur gardée par espace.
  3. Bande réduite → barre de défilement, piste Son 1 atteignable, M / S / verrou cliquables.

#### Partie 85 — Stabilité et retouches de l'audit (2026-10-30, abadcb0)
- Source : INTERFACE partie 85, ligne 6555
- Statut : jamais vu.
- Manipulations : Échap ferme la boîte d'un export sans rien lancer ; un export réussi s'efface seul après 8 s (pointeur dessus : reste),
  « Ouvrir le dossier » dans la barre d'état (ne fait rien sur Mac, voir B) ; dessiner sur un calque vidéo → bouton « Transformer en
  calque d'animation ».

#### Partie 84 — Exports V1 (2026-10-30, c1594e9)
- Source : INTERFACE partie 84, ligne 6436
- Statut : en partie remplacé par 130 (plus de grande fenêtre dans le menu : quatre sous-menus Fichier > Exporter en vidéo / en image /
  le son / pour la production ; la grande fenêtre reste par Cmd+K « Exporter »). Menus vus par Anthony (captures de la partie 130) ;
  fichiers produits jamais ouverts dans un logiciel de montage (l. 6477).
- Manipulations :
  1. Exporter en vidéo > Film : MP4, ProRes 422, ProRes 4444, DNxHR ; un format sans encodeur est grisé avec sa raison ; taille et cadence.
  2. « Graver les informations » → numéro, time code, séquence, texte, sans calque ajouté.
  3. Exporter en image > une image par plan → noms `SQ01_PL012A`, JPEG, fond transparent.
  4. Storyboard en PDF : format, cases par page, textes ; ouvrir dans Aperçu → accents, œ, €, texte sélectionnable (sur Mac, police
     non embarquée : voir B).
  5. Son par piste (WAV 24 bits, AIFF), Pour le montage (FCPXML 1.9 / 1.10, marqueurs), Calques séparés, Caméra (JSON).
  6. Pendant un export, dessiner (rien ne rame) ; « Arrêter » efface ce qui a été écrit.

#### Partie 83 — Panneau Bibliothèque (2026-10-30, 53f26f3)
- Source : INTERFACE partie 83, ligne 6411
- Statut : jamais vu. L'« icône livres de la barre de droite » n'existe plus (194) : Affichage > Panneaux ou « + Panneau ».
  Sur Mac, « lecteurs » = « / » seul (`biblio.rs` l. 659).
- Manipulations :
  1. Ouvrir la Bibliothèque → dossier du projet ; un dossier de milliers de fichiers → liste tout de suite, vignettes ensuite.
  2. Grille / liste, curseur de taille, tri par Taille / Date, filtre Vidéos, recherche, œil « Tout montrer ».
  3. Clic sur une image (molette = zoom), un son (onde, Espace), une vidéo (Espace, barre de temps, flèches image par image).
  4. Choisir un autre fichier ou fermer le panneau → le son s'arrête.
  5. Glisser une image sur le dessin, une case des Vignettes, les Références ; un son sur une piste ; clic droit : cinq commandes.
  6. Étoile → favori ; relancer → dernier dossier, vue et favoris revenus.

#### Partie 82 — Fusionner des calques, menu clic droit unique (2026-10-30, 1e6ff18)
- Source : INTERFACE partie 82, ligne 6488
- Statut : jamais vu. Le panneau Calques, son « … » et son ⋮ ont disparu (160) ; le menu se trouve au clic droit de la ligne du calque
  dans la timeline, au ⋮ de la ligne, au menu Calques de la barre.
- Manipulations :
  1. Clic droit sur une ligne de calque (timeline) → entrées en groupes, « Fusionner vers le bas » avec Cmd+E à droite.
  2. Deux calques bitmap, l'un à 50 % : Fusionner vers le bas → image identique ; Cmd+Z.
  3. Vectoriel sous bitmap : la bulle annonce « calque bitmap » ; deux vectoriels → « reste vectoriel », trait encore sélectionnable.
  4. Calque avec flou, calque « Produit », calque animé → fusion cuite image par image (rejouer).
  5. Calque caché, vidéo, d'effet, verrouillé → entrée grisée, raison en bulle.
  6. Cmd+clic sur trois calques → « Fusionner les 3 calques choisis ».

#### Partie 81 — Planche de références et post-it en place (2026-10-30, 1df74a1 et 012c17a)
- Source : INTERFACE partie 81, ligne 6368
- Statut : jamais vu. Sur Mac le choix « Écran 2 » est vide (`planchref::ecrans` rend une liste vide hors Windows, l. 599).
- Manipulations :
  1. Ouvrir le panneau Références (Affichage > Panneaux) ; glisser une image de la planche sur une case des Vignettes → « 1 image
     déposée » ; Cmd+Z.
  2. « Détacher » → fenêtre à part ; Plein écran (icône, F11 / Fn+F11), Échap ; fermer → retour dans l'onglet ; relancer → retenu.
  3. « Fenêtre transparente » → le fond devient-il transparent sur Mac ?
  4. Post-it, texte, titre, cadre ; double-clic sur un post-it, taper avec accents, Entrée = nouvelle ligne, Échap pose ; long texte →
     lettres plus petites puis post-it allongé.
  5. Pendant l'écriture : lettres d'outils, Suppr, Cmd+Z, Cmd+A n'agissent que sur le texte ; après, Cmd+Z défait tout le texte.
  6. Cadre déplacé → emporte son contenu ; Ranger ; Recadrer une image ; « … > Importer la planche d'un autre projet… ».

#### Partie 80 — Dégradé avancé (2026-10-30, 3774514, format 44)
- Source : INTERFACE partie 80, ligne 6323
- Statut : jamais vu (au Storyboard seulement : refusé dans les Vignettes depuis 113).
- Manipulations :
  1. Outil Dégradé, Réglages : cinq formes, bande avec pastilles de couleur dessous et d'opacité dessus, Angle, Échelle, Inverser,
     Tramage, Mes dégradés.
  2. Tracer dans un anneau → l'axe reste avec deux ronds ; tirer le rond d'arrivée → suit sans retard ; Cmd+Z → tout part d'un coup.
  3. Clic sous la bande → pastille ; glisser ; tirer vers le bas → supprimée ; clic au-dessus → opacité à 0 %.
  4. « + » enregistre, clic rappelle, clic droit renomme/supprime ; relancer → toujours là.
  5. Échap → l'axe part sans annuler ; calque vectoriel : Sélection de la zone puis Dégradé → l'axe revient.
  6. Noir → gris doux sur grande zone : pas de marches avec Tramage, marches sans.

#### Partie 79 — Hors-cadres texturé, passe-partout à l'export (2026-10-30, e546fc7)
- Source : INTERFACE partie 79, ligne 6293
- Statut : jamais vu. Le découpage du trait décrit est remplacé par 131 (trait entier) ; la texture du trait sur le papier reste.
- Manipulations :
  1. Pinceau à papier (crayon, fusain), trait qui part du papier hors des cases → grain du pinceau (un instant lisse, puis texturé).
  2. Fermer, rouvrir → grain là ; zoomer et défiler avec beaucoup de traits → fluide.
  3. Export de la planche : texture dans l'image ; case « Passe-partout » décochée d'origine, indépendante du bouton de l'écran,
     retenue à la séance suivante.

#### Partie 78 — Recherche dans le panneau Scénario (2026-10-30, 4d8677d)
- Source : INTERFACE partie 78, ligne 6281
- Statut : jamais vu.
- Manipulations :
  1. Pointeur sur le panneau Scénario, Cmd+F (ou la loupe de l'en-tête) → barre de recherche, curseur dans le champ.
  2. Taper « ete » → trouve « Été », compteur « 3 / 12 », surlignage, défilement vers la courante.
  3. Entrée = suivant, Maj+Entrée = précédent (boucle), Échap ferme et efface.
  4. Pointeur hors du panneau, Cmd+F → « Aller à un plan ».

#### Partie 77 — Vignettes par séquences (2026-10-30, 7e3f7e8)
- Source : INTERFACE partie 77, ligne 6220
- Statut : jamais vu. Le bloc « Séquences » du panneau Planche existe toujours (160 ne le cite pas comme retiré, non vérifié à l'écran).
- Manipulations :
  1. Projet à plusieurs séquences → chacune sous son titre, nouvelle rangée, une case pâle par séquence ; une seule séquence → pas de titre.
  2. Panneau Planche > Séquences : cocher/décocher (la dernière refuse) ; glisser une séquence dans la liste → elle bouge sur la planche,
     pas dans le montage ; « Seulement la courante », « Aucune autre », « Toutes ».
  3. Clic droit sur une case > Plus > Séquence : Nouvelle séquence à la fin, Couper la séquence ici, Nouvelle séquence avec cette case ;
     un Cmd+Z chacun.
  4. Dessiner dans la case pâle d'une séquence du milieu → le plan naît dans cette séquence.
  5. Export de la planche avec une séquence cachée → « Ce que la planche montre », titres, ordre de l'écran.

#### Partie 76 — Vignettes : continuité tout de suite, numéros, lignes de texte, réglages (2026-10-30, 5335c60)
- Source : INTERFACE partie 76, ligne 6126 ; JOURNAL l. 1320 (renvoyé pour 3 défauts « vus sur capture »)
- Statut : jamais vu point par point. Points 3 (répartition du trait entre cases) remplacés par 131 (vu par Anthony) ; barre du haut
  allégée par 113 (taille des cases dans les Préférences).
- Manipulations :
  1. Dessiner dans la dernière case, cliquer la barre devant la 1re case pâle → la case apparaît aussitôt avec le dessin ; la case
     d'après reste une coupe ; Cmd+Z défait.
  2. Réunir deux plans par la barre noire → la case de droite montre le dessin de gauche ; décocher/recocher → seule cette barre change.
  3. Numéros lisibles à six cases par rangée ; Préférences > Vignettes > Numéros et textes ; pastille « V2 » hors de l'image.
  4. Cocher Note, Action, Dialogue → trois lignes colorées, même vides ; décochées pour un nouvel utilisateur.
  5. Fermer, rouvrir → planche sur 2 rangées de 6 ; « 6 × 2 » de la barre du bas recadre.
  6. Barre du bas : passe-partout des cases (icône, opacité), retenu à la séance suivante.

### Parties 1 à 75 (lot 1)

#### Partie 75 — Le panneau Scénario (2026-10-30, 32dd5f5)
- Source : docs/INTERFACE.md partie 75, l. 6036 (liste l. 6096) ; découpage partie 10, l. 565 ; JOURNAL l. 1317.
- Statut : jamais vu. La recherche Cmd+F du panneau (partie 78) est dans un autre lot.
- Manipulations :
  1. Fichier > Importer > « Un scénario, à garder dans le projet… » sur un `.fountain` (ou `.fdx`, `.docx`) → la fenêtre dit « Aucun plan n'est créé » ; Importer → le panneau Scénario s'ouvre, le storyboard ne change pas ; Cmd+Z retire le scénario, Cmd+Y le remet.
  2. Regarder la mise en page (intitulés en gras, personnage centré, réplique étroite, parenthèse en italique) ; les trois boutons « A » changent la taille, qui reste après relance.
  3. « Créer les plans depuis le scénario… » → fenêtre de découpage sans choix de fichier ; avec des plans existants, elle dit qu'ils sont gardés ; « Créer les plans » = une seule étape d'annulation (Historique « Créer les plans du scénario »).
  4. Changer de plan (flèches, clic dans le montage) → son passage est surligné et remis en vue ; clic sur une ligne → on va au plan.
  5. Supprimer, réunir, déplacer des plans liés → aucun plantage ; clic sur le passage d'un plan disparu → plan lié le plus proche.
  6. Réimporter un autre scénario → « Remplacer le scénario ? » ; après remplacement, `scenario.ancien-….json` dans le dossier du projet.

#### Partie 74 — La fenêtre d'accueil compacte (2026-10-29, b201dc1)
- Source : partie 74, l. 5944 (liste l. 6010) ; ETAT l. 185 et l. 197 (item 6) ; JOURNAL l. 1311, 1420.
- Statut : vu en partie : Anthony a tranché sur l'engrenage de l'accueil (« gardé », JOURNAL l. 1311). Le reste n'a jamais été vu (ETAT l. 185 « clics dans la fenêtre d'accueil, agrandissement / retour, plusieurs écrans, 150 % »). Le logo et le nom de cette fenêtre ont changé en 96 (FEANOR).
- Manipulations :
  1. Lancer FEANOR avec « Rouvrir le dernier projet » décoché → petite fenêtre centrée, titre de l'application, ni menus ni barre d'état, bandeau image + logo à gauche.
  2. Taper un nom, Entrée → le projet se crée, la fenêtre s'agrandit sur la page Vignettes ; Cmd+W → la fenêtre redevient petite et centrée.
  3. Projet récent : double-clic, ou clic puis Entrée ; un projet déplacé dans le Finder apparaît grisé « introuvable » ; clic droit > Retirer de la liste.
  4. « Ouvrir un projet… » → le sélecteur accepte un projet ; « Rouvrir le dernier projet » coché → ouverture directe sans petite fenêtre.
  5. Projet ouvert, Fichier > Nouveau (Cmd+N) → même formulaire, avec Annuler.
  6. Écran Retina / échelle 150 % et second écran → la petite fenêtre reste entière et centrée (point jamais vu, ETAT l. 185).

#### Partie 73 — Démarrer sans projet ouvert, un seul thème au premier lancement (2026-10-29, 82feaad)
- Source : partie 73, l. 5848 (liste l. 5921).
- Statut : jamais vu (la demande vient d'une capture d'Anthony du défaut, pas de la correction).
- Manipulations :
  1. Lancer sans fichier de réglages → accueil seul, aucun onglet, aucune zone blanche.
  2. « Nouveau projet », OK → un seul onglet ; Cmd+W → retour à l'accueil sans onglet (plus de projet en double).
  3. Sans projet : B, Cmd+Z, Cmd+V, Tab → rien, la barre d'état le dit ; menus Édition à Labo grisés.
  4. macOS en apparence claire (Réglages Système > Apparence) puis relance → FEANOR reste sombre en entier ; Préférences > Apparence > « Gris clair » → tout clair et lisible. (La liste d'origine parle du thème clair de Windows.)

#### Partie 72 — Clés d'opacité et calque « Couleur unie » (2026-10-29, 27f31ec)
- Source : partie 72, l. 5779 (liste l. 5832) ; ETAT l. 202 (item 10).
- Statut : jamais vu. Le panneau Calques cité dans la liste n'existe plus (160) ; le losange des clés d'opacité est sur la ligne du calque de la timeline (160, 165, 174).
- Manipulations :
  1. Bouton « Calques » de la timeline > nouveau calque « Couleur unie » → toute la vue passe au gris moyen ; changer la couleur dans les réglages du calque.
  2. Dessiner puis gommer sur ce calque → la gomme s'arrête au fond uni ; changer la couleur → le dessin reste.
  3. Sur un calque quelconque : losange à côté de l'opacité, clé image 0 à 100 %, clé image 12 à 0 % → la lecture (Espace) fait un fondu.
  4. Losanges gris clair dans la timeline et le montage : glisser, supprimer, Cmd+Z.
  5. Un fondu au noir avec un calque Couleur unie noir et des clés d'opacité → visible en lecture et dans le film exporté.

#### Partie 70 — Brosses perso : personnage, classique, décor (2026-10-29, a289602)
- Source : partie 70, l. 5682 (liste l. 5745).
- Statut : en partie remplacé par la partie 107 (nom « Brosses dédiées », pinceaux propres à chaque brosse, plus de pinceaux de décor fixes). Reste à voir : la rangée et la création des calques.
- Manipulations :
  1. Projet neuf → au bout droit de la barre du haut du dessin : 3 ronds de couleur, 3 petits ronds gris, 3 petits carrés gris, « + », icône du panneau ; pas de ⊘.
  2. Trait avec un rond de couleur → un calque au nom de la brosse naît avec son aplat ; petit rond gris → trait dans le calque courant ; carré → calque « Décor » tout en bas ; Cmd+Z défait calque et trait d'un coup.
  3. Recliquer la brosse choisie → elle est lâchée ; « + » → menu à trois lignes (Personnage, Classique, Décor).
  4. Dupliquer un calque de personnage, rechoisir sa brosse depuis un autre calque → retour au dernier calque du personnage utilisé.

#### Partie 69 — Coller une image en objet vectoriel ; copier-coller des calques (2026-10-29, 4b42226)
- Source : partie 69, l. 5636 (liste l. 5666).
- Statut : jamais vu. Les gestes « panneau Calques » passent par la timeline (160).
- Manipulations :
  1. Calque vectoriel, copier une image dans Safari, Cmd+V → image centrée dans un cadre de sélection, aucun calque neuf ; déplacer, tourner, agrandir, Suppr, Cmd+Z.
  2. Calque bitmap, même geste → zone flottante de pixels.
  3. Coller une grande capture (4K, Cmd+Maj+4 puis copier) → l'interface ne se fige pas.
  4. Choisir un calque dans la timeline, Cmd+C, aller dans un autre plan, choisir un calque, Cmd+V → « Nom (2) », un seul Cmd+Z. Même chose avec Cmd + clic sur deux calques ; clic droit sur un calque : « Copier, coller le calque ».
  5. Fermer, rouvrir → l'image est là ; le dossier du projet a un `images/`.

#### Partie 67 — Enregistrer par-dessus un projet : remplacer sans rien perdre (2026-10-28, 57c04bf)
- Source : partie 67, l. 5538 (liste l. 5565) ; ETAT l. 179.
- Statut : en partie remplacé par la partie 135 (écriture continue) : la question « Modifications non enregistrées » ne reste que pour un projet sans dossier ou avec Préférences > En cas de plantage > « Enregistrer en continu » décoché. Jamais vu. C'est aussi la seule partie de la tranche sur la récupération après plantage (copie de secours).
- Manipulations (décocher d'abord « Enregistrer en continu ») :
  1. Nouveau projet, quelques traits, croix de la fenêtre, « Enregistrer », choisir un projet existant → fenêtre « Un projet existe déjà ici » avec la date ; Annuler → la question revient, rien n'est fermé.
  2. Même chose puis Remplacer → enregistrement puis fermeture ; dans Documents, un dossier « … (remplacé le …) » qui s'ouvre normalement.
  3. « Choisir un autre nom » puis fermer le sélecteur sans choisir → la question revient.
  4. Le projet visé ouvert dans un autre onglet → remplacement refusé avec la raison.
  5. Récupération : dessiner, puis Forcer à quitter (Cmd+Option+Échap) → à la relance, le travail est proposé ou présent (copie de secours sans écriture continue ; dossier à jour avec).

#### Partie 66 — Clic droit : les réglages de l'outil sous le curseur (2026-10-28, 98fc746)
- Source : partie 66, l. 5490 (liste l. 5523). Toujours valable : la partie 111 (anneau) garde le clic bref.
- Statut : jamais vu.
- Manipulations :
  1. Pinceau, clic droit dans le dessin → fenêtre des réglages sous le curseur ; changer la taille → le panneau Réglages suit ; près du bord d'écran, elle reste entière.
  2. Cliquer hors de la fenêtre (dessin, panneau, outil) → elle se ferme sans trait ni changement d'outil ; Échap aussi.
  3. Gomme, Remplir, Texte, Caméra, Transformer, Grab → chacun ses réglages ; sous-ligne Aplat visée → réglages de l'aplat.
  4. Sélection, Sélection par couleur, Pipette → l'ancien menu (Annuler, Couper, Copier…), pas la fenêtre.
  5. Vignettes, clic droit sur une case → menu de la case avec « Réglages de l'outil… » en tête.

#### Partie 65 — Une pipette dans chaque choix de couleur (2026-10-28, c1be89b)
- Source : partie 65, l. 5442 (liste l. 5474).
- Statut : jamais vu.
- Manipulations :
  1. Réglages > Couleur de l'aplat, cliquer la case → icône pipette en haut de la fenêtre, bulle à 1 s.
  2. Cliquer la pipette, puis un pixel du dessin → la case prend la couleur, la fenêtre reste ; Cmd+Z = une étape. La loupe et le code couleur suivent le curseur.
  3. Viser une vignette, le panneau Références, la barre des pastilles → la couleur est prise.
  4. Pendant la pipette, cliquer un bouton ou dessiner → rien ; Échap ou clic droit → abandon sans changement.
  5. Couleur d'une brosse dédiée (clic droit > Modifier) → seule cette case change, pas la couleur de dessin.

#### Partie 64b — Petites retouches du 2026-10-28 au soir
- Source : partie 64b, l. 5431.
- Statut : jamais vu (demandées sur capture, faites ensuite). Les couleurs de calques ont changé depuis (98).
- Manipulations :
  1. Clic droit sur un plan du montage → pas de sous-menu « Plus » ; « Couleur du plan », « Transition au début / à la fin » au premier niveau ; « Version du plan » contient directement les deux « Nouvelle version » ; douze entrées au plus.
  2. Cadrage > Format → cadre gris sans étiquette « 16:9 » ; Préférences > Dessin > Caméra > « Cadre de format » change sa couleur.
  3. « + » d'une brosse dédiée → aucune fenêtre « Modifier », la brosse arrive dans la barre.

#### Partie 64 — Mon panneau (Labo) (2026-10-28, b3ca8b7)
- Source : partie 64, l. 5346 (liste l. 5414) ; panels.rs:293.
- Statut : jamais vu. Labo caché d'origine (Préférences > Général > « Afficher les fonctions expérimentales »). Hors fonctions de base.
- Manipulations :
  1. Labo > Mon panneau (essai) → panneau vide avec phrase d'aide.
  2. Panneau ouvert, clic droit sur une icône d'outil → « Ajouter à Mon panneau » ; panneau fermé → l'entrée n'y est pas.
  3. « Enregistrer l'outil actuel ici » (crayon, taille, couleur), changer d'outil, recliquer le bouton → tout revient.
  4. Glisser un bouton ; clic droit : séparateur, renommer, retirer ; relancer → tout est là.

#### Partie 63 — L'aplat intelligent (2026-10-28, 020f8a2)
- Source : partie 63, l. 5247 (liste l. 5331) ; ETAT l. 196 (item 3).
- Statut : jamais vu.
- Manipulations :
  1. Calque avec aplat, sous-ligne Aplat choisie (ligne du calque dans la timeline) → Réglages : case « Aplat intelligent » décochée ; cochée → bouton « Colorier comme la clé précédente ».
  2. Visage fermé avec œil et ombre fermés, pot de peinture sur l'aplat (3 couleurs), cocher la case, retoucher le contour plus loin → la peau suit, l'œil et l'ombre gardent leur couleur ; un Cmd+Z.
  3. Gommer une brèche puis refermer avec un renflement → la peau revient.
  4. Nouvelle clé vide, redessiner le visage décalé → l'aplat prend les couleurs de la clé d'avant à chaque zone fermée.
  5. Trois clés choisies (Cmd + clic), « Colorier comme la clé précédente » → chacune prend la précédente ; un Cmd+Z.

#### Partie 60 — Alt + glisser duplique dans le montage (2026-10-28, 03081ad)
- Source : partie 60, l. 5080 (liste l. 5100).
- Statut : jamais vu. La partie 138 (glisser une clé avant ou après une autre) a revu les gestes de glisser : à recouper avec le lot qui la couvre.
- Manipulations :
  1. Montage, Option + glisser un dessin par son milieu → fantôme, rien ne bouge ; au lâcher une copie, l'original reste ; barre d'état « une copie … avec son dessin ».
  2. Redessiner dans la copie → l'original ne change pas ; Cmd+Z retire la copie.
  3. Plusieurs clés au rectangle sur deux calques, Option + glisser → toutes copiées ; un Cmd+Z.
  4. Échap pendant le glisser → rien ; copie tout au bout du plan → coupée, le plan garde sa durée.

#### Partie 59 — Versions de séquence et versions de plans (2026-10-28, de69a96)
- Source : partie 59, l. 5000 (liste l. 5060) ; remplace la partie 36.
- Statut : jamais vu (AUDIT-2026-11-10 : « versions, variantes » jamais touchées). Corrigé depuis en partie 102 (plan déplacé vers une autre séquence) ; l'étiquette de version du montage a changé en partie 168 (« V2 ▸ »).
- Manipulations :
  1. Vignettes : icône branche de la séquence > « Nouvelle version, en gardant les plans » → « Séquence 2 · A B », B enfoncée, le film n'a pas bougé.
  2. Plan en V2 dans la B ; cliquer A → V1 ; cliquer B → V2 ; Cmd+Z après chaque passage.
  3. Dans la B : raccourcir un plan fixe, en supprimer un, changer l'ordre ; cliquer A → tout est comme avant ; B → le découpage revient.
  4. Dans la B, clic droit > Version du plan > « Mettre à jour dans les autres versions de séquence… » → cocher A, valider ; passer à A → V2 à sa durée d'avant ; Cmd+Z.
  5. « Nouvelle version, en copiant les plans », redessiner → la première ne change pas.
  6. Fermer, rouvrir → versions, noms, durées, version choisie retrouvés.

#### Partie 58 — Couleur qui se mélange, Mélangeur, palette de mélange (2026-10-28, 29e1ef8)
- Source : partie 58, l. 4939 (liste l. 4988) ; ETAT l. 195 (item 2).
- Statut : jamais vu.
- Manipulations :
  1. Calque bitmap, pinceau, Réglages > Plus de réglages > « Couleur et mélange » → case « La couleur se mélange », types Pigment, Eau, Lisser.
  2. Rectangle jaune, bleu en Pigment (Mélange 0,1-0,2) tracé à travers → passe par du vert, pas du gris ; Charge basse → le trait se vide.
  3. Lisser entre deux aplats → elles s'étalent, aucune couleur nouvelle ; un Cmd+Z.
  4. Calque vectoriel → case grisée, bulle « Sur un calque bitmap ».
  5. Clic maintenu sur l'icône du pinceau → Mélangeur ; panneau Couleur > « Palette de mélange » : jaune + bleu → vert ; Option + clic prend le vert.

#### Partie 57 — Accueil : image au hasard, images d'accueil, ouverture sur Vignettes ou Storyboard (2026-10-28, 5a2566c)
- Source : partie 57, l. 4883 (liste l. 4926).
- Statut : en partie remplacé par la partie 74 (la carte d'accueil est devenue la petite fenêtre). Restent : images d'accueil des Préférences et le choix de l'espace d'un nouveau projet. Jamais vu.
- Manipulations :
  1. Préférences > Apparence > Images d'accueil > « Importer des images… » (2-3 png/jpeg) → miniatures, copiées dans `Documents/SHOT/Identité/Accueil` ; la même deux fois → `_2`.
  2. Fermer le projet / relancer → une de ces images dans le bandeau d'accueil, pas toujours la même ; dossier vide → le logo.
  3. Préférences > Général > « Un nouveau projet s'ouvre sur » : Storyboard → nouveau projet sur l'espace Storyboard ; choix gardé après relance.
  4. « Retirer » une image → question, « Supprimer » l'efface (sans Corbeille) ; un fichier texte dans le dossier → aucune erreur.

#### Partie 56 — Les outils de l'aplat (2026-10-28, 9c24da9)
- Source : partie 56, l. 4815 (liste l. 4869).
- Statut : vu en partie : les deux boutons « Peindre derrière » et « Peint sur l'aplat » figurent sur la capture annotée d'Anthony du panneau Pinceau (partie 45, l. 4234). Leur effet et le reste jamais vus.
- Manipulations :
  1. Allumer l'un des deux boutons → l'autre s'éteint.
  2. « Peint sur l'aplat » sur un personnage fermé → la peinture va dans l'aplat (créé s'il manquait), le trait ne bouge pas.
  3. Sous-ligne Aplat + outil Sélection → lasso sur la peinture d'aplat, déplacer / tourner : seul l'aplat bouge ; Cmd+Z.
  4. Trois clés choisies dans la timeline, aplat visé : peindre, gommer, pot → chaque clé reçoit ; un Cmd+Z.
  5. Couleur de l'aplat avec portée « La clé » / « Le plan » / « Tout le storyboard » → la barre d'état dit combien de dessins.

#### Partie 55 — Timeline et plans : Alt + glisser, coller les clés, séquence des plans choisis, calques de même nom, durées (2026-10-28, a5877e1)
- Source : partie 55, l. 4751 (liste l. 4801).
- Statut : jamais vu.
- Manipulations :
  1. Trois plans qui se suivent (Cmd + clic), clic droit > « Nouvelle séquence avec les 3 plans choisis » → séquence neuve, l'ancienne coupée en deux ; Cmd+Z. En numérotation verrouillée → numéros gardés.
  2. Deux plans ayant chacun un calque « Dessin », « Réunir avec le plan suivant » → fenêtre « Calques de même nom » : Fusionner / Renommer avant de fusionner / Ne pas fusionner ; Annuler → rien.
  3. Préférences > Montage > « Durée d'un nouveau plan » 3 s → Nouveau plan = 72 images à 24 i/s.
  4. Trois plans, clic droit > « Changer la durée des plans choisis… » : 36 puis « +12 » → durées changées ; avec du son après, la fenêtre le dit ; un Cmd+Z.
  5. Montage : clé de dessin, Cmd+C, tête sur une autre clé, Cmd+V → copie après la clé ; Cmd+Maj+V → posée sur la tête.

#### Partie 54 — Corrections : paliers à l'écran, lasso bitmap fluide, tremblement par défaut, stylet (2026-10-28, e6a506b)
- Source : partie 54, l. 4696 (liste l. 4740).
- Statut : corrections de défauts vus par Anthony (« cinq retours après essais ») ; les corrections elles-mêmes jamais vues. 54.3 (question à la fermeture) remplacé par 135. Le réglage du stylet est passé dans Préférences > Dessin (130).
- Manipulations :
  1. Caméra animée, outil Caméra > Réglages > « Mouvement par paliers », 3 images ; prendre un autre outil ou lancer la lecture → mouvement saccadé à l'écran ; outil Caméra arrêté → mouvement doux et le petit texte sous la case le dit.
  2. Grand dessin bitmap, lasso d'une grande zone, déplacer / échelle / tourner → pas de retard, zone nette au lâcher ; tourner deux fois → pas de flou cumulé.
  3. Préférences > Dessin > Tremblement des traits → aperçu ; « Animer à l'arrêt » fait bouger le trait d'exemple ; vitesse, variantes, graine suivent sans à-coup.
  4. Préférences > Dessin > « Stylet à ma main » : trois traits, Appliquer → fenêtre fermée ; rouvrir, Rétablir. (Le toucher du stylet sur Mac : tablette nécessaire.)

#### Partie 53 — L'outil Dégradé, bitmap et vectoriel (2026-10-28)
- Source : partie 53, l. 4620 (liste l. 4668) ; correction l. 4676.
- Statut : vu en partie : Anthony a essayé le dégradé bitmap (son retour « dégradé sur dégradé », l. 4678). Restent jamais vus : la correction, le vectoriel, l'aplat, la zone au lasso. Étendu par la partie 80 (autre lot).
- Manipulations :
  1. Refaire le cas d'Anthony : un dégradé noir → gris clair sur un calque bitmap, un second dégradé par-dessus → il couvre toute la zone, sans bande diagonale.
  2. Lasso une zone, outil Dégradé, glisser → seule la zone ; Option + glisser → toute la feuille.
  3. Calque vectoriel → zone vectorielle en dégradé ; Sélection, déplacer, tourner → le dégradé suit ; pot de peinture → zone unie.
  4. Sous-ligne Aplat choisie, dégradé dans une zone fermée → sous le trait.

#### Partie 52 — Fusionner deux plans en glissant l'un sur l'autre (2026-10-24, 0b88295)
- Source : partie 52, l. 4583 (liste l. 4609) ; 104 (l. 7574) a corrigé la fin rognée.
- Statut : jamais vu.
- Manipulations :
  1. Montage : glisser un plan par son milieu sur le milieu d'un autre → contour, « Fusionner » ; lâcher → un seul plan ; Cmd+Z.
  2. Lâcher sur le premier ou le dernier quart → simple déplacement.
  3. Échap pendant le glissé → rien ; plan d'une autre séquence → pas de fusion ; deux plans choisis → déplacement sans fusion.

#### Partie 51 — Caméra « par paliers » (2026-10-24, 0b88295)
- Source : partie 51, l. 4540 (liste l. 4574) ; suite 54.1.
- Statut : vu en partie : Anthony a vu que les paliers ne se voyaient pas à l'écran (retour à l'origine de 54.1). La correction et l'export jamais vus.
- Manipulations :
  1. Paliers à 2, 3, 4, 6 → lecture, vue caméra et vignettes de la timeline montrent le mouvement saccadé ; décocher → doux.
  2. Les losanges de la piste Caméra ne bougent pas ; Cmd+Z après la case puis après le nombre.
  3. Exporter une suite PNG ou un film → images d'un même palier identiques, dernière image au cadre de la dernière clé.

#### Partie 50 — Les pastilles dans la barre du haut (2026-10-24, f2e520f)
- Source : partie 50, l. 4503 (liste l. 4522) ; place confirmée par 160 (l. 12880).
- Statut : en partie remplacé par 70 et 107 (contenu de la rangée). Reste : la place et le défilement. Jamais vu.
- Manipulations :
  1. Fenêtre étroite ou beaucoup de brosses → la rangée défile à la molette, sans recouvrir le dessin.
  2. Vignettes → la rangée au bout de la ligne des réglages de la planche ; un clic ne touche pas la planche.
  3. Affichage > Cacher les pastilles → la rangée part, rien ne bouge.

#### Partie 49 — Choisir une couleur recolore la sélection (2026-10-24, f2e520f)
- Source : partie 49, l. 4458 (liste l. 4481).
- Statut : jamais vu.
- Manipulations :
  1. Vectoriel, lasso sur deux traits, clic sur une couleur du nuancier → seuls eux changent ; un Cmd+Z.
  2. Tirer un curseur RVB/TSV ou la roue → un seul Cmd+Z ; taper `#ff8800` → idem.
  3. Bitmap, lasso sur une zone peinte à bords doux, nouvelle couleur → bords doux gardés, pas de liseré.
  4. Sans sélection (Échap) → aucun dessin ne change ; Option + clic (pipette) avec sélection → pas de recoloration.

#### Partie 48 — Le tremblement du trait par défaut (2026-10-24, f2e520f)
- Source : partie 48, l. 4419 (liste l. 4439) ; complété en 54.4.
- Statut : jamais vu.
- Manipulations :
  1. Préférences > Dessin : cocher « Tremblement du trait par défaut » ; aller d'image en image sur un calque vectoriel → le trait bouge.
  2. Calque avec son propre effet Tremblement → il garde son réglage.
  3. Dessiner → net pendant le geste, tremble après ; exporter quelques images → même tremblement.

#### Partie 47 — Le passe-partout autour du cadre de la caméra (2026-10-24, f2e520f)
- Source : partie 47, l. 4379 (liste l. 4401). L'icône est dans la barre du bas depuis 160 ; dans les Vignettes, voir 110.
- Statut : jamais vu.
- Manipulations :
  1. Cadre de caméra resserré → hors-champ noir opaque, au-delà de la feuille aussi ; tourner la vue, miroir, incliner la caméra → pas de trou.
  2. Barre sous le dessin : icône passe-partout (éteint / rallume), curseur 50 % puis 0 %.
  3. Préférences > Dessin : couleur (bleu), opacité, Rétablir ; relancer → gardé.
  4. Exporter un plan → l'image du cadre, sans passe-partout.

#### Partie 46 — Calques d'annotation et lasso d'annotation (2026-10-24, f9d5696)
- Source : partie 46, l. 4271 (liste l. 4329, suites l. 4342).
- Statut : jamais vu. Le « + » du panneau Calques passe par la timeline (160).
- Manipulations :
  1. Vignettes, bloc Calques du panneau Planche : « + » → « Annotations 2 », stylo pris ; tracer ; cliquer « Dessin » → le pinceau dessine dans la case.
  2. Œil d'un calque d'annotation, double-clic pour renommer, clic droit monter / descendre / supprimer (Cmd+Z rend).
  3. Lasso d'annotation : entourer, déplacer, coin pour agrandir, hors coin pour tourner ; Échap.
  4. Vue Dessin : nouveau « Calque d'annotation » → rouge-rose, en haut ; exporter image / vidéo → aucun trait d'annotation ; vignettes non plus.

#### Partie 45 — Les pinceaux dans les Réglages de l'outil (2026-10-24, 1137346 ; retouche 2026-11-18, 907c92a)
- Source : partie 45, l. 4171 (liste l. 4209) et retouche l. 4232 (liste l. 4255).
- Statut : vu en partie : Anthony a annoté une capture du panneau Pinceau (l. 4234), donc la liste dans les Réglages est vue. Jamais vu : la disposition du 18 novembre qui en découle.
- Manipulations :
  1. Pinceau, liste dépliée (bouton « livres ») → sous l'aperçu : trois boutons puis « Plus de réglages », puis importer / + / corbeille, puis recherche et liste.
  2. Déplier « Plus de réglages », tourner un réglage → l'aperçu reste visible sans défiler.
  3. « + » → la saisie du nom en tête de la liste ; crayon et gomme : même ordre ; clic droit dans le dessin : même ordre.

#### Partie 44 — Le panneau Vignettes : sens de lecture, textes à côté des cases, version sur la case (2026-10-24, 5aa9117)
- Source : partie 44, l. 4057 (liste l. 4120) ; retouches après essai d'Anthony l. 4131.
- Statut : vu en partie : mise en page à la japonaise et alignement des mouvements vus par Anthony (capture annotée, l. 4133). Les textes décochés au retour ont été corrigés en 106. Jamais vus : textes multiples, étiquette de version sur la case, export.
- Manipulations :
  1. Panneau Vignettes : cocher Action puis Dialogue → une ligne chacun avec étiquette colorée, en lignes et à la japonaise.
  2. Relancer → sens et cases cochées gardés ; Fichier > Exporter en image > Planche des vignettes → même ordre, mêmes textes.
  3. Plan à deux versions → « V2 » en haut à droite de la case ; survol → liste ; clic sur V1 ; Cmd+Z.

#### Partie 43 — Grab et Pull (2026-10-24, 38ea4eb)
- Source : partie 43, l. 3971 (liste l. 4023).
- Statut : jamais vu.
- Manipulations :
  1. Barre de gauche, case mains sous Transformer / Déformer, clic maintenu → Grab et Pull.
  2. Calque vectoriel au pinceau texturé : Grab sur un bras → les points suivent, rien ne saute au lâcher ; Cmd+Z d'un coup.
  3. Pull sur une droite de deux points → courbe lisse.
  4. Calque bitmap → les pixels s'étirent ; trois clés choisies → les trois changent, un Cmd+Z.
  5. Rayon 400 et geste rapide → pinceau fluide.

#### Partie 42 — Retouches d'interface (2026-10-23, 512ad42)
- Source : partie 42, l. 3877 (liste l. 3937).
- Statut : en partie remplacé : pastilles (§3) par 70/107 ; « Peint sur l'aplat » (§2) par 56 ; repères de composition dans la barre du bas (160). Reste : point fixe et rotation de Transformer, la règle = perspective. Jamais vu.
- Manipulations :
  1. Sélection / Transformer : « Point fixe » et « Rotation » ; tirer un coin (coin opposé fixe), Cmd tenu (autour du centre).
  2. Rotation « Coin opposé » : tourner hors d'un coin → le petit cercle passe sur le coin opposé ; mêmes choix dans Préférences > Dessin.
  3. Icône règle (barre du haut du dessin) → perspective seule ; « Repères » dans la barre du bas → repères et aspect.

#### Partie 17 (bis, l. 3750) — Concentration, conseils, taille de l'interface, aide-mémoire, stylet (2026-10-21, 42b0187)
- Source : partie 17 (seconde du nom), l. 3750 (liste l. 3813).
- Statut : en partie remplacé : l'accueil à trois entrées (§1) par 57 et 74 ; la touche Z par F11 (64b) ; le stylet est dans Préférences > Dessin (130). Jamais vu.
- Manipulations :
  1. F11 (sur Mac, il faut peut-être fn+F11, voir B) → seuls le dessin et une barre mince ; F11 remet tout.
  2. Premier passage dans Montage, Vignettes, Espace 3D → une bulle de conseil, une seule ; « Compris » ; Aide > Revoir les conseils.
  3. Préférences > Apparence > Échelle 125 %, 150 %, 90 % → appliqué tout de suite ; le trait part de la pointe ; relancer → gardé.
  4. Tenir R → carte des raccourcis ; relâcher → elle part.

#### Partie 33 — L'aplat sous le trait, depuis le calque (2026-10-21, c3507d9)
- Source : partie 33, l. 3409 (liste l. 3493) ; correction l. 3511.
- Statut : vu en partie : le bouton « Aplat sous le trait » est utilisé (50 clics, AUDIT-2026-11-10). La sous-ligne n'est plus dans le panneau Calques mais sur la ligne du calque de la timeline (160). Jamais vus : peindre / gommer l'aplat seul, réglages et retrait.
- Manipulations :
  1. Cliquer la sous-ligne « Aplat » → la vue dit « Aplat de … » ; pinceau d'une autre couleur → la peinture reste sous le trait ; gomme → l'aplat seul ; pot → zone remplie ; Cmd+Z.
  2. Recliquer le nom du calque → le pinceau redessine le trait.
  3. Clic droit sur la sous-ligne → réglages et « Retirer l'aplat » ; Cmd+Z rend l'aplat avec sa peinture.
  4. Fermer, rouvrir → aplat, peinture et sous-ligne là, on dessine le trait.

#### Partie 31 — Effets au clavier et tremblement dans « Trait » (2026-10-21, 5ebb802)
- Source : partie 31, l. 3207 (liste l. 3259).
- Statut : jamais vu (« effets » jamais touchés, AUDIT-2026-11-10).
- Manipulations :
  1. Panneau Effets, calque à trois effets : clic, Cmd + clic, Maj + clic sur les noms → choix d'un, deux, d'une plage.
  2. Deux effets choisis, Cmd+C, choisir un autre calque, Cmd+V → deux effets en bas de sa pile avec leurs clés ; Historique « Coller 2 effets » ; un Cmd+Z.
  3. Copier un effet puis un dessin → Cmd+V colle le dessin.
  4. Menu « Trait ▾ » → « Tremblement du trait » en premier, grisé sur bitmap.

#### Partie 30 — Préférences en onglets (2026-10-21)
- Source : partie 30, l. 3104 (liste l. 3186).
- Statut : jamais vu. Les onglets ont grandi depuis (En cas de plantage, Anneau…). Pas de raccourci (voir B).
- Manipulations :
  1. Fichier > Préférences… → fenêtre large, onglets à gauche, recherche en haut ; tirer un bord, la barre de titre.
  2. Parcourir les onglets → rien de coupé ; l'onglet ouvert reste après fermeture et relance.
  3. Recherche « couleur », « cache », « voile », « zzz » → groupes trouvés / rien ; effacer par la croix.
  4. Petit écran ou fenêtre réduite → la fenêtre revient dans l'écran et défile.

#### Partie 28 — Exports : le projet reste libre pendant qu'ils tournent (2026-10-20)
- Source : partie 28, l. 2869 (liste l. 2901) ; ETAT mise à jour 2026-11-07 (« Annuler n'a pas été cliqué dans le vrai exe »).
- Statut : jamais vu.
- Manipulations :
  1. Fichier > Exporter en vidéo > Film en 4K → pendant l'export : tête de lecture, trait, zoom, changer de plan et d'onglet, tout répond ; fenêtre « Export en cours » en bas à droite.
  2. Pendant l'encodage, gros trait rouge sur le plan 1 → absent du film.
  3. Annuler pendant le film → « Export annulé », pas de .mp4 ; un ancien fichier du même nom intact. Même chose pour le PDF.
  4. Export du projet A, passer à B → la fenêtre dit « A » ; Quitter pendant l'export → message au premier Cmd+Q, sortie au second.
  5. Mémoire (Moniteur d'activité) → pas plus d'environ 1 Go de plus pour un film 4K.

#### Partie 27 — Travailler à plusieurs (2026-10-20, 1d1ccca)
- Source : partie 27, l. 2918 (liste l. 2950) ; docs/COOPERATION.md ; JOURNAL l. 1360 (à retravailler après la V1).
- Statut : jamais vu (« équipe » jamais touchée, AUDIT-2026-11-10). Faisable sur un seul Mac avec deux fenêtres sur le même dossier.
- Manipulations :
  1. Fichier > Partager ce projet… → choisir un dossier, le film partagé s'ouvre dans un nouvel onglet.
  2. Deux fenêtres : clic droit sur un plan > Prendre ce plan dans l'une → pastille dans l'autre ; dessiner dedans y est refusé avec la phrase.
  3. Un trait écrit dans une fenêtre arrive dans l'autre ; fermer une fenêtre → ses plans libres en quelques secondes.
  4. Montage refusé sans le jeton, permis avec ; fenêtre Équipe (Fichier > Équipe…).

#### Partie 26 — Fiche technique : le découpage en tableur (2026-10-20, a6dbd2c)
- Source : partie 26, l. 2705 (liste l. 2839).
- Statut : jamais vu par Anthony. Ouverture dans Excel vérifiée par l'agent (26.6, l. 2827) ; LibreOffice et Google Sheets jamais.
- Manipulations :
  1. Fichier > Exporter pour la production > Fiche technique → décocher « Notes », monter « Mouvement de caméra » ; exporter en XLSX avec vignettes.
  2. Ouvrir le .xlsx (Numbers, Excel Mac, LibreOffice) → en-tête figé, filtre, vignettes dans les cellules, durées en nombres, un dialogue commençant par `=` reste du texte.
  3. CSV point-virgule et CSV virgule → accents justes, une colonne par rubrique.
  4. Plan en V2 et plan « 12A » → colonnes Version et numéro ; « Une ligne par image de storyboard ».
  5. « Copier dans le presse-papiers », coller dans A1 d'un tableur ; Arrêter un export → rien laissé.

#### Partie 25 — Notes d'Anthony : calques, effets, montage, export, caméra (2026-10-20, 14637c1)
- Source : partie 25, l. 2406 (liste l. 2480).
- Statut : en partie remplacé : dossiers et « fx » passent par la timeline (160) ; aplat (§3) par 33 ; passe-partout (§7) par 47. Restent : GIF, durées des versions, menus Trait / Matière. Jamais vu.
- Manipulations :
  1. Fichier > Exporter en vidéo > GIF animé : plan courant, 480 px, 12 i/s, boucle → ouvrir dans Safari (boucle, couleurs) ; film entier → barre d'avancement, Annuler efface le fichier ; « Plans choisis ».
  2. Plan de 2 s, Nouvelle version, V2 à 3 s ; revenir à V1 → la bande se resserre, la barre d'état dit de combien, Cmd+Z ; export montage → durées de la version affichée.
  3. Panneau Effets → « Trait ▾ » et « Matière ▾ » ; « fx » sur la ligne d'un calque avec effet : un clic éteint / rallume, gardé après réouverture.

#### Partie 24 — Notes d'Anthony : dessin (2026-10-20, ccd3f7b)
- Source : partie 24, l. 2510 (liste l. 2679).
- Statut : jamais vu (demandes d'Anthony, pas de retour d'essai trouvé).
- Manipulations :
  1. Pinceau : tenir Maj en plein trait → segment droit jusqu'à la croix ; Maj+Option → angle par 15° ; un Cmd+Z.
  2. Cmd + clic sur une tache bitmap, Cmd+Maj + clic sur une autre → deux cernées ; recliquer → retirée.
  3. Pinceau « encre » en vectoriel, « gouache » en bitmap → chaque sorte de calque retrouve le sien, après relance aussi.
  4. Lasso bitmap, Maj+Suppr → rempli de la couleur ; calque verrouillé → refus.
  5. Transformer > clés de position : clé image 1, aller à 12, tirer le calque → losanges verts ; lecture et export le déplacent.

#### Partie 23 — Onglets de projets (2026-10-20, 17e7d5f ; collage adapté 396b57d)
- Source : partie 23, l. 2151 (listes l. 2259 et l. 2283).
- Statut : en partie remplacé par 135 : plus de « • » d'onglet modifié, plus de boîte au Quitter pour un projet écrit en continu. Jamais vu.
- Manipulations :
  1. Ouvrir trois projets (Cmd+O) → trois onglets ; passer de l'un à l'autre à la souris → tête de lecture, sélection, zoom, espace retrouvés ; Cmd+Z ne recule que l'onglet affiché. (Cmd+Tab est pris par macOS : utiliser Cmd+Maj+Tab ou la souris, voir B.)
  2. Cmd+T → onglet d'accueil ; « Nouveau projet » y prend la place ; Cmd+W ferme l'onglet.
  3. Clic droit sur un onglet : Fermer, Fermer les autres, Afficher dans l'Explorateur (voir B : ne marche pas sur Mac) ; glisser un onglet le déplace.
  4. Copier un plan (Cmd+C dans le montage), autre projet, Cmd+V → plan et dessins copiés, indépendants.
  5. Projet 1080 et projet 4K : coller un plan du 4K dans le 1080 → « mis à l'échelle ×0,5 », un Cmd+Z ; projet carré ↔ 16:9 → fenêtre Contenir / Remplir ; 24 ↔ 25 i/s → « Garder les images / la durée ».
  6. Fermer FEANOR avec trois onglets, relancer → les trois reviennent.

#### Partie 22 — Relecture par le réalisateur (page HTML) (2026-10-20, 89e495b)
- Source : partie 22, l. 2308 (liste l. 2368).
- Statut : jamais vu par Anthony ; la page a seulement été essayée dans un navigateur par l'agent (l. 2313). Fonction de base.
- Manipulations :
  1. Fichier > Exporter pour la production > Relecture → « Seulement les plans modifiés depuis l'envoi » grisé sans envoi, compte juste après un envoi et des retouches.
  2. Exporter 20 à 30 plans avec « PDF » → interface vivante, pas de `.part`, un PDF à trois plans par page avec cadre « Notes ».
  3. Ouvrir le HTML dans Safari, puis sur un iPhone en mode avion → images, texte lisible, « Dessiner » au doigt sans défilement, gomme, « Enregistrer mes notes » télécharge `notes-….json` ; fermer et rouvrir la page → notes retrouvées.
  4. « Lire le plan » → images à la bonne cadence.
  5. Glisser le .json sur FEANOR (ou Fichier > Importer) → bon nombre de notes ; fanions jaunes, panneau Commentaires, dessin du relecteur par-dessus ; Cmd+Z.
  6. Réimporter le même fichier → « Rien de nouveau » ; supprimer un plan puis importer → note « sans plan » ; renuméroter → les notes tombent sur les bons plans.

#### Partie 21 — Effets de matière (2026-10-20, 1df8c81)
- Source : partie 21, l. 2090 (liste l. 2130).
- Statut : jamais vu.
- Manipulations :
  1. Calque vectoriel, icône liseré → contour blanc 4 px ; largeur, douceur, couleur.
  2. Encre qui bave, trait sec (« Changer toutes les 2 images »), dégradé le long du trait, plume (calligraphique, fusain).
  3. « Appliquer pour de bon » → l'effet disparaît, les traits changent, Cmd+Z / Cmd+Maj+Z.
  4. Calque bitmap → dégradé et plume grisés avec bulle.

#### Partie 20 — Effets du trait (2026-10-20, d8b0f81)
- Source : partie 20, l. 2020 (liste l. 2061).
- Statut : jamais vu.
- Manipulations :
  1. Calque vectoriel, icône signature → dessin tracé à 60 %.
  2. « Apparition écrite » sur un plan de 3 s → le dessin se trace en 1 s, clés visibles ; tirer la tête de lecture → suit.
  3. Crayonné multiple, respiration, ondulation, pointillés, désagrégation → chacun à l'œil.
  4. Lecture d'un calque chargé → fluide après le premier tour ; calque bitmap → icônes grisées.

#### Partie 19 — Toile infinie (2026-10-20, 1e73243)
- Source : partie 19, l. 1938 (liste l. 1971) ; correction l. 1992.
- Statut : jamais vu (la correction du 28/10 vient d'un défaut, sans retour vu après).
- Manipulations :
  1. Dézoomer et défiler loin (Espace + glisser) → papier sans bord ni trou.
  2. Dessiner à gauche et au-dessus de la feuille, puis un trait qui traverse le bord → pas de coupure ni scintillement ; Cmd+Z ; fermer, rouvrir → tout est là.
  3. Caméra sur du dessin hors feuille → l'export montre ce dessin.
  4. Pot hors feuille → message ; pipette hors feuille → couleur du trait.

#### Partie 18 — Calque de couleur et Griffonner (2026-10-20 / 21, 1e73243, 51fcf29)
- Source : partie 18, l. 1789 (listes l. 1909 et l. 1926).
- Statut : jamais vu. L'icône palette du panneau Calques passe par le bouton « Calques » de la timeline ; la rangée à part est devenue un liseré (68, puis 160).
- Manipulations :
  1. « Ajouter un calque de couleur » sous un trait fermé, pot → rempli sous le trait sans liseré ; petit trou ne fuit pas.
  2. Pinceau avec « Griffonner » → toute la zone se remplit au lâcher ; retoucher le trait → mise à jour seule.
  3. Griffonner éteint, tache d'une autre couleur → au-dessus du remplissage.
  4. Poser, allonger, supprimer une clé du trait → la couleur suit ; export image et PDF → couleur sous le trait.

#### Partie 17 (l. 1761) — Exporter pour le montage (EDL / XML / OTIO) (2026-10-20, 0176960)
- Source : partie 17, l. 1761 (liste l. 1780) ; docs/EXPORT_MONTAGE.md ; JOURNAL l. 1344 ; complété par 84 (FCPXML, marqueurs).
- Statut : jamais vu ; aucun fichier n'a été ouvert dans un logiciel de montage (JOURNAL l. 1344, AUDIT-2026-10-30 H3).
- Manipulations :
  1. Fichier > Exporter pour la production > Pour le montage → résumé juste ; time code rouge si illisible.
  2. Export → fenêtre vivante, deux barres ; « Arrêter » efface le dossier.
  3. Dossier produit : `media/`, `audio/`, les fichiers de montage, `LISEZ-MOI.txt`.
  4. Importer le FCPXML dans Final Cut Pro, le XML / EDL dans DaVinci Resolve (Mac) → plans, durées, son calés ; un plan en V2 → clip `…_V2`.

#### Partie 16 — Numérotation des plans : suivie ou verrouillée (2026-10-20, 0176960)
- Source : partie 16, l. 1722 (liste l. 1754) ; 86 (verrouillée d'origine, autre lot).
- Statut : jamais vu. Les projets neufs sont verrouillés d'origine depuis 86 (JOURNAL l. 1353).
- Manipulations :
  1. Réglages du projet > Numérotation des plans « Verrouillée » ; nouveau plan après le 2 → « Plan 2A ».
  2. Supprimer le 3 → les autres gardent leur numéro ; nouveau plan après le 2 → 2A, pas 3.
  3. Glisser le 5 devant le 1 → il reste « 5 » ; « Renuméroter les plans… » → confirmation, tout repart de 1 ; Cmd+Z.
  4. Planche, fiche, PDF → « Plan 12A ».

#### Partie 15 (l. 1542) — Envois, plans modifiés, versions de plans et comparaison (2026-10-20, da13a79)
- Source : partie 15, l. 1542 (listes l. 1673 et l. 1703).
- Statut : jamais vu (« envois, versions » jamais touchés, AUDIT-2026-11-10). L'étiquette « V2 » du bloc et sa pile de cartes ont été revues en 168 (« V2 ▸ » dans la timeline) : la liste l. 1673 (points 1 à 3, 6) est à lire avec 168.
- Manipulations :
  1. Storyboard > Envois > Marquer un envoi… sur une copie de projet → nom proposé, date ; `envois.json` et `versions/` dans le dossier ; rien ne fige.
  2. Retoucher un plan → pastille ambre une demi-seconde après le lever du stylet ; Cmd+Z la retire.
  3. « Plan modifié suivant » et « Choisir les plans modifiés » → la tête et la sélection suivent.
  4. Storyboard > Versions du plan > Comparer : côte à côte, volet (à la souris), fondu, superposition ; part de l'image qui diffère.
  5. Fermer, rouvrir → envois, pastilles (aucun faux « modifié »), versions là.
  6. « Supprimer les versions non affichées… » → compte dans la confirmation, grisé sans rien à supprimer.

#### Partie 14 — Espaces de travail, barres minces, « Plus… » ; onglets Montage et Vignettes (2026-10-19 / 21)
- Source : partie 14, l. 1325 (listes l. 1409, 1490, 1531).
- Statut : en partie remplacé : plus d'onglet Animation (100) ; la 3D a son onglet (198) ; barre des panneaux retirée (194) ; colonne en pile (194). Restent : passer d'un espace à l'autre sans rien perdre, Tab, la disposition retenue. Jamais vu.
- Manipulations :
  1. En haut à droite : Vignettes, Storyboard, 3D, Montage → le bon allumé.
  2. Passer d'un espace à l'autre → même plan, même image, même tête de lecture, mêmes plans choisis.
  3. Montage → lecteur grand en vue caméra, bande haute avec toutes les pistes ; Tab → retour au Storyboard.
  4. Fermer un panneau dans le Montage, changer d'espace, revenir → toujours fermé ; Affichage > Espaces de travail > Rétablir la disposition → il revient.
  5. Fermer FEANOR dans le Montage → il rouvre dans le Montage. Barre de la timeline étroite → « Plus… » visible à droite.

#### Partie 12 — Calques d'effets et préréglages d'effets (2026-10-19, 96b510c)
- Source : partie 12, l. 1208 (« À vérifier » l. 1236).
- Statut : jamais vu (l. 1244 : « Pas vu à l'écran »).
- Manipulations :
  1. Ajouter un calque d'effet avec un préréglage (impact, lueur, flash, secousse, vignettage, grain, aberration, timecode) → fenêtre du préréglage lisible.
  2. Dans un dossier → l'effet n'agit que sur le dossier ; hors dossier → sur tout ce qui est dessous.
  3. Incrustation (timecode) → suit le cadre de la caméra à l'export.

#### Partie 11 — Vignettes simples (planche) et exports de la planche (2026-10-21 / 22, 8b77e2c, fde3ca8, c4f59f0)
- Source : partie 11, l. 592 (listes l. 1028, 1040, 1062, 1184).
- Statut : en grande partie remplacé : canevas libre et outil Caméra (101), continuité et papier (76), séquences (77), passe-partout (110), color script dédié (109), traits à travers les cases (111 à 134), « Hors cadres » ; annotations par 46. Anthony a beaucoup utilisé la planche (retours 42, 44, 101). Restent jamais vus : coupe / réunion par la barre, déplacer un plan, dépôt d'image, export de la planche.
- Manipulations :
  1. Cliquer une barre entre deux images → coupe ou réunion ; Cmd+Z.
  2. Maj + glisser le cadre d'une image d'un plan de trois images → tout le plan suit ; Échap en plein geste → rien.
  3. Glisser une image du Finder sur une case, une case vide, une barre → calque « Image » sous le dessin ; Cmd+Z défait le dépôt.
  4. Fichier > Exporter en image > Planche des vignettes en PNG puis PDF A4, avec et sans annotations → ouvrir les fichiers ; « Arrêter » → aucun fichier.

#### Partie 9 — Suggestions sonores (Labo) (2026-10-19)
- Source : partie 9, l. 510 (liste l. 540) ; panels.rs:290.
- Statut : jamais vu. Labo caché d'origine ; envoi en ligne non branché (B).
- Manipulations :
  1. Labo > Suggestions sonores… → panneau, lignes et icônes lisibles.
  2. « Lire le storyboard (proposer des sons) » → propositions ; double-clic → écoute ou génération ; clic droit → menu.
  3. Préférences > Génération → « Démarrer le serveur local », « Tester ».

#### Partie 8 — Transitions, effets sonores, bruitages (2026-10-19)
- Source : partie 8, l. 448 (liste l. 485).
- Statut : jamais vu (« son, transitions » jamais touchés, AUDIT-2026-11-10). Les transitions ont des préréglages depuis 175 (autre lot).
- Manipulations :
  1. Clic droit sur une coupe (± 4 points de la jonction) → menu de transition ; poser un fondu → lecture fluide.
  2. Poignées de fondu aux coins d'un extrait son ; double-clic → ligne de volume.
  3. Panneau Bruitages (« + Panneau » ou menu Son > Bruitages…) → liste longue, défilement, glisser un son vers la bande.
  4. Lecture du montage avec son → sans décalage (ETAT l. 201, item 8).

#### Partie 4 et 7 — Cohérence des gestes des trois sortes de clés, zoom de la bande (2026-10-19)
- Source : partie 4, l. 234 (4.4 l. 277, 4.6 l. 329) ; partie 7, l. 420.
- Statut : jamais vu (l. 422 : « Rien de cette passe n'a été regardé »). Le choix de clés a été étendu en 132 et 138 (autres lots).
- Manipulations :
  1. Montage : rectangle depuis un espace vide sur plusieurs pistes → clés prises ; Cmd + clic bascule ; Maj + clic → rectangle ; barre d'état « 12 clés sur 3 calques ».
  2. Cmd+C / Cmd+V de clés sur plusieurs calques → les calques manquants sont créés sous la pile.
  3. Clic droit sur une clé de dessin, de caméra, d'effet → cinq commandes communes dans le même ordre.
  4. Zoom de la bande (curseur, + / −, Option+1 / Option+2, Cmd + molette) → la tête de lecture reste au même endroit de l'écran.

### Autres sources (docs, suivi, audits) (lot 7)

#### Notes d'Anthony au statut « proposé, en attente de ton test » (suivi/NOTES.md l. 15-33, 2026-10-30/31)
- Source : NOTES.md l. 15 à 33 ; une ligne par note, toutes « proposé », aucune marquée « résolu ».
- Statut : jamais vu, sauf mention. Correspondance : Panneau Scénario (partie 75, 32dd5f5) ; recherche Ctrl+F du Scénario (78, 4d8677d) ;
  Vignettes ajustements (76, 5335c60) ; Vignettes par séquences (77, 7e3f7e8) ; hors-cadres texturé et passe-partout à l'export (79, e546fc7) ;
  Dégradé avancé (80, 3774514) ; planche de Références (81, 1df74a1 ; plus tard 108) ; Bibliothèque (83) ; fusion et menu des calques (82,
  1e6ff18) ; pipeline étape 1 (87, 3cd07b7) et Kitsu (93, c0e8bba) ; recommandations d'audit 1-4 (88 fil de décision, 89 premier contact, 91
  petits livrables, 92 journal des différences) ; noms de marque / « Papier » (90) ; logo retiré à côté de Fichier (1c5a102, fusionné).
- Manipulations (Mac : Cmd à la place de Ctrl) :
  - Fenêtre Scénario ouverte, Cmd+F, taper un mot sans accent → toutes les occurrences surlignées, compteur, Entrée = suivante, Échap ferme.
  - Clic sur une scène du panneau Scénario → le plan correspondant devient courant.
  - Vignettes : clic droit sur le fond, créer une séquence depuis des cases ; choisir l'affichage d'une seule séquence → chaque séquence
    commence sur une rangée avec son titre, l'ordre du film ne change pas.
  - Outil Dégradé : deux pastilles de couleur + une d'opacité, forme Losange, puis retoucher l'angle en chiffres sans changer d'outil.
  - Menu clic droit d'un calque → « Fusionner vers le bas » et « Fusionner les choisis » trouvables au premier niveau (au plus ~11 entrées).
  - Kitsu : nécessite un vrai serveur (jamais essayé, V1.md l. 108) ; sur Mac le jeton ne peut pas être gardé (voir E).
- Remarque : « Points 1 à 4 » et « pipeline » détaillés par les lots INTERFACE.

#### ESPACE3D.md §10 — liste de contrôle de l'Espace 3D simplifié (2026-10-21, partie 13)
- Source : docs/ESPACE3D.md l. 184-198 (« rien n'a été regardé »).
- Statut : jamais vu ; en partie remplacé : le panneau Calques n'existe plus (partie 160), le décor 3D est revenu (180, donc « aucune mention
  d'objet, de décor, de lumière » est caduc), l'onglet 3D existe (198). Reste valable : cartes, poignées, caméra.
- Manipulations :
  - Ouvrir l'Espace 3D d'un plan à deux calques → chaque calque est un rectangle texturé ; clic sur une carte = calque courant.
  - Poignées : flèches rouge/vert/bleu, survol en jaune ; tirer une flèche déplace la carte sur un seul axe.
  - « Regarder par la caméra » → la vue montre le cadre de la caméra du plan.
  - Ouvrir une copie d'un ancien projet qui avait des objets 3D → calque « Fond 3D » présent, mêmes pixels, verrouillé.

#### FIABILITE.md §8 — à regarder à l'écran (2026-10-19)
- Source : docs/FIABILITE.md l. 164-170.
- Statut : jamais vu.
- Manipulations : lancer avec beaucoup de pinceaux importés → la liste grossit après quelques secondes, « Mes brosses » juste au lancement dit
  « se chargent encore » ; dernier projet sur un disque débranché (clé USB retirée) → l'accueil s'ouvre sans attente ni message.
  (`SHOT_DEMARRAGE=1` et `%LOCALAPPDATA%\SHOT\demarrage.log` : Windows seul, sans objet sur Mac.)

#### AUDIT-RENDU-BROSSES.md §6 bis/ter — A/B des brosses à la tablette (2026-10-18)
- Source : docs/AUDIT-RENDU-BROSSES.md l. 368-377 et 395-411.
- Statut : vu en partie (Anthony a donné un retour en 6 ter : mordant du crayon, grain « sable », « Doux ») ; la courbe et la pointe refaites
  après ce retour ne sont pas notées comme revues.
- Manipulations : Crayon graphite sur calque bitmap puis vectoriel : hachures légères (grain visible ?), trois passages au même endroit
  (ça fonce), appui à fond (gris très foncé, pas noir) ; « Grain du crayon » Soie / Doux / Moyen / Dur ; Craie et Gouache, aplat croisé deux
  couleurs. **Sur Mac l'appui n'arrive sans doute pas (voir E) : ce test n'a de sens que sur Windows.**
  (Comparaison avec ses fichiers TVPaint : PC d'Anthony seul.)

#### ETAT.md §4-5 — « à vérifier à la main » (2026-10-30)
- Source : suivi/ETAT.md l. 184 et 191-203.
- Statut : jamais confirmé dans une source plus récente, sauf l'usage quotidien du dessin (compteur, AUDIT-2026-11-10).
  Écarté : point 7 (fichier Storyboard Pro, retiré 17dda2a).
- Manipulations :
  - Fenêtre d'accueil : créer un projet → la fenêtre s'agrandit ; fermer le projet → la petite fenêtre revient ; récents ; engrenage.
  - Agrandir / restaurer la fenêtre, la passer sur un second écran, interface à 150 % (Préférences) → rien ne se coupe.
  - Calque Couleur unie noir + clés d'opacité 0 → 100 % → fondu au noir visible en lecture (Espace).
  - Lecture du montage avec un son importé → pas de décalage image/son.
  - Export MP4 et PDF ouverts dans un lecteur → images, caméra, parallaxe présentes (sur Mac ffmpeg doit être indiqué à la main, voir E).

#### V1.md « Écarts » faits depuis, jamais vus (parties 102, 103, 104, 105, 109, 113, 124)
- Source : suivi/V1.md l. 66-108 ; corrections détaillées en C.
- Statut : jamais vu (INTERFACE 102.x, 103.x « À vérifier à l'écran », 124 « propositions à valider »).
- Manipulations (les plus parlantes) :
  - Cmd+Maj+D sur un plan → dupliqué ; Maj+C → outil Caméra ; Maj+K → losange de clé de caméra à la tête de lecture.
  - Vignettes, Maj+Tab → dessin du Storyboard ; Storyboard, Maj+Tab → retour planche.
  - Vignettes : touche d'un outil interdit (ex. remplissage) → rien ne change, la ligne d'état dit « Dans les Vignettes : pinceau, crayon, gomme et caméra ».
  - Plan avec V1 et V2, F5 (fn+F5 sur Mac) au milieu → fenêtre « Ce plan a 1 version rangée. Elle va avec : » ; choisir ; Cmd+Z = un pas.
  - Séquence à versions A/B, supprimer son dernier plan → refusé avec message, rien ne bouge.


## Partie 2 — Limites et bugs connus encore ouverts (248 points, dont 28 propres au Mac)

### Parties 189 à 201 (lot 6)

- Premier clic perdu dans les Préférences, vu sur le banc seulement — JOURNAL l. 99 (et l. 929 : « le premier clic après l'apparition du pointeur se perd ») — non vérifiable sans lancer, à guetter.
- Lampes : pas d'ombres, pas animables, pas de lumière sur les cartes de la vue Espace 3D (l'image du plan les a depuis 201) — 200.3 l. 18156, 201.3 l. 18356 — non vérifiable sans lancer.
- Enlever le dernier objet d'un décor fait disparaître ses lampes à l'enregistrement — 200.3 l. 18161 — non vérifiable sans lancer.
- Clés d'objet comptées en images de la séquence : rallonger un plan avant elles les décale — 200.3 l. 18159 — non vérifiable sans lancer.
- Objet animé : rotation verticale seule, clés toujours adoucies ; la rangée « Objet choisi » ne montre que le premier objet choisi — 200.3 l. 18158, 18162 — non vérifiable sans lancer.
- Étiquette « qui a la main » pas mise à jour quand un objet bouge sous un pointeur immobile ; zoom de la timeline de la séquence oublié entre séances — 201.3 l. 18362-18364 — non vérifiable sans lancer.
- Lâcher des objets sur le panneau Scène du Storyboard : code prêt, mais impossible d'avoir les deux panneaux ensemble, et pas d'essai — 201.3 l. 18359 — non vérifiable sans lancer.
- Caméras des autres plans non sélectionnables dans la vue (seulement par Scène 3D) ; un clic Banque 3D pose devant la caméra du plan, pas au centre de la vue — 198.3 l. 17871-17872 — non vérifiable sans lancer.
- Groupes egui_dock faits à la main sans noms d'onglets — 198.3 l. 17874, 194.4 l. 17387 — non vérifiable sans lancer.
- Aspect de la vue 3D choisi seulement dans l'onglet 3D (pas de bouton dans la vue Espace 3D du Storyboard) ; export sans carte graphique sans ombres du décor ; palette fixe de dix couleurs — 199.3 l. 18019-18022 — non vérifiable sans lancer.
- Scène du Storyboard : « Renommer… » au clic droit ouvre le champ dans la timeline, pas dans le panneau ; pas de glisser de plans ou séquences ; pas de menu sur les lignes de plan et séquence ; 230 pt en haut de pile, qui serrent Propriétés dans une fenêtre de 1080 — 197.3 l. 17750-17757 — non vérifiable sans lancer.
- Lignes Séquence et Plans restées en haut : on ne peut pas y glisser un plan ; lisibilité du texte à 8,5 pt dans 10 pt non jugée — 196.3 l. 17608-17612 — non vérifiable sans lancer.
- « Préréglages de caméra… » n'est plus accessible qu'au clic droit de la ligne Caméra (ou par « + Panneau ») — 196.3 l. 17614 — vérifié : tableau 196.1.
- Règle des temps restée en haut : on n'y prend ni signets, ni marques, ni entrée/sortie, ni fanions — 195.3 l. 17508 — non vérifiable sans lancer.
- Les anciens réglages sans clé passent à 16 pt et 90 % (changement d'apparence au premier lancement) — 195.3 l. 17510-17513 — confirmé par 195.1 points 1-2.
- Zoom et rotation en double (barre du haut et curseurs du bas), en attente de la décision d'Anthony — 195.1 point 9 l. 17465 — non tranché dans les parties suivantes.
- Les petites vidéos d'aide coupées hors du menu Aide, code éteint (`AIDE_HORS_DU_MENU = false`) — 195.1 point 6 — vérifié : src/aideanim.rs l. 472.
- Propriétés 31 pt plus étroit (Préréglages et Bibliothèque sur une seule rangée) ; la poignée de 8 pt range toute la colonne ; onglets de Propriétés non réordonnables ; un FEANOR plus ancien ne lit pas la disposition v10 — 194.4 l. 17383-17394 — non vérifiable sans lancer.
- Bords doux : seulement 5 niveaux ; un peu de ciel peut se mêler au bord ; bords nets au-delà de 0,7 M triangles, pour une feuille de plus de 2,5 Mpx en rendu de secours, et tant que le budget n'est pas mesuré (premier lancement) — 193.5 l. 17252-17267 — non vérifiable sans lancer.
- Barre de la timeline de l'Animation encore sur deux lignes (« À lui de dire ») ; alignement de la seconde rangée (« ce qui lui paraît encore bas : je ne l'ai pas retrouvé ») — 192.1 l. 16761, 192.10 l. 16945 — aucune réponse dans les parties suivantes.
- Réglages encore en rangées de boutons (thème, hauteur des plans et des pistes son, anneau, génération sonore) — 192.17 l. 17084 — non changés depuis.
- Dupliquer la séquence : le son n'est pas copié et ce qui suit est décalé par rapport au son ; plans écartés et transition d'entrée non repris ; séquence lourde jamais mesurée — 191.4 l. 16688-16695 — non vérifiable sans lancer.
- Objets du décor : Édition > Copier/Coller (le menu) ne vise pas les objets ; la main au clavier (décor ou dessin) ne se voit pas, l'étiquette de 201 montre le choix, pas la cible de Cmd+C ; les versions de séquence partagent le décor ; coller à côté après des annulations peut aller un cran trop loin — 190.4 l. 16551-16558 — non vérifiable sans lancer.
- Ombres dans la vue : sans les effets du calque ; une sélection déplacée sans ombre pendant le geste ; un dessin neuf ombré à l'image suivante ; pas d'ombre colorée ; rendu de secours sans ombres — 189.5 l. 16414-16422 — non vérifiable sans lancer.
- Mesures de performance à refaire sur le PC d'Anthony (189.3, 193.3) — hors audit Mac.

### Parties 170 à 188 (lot 5)

- C11 « Panoramique de suivi d'un calque » pas fait — 171.3 l. 14441 — aucun préréglage de suivi dans `camfx.rs`.
- Flou de vitesse du zip pan (C8) pas fait ; C28 (bascule de point) pas fait — 171.3 l. 14443, 177.3 l. 14992 — non vérifiable
  sans lancer, aucune partie plus récente ne les reprend.
- Les blocs de préréglages ne se voient pas dans le Montage et ne se copient pas d'un plan à l'autre — 170.3 l. 14347 — la 178 les
  montre dans la timeline du Storyboard seulement ; rien sur le Montage depuis.
- L'intensité d'un bloc ne se règle pas en le tirant vers le haut — 170.3 l. 14348 — non repris.
- Le sujet d'une orbite ou d'une grue est pris au plan de référence, pas à la profondeur du calque ; une orbite autour d'un
  calque en profondeur tourne autour d'un autre point — 172.3 l. 14544 — non repris.
- Le sujet d'une trajectoire toute faite ne se déplace pas à la main ; le survol décolle depuis la caméra du plan ; une orbite
  de plus de 80° voit les cartes par la tranche — 172.3 l. 14547-14551 — non vérifiable sans lancer.
- Raccord filé : la jointure reste nette au milieu de la traînée ; pas d'objet à soi pour le « volet par un objet » ; dans une
  vue tournée, la découpe d'une poussée suit le rectangle qui contient le cadre — 175.3 l. 14852-14854, 177.3 l. 14992 — toujours
  listés comme non faits en 177.
- La ligne de la carte (P17) ne se retouche pas à l'outil Trajectoire ; « Précédemment » prend le milieu des plans ; le lettre à
  lettre part du début du plan ; le logo se choisit à chaque pose — 176.3 l. 14911-14915 — non repris.
- Le compte à rebours suppose le cadre au milieu de la feuille ; l'horloge part de midi dans le sens horaire seulement ; l'iris
  n'a que le rond — 177.3 l. 14988-14991.
- Les aperçus des cartons faits d'incrustations (titre de séquence, plan manquant, ardoise) sont noirs — 177.3 l. 14989 — non
  vérifiable sans lancer (la vignette est devenue un aperçu unique à la 195, rien ne dit que c'est réglé).
- « Préserver la transparence » et « verrouiller la position » n'agissent que sur le calque cliqué (décision attendue d'Anthony) —
  174.1 l. 14740 — confirmé par la 192.16 (« la position, sur le calque cliqué, comme avant »).
- Choix faits sans Anthony, à confirmer : un dossier caché ou verrouillé ne le montre que sur sa ligne — 179.2 l. 15141 — pas de
  réponse trouvée.
- Le quadrillage du sol de la vue ne se fond pas dans la brume ; pas de mipmaps (une texture fine scintille de loin) ; `BLEND` lu
  comme opaque, couleurs de sommets et `KHR_texture_transform` ignorés ; un décor d'instances grossit une fois allégé ; les calques
  séparés exportés n'ont pas le décor — 180.5 l. 15345-15359 — non vérifiable sans lancer.
- Sans carte graphique : pas d'ombres (décor ni cartes), et le décor suit la caméra avec un temps de retard — 180.5, 185.5
  l. 16111 — non repris.
- Les couleurs du ciel, de la brume et de l'ambiance sont des constantes — 180.5 l. 15358 — la 198 (Propriétés > Monde) a ciel et
  brume en cases ; rien ne dit que leurs couleurs se règlent, non vérifié.
- Un modèle lâché sur le dessin se pose devant la caméra, pas au point lâché ; la recherche de la banque est en anglais — 181.4
  l. 15515, 15525 — non repris.
- L'aimant ne lit que neuf verticales : une pointe fine peut entrer un peu dans l'objet du dessus, un dessous irrégulier peut
  flotter, un objet ouvert peut tromper le test ; changer la taille du cadre ne repose pas les objets — 181.4 l. 15517-15520.
- Un calque d'ajustement résolu par le processeur ignore la profondeur partagée et les ombres des cartes ; dans la vue 3D, le voile
  d'une carte transparente peut recouvrir une autre carte — 183.5 l. 15809-15817, 185.5 l. 16123 — non repris.
- « Modifier l'origine » ne vise qu'une chose choisie seule ; pas d'aimantation à la grille ; un objet ne s'incline pas ; un objet
  masqué « lourd » ne se prend pas dans la vue ; ⌫ n'enlève pas un objet dans la vue (voulu) — 184.4 l. 15948-15957 — la 200.6
  confirme « pas de rotation hors de la verticale ».
- Ombres : le bord est plus net sur une carte que sur le décor ; rien hors de la boîte du décor ; à l'export processeur, l'encre
  qui sort de plus d'une demi-feuille ne porte pas d'ombre ; pas d'ombre translucide (une carte à 30 % n'en porte aucune) ; une
  lueur large fait une ombre large — 185.5 l. 16112-16122.
- Raccourcis : si la barre d'état ne dit rien alors qu'une touche ne fait rien, la cause est inconnue (non reproduit) — 186.3
  l. 16188.
- Espace 3D à côté : le côte à côte n'est pas retenu d'une séance à l'autre ; la vue montre les dessins bruts (sans effets ni modes
  de fusion) ; la rangée d'outils est coupée quand elle est étroite ; aucune touche par défaut — ESPACE3D 13 limites 3-4, 187 l. 16211
  et l. 16235.
- Les versions dépliées se lisent sans la plage de lecture — 188.2 l. 16280.
- Lignes Séquence et Plans restées en haut : on ne glisse ni plan ni séquence (on les choisit seulement) — 196.3, touche la 178.
- Projets : les formats 57 → 61 sont refusés par une version précédente (170 à 172, 176, 182) : garder une copie avant l'essai.

### Parties 150 à 169 (lot 4)

- Échap pendant qu'on glisse sur la timeline la clé de position d'un jalon (carte en clés d'images) rend chemin et clés de position,
  pas les clés de dessin — INTERFACE 152, l. 11978 ; JOURNAL l. 658 — code : `poskeysui.rs:496` `cancel_pos_drag` ne rend que
  `pos_keys` et `trajectoire`, toujours ouvert.
- La caméra choisie seule n'existe que vue Espace 3D ouverte (vue fermée la piste « Caméra » ne se montre pas choisie) — 152, l. 11981 — non vérifiable sans lancer.
- Lasso dans la vue 3D : un seul calque (pas de multi-calques, pas de sélection par couleur), rien sur une carte vue de dos ;
  Transformer, pot, dégradé, formes, texte, déformation ne prennent pas le pointeur dans la vue — 152, l. 11990-11995 ; ESPACE-LIBRE
  §5 l. 97 — non vérifiable sans lancer.
- Sélection déplacée dans la vue montrée à 1 024 px de large, nette au lâcher — 152, l. 11996 — non vérifiable sans lancer.
- Jalon posé à côté d'une clé posée à la main : la clé ne tient qu'une image ; dessins posés à la main ne suivent pas le chemin ;
  les copies de calques ne renumérotent pas les dessins de la trajectoire — 152, l. 11966-11977 — non vérifiable sans lancer.
- Lissage : la fenêtre est une part de la longueur, après un prolongement l'ancienne partie est relissée plus fort ; prolonger ne
  change pas la durée (le sujet va plus vite) ; pas de raccourci pour « Lissage » ; sur la feuille le prolongement ne vaut que pour
  le calque courant — 151, l. 11518-11526 — non vérifiable sans lancer.
- Poignées : le temps se mesure à plat (plus vite sur une pente) ; un chemin lissé passe près de la poignée, pas dessus ; un retracé
  remet le chemin à plat ; après Cmd+Z la poignée choisie peut désigner sa voisine ; le texte d'aide « Trajectoire dans l'espace »
  ne dit pas « ailleurs que sur le chemin » — 154, l. 12296-12314 — non vérifiable sans lancer.
- L'environnement 3D d'un projet partagé n'est pas gardé (revient à l'origine) — 157.1, l. 12472 — non vérifiable sans lancer.
- Caméra libérée reste libre ; une image passée par le changement de repère ne filme pas l'encre hors feuille (elle peut apparaître
  / disparaître en cours de plan) ; un calque tenu à la borne ne passe plus derrière la caméra ; contenu rogné non migré —
  159.7, l. 12843-12850 — non vérifiable sans lancer.
- Messages de la barre d'état des Vignettes disent « calque d'annotation » quand les menus disent « couche » — 160.2, l. 12926 —
  code : 40 occurrences de « calque d'annotation » hors essais dans `src/`, toujours ouvert.
- `vignettecam::dans_polygone` répond « dedans » pour un polygone sans aire (repérage de la planche) — 161.3, l. 13000 — code :
  `vignettecam.rs:228` inchangé depuis 593f094, toujours ouvert.
- Panneau 3D : « Au sol » / « Mur » tournent sur place (un sol flotte) ; distance de la réglette fausse pour une caméra très tournée ;
  le plateau ne tire pas la caméra ; « Précis » oublié d'une séance à l'autre — 162.4, l. 13144-13151 — non vérifiable sans lancer.
- Pas de pictogramme Tremblement dans les mouvements de caméra des Vignettes ; pas d'effet « Nuit » — 163.2 l. 13216, 164.2
  l. 13375 — question à Anthony jamais tranchée (164.7 point 4).
- Avant | après : repli sur le schéma pour les effets sans force, de déformation, liseré, encre qui bave, effets du trait — 165.1,
  l. 13505 — non vérifiable sans lancer.
- Vue nette absente : plan sans papier, calque à effets, carte inclinée 3D, caméra libre, perspective, lecture, cases des Vignettes ;
  l'export et une caméra serrée restent en pixels de la feuille ; brosses à grain rejouées à l'échelle (aspect différent) ; dessins
  lourds à fort zoom non mesurés — 166.4, l. 13771-13790 — non vérifiable sans lancer.
- Vue réduite : hors-feuille, affichage sans carte graphique, calque réduit dans la composition gardent une seule lecture ; coût des
  vignettes jusqu'à 1,6 fois, plan lourd 0,3 s au lieu de 0,1 — 166.9 l. 13925-13930, 166.10 l. 13984-13993 — non vérifiable sans
  lancer.
- Versions : une version rangée avant le 18/11 n'a pas de son gardé ; fondus et volume du son jamais gardés ; le dépliage ne survit
  pas à la fermeture — 168.7, l. 14166-14178 ; JOURNAL l. 432 — non vérifiable sans lancer.
- Cutter : un trait effilé perd ses appuis enregistrés ; dans une case des Vignettes la portée est le calque actif seul ; écart
  d'origine 0 (à décider) — 169.1, 169.3, l. 14214, 14222, 14277 — non vérifiable sans lancer.
- La vue Espace 3D montre le dessin brut des cartes, sans effets ni modes de fusion — ESPACE-LIBRE.md l. 453 — non vérifié dans
  cette tranche.

### Parties 120 à 149 (lot 3)

- Calque d'ajustement : il s'arrête au bord de la feuille (trait « ajusté » puis « non ajusté ») — INTERFACE 121, l. 8734 — non vérifiable sans lancer, aucune partie postérieure ne le reprend (grep).
- Flash ou impact suivi d'un flou sur un dessin qui dépasse : le fond rempli déborde de la portée du flou — 121, l. 8735 — non vérifiable.
- Plume (effet du trait) jamais mesurée au bord — 121, l. 8736 — non vérifiable.
- Aperçu réduit en lecture ou défilement de loin : sans ce qui lit au-delà de la feuille ; bande près du bord qui casse un instant pendant un geste — 121, l. 8737 et 8740 — non vérifiable.
- Le calque qu'on édite se montre sans ses effets du trait (il faut choisir un autre calque pour les voir) — 121, l. 8738 — non vérifiable.
- Export « sans les cadres » d'un plan à cadres tournés : la case montre encore l'image de A tournée — 122, l. 8828 — non vérifiable.
- Trait d'un pixel plus pâle dans la tuile d'un plan à cadres tournés — 122, l. 8826 — non vérifiable.
- Durée avec Ctrl : trois choix « à faire valider par Anthony » (bord gauche, réserve qui revient d'abord, images ajoutées vides ; teinte `PAS` 0,075, cles_alternees.rs:24) — 123, l. 8911-8916 — aucune réponse trouvée dans le journal.
- Saccade de caméra dont le rythme ne suit pas un décalage Ctrl ; Annuler d'un export jamais cliqué dans le vrai exe — ETAT.md l. 6 — non vérifiable.
- Version portable : l'archive des sources ffmpeg (LGPL) n'est pas jointe alors que CONFIGURATION.txt la dit « ci-jointe » — 124, l. 8959 ; JOURNAL l. 1207 — rien dans le dépôt ne la joint (grep « tar.xz » : seulement docs et aproposui.rs). Bloquant juridique avant livraison.
- Planche, vectoriel : les flèches du clavier ne déplacent pas le choix de la planche — 125, l. 9035 ; 127, l. 9180 — confirmé : aucune gestion des flèches dans vignettesel.rs / vignetteselbmp.rs.
- Le choix de la planche est levé dès qu'un autre geste change l'historique (Cmd+Z compris) — 125, l. 9036 — non vérifiable.
- Aperçu du geste de la planche sans le grain du pinceau, ancienne place visible jusqu'au lâcher — 125, l. 9034 ; 127, l. 9178 — non vérifiable.
- « Relier les morceaux » (anciens traits coupés) proposé, non fait — 125, l. 8987 — pas de reprise trouvée.
- Compte d'étapes de `finish_creation` (création d'un plan par un trait dans une case vide) non corrigé — 125, l. 9030 — non vérifiable.
- Lâcher d'une très grande sélection bitmap : jusqu'à ≈ 2,6 s (déplacer) / 4,7 s (tourner) sur le fil de l'interface, sans barre d'avancement — 127, l. 9172, 9177 — limite LIMITE_TUILES = 7 680 toujours là.
- Un trait bitmap repeint ou gommé, même un peu, ne se suit plus entier (choisi case par case) — 127, l. 9179 — non vérifiable.
- Film partagé sans le jeton du montage : les traits **vectoriels** posés sur le papier se perdent à l'enregistrement — 127, l. 9170 — toujours ouvert : seul le bitmap est gardé (`planche_bitmap_ecrivable`, horschamp.rs:869-877).
- Empreintes bitmap non compressées (≈ 75 Ko par trait réparti) — 127, l. 9108, 9173 — non fait.
- Couper ou rogner au milieu d'un intervalle adouci : quelques images changent légèrement — 129, l. 9371 — non vérifiable.
- Deux plans sans clé côte à côte, chacun avec un effet : convertir le premier seul peut changer le second ; insérer des images sur certains calques ne convertit pas l'effet — 129, l. 9386-9387 — à recouper avec 170 (blocs).
- Vignettes : un zoom vivant ne dessine pas ses cadres A/B (choix « à valider ») — 129, l. 9368 — non vérifiable.
- Grande fenêtre « Exporter » toujours joignable (palette, Préférences > Export, visite) ; astuce « Maj : à l'horizontale » plus écrite dans le panneau Transformer — 130, l. 9292, 9299 — non vérifiable.
- Règle A : caméra en perspective → le trait qui sort d'une case s'amincit, non continu sur la planche ; au-delà de 4 cadres un trait ne se voit plus sur la planche ; vignette jusqu'à 4096 px (64 Mo) ; export marge bornée à 12 288 px — 131, l. 9502-9516 — `MARGE_MAX_CADRES` toujours dans render.rs:1122.
- Règle A : le trait d'une case qui passe dans l'image d'une autre ne se gomme pas depuis cette autre ; la gomme de la planche ne touche ni Décor, ni planche-mère, ni calques verrouillés ; un plan vectoriel qui naît ne prend pas les morceaux bitmap du papier ; dessiner vite : le bout hors cadre peut manquer un instant — 131, l. 9505-9512 — non vérifiable.
- Déformer sur beaucoup de dessins : lâcher ≈ 0,3 s pour 200 dessins sur le fil de l'interface ; Déformer ne reprend pas une sélection partielle sur plusieurs calques ; pot, cutter, pinceaux restent sur un calque — 132, l. 9752-9755 — non vérifiable.
- « Peint sur l'aplat » : avec plusieurs clés choisies, un trait hors de la forme garde son étape ; la vue Dessin n'a pas le signal de la barre du bas — 133, l. 9833-9836 — non vérifiable.
- Ctrl+A des Vignettes ne prend qu'un calque même si plusieurs sont choisis — 134, l. 9875 — confirmé dans le code (vignettesel.rs:1369, message « le choix se fait sur un calque à la fois »).
- Un trait du papier glissé dans une case arrive dans le calque courant, pas dans celui où il a été tracé (« à décider par Anthony ») — 134, l. 9899 — pas de décision trouvée.
- Écriture continue : l'annulation ne survit pas à la fermeture ; un projet sans dossier garde la question ; revenir à un jalon remet l'onglet en dernier ; dossiers synchronisés (iCloud, Dropbox) envoient chaque jalon ; fichier de projet unique non fait — 135, l. 9966-9970 — non vérifiable.
- Propositions 34, 35, 36, 38 (calques cachés en une ligne, noms, retrouver un calque par son dessin, « n-ième calque de chaque plan ») non faites — 137, l. 10096 — non vérifiable.
- Dossier : on ne pose pas un calque tout en bas d'un dossier d'un seul geste ; collage « plusieurs dessins » sur une image sans dessin : rien n'est collé — 139, l. 10267, 10282 — non vérifiable.
- Nouvelle clé sur plusieurs calques : un calque verrouillé choisi est laissé de côté alors qu'un calque verrouillé seul reçoit la clé (« à décider ») — 139.5, l. 10310 — toujours ouvert (nouvellescles.rs:64 « verrouillé »).
- Les calques agrandis n'ont que deux hauteurs ; menu « Déplier la piste » sur une piste déjà grande — 140, l. 10340-10342 — non vérifiable.
- Calque accroché : bascule et panoramique de la caméra non suivis ; dans l'Espace 3D et la vue de dessus il est montré à sa pose dans le cadre ; sa piste n'a pas de marque — 141, l. 10392-10396 — à recouper avec 201 (« calque devant la caméra »).
- Mouvement de case : il dure toute la case ; déplacer au montage la clé de tenue lui fait perdre sa case ; l'outil Caméra souligne toujours la première case ; japonaise et export de planche sans essai propre — 142, l. 10440-10443 — non vérifiable.
- Espace 3D : deux cartes qui se traversent ne sont pas départagées point par point ; un dossier passe en bloc ; outils 2D (sélection, Transformer, gomme de débord) posent leurs boîtes comme sous la caméra de référence ; Transformer sur carte inclinée ; netteté d'une carte au sol de près — ESPACE-LIBRE.md l. 90-100, 287-297, 332-341 — non vérifiable.
- Poignées de rotation d'une carte près de ±90° peuvent sauter — ESPACE-LIBRE.md l. 424 ; 143 point 15 — Claude n'a pas vu de saut (85-95°, 80-125°), jamais vu par un humain.
- Vue Espace 3D : dessin brut des cartes, sans effets ni modes de fusion ; on peint sur la carte courante même si une autre la cache, sans signe à l'écran — 146, l. 10754-10766 ; ESPACE-LIBRE l. 453 — non vérifiable.
- Dans la vue : pot de peinture, dégradé, formes, Transformer, texte, coller une image absents ; gomme multi-calques limitée au calque courant ; pas de guides de perspective ; dessiner sur une carte vue de dos refusé — 146, l. 10754-10760 (sélection ajoutée par 152) — non vérifiable.
- Une carte inclinée « d'un plan ordinaire » accepte un trait hors de ses bords qui ne se voit pas ; près de l'horizon la dernière maille peut cacher un trait — 146.1, l. 10834-10838 — à recouper avec 159 (plus deux sortes de plans).
- Ralentissements dans la vue Espace 3D signalés par Anthony le 12/11 au matin : cause inconnue, rien changé (piste : chaque carte peinte deux fois en transparence) — 147, l. 10910-10916, 10924-10930 ; JOURNAL l. 808-810 — non reproduit sur deux PC. À surveiller sur Mac.
- Entre deux clés de position à des profondeurs différentes, la carte ne garde pas sa taille dans l'espace (enfle ×1,7 puis ×0,5) — 148, l. 11046 ; 149 l. 11236 — aucune correction trouvée après (grep).
- Dès qu'une carte passe la borne, toutes les cartes du plan sont posées par maillage : le dessin hors feuille n'est plus filmé ; carte tout contre/derrière la caméra limitée à 10× la feuille — 148, l. 11053-11060 — non vérifiable.
- Champ « Échelle » affiche 1000 % (valeur interne) pour une carte derrière la caméra — JOURNAL l. 753-754 (vu par Claude) — pas de correction trouvée.
- Trajectoire 3D : le chemin de la caméra ne suit pas la caméra déplacée à la main ; pas de silhouettes aux jalons dans la vue ; un chemin tient sur une seule surface ; le miroir d'un calque ne retourne pas son chemin ; un calque à trajectoire « feuille » ne peut pas en prendre une dans l'espace — 149, l. 11222-11236 — non vérifiable.
- Un plan sans clé de caméra qui suit un plan à trajectoire de caméra ne reprend pas la caméra là où le chemin l'a laissée — JOURNAL l. 778 — non vérifiable.
- Échap pendant le glissé, sur la timeline, de la clé de position d'un jalon (carte en clés d'images) ne rend pas les clés de dessin — JOURNAL l. 658-660 — « vu en lisant le code, pas réglé ».
- Essais instables non expliqués (`cases_tests::uniting_two_plans_...`, `scrubbing_far_away_never_cuts_the_stroke_again`, `retours_aplat_tests::a_fill_stays_on_screen…`) — ETAT l. 7 ; 127 l. 9126 ; JOURNAL l. 1095 — non vérifiable.

### Parties 76 à 119 (lot 2)

- Mac : tous les « Ouvrir le dossier » / « Montrer dans l'explorateur » / « Aide > Mes frictions notées » / clic droit Bibliothèque
  « Ouvrir le dossier » lancent `explorer.exe` : rien ne s'ouvre sur Mac — parties 83, 85, 92, 112 — vérifié : `src/exportui.rs`
  l. 1082-1099 (`Command::new("explorer.exe")`, aucune branche macOS), appelé par 13 endroits.
- Mac : Kitsu « Rester connecté » impossible (DPAPI Windows seulement) — partie 93 l. 6947 — vérifié : `src/kitsu.rs` l. 884-887
  (« Le jeton ne peut être gardé que sous Windows »).
- Mac : PDF (storyboard, planche, journal, color script) sans police embarquée : `fonts_dir` ne lit que `%WINDIR%\Fonts`, repli sur les
  polices standard d'avant — parties 84, 91 — vérifié : `src/pdffont.rs` l. 185-188. Accents hors WinAnsi à regarder.
- Mac : Références « Écran 2 » absent (liste vide hors Windows) — partie 81 — vérifié : `src/planchref.rs` l. 599-602.
- Mac : banc du stylet, bouton du stylet lu auprès de Windows et gomme au dos inexistants — partie 114 — vérifié : `src/stylet.rs`
  l. 369 / 543 (`cfg(all(windows, …))`).
- Mac : Bibliothèque sans « lecteurs » (seulement « / ») — partie 83 — vérifié : `src/biblio.rs` l. 659.
- F9 : trois notes lancées le 8 novembre, dossier `Documents\SHOT\Frictions` vide, « à vérifier » — AUDIT-2026-11-10 l. « Les notes de
  friction » — aucune correction depuis (`git log -S"frictions.md"` : seul b76d29d, le commit d'origine) ; non vérifiable sans lancer.
- Morceaux pâles des cases 15 et 16 : non reproduits, « à confirmer à l'écran » — INTERFACE l. 7766, ETAT l. 18 — rien de plus récent
  ne les cite ; non vérifiable sans lancer (la règle A de 131 a pu les faire disparaître).
- Décision 6 de l'analyse des Vignettes « en attente » — ETAT l. 19, INTERFACE l. 7899 et 7930 — toujours notée en attente ; voir D.
- Point 3 de l'aplat fiable « non reproduit », voie qui pose le fond avant la copie « non élucidée » ; garde-fou de 2 s gardé —
  INTERFACE l. 7287, 7296 — rien de plus récent.
- « La clé » ne suit pas un dessin repeint à la main à moins de 90 % — INTERFACE l. 7295 — limite assumée.
- Versions : copie d'un plan déplacé non liée à l'original ; couper ou réunir un plan montré par d'autres versions de séquence « non
  traité » — INTERFACE l. 7265-7266 — non vérifiable sans lancer.
- Fusion de calques : Couleur unie non fusionnable ; le mode de fusion du dessus n'agit que sur le dessous ; calques non choisis entre
  deux choisis remontent — INTERFACE l. 6537-6540 — limites assumées.
- Dégradé bitmap retouchable seulement tant qu'on ne change pas d'outil ; aperçu de retouche par-dessus l'ancien dégradé vectoriel
  retrouvé — INTERFACE l. 6353.
- Recherche du Scénario : une occurrence ne traverse pas deux parties ; Cmd+F ne cherche que pointeur sur le panneau — INTERFACE l. 6291.
- Références : « Fenêtre transparente » jamais vérifiée à l'écran (« si le fond reste gris, dis-le ») — INTERFACE l. 6385.
- Exports montage (EDL avec son, FCPXML, marqueurs, son NTSC, ProRes 4444 transparent, DNxHR, calques séparés, JSON caméra) jamais
  ouverts dans un vrai logiciel — INTERFACE l. 6477-6485 — aucune trace d'essai depuis.
- Pont Kitsu : toutes les adresses de l'API écrites de mémoire, « À CONFIRMER » ; pas de pagination, pas de renommage/suppression,
  pas de rafraîchissement du jeton — INTERFACE l. 6948, 6984 — AUDIT-2026-11-10 : un seul essai.
- En-têtes CSV Kitsu à confirmer ; paquet de livraison sans sélection libre de plans ; PDF du paquet en mise en page d'origine —
  INTERFACE l. 6757, 6776-6778.
- Bobines EDL changées (`PL012A` → `S01P012A`) par rapport aux EDL déjà livrés — INTERFACE l. 6844 — à prévenir.
- Rapport de numérotation, colonne « Écart cumulé », page de révision : limites listées — INTERFACE l. 6848-6852.
- Journal des différences : noms au modèle d'aujourd'hui, cadence d'aujourd'hui, notes supprimées absentes, « relevé en cours »
  — INTERFACE l. 6907-6912.
- Anneau : pas de touche clavier pour l'ouvrir ; second anneau qui peut sortir de la vue près d'un bord ; au-delà de trente
  pastilles, illisible — INTERFACE l. 7711-7714.
- Stylet : inclinaison non branchée — INTERFACE l. 8089 — toujours (rien de plus récent).
- Mes habitudes : ne voit ni les gestes de dessin ni les curseurs et cases à cocher — INTERFACE l. 7812.
- Plan écarté : tolérance de 5 points au stylet à juger ; un trait qui a traversé une case écartée reste sous la case reprise ;
  textes sous une case écartée non peints ; refusé en film partagé — INTERFACE l. 8186-8192, 8158.
- Le « + » de Maj : cible de 24 points et place sur planche serrée à juger au stylet — INTERFACE l. 8250.
- Filets de case (un pixel) peints par-dessus le trait ; trous restants = filets bleus des cadres — INTERFACE l. 7769, 8336.
- Badge de numéro très aminci quand un cadre B descend près de la rangée (badge 12) : laissé à la décision d'Anthony — INTERFACE
  l. 8352 — non tranché.
- Tremblement du trait : continuité à la verticale où finit la feuille « dis-moi » — INTERFACE l. 8362.
- Trajectoire : pas de lasso ni de boîte d'étirement, l'échelle ne suit pas la courbe ; calque verrouillé, dossier, calque de couleur
  refusés — INTERFACE l. 8554-8558.
- Saccade de caméra dont le rythme ne suit pas un décalage Cmd (durée de plan) — ETAT l. 6 — hors de la tranche mais touche 117/119.
- Planche-mère : le lasso de la planche ne prend un trait partagé que dans la 1re case qui le montre ; la Fiche du plan ne dit pas
  qu'un plan est lié ; volet Vignettes (maillon sur les cases) non fait — INTERFACE l. 7117-7125 — rien de plus récent trouvé.
- Corbeille : vignette = premier dessin du calque du haut — INTERFACE l. 7596.
- Couleurs : focus clavier jade sur les boutons « pas fait » — JOURNAL l. 1298 — rien de plus récent.
- Vocabulaire à décider (89.4) toujours ouvert : « Dessin » à trois sens, « Variantes / Versions du plan / Versions de séquence »,
  « Exposition » et « durée », « calque » d'annotation — INTERFACE l. 6718-6723 (seul « Papier » a été tranché en 90).
- Rotation du canevas des Vignettes non faite (décision d'Anthony : pas de rotation) — INTERFACE l. 6198, JOURNAL l. 1324 — fermé par
  décision, à ne pas signaler comme bug.

### Parties 1 à 75 (lot 1)

- « Montrer dans l'explorateur » et « Afficher dans l'Explorateur » lancent `explorer.exe` : rien ne s'ouvrira sur Mac — src/exportui.rs:1082 (appelé par editorialui, journalui, paquetui, relectureui, aproposui) — code lu : aucune branche macOS (0 `cfg(target_os = "macos")` dans src/).
- Onglet suivant = Ctrl+Tab ; sur Mac le code lit Cmd (keymap.rs:892), donc Cmd+Tab, que macOS garde pour changer d'application — keymap.toml:78, partie 23 — code lu ; effet réel non vérifiable sans lancer.
- Mode concentration sur F11, touche système de macOS (Afficher le bureau) d'origine — keymap.toml:273, partie 64b — non vérifiable sans lancer.
- Fermer un onglet au clic du milieu : pas de bouton du milieu sur un trackpad — partie 23 l. 2270 — limite de matériel.
- Préférences sans raccourci (un utilisateur Mac attend Cmd+,) — partie 30.5 l. 3181 ; keymap.toml:2331 `touches = []` — vérifié.
- Exports pour le montage jamais ouverts dans un logiciel de montage — JOURNAL l. 1344, AUDIT-2026-10-30 H3, partie 17 l. 1784 — toujours dans la liste « À vérifier » de la partie 84 (l. 6482) : ouvert.
- Export de la planche : pour une case à mouvement de caméra, seul le cadre A sort — JOURNAL l. 1333 — la « planche en tuiles exactes » (JOURNAL l. 1214, 760cba4) l'a peut-être réglé ; non vérifiable sans lancer.
- Annuler un export n'a jamais été cliqué dans le vrai exe ; les autres exports attendent leur fin à l'annulation (non mesuré) — ETAT, mise à jour du 2026-11-07 — non vérifiable sans lancer.
- Fenêtre fermée toute seule après un enregistrement refusé : cause non trouvée — partie 67 l. 5561, ETAT l. 179 — l'écriture continue (135) rend le cas plus rare ; toujours inexpliqué.
- Plantages quand la structure change (supprimer, réunir des plans) pendant qu'un écran garde un ancien numéro de plan — ETAT l. 177 — aucun journal de plantage depuis le 5 novembre (AUDIT-2026-11-10) ; la passe de débogage annoncée n'est pas trouvée ; non vérifiable sans lancer.
- Génération sonore : pas de bruitage généré, envoi en ligne non branché — ETAT l. 188 — confirmé : `TransportABrancher` dans suggestionsui.rs:130 (Labo).
- Grab / Pull : réglages perdus d'un lancement à l'autre, pas d'Échap pour abandonner un geste, décalage plus grand que le rayon qui replie le trait — partie 43 l. 4037 — confirmé : aucune touche Échap dans sculptapp.rs.
- Effets : un seul exemplaire par calque pour les effets du trait et de matière ; seul le premier effet choisi montre ses clés ; ni Cmd+X ni glisser-déposer d'un effet ; Cmd+V depuis la timeline ne colle pas d'effet — partie 31 l. 3242 — non vérifiable sans lancer.
- Clés : pas de rectangle pour les clés d'effets ; Maj + clic des clés d'effets reste sur un calque ; Cmd+D et Option + glisser seulement pour les dessins — partie 4.5 l. 317 (60 a ajouté Option + glisser des dessins au montage) — reste ouvert.
- Aplat de personnage : un personnage dessiné dans l'aplat d'un autre, sur le même calque, garde l'ancienne couleur ; déplacer un dessin ne déplace pas sa peinture d'aplat ; formes et texte sans aplat — partie 32 l. 3362, partie 33 l. 3474 — non vérifiable sans lancer.
- Aplat intelligent : zones de moins de 6 px ignorées, pose qui change ou rotation de plus de 10° mal suivies, retouches sans trait (déplacer, coller) qui ne suivent pas seules — partie 63 l. 5317 — limite connue.
- Versions de séquence : retoucher une version de plan la change dans toutes les versions de séquence qui la montrent ; durée propre appliquée sans couper d'images ; comparaison plan par plan seulement ; anciennes variantes (format 38) sans versions partagées — partie 59 l. 5052 — limite connue.
- Fiche technique : LibreOffice et Google Sheets jamais essayés ; cadrage du début du plan seulement ; vignette = première image, sans choix — partie 26.6 l. 2825 — ouvert.
- GIF : des plans choisis qui ne se suivent pas sont mis bout à bout ; contenu rogné attaché au plan, pas à la version — partie 25 l. 2461 — limite connue.
- Recherche des Préférences : ne surligne pas le mot trouvé — partie 30.5 l. 3176 — limite connue.
- Pipette sur l'affichage de secours OpenGL : pas de loupe, rien hors du dessin — partie 65 l. 5466 — à surveiller si FEANOR tourne sur OpenGL sur ce Mac ; non vérifiable sans lancer.
- Collage d'un plan lourd : raccourcis figés et onglet bloqué pendant « Collage en cours… » — partie 23 l. 2301 — voulu, mais gênant sur un gros plan.
- Scénario remplacé : les plans créés depuis l'ancien ne sont plus reliés au texte — partie 75 l. 6068 — voulu.
- Coopération : outil individuel en l'état, jamais essayé à deux postes ; « à retravailler après la V1 » — JOURNAL l. 1360, AUDIT-2026-10-30 H1 — ouvert.
- Mon panneau : resté au Labo, sans raccourci, décision « le sortir du Labo ? » en attente — partie 64 l. 5406, ETAT l. 124 — ouvert.
- Texte sur un dessin de décor ocre : contraste 1,9:1 dans les thèmes sombres — partie 2 l. 162 — jamais regardé.
- ffmpeg : la vidéo (MP4, GIF, ProRes) dépend d'un ffmpeg posé à côté de l'exe ; seul le poste Windows est décrit (JOURNAL l. 1283, 1288) — sur Mac, non vérifiable sans lancer.
- Feuille plafonnée à 8192 px (toile infinie mise à part) — ETAT l. 190 — décision.

### Autres sources (docs, suivi, audits) (lot 7)

Fiabilité, données
- Fermeture « toute seule » après un enregistrement refusé : cause jamais trouvée — ETAT l. 178, V1 l. 84 — INTERFACE 67 (l. 5563) dit seulement que le code ne ferme pas seul ; plus d'enregistrement manuel depuis 135, non revu.
- Plantages par numéro de plan périmé (changer la structure pendant qu'un écran garde un ancien numéro) : « semble traité », passe de stabilisation « après la V1 » — ETAT l. 176, V1 l. 83 — non vérifiable sans lancer.
- Hors tas 2,6 à 3,0 Go après une longue séance (pilote Vulkan/carte), « Non résolu » — ENDURANCE §7.4 l. 327, V1 l. 84 — inchangé.
- Après le passage à une version de séquence, la copie d'un plan déplacé n'est plus liée à l'original ; couper/réunir un plan montré par d'autres versions de séquence le coupe pour elles aussi — INTERFACE 102 « Limites connues » (cité par V1) — ouvert.
- Aplat « La clé » : un trait sans pastille retombe sur la couleur du calque si moins de 90 % de la forme est de la même couleur ; le fond d'un trait n'attend que 2 s avant une commande qui copie — INTERFACE 103 « Limites restantes » — ouvert.
- Annuler un export autre que la planche (PDF, relecture, color script…) attend toujours la fin de son `finish` — FIABILITE §20 l. 485, ETAT l. 6 — ouvert, non mesuré ; « Annuler n'a pas été cliqué dans le vrai exe » (ETAT l. 6).
- Essais non expliqués (à garder si ça revient) : `cases_tests::uniting_two_plans_…` (1 échec sous charge), `scrubbing_far_away_never_cuts_the_stroke_again` (écart de 44 px vu une fois) — ETAT l. 7 — non revus.
- Morceaux pâles des cases 15 et 16 : « Pas reproduit » — ETAT l. 18, INTERFACE l. 7766 — à confirmer à l'écran.
- Une image de case déborde dans les Vignettes (B7, « à confirmer ») — AUDIT-2026-10-30 l. 102 — aucune trace de correction ; non vérifiable sans lancer.
- Image de 1,9 s après un changement de durée, vue une fois — ENDURANCE §4 point 8 — non reproduit.
- Envoi (marquer) : 70 à 160 ms d'image figée à la fin sur gros projet — ENDURANCE §4.3 et §6.6 — ouvert.

Pont Vignettes / Storyboard
- « La planche et les plans tirent chacun de leur côté » : huit réparations des traits au bord des cases ; si l'essai de la règle A montre encore des surprises, changer le modèle — AUDIT-2026-11-10 constat 4 — parties 125/127/128/131/133 ensuite ; jamais confirmé par Anthony.
- Stylet à états cachés : trois interrupteurs (« Aplat sous le trait », « Peint sur l'aplat », « Peindre derrière ») toujours là (grep : 10, 13 et 6 fichiers de src/) — AUDIT-2026-11-10 constat 3 et proposition 22 (non faite).
- Vidéo à 25 i/s dans un projet à 24 i/s : `VideoClip::index_at` (src/video.rs:45-49) saute une image vidéo sur 25, les numéros ne sont pas ceux des animateurs ; aucun choix de cadence à l'import — AUDIT-2026-11-10 l. 44 — code inchangé depuis 88986ea (2026-10-19).
- F9 (notes de friction) : dossier `Documents\SHOT\Frictions` trouvé vide après trois F9 ; non vérifié. Lecture du code : la note ne s'écrit qu'à Entrée, Échap l'abandonne (src/frictionui.rs:1-11) — explication possible, non prouvée — AUDIT-2026-11-10 l. 47.

Montage et exports (V1.md « Écarts », toujours absents du code)
- Entrée/sortie pour monter, lift, extract, vrai écrasement : seule une API « pour les gestes à venir » existe (INTERFACE l. 7586) ; aucune commande `montage.*` correspondante dans keymap.toml.
- Lier/délier image et son, montages J et L ; vitesse d'un plan ou d'une vidéo, arrêt sur image ; slide ; marqueurs nommés ; titres dans le montage — V1 l. 95-97 — aucune commande trouvée.
- Paquet de livraison limité au film ou à la séquence courante, pas à une sélection de plans — V1 l. 106, INTERFACE l. 6776.
- Fichiers de montage, ProRes avec transparence, DNxHR, calques séparés jamais ouverts dans un logiciel tiers ; Kitsu jamais essayé sur un vrai serveur — V1 l. 107-108, ROADMAP l. 2975, AUDIT H3 — inchangé.
- EDL : pas de son dans l'EDL, pas de time code à sauts d'images (29,97 DF), pas d'AAF ; XML/OTIO écrivent le fondu au noir en coupe — ROADMAP l. 2989, EXPORT_MONTAGE l. 99, 231.
- Relecture HTML : une note modifiée et renvoyée n'est pas mise à jour ; notes sans plan perdues si on ferme sans les poser — ROADMAP l. 3080 — non vérifié dans le code.
- MP4 très lourd (2,7 Go pour 15,5 min, débit de l'encodeur Windows) — ENDURANCE §4.5 — ouvert.
- Génération sonore : aucun bruitage généré, envoi HTTPS « à brancher » — ETAT l. 186, ROADMAP l. 2704 — ouvert (V1 : « si possible »).

Dessin
- Inclinaison du stylet jamais branchée au pinceau (src/brush.rs:425 « viendra quand Pelure saura la lire » ; stylet.rs n'en fait qu'un relevé) — AUDIT-RENDU-BROSSES étape 7.
- Mode de fusion par trait (feutres qui se multiplient), peinture à poils / séchage / aquarelle : non faits (`StrokeMode` = Paint/Erase, brush.rs:1095) — AUDIT-RENDU-BROSSES étapes 4, 6, 8 ; la peinture humide ne se mêle pas d'un trait vectoriel à l'autre (6 ter).
- Outil Déformer, étirement par le milieu, « Effacer hors de la sélection » sur le calque courant seul ; zones bitmap oubliées en changeant d'outil — ROADMAP l. 2556 — en partie repris par la partie 132 (multi-calques), non vérifié ligne à ligne.
- Grab à 400 px : 16 ms par image ; première image où une case devient le plan courant : 25 à 150 ms — PERFORMANCE l. 1772, 1938.

Espace 3D (plans ordinaires et « Plan en Espace 3D »)
- Carte inclinée coupée aux bords de la feuille ; le dessin hors feuille n'est pas filmé quand la caméra a bougé — ESPACE3D §11.2, ESPACE-LIBRE A3.
- La vue Espace 3D montre le dessin brut (sans effets, modes de fusion, opacités) — ESPACE3D §11.3, ESPACE-LIBRE A7 ; encore écrit en INTERFACE l. 10763.
- Outils 2D (sélection, Transformer, multisélection, gomme de débord) sur carte inclinée : boîtes posées comme sous la caméra de référence — ESPACE3D §11.4, ESPACE-LIBRE A3.
- Poignées de rotation qui peuvent sauter près de ±90° (angles d'Euler) ; bascule caméra bornée à ±89° — ESPACE3D §11.5, ESPACE-LIBRE A6/A7.
- Deux cartes qui se traversent : l'une passe entière devant l'autre ; un dossier passe en bloc — ESPACE-LIBRE A4.
- On peint sur le calque courant même caché derrière une autre carte, sans que l'écran le dise — ESPACE-LIBRE A5.
- Un dossier n'est pas une carte (sa pose ne place pas ses calques) — ESPACE3D §11.6.
- Côte à côte : gauche/droite seulement, non retenu d'une séance à l'autre — ESPACE3D §13 « Limites ».
- Budget de temps des 20 cartes mesuré sur Mac seulement (1,14 ms), « à confirmer sur le poste Windows » — ESPACE-LIBRE A0 point 5.
- Commandes de surface (face, sol, mur) sans touche ni entrée dans le menu des calques — ESPACE-LIBRE A6 « Pas fait ».

### Spécifique Mac (lot 7)

Compilation et lancement
- `rust-toolchain.toml` impose `stable-x86_64-pc-windows-msvc` et `.cargo/config.toml` `target-cpu=x86-64-v3` : `cargo` échoue tel quel sur Apple Silicon. Contournement des sessions : `cargo +stable-aarch64-apple-darwin` (ou `RUSTUP_TOOLCHAIN=stable`) et `RUSTFLAGS="-Awarnings"` (ESPACE-LIBRE A0 point 6, l. 173-181 ; JOURNAL l. 586). La version optimisée se construit (JOURNAL l. 74, 211 : exécutable Mac de ~52 Mo).
- `build.rs` n'ajoute l'icône et le manifeste (tas segmenté) que pour Windows MSVC : sur Mac, pas d'icône, pas de paquet `.app`, pas de signature (message « développeur non identifié », NOTES l. 30).
- Les essais ne se lient sur Mac qu'avec des remplaçants vides de kernel32/psapi hors du dépôt ; la suite y a « 86 à 90 échecs propres au Mac » (ESPACE-LIBRE l. 211-213, 484 ; JOURNAL l. 63-265), jamais listés ni expliqués nulle part. La référence reste `verifier.ps1` sur Windows.
- JOURNAL l. 615 : « FEANOR ne se lance pas sur ce Mac comme sur le PC » ; aucune session n'a regardé l'application sur Mac. **Personne n'a encore vu FEANOR tourner sur Mac.**
- Affichage : wgpu avec `features = ["dx12"]` pour Windows ; sur Mac, Metal par egui-wgpu, OpenGL en secours (src/lib.rs:1515-1520). Non vérifiable sans lancer ; tous les budgets « sans latence » ont été mesurés sous Vulkan (PC) ou en essais (Mac).

Dégradations codées (`cfg(windows)` hors essais)
- Stylet : l'écoute de Windows n'existe pas (src/stylet.rs:346-354 `fenetre_de` rend 0 ; stylet.rs:543-551 `ecouter` rend faux). Perdus : bouton du stylet = clic droit, stylet retourné = Gomme (styletlien.rs:1-16), donc l'anneau sous le stylet au bouton et la fenêtre des réglages au bouton ; le banc Aide > « Ce que le stylet transmet » ne lit rien.
- Pression : elle ne vient que des `Event::Touch { force }` d'egui (src/canvas.rs:469-476, 540, 595, 605) ; sans eux, appui = 1,0 (pression pleine). winit 0.30 ne transmet pas, à ma connaissance, la pression d'une tablette sous macOS : **probablement aucune pression sur Mac** (à confirmer dès le premier trait : un appui léger doit faire un trait fin).
- Inclinaison : jamais branchée, même sous Windows (brush.rs:425).
- Fichiers de données : `donnees::roaming()` / `local()` lisent les variables `APPDATA` et `LOCALAPPDATA` (src/donnees.rs:22-29, 46), absentes sur Mac. Conséquences : réglages jamais enregistrés (settings.rs:724 « dossier %APPDATA% introuvable », recents.rs:89 « Réglages non enregistrés ») donc thème, préférences, **projets récents**, visite « Premier jour » remis à zéro à chaque lancement ; raccourcis perso (`keymap.rs:1357`) ; brosses importées (`brushtex.rs:1064`) ; modèles d'épisode (`modeleepisode.rs:211`) ; mélangeur (`melangeur.rs:29`) ; compteur « Mes habitudes » (`usageui.rs:70, 103`) ; **pas de copie de secours ni de reprise après plantage** (`recovery.rs:77-78`, `app.rs:1181-1182`, `onglets.rs:883`) ; pas de rapport de plantage (`plantage.rs:60`) ni de journal de démarrage (`demarrage.rs:85`). Contournement possible : lancer avec `APPDATA` et `LOCALAPPDATA` définis vers un dossier (non essayé).
- Récupération : même avec le dossier, le fichier de verrou n'est pas exclusif hors Windows (recovery.rs:86-90, `share_mode(0)` Windows seul).
- ffmpeg (export vidéo, GIF, import vidéo, paquet, animatique) : `find_ffmpeg` ne cherche que `ffmpeg.exe` (src/export.rs:352-384 : à côté de l'exe, PATH, Program Files, WinGet, C:\ffmpeg). Sur Mac, jamais trouvé seul : il faut `PELURE_FFMPEG=/chemin/ffmpeg` ou le chemin choisi dans les Préférences (et celui-ci n'est pas retenu, voir ci-dessus). ffmpeg livré : Windows seulement.
- Priorités : l'encodage ffmpeg n'est pas mis sous la priorité normale (export.rs:416-423) et les fils de fond non plus (parallele.rs:64-67) : l'interface peut ralentir pendant un export ou un rejeu.
- « Ouvrir le dossier » / « Montrer dans l'explorateur » lance `explorer.exe` sans condition (src/exportui.rs:1081-1101) : sans effet sur Mac (une douzaine d'appels, dont fin d'export et dossier des frictions).
- Dépôt de fichiers (images, sons, vidéos sur la planche ou le montage) : la place du lâcher vient de `GetCursorPos` sous Windows, sinon de la dernière position du pointeur connue d'egui (src/deposer.rs:421-447, soundtrack.rs:320) : risque de dépôt au mauvais endroit si le pointeur n'a pas bougé dans la fenêtre pendant le glisser. Non vérifiable sans lancer.
- Heure : `decalage_local` rend 0 hors Windows (src/remplacer.rs:69-99) : heures affichées en UTC (une heure de moins à Paris en novembre) dans les jalons (jalons.rs:82, 104 ; jalonsui.rs:118), les projets « remplacés le … », les notes de friction.
- Planche de références sur un second écran : `ecrans()` rend une liste vide (src/planchref.rs:568-603) : pas de choix d'écran, pas de plein écran sur l'écran 2.
- Bibliothèque : un seul « lecteur », `/` (src/biblio.rs:648-663) ; les fichiers cachés ne sont filtrés que par le point initial (biblio.rs:292-304).
- Mémoire : `materiel::memoire()` et les cœurs physiques rendent None (src/materiel.rs:462-470) ; la mémoire vidéo n'est pas suivie (memoirevideo.rs:342-350, pas de DXGI). Le diagnostic des performances perd ces mesures ; le budget du décor 3D prend 512 Mo de textures par défaut (decor3dbudget.rs:199-203).
- Polices : Segoe UI absente, l'interface prend la police d'egui (theme.rs:969-975) ; l'outil Texte ne trouve aucune police du système (text.rs:60-68, WINDIR seulement) et retombe sur « Ubuntu Light » ; le PDF n'embarque aucune police (pdffont.rs:185-188) et passe aux polices standard (caractères hors Latin-1 à vérifier).
- Kitsu : `curl` du système est pris (kitsu.rs:180-188), mais le jeton ne peut pas être gardé (« seulement sous Windows (DPAPI) », kitsu.rs:884-887) : reconnexion à chaque fois, sans doute.
- Clés des services en ligne (génération sonore) : refus « le Gestionnaire d'identifiants n'existe que sous Windows » (src/clessecretes.rs:285-293).
- Serveur local de génération sonore : cherche `venv\Scripts\python.exe` (serveurlocal.rs:65-67) : introuvable sur Mac.
- Diagnostic : `tas_segmente` rend None (lacher.rs:75-79), sans effet visible.
- Presse-papiers : `arboard` sans garde Windows (src/pasteimage.rs:61-72) ; le commentaire de Cargo.toml (« presse-papiers de Windows ») est trompeur ; coller une image depuis Aperçu/Safari devrait marcher, jamais essayé.

Clavier et souris sur Mac
- Les raccourcis suivent Cmd (`modifiers.command`, keymap.rs:892, 1551) mais l'interface écrit « Ctrl » partout.
- **Ctrl seul tenu = lasso** (dessin) lit la touche Control physique, pas Cmd (src/springtools.rs:94) ; dans les Vignettes Cmd prend le lasso à l'appui (boardui.rs:969) mais l'outil revient dès que la souris est relâchée (le ressort ne voit pas Cmd). Ajouter à la sélection sur le fond de la planche : Control ou Maj, pas Cmd (boardui.rs:1628, 1983) ; poser une note : Control+Entrée, pas Cmd+Entrée (boardui.rs:2083, la bulle dit « Ctrl + Entrée »).
- Touches prises par macOS : Cmd+Espace (Spotlight) = zoom temporaire avant (keymap.toml:233) ; Cmd+Tab = onglet suivant (l. 78) ; Cmd+Q / Cmd+W quittent ou ferment (l. 63, 67), Cmd+Q peut passer par le menu de winit sans la fermeture de FEANOR (risque de perdre la dernière seconde d'écriture continue, non vérifié). F1, F4, F5, F6, F9, F11 demandent fn sur un clavier Mac (F9 = notes de friction, F5 = couper le plan / clé de dessin).
- « Suppr » : Retour arrière fait aussi Supprimer (keymap.toml:157-158) : utilisable sur un clavier Mac.


## Partie 3 — Doublons et incohérences relevés, pas encore traités (91 points)

### Parties 189 à 201 (lot 6)

- JOURNAL l. 14, 21, 31, 37, 52, 63, 74, 89 : « pas de commit » pour 193 à 201, alors que les commits 0335c3c, 62620e6, 25c1084, d9c6788, 3446588, e016b52 et 23cb4e4 existent sur main.
- INTERFACE 192.11 (l. 16963) : « Il n'existe ni dupliquer… » de séquence, alors que 191 l'a ajouté (JOURNAL l. 96 : ajouté au ⋯ de la ligne Séquence). Ce ⋯ a lui-même disparu en 196.
- INTERFACE 191.1 (l. 16595) place « Dupliquer la séquence » dans « le menu ⋯ de l'en-tête de la timeline » : obsolète depuis 196 (clic droit de la ligne Séquence, ⋯ de la ligne Plans). La checklist 191.5 point 2 aussi.
- INTERFACE 192.4, 192.9 et 192.12 : checklists en partie caduques (13 pt devenu 16 ; ligne Séquence avec icônes ; calque actif seul déplié ; indicateurs dossier, +/− et punaise) et non barrées. Seule 192.15 reste à jour, et encore : sa ligne Séquence, d'origine à 20, est à 10 depuis 196.
- INTERFACE 192.13 (tableau, l. 17011) : Séquence « 18 à 32, d'origine 20 », remplacé par 196.1 point 3 (10 à 32, d'origine 10, jusqu'à 8), sans renvoi.
- INTERFACE 197.1 (l. 17673) : « proposé par « + Panneau » et la barre des panneaux », alors que la barre a été retirée en 194 (src/barrepanneaux.rs l. 1-7 : « la barre n'est plus dessinée »).
- INTERFACE 190.1 point 5 et 190.5 : « Envoyer vers la séquence… » (Propriétés > Décor, decor3dui.rs l. 985) et 201 « Déplacer vers la séquence ▸ » (seqdecor3d.rs l. 157) : deux noms pour la même action.
- ESPACE3D §22 (l. 523-525) : œil et cadenas « pour la séance, pas dans le fichier » ; ESPACE3D §24 (l. 541) : lampes qui n'éclairent « pas les cartes ». Les deux sont contredits par §23 et §25, sans renvoi sur place. Même chose dans INTERFACE 200.1 (l. 18075) et 200.6 (l. 18207) face à 201.4.
- INTERFACE 194.5, 195.4 et 196.4 parlent de « %APPDATA%\…\reglages.toml » (Windows). Sur Mac, il faut le chemin des réglages de l'application, que la doc ne donne pas.
- INTERFACE 192 : en-tête « (2026-11-19) » alors que la fusion date du 20 (9b63079), et le texte parle de « partie 179 » (192.5 : « après 179 », 192.17 : « pendant la partie 179 ») : les numéros changent selon la branche.

### Parties 170 à 188 (lot 5)

- 170.1 et 170.4 point 1 : « icône pellicule dans la barre des panneaux » ; cette barre est retirée par la 194, et la grille de
  vignettes par la 195. Les checklists 170.4, 171.4, 172.4, 176.4 et 177.4 parlent encore de « vignettes ».
- 173.5 (la barre sur deux lignes, bornes 250-520, trois rangées) et 173.6 (espacement 19-30) sont contredits par la 192 (une ligne,
  minimum 286, deux rangées puis tous épais, « collé à droite ») sans renvoi dans la 173.
- 174 : l'en-tête des colonnes et ses sept icônes (174.1 point 1, checklist points 1 et 3) n'existe plus (192.6). Aucune note dans
  la 174.
- Maj+Option+C a trois sens selon la partie : tout replier (174), la rangée Calques du Storyboard (178.1 point 4), le réglage
  « Replier les calques » de l'Animation (192.13). Ce qu'il fait aujourd'hui dans le Storyboard n'est pas dit.
- 178.1 : le tableau des hauteurs est périmé (Séquence 16, puis 20 à la 192.11, puis 10 à la 196) ; les icônes d'en-tête de la ligne
  Caméra sont retirées par la 196.
- 180.1 point 1 et 180.6 point 1 : libellé « Décor 3D de la séquence (glTF, .glb)… » ; le code et la 181 disent « Objet 3D pour le
  décor de la séquence (glTF, .glb)… ».
- 180.5 : « Copier ou dupliquer une séquence n'emporte pas son décor », avec un renvoi à la 190 seulement ; la 191 a depuis ajouté
  « Dupliquer la séquence » (JOURNAL l. 124 dit encore « FEANOR n'a pas de commande qui duplique une séquence »).
- 181.1 point 3 et 180.1 point 5 : les champs du décor sont « dans le panneau Espace 3D » ; dans l'onglet 3D ils sont dans
  Propriétés > Objet / Décor (198), sans renvoi.
- 187 (tableau et « Panneau Réglages ») : l'onglet « Espace 3D à côté de Réglages » est devenu un onglet de Propriétés (194).
- ESPACE3D 15 : « profondeur dans le panneau Calques », alors que ce panneau est retiré depuis la 160.
- Numérotation dans le JOURNAL : la plage de lecture est appelée « partie 178 » (l. 261-266, 188 aujourd'hui) ; la timeline plus
  claire « partie 168 » (l. 307, 179 aujourd'hui) ; la timeline resserrée s'appelle 179 dans ses commits (192 dans INTERFACE).
- 188.3 : « à l'écran sur le Mac : la commande dans la palette, le crochet et le voile sur la règle ». On ne sait pas qui a regardé
  (sans doute la session) : à ne pas compter comme vu par un humain.
- JOURNAL l. 239-240 : un fichier vide `storyboard.rs` créé par erreur dans un dossier d'Anthony ; sa suppression lui a été laissée
  et aucune suite n'apparaît (rien à tester).
- Entrées du JOURNAL de ma plage sur des parties d'autres tranches (166.9/166.10 vue réduite l. 363-403, 168 versions de plan
  l. 428-440, 169 Cutter l. 413-427) : laissées à la tranche des parties 166 à 169. À noter : la 166.9 a été vue par Anthony dans
  l'essai (« la 2 ») ; la 166.10 (vignettes) et la 169 (« à juger : écart d'origine, 12 px, pointe effilée ») ne l'ont pas été.

### Parties 150 à 169 (lot 4)

- Les points 22 à 31 de la liste « À vérifier » de la partie 152 sont rangés à la fin de la partie 153 (l. 12157-12196), après la
  liste 1-7 de 153 ; le journal les cite comme « partie 152, points 22 à 25 / 26 à 31 » (l. 662, 681).
- 152.3 se contredit : « Sa hauteur… prend la hauteur de son contenu » (l. 11763) et « Ce qui n'est pas fait… le bord ne s'ajuste
  pas tout seul… garde la hauteur que Réglages avait » (l. 11987).
- JOURNAL : aucune entrée datée 2026-11-14 ni 2026-11-15 (parties 156 à 165 non journalisées) ; l'entrée du 11-13 ne cite ni 153,
  ni 154, ni 155.
- JOURNAL l. 307 et l. 572 : « la timeline plus claire devient la partie 168 » ; dans INTERFACE la 168 est les versions de plan et
  la timeline plus claire est la 179. Journal l. 605 et 618 : « partie 163 » (vue à côté) et « partie 162 » (raccourcis) sont
  devenues 187 et 186.
- Commit 1d30449 « Fusion de logo-joyau … (partie 131) » alors qu'INTERFACE en fait la 167 ; commits du Cutter et des versions
  numérotés 168 et 167 (renumérotation notée dans les titres seulement) ; message de ab1c03f inexact (noté 168.7).
- 165.4 (ligne flottante), 165.5 (barre du Storyboard sur une ligne) et 160.1 points 3 à 7 n'ont pas de mention « modifié par »
  alors que 179.4, 174 / 192 et 194 les remplacent.
- 157.1 (Environnement dans le panneau Espace 3D) et 157.2 (« Hors d'un plan en Espace 3D : refusé ») sans renvoi à 162 et 158/159 qui
  les ont changés ; 150 (icône cube de la barre des panneaux) et 159.1 (« pas d'icône Espace 3D dans la barre des panneaux ») sans
  renvoi à 194 qui retire cette barre.
- 168.5 (« dix teintes des couleurs de plan », « déplie l'ancienne à sa place ») contredit 168.2/168.3/168.7 du même jour révisés le 18,
  sans annotation dans 168.5.
- docs/ESPACE-LIBRE.md (dernier commit f2860cd, 2026-11-12) décrit encore « Plan en Espace 3D » comme une case à cocher (l. 32, 87) et
  la vue de dessus / les mouvements des Vignettes désactivés pour ces plans (l. 98), tous deux changés par la partie 159.
- 166.2 parle de « l'échelle de Windows » et 166.7 / 168.6 / 169.3 de dossiers `Documents\SHOT\Tests` : propres au PC d'Anthony,
  à transposer sur Mac (échelle Retina).

### Parties 120 à 149 (lot 3)

- Partie 126 (planche-mère) est rangée à la l. 7070, hors de sa place : la suite saute de 125 à 127 (INTERFACE l. 8964-9051).
- Partie 130 placée avant 129 (l. 9265 et 9305).
- Ligne orpheline à la fin de 127 : « 9. Un calque bitmap : la sélection reste celle de la case, comme avant. » (l. 9198), recopiée de 125 et contredite par 127 lui-même.
- 125, 127, 128 décrivent la répartition en « parts » et la case née par le bout d'un trait sans renvoi vers 131 qui les remplace (seule 125 point 9 renvoie à 127) ; 127 relecture (l. 9155) et 128 « Suites » (l. 9246) parlent d'une suite à faire devenue sans objet.
- 132, 136, 137.11, 139.1, 141.2 donnent le panneau Calques comme lieu des gestes, sans renvoi à 160 qui l'a retiré.
- 143 (points 1, 13, 14, messages l. 10518-10521), 148 (points 9, 11, « Décocher ») et 149 (point 14, message « plan ordinaire ») sans renvoi à 159 (plus de « Plan en Espace 3D »). Seul le vocabulaire « Réglages → panneau Espace 3D » est réglé par la note de 150 (l. 11345).
- 146 : la section Limites dit encore « Ctrl tenu n'arme plus le lasso dans la vue » et « la sélection n'y est pas », contredit par la note « Modifié par la partie 152 » en tête de la même partie.
- 146.1 et 148 parlent de « plan ordinaire » comme d'une sorte de plan encore active ; caduc depuis 159.
- 144 parle du bouton « Vues » ; 160 et 198 l'ont remplacé par « 1/2/4 vues » : pas de renvoi.
- 129.2 (un seul effet vivant par plan, liste de l'outil Caméra) sans renvoi vers 170 (plusieurs blocs).
- 140 sans renvoi vers 192 (calque actif replié d'origine, l. 16925-16929), alors que 137.1/137.5 ont leur renvoi vers 165.7.
- 123 l. 8872 dit « Non traité : insérer des images ne décale pas la trajectoire » et ETAT.md l. 6 le répète, alors que finitions (partie 119, 7c7df04) l'a fait.
- 138 est daté du 2026-11-10 mais vient après 137 (11/11) ; son en-tête explique la renumérotation, pas l'écart de date.
- 143 dit « Rien de cette partie n'a été regardé à l'écran » (l. 10488) alors que le journal (l. 797-807) note la liste faite par Claude et le point 9 vu par Anthony.

### Parties 76 à 119 (lot 2)

- Deux parties numérotées 111 (anneau l. 7623 ; traits continus l. 7735), assumé l. 7899 mais non renuméroté.
- Ordre des parties désordonné : 82 après 84 (l. 6488), 87 après 89 (l. 6739), 104 après 110 (l. 7561), 105 après 108 (l. 7433),
  106 après 103 (l. 7308), 126 inséré entre 97 et 98 (l. 7070).
- La partie 126 coupe la partie 97 en deux : le tableau « Séance de 2 000 gestes », le paragraphe Vulkan, `SHOT_SANS_SEANCE` et la
  liste « À vérifier » de 97 sont sous le titre 126 (l. 7140-7160).
- Partie 77, checklist : deux points numérotés « 8 » (l. 6275-6276), pas de 7 ; partie 76 : sections « 10 et 11 » sans 9 (l. 6190).
- Partie 96 décrit le Joyau et `assets/feanor-viseur-*.svg`, remplacés par la partie 167 sans renvoi depuis 96.
- Parties 81, 83 et 98 renvoient à « la barre de droite » et au « panneau Calques », retirés en 194 et 160, sans note.
- Partie 84 décrit « Fichier > Exporter… : quatre titres, quinze entrées » ; la partie 130 l'a remplacé par quatre sous-menus, sans
  renvoi dans 84 ; la partie 105 cite « 23 entrées ».
- Partie 85 (marquer l'envoi : « le message dit d'enregistrer (Ctrl+S) ») et 99 (pastille jade d'un projet « pas enregistré »)
  supposent l'enregistrement manuel, retiré par la partie 135 (Ctrl+S pose un jalon) : à revérifier.
- Partie 113 point 2 « Retour aux Vignettes » et partie 100 « trois onglets » : l'onglet 3D (198) en fait quatre ; 100 non mis à jour.
- Décision 6 : notée « en attente » (ETAT l. 19, INTERFACE l. 7899, 7930, ANALYSE-VIGNETTES l. 123, 217) alors que la partie 116
  (« + » de Maj) a donné « Ajouter un cadre » sans le clic droit en gardant « glisser dans B déplace B » : statut à clore ou à reposer.
- suivi/NOTES.md : toutes les notes du 30 octobre restent « proposé, en attente de ton test » ; la note « Enlever le logo SHOT à côté
  de Fichier — à fusionner » alors que 1c5a102 l'a fusionné (JOURNAL l. 1351).
- ETAT.md l. 6 garde « insérer des images ne décale pas la trajectoire » corrigé le lendemain (C).
- Partie 89.1 et `premierjour.rs` / `app.rs` gardent « Régler SHOT pour cette machine » dans des commentaires ; l'interface dit
  FEANOR (seulement des commentaires, sans effet visible).
- Partie 107 dit `project::VERSION` « reste 43 » alors que le format était déjà 46 à cette date (l. 7386).
- Partie 95 « Gestionnaire des tâches », partie 97 idem, partie 81 « listé par Windows », partie 83 « lecteurs de Windows » : checklists
  Windows à transposer pour un test sur Mac.

### Parties 1 à 75 (lot 1)

- Deux parties « 15 » (l. 1542 envois ; l. 3634 les quatre panneaux) et deux parties « 17 » (l. 1761 export montage ; l. 3750 accueil, concentration).
- Numéros 34, 35, 37, 38, 39 absents ; ordre inversé 23/22, 25/24, 28/27 ; 71 (2026-10-30) placée avant 72 (2026-10-29).
- Partie 23, liste l. 2283 : la numérotation repart à 11 et 12 après le point 17 (l. 2304-2305).
- Partie 17 bis : le titre dit « F11 ; Z jusqu'au 2026-10-28 » mais la liste l. 3815 dit encore « Z » ; son point 1 (« trois grandes entrées ») contredit 57 et 74.
- Partie 25, point 7 (l. 2503) : passe-partout à 27 % d'origine, contredit par 47 (noir opaque à 100 %).
- Partie 57 et code : le dossier des images d'accueil reste `Documents\SHOT\Identité\Accueil` (accueilimages.rs:65) alors que le logiciel s'appelle FEANOR.
- Les listes « À vérifier » des parties 18, 25, 33, 46, 62, 68, 69, 72 envoient encore au panneau Calques, retiré en 160 ; celles des parties 14, 32, 40, 41, 45 à la barre des panneaux, retirée en 194 ; plusieurs parlent de « trois thèmes » alors que Gris sombre est le thème de base (99).
- ETAT.md §2 et §4 (l. 85-190) citent encore l'import Storyboard Pro, l'import de pinceaux Toon Boom, « Windows seulement » et « 3D retirée », alors que la 3D est revenue (143 à 201) et que l'audit se fait sur Mac ; ETAT §5 point 7 (fichier Storyboard Pro) est périmé.
- JOURNAL.md : l'entrée du 2026-10-29 (l. 1413) est sous celle du 2026-11-01, hors de l'ordre « plus récent en haut ».
- Décisions laissées à Anthony et jamais tranchées dans la doc : rectangle pour les clés d'effets (4.5 l. 319) ; ambre de la pastille et sens de « Restaurer » (15, l. 1719) ; place du dessin du relecteur (22, l. 2392) ; qui garde le numéro en fusion glissée (52, l. 4604) ; dégradé radial elliptique et place de l'outil (53, l. 4664) ; mélange par distance, aquarelle, palette vidée à chaque lancement (58, l. 4983) ; dossiers et groupes façon Photoshop (12, l. 1236) ; sortir Mon panneau du Labo (64).

### Autres sources (docs, suivi, audits) (lot 7)

- DOUBLONS §5, question 1 (menus du haut qui répètent les icônes) : jamais tranchée ; la partie 130 a raccourci les menus sans répondre au principe.
- Question 2 (Exporter le film, icône du haut et Fichier > Exporter) : aucune réponse trouvée ; depuis 130, l'export est en quatre sous-menus.
- Question 3 (Annuler/Copier/Coller au clic droit du dessin) : tranchée de fait par la partie 66 : le clic droit ouvre les réglages de l'outil, sauf Sélection, Sélection par couleur et Pipette qui gardent ce menu (INTERFACE l. 5506).
- Question 4 (bouton « Animer ce plan ») : caduque, plus d'onglet Animation (partie 100) : double-clic sur un plan, Tab, bouton « Retour au storyboard / aux Vignettes » (113).
- Question 5 (Personnages d'office en Storyboard, retiré en Animation) : caduque, la barre des panneaux est remplacée par la colonne en pile (194) ; `panneaux_d_office` ne sert plus qu'à l'ordre d'office de la pile (src/colonne.rs:220).
- Question 6 (clés de dessin en « débuts seulement ») : pas tranchée ; seule l'alternance de teinte des clés (123) a été faite.
- Question 7 (une seule icône « Repères » pour commentaire, marque, signet) : pas trouvée dans le code ni dans INTERFACE ; non tranchée.
- Question 8 (barre d'état) : pas de regret noté ; état inchangé depuis la partie 40.
- DOUBLONS §1-2 est périmé : il cite l'onglet Animation (retiré, 100), le panneau Calques (retiré, 160), la barre des panneaux (retirée, 194) et la « colonne ronde » des pastilles, passée dans la barre du haut dès la partie 50 (2026-10-24).
- ETAT.md garde des sections périmées en tête de fichier : §2 « Échange avec Storyboard Pro », §6 raccourcis SBP (retirés 17dda2a), « Plateforme : Windows 64 bits seulement » alors que les sessions compilent et essaient sur Mac depuis le 2026-11-11 ; « Version Mac : reportée » (l. 14) contredit V1.md (« Bêta et lancement sur Windows et Mac »).
- ETAT l. 19 : « Décision 6 (glisser dans B déplace B) en attente » ; la partie 116 a gardé ce geste et ajouté le « + » ; le statut n'a jamais été remis à jour.
- ETAT l. 8 : « Le montage restant (entrée/sortie, liens, vitesse, titres) attend le test d'Anthony » peut se lire comme fait ; rien de tout cela n'est dans le code (voir B).
- ENDURANCE §4 (6,8 Go, Ctrl+S, plans supprimés) contredit ses propres §6-7 sans renvoi.
- ESPACE3D.md : titre « les calques et la caméra, rien d'autre » et §1 (objets 3D supprimés) contredits par §14-25 (décor 3D, lampes, objets animés) ; le §17 le reconnaît à moitié.
- FIABILITE §21 et `essais_temp.rs` sont Windows seuls (`%TEMP%`, section `.CRT$XCU`) : rien n'est dit pour les essais sur Mac.
- NOTES.md : toutes les notes restent « proposé » même quand la chose est fusionnée depuis trois semaines ; aucune n'est « résolu ».
- AUDIT-2026-11-10, propositions 21 (« Corriger un plan »), 22 (un seul sélecteur), 23 (« Passer au propre »), 24 (« FEANOR Essentiel », Atelier), 26 (entrer dans la case), 27 (une seule façon de garder une version) : aucune trace dans src/, git log ni INTERFACE. 25 faite (135). 28 et 29 (un mois sans nouveauté ; refermer « Tout au stylet » dans la Station) : contredites par le rythme des parties 136 à 201 (3D surtout) ; aucune décision notée.
- AUDIT-2026-10-30 §5 « À ne pas faire avant la V1 : retour de la 3D, Mac… » : contredit par les parties 143-201 (3D) et par le travail fait sur Mac ; V1.md range le moteur 3D en V2.


## Partie 4 — Trouvé par les audits et les notes précédents, corrigé depuis : contrôler que la correction tient (126 points)

Les audits hors dépôt (ARDA) ne concernent pas FEANOR : voir l'en-tête.

### Parties 189 à 201 (lot 6)

- Image du plan grise ou clignotante quand on tire une caméra libre haut, en plongée, dans la vue à côté — 200.7 (vu par Anthony, vidéo) — `cadre_sur_la_feuille` (e016b52, src/document.rs) — Storyboard, vue à côté, tirer la caméra haut et en plongée : l'image ne doit plus passer au gris.
- « Tree oak » (196 triangles) jugé « Ce décor est lourd » juste après l'étalonnage — 198.5 (vu par le coordinateur et Anthony) — attente de l'étalonnage et `loin_sous_le_budget` (e016b52 ou d9c6788, src/decor3d.rs) — Préférences > Performance > Remesurer, puis clic sur Tree oak dans la Banque 3D : pas de fenêtre.
- Aperçus de la Banque 3D minuscules — 198.5 — `banque3dui::cadrer` — chaque modèle doit remplir sa case.
- Pastille de couleur qui coupe « Couleur du fond » et « Couleur du maillage » dans l'onglet Monde — 198.5 — pastille au bord droit — onglet Monde : les noms doivent être entiers.
- « ◆ » affiché en case vide (« □ Clé sur … ») — 201.6 (vu par le coordinateur) — `ph::DIAMOND` (23cb4e4) ; « ✕ » d'un texte de colonneui.rs dit en mots — sélectionner un objet : « ◆ Clé sur … » doit montrer un losange. « Tous les onglets sortis » doit parler de « la croix de son en-tête ».
- Couleur écrasée à 55 points et Préréglages coupé dans la pile — 196.5 (vu par le coordinateur, 1384 × 868 à 90 %) — nouvelle règle `colonne::hauteurs`, `GRANDIT_MIN` = 120 (src/colonne.rs l. 27) ; remplace 195.1 point 12 — refaire le cas, la roue doit être entière et la colonne défiler.
- Un panneau ajouté sous Propriétés l'écrasait — 194 (vu par Anthony) — 195.1 point 12, puis 196.5 — « + Panneau » > Préréglages dans une fenêtre basse.
- « Pinceau » et « calque vectoriel » collés dans l'en-tête des Réglages — 195.1 point 11 (Anthony) — écarts d'origine rendus — Réglages du Pinceau.
- Cmd + clic au stylet qui tremble devenait un lasso vide ; ne marchait que sur le calque courant — 195.1 point 13 (« ctrl+clic ne sélectionne pas un objet ») — seuil de 6 px, autre calque — Cmd + clic au stylet sur le trait d'un autre calque.
- Pas de zoom ni de défilement sur la timeline de la séquence — 198.3 l. 17873 — 201.1 (timelineseq.rs) — Cmd + molette, Option+1 / Option+2.
- Œil et cadenas d'un objet valables pour la séance seulement — 198.3 l. 17870 — enregistrés dans le projet en 199.1 point 4 — enregistrer, rouvrir.
- Les lampes n'éclairaient pas les cartes — 200.1 et 200.6 — 201.4 (image du plan et export seulement) — une lampe rouge près d'une carte qui reçoit les ombres.
- Pas de commande « Dupliquer la séquence », et la séquence née d'une autre perdait son décor — 190.4 l. 16549, 180.5/181.4 — 191, et `copier_decor_de_sequence` en 190 — voir A 191.
- Pas de glisser d'un objet d'une séquence à l'autre — 190.4 l. 16547 — 201.1 point 3 (glisser dans Scène 3D vers le sélecteur) — voir A 201 point 3.
- Barre de lecture : une commande moins utilisée pouvait passer avant une plus utilisée — 192.6 l. 16868 — corrigé là — colonne étroite : ce qui passe sous « ⋯ » doit être le moins utilisé.
- Animation : le « ⋯ » des calques passait au-dessus de l'onglet « Notes » — 192.8 l. 16899 — onglets du bas posés à la vraie largeur — Animation avec une colonne large.
- Message « 1.6 ms au-delà des 1.6 ms » — 198.5 — arrondi à deux chiffres — lire le message d'un vrai décor lourd.

### Parties 170 à 188 (lot 5)

- Copier ou dupliquer une séquence n'emportait pas son décor — 180.5, 181.4 — 190 (couper la séquence, nouvelle séquence avec les
  plans choisis) et 191 (Dupliquer la séquence) — clic droit sur la ligne Séquence > « Dupliquer la séquence » : la copie a le décor.
- Pas de manipulateur ni de clic dans la vue pour les objets du décor — 180.5, 181.4 l. 15512 — 184 (9c7d444) — clic sur un objet
  dans la vue, puis le déplacer à la flèche.
- Copier, coller ou glisser un objet d'une séquence à l'autre — 181.4 l. 15521 — 190 (Cmd+C / Cmd+V, « Copier vers la séquence… »)
  et 201 (glisser une ligne d'objet de Scène 3D sur une séquence, Option pour copier) — essayer les deux.
- Le décor n'était pas sur la vue de dessin (seulement dans l'export et la vue 3D) — trouvé en 183.1.8 — 183 (343a061) — décor
  visible sous les calques sur le dessin, et « Exporter l'image courante » l'a.
- Le bord du décor qui cache une carte était net, au pixel — 183.5 l. 15812 — 193 (bords doux) — un arbre devant une carte, zoomer :
  pas d'escalier.
- La vue Espace 3D ne montrait pas les ombres des cartes — 185.5 l. 16110 — 189 (4481e66) — un personnage aimanté a son ombre dans
  la vue, de tous les côtés.
- « Tree oak » (196 triangles) jugé lourd juste après l'étalonnage — 198.5 (touche le chemin d'import de la 181) — 198.5 — Remesurer
  dans Préférences > Performance, puis cliquer « Tree oak » : posé sans fenêtre.
- Le plan devenait gris quand on tirait la caméra libre dans la vue à côté (avec un décor) — 200.7, depuis 159 + 180 — 200.7 — vue à
  côté, tirer une caméra haut et en plongée : l'image reste.
- La poussée et le raccord filé n'allaient que de droite à gauche ; T1, T2, T5 n'étaient pas faits — 175.3 l. 14851 — 177 (7a9b2bb)
  — clic droit sur une poussée > Sens « du haut ».
- Un glisser du montage dont le lâcher se perdait bloquait tous les raccourcis ; un bouton gardait le clavier — 186.1 — corrigé
  dans la 186 elle-même — geste de la manipulation 186.1.
- Sur la carte graphique, le volet en horloge passait par le flou du raccord filé — 177.2 l. 14983 — corrigé dans la 177 — lire un
  volet en horloge : pas de flou.
- La grille de la timeline passait sur la colonne une fois les calques repliés — 174.2 l. 14760 — corrigé dans la 174 — replier tous
  les calques : la grille commence après la colonne.

### Parties 150 à 169 (lot 4)

- Cmd+clic sur la caméra avec l'outil Trajectoire prenait la surface au sol dessous — JOURNAL l. 758 (vu par Claude) — corrigé
  partie 150 (b7121f5) — Cmd+clic sur le dessin de la caméra posé devant un sol : la caméra est choisie.
- Clic simple de l'outil « choisir » prenait la carte avant la caméra — 150 « pas fait », l. 11380 — corrigé 152.1 (1743878) — clic
  sur la caméra passée devant une carte : la caméra.
- Le panneau Espace 3D devant gardait la hauteur de Réglages et coupait la liste — 152.3, JOURNAL l. 669 — 238e6e1 (aujourd'hui
  la pile de 194/196 décide) — ouvrir le cube, l'onglet Espace 3D se voit en entier.
- La phrase « dessin posé à la main gardé » était coupée par la barre d'état — 152.2 l. 11695 — 9ea2375 — lire la barre d'état après
  avoir glissé une marque près d'une clé posée à la main.
- Une marque posée au milieu décalait les poses redessinées ; un calque dupliqué partageait les poses de l'original — 152.2 — 319a112,
  c99eaf3 — poser une marque entre deux poses redessinées ; dupliquer le calque puis redessiner une pose de la copie.
- Le maillage disparaissait caméra trop proche (vu par Anthony) — 158 l. 12546 — 158.1 (47addb0) — molette tout près du sol.
- Friction du bouton « Plan en Espace 3D » (vu par Anthony) — 158 l. 12547 — 158.2 puis 159 (c0e3df2), bouton retiré — le cube
  suffit, aucun bouton à cocher.
- Champs Bascule / Panoramique ramenaient à 85° une caméra tournée plus loin — 159.3 l. 12743 — corrigé en 159 — panoramique 150°
  dans le cube, fermer, ouvrir l'outil Caméra : la valeur reste 150°.
- Caméra libre « dans les anciennes limites » dont le cadre sort de la feuille : plan gris (vu par Anthony) — issu de 159 — corrigé
  200.7 (JOURNAL l. 22-24) — tirer la caméra dans la vue 3D à côté hors de la feuille : l'image reste filmée.
- Planche des Vignettes et panneau Espace 3D débordaient de la colonne de 290 — 161.3 — 161.4 (d21ede7) — Vignettes et caméra du
  panneau 3D visibles sans défiler (la pile de 194 peut faire défiler, voir 194).
- « Remettre à plat » (Précis du calque 3D) ne faisait pas d'étape d'annulation — 162.2 l. 13115 — corrigé 162 — Remettre à plat
  puis Cmd+Z.
- Ligne du calque en pastilles et opacité sur la miniature (164.3), refusées par Anthony — 165.3 (8de9324) — « 100 % » et
  « Normal ▾ » en texte dans la barre du calque.
- Avant | après : appui immobile + glissé = deux étapes ; valeur au-delà de la barre ramenée ; pincement à l'envers ; effet sans
  force qui réglait un autre — 164.8 — corrigés en 164 — tirer la barre après un appui immobile : un seul Cmd+Z.
- Version dépliée : la vue restait sombre — 168.6 l. 14155 — corrigé en 168 — clic dans le bloc déplié : l'image s'affiche.
- Trait vectoriel pixelisé au zoom (Anthony) → 166 (1362bf6) ; pixelisation pendant zoom / déplacement (Anthony) → 166.8 (bcc3e02) ;
  pointillés de la vue réduite (Anthony) → 166.9 (19f8b18) ; vignettes en pointillés → 166.10 (24b1d44) — gestes de la partie 166
  en A.

### Parties 120 à 149 (lot 3)

- Insérer des images (hors rognage) ne décalait pas la trajectoire — 123 l. 8872, ETAT l. 6 — finitions, partie 119 (7c7df04, 2026-11-08) — insérer des images dans un plan à trajectoire : départ, fin et marques restent sur leurs images.
- La clé de caméra ne se poussait pas quand le plan avait une clé sur sa dernière image — 129.1 — 008de95 — voir A/129 manipulations 1-2.
- Effet vivant sans marque sur la piste Caméra — 129 l. 9385 — blocs dans la timeline (170, blocscamera.rs) — poser un effet : un bloc apparaît sous les clés.
- Trajectoire de la caméra invisible sur la timeline — 149 l. 11224 — barre du chemin de la caméra (172, montagerows.rs:280) — tracer un chemin de caméra : une barre sur la rangée caméra.
- Après Ctrl+Z d'une continuité, un trait dans la case partait dans une image neuve au bout du plan — 128 l. 9236 — `clamp_nav` (b334410) — continuité, Cmd+Z, trait dans la case : le plan garde sa durée, la vignette du montage le montre.
- Ctrl+A des Vignettes prenait la case courante seulement — 132 l. 9685 — partie 134 (d07678e) — voir A/134.
- Mode de fusion, œil, cadenas, pelure, pochoir, effets et clés d'effets, retournement d'un calque à trajectoire : sur un seul calque — 132 (c) l. 9736-9746 — partie 136 (d3662ef) — voir A/136.
- « Peint sur l'aplat » : traits disparus, plans vides créés dans les cases pâles — vidéo d'Anthony — partie 133 (cc9e505) — voir A/133.
- F5 sur plusieurs calques ne posait qu'une clé (clic dans une piste, clés sans nom, double-clic) — retours d'Anthony 11/11 — 137.11 (49472b8) — voir A/137 manipulation 4.
- F5 laissait de côté les calques masqués — retour d'Anthony avec captures — 139.5 (02c1ceb) — trois calques dont deux masqués, fn+F5 → trois clés.
- Ctrl+V ne collait rien avec deux clés restées choisies et un calque — retour d'Anthony — 139.2 (b78ea38) — voir A/139 manipulation 3.
- Glisser des calques dans un dossier ne marchait pas — retour d'Anthony — 139.1 (b78ea38) — voir A/139 manipulation 1.
- Fenêtre des Préférences qui s'élargissait pendant une recherche ; arrêt net sur une feuille < 128 × 128 px — 137.9 l. 10114-10115 — 7e89e7b — taper un mot dans « Chercher un réglage… » : la fenêtre garde sa taille.
- Plantage « Buffer slices can not be empty » en se promenant dans la vue Espace 3D — 147 — 41fe5a9 ; **validé par Anthony** (« Non, plus de plantage », 12/11, JOURNAL l. 808) — traverser sol et mur à la molette, tourner longtemps.
- Trait invisible enregistré hors de la carte (caméra dos aux cartes) — 143 point 9, vu par Anthony — 146.1 (3f4e3e9) — voir A/146 manipulation 6.
- Surface refusée derrière la caméra d'origine ; carte bloquée à −0,9 — 143 l. 10520, ESPACE-LIBRE l. 408 — partie 148 (f2860cd) — voir A/148.
- Caméra retournée relue à 85° à la réouverture ; message « En construction » de la case — 148 l. 11060-11064 — partie 149 (651dfb0 ; `project::bornes_de_camera`, aucun « En construction » restant dans src) — voir A/149 manipulation 6.
- Ctrl+clic sur la caméra avec l'outil Trajectoire prenait la surface dessous — JOURNAL l. 758 (vu par Claude) — partie 150, puis 152 — vu corrigé par Claude seulement (JOURNAL l. 728, 693-697).
- La couche bitmap pouvait perdre des pixels (coupure à l'enregistrement, couche illisible, fantômes, « Enregistrer sous » dans un dossier d'autre projet) — 127 relectures l. 9134-9168 — 8f6469b — non testable à la main sauf « Enregistrer sous » dans un dossier qui a déjà un projet.

### Parties 76 à 119 (lot 2)

- Insérer des images ne décalait pas la trajectoire — ETAT l. 6 (2026-11-07 soir) — corrigé partie 119, ajout du 2026-11-08
  (`Trajectoire::decaler_images`, 7c7df04) — geste : timeline, tête entre deux marques, « Insérer une image avant » plusieurs fois → les
  marques d'après reculent d'autant ; Cmd+Z.
- Bouton du stylet qui n'arrivait pas comme clic droit (winit le jetait) — partie 111 l. 7708 — corrigé partie 114 « branché »
  (9819707), Windows seulement — geste (PC) : bouton tenu, pointe posée → réglages ; sur Mac, régler le pilote en « clic droit ».
- Un trait commencé sur une barre à la hauteur de la barre « réunissait les plans » sans dessiner — partie 111 l. 7768 — corrigé partie 113
  point 15 — geste : poser le stylet sur la barre et tracer vers la case voisine → un trait ; clic net → coupe.
- Part de trait refusée par une case (autre zoom, roulis, calque verrouillé) cachée sous l'image, sans message — parties 111 l. 7767,
  113 l. 7964, 118 l. 8348 — réglé partie 122 puis rendu sans objet par la règle A (131) — geste : trait parti d'une case qui traverse une
  case à autre caméra → visible en entier par-dessus.
- Un bout de trait dans une case pâle lointaine créait des plans vides — analyse capture 02 — corrigé 113 point 12, puis règle 131
  (la marge de la 1re case pâle seule fait naître) — geste : trait qui finit dans la 3e case pâle → aucun plan.
- Notion de « clé » du color script à rapprocher de celle des Images à la clé — partie 109 l. 7490 — fait : `colorscript::premiere_cle`
  appelle `exportcles::cles_du_plan` (`src/colorscript.rs` l. 295-298) — geste : color script « première clé » et Images à la clé sur
  le même plan → même image.
- Les lignes Dialogue / Notes revenaient décochées au lancement — partie 76 / constat 106 — corrigé partie 106 (`same_layout`) — geste :
  cocher, passer au Montage, quitter, relancer → toujours cochées.
- Le double-clic sur un titre de séquence existait sans qu'on le sache — 77 — 106 ajoute bulle, clic droit, petite fenêtre — geste :
  survol 1 s d'un titre.
- Pastille de couleur pâle pendant le geste (38 %) pour le trait qui déborde — 101 l. 7230 — corrigé 101 — geste : trait qui sort d'une
  case → même teinte pendant et après.
- Dégradé perdu par un changement de calque en pleine retouche — JOURNAL l. 1336 — corrigé avant fusion de 3774514 — geste : retoucher
  un dégradé, cliquer un autre calque → le dégradé est écrit sur le calque d'origine.
- Export de la planche : une case à mouvement ne montrait que le cadre A (papier entre les cadres) — JOURNAL l. 1333 — corrigé partie 110
  (tuile rendue à l'export) — geste : exporter la planche avec un plan A→B, passe-partout décoché → dessin entre A et B.
- PDF avec « ? » à la place des accents — JOURNAL l. 1345 — corrigé partie 84 (police TrueType embarquée) — sous Windows seulement ; sur
  Mac voir B.
- Option `WGPU_BACKEND=dx12` qui basculait en OpenGL — partie 95 point 10 — retiré le 2026-11-01 (JOURNAL l. 1403) — sans objet sur Mac.
- Deux pertes de dessins dans la corbeille trouvées en relecture — JOURNAL l. 1274 — corrigées avant fusion de la partie 104 — geste :
  supprimer un plan, fermer, rouvrir, Restaurer → dessins complets.
- Tremblement par défaut qui coupait tout ce qui dépassait de la feuille — partie 118 — corrigé 118 puis 121 (`fxmonde.rs`) — geste :
  liste de la partie 118 ci-dessus.

### Parties 1 à 75 (lot 1)

- Effets du calque absents sur le dessin hors de la feuille — partie 19 l. 1988 (point 10) — partie 121 (effets de part et d'autre du bord, 760cba4) — un flou sur un calque qui déborde de la feuille : le flou continue dehors.
- Dessin coupé loin de la feuille — partie 19 — correction du 2026-10-28 (l. 1992) — dessiner très loin de la feuille, dézoomer : rien de coupé.
- Trous blancs entre le trait et l'aplat après une retouche — partie 33 — correction du 2026-10-28 (l. 3511) — retoucher un contour d'un personnage avec aplat : pas de liseré blanc.
- Second dégradé qui ne couvrait qu'une bande irrégulière — partie 53 l. 4678 (vu par Anthony) — correction `degrade-sur-degrade` (l. 4676) — deux dégradés bitmap l'un sur l'autre.
- Paliers de caméra invisibles à l'écran et à la lecture — partie 51 (vu par Anthony) — 54.1 (e6a506b) — lecture d'un plan à paliers : saccadé à l'écran.
- Lasso bitmap lent sur grande zone (215-418 ms par image) — partie 54.2 (vu par Anthony) — e6a506b — déplacer une zone de 4K au lasso : fluide.
- Question « Modifications non enregistrées » à tort — partie 54.3 (non reproduite) — remplacée par l'écriture continue (135) — fermer un projet ouvert sans y toucher : aucune question.
- Refus d'écrire sur un projet existant dit seulement dans la barre d'état, soirée perdue — partie 67 l. 5531 — 57c04bf — geste 1 de la partie 67 (avec l'écriture continue décochée).
- Deux projets au démarrage, couleurs claires et sombres mêlées sur un poste neuf — partie 73 (capture d'Anthony) — 82feaad puis b201dc1 — premier lancement sans réglages.
- Plan déplacé vers une autre séquence plus montré par les autres versions de sa séquence d'origine — partie 59 l. 5056 — partie 102 — déplacer un plan, revenir à la version A : il y est.
- Aplat d'un trait de personnage manquant si l'on enregistre juste après le trait — partie 32 l. 3373 — partie 103 (point 1 corrigé, point 3 « non reproduit ») — trait de personnage puis Cmd+S tout de suite, rouvrir.
- Cases de textes des Vignettes qui revenaient décochées — réglages de la partie 44 — partie 106 — cocher Action, relancer.
- « ? » à la place des accents dans le PDF — JOURNAL l. 1345 — partie 84 (c1594e9, « PDF avec accents ») — exporter un storyboard PDF avec « é », « à », « ç ».
- Son NTSC qui dérive — JOURNAL l. 1345 — partie 84 (« son NTSC sans dérive », l. 6454) — export montage 23,976 i/s d'un film long, son calé à la fin.
- Mémoire d'un gros projet (~6 Go) — ETAT l. 183 — partie 95 (5f8c27a) et partie 97 — ouvrir un long projet, Moniteur d'activité.
- Couper un plan qui a des versions rangées — question ajoutée en partie 102 — couper un plan à deux versions : une fenêtre demande où elles vont.

### Autres sources (docs, suivi, audits) (lot 7)

- « Marquer l'envoi » renommait `.pelure` en `.shot` sans prévenir — AUDIT-10-30 B1 — partie 85 (abadcb0) — sur un projet `.pelure`, marquer un envoi : rien n'est renommé, le message dit d'enregistrer.
- Dessiner sur un calque vidéo : rien qu'un message — B2 — partie 85 (`videoui.rs`) — tracer sur un calque vidéo : bouton « Transformer en calque d'animation » dans la barre d'état.
- Clic droit d'un calque = seulement « Assigner un raccourci » — B3 — partie 82 (1e6ff18) — clic droit sur un calque : menu avec Fusionner vers le bas.
- Échap ne fermait pas la fenêtre Exporter — B4 — partie 85 — ouvrir Fichier > Exporter, Échap : fermée, rien lancé.
- « crayonnes.json : la planche n'existe plus » à chaque ouverture — B5 — partie 85 (`crayonnesheritage.rs`) — ouvrir un vieux projet : « Ouvert avec N remarque(s) » + « Mettre de côté » puis « Annuler ».
- Deux chemins vers le PDF storyboard — B6 — partie 84 (c1594e9), puis menus d'export en quatre sous-menus (130) — Fichier > Exporter : un seul PDF storyboard.
- Messages de fin d'export qui restaient — B8 — partie 85 — export réussi : la fenêtre s'efface après 8 s, « Ouvrir le dossier » reste dans la barre d'état (sur Mac ce bouton ne fait rien, voir E).
- Montage trop compact, piste son coupée — B9 — partie 86 — réduire la bande : barre de défilement, Son 1 atteignable, boutons M/S/verrou cliquables.
- Numérotation non verrouillée par défaut — B10 — partie 86 — nouveau projet, 3 plans, « Nouveau plan » après le 2 : « 2A ».
- Trop de bulles au premier lancement — audit pratique — partie 89 (92e480d) — premier lancement : une seule proposition « Visite rapide (2 min) ».
- Boîte d'export toujours dans Documents — audit pratique — `dialogues.rs` (81aa222, partie 91) — projet ouvert, exporter : la fenêtre s'ouvre dans le dossier du projet.
- Bobine EDL « PL001 » sans la séquence — livrables — partie 91 §6 — EDL exportée : bobines `S01P012A`.
- PDF sans épisode/version/date, planche PDF « plate » — H4 et livrables — partie 91 (en-tête de studio, planche légendée) — exporter le PDF : en-tête et pages n/N.
- Notes sans statut, retakes non distingués — moyennes de l'audit — partie 88 (5a3687a) — commentaire : statut à faire/fait/refusé, « Marquer comme retake », export CSV.
- Son NTSC qui dérive ; transitions écrites en coupes — H3 — EXPORT_MONTAGE l. 101 et 238 — NTSC coché : son calé ; fondu enchaîné exporté en dissolve (EDL/XML/OTIO). Jamais ouvert dans un logiciel de montage (reste en B).
- Fichier unique / « on n'enregistre plus » — H6, proposition 25 — partie 135 (dac56c7) — dessiner, attendre 1 s : « Enregistré » à droite ; Cmd+S pose un jalon ; Fichier > Jalons… « Revenir à ce jalon ». Le projet reste un dossier.
- ffmpeg non livré — H6 — ffmpeg LGPL allégé dans `Logiciel\ffmpeg\allege` et licence dans À propos (ETAT l. 23, partie 124) — **Windows seulement** (aucun ffmpeg Mac).
- Mémoire 6 à 7 Go pour 200 plans — ETAT l. 182, ENDURANCE §4.1 — §6 (5f8c27a) : 2,0 Go à l'ouverture — geste : sans banc sur Mac (Moniteur d'activité, mémoire de FEANOR après navigation d'un gros projet).
- Dessins des plans supprimés jamais élagués — ENDURANCE §4.2 — §7.2 point 3 (partie 97, `elagage.rs`) — supprimer des plans, pause de 15 s : la mémoire redescend.
- Ctrl+S 60 à 236 ms sur le fil de l'interface — ENDURANCE §4.4 — remplacé par l'écriture continue (135) — dessiner vite : aucune saccade à l'écriture.
- Un plan sorti de sa séquence manquait dans ses versions rangées ; supprimer le dernier plan effaçait les versions ; couper/réunir perdait des versions ; raccourci de version pendant un glisser — V1 Fiabilité — partie 102 (75f900d) — gestes de 102 « À vérifier » 1 à 6.
- Aplat calculé en retard sans étape d'annulation ; « La clé » ne suivait pas un trait modifié ni un copier/coller — V1 — partie 103 (2d1813c) — gros trait, autre trait, Cmd+Z ×2 : plus d'aplat ; Cmd+Maj+Z ×2 : revenus.
- Tab ne menait jamais aux Vignettes ; outils « limités » à l'affichage seulement — V1 Pont — partie 113 (efabcd4) — Maj+Tab dans les deux sens ; touche d'un outil interdit refusée en Vignettes.
- Gestes de la planche peu découvrables (ajouter un cadre seulement au clic droit) — V1 Pont, décision 6 — partie 101 (outil Caméra) et 116 (c6b873c, « + » de Maj) — tenir Maj sur une case : un « + » propose le cadre suivant.
- Dupliquer un plan, outil Caméra, clé de caméra sans touche — V1 Pont — partie 124 (65980bf) — Cmd+Maj+D, Maj+C, Maj+K.
- Traits « hors cadres » dans aucun plan — V1 Pont — parties 125, 127, 128, 131 (a9f3540) — tracer un trait qui sort d'une case : il appartient entier à la case où le stylet s'est posé.
- J/K/L et lecture à vitesse variable — V1 Montage — partie 104 (4e13532/7a0c6ad) — espace Montage, L, L : lecture ×2 ; K arrête ; J recule.
- Export à la clé, son par plan, color script dédié — V1 Exports — parties 105 (10972d7) et 109 (7cac5ed) — exporter « images à la clé » : une image par dessin clé numérotée.
- Insérer des images ne décalait pas la trajectoire — ETAT l. 6 — partie 119/124 (65980bf, « la trajectoire suit les images insérées ») — insérer 3 images avant une clé de trajectoire : le chemin suit.
- Carte impossible derrière la caméra d'origine — ESPACE-LIBRE A6 — levée partie 148 (format 55) — plan en Espace 3D, pousser une carte sous −0,9 de profondeur : elle passe derrière.
- Objets cachés/verrouillés du décor seulement pour la séance — ESPACE3D §22 — §23 / partie 199 : écrits dans le fichier — cacher un objet, rouvrir le projet : toujours caché.
- Branche `sans-logo-barre` « à fusionner » — ETAT l. 37, NOTES l. 20 — fusionnée (1c5a102) — plus de logo à gauche de Fichier.


## Annexe — Parties écartées (remplacées, retirées ou déjà validées par Anthony)

### Parties 189 à 201 (lot 6)

- Aucune partie de la tranche n'est entièrement remplacée ou validée par Anthony. 195 et 196 ne sont gardées que pour ce qui n'a pas été vu. Les retours d'écran 198.5, 200.7 et 201.6 sont rangés en C.

### Parties 170 à 188 (lot 5)

- 179, points 1 à 3 et 179.4 (lignes à 28 points, barre du calque, exception seule) : retirés par le choix C d'Anthony (179.0,
  bebafae).
- 173.6 (icônes espacées de 19 à 30 points) : remplacé par « collé à droite » (192.3, 192.6).
- 174, en-tête des colonnes : retiré (192.6).
- Grille de vignettes du panneau Préréglages (170 à 177) : remplacée par un aperçu et une liste de noms (195.1 point 3), vus par
  Anthony (196).
- Les fusions, poussées et rangements de branches et de dossiers du JOURNAL (l. 142-147, 233-260, 267-297, 319-351, 361-374,
  404-446, 472-478) ne sont pas des fonctions : rien à tester.

### Parties 150 à 169 (lot 4)

- Partie 161 (une colonne de 290 points, densité) : Anthony a validé les 290 points (161.4, l. 13003) ; colonne refaite en pile par 194.
- Partie 156 (suite des Vignettes au démarrage) : retirée par 195.6 (plus aucune vidéo hors du menu Aide ; `AIDE_HORS_DU_MENU = false`, `aideanim.rs:472`).
- Partie 155 (aide dans une fenêtre flottante) : retirée par 195.6, même preuve.
- Partie 153 (aide animée dans Réglages) : joueur retiré par 155 puis 195.6 ; le registre reste et est testé par la liste 195.4 point 11.
- Partie 150 (panneau Espace 3D, Réglages montre l'outil) : essayé par Anthony (ses quatre remarques, 152, l. 11546) ; place du panneau
  remplacée par 152.3, 159 puis 194.7.
- Parties 164.3 et 165.2 (pastilles, opacité sur la miniature) : annulées par 165.3 à la demande d'Anthony.
- Partie 165.4 (ligne du calque toujours visible, flottante) : remplacée par 179.4 ; 165.5 (barre du Storyboard) remplacée par 174 puis 192.1.
- Partie 158.2 (la vue met le plan en Espace 3D toute seule) : remplacée par 159.

### Parties 120 à 149 (lot 3)

- Partie 120 (poignées de lissage au repos) : vue et validée par Anthony, fusion « après deux essais (« ça marche très bien ») », JOURNAL l. 1215-1218, 3a6cfc0.
- Partie 128 (case qui naît prend le papier) : essayée par Anthony avant fusion (« Oui, fusionne » après son essai de l'exe de branche, JOURNAL l. 1178), puis reprise en « traits entiers » par 131 (point 9 de sa liste).
- Partie 147 (plantage de la promenade 3D) : validée par Anthony (« Non, plus de plantage », JOURNAL l. 808 ; INTERFACE l. 10924-10930). Reste seulement les ralentissements (en B).
- Partie 126 : hors de la tranche (l. 7070).

### Parties 76 à 119 (lot 2)

- Partie 111 « Un trait tracé à travers les cases reste continu » (c11f743) : modèle de répartition et marges remplacé par la règle A de
  la partie 131 (JOURNAL l. 1139 ; essayée par Anthony, « ok tu peux fusionner », l. 1081). Le « pas reproduit » des cases 15 et 16 est
  reporté en B.
- Partie 94 — Endurance : aucune interface, seulement des essais `#[ignore]` et la priorité des enregistrements (INTERFACE l. 6991).

### Parties 1 à 75 (lot 1)

- Parties 1, 2, 3, 5, 6 — passes d'inventaire (raccourcis, vocabulaire, doublons, encombrement, propositions) sans fonction propre ; vocabulaire tranché en 89 ; propositions faites en 14.
- Partie 10 — Importer un scénario : remplacé par 75 (l. 567 : l'import ne crée plus de plans) ; sa fenêtre de découpage est vérifiée dans 75.
- Partie 13 — Espace 3D simplifié : refait de bout en bout par les parties 143 à 201 (onglet 3D en 198).
- Partie 15 bis (l. 3634) — Les quatre panneaux plus simples : panneau Calques retiré (160), Couleur et Effets refaits (163, 164).
- Partie 29 — Pistes repliables : remplacée par 68 (l. 2966), puis 137, 165, 174, 178, 179, 192.
- Partie 32 — Brosses par personnage : reprise par 70 (l. 3274) puis 107 ; seules ses limites d'aplat passent en B.
- Partie 36 — Variantes de séquence : remplacée par 59 (l. 3536, format 39).
- Partie 40 — Barres simplifiées : barre des panneaux retirée (194), barres de la vue réorganisées (160), menu du plan refait (64b).
- Partie 41 — Règle des guides, Cadrage, colonne des pastilles : la règle et les repères revus en 42 puis 160, la colonne remplacée par la rangée (50, 70).
- Partie 61 — Charte SHOT orange : remplacée par l'identité FEANOR (96), le gris sombre à rehauts jade (99) et le logo de 167.
- Partie 62 — Les pastilles créent leurs calques : reprise par 70 (l. 5175) puis 107.
- Partie 68 — Seul le calque actif est déplié dans le montage : remplacé par 137 (trois calques, puis toute la pile en 165.7), 174, 178, 179, 192 ; « Calques non actifs » n'existe plus dans src/.
- Partie 71 — Import / export Storyboard Pro retirés (17dda2a, 2026-10-30), consigne.
- Partie 54.3 — Question à la fermeture : remplacée par l'écriture continue (135) ; le reste de 54 est en A et en C.
