# Audit utilisateur de FEANOR — 2026-11-21 (en cours)

Test mené comme un storyboarder, à la souris et au clavier, sur `main` 23cb4e4 (partie 201) construit pour Mac.
**État : quatre lots faits (parcours de bout en bout ; secours, jalons, structure, préférences, exports ; raccourcis, aplat, versions, corbeille, maltraitance ; pinceaux, effets, transformation, commentaires). L'audit n'est pas exhaustif : la liste « Reste à faire » en fin de document dit ce qui n'a jamais été ouvert.**

Pièces dans `suivi/audit-utilisateur-2026-11-21/` :
- `captures/` : les captures citées ci-dessous (01 à 07) ;
- `grille-commandes.md` : toutes les commandes, menus, outils et raccourcis (lu dans le code) ;
- `grille-reglages.md` : réglages, panneaux, imports et exports (lu dans le code) ;
- `a-verifier.md` : fonctions fusionnées jamais vues à l'écran, limites ouvertes, doublons connus.

## Corrections du 2026-11-21 (branche `corrections-audit`, partie de main 85fa0a0)

### Corrigé

| # | Correction | Fichiers | Essai |
|---|---|---|---|
| 2 | Accueil : un chemin de dossier long est abrégé par le milieu (« /Volumes/…/Travail/Anthony »), le chemin entier reste dans la bulle. | `nouveau.rs`, `nouveauui.rs` | `nouveau_tests::a_long_folder_path_…` |
| 3 | La carte « Première fois sur FEANOR ? », un conseil et l'offre du diagnostic passent au-dessus des notifications quand leur pile est haute. | `messages.rs`, `premierjour.rs`, `conseils.rs`, `diagnosticui.rs` | `messages_tests::a_corner_card_sits_…` |
| 6 | Le nom du plan dans une phrase garde ses lettres : « plan 1A », plus « plan 1a » (8 messages). Annuler et Rétablir écrivent « Annulé. » et « Rétabli. » : le message de ce qu'on vient de défaire ne reste plus. | `storyboard.rs` et 6 autres, `app.rs` | pas d'essai dédié |
| 13 | Préférences > Apparence : la phrase des tailles d'origine lit la vraie taille par défaut (16 points). | `echelle.rs` | pas d'essai dédié |
| 16 | « Assigner un raccourci » : le bouton dit « Assigner » quand il n'y a rien à remplacer. | `raccourcisui.rs` | pas d'essai dédié |
| — | Banque 3D : un nom français court pour chacun des 45 modèles ; la recherche trouve encore le nom anglais du fichier. | `banque3d.rs` | `banque3d_tests::every_model_of_the_bank_…` |

### Constats retirés après lecture du code

J'avais mal jugé ces points : ce sont des choix écrits dans le dépôt, ou ils marchent.

