# FEANOR — Grille d'audit : réglages, panneaux et fenêtres, imports et exports

Lecture du code de `/Users/anthony/CLAUDE/Feanor-main` (branche `main`, 21/11/2026), sans lancer l'application. Les libellés sont recopiés du
code ; une valeur entre « » est le texte lu à l'écran. Chemins de source relatifs à `src/` sauf mention. Les commandes de menus, les
outils et les raccourcis sont couverts par une autre grille : ils ne sont cités ici que pour dire où s'ouvre une fenêtre.

**À lire avant de commencer sur Mac (point bloquant, voir liste 3)** : les dossiers de données sont lus dans les variables `APPDATA` et
`LOCALAPPDATA` (`donnees.rs:23-45`), qui n'existent pas sous macOS. Sauf si le testeur les définit avant de lancer FEANOR
(`export APPDATA=~/Library/Application\ Support LOCALAPPDATA=~/Library/Caches` dans le Terminal, puis lancer l'exécutable depuis ce
Terminal), **aucun réglage de cette grille ne survit à la fermeture**, il n'y a ni copie de secours ni récupération, ni projets récents,
ni modèles d'épisode, ni raccourcis personnels. Conseil : faire la grille deux fois, une fois sans les variables (pour constater les
échecs), une fois avec (pour tester les réglages eux-mêmes).

Abréviations de la colonne « Où » : **Préf.** = Fichier > Préférences… (fenêtre « Préférences », onglets à gauche, champ « Chercher un
réglage… » en haut) ; **Exp.** = Fichier > Exporter… (fenêtre « Exporter », quatre familles) ; **Ctrl+K** = palette de commandes.

---

## Partie 1 — Réglages et options