- **« Q » supprime un calque (n° 8)** : le raccourci est marqué `source = "perso"` dans `keymap.toml`. C'est le choix d'Anthony, pas un défaut d'origine. Non touché.
- **Passe-partout noir à 100 % (n° 7)** : demandé ainsi (INTERFACE, partie sur le passe-partout : l'ancien voile à 27 % « n'est pas ce qu'on demande »). Non touché. Le constat d'usage reste : un trait hors cadre ne se voit pas.
- **Palette sans les commandes 3D** : voulu par la partie 159 (« la 3D ne doit jamais se sentir tant qu'on ne bascule pas dans la vue de la scène 3D »). Non touché. À revoir si l'onglet 3D, lui, reste visible partout.
- **Colonne de droite (n° 18)** : les panneaux ne sont pas évincés, la colonne défile (partie 196.5 : un panneau garde la hauteur de son contenu, « aucun panneau n'est coupé »). C'est moi qui l'avais fait défiler à la molette. Reste vrai : sur 868 points de haut, il faut défiler pour voir les calques dès que les réglages du pinceau sont dépliés.
- **Scénario dans Propriétés** : c'est la règle de la pile (partie 194 : les panneaux de réglages sont des onglets de Propriétés ; ↗ le sort en panneau). Non touché.
- **« raccourcis du montage » dans la barre d'état (n° 17)** : voulu, il dit quel panneau a la main (`panels.rs:789`).
- **Fiche technique « sans bulle »** : faux, chaque icône a sa bulle (`ficheui.rs:419`) ; mon survol simulé ne l'a pas déclenchée.
- **Son > Bruitages… sans effet (n° 19)** : la commande ouvre un onglet de Propriétés ; je ne l'ai pas vu dans la colonne. Probablement pas un défaut, à regarder à la main.
- **« Notes : Note : » dans le PDF** : l'import étiquette exprès chaque ligne des notes (Décor, Section, Note, Transition). Non touché.

### Laissé de côté

- **Tête de lecture après l'annulation d'une coupe (n° 5)** : l'annulation retient le plan et le calque, pas l'image. La retenir touche 49 endroits du code : trop pour un défaut mineur, à décider.
- **Vignette pas rafraîchie après la pose d'un décor (n° 11)** : pas cherché.
- **Pastille « fx » (n° 20)** : à confirmer à la main avant de toucher.

## Comment lire ce rapport

- **Vu à l'écran** : je l'ai fait et regardé moi-même dans l'app.
- **Lu dans le code** : relevé par un agent dans les sources, pas encore vu à l'écran.
- **Mac** : propre à la version Mac, sans doute absent du PC.
- Gravité : **bloquant** (empêche de travailler), **gênant** (on contourne), **mineur** (cosmétique).

## Conditions du test

- App de test isolée (réglages et projets dans un dossier jetable), projet « Le Reveil » créé à partir de `docs/exemples/Le-Reveil.fountain`.
- Construction Mac : le dépôt ne compile pas tel quel. Il faut `RUSTUP_TOOLCHAIN=stable` (le fichier `rust-toolchain.toml` impose la chaîne Windows), neutraliser `target-cpu=x86-64-v3` de `.cargo/config.toml` (rustc plante), et fournir deux bibliothèques vides `kernel32` et `psapi`.
- Non jugé : pression et inclinaison du stylet, latence ressentie, son à l'oreille, Windows.
- Non testé faute de ffmpeg sur ce Mac : export film, GIF, import vidéo.

## Lot 1 — parcours storyboarder (vu à l'écran)

### Ce qui marche

- Import Fountain, puis « Créer les plans depuis le scénario » : 3 séquences, 9 plans, textes Action / Dialogue / Notes repris (capture 04).
- Dessin dans les cases des Vignettes et sur la planche ; annulation.
- Storyboard : pinceau, gomme, rectangle, pot de peinture, texte avec aperçu, calque bitmap ; lecture à 23,9 i/s.
- Plans : nouveau plan (P, numéroté « 2A »), coupe (F6), annulation de la coupe.
- Caméra : zoom par le coin, déplacement, deux clés, mouvement interpolé.
- 3D : scène affichée, Banque 3D, pose d'un objet au clic, vue « Caméra du plan ».
- Export « Storyboard PDF » : 3 pages, une par séquence, proposé dans le dossier du projet (capture 07).
- Quitter et relancer : le projet se rouvre seul, complet, dans l'onglet où on l'a laissé.

### Bugs

| # | Gravité | Constat | Comment le refaire |
|---|---|---|---|
| 1 | gênant (vu sur Mac, à vérifier sur PC) | Une touche reste « enfoncée » après une saisie de texte : la carte « Raccourcis » (R tenu) reste affichée sur le dessin ; Espace resté pris fait déplacer la vue au lieu de dessiner. | Outil Texte, taper « La cle de Rose », Entrée. Ou : nom de projet « Le Reveil », Créer. La carte part en rappuyant sur R. `aidetouches.rs:203`. Capture 06. |
| 2 | mineur | Accueil : un chemin de dossier long n'est pas tronqué ; le champ déborde sur « Projets récents », « Parcourir… » sort du panneau, « Sera créé » est coupé. | Choisir un dossier au chemin de plus de 60 caractères. Capture 01. |
| 3 | mineur | À chaque ouverture, la bulle « Première fois sur FEANOR ? » recouvre les messages d'ouverture ou de création. | Créer ou rouvrir un projet sans avoir choisi « Ne plus proposer ». Capture 02. |
| 4 | mineur | PDF : la date d'en-tête est en heure universelle (20 novembre affiché le 21 à 0 h 29). Les heures des jalons seraient touchées aussi (`remplacer.rs:69`, lu dans le code ; décalage nul hors Windows). | Exporter le PDF après minuit. |
| 5 | mineur | Après l'annulation d'une coupe F6, la tête de lecture saute à 0. | F6 au milieu d'un plan, puis Annuler. |
| 6 | mineur | Barre d'état périmée : « Plan coupé : la suite est le plan 1a. » reste après l'annulation et d'autres actions ; « 1a » alors que le plan s'appelle « 1A ». | Idem. |

### Ce qui prête à confusion

- **« Enregistrer » introuvable** : Fichier propose « Poser un jalon  Ctrl+S », la carte Raccourcis dit « Enregistrer  Ctrl+S ». Pas d'entrée « Enregistrer ».
- **Caméra** : déplacer le cadre à un autre instant ne crée pas de mouvement ; cela modifie toute la caméra sans message. Il faut le losange du panneau Propriétés.
- **Scénario** : après l'import, il s'affiche dans un onglet de « Propriétés », en bas à droite, sur environ 150 px de haut (capture 03).
- **Décor 3D** : un objet posé depuis un plan apparaît dans tous les plans de la séquence, sans prévenir.
- **Timeline** : la ligne s'appelle « Calque 2 » (plan courant) mais ses blocs dans les autres plans sont étiquetés « Dessin ».
- **Vocabulaire** : « scène(s) » puis « séquence(s) » à l'import ; « Calque de couleur » et « Couleur unie » côte à côte ; modèles 3D en anglais et tronqués ; « Notes : Note : » dans le PDF.
- **Montage** : colonne des en-têtes de calques trop étroite à la première ouverture.

### À confirmer par Anthony

- Décor 3D presque blanc en mode « Matières » et très pâle dans le Montage et le PDF : voulu ?
- Un trait dont l'appui arrive sans mouvement préalable part de l'ancienne position du pointeur (capture 05). Probable artefact du clic simulé : essayer en posant le stylet loin de sa dernière position.
- Un trait tracé de la case 1 vers la case 2 : sa partie tombée dans la case 2 n'apparaît pas dans le plan 2.
- F5 ou F6 pour couper un plan ? F6 coupe à l'écran ; la liste `a-verifier.md` cite F5.

### Propre au Mac (vu à l'écran)

- L'interface affiche « Ctrl+… », c'est Cmd qui agit (Ctrl+Z ne fait rien).
- « Explorateur » dans les menus et messages.
- Outil Texte : Arial déclarée absente, texte dessiné en Ubuntu Light.
- ffmpeg introuvable : l'export film s'arrête sur un message clair, mais « Exporter le film… » garde l'air actif.
- Le sélecteur de fichier s'ouvre sur `~/Documents`, pas sur le dossier du projet.

## Lot 2 — secours, jalons, structure, préférences, exports (vu à l'écran)

### Ce qui marche

- **Écriture continue** : un trait, arrêt forcé du programme 3 secondes après, relance : le trait est là et le projet se rouvre seul.
- **Jalons** : Cmd+S pose un jalon ; « Revenir à ce jalon… » demande confirmation, rend l'état d'avant et garde l'état du jour en jalon.
- **Dupliquer la séquence** : copie complète (dessins, caméra, textes, décor), plans numérotés 4A à 4E ; Annuler la retire en une étape (capture 08).
- **Préférences** : dix onglets, recherche, réglages appliqués tout de suite.
- **Fiche technique XLSX** : 10 lignes, 19 colonnes, time codes, cadrage (« 51,2 mm »), mouvement (« travelling à gauche et vers le haut »), textes du scénario, vignettes à côté.

### Bugs et défauts

| # | Gravité | Constat | Comment le refaire |
|---|---|---|---|
| 7 | gênant | Storyboard : dès que la caméra a bougé, tout ce qui est hors du cadre est noir opaque (passe-partout à 100 % par défaut). Un trait fait hors cadre est invisible ; on ne voit pas la feuille autour pour préparer un pano. | Zoomer la caméra, dessiner à côté du cadre. Réglage : Préférences > Dessin > Opacité du passe-partout. |
| 8 | gênant | Calques > Supprimer a pour raccourci la lettre « Q » seule : une frappe perdue supprime le calque (rattrapable par Annuler). | Menu Calques. |
| 9 | gênant, à confirmer | Une lettre tapée dans la palette de commandes déclenche aussi son raccourci : en tapant « 3D », la barre d'état affiche « Couleurs par défaut… » (touche D). Même famille que le bug 1. | Cmd+K, taper « 3D », regarder la barre d'état. |
| 10 | mineur (Mac) | Heures en temps universel : « Jalon posé à 00:19 » à 1 h 19 ; « À l'ouverture 20/11/2026 à 23:30 » pour 0 h 30 le 21 ; « mesuré le 20/11/2026 » dans Performance. | Fichier > Jalons…. |
| 11 | mineur | Vignettes : la case du plan 1 n'affichait pas le décor 3D alors que sa copie l'affichait ; elle se met à jour après une annulation. | Poser un objet 3D, revenir aux Vignettes. Capture 08. |
| 12 | mineur | Séquence dupliquée nommée « 1. INT. CUISINE - NUIT (copie) » : le « 1. » fait partie du nom venu du scénario, on a deux séquences « 1. » puis « 2. », « 3. ». | Storyboard > Dupliquer la séquence. Capture 08. |
| 13 | mineur | Apparence : « Taille des icônes  16 pt (d'origine) » et, trois lignes plus bas, « 90 % et 13 points : les tailles d'origine ». | Préférences > Apparence. |

### Doublons et points confus

- **Palette de commandes** : « 3D » ne donne que deux résultats (« Espace 3D (afficher / quitter) » et une grille de repère). Ni l'onglet 3D, ni « Ajouter un objet 3D » : confirmé à l'écran.
- **Deux fenêtres d'export pour le montage** : la tuile « Pour le montage » de la fenêtre Exporter, et la fenêtre « Exporter pour le montage » du menu Production.
- **Export pour le montage bloqué sans ffmpeg** : le bouton Exporter est grisé, même pour un EDL seul.
- **Fiche technique** : les boutons d'export et de format sont des icônes sans texte ni bulle au survol.
- **« Animer à l'arrêt »** existe dans Dessin (tremblement par défaut) et, à l'envers, dans Performance (« couper Animer à l'arrêt »).
- **Durée d'un nouveau plan** : réglée à l'accueil (« Premier plan ») et dans Préférences > Montage.
- **Trois cases pour replier les calques** dans Préférences > Montage (« Déplier le calque courant tout seul », « Replier les calques », « Agrandir les calques proches »).
- **Onglets Raccourcis et Export des Préférences** presque vides (trois boutons, une ligne).
- **Mac** : « Bouton et gomme du stylet lus auprès de Windows », coché ; « Nouveau dessin  Inser ».

### À confirmer par Anthony

- Vignettes : la case d'un plan à mouvement de caméra déborde à gauche de sa colonne (cadres A et B hors de la case, numéro recouvert). Voulu ?

## Lot 3 — raccourcis, aplat, versions, corbeille, maltraitance (vu à l'écran)

### Ce qui marche

- **Raccourci personnel** : F1 ouvre « Raccourcis clavier » ; F8 assigné à « Jalons du projet », écrit dans le fichier personnel, et la touche répond aussitôt.
- **Aplat** : Calques > Ajouter un aplat, puis une forme fermée se remplit toute seule ; Annuler retire le trait et son aplat en une étape.
- **Versions du plan** : nouvelle version par duplication (« plan 1 · V2 »), retour à V1, fenêtre Comparer côte à côte avec « 0,4 % de l'image diffère ».
- **Corbeille** : un plan supprimé y arrive (« Plan envoyé à la corbeille »), « Restaurer » le remet à sa place.
- **Annulations en chaîne** : 15 annulations puis 15 rétablissements, sans plantage, état retrouvé ; « Rien à annuler » et « Rien à rétablir » en bout de course.
- **Durée absurde** : tirer la fin d'un plan bien avant son début s'arrête à 1 image, avec un message clair.
- **26 lettres tapées au hasard** : aucun plantage ; tout ce qui a été déclenché est rattrapable.

### Bugs et défauts

| # | Gravité | Constat | Comment le refaire |
|---|---|---|---|
| 14 | gênant, à confirmer | Après « Supprimer les plans choisis », FEANOR s'est retrouvé en mode Animation sur le plan suivant (bandeau « Retour au storyboard ») sans que je l'aie demandé. | Storyboard, cliquer un plan vide du montage, Storyboard > Plans > Supprimer les plans choisis. |
| 15 | mineur | Numérotation mêlée : un plan inséré entre « 2 » et « 2A » s'appelle « 2.5 ». Lettres et décimales cohabitent. | P avec le plan 2 courant alors que 2A existe. |
| 16 | mineur | Fenêtre « Assigner un raccourci » : le bouton dit « Remplacer » alors que la commande n'avait aucun raccourci et que la touche est « Libre ». | F1, icône clavier d'une commande sans touche. |
| 17 | mineur | Un texte « raccourcis du montage » reste collé à gauche de la barre d'état, par-dessus la taille du projet. | Vu après la fermeture de la fenêtre Raccourcis. |

### Révision du bug 1 (touche qui reste enfoncée)

Il s'est reproduit après les 26 lettres tapées au hasard, sans aucun champ de texte. La cause la plus probable est donc ma frappe simulée (appui et relâchement dans la même image d'écran), pas la saisie de texte. À vérifier à la main avant de s'en inquiéter : taper très vite un R, ou un texte avec un R. S'il se reproduit à la main, c'est qu'un appui très bref n'est jamais vu comme relâché.

### Doublons et points confus

- **Trois noms pour Ctrl+S** : « Poser un jalon » (menu), « Enregistrer » (carte), « Enregistrer, ou poser un jalon » (liste des raccourcis).
- **Réunir et fusionner** : « Réunir avec le plan suivant », « Réunir avec le plan précédent », « Fusionner les plans choisis ».
- **Dupliquer, réutiliser, nouvelle version** : « Dupliquer le plan », « Réutiliser le plan (même dessin, autre cadre) », « Nouvelle version, en dupliquant la version en cours ».
- **Allonger** : « Allonger le plan d'une image  + » (Storyboard) et « Allonger l'exposition  Alt++ » (Animation).
- **Menu Aide** : pas d'entrée « Tous les raccourcis » (F1 n'apparaît que sur la carte), pas de mode d'emploi. « Revoir les conseils » et la visite rapide sont aussi dans Préférences > Général.
- **Mode Animation** : il n'a plus d'onglet, mais le menu Animation et « Animer ce plan  Tab » y mènent ; on en sort par un bandeau « Retour au storyboard ».
- **Mac** : « Afficher le projet dans l'Explorateur Windows » dans la liste des raccourcis.

## Lot 4 — pinceaux, effets, transformation, commentaires (vu à l'écran)

### Ce qui marche

- **Pinceaux** : bibliothèque par familles (crayons, encres et feutres, peinture, craie et fusain) avec aperçu du trait ; changer de brosse change bien le rendu (craie texturée, pinceau net) ; réglages de pointe (dureté, rondeur, angle, espacement).
- **Effets de calque** : 11 familles (Trait, Matière, Flou, Déformation, Réglages, Bruit, Pixéliser, Rendu, Netteté, Styliser, Cinéma). Le flou gaussien s'applique en direct, sans toucher au dessin, avec une pastille « fx » sur le calque (capture 10).
- **Transformer** : boîte à poignées, agrandir par le coin, déplacer, valeurs chiffrées (x, y, largeur, hauteur, rotation, origine).
- **Commentaires** : « Nouveau commentaire » en pose un à l'image courante, compté dans « À faire ».

### Bugs et défauts

| # | Gravité | Constat | Comment le refaire |
|---|---|---|---|
| 18 | gênant | La colonne de droite ne tient pas sur un écran de portable : selon l'outil, un panneau grandit et évince les autres. Choisir une brosse a fait disparaître les panneaux Scène (les calques) et Couleur ; avec Transformer, Scène est réduit à une ligne ; le champ pour écrire un commentaire est hors de vue. Aucune barre de défilement ne le signale. | Fenêtre de 1384 × 868 points, Storyboard, déplier « Plus de réglages » du pinceau puis choisir une brosse. Capture 09. |
| 19 | gênant, à confirmer | Son > Bruitages… n'a eu aucun effet visible, ni dans le Storyboard ni dans le Montage (la commande bascule le panneau Bruitages : il s'ouvre peut-être hors de vue, voir 18). | Menu Son > Bruitages…. |
| 20 | mineur, à confirmer | La pastille « fx » reste sur le calque après avoir décoché le seul effet. | Calques > Effets > Flou > Flou gaussien, cocher puis décocher. |

### Non testé dans ce lot

- **Son** (import, lecture, pistes) et **import d'image** : ils passent par le sélecteur de fichier de macOS, que je ne peux piloter qu'en prenant l'écran ; la demande est restée sans réponse. Un son et une image de test sont prêts.
- **Pelure d'oignon**, effets de trait (tremblement, écriture…), sélection au lasso.

## Relevé dans le code, pas encore vu à l'écran

Valable aussi sur PC :
- La palette de commandes cache tout nom contenant « 3D » ou « carte » hors de la vue Espace 3D (`keymap.rs:831`).
- « Importer > Une image (calque bitmap)… » n'a pas d'action : absent de la palette, pas de raccourci possible (`panels.rs:133`).
- « Vue caméra » et « 2D » affichent le même raccourci pour deux effets opposés (`viewbar.rs:545`, `548`).
- Clic droit sur l'onglet « Storyboard » : assigne une touche à « Entrer dans le plan », pas à l'onglet (`panels.rs:348`).
- Menus du clic droit d'un plan du montage et d'une case des Vignettes refaits à la main : ni raccourci affiché, ni « Assigner » (`storyboard.rs:782-806`, `boardui.rs:2680-2719`).
- Formats proposés mais non lus : bmp et tga (séquence d'images), bmp, tif, webp (pointes et papiers).
- Taille des icônes : défaut réel 16 pt, l'écran annonce « 13 points : les tailles d'origine » (`tailleicones.rs:29`, `echelle.rs:88`).
- « Valeurs d'origine » (Préférences > Performance) remet les défauts du code, pas l'état d'avant le diagnostic promis par la bulle (`diagnosticui.rs:597`).
- Doublons de réglages : quatre réglages pour replier les calques dont un jamais lu (`timeline_tout_replie`) ; deux durées de nouveau plan (Nouveau projet, Préférences) ; « Animer à l'arrêt » (Dessin) contre « couper Animer à l'arrêt » (Performance).
- Vidéo à 25 i/s dans un projet à 24 i/s : une image sur 25 saute (`video.rs:45`).
- 731 actions dans le code : 22 non implémentées, 22 doubles cachés, 14 touches par défaut sans effet.
- Outils de montage prévus par `V1.md` et absents du code (entrée/sortie, lier image et son, vitesse, marqueurs, titres), sans décision écrite.
- Suivi plus à jour : `ETAT.md` (Storyboard Pro, « 3D retirée »), `JOURNAL.md` (« pas de commit » pour 193 à 201), `NOTES.md` (tout en « proposé »).

Propre au Mac :
- Lancé normalement, FEANOR n'enregistre rien hors du projet : réglages, récents, copie de secours, raccourcis personnels dépendent de `APPDATA` et `LOCALAPPDATA` (`donnees.rs:22-46`).
- ffmpeg : seul `ffmpeg.exe` est cherché, et le sélecteur n'accepte que `.exe` (`export.rs:353-384`, `exportui.rs:96-97`).
- « Montrer dans l'explorateur » lance `explorer.exe`, « Modifier le fichier à la main… » lance `notepad.exe` (`exportui.rs:1089`, `app.rs:5677`).
- Changer d'onglet de projet au clavier devient Cmd+Tab, pris par macOS ; « Nouveau dessin » n'a que Inser ; F11 est pris par macOS.
- « Ctrl tenu = lasso » et la validation d'une note de planche lisent la vraie touche Control, à l'inverse du reste (`springtools.rs:94`, `boardui.rs:2083`).
- Pression et bouton du stylet probablement absents (`stylet.rs:544`, `canvas.rs:469`) : déduction, à confirmer au premier trait.
- Clés d'API, « Rester connecté » de Kitsu et serveur de son local reposent sur des mécanismes Windows.

## Reste à faire

- **Jamais ouvert** : effets de trait, pelure d'oignon, sélection au lasso, plan écarté, versions de séquence, réunir des plans, réglages du projet, commentaires, références, bibliothèque, relecture et envois, équipe et partage, Kitsu, son (import, lecture, bruitages, exports), liste des plans, images par plan, planche, color script, menus du clic droit, sous-menus de Calques et d'Affichage.
- **Gros projet** : tenue avec des centaines de plans, non essayée.
- Imports d'image, de vidéo, de son et d'objet 3D ; export film et GIF (ffmpeg à fournir).