### 1.1 Fenêtre Préférences : cadre général

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Préférences (fenêtre) | Fichier > Préférences…, ou roue dentée de l'accueil | 850×770 au départ, min 560×320, redimensionnable | Onze onglets à gauche ; « Les réglages jouent tout de suite. » | prefsui.rs:478-499 |
| Chercher un réglage… | Préf., en haut | texte libre, sans accents ni majuscules | Montre les groupes de tous les onglets qui répondent ; le nombre trouvé s'affiche à côté de chaque onglet ; croix « Effacer la recherche » | prefsui.rs:507-603, prefsonglets.rs:376 |
| Onglet retenu | Préf. | Général d'origine | La fenêtre rouvre sur le dernier onglet (clé `onglet_preferences`) | prefsui.rs:607-610 |
| Onglet « Génération » | Préf. | caché tant que « Afficher les fonctions expérimentales » est décoché (sauf s'il répond à la recherche) | Vérifier qu'il apparaît/disparaît | prefsui.rs:531 |

### 1.2 Préf. > Général

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Rouvrir le dernier projet au lancement | Préf. > Général > Au lancement et à l'ouverture (aussi sur l'accueil) | case, cochée | Au lancement, rouvre les onglets de la séance d'avant dans le même ordre ; si la séance s'était finie sans projet, démarre sur l'accueil | prefsui.rs:634, accueil.rs:117, settings.rs:202 |
| Ouvrir dans un nouvel onglet | id. | case, cochée | Ouvrir / Nouveau / récent / import prennent un onglet ; décoché : remplace le projet après la question habituelle | prefsui.rs:641, settings.rs:206 |
| Un nouveau projet s'ouvre sur | id. | Vignettes (défaut) / Storyboard | Espace affiché après création d'un projet | prefsui.rs:648-659 |
| Enregistrer en continu | Préf. > Général > En cas de plantage | case, cochée | Le projet s'écrit dès que le stylet se lève ; fermer ne demande rien ; Ctrl+S pose un jalon. Décoché : seul « Enregistrer » écrit | continu.rs:223, settings.rs:21 |
| Copie de secours toutes les … min | id. | 0 à 30 min, 1 ; 0 = jamais | Copie dans `%LOCALAPPDATA%\SHOT\secours` ; indicateur « Secours il y a … » | recoveryui.rs:107-127, settings.rs:17 |
| Proportions différentes : Contenir / Remplir | Préf. > Général > Coller depuis un autre projet | Contenir (défaut) / Remplir | Coller d'un projet d'autre format : tout visible avec bandes, ou cadre rempli et bords coupés | collage.rs:430, adaptation.rs:108 |
| Cadence différente : Garder les images / Garder la durée en secondes | id. | Garder les images (défaut) | Le plan collé change de durée, ou garde ses secondes | collage.rs:435 |
| Demander à chaque collage dont les proportions ou la cadence diffèrent | id. | case, cochée | Fenêtre « Coller depuis un autre projet » à chaque collage concerné | collage.rs:439, collage.rs:387 |
| Revoir les conseils | Préf. > Général > Conseils et visite | bouton ; « n conseil(s) vu(s) sur N » | Les bulles d'un seul passage reviennent | conseils.rs:199-205 |
| Refaire la visite rapide | id. | bouton | Relance « Premier jour » (5 étapes) | premierjour.rs:339 |
| Afficher les fonctions expérimentales | Préf. > Général > Fonctions expérimentales | case, décochée | Montre le menu Labo (suggestions sonores, Mon panneau, banc d'essai) et l'onglet Génération | prefsui.rs:769, settings.rs:327 |

### 1.3 Préf. > Apparence

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Thème | Préf. > Apparence > Thème et couleurs | Gris sombre (défaut), Atelier, Papier, Gris clair, Sombre, Personnalisé | Change toute l'interface aussitôt. **La bulle ne cite que 4 thèmes sur 6** | prefsui.rs:163-185, theme.rs:44-69 |
| Rétablir (thème) | id. | bouton | Revient à Gris sombre et efface les couleurs personnalisées | prefsui.rs:178 |
| Fond des panneaux / Barres / Fenêtres et menus / Boutons et curseurs / Texte / Accent et sélection / Autour du dessin / Timeline et montage / Bordures | id. | 9 pastilles de couleur | Toucher une couleur passe en « Personnalisé » à partir du thème courant | prefsui.rs:186-197, theme.rs:253-261 |
| Taille de l'interface | Préf. > Apparence > Tailles | 90 % (défaut), 100, 125, 150 ; « Par défaut » | Textes, boutons, panneaux ; le dessin ne change pas | echelle.rs:23-26, 67-80 |
| Taille des icônes | id. | 11 à 16 pt, **16 d'origine** | Icônes de la timeline et des deux barres de la vue. **Le texte sous la grille dit « 90 % et 13 points : les tailles d'origine »** | tailleicones.rs:24-29, echelle.rs:88 |
| Ligne Séquence / Ligne Plans / Ligne Caméra / Ligne Calques (hauteur) | id. | Séquence 10,14…32 (défaut 10) ; Plans 22…56 (26) ; Caméra 18…36 (22) ; Calques 36…64 (défaut LAYER_H) | Hauteur de chaque rangée de la timeline du Storyboard | hauteurstimeline.rs:25-37, montagerows.rs:79-123 |
| Ligne … (couleur) + « Par défaut » | id. | pastille, absente = couleur du thème | Couleur de fond de chaque rangée (mêlée à 35 %) | hauteurstimeline.rs:40, settings.rs:147-156 |
| Ajuster les panneaux à leur contenu | Préf. > Apparence > Panneaux (aussi Affichage > Panneaux) | case, cochée | Réglages de l'outil, fiche, effets prennent la hauteur de leur contenu ; un bord tiré reste | prefsui.rs:621, workspaceui.rs:229 |
| Réajuster les panneaux | id. | bouton | Remet tous les panneaux à la hauteur de leur contenu, bords tirés compris | prefsui.rs:628 |
| Dossier des images d'accueil : Changer… / Dossier d'origine | Préf. > Apparence > Images d'accueil | défaut `Documents/SHOT/Identité/Accueil` (la bulle écrit des antislashs) | Une image du dossier, au hasard, sur l'écran d'accueil ; sans image : le logo | accueilimages.rs:58-66, 396-416 |
| Importer des images… | id. | png, jpeg | Copie les images dans le dossier (créé s'il manque) | accueilimages.rs:419, 476 |
| Images du dossier : Retirer / Supprimer ce fichier du dossier ? Supprimer / Garder | id. | liste de miniatures | **Efface le fichier du disque, pas de Corbeille** | accueilimages.rs:429-461 |

### 1.4 Préf. > Dessin (stylet, poignées, caméra, tremblement)

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Croix du système sur le pinceau | Préf. > Dessin > Stylet et pinceau | case, cochée | Curseur système en plus du cercle du pinceau | prefsui.rs:667, settings.rs:25 |
| Bouton et gomme du stylet lus auprès de Windows | Préf. > Dessin > Stylet à ma main | case, cochée | Bouton du stylet = clic droit, stylet retourné = Gomme. **Sans effet hors Windows** | prefsui.rs:777, stylet.rs:544-547 |
| Régler le stylet à ma main… | id. | bouton → fenêtre (voir 2.3) | Calibre la courbe de pression sur trois traits | prefsui.rs:785, reglagestylet.rs:234 |
| Ce que le stylet transmet… | id. | bouton → fenêtre (voir 2.3) | Banc d'essai : appui, inclinaison, bouton, gomme | prefsui.rs:790, styletui.rs:118 |
| Point fixe des poignées | Préf. > Dessin > Poignées de la Sélection et du Transformer | Poignée opposée (défaut) / Centre | Point qui reste en place quand on tire un coin ou un côté | prefsui.rs:689, settings.rs:358 |
| Rotation autour de | id. | Pivot (défaut) / Coin opposé | Centre de rotation juste hors d'un coin | prefsui.rs:696, settings.rs:361 |
| Couleur du passe-partout | Préf. > Dessin > Caméra | pastille, noir | Couleur du hors-champ autour du cadre caméra (pas dans les exports) | prefsui.rs:336, settings.rs:291 |
| Cadre de format | id. | pastille, gris 150,150,150 ; clic droit : gris | Couleur du cadre de format de diffusion (Cadrage > Format) | prefsui.rs:352-367, settings.rs:421 |
| Opacité du passe-partout + Rétablir | id. | 0 à 100 %, 100 | 0 % : rien couvert ; Rétablir = noir opaque | prefsui.rs:370-389, settings.rs:281 |
| Placer un nouveau calque devant la caméra | id. | case, cochée | Un calque neuf se pose devant la caméra quand elle a quitté la feuille | calquedevant.rs:81, settings.rs:174 |
| Tremblement du trait par défaut | Préf. > Dessin > Tremblement des traits | case, décochée | Tous les traits vectoriels tremblent (écran, vignettes, exports) sauf calques qui ont le leur ; avertissement « n calques ont leur propre tremblement » | prefsui.rs:396-461 |
| Intensité du tremblement par défaut | id. | 0 à MAX px, 2 | Amplitude du déplacement | prefsui.rs:411 |
| Fréquence du tremblement par défaut | id. | 0 à 100 %, 55 | Doux → nerveux | prefsui.rs:415 |
| Variantes du tremblement par défaut | id. | MIN à MAX, 3 | Nombre de dessins qui se suivent | prefsui.rs:419 |
| Vitesse du tremblement par défaut | id. | 1 à MAX img/var., 2 | Images par variante | prefsui.rs:423 |
| Épaisseur du tremblement par défaut | id. | 0 à 100 %, 0 | Variation d'épaisseur du bord | prefsui.rs:427 |
| Graine du tremblement par défaut | id. | 1 à 999, 1 | Autre hasard | prefsui.rs:431 |
| Animer à l'arrêt (tremblement par défaut) | id. | case, décochée | Le trait bouge à l'arrêt ; aperçu à droite | prefsui.rs:436-444 |
| Rétablir (tremblement) | id. | bouton grisé si d'origine | Éteint, 2 px, 55 %, 3, 2, 0 %, 1, immobile | prefsui.rs:447 |

### 1.5 Préf. > Anneau

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Anneau sous le stylet | Préf. > Anneau > Le geste | case, cochée | Clic droit maintenu : anneau de 8 commandes ; clic bref : réglages de l'outil | anneauui.rs:463-496, anneau.rs:335 |
| Délai avant d'afficher l'anneau + Rétablir | id. | 0 à 1000 ms, 180 | Attente avant que l'anneau se dessine | anneauui.rs:477-490, anneau.rs:255 |
| Commande de chaque direction | Préf. > Anneau > Les huit commandes | une liste par direction ; « Centre : Les réglages de l'outil » fixe | Une commande déjà présente échange sa place | anneauui.rs:518-571 |
| Rétablir les commandes d'origine | id. | bouton | Crayon et pinceau en haut, gomme et annuler en bas, plan précédent à gauche… | anneauui.rs:572 |
| Envoyer dans l'anneau | clic droit sur une commande (Mes habitudes, raccourcis) | menu des 8 directions | Remplace la commande de la direction | anneauui.rs:160-165 |

### 1.6 Préf. > Montage

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Hauteur des plans | Préf. > Montage > Pistes du montage | moyenne (défaut) / en bandeau | Hauteur de la rangée des plans (Storyboard et Montage) | pistes.rs:1204, settings.rs:179 |
| Déplier le calque courant tout seul | id. | case, cochée | Espace Montage : seule la piste du calque actif est dépliée | pistes.rs:1210, settings.rs:123 |
| Replier les calques | id. (aussi ⋯ de la ligne « Calques ») | case, décochée | Storyboard et Animation : tous les calques sur une ligne fine | pistes.rs:446-470, settings.rs:134 |
| Agrandir les calques proches quand il y a de la place | id. | case, cochée | Les calques voisins du calque actif prennent la grande hauteur | pistes.rs:1219, settings.rs:183 |
| Hauteur des pistes son | id. | petite / moyenne / grande, par espace de travail | Hauteur d'une piste son | prefsonglets.rs:279, montagehauteur.rs |
| Durée d'un nouveau plan … s ou … i + Rétablir 2 s | Préf. > Montage > Plans neufs | secondes, 2 s ; « à N i/s, la cadence du projet ouvert » | Seuls les plans créés ensuite ont cette durée | dureeplans.rs:227-242, settings.rs:366 |
| Verrouiller la numérotation des projets neufs | id. | case, cochée | Un plan inséré dans un projet neuf reçoit 2A ; ne touche pas les projets existants | numerotationui.rs:25, settings.rs:384 |

### 1.7 Préf. > Vignettes

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Rangées de cases pâles | Préf. > Vignettes > Planche des Vignettes | 1 à MIN_ROWS_MAX, 3 | Rangées vides toujours visibles ; une de plus dès qu'on dessine dans la dernière | prefsui.rs:204-218 |
| Cases par rangée | id. | 1 à 6 | Colonnes de la planche (= réglage « Colonnes » en haut de la planche) | prefsui.rs:219-227 |
| Taille des cases | id. | CARD_MIN à CARD_MAX pt, 240 | Largeur d'une case, aussi dans l'export de la planche | prefsui.rs:230-238 |
| Débord du trait hors des cases | id. | 0 à DEBORD_MAX %, 12 | Partie du trait visible hors du cadre (sauf case active) | prefsui.rs:239-253, vignettesreglages.rs:99 |
| Zoom d'ouverture … cases × … rangées | Préf. > Vignettes > À l'ouverture | 6 cases × 2 rangées | Cadrage de la planche à l'ouverture d'un projet | vignettesprefs.rs:308-318 |
| Taille des numéros de plan | Préf. > Vignettes > Numéros et textes | %, 100 | Taille du numéro (reste entre 15 et 72 points à l'écran) | vignettesprefs.rs:178-181 |
| Montrer les numéros | id. | case, cochée ; touche N | Numéros au-dessus des cases et dans l'export | vignettesprefs.rs:188 |
| Lignes de texte d'un projet neuf (action, dialogue, notes) | id. | trois cases, décochées | Lignes sous les cases d'un projet neuf | vignettesprefs.rs:194-200 |
| Passe-partout des cases + opacité | Préf. > Vignettes > Passe-partout des cases | case + 0-100 % | Voile hors des cases (affichage seul) | vignettesprefs.rs:251-263 |
| Plus de réglages (repliable) | Préf. > Vignettes | replié d'origine | Ouvre les réglages ci-dessous | prefsui.rs:565-575 |
| Fond clair derrière les numéros | id. > Numéros et textes | case, cochée | Pastille claire derrière le numéro | vignettesprefs.rs:218 |
| Pastille de version à côté du numéro | id. | case, cochée | « V2 » à côté du numéro d'un plan à plusieurs versions | vignettesprefs.rs:224 |
| Afficher la durée du plan | id. | case, décochée | Durée secondes+images à côté du numéro | vignettesprefs.rs:230 |
| Couleur des lignes de texte | id. | une pastille par ligne | Couleur de l'intitulé et du filet | vignettesprefs.rs:237-242 |
| Épaisseur du filet | id. > Aspect des cases | pt, 1 | Trait autour de chaque case | vignettesprefs.rs:274-277 |
| Couleur du filet : Choisir | id. | case décochée = gris d'origine | Couleur du cadre des cases | vignettesprefs.rs:283-286 |
| Dessin hors des cases | id. | % d'opacité | Opacité du dessin hors de toute case | vignettesprefs.rs:297-300 |
| Aimant des cadres de caméra | id. > Cadres de caméra | 0 à AIMANT_MAX points, 6 ; 0 = éteint | Collage des cadres A, B, C aux alignements | prefsui.rs:257-272 |
| Couleur des cadres de caméra | id. | pastille, bleu #378ADD | Cadres A, B, C, lettres et flèche, écran et export | prefsui.rs:273-282 |
| Report du décor | id. > Décor | case, **cochée d'origine** | Décocher la barre entre deux cases reporte le décor | prefsui.rs:288-299, vignettesreglages.rs:105 |
| Calque Décor : Vectoriel / Bitmap | id. | Vectoriel (défaut) | Sorte du calque Décor créé par les brosses du décor ; avertit si des plans ont déjà le leur | prefsui.rs:300-317 |
| Brosses Décor : Ouvrir le panneau | id. | bouton | Ouvre « Brosses dédiées », section Décors | prefsui.rs:319-323 |
| Rétablir les réglages des Vignettes | id. | bouton | Remet débord, aimant, couleur des cadres, report, sorte du décor (pas rangées ni colonnes) | prefsui.rs:325 |

### 1.8 Préf. > Raccourcis, Export, Équipe

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Voir et changer les raccourcis… | Préf. > Raccourcis | bouton | Ouvre « Raccourcis clavier » (voir 2.9) | prefsui.rs:731 |
| Modifier le fichier à la main… | id. | bouton | Crée `%APPDATA%\SHOT\keymap.toml` et l'ouvre dans **notepad.exe** | prefsui.rs:735, app.rs:5677, keymap.rs:1358 |
| Recharger | id. | bouton | Relit le fichier de raccourcis | prefsui.rs:739 |
| ffmpeg (vidéo, GIF, son) : « cherché tout seul » / Choisir… / Automatique | Préf. > Export > Programmes | auto (défaut) ou chemin | Programme des vidéos, GIF, son, import vidéo. **Le sélecteur ne montre que les fichiers .exe** | prefsui.rs:800-822, exportui.rs:94-107 |
| Ouvrir la fenêtre Exporter… | id. | bouton | Ouvre la fenêtre Exporter | prefsui.rs:826 |
| Signer mes commentaires : | Préf. > Équipe > Commentaires | texte ; vide = variable USERNAME, sinon « Anonyme » | Nom sur les commentaires de la timeline | prefsui.rs:746-755, commentaires.rs:183-188 |
| Mon nom : | Préf. > Équipe > Travail à plusieurs | texte, 40 car. max ; vide = signature | Nom vu des autres équipiers | cooperationui.rs:930-935, equipe.rs:50 |
| Une prise périme après … min sans nouvelles. | id. | 2 à 240 min, 10 | Un plan tenu redevient libre | cooperationui.rs:944-950, equipe.rs:38 |
| Prendre le plan libre que je commence à modifier | id. | case, décochée | Sinon modifier un plan libre est refusé | cooperationui.rs:952 |
| Dire dans la barre d'état ce que font les autres | id. | case, cochée | « Léa a enregistré le plan 12 »… | cooperationui.rs:954 |

### 1.9 Préf. > Génération (fonctions expérimentales)

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Lecture du storyboard | Préf. > Génération (cocher d'abord « fonctions expérimentales ») | Règles (sans modèle) (défaut) / Local (Qwen2.5-VL) / Service en ligne (Claude) | Qui lit les textes des plans pour proposer des sons | genprefsui.rs:149-180, genreglages.rs:17-135 |
| Joindre la vignette de chaque plan | id. (si Lecture ≠ Règles) | case, cochée | Le modèle voit l'image du plan ; en ligne, l'image quitte l'ordinateur | genprefsui.rs:180 |
| Bruitages et ambiances | id. | Local (Stable Audio Open) (défaut) / Service en ligne (ElevenLabs) | Fournisseur des sons | genreglages.rs:29-47 |
| Voix témoin des dialogues + Voix par défaut | id. | Local (Kokoro-82M, voix « ff_siwis ») / en ligne | Fournisseur et voix | genprefsui.rs:185, genreglages.rs:131 |
| Service / Modèle / Saisir la clé… / clé enregistrée / Effacer la clé | id. (rôle en ligne) | liste de services ; fenêtre « Clé de … » | Clé rangée dans le Gestionnaire d'identifiants **de Windows** | genprefsui.rs:273-337, clessecretes.rs:285-300 |
| Dossier des modèles : Choisir… | id. > Serveur local | vide = `Documents/SHOT/Modèles/SHOT-modeles` | Dossier python, venv, modèles | genprefsui.rs:214-217, serveurlocal.rs:41-58 |
| Python du serveur : Choisir… | id. | vide = `venv/Scripts/python.exe` du dossier ; filtre `.exe` | Python qui lance `sidecar/serveur.py` | genprefsui.rs:225-229, serveurlocal.rs:65-67 |
| Démarrer le serveur tout seul quand une génération locale en a besoin | id. | case, cochée | Lance le serveur local à la demande | genprefsui.rs:236 |
| Adresse 127.0.0.1, port | id. | 8765 | Port du serveur local | genprefsui.rs:241, sidecar.rs:23 |

### 1.10 Préf. > Performance (et Aide > Diagnostic des performances)

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Effets en aperçu réduit : pendant la lecture et les gestes | Préf. > Performance | case, décochée | Effets à mi-résolution quand l'image bouge | diagnosticui.rs:546, diagnostic.rs:153-168 |
| Traits vectoriels texturés | id. | Rendu exact aussitôt (défaut) / Rendu rapide jusqu'au repos / Rendu rapide partout | Vaut pour les projets ouverts ensuite | diagnosticui.rs:549, diagnostic.rs:69-71, ouverture.rs:74-120 |
| Tremblement du trait : couper « Animer à l'arrêt » | id. | case, décochée | Force immobile tout tremblement à l'arrêt | diagnosticui.rs:556 |
| Pelure d'oignon | id. | 0 = pas de plafond (défaut) à 12 images | Plafonne les images de pelure de chaque côté | diagnosticui.rs:559, onion.rs:134 |
| Cache de la carte graphique | id. | 128 à 16384 Mo, 1024 | Mémoire vidéo gardée ; règle aussi le budget de la planche de références | diagnosticui.rs:562, referencesui.rs:344 |
| Cache des traits vectoriels | id. | 64 à 8192 Mo, 512 | Pixels gardés des traits | diagnosticui.rs:565 |
| Cache des effets | id. | 64 à 8192 Mo, 768 | Dessins gardés des effets | diagnosticui.rs:568 |
| Fils de calcul | id. | 0 = automatique à 64 | Cœurs utilisés | diagnosticui.rs:571 |
| Vignettes | id. | Résolution normale (défaut) / Moitié / Quart | Vignettes du montage et de la timeline | diagnosticui.rs:574, diagnostic.rs:93-95 |
| Animations : interface sans animations | id. | case, décochée | Plus de transitions de survol, panneaux d'un coup | diagnosticui.rs:581 |
| Effets sur la carte graphique | id. | case, cochée ; grisée « non disponible sur cette carte graphique » | Flous, lueurs, bruit sur la carte à chaque image | diagnosticui.rs:583-586 |
| Diagnostic des performances… / Revenir comme avant / Valeurs d'origine | id. | boutons | Ouvre la fenêtre de diagnostic ; « Revenir » seulement après un diagnostic ; **« Valeurs d'origine » met les défauts du code alors que sa bulle dit « ce qu'ils étaient avant le diagnostic »** | diagnosticui.rs:588-598 |
| Affichage : … | Préf. > Performance > Affichage | texte | Nom de la carte et du moteur d'affichage | prefsui.rs:760 |

### 1.11 Réglages du projet (écrits dans le projet)

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Réglages du projet (fenêtre), onglets Réglages / Résolution | `projet.proprietes` (menu Projet, Ctrl+K « Réglages du projet ») | — | — | projetui.rs:192-202 |
| Préréglages (format) | Résolution | 13 formats : HD 1080, HD 720, UHD 4K, 2K DCI, Scope 2,39, SD PAL 4:3, Vertical 9:16, Carré 1:1, Portrait 4:5, A4, A3, A5, US Letter | Pose largeur et hauteur | projetui.rs:222-225, nouveau.rs:136-149 |
| Largeur / Hauteur | Résolution | px, 16 à 4096 | Taille du cadre | projetui.rs:234-240, nouveau.rs:15-16 |
| Mettre les dessins à l'échelle / Garder les dessins à leur taille | Résolution | radio | Garder : seul le cadre change, centré | projetui.rs:248-255 |
| Cadence | Résolution | liste 12, 15, 24, 25, 30, 48, 50, 60 i/s + champ libre ; **pas de 23,976 ni 29,97** | Images par seconde | projetui.rs:62, 259-265 |
| Garder les durées en secondes / Garder les images (le film va plus vite ou plus lentement) | Résolution | radio | Effet d'un changement de cadence | projetui.rs:268-273 |
| Remettre | Résolution | bouton | Annule les changements en cours | projetui.rs:282 |
| Titre / Information / Auteur / Notes / Copyright | Réglages | texte | Métadonnées ; Auteur et Copyright vont dans les modèles d'épisode | projetui.rs:387-413 |
| Date de création / Dernière modification | Réglages | lecture seule (« Posée à chaque enregistrement. ») | — | projetui.rs:402-406 |
| Série ou projet de production / Épisode | Réglages | texte | Servent aux noms des exports | projetui.rs:419-424 |
| Durée cible du film / Durée cible par séquence | Réglages | « 11:30 » ou « 690 » ; « aucune » | Affiche « trop long » quand dépassé | projetui.rs:354-445 |
| Noms des plans : Modèle de noms… | Réglages | bouton → « Noms et production » | Modèle de noms des plans | projetui.rs:450-453 |
| Numéro de la première image | Réglages | nombre | Numérotation des images | projetui.rs:458 |
| Couleur des aplats sous le trait + Aux aplats déjà posés | Réglages | pastille | Couleur des aplats neufs ; le bouton recolore les existants | projetui.rs:466-474 |
| Noms et production (fenêtre) : Série ou projet / Épisode / Modèle prêt / Modèle / Jetons / Aperçu | Exp. > Noms et production, ou `projet.noms_production` | modèles prêts (SQ01_PL012A…) ou modèle à jetons | Tous les exports nomment leurs fichiers ainsi | nommageui.rs:27-122 |
| Enregistrer comme modèle d'épisode… | Noms et production, ou `projet.enregistrer_modele_episode` | nom | Voir 3.4 | nommageui.rs:133, modeleepisodeui.rs:31 |
| Numérotation des plans : Suivie / Verrouillée + Renuméroter les plans | barre de la planche / fiche (numérotation) | Suivie ou Verrouillée (défaut des projets neufs : Verrouillée) | Verrouillée : plan inséré = 2A ; Renuméroter remet 1, 2, 3 après confirmation | numerotationui.rs:82-132 |

### 1.12 Pinceaux, brosses et toucher du stylet

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Liste des pinceaux : Rechercher / dossiers / liste ou vignettes | Réglages de l'outil (pinceau, crayon), bouton à droite de l'aperçu du trait | — | Choisir une brosse | pinceauxui.rs:250-271, settings.rs:236 |
| Menu d'une brosse : Renommer… / Dupliquer dans « Mes brosses » / Remplacer par le réglage courant / Supprimer / Ajouter à Mon panneau | clic droit sur une brosse | — | — | brushui.rs:59-71 |
| Catégorie (liste) | Réglages de l'outil | Forme de la pointe, Taille et appui, Dispersion, Grain et papier, Couleur et mélange, Lissage, Toucher du stylet ; ● allumée, ○ éteinte | Chaque catégorie s'allume / s'éteint sans perdre ses valeurs | pinceauxui.rs:514-530, pinceaux.rs:276-294 |
| Taille | id. | px | — | pinceauxui.rs:166-174 |
| Pointe : ronde ou image ; Cases ; ordre (À la suite / Au hasard / Selon l'appui / Selon la direction) ; Dureté ; Rondeur ; angle ; Espacement | Forme de la pointe | — | Forme des empreintes | pinceauxui.rs:550-655, 849, dynui.rs:19-21, brush.rs:351-354 |
| Taille selon l'appui, Taille mini, Opacité, Opacité mini, Débit | Taille et appui | — | Réponse à la pression | pinceauxui.rs:884-899 |
| Connexions : source (Aucune, Pression, Vitesse, Direction, Fondu, Hasard), Inverser, Mini, vitesse px/s, longueur px | Taille et appui | — | Ce qui fait varier un réglage | dynui.rs:128-183, brush.rs:450-466 |
| Retourner au hasard ↔ ↕, Taille au hasard, Dispersion, Dispersion le long, Nombre | Dispersion | — | Empreintes au hasard | dynui.rs:25-42 |
| Papier (lisse, papiers…), Grain, Taille du papier, angle, Inverser, Douceur du grain, Grain du crayon (Aucun, Soie, Doux, Moyen, Dur), Le grain suit le trait | Grain et papier | — | Texture | pinceauxui.rs:674-711, brush.rs:395-399 |
| Mélange (Pigment / Eau / Lisser), Charge, Eau, Entraînement | Couleur et mélange | — | Peinture humide | dynui.rs:87-103, brush.rs:218-228 |
| Lissage (Suivi / Corde tirée / Moyenne (points) / Moyenne (distance)), force, px, Points, Après le trait, Selon le temps, Pixels d'écran, rattrapage (Sans / droit / en courbe) | Lissage | — | Lissage du trait | pinceauxui.rs:911-946, brush.rs:260-308 |
| Toucher : courbe Droite / Douce / Ferme, Plancher de taille, Seuil d'appui, Traits fins pleins, Bord doux, Nouveau toucher / Ancien toucher | Toucher du stylet (commun à tous les pinceaux) | courbe linéaire, plancher 0,10, seuil 0,03, fins pleins oui, bord doux oui | Réponse du stylet ; « Ancien toucher » = plancher 0,03, seuil 0, sans fins pleins ni bord doux | pinceauxui.rs:731-752, brush.rs:660-695 |
| Opacité (brosse) | id. | — | — | pinceauxui.rs:774 |
| Enregistrer comme nouvelle brosse… / Rétablir l'ancien réglage de « … » / Enregistrer dans la brosse | bas des réglages de brosse | — | « Rien à enregistrer » si inchangé | pinceauxui.rs:808-824 |
| Peindre derrière / Peint sur l'aplat | Réglages du pinceau et du crayon | deux interrupteurs exclusifs, éteints | Peint sous le trait, ou directement dans l'aplat ; retenus d'une séance à l'autre | settings.rs:188-193 |
| Brosses dédiées (Classique, Décor, Personnage) : Modifier… / Dupliquer / Supprimer / Nom / « + » | panneau « Brosses dédiées » (onglet de la Couleur) | Classique et Décor = réglages du poste ; Personnage = du projet | Pastilles de la barre de la vue | personnagesui.rs:1124-1262, settings.rs:306-313 |
| Dégradé : Forme, Vers la couleur de fond / Vers le transparent, Opacité, angle, %, Inverser, Tramage, Tout le calque, Mes dégradés (Nom, Supprimer), Couleur de la pastille | Réglages de l'outil Dégradé | — | Liste « Mes dégradés » retenue sur le poste | degradeui.rs:870-1122, settings.rs:376 |
| Cutter : Écart toléré, Bouts effilés, Dépassements jusqu'à (px), mode, portée, geste | Réglages de l'outil Cutter | 0 px, 0 px, 12 px ; Couper ; Calque ; Travers | Réglages du poste | cutterui.rs:247-256, cutter.rs:94 |

### 1.13 Couleur, pelure d'oignon, caméra, Espace 3D, son

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Couleurs d'avant-plan / d'arrière-plan | panneau Couleur | noir (encre) / blanc | — | couleur.rs:206-219 |
| Roue chromatique (montrer/cacher) ; carré ou triangle | panneau Couleur, menu | montrée, **Triangle** | Forme du sélecteur saturation / valeur | couleur.rs:211-213, keymap.rs:204-206 |
| Curseurs TSV / RVB / TSL | panneau Couleur (clic droit sur les curseurs) | TSV | — | colorui.rs:122, 516 |
| Code hexadécimal | panneau Couleur | #RRGGBB ou #RGB ; clic droit : copier | — | colorui.rs:410 |
| Couleurs récentes / Effacer les couleurs récentes | panneau Couleur | vide | — | colorui.rs:644-649 |
| Nuanciers : choix du nuancier montré, Ajouter, Nouveau, Renommer, Dupliquer, Supprimer, Importer, Exporter | panneau Couleur | nuancier « Pelure » d'origine | — | nuancierui.rs:248-334, keymap.rs:216-222 |
| Pelure d'oignon (fenêtre) | `pelure.panneau`, barre sous le dessin | — | — | onionui.rs:68 |
| Mode de pelure | id. | Instance (défaut) / Image / Signet / Marque de couleur | Ce que « 3 avant » compte | onionui.rs:85, onion.rs:41-53 |
| Avant / Après : nombre, Coloriser, Dégradé, couleurs, opacité par rang | id. | 3 avant (rouge), 3 après (vert), coloriser oui, dégradé non, max 12 | — | onionui.rs:21-56, onion.rs:105-125 |
| Devant / Lier les curseurs / Opacité globale | id. | Opacité 45 % | — | onionui.rs:90-105 |
| Activer tout / Désactiver tout / Réinitialiser | id. | boutons | — | onionui.rs:108-114 |
| Décalage (± px) | id. | glisser sur le dessin | Décaler la pelure | onionui.rs:129 |
| Réglages rangés : Ranger / Reprendre ces réglages | id. | « Aucun : range les réglages courants sous un nom. » | Préréglages de pelure | onionui.rs:138-159 |
| Cadrage > Format | barre sous le dessin, menu Cadrage | Aucun / 16:9, 9:16, 4:5, 1:1, 4:3, 1.85, anamorphique / Personnalisé | Cadre de format dans le cadre caméra | cadrageui.rs:160-200, frames.rs:8 |
| Zones de sécurité | id. | aucune / Action 93 % · titres 90 % (EBU R 95) / Action 90 % · titres 80 % (à vérifier) | — | cadrageui.rs:213, cadrage.rs:56-57 |
| Repères de composition (tiers, grille d'or, spirale + orientation, diagonales, croix) | id. | tous éteints | Aides d'écran, jamais exportées | cadrageui.rs:74-100, cadrage.rs:112-127 |
| Aspect des traits des repères : opacité %, épaisseur px, Autre couleur | id. | 70 %, 1 px, cyan 90,220,255 | — | cadrageui.rs:226-251 |
| Perspective | id. | — | Guides de perspective (menu de la règle) | cadrageui.rs:63 |
| Passe-partout (allumé/éteint) | barre sous le dessin, `camera.masque` | allumé | Retenu sur le poste | settings.rs:289 |
| Effet de caméra : Aucun effet / liste ; Intensité, Durée, Hz, Graine, Avancée, Zoom, mm, °, Hauteur, Paliers, Coup de zoom, Placer | Fiche du plan / caméra | — | Pose des clés de caméra | camfxui.rs:98-378 |
| Aspect de la vue 3D | onglet 3D / vue Espace 3D | Matières (défaut) / Couleurs / Argile | Réglage du poste | couleurs3d.rs:86-96, settings.rs:171 |
| Espace 3D : Le plus proche passe devant ; Position, Rotation, Échelle, Étirement, Profondeur ; Caméra du plan : Position, Angles (pan, basc., rot.), Focale mm + objectif ; Grille du sol ; Montrer les calques ; Poignées dans le repère du calque ; Aimanter à la grille (Déplacement m, Rotation °, Taille) | panneau « Espace 3D » (onglet de Propriétés quand la vue s'ouvre) | — | — | espace3dui.rs:708-1065 |
| Décor 3D de la séquence : Soleil (° autour, ° haut), Facettes, Ciel, Brume, Ombres, Dans les plans | panneau Décor (onglet 3D) | — | Rendu du décor | decor3dui.rs:882-908 |
| Lampes : Nom, Position, Intensité, Portée m, Teinte, Enlever la lampe ; objets : Caché dans le plan, Clé à l'image, Effacer les clés | panneau Lumières / Objet | — | « Une lampe ne porte pas d'ombres » | miseenscene3dui.rs:450-546 |
| Son pendant la lecture / Son au défilement | commandes `son.lecture`, `son.defilement` | allumés ? (à vérifier) | — | keymap.rs:742-743 |
| Effets du son : volume %, Fondu d'entrée / sortie (i), Fondu enchaîné avec le suivant, Clés de volume, Égaliseur dB, Écho %, ms, % | clic droit sur un extrait son | — | — | audiofxui.rs:381-444 |
| Feuille d'exposition : Image courante surlignée, Points d'entrée et de sortie, signets, Trait épais toutes les … secondes, Colonnes Action, Dialogues/Notes, Son, Calques, Caméra, Valeurs par défaut | panneau Feuille d'exposition (Animation) | tout coché, séparateur 0 | — | xsheetui.rs:335-352, xsheet.rs:159-172 |
| Flips : Nom, Sorte, Limites, Checkpoints, Flips par seconde, Direction H/V, En arrière, Distance px, Boucler, Revenir à l'image de départ, Nouveau flip | fenêtre Flips | un flip | — | flipsui.rs:96-324 |
| Rôles : Montrer les rôles dans la timeline, Opacité des rôles | menu des rôles | — | — | rolesui.rs:72-76 |

### 1.14 Ce qui s'écrit dans `reglages.toml` sans case dans les Préférences

| Clé | Où ça se règle | Défaut | Effet | Source |
|---|---|---|---|---|
| `disposition`, `disposition_animation`, `disposition_montage`, `disposition_vignettes`, `disposition_3d`, `espaces` | glisser les panneaux, Affichage > Espaces de travail | dispositions d'origine | Dispositions par espace et espaces nommés | settings.rs:56-111, workspaceui.rs:152-271 |
| `barres` | épingler des commandes dans les barres | vide | Barre de commandes / montage / timeline | settings.rs:104 |
| `cles_vues` | légende des clés | tout | Sortes de clés montrées | settings.rs:107 |
| `largeur_entetes_montage`, `largeur_colonne_storyboard` | tirer le bord de la colonne de gauche | constantes | — | settings.rs:114-120 |
| `storyboard_camera_depliee`, `storyboard_calques_replies` | ⊕/⊖ des rangées de la timeline du Storyboard | repliée / dépliée | — | settings.rs:159-163 |
| `panneau_scene` | panneau Scène (Depuis : film, séquence, plan) | d'origine | — | settings.rs:166, sceneui.rs:119 |
| `compteur_en_images`, `compteur_timeline_en_temps` | Ctrl + clic sur le compteur | time code / images | — | settings.rs:336-340 |
| `references_fenetre` | planche de références détachée | non détachée | Détachée, plein écran, écran, transparente | settings.rs:381, planchref.rs:502-514 |
| `scenario_taille` | panneau Scénario | Moyen (Petit / Moyen / Grand) | Taille du texte | settings.rs:373, scenariogarde.rs:236 |
| `bibliotheque` | panneau Bibliothèque | vue Grille, vignettes 104, volume 0,8, tri Nom | — | biblio.rs:170-185 |
| `bruitages` | panneau Bruitages | aucun dossier | — | settings.rs:268 |
| `pistes_hauteur` | Affichage / Préf. | moyenne | — | settings.rs:389 |
| `compter_les_commandes` | Aide > Mes habitudes > « Compter mes commandes » | coché | Compteur local des commandes | settings.rs:397, usageui.rs:158 |
| `mon_panneau` | Labo > Mon panneau | vide | Boutons rangés | settings.rs:370 |
| `exports_v1`, `export_avec_annotations`, `fiche_technique`, `liste_des_plans`, `paquet_de_livraison`, `kitsu`, `dossier_export_montage` | fenêtres d'export | voir partie 3 | Choix d'export retenus | settings.rs:27-42, 91-99 |
| `budget_decor_3d`, `diagnostic` | diagnostic, premier décor 3D | mesuré | Budget 3D de la machine | settings.rs:257-265 |
| `projets_recents`, `onglets_ouverts`, `onglet_actif`, `sans_projet_a_la_fin` | automatique | — | — | settings.rs:199-217 |
| `nouveau_projet` | fenêtre Nouveau projet | dernières valeurs, préréglages perso | — | settings.rs:240 |
| `ffmpeg` | Choisir… | absent | Chemin de ffmpeg | settings.rs:13-15 |

---

## Partie 2 — Panneaux et fenêtres

### 2.1 Panneaux de la colonne (pile « façon Blender »)

Règles communes (parties 194-196) : en-tête de 24 pt avec poignée ⠿, chevron et titre (plier/déplier), ▲ ▼ ✕ ; bord du bas tirable (70 à
700 pt) ; « + Panneau » en haut de la colonne ; **Propriétés** regroupe les panneaux de réglages en onglets d'icônes verticaux (clic
droit : Sortir l'onglet en panneau / Dans une fenêtre à part / Fermer l'onglet). **Il n'y a plus de panneau « Calques » (partie 160) ni
de panneau « Pinceaux »** : leurs contrôles sont dans la timeline et dans les Réglages de l'outil.

| Libellé à l'écran | Où l'ouvrir | Contrôles et valeurs | Effet attendu | Source |
|---|---|---|---|---|
| + Panneau / Ajouter un panneau | haut de la colonne | menu ; « (déjà là) » grisé | Le nouveau panneau arrive en bas ; « La colonne est vide : « + Panneau »… » | colonneui.rs:123-184 |
| Dans une fenêtre à part / Panneau flottant / Déplier / Dans la colonne | clic droit sur un en-tête | — | Fenêtre système séparée ou panneau flottant | colonneui.rs:342-393, dockui.rs:81-90 |
| Outils | colonne de gauche | icônes | Barre d'outils | dock.rs:233 |
| Couleur (+ Brosses dédiées en onglet) | haut de la colonne | voir 1.13 | — | dock.rs:234, 251 |
| Réglages de l'outil (« Réglages ») | Propriétés | selon l'outil | — | dock.rs:236 |
| Fiche du plan (« Fiche ») | Propriétés | textes, durée, caméra, versions du plan (« Version du plan (n) » : Passer à / Restaurer / Supprimer la Vn, Comparer le plan…, Mettre à jour dans les autres versions de séquence…) | — | dock.rs:239, planversui.rs:247-366, panneauxvisuelsui.rs:145-197 |
| Mouvement de caméra (fiche) : Cadre A…, départ / arrivée, Ajouter un cadre | Fiche du plan | — | « Pas encore de mouvement : un pictogramme le pose, ou Ctrl+Maj+glisser » | panneauxvisuelsui.rs:335-424 |
| Effets | Propriétés | Effets du calque allumés, Animer à l'arrêt, Ajouter un effet, Préréglages, Tout réinitialiser, Tout supprimer, Création automatique des clés, Clés d'effets dans la timeline et le montage, Appliquer pour de bon, Créer une clé, Supprimer la clé, Clé précédente / suivante, force | « Aucun effet sur ce calque : clique une vignette » ; refus sur un dossier ou storyboard verrouillé | fxui.rs:196-1069 |
| Scénario | Propriétés | texte du scénario importé, taille Petit/Moyen/Grand | Lecture du scénario ; vide tant qu'aucun scénario n'est importé | dock.rs:254, scenariopanneau.rs |
| Espace 3D | Propriétés (arrive quand la vue Espace 3D s'ouvre) | voir 1.13 | — | dock.rs:256, espace3dpanneau.rs:61 |
| Historique | Propriétés | Annuler / Rétablir, « Début de l'historique », liste des étapes | Clic sur une étape : y revenir ; « Rien à annuler pour l'instant » | historyui.rs:13-58 |
| Commentaires | Propriétés (Montage) | Nouveau commentaire, précédent / suivant, liste, Modifier, Couleur, Statut, Retirer le retake, Supprimer, texte, Auteur, Couleur | Commentaires posés sur la règle de la timeline | commentairesui.rs:144-650 |
| Notes / fil de décision | Commentaires (vue notes) | Tout le film / Fil du plan ; envoi (Tous les envois) ; Statut ; Raison du refus (obligatoire) ; Retake + motif ; En conflit avec… ; réponse ; export des retakes (CSV) | Suivi des notes de relecture | fildecisionui.rs:91-590 |
| Bruitages | Propriétés (Montage) | Dossiers (n), Retirer le dossier, Chercher un bruitage… | « Aucun dossier de sons. » ; glisser un son dans le montage | bruitagesui.rs:265-343 |
| Préréglages | Propriétés | vignettes animées des mouvements de caméra, transitions, plans types | Glisser sur la piste Caméra | dock.rs:248, panneauprereglages.rs |
| Feuille d'exposition (« Exposition ») | Propriétés, Animation seulement | voir 1.13 | — | dock.rs:243 |
| Suggestions sonores (« Suggestions », Labo) | Propriétés, si fonctions expérimentales | portée (du plan … au …), Lire le storyboard, propositions, Choisir les fournisseurs, Modifier la proposition (Sorte, Image du plan, Durée voulue, Description, Personnage, Réplique, Voix, Prompt (anglais)) | Première génération en ligne : fenêtre « Envoyer vers un service en ligne ? » (Confirmer et continuer / Annuler) | suggestionsui.rs:535-951 |
| Références | Propriétés | Chercher…, étiquette (Toutes…), lecture vidéo ×vitesse, décalage | « Aucune référence. » ; se détache en fenêtre | referencesui.rs:803-1529 |
| Planche de références (vue détachée) | Références > détacher | Écran n, opacité de la fenêtre 20-100 %, menu : Ajuster à l'écran, Taille réelle, Écrire dans le bloc, Mettre devant/derrière, Ranger, Mettre dans un cadre, Recadrer, Rétablir l'image entière, Retourner G-D / H-B, Noir et blanc, Opacité, Couleur, Remettre droit, Lier au plan courant, Supprimer, Post-it / Texte / Titre / Cadre ici, Ranger toute la planche, Tout voir, Tout sélectionner, Importer des images… / une vidéo…, Coller une image, Importer la planche d'un autre projet… | — | planchrefui.rs:433-823 |
| Bibliothèque | Propriétés | onglets Fichiers / 3D, favoris (Retirer des favoris), Chercher dans ce dossier…, Taille des vignettes, Remonter d'un dossier, Tout montrer (œil), Volume de l'aperçu ; lecteurs (sur Mac : seulement « / ») | Clic droit sur un fichier : importer dans le plan, montrer dans l'explorateur | biblioui.rs:576-1336, biblio.rs:645-662 |
| Banque 3D | onglet 3D / Bibliothèque > 3D | Chercher un modèle (arbre, route, maison…), familles | Clic : dans le décor 3D de la séquence, devant la caméra | banque3dui.rs:176-234 |
| Mon panneau (Labo) | Propriétés, si fonctions expérimentales | Enregistrer l'outil actuel ici, Retirer, Assigner un raccourci…, Renommer…, Ajouter un séparateur, recherche « Une commande : persp, retourner, flou… », Afficher les noms | — | monpanneauui.rs:339-636 |
| Planche (des Vignettes) | colonne de l'espace Vignettes | séquences (Renommer la séquence…, Dupliquer la séquence), menus Équipe / Plus / Séquence, notes choisies (Attacher, Détacher, Couleur courante, Texte plus grand, Supprimer), Nouveau plan, Tout choisir (plans), Planche à la hauteur de la vue, zoom %, Cadrer tout | — | boardui.rs:2684-2833, planchesequencesui.rs:54-441, vignettesprefs.rs:103-141 |
| Scène (arbre film → séquences → plans → calques) | colonne, Storyboard | Depuis : film / séquence / plan | — | sceneui.rs:119-170, settings.rs:166 |
| Onglet 3D : Scène 3D, Objet, Caméra, Lumières, Monde, Décor, Timeline de la séquence, Banque 3D | espace 3D | Chercher dans la scène ; Choisir la caméra du plan ; Montrer le soleil et les lampes dans la vue ; Grille du sol ; « image n / N », « Séquence n · n plans » | — | onglet3dui.rs:44-513, dock.rs:259-266 |
| Montage et timeline (bande du bas) | toujours en bas | voir grille des outils | — | dock.rs:241 |

### 2.2 Accueil, nouveau projet, ouverture, récupération

| Libellé à l'écran | Où l'ouvrir | Contrôles et valeurs | Effet attendu | Source |
|---|---|---|---|---|
| Écran d'accueil compact | au lancement sans projet à rouvrir, ou s'il y a des copies de secours | cadre « Nouveau projet », cadre « Projets récents », roue « Préférences », image d'accueil | — | accueilfenetre.rs, accueil.rs:173-260, lib.rs:1524-1534 |
| Projets récents : liste, « Introuvable : … », Retirer de la liste, Ouvrir un projet… (Ctrl+O), Entrée ouvre | accueil | — | Le projet reste sur le disque | accueil.rs:217-278 |
| Rouvrir le dernier projet au lancement | accueil | case (même réglage que Préf.) | — | accueil.rs:117 |
| Nouveau projet (fenêtre) | Fichier > Nouveau projet, ou cadre de l'accueil | Nom du projet (« Mon film », requis), Dossier (8 derniers + Parcourir…), Modèle d'épisode (Aucun / modèles), Épisode, Titre, Sous-titre, Format (préréglages + Mes préréglages + Personnalisé), Taille / Largeur, Cadence (12…60 i/s + 1-120), Nom du préréglage (« HD sombre… ») | Défaut 1920×1080, 24 i/s | nouveauui.rs:127-483, nouveau.rs:136-365 |
| Plus d'options | Nouveau projet | Unité px / mm / cm / in ; Résolution 36-1200 dpi (300) ; Papier (case cochée + couleur blanche) ; Aplat sous le trait (couleur blanche) ; Premier plan 0,1-600 s (2 s) | Papier décoché : transparent | nouveauui.rs:357-403, nouveau.rs:18-19, 365 |
| Préréglages perso : enregistrer / supprimer | Nouveau projet | nom | Liste « Mes préréglages » | nouveau.rs:209-245 |
| FEANOR s'est fermé sans enregistrer (fenêtre modale) | au lancement si copies orphelines | « Des copies de secours ont été trouvées » : Ouvrir la copie / Supprimer / Tout ouvrir / Plus tard | Plus tard : reproposé au prochain démarrage | recoveryui.rs:141-180, lib.rs:1525 |
| Projets non enregistrés / Modifications non enregistrées / Enregistrement / Enregistrement impossible | fermeture, enregistrement | — | Questions de fermeture (si l'écriture continue est éteinte) | onglets.rs:1081-1149, panels.rs:916, enregistrement.rs:273-414 |
| Ouverture (progression) | ouverture d'un gros projet | — | — | ouverture.rs:84 |
| Jalons | Fichier > Jalons… (Ctrl+S en pose un) | nom du jalon, « automatique », Revenir / Annuler / Supprimer | Dix derniers jalons automatiques gardés ; un jalon nommé n'est jamais retiré | jalonsui.rs:242-285 |
| Corbeille | menu | Supprimer…, Restaurer, Supprimer définitivement, Vider la corbeille… | « Ça ne s'annule pas » | corbeilleui.rs:288-365 |
| Onglets de projets : Fermer, Montrer dans l'explorateur | onglets en haut | — | — | ongletsui.rs:167-208 |

### 2.3 Aide, stylet, diagnostic, À propos

| Libellé à l'écran | Où l'ouvrir | Contrôles et valeurs | Effet attendu | Source |
|---|---|---|---|---|
| À propos (modale) | Aide > À propos | version, logo ; Licence et sources de ffmpeg (Sur ce poste : …), Ouvrir le dossier ffmpeg-licence, Les sources (ffmpeg.org), configuration du ffmpeg livré, Fermer | Dit quel ffmpeg est trouvé | aproposui.rs:27-132 |
| Raccourcis clavier | Aide > Liste des raccourcis (F1), Préf. | Chercher une commande ou une touche ; (storyboard) (animation) (montage) ; « Raccourci de FEANOR » / « Ton raccourci » ; Souris et stylet ; clic droit : Assigner un raccourci…, Ajouter à Mon panneau, Revoir l'explication | — | raccourcisui.rs:44-346 |
| Assigner un raccourci | id. | « Appuie sur la combinaison voulue… », Remplacer / Ajouter / Autre touche / Retirer le raccourci / Raccourci par défaut ; Échap ferme | — | raccourcisui.rs:146-219 |
| Registre des explications | Aide > Registre | animations des outils et textes des fenêtres | — | aideregistre.rs:50 |
| Aide-mémoire des raccourcis (maintenir) | Aide | — | — | keymap.rs:145, aidetouches.rs |
| Palette de commandes | Ctrl+K | recherche | — | palette.rs:122 |
| Messages | barre d'état | liste | Historique des messages | messages.rs:209 |
| Régler le stylet à ma main | Préf. > Dessin, Aide | « n/3 · un trait … », Aujourd'hui, Appliquer, Rétablir, Recommencer | Rien ne change avant « Appliquer » ; souris = 100 % | reglagestylet.rs:234-335 |
| Ce que le stylet transmet | Préf. > Dessin, `aide.stylet_transmet` | Stylet, Appui %, Bouton du stylet, Inclinaisons, Rotation, À essayer, Ce que FEANOR reçoit aujourd'hui, Clics de souris vus par Windows, Enregistrer le relevé, Recommencer | Relevé écrit dans `%LOCALAPPDATA%\SHOT` | styletui.rs:70, 118-161 |
| Diagnostic des performances | Aide > Diagnostic, Préf. > Performance ; proposé au premier lancement (« Régler FEANOR pour cette machine ? » Lancer / Plus tard) | Ta machine, Lancer le diagnostic, Annuler, Mesures, Profil conseillé (Qualité / Équilibré / Performance), Réglages à poser (cases), Relancer, Copier le rapport, Enregistrer le rapport… (.txt), Revenir comme avant | Mesure ~15 s ; tout s'applique sans redémarrer | diagnosticui.rs:294-510, diagnostic.rs:454-464 |
| Mes habitudes | Aide | Compter mes commandes, Oui, remettre à zéro / Non, Les plus utilisées, Jamais utilisées | Compte gardé sur le poste | usageui.rs:137-239 |
| Noter une friction | Aide | « Qu'est-ce qui t'a freiné ? », Noter / Abandonner (Entrée / Échap) | Capture + dernières commandes dans un dossier local | frictionui.rs:74-227 |
| Visite « Premier jour » | Aide > Visite rapide, Préf. | 5 étapes | — | premierjour.rs |

### 2.4 Autres fenêtres de travail

| Libellé à l'écran | Où l'ouvrir | Contrôles et valeurs | Effet attendu | Source |
|---|---|---|---|---|
| Équipe — <film> / Partage / Reprendre ? | menu Équipe de la planche | état des équipiers, hors ligne, Copies en conflit, Brouillons, Nouveautés, Rendre ce plan, Prendre la séquence, Rendre / Prendre le montage | Travail à plusieurs par dossier partagé | cooperationui.rs:402-910 |
| Envois / Marquer un envoi | menu | Nom (« v2 au réal »), Date (JJ/MM/AAAA), liste des envois, Retirer | Repère pour « plans modifiés depuis le dernier envoi » | envoisui.rs:668-836 |
| Durée du plan | double clic sur la durée | « 36, 1:12, +12, -4 » | Rogne ou prolonge la fin | rognageui.rs:590-629 |
| Durée des plans choisis / Durée des clés d'images | sélection | — | — | dureeplans.rs:175, clesduree.rs:288 |
| Aller à un plan | commande | numéro | — | aller.rs:37 |
| Vue de dessus | commande | — | — | topview.rs:116 |
| Comparer (le plan) | Fiche > Comparer le plan… | côté, Image, Volet ; « Lecture seule » | — | comparerui.rs:292-364 |
| Supprimer les versions non affichées / Couper un plan à plusieurs versions / Supprimer la version de séquence / Renommer / Mettre à jour dans les autres versions de séquence | menus des versions | Annuler / Renommer / Appliquer | — | planversui.rs:434, coupeversions.rs:111, variantesui.rs:441-542 |
| Couleur de l'étiquette (version dépliée) | timeline, « V2 ▸ » | Aucune / couleurs ; Passer à la … | — | versiondeplieeui.rs:379-429 |
| Nouveau calque | timeline « + » | — | — | newlayer.rs:121 |
| Groupes de calques | menu | Montrer tous / Cacher tous / Seulement les couleurs utilisées | — | groupsui.rs:195-246 |
| Calques de même nom / Renommer intelligemment les instances | menus | — | — | fusionnoms.rs:365, images.rs:1057 |
| Réutiliser « <calque> » dans… | menu du calque (planche-mère) | plans cochés, Appliquer / Fermer | — | partageui.rs:155-174 |
| Colorier par zones / rapport | menu du calque | Trait, Clés peintes sur (les images où ce calque est peint / ces images : 1, 5-8, 12), Écrire les aplats dans, Images (tout le plan / de … à …), Fermer les trous jusqu'à px, zone sous le trait px, Zone incertaine (sans couleur / magenta), Confiance minimale | Une étape d'annulation | coloriageui.rs:452-761, docs/COLORIAGE.md |
| Calque de couleur : Griffonner, Pinceau dans la zone fermée, Fermer les trous jusqu'à px, Sources supplémentaires | réglages du calque de couleur | — | — | calquecouleurui.rs:330-432 |
| Aplat (fond) : Débord, Fermer les trous, Le bord de l'image ferme la forme, Aplat intelligent, Colorier comme la clé précédente, Cet aplat aussi sur (Les calques choisis / Tout le plan), Retirer l'aplat | Réglages de l'aplat | — | — | fondui.rs:88-205 |
| Propager la peinture | menu | Suivre (guide), De l'image à l'image, une image toutes les N / seulement aux clés du guide, Carte graphique / Processeur, Fidélité au guide, Suivi du mouvement, Anti-scintillement, Diversité, Douceur, Réglages par défaut | Calcul processeur ~20× plus lent | propagerui.rs:222-337 |
| Images intermédiaires / Nouvelle durée / Nouvelle cadence | menus d'animation | images, i/s, Interpoler les images ajoutées | — | tweenui.rs:274-307 |
| Préréglage d'effet / Incrustation | Effets > Préréglages | Préréglage, Où (Nouveau calque d'effet / Sur le calque courant), Intensité, Durée, Jusqu'à la fin du plan, Couleur, Graine, mode, Teinte, Éclat, Lignes de vitesse, Seuil, Pulsation, style, Fréquence, Flou de bougé, Direction, Dans tous les plans du film ; incrustation : Contenu, Texte ({timecode} {image} {plan} {sequence} {titre} {date}), Compté depuis, Place, Taille, Couleur, Boîte, Marge, Visible, Lettre à lettre | — | prereglagesui.rs:284-623 |
| Transitions : Sorte, images, Sens, Centre, Couleur, Supprimer / Modifier la transition… | clic sur une transition | — | — | transitionsui.rs:495-610 |
| Annotations | menu Annotations | Effacer / Garder | — | annotationsui.rs:951-1014 |
| Ce décor est lourd | import d'un décor 3D trop lourd | Alléger, Textures au plus 4096/2048/1024 px, Simplifier la géométrie 50 % / 25 % / jusqu'à N triangles, Garder tel quel, Le masquer sur cet ordinateur | — | decor3dui.rs:508-616 |

---

## Partie 3 — Imports et exports

### 3.1 Imports

| Libellé à l'écran | Où l'ouvrir | Formats et options | Effet attendu / dépendance | Source |
|---|---|---|---|---|
| Ouvrir | Fichier > Ouvrir, accueil, glisser-déposer | .shot, ancien .pelure | Nouvel onglet si réglé | app.rs:3955, project.rs:1104-1109, onglets.rs:701 |
| Importer un scénario (fenêtre « Lecture du scénario… ») | `fichier.importer_scenario` | .fountain, .spmd, .txt, .fdx, .docx (« Tous les fichiers » aussi) ; .doc refusé avec message | Découpage en plans : Un plan par scène / par paragraphe (défaut) / par réplique ; Noter le décor de la scène dans les notes (coché) ; Rythme : Répliques 2,5 mots/s, Action 1 s par ligne, Au plus court 1 s, Au plus long 30 s, Valeurs d'origine ; Où poser les plans ; Séquences, Personnages, Remarques ; Importer / Remplacer le scénario / Garder l'actuel / Autre fichier… | scenarioui.rs:113-524, scenario.rs:47-232, scenarioplans.rs:41-121 |
| Importer une image | menu du plan (panels.rs) / Plans types | png, jpg, jpeg | Image dans le plan | app.rs:4170-4188 |
| Importer une vidéo (calque vidéo) | `fichier.importer_video`, glisser | mp4, mov, avi, mkv, webm, m4v, mpg, mpeg, wmv | **ffmpeg obligatoire** (« ffmpeg introuvable : choisis-le… ») ; calque vidéo « n images à x i/s », Commence à l'image | videoui.rs:12-64, video.rs:262-314 |
| Importer une séquence d'images | `fichier.importer_sequence` | choisir une image : png, jpg, jpeg, **bmp, tga** proposés | La suite numérotée devient un calque vidéo | videoui.rs:14, 31-41, video.rs:339 |
| Importer un son | `son.importer`, glisser | wav, mp3, flac, ogg, oga | Extrait sur une piste | soundtrack.rs:246-269, project.rs:364 |
| Bruitages : ajouter un dossier | panneau Bruitages | dossiers de sons | Liste à chercher et glisser | bruitagesui.rs:265-287 |
| Importer des brosses | Réglages de l'outil > liste des pinceaux | .abr (Photoshop), .brush / .brushset (Procreate), .tvpx / .bin / .ini (TVPaint), .kpp / .bundle / .myb (Krita, MyPaint) | Fenêtre « Import de « … » » : « Ce qui n'a pas pu être repris tel quel » ; papiers TVPaint cherchés dans `ProgramFiles\TVPaint Developpement` ou `PELURE_TVPAINT` | brushui.rs:444-523, abr.rs:346, procreate.rs:236, kritabrush.rs:253-373, tvpbrush.rs:372-380 |
| Importer une pointe / un papier | réglages de la brosse | png, jpg, jpeg, bmp, tif, tiff, webp proposés | Image de pointe ou de papier | brushui.rs:321-419 |
| Importer un nuancier / Exporter le nuancier | panneau Couleur | .gpl, .aco | — | nuancierui.rs:155-188 |
| Ajouter un objet 3D au décor de la séquence… | `fichier.importer_decor3d`, panneau Décor | .glb, .gltf (glTF 2.0) | Mesure du budget 3D la première fois ; fenêtre « Ce décor est lourd » si besoin | decor3dui.rs:180, 842, decor3dbudget.rs |
| Références : Importer des images… / une vidéo… / Coller une image / Importer la planche d'un autre projet… | panneau Références | images png, jpg, jpeg ; vidéos (liste vidéo) ; presse-papiers ; .shot | Vidéo : ffmpeg pour les vignettes | referencesui.rs:473-489, planchrefui.rs:315, 811-823 |
| Bibliothèque : importer le fichier choisi dans le plan | clic droit dans Bibliothèque | image, vidéo, son | — | biblioui.rs:474-509 |
| Importer les notes de relecture | `fichier.importer_notes`, glisser | .json (rendu par la page HTML) | Fenêtre « Notes de relecture » : Les poser sur le plan courant, avec leur provenance ; Importer quand même ; doublons détectés | relectureui.rs:184-624 |
| Kitsu : lire les statuts et commentaires | `fichier.kitsu_retours` | — | Notes du fil de décision ; « Ne change rien dans Kitsu » | kitsuui.rs:1007-1021 |
| Nouveau projet depuis un modèle d'épisode | Nouveau projet > Modèle d'épisode | fichiers `.episode.json` du dossier `%APPDATA%\SHOT\Modèles` | Reprend cadre, cadence, papier, aplat, numérotation, série, épisode suivant, modèle de noms, auteur, copyright, calques, personnages, choix d'export, fiche, liste des plans, paquet | modeleepisode.rs:1-250, nouveauui.rs:173-204 |
| Coller depuis un autre projet | Ctrl+V d'un autre onglet | voir 1.2 | — | collage.rs:387-439 |
| (retiré) Import Storyboard Pro | — | `fichier.importer_sboard` n'existe plus : un raccourci perso qui la nomme est ignoré avec un avertissement | — | ancien_projet_tests.rs:21-34 |

### 3.2 Fenêtre « Exporter » et fenêtre commune des exports

| Libellé à l'écran | Où l'ouvrir | Valeurs possibles / défaut | Effet attendu | Source |
|---|---|---|---|---|
| Exporter (fenêtre des familles) | Fichier > Exporter… | Vidéo, Images & PDF, Son, Montage & échange ; Échap ferme | Chaque bouton ouvre la fenêtre de l'export | prefsui.rs:64-157 |
| Vignettes « Storyboard PDF / Film vidéo / Images une à une / Pour le montage », aperçu, gros bouton « Exporter… », réglages sous « Précis » | fenêtre d'export du PDF, du film, des images | — | Le son, les calques séparés et la caméra gardent la fenêtre classique | exportui.rs:160-180, panneauxvisuels.rs:453-465 |
| Format (film) | Film | MP4 (H.264) / MOV ProRes 422 HQ / MOV ProRes 4444 (fond transparent) / DNxHR HQ (MOV ou MXF) / GIF animé ; un format que ce ffmpeg ne sait pas faire est grisé « Grisé : … » | — | exportui.rs:190-207, 1036-1050, exportvideo.rs:272-280 |
| Étendue | tous | Montage (tout le film) / séquence courante / plan courant / Plans choisis, avec durée | — | exportui.rs:126-215, export.rs:100-108 |
| Taille (film, images) | Film, Images par plan | Taille du projet (défaut) / 4K UHD / 1080p / 720p / 50 % / Largeur libre … px | Toujours pair | exportui.rs:252-266, exportvideo.rs:42-66 |
| Cadence (film) | Film | Celle du projet (défaut), 23,976, 24, 25, 29,97, 30, 50, 60 | Conversion par ffmpeg | exportui.rs:280-286, exportvideo.rs:92-117 |
| Fichier : MOV / MXF | DNxHR | MOV (défaut) | — | exportui.rs:295-298 |
| Infos : Graver les informations dans l'image ; Numéro du plan (haut gauche), Nom de la séquence (haut droite), Time code (bas droite), Action et dialogue (bas) | Film | gravure éteinte ; plan, TC, séquence cochés ; textes non | — | exportui.rs:302-315, exportvideo.rs:309-324 |
| Fond : « Transparent : le papier du projet n'est pas dans le film. » | ProRes 4444 | — | — | exportui.rs:321-322 |
| Son : Avec le son du montage | Film | coché ; « Aucun son dans cette étendue » grisé | — | exportui.rs:325-329 |
| Taille / Cadence / Lecture (GIF) | GIF | 480 px de large (défaut) / 720 px / 50 % / Image finale ; 6, 8, 10, 12, 15 (défaut), 20, 24 i/s jamais au-dessus du projet ; Tourne en boucle (oui) ; Tramage (oui) | Sans son ; **non retenu d'une séance à l'autre** | exportui.rs:220-247, export.rs:113-160 |
| Page / Cases par page / Textes / Pages / Production / Commentaires (PDF) | Storyboard PDF | papier A4 (défaut) / A3 / Lettre (US) ; Paysage (défaut) / Portrait ; 1, 2, 3, 4, 6 (défaut), 8, 9 ; Numéro et titre, Durée, Action, Dialogue, Notes (cochés) ; Titre du film et date, Numéros de page (cochés) ; En-tête de studio (non), Marquer les plans modifiés depuis le dernier envoi (oui), Page finale « Notes de révision » (non) ; Avec les commentaires (non) | — | exportui.rs:335-384, 572-574, pdfmise.rs:28-90 |
| Images par plan : Images (Une par plan / Une par case / Une par clé), Clés (Une image par calque), Fichier PNG / JPEG + qualité, Sans le papier, Taille, Nomenclature | Images par plan, Images à la clé | Une par plan, PNG, qualité 92, opaque | « Un calque seul est toujours un PNG sur fond transparent » ; aperçu des noms | exportui.rs:492-565, exportstills.rs:49-63 |
| Nomenclature : formes prêtes / modèle perso, Chiffres séquence/plan/clé (0 = d'après le modèle), Casse (Telle quelle / MAJUSCULES / minuscules), Nom du calque au début | exports par plan | d'origine : modèle du projet | Erreur de nom = bouton Exporter fermé | exportui.rs:960-1004, nomenclature.rs:90 |
| Fond / Annotations / Transitions | images, film, PDF | Sans le papier (non), Avec les annotations (non, retenu sur le poste), Avec les transitions du montage (oui) | — | exportui.rs:579-598, settings.rs:94 |
| Caméra appliquée / Sans caméra (Calques séparés) | Calques séparés | — | « n plans, n calques, n images » | exportui.rs:391-406 |
| Avec la caméra de chaque image (déjà interpolée) / Aussi en CSV | Clés de caméra | JSON toujours | — | exportui.rs:418-421 |
| Fichiers WAV / AIFF, 16 / 24 bits, « 48 kHz, stéréo », Avec le mixage de toutes les pistes, Pistes | Son par piste | — | Muet et solo respectés | exportui.rs:427-448 |
| Fichiers WAV / AIFF, 16 / 24 bits, Plans « n plans avec du son » | Son par plan | — | « Les transitions ne changent rien au son » | exportui.rs:456-482 |
| ffmpeg : … / Choisir un autre ffmpeg… | toute fenêtre d'export vidéo/GIF | — | Même sélecteur `.exe` | exportui.rs:614-636 |
| Export en cours : progression, Arrêter, Finalisation…, Terminé en m:ss, Montrer dans l'explorateur, Fermer | après « Exporter… » | — | On peut continuer à travailler | exportui.rs:863-945 |
| Choix retenus | — | taille, cadence, MXF, gravure, PDF, images, son, nomenclature, calques, caméra (`exports_v1`) | Retenus sur le poste, pas le GIF ni le son ni les transitions | export.rs:286-322 |

### 3.3 Exports un par un

| Libellé à l'écran | Où l'ouvrir | Options et défauts | Effet / dépendance | Source |
|---|---|---|---|---|
| Film | Exp. > Vidéo > Film | voir 3.2 | **ffmpeg** (H.264 : libx264, sinon h264_mf (Windows), sinon OpenH264 ; ProRes : prores_ks ; DNxHR : dnxhd ≥ 256×120) | export.rs:353-408, exportvideo.rs:272-287 |
| GIF animé | Exp. > Vidéo | voir 3.2 | **ffmpeg** (palettegen / paletteuse) | export.rs:180-200 |
| Images | Exp. > Images & PDF | PNG une par image | Sans ffmpeg | prefsui.rs:75 |
| Image courante | id. | PNG | — | prefsui.rs:76 |
| Images par plan | id. | voir 3.2 | — | exportstills.rs |
| Images à la clé | id. | Une par clé, par calque possible (SQ01_PL012A_K01) | — | exportcles.rs |
| Storyboard PDF | id. | voir 3.2 | — | pdf.rs, pdfmise.rs |
| Planche des vignettes | id. | Plans : Tout le film / Ce que la planche montre / séquence courante / plans choisis ; Fichier image ou PDF ; Résolution 150 / 300 dpi / Largeur px ; Dessus : Numéros de plan, Notes, Légendes (nom et durée), Bloc de titre en haut des pages du PDF, Barres de coupe, Cadres de caméra, Avec les annotations, Passe-partout | « n vignettes, image de L × H » | plancheui.rs:256-383 |
| Color script | id. | Plans (film / séquence / choisis) ; Présentation (Planche / Bande continue / Bande de couleurs seule) ; Image d'un plan (première, milieu, moyenne, première clé ; une par plan / par case) ; Colonnes (selon la durée / égales) ; Couleur (dominante / moyenne) ; Repères (titres, numéro, minutage) ; Fichier (un PNG / un PNG par séquence / un PDF + papier) ; Couleurs (pas de fichier / CSV / JSON) ; Réglages : Taille px, Fond, Dessous, Bande des couleurs dominantes, Version du PDF | — | colorscriptui.rs:122-303 |
| Relecture (page HTML) | id. | Titre, Envoi (Aucun / envois), Plans, Lecture animée de chaque plan, Les commentaires déjà posés, Aussi un PDF à imprimer | Page sans internet ; retour en .json (3.1) | relectureui.rs:303-443 |
| Journal des différences | id. | De / À (État actuel), Formats PDF / Page HTML | « n modifiés, ajoutés, retirés, déplacés, notes traitées » ; il faut deux envois | journalui.rs:207-325 |
| Son mixé | Exp. > Son | WAV 48 kHz 16 bits stéréo | — | export.rs:46 |
| Son par piste / Son par plan | id. | voir 3.2 | — | exportstems.rs, exportsonplan.rs |
| Pour le montage (« Exporter pour le montage ») | Exp. > Montage & échange | Fichiers : EDL CMX 3600 (oui), XML de montage xmeml (oui), OpenTimelineIO (oui), FCPXML (non) + version 1.9 / 1.10 ; Dossier ; Titre ; Étendue film / séquence ; Time code de départ 01:00:00:00 ; NTSC (24, 30, 60 i/s) ; Vidéos ProRes 422 HQ / H.264, À moitié de la taille ; Son : Avec les pistes son ; Marqueurs (oui) ; Numérotation ; Chemins : Absolus dans le XML et l'OTIO | Dossier `<Titre>_montage/` + `media/`, `audio/`, `LISEZ-MOI.txt` ; « Transitions pas écrites telles quelles » ; **ffmpeg** | editorialui.rs:149-395, editorial.rs:132-167, docs/EXPORT_MONTAGE.md |
| Liste des plans | id. | Plans ; En-têtes (Kitsu, défaut / générique) ; Format CSV (défaut) / JSON ; Séparateur **virgule** (défaut) / point-virgule ; Image d'entrée 1001 ou autre ; Vignettes (non) | — | listeplansui.rs:74-171, listeplans.rs:48-96 |
| Kitsu | id. | voir 3.5 | — | kitsuui.rs |
| Paquet de livraison | id. | Une vidéo par plan (oui) en ProRes (non) ; vignettes PNG (oui) ; PDF (oui) ; animatique MP4 (oui) ; liste CSV (oui) / JSON (non) ; color script (non) ; journal des différences (non) ; « Le manifeste est toujours écrit » ; Plans film / séquence ; Dossier ; Rangement (modèles de chemin) ; Remplacer les fichiers qui existent déjà (non) ; Aperçu | **ffmpeg** pour vidéos et animatique ; manifeste avec empreintes | paquetui.rs:109-278, paquet.rs:52-95 |
| Rapport de numérotation | id. | CSV et page lisible | Depuis le dernier envoi | prefsui.rs:101 |
| Noms et production | id. | voir 1.11 | — | nommageui.rs |
| Fiche technique | id. | Plans ; Envoi ; Lignes (une par plan / une par image de storyboard) ; Time code de départ 01:00:00:00 ; Format XLSX (défaut) / CSV ; Séparateur **point-virgule** (défaut) / virgule ; Colonnes cochées et ordre (séquence, numéro, référence, version, nom, image, durées, TC, cadrage, mouvement, textes, transitions…) | — | ficheui.rs:258-460, fiche.rs:138-360, ficheformats.rs:41-75 |
| Calques séparés | id. | Caméra appliquée / sans caméra | Un dossier par plan | exportlayers.rs |
| Caméra | id. | JSON (+ CSV) | — | exportcamera.rs |
| Retakes (CSV) | Notes > exporter les retakes | .csv | — | fildecisionui.rs:207 |
| Rapport du diagnostic | Diagnostic > Enregistrer le rapport… | .txt | — | diagnosticui.rs:480 |

### 3.4 Modèles d'épisode

| Libellé à l'écran | Où l'ouvrir | Options | Effet attendu | Source |
|---|---|---|---|---|
| Enregistrer comme modèle d'épisode | Noms et production, `projet.enregistrer_modele_episode` | Nom (« Brawlidays, épisode type »), Modèles enregistrés (liste, supprimer) | Écrit `<nom>.episode.json` dans `%APPDATA%\SHOT\Modèles` ; « Pour créer un épisode : Fichier > Nouveau, puis Modèle d'épisode » | modeleepisodeui.rs:31-91, modeleepisode.rs:210-250 |

### 3.5 Kitsu

| Libellé à l'écran | Où l'ouvrir | Options | Effet / dépendance | Source |
|---|---|---|---|---|
| Activer le pont Kitsu | Exp. > Montage & échange > Kitsu | case, décochée | Rien ne part sans clic | kitsuui.rs:837, kitsu.rs:645 |
| Connexion : Serveur (« https://kitsu.monstudio.com »), E-mail, Mot de passe, Rester connecté | id. | Rester connecté : non | « Connecté à … » / « Pas connecté » ; HTTP par **curl** (`System32\curl.exe` sous Windows, sinon `curl` du PATH) ; jeton chiffré par DPAPI (Windows seulement) | kitsuui.rs:843-881, kitsu.rs:170-280, 830-895 |
| Production : Projet Kitsu, Tâche du storyboard, À la publication (statut), Correspondance des statuts (à faire, discussion, fait, refusé) | id. | « n projet(s), n type(s) de tâche, n statut(s) lus » | — | kitsuui.rs:887-930 |
| Publier : Tout le film / La séquence courante ; Une vignette (PNG) par plan changé ; Une vidéo MP4 par plan changé | id. | — | « Kitsu est déjà à jour » ; MP4 : **ffmpeg** | kitsuui.rs:931-992 |
| Retours de Kitsu / Statuts lus | id. | — | Voir 3.1 | kitsuui.rs:1007-1021 |

---

## Listes de fin

### (1) Réglages qui font doublon ou se contredisent

1. **Taille des icônes** : le défaut est 16 pt (`tailleicones.rs:29`), mais le texte de Préférences > Apparence > Tailles dit « 90 % et 13 points : les tailles d'origine » (`echelle.rs:88`) ; le commentaire de `settings.rs:345` dit aussi 13.
2. **« Valeurs d'origine »** (Performance) remet les défauts du code (`diagnosticui.rs:597`, `PerfReglages::default()`), alors que sa bulle dit « reviennent à ce qu'ils étaient avant le diagnostic des performances », ce qui est le rôle de « Revenir comme avant » (`diagnosticui.rs:593`).
3. **Trois ou quatre réglages pour replier les calques** : `replier_calques` (Préf. > Montage, `settings.rs:134`), `timeline_tout_replie`, qui n'est plus jamais lu et que le code se contente de remettre à faux (`settings.rs:127`, `pistes.rs:449`), `storyboard_calques_replies` (⊖ de la rangée, `settings.rs:162`) et « Déplier le calque courant tout seul » (`settings.rs:123`, espace Montage seulement).
4. **Durée du premier plan** : « Premier plan » dans Nouveau projet > Plus d'options (2 s, `nouveau.rs:360-377`) et « Durée d'un nouveau plan » dans Préf. > Montage (2 s, `settings.rs:366`) sont deux réglages séparés. Mettre des valeurs différentes et regarder ce que reçoit le premier plan, puis le deuxième.
5. **Deux « Passe-partout »** : celui de la caméra (Dessin > Caméra, couleur et opacité, `prefsui.rs:334-389`) et celui des cases (Vignettes > Passe-partout des cases, opacité à lui, `vignettesprefs.rs:251-263`). Leurs libellés se ressemblent.
6. **« Cases par rangée »** (colonnes de la planche, `prefsui.rs:219`) et « Zoom d'ouverture … cases » (cases visibles à l'ouverture, `vignettesprefs.rs:311-315`) : même mot pour deux choses différentes.
7. **Tremblement** : Dessin > « Animer à l'arrêt (tremblement par défaut) » (`prefsui.rs:436`) et Performance > « Tremblement du trait : couper « Animer à l'arrêt » » (`diagnosticui.rs:556`) peuvent se contredire. Les allumer tous les deux et voir lequel gagne.
8. **Pelure** : le nombre d'images avant/après de la fenêtre Pelure (jusqu'à 12, `onion.rs:105-134`) peut être plafonné en silence par Performance > Pelure d'oignon (`diagnostic.rs:186`).
9. **Séparateur CSV** : la fiche technique sort en point-virgule par défaut (`ficheformats.rs:43`), la liste des plans en virgule (`listeplans.rs:65`).
10. **Cadence** : le projet ne propose que des cadences entières (`projetui.rs:62`), mais l'export du film propose 23,976 et 29,97 (`exportvideo.rs:92-117`) et l'export pour le montage a sa case NTSC (`editorial.rs:143`). Trois façons de parler de la même chose.
11. **Thème** : la bulle cite 4 thèmes (« Gris sombre, Gris clair, Sombre ou Personnalisé ») alors que la liste en a 6, avec Atelier et Papier (`prefsui.rs:177`, `theme.rs:60`).
12. **Hauteurs des lignes** : `settings.rs:137-138` annonce comme origine (20, 26, 22, 40), mais la ligne Séquence vaut 10 (`montagerows.rs:79`, `hauteurstimeline.rs:25`).
13. **« Rouvrir le dernier projet au lancement »** et **« Ajuster les panneaux à leur contenu »** existent chacun à deux endroits (accueil et Préférences, `accueil.rs:117` ; Affichage > Panneaux et Préférences, `workspaceui.rs:229`). C'est le même réglage : vérifier que les deux cases suivent.
14. La commande `vue.panneau_fiche` s'appelle « Colonne des calques (montrer / cacher) » (`keymap.rs:729`), alors que le panneau Calques n'existe plus (partie 160).

### (2) Réglages lus nulle part ou sans effet visible

1. `afficher_explications` (« Aide > Afficher les explications ») est encore lu et écrit dans `reglages.toml` (`settings.rs:331-333`), mais aucun code ne le lit plus et sa case a été retirée (`aideregistre.rs:73`).
2. `barre_panneaux` / `panel_bar` (`settings.rs:222-226`) : il est seulement gardé pour réécrire le fichier, et rien ne le lit (zéro usage hors `settings.rs`).
3. `timeline_tout_replie` / `calques_replies` (`settings.rs:127`) : écrit à faux (`pistes.rs:449`), jamais lu.
4. `remplir_la_forme` (`settings.rs:194-198`) : lu, jamais écrit, sans effet (assumé dans le code).
5. `panneau_espace3d_pose` (`settings.rs:83`) : une marque que le code pose et qui ne déclenche plus rien (`settings.rs:699-700`).
6. `vignettes.tailles_decor` et `vignettes.couleur_decor` : remis à leur défaut à chaque lecture (`settings.rs:662-663`), donc sans effet après la migration.
7. Sur Mac, ces réglages sont sans effet par construction : « Bouton et gomme du stylet lus auprès de Windows » (`stylet.rs:544-547`), le « Gestionnaire d'identifiants » des clés (`clessecretes.rs:285-300`), « Rester connecté » de Kitsu (`kitsu.rs:884-887`), et le choix « Écran n » de la planche de références, dont la liste est vide hors Windows (`planchref.rs:567-601`).
8. Le GIF (taille, cadence, boucle, tramage), « Avec le son », « Sans le papier » et les transitions ne sont pas dans `ExportPrefs` (`export.rs:286-299`) : ils reviennent aux défauts à chaque séance. Ce n'est pas un bug si c'est voulu, mais le testeur ne doit pas s'attendre à les retrouver.

### (3) Ce qui risque de ne pas marcher sur Mac

1. **Aucun dossier de données** : `roaming()` et `local()` lisent `APPDATA` et `LOCALAPPDATA` (`donnees.rs:23-45`), qui n'existent pas sous macOS. Rien n'est écrit (`settings.rs:724`, message « dossier %APPDATA% introuvable ») : pas de réglages, de récents, de copies de secours, de fenêtre de récupération (`recovery.rs:77`), de raccourcis personnels (`keymap.rs:1358`), de modèles d'épisode (`modeleepisode.rs:210`, avec en plus le message trompeur « Donne un nom au modèle. »), de jeton Kitsu, de compteur d'usage, de rapports de plantage (`plantage.rs:60`), de brosses importées (`brushtex.rs:1064`), ni de relevé du stylet.
2. **ffmpeg introuvable et impossible à choisir** : la recherche ne cherche que `ffmpeg.exe` (PATH, dossier de l'exécutable, `ProgramFiles`, WinGet, `C:\ffmpeg\bin\ffmpeg.exe`, `export.rs:353-384`), et le sélecteur filtre sur l'extension `exe` (`exportui.rs:96-97`), alors que le `ffmpeg` de Homebrew n'a pas d'extension. Seul contournement : `PELURE_FFMPEG=/opt/homebrew/bin/ffmpeg` (`export.rs:355`). Sans lui, ces fonctions échouent : film, GIF, import vidéo et séquence d'images, vignettes vidéo de la Bibliothèque et des Références, export pour le montage, paquet, MP4 vers Kitsu.
3. **« Montrer dans l'explorateur »** lance `explorer.exe` (`exportui.rs:1082-1099`) : sans effet sur Mac, dans environ douze fenêtres (export, montage, fiche, paquet, relecture, journal, À propos, Bibliothèque, onglets…).
4. **« Modifier le fichier à la main… »** (raccourcis) lance `notepad.exe` (`app.rs:5677`).
5. **Pression et bouton du stylet** : la pression vient des événements `Touch` d'egui/winit (`canvas.rs:469`, `boardui.rs:1487`), que winit 0.30 ne produit probablement pas pour une tablette sur macOS. L'écoute du bouton et de la gomme est vide hors Windows (`stylet.rs:544`). À vérifier en premier avec « Ce que le stylet transmet » ; il faut s'attendre à un appui figé à 100 %.
6. **Génération sonore** : Python attendu dans `venv/Scripts/python.exe` (`serveurlocal.rs:65-67`, sur Mac `venv/bin/python`) ; le sélecteur filtre sur `.exe` (`genprefsui.rs:229`) ; les clés des services en ligne sont refusées (« n'existe que sous Windows », `clessecretes.rs:289`) ; le script `sidecar/lancer.ps1` est en PowerShell.
7. **Polices** : l'outil Texte cherche les polices seulement dans `WINDIR\Fonts` et `LOCALAPPDATA\Microsoft\Windows\Fonts` (`text.rs:59-68`), donc aucune police système sur Mac. Segoe UI est absente (`theme.rs:970-975`) : l'interface s'affiche dans la police d'egui.
8. **Heure locale** : le décalage horaire vaut zéro hors Windows (`remplacer.rs:69-99`), donc les jalons et les frictions affichent l'heure UTC.
9. **Formats d'image annoncés mais non décodés** : la caisse `image` est compilée avec png et jpeg seulement (`Cargo.toml`, ligne `image = … features = ["png","jpeg"]`). Pourtant, la séquence d'images propose bmp et tga (`videoui.rs:14`) et les pointes et papiers proposent bmp, tif, tiff et webp (`brushui.rs:327, 414`). Cela concerne aussi Windows.
10. **Mémoire et cœurs** : rien n'est détecté hors Windows (`materiel.rs:462-470`, `memoirevideo.rs:342`), donc le diagnostic et le budget 3D travaillent à l'aveugle. Même chose pour la priorité basse des exports (`parallele.rs:64-94`).
11. **Dépôt de fichiers** : sans `GetCursorPos`, la position vient du dernier pointeur egui (`deposer.rs:420-445`). Un fichier lâché peut tomber au mauvais endroit.
12. **Chemins écrits à la Windows dans l'interface** (affichage seul) : `Documents\SHOT\Identité\Accueil` (`accueilimages.rs:416`), `%LOCALAPPDATA%\SHOT\secours` (`recoveryui.rs:117`), `Documents\SHOT\Modèles\SHOT-modeles` (`genprefsui.rs:217`, `aidefenetres.rs:34`), et l'aperçu du paquet construit avec `\` (`paquetui.rs:40, 230-231`).
13. **Les dossiers proposés dans Nouveau projet** sont dédoublonnés en minuscules, avec `/` changé en `\` (`nouveau.rs:457`). Sur un disque Mac sensible à la casse, deux dossiers peuvent être confondus.
14. **Auteur des commentaires** : il est lu dans `USERNAME` (`commentaires.rs:188`), qui est vide sur Mac. La signature par défaut sera donc « Anonyme ».
15. Les textes de l'interface parlent de Windows (« Ce que Windows transmet », « l'utilisateur Windows », « Écran 1 : l'écran principal de Windows »), ce qui trompe sur Mac.
16. **Ce qui devrait marcher** : Kitsu passe par `curl` (présent dans `/usr/bin`) ; l'affichage passe par wgpu, avec Metal activé par eframe (`~/.cargo/…/eframe-0.31.1/Cargo.toml:255-260`) et OpenGL en secours (`PELURE_AFFICHAGE=opengl`, `lib.rs:1513`) ; le son passe par cpal (CoreAudio) ; le presse-papiers par arboard.
