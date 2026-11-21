# Grille d'audit : toutes les commandes de FEANOR

Dépôt lu : `/Users/anthony/CLAUDE/Feanor-main` (branche `main`), en lecture seule, sans compiler ni lancer l'application. Chaque ligne a été retrouvée dans le code actuel ; les documents n'ont servi qu'à repérer.

**À lire avant de tester sur Mac.**

- Partout où la grille écrit `Ctrl`, l'application attend **⌘ (Commande)** sur Mac : `Combo::matches` compare `Modifiers::command`, qui vaut ⌘ sur macOS (`src/keymap.rs:892`). La touche physique Contrôle (⌃) ne déclenche **aucun** raccourci `Ctrl+…`. Les menus et la palette affichent pourtant « Ctrl+ » (`src/keymap.rs:903-905`), jamais « ⌘ ».
- `Alt` = ⌥ Option. `Maj` = ⇧. `Suppr` = suppression avant (fn + ⌫ sur un clavier Mac), `Retour arrière` = ⌫. `Origine`/`Fin`/`PgPréc`/`PgSuiv` = fn + flèches. `Inser` n'existe pas sur un clavier Mac.
- Les lettres sont reconnues par le caractère tapé (AZERTY : la touche marquée Q est Q) ; les chiffres et la ponctuation par leur **position QWERTY** (`keymap.toml:17-21`).
- `contexte storyboard` = le montage a la main ou on est au Storyboard ; `contexte animation` = la timeline a la main ou on est dans un plan (`src/app.rs:40-50`) ; `contexte montage` = seulement dans l'onglet Montage, où ces touches passent devant les autres (`src/keymap.rs:1385-1392`).
- Clic droit sur presque toute entrée de menu ou icône : « Assigner un raccourci… » (`src/panels.rs:71-74`, `src/raccourcisui.rs:41`). Ce n'est pas répété à chaque ligne.
- Sans projet ouvert, seuls Fichier et Aide s'ouvrent ; les autres menus sont grisés (`src/panels.rs:39-44`, `src/sansprojet.rs:80`).
- La colonne « Effet attendu » reprend le nom que le code donne à l'action (celui que montre la palette Ctrl+K), suivi de son identifiant `[id]`.

## 1. Barre de menus

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| Nouveau… | Fichier | `Ctrl+N` | Nouveau projet `[fichier.nouveau]` | src/panels.rs:119 |
| Ouvrir… | Fichier | `Ctrl+O` | Ouvrir `[fichier.ouvrir]` | src/panels.rs:120 |
| Projets récents ▸ (un projet par ligne) | Fichier | — | Rouvre le projet ; grisé s'il est introuvable | src/recents.rs:103 |
| Projets récents ▸ Aucun projet récent. | Fichier | — | Texte grisé quand la liste est vide | src/recents.rs:98 |
| Projets récents ▸ Rouvrir le dernier projet au lancement | Fichier | — | Case à cocher (réglage retenu) | src/recents.rs:113 |
| Projets récents ▸ Effacer la liste | Fichier | — | Vide la liste des récents | src/recents.rs:117 |
| Enregistrer (ou « Poser un jalon » si l'écriture continue est active) | Fichier | `Ctrl+S` | Enregistrer, ou poser un jalon `[fichier.enregistrer]` | src/panels.rs:123-124 |
| Jalons… | Fichier | — | Jalons du projet `[fichier.jalons]` | src/panels.rs:125 |
| Enregistrer sous… | Fichier | `Ctrl+Maj+S` | Enregistrer sous `[fichier.enregistrer_sous]` | src/panels.rs:126 |
| Afficher le projet dans l'Explorateur | Fichier | — | Afficher le projet dans l'Explorateur Windows `[fichier.afficher_explorateur]` | src/panels.rs:127 |
| Nouvel onglet | Fichier | `Ctrl+T` | Nouvel onglet `[onglet.nouveau]` | src/panels.rs:128 |
| Fermer l'onglet | Fichier | `Ctrl+W` | Fermer le projet (l'onglet) `[fichier.fermer]` | src/panels.rs:129 |
| Fermer les autres onglets | Fichier | — | Fermer les autres onglets `[onglet.fermer_autres]` | src/panels.rs:130 |
| Une image (calque bitmap)… | Fichier ▸ Importer | — | Importe une image en calque bitmap (bouton sans action : pas de raccourci, pas de clic droit « Assigner ») | src/panels.rs:133-136 |
| Une vidéo (calque vidéo)… | Fichier ▸ Importer | — | Importer une vidéo (calque vidéo) `[fichier.importer_video]` | src/panels.rs:137 |
| Une séquence d'images (calque vidéo)… | Fichier ▸ Importer | — | Importer une séquence d'images (calque vidéo) `[fichier.importer_sequence]` | src/panels.rs:138 |
| Objet 3D pour le décor de la séquence (glTF, .glb)… | Fichier ▸ Importer | — | Ajouter un objet 3D au décor de la séquence (glTF, .glb ou .gltf) `[fichier.importer_decor3d]` | src/panels.rs:139 |
| Enlever le décor 3D de la séquence (seulement si la séquence a un décor) | Fichier ▸ Importer | — | Enlever le décor 3D de la séquence (tous ses objets) `[decor3d.enlever]` | src/panels.rs:140-142 |
| Des images comme références… | Fichier ▸ Importer | — | Importer des images dans les références `[references.importer_image]` | src/panels.rs:143 |
| Une vidéo comme référence… | Fichier ▸ Importer | — | Importer une vidéo dans les références `[references.importer_video]` | src/panels.rs:144 |
| Un scénario, à garder dans le projet (Fountain, .fdx, .docx, texte)… | Fichier ▸ Importer | — | Importer un scénario et le garder dans le projet (Fountain, .fdx, .docx, texte) `[fichier.importer_scenario]` | src/panels.rs:146 |
| Des notes de relecture (.json)… | Fichier ▸ Importer | — | Importer les notes de relecture (fichier .json) `[fichier.importer_notes]` | src/panels.rs:147 |
| Film (MP4, ProRes, DNxHR)… | Fichier ▸ Exporter en vidéo | — | Exporter le film (vidéo) `[fichier.exporter_film]` | src/exportmenu.rs:18-65 |
| GIF animé… | Fichier ▸ Exporter en vidéo | — | Exporter un GIF animé (le film, le plan courant ou les plans choisis) `[fichier.exporter_gif]` | src/exportmenu.rs:18-65 |
| Image courante… | Fichier ▸ Exporter en image | — | Exporter l'image courante (PNG) `[fichier.exporter_image_courante]` | src/exportmenu.rs:18-65 |
| Une image par image du film… | Fichier ▸ Exporter en image | — | Exporter des images (PNG) `[fichier.exporter_images]` | src/exportmenu.rs:18-65 |
| Une image par plan… | Fichier ▸ Exporter en image | — | Exporter des images par plan (une image par plan ou par case, en PNG ou en JPEG) `[fichier.exporter_images_par_plan]` | src/exportmenu.rs:18-65 |
| Une image par dessin clé… | Fichier ▸ Exporter en image | — | Exporter des images à la clé (une image par dessin clé, numérotée, composée ou par calque) `[fichier.exporter_images_cles]` | src/exportmenu.rs:18-65 |
| Storyboard en PDF… | Fichier ▸ Exporter en image | — | Exporter le storyboard (PDF) `[fichier.exporter_pdf]` | src/exportmenu.rs:18-65 |
| Planche des vignettes… | Fichier ▸ Exporter en image | — | Exporter la planche des vignettes (PNG, JPEG ou PDF) `[fichier.exporter_planche]` | src/exportmenu.rs:18-65 |
| Color script… | Fichier ▸ Exporter en image | — | Exporter le color script (une petite image par plan, rangées par séquence) `[fichier.exporter_colorscript]` | src/exportmenu.rs:18-65 |
| Son mixé… | Fichier ▸ Exporter le son | — | Exporter le son (WAV) `[fichier.exporter_son]` | src/exportmenu.rs:18-65 |
| Un son par piste… | Fichier ▸ Exporter le son | — | Exporter le son par piste (un fichier WAV ou AIFF par piste audible, avec ou sans le mixage) `[fichier.exporter_son_par_piste]` | src/exportmenu.rs:18-65 |
| Un son par plan… | Fichier ▸ Exporter le son | — | Exporter le son par plan (un fichier WAV ou AIFF par plan, le mixage de ce qu'on y entend) `[fichier.exporter_son_par_plan]` | src/exportmenu.rs:18-65 |
| Pour le montage (EDL, XML, FCPXML, OpenTimelineIO)… | Fichier ▸ Exporter pour la production | — | Exporter pour le montage (EDL, XML de montage, OpenTimelineIO) `[fichier.exporter_montage]` | src/exportmenu.rs:18-65 |
| Calques séparés… | Fichier ▸ Exporter pour la production | — | Exporter les calques séparés (un dossier par plan, une suite d'images transparentes par calque) `[fichier.exporter_calques]` | src/exportmenu.rs:18-65 |
| Caméra (JSON, CSV)… | Fichier ▸ Exporter pour la production | — | Exporter les clés de caméra (JSON et CSV, pour un script de compositing) `[fichier.exporter_camera]` | src/exportmenu.rs:18-65 |
| Fiche technique (XLSX, CSV)… | Fichier ▸ Exporter pour la production | — | Exporter la fiche technique (tableur CSV ou XLSX, un plan par ligne) `[fichier.exporter_fiche]` | src/exportmenu.rs:18-65 |
| Liste des plans (CSV, JSON)… | Fichier ▸ Exporter pour la production | — | Exporter la liste des plans pour le suivi de production (CSV ou JSON, profil Kitsu ou générique) `[fichier.exporter_liste_plans]` | src/exportmenu.rs:18-65 |
| Rapport de numérotation… | Fichier ▸ Exporter pour la production | — | Exporter le rapport de numérotation pour le monteur (plans ajoutés, supprimés, déplacés, renumérotés depuis le dernier envoi) `[fichier.exporter_renumerotation]` | src/exportmenu.rs:18-65 |
| Relecture (la page à envoyer au réalisateur)… | Fichier ▸ Exporter pour la production | — | Exporter pour relecture (page HTML à envoyer au réalisateur) `[fichier.exporter_relecture]` | src/exportmenu.rs:18-65 |
| Journal des différences… | Fichier ▸ Exporter pour la production | — | Journal des différences entre deux envois (plans modifiés avant / après, notes traitées ; PDF et page HTML) `[envoi.journal]` | src/exportmenu.rs:18-65 |
| Paquet de livraison… | Fichier ▸ Exporter pour la production | — | Exporter le paquet de livraison (vidéos par plan, vignettes, PDF, animatique, liste des plans et manifeste, rangés dans un dossier) `[fichier.exporter_paquet]` | src/exportmenu.rs:18-65 |
| Kitsu… | Fichier ▸ Exporter pour la production | — | Pont Kitsu : publier les plans dans Kitsu (créer, mettre à jour, prévisualiser) `[fichier.kitsu_publier]` | src/exportmenu.rs:18-65 |
| Noms des plans, série et épisode… | Fichier ▸ Exporter pour la production | — | Noms des plans et production (modèle de noms, série, épisode) `[projet.noms_production]` | src/exportmenu.rs:18-65 |
| Réglages du projet… | Fichier | — | Réglages du projet (propriétés, résolution, cadence) `[projet.proprietes]` | src/panels.rs:153 |
| Corbeille… | Fichier | — | Corbeille du projet (restaurer ce qui est sorti du film, vider) `[fichier.corbeille]` | src/panels.rs:154 |
| Partager ce projet… (projet non partagé) | Fichier | — | Partager ce projet (créer un film partagé dans un dossier partagé ou réseau) `[fichier.partager]` | src/panels.rs:159 |
| Équipe… (projet partagé) | Fichier | — | Équipe : qui est en ligne, ce que chacun tient (ouvrir / fermer) `[equipe.fenetre]` | src/panels.rs:157 |
| Préférences… | Fichier | — | Préférences de FEANOR (copie de secours, curseur, panneaux, raccourcis) `[fichier.preferences]` | src/panels.rs:161 |
| Quitter | Fichier | `Ctrl+Q` | Quitter `[fichier.quitter]` | src/panels.rs:163 |
| Annuler | Édition | `Ctrl+Z` | Annuler `[edition.annuler]` | src/panels.rs:166 |
| Rétablir | Édition | `Ctrl+Maj+Z` ou `Ctrl+Y` | Rétablir `[edition.retablir]` | src/panels.rs:167 |
| Historique… | Édition | — | Panneau Historique des annulations (ouvrir / fermer) `[vue.historique]` | src/panels.rs:168 |
| Couper | Édition | `Ctrl+X` | Couper `[edition.couper]` | src/panels.rs:170 |
| Copier | Édition | `Ctrl+C` | Copier `[edition.copier]` | src/panels.rs:171 |
| Coller | Édition | `Ctrl+V` | Coller `[edition.coller]` | src/panels.rs:172 |
| Coller à la tête de lecture (clés) | Édition | `Ctrl+Maj+V` | Coller à la tête de lecture `[edition.coller_a_la_tete]` | src/panels.rs:175 |
| Supprimer | Édition | `Suppr` ou `Retour arrière` | Supprimer la sélection (rien sans sélection) `[edition.supprimer]` | src/panels.rs:176 |
| Vider le dessin | Édition | — | Vider le dessin de l'image courante `[dessin.vider]` | src/panels.rs:177 |
| Tout sélectionner | Édition | `Ctrl+A` | Tout sélectionner `[edition.tout_selectionner]` | src/panels.rs:179 |
| Tout désélectionner | Édition | `Échap` | Tout désélectionner `[edition.tout_deselectionner]` | src/panels.rs:180 |
| Retourner la sélection ▸ De gauche à droite | Édition | `Alt+H` | Retourner la sélection horizontalement `[selection.miroir_h]` | src/panels.rs:182 |
| Retourner la sélection ▸ De haut en bas | Édition | `Alt+V` | Retourner la sélection verticalement `[selection.miroir_v]` | src/panels.rs:183 |
| Nouveau plan | Storyboard | `P` ou `Inser` (storyboard) | Nouveau plan `[panneau.nouveau]` | src/panels.rs:187 |
| Nouvelle séquence | Storyboard | — | Nouvelle séquence `[sequence.nouvelle]` | src/panels.rs:188 |
| Dupliquer la séquence | Storyboard | — | Dupliquer la séquence (une copie indépendante, juste après elle) `[sequence.dupliquer]` | src/panels.rs:189 |
| Aller à un plan… | Storyboard | `Ctrl+F` | Aller à un plan `[scene.aller_a]` | src/panels.rs:192 |
| Fiche du plan | Storyboard | `Alt+Entrée` | Fiche du plan : ouvrir les propriétés du plan courant `[panneau.proprietes]` | src/panels.rs:193 |
| Nouveau plan avant le plan courant | Storyboard ▸ Plans | — | Nouveau plan avant le plan courant `[plan.nouveau_avant]` | src/panels.rs:79 |
| Dupliquer le plan | Storyboard ▸ Plans | `Ctrl+Maj+D` | Dupliquer le plan `[panneau.dupliquer]` | src/panels.rs:80 |
| Réutiliser le plan (même dessin, autre cadre) | Storyboard ▸ Plans | — | Réutiliser le plan : un plan lié au sien, même dessin, à cadrer autrement (planche-mère) `[plan.reutiliser]` | src/panels.rs:82 |
| Détacher le plan de la planche-mère (si plan partagé) | Storyboard ▸ Plans | — | Détacher le plan de la planche-mère (ses calques gardent des copies) `[plan.detacher]` | src/panels.rs:83-85 |
| Copier les plans choisis | Storyboard ▸ Plans | — | Copier les plans choisis `[panneau.copier]` | src/panels.rs:87 |
| Coller les plans après le plan courant | Storyboard ▸ Plans | — | Coller les plans après le plan courant `[panneau.coller]` | src/panels.rs:88 |
| Supprimer les plans choisis | Storyboard ▸ Plans | `Ctrl+Suppr` (storyboard) | Supprimer les plans choisis `[panneau.supprimer]` | src/panels.rs:89 |
| Allonger le plan d'une image | Storyboard ▸ Plans | `+` | Allonger le plan d'une image `[panneau.ajouter_image]` | src/panels.rs:91 |
| Raccourcir le plan d'une image | Storyboard ▸ Plans | `-` | Raccourcir le plan d'une image `[panneau.retirer_image]` | src/panels.rs:92 |
| Rogner le plan ▸ Durée… | Storyboard ▸ Plans | — | Durée du plan : taper la durée exacte… `[montage.duree]` | src/rognageui.rs:556 |
| Rogner le plan ▸ Raccourcir le début jusqu'à la tête de lecture | Storyboard ▸ Plans | — | Raccourcir le début du plan jusqu'à la tête de lecture `[montage.debut_a_tete]` | src/rognageui.rs:557 |
| Rogner le plan ▸ Raccourcir la fin jusqu'à la tête de lecture | Storyboard ▸ Plans | — | Raccourcir la fin du plan jusqu'à la tête de lecture `[montage.fin_a_tete]` | src/rognageui.rs:558 |
| Rogner le plan ▸ Rétablir le contenu rogné (N i au début, N i à la fin) — si rogné | Storyboard ▸ Plans | — | Rétablir le contenu rogné du plan (début et fin) `[montage.rognage_retablir]` | src/rognageui.rs:561 |
| Rogner le plan ▸ Oublier le contenu rogné — si rogné | Storyboard ▸ Plans | — | Oublier le contenu rogné du plan `[montage.rognage_oublier]` | src/rognageui.rs:562 |
| Couper le plan à l'image courante | Storyboard ▸ Plans | `F6` | Couper le plan à l'image courante `[panneau.couper]` | src/panels.rs:97 |
| Réunir avec le plan suivant | Storyboard ▸ Plans | — | Réunir le plan et le suivant `[panneau.fusionner_suivant]` | src/panels.rs:98 |
| Réunir avec le plan précédent | Storyboard ▸ Plans | — | Réunir avec le plan précédent `[scene.joindre_precedente]` | src/panels.rs:99 |
| Fusionner les plans choisis | Storyboard ▸ Plans | — | Fusionner les plans choisis (qui se suivent) `[panneau.fusionner_choisis]` | src/panels.rs:100 |
| Nouvelle séquence avec les plans choisis | Storyboard ▸ Plans | — | Nouvelle séquence avec les plans choisis `[sequence.nouvelle_avec_plans]` | src/panels.rs:101 |
| Changer la durée des plans choisis… | Storyboard ▸ Plans | — | Changer la durée des plans choisis… `[montage.duree_plans]` | src/panels.rs:102 |
| Retourner les plans choisis | Storyboard ▸ Plans | — | Retourner les plans choisis de gauche à droite `[plan.retourner]` | src/panels.rs:103 |
| Nouveau projet à partir des plans choisis | Storyboard ▸ Plans | — | Nouveau projet à partir des plans choisis `[projet.nouveau_depuis_plans]` | src/panels.rs:105 |
| Modifier la durée des clés d'images… | Storyboard | — | Modifier la durée des clés d'images… `[cles.duree]` | src/panels.rs:195 |
| Marque d'image ici (ajouter / retirer) | Storyboard | — | Marque d'image à l'image courante (ajouter / retirer) `[marque.basculer]` | src/panels.rs:196 |
| Nouveau commentaire ici | Storyboard | — | Nouveau commentaire (à l'image courante) `[commentaire.ajouter]` | src/panels.rs:197 |
| Commentaires… | Storyboard | — | Panneau Commentaires (ouvrir / fermer) `[vue.commentaires]` | src/panels.rs:198 |
| Marquer un envoi… | Storyboard ▸ Envois | — | Marquer un envoi (noter l'état de tous les plans) `[envoi.marquer]` | src/panels.rs:200 |
| Envois et plans modifiés… | Storyboard ▸ Envois | — | Envois : liste et plans modifiés (ouvrir / fermer) `[envoi.liste]` | src/panels.rs:201 |
| Plan modifié suivant | Storyboard ▸ Envois | — | Aller au plan modifié suivant (depuis le dernier envoi) `[envoi.modifie_suivant]` | src/panels.rs:202 |
| Plan modifié précédent | Storyboard ▸ Envois | — | Aller au plan modifié précédent (depuis le dernier envoi) `[envoi.modifie_precedent]` | src/panels.rs:203 |
| Choisir les plans modifiés | Storyboard ▸ Envois | — | Choisir les plans modifiés depuis le dernier envoi `[envoi.modifies_choisir]` | src/panels.rs:204 |
| Journal des différences… | Storyboard ▸ Envois | — | Journal des différences entre deux envois (plans modifiés avant / après, notes traitées ; PDF et page HTML) `[envoi.journal]` | src/panels.rs:205 |
| Nouvelle version, en dupliquant la version en cours | Storyboard ▸ Versions du plan | — | Nouvelle version du plan, en dupliquant la version en cours `[version.nouvelle]` | src/panels.rs:208 |
| Nouvelle version, en partant d'un plan vide | Storyboard ▸ Versions du plan | — | Nouvelle version du plan, en partant d'un plan vide `[version.vide]` | src/panels.rs:209 |
| Version précédente | Storyboard ▸ Versions du plan | — | Passer à la version précédente du plan `[version.precedente]` | src/panels.rs:210 |
| Version suivante | Storyboard ▸ Versions du plan | — | Passer à la version suivante du plan `[version.suivante]` | src/panels.rs:211 |
| Comparer le plan… | Storyboard ▸ Versions du plan | — | Comparer le plan : versions ou envois (ouvrir / fermer) `[comparer.ouvrir]` | src/panels.rs:213 |
| Supprimer les versions non affichées… | Storyboard ▸ Versions du plan | — | Supprimer les versions non affichées (tous les plans) `[version.purger]` | src/planversui.rs:382 |
| Supprimer les versions non affichées du plan courant… | Storyboard ▸ Versions du plan | — | Supprimer les versions non affichées du plan courant `[version.purger_plan]` | src/planversui.rs:383 |
| Nouvelle version, en gardant les plans | Storyboard ▸ Versions de la séquence (titre « Versions de la séquence (B) » s'il y en a plusieurs) | — | Nouvelle version de la séquence, en gardant les plans `[variante.nouvelle]` | src/variantesui.rs:400 |
| Nouvelle version, en copiant les plans | Storyboard ▸ Versions de la séquence (titre « Versions de la séquence (B) » s'il y en a plusieurs) | — | Nouvelle version de la séquence, en copiant les plans `[variante.copie]` | src/variantesui.rs:401 |
| Nouvelle version, en partant d'un plan vide | Storyboard ▸ Versions de la séquence (titre « Versions de la séquence (B) » s'il y en a plusieurs) | — | Nouvelle version de la séquence, en partant d'un plan vide `[variante.vide]` | src/variantesui.rs:402 |
| Passer à la version de séquence X / « X (choisie) » grisée | Storyboard ▸ Versions de la séquence (titre « Versions de la séquence (B) » s'il y en a plusieurs) | — | Change la version affichée de la séquence | src/variantesui.rs:408-409 |
| Mettre à jour le plan dans les autres versions de séquence… | Storyboard ▸ Versions de la séquence (titre « Versions de la séquence (B) » s'il y en a plusieurs) | — | Mettre à jour le plan dans les autres versions de la séquence `[variante.mettre_a_jour]` | src/variantesui.rs:415 |
| Renommer la version de séquence… | Storyboard ▸ Versions de la séquence (titre « Versions de la séquence (B) » s'il y en a plusieurs) | — | Renommer la version de la séquence `[variante.renommer]` | src/variantesui.rs:416 |
| Comparer le plan à une autre version de séquence… | Storyboard ▸ Versions de la séquence (titre « Versions de la séquence (B) » s'il y en a plusieurs) | — | Comparer le plan à une autre version de la séquence `[variante.comparer]` | src/variantesui.rs:417 |
| Supprimer la version de séquence… | Storyboard ▸ Versions de la séquence (titre « Versions de la séquence (B) » s'il y en a plusieurs) | — | Supprimer la version de la séquence `[variante.supprimer]` | src/variantesui.rs:418 |
| Verrouiller le storyboard / Déverrouiller le storyboard | Storyboard | — | Verrouiller / déverrouiller le storyboard `[storyboard.verrouiller]` | src/panels.rs:219-220 |
| Verrouiller / Déverrouiller la numérotation des plans | Storyboard | — | Verrouiller / déverrouiller la numérotation des plans `[numerotation.verrouiller]` | src/panels.rs:221-222 |
| Renuméroter les plans… | Storyboard | — | Renuméroter les plans (1, 2, 3… dans l'ordre du film) `[numerotation.renumeroter]` | src/panels.rs:223 |
| Animer ce plan (ou « Retour au storyboard » dans un plan) | Storyboard | `Tab` | Entrer dans le plan pour l'animer, et en revenir `[mode.basculer]` | src/panels.rs:225-229 |
| Importer un son… | Son | — | Importer un son `[son.importer]` | src/storyboard.rs:682 |
| Bruitages… | Son | — | Panneau Bruitages (ouvrir / fermer) `[vue.bruitages]` | src/storyboard.rs:683 |
| Nouvelle piste son | Son | — | Nouvelle piste son `[son.nouvelle_piste]` | src/storyboard.rs:684 |
| Couper le son à l'image courante | Son | — | Couper le son à l'image courante `[son.couper]` | src/storyboard.rs:685 |
| Supprimer l'extrait sélectionné (si un extrait est choisi) | Son | — | Supprime l'extrait de son choisi (bouton sans action) | src/storyboard.rs:686-687 |
| Son pendant la lecture (case) | Son | — | Son pendant la lecture (activer / couper) `[son.lecture]` | src/storyboard.rs:694-696 |
| Son au défilement (case) | Son | — | Son au défilement (activer / couper) `[son.defilement]` | src/storyboard.rs:699-701 |
| Calques (menu entier) | Calques | — | Ouvre le menu du calque courant, identique au clic droit sur un calque : voir §7.1 | src/panels.rs:232, src/menucalque.rs:50 |
| Lecture / pause | Animation | `Entrée` | Lecture / pause `[lecture.basculer]` | src/panels.rs:234 |
| Aller à ▸ Image précédente | Animation | `,` ou `Flèche gauche` | Image précédente `[image.precedente]` | src/panels.rs:236 |
| Aller à ▸ Image suivante | Animation | `.` ou `Flèche droite` | Image suivante `[image.suivante]` | src/panels.rs:237 |
| Aller à ▸ Première image du plan | Animation | `Origine` | Première image du plan `[image.premiere]` | src/panels.rs:238 |
| Aller à ▸ Dernière image du plan | Animation | `Fin` | Dernière image du plan `[image.derniere]` | src/panels.rs:239 |
| Nouveau dessin | Animation | `Inser` (animation) | Nouveau dessin après l'exposition `[dessin.nouveau]` | src/panels.rs:242 |
| Dupliquer le dessin | Animation | `Ctrl+D` | Dupliquer le dessin `[dessin.dupliquer]` | src/panels.rs:243 |
| Allonger l'exposition | Animation | `Alt++` | Tenir le dessin une image de plus `[exposition.allonger]` | src/panels.rs:244 |
| Raccourcir l'exposition | Animation | `Alt+-` | Tenir le dessin une image de moins `[exposition.raccourcir]` | src/panels.rs:245 |
| Retirer le dessin de la timeline | Animation | `Ctrl+Suppr` (animation) | Retirer le dessin de la timeline `[dessin.supprimer]` | src/panels.rs:246 |
| Images ▸ (menu Images, voir §3.4) | Animation | — | Même contenu que le menu « Images » de la timeline | src/panels.rs:248, src/images.rs:1126 |
| Colorier par zones… | Animation | — | Colorier par zones (reporter les aplats des clés sur les autres images) `[calque.colorier_zones]` | src/panels.rs:250 |
| Propager la peinture du calque… | Animation | — | Propager la peinture du calque en suivant une vidéo ou une animation (de clé en clé) `[calque.propager]` | src/panels.rs:251 |
| Ajuster à la fenêtre | Affichage | `Ctrl+0` | Ajuster à la fenêtre `[vue.ajuster]` | src/panels.rs:255 |
| Zoom 100 % | Affichage | `Maj+Z` | Zoom 100 % `[vue.reinit_zoom]` | src/panels.rs:256 |
| Remettre la vue droite | Affichage | `Maj+X` | Remettre la vue droite `[vue.reinit_rotation]` | src/panels.rs:257 |
| Caméra ▸ Vue à travers la caméra | Affichage | — | Vue à travers la caméra `[camera.vue]` | src/panels.rs:260 |
| Caméra ▸ Passe-partout | Affichage | — | Passe-partout autour du cadre de la caméra `[camera.masque]` | src/panels.rs:261 |
| Caméra ▸ Zones de sécurité | Affichage | — | Zones de sécurité (action, titres) `[vue.cadres_securite]` | src/panels.rs:262 |
| Caméra ▸ Réinitialiser la caméra du plan | Affichage | — | Réinitialiser la caméra `[camera.reinitialiser]` | src/panels.rs:264 |
| Plateau ▸ Espace 3D | Affichage | — | Espace 3D (afficher / quitter) `[espace3d.basculer]` | src/panels.rs:267 |
| Plateau ▸ Vue de dessus (profondeur des calques) | Affichage | — | Vue de dessus (profondeur des calques) `[vue.dessus]` | src/panels.rs:268 |
| Guides ▸ Afficher et utiliser | Affichage | `G` | Guide de perspective : afficher et utiliser `[guide.activer]` | src/panels.rs:271 |
| Guides ▸ Aligner les traits | Affichage | `Maj+A` | Guide : aligner les traits `[guide.aligner]` | src/panels.rs:272 |
| Guides ▸ Verrouiller | Affichage | `Maj+L` | Guide : verrouiller `[guide.verrouiller]` | src/panels.rs:273 |
| Guides ▸ Lignes de référence | Affichage | `Maj+G` | Guide : lignes de référence `[guide.lignes_reference]` | src/panels.rs:274 |
| Montrer / Cacher les pastilles des brosses dédiées | Affichage | — | Pastilles des brosses dédiées dans la barre du haut (montrer / cacher, dans cet espace de travail) `[perso.barre]` | src/panels.rs:277-278 |
| Espaces de travail ▸ Vignettes / Storyboard / 3D / Montage (« (en cours) » pour l'actuel) | Affichage | `Alt+Maj+V` | Va à cet espace ; une ligne par espace : espace.vignettes, espace.storyboard, espace.3d, espace.montage `[espace.vignettes]` | src/workspaceui.rs:155-158 |
| Espaces de travail ▸ Rétablir la disposition | Affichage | — | Rétablir la disposition de l'espace de travail en cours `[espace.retablir]` | src/workspaceui.rs:159 |
| Espaces de travail ▸ Standard | Affichage | — | Espace de travail : Standard `[espace.standard]` | src/workspaceui.rs:161-163, src/workspace.rs:370 |
| Espaces de travail ▸ Dessin | Affichage | — | Espace de travail : Dessin `[espace.dessin]` | src/workspace.rs:372 |
| Espaces de travail ▸ Timeline | Affichage | — | Espace de travail : Timeline `[espace.timeline]` | src/workspace.rs:382 |
| Espaces de travail ▸ (dispositions enregistrées, une par nom) | Affichage | — | Applique la disposition enregistrée | src/workspaceui.rs:165-176 |
| Espaces de travail ▸ Enregistrer la disposition… | Affichage | — | Enregistrer la disposition comme espace de travail `[espace.enregistrer]` | src/workspaceui.rs:179 |
| Espaces de travail ▸ Supprimer un espace ▸ (nom) | Affichage | — | Supprime la disposition enregistrée, sans confirmation | src/workspaceui.rs:181-189 |
| Panneaux ▸ … (cases des panneaux : hors de ma part, voir l'autre agent) | Affichage | — | Montre / cache barres et panneaux ; « Réajuster les panneaux » = vue.reajuster_panneaux | src/workspaceui.rs:193-245 |
| Mode concentration (le dessin seul) | Affichage | `F11` | Mode concentration : ne garder que le dessin (la même touche remet tout) `[vue.concentration]` | src/panels.rs:280 |
| Plein écran | Affichage | `Ctrl+Maj+Alt+F` | Plein écran `[vue.plein_ecran]` | src/panels.rs:281 |
| Suggestions sonores… | Labo (visible seulement si Préférences > Général > « Afficher les fonctions expérimentales ») | — | Panneau Suggestions sonores (fonction expérimentale ; ouvrir / fermer) `[vue.suggestions_sonores]` | src/panels.rs:290 |
| Lire le storyboard (proposer des sons) | Labo (visible seulement si Préférences > Général > « Afficher les fonctions expérimentales ») | — | Lire le storyboard pour proposer des sons (fonction expérimentale) `[suggestion.lire]` | src/panels.rs:291 |
| Mon panneau (essai) | Labo (visible seulement si Préférences > Général > « Afficher les fonctions expérimentales ») | — | Mon panneau (fonction expérimentale ; ouvrir / fermer) `[vue.mon_panneau]` | src/panels.rs:294 |
| Enregistrer mes traits (banc d'essai du moteur) / Arrêter d'enregistrer mes traits | Labo (visible seulement si Préférences > Général > « Afficher les fonctions expérimentales ») | — | Enregistrer mes traits (fonction expérimentale : banc d'essai du moteur de dessin) `[pinceau.enregistrer_traits]` | src/panels.rs:296-297 |
| Performance (temps de l'interface) | Labo (visible seulement si Préférences > Général > « Afficher les fonctions expérimentales ») | — | Performance : superposition des temps (images, panneaux, arrière-plan) `[vue.performance]` | src/panels.rs:299 |
| Palette de commandes… | Aide | `Ctrl+K` | Palette de commandes : chercher une commande par son nom `[aide.palette]` | src/panels.rs:302 |
| Visite rapide « Premier jour »… | Aide | — | Visite rapide « Premier jour » : créer un projet, dessiner, mettre une durée, écouter, exporter `[aide.visite]` | src/panels.rs:305 |
| Revoir les conseils | Aide | — | Revoir les conseils `[aide.conseils]` | src/panels.rs:306 |
| Registre des explications… | Aide | — | Registre des explications : toutes les animations des outils et les textes des fenêtres `[aide.registre]` | src/aideanim.rs:609 |
| Régler FEANOR pour cette machine… | Aide | — | Régler FEANOR pour cette machine (Diagnostic des performances : détecter le matériel, mesurer, proposer des réglages) `[aide.diagnostic]` | src/panels.rs:309 |
| Noter une friction | Aide | `F9` | Noter une friction : ce qui vient de me freiner, avec une capture de la fenêtre et mes dernières commandes `[aide.noter_friction]` | src/panels.rs:312, src/frictionui.rs:25 |
| Mes frictions notées… | Aide | — | Mes frictions notées : ouvrir leur dossier `[aide.ouvrir_frictions]` | src/panels.rs:313, src/frictionui.rs:26 |
| Mes habitudes… | Aide | — | Mes habitudes : les commandes que j'utilise le plus et celles qui ne servent jamais (compteur du poste) `[aide.habitudes]` | src/panels.rs:314 |
| À propos de FEANOR… | Aide | — | À propos de FEANOR (version, logo) `[aide.a_propos]` | src/panels.rs:316 |

## 2. Onglets de projets, espaces de travail et modes

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| Vignettes | Barre du haut, à droite (sans projet : absents) | `Alt+Maj+V` | Passe à l'espace Vignettes (la planche seule) `[espace.vignettes]` | src/panels.rs:355-365 |
| Storyboard | Barre du haut, à droite (sans projet : absents) | — | Passe à l'espace Storyboard (bulle liée à mode.basculer) `[espace.storyboard]` | src/panels.rs:346-354 |
| 3D | Barre du haut, à droite (sans projet : absents) | — | Passe à l'onglet 3D de la séquence (décor, caméras, cartes) `[espace.3d]` | src/panels.rs:336-345 |
| Montage | Barre du haut, à droite (sans projet : absents) | `Alt+M` | Passe à l'espace Montage `[espace.montage]` | src/panels.rs:327-335 |
| Retour au storyboard / Retour aux Vignettes (seulement dans un plan) | Barre du haut, à droite (sans projet : absents) | `Tab` | Quitte l'animation vers l'espace d'où l'on vient `[mode.basculer]` | src/panels.rs:368-393 |
| Entrer dans le plan / en revenir | Clavier (pas d'onglet) | `Tab` | Entrer dans le plan pour l'animer, et en revenir `[mode.basculer]` | src/keymap.rs:138 |
| Vignettes ⇄ Storyboard | Clavier (pas d'onglet) | `Maj+Tab` | Vignettes et Storyboard : passer de l'un à l'autre (la planche, le dessin du plan) `[vue.planches]` | src/app.rs:5032-5039 |
| Double-clic sur un plan (montage, planche) | Clavier (pas d'onglet) | — | Entre dans le plan pour l'animer (espace Animation, sans onglet) | src/panels.rs:18 |
| ＋ (Nouvel onglet) | Barre des onglets de projets (sous les menus) | `Ctrl+T` | Nouvel onglet `[onglet.nouveau]` | src/ongletsui.rs:180 |
| Clic sur un onglet | Barre des onglets de projets (sous les menus) | — | Affiche ce projet | src/ongletsui.rs:55 |
| Clavier : onglet suivant | Barre des onglets de projets (sous les menus) | `Ctrl+Tab` | Onglet suivant `[onglet.suivant]` | src/keymap.rs:66 |
| Clavier : onglet précédent | Barre des onglets de projets (sous les menus) | `Ctrl+Maj+Tab` | Onglet précédent `[onglet.precedent]` | src/keymap.rs:67 |
| Clic droit sur un onglet ▸ Fermer | Barre des onglets de projets (sous les menus) | — | Ferme cet onglet (demande d'enregistrer s'il est modifié) | src/ongletsui.rs:166 |
| Clic droit sur un onglet ▸ Fermer les autres | Barre des onglets de projets (sous les menus) | — | Ferme les autres onglets (grisé s'il n'y en a qu'un) | src/ongletsui.rs:166 |
| Clic droit sur un onglet ▸ Afficher dans l'Explorateur | Barre des onglets de projets (sous les menus) | — | Ouvre le dossier du projet (grisé sans chemin) — appelle explorer.exe | src/ongletsui.rs:166, src/exportui.rs:1082-1099 |

## 3. Barres d'icônes et leurs « Plus… »

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| *Commun à toutes les barres* | bouton « Plus… » au bout de chaque barre | — | Liste les commandes absentes de la barre, chacune avec son épingle « Épingler dans la barre » / « Retirer de la barre » ; sous-menu « Dans la barre » ; « Rétablir la barre d'origine » | src/barres.rs:855-972 |

### 3.1 Barre de commandes (sous les menus ; Affichage ▸ Panneaux ▸ « Barre de commandes » la montre ou la cache)

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| Nouveau projet | Barre de commandes — visible d'office | `Ctrl+N` | Commencer un projet vide. `[fichier.nouveau]` | src/barres.rs:154 |
| Ouvrir | Barre de commandes — visible d'office | `Ctrl+O` | Ouvrir un projet FEANOR. `[fichier.ouvrir]` | src/barres.rs:155 |
| Enregistrer | Barre de commandes — visible d'office | `Ctrl+S` | Enregistrer le projet. Quand FEANOR l'écrit en continu : poser un jalon, un moment auquel revenir. `[fichier.enregistrer]` | src/barres.rs:156 |
| Exporter le film | Barre de commandes — visible d'office | — | Exporter le montage. `[fichier.exporter_film]` | src/barres.rs:157 |
| Annuler | Barre de commandes — visible d'office | `Ctrl+Z` | Défaire la dernière opération. `[edition.annuler]` | src/barres.rs:158 |
| Rétablir | Barre de commandes — visible d'office | `Ctrl+Maj+Z` ou `Ctrl+Y` | Refaire ce qui vient d'être annulé. `[edition.retablir]` | src/barres.rs:159 |
| Couper | Barre de commandes — visible d'office | `Ctrl+X` | Couper la sélection. `[edition.couper]` | src/barres.rs:160 |
| Copier | Barre de commandes — visible d'office | `Ctrl+C` | Copier la sélection. `[edition.copier]` | src/barres.rs:161 |
| Coller | Barre de commandes — visible d'office | `Ctrl+V` | Coller ce qui a été copié. `[edition.coller]` | src/barres.rs:162 |

### 3.2 Barre du montage (en tête de la bande, espaces Storyboard et Vignettes)

Particularité : dans cette barre, « Copier / Coller / Supprimer les plans » visent les plans choisis et « Lire le montage » lit tout le film, quel que soit le panneau qui a la main (`src/barres.rs:1073-1093`).

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| Début du montage | Barre du montage — visible d'office | `Ctrl+Origine` (storyboard) | Revenir à la première image du film. `[montage.debut]` | src/barres.rs:169 |
| Lire le montage / pause | Barre du montage — visible d'office | `Entrée` | Tout le film à la suite (animatique), avec le son. `[lecture.basculer]` | src/barres.rs:170 |
| Lecture en boucle | Barre du montage — visible d'office | — | Reprendre au début une fois à la fin. (interrupteur allumé / éteint) `[lecture.boucle]` | src/barres.rs:171 |
| Son pendant la lecture | Barre du montage — sous « Plus… » | — | Entendre les pistes son en lisant. (interrupteur allumé / éteint) `[son.lecture]` | src/barres.rs:172 |
| (Affichage du temps du montage (pas une commande)) | Barre du montage | — | Affichage fixe, ne passe jamais sous « Plus… » | src/barres.rs:173 |
| Nouveau plan | Barre du montage — visible d'office | `P` ou `Inser` (storyboard) | Après le plan courant, dans la même séquence. `[panneau.nouveau]` | src/barres.rs:174 |
| Nouvelle séquence | Barre du montage — visible d'office | — | Après la séquence courante, avec un plan. `[sequence.nouvelle]` | src/barres.rs:175 |
| Nouveau calque vectoriel | Barre du montage — sous « Plus… » | `Maj+S` | Dans le plan courant, au-dessus du calque courant : une piste de plus. `[calque.nouveau_vectoriel]` | src/barres.rs:176 |
| Copier les plans | Barre du montage — sous « Plus… » | `Ctrl+C` | Le plan courant ; Ctrl ou Maj + clic pour en choisir plusieurs. `[edition.copier]` | src/barres.rs:177 |
| Coller les plans | Barre du montage — sous « Plus… » | `Ctrl+V` | Après le plan courant, dans sa séquence. `[edition.coller]` | src/barres.rs:178 |
| Supprimer les plans | Barre du montage — sous « Plus… » | `Suppr` ou `Retour arrière` | Le plan courant ; Ctrl ou Maj + clic pour en choisir plusieurs. `[edition.supprimer]` | src/barres.rs:179 |
| Plans ▾ | Barre du montage — visible d'office | — | Menu : Nouveau plan avant, Dupliquer, Copier / Coller / Supprimer les plans, Couper, Réunir, Fusionner, Nouvelle séquence avec, Changer la durée, Retourner, Marque d'image (src/storyboard.rs:595-620) | src/barres.rs:180 |
| Avancer le plan | Barre du montage — sous « Plus… » | — | Le placer plus tôt dans le montage. `[plan.avancer]` | src/barres.rs:181 |
| Reculer le plan | Barre du montage — sous « Plus… » | — | Le placer plus tard dans le montage. `[plan.reculer]` | src/barres.rs:182 |
| Verrouiller le storyboard | Barre du montage — sous « Plus… » | — | Verrouillé : l'ordre, le nombre et la durée des plans, leurs dessins et leurs calques ne changent plus. (interrupteur allumé / éteint) `[storyboard.verrouiller]` | src/barres.rs:183 |
| Verrouiller la numérotation | Barre du montage — sous « Plus… » | — | Verrouillée : chaque plan garde son numéro. Inséré entre le 12 et le 13, un plan devient le 12A ; supprimé, il laisse un trou ; déplacé, il garde son numéro. (interrupteur allumé / éteint) `[numerotation.verrouiller]` | src/barres.rs:192 |
| Renuméroter les plans | Barre du montage — sous « Plus… » | — | Remet des numéros propres, 1, 2, 3… dans l'ordre du film (numérotation verrouillée ; après confirmation, une étape d'annulation). `[numerotation.renumeroter]` | src/barres.rs:200 |
| Aimantation des bords | Barre du montage — sous « Plus… » | — | Rogner un plan à la souris : le bord se colle à la tête de lecture, aux autres coupes et aux clés de caméra et d'effets. Éteint, il suit la souris à l'image près. (interrupteur allumé / éteint) `[montage.aimanter]` | src/barres.rs:208 |
| Replier toutes les pistes | Barre du montage — sous « Plus… » | — | Tous les calques en bandes fines, même le calque actif : le nom, les dessins et les clés restent visibles. Double-clic sur un calque : le déplier seul. `[piste.replier_tout]` | src/barres.rs:218 |
| Déplier toutes les pistes | Barre du montage — sous « Plus… » | — | Toutes les pistes de calques à leur pleine hauteur (les dossiers s'ouvrent). Un clic sur un autre calque les replie de nouveau. `[piste.deplier_tout]` | src/barres.rs:226 |
| Déplier le calque courant | Barre du montage — sous « Plus… » | — | Allumé, seul le calque actif est déplié, dès que la souris est relâchée. Éteint, tous les calques restent en bandes fines tant qu'on ne les déplie pas. (interrupteur allumé / éteint) `[piste.auto_deplier]` | src/barres.rs:227 |
| Ajouter une transition | Barre du montage — sous « Plus… » | — | À la coupe la plus proche de la tête de lecture : un fondu enchaîné entre deux plans, un fondu au noir au bord du film. Clic droit sur une coupe pour choisir la sorte. `[transition.ajouter]` | src/barres.rs:237 |
| Marquer un envoi… | Barre du montage — sous « Plus… » | — | Noter l'état de tous les plans (par exemple « v2 au réal ») : ceux qui changent ensuite sont repérés dans la bande et sur la planche. `[envoi.marquer]` | src/barres.rs:247 |
| Envois | Barre du montage — sous « Plus… » | — | La liste des envois et des plans modifiés depuis le dernier. (interrupteur allumé / éteint) `[envoi.liste]` | src/barres.rs:255 |
| Journal des différences | Barre du montage — sous « Plus… » | — | Ce qui a changé entre deux envois : les plans modifiés avant / après, ajoutés, retirés, déplacés, et les notes traitées. En PDF et en page HTML. `[envoi.journal]` | src/barres.rs:256 |
| Plan modifié précédent | Barre du montage — sous « Plus… » | — | Aller au plan modifié avant le plan courant, depuis le dernier envoi. `[envoi.modifie_precedent]` | src/barres.rs:257 |
| Plan modifié suivant | Barre du montage — sous « Plus… » | — | Aller au plan modifié après le plan courant, depuis le dernier envoi. `[envoi.modifie_suivant]` | src/barres.rs:258 |
| Nouvelle version du plan | Barre du montage — sous « Plus… » | — | Garde le plan tel qu'il est comme version plus ancienne (V1) et continue sur une copie (V2). Les versions se changent en survolant « V2 » sur le plan, dans le montage. `[version.nouvelle]` | src/barres.rs:260 |
| Comparer le plan | Barre du montage — sous « Plus… » | — | Deux versions du plan, ou le plan d'aujourd'hui et son état à un envoi : côte à côte, en volet, en fondu ou en superposition. Lecture seule. (interrupteur allumé / éteint) `[comparer.ouvrir]` | src/barres.rs:261 |
| Exporter pour le montage | Barre du montage — sous « Plus… » | — | Un fichier vidéo par plan et le montage en EDL, XML de montage ou OpenTimelineIO. `[fichier.exporter_montage]` | src/barres.rs:262 |
| Exporter la fiche technique | Barre du montage — sous « Plus… » | — | Le découpage du film en tableau (CSV ou classeur XLSX) : un plan par ligne, avec durées, time codes, cadrage, mouvement de caméra, textes, sons et vignettes. `[fichier.exporter_fiche]` | src/barres.rs:263 |
| Exporter pour relecture | Barre du montage — sous « Plus… » | — | Une page sans internet à envoyer au réalisateur : il y écrit ses notes et dessine dessus, puis vous renvoie un petit fichier. `[fichier.exporter_relecture]` | src/barres.rs:265 |
| Importer les notes de relecture | Barre du montage — sous « Plus… » | — | Le fichier de notes renvoyé par le réalisateur devient des commentaires sur les bons plans. On peut aussi le lâcher sur la fenêtre. `[fichier.importer_notes]` | src/barres.rs:266 |
| Importer un son | Barre du montage — visible d'office | — | WAV, MP3, FLAC ou OGG, à l'image courante ; on peut aussi lâcher le fichier sur la fenêtre. `[son.importer]` | src/barres.rs:267 |
| Son ▾ | Barre du montage — visible d'office | — | Menu Son, identique au menu Son de la barre de menus (src/storyboard.rs:674) | src/barres.rs:268 |
| Importer une vidéo | Barre du montage — sous « Plus… » | — | Une vidéo devient un calque vidéo du plan courant, image par image (à dessiner par-dessus, ou à transformer en calque d'animation). `[fichier.importer_video]` | src/barres.rs:269 |
| Nouveau commentaire | Barre du montage — sous « Plus… » | — | Laisser un commentaire à l'image courante : un fanion dans la règle (à déplacer), qui s'écrit dans le panneau Commentaires. Ou double-clic dans la règle. `[commentaire.ajouter]` | src/barres.rs:277 |
| Animer ce plan | Barre du montage — sous « Plus… » | `Tab` | Entrer dans le plan courant pour l'animer (ou double-clic sur le plan). `[mode.basculer]` | src/barres.rs:286 |

### 3.3 Barre de l'espace Montage (onglet Montage)

Reprend les commandes de la barre du montage, dans un autre ordre et avec d'autres visibles d'office ; elle a ses propres épingles. Visibles d'office : Début du montage, Lire le montage / pause, Lecture en boucle, Son pendant la lecture, position, Nouveau plan, Nouvelle séquence, **Couper le plan**, **Réunir avec le suivant**, **Rogner le début**, **Rogner la fin**, **Durée du plan**, Avancer / Reculer le plan, Aimantation des bords, Plans ▾, Ajouter une transition, Importer un son, **Nouvelle piste son**, **Couper le son**, Son ▾, Replier / Déplier toutes les pistes, Nouveau commentaire, Exporter pour relecture, Importer les notes de relecture, Exporter pour le montage, Exporter la fiche technique. Sous « Plus… » : Déplier le calque courant, Animer ce plan, et tout le reste de la barre du montage (`src/barres.rs:369-425`). Seules les commandes propres à cette barre sont listées ici.

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| Couper le plan | Barre de l'espace Montage — visible d'office | `F6` | Couper le plan à l'image courante `[panneau.couper]` | src/barres.rs:389 |
| Réunir avec le suivant | Barre de l'espace Montage — visible d'office | — | Réunir le plan et le suivant `[panneau.fusionner_suivant]` | src/barres.rs:390 |
| Rogner le début | Barre de l'espace Montage — visible d'office | — | Raccourcir le début du plan jusqu'à la tête de lecture `[montage.debut_a_tete]` | src/barres.rs:391 |
| Rogner la fin | Barre de l'espace Montage — visible d'office | — | Raccourcir la fin du plan jusqu'à la tête de lecture `[montage.fin_a_tete]` | src/barres.rs:392 |
| Durée du plan | Barre de l'espace Montage — visible d'office | — | Durée du plan : taper la durée exacte… `[montage.duree]` | src/barres.rs:393 |
| Nouvelle piste son | Barre de l'espace Montage — visible d'office | — | Nouvelle piste son `[son.nouvelle_piste]` | src/barres.rs:401 |
| Couper le son | Barre de l'espace Montage — visible d'office | — | Couper le son à l'image courante `[son.couper]` | src/barres.rs:402 |

### 3.4 Barre de la timeline (en tête de la timeline, dans un plan et au Storyboard)

Particularité : les boutons Première / Précédente / Suivante / Dernière image et Lecture visent toujours le plan, même si le montage a la main (`src/barres.rs:1076-1082`).

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| Animer ce plan | Barre de la timeline — sous « Plus… » | `Tab` | Le dessin montre ce plan pour l'animer. `[mode.basculer]` | src/barres.rs:294 |
| (Nom / numéro du plan courant (affichage)) | Barre de la timeline | — | Affichage fixe, ne passe jamais sous « Plus… » | src/barres.rs:295 |
| Première image | Barre de la timeline — visible d'office | `Origine` | Première image du plan. `[image.premiere]` | src/barres.rs:296 |
| Image précédente | Barre de la timeline — visible d'office | `,` ou `Flèche gauche` | Reculer d'une image. `[image.precedente]` | src/barres.rs:297 |
| Lecture / pause | Barre de la timeline — visible d'office | `Entrée` | Lire le plan. `[lecture.basculer]` | src/barres.rs:298 |
| Image suivante | Barre de la timeline — visible d'office | `.` ou `Flèche droite` | Avancer d'une image. `[image.suivante]` | src/barres.rs:299 |
| Dernière image | Barre de la timeline — visible d'office | `Fin` | Dernière image du plan. `[image.derniere]` | src/barres.rs:300 |
| Lecture en boucle | Barre de la timeline — visible d'office | — | Reprendre au début une fois à la fin. (interrupteur allumé / éteint) `[lecture.boucle]` | src/barres.rs:301 |
| Cadence | Barre de la timeline — sous « Plus… » | — | Champ : images par seconde du projet (1 à 120) | src/barres.rs:302 |
| (Numéro de l'image courante (affichage)) | Barre de la timeline | — | Affichage fixe, ne passe jamais sous « Plus… » | src/barres.rs:303 |
| Repères ▾ | Barre de la timeline — visible d'office | — | Menu Repères, voir §3.5 (src/cues.rs:284) | src/barres.rs:304 |
| Affichage de la timeline ▾ | Barre de la timeline — visible d'office | — | Menu Affichage de la timeline, voir §3.5 (src/cues.rs:256) | src/barres.rs:305 |
| Nouveau dessin | Barre de la timeline — visible d'office | `Inser` (animation) | Un dessin vide après l'exposition courante. `[dessin.nouveau]` | src/barres.rs:306 |
| Dupliquer le dessin | Barre de la timeline — sous « Plus… » | `Ctrl+D` | Une copie du dessin courant, après lui. `[dessin.dupliquer]` | src/barres.rs:307 |
| Allonger l'exposition | Barre de la timeline — visible d'office | `Alt++` | Tenir le dessin une image de plus. `[exposition.allonger]` | src/barres.rs:308 |
| Raccourcir l'exposition | Barre de la timeline — visible d'office | `Alt+-` | Tenir le dessin une image de moins. `[exposition.raccourcir]` | src/barres.rs:309 |
| Supprimer le dessin | Barre de la timeline — sous « Plus… » | `Ctrl+Suppr` (animation) | Retirer le dessin et ses expositions de la timeline. `[dessin.supprimer]` | src/barres.rs:310 |
| Vider le dessin | Barre de la timeline — sous « Plus… » | — | Effacer tout ce qu'il contient ; il reste exposé. `[dessin.vider]` | src/barres.rs:311 |
| Exposition des nouveaux dessins | Barre de la timeline — visible d'office | — | Champ : « en 1 », « en 2 »… des nouveaux dessins | src/barres.rs:312 |
| Images ▾ | Barre de la timeline — visible d'office | — | Menu Images, voir §3.4 (src/images.rs:1126) | src/barres.rs:313 |
| Clé de caméra au début | Barre de la timeline — visible d'office | — | Une clé de caméra sur la première image du plan. `[camera.cle_debut]` | src/barres.rs:321 |
| Clé de caméra ici | Barre de la timeline — visible d'office | `Maj+K` | Une clé de caméra à l'image courante. `[camera.cle_ici]` | src/barres.rs:322 |
| Clé de caméra à la fin | Barre de la timeline — visible d'office | — | Une clé de caméra sur la dernière image du plan. `[camera.cle_fin]` | src/barres.rs:323 |
| Supprimer la clé de caméra | Barre de la timeline — visible d'office | — | Supprimer la clé de caméra de cette image. `[camera.cle_supprimer]` | src/barres.rs:324 |
| Adoucir | Barre de la timeline — sous « Plus… » | — | Pose un lissage de chaque côté de la clé : deux petits ronds creux apparaissent sur la piste de caméra, à tirer pour régler où il commence et où il finit. Un second clic le retire. (interrupteur allumé / éteint) `[camera.cle_adoucir]` | src/barres.rs:325 |
| Marque d'image | Barre de la timeline — sous « Plus… » | — | Une image de storyboard de plus dans le plan, à l'image courante (vignettes, PDF). Clic : poser ou retirer. (interrupteur allumé / éteint) `[marque.basculer]` | src/barres.rs:333 |
| Nouveau commentaire | Barre de la timeline — sous « Plus… » | — | Laisser un commentaire à l'image courante : un fanion dans la règle (à déplacer), qui s'écrit dans le panneau Commentaires. Ou double-clic dans la règle. `[commentaire.ajouter]` | src/barres.rs:341 |
| Pelure d'oignon | Barre de la timeline — sous « Plus… » | `Alt+O` | Montrer les dessins d'avant et d'après. (interrupteur allumé / éteint) `[vue.pelure]` | src/barres.rs:350 |
| Pelure : dessins avant | Barre de la timeline — sous « Plus… » | — | Champ : nombre de dessins de pelure avant | src/barres.rs:351 |
| Pelure : dessins après | Barre de la timeline — sous « Plus… » | — | Champ : nombre de dessins de pelure après | src/barres.rs:352 |
| Réglages de la pelure d'oignon | Barre de la timeline — sous « Plus… » | — | Mode, devant, couleurs, chaque image et son opacité, réglages rangés, décalage. (interrupteur allumé / éteint) `[pelure.panneau]` | src/barres.rs:353 |
| Flips | Barre de la timeline — sous « Plus… » | — | Feuilleter vite des images pour juger un mouvement. (interrupteur allumé / éteint) `[flip.panneau]` | src/barres.rs:361 |
| Son pendant la lecture | Barre de la timeline — sous « Plus… » | — | Entendre les pistes son en lisant. (interrupteur allumé / éteint) `[son.lecture]` | src/barres.rs:362 |

#### Menu « Images » (barre de la timeline, menu Animation ▸ Images, et clic droit dans la grille de la timeline)

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| Couper | Images | — | Couper les images choisies `[images.couper]` | src/images.rs:1132 |
| Copier | Images | — | Copier les images choisies `[images.copier]` | src/images.rs:1133 |
| Coller (insérer avant l'image courante) | Images | — | Coller les images (insérer avant l'image courante) `[images.coller]` | src/images.rs:1134 |
| Dupliquer | Images | — | Dupliquer les images choisies `[images.dupliquer]` | src/images.rs:1135 |
| Supprimer | Images | — | Supprimer les images choisies (calques choisis) `[images.supprimer]` | src/images.rs:1136 |
| Insérer une image avant | Images ▸ Insérer ou retirer des images | — | Insérer une image avant (calques choisis) `[images.inserer_avant_calques]` | src/images.rs:1140 |
| Insérer une image après | Images ▸ Insérer ou retirer des images | — | Insérer une image après (calques choisis) `[images.inserer_apres_calques]` | src/images.rs:1141 |
| Ajouter une image après l'image courante | Images ▸ Insérer ou retirer des images | `I` (animation) | Ajouter une image au plan, après l'image courante `[images.ajouter]` | src/images.rs:1144 |
| Insérer une image avant l'image courante | Images ▸ Insérer ou retirer des images | `Maj+I` (animation) | Insérer une image dans le plan, avant l'image courante `[images.inserer]` | src/images.rs:1145 |
| Supprimer l'image (ou les images choisies) | Images ▸ Insérer ou retirer des images | `Maj+F5` (animation) | Supprimer l'image du plan (tous les calques) `[images.supprimer_plan]` | src/images.rs:1146 |
| Modifier la durée des clés d'images… | Images ▸ Instances (dessin et ses expositions) | — | Modifier la durée des clés d'images… `[cles.duree]` | src/images.rs:1150 |
| Instance précédente | Images ▸ Instances (dessin et ses expositions) | `Ctrl+Flèche gauche` (animation) | Instance précédente (début du dessin d'avant) `[instance.precedente]` | src/images.rs:1152 |
| Instance suivante | Images ▸ Instances (dessin et ses expositions) | `Ctrl+Flèche droite` (animation) | Instance suivante (début du dessin d'après) `[instance.suivante]` | src/images.rs:1153 |
| Clé précédente (tous les calques) | Images ▸ Instances (dessin et ses expositions) | `A` | Clé précédente (début de dessin d'avant, tous les calques) `[cle.precedente]` | src/images.rs:1154 |
| Clé suivante (tous les calques) | Images ▸ Instances (dessin et ses expositions) | `F` | Clé suivante (début de dessin d'après, tous les calques) `[cle.suivante]` | src/images.rs:1155 |
| Tête de l'instance | Images ▸ Instances (dessin et ses expositions) | — | Tête de l'instance (sa première image) `[instance.tete]` | src/images.rs:1156 |
| Queue de l'instance | Images ▸ Instances (dessin et ses expositions) | — | Queue de l'instance (sa dernière image) `[instance.queue]` | src/images.rs:1157 |
| Casser l'instance à l'image courante | Images ▸ Instances (dessin et ses expositions) | — | Casser l'instance à l'image courante `[instance.casser]` | src/images.rs:1159 |
| Nouveau dessin avant l'instance | Images ▸ Instances (dessin et ses expositions) | — | Nouveau dessin avant l'instance courante `[instance.ajouter_avant]` | src/images.rs:1160 |
| Nouveau dessin après l'instance | Images ▸ Instances (dessin et ses expositions) | `Inser` (animation) | Nouveau dessin après l'exposition `[dessin.nouveau]` | src/images.rs:1161 |
| Une exposition de plus à chaque instance choisie | Images ▸ Instances (dessin et ses expositions) | — | Une exposition de plus à chaque instance choisie `[instance.exposition_plus]` | src/images.rs:1163 |
| Une exposition de moins à chaque instance choisie | Images ▸ Instances (dessin et ses expositions) | — | Une exposition de moins à chaque instance choisie `[instance.exposition_moins]` | src/images.rs:1164 |
| Coller (remplacer les images choisies) | Images ▸ Plus | — | Coller les images (remplacer les images choisies) `[images.coller_remplacer]` | src/images.rs:1167 |
| Faux-fixe ▸ À la suite, en boucle | Images ▸ Plus | — | Faux-fixe : rejouer les images choisies en boucle `[images.faux_fixe_boucle]` | src/images.rs:1173 |
| Faux-fixe ▸ En va-et-vient | Images ▸ Plus | — | Faux-fixe : rejouer les images choisies en va-et-vient `[images.faux_fixe_va_et_vient]` | src/images.rs:1174 |
| Faux-fixe ▸ Au hasard | Images ▸ Plus | — | Faux-fixe : rejouer les images choisies au hasard `[images.faux_fixe_hasard]` | src/images.rs:1175 |
| Étirer le calque jusqu'à la fin du plan ▸ En tenant sa dernière image | Images ▸ Plus | — | Étirer le calque jusqu'à la fin du plan (tenir sa dernière image) `[calque.etirer_tenir]` | src/images.rs:1178 |
| Étirer… ▸ En rejouant ses images en boucle | Images ▸ Plus | — | Étirer le calque jusqu'à la fin du plan (en boucle) `[calque.etirer_boucle]` | src/images.rs:1179 |
| Étirer… ▸ En rejouant ses images en va-et-vient | Images ▸ Plus | — | Étirer le calque jusqu'à la fin du plan (en va-et-vient) `[calque.etirer_va_et_vient]` | src/images.rs:1180 |
| Étirer… ▸ Couper le calque après l'image courante | Images ▸ Plus | — | Couper le calque après l'image courante `[calque.couper_apres]` | src/images.rs:1182 |
| Interpolation ▸ Insérer une image intermédiaire | Images ▸ Plus | — | Insérer une image intermédiaire (entre le dessin courant et le suivant) `[images.intermediaire]` | src/images.rs:1186 |
| Interpolation ▸ Insérer des images intermédiaires… | Images ▸ Plus | — | Insérer des images intermédiaires… `[images.intermediaires]` | src/images.rs:1187 |
| Interpolation ▸ Remplir les vides par des intervalles | Images ▸ Plus | — | Remplir les images vides par des intervalles `[images.intervalles]` | src/images.rs:1188 |
| Interpolation ▸ Étirer en interpolant… | Images ▸ Plus | — | Étirer des images en interpolant… `[images.etirer_interpoler]` | src/images.rs:1189 |
| Interpolation ▸ Changer la cadence du projet (durées gardées)… | Images ▸ Plus | — | Changer la cadence du projet en gardant les durées… `[projet.changer_cadence]` | src/images.rs:1191 |
| Choisir toutes les images | Images ▸ Plus | — | Choisir toutes les images du plan `[images.tout_choisir]` | src/images.rs:1194 |
| Inverser l'ordre | Images ▸ Plus | — | Inverser l'ordre des images choisies `[images.inverser]` | src/images.rs:1195 |
| Retourner ▸ Horizontalement | Images ▸ Plus | — | Retourner les images choisies horizontalement `[images.retourner_h]` | src/images.rs:1197 |
| Retourner ▸ Verticalement | Images ▸ Plus | — | Retourner les images choisies verticalement `[images.retourner_v]` | src/images.rs:1198 |
| Retourner ▸ Horizontalement et verticalement | Images ▸ Plus | — | Retourner les images choisies horizontalement et verticalement `[images.retourner_hv]` | src/images.rs:1199 |
| Noms d'instance ▸ Renommer l'instance courante | Images ▸ Plus | — | Renommer l'instance courante `[instance.renommer]` | src/images.rs:1202 |
| Noms d'instance ▸ Effacer les noms | Images ▸ Plus | — | Effacer les noms d'instance (calques choisis) `[instance.effacer_noms]` | src/images.rs:1203 |
| Noms d'instance ▸ Tout renommer (1, 2, 3…) | Images ▸ Plus | — | Renommer toutes les instances 1, 2, 3… `[instance.tout_renommer]` | src/images.rs:1204 |
| Noms d'instance ▸ Renommer intelligemment… | Images ▸ Plus | — | Renommer intelligemment les instances (préfixe, suffixe) `[instance.renommer_intelligemment]` | src/images.rs:1205 |
| Trou laissé par une suppression ▸ (boutons radio) | Images ▸ Plus | — | Choisit si une suppression d'images laisse un trou ou resserre | src/images.rs:1208-1214 |

#### 3.5 Menus « Repères » et « Affichage de la timeline » (barre de la timeline)

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| Poser / retirer un signet | Repères ▸ Signets | `Ctrl+Maj+Flèche haut` | Signet à l'image courante (poser / retirer) `[signet.basculer]` | src/cues.rs:286 |
| Signet précédent | Repères ▸ Signets | `Ctrl+Maj+Flèche gauche` | Signet précédent `[signet.precedent]` | src/cues.rs:287 |
| Signet suivant | Repères ▸ Signets | `Ctrl+Maj+Flèche droite` | Signet suivant `[signet.suivant]` | src/cues.rs:288 |
| Enlever les signets des images choisies | Repères ▸ Signets | — | Enlever les signets des images choisies `[signets.enlever_choisis]` | src/cues.rs:289 |
| Enlever tous les signets | Repères ▸ Signets | — | Enlever tous les signets du plan `[signets.tout_enlever]` | src/cues.rs:290 |
| Seulement les couleurs utilisées (case) / pastilles de couleur | Repères ▸ Marques de couleur (sur les dessins du calque) | — | Filtre et choisit la couleur de marque | src/marks.rs:177-192 |
| Poser / retirer sur l'instance courante | Repères ▸ Marques de couleur (sur les dessins du calque) | — | Marque de couleur sur l'instance courante (poser / retirer) `[marque_couleur.basculer]` | src/marks.rs:194 |
| Retirer la marque de l'instance courante | Repères ▸ Marques de couleur (sur les dessins du calque) | — | Retirer la marque de couleur de l'instance courante `[marque_couleur.retirer]` | src/marks.rs:195 |
| Marquer toutes les instances du calque | Repères ▸ Marques de couleur (sur les dessins du calque) | — | Marque de couleur sur toutes les instances du calque `[marque_couleur.toutes]` | src/marks.rs:196 |
| Marquer les instances non marquées | Repères ▸ Marques de couleur (sur les dessins du calque) | — | Marque de couleur sur les instances non marquées `[marque_couleur.non_marquees]` | src/marks.rs:197 |
| Retirer les marques du calque | Repères ▸ Marques de couleur (sur les dessins du calque) | — | Retirer les marques de couleur du calque `[marque_couleur.effacer_calque]` | src/marks.rs:198 |
| Retirer les marques du plan | Repères ▸ Marques de couleur (sur les dessins du calque) | — | Retirer les marques de couleur du plan `[marque_couleur.effacer_plan]` | src/marks.rs:199 |
| Marque précédente | Repères ▸ Marques de couleur (sur les dessins du calque) | — | Marque de couleur précédente `[marque_couleur.precedente]` | src/marks.rs:201 |
| Marque suivante | Repères ▸ Marques de couleur (sur les dessins du calque) | — | Marque de couleur suivante `[marque_couleur.suivante]` | src/marks.rs:202 |
| Marque précédente de cette couleur | Repères ▸ Marques de couleur (sur les dessins du calque) | — | Marque précédente de la couleur choisie `[marque_couleur.precedente_couleur]` | src/marks.rs:203 |
| Marque suivante de cette couleur | Repères ▸ Marques de couleur (sur les dessins du calque) | — | Marque suivante de la couleur choisie `[marque_couleur.suivante_couleur]` | src/marks.rs:204 |
| Répéter (revenir à la première au bout) / Continuer sur le calque suivant (cases) | Repères ▸ Marques de couleur (sur les dessins du calque) | — | Options de navigation entre marques | src/marks.rs:205-206 |
| Point d'entrée à l'image courante | Repères ▸ Entrée et sortie (lecture du plan) | — | Point d'entrée à l'image courante `[entree.poser]` | src/cues.rs:294 |
| Poser / retirer le point d'entrée | Repères ▸ Entrée et sortie (lecture du plan) | — | Point d'entrée (poser / retirer) `[entree.basculer]` | src/cues.rs:295 |
| Point de sortie à l'image courante | Repères ▸ Entrée et sortie (lecture du plan) | — | Point de sortie à l'image courante `[sortie.poser]` | src/cues.rs:296 |
| Poser / retirer le point de sortie | Repères ▸ Entrée et sortie (lecture du plan) | — | Point de sortie (poser / retirer) `[sortie.basculer]` | src/cues.rs:297 |
| Les images choisies deviennent les limites | Repères ▸ Entrée et sortie (lecture du plan) | — | Les images choisies deviennent les limites de la lecture `[boucle.limites_choisies]` | src/cues.rs:298 |
| Boucle entre les images choisies (allumer / éteindre) | Repères ▸ Entrée et sortie (lecture du plan) | — | Boucle entre les images choisies (allumer / éteindre) `[boucle.images_choisies]` | src/cues.rs:299 |
| Enlever les points d'entrée et de sortie | Repères ▸ Entrée et sortie (lecture du plan) | — | Enlever les points d'entrée et de sortie `[entree_sortie.enlever]` | src/cues.rs:300 |
| (boutons radio : image, time code, trame…) et « Image de départ du projet » | Repères ▸ Numéros | — | Change l'affichage des numéros et l'image de départ | src/cues.rs:302-313 |
| Zoom (curseur) | Affichage de la timeline | — | Zoom de la grille (aussi Ctrl + molette) | src/cues.rs:258-268 |
| Adapter la vue à… ▸ Le plan | Affichage de la timeline | — | Timeline : adapter au plan `[timeline.adapter_plan]` | src/cues.rs:271 |
| … ▸ Le calque courant | Affichage de la timeline | — | Timeline : adapter au calque courant `[timeline.adapter_calque]` | src/cues.rs:272 |
| … ▸ Les images choisies | Affichage de la timeline | — | Timeline : adapter aux images choisies `[timeline.adapter_choix]` | src/cues.rs:273 |
| … ▸ L'image courante | Affichage de la timeline | — | Timeline : adapter à l'image courante `[timeline.adapter_image]` | src/cues.rs:274 |
| … ▸ Les clés de caméra | Affichage de la timeline | — | Timeline : adapter aux clés de caméra `[timeline.adapter_cles]` | src/cues.rs:275 |
| … ▸ Tout voir | Affichage de la timeline | — | Timeline : tout voir `[timeline.tout_voir]` | src/cues.rs:276 |
| Croix sur les instances vides (case) | Affichage de la timeline | — | Montre une croix sur les instances vides | src/cues.rs:278 |
| Miniatures des dessins (lignes complètes) (case) | Affichage de la timeline | — | Montre les miniatures dans la timeline | src/cues.rs:279 |

## 4. Barre d'outils (colonne à gauche) et outils de dessin

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| Sélection | Barre d'outils, case 1 avec Sélection par couleur | `Alt+S` | Outil sélection `[outil.selection]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Sélection par couleur | Barre d'outils, case 1 (clic maintenu) | — | Outil sélection par couleur `[outil.selection_couleur]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Pinceau | Barre d'outils, case 2 avec Mélangeur | `Alt+B` | Outil pinceau `[outil.pinceau]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Mélangeur | Barre d'outils, case 2 (clic maintenu) | — | Outil mélangeur (étaler et mêler les couleurs déjà peintes, calque bitmap) `[outil.melangeur]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Crayon | Barre d'outils, case 3 | `Alt+P` | Outil crayon `[outil.crayon]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Gomme | Barre d'outils, case 4 | `Alt+E` | Outil gomme `[outil.gomme]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Cutter | Barre d'outils, case 5 | `Alt+T` | Outil cutter `[outil.cutter]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Remplir | Barre d'outils, case 6 avec Dégradé | `Alt+I` | Outil remplissage `[outil.remplissage]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Dégradé | Barre d'outils, case 6 (clic maintenu) | `Alt+G` | Outil dégradé (linéaire ou radial, bitmap et vectoriel) `[outil.degrade]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Rectangle | Barre d'outils, groupe 2, case formes | — | Outil rectangle `[outil.rectangle]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Ligne | Barre d'outils, case formes (clic maintenu) | — | Outil ligne `[outil.ligne]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Ellipse | Barre d'outils, case formes (clic maintenu) | — | Outil ellipse `[outil.ellipse]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Polyligne | Barre d'outils, case formes (clic maintenu) | — | Outil polyligne `[outil.polyligne]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Texte | Barre d'outils, groupe 2 | — | Outil texte `[outil.texte]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Transformer | Barre d'outils, groupe 3, case avec Déformer | — | Outil transformer le calque `[outil.transformer]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Déformer | Barre d'outils, case Transformer (clic maintenu) | — | Outil déformer (enveloppe ou perspective, le dernier mode pris) `[outil.perspective]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Grab | Barre d'outils, groupe 3, case avec Pull | — | Outil Grab (saisir les points sous le pinceau et les tirer) `[outil.grab]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Pull | Barre d'outils, case Grab (clic maintenu) | — | Outil Pull (étirer le trait avec le pinceau, comme un doigt) `[outil.pull]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Trajectoire | Barre d'outils, groupe 3 | — | Outil Trajectoire (dessiner le chemin du calque et ses marques d'espacement) `[outil.trajectoire]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Pipette | Barre d'outils, groupe 3 | `Alt+D` | Outil pipette `[outil.pipette]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Main | Barre d'outils, groupe 4 | `H` | Outil main `[outil.main]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Caméra | Barre d'outils, groupe 4 | `Maj+C` | Outil caméra `[outil.camera]` | src/toolbars.rs:15-24, src/app.rs:143-253 |
| Stylo d'annotation | Barre d'outils (Vignettes : seulement Pinceau, Crayon, Gomme, Caméra + annotation) | — | Vignettes : stylo d'annotation (par-dessus la planche) `[vignettes.annotation_stylo]` | src/toolbars.rs:30, src/annotationsui.rs |
| Gomme d'annotation | Barre d'outils (Vignettes : seulement Pinceau, Crayon, Gomme, Caméra + annotation) | — | Vignettes : gomme d'annotation `[vignettes.annotation_gomme]` | src/annotationsui.rs |
| Lasso d'annotation | Barre d'outils (Vignettes : seulement Pinceau, Crayon, Gomme, Caméra + annotation) | — | Vignettes : lasso d'annotation (choisir, déplacer, agrandir, tourner des traits) `[vignettes.annotation_lasso]` | src/annotationsui.rs |
| Autre outil tapé au clavier dans les Vignettes | Barre d'outils (Vignettes : seulement Pinceau, Crayon, Gomme, Caméra + annotation) | — | Refusé, la barre d'état dit « Dans les Vignettes : pinceau, crayon, gomme et caméra… » | src/app.rs:4450-4474 |
| Outil formes (dernière forme) | Clavier seulement (variantes d'outils) | `Alt+R` | Outil formes (dernière forme utilisée) `[outil.formes]` | src/keymap.rs:179 |
| Ligne droite | Clavier seulement (variantes d'outils) | — | Outil ligne, droite `[outil.ligne_droite]` | src/keymap.rs:181 |
| Ligne courbe | Clavier seulement (variantes d'outils) | — | Outil ligne, courbe `[outil.ligne_courbe]` | src/keymap.rs:182 |
| Ligne en S | Clavier seulement (variantes d'outils) | — | Outil ligne, courbe en S `[outil.ligne_s]` | src/keymap.rs:183 |
| Déformer : enveloppe | Clavier seulement (variantes d'outils) | — | Outil déformer : enveloppe de Bézier `[deformer.enveloppe]` | src/keymap.rs:192 |
| Déformer : perspective | Clavier seulement (variantes d'outils) | — | Outil déformer : perspective (quatre coins) `[deformer.perspective]` | src/keymap.rs:193 |
| Pinceau plus gros | Clavier seulement (variantes d'outils) | `]` | Pinceau plus gros `[outil.taille_plus]` | src/keymap.rs:198 |
| Pinceau plus fin | Clavier seulement (variantes d'outils) | `[` | Pinceau plus fin `[outil.taille_moins]` | src/keymap.rs:199 |
| Touche d'un outil tenue | Clavier seulement (variantes d'outils) | — | L'outil le temps de la tenir, puis retour au précédent | src/panels.rs:14, src/app.rs:4424-4427 |
| Clic maintenu 0,35 s sur une case à coin | Clavier seulement (variantes d'outils) | — | Ouvre la liste des outils de la case (Échap ferme) | src/toolbars.rs:33, 196-203 |
| Peindre derrière (interrupteur, pinceau et crayon) | Réglages de l'outil (panneau) — boutons de commande | — | Pinceau et crayon : peindre derrière (oui / non) `[pinceau.derriere]` | src/panels.rs:445-456 |
| Peint sur l'aplat (interrupteur) | Réglages de l'outil (panneau) — boutons de commande | — | Pinceau et crayon : peint sur l'aplat (oui / non) `[pinceau.sur_aplat]` | src/panels.rs:457-468 |
| Aplat sous le trait (bouton du calque) | Réglages de l'outil (panneau) — boutons de commande | — | Aplat sous le trait du calque courant (ajouter / retirer) `[pinceau.fond]` | src/panels.rs:469-474 |
| Effacer des traits entiers (gomme, calque vectoriel) | Réglages de l'outil (panneau) — boutons de commande | — | La gomme retire chaque trait touché en entier | src/panels.rs:539-546 |
| Remplir : Tolérance / Contours pris sur « Le calque courant » ou « Tous les calques visibles » / Plus de réglages ▸ Débord | Réglages de l'outil (panneau) — boutons de commande | — | Réglages du pot | src/panels.rs:554-567 |
| Sélection bitmap : Lasso / Rectangle | Réglages de l'outil (panneau) — boutons de commande | — | Mode de sélection | src/panels.rs:650-655 |
| Sélection : Tout sélectionner | Réglages de l'outil (panneau) — boutons de commande | `Ctrl+A` | Tout sélectionner `[edition.tout_selectionner]` | src/panels.rs:665, 726 |
| Sélection : Copier (si sélection) | Réglages de l'outil (panneau) — boutons de commande | `Ctrl+C` | Copier `[edition.copier]` | src/panels.rs:668, 729 |
| Sélection bitmap : Couper (si sélection) | Réglages de l'outil (panneau) — boutons de commande | `Ctrl+X` | Couper `[edition.couper]` | src/panels.rs:671 |
| Sélection : Coller (si quelque chose est copié) | Réglages de l'outil (panneau) — boutons de commande | `Ctrl+V` | Coller `[edition.coller]` | src/panels.rs:674, 732 |
| Sélection bitmap : Remplir | Réglages de l'outil (panneau) — boutons de commande | `Maj+Suppr` | Remplir la sélection de la couleur courante `[selection.remplir]` | src/panels.rs:677 |
| Sélection vectorielle : Supprimer | Réglages de l'outil (panneau) — boutons de commande | `Suppr` ou `Retour arrière` | Supprimer la sélection (rien sans sélection) `[edition.supprimer]` | src/panels.rs:735 |
| Plus de réglages ▸ Miroir horizontal | Réglages de l'outil (panneau) — boutons de commande | `Alt+H` | Retourner la sélection horizontalement `[selection.miroir_h]` | src/panels.rs:684, 751 |
| Plus de réglages ▸ Miroir vertical | Réglages de l'outil (panneau) — boutons de commande | `Alt+V` | Retourner la sélection verticalement `[selection.miroir_v]` | src/panels.rs:687, 754 |
| Plus de réglages ▸ Effacer dedans (bitmap) | Réglages de l'outil (panneau) — boutons de commande | — | Efface ce qui est dans la zone | src/panels.rs:690 |
| Plus de réglages ▸ Effacer autour (bitmap) | Réglages de l'outil (panneau) — boutons de commande | — | Efface hors de la zone (même effet que dessin.supprimer_hors_selection, sans le dire) | src/panels.rs:693 |
| Plus de réglages ▸ Poser (bitmap) | Réglages de l'outil (panneau) — boutons de commande | — | Repose la zone et quitte la sélection (Échap) | src/panels.rs:696 |
| Plus de réglages ▸ Couleur courante / Trait plus épais / Trait plus fin / Devant / Derrière (vectoriel) | Réglages de l'outil (panneau) — boutons de commande | — | Recolore, épaissit, amincit, change l'ordre des éléments choisis | src/panels.rs:742-762 |
| Transformer : champs x, y, largeur, hauteur, angle ; point fixe ; clés de position | Réglages de l'outil (panneau) — boutons de commande | — | Transforme le calque ; voir clés de position §7 | src/panels.rs:579-591 |
| ▶ (aide animée de l'outil) | Réglages de l'outil (panneau) — boutons de commande | — | Rejoue l'explication animée de l'outil dans une fenêtre flottante | src/panels.rs:508 |
| Plus de réglages (section repliable) | Réglages de l'outil (panneau) — boutons de commande | — | Déplie les réglages rares de l'outil, retenu outil par outil | src/panels.rs:419-433 |

## 5. Barres au-dessus et au-dessous du dessin

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| Vue caméra (icône caméra) | Barre au-dessus du dessin | — | Montre ce que filme la caméra du plan `[camera.vue]` | src/viewbar.rs:545-547 |
| 2D (Vue du plateau) | Barre au-dessus du dessin | — | Revient à la vue du plateau (quitte aussi l'Espace 3D) — même action affichée que « Vue caméra » `[camera.vue]` | src/viewbar.rs:548-553 |
| 3D (Espace 3D) | Barre au-dessus du dessin | — | Espace 3D (afficher / quitter) `[espace3d.basculer]` | src/viewbar.rs:554-566 |
| Espace 3D à côté | Barre au-dessus du dessin | — | Espace 3D à côté du dessin (afficher / masquer) `[espace3d.a_cote]` | src/viewbar.rs:568-581 |
| Aspect du décor 3D dans les plans (si la séquence a un décor) | Barre au-dessus du dessin | — | Matières, couleurs, argile | src/viewbar.rs:597-602 |
| Guides (bouton de la règle des guides de perspective) | Barre au-dessus du dessin | — | Ouvre les guides de perspective | src/viewbar.rs:603 |
| Ajuster à la fenêtre | Barre au-dessus du dessin | `Ctrl+0` | Ajuster à la fenêtre `[vue.ajuster]` | src/viewbar.rs:514 |
| Zoom (loupe) ▸ − / champ % / + / 100 % / Ajuster | Barre au-dessus du dessin | `2` | Panneau du zoom : vue.zoom_arriere, vue.zoom_avant, vue.reinit_zoom, vue.ajuster `[vue.zoom_avant]` | src/viewbar.rs:306-333 |
| Rotation de la vue ▸ ↶ / angle / ↷ / Remettre droite | Barre au-dessus du dessin | `V` | Panneau de rotation : vue.rotation_antihoraire, vue.rotation_horaire, vue.reinit_rotation `[vue.rotation_horaire]` | src/viewbar.rs:337-360 |
| Miroir de la vue | Barre au-dessus du dessin | `M` | Miroir de la vue `[vue.miroir]` | src/viewbar.rs:517 |
| Pastilles des brosses dédiées (à droite) | Barre au-dessus du dessin | — | Choisit une brosse dédiée ; clic droit : Modifier…, Dupliquer, Monter, Descendre, Supprimer | src/viewbar.rs:611, src/personnagesui.rs:1117-1142 |
| Outils de la vue (Choisir, Déplacer, Tourner, Échelle…) | Barre au-dessus du dessin, Espace 3D ouvert | — | Un bouton par mode ; voir la liste des actions espace3d.* au §8 `[espace3d.outil_choisir]` | src/espace3dui.rs:637-700 |
| Modifier l'origine | Barre au-dessus du dessin, Espace 3D ouvert | — | Espace 3D : modifier l'origine du calque ou de l'objet du décor choisi, sans le bouger (oui / non) `[espace3d.modifier_origine]` | src/espace3dui.rs, src/poignees3d.rs:50 |
| Aimanter au sol | Barre au-dessus du dessin, Espace 3D ouvert | — | Espace 3D : aimantation à la grille (oui / non) `[espace3d.aimanter]` | src/espace3dui.rs |
| Caméra | Barre au-dessus du dessin, Espace 3D ouvert | — | Espace 3D : choisir la caméra du plan `[espace3d.outil_camera]` | src/espace3dui.rs |
| Dessiner sur la carte | Barre au-dessus du dessin, Espace 3D ouvert | — | Espace 3D : dessiner sur la carte du calque courant (oui / non) `[espace3d.dessiner]` | src/espace3ddessin.rs:39 |
| Recadrer | Barre au-dessus du dessin, Espace 3D ouvert | — | Espace 3D : recadrer la vue sur tout l'espace `[espace3d.cadrer]` | src/espace3dui.rs:622-636 |
| Regarder par la caméra | Barre au-dessus du dessin, Espace 3D ouvert | — | Espace 3D : regarder par la caméra du plan `[espace3d.vue_camera]` | src/espace3dui.rs:622-636 |
| Vues (1, 2, 4) | Barre au-dessus du dessin, Espace 3D ouvert | — | Espace 3D : vues, disposition suivante (1, 2 côte à côte, 2 l'une sur l'autre, 4) `[espace3d.vues]` | src/espace3dui.rs:622-636 |
| Points de vue (Perspective, Dessus, Face…) | Barre au-dessus de la vue, onglet 3D | — | Change le point de vue de la vue | src/onglet3d.rs:594-620 |
| 1 / 2 / 4 (vues) | Barre au-dessus de la vue, onglet 3D | — | Une, deux (espace3d.vues_cote) ou quatre (espace3d.vues_quatre) vues `[espace3d.vues_une]` | src/onglet3d.rs |
| Ombres | Barre au-dessus de la vue, onglet 3D | — | Ombres du décor et des cartes (grisé sans décor) | src/onglet3d.rs |
| Grille du sol | Barre au-dessus de la vue, onglet 3D | — | Espace 3D : grille du sol (oui / non) `[espace3d.grille]` | src/onglet3d.rs |
| Cartes | Barre au-dessus de la vue, onglet 3D | — | Montre / cache les calques en cartes | src/onglet3d.rs |
| Caméras | Barre au-dessus de la vue, onglet 3D | — | Montre / cache les caméras des autres plans | src/onglet3d.rs |
| Lumières | Barre au-dessus de la vue, onglet 3D | — | Montre / cache soleil et lampes | src/onglet3d.rs |
| Recadrer | Barre au-dessus de la vue, onglet 3D | — | Espace 3D : recadrer la vue sur tout l'espace `[espace3d.cadrer]` | src/onglet3d.rs |
| Cadrage | Barre sous le dessin | — | Panneau : format (16:9, 2,39:1…) et zones de sécurité | src/viewbar.rs:281-286 |
| Repères de composition | Barre sous le dessin | — | Panneau : tiers, grille d'or, spirale, diagonales, croix (actions cadrage.*) | src/viewbar.rs:291-303 |
| Pelure d'oignon | Barre sous le dessin | `Alt+O` | Pelure d'oignon `[vue.pelure]` | src/viewbar.rs:366 |
| ˄ Réglages de la pelure d'oignon ▸ Avant / Après / Tous les réglages… | Barre sous le dessin | — | Nombre de dessins avant / après ; « Tous les réglages… » ouvre la fenêtre de la pelure `[pelure.panneau]` | src/viewbar.rs:367-385 |
| Table lumineuse | Barre sous le dessin | — | Table lumineuse (autres calques estompés) `[vue.table_lumineuse]` | src/viewbar.rs:391 |
| ˄ Réglage de la table lumineuse | Barre sous le dessin | — | Curseur « Autres calques » | src/viewbar.rs:392-399 |
| Passe-partout | Barre sous le dessin | — | Passe-partout autour du cadre de la caméra `[camera.masque]` | src/viewbar.rs:410-422 |
| ˄ Opacité du passe-partout | Barre sous le dessin | — | Curseur 0 à 100 % | src/viewbar.rs:423-437 |
| Zoom de la vue (curseur) | Barre sous le dessin | — | 5 à 3 200 %, double-clic pour taper | src/viewbar.rs:442-455 |
| Inclinaison de la vue (curseur) + Remettre la vue droite | Barre sous le dessin | `Maj+X` | Remettre la vue droite `[vue.reinit_rotation]` | src/viewbar.rs:458-477 |

## 6. Menus du clic droit : dessin, montage, timeline, planche

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| Annuler | Clic droit sur le dessin (feuille) | `Ctrl+Z` | Annuler `[edition.annuler]` | src/canvas.rs:383 |
| Rétablir | Clic droit sur le dessin (feuille) | `Ctrl+Maj+Z` ou `Ctrl+Y` | Rétablir `[edition.retablir]` | src/canvas.rs:384 |
| Couper | Clic droit sur le dessin (feuille) | `Ctrl+X` | Couper `[edition.couper]` | src/canvas.rs:386 |
| Copier | Clic droit sur le dessin (feuille) | `Ctrl+C` | Copier `[edition.copier]` | src/canvas.rs:387 |
| Coller | Clic droit sur le dessin (feuille) | `Ctrl+V` | Coller `[edition.coller]` | src/canvas.rs:388 |
| Supprimer la sélection | Clic droit sur le dessin (feuille) | `Suppr` ou `Retour arrière` | Supprimer la sélection (rien sans sélection) `[edition.supprimer]` | src/canvas.rs:389 |
| Ajuster à la fenêtre | Clic droit sur le dessin (feuille) | `Ctrl+0` | Ajuster à la fenêtre `[vue.ajuster]` | src/canvas.rs:391 |
| Animer ce plan / Retour au storyboard | Clic droit sur le dessin (feuille) | `Tab` | Entrer dans le plan pour l'animer, et en revenir `[mode.basculer]` | src/canvas.rs:392-394 |
| (Si « Réglages à droite » est permis, le clic droit ouvre plutôt les réglages de l'outil près du pointeur) | Clic droit sur le dessin (feuille) | — | Voir Préférences ; `reglagesdroit.rs` | src/canvas.rs:112-116 |
| Animer ce plan | Clic droit sur un plan dans la bande du montage | — | Entre dans le plan (bouton sans action : pas de raccourci affiché) | src/storyboard.rs:782 |
| Équipe ▸ Prendre ce plan / Reprendre ce plan / Rendre ce plan / Prendre la séquence / Prendre ou Rendre le montage (projet partagé) | Clic droit sur un plan dans la bande du montage | — | Jetons du travail à plusieurs | src/storyboard.rs:787-788, src/cooperationui.rs:392-426 |
| Nouveau plan après | Clic droit sur un plan dans la bande du montage | — | Nouveau plan (bouton sans action ; même effet que panneau.nouveau) | src/storyboard.rs:790 |
| Dupliquer le plan | Clic droit sur un plan dans la bande du montage | — | Duplique (bouton sans action ; même effet que panneau.dupliquer) | src/storyboard.rs:794 |
| Copier le plan / les N plans | Clic droit sur un plan dans la bande du montage | `Ctrl+C` | Copie les plans visés (raccourci affiché : celui de edition.copier) `[edition.copier]` | src/storyboard.rs:707-727 |
| Coller après le plan courant / Coller N plans après le plan courant | Clic droit sur un plan dans la bande du montage | `Ctrl+V` | Colle les plans (grisé sans copie) `[edition.coller]` | src/storyboard.rs:715-722 |
| Supprimer le plan / les N plans | Clic droit sur un plan dans la bande du montage | `Suppr` ou `Retour arrière` | Supprime les plans visés `[edition.supprimer]` | src/storyboard.rs:725 |
| Couper et réunir ▸ Couper le plan à l'image courante | Clic droit sur un plan dans la bande du montage | — | Bouton sans action (= panneau.couper) | src/storyboard.rs:802 |
| Couper et réunir ▸ Réunir avec le plan suivant | Clic droit sur un plan dans la bande du montage | — | Bouton sans action (= panneau.fusionner_suivant) | src/storyboard.rs:806 |
| Couper et réunir ▸ Fusionner les N plans choisis (N > 1) | Clic droit sur un plan dans la bande du montage | — | Fusionne | src/storyboard.rs:812 |
| Couper et réunir ▸ Nouvelle séquence avec les N plans choisis (N > 1) | Clic droit sur un plan dans la bande du montage | — | Crée une séquence | src/storyboard.rs:817 |
| Version du plan (Vn) ▸ Nouvelle version, en dupliquant / Nouvelle version vide / Passer à la Vn / Restaurer la Vn / Supprimer la Vn / Comparer le plan… / Mettre à jour dans les autres versions de séquence | Clic droit sur un plan dans la bande du montage | — | Versions du plan | src/storyboard.rs:824, src/planversui.rs:310-370 |
| Couleur du plan ▸ Aucune / pastilles | Clic droit sur un plan dans la bande du montage | — | Étiquette de couleur des plans choisis | src/storyboard.rs:745-770 |
| Transition au début du plan ▸ (sortes) / Supprimer la transition / Modifier la transition… | Clic droit sur un plan dans la bande du montage | — | Pose ou modifie la transition | src/storyboard.rs:831, src/transitionsui.rs:588-612 |
| Transition à la fin du plan ▸ (idem) | Clic droit sur un plan dans la bande du montage | — | idem | src/storyboard.rs:832 |
| Changer la durée des plans choisis… (plusieurs choisis) | Clic droit sur un plan dans la bande du montage | — | Changer la durée des plans choisis… `[montage.duree_plans]` | src/storyboard.rs:833-834 |
| Transition ▸ (sortes) / Supprimer la transition / Modifier la transition… | Clic droit sur une coupe (montage) | — | Transition à cette coupe | src/transitionsui.rs:583-612 |
| Supprimer la transition | Clic droit sur une transition (montage, timeline du Storyboard) | — | Retire la transition | src/transitionsui.rs:496, 576 |
| Sorte : X ▸ (sortes) | Clic droit sur une transition (montage, timeline du Storyboard) | — | Change la sorte | src/transitionsui.rs:504-510 |
| Sens : X ▸ (côtés) | Clic droit sur une transition (montage, timeline du Storyboard) | — | Change le sens | src/transitionsui.rs:530-540 |
| Centre / Couleur / Alignement (selon la sorte) | Clic droit sur une transition (montage, timeline du Storyboard) | — | Réglages | src/transitionsui.rs:546-572 |
| Couper le son ici (tc) | Clic droit sur une piste son / un extrait (montage, timeline) | — | Coupe l'extrait | src/soundtrack.rs:803 |
| Supprimer l'extrait | Clic droit sur une piste son / un extrait (montage, timeline) | — | Supprime l'extrait | src/soundtrack.rs:807 |
| (effets de l'extrait) | Clic droit sur une piste son / un extrait (montage, timeline) | — | Volume, fondus… (`clip_fx_menu`) | src/soundtrack.rs:812 |
| Importer un son ici (tc)… | Clic droit sur une piste son / un extrait (montage, timeline) | — | Importe à cette image | src/soundtrack.rs:815 |
| La piste ▸ Nom, réglages, Importer un son sur cette piste…, Nouvelle piste son, Dupliquer la piste, Supprimer la piste | Clic droit sur une piste son / un extrait (montage, timeline) | — | Piste son | src/soundtrack.rs:748-797, 821 |
| Nom, réglages, Importer un son sur cette piste…, Nouvelle piste son, Dupliquer la piste, Supprimer la piste | Clic droit sur l'en-tête d'une piste son | — | Piste son | src/soundtrack.rs:725, 748-797 |
| Modifier | Clic droit sur un commentaire (règle, liste, fil) | — | Édite le texte | src/commentairesui.rs:462 |
| Couleur ▸ pastilles | Clic droit sur un commentaire (règle, liste, fil) | — | Couleur | src/commentairesui.rs:466 |
| Statut ▸ (statuts) | Clic droit sur un commentaire (règle, liste, fil) | — | Statut de la note | src/commentairesui.rs:472-478 |
| Marquer comme retake / Retirer le retake | Clic droit sur un commentaire (règle, liste, fil) | — | Retake | src/commentairesui.rs:481 |
| Supprimer | Clic droit sur un commentaire (règle, liste, fil) | — | Supprime le commentaire | src/commentairesui.rs:486 |
| Couleur de l'étiquette Vn ▸ pastilles / Aucune | Clic droit sur une étiquette de version dépliée (montage) | — | Repère de couleur | src/versiondeplieeui.rs:366-400 |
| Passer à la Vn / (autre bouton) | Clic droit sur une étiquette de version dépliée (montage) | — | Change de version | src/versiondeplieeui.rs:405-415 |
| Nouvelle séquence | Timeline du Storyboard : clic droit sur la ligne « Séquence » | — | Nouvelle séquence `[sequence.nouvelle]` | src/timelinesb.rs:157 |
| Renommer la séquence… | Timeline du Storyboard : clic droit sur la ligne « Séquence » | — | Renommer la séquence (son titre sur la planche, ou une petite fenêtre) `[sequence.renommer]` | src/timelinesb.rs:158 |
| Versions de la séquence ▸ (voir §1) | Timeline du Storyboard : clic droit sur la ligne « Séquence » | — | idem menu Storyboard | src/timelinesb.rs:160 |
| Dupliquer la séquence | Timeline du Storyboard : clic droit sur la ligne « Séquence » | — | Dupliquer la séquence (une copie indépendante, juste après elle) `[sequence.dupliquer]` | src/timelinesb.rs:164 |
| Nouvelle séquence avec les plans choisis | Timeline du Storyboard : clic droit sur la ligne « Séquence » | — | Nouvelle séquence avec les plans choisis `[sequence.nouvelle_avec_plans]` | src/timelinesb.rs:165 |
| Couper la séquence ici (le plan courant la commence) | Timeline du Storyboard : clic droit sur la ligne « Séquence » | — | Couper la séquence ici (la case courante commence une séquence) `[sequence.couper_ici]` | src/timelinesb.rs:166 |
| Clé de caméra ici | Timeline du Storyboard : clic droit sur la ligne « Caméra » | `Maj+K` | Clé de caméra à l'image courante `[camera.cle_ici]` | src/timelinesb.rs:284 |
| Préréglages de caméra | Timeline du Storyboard : clic droit sur la ligne « Caméra » | — | Ouvre le panneau Préréglages, onglet Caméra | src/timelinesb.rs:287 |
| Remettre la caméra à zéro | Timeline du Storyboard : clic droit sur la ligne « Caméra » | — | Réinitialiser la caméra `[camera.reinitialiser]` | src/timelinesb.rs:296-298 |
| Convertir les préréglages en clés | Timeline du Storyboard : clic droit sur la ligne « Caméra » | — | Tous les préréglages du plan deviennent des clés | src/timelinesb.rs:303 |
| Copier les clés de caméra choisies | Timeline du Storyboard : clic droit sur la ligne « Caméra » | — | Copier les clés de caméra choisies `[camera.cles_copier]` | src/timelinesb.rs:308 |
| Coller les clés de caméra | Timeline du Storyboard : clic droit sur la ligne « Caméra » | — | Coller les clés de caméra (insérées après la clé de la tête de lecture) `[camera.cles_coller]` | src/timelinesb.rs:309 |
| + tout le menu Caméra de la timeline (ci-dessous) | Timeline du Storyboard : clic droit sur la ligne « Caméra » | — |  | src/timelinesb.rs:311 |
| Signet ici (poser / retirer) | Timeline : clic droit dans la règle | `Ctrl+Maj+Flèche haut` | Signet à l'image courante (poser / retirer) `[signet.basculer]` | src/handles.rs:1133 |
| Marque d'image ici (poser / retirer) | Timeline : clic droit dans la règle | — | Marque d'image à l'image courante (ajouter / retirer) `[marque.basculer]` | src/handles.rs:1134 |
| Commentaire ici | Timeline : clic droit dans la règle | — | Nouveau commentaire (à l'image courante) `[commentaire.ajouter]` | src/handles.rs:1135 |
| Point d'entrée ici | Timeline : clic droit dans la règle | — | Point d'entrée à l'image courante `[entree.poser]` | src/handles.rs:1137 |
| Point de sortie ici | Timeline : clic droit dans la règle | — | Point de sortie à l'image courante `[sortie.poser]` | src/handles.rs:1138 |
| Enlever l'entrée et la sortie | Timeline : clic droit dans la règle | — | Enlever les points d'entrée et de sortie `[entree_sortie.enlever]` | src/handles.rs:1139 |
| Rogner le plan ▸ (voir Storyboard ▸ Plans) | Timeline : clic droit dans la règle | — |  | src/handles.rs:1143 |
| Adapter la timeline au plan | Timeline : clic droit dans la règle | — | Timeline : adapter au plan `[timeline.adapter_plan]` | src/handles.rs:1145 |
| Timeline : tout voir | Timeline : clic droit dans la règle | — | Timeline : tout voir `[timeline.tout_voir]` | src/handles.rs:1146 |
| Clé de caméra ici (si pas de clé) | Timeline : clic droit sur la piste Caméra | `Maj+K` | Clé de caméra à l'image courante `[camera.cle_ici]` | src/handles.rs:1155 |
| Clé de caméra au début du plan | Timeline : clic droit sur la piste Caméra | — | Clé de caméra au début du plan `[camera.cle_debut]` | src/handles.rs:1157 |
| Clé de caméra à la fin du plan | Timeline : clic droit sur la piste Caméra | — | Clé de caméra à la fin du plan `[camera.cle_fin]` | src/handles.rs:1158 |
| Adoucir le mouvement à cette clé (sur une clé) | Timeline : clic droit sur la piste Caméra | — | Adoucir le mouvement à cette clé de caméra `[camera.cle_adoucir]` | src/handles.rs:1160 |
| Supprimer la clé de caméra d'ici (sur une clé) | Timeline : clic droit sur la piste Caméra | — | Supprimer la clé de caméra de l'image courante `[camera.cle_supprimer]` | src/handles.rs:1161 |
| Copier / Coller / Coller à la tête de lecture / Supprimer / Choisir toutes les clés de caméra du plan | Timeline : clic droit sur la piste Caméra | `Ctrl+C` | Menu commun des clés ; raccourcis de edition.copier / coller / coller_a_la_tete / supprimer / tout_selectionner `[edition.copier]` | src/camkeys.rs:881, src/clescommun.rs:63-137 |
| Plus ▸ Effets de caméra (réglages de l'outil Caméra) | Timeline : clic droit sur la piste Caméra | `Maj+C` | Outil caméra `[outil.camera]` | src/handles.rs:1168 |
| Plus ▸ Vue à travers la caméra | Timeline : clic droit sur la piste Caméra | — | Vue à travers la caméra `[camera.vue]` | src/handles.rs:1169 |
| Plus ▸ Réinitialiser la caméra | Timeline : clic droit sur la piste Caméra | — | Réinitialiser la caméra `[camera.reinitialiser]` | src/handles.rs:1170 |
| Plus ▸ Adapter la timeline aux clés | Timeline : clic droit sur la piste Caméra | — | Timeline : adapter aux clés de caméra `[timeline.adapter_cles]` | src/handles.rs:1171 |
| Copier / Coller / Coller à la tête / Supprimer / Choisir toutes les clés de l'effet | Timeline : clic droit sur une clé d'effet | — | Menu commun des clés | src/fxkeys.rs:800 |
| Pile d'effets du calque… | Timeline : clic droit sur une clé d'effet | — | Pile d'effets du calque (ouvrir / fermer) `[fx.panneau]` | src/fxkeys.rs:802 |
| Clés d'effets (montrer / cacher) | Timeline : clic droit sur une clé d'effet | — | Clés d'effets dans la timeline et le montage (montrer / cacher) `[fx.cles_timeline]` | src/fxkeys.rs:803 |
| Lisser la clé de position | Timeline : clic droit sur une clé de position | — | Lisser la clé de position de l'image courante `[calque.cle_position_lisser]` | src/poskeysui.rs:592 |
| Supprimer la clé de position | Timeline : clic droit sur une clé de position | — | Supprimer la clé de position de l'image courante `[calque.cle_position_supprimer]` | src/poskeysui.rs:593 |
| Clé de position ici (pose actuelle) | Timeline : clic droit sur une clé de position | — | Poser une clé de position à l'image courante `[calque.cle_position]` | src/poskeysui.rs:595 |
| Supprimer toutes les clés du calque | Timeline : clic droit sur une clé de position | — | Supprimer toutes les clés de position du calque `[calque.cles_position_supprimer]` | src/poskeysui.rs:596 |
| Lisser la clé d'opacité | Timeline : clic droit sur une clé d'opacité | — | Lisser la clé d'opacité de l'image courante `[calque.cle_opacite_lisser]` | src/opacitecleselui.rs:450 |
| Supprimer la clé d'opacité | Timeline : clic droit sur une clé d'opacité | — | Supprimer la clé d'opacité de l'image courante `[calque.cle_opacite_supprimer]` | src/opacitecleselui.rs:451 |
| Clé d'opacité ici (opacité actuelle) | Timeline : clic droit sur une clé d'opacité | — | Poser une clé d'opacité à l'image courante `[calque.cle_opacite]` | src/opacitecleselui.rs:453 |
| Supprimer toutes les clés d'opacité du calque | Timeline : clic droit sur une clé d'opacité | — | Supprimer toutes les clés d'opacité du calque `[calque.cles_opacite_supprimer]` | src/opacitecleselui.rs:454 |
| Importer un son… | Timeline : clic droit sur l'en-tête Son / Notes | — | Importer un son `[son.importer]` | src/handles.rs:1179 |
| Nouvelle piste son | Timeline : clic droit sur l'en-tête Son / Notes | — | Nouvelle piste son `[son.nouvelle_piste]` | src/handles.rs:1180 |
| Couper le son à l'image courante | Timeline : clic droit sur l'en-tête Son / Notes | — | Couper le son à l'image courante `[son.couper]` | src/handles.rs:1181 |
| Son au défilement (activer / couper) | Timeline : clic droit sur l'en-tête Son / Notes | — | Son au défilement (activer / couper) `[son.defilement]` | src/handles.rs:1183 |
| Son pendant la lecture (activer / couper) | Timeline : clic droit sur l'en-tête Son / Notes | — | Son pendant la lecture (activer / couper) `[son.lecture]` | src/handles.rs:1184 |
| Onglet Son (pistes, volumes) | Timeline : clic droit sur l'en-tête Son / Notes | — | Onglet Son de la timeline (ouvrir / fermer) `[onglet.son]` | src/handles.rs:1185 |
| Onglet Notes (crayon, gomme, couleurs) | Timeline : clic droit sur l'en-tête Son / Notes | — | Onglet Notes de la timeline (ouvrir / fermer) `[onglet.notes]` | src/handles.rs:1188 |
| Rôle ▸ (Clé, Breakdown, Intervalle à faire, … ) / Aucun rôle | Timeline : clic droit dans la grille des images | — | Une ligne par rôle : role.cle, role.breakdown, role.intervalle, role.intervalle_complete, role.intervalle_special, role.intervalle_special_complete, role.aucun `[role.cle]` | src/timeline.rs:830-833, src/rolesui.rs:63-70 |
| Rôle ▸ Montrer les rôles dans la timeline / Sur le calque courant seulement / Opacité des rôles | Timeline : clic droit dans la grille des images | — | Affichage des rôles | src/rolesui.rs:72-76 |
| + tout le menu Images (§3.4) | Timeline : clic droit dans la grille des images | — |  | src/timeline.rs:834 |
| Rôle ▸ (idem) | Feuille d'exposition : clic droit | — | Menu des rôles | src/xsheetui.rs:120 |
| Animer cette image | Vignettes : clic droit sur une case | — | Entre dans le plan à cette image | src/boardui.rs:2680 |
| Réglages de l'outil… (si permis) | Vignettes : clic droit sur une case | — | Ouvre les réglages près du pointeur | src/boardui.rs:2684 |
| Retirer ce cadre / Ajouter un cadre | Vignettes : clic droit sur une case | — | Cadres de caméra A, B, C de la case | src/boardui.rs:2694-2699 |
| Équipe ▸ … (projet partagé) | Vignettes : clic droit sur une case | — | Jetons | src/boardui.rs:2702 |
| Nouveau plan après | Vignettes : clic droit sur une case | — | Nouveau plan, montré sur la planche | src/boardui.rs:2704 |
| Dupliquer le plan | Vignettes : clic droit sur une case | — | Duplique, montré sur la planche | src/boardui.rs:2708 |
| Copier / Coller après le plan courant / Supprimer | Vignettes : clic droit sur une case | `Ctrl+C` | Presse-papiers des plans (idem montage) `[edition.copier]` | src/boardui.rs:2713, src/storyboard.rs:707 |
| Faire de cette image un nouveau plan (image > 0) / Réunir avec le plan précédent | Vignettes : clic droit sur une case | — | Coupe ou réunit | src/boardui.rs:2715-2719 |
| Version du plan (Vn) ▸ … | Vignettes : clic droit sur une case | — | Versions | src/boardui.rs:2722 |
| Plus ▸ Écarter / Reprendre le plan (planannuleui::MENU_ECARTER) | Vignettes : clic droit sur une case | — | Sort le plan du film sans le supprimer | src/boardui.rs:2726 |
| Plus ▸ Séquence ▸ Nouvelle séquence à la fin du film / Nouvelle séquence avec les cases choisies / Couper la séquence ici / Renommer la séquence… / Dupliquer la séquence | Vignettes : clic droit sur une case | — | Actions sequence.nouvelle_fin, sequence.nouvelle_avec_cases, sequence.couper_ici, sequence.renommer, sequence.dupliquer `[sequence.nouvelle_fin]` | src/planchesequencesui.rs:502-509 |
| Plus ▸ Couleur du plan ▸ … | Vignettes : clic droit sur une case | — | Étiquette | src/boardui.rs:2733 |
| Plus ▸ Nouvelle séquence avec les N plans choisis / Changer la durée des N plans choisis… (N > 1) | Vignettes : clic droit sur une case | — | Nouvelle séquence avec les plans choisis `[sequence.nouvelle_avec_plans]` | src/boardui.rs:2737-2738 |
| Plus ▸ Recadrer (outil Caméra) | Vignettes : clic droit sur une case | — | Prend l'outil Caméra | src/boardui.rs:2740 |
| Plus ▸ Réinitialiser le cadrage | Vignettes : clic droit sur une case | — | Réinitialiser la caméra `[camera.reinitialiser]` | src/boardui.rs:2741 |
| N note(s) choisie(s) ▸ Attacher à la vignette la plus proche / Détacher / Couleur courante / Texte plus grand / Texte plus petit / Supprimer les notes choisies (Suppr) | Vignettes : clic droit sur le papier (hors case) | — | Notes de la planche | src/boardui.rs:2764-2810 |
| Nouveau plan | Vignettes : clic droit sur le papier (hors case) | — | Nouveau plan | src/boardui.rs:2818 |
| Nouvelle séquence à la fin du film | Vignettes : clic droit sur le papier (hors case) | — | Nouvelle séquence à la fin du film (depuis les vignettes) `[sequence.nouvelle_fin]` | src/boardui.rs:2822 |
| Annotations ▸ Stylo / Gomme / Lasso d'annotation / Nouveau calque d'annotation / Cacher-Montrer les annotations / Effacer les annotations… / Marie-louise / Exporter la planche des vignettes… | Vignettes : clic droit sur le papier (hors case) | — | Actions vignettes.annotation_*, vignettes.annotations_afficher, vignettes.marie_louise, fichier.exporter_planche `[vignettes.annotation_stylo]` | src/annotationsui.rs:974-991 |
| Tout choisir (plans) | Vignettes : clic droit sur le papier (hors case) | — | Choisit tous les plans | src/boardui.rs:2829 |
| Page à la largeur de la vue / Planche à la hauteur de la vue | Vignettes : clic droit sur le papier (hors case) | — | Cadre la planche | src/boardui.rs:2833 |
| Séquence X ▸ Renommer la séquence… / Dupliquer la séquence + menu du papier | Vignettes : clic droit sur un titre de séquence | — | Séquence | src/planchesequencesui.rs:271-282 |
| Reprendre (planannuleui::MENU_REPRENDRE) | Vignettes : clic droit sur un plan écarté | — | Remet le plan dans le film | src/planannuleui.rs:278-279 |
| Couche ▸ Viser cette couche / Cacher-Montrer / Renommer / Monter / Descendre / Supprimer ; Nouvelle couche d'annotation | Vignettes : panneau Planche, clic droit sur les couches d'annotation | — | Couches d'annotation `[vignettes.annotation_calque_nouveau]` | src/vignettespanneau.rs:138-183 |
| Renommer la séquence… / Dupliquer la séquence | Planche des séquences (Vignettes), clic droit | — |  | src/planchesequencesui.rs:436 |

## 7. Menus du clic droit : calques, couleur, panneaux

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
### 7.1 Menu d'un calque — le même pour : clic droit ou ⋮ sur une ligne de la timeline, clic droit sur une piste de calque du montage, menu Calques de la barre de menus, « Opérations sur les calques » de la ligne Calques, panneau Scène

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| ＋ Nouveau calque ▸ (sortes : vectoriel, bitmap, couleur, couleur unie, effet ▸ préréglages, annotation, vidéo, dossier, Calques 3D ▸ si vue 3D) ; Nom, groupe, place… | Calque | — | Sortes : calque.nouveau_vectoriel, calque.nouveau_bitmap, calque.nouveau_couleur_unie… `[calque.nouveau]` | src/menucalque.rs:119, src/calquemenu.rs:166-189 |
| Dupliquer / Dupliquer les calques choisis (N) | Calque | — | Dupliquer les calques choisis `[calque.dupliquer]` | src/menucalque.rs:120 |
| Réutiliser dans… ▸ Le plan suivant | Calque | — | Réutiliser le calque dans le plan suivant (planche-mère) `[calque.reutiliser_suivant]` | src/partageui.rs:134 |
| Réutiliser dans… ▸ Le plan précédent | Calque | — | Réutiliser le calque dans le plan précédent (planche-mère) `[calque.reutiliser_precedent]` | src/partageui.rs:135 |
| Réutiliser dans… ▸ Tous les plans de la séquence | Calque | — | Réutiliser le calque dans tous les plans de la séquence (planche-mère) `[calque.reutiliser_sequence]` | src/partageui.rs:136 |
| Réutiliser dans… ▸ Des plans choisis… | Calque | — | Réutiliser le calque dans des plans choisis… (planche-mère) `[calque.reutiliser_choisir]` | src/partageui.rs:137 |
| Réutiliser dans… ▸ Détacher (garder une copie) (calque partagé) | Calque | — | Détacher le calque de la planche-mère (il garde une copie) `[calque.detacher_partage]` | src/partageui.rs:140 |
| Renommer… | Calque | — | Champ de renommage (double-clic sur le nom : pareil) | src/layersui.rs:257-259 |
| Supprimer / Supprimer les calques choisis (N) | Calque | `Q` | Supprimer les calques choisis `[calque.supprimer]` | src/menucalque.rs:124 |
| Fusionner vers le bas (grisé avec raison en bulle) | Calque | `Ctrl+E` | Fusionner vers le bas (avec le calque du dessous) `[calque.fusionner_dessous]` | src/menucalque.rs:134 |
| Fusionner les calques choisis / les N calques choisis | Calque | — | Fusionner les calques choisis `[calque.fusionner_choisis]` | src/menucalque.rs:146 |
| Organiser ▸ Monter le calque | Calque | `Ctrl+Flèche haut` | Monter le calque d'un rang `[calque.monter]` | src/menucalque.rs:172 |
| Organiser ▸ Descendre le calque | Calque | `Ctrl+Flèche bas` | Descendre le calque d'un rang `[calque.descendre]` | src/menucalque.rs:173 |
| Organiser ▸ Aller au calque ▸ Du dessus | Calque | `Flèche haut` | Calque du dessus `[calque.dessus]` | src/menucalque.rs:175 |
| … ▸ Du dessous | Calque | `Flèche bas` | Calque du dessous `[calque.dessous]` | src/menucalque.rs:176 |
| … ▸ Le premier (le plus bas) | Calque | `Ctrl+Origine` (animation) | Premier calque (le plus bas) `[calque.premier]` | src/menucalque.rs:177 |
| … ▸ Le dernier (le plus haut) | Calque | `Ctrl+Fin` (animation) | Dernier calque (le plus haut) `[calque.dernier]` | src/menucalque.rs:178 |
| Organiser ▸ Dossiers ▸ Nouveau dossier | Calque | — | Nouveau dossier de calques `[dossier.nouveau]` | src/menucalque.rs:182 |
| … ▸ Placer les calques choisis dans un dossier | Calque | — | Placer les calques choisis dans un nouveau dossier `[dossier.ranger]` | src/menucalque.rs:183 |
| … ▸ Déposer les calques choisis dans le dossier | Calque | — | Déposer les calques choisis dans le dossier `[dossier.deposer]` | src/menucalque.rs:184 |
| … ▸ Sortir les calques choisis de leur dossier | Calque | — | Sortir les calques choisis de leur dossier `[dossier.sortir]` | src/menucalque.rs:185 |
| … ▸ Joindre les dossiers choisis dans le dossier courant | Calque | — | Joindre les dossiers choisis dans le dossier courant `[dossier.joindre]` | src/menucalque.rs:187 |
| … ▸ Supprimer uniquement le dossier | Calque | — | Supprimer uniquement le dossier (ses calques restent) `[dossier.supprimer_seul]` | src/menucalque.rs:188 |
| … ▸ Supprimer le dossier et ses calques | Calque | `Q` | Même action que « Supprimer » le calque `[calque.supprimer]` | src/menucalque.rs:189 |
| Organiser ▸ Groupes ▸ Éditer les groupes… | Calque | — | Groupes de calques (noms, couleurs, montrés dans la timeline) `[groupes.editer]` | src/groupsui.rs:107 |
| … ▸ Afficher tous les groupes / Afficher le groupe courant / Cacher le groupe courant / Afficher les autres groupes / Cacher les autres groupes | Calque | — | Visibilité des groupes dans la timeline | src/groupsui.rs:110-125 |
| … ▸ Focus sur le groupe du calque courant | Calque | — | Focus sur le groupe du calque courant (la timeline ne montre que lui) `[groupes.focus]` | src/groupsui.rs:127 |
| … ▸ Choisir les calques du groupe courant | Calque | — | Choisir les calques du même groupe `[calque.choisir_groupe]` | src/groupsui.rs:129 |
| … ▸ Choisir les calques des autres groupes | Calque | — | Choisir les calques des autres groupes `[calque.choisir_autres_groupes]` | src/groupsui.rs:130 |
| … ▸ Tous les calques / Groupe courant / Autres groupes ▸ Pelure allumer-éteindre, Verrouiller-Déverrouiller, Préserver la transparence, Afficher-Cacher, Pochoir activer-désactiver-inverser, Fusionner ces calques | Calque | — | Réglage en masse | src/groupsui.rs:132-175 |
| Apparence ▸ Afficher le calque (case) | Calque | — | Visibilité | src/menucalque.rs:198 |
| Apparence ▸ Verrouiller / Déverrouiller le calque | Calque | `Alt+L` | Verrouiller le calque `[calque.verrouiller]` | src/menucalque.rs:201-205 |
| Apparence ▸ Opacité (curseur) | Calque | — | Opacité à l'image courante | src/menucalque.rs:206-215 |
| Apparence ▸ Mode de fusion : X ▸ (modes) | Calque | — | Mode de fusion | src/menucalque.rs:217-227 |
| Apparence ▸ Groupe · Couleur d'étiquette ▸ (groupes) / Éditer les groupes… | Calque | — | Groupe du calque ; « Éditer les groupes… » ici est un bouton sans action | src/menucalque.rs:232, src/layerheads.rs:1133-1147 |
| Apparence ▸ Pochoir négatif (grisé si pas pochoir) | Calque | — | Sens du pochoir | src/menucalque.rs:235-239 |
| Apparence ▸ Verrouiller la position | Calque | — | Bloque Transformer | src/menucalque.rs:240-242 |
| Apparence ▸ Pelure d'oignon du calque | Calque | `Ctrl+L` | Pelure d'oignon du calque courant (allumer / éteindre) `[calque.pelure]` | src/menucalque.rs:249 |
| Apparence ▸ Panneau de la pelure d'oignon | Calque | — | Panneau de la pelure d'oignon (ouvrir / fermer) `[pelure.panneau]` | src/menucalque.rs:252 |
| Effets ▸ (familles ▸ effets à cocher, grisés avec raison) | Calque | — | Ajoute / retire un effet (flou : calque.flou) | src/fxui.rs:230-256 |
| Effets ▸ Copier l'effet / Copier tous les effets du calque / Coller… / Remplacer les effets | Calque | — | Actions fx.copier, fx.copier_tout, fx.coller, fx.remplacer `[fx.copier]` | src/fxcopie.rs:523-553 |
| Effets ▸ Effets du calque allumés (case) | Calque | — | Coupe tous les effets sans les perdre | src/fxui.rs:262-270 |
| Effets ▸ Tremblement (WigglyPaint) / Animer à l'arrêt | Calque | — | Tremblement du trait du calque courant (boil) `[fx.tremblement]` | src/fxui.rs:276-300 |
| Ajouter un aplat / Retirer l'aplat ; Réglages de l'aplat ▸ | Calque | — | Aplat sous le trait (bouton sans action) | src/fondui.rs:187-196 |
| Plus ▸ Copier, coller ▸ Couper le calque | Calque | — | Couper le calque `[calque.couper]` | src/menucalque.rs:261 |
| … ▸ Copier le calque | Calque | `W` | Copier le calque `[calque.copier]` | src/menucalque.rs:262 |
| … ▸ Coller le calque | Calque | `X` | Coller le calque `[calque.coller]` | src/menucalque.rs:263 |
| … ▸ Coller à la place du calque courant | Calque | — | Coller le calque à la place du calque courant `[calque.coller_remplacer]` | src/menucalque.rs:264 |
| Plus ▸ Retourner le calque ▸ De gauche à droite | Calque | — | Retourner le calque de gauche à droite `[calque.miroir_h]` | src/menucalque.rs:267 |
| … ▸ De haut en bas | Calque | — | Retourner le calque de haut en bas `[calque.miroir_v]` | src/menucalque.rs:268 |
| Plus ▸ Origine ▸ (centre du dessin, bas au centre, coins, ici, centre de la feuille) | Calque | — | Actions origine.* `[origine.centre_dessin]` | src/menucalque.rs:272, src/origine.rs:413-430 |
| Plus ▸ Réglages avancés ▸ Préserver la transparence / Pochoir (cases) | Calque | — |  | src/menucalque.rs:332-337 |
| Plus ▸ Réglages avancés ▸ Accrocher à la caméra | Calque | — | Accrocher les calques choisis à la caméra (ou les décrocher) : ils gardent leur place dans l'image `[calque.accrocher_camera]` | src/menucalque.rs:344 |
| Plus ▸ Réglages avancés ▸ Profondeur (champ) | Calque | — | Profondeur du calque | src/layersui.rs:266-290 |
| Plus ▸ Réglages avancés ▸ Aimanté au sol (carte) | Calque | — | Aimanté au sol : l'origine du calque repose sur le sol de l'Espace 3D (oui / non) `[calque.aimante_au_sol]` | src/menucalque.rs:353 |
| Plus ▸ Réglages avancés ▸ Toujours devant le décor 3D (carte) | Calque | — | Toujours devant le décor 3D : aucun objet du décor ne cache la carte du calque (oui / non) `[calque.devant_decor_3d]` | src/menucalque.rs:362 |
| Plus ▸ Réglages avancés ▸ Porte une ombre / Reçoit les ombres | Calque | — | Et calque.recoit_ombres `[calque.porte_ombre]` | src/menucalque.rs:371-375 |
| Plus ▸ Réglages avancés ▸ Grille de repère (calque Sol, Mur, Ciel) | Calque | — |  | src/menucalque.rs:380 |
| Plus ▸ Animation (avant, après, automatismes) ▸ Avant la première image / Après la dernière image / Cassure automatique / Création automatique d'image | Calque | — | Comportements du calque | src/layerheads.rs:1154-1185 |
| Plus ▸ Calque vidéo ▸ Commence à l'image / Transformer en calque d'animation (calque vidéo) | Calque | — | Transformer le calque vidéo en calque d'animation `[calque.video_en_animation]` | src/videoui.rs:167-185 |
| Plus ▸ Calque de couleur ▸ Ajouter un calque de couleur sous ce trait / Griffonner / Pinceau dans la zone fermée / Fermer les trous / Sources supplémentaires / Détacher du calque de trait | Calque | — | Et calque.griffonner, calque.detacher_couleur `[calque.nouveau_couleur]` | src/calquecouleurui.rs:323-371 |
| Plus ▸ Propager la peinture… (calque bitmap, grisé avec raison) | Calque | — | Propager la peinture du calque en suivant une vidéo ou une animation (de clé en clé) `[calque.propager]` | src/montagehead.rs:124-147 |
| Plus ▸ Colorier par zones… | Calque | — | Colorier par zones (reporter les aplats des clés sur les autres images) `[calque.colorier_zones]` | src/coloriageui.rs:397-409 |
| Plus ▸ Choisir ▸ Tous les calques / Les calques visibles / Du même groupe / Des autres groupes / Aucun | Calque | — | Actions calque.choisir_* `[calque.choisir_tout]` | src/menucalque.rs:289-295 |
| Plus ▸ Afficher ▸ Tous les calques / Aucun calque / Le calque courant seul | Calque | — | Et calque.tous_caches, calque.courant_seul `[calque.tous_visibles]` | src/menucalque.rs:296-300 |
| Plus ▸ Fusionner plus de calques ▸ Les calques visibles / Tous les calques | Calque | — | Et calque.fusionner_tout `[calque.fusionner_visibles]` | src/menucalque.rs:301-310 |
| Plus ▸ Pochoirs ▸ Éteindre tous les pochoirs / Inverser tous les pochoirs | Calque | — | Et calque.pochoirs_inverser `[calque.pochoirs_eteindre]` | src/menucalque.rs:311-314 |
| Plus ▸ Nettoyer ▸ Supprimer les calques vides / cachés | Calque | — | Et calque.supprimer_caches `[calque.supprimer_vides]` | src/menucalque.rs:315-318 |
| Plus ▸ Aplat sur plusieurs calques ▸ Sur les calques choisis / Sur tout le plan / Retirer des calques choisis / Couleur du projet à tous les aplats | Calque | — | Et fond.tout_le_plan, fond.retirer_choisis, fond.couleur_partout `[fond.calques_choisis]` | src/fondui.rs:202-211 |
| Plus ▸ Calques dans la timeline ▸ Replier/Déplier cette piste, Replier toutes, Déplier toutes, Déplier le calque courant tout seul (depuis le montage seulement) | Calque | — | Et piste.deplier_tout, piste.auto_deplier `[piste.replier_tout]` | src/pistes.rs:1171-1184 |
| ＋ (Nouveau calque ▸ …) | Ligne « Calques » de la timeline (boutons et clic droit dans le vide) | — | Nouveau calque… (nom, sorte, groupe, mode, place) `[calque.nouveau]` | src/barrecalques.rs:60-70 |
| Nouveau dossier | Ligne « Calques » de la timeline (boutons et clic droit dans le vide) | — | Nouveau dossier de calques `[dossier.nouveau]` | src/barrecalques.rs |
| Calque d'effet ▸ préréglages | Ligne « Calques » de la timeline (boutons et clic droit dans le vide) | — |  | src/barrecalques.rs |
| Dupliquer | Ligne « Calques » de la timeline (boutons et clic droit dans le vide) | — | Dupliquer les calques choisis `[calque.dupliquer]` | src/barrecalques.rs |
| ☰ Opérations sur les calques ▸ (menu du calque courant) | Ligne « Calques » de la timeline (boutons et clic droit dans le vide) | — |  | src/barrecalques.rs:116 |
| Clic droit dans le vide ▸ Nouveau calque ▸ / Coller le calque | Ligne « Calques » de la timeline (boutons et clic droit dans le vide) | `X` | Coller le calque `[calque.coller]` | src/barrecalques.rs:54, 194 |
| Replier tous les calques sur une ligne | Ligne « Calques » de la timeline (boutons et clic droit dans le vide) | `Alt+Maj+C` | Colonne des calques de la timeline : replier tous les calques sur une ligne (ou les déplier) `[calques.replier_tout]` | src/layerheads.rs:48 |
| Copier l'effet / Copier tous les effets / Coller / Remplacer les effets | Panneau Effets : clic droit sur un effet | — | Copier l'effet choisi dans le panneau Effets (réglages et clés) `[fx.copier]` | src/fxui.rs:809, src/fxcopie.rs:523 |
| Intervertir les couleurs | Couleur : clic droit sur les carrés avant-plan / arrière-plan | — | Intervertir les couleurs d'avant-plan et d'arrière-plan (les deux carrés du panneau Couleur) `[couleur.intervertir]` | src/colorui.rs:384 |
| Couleurs par défaut (noir, blanc) | Couleur : clic droit sur les carrés avant-plan / arrière-plan | `D` | Couleurs par défaut (noir à l'avant-plan, blanc à l'arrière-plan) `[couleur.defaut]` | src/colorui.rs:385 |
| Copier le code de la couleur d'avant-plan | Couleur : clic droit sur les carrés avant-plan / arrière-plan | — | Copier le code hexadécimal de la couleur d'avant-plan `[couleur.copier_code]` | src/colorui.rs:387 |
| Ajouter la couleur d'avant-plan au nuancier | Couleur : clic droit sur les carrés avant-plan / arrière-plan | — | Ajouter au nuancier la couleur d'avant-plan (ou la couleur récente visée) `[nuancier.ajouter]` | src/colorui.rs:388 |
| Copier le code de la couleur d'avant-plan | Couleur : clic droit sur le champ hexadécimal | — | Copier le code hexadécimal de la couleur d'avant-plan `[couleur.copier_code]` | src/colorui.rs:411 |
| Carré saturation / valeur | Couleur : clic droit sur la roue | — | Roue chromatique : carré saturation / valeur `[couleur.roue_carre]` | src/colorui.rs:490 |
| Triangle saturation / valeur | Couleur : clic droit sur la roue | — | Roue chromatique : triangle saturation / valeur `[couleur.roue_triangle]` | src/colorui.rs:491 |
| Montrer / cacher la roue | Couleur : clic droit sur la roue | — | Roue chromatique (montrer / cacher) `[couleur.roue]` | src/colorui.rs:494 |
| Curseurs TSV (teinte, saturation, valeur) | Couleur : clic droit sur les curseurs | — | Curseurs teinte, saturation, valeur (TSV) `[couleur.curseurs_tsv]` | src/colorui.rs:628 |
| Curseurs RVB (rouge, vert, bleu) | Couleur : clic droit sur les curseurs | — | Curseurs rouge, vert, bleu (RVB) `[couleur.curseurs_rvb]` | src/colorui.rs:629 |
| Curseurs TSL (teinte, saturation, luminosité) | Couleur : clic droit sur les curseurs | — | Curseurs teinte, saturation, luminosité (TSL) `[couleur.curseurs_tsl]` | src/colorui.rs:630 |
| Couleur d'avant-plan | Couleur : clic droit sur une pastille (récente, nuancier) | — | La pastille visée (menu ouvert, sinon sous la souris) devient la couleur d'avant-plan `[pastille.avant_plan]` | src/colorui.rs:681 |
| Couleur d'arrière-plan | Couleur : clic droit sur une pastille (récente, nuancier) | — | La pastille visée (menu ouvert, sinon sous la souris) devient la couleur d'arrière-plan `[pastille.arriere_plan]` | src/colorui.rs:682 |
| Ajouter au nuancier (récente) | Couleur : clic droit sur une pastille (récente, nuancier) | — | Ajouter au nuancier la couleur d'avant-plan (ou la couleur récente visée) `[nuancier.ajouter]` | src/colorui.rs:685 |
| Effacer les couleurs récentes (récente) | Couleur : clic droit sur une pastille (récente, nuancier) | — | Effacer les couleurs récentes `[couleur.recentes_effacer]` | src/colorui.rs:687, 644 |
| (pastille du nuancier : nommer, remplacer, retirer) | Couleur : clic droit sur une pastille (récente, nuancier) | — | Et pastille.remplacer, pastille.retirer `[pastille.renommer]` | src/colorui.rs:689 |
| Ajouter la couleur d'avant-plan | Nuancier : clic droit sur son en-tête | — | Ajouter au nuancier la couleur d'avant-plan (ou la couleur récente visée) `[nuancier.ajouter]` | src/nuancierui.rs:202 |
| Nouveau nuancier | Nuancier : clic droit sur son en-tête | — | Nouveau nuancier `[nuancier.nouveau]` | src/nuancierui.rs:204 |
| Renommer le nuancier… | Nuancier : clic droit sur son en-tête | — | Renommer le nuancier montré `[nuancier.renommer]` | src/nuancierui.rs:205 |
| Dupliquer le nuancier | Nuancier : clic droit sur son en-tête | — | Dupliquer le nuancier montré `[nuancier.dupliquer]` | src/nuancierui.rs:206 |
| Supprimer le nuancier… | Nuancier : clic droit sur son en-tête | — | Supprimer le nuancier montré (après confirmation) `[nuancier.supprimer]` | src/nuancierui.rs:207 |
| Importer un nuancier (.gpl, .aco)… | Nuancier : clic droit sur son en-tête | — | Importer un nuancier (.gpl, .aco) `[nuancier.importer]` | src/nuancierui.rs:209 |
| Exporter le nuancier (.gpl, .aco)… | Nuancier : clic droit sur son en-tête | — | Exporter le nuancier montré (.gpl, .aco) `[nuancier.exporter]` | src/nuancierui.rs:210 |
| Supprimer | Dégradés enregistrés : clic droit | — | Retire le dégradé enregistré | src/degradeui.rs:1116 |
| (menu de la brosse) | Liste des brosses : clic droit sur une brosse | — | Voir `pinceauxui.rs` (hors de ma part : réglages) | src/pinceauxui.rs:286, 304 |
| Ajuster à l'écran / Taille réelle (100 %) / Écrire dans le bloc | Références : clic droit sur un élément | — |  | src/planchrefui.rs:690-698 |
| Mettre devant | Références : clic droit sur un élément | — | Références : mettre devant `[references.devant]` | src/planchrefui.rs:703 |
| Mettre derrière | Références : clic droit sur un élément | — | Références : mettre derrière `[references.derriere]` | src/planchrefui.rs:707 |
| Ranger | Références : clic droit sur un élément | — | Références : ranger la sélection (ou toute la planche), serré `[references.ranger]` | src/planchrefui.rs:712 |
| Mettre dans un cadre | Références : clic droit sur un élément | — | Références : poser un cadre (autour de la sélection s'il y en a une) `[references.cadre]` | src/planchrefui.rs:717 |
| Recadrer | Références : clic droit sur un élément | — | Références : recadrer l'image choisie (oui / non) `[references.recadrer]` | src/planchrefui.rs:723 |
| Rétablir l'image entière | Références : clic droit sur un élément | — |  | src/planchrefui.rs:727 |
| Retourner de gauche à droite | Références : clic droit sur un élément | — | Références : retourner de gauche à droite `[references.retourner_h]` | src/planchrefui.rs:731 |
| Retourner de haut en bas | Références : clic droit sur un élément | — | Références : retourner de haut en bas `[references.retourner_v]` | src/planchrefui.rs:735 |
| Noir et blanc (oui / non) | Références : clic droit sur un élément | — | Références : noir et blanc (oui / non) `[references.noir_blanc]` | src/planchrefui.rs:739 |
| Opacité ▸ % / Couleur ▸ / Remettre droit | Références : clic droit sur un élément | — |  | src/planchrefui.rs:744-767 |
| Lier au plan courant (ou délier) | Références : clic droit sur un élément | — | Lier les références choisies au plan courant (ou délier) `[references.lier_plan]` | src/planchrefui.rs:771 |
| Supprimer | Références : clic droit sur un élément | — | Supprimer les références choisies `[references.supprimer]` | src/planchrefui.rs:775 |
| Post-it ici / Texte ici / Titre ici / Cadre ici | Références : clic droit sur le fond | — | Et references.texte, references.titre, references.cadre (posés au pointeur) `[references.postit]` | src/planchrefui.rs:781-793 |
| Ranger toute la planche | Références : clic droit sur le fond | — | Références : ranger la sélection (ou toute la planche), serré `[references.ranger]` | src/planchrefui.rs:798 |
| Tout voir | Références : clic droit sur le fond | — | Références : tout voir sur le tableau `[references.tout_voir]` | src/planchrefui.rs:802 |
| Tout sélectionner | Références : clic droit sur le fond | — | Références : tout sélectionner `[references.tout_selectionner]` | src/planchrefui.rs:806 |
| Importer des images… | Références : clic droit sur le fond | — | Importer des images dans les références `[references.importer_image]` | src/planchrefui.rs:811 |
| Importer une vidéo… | Références : clic droit sur le fond | — | Importer une vidéo dans les références `[references.importer_video]` | src/planchrefui.rs:815 |
| Coller une image | Références : clic droit sur le fond | — | Coller l'image du presse-papiers dans les références `[references.coller]` | src/planchrefui.rs:819 |
| Importer la planche d'un autre projet… | Références : clic droit sur le fond | — | Références : importer la planche d'un autre projet… `[references.importer_planche]` | src/planchrefui.rs:823 |
| Dans une fenêtre à part / Panneau flottant / Sortir l'onglet en panneau / Déplier-Replier / Fermer le panneau | Colonne de panneaux : clic droit sur un en-tête | — | Rangement des panneaux (détail : autre agent) | src/colonneui.rs:340-360, 384 |
| Panneau flottant : Dans la colonne / Dans une fenêtre à part | Colonne de panneaux : clic droit sur un en-tête | — |  | src/dockui.rs:78 |
| Accueil : un projet récent ▸ Retirer de la liste | Divers clic droit | — | Le retire des récents | src/accueil.rs:248 |
| Bibliothèque : favori ▸ Retirer des favoris ; fichier ▸ menu du fichier | Divers clic droit | — |  | src/biblioui.rs:737, 1044 |
| Blocs de caméra (Storyboard) ▸ Retirer cet effet / Convertir les effets en clés | Divers clic droit | — |  | src/blocscamera.rs:419 |
| Chemin de la caméra ▸ La caméra garde son sujet dans le cadre / Retirer le chemin de la caméra | Divers clic droit | — |  | src/blocscamera.rs:536 |
| Objet du décor 3D ▸ Copier vers la séquence ▸ / Déplacer vers la séquence ▸ | Divers clic droit | — |  | src/seqdecor3d.rs:154 |
| Scène (Storyboard) : caméra ▸ son menu ; calque ▸ menu du calque | Divers clic droit | — |  | src/sceneui.rs:245-347 |
| Version de séquence (barre des variantes) ▸ Renommer… / Comparer / Supprimer la version de séquence… | Divers clic droit | — |  | src/variantesui.rs:377-391 |
| Fiche, versions ▸ Mettre à jour… / Comparer le plan… / Passer à / Comparer à la version d'aujourd'hui / Restaurer / Supprimer | Divers clic droit | — |  | src/panneauxvisuelsui.rs:172-215 |
| Toute commande ▸ Assigner un raccourci… / Ajouter à Mon panneau (si ouvert) / Revoir l'explication | Divers clic droit | — | Assignation de touche | src/raccourcisui.rs:41 |
| Mon panneau ▸ Retirer / Assigner un raccourci… / Renommer… / Ajouter un séparateur | Divers clic droit | — |  | src/monpanneauui.rs:424-437 |

## 8. Palette de commandes (Ctrl+K) : toutes les commandes connues du code

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
La palette propose toutes les actions de `ACTIONS` marquées « faites », sauf les doubles (`ALIASES`) ; hors de la vue Espace 3D, elle cache aussi celles dont le nom parle de la 3D (`src/palette.rs:84-105`, `src/keymap.rs:831-834`). Flèches haut / bas : choisir ; Entrée ou clic : lancer ; Échap : fermer ; 14 résultats au plus (`src/palette.rs:108-182`). On peut taper l'identifiant ou un ancien nom (`src/palette.rs:30-43`). La colonne « Où » donne aussi les autres endroits du code où la même action est posée sous un libellé (fichier:ligne « libellé »).

| Libellé à l'écran (palette) | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| Nouveau projet | Palette ; aussi : barres.rs:154 « Nouveau projet » ; panels.rs:119 « Nouveau… » | `Ctrl+N` | Nouveau projet `[fichier.nouveau]` | src/keymap.rs:58 |
| Ouvrir | Palette ; aussi : barres.rs:155 « Ouvrir » ; panels.rs:120 « Ouvrir… » | `Ctrl+O` | Ouvrir `[fichier.ouvrir]` | src/keymap.rs:59 |
| Enregistrer, ou poser un jalon | Palette ; aussi : barres.rs:156 « Enregistrer » | `Ctrl+S` | Enregistrer, ou poser un jalon `[fichier.enregistrer]` | src/keymap.rs:60 |
| Jalons du projet | Palette ; aussi : panels.rs:125 « Jalons… » | — | Jalons du projet `[fichier.jalons]` | src/keymap.rs:61 |
| Enregistrer sous | Palette ; aussi : panels.rs:126 « Enregistrer sous… » | `Ctrl+Maj+S` | Enregistrer sous `[fichier.enregistrer_sous]` | src/keymap.rs:62 |
| Afficher le projet dans l'Explorateur Windows | Palette ; aussi : panels.rs:127 « Afficher le projet dans l'Explorateur » | — | Afficher le projet dans l'Explorateur Windows `[fichier.afficher_explorateur]` | src/keymap.rs:63 |
| Fermer le projet (l'onglet) | Palette ; aussi : panels.rs:129 « Fermer l'onglet » | `Ctrl+W` | Fermer le projet (l'onglet) `[fichier.fermer]` | src/keymap.rs:64 |
| Nouvel onglet | Palette ; aussi : panels.rs:128 « Nouvel onglet » | `Ctrl+T` | Nouvel onglet `[onglet.nouveau]` | src/keymap.rs:65 |
| Onglet suivant | Palette | `Ctrl+Tab` | Onglet suivant `[onglet.suivant]` | src/keymap.rs:66 |
| Onglet précédent | Palette | `Ctrl+Maj+Tab` | Onglet précédent `[onglet.precedent]` | src/keymap.rs:67 |
| Fermer les autres onglets | Palette ; aussi : panels.rs:130 « Fermer les autres onglets » | — | Fermer les autres onglets `[onglet.fermer_autres]` | src/keymap.rs:68 |
| Quitter | Palette ; aussi : panels.rs:163 « Quitter » | `Ctrl+Q` | Quitter `[fichier.quitter]` | src/keymap.rs:69 |
| Exporter le film (vidéo) | Palette ; aussi : barres.rs:157 « Exporter le film » ; exportmenu.rs:22 « Film (MP4, ProRes, DNxHR)… » | — | Exporter le film (vidéo) `[fichier.exporter_film]` | src/keymap.rs:70 |
| Exporter des images (PNG) | Palette ; aussi : exportmenu.rs:30 « Une image par image du film… » | — | Exporter des images (PNG) `[fichier.exporter_images]` | src/keymap.rs:71 |
| Exporter le son (WAV) | Palette ; aussi : exportmenu.rs:42 « Son mixé… » | — | Exporter le son (WAV) `[fichier.exporter_son]` | src/keymap.rs:72 |
| Exporter le storyboard (PDF) | Palette ; aussi : exportmenu.rs:34 « Storyboard en PDF… » | — | Exporter le storyboard (PDF) `[fichier.exporter_pdf]` | src/keymap.rs:73 |
| Exporter un GIF animé (le film, le plan courant ou les plans choisis) | Palette ; aussi : exportmenu.rs:23 « GIF animé… » | — | Exporter un GIF animé (le film, le plan courant ou les plans choisis) `[fichier.exporter_gif]` | src/keymap.rs:74 |
| Exporter pour le montage (EDL, XML de montage, OpenTimelineIO) | Palette ; aussi : barres.rs:262 « Exporter pour le montage » ; exportmenu.rs:50 « Pour le montage (EDL, XML, FCPXML, OpenTimelineIO)… » | — | Exporter pour le montage (EDL, XML de montage, OpenTimelineIO) `[fichier.exporter_montage]` | src/keymap.rs:75 |
| Exporter la fiche technique (tableur CSV ou XLSX, un plan par ligne) | Palette ; aussi : barres.rs:263 « Exporter la fiche technique » ; exportmenu.rs:54 « Fiche technique (XLSX, CSV)… » | — | Exporter la fiche technique (tableur CSV ou XLSX, un plan par ligne) `[fichier.exporter_fiche]` | src/keymap.rs:76 |
| Exporter la liste des plans pour le suivi de production (CSV ou JSON, profil Kitsu ou générique) | Palette ; aussi : exportmenu.rs:55 « Liste des plans (CSV, JSON)… » | — | Exporter la liste des plans pour le suivi de production (CSV ou JSON, profil Kitsu ou générique) `[fichier.exporter_liste_plans]` | src/keymap.rs:77 |
| Exporter le paquet de livraison (vidéos par plan, vignettes, PDF, animatique, liste des plans et manifeste, rangés dans un dossier) | Palette ; aussi : exportmenu.rs:60 « Paquet de livraison… » | — | Exporter le paquet de livraison (vidéos par plan, vignettes, PDF, animatique, liste des plans et manifeste, rangés dans un dossier) `[fichier.exporter_paquet]` | src/keymap.rs:78 |
| Exporter le rapport de numérotation pour le monteur (plans ajoutés, supprimés, déplacés, renumérotés depuis le dernier envoi) | Palette ; aussi : exportmenu.rs:56 « Rapport de numérotation… » | — | Exporter le rapport de numérotation pour le monteur (plans ajoutés, supprimés, déplacés, renumérotés depuis le dernier envoi) `[fichier.exporter_renumerotation]` | src/keymap.rs:79 |
| Pont Kitsu : publier les plans dans Kitsu (créer, mettre à jour, prévisualiser) | Palette ; aussi : exportmenu.rs:61 « Kitsu… » | — | Pont Kitsu : publier les plans dans Kitsu (créer, mettre à jour, prévisualiser) `[fichier.kitsu_publier]` | src/keymap.rs:80 |
| Pont Kitsu : lire les statuts et les commentaires de Kitsu (notes du fil de décision) | Palette | — | Pont Kitsu : lire les statuts et les commentaires de Kitsu (notes du fil de décision) `[fichier.kitsu_retours]` | src/keymap.rs:81 |
| Noms des plans et production (modèle de noms, série, épisode) | Palette ; aussi : exportmenu.rs:63 « Noms des plans, série et épisode… » | — | Noms des plans et production (modèle de noms, série, épisode) `[projet.noms_production]` | src/keymap.rs:82 |
| Enregistrer comme modèle d'épisode… | Palette | — | Enregistrer comme modèle d'épisode… `[projet.enregistrer_modele_episode]` | src/keymap.rs:83 |
| Exporter pour relecture (page HTML à envoyer au réalisateur) | Palette ; aussi : barres.rs:265 « Exporter pour relecture » ; exportmenu.rs:58 « Relecture (la page à envoyer au réalisateur)… » | — | Exporter pour relecture (page HTML à envoyer au réalisateur) `[fichier.exporter_relecture]` | src/keymap.rs:84 |
| Exporter la planche des vignettes (PNG, JPEG ou PDF) | Palette ; aussi : annotationsui.rs:991 « Exporter la planche des vignettes… » ; exportmenu.rs:35 « Planche des vignettes… » | — | Exporter la planche des vignettes (PNG, JPEG ou PDF) `[fichier.exporter_planche]` | src/keymap.rs:84 |
| Exporter le color script (une petite image par plan, rangées par séquence) | Palette ; aussi : exportmenu.rs:36 « Color script… » | — | Exporter le color script (une petite image par plan, rangées par séquence) `[fichier.exporter_colorscript]` | src/keymap.rs:85 |
| Exporter les calques séparés (un dossier par plan, une suite d'images transparentes par calque) | Palette ; aussi : exportmenu.rs:51 « Calques séparés… » | — | Exporter les calques séparés (un dossier par plan, une suite d'images transparentes par calque) `[fichier.exporter_calques]` | src/keymap.rs:86 |
| Exporter les clés de caméra (JSON et CSV, pour un script de compositing) | Palette ; aussi : exportmenu.rs:52 « Caméra (JSON, CSV)… » | — | Exporter les clés de caméra (JSON et CSV, pour un script de compositing) `[fichier.exporter_camera]` | src/keymap.rs:87 |
| Exporter le son par piste (un fichier WAV ou AIFF par piste audible, avec ou sans le mixage) | Palette ; aussi : exportmenu.rs:43 « Un son par piste… » | — | Exporter le son par piste (un fichier WAV ou AIFF par piste audible, avec ou sans le mixage) `[fichier.exporter_son_par_piste]` | src/keymap.rs:88 |
| Exporter des images à la clé (une image par dessin clé, numérotée, composée ou par calque) | Palette ; aussi : exportmenu.rs:32 « Une image par dessin clé… » | — | Exporter des images à la clé (une image par dessin clé, numérotée, composée ou par calque) `[fichier.exporter_images_cles]` | src/keymap.rs:89 |
| Exporter le son par plan (un fichier WAV ou AIFF par plan, le mixage de ce qu'on y entend) | Palette ; aussi : exportmenu.rs:44 « Un son par plan… » | — | Exporter le son par plan (un fichier WAV ou AIFF par plan, le mixage de ce qu'on y entend) `[fichier.exporter_son_par_plan]` | src/keymap.rs:90 |
| Exporter des images par plan (une image par plan ou par case, en PNG ou en JPEG) | Palette ; aussi : exportmenu.rs:31 « Une image par plan… » | — | Exporter des images par plan (une image par plan ou par case, en PNG ou en JPEG) `[fichier.exporter_images_par_plan]` | src/keymap.rs:91 |
| Vignettes : tenir pour tirer les cadres de caméra (maintenir ; comme Ctrl + Maj) | Palette | `T` | Vignettes : tenir pour tirer les cadres de caméra (maintenir ; comme Ctrl + Maj) `[vignettes.camera]` | src/keymap.rs:92 |
| Vignettes : stylo d'annotation (par-dessus la planche) | Palette ; aussi : annotationsui.rs:978 « Stylo d'annotation » | — | Vignettes : stylo d'annotation (par-dessus la planche) `[vignettes.annotation_stylo]` | src/keymap.rs:93 |
| Vignettes : gomme d'annotation | Palette ; aussi : annotationsui.rs:979 « Gomme d'annotation » | — | Vignettes : gomme d'annotation `[vignettes.annotation_gomme]` | src/keymap.rs:94 |
| Vignettes : lasso d'annotation (choisir, déplacer, agrandir, tourner des traits) | Palette ; aussi : annotationsui.rs:980 « Lasso d'annotation » | — | Vignettes : lasso d'annotation (choisir, déplacer, agrandir, tourner des traits) `[vignettes.annotation_lasso]` | src/keymap.rs:95 |
| Vignettes : nouveau calque d'annotation | Palette ; aussi : annotationsui.rs:981 « Nouveau calque d'annotation » | — | Vignettes : nouveau calque d'annotation `[vignettes.annotation_calque_nouveau]` | src/keymap.rs:96 |
| Vignettes : cacher ou montrer les annotations | Palette ; aussi : annotationsui.rs:957 « Cacher les annotations » | — | Vignettes : cacher ou montrer les annotations `[vignettes.annotations_afficher]` | src/keymap.rs:97 |
| Vignettes : effacer les annotations | Palette | — | Vignettes : effacer les annotations `[vignettes.annotations_effacer]` | src/keymap.rs:98 |
| Vignettes : marie-louise (ne montrer que les vignettes) | Palette ; aussi : annotationsui.rs:965 « Marie-louise » ; annotationsui.rs:989 « Retirer la marie-louise » ; vignettesprefs.rs:103 « Passe-partout des cases » | — | Vignettes : marie-louise (ne montrer que les vignettes) `[vignettes.marie_louise]` | src/keymap.rs:99 |
| Vignettes : écrire la note de la case courante | Palette ; aussi : boardui.rs:657 « Écrire la note » | — | Vignettes : écrire la note de la case courante `[vignettes.note]` | src/keymap.rs:100 |
| Importer les notes de relecture (fichier .json) | Palette ; aussi : barres.rs:266 « Importer les notes de relecture » ; panels.rs:147 « Des notes de relecture (.json)… » | — | Importer les notes de relecture (fichier .json) `[fichier.importer_notes]` | src/keymap.rs:101 |
| Verrouiller / déverrouiller la numérotation des plans | Palette | — | Verrouiller / déverrouiller la numérotation des plans `[numerotation.verrouiller]` | src/keymap.rs:102 |
| Renuméroter les plans (1, 2, 3… dans l'ordre du film) | Palette ; aussi : panels.rs:223 « Renuméroter les plans… » | — | Renuméroter les plans (1, 2, 3… dans l'ordre du film) `[numerotation.renumeroter]` | src/keymap.rs:103 |
| Exporter l'image courante (PNG) | Palette ; aussi : exportmenu.rs:29 « Image courante… » | — | Exporter l'image courante (PNG) `[fichier.exporter_image_courante]` | src/keymap.rs:104 |
| Exporter (choisir le format : film, images, PDF, son…) | Palette ; aussi : prefsui.rs:827 « Ouvrir la fenêtre Exporter » | — | Exporter (choisir le format : film, images, PDF, son…) `[fichier.exporter]` | src/keymap.rs:105 |
| Corbeille du projet (restaurer ce qui est sorti du film, vider) | Palette ; aussi : panels.rs:154 « Corbeille… » | — | Corbeille du projet (restaurer ce qui est sorti du film, vider) `[fichier.corbeille]` | src/keymap.rs:106 |
| Préférences de FEANOR (copie de secours, curseur, panneaux, raccourcis) | Palette ; aussi : panels.rs:161 « Préférences… » | — | Préférences de FEANOR (copie de secours, curseur, panneaux, raccourcis) `[fichier.preferences]` | src/keymap.rs:107 |
| Annuler | Palette ; aussi : barres.rs:158 « Annuler » ; canvas.rs:383 « Annuler » ; concentration.rs:91 « Annuler » ; historyui.rs:30 « Annuler » ; panels.rs:166 « Annuler » | `Ctrl+Z` | Annuler `[edition.annuler]` | src/keymap.rs:108 |
| Rétablir | Palette ; aussi : barres.rs:159 « Rétablir » ; canvas.rs:384 « Rétablir » ; concentration.rs:88 « Rétablir » ; historyui.rs:31 « Rétablir » ; panels.rs:167 « Rétablir » | `Ctrl+Maj+Z` ou `Ctrl+Y` | Rétablir `[edition.retablir]` | src/keymap.rs:109 |
| Couper | Palette ; aussi : barres.rs:160 « Couper » ; canvas.rs:386 « Couper » ; panels.rs:170 « Couper » | `Ctrl+X` | Couper `[edition.couper]` | src/keymap.rs:110 |
| Copier | Palette ; aussi : barres.rs:161 « Copier » ; barres.rs:177 « Copier les plans » ; canvas.rs:387 « Copier » ; panels.rs:171 « Copier » | `Ctrl+C` | Copier `[edition.copier]` | src/keymap.rs:111 |
| Coller | Palette ; aussi : barres.rs:162 « Coller » ; barres.rs:178 « Coller les plans » ; canvas.rs:388 « Coller » ; panels.rs:172 « Coller » | `Ctrl+V` | Coller `[edition.coller]` | src/keymap.rs:112 |
| Coller sur place | Palette | — | Coller sur place `[edition.coller_sur_place]` | src/keymap.rs:113 |
| Coller à la tête de lecture | Palette ; aussi : panels.rs:175 « Coller à la tête de lecture (clés) » | `Ctrl+Maj+V` | Coller à la tête de lecture `[edition.coller_a_la_tete]` | src/keymap.rs:114 |
| Supprimer la sélection (rien sans sélection) | Palette ; aussi : barres.rs:179 « Supprimer les plans » ; canvas.rs:389 « Supprimer la sélection » ; panels.rs:176 « Supprimer » | `Suppr` ou `Retour arrière` | Supprimer la sélection (rien sans sélection) `[edition.supprimer]` | src/keymap.rs:115 |
| Vider le dessin de l'image courante | Palette ; aussi : barres.rs:311 « Vider le dessin » ; panels.rs:177 « Vider le dessin » | — | Vider le dessin de l'image courante `[dessin.vider]` | src/keymap.rs:116 |
| Supprimer et resserrer | absent de la palette : **non implémenté** | — | Supprimer et resserrer — **non implémenté** `[edition.supprimer_et_resserrer]` | src/keymap.rs:117 |
| Tout sélectionner | Palette ; aussi : multitools.rs:958 « Tout sélectionner » ; panels.rs:179 « Tout sélectionner » | `Ctrl+A` | Tout sélectionner `[edition.tout_selectionner]` | src/keymap.rs:118 |
| Sélectionner tous les dessins du plan | absent de la palette : **non implémenté** | `Ctrl+Maj+A` *en réserve, inactif* | Sélectionner tous les dessins du plan — **non implémenté** `[edition.selectionner_dessins_scene]` | src/keymap.rs:119 |
| Tout désélectionner | Palette ; aussi : panels.rs:180 « Tout désélectionner » | `Échap` | Tout désélectionner `[edition.tout_deselectionner]` | src/keymap.rs:120 |
| Outil zoom | absent de la palette : **non implémenté** | `Alt+Z` *en réserve, inactif* | Outil zoom — **non implémenté** `[vue.outil_zoom]` | src/keymap.rs:121 |
| Zoom avant | Palette ; aussi : viewbar.rs:320 « Zoom avant » | `2` | Zoom avant `[vue.zoom_avant]` | src/keymap.rs:122 |
| Zoom arrière | Palette ; aussi : viewbar.rs:311 « Zoom arrière » | `1` | Zoom arrière `[vue.zoom_arriere]` | src/keymap.rs:123 |
| Plein écran | Palette ; aussi : panels.rs:281 « Plein écran » | `Ctrl+Maj+Alt+F` | Plein écran `[vue.plein_ecran]` | src/keymap.rs:124 |
| Tourner la vue (sens horaire) | Palette ; aussi : viewbar.rs:352 « Tourner la vue » | `V` | Tourner la vue (sens horaire) `[vue.rotation_horaire]` | src/keymap.rs:125 |
| Tourner la vue (sens antihoraire) | Palette ; aussi : viewbar.rs:343 « Tourner la vue » | `C` | Tourner la vue (sens antihoraire) `[vue.rotation_antihoraire]` | src/keymap.rs:126 |
| Remettre la vue droite | Palette ; aussi : panels.rs:257 « Remettre la vue droite » ; viewbar.rs:473 « Remettre la vue droite » | `Maj+X` | Remettre la vue droite `[vue.reinit_rotation]` | src/keymap.rs:127 |
| Zoom 100 % | Palette ; aussi : panels.rs:256 « Zoom 100 % » | `Maj+Z` | Zoom 100 % `[vue.reinit_zoom]` | src/keymap.rs:128 |
| Ajuster à la fenêtre | Palette ; aussi : canvas.rs:391 « Ajuster à la fenêtre » ; panels.rs:255 « Ajuster à la fenêtre » ; viewbar.rs:514 « Ajuster à la fenêtre » | `Ctrl+0` | Ajuster à la fenêtre `[vue.ajuster]` | src/keymap.rs:129 |
| Déplacer la vue (maintenir) | Palette | `Espace` | Déplacer la vue (maintenir) `[vue.pan_libre]` | src/keymap.rs:130 |
| Rotation temporaire (maintenir) | absent de la palette : **non implémenté** | `Ctrl+Alt` *en réserve, inactif* | Rotation temporaire (maintenir) — **non implémenté** `[vue.rotation_temporaire]` | src/keymap.rs:131 |
| Zoom avant temporaire (maintenir) | absent de la palette : **non implémenté** | `Ctrl+Espace` *en réserve, inactif* | Zoom avant temporaire (maintenir) — **non implémenté** `[vue.zoom_temporaire_avant]` | src/keymap.rs:132 |
| Zoom arrière temporaire (maintenir) | absent de la palette : **non implémenté** | `Alt+Espace` *en réserve, inactif* | Zoom arrière temporaire (maintenir) — **non implémenté** `[vue.zoom_temporaire_arriere]` | src/keymap.rs:133 |
| Afficher la grille | absent de la palette : **non implémenté** | `Ctrl+G` *en réserve, inactif* | Afficher la grille — **non implémenté** `[vue.grille]` | src/keymap.rs:134 |
| Pelure d'oignon | Palette ; aussi : barres.rs:350 « Pelure d'oignon » ; onionui.rs:83 « Pelure d'oignon » ; viewbar.rs:366 « Pelure d'oignon » | `Alt+O` | Pelure d'oignon `[vue.pelure]` | src/keymap.rs:135 |
| Miroir de la vue | Palette ; aussi : panels.rs:611 « Miroir de la vue » ; viewbar.rs:517 « Miroir de la vue » | `M` | Miroir de la vue `[vue.miroir]` | src/keymap.rs:136 |
| Vignettes et Storyboard : passer de l'un à l'autre (la planche, le dessin du plan) | Palette | `Maj+Tab` | Vignettes et Storyboard : passer de l'un à l'autre (la planche, le dessin du plan) `[vue.planches]` | src/keymap.rs:137 |
| Entrer dans le plan pour l'animer, et en revenir | Palette ; aussi : barres.rs:286 « Animer ce plan » ; panels.rs:348 « Storyboard » | `Tab` | Entrer dans le plan pour l'animer, et en revenir `[mode.basculer]` | src/keymap.rs:138 |
| Liste des raccourcis | Palette ; aussi : prefsui.rs:732 « Voir et changer les raccourcis » | `F1` | Liste des raccourcis `[aide.raccourcis]` | src/keymap.rs:139 |
| À propos de FEANOR (version, logo) | Palette ; aussi : panels.rs:316 « À propos de FEANOR… » | — | À propos de FEANOR (version, logo) `[aide.a_propos]` | src/keymap.rs:140 |
| Régler FEANOR pour cette machine (Diagnostic des performances : détecter le matériel, mesurer, proposer des réglages) | Palette ; aussi : diagnosticui.rs:590 « Diagnostic des performances… » ; panels.rs:309 « Régler FEANOR pour cette machine… » | — | Régler FEANOR pour cette machine (Diagnostic des performances : détecter le matériel, mesurer, proposer des réglages) `[aide.diagnostic]` | src/keymap.rs:141 |
| Palette de commandes : chercher une commande par son nom | Palette ; aussi : panels.rs:302 « Palette de commandes… » | `Ctrl+K` | Palette de commandes : chercher une commande par son nom `[aide.palette]` | src/keymap.rs:142 |
| Mode concentration : ne garder que le dessin (la même touche remet tout) | Palette ; aussi : panels.rs:280 « Mode concentration (le dessin seul) » | `F11` | Mode concentration : ne garder que le dessin (la même touche remet tout) `[vue.concentration]` | src/keymap.rs:144 |
| Aide-mémoire des raccourcis (maintenir) | Palette | `R` | Aide-mémoire des raccourcis (maintenir) `[aide.aide_memoire]` | src/keymap.rs:145 |
| Revoir les conseils | Palette ; aussi : conseils.rs:202 « Revoir les conseils » ; panels.rs:306 « Revoir les conseils » | — | Revoir les conseils `[aide.conseils]` | src/keymap.rs:146 |
| Visite rapide « Premier jour » : créer un projet, dessiner, mettre une durée, écouter, exporter | Palette ; aussi : panels.rs:305 « Visite rapide « Premier jour »… » ; premierjour.rs:342 « Visite rapide « Premier jour » » | — | Visite rapide « Premier jour » : créer un projet, dessiner, mettre une durée, écouter, exporter `[aide.visite]` | src/keymap.rs:147 |
| Régler le stylet à ma main | Palette ; aussi : prefsui.rs:786 « Régler le stylet à ma main » | — | Régler le stylet à ma main `[aide.regler_stylet]` | src/keymap.rs:148 |
| Ce que le stylet transmet : appui, inclinaison, bouton et gomme du stylet, lus auprès de Windows (banc d'essai) | Palette | — | Ce que le stylet transmet : appui, inclinaison, bouton et gomme du stylet, lus auprès de Windows (banc d'essai) `[aide.stylet_transmet]` | src/keymap.rs:150 |
| Noter une friction : ce qui vient de me freiner, avec une capture de la fenêtre et mes dernières commandes | Palette | `F9` | Noter une friction : ce qui vient de me freiner, avec une capture de la fenêtre et mes dernières commandes `[aide.noter_friction]` | src/keymap.rs:152 |
| Mes frictions notées : ouvrir leur dossier | Palette | — | Mes frictions notées : ouvrir leur dossier `[aide.ouvrir_frictions]` | src/keymap.rs:153 |
| Mes habitudes : les commandes que j'utilise le plus et celles qui ne servent jamais (compteur du poste) | Palette ; aussi : panels.rs:314 « Mes habitudes… » | — | Mes habitudes : les commandes que j'utilise le plus et celles qui ne servent jamais (compteur du poste) `[aide.habitudes]` | src/keymap.rs:154 |
| Explications animées : ouvrir le registre des animations des outils (menu Aide) | Palette | — | Explications animées : ouvrir le registre des animations des outils (menu Aide) `[aide.explications]` | src/keymap.rs:156 |
| Registre des explications : toutes les animations des outils et les textes des fenêtres | Palette ; aussi : aideanim.rs:609 « Registre des explications… » | — | Registre des explications : toutes les animations des outils et les textes des fenêtres `[aide.registre]` | src/keymap.rs:157 |
| Outil pinceau | Palette | `Alt+B` | Outil pinceau `[outil.pinceau]` | src/keymap.rs:158 |
| Outil crayon | Palette | `Alt+P` | Outil crayon `[outil.crayon]` | src/keymap.rs:159 |
| Outil gomme | Palette | `Alt+E` | Outil gomme `[outil.gomme]` | src/keymap.rs:160 |
| Outil remplissage | Palette | `Alt+I` | Outil remplissage `[outil.remplissage]` | src/keymap.rs:161 |
| Outil dégradé (linéaire ou radial, bitmap et vectoriel) | Palette | `Alt+G` | Outil dégradé (linéaire ou radial, bitmap et vectoriel) `[outil.degrade]` | src/keymap.rs:162 |
| Outil mélangeur (étaler et mêler les couleurs déjà peintes, calque bitmap) | Palette | — | Outil mélangeur (étaler et mêler les couleurs déjà peintes, calque bitmap) `[outil.melangeur]` | src/keymap.rs:163 |
| Outil peindre les zones non peintes | absent de la palette : **non implémenté** | `Alt+Y` *en réserve, inactif* | Outil peindre les zones non peintes — **non implémenté** `[outil.peindre_non_peint]` | src/keymap.rs:164 |
| Outil pipette | Palette | `Alt+D` | Outil pipette `[outil.pipette]` | src/keymap.rs:165 |
| Outil sélection | Palette | `Alt+S` | Outil sélection `[outil.selection]` | src/keymap.rs:166 |
| Outil sélection par couleur | Palette | — | Outil sélection par couleur `[outil.selection_couleur]` | src/keymap.rs:167 |
| Pinceau et crayon : peindre derrière (oui / non) | Palette | — | Pinceau et crayon : peindre derrière (oui / non) `[pinceau.derriere]` | src/keymap.rs:168 |
| Pinceau et crayon : peint sur l'aplat (oui / non) | Palette | — | Pinceau et crayon : peint sur l'aplat (oui / non) `[pinceau.sur_aplat]` | src/keymap.rs:169 |
| Aplat sous le trait du calque courant (ajouter / retirer) | Palette | — | Aplat sous le trait du calque courant (ajouter / retirer) `[pinceau.fond]` | src/keymap.rs:170 |
| Aplat sous le trait sur les calques choisis | Palette ; aussi : fondui.rs:207 « Sur les calques choisis » | — | Aplat sous le trait sur les calques choisis `[fond.calques_choisis]` | src/keymap.rs:171 |
| Aplat sous le trait sur tous les calques de dessin du plan | Palette ; aussi : fondui.rs:208 « Sur tout le plan » | — | Aplat sous le trait sur tous les calques de dessin du plan `[fond.tout_le_plan]` | src/keymap.rs:172 |
| Retirer l'aplat sous le trait des calques choisis | Palette ; aussi : fondui.rs:210 « Retirer des calques choisis » | — | Retirer l'aplat sous le trait des calques choisis `[fond.retirer_choisis]` | src/keymap.rs:173 |
| Couleur du projet à tous les aplats sous le trait | Palette ; aussi : fondui.rs:211 « Couleur du projet à tous les aplats » | — | Couleur du projet à tous les aplats sous le trait `[fond.couleur_partout]` | src/keymap.rs:174 |
| Aplat intelligent : colorier les clés choisies comme la clé précédente | Palette | — | Aplat intelligent : colorier les clés choisies comme la clé précédente `[aplat.colorier_comme_precedente]` | src/keymap.rs:175 |
| Aplat sous le trait du calque courant (ancien « remplir la forme ») | absent de la palette : double de `pinceau.fond` | — | Aplat sous le trait du calque courant (ancien « remplir la forme ») `[pinceau.remplir]` | src/keymap.rs:177 |
| Outil cutter | Palette | `Alt+T` | Outil cutter `[outil.cutter]` | src/keymap.rs:178 |
| Outil formes (dernière forme utilisée) | Palette | `Alt+R` | Outil formes (dernière forme utilisée) `[outil.formes]` | src/keymap.rs:179 |
| Outil ligne | Palette | — | Outil ligne `[outil.ligne]` | src/keymap.rs:180 |
| Outil ligne, droite | Palette | — | Outil ligne, droite `[outil.ligne_droite]` | src/keymap.rs:181 |
| Outil ligne, courbe | Palette | — | Outil ligne, courbe `[outil.ligne_courbe]` | src/keymap.rs:182 |
| Outil ligne, courbe en S | Palette | — | Outil ligne, courbe en S `[outil.ligne_s]` | src/keymap.rs:183 |
| Outil rectangle | Palette | — | Outil rectangle `[outil.rectangle]` | src/keymap.rs:184 |
| Outil ellipse | Palette | — | Outil ellipse `[outil.ellipse]` | src/keymap.rs:185 |
| Outil polyligne | Palette | — | Outil polyligne `[outil.polyligne]` | src/keymap.rs:186 |
| Outil texte | Palette | — | Outil texte `[outil.texte]` | src/keymap.rs:187 |
| Outil déformer (enveloppe ou perspective, le dernier mode pris) | Palette | — | Outil déformer (enveloppe ou perspective, le dernier mode pris) `[outil.perspective]` | src/keymap.rs:188 |
| Outil Grab (saisir les points sous le pinceau et les tirer) | Palette | — | Outil Grab (saisir les points sous le pinceau et les tirer) `[outil.grab]` | src/keymap.rs:189 |
| Outil Pull (étirer le trait avec le pinceau, comme un doigt) | Palette | — | Outil Pull (étirer le trait avec le pinceau, comme un doigt) `[outil.pull]` | src/keymap.rs:190 |
| Outil Trajectoire (dessiner le chemin du calque et ses marques d'espacement) | Palette | — | Outil Trajectoire (dessiner le chemin du calque et ses marques d'espacement) `[outil.trajectoire]` | src/keymap.rs:191 |
| Outil déformer : enveloppe de Bézier | Palette | — | Outil déformer : enveloppe de Bézier `[deformer.enveloppe]` | src/keymap.rs:192 |
| Outil déformer : perspective (quatre coins) | Palette | — | Outil déformer : perspective (quatre coins) `[deformer.perspective]` | src/keymap.rs:193 |
| Remettre le pivot de la sélection au centre | Palette | — | Remettre le pivot de la sélection au centre `[selection.reinit_pivot]` | src/keymap.rs:194 |
| Sélection : garder les proportions (oui / non) | Palette | — | Sélection : garder les proportions (oui / non) `[selection.proportions]` | src/keymap.rs:195 |
| Outil main | Palette | `H` | Outil main `[outil.main]` | src/keymap.rs:196 |
| Outil épaisseur du trait de crayon | absent de la palette : **non implémenté** | `O` *en réserve, inactif* | Outil épaisseur du trait de crayon — **non implémenté** `[outil.epaisseur_crayon]` | src/keymap.rs:197 |
| Pinceau plus gros | Palette | `]` | Pinceau plus gros `[outil.taille_plus]` | src/keymap.rs:198 |
| Pinceau plus fin | Palette | `[` | Pinceau plus fin `[outil.taille_moins]` | src/keymap.rs:199 |
| Intervertir les couleurs d'avant-plan et d'arrière-plan (les deux carrés du panneau Couleur) | Palette ; aussi : colorui.rs:384 « Intervertir les couleurs » | — | Intervertir les couleurs d'avant-plan et d'arrière-plan (les deux carrés du panneau Couleur) `[couleur.intervertir]` | src/keymap.rs:201 |
| Couleurs par défaut (noir à l'avant-plan, blanc à l'arrière-plan) | Palette ; aussi : colorui.rs:385 « Couleurs par défaut (noir, blanc) » ; colorui.rs:424 « Couleurs par défaut » | `D` | Couleurs par défaut (noir à l'avant-plan, blanc à l'arrière-plan) `[couleur.defaut]` | src/keymap.rs:202 |
| Copier le code hexadécimal de la couleur d'avant-plan | Palette ; aussi : colorui.rs:387 « Copier le code de la couleur d'avant-plan » ; colorui.rs:427 « Copier le code » | — | Copier le code hexadécimal de la couleur d'avant-plan `[couleur.copier_code]` | src/keymap.rs:203 |
| Roue chromatique (montrer / cacher) | Palette | — | Roue chromatique (montrer / cacher) `[couleur.roue]` | src/keymap.rs:204 |
| Roue chromatique : carré saturation / valeur | Palette ; aussi : colorui.rs:490 « Carré saturation / valeur » | — | Roue chromatique : carré saturation / valeur `[couleur.roue_carre]` | src/keymap.rs:205 |
| Roue chromatique : triangle saturation / valeur | Palette ; aussi : colorui.rs:491 « Triangle saturation / valeur » | — | Roue chromatique : triangle saturation / valeur `[couleur.roue_triangle]` | src/keymap.rs:206 |
| Curseurs teinte, saturation, valeur (TSV) | Palette ; aussi : colorui.rs:628 « Curseurs TSV (teinte, saturation, valeur) » | — | Curseurs teinte, saturation, valeur (TSV) `[couleur.curseurs_tsv]` | src/keymap.rs:207 |
| Curseurs rouge, vert, bleu (RVB) | Palette ; aussi : colorui.rs:629 « Curseurs RVB (rouge, vert, bleu) » | — | Curseurs rouge, vert, bleu (RVB) `[couleur.curseurs_rvb]` | src/keymap.rs:208 |
| Curseurs teinte, saturation, luminosité (TSL) | Palette ; aussi : colorui.rs:630 « Curseurs TSL (teinte, saturation, luminosité) » | — | Curseurs teinte, saturation, luminosité (TSL) `[couleur.curseurs_tsl]` | src/keymap.rs:209 |
| Effacer les couleurs récentes | Palette ; aussi : colorui.rs:644 « Effacer les couleurs récentes » | — | Effacer les couleurs récentes `[couleur.recentes_effacer]` | src/keymap.rs:210 |
| La pastille visée (menu ouvert, sinon sous la souris) devient la couleur d'avant-plan | Palette ; aussi : colorui.rs:681 « Couleur d'avant-plan » | — | La pastille visée (menu ouvert, sinon sous la souris) devient la couleur d'avant-plan `[pastille.avant_plan]` | src/keymap.rs:211 |
| La pastille visée (menu ouvert, sinon sous la souris) devient la couleur d'arrière-plan | Palette ; aussi : colorui.rs:682 « Couleur d'arrière-plan » | — | La pastille visée (menu ouvert, sinon sous la souris) devient la couleur d'arrière-plan `[pastille.arriere_plan]` | src/keymap.rs:212 |
| Nommer la pastille visée du nuancier (sinon celle de la couleur d'avant-plan) | Palette ; aussi : nuancierui.rs:216 « Nommer la couleur… » | — | Nommer la pastille visée du nuancier (sinon celle de la couleur d'avant-plan) `[pastille.renommer]` | src/keymap.rs:213 |
| Remplacer la pastille visée du nuancier par la couleur d'avant-plan | Palette ; aussi : nuancierui.rs:217 « Remplacer par la couleur d'avant-plan » | — | Remplacer la pastille visée du nuancier par la couleur d'avant-plan `[pastille.remplacer]` | src/keymap.rs:214 |
| Retirer du nuancier la pastille visée (sinon celle de la couleur d'avant-plan) | Palette ; aussi : nuancierui.rs:218 « Retirer du nuancier » | — | Retirer du nuancier la pastille visée (sinon celle de la couleur d'avant-plan) `[pastille.retirer]` | src/keymap.rs:215 |
| Ajouter au nuancier la couleur d'avant-plan (ou la couleur récente visée) | Palette ; aussi : colorui.rs:388 « Ajouter la couleur d'avant-plan au nuancier » ; colorui.rs:685 « Ajouter au nuancier » ; nuancierui.rs:202 « Ajouter la couleur d'avant-plan » | — | Ajouter au nuancier la couleur d'avant-plan (ou la couleur récente visée) `[nuancier.ajouter]` | src/keymap.rs:216 |
| Nouveau nuancier | Palette ; aussi : nuancierui.rs:204 « Nouveau nuancier » | — | Nouveau nuancier `[nuancier.nouveau]` | src/keymap.rs:217 |
| Renommer le nuancier montré | Palette ; aussi : nuancierui.rs:205 « Renommer le nuancier… » | — | Renommer le nuancier montré `[nuancier.renommer]` | src/keymap.rs:218 |
| Dupliquer le nuancier montré | Palette ; aussi : nuancierui.rs:206 « Dupliquer le nuancier » | — | Dupliquer le nuancier montré `[nuancier.dupliquer]` | src/keymap.rs:219 |
| Supprimer le nuancier montré (après confirmation) | Palette ; aussi : nuancierui.rs:207 « Supprimer le nuancier… » | — | Supprimer le nuancier montré (après confirmation) `[nuancier.supprimer]` | src/keymap.rs:220 |
| Importer un nuancier (.gpl, .aco) | Palette ; aussi : nuancierui.rs:209 « Importer un nuancier (.gpl, .aco)… » | — | Importer un nuancier (.gpl, .aco) `[nuancier.importer]` | src/keymap.rs:221 |
| Exporter le nuancier montré (.gpl, .aco) | Palette ; aussi : nuancierui.rs:210 « Exporter le nuancier (.gpl, .aco)… » | — | Exporter le nuancier montré (.gpl, .aco) `[nuancier.exporter]` | src/keymap.rs:222 |
| Retourner la sélection horizontalement | Palette ; aussi : panels.rs:182 « De gauche à droite » ; panels.rs:684 « Miroir horizontal » | `Alt+H` | Retourner la sélection horizontalement `[selection.miroir_h]` | src/keymap.rs:223 |
| Retourner le calque de gauche à droite | Palette ; aussi : menucalque.rs:267 « De gauche à droite » | — | Retourner le calque de gauche à droite `[calque.miroir_h]` | src/keymap.rs:224 |
| Retourner le calque de haut en bas | Palette ; aussi : menucalque.rs:268 « De haut en bas » | — | Retourner le calque de haut en bas `[calque.miroir_v]` | src/keymap.rs:225 |
| Retourner les plans choisis de gauche à droite | Palette ; aussi : panels.rs:103 « Retourner les plans choisis » ; storyboard.rs:618 « Retourner les plans choisis » | — | Retourner les plans choisis de gauche à droite `[plan.retourner]` | src/keymap.rs:226 |
| Retourner la sélection verticalement | Palette ; aussi : panels.rs:183 « De haut en bas » ; panels.rs:687 « Miroir vertical » | `Alt+V` | Retourner la sélection verticalement `[selection.miroir_v]` | src/keymap.rs:227 |
| Effacer ce qui est hors de la sélection | Palette | `Maj+Retour arrière` | Effacer ce qui est hors de la sélection `[dessin.supprimer_hors_selection]` | src/keymap.rs:228 |
| Remplir la sélection de la couleur courante | Palette ; aussi : panels.rs:677 « Remplir » | `Maj+Suppr` | Remplir la sélection de la couleur courante `[selection.remplir]` | src/keymap.rs:229 |
| Poser une clé de position à l'image courante | Palette ; aussi : espace3dcles.rs:89 « Clé de position » ; poskeysui.rs:283 « Clé de position » ; poskeysui.rs:595 « Clé de position ici (pose actuelle) » | — | Poser une clé de position à l'image courante `[calque.cle_position]` | src/keymap.rs:230 |
| Lisser la clé de position de l'image courante | Palette ; aussi : poskeysui.rs:288 « Lisser la clé » ; poskeysui.rs:592 « Lisser la clé de position » | — | Lisser la clé de position de l'image courante `[calque.cle_position_lisser]` | src/keymap.rs:231 |
| Supprimer la clé de position de l'image courante | Palette ; aussi : poskeysui.rs:593 « Supprimer la clé de position » | — | Supprimer la clé de position de l'image courante `[calque.cle_position_supprimer]` | src/keymap.rs:232 |
| Supprimer toutes les clés de position du calque | Palette ; aussi : poskeysui.rs:596 « Supprimer toutes les clés du calque » | — | Supprimer toutes les clés de position du calque `[calque.cles_position_supprimer]` | src/keymap.rs:233 |
| Poser une clé d'opacité à l'image courante | Palette ; aussi : opacitecleselui.rs:453 « Clé d'opacité ici (opacité actuelle) » | — | Poser une clé d'opacité à l'image courante `[calque.cle_opacite]` | src/keymap.rs:234 |
| Lisser la clé d'opacité de l'image courante | Palette ; aussi : opacitecleselui.rs:450 « Lisser la clé d'opacité » | — | Lisser la clé d'opacité de l'image courante `[calque.cle_opacite_lisser]` | src/keymap.rs:235 |
| Supprimer la clé d'opacité de l'image courante | Palette ; aussi : opacitecleselui.rs:451 « Supprimer la clé d'opacité » | — | Supprimer la clé d'opacité de l'image courante `[calque.cle_opacite_supprimer]` | src/keymap.rs:236 |
| Supprimer toutes les clés d'opacité du calque | Palette ; aussi : opacitecleselui.rs:454 « Supprimer toutes les clés d'opacité du calque » | — | Supprimer toutes les clés d'opacité du calque `[calque.cles_opacite_supprimer]` | src/keymap.rs:237 |
| Nouveau calque Couleur unie (un fond d'une seule couleur, dessins par-dessus) | Palette | — | Nouveau calque Couleur unie (un fond d'une seule couleur, dessins par-dessus) `[calque.nouveau_couleur_unie]` | src/keymap.rs:238 |
| Aplatir le dessin | absent de la palette : **non implémenté** | `Alt+Maj+F` *en réserve, inactif* | Aplatir le dessin — **non implémenté** `[dessin.aplatir]` | src/keymap.rs:239 |
| Aimanter au contour | absent de la palette : **non implémenté** | `Ctrl+Alt+;` *en réserve, inactif* | Aimanter au contour — **non implémenté** `[dessin.aimanter_contour]` | src/keymap.rs:240 |
| Remplir automatiquement les zones fermées | absent de la palette : **non implémenté** | `Ctrl+Maj+R` *en réserve, inactif* | Remplir automatiquement les zones fermées — **non implémenté** `[dessin.remplir_zones_auto]` | src/keymap.rs:241 |
| Lecture / pause | Palette ; aussi : barres.rs:170 « Lire le montage / pause » ; barres.rs:298 « Lecture / pause » ; onglet3dui.rs:382 « Lire / pause » ; panels.rs:234 « Lecture / pause » | `Entrée` | Lecture / pause `[lecture.basculer]` | src/keymap.rs:242 |
| Image précédente | Palette ; aussi : barres.rs:297 « Image précédente » ; panels.rs:236 « Image précédente » | `,` ou `Flèche gauche` | Image précédente `[image.precedente]` | src/keymap.rs:243 |
| Image suivante | Palette ; aussi : barres.rs:299 « Image suivante » ; panels.rs:237 « Image suivante » | `.` ou `Flèche droite` | Image suivante `[image.suivante]` | src/keymap.rs:244 |
| Première image du plan | Palette ; aussi : barres.rs:296 « Première image » ; panels.rs:238 « Première image du plan » | `Origine` | Première image du plan `[image.premiere]` | src/keymap.rs:245 |
| Dernière image du plan | Palette ; aussi : barres.rs:300 « Dernière image » ; panels.rs:239 « Dernière image du plan » | `Fin` | Dernière image du plan `[image.derniere]` | src/keymap.rs:246 |
| Début du montage | Palette ; aussi : barres.rs:169 « Début du montage » | `Ctrl+Origine` (storyboard) | Début du montage `[montage.debut]` | src/keymap.rs:247 |
| Fin du montage | Palette | `Ctrl+Fin` (storyboard) | Fin du montage `[montage.fin]` | src/keymap.rs:248 |
| Montage : lire en avant (un nouvel appui accélère) | Palette | `L` (montage) | Montage : lire en avant (un nouvel appui accélère) `[montage.lecture_avant]` | src/keymap.rs:249 |
| Montage : lire en arrière (un nouvel appui accélère) | Palette | `J` (montage) | Montage : lire en arrière (un nouvel appui accélère) `[montage.lecture_arriere]` | src/keymap.rs:250 |
| Montage : arrêter la lecture (avec J ou L : image par image) | Palette | `K` (montage) | Montage : arrêter la lecture (avec J ou L : image par image) `[montage.lecture_arret]` | src/keymap.rs:251 |
| Zoom avant de la bande (le montage, ou la timeline en animation) | Palette | `Alt+2` | Zoom avant de la bande (le montage, ou la timeline en animation) `[bande.zoom_avant]` | src/keymap.rs:252 |
| Zoom arrière de la bande (le montage, ou la timeline en animation) | Palette | `Alt+1` | Zoom arrière de la bande (le montage, ou la timeline en animation) `[bande.zoom_arriere]` | src/keymap.rs:253 |
| Bande : tout voir (le film en storyboard, le plan en animation) | Palette ; aussi : timelineseq.rs:114 « Ajuster la timeline » | `Alt+3` | Bande : tout voir (le film en storyboard, le plan en animation) `[bande.tout_voir]` | src/keymap.rs:254 |
| Plan précédent | Palette | `PgPréc` | Plan précédent `[panneau.precedent]` | src/keymap.rs:255 |
| Plan suivant | Palette | `PgSuiv` | Plan suivant `[panneau.suivant]` | src/keymap.rs:256 |
| Nouveau plan | Palette ; aussi : barres.rs:174 « Nouveau plan » ; montagerows.rs:575 « Nouveau plan » ; panels.rs:187 « Nouveau plan » ; storyboard.rs:2044 « Nouveau plan » | `P` ou `Inser` (storyboard) | Nouveau plan `[panneau.nouveau]` | src/keymap.rs:257 |
| Dupliquer le plan | Palette ; aussi : montagerows.rs:582 « Dupliquer le plan » ; panels.rs:80 « Dupliquer le plan » ; storyboard.rs:604 « Dupliquer le plan » | `Ctrl+Maj+D` | Dupliquer le plan `[panneau.dupliquer]` | src/keymap.rs:258 |
| Réutiliser le plan : un plan lié au sien, même dessin, à cadrer autrement (planche-mère) | Palette ; aussi : panels.rs:82 « Réutiliser le plan (même dessin, autre cadre) » | — | Réutiliser le plan : un plan lié au sien, même dessin, à cadrer autrement (planche-mère) `[plan.reutiliser]` | src/keymap.rs:260 |
| Détacher le plan de la planche-mère (ses calques gardent des copies) | Palette ; aussi : panels.rs:84 « Détacher le plan de la planche-mère » | — | Détacher le plan de la planche-mère (ses calques gardent des copies) `[plan.detacher]` | src/keymap.rs:261 |
| Nouveau plan (second raccourci) | absent de la palette : double de `panneau.nouveau` | `Maj+P` | Nouveau plan (second raccourci) `[scene.nouvelle]` | src/keymap.rs:264 |
| Dupliquer le plan (second raccourci) | absent de la palette : double de `panneau.dupliquer` | — | Dupliquer le plan (second raccourci) `[scene.dupliquer]` | src/keymap.rs:265 |
| Supprimer les plans choisis (second raccourci) | absent de la palette : double de `panneau.supprimer` | — | Supprimer les plans choisis (second raccourci) `[scene.supprimer]` | src/keymap.rs:266 |
| Plan précédent (second raccourci) | absent de la palette : double de `panneau.precedent` | `Ctrl+PgPréc` | Plan précédent (second raccourci) `[scene.precedente]` | src/keymap.rs:267 |
| Plan suivant (second raccourci) | absent de la palette : double de `panneau.suivant` | `Ctrl+PgSuiv` | Plan suivant (second raccourci) `[scene.suivante]` | src/keymap.rs:268 |
| Nouvelle séquence | Palette ; aussi : barres.rs:175 « Nouvelle séquence » ; montagerows.rs:585 « Nouvelle séquence » ; panels.rs:188 « Nouvelle séquence » ; storyboard.rs:2042 « Nouvelle séquence » ; timelinesb.rs:157 « Nouvelle séquence » | — | Nouvelle séquence `[sequence.nouvelle]` | src/keymap.rs:269 |
| Nouvelle séquence avec les plans choisis | Palette ; aussi : boardui.rs:2737 « Nouvelle séquence avec les {n} plans choisis » ; panels.rs:101 « Nouvelle séquence avec les plans choisis » ; storyboard.rs:615 « Nouvelle séquence avec les {n} plans choisis » ; timelinesb.rs:165 « Nouvelle séquence avec les plans choisis » | — | Nouvelle séquence avec les plans choisis `[sequence.nouvelle_avec_plans]` | src/keymap.rs:270 |
| Nouvelle séquence à la fin du film (depuis les vignettes) | Palette ; aussi : boardui.rs:2822 « Nouvelle séquence à la fin du film » ; planchesequencesui.rs:503 « Nouvelle séquence à la fin du film » | — | Nouvelle séquence à la fin du film (depuis les vignettes) `[sequence.nouvelle_fin]` | src/keymap.rs:271 |
| Nouvelle séquence avec les cases choisies (vignettes) | Palette | — | Nouvelle séquence avec les cases choisies (vignettes) `[sequence.nouvelle_avec_cases]` | src/keymap.rs:272 |
| Couper la séquence ici (la case courante commence une séquence) | Palette ; aussi : planchesequencesui.rs:507 « Couper la séquence ici » ; timelinesb.rs:166 « Couper la séquence ici (le plan courant la commence) » | — | Couper la séquence ici (la case courante commence une séquence) `[sequence.couper_ici]` | src/keymap.rs:273 |
| Renommer la séquence (son titre sur la planche, ou une petite fenêtre) | Palette ; aussi : planchesequencesui.rs:508 « Renommer la séquence… » ; timelinesb.rs:158 « Renommer la séquence… » | — | Renommer la séquence (son titre sur la planche, ou une petite fenêtre) `[sequence.renommer]` | src/keymap.rs:274 |
| Dupliquer la séquence (une copie indépendante, juste après elle) | Palette ; aussi : montagerows.rs:586 « Dupliquer la séquence » ; panels.rs:189 « Dupliquer la séquence » ; planchesequencesui.rs:509 « Dupliquer la séquence » ; storyboard.rs:2043 « Dupliquer la séquence » ; timelinesb.rs:164 « Dupliquer la séquence » | — | Dupliquer la séquence (une copie indépendante, juste après elle) `[sequence.dupliquer]` | src/keymap.rs:275 |
| Planche : montrer toutes les séquences | Palette | — | Planche : montrer toutes les séquences `[sequence.afficher_toutes]` | src/keymap.rs:276 |
| Planche : seulement la séquence courante | Palette | — | Planche : seulement la séquence courante `[sequence.afficher_courante]` | src/keymap.rs:277 |
| Planche : aucune autre séquence que la courante | Palette | — | Planche : aucune autre séquence que la courante `[sequence.afficher_aucune_autre]` | src/keymap.rs:278 |
| Planche : ranger les séquences dans l'ordre du film | Palette | — | Planche : ranger les séquences dans l'ordre du film `[sequence.afficher_ordre_film]` | src/keymap.rs:279 |
| Supprimer les plans choisis | Palette ; aussi : montagerows.rs:583 « Supprimer les plans choisis » ; panels.rs:89 « Supprimer les plans choisis » ; timelinesb.rs:213 « Supprimer les plans choisis » | `Ctrl+Suppr` (storyboard) | Supprimer les plans choisis `[panneau.supprimer]` | src/keymap.rs:280 |
| Copier les plans choisis | Palette ; aussi : panels.rs:87 « Copier les plans choisis » | — | Copier les plans choisis `[panneau.copier]` | src/keymap.rs:281 |
| Coller les plans après le plan courant | Palette ; aussi : panels.rs:88 « Coller les plans après le plan courant » | — | Coller les plans après le plan courant `[panneau.coller]` | src/keymap.rs:282 |
| Allonger le plan d'une image | Palette ; aussi : panels.rs:91 « Allonger le plan d'une image » | `+` | Allonger le plan d'une image `[panneau.ajouter_image]` | src/keymap.rs:283 |
| Raccourcir le plan d'une image | Palette ; aussi : panels.rs:92 « Raccourcir le plan d'une image » | `-` | Raccourcir le plan d'une image `[panneau.retirer_image]` | src/keymap.rs:284 |
| Fiche du plan : ouvrir les propriétés du plan courant | Palette ; aussi : panels.rs:193 « Fiche du plan » ; timelinesb.rs:218 « Nom et fiche du plan… » | `Alt+Entrée` | Fiche du plan : ouvrir les propriétés du plan courant `[panneau.proprietes]` | src/keymap.rs:285 |
| Couper le plan à l'image courante | Palette ; aussi : barres.rs:389 « Couper le plan » ; panels.rs:97 « Couper le plan à l'image courante » ; storyboard.rs:609 « Couper le plan à l'image courante » ; storyboard.rs:2053 « Couper le plan ici » | `F6` | Couper le plan à l'image courante `[panneau.couper]` | src/keymap.rs:286 |
| Réunir le plan et le suivant | Palette ; aussi : barres.rs:390 « Réunir avec le suivant » ; panels.rs:98 « Réunir avec le plan suivant » ; storyboard.rs:610 « Réunir le plan et le suivant » ; timelinesb.rs:215 « Réunir le plan et le suivant » | — | Réunir le plan et le suivant `[panneau.fusionner_suivant]` | src/keymap.rs:287 |
| Fusionner les plans choisis (qui se suivent) | Palette ; aussi : panels.rs:100 « Fusionner les plans choisis » ; storyboard.rs:614 « Fusionner les {n} plans choisis » | — | Fusionner les plans choisis (qui se suivent) `[panneau.fusionner_choisis]` | src/keymap.rs:288 |
| Couper le plan à l'image courante (second raccourci) | absent de la palette : double de `panneau.couper` | — | Couper le plan à l'image courante (second raccourci) `[scene.couper]` | src/keymap.rs:289 |
| Réunir avec le plan précédent | Palette ; aussi : panels.rs:99 « Réunir avec le plan précédent » ; storyboard.rs:611 « Réunir avec le plan précédent » ; timelinesb.rs:216 « Réunir avec le plan précédent » | — | Réunir avec le plan précédent `[scene.joindre_precedente]` | src/keymap.rs:290 |
| Aller à un plan | Palette ; aussi : panels.rs:192 « Aller à un plan… » | `Ctrl+F` | Aller à un plan `[scene.aller_a]` | src/keymap.rs:291 |
| Marque d'image à l'image courante (ajouter / retirer) | Palette ; aussi : handles.rs:1134 « Marque d'image ici (poser / retirer) » ; panels.rs:196 « Marque d'image ici (ajouter / retirer) » ; storyboard.rs:620 « Marque d'image ici (ajouter / retirer) » | — | Marque d'image à l'image courante (ajouter / retirer) `[marque.basculer]` | src/keymap.rs:292 |
| Nouveau dessin après l'exposition | Palette ; aussi : barres.rs:306 « Nouveau dessin » ; images.rs:1161 « Nouveau dessin après l'instance » ; panels.rs:242 « Nouveau dessin » | `Inser` (animation) | Nouveau dessin après l'exposition `[dessin.nouveau]` | src/keymap.rs:293 |
| Dupliquer le dessin | Palette ; aussi : barres.rs:307 « Dupliquer le dessin » ; panels.rs:243 « Dupliquer le dessin » | `Ctrl+D` | Dupliquer le dessin `[dessin.dupliquer]` | src/keymap.rs:294 |
| Nouvelle clé à l'image courante, copie du dessin (coupe l'exposition) | Palette | `F5` | Nouvelle clé à l'image courante, copie du dessin (coupe l'exposition) `[dessin.nouvelle_cle]` | src/keymap.rs:295 |
| Nouvelle clé vide à l'image courante (coupe l'exposition) | Palette | `F4` | Nouvelle clé vide à l'image courante (coupe l'exposition) `[dessin.nouvelle_cle_vide]` | src/keymap.rs:296 |
| Retirer le dessin de la timeline | Palette ; aussi : barres.rs:310 « Supprimer le dessin » ; panels.rs:246 « Retirer le dessin de la timeline » | `Ctrl+Suppr` (animation) | Retirer le dessin de la timeline `[dessin.supprimer]` | src/keymap.rs:297 |
| Tenir le dessin une image de plus | Palette ; aussi : barres.rs:308 « Allonger l'exposition » ; panels.rs:244 « Allonger l'exposition » | `Alt++` | Tenir le dessin une image de plus `[exposition.allonger]` | src/keymap.rs:298 |
| Tenir le dessin une image de moins | Palette ; aussi : barres.rs:309 « Raccourcir l'exposition » ; panels.rs:245 « Raccourcir l'exposition » | `Alt+-` | Tenir le dessin une image de moins `[exposition.raccourcir]` | src/keymap.rs:299 |
| Nouveau calque vectoriel | Palette ; aussi : barres.rs:176 « Nouveau calque vectoriel » | `Maj+S` | Nouveau calque vectoriel `[calque.nouveau_vectoriel]` | src/keymap.rs:300 |
| Nouveau calque bitmap | Palette | `Maj+B` | Nouveau calque bitmap `[calque.nouveau_bitmap]` | src/keymap.rs:301 |
| Copier le calque | Palette ; aussi : menucalque.rs:262 « Copier le calque » | `W` | Copier le calque `[calque.copier]` | src/keymap.rs:302 |
| Coller le calque | Palette ; aussi : barrecalques.rs:56 « Coller le calque » ; menucalque.rs:263 « Coller le calque » | `X` | Coller le calque `[calque.coller]` | src/keymap.rs:303 |
| Supprimer les calques choisis | Palette ; aussi : barrecalques.rs:85 « Le calque courant et ceux choisis par Ctrl + clic sur leur ligne. » ; menucalque.rs:124 « Supprimer » ; menucalque.rs:189 « Supprimer le dossier et ses calques » | `Q` | Supprimer les calques choisis `[calque.supprimer]` | src/keymap.rs:304 |
| Flou sur le calque (effet Flou gaussien, cocher / décocher) | Palette | `Maj+D` | Flou sur le calque (effet Flou gaussien, cocher / décocher) `[calque.flou]` | src/keymap.rs:305 |
| Verrouiller le calque | Palette ; aussi : layerheads.rs:1116 « Verrouiller / déverrouiller » | `Alt+L` | Verrouiller le calque `[calque.verrouiller]` | src/keymap.rs:306 |
| Fusionner vers le bas (avec le calque du dessous) | Palette | `Ctrl+E` | Fusionner vers le bas (avec le calque du dessous) `[calque.fusionner_dessous]` | src/keymap.rs:307 |
| Annuler la transformation du calque | Palette | `Ctrl+R` | Annuler la transformation du calque `[calque.reinit_transformation]` | src/keymap.rs:308 |
| Accrocher les calques choisis à la caméra (ou les décrocher) : ils gardent leur place dans l'image | Palette | — | Accrocher les calques choisis à la caméra (ou les décrocher) : ils gardent leur place dans l'image `[calque.accrocher_camera]` | src/keymap.rs:309 |
| Origine du calque au centre du dessin | Palette | — | Origine du calque au centre du dessin `[origine.centre_dessin]` | src/keymap.rs:311 |
| Origine du calque en bas au centre du dessin | Palette | — | Origine du calque en bas au centre du dessin `[origine.bas_centre]` | src/keymap.rs:312 |
| Origine du calque en haut à gauche du dessin | Palette | — | Origine du calque en haut à gauche du dessin `[origine.haut_gauche]` | src/keymap.rs:313 |
| Origine du calque en haut à droite du dessin | Palette | — | Origine du calque en haut à droite du dessin `[origine.haut_droite]` | src/keymap.rs:314 |
| Origine du calque en bas à gauche du dessin | Palette | — | Origine du calque en bas à gauche du dessin `[origine.bas_gauche]` | src/keymap.rs:315 |
| Origine du calque en bas à droite du dessin | Palette | — | Origine du calque en bas à droite du dessin `[origine.bas_droite]` | src/keymap.rs:316 |
| Origine du calque ici (au point de la feuille sous le pointeur) | Palette | — | Origine du calque ici (au point de la feuille sous le pointeur) `[origine.ici]` | src/keymap.rs:317 |
| Origine du calque au centre de la feuille (comme d'origine) | Palette | — | Origine du calque au centre de la feuille (comme d'origine) `[origine.feuille]` | src/keymap.rs:318 |
| Aimanté au sol : l'origine du calque repose sur le sol de l'Espace 3D (oui / non) | Palette, **seulement vue Espace 3D ouverte** | — | Aimanté au sol : l'origine du calque repose sur le sol de l'Espace 3D (oui / non) `[calque.aimante_au_sol]` | src/keymap.rs:319 |
| Toujours devant le décor 3D : aucun objet du décor ne cache la carte du calque (oui / non) | Palette, **seulement vue Espace 3D ouverte** | — | Toujours devant le décor 3D : aucun objet du décor ne cache la carte du calque (oui / non) `[calque.devant_decor_3d]` | src/keymap.rs:320 |
| Porte une ombre : l'encre de la carte du calque fait de l'ombre sous le soleil du décor 3D (oui / non) | Palette, **seulement vue Espace 3D ouverte** | — | Porte une ombre : l'encre de la carte du calque fait de l'ombre sous le soleil du décor 3D (oui / non) `[calque.porte_ombre]` | src/keymap.rs:321 |
| Reçoit les ombres : la carte du calque s'assombrit à l'ombre du décor 3D et des autres cartes (oui / non) | Palette, **seulement vue Espace 3D ouverte** | — | Reçoit les ombres : la carte du calque s'assombrit à l'ombre du décor 3D et des autres cartes (oui / non) `[calque.recoit_ombres]` | src/keymap.rs:322 |
| Calque du dessus | Palette ; aussi : menucalque.rs:175 « Du dessus » | `Flèche haut` | Calque du dessus `[calque.dessus]` | src/keymap.rs:323 |
| Calque du dessous | Palette ; aussi : menucalque.rs:176 « Du dessous » | `Flèche bas` | Calque du dessous `[calque.dessous]` | src/keymap.rs:324 |
| Nouveau calque… (nom, sorte, groupe, mode, place) | Palette | — | Nouveau calque… (nom, sorte, groupe, mode, place) `[calque.nouveau]` | src/keymap.rs:325 |
| Dupliquer les calques choisis | Palette ; aussi : barrecalques.rs:81 « Une copie de chaque calque choisi et de ses dessins, juste au-dessus de lui. » ; menucalque.rs:120 « Dupliquer » | — | Dupliquer les calques choisis `[calque.dupliquer]` | src/keymap.rs:326 |
| Réutiliser le calque dans le plan suivant (planche-mère) | Palette ; aussi : partageui.rs:134 « Le plan suivant » | — | Réutiliser le calque dans le plan suivant (planche-mère) `[calque.reutiliser_suivant]` | src/keymap.rs:327 |
| Réutiliser le calque dans le plan précédent (planche-mère) | Palette ; aussi : partageui.rs:135 « Le plan précédent » | — | Réutiliser le calque dans le plan précédent (planche-mère) `[calque.reutiliser_precedent]` | src/keymap.rs:328 |
| Réutiliser le calque dans tous les plans de la séquence (planche-mère) | Palette ; aussi : partageui.rs:136 « Tous les plans de la séquence » | — | Réutiliser le calque dans tous les plans de la séquence (planche-mère) `[calque.reutiliser_sequence]` | src/keymap.rs:329 |
| Réutiliser le calque dans des plans choisis… (planche-mère) | Palette ; aussi : partageui.rs:137 « Des plans choisis… » | — | Réutiliser le calque dans des plans choisis… (planche-mère) `[calque.reutiliser_choisir]` | src/keymap.rs:330 |
| Détacher le calque de la planche-mère (il garde une copie) | Palette ; aussi : partageui.rs:140 « Détacher (garder une copie) » | — | Détacher le calque de la planche-mère (il garde une copie) `[calque.detacher_partage]` | src/keymap.rs:331 |
| Supprimer les calques vides | Palette ; aussi : menucalque.rs:316 « Supprimer les calques vides » | — | Supprimer les calques vides `[calque.supprimer_vides]` | src/keymap.rs:332 |
| Supprimer les calques cachés | Palette ; aussi : menucalque.rs:317 « Supprimer les calques cachés » | — | Supprimer les calques cachés `[calque.supprimer_caches]` | src/keymap.rs:333 |
| Fusionner les calques choisis | Palette | — | Fusionner les calques choisis `[calque.fusionner_choisis]` | src/keymap.rs:334 |
| Fusionner les calques visibles | Palette ; aussi : menucalque.rs:303 « Les calques visibles » | — | Fusionner les calques visibles `[calque.fusionner_visibles]` | src/keymap.rs:335 |
| Fusionner tous les calques | Palette ; aussi : menucalque.rs:303 « Les calques visibles » | — | Fusionner tous les calques `[calque.fusionner_tout]` | src/keymap.rs:336 |
| Choisir tous les calques | Palette ; aussi : menucalque.rs:290 « Tous les calques » | — | Choisir tous les calques `[calque.choisir_tout]` | src/keymap.rs:337 |
| Choisir les calques visibles | Palette ; aussi : menucalque.rs:291 « Les calques visibles » | — | Choisir les calques visibles `[calque.choisir_visibles]` | src/keymap.rs:338 |
| Choisir les calques du même groupe | Palette ; aussi : groupsui.rs:129 « Choisir les calques du groupe courant » ; menucalque.rs:292 « Les calques du même groupe » | — | Choisir les calques du même groupe `[calque.choisir_groupe]` | src/keymap.rs:339 |
| Choisir les calques des autres groupes | Palette ; aussi : groupsui.rs:130 « Choisir les calques des autres groupes » ; menucalque.rs:293 « Les calques des autres groupes » | — | Choisir les calques des autres groupes `[calque.choisir_autres_groupes]` | src/keymap.rs:340 |
| Ne plus choisir de calque (le courant seul) | Palette ; aussi : menucalque.rs:294 « Aucun (le calque courant seul) » | — | Ne plus choisir de calque (le courant seul) `[calque.choisir_aucun]` | src/keymap.rs:341 |
| Calque du dessus, en gardant le choix | Palette | `Maj+Flèche haut` | Calque du dessus, en gardant le choix `[calque.choisir_dessus]` | src/keymap.rs:342 |
| Calque du dessous, en gardant le choix | Palette | `Maj+Flèche bas` | Calque du dessous, en gardant le choix `[calque.choisir_dessous]` | src/keymap.rs:343 |
| Monter le calque d'un rang | Palette ; aussi : menucalque.rs:172 « Monter le calque » | `Ctrl+Flèche haut` | Monter le calque d'un rang `[calque.monter]` | src/keymap.rs:344 |
| Descendre le calque d'un rang | Palette ; aussi : menucalque.rs:173 « Descendre le calque » | `Ctrl+Flèche bas` | Descendre le calque d'un rang `[calque.descendre]` | src/keymap.rs:345 |
| Premier calque (le plus bas) | Palette ; aussi : menucalque.rs:177 « Le premier (le plus bas) » | `Ctrl+Origine` (animation) | Premier calque (le plus bas) `[calque.premier]` | src/keymap.rs:346 |
| Dernier calque (le plus haut) | Palette ; aussi : menucalque.rs:178 « Le dernier (le plus haut) » | `Ctrl+Fin` (animation) | Dernier calque (le plus haut) `[calque.dernier]` | src/keymap.rs:347 |
| Afficher tous les calques | Palette ; aussi : menucalque.rs:297 « Tous les calques » | — | Afficher tous les calques `[calque.tous_visibles]` | src/keymap.rs:348 |
| Cacher tous les calques | Palette ; aussi : menucalque.rs:298 « Aucun calque » | — | Cacher tous les calques `[calque.tous_caches]` | src/keymap.rs:349 |
| Afficher le calque courant seul | Palette ; aussi : menucalque.rs:299 « Le calque courant seul » | — | Afficher le calque courant seul `[calque.courant_seul]` | src/keymap.rs:350 |
| Éteindre tous les pochoirs | Palette ; aussi : menucalque.rs:312 « Éteindre tous les pochoirs » | — | Éteindre tous les pochoirs `[calque.pochoirs_eteindre]` | src/keymap.rs:351 |
| Inverser tous les pochoirs | Palette ; aussi : menucalque.rs:313 « Inverser tous les pochoirs » | — | Inverser tous les pochoirs `[calque.pochoirs_inverser]` | src/keymap.rs:352 |
| Pelure d'oignon du calque courant (allumer / éteindre) | Palette | `Ctrl+L` | Pelure d'oignon du calque courant (allumer / éteindre) `[calque.pelure]` | src/keymap.rs:353 |
| Couper le calque | Palette ; aussi : menucalque.rs:261 « Couper le calque » | — | Couper le calque `[calque.couper]` | src/keymap.rs:354 |
| Coller le calque à la place du calque courant | Palette ; aussi : menucalque.rs:264 « Coller à la place du calque courant » | — | Coller le calque à la place du calque courant `[calque.coller_remplacer]` | src/keymap.rs:355 |
| Supprimer les images choisies (calques choisis) | Palette ; aussi : images.rs:1136 « Supprimer » | — | Supprimer les images choisies (calques choisis) `[images.supprimer]` | src/keymap.rs:356 |
| Couper les images choisies | Palette ; aussi : images.rs:1132 « Couper » | — | Couper les images choisies `[images.couper]` | src/keymap.rs:357 |
| Copier les images choisies | Palette ; aussi : images.rs:1133 « Copier » | — | Copier les images choisies `[images.copier]` | src/keymap.rs:358 |
| Coller les images (insérer avant l'image courante) | Palette ; aussi : images.rs:1134 « Coller (insérer avant l'image courante) » | — | Coller les images (insérer avant l'image courante) `[images.coller]` | src/keymap.rs:359 |
| Coller les images (remplacer les images choisies) | Palette ; aussi : images.rs:1167 « Coller (remplacer les images choisies) » | — | Coller les images (remplacer les images choisies) `[images.coller_remplacer]` | src/keymap.rs:360 |
| Dupliquer les images choisies | Palette ; aussi : images.rs:1135 « Dupliquer » | — | Dupliquer les images choisies `[images.dupliquer]` | src/keymap.rs:361 |
| Inverser l'ordre des images choisies | Palette ; aussi : images.rs:1195 « Inverser l'ordre » | — | Inverser l'ordre des images choisies `[images.inverser]` | src/keymap.rs:362 |
| Insérer une image avant (calques choisis) | Palette ; aussi : images.rs:1140 « Insérer une image avant » | — | Insérer une image avant (calques choisis) `[images.inserer_avant_calques]` | src/keymap.rs:363 |
| Insérer une image après (calques choisis) | Palette ; aussi : images.rs:1141 « Insérer une image après » | — | Insérer une image après (calques choisis) `[images.inserer_apres_calques]` | src/keymap.rs:364 |
| Ajouter une image au plan, après l'image courante | Palette ; aussi : images.rs:1144 « Ajouter une image après l'image courante » | `I` (animation) | Ajouter une image au plan, après l'image courante `[images.ajouter]` | src/keymap.rs:365 |
| Insérer une image dans le plan, avant l'image courante | Palette ; aussi : images.rs:1145 « Insérer une image avant l'image courante » | `Maj+I` (animation) | Insérer une image dans le plan, avant l'image courante `[images.inserer]` | src/keymap.rs:366 |
| Supprimer l'image du plan (tous les calques) | Palette ; aussi : images.rs:1146 « Supprimer l'image (ou les images choisies) » | `Maj+F5` (animation) | Supprimer l'image du plan (tous les calques) `[images.supprimer_plan]` | src/keymap.rs:367 |
| Choisir toutes les images du plan | Palette ; aussi : images.rs:1194 « Choisir toutes les images » | — | Choisir toutes les images du plan `[images.tout_choisir]` | src/keymap.rs:368 |
| Instance précédente (début du dessin d'avant) | Palette ; aussi : images.rs:1152 « Instance précédente » | `Ctrl+Flèche gauche` (animation) | Instance précédente (début du dessin d'avant) `[instance.precedente]` | src/keymap.rs:369 |
| Instance suivante (début du dessin d'après) | Palette ; aussi : images.rs:1153 « Instance suivante » | `Ctrl+Flèche droite` (animation) | Instance suivante (début du dessin d'après) `[instance.suivante]` | src/keymap.rs:370 |
| Clé précédente (début de dessin d'avant, tous les calques) | Palette ; aussi : images.rs:1154 « Clé précédente (tous les calques) » | `A` | Clé précédente (début de dessin d'avant, tous les calques) `[cle.precedente]` | src/keymap.rs:371 |
| Clé suivante (début de dessin d'après, tous les calques) | Palette ; aussi : images.rs:1155 « Clé suivante (tous les calques) » | `F` | Clé suivante (début de dessin d'après, tous les calques) `[cle.suivante]` | src/keymap.rs:372 |
| Tête de l'instance (sa première image) | Palette ; aussi : images.rs:1156 « Tête de l'instance » | — | Tête de l'instance (sa première image) `[instance.tete]` | src/keymap.rs:373 |
| Queue de l'instance (sa dernière image) | Palette ; aussi : images.rs:1157 « Queue de l'instance » | — | Queue de l'instance (sa dernière image) `[instance.queue]` | src/keymap.rs:374 |
| Casser l'instance à l'image courante | Palette ; aussi : images.rs:1159 « Casser l'instance à l'image courante » | — | Casser l'instance à l'image courante `[instance.casser]` | src/keymap.rs:375 |
| Nouveau dessin avant l'instance courante | Palette ; aussi : images.rs:1160 « Nouveau dessin avant l'instance » | — | Nouveau dessin avant l'instance courante `[instance.ajouter_avant]` | src/keymap.rs:376 |
| Une exposition de plus à chaque instance choisie | Palette ; aussi : images.rs:1163 « Une exposition de plus à chaque instance choisie » | — | Une exposition de plus à chaque instance choisie `[instance.exposition_plus]` | src/keymap.rs:377 |
| Une exposition de moins à chaque instance choisie | Palette ; aussi : images.rs:1164 « Une exposition de moins à chaque instance choisie » | — | Une exposition de moins à chaque instance choisie `[instance.exposition_moins]` | src/keymap.rs:378 |
| Faux-fixe : rejouer les images choisies en boucle | Palette ; aussi : images.rs:1173 « À la suite, en boucle » | — | Faux-fixe : rejouer les images choisies en boucle `[images.faux_fixe_boucle]` | src/keymap.rs:379 |
| Faux-fixe : rejouer les images choisies en va-et-vient | Palette ; aussi : images.rs:1174 « En va-et-vient » | — | Faux-fixe : rejouer les images choisies en va-et-vient `[images.faux_fixe_va_et_vient]` | src/keymap.rs:380 |
| Faux-fixe : rejouer les images choisies au hasard | Palette ; aussi : images.rs:1175 « Au hasard » | — | Faux-fixe : rejouer les images choisies au hasard `[images.faux_fixe_hasard]` | src/keymap.rs:381 |
| Étirer le calque jusqu'à la fin du plan (tenir sa dernière image) | Palette ; aussi : images.rs:1178 « En tenant sa dernière image » | — | Étirer le calque jusqu'à la fin du plan (tenir sa dernière image) `[calque.etirer_tenir]` | src/keymap.rs:382 |
| Étirer le calque jusqu'à la fin du plan (en boucle) | Palette ; aussi : images.rs:1179 « En rejouant ses images en boucle » | — | Étirer le calque jusqu'à la fin du plan (en boucle) `[calque.etirer_boucle]` | src/keymap.rs:383 |
| Étirer le calque jusqu'à la fin du plan (en va-et-vient) | Palette ; aussi : images.rs:1180 « En rejouant ses images en va-et-vient » | — | Étirer le calque jusqu'à la fin du plan (en va-et-vient) `[calque.etirer_va_et_vient]` | src/keymap.rs:384 |
| Couper le calque après l'image courante | Palette ; aussi : images.rs:1182 « Couper le calque après l'image courante » | — | Couper le calque après l'image courante `[calque.couper_apres]` | src/keymap.rs:385 |
| Retourner les images choisies horizontalement | Palette ; aussi : images.rs:1197 « Horizontalement » | — | Retourner les images choisies horizontalement `[images.retourner_h]` | src/keymap.rs:386 |
| Retourner les images choisies verticalement | Palette ; aussi : images.rs:1198 « Verticalement » | — | Retourner les images choisies verticalement `[images.retourner_v]` | src/keymap.rs:387 |
| Retourner les images choisies horizontalement et verticalement | Palette ; aussi : images.rs:1199 « Horizontalement et verticalement » | — | Retourner les images choisies horizontalement et verticalement `[images.retourner_hv]` | src/keymap.rs:388 |
| Renommer l'instance courante | Palette ; aussi : images.rs:1202 « Renommer l'instance courante » | — | Renommer l'instance courante `[instance.renommer]` | src/keymap.rs:389 |
| Effacer les noms d'instance (calques choisis) | Palette ; aussi : images.rs:1203 « Effacer les noms » | — | Effacer les noms d'instance (calques choisis) `[instance.effacer_noms]` | src/keymap.rs:390 |
| Renommer toutes les instances 1, 2, 3… | Palette ; aussi : images.rs:1204 « Tout renommer (1, 2, 3…) » | — | Renommer toutes les instances 1, 2, 3… `[instance.tout_renommer]` | src/keymap.rs:391 |
| Renommer intelligemment les instances (préfixe, suffixe) | Palette ; aussi : images.rs:1205 « Renommer intelligemment… » | — | Renommer intelligemment les instances (préfixe, suffixe) `[instance.renommer_intelligemment]` | src/keymap.rs:392 |
| Signet à l'image courante (poser / retirer) | Palette ; aussi : cues.rs:286 « Poser / retirer un signet » ; handles.rs:1133 « Signet ici (poser / retirer) » ; storyboard.rs:2054 « Signet du plan ici (poser / retirer) » | `Ctrl+Maj+Flèche haut` | Signet à l'image courante (poser / retirer) `[signet.basculer]` | src/keymap.rs:393 |
| Signet précédent | Palette ; aussi : cues.rs:287 « Signet précédent » | `Ctrl+Maj+Flèche gauche` | Signet précédent `[signet.precedent]` | src/keymap.rs:394 |
| Signet suivant | Palette ; aussi : cues.rs:288 « Signet suivant » | `Ctrl+Maj+Flèche droite` | Signet suivant `[signet.suivant]` | src/keymap.rs:395 |
| Enlever les signets des images choisies | Palette ; aussi : cues.rs:289 « Enlever les signets des images choisies » | — | Enlever les signets des images choisies `[signets.enlever_choisis]` | src/keymap.rs:396 |
| Enlever tous les signets du plan | Palette ; aussi : cues.rs:290 « Enlever tous les signets » | — | Enlever tous les signets du plan `[signets.tout_enlever]` | src/keymap.rs:397 |
| Point d'entrée à l'image courante | Palette ; aussi : cues.rs:294 « Point d'entrée à l'image courante » ; handles.rs:1137 « Point d'entrée ici » | — | Point d'entrée à l'image courante `[entree.poser]` | src/keymap.rs:398 |
| Point d'entrée (poser / retirer) | Palette ; aussi : cues.rs:295 « Poser / retirer le point d'entrée » | — | Point d'entrée (poser / retirer) `[entree.basculer]` | src/keymap.rs:399 |
| Point de sortie à l'image courante | Palette ; aussi : cues.rs:296 « Point de sortie à l'image courante » ; handles.rs:1138 « Point de sortie ici » | — | Point de sortie à l'image courante `[sortie.poser]` | src/keymap.rs:400 |
| Point de sortie (poser / retirer) | Palette ; aussi : cues.rs:297 « Poser / retirer le point de sortie » | — | Point de sortie (poser / retirer) `[sortie.basculer]` | src/keymap.rs:401 |
| Enlever les points d'entrée et de sortie | Palette ; aussi : cues.rs:300 « Enlever les points d'entrée et de sortie » ; handles.rs:1139 « Enlever l'entrée et la sortie » | — | Enlever les points d'entrée et de sortie `[entree_sortie.enlever]` | src/keymap.rs:402 |
| Les images choisies deviennent les limites de la lecture | Palette ; aussi : cues.rs:298 « Les images choisies deviennent les limites » | — | Les images choisies deviennent les limites de la lecture `[boucle.limites_choisies]` | src/keymap.rs:403 |
| Boucle entre les images choisies (allumer / éteindre) | Palette ; aussi : cues.rs:299 « Boucle entre les images choisies (allumer / éteindre) » | — | Boucle entre les images choisies (allumer / éteindre) `[boucle.images_choisies]` | src/keymap.rs:404 |
| Timeline : adapter au plan | Palette ; aussi : cues.rs:271 « Le plan » ; handles.rs:1145 « Adapter la timeline au plan » | — | Timeline : adapter au plan `[timeline.adapter_plan]` | src/keymap.rs:405 |
| Timeline : adapter au calque courant | Palette ; aussi : cues.rs:272 « Le calque courant » | — | Timeline : adapter au calque courant `[timeline.adapter_calque]` | src/keymap.rs:406 |
| Timeline : adapter aux images choisies | Palette ; aussi : cues.rs:273 « Les images choisies » | — | Timeline : adapter aux images choisies `[timeline.adapter_choix]` | src/keymap.rs:407 |
| Timeline : adapter à l'image courante | Palette ; aussi : cues.rs:274 « L'image courante » | — | Timeline : adapter à l'image courante `[timeline.adapter_image]` | src/keymap.rs:408 |
| Timeline : adapter aux clés de caméra | Palette ; aussi : cues.rs:275 « Les clés de caméra » ; handles.rs:1171 « Adapter la timeline aux clés » | — | Timeline : adapter aux clés de caméra `[timeline.adapter_cles]` | src/keymap.rs:409 |
| Timeline : tout voir | Palette ; aussi : cues.rs:276 « Tout voir » ; handles.rs:1146 « Timeline : tout voir » | — | Timeline : tout voir `[timeline.tout_voir]` | src/keymap.rs:410 |
| Marque de couleur sur l'instance courante (poser / retirer) | Palette ; aussi : marks.rs:194 « Poser / retirer sur l'instance courante » | — | Marque de couleur sur l'instance courante (poser / retirer) `[marque_couleur.basculer]` | src/keymap.rs:411 |
| Retirer la marque de couleur de l'instance courante | Palette ; aussi : marks.rs:195 « Retirer la marque de l'instance courante » | — | Retirer la marque de couleur de l'instance courante `[marque_couleur.retirer]` | src/keymap.rs:412 |
| Marque de couleur sur toutes les instances du calque | Palette ; aussi : marks.rs:196 « Marquer toutes les instances du calque » | — | Marque de couleur sur toutes les instances du calque `[marque_couleur.toutes]` | src/keymap.rs:413 |
| Marque de couleur sur les instances non marquées | Palette ; aussi : marks.rs:197 « Marquer les instances non marquées » | — | Marque de couleur sur les instances non marquées `[marque_couleur.non_marquees]` | src/keymap.rs:414 |
| Retirer les marques de couleur du calque | Palette ; aussi : marks.rs:198 « Retirer les marques du calque » | — | Retirer les marques de couleur du calque `[marque_couleur.effacer_calque]` | src/keymap.rs:415 |
| Retirer les marques de couleur du plan | Palette ; aussi : marks.rs:199 « Retirer les marques du plan » | — | Retirer les marques de couleur du plan `[marque_couleur.effacer_plan]` | src/keymap.rs:416 |
| Marque de couleur précédente | Palette ; aussi : marks.rs:201 « Marque précédente » | — | Marque de couleur précédente `[marque_couleur.precedente]` | src/keymap.rs:417 |
| Marque de couleur suivante | Palette ; aussi : marks.rs:202 « Marque suivante » | — | Marque de couleur suivante `[marque_couleur.suivante]` | src/keymap.rs:418 |
| Marque précédente de la couleur choisie | Palette ; aussi : marks.rs:203 « Marque précédente de cette couleur » | — | Marque précédente de la couleur choisie `[marque_couleur.precedente_couleur]` | src/keymap.rs:419 |
| Marque suivante de la couleur choisie | Palette ; aussi : marks.rs:204 « Marque suivante de cette couleur » | — | Marque suivante de la couleur choisie `[marque_couleur.suivante_couleur]` | src/keymap.rs:420 |
| Onglet Notes de la timeline (ouvrir / fermer) | Palette ; aussi : handles.rs:1188 « Onglet Notes (crayon, gomme, couleurs) » | — | Onglet Notes de la timeline (ouvrir / fermer) `[onglet.notes]` | src/keymap.rs:421 |
| Onglet Son de la timeline (ouvrir / fermer) | Palette ; aussi : handles.rs:1185 « Onglet Son (pistes, volumes) » | — | Onglet Son de la timeline (ouvrir / fermer) `[onglet.son]` | src/keymap.rs:422 |
| Onglet Groupes de la timeline (ouvrir / fermer) | Palette | — | Onglet Groupes de la timeline (ouvrir / fermer) `[onglet.groupes]` | src/keymap.rs:423 |
| Onglet Papier de la timeline (ouvrir / fermer) | Palette | — | Onglet Papier de la timeline (ouvrir / fermer) `[onglet.arriere_plan]` | src/keymap.rs:424 |
| Onglet Image de départ de la timeline (ouvrir / fermer) | Palette | — | Onglet Image de départ de la timeline (ouvrir / fermer) `[onglet.image_depart]` | src/keymap.rs:425 |
| Papier : aucun (transparent) | Palette | — | Papier : aucun (transparent) `[fond.aucun]` | src/keymap.rs:426 |
| Papier : couleur | Palette | — | Papier : couleur `[fond.couleur]` | src/keymap.rs:427 |
| Papier : damier | Palette | — | Papier : damier `[fond.damier]` | src/keymap.rs:428 |
| Nouveau plan avant le plan courant | Palette ; aussi : montagerows.rs:581 « Nouveau plan avant le plan courant » ; panels.rs:79 « Nouveau plan avant le plan courant » ; storyboard.rs:603 « Nouveau plan avant le plan courant » ; timelinesb.rs:212 « Nouveau plan avant le plan courant » | — | Nouveau plan avant le plan courant `[plan.nouveau_avant]` | src/keymap.rs:429 |
| Avancer le plan (plus tôt dans le montage) | Palette ; aussi : barres.rs:181 « Avancer le plan » | — | Avancer le plan (plus tôt dans le montage) `[plan.avancer]` | src/keymap.rs:430 |
| Reculer le plan (plus tard dans le montage) | Palette ; aussi : barres.rs:182 « Reculer le plan » | — | Reculer le plan (plus tard dans le montage) `[plan.reculer]` | src/keymap.rs:431 |
| Lecture en boucle (oui / non) | Palette ; aussi : barres.rs:171 « Lecture en boucle » | — | Lecture en boucle (oui / non) `[lecture.boucle]` | src/keymap.rs:432 |
| Début de la plage de lecture du film à l'image courante | Palette ; aussi : plagefilm.rs:145 « Début de la plage de lecture ici » | — | Début de la plage de lecture du film à l'image courante `[plage.debut]` | src/keymap.rs:434 |
| Fin de la plage de lecture du film à l'image courante | Palette ; aussi : plagefilm.rs:146 « Fin de la plage de lecture ici » | — | Fin de la plage de lecture du film à l'image courante `[plage.fin]` | src/keymap.rs:435 |
| Plage de lecture du film : les plans choisis | Palette | — | Plage de lecture du film : les plans choisis `[plage.plans_choisis]` | src/keymap.rs:436 |
| Enlever la plage de lecture du film | Palette ; aussi : plagefilm.rs:151 « Enlever la plage de lecture » | — | Enlever la plage de lecture du film `[plage.enlever]` | src/keymap.rs:437 |
| Durée du plan : taper la durée exacte… | Palette ; aussi : barres.rs:393 « Durée du plan » ; rognageui.rs:556 « Durée… » ; timelinesb.rs:217 « Durée du plan… » | — | Durée du plan : taper la durée exacte… `[montage.duree]` | src/keymap.rs:439 |
| Changer la durée des plans choisis… | Palette ; aussi : boardui.rs:2738 « Changer la durée des {n} plans choisis… » ; panels.rs:102 « Changer la durée des plans choisis… » ; storyboard.rs:616 « Changer la durée des {n} plans choisis… » ; storyboard.rs:834 « Changer la durée des plans choisis… » | — | Changer la durée des plans choisis… `[montage.duree_plans]` | src/keymap.rs:440 |
| Modifier la durée des clés d'images… | Palette ; aussi : images.rs:1150 « Modifier la durée des clés d'images… » ; panels.rs:195 « Modifier la durée des clés d'images… » ; storyboard.rs:2020 « Modifier la durée des clés d'images… » | — | Modifier la durée des clés d'images… `[cles.duree]` | src/keymap.rs:441 |
| Raccourcir le début du plan jusqu'à la tête de lecture | Palette ; aussi : barres.rs:391 « Rogner le début » ; rognageui.rs:557 « Raccourcir le début jusqu'à la tête de lecture » ; storyboard.rs:2057 « Raccourcir le début du plan jusqu'ici » | — | Raccourcir le début du plan jusqu'à la tête de lecture `[montage.debut_a_tete]` | src/keymap.rs:442 |
| Raccourcir la fin du plan jusqu'à la tête de lecture | Palette ; aussi : barres.rs:392 « Rogner la fin » ; rognageui.rs:558 « Raccourcir la fin jusqu'à la tête de lecture » ; storyboard.rs:2058 « Raccourcir la fin du plan jusqu'ici » | — | Raccourcir la fin du plan jusqu'à la tête de lecture `[montage.fin_a_tete]` | src/keymap.rs:443 |
| Rétablir le contenu rogné du plan (début et fin) | Palette ; aussi : rognageui.rs:561 « Rétablir le contenu rogné ({h} i au début, {t} i à la fin) » | — | Rétablir le contenu rogné du plan (début et fin) `[montage.rognage_retablir]` | src/keymap.rs:444 |
| Oublier le contenu rogné du plan | Palette ; aussi : rognageui.rs:562 « Oublier le contenu rogné » | — | Oublier le contenu rogné du plan `[montage.rognage_oublier]` | src/keymap.rs:445 |
| Aimantation des bords rognés dans le montage (oui / non) | Palette | — | Aimantation des bords rognés dans le montage (oui / non) `[montage.aimanter]` | src/keymap.rs:446 |
| Pistes du montage : replier / déplier la piste du calque courant | Palette | — | Pistes du montage : replier / déplier la piste du calque courant `[piste.basculer]` | src/keymap.rs:448 |
| Pistes du montage : replier toutes les pistes | Palette ; aussi : pistes.rs:1179 « Replier toutes les pistes » | — | Pistes du montage : replier toutes les pistes `[piste.replier_tout]` | src/keymap.rs:449 |
| Colonne des calques de la timeline : replier tous les calques sur une ligne (ou les déplier) | Palette | `Alt+Maj+C` | Colonne des calques de la timeline : replier tous les calques sur une ligne (ou les déplier) `[calques.replier_tout]` | src/keymap.rs:450 |
| Pistes du montage : déplier toutes les pistes | Palette ; aussi : barres.rs:226 « Déplier toutes les pistes » ; pistes.rs:1180 « Déplier toutes les pistes » | — | Pistes du montage : déplier toutes les pistes `[piste.deplier_tout]` | src/keymap.rs:451 |
| Pistes du montage : déplier tout seul le calque courant (oui / non) | Palette | — | Pistes du montage : déplier tout seul le calque courant (oui / non) `[piste.auto_deplier]` | src/keymap.rs:452 |
| Plans du montage : en bandeau / de hauteur moyenne | Palette | — | Plans du montage : en bandeau / de hauteur moyenne `[bande.plans_en_bandeau]` | src/keymap.rs:453 |
| Verrouiller / déverrouiller le storyboard | Palette ; aussi : montagerows.rs:588 « Déverrouiller le storyboard » ; storyboard.rs:2049 « Verrouiller / déverrouiller le storyboard » ; timelinesb.rs:226 « Déverrouiller le storyboard » | — | Verrouiller / déverrouiller le storyboard `[storyboard.verrouiller]` | src/keymap.rs:454 |
| Réglages du projet (propriétés, résolution, cadence) | Palette ; aussi : montagerows.rs:599 « Réglages du projet » ; panels.rs:153 « Réglages du projet… » ; timelinesb.rs:225 « Réglages du projet… » | — | Réglages du projet (propriétés, résolution, cadence) `[projet.proprietes]` | src/keymap.rs:455 |
| Nouveau projet à partir des plans choisis | Palette ; aussi : panels.rs:105 « Nouveau projet à partir des plans choisis » | — | Nouveau projet à partir des plans choisis `[projet.nouveau_depuis_plans]` | src/keymap.rs:456 |
| Panneau de la pelure d'oignon (ouvrir / fermer) | Palette | — | Panneau de la pelure d'oignon (ouvrir / fermer) `[pelure.panneau]` | src/keymap.rs:457 |
| Pelure d'oignon devant le dessin (oui / non) | Palette | — | Pelure d'oignon devant le dessin (oui / non) `[pelure.devant]` | src/keymap.rs:458 |
| Lier les curseurs d'opacité de la pelure (oui / non) | Palette | — | Lier les curseurs d'opacité de la pelure (oui / non) `[pelure.lier]` | src/keymap.rs:459 |
| Pelure d'oignon : montrer toutes les images | Palette | — | Pelure d'oignon : montrer toutes les images `[pelure.activer_tout]` | src/keymap.rs:460 |
| Pelure d'oignon : cacher toutes les images | Palette | — | Pelure d'oignon : cacher toutes les images `[pelure.desactiver_tout]` | src/keymap.rs:461 |
| Pelure d'oignon : mode instance | Palette | — | Pelure d'oignon : mode instance `[pelure.mode_instance]` | src/keymap.rs:462 |
| Pelure d'oignon : mode image | Palette | — | Pelure d'oignon : mode image `[pelure.mode_image]` | src/keymap.rs:463 |
| Pelure d'oignon : mode signet | Palette | — | Pelure d'oignon : mode signet `[pelure.mode_signet]` | src/keymap.rs:464 |
| Pelure d'oignon : mode marque de couleur | Palette | — | Pelure d'oignon : mode marque de couleur `[pelure.mode_marque]` | src/keymap.rs:465 |
| Décaler les images de la pelure d'oignon (glisser sur le dessin) | Palette ; aussi : onionui.rs:123 « Décaler la pelure » | — | Décaler les images de la pelure d'oignon (glisser sur le dessin) `[pelure.decaler]` | src/keymap.rs:466 |
| Remettre les images de la pelure d'oignon en place | Palette | — | Remettre les images de la pelure d'oignon en place `[pelure.remettre]` | src/keymap.rs:467 |
| Panneau Feuille d'exposition (ouvrir / fermer) | Palette | — | Panneau Feuille d'exposition (ouvrir / fermer) `[vue.feuille_exposition]` | src/keymap.rs:468 |
| Panneau Historique des annulations (ouvrir / fermer) | Palette ; aussi : panels.rs:168 « Historique… » | — | Panneau Historique des annulations (ouvrir / fermer) `[vue.historique]` | src/keymap.rs:469 |
| Panneau Commentaires (ouvrir / fermer) | Palette ; aussi : panels.rs:198 « Commentaires… » | — | Panneau Commentaires (ouvrir / fermer) `[vue.commentaires]` | src/keymap.rs:471 |
| Nouveau commentaire (à l'image courante) | Palette ; aussi : handles.rs:1135 « Commentaire ici » ; panels.rs:197 « Nouveau commentaire ici » ; storyboard.rs:2055 « Commentaire ici » | — | Nouveau commentaire (à l'image courante) `[commentaire.ajouter]` | src/keymap.rs:472 |
| Commentaire choisi : résolu / rouvert | Palette | — | Commentaire choisi : résolu / rouvert `[commentaire.resoudre]` | src/keymap.rs:473 |
| Supprimer le commentaire choisi | Palette | — | Supprimer le commentaire choisi `[commentaire.supprimer]` | src/keymap.rs:474 |
| Aller au commentaire précédent | Palette ; aussi : commentairesui.rs:502 « Commentaire précédent » | — | Aller au commentaire précédent `[commentaire.precedent]` | src/keymap.rs:475 |
| Aller au commentaire suivant | Palette ; aussi : commentairesui.rs:503 « Commentaire suivant » | — | Aller au commentaire suivant `[commentaire.suivant]` | src/keymap.rs:476 |
| Aller à la note suivante à traiter (à faire ou en discussion) | Palette ; aussi : fildecisionui.rs:249 « Note suivante à traiter » | — | Aller à la note suivante à traiter (à faire ou en discussion) `[notes.suivante_a_traiter]` | src/keymap.rs:478 |
| Fil du plan courant (panneau Commentaires) | Palette ; aussi : fildecisionui.rs:262 « Fil du plan » | — | Fil du plan courant (panneau Commentaires) `[notes.fil]` | src/keymap.rs:479 |
| Note choisie : à faire | Palette | — | Note choisie : à faire `[notes.statut_a_faire]` | src/keymap.rs:480 |
| Note choisie : fait | Palette | — | Note choisie : fait `[notes.statut_fait]` | src/keymap.rs:481 |
| Note choisie : refusé (la raison est demandée) | Palette | — | Note choisie : refusé (la raison est demandée) `[notes.statut_refuse]` | src/keymap.rs:482 |
| Note choisie : en discussion | Palette | — | Note choisie : en discussion `[notes.statut_discussion]` | src/keymap.rs:483 |
| Note choisie : retake (oui / non) | Palette | — | Note choisie : retake (oui / non) `[notes.retake]` | src/keymap.rs:484 |
| Note choisie : marquer en conflit avec une autre note | Palette | — | Note choisie : marquer en conflit avec une autre note `[notes.conflit]` | src/keymap.rs:485 |
| Exporter la liste des retakes (CSV) | Palette ; aussi : fildecisionui.rs:250 « Liste des retakes (CSV) » | — | Exporter la liste des retakes (CSV) `[notes.retakes_exporter]` | src/keymap.rs:486 |
| Panneau Références (ouvrir / fermer) | Palette | — | Panneau Références (ouvrir / fermer) `[vue.references]` | src/keymap.rs:488 |
| Importer des images dans les références | Palette ; aussi : panels.rs:143 « Des images comme références… » ; referencesui.rs:783 « Importer des images » | — | Importer des images dans les références `[references.importer_image]` | src/keymap.rs:489 |
| Importer une vidéo dans les références | Palette ; aussi : panels.rs:144 « Une vidéo comme référence… » ; referencesui.rs:786 « Importer une vidéo » | — | Importer une vidéo dans les références `[references.importer_video]` | src/keymap.rs:490 |
| Coller l'image du presse-papiers dans les références | Palette ; aussi : referencesui.rs:789 « Coller une image » | — | Coller l'image du presse-papiers dans les références `[references.coller]` | src/keymap.rs:491 |
| Supprimer les références choisies | Palette ; aussi : planchrefui.rs:554 « Supprimer » | — | Supprimer les références choisies `[references.supprimer]` | src/keymap.rs:492 |
| Lier les références choisies au plan courant (ou délier) | Palette ; aussi : planchrefui.rs:551 « Lier au plan courant » | — | Lier les références choisies au plan courant (ou délier) `[references.lier_plan]` | src/keymap.rs:493 |
| Références : montrer seulement celles du plan courant (oui / non) | Palette ; aussi : referencesui.rs:820 « Références du plan courant » | — | Références : montrer seulement celles du plan courant (oui / non) `[references.filtre_plan]` | src/keymap.rs:494 |
| Références : tout voir sur le tableau | Palette ; aussi : referencesui.rs:823 « Tout voir » | — | Références : tout voir sur le tableau `[references.tout_voir]` | src/keymap.rs:495 |
| Références : poser un post-it | Palette ; aussi : planchrefui.rs:409 « Post-it » | — | Références : poser un post-it `[references.postit]` | src/keymap.rs:496 |
| Références : poser un texte | Palette ; aussi : planchrefui.rs:412 « Texte » | — | Références : poser un texte `[references.texte]` | src/keymap.rs:497 |
| Références : poser un titre | Palette ; aussi : planchrefui.rs:415 « Titre » | — | Références : poser un titre `[references.titre]` | src/keymap.rs:498 |
| Références : poser un cadre (autour de la sélection s'il y en a une) | Palette ; aussi : planchrefui.rs:418 « Cadre » | — | Références : poser un cadre (autour de la sélection s'il y en a une) `[references.cadre]` | src/keymap.rs:499 |
| Références : ranger la sélection (ou toute la planche), serré | Palette ; aussi : planchrefui.rs:429 « Ranger » | — | Références : ranger la sélection (ou toute la planche), serré `[references.ranger]` | src/keymap.rs:500 |
| Références : recadrer l'image choisie (oui / non) | Palette ; aussi : planchrefui.rs:534 « Recadrer » | — | Références : recadrer l'image choisie (oui / non) `[references.recadrer]` | src/keymap.rs:501 |
| Références : retourner de gauche à droite | Palette ; aussi : planchrefui.rs:524 « Retourner de gauche à droite » | — | Références : retourner de gauche à droite `[references.retourner_h]` | src/keymap.rs:502 |
| Références : retourner de haut en bas | Palette ; aussi : planchrefui.rs:527 « Retourner de haut en bas » | — | Références : retourner de haut en bas `[references.retourner_v]` | src/keymap.rs:503 |
| Références : noir et blanc (oui / non) | Palette ; aussi : planchrefui.rs:530 « Noir et blanc » | — | Références : noir et blanc (oui / non) `[references.noir_blanc]` | src/keymap.rs:504 |
| Références : mettre devant | Palette ; aussi : planchrefui.rs:545 « Mettre devant » | — | Références : mettre devant `[references.devant]` | src/keymap.rs:505 |
| Références : mettre derrière | Palette ; aussi : planchrefui.rs:548 « Mettre derrière » | — | Références : mettre derrière `[references.derriere]` | src/keymap.rs:506 |
| Références : tout sélectionner | Palette | — | Références : tout sélectionner `[references.tout_selectionner]` | src/keymap.rs:507 |
| Références : importer la planche d'un autre projet… | Palette | — | Références : importer la planche d'un autre projet… `[references.importer_planche]` | src/keymap.rs:508 |
| Références : tirer sur le fond dessine un rectangle de sélection (oui / non) | Palette ; aussi : planchrefui.rs:426 « Rectangle de sélection » | — | Références : tirer sur le fond dessine un rectangle de sélection (oui / non) `[references.rectangle]` | src/keymap.rs:509 |
| Références : crayon de la planche (oui / non) | Palette ; aussi : planchdessinui.rs:127 « Crayon » | — | Références : crayon de la planche (oui / non) `[references.crayon]` | src/keymap.rs:510 |
| Références : gomme de la planche (oui / non) | Palette ; aussi : planchdessinui.rs:130 « Gomme de la planche » | — | Références : gomme de la planche (oui / non) `[references.gomme]` | src/keymap.rs:511 |
| Références : détacher la planche dans sa fenêtre | Palette ; aussi : planchrefui.rs:451 « Détacher » | — | Références : détacher la planche dans sa fenêtre `[references.detacher]` | src/keymap.rs:512 |
| Références : plein écran de la fenêtre à part (oui / non) | Palette | — | Références : plein écran de la fenêtre à part (oui / non) `[references.plein_ecran]` | src/keymap.rs:513 |
| Références : plein écran sur l'écran 2 | Palette | — | Références : plein écran sur l'écran 2 `[references.ecran_2]` | src/keymap.rs:514 |
| Références : fenêtre à part transparente (oui / non) | Palette ; aussi : planchrefui.rs:487 « Fenêtre transparente » | — | Références : fenêtre à part transparente (oui / non) `[references.transparente]` | src/keymap.rs:515 |
| Références : fenêtre à part toujours au-dessus (oui / non) | Palette ; aussi : planchrefui.rs:484 « Toujours au-dessus » | — | Références : fenêtre à part toujours au-dessus (oui / non) `[references.toujours_dessus]` | src/keymap.rs:516 |
| Panneau Brosses dédiées (ouvrir / fermer) | Palette | — | Panneau Brosses dédiées (ouvrir / fermer) `[vue.personnages]` | src/keymap.rs:518 |
| Pastilles des brosses dédiées dans la barre du haut (montrer / cacher, dans cet espace de travail) | Palette | — | Pastilles des brosses dédiées dans la barre du haut (montrer / cacher, dans cet espace de travail) `[perso.barre]` | src/keymap.rs:519 |
| Nouvelle brosse personnage | Palette | — | Nouvelle brosse personnage `[perso.ajouter]` | src/keymap.rs:520 |
| Nouvelle brosse classique | Palette | — | Nouvelle brosse classique `[perso.ajouter_classique]` | src/keymap.rs:521 |
| Nouvelle brosse décor | Palette | — | Nouvelle brosse décor `[perso.ajouter_decor]` | src/keymap.rs:522 |
| Brosse dédiée suivante (trait et aplat) | Palette | — | Brosse dédiée suivante (trait et aplat) `[perso.suivant]` | src/keymap.rs:523 |
| Brosse dédiée précédente (trait et aplat) | Palette | — | Brosse dédiée précédente (trait et aplat) `[perso.precedent]` | src/keymap.rs:524 |
| Lâcher la brosse dédiée (couleur libre, dans le calque courant) | Palette | — | Lâcher la brosse dédiée (couleur libre, dans le calque courant) `[perso.aucun]` | src/keymap.rs:525 |
| Panneau Planche (ouvrir / fermer) | Palette | — | Panneau Planche (ouvrir / fermer) `[vue.vignettes_panneau]` | src/keymap.rs:527 |
| Vignettes : lire en lignes (de gauche à droite, le texte sous les cases) | Palette | — | Vignettes : lire en lignes (de gauche à droite, le texte sous les cases) `[vignettes.sens_lignes]` | src/keymap.rs:528 |
| Vignettes : lire à la japonaise (en colonnes, le texte à droite des cases) | Palette | — | Vignettes : lire à la japonaise (en colonnes, le texte à droite des cases) `[vignettes.sens_japonais]` | src/keymap.rs:529 |
| Vignettes : montrer ou cacher les numéros de plan | Palette ; aussi : panneauxvisuelsui.rs:503 « Numéros de plan » ; vignettesprefs.rs:189 « Montrer les numéros de plan » | `N` | Vignettes : montrer ou cacher les numéros de plan `[vignettes.numeros]` | src/keymap.rs:530 |
| Vignettes : montrer l'action à côté des cases (oui / non) | Palette | — | Vignettes : montrer l'action à côté des cases (oui / non) `[vignettes.texte_action]` | src/keymap.rs:531 |
| Vignettes : montrer le dialogue à côté des cases (oui / non) | Palette | — | Vignettes : montrer le dialogue à côté des cases (oui / non) `[vignettes.texte_dialogue]` | src/keymap.rs:532 |
| Vignettes : montrer les notes à côté des cases (oui / non) | Palette | — | Vignettes : montrer les notes à côté des cases (oui / non) `[vignettes.texte_notes]` | src/keymap.rs:533 |
| Panneau Réglages de l'outil (ouvrir / fermer) | Palette | — | Panneau Réglages de l'outil (ouvrir / fermer) `[vue.reglages]` | src/keymap.rs:535 |
| Liste des pinceaux, dans les réglages (déplier / replier) | Palette | — | Liste des pinceaux, dans les réglages (déplier / replier) `[vue.pinceaux]` | src/keymap.rs:538 |
| Panneau Couleur (ouvrir / fermer) | Palette | — | Panneau Couleur (ouvrir / fermer) `[vue.couleur]` | src/keymap.rs:539 |
| Calques (la colonne de la timeline) | Palette | — | Calques (la colonne de la timeline) `[vue.calques]` | src/keymap.rs:540 |
| Panneau Fiche du plan (ouvrir / fermer) | Palette | — | Panneau Fiche du plan (ouvrir / fermer) `[vue.fiche]` | src/keymap.rs:541 |
| Rôle des images visées : clé | Palette | — | Rôle des images visées : clé `[role.cle]` | src/keymap.rs:542 |
| Rôle des images visées : breakdown | Palette | — | Rôle des images visées : breakdown `[role.breakdown]` | src/keymap.rs:543 |
| Rôle des images visées : intervalle à faire | Palette | — | Rôle des images visées : intervalle à faire `[role.intervalle]` | src/keymap.rs:544 |
| Rôle des images visées : intervalle complété | Palette | — | Rôle des images visées : intervalle complété `[role.intervalle_complete]` | src/keymap.rs:545 |
| Rôle des images visées : intervalle spécial à faire | Palette | — | Rôle des images visées : intervalle spécial à faire `[role.intervalle_special]` | src/keymap.rs:546 |
| Rôle des images visées : intervalle spécial complété | Palette | — | Rôle des images visées : intervalle spécial complété `[role.intervalle_special_complete]` | src/keymap.rs:547 |
| Retirer le rôle des images visées | Palette ; aussi : rolesui.rs:70 « Aucun rôle » | — | Retirer le rôle des images visées `[role.aucun]` | src/keymap.rs:548 |
| Montrer / cacher les rôles dans la timeline | Palette | — | Montrer / cacher les rôles dans la timeline `[role.timeline]` | src/keymap.rs:549 |
| Rôles sur le calque courant seulement (oui / non) | Palette | — | Rôles sur le calque courant seulement (oui / non) `[role.calque_courant]` | src/keymap.rs:550 |
| Panneau des flips (ouvrir / fermer) | Palette ; aussi : barres.rs:361 « Flips » | — | Panneau des flips (ouvrir / fermer) `[flip.panneau]` | src/keymap.rs:551 |
| Flipper avec le flip courant (maintenir) | Palette | — | Flipper avec le flip courant (maintenir) `[flip.jouer]` | src/keymap.rs:552 |
| Flip manuel : image précédente du flip | Palette | — | Flip manuel : image précédente du flip `[flip.precedent]` | src/keymap.rs:553 |
| Flip manuel : image suivante du flip | Palette | — | Flip manuel : image suivante du flip `[flip.suivant]` | src/keymap.rs:554 |
| Passer au flip suivant de la liste | Palette | — | Passer au flip suivant de la liste `[flip.changer]` | src/keymap.rs:555 |
| Flipper les clés (maintenir) | Palette | — | Flipper les clés (maintenir) `[flip.cles]` | src/keymap.rs:556 |
| Flipper les breakdowns (maintenir) | Palette | — | Flipper les breakdowns (maintenir) `[flip.breakdowns]` | src/keymap.rs:557 |
| Flipper les intervalles à faire (maintenir) | Palette | — | Flipper les intervalles à faire (maintenir) `[flip.intervalles]` | src/keymap.rs:558 |
| Flipper les intervalles complétés (maintenir) | Palette | — | Flipper les intervalles complétés (maintenir) `[flip.intervalles_completes]` | src/keymap.rs:559 |
| Flipper les intervalles spéciaux à faire (maintenir) | Palette | — | Flipper les intervalles spéciaux à faire (maintenir) `[flip.speciaux]` | src/keymap.rs:560 |
| Flipper les intervalles spéciaux complétés (maintenir) | Palette | — | Flipper les intervalles spéciaux complétés (maintenir) `[flip.speciaux_completes]` | src/keymap.rs:561 |
| Pile d'effets du calque (ouvrir / fermer) | Palette ; aussi : fxkeys.rs:802 « Pile d'effets du calque… » | — | Pile d'effets du calque (ouvrir / fermer) `[fx.panneau]` | src/keymap.rs:562 |
| Créer une clé de l'effet choisi à l'image courante | Palette | — | Créer une clé de l'effet choisi à l'image courante `[fx.cle_creer]` | src/keymap.rs:563 |
| Supprimer la clé de l'effet choisi à l'image courante | Palette | — | Supprimer la clé de l'effet choisi à l'image courante `[fx.cle_supprimer]` | src/keymap.rs:564 |
| Création automatique des clés d'effets (oui / non) | Palette | — | Création automatique des clés d'effets (oui / non) `[fx.cle_auto]` | src/keymap.rs:565 |
| Clés d'effets dans la timeline et le montage (montrer / cacher) | Palette ; aussi : fxkeys.rs:803 « Clés d'effets (montrer / cacher) » | — | Clés d'effets dans la timeline et le montage (montrer / cacher) `[fx.cles_timeline]` | src/keymap.rs:566 |
| Ne choisir aucun effet (les clés d'effets se cachent dans la timeline et le montage) | Palette | — | Ne choisir aucun effet (les clés d'effets se cachent dans la timeline et le montage) `[fx.choisir_aucun]` | src/keymap.rs:567 |
| Copier les clés d'effets choisies | Palette | — | Copier les clés d'effets choisies `[fx.cles_copier]` | src/keymap.rs:568 |
| Coller les clés d'effets (insérées après la clé de la tête de lecture) | Palette | — | Coller les clés d'effets (insérées après la clé de la tête de lecture) `[fx.cles_coller]` | src/keymap.rs:569 |
| Supprimer les clés d'effets choisies (rien sans clé choisie) | Palette | — | Supprimer les clés d'effets choisies (rien sans clé choisie) `[fx.cles_supprimer]` | src/keymap.rs:570 |
| Copier l'effet choisi dans le panneau Effets (réglages et clés) | Palette ; aussi : fxcopie.rs:532 « Copier l'effet » | — | Copier l'effet choisi dans le panneau Effets (réglages et clés) `[fx.copier]` | src/keymap.rs:571 |
| Copier tous les effets du calque courant | Palette ; aussi : fxcopie.rs:536 « Copier tous les effets du calque » | — | Copier tous les effets du calque courant `[fx.copier_tout]` | src/keymap.rs:572 |
| Coller les effets copiés sur le calque courant (ou les calques choisis) | Palette | — | Coller les effets copiés sur le calque courant (ou les calques choisis) `[fx.coller]` | src/keymap.rs:573 |
| Remplacer les effets du calque courant (ou des calques choisis) par ceux copiés | Palette ; aussi : fxcopie.rs:553 « Remplacer les effets » | — | Remplacer les effets du calque courant (ou des calques choisis) par ceux copiés `[fx.remplacer]` | src/keymap.rs:574 |
| Tremblement du trait du calque courant (boil) | Palette | — | Tremblement du trait du calque courant (boil) `[fx.tremblement]` | src/keymap.rs:575 |
| Tremblement du trait : animer aussi à l'arrêt (oui / non) | Palette | — | Tremblement du trait : animer aussi à l'arrêt (oui / non) `[fx.tremblement_repos]` | src/keymap.rs:576 |
| Écriture du trait du calque courant (le dessin se trace de lui-même) | Palette | — | Écriture du trait du calque courant (le dessin se trace de lui-même) `[fx.ecriture]` | src/keymap.rs:577 |
| Crayonné multiple du calque courant (chaque trait redoublé de passes) | Palette | — | Crayonné multiple du calque courant (chaque trait redoublé de passes) `[fx.crayonne]` | src/keymap.rs:578 |
| Respiration du trait du calque courant (l'épaisseur pulse) | Palette | — | Respiration du trait du calque courant (l'épaisseur pulse) `[fx.respiration]` | src/keymap.rs:579 |
| Ondulation du trait du calque courant (une vague le long des traits) | Palette | — | Ondulation du trait du calque courant (une vague le long des traits) `[fx.ondulation_trait]` | src/keymap.rs:580 |
| Pointillés qui défilent sur les traits du calque courant | Palette | — | Pointillés qui défilent sur les traits du calque courant `[fx.pointilles]` | src/keymap.rs:581 |
| Désagrégation du trait du calque courant (le trait part en poussière) | Palette | — | Désagrégation du trait du calque courant (le trait part en poussière) `[fx.desagregation]` | src/keymap.rs:582 |
| Préréglage du trait : apparition écrite (clés dès l'image courante) | Palette | — | Préréglage du trait : apparition écrite (clés dès l'image courante) `[fx.preset_apparition]` | src/keymap.rs:583 |
| Préréglage du trait : croquis nerveux (crayonné et léger tremblement) | Palette | — | Préréglage du trait : croquis nerveux (crayonné et léger tremblement) `[fx.preset_croquis]` | src/keymap.rs:584 |
| Préréglage du trait : poussière (clés dès l'image courante) | Palette | — | Préréglage du trait : poussière (clés dès l'image courante) `[fx.preset_poussiere]` | src/keymap.rs:585 |
| Liseré du calque courant (un contour d'une autre couleur autour du dessin) | Palette | — | Liseré du calque courant (un contour d'une autre couleur autour du dessin) `[fx.lisere]` | src/keymap.rs:586 |
| Trait sec du calque courant (trous et bord qui s'effrite) | Palette | — | Trait sec du calque courant (trous et bord qui s'effrite) `[fx.trait_sec]` | src/keymap.rs:587 |
| Encre qui bave sur le calque courant (bord irrégulier, comme de l'encre sur du papier buvard) | Palette | — | Encre qui bave sur le calque courant (bord irrégulier, comme de l'encre sur du papier buvard) `[fx.encre_bave]` | src/keymap.rs:588 |
| Dégradé le long du trait du calque courant (la couleur change du début à la fin des traits) | Palette | — | Dégradé le long du trait du calque courant (la couleur change du début à la fin des traits) `[fx.degrade_trait]` | src/keymap.rs:589 |
| Changer de plume sur le calque courant (les traits redessinés avec un autre pinceau) | Palette | — | Changer de plume sur le calque courant (les traits redessinés avec un autre pinceau) `[fx.plume]` | src/keymap.rs:590 |
| Appliquer pour de bon la plume du calque courant (remplace les traits, annulable) | Palette | — | Appliquer pour de bon la plume du calque courant (remplace les traits, annulable) `[fx.plume_appliquer]` | src/keymap.rs:591 |
| Préréglage de la matière : encre de Chine (encre qui bave) | Palette | — | Préréglage de la matière : encre de Chine (encre qui bave) `[fx.preset_encre_chine]` | src/keymap.rs:592 |
| Préréglage de la matière : feutre usé (plume feutre et trait sec) | Palette | — | Préréglage de la matière : feutre usé (plume feutre et trait sec) `[fx.preset_feutre_use]` | src/keymap.rs:593 |
| Préréglage de la matière : contour blanc (liseré) | Palette | — | Préréglage de la matière : contour blanc (liseré) `[fx.preset_contour_blanc]` | src/keymap.rs:594 |
| Préréglage de la matière : arc-en-ciel (dégradé sur tout le dessin) | Palette | — | Préréglage de la matière : arc-en-ciel (dégradé sur tout le dessin) `[fx.preset_arc_en_ciel]` | src/keymap.rs:595 |
| Préréglage de la matière : plume calligraphique (changer de plume) | Palette | — | Préréglage de la matière : plume calligraphique (changer de plume) `[fx.preset_plume_calligraphique]` | src/keymap.rs:596 |
| Nouveau calque de couleur (sous le trait du calque courant) | Palette ; aussi : calquecouleurui.rs:327 « Ajouter un calque de couleur sous ce trait » ; calquecouleurui.rs:367 « Ajouter un autre calque de couleur sous ce trait » | — | Nouveau calque de couleur (sous le trait du calque courant) `[calque.nouveau_couleur]` | src/keymap.rs:597 |
| Nouveau calque d'annotation (des notes par-dessus, jamais dans les exports) | Palette | — | Nouveau calque d'annotation (des notes par-dessus, jamais dans les exports) `[calque.nouveau_annotation]` | src/keymap.rs:598 |
| Nouveau calque 3D : Sol | Palette, **seulement vue Espace 3D ouverte** | — | Nouveau calque 3D : Sol `[calque.nouveau_sol]` | src/keymap.rs:599 |
| Nouveau calque 3D : Mur | Palette, **seulement vue Espace 3D ouverte** | — | Nouveau calque 3D : Mur `[calque.nouveau_mur]` | src/keymap.rs:600 |
| Nouveau calque 3D : Ciel | Palette, **seulement vue Espace 3D ouverte** | — | Nouveau calque 3D : Ciel `[calque.nouveau_ciel]` | src/keymap.rs:601 |
| Grille de repère du calque Sol, Mur ou Ciel courant (oui / non) | Palette ; aussi : espace3ddecor.rs:462 « Grille » | — | Grille de repère du calque Sol, Mur ou Ciel courant (oui / non) `[calque.grille_3d]` | src/keymap.rs:602 |
| Détacher le calque de couleur de son trait | Palette ; aussi : calquecouleurui.rs:368 « Détacher du calque de trait » | — | Détacher le calque de couleur de son trait `[calque.detacher_couleur]` | src/keymap.rs:603 |
| Calque de couleur : griffonner (les couleurs posées remplissent les zones fermées par le trait) | Palette | — | Calque de couleur : griffonner (les couleurs posées remplissent les zones fermées par le trait) `[calque.griffonner]` | src/keymap.rs:604 |
| Importer une vidéo (calque vidéo) | Palette ; aussi : panels.rs:137 « Une vidéo (calque vidéo)… » | — | Importer une vidéo (calque vidéo) `[fichier.importer_video]` | src/keymap.rs:605 |
| Ajouter un objet 3D au décor de la séquence (glTF, .glb ou .gltf) | Palette, **seulement vue Espace 3D ouverte** ; aussi : panels.rs:139 « Objet 3D pour le décor de la séquence (glTF, .glb)… » | — | Ajouter un objet 3D au décor de la séquence (glTF, .glb ou .gltf) `[fichier.importer_decor3d]` | src/keymap.rs:606 |
| Enlever le décor 3D de la séquence (tous ses objets) | Palette, **seulement vue Espace 3D ouverte** ; aussi : panels.rs:141 « Enlever le décor 3D de la séquence » | — | Enlever le décor 3D de la séquence (tous ses objets) `[decor3d.enlever]` | src/keymap.rs:607 |
| Remesurer le budget 3D de cet ordinateur (décor 3D) | Palette, **seulement vue Espace 3D ouverte** | — | Remesurer le budget 3D de cet ordinateur (décor 3D) `[decor3d.remesurer]` | src/keymap.rs:608 |
| Importer un scénario et le garder dans le projet (Fountain, .fdx, .docx, texte) | Palette ; aussi : panels.rs:146 « Un scénario, à garder dans le projet (Fountain, .fdx, .docx, texte)… » ; scenariopanneau.rs:391 « Importer un scénario » | — | Importer un scénario et le garder dans le projet (Fountain, .fdx, .docx, texte) `[fichier.importer_scenario]` | src/keymap.rs:609 |
| Panneau Scénario (ouvrir / fermer) | Palette | — | Panneau Scénario (ouvrir / fermer) `[vue.scenario]` | src/keymap.rs:610 |
| Créer les plans depuis le scénario gardé dans le projet… | Palette | — | Créer les plans depuis le scénario gardé dans le projet… `[scenario.creer_plans]` | src/keymap.rs:611 |
| Enregistrer mes traits (fonction expérimentale : banc d'essai du moteur de dessin) | Palette | — | Enregistrer mes traits (fonction expérimentale : banc d'essai du moteur de dessin) `[pinceau.enregistrer_traits]` | src/keymap.rs:612 |
| Importer une séquence d'images (calque vidéo) | Palette ; aussi : panels.rs:138 « Une séquence d'images (calque vidéo)… » | — | Importer une séquence d'images (calque vidéo) `[fichier.importer_sequence]` | src/keymap.rs:613 |
| Transformer le calque vidéo en calque d'animation | Palette ; aussi : videoui.rs:185 « Transformer en calque d'animation » | — | Transformer le calque vidéo en calque d'animation `[calque.video_en_animation]` | src/keymap.rs:614 |
| Propager la peinture du calque en suivant une vidéo ou une animation (de clé en clé) | Palette ; aussi : panels.rs:251 « Propager la peinture du calque… » ; propagerui.rs:160 « Propager la peinture… » | — | Propager la peinture du calque en suivant une vidéo ou une animation (de clé en clé) `[calque.propager]` | src/keymap.rs:615 |
| Colorier par zones (reporter les aplats des clés sur les autres images) | Palette ; aussi : panels.rs:250 « Colorier par zones… » | — | Colorier par zones (reporter les aplats des clés sur les autres images) `[calque.colorier_zones]` | src/keymap.rs:616 |
| Insérer une image intermédiaire (entre le dessin courant et le suivant) | Palette ; aussi : images.rs:1186 « Insérer une image intermédiaire » | — | Insérer une image intermédiaire (entre le dessin courant et le suivant) `[images.intermediaire]` | src/keymap.rs:617 |
| Insérer des images intermédiaires… | Palette ; aussi : images.rs:1187 « Insérer des images intermédiaires… » | — | Insérer des images intermédiaires… `[images.intermediaires]` | src/keymap.rs:618 |
| Remplir les images vides par des intervalles | Palette ; aussi : images.rs:1188 « Remplir les vides par des intervalles » | — | Remplir les images vides par des intervalles `[images.intervalles]` | src/keymap.rs:619 |
| Étirer des images en interpolant… | Palette ; aussi : images.rs:1189 « Étirer en interpolant… » | — | Étirer des images en interpolant… `[images.etirer_interpoler]` | src/keymap.rs:620 |
| Changer la cadence du projet en gardant les durées… | Palette ; aussi : images.rs:1191 « Changer la cadence du projet (durées gardées)… » | — | Changer la cadence du projet en gardant les durées… `[projet.changer_cadence]` | src/keymap.rs:621 |
| Nouveau dossier de calques | Palette ; aussi : menucalque.rs:182 « Nouveau dossier » | — | Nouveau dossier de calques `[dossier.nouveau]` | src/keymap.rs:622 |
| Placer les calques choisis dans un nouveau dossier | Palette ; aussi : menucalque.rs:183 « Placer les calques choisis dans un dossier » | — | Placer les calques choisis dans un nouveau dossier `[dossier.ranger]` | src/keymap.rs:623 |
| Déposer les calques choisis dans le dossier | Palette ; aussi : menucalque.rs:184 « Déposer les calques choisis dans le dossier » | — | Déposer les calques choisis dans le dossier `[dossier.deposer]` | src/keymap.rs:624 |
| Sortir les calques choisis de leur dossier | Palette ; aussi : menucalque.rs:185 « Sortir les calques choisis de leur dossier » | — | Sortir les calques choisis de leur dossier `[dossier.sortir]` | src/keymap.rs:625 |
| Supprimer uniquement le dossier (ses calques restent) | Palette ; aussi : menucalque.rs:188 « Supprimer uniquement le dossier » | — | Supprimer uniquement le dossier (ses calques restent) `[dossier.supprimer_seul]` | src/keymap.rs:626 |
| Joindre les dossiers choisis dans le dossier courant | Palette ; aussi : menucalque.rs:187 « Joindre les dossiers choisis dans le dossier courant » | — | Joindre les dossiers choisis dans le dossier courant `[dossier.joindre]` | src/keymap.rs:627 |
| Effets du calque courant : tout allumer / tout éteindre (bouton fx) | Palette ; aussi : fxui.rs:749 « Rallumer » | — | Effets du calque courant : tout allumer / tout éteindre (bouton fx) `[calque.effets]` | src/keymap.rs:628 |
| Groupes de calques (noms, couleurs, montrés dans la timeline) | Palette ; aussi : groupsui.rs:107 « Éditer les groupes… » | — | Groupes de calques (noms, couleurs, montrés dans la timeline) `[groupes.editer]` | src/keymap.rs:629 |
| Focus sur le groupe du calque courant (la timeline ne montre que lui) | Palette ; aussi : groupsui.rs:127 « Focus sur le groupe du calque courant » | — | Focus sur le groupe du calque courant (la timeline ne montre que lui) `[groupes.focus]` | src/keymap.rs:630 |
| Outil caméra | Palette ; aussi : handles.rs:1168 « Effets de caméra (réglages de l'outil Caméra) » | `Maj+C` | Outil caméra `[outil.camera]` | src/keymap.rs:631 |
| Outil transformer le calque | Palette | — | Outil transformer le calque `[outil.transformer]` | src/keymap.rs:632 |
| Vue à travers la caméra | Palette ; aussi : camtools.rs:747 « Vue à travers la caméra » ; handles.rs:1169 « Vue à travers la caméra » ; panels.rs:260 « Vue à travers la caméra » ; viewbar.rs:545 « Vue caméra » ; viewbar.rs:548 « Vue du plateau » | — | Vue à travers la caméra `[camera.vue]` | src/keymap.rs:633 |
| Passe-partout autour du cadre de la caméra | Palette ; aussi : camtools.rs:751 « Passe-partout » ; panels.rs:261 « Passe-partout » | — | Passe-partout autour du cadre de la caméra `[camera.masque]` | src/keymap.rs:634 |
| Réinitialiser la caméra | Palette ; aussi : boardui.rs:2741 « Réinitialiser le cadrage » ; camtools.rs:699 « Réinitialiser la caméra » ; handles.rs:1170 « Réinitialiser la caméra » ; panels.rs:264 « Réinitialiser la caméra du plan » | — | Réinitialiser la caméra `[camera.reinitialiser]` | src/keymap.rs:635 |
| Clé de caméra au début du plan | Palette ; aussi : barres.rs:321 « Clé de caméra au début » ; handles.rs:1157 « Clé de caméra au début du plan » | — | Clé de caméra au début du plan `[camera.cle_debut]` | src/keymap.rs:636 |
| Clé de caméra à l'image courante | Palette ; aussi : barres.rs:322 « Clé de caméra ici » ; espace3dcles.rs:67 « Clé de caméra ici » ; handles.rs:1155 « Clé de caméra ici » ; timelinesb.rs:284 « Clé de caméra ici » | `Maj+K` | Clé de caméra à l'image courante `[camera.cle_ici]` | src/keymap.rs:637 |
| Clé de caméra à la fin du plan | Palette ; aussi : barres.rs:323 « Clé de caméra à la fin » ; handles.rs:1158 « Clé de caméra à la fin du plan » | — | Clé de caméra à la fin du plan `[camera.cle_fin]` | src/keymap.rs:638 |
| Supprimer la clé de caméra de l'image courante | Palette ; aussi : barres.rs:324 « Supprimer la clé de caméra » ; handles.rs:1161 « Supprimer la clé de caméra d'ici » | — | Supprimer la clé de caméra de l'image courante `[camera.cle_supprimer]` | src/keymap.rs:639 |
| Adoucir le mouvement à cette clé de caméra | Palette ; aussi : handles.rs:1160 « Adoucir le mouvement à cette clé » | — | Adoucir le mouvement à cette clé de caméra `[camera.cle_adoucir]` | src/keymap.rs:640 |
| Copier les clés de caméra choisies | Palette ; aussi : timelinesb.rs:308 « Copier les clés de caméra choisies » | — | Copier les clés de caméra choisies `[camera.cles_copier]` | src/keymap.rs:641 |
| Coller les clés de caméra (insérées après la clé de la tête de lecture) | Palette ; aussi : timelinesb.rs:309 « Coller les clés de caméra » | — | Coller les clés de caméra (insérées après la clé de la tête de lecture) `[camera.cles_coller]` | src/keymap.rs:642 |
| Supprimer les clés de caméra choisies (rien sans clé choisie) | Palette | — | Supprimer les clés de caméra choisies (rien sans clé choisie) `[camera.cles_supprimer]` | src/keymap.rs:643 |
| Choisir toutes les clés de caméra du plan | Palette | — | Choisir toutes les clés de caméra du plan `[camera.cles_tout_choisir]` | src/keymap.rs:644 |
| Convertir l'effet de caméra en clés (tremblement, explosion, zoom…) | Palette | — | Convertir l'effet de caméra en clés (tremblement, explosion, zoom…) `[camera.effet_appliquer]` | src/keymap.rs:645 |
| Préréglage d'effet : image d'impact (impact frame) | Palette | — | Préréglage d'effet : image d'impact (impact frame) `[effet.impact]` | src/keymap.rs:646 |
| Préréglage d'effet : lueur (glow) | Palette | — | Préréglage d'effet : lueur (glow) `[effet.lueur]` | src/keymap.rs:647 |
| Préréglage d'effet : flash blanc | Palette | — | Préréglage d'effet : flash blanc `[effet.flash_blanc]` | src/keymap.rs:648 |
| Préréglage d'effet : flash de couleur | Palette | — | Préréglage d'effet : flash de couleur `[effet.flash_couleur]` | src/keymap.rs:649 |
| Préréglage d'effet : tremblement de l'image | Palette | — | Préréglage d'effet : tremblement de l'image `[effet.tremblement]` | src/keymap.rs:650 |
| Préréglage d'effet : vignettage | Palette | — | Préréglage d'effet : vignettage `[effet.vignettage]` | src/keymap.rs:651 |
| Préréglage d'effet : grain de film | Palette | — | Préréglage d'effet : grain de film `[effet.grain]` | src/keymap.rs:652 |
| Préréglage d'effet : aberration chromatique | Palette | — | Préréglage d'effet : aberration chromatique `[effet.aberration]` | src/keymap.rs:653 |
| Préréglage d'effet : flou de vitesse | Palette | — | Préréglage d'effet : flou de vitesse `[effet.flou_vitesse]` | src/keymap.rs:654 |
| Préréglage d'effet : timecode et incrustations | Palette | — | Préréglage d'effet : timecode et incrustations `[effet.timecode]` | src/keymap.rs:655 |
| Appliquer le préréglage d'effet en cours de réglage | Palette | — | Appliquer le préréglage d'effet en cours de réglage `[effet.appliquer]` | src/keymap.rs:656 |
| Fermer le préréglage d'effet sans rien poser | Palette | — | Fermer le préréglage d'effet sans rien poser `[effet.fermer]` | src/keymap.rs:657 |
| Nouveau calque d'effet vide (agit sur tout ce qui est dessous) | Palette ; aussi : prereglagesui.rs:264 « Calque d'effet vide (au-dessus de tout) » | — | Nouveau calque d'effet vide (agit sur tout ce qui est dessous) `[effet.vide]` | src/keymap.rs:658 |
| Aide au cadrage : règle des tiers (grille 3 × 3) | Palette ; aussi : cadrageui.rs:77 « Tiers » | — | Aide au cadrage : règle des tiers (grille 3 × 3) `[cadrage.tiers]` | src/keymap.rs:659 |
| Aide au cadrage : grille d'or (phi), points aux intersections | Palette | — | Aide au cadrage : grille d'or (phi), points aux intersections `[cadrage.grille_or]` | src/keymap.rs:660 |
| Aide au cadrage : spirale d'or et ses carrés | Palette | — | Aide au cadrage : spirale d'or et ses carrés `[cadrage.spirale]` | src/keymap.rs:661 |
| Aide au cadrage : orientation suivante de la spirale d'or (8) | Palette | — | Aide au cadrage : orientation suivante de la spirale d'or (8) `[cadrage.spirale_orientation]` | src/keymap.rs:662 |
| Aide au cadrage : diagonales et réciproques (symétrie dynamique) | Palette | — | Aide au cadrage : diagonales et réciproques (symétrie dynamique) `[cadrage.diagonales]` | src/keymap.rs:663 |
| Aide au cadrage : croix centrale | Palette ; aussi : cadrageui.rs:116 « Croix centrale » | — | Aide au cadrage : croix centrale `[cadrage.croix]` | src/keymap.rs:664 |
| Aides au cadrage : toutes éteintes | Palette ; aussi : cadrageui.rs:117 « Éteindre les repères » | — | Aides au cadrage : toutes éteintes `[cadrage.aucune]` | src/keymap.rs:665 |
| Aide au cadrage : cadre de format suivant (16:9, 2,39:1…) | Palette | — | Aide au cadrage : cadre de format suivant (16:9, 2,39:1…) `[cadrage.format_suivant]` | src/keymap.rs:666 |
| Table lumineuse (autres calques estompés) | Palette ; aussi : panels.rs:626 « Table lumineuse » ; viewbar.rs:391 « Table lumineuse » | — | Table lumineuse (autres calques estompés) `[vue.table_lumineuse]` | src/keymap.rs:667 |
| Vue de dessus (profondeur des calques) | Palette ; aussi : panels.rs:268 « Vue de dessus (profondeur des calques) » | — | Vue de dessus (profondeur des calques) `[vue.dessus]` | src/keymap.rs:668 |
| Zones de sécurité (action, titres) | Palette ; aussi : panels.rs:262 « Zones de sécurité » | — | Zones de sécurité (action, titres) `[vue.cadres_securite]` | src/keymap.rs:669 |
| Espace 3D (afficher / quitter) | Palette ; aussi : espace3dcote.rs:244 « Agrandir » ; espace3dpanneau.rs:83 « Espace 3D » ; panels.rs:267 « Espace 3D » | — | Espace 3D (afficher / quitter) `[espace3d.basculer]` | src/keymap.rs:675 |
| Espace 3D à côté du dessin (afficher / masquer) | Palette, **seulement vue Espace 3D ouverte** ; aussi : espace3dcote.rs:247 « Fermer la vue 3D d'à côté » | — | Espace 3D à côté du dessin (afficher / masquer) `[espace3d.a_cote]` | src/keymap.rs:676 |
| Espace 3D : vues, disposition suivante (1, 2 côte à côte, 2 l'une sur l'autre, 4) | Palette, **seulement vue Espace 3D ouverte** | — | Espace 3D : vues, disposition suivante (1, 2 côte à côte, 2 l'une sur l'autre, 4) `[espace3d.vues]` | src/keymap.rs:678 |
| Espace 3D : 1 vue | Palette, **seulement vue Espace 3D ouverte** | — | Espace 3D : 1 vue `[espace3d.vues_une]` | src/keymap.rs:679 |
| Espace 3D : 2 vues côte à côte | Palette, **seulement vue Espace 3D ouverte** | — | Espace 3D : 2 vues côte à côte `[espace3d.vues_cote]` | src/keymap.rs:680 |
| Espace 3D : 2 vues l'une sur l'autre | Palette, **seulement vue Espace 3D ouverte** | — | Espace 3D : 2 vues l'une sur l'autre `[espace3d.vues_superposees]` | src/keymap.rs:681 |
| Espace 3D : 4 vues | Palette, **seulement vue Espace 3D ouverte** | — | Espace 3D : 4 vues `[espace3d.vues_quatre]` | src/keymap.rs:682 |
| Espace 3D : agrandir la vue sous le pointeur en plein cadre (ou revenir) | Palette, **seulement vue Espace 3D ouverte** | — | Espace 3D : agrandir la vue sous le pointeur en plein cadre (ou revenir) `[espace3d.vue_agrandir]` | src/keymap.rs:683 |
| Espace 3D : outil choisir | Palette, **seulement vue Espace 3D ouverte** | — | Espace 3D : outil choisir `[espace3d.outil_choisir]` | src/keymap.rs:684 |
| Espace 3D : outil déplacer | Palette, **seulement vue Espace 3D ouverte** | — | Espace 3D : outil déplacer `[espace3d.outil_deplacer]` | src/keymap.rs:685 |
| Espace 3D : outil tourner | Palette, **seulement vue Espace 3D ouverte** | — | Espace 3D : outil tourner `[espace3d.outil_tourner]` | src/keymap.rs:686 |
| Espace 3D : outil mettre à l'échelle | Palette, **seulement vue Espace 3D ouverte** | — | Espace 3D : outil mettre à l'échelle `[espace3d.outil_echelle]` | src/keymap.rs:687 |
| Espace 3D : choisir la caméra du plan | Palette, **seulement vue Espace 3D ouverte** ; aussi : espace3dui.rs:676 « Caméra » | — | Espace 3D : choisir la caméra du plan `[espace3d.outil_camera]` | src/keymap.rs:688 |
| Espace 3D : modifier l'origine du calque ou de l'objet du décor choisi, sans le bouger (oui / non) | Palette, **seulement vue Espace 3D ouverte** | — | Espace 3D : modifier l'origine du calque ou de l'objet du décor choisi, sans le bouger (oui / non) `[espace3d.modifier_origine]` | src/keymap.rs:690 |
| Espace 3D : recadrer la vue sur tout l'espace | Palette, **seulement vue Espace 3D ouverte** ; aussi : espace3dui.rs:623 « Recadrer » ; onglet3d.rs:641 « Recadrer » | — | Espace 3D : recadrer la vue sur tout l'espace `[espace3d.cadrer]` | src/keymap.rs:691 |
| Espace 3D : regarder par la caméra du plan | Palette, **seulement vue Espace 3D ouverte** ; aussi : espace3dgraphes.rs:1053 « Regarder par la caméra » ; espace3dui.rs:626 « Regarder par la caméra » | — | Espace 3D : regarder par la caméra du plan `[espace3d.vue_camera]` | src/keymap.rs:692 |
| Espace 3D : ordonner la pile des calques selon la profondeur | Palette, **seulement vue Espace 3D ouverte** ; aussi : espace3dui.rs:773 « Ordonner la pile selon la profondeur » | — | Espace 3D : ordonner la pile des calques selon la profondeur `[espace3d.ordonner_profondeur]` | src/keymap.rs:693 |
| Espace 3D : le plus proche passe devant, pour le plan courant (oui / non) | Palette, **seulement vue Espace 3D ouverte** ; aussi : espace3dui.rs:769 « Le plus proche passe devant » | — | Espace 3D : le plus proche passe devant, pour le plan courant (oui / non) `[espace3d.ordre_distance]` | src/keymap.rs:695 |
| Espace 3D : nouvelle surface face à la caméra | Palette, **seulement vue Espace 3D ouverte** ; aussi : espace3dgraphes.rs:549 « Surface » | — | Espace 3D : nouvelle surface face à la caméra `[espace3d.surface_face]` | src/keymap.rs:696 |
| Espace 3D : nouvelle surface au sol | Palette, **seulement vue Espace 3D ouverte** ; aussi : espace3dgraphes.rs:550 « Sol » | — | Espace 3D : nouvelle surface au sol `[espace3d.surface_sol]` | src/keymap.rs:697 |
| Espace 3D : nouvelle surface en mur | Palette, **seulement vue Espace 3D ouverte** ; aussi : espace3dgraphes.rs:551 « Mur » | — | Espace 3D : nouvelle surface en mur `[espace3d.surface_mur]` | src/keymap.rs:698 |
| Espace 3D : aimantation à la grille (oui / non) | Palette, **seulement vue Espace 3D ouverte** ; aussi : espace3dui.rs:1052 « Aimanter à la grille » | — | Espace 3D : aimantation à la grille (oui / non) `[espace3d.aimanter]` | src/keymap.rs:699 |
| Espace 3D : grille du sol (oui / non) | Palette, **seulement vue Espace 3D ouverte** ; aussi : espace3dui.rs:1036 « Grille du sol » ; onglet3d.rs:629 « Grille du sol » | — | Espace 3D : grille du sol (oui / non) `[espace3d.grille]` | src/keymap.rs:700 |
| Espace 3D : dessiner sur la carte du calque courant (oui / non) | Palette, **seulement vue Espace 3D ouverte** | — | Espace 3D : dessiner sur la carte du calque courant (oui / non) `[espace3d.dessiner]` | src/keymap.rs:701 |
| Panneau Espace 3D (ouvrir / fermer) | Palette, **seulement vue Espace 3D ouverte** | — | Panneau Espace 3D (ouvrir / fermer) `[vue.espace3d_panneau]` | src/keymap.rs:703 |
| Espace 3D (ancien plateau en 3D) | absent de la palette : double de `espace3d.basculer` | — | Espace 3D (ancien plateau en 3D) `[vue.3d]` | src/keymap.rs:704 |
| Espace 3D (ancien nom : scène 3D de blocage) | absent de la palette : double de `espace3d.basculer` | — | Espace 3D (ancien nom : scène 3D de blocage) `[scene3d.basculer]` | src/keymap.rs:705 |
| Espace 3D : recadrer (ancien plateau en 3D) | absent de la palette : double de `espace3d.cadrer` | — | Espace 3D : recadrer (ancien plateau en 3D) `[vue3d.recadrer]` | src/keymap.rs:706 |
| Espace 3D : recadrer (ancien nom) | absent de la palette : double de `espace3d.cadrer` | — | Espace 3D : recadrer (ancien nom) `[scene3d.cadrer]` | src/keymap.rs:707 |
| Espace 3D : outil choisir (ancien outil du plateau en 3D) | absent de la palette : double de `espace3d.outil_choisir` | — | Espace 3D : outil choisir (ancien outil du plateau en 3D) `[outil3d.orbite]` | src/keymap.rs:708 |
| Espace 3D : outil choisir (ancien outil du plateau en 3D) | absent de la palette : double de `espace3d.outil_choisir` | — | Espace 3D : outil choisir (ancien outil du plateau en 3D) `[outil3d.glisser]` | src/keymap.rs:709 |
| Espace 3D : outil choisir (ancien outil du plateau en 3D) | absent de la palette : double de `espace3d.outil_choisir` | — | Espace 3D : outil choisir (ancien outil du plateau en 3D) `[outil3d.avancer]` | src/keymap.rs:710 |
| Espace 3D : outil déplacer (ancien outil du plateau en 3D) | absent de la palette : double de `espace3d.outil_deplacer` | — | Espace 3D : outil déplacer (ancien outil du plateau en 3D) `[outil3d.deplacer]` | src/keymap.rs:711 |
| Espace 3D : outil choisir (ancien nom) | absent de la palette : double de `espace3d.outil_choisir` | — | Espace 3D : outil choisir (ancien nom) `[scene3d.outil_choisir]` | src/keymap.rs:712 |
| Espace 3D : outil déplacer (ancien nom) | absent de la palette : double de `espace3d.outil_deplacer` | — | Espace 3D : outil déplacer (ancien nom) `[scene3d.outil_deplacer]` | src/keymap.rs:713 |
| Espace 3D : outil tourner (ancien nom) | absent de la palette : double de `espace3d.outil_tourner` | — | Espace 3D : outil tourner (ancien nom) `[scene3d.outil_tourner]` | src/keymap.rs:714 |
| Espace 3D : outil mettre à l'échelle (ancien nom) | absent de la palette : double de `espace3d.outil_echelle` | — | Espace 3D : outil mettre à l'échelle (ancien nom) `[scene3d.outil_echelle]` | src/keymap.rs:715 |
| Espace 3D : regarder par la caméra (ancien nom) | absent de la palette : double de `espace3d.vue_camera` | — | Espace 3D : regarder par la caméra (ancien nom) `[scene3d.vue_camera]` | src/keymap.rs:716 |
| Espace 3D : aimantation à la grille (ancien nom) | absent de la palette : double de `espace3d.aimanter` | — | Espace 3D : aimantation à la grille (ancien nom) `[scene3d.aimanter]` | src/keymap.rs:717 |
| Espace 3D : grille du sol (ancien nom) | absent de la palette : double de `espace3d.grille` | — | Espace 3D : grille du sol (ancien nom) `[scene3d.grille]` | src/keymap.rs:718 |
| Espace de travail : Standard | Palette | — | Espace de travail : Standard `[espace.standard]` | src/keymap.rs:719 |
| Espace de travail : Dessin | Palette | — | Espace de travail : Dessin `[espace.dessin]` | src/keymap.rs:720 |
| Espace de travail : Timeline | Palette | — | Espace de travail : Timeline `[espace.timeline]` | src/keymap.rs:721 |
| Enregistrer la disposition comme espace de travail | Palette ; aussi : workspaceui.rs:179 « Enregistrer la disposition… » | — | Enregistrer la disposition comme espace de travail `[espace.enregistrer]` | src/keymap.rs:722 |
| Espace de travail : Storyboard (sa disposition retenue) | Palette | — | Espace de travail : Storyboard (sa disposition retenue) `[espace.storyboard]` | src/keymap.rs:723 |
| Espace de travail : Montage (le film en haut, toutes les pistes en bas) | Palette ; aussi : panels.rs:329 « Montage » | `Alt+M` | Espace de travail : Montage (le film en haut, toutes les pistes en bas) `[espace.montage]` | src/keymap.rs:724 |
| Espace de travail : 3D (la séquence en trois dimensions : décor, caméras des plans, cartes du plan) | Palette, **seulement vue Espace 3D ouverte** | — | Espace de travail : 3D (la séquence en trois dimensions : décor, caméras des plans, cartes du plan) `[espace.3d]` | src/keymap.rs:725 |
| Espace de travail : Vignettes (la planche seule, pour dessiner dans les cases) | Palette ; aussi : panels.rs:357 « Vignettes » | `Alt+Maj+V` | Espace de travail : Vignettes (la planche seule, pour dessiner dans les cases) `[espace.vignettes]` | src/keymap.rs:726 |
| Rétablir la disposition de l'espace de travail en cours | Palette ; aussi : workspaceui.rs:159 « Rétablir la disposition » | — | Rétablir la disposition de l'espace de travail en cours `[espace.retablir]` | src/keymap.rs:727 |
| Colonne des réglages et fiche (montrer / cacher) | Palette ; aussi : workspaceui.rs:215 « Colonne des réglages et fiche » | — | Colonne des réglages et fiche (montrer / cacher) `[vue.panneau_outils]` | src/keymap.rs:728 |
| Colonne des calques (montrer / cacher) | Palette ; aussi : workspaceui.rs:216 « Colonne des calques » | — | Colonne des calques (montrer / cacher) `[vue.panneau_fiche]` | src/keymap.rs:729 |
| Montage et timeline (montrer / cacher) | Palette ; aussi : workspaceui.rs:217 « Montage et timeline » | — | Montage et timeline (montrer / cacher) `[vue.panneau_bande]` | src/keymap.rs:730 |
| Réajuster les panneaux à leur contenu | Palette ; aussi : prefsui.rs:629 « Réajuster les panneaux » | — | Réajuster les panneaux à leur contenu `[vue.reajuster_panneaux]` | src/keymap.rs:731 |
| Performance : superposition des temps (images, panneaux, arrière-plan) | Palette ; aussi : panels.rs:299 « Performance (temps de l'interface) » | — | Performance : superposition des temps (images, panneaux, arrière-plan) `[vue.performance]` | src/keymap.rs:732 |
| Guide de perspective : afficher et utiliser | Palette ; aussi : panels.rs:271 « Afficher et utiliser » | `G` | Guide de perspective : afficher et utiliser `[guide.activer]` | src/keymap.rs:733 |
| Guide : aligner les traits | Palette ; aussi : panels.rs:272 « Aligner les traits » | `Maj+A` | Guide : aligner les traits `[guide.aligner]` | src/keymap.rs:734 |
| Guide : verrouiller | Palette ; aussi : panels.rs:273 « Verrouiller » | `Maj+L` | Guide : verrouiller `[guide.verrouiller]` | src/keymap.rs:735 |
| Guide : lignes de référence | Palette ; aussi : panels.rs:274 « Lignes de référence » | `Maj+G` | Guide : lignes de référence `[guide.lignes_reference]` | src/keymap.rs:736 |
| Guide : aligner aussi la gomme | Palette | — | Guide : aligner aussi la gomme `[guide.gomme]` | src/keymap.rs:737 |
| Guide : lignes du curseur jusqu'au bord | Palette | — | Guide : lignes du curseur jusqu'au bord `[guide.curseur_complet]` | src/keymap.rs:738 |
| Importer un son | Palette ; aussi : barres.rs:267 « Importer un son » ; bottomtabs.rs:158 « Importer un son… » ; handles.rs:1179 « Importer un son… » ; storyboard.rs:682 « Importer un son… » ; storyboard.rs:2062 « Importer un son ici… » | — | Importer un son `[son.importer]` | src/keymap.rs:739 |
| Nouvelle piste son | Palette ; aussi : barres.rs:401 « Nouvelle piste son » ; handles.rs:1180 « Nouvelle piste son » ; storyboard.rs:684 « Nouvelle piste son » | — | Nouvelle piste son `[son.nouvelle_piste]` | src/keymap.rs:740 |
| Couper le son à l'image courante | Palette ; aussi : barres.rs:402 « Couper le son » ; handles.rs:1181 « Couper le son à l'image courante » ; storyboard.rs:685 « Couper le son à l'image courante » | — | Couper le son à l'image courante `[son.couper]` | src/keymap.rs:741 |
| Son au défilement (activer / couper) | Palette ; aussi : handles.rs:1183 « Son au défilement (activer / couper) » | — | Son au défilement (activer / couper) `[son.defilement]` | src/keymap.rs:742 |
| Son pendant la lecture (activer / couper) | Palette ; aussi : barres.rs:172 « Son pendant la lecture » ; handles.rs:1184 « Son pendant la lecture (activer / couper) » | — | Son pendant la lecture (activer / couper) `[son.lecture]` | src/keymap.rs:743 |
| Panneau Bruitages (ouvrir / fermer) | Palette ; aussi : storyboard.rs:683 « Bruitages… » | — | Panneau Bruitages (ouvrir / fermer) `[vue.bruitages]` | src/keymap.rs:744 |
| Panneau Préréglages (ouvrir / fermer) | Palette | — | Panneau Préréglages (ouvrir / fermer) `[vue.prereglages]` | src/keymap.rs:745 |
| Panneau Scène, du film jusqu'aux calques (ouvrir / fermer) | Palette | — | Panneau Scène, du film jusqu'aux calques (ouvrir / fermer) `[vue.scene]` | src/keymap.rs:746 |
| Poser le bruitage choisi à la tête de lecture | Palette | — | Poser le bruitage choisi à la tête de lecture `[bruitage.inserer]` | src/keymap.rs:747 |
| Écouter (ou arrêter) le bruitage choisi | Palette | — | Écouter (ou arrêter) le bruitage choisi `[bruitage.ecouter]` | src/keymap.rs:748 |
| Panneau Suggestions sonores (fonction expérimentale ; ouvrir / fermer) | Palette ; aussi : panels.rs:290 « Suggestions sonores… » | — | Panneau Suggestions sonores (fonction expérimentale ; ouvrir / fermer) `[vue.suggestions_sonores]` | src/keymap.rs:749 |
| Lire le storyboard pour proposer des sons (fonction expérimentale) | Palette ; aussi : panels.rs:291 « Lire le storyboard (proposer des sons) » ; suggestionsui.rs:645 « Lire le storyboard » | — | Lire le storyboard pour proposer des sons (fonction expérimentale) `[suggestion.lire]` | src/keymap.rs:750 |
| Mon panneau (fonction expérimentale ; ouvrir / fermer) | Palette | — | Mon panneau (fonction expérimentale ; ouvrir / fermer) `[vue.mon_panneau]` | src/keymap.rs:751 |
| Panneau Bibliothèque (ouvrir / fermer) | Palette | — | Panneau Bibliothèque (ouvrir / fermer) `[vue.bibliotheque]` | src/keymap.rs:753 |
| Bibliothèque : revenir au dossier d'avant | Palette | — | Bibliothèque : revenir au dossier d'avant `[bibliotheque.retour]` | src/keymap.rs:754 |
| Bibliothèque : remonter d'un dossier | Palette | — | Bibliothèque : remonter d'un dossier `[bibliotheque.dossier_parent]` | src/keymap.rs:755 |
| Bibliothèque : relire le dossier | Palette ; aussi : biblioui.rs:689 « Actualiser » | — | Bibliothèque : relire le dossier `[bibliotheque.actualiser]` | src/keymap.rs:756 |
| Bibliothèque : ajouter (ou retirer) le dossier courant des favoris | Palette ; aussi : biblioui.rs:693 « Retirer des favoris » | — | Bibliothèque : ajouter (ou retirer) le dossier courant des favoris `[bibliotheque.favori]` | src/keymap.rs:757 |
| Bibliothèque : lire (ou mettre en pause) l'aperçu | Palette | — | Bibliothèque : lire (ou mettre en pause) l'aperçu `[bibliotheque.lecture]` | src/keymap.rs:758 |
| Bibliothèque : importer le fichier choisi dans le plan | Palette ; aussi : biblioui.rs:1019 « Importer dans le plan » | — | Bibliothèque : importer le fichier choisi dans le plan `[bibliotheque.importer_plan]` | src/keymap.rs:759 |
| Bibliothèque : ajouter le fichier choisi aux références | Palette ; aussi : biblioui.rs:1021 « Ajouter aux références » | — | Bibliothèque : ajouter le fichier choisi aux références `[bibliotheque.ajouter_references]` | src/keymap.rs:760 |
| Bibliothèque : ajouter le fichier choisi au montage | Palette ; aussi : biblioui.rs:1024 « Ajouter au montage » | — | Bibliothèque : ajouter le fichier choisi au montage `[bibliotheque.ajouter_montage]` | src/keymap.rs:761 |
| Bibliothèque : ouvrir le dossier du fichier choisi dans l'Explorateur | Palette ; aussi : biblioui.rs:1028 « Ouvrir le dossier dans l'Explorateur » | — | Bibliothèque : ouvrir le dossier du fichier choisi dans l'Explorateur `[bibliotheque.afficher_explorateur]` | src/keymap.rs:762 |
| Bibliothèque : copier le chemin du fichier choisi | Palette ; aussi : biblioui.rs:1029 « Copier le chemin » | — | Bibliothèque : copier le chemin du fichier choisi `[bibliotheque.copier_chemin]` | src/keymap.rs:763 |
| Fondu d'entrée de l'extrait choisi (poser / retirer) | Palette | — | Fondu d'entrée de l'extrait choisi (poser / retirer) `[son.fondu_entree]` | src/keymap.rs:764 |
| Fondu de sortie de l'extrait choisi (poser / retirer) | Palette | — | Fondu de sortie de l'extrait choisi (poser / retirer) `[son.fondu_sortie]` | src/keymap.rs:765 |
| Fondu enchaîné de l'extrait choisi avec le suivant (poser / retirer) | Palette | — | Fondu enchaîné de l'extrait choisi avec le suivant (poser / retirer) `[son.enchaine]` | src/keymap.rs:766 |
| Clé de volume à la tête de lecture (extrait choisi) | Palette | — | Clé de volume à la tête de lecture (extrait choisi) `[son.cle_volume]` | src/keymap.rs:767 |
| Solo de la piste de l'extrait choisi (oui / non) | Palette | — | Solo de la piste de l'extrait choisi (oui / non) `[son.solo]` | src/keymap.rs:768 |
| Retirer les effets de l'extrait choisi | Palette | — | Retirer les effets de l'extrait choisi `[son.effets_reinit]` | src/keymap.rs:769 |
| Ajouter une transition (à la coupe la plus proche de la tête de lecture) | Palette | — | Ajouter une transition (à la coupe la plus proche de la tête de lecture) `[transition.ajouter]` | src/keymap.rs:770 |
| Supprimer la transition (choisie, sinon la plus proche de la tête de lecture) | Palette | — | Supprimer la transition (choisie, sinon la plus proche de la tête de lecture) `[transition.supprimer]` | src/keymap.rs:771 |
| Transition : fondu enchaîné | Palette | — | Transition : fondu enchaîné `[transition.fondu_enchaine]` | src/keymap.rs:772 |
| Transition : fondu au noir | Palette | — | Transition : fondu au noir `[transition.fondu_noir]` | src/keymap.rs:773 |
| Transition : fondu au blanc | Palette | — | Transition : fondu au blanc `[transition.fondu_blanc]` | src/keymap.rs:774 |
| Transition : volet | Palette | — | Transition : volet `[transition.volet]` | src/keymap.rs:775 |
| Transition : noir | Palette | — | Transition : noir `[transition.noir]` | src/keymap.rs:776 |
| Transition : poussée | Palette | — | Transition : poussée `[transition.poussee]` | src/keymap.rs:777 |
| Transition : raccord filé | Palette | — | Transition : raccord filé `[transition.raccord_file]` | src/keymap.rs:778 |
| Transition : flash de coupe | Palette | — | Transition : flash de coupe `[transition.flash]` | src/keymap.rs:779 |
| Transition : fondu par une couleur | Palette | — | Transition : fondu par une couleur `[transition.couleur]` | src/keymap.rs:780 |
| Transition : volet par un objet | Palette | — | Transition : volet par un objet `[transition.objet]` | src/keymap.rs:781 |
| Transition : iris | Palette | — | Transition : iris `[transition.iris]` | src/keymap.rs:782 |
| Transition : volet en horloge | Palette | — | Transition : volet en horloge `[transition.horloge]` | src/keymap.rs:783 |
| Transition : zoom traversant | Palette | — | Transition : zoom traversant `[transition.zoom_traversant]` | src/keymap.rs:784 |
| Aperçu des transitions à l'arrêt (oui / non) | Palette | — | Aperçu des transitions à l'arrêt (oui / non) `[transition.apercu]` | src/keymap.rs:785 |
| Marquer un envoi (noter l'état de tous les plans) | Palette ; aussi : panels.rs:200 « Marquer un envoi… » | — | Marquer un envoi (noter l'état de tous les plans) `[envoi.marquer]` | src/keymap.rs:786 |
| Envois : liste et plans modifiés (ouvrir / fermer) | Palette ; aussi : barres.rs:255 « Envois » ; panels.rs:201 « Envois et plans modifiés… » | — | Envois : liste et plans modifiés (ouvrir / fermer) `[envoi.liste]` | src/keymap.rs:787 |
| Aller au plan modifié suivant (depuis le dernier envoi) | Palette ; aussi : barres.rs:258 « Plan modifié suivant » ; panels.rs:202 « Plan modifié suivant » | — | Aller au plan modifié suivant (depuis le dernier envoi) `[envoi.modifie_suivant]` | src/keymap.rs:788 |
| Aller au plan modifié précédent (depuis le dernier envoi) | Palette ; aussi : barres.rs:257 « Plan modifié précédent » ; panels.rs:203 « Plan modifié précédent » | — | Aller au plan modifié précédent (depuis le dernier envoi) `[envoi.modifie_precedent]` | src/keymap.rs:789 |
| Choisir les plans modifiés depuis le dernier envoi | Palette ; aussi : panels.rs:204 « Choisir les plans modifiés » | — | Choisir les plans modifiés depuis le dernier envoi `[envoi.modifies_choisir]` | src/keymap.rs:790 |
| Journal des différences entre deux envois (plans modifiés avant / après, notes traitées ; PDF et page HTML) | Palette ; aussi : barres.rs:256 « Journal des différences » ; exportmenu.rs:59 « Journal des différences… » ; panels.rs:205 « Journal des différences… » | — | Journal des différences entre deux envois (plans modifiés avant / après, notes traitées ; PDF et page HTML) `[envoi.journal]` | src/keymap.rs:791 |
| Nouvelle version du plan, en dupliquant la version en cours | Palette ; aussi : barres.rs:260 « Nouvelle version du plan » ; panels.rs:208 « Nouvelle version, en dupliquant la version en cours » | — | Nouvelle version du plan, en dupliquant la version en cours `[version.nouvelle]` | src/keymap.rs:792 |
| Nouvelle version du plan, en partant d'un plan vide | Palette ; aussi : panels.rs:209 « Nouvelle version, en partant d'un plan vide » | — | Nouvelle version du plan, en partant d'un plan vide `[version.vide]` | src/keymap.rs:793 |
| Supprimer les versions non affichées (tous les plans) | Palette ; aussi : planversui.rs:382 « Supprimer les versions non affichées… » | — | Supprimer les versions non affichées (tous les plans) `[version.purger]` | src/keymap.rs:794 |
| Supprimer les versions non affichées du plan courant | Palette ; aussi : planversui.rs:383 « Supprimer les versions non affichées du plan courant… » | — | Supprimer les versions non affichées du plan courant `[version.purger_plan]` | src/keymap.rs:795 |
| Passer à la version précédente du plan | Palette ; aussi : panels.rs:210 « Version précédente » | — | Passer à la version précédente du plan `[version.precedente]` | src/keymap.rs:796 |
| Passer à la version suivante du plan | Palette ; aussi : panels.rs:211 « Version suivante » | — | Passer à la version suivante du plan `[version.suivante]` | src/keymap.rs:797 |
| Nouvelle version de la séquence, en gardant les plans | Palette ; aussi : variantesui.rs:400 « Nouvelle version, en gardant les plans » | — | Nouvelle version de la séquence, en gardant les plans `[variante.nouvelle]` | src/keymap.rs:798 |
| Nouvelle version de la séquence, en copiant les plans | Palette ; aussi : variantesui.rs:401 « Nouvelle version, en copiant les plans » | — | Nouvelle version de la séquence, en copiant les plans `[variante.copie]` | src/keymap.rs:799 |
| Nouvelle version de la séquence, en partant d'un plan vide | Palette ; aussi : variantesui.rs:402 « Nouvelle version, en partant d'un plan vide » | — | Nouvelle version de la séquence, en partant d'un plan vide `[variante.vide]` | src/keymap.rs:800 |
| Passer à la version précédente de la séquence | Palette | — | Passer à la version précédente de la séquence `[variante.precedente]` | src/keymap.rs:801 |
| Passer à la version suivante de la séquence | Palette | — | Passer à la version suivante de la séquence `[variante.suivante]` | src/keymap.rs:802 |
| Mettre à jour le plan dans les autres versions de la séquence | Palette ; aussi : variantesui.rs:415 « Mettre à jour le plan dans les autres versions de séquence… » | — | Mettre à jour le plan dans les autres versions de la séquence `[variante.mettre_a_jour]` | src/keymap.rs:803 |
| Renommer la version de la séquence | Palette ; aussi : variantesui.rs:416 « Renommer la version de séquence… » | — | Renommer la version de la séquence `[variante.renommer]` | src/keymap.rs:804 |
| Supprimer la version de la séquence | Palette ; aussi : variantesui.rs:418 « Supprimer la version de séquence… » | — | Supprimer la version de la séquence `[variante.supprimer]` | src/keymap.rs:805 |
| Comparer le plan à une autre version de la séquence | Palette ; aussi : variantesui.rs:417 « Comparer le plan à une autre version de séquence… » | — | Comparer le plan à une autre version de la séquence `[variante.comparer]` | src/keymap.rs:806 |
| Comparer le plan : versions ou envois (ouvrir / fermer) | Palette ; aussi : barres.rs:261 « Comparer le plan » ; panels.rs:213 « Comparer le plan… » | — | Comparer le plan : versions ou envois (ouvrir / fermer) `[comparer.ouvrir]` | src/keymap.rs:807 |
| Partager ce projet (créer un film partagé dans un dossier partagé ou réseau) | Palette ; aussi : panels.rs:159 « Partager ce projet… » | — | Partager ce projet (créer un film partagé dans un dossier partagé ou réseau) `[fichier.partager]` | src/keymap.rs:808 |
| Équipe : qui est en ligne, ce que chacun tient (ouvrir / fermer) | Palette ; aussi : panels.rs:157 « Équipe… » | — | Équipe : qui est en ligne, ce que chacun tient (ouvrir / fermer) `[equipe.fenetre]` | src/keymap.rs:809 |
| Prendre le plan courant (ou les plans choisis) : les autres ne le modifient plus | Palette | — | Prendre le plan courant (ou les plans choisis) : les autres ne le modifient plus `[equipe.prendre_plan]` | src/keymap.rs:810 |
| Rendre le plan courant (ou les plans choisis) après l'avoir enregistré | Palette | — | Rendre le plan courant (ou les plans choisis) après l'avoir enregistré `[equipe.rendre_plan]` | src/keymap.rs:811 |
| Prendre tous les plans libres de la séquence courante | Palette | — | Prendre tous les plans libres de la séquence courante `[equipe.prendre_sequence]` | src/keymap.rs:812 |
| Rendre la séquence courante après l'avoir enregistrée | Palette | — | Rendre la séquence courante après l'avoir enregistrée `[equipe.rendre_sequence]` | src/keymap.rs:813 |
| Prendre le jeton du montage (ordre, durées, noms, son, transitions) | Palette | — | Prendre le jeton du montage (ordre, durées, noms, son, transitions) `[equipe.prendre_montage]` | src/keymap.rs:814 |
| Rendre le jeton du montage après l'avoir enregistré | Palette | — | Rendre le jeton du montage après l'avoir enregistré `[equipe.rendre_montage]` | src/keymap.rs:815 |
| Rendre tout ce que je tiens | Palette | — | Rendre tout ce que je tiens `[equipe.rendre_tout]` | src/keymap.rs:816 |
| Décharger la référence courante | absent de la palette : **non implémenté** | — | Décharger la référence courante — **non implémenté** `[reference.decharger]` | src/keymap.rs:817 |
| Chercher dans les légendes | absent de la palette : **non implémenté** | `Ctrl+Maj+F` *en réserve, inactif* | Chercher dans les légendes — **non implémenté** `[legende.chercher]` | src/keymap.rs:818 |

Décompte : 660 visibles partout dans la palette, 35 visibles seulement vue Espace 3D ouverte, 22 doubles invisibles, 14 non implémentées.

## 9. Tous les raccourcis clavier par défaut (keymap.toml), triés par touche

| Libellé à l'écran | Où le trouver | Raccourci par défaut | Effet attendu | Source |
|---|---|---|---|---|
| `,` | contexte partout | `,` | Image précédente `[image.precedente]` | keymap.toml:665 |
| `-` | contexte partout | `-` | Raccourcir le plan d'une image `[panneau.retirer_image]` | keymap.toml:835 |
| `.` | contexte partout | `.` | Image suivante `[image.suivante]` | keymap.toml:669 |
| `1` | contexte partout | `1` | Zoom arrière `[vue.zoom_arriere]` | keymap.toml:189 |
| `2` | contexte partout | `2` | Zoom avant `[vue.zoom_avant]` | keymap.toml:193 |
| `[` | contexte partout | `[` | Pinceau plus fin `[outil.taille_moins]` | keymap.toml:465 |
| `]` | contexte partout | `]` | Pinceau plus gros `[outil.taille_plus]` | keymap.toml:461 |
| `A` | contexte partout | `A` | Clé précédente (début de dessin d'avant, tous les calques) `[cle.precedente]` | keymap.toml:760 |
| `C` | contexte partout | `C` | Tourner la vue (sens antihoraire) `[vue.rotation_antihoraire]` | keymap.toml:205 |
| `D` | contexte partout | `D` | Couleurs par défaut (noir à l'avant-plan, blanc à l'arrière-plan) `[couleur.defaut]` | keymap.toml:479 |
| `Entrée` | contexte partout | `Entrée` | Lecture / pause `[lecture.basculer]` | keymap.toml:660 |
| `Espace` | contexte partout | `Espace` | Déplacer la vue (maintenir) (à maintenir) `[vue.pan_libre]` | keymap.toml:222 |
| `F` | contexte partout | `F` | Clé suivante (début de dessin d'après, tous les calques) `[cle.suivante]` | keymap.toml:765 |
| `F1` | contexte partout | `F1` | Liste des raccourcis `[aide.raccourcis]` | keymap.toml:336 |
| `F11` | contexte partout | `F11` | Mode concentration : ne garder que le dessin (la même touche remet tout) `[vue.concentration]` | keymap.toml:271 |
| `F4` | contexte partout | `F4` | Nouvelle clé vide à l'image courante (coupe l'exposition) `[dessin.nouvelle_cle_vide]` | keymap.toml:856 |
| `F5` | contexte partout | `F5` | Nouvelle clé à l'image courante, copie du dessin (coupe l'exposition) `[dessin.nouvelle_cle]` | keymap.toml:850 |
| `F6` | contexte partout | `F6` | Couper le plan à l'image courante `[panneau.couper]` | keymap.toml:843 |
| `F9` | contexte partout | `F9` | Noter une friction : ce qui vient de me freiner, avec une capture de la fenêtre et mes dernières commandes `[aide.noter_friction]` | keymap.toml:311 |
| `Fin` | contexte partout | `Fin` | Dernière image du plan `[image.derniere]` | keymap.toml:687 |
| `Flèche bas` | contexte partout | `Flèche bas` | Calque du dessous `[calque.dessous]` | keymap.toml:1038 |
| `Flèche droite` | contexte partout | `Flèche droite` | Image suivante `[image.suivante]` | keymap.toml:678 |
| `Flèche gauche` | contexte partout | `Flèche gauche` | Image précédente `[image.precedente]` | keymap.toml:673 |
| `Flèche haut` | contexte partout | `Flèche haut` | Calque du dessus `[calque.dessus]` | keymap.toml:1033 |
| `G` | contexte partout | `G` | Guide de perspective : afficher et utiliser `[guide.activer]` | keymap.toml:2253 |
| `H` | contexte partout | `H` | Outil main `[outil.main]` | keymap.toml:451 |
| `I` | contexte animation | `I` | Ajouter une image au plan, après l'image courante `[images.ajouter]` | keymap.toml:1204 |
| `Inser` | contexte storyboard | `Inser` | Nouveau plan `[panneau.nouveau]` | keymap.toml:790 |
| `Inser` | contexte animation | `Inser` | Nouveau dessin après l'exposition `[dessin.nouveau]` | keymap.toml:895 |
| `J` | contexte montage | `J` | Montage : lire en arrière (un nouvel appui accélère) `[montage.lecture_arriere]` | keymap.toml:714 |
| `K` | contexte montage | `K` | Montage : arrêter la lecture (avec J ou L : image par image) `[montage.lecture_arret]` | keymap.toml:720 |
| `L` | contexte montage | `L` | Montage : lire en avant (un nouvel appui accélère) `[montage.lecture_avant]` | keymap.toml:726 |
| `M` | contexte partout | `M` | Miroir de la vue `[vue.miroir]` | keymap.toml:250 |
| `N` | contexte partout | `N` | Vignettes : montrer ou cacher les numéros de plan `[vignettes.numeros]` | keymap.toml:3378 |
| `O` | contexte partout — en réserve, inactif | `O` | Outil épaisseur du trait de crayon `[outil.epaisseur_crayon]` | keymap.toml:456 |
| `Origine` | contexte partout | `Origine` | Première image du plan `[image.premiere]` | keymap.toml:683 |
| `P` | contexte partout | `P` | Nouveau plan `[panneau.nouveau]` | keymap.toml:786 |
| `PgPréc` | contexte partout | `PgPréc` | Plan précédent `[panneau.precedent]` | keymap.toml:750 |
| `PgSuiv` | contexte partout | `PgSuiv` | Plan suivant `[panneau.suivant]` | keymap.toml:755 |
| `Q` | contexte partout | `Q` | Supprimer les calques choisis `[calque.supprimer]` | keymap.toml:944 |
| `R` | contexte partout | `R` | Aide-mémoire des raccourcis (maintenir) (à maintenir) `[aide.aide_memoire]` | keymap.toml:276 |
| `Retour arrière` | contexte partout | `Retour arrière` | Supprimer la sélection (rien sans sélection) `[edition.supprimer]` | keymap.toml:156 |
| `Suppr` | contexte partout | `Suppr` | Supprimer la sélection (rien sans sélection) `[edition.supprimer]` | keymap.toml:152 |
| `T` | contexte partout | `T` | Vignettes : tenir pour tirer les cadres de caméra (maintenir ; comme Ctrl + Maj) (à maintenir) `[vignettes.camera]` | keymap.toml:284 |
| `Tab` | contexte partout | `Tab` | Entrer dans le plan pour l'animer, et en revenir `[mode.basculer]` | keymap.toml:260 |
| `V` | contexte partout | `V` | Tourner la vue (sens horaire) `[vue.rotation_horaire]` | keymap.toml:201 |
| `W` | contexte partout | `W` | Copier le calque `[calque.copier]` | keymap.toml:934 |
| `X` | contexte partout | `X` | Coller le calque `[calque.coller]` | keymap.toml:939 |
| `Échap` | contexte partout | `Échap` | Tout désélectionner `[edition.tout_deselectionner]` | keymap.toml:178 |
| `+` | contexte partout | `+` | Allonger le plan d'une image `[panneau.ajouter_image]` | keymap.toml:831 |
| `Alt+-` | contexte partout | `Alt+-` | Tenir le dessin une image de moins `[exposition.raccourcir]` | keymap.toml:917 |
| `Alt+1` | contexte partout | `Alt+1` | Zoom arrière de la bande (le montage, ou la timeline en animation) `[bande.zoom_arriere]` | keymap.toml:733 |
| `Alt+2` | contexte partout | `Alt+2` | Zoom avant de la bande (le montage, ou la timeline en animation) `[bande.zoom_avant]` | keymap.toml:738 |
| `Alt+3` | contexte partout | `Alt+3` | Bande : tout voir (le film en storyboard, le plan en animation) `[bande.tout_voir]` | keymap.toml:743 |
| `Alt+B` | contexte partout | `Alt+B` | Outil pinceau `[outil.pinceau]` | keymap.toml:343 |
| `Alt+D` | contexte partout | `Alt+D` | Outil pipette `[outil.pipette]` | keymap.toml:375 |
| `Alt+E` | contexte partout | `Alt+E` | Outil gomme `[outil.gomme]` | keymap.toml:352 |
| `Alt+Entrée` | contexte partout | `Alt+Entrée` | Fiche du plan : ouvrir les propriétés du plan courant `[panneau.proprietes]` | keymap.toml:839 |
| `Alt+Espace` | contexte partout — en réserve, inactif | `Alt+Espace` | Zoom arrière temporaire (maintenir) (à maintenir) `[vue.zoom_temporaire_arriere]` | keymap.toml:236 |
| `Alt+G` | contexte partout | `Alt+G` | Outil dégradé (linéaire ou radial, bitmap et vectoriel) `[outil.degrade]` | keymap.toml:360 |
| `Alt+H` | contexte partout | `Alt+H` | Retourner la sélection horizontalement `[selection.miroir_h]` | keymap.toml:602 |
| `Alt+I` | contexte partout | `Alt+I` | Outil remplissage `[outil.remplissage]` | keymap.toml:356 |
| `Alt+L` | contexte partout | `Alt+L` | Verrouiller le calque `[calque.verrouiller]` | keymap.toml:954 |
| `Alt+M` | contexte partout | `Alt+M` | Espace de travail : Montage (le film en haut, toutes les pistes en bas) `[espace.montage]` | keymap.toml:2210 |
| `Alt+O` | contexte partout | `Alt+O` | Pelure d'oignon `[vue.pelure]` | keymap.toml:246 |
| `Alt+P` | contexte partout | `Alt+P` | Outil crayon `[outil.crayon]` | keymap.toml:347 |
| `Alt+R` | contexte partout | `Alt+R` | Outil formes (dernière forme utilisée) `[outil.formes]` | keymap.toml:393 |
| `Alt+S` | contexte partout | `Alt+S` | Outil sélection `[outil.selection]` | keymap.toml:379 |
| `Alt+T` | contexte partout | `Alt+T` | Outil cutter `[outil.cutter]` | keymap.toml:387 |
| `Alt+V` | contexte partout | `Alt+V` | Retourner la sélection verticalement `[selection.miroir_v]` | keymap.toml:606 |
| `Alt+Y` | contexte partout — en réserve, inactif | `Alt+Y` | Outil peindre les zones non peintes `[outil.peindre_non_peint]` | keymap.toml:370 |
| `Alt+Z` | contexte partout — en réserve, inactif | `Alt+Z` | Outil zoom `[vue.outil_zoom]` | keymap.toml:184 |
| `Ctrl+0` | contexte partout | `Ctrl+0` | Ajuster à la fenêtre `[vue.ajuster]` | keymap.toml:217 |
| `Ctrl+A` | contexte partout | `Ctrl+A` | Tout sélectionner `[edition.tout_selectionner]` | keymap.toml:169 |
| `Ctrl+Alt` | contexte partout — en réserve, inactif | `Ctrl+Alt` | Rotation temporaire (maintenir) (à maintenir) `[vue.rotation_temporaire]` | keymap.toml:226 |
| `Ctrl+C` | contexte partout | `Ctrl+C` | Copier `[edition.copier]` | keymap.toml:131 |
| `Ctrl+D` | contexte partout | `Ctrl+D` | Dupliquer le dessin `[dessin.dupliquer]` | keymap.toml:901 |
| `Ctrl+E` | contexte partout | `Ctrl+E` | Fusionner vers le bas (avec le calque du dessous) `[calque.fusionner_dessous]` | keymap.toml:958 |
| `Ctrl+Espace` | contexte partout — en réserve, inactif | `Ctrl+Espace` | Zoom avant temporaire (maintenir) (à maintenir) `[vue.zoom_temporaire_avant]` | keymap.toml:231 |
| `Ctrl+F` | contexte partout | `Ctrl+F` | Aller à un plan `[scene.aller_a]` | keymap.toml:780 |
| `Ctrl+Fin` | contexte storyboard | `Ctrl+Fin` | Fin du montage `[montage.fin]` | keymap.toml:698 |
| `Ctrl+Fin` | contexte animation | `Ctrl+Fin` | Dernier calque (le plus haut) `[calque.dernier]` | keymap.toml:1072 |
| `Ctrl+Flèche bas` | contexte partout | `Ctrl+Flèche bas` | Descendre le calque d'un rang `[calque.descendre]` | keymap.toml:1061 |
| `Ctrl+Flèche droite` | contexte animation | `Ctrl+Flèche droite` | Instance suivante (début du dessin d'après) `[instance.suivante]` | keymap.toml:1280 |
| `Ctrl+Flèche gauche` | contexte animation | `Ctrl+Flèche gauche` | Instance précédente (début du dessin d'avant) `[instance.precedente]` | keymap.toml:1273 |
| `Ctrl+Flèche haut` | contexte partout | `Ctrl+Flèche haut` | Monter le calque d'un rang `[calque.monter]` | keymap.toml:1056 |
| `Ctrl+G` | contexte partout — en réserve, inactif | `Ctrl+G` | Afficher la grille `[vue.grille]` | keymap.toml:241 |
| `Ctrl+K` | contexte partout | `Ctrl+K` | Palette de commandes : chercher une commande par son nom `[aide.palette]` | keymap.toml:265 |
| `Ctrl+L` | contexte partout | `Ctrl+L` | Pelure d'oignon du calque courant (allumer / éteindre) `[calque.pelure]` | keymap.toml:1083 |
| `Ctrl+N` | contexte partout | `Ctrl+N` | Nouveau projet `[fichier.nouveau]` | keymap.toml:34 |
| `Ctrl+O` | contexte partout | `Ctrl+O` | Ouvrir `[fichier.ouvrir]` | keymap.toml:38 |
| `Ctrl+Origine` | contexte storyboard | `Ctrl+Origine` | Début du montage `[montage.debut]` | keymap.toml:691 |
| `Ctrl+Origine` | contexte animation | `Ctrl+Origine` | Premier calque (le plus bas) `[calque.premier]` | keymap.toml:1066 |
| `Ctrl+PgPréc` | contexte partout | `Ctrl+PgPréc` | Plan précédent (second raccourci) `[scene.precedente]` | keymap.toml:770 |
| `Ctrl+PgSuiv` | contexte partout | `Ctrl+PgSuiv` | Plan suivant (second raccourci) `[scene.suivante]` | keymap.toml:775 |
| `Ctrl+Q` | contexte partout | `Ctrl+Q` | Quitter `[fichier.quitter]` | keymap.toml:65 |
| `Ctrl+R` | contexte partout | `Ctrl+R` | Annuler la transformation du calque `[calque.reinit_transformation]` | keymap.toml:962 |
| `Ctrl+S` | contexte partout | `Ctrl+S` | Enregistrer, ou poser un jalon `[fichier.enregistrer]` | keymap.toml:42 |
| `Ctrl+Suppr` | contexte storyboard | `Ctrl+Suppr` | Supprimer les plans choisis `[panneau.supprimer]` | keymap.toml:875 |
| `Ctrl+Suppr` | contexte animation | `Ctrl+Suppr` | Retirer le dessin de la timeline `[dessin.supprimer]` | keymap.toml:906 |
| `Ctrl+T` | contexte partout | `Ctrl+T` | Nouvel onglet `[onglet.nouveau]` | keymap.toml:71 |
| `Ctrl+Tab` | contexte partout | `Ctrl+Tab` | Onglet suivant `[onglet.suivant]` | keymap.toml:76 |
| `Ctrl+V` | contexte partout | `Ctrl+V` | Coller `[edition.coller]` | keymap.toml:135 |
| `Ctrl+W` | contexte partout | `Ctrl+W` | Fermer le projet (l'onglet) `[fichier.fermer]` | keymap.toml:61 |
| `Ctrl+X` | contexte partout | `Ctrl+X` | Couper `[edition.couper]` | keymap.toml:127 |
| `Ctrl+Y` | contexte partout | `Ctrl+Y` | Rétablir `[edition.retablir]` | keymap.toml:122 |
| `Ctrl+Z` | contexte partout | `Ctrl+Z` | Annuler `[edition.annuler]` | keymap.toml:114 |
| `Maj+A` | contexte partout | `Maj+A` | Guide : aligner les traits `[guide.aligner]` | keymap.toml:2257 |
| `Maj+B` | contexte partout | `Maj+B` | Nouveau calque bitmap `[calque.nouveau_bitmap]` | keymap.toml:929 |
| `Maj+C` | contexte partout | `Maj+C` | Outil caméra `[outil.camera]` | keymap.toml:1909 |
| `Maj+D` | contexte partout | `Maj+D` | Flou sur le calque (effet Flou gaussien, cocher / décocher) `[calque.flou]` | keymap.toml:949 |
| `Maj+F5` | contexte animation | `Maj+F5` | Supprimer l'image du plan (tous les calques) `[images.supprimer_plan]` | keymap.toml:1216 |
| `Maj+Flèche bas` | contexte partout | `Maj+Flèche bas` | Calque du dessous, en gardant le choix `[calque.choisir_dessous]` | keymap.toml:1051 |
| `Maj+Flèche haut` | contexte partout | `Maj+Flèche haut` | Calque du dessus, en gardant le choix `[calque.choisir_dessus]` | keymap.toml:1046 |
| `Maj+G` | contexte partout | `Maj+G` | Guide : lignes de référence `[guide.lignes_reference]` | keymap.toml:2265 |
| `Maj+I` | contexte animation | `Maj+I` | Insérer une image dans le plan, avant l'image courante `[images.inserer]` | keymap.toml:1210 |
| `Maj+K` | contexte partout | `Maj+K` | Clé de caméra à l'image courante `[camera.cle_ici]` | keymap.toml:1935 |
| `Maj+L` | contexte partout | `Maj+L` | Guide : verrouiller `[guide.verrouiller]` | keymap.toml:2261 |
| `Maj+P` | contexte partout | `Maj+P` | Nouveau plan (second raccourci) `[scene.nouvelle]` | keymap.toml:812 |
| `Maj+Retour arrière` | contexte partout | `Maj+Retour arrière` | Effacer ce qui est hors de la sélection `[dessin.supprimer_hors_selection]` | keymap.toml:613 |
| `Maj+S` | contexte partout | `Maj+S` | Nouveau calque vectoriel `[calque.nouveau_vectoriel]` | keymap.toml:924 |
| `Maj+Suppr` | contexte partout | `Maj+Suppr` | Remplir la sélection de la couleur courante `[selection.remplir]` | keymap.toml:617 |
| `Maj+Tab` | contexte partout | `Maj+Tab` | Vignettes et Storyboard : passer de l'un à l'autre (la planche, le dessin du plan) `[vue.planches]` | keymap.toml:255 |
| `Maj+X` | contexte partout | `Maj+X` | Remettre la vue droite `[vue.reinit_rotation]` | keymap.toml:209 |
| `Maj+Z` | contexte partout | `Maj+Z` | Zoom 100 % `[vue.reinit_zoom]` | keymap.toml:213 |
| `Alt++` | contexte partout | `Alt++` | Tenir le dessin une image de plus `[exposition.allonger]` | keymap.toml:912 |
| `Alt+Maj+C` | contexte partout | `Alt+Maj+C` | Colonne des calques de la timeline : replier tous les calques sur une ligne (ou les déplier) `[calques.replier_tout]` | keymap.toml:2691 |
| `Alt+Maj+F` | contexte partout — en réserve, inactif | `Alt+Maj+F` | Aplatir le dessin `[dessin.aplatir]` | keymap.toml:643 |
| `Alt+Maj+V` | contexte partout | `Alt+Maj+V` | Espace de travail : Vignettes (la planche seule, pour dessiner dans les cases) `[espace.vignettes]` | keymap.toml:2200 |
| `Ctrl+Alt+;` | contexte partout — en réserve, inactif | `Ctrl+Alt+;` | Aimanter au contour `[dessin.aimanter_contour]` | keymap.toml:648 |
| `Ctrl+Maj+A` | contexte partout — en réserve, inactif | `Ctrl+Maj+A` | Sélectionner tous les dessins du plan `[edition.selectionner_dessins_scene]` | keymap.toml:173 |
| `Ctrl+Maj+D` | contexte partout | `Ctrl+Maj+D` | Dupliquer le plan `[panneau.dupliquer]` | keymap.toml:796 |
| `Ctrl+Maj+F` | contexte partout — en réserve, inactif | `Ctrl+Maj+F` | Chercher dans les légendes `[legende.chercher]` | keymap.toml:2312 |
| `Ctrl+Maj+Flèche droite` | contexte partout | `Ctrl+Maj+Flèche droite` | Signet suivant `[signet.suivant]` | keymap.toml:1396 |
| `Ctrl+Maj+Flèche gauche` | contexte partout | `Ctrl+Maj+Flèche gauche` | Signet précédent `[signet.precedent]` | keymap.toml:1391 |
| `Ctrl+Maj+Flèche haut` | contexte partout | `Ctrl+Maj+Flèche haut` | Signet à l'image courante (poser / retirer) `[signet.basculer]` | keymap.toml:1386 |
| `Ctrl+Maj+R` | contexte partout — en réserve, inactif | `Ctrl+Maj+R` | Remplir automatiquement les zones fermées `[dessin.remplir_zones_auto]` | keymap.toml:653 |
| `Ctrl+Maj+S` | contexte partout | `Ctrl+Maj+S` | Enregistrer sous `[fichier.enregistrer_sous]` | keymap.toml:51 |
| `Ctrl+Maj+Tab` | contexte partout | `Ctrl+Maj+Tab` | Onglet précédent `[onglet.precedent]` | keymap.toml:81 |
| `Ctrl+Maj+V` | contexte partout | `Ctrl+Maj+V` | Coller à la tête de lecture `[edition.coller_a_la_tete]` | keymap.toml:147 |
| `Ctrl+Maj+Z` | contexte partout | `Ctrl+Maj+Z` | Rétablir `[edition.retablir]` | keymap.toml:118 |
| `Ctrl+Maj+Alt+F` | contexte partout | `Ctrl+Maj+Alt+F` | Plein écran `[vue.plein_ecran]` | keymap.toml:197 |

Touches câblées en dur hors du fichier de raccourcis (non réassignables) :

| Touche | Où | Effet | Source |
|---|---|---|---|
| Échap | partout | ferme la palette, les panneaux de case, la liste d'outils d'une case, annule un glisser en cours, lâche la caméra des Vignettes ; `edition.tout_deselectionner` sinon | src/app.rs:4329-4339, src/toolbars.rs:250, src/palette.rs:113 |
| Ctrl+Z pendant un glisser | partout | abandonne le geste (même touche que Annuler) | src/app.rs:4330-4339 |
| Entrée / Échap / Retour arrière | outil en cours (polyligne, texte…) | finit / annule la forme avant tout raccourci | src/app.rs:4397-4401 |
| Flèches | sélection sur le dessin | déplacent la sélection de 1 px (Maj : 10) avant de changer d'image ou de calque | src/app.rs:4402-4407, src/nudge.rs |
| Ctrl+F | pointeur sur le panneau Scénario | recherche du scénario au lieu de « Aller à un plan » | src/app.rs:4408-4411 |
| Espace, flèches, Retour arrière, Entrée | souris sur la Bibliothèque | navigation de la Bibliothèque | src/app.rs:4393-4396, src/biblioui.rs:132 |
| Ctrl+Entrée | texte en cours (outil Texte, planche, références) | valide le texte | src/app.rs:190, src/planchrefui.rs:996 |
| Ctrl+Entrée (physique Ctrl) | note de la planche des Vignettes | valide la note — teste `modifiers.ctrl`, donc ⌃ et non ⌘ sur Mac | src/boardui.rs:2083 |
| Maj+Entrée / Entrée | recherche du panneau Scénario | résultat précédent / suivant | src/scenariopanneau.rs:341 |
| F11 / Échap | fenêtre à part des Références | plein écran / sortie | src/planchrefui.rs:396 |
| Ctrl+Z, Ctrl+Maj+Z, Ctrl+Y, Suppr, Ctrl+A | panneau Références qui a la main | annuler / rétablir / supprimer / tout choisir propres aux Références | src/referencesui.rs:753-755, 1014 |
| Molette / Maj+molette / bouton du milieu | dessin | zoom / rotation / déplacement de la vue | src/panels.rs:15-17 |
| Ctrl + molette | grille de la timeline | zoom de la timeline | src/cues.rs:268 |

## 10. Points à vérifier

### 10.1 Raccourcis en conflit ou attribués deux fois dans le même contexte

Le fichier par défaut n'a **aucun conflit strict** : l'essai `keymap_tests::no_two_default_bindings_share_a_key_in_the_same_context` l'interdit (`keymap.toml:12-13`) et `check_conflicts` le signale au chargement (`src/keymap.rs:1361-1378`). Restent les doubles volontaires et les touches qui changent de sens selon ce qui a la main :

- `Inser` : Nouveau plan au Storyboard (`keymap.toml:790`), Nouveau dessin en animation (`keymap.toml:895`). Idem `Ctrl+Suppr` (supprimer les plans / retirer le dessin, `keymap.toml:875`, `906`), `Ctrl+Origine` et `Ctrl+Fin` (début / fin du montage contre premier / dernier calque, `keymap.toml:691`, `698`, `1066`, `1072`). Le « contexte » suit le panneau qui a la main (`src/app.rs:40-50`) : cliquer dans la timeline puis taper `Ctrl+Origine` doit aller au premier calque, pas au début du film.
- `J`, `K`, `L` (contexte montage) passent **devant** toute autre attribution dans l'onglet Montage (`src/keymap.rs:1387-1391`) ; `check_conflicts` ne compare pas un raccourci « montage » à un raccourci « partout » (`src/keymap.rs:1365`), donc un `J`/`K`/`L` posé par l'utilisateur ailleurs serait muet dans le Montage sans avertissement.
- Une action, plusieurs touches : Rétablir = `Ctrl+Maj+Z` et `Ctrl+Y` (`keymap.toml:118`, `122`) ; Supprimer = `Suppr` et `Retour arrière` (`152`, `156`) ; Image précédente = `,` et `Flèche gauche` (`665`, `673`) ; Image suivante = `.` et `Flèche droite` (`669`, `678`) ; Nouveau plan = `P`, `Inser` et `Maj+P` via le double `scene.nouvelle` (`786`, `790`, `812`) ; Plan précédent / suivant = `PgPréc`/`PgSuiv` et `Ctrl+PgPréc`/`Ctrl+PgSuiv` via les doubles `scene.precedente`/`scene.suivante` (`750`, `755`, `770`, `775`). Les menus n'affichent que la première (`src/keymap.rs:1395-1403`).
- `Échap` = « Tout désélectionner » (`keymap.toml:178`) mais aussi, câblé en dur, fermer la palette, la liste d'outils d'une case, les petits panneaux de la barre du dessin, abandonner un glisser, lâcher la caméra des Vignettes (`src/app.rs:4329-4339`, `src/toolbars.rs:250`, `src/viewbar.rs:264`).
- `Entrée` = Lecture / pause (`keymap.toml:660`), sauf si l'outil en cours la prend (polyligne, texte : `src/app.rs:4397-4401`) ou si la souris est sur la Bibliothèque (`src/app.rs:4393-4396`). Le bouton Lecture de la timeline lit toujours le plan, celui du montage le film ; la touche, elle, suit la zone active (`src/barres.rs:1075-1077`).
- `Ctrl+F` = Aller à un plan (`keymap.toml:780`), mais recherche du Scénario quand le pointeur est sur ce panneau (`src/app.rs:4408-4411`).
- `Ctrl+C`, `Ctrl+V`, `Suppr` visent tour à tour le dessin, les plans, les images, les clés, les calques, les références, les commentaires, une transition ou une clé de volume, selon « qui a la main » (`src/app.rs:4509-4640`, indicateur dans la barre d'état `src/panels.rs:786-799`). Une image dans le presse-papiers système passe devant un `Ctrl+V` de dessin (`src/app.rs:4526-4534`).
- `F11` = Mode concentration (`keymap.toml:271`) et, câblé en dur, plein écran de la fenêtre à part des Références (`src/planchrefui.rs:396`). Le libellé du menu Affichage « Plein écran » est une autre action (`vue.plein_ecran`, `Ctrl+Maj+Alt+F`, `keymap.toml:197`).
- `Ctrl+Z` = Annuler, et pendant un glisser « abandonner le geste » (`src/app.rs:4330-4339`) ; le panneau Références a ses propres `Ctrl+Z`/`Ctrl+Maj+Z`/`Ctrl+Y` câblés (`src/referencesui.rs:753-754`).
- `T` est attribué « partout » à `vignettes.camera` (`keymap.toml:284`) mais ne fait rien hors des Vignettes (`src/app.rs:5551`, tenue lue seulement dans `src/boardui.rs:1477`).

### 10.2 Commandes atteignables par plusieurs chemins sous des noms différents

- `camera.vue` : « Vue à travers la caméra » (Affichage ▸ Caméra, `src/panels.rs:260` ; outil Caméra `src/camtools.rs:747`), « Vue caméra » et **« 2D » / « Vue du plateau »** (`src/viewbar.rs:545`, `548`). Les deux cases de la barre affichent le même raccourci et le même « Assigner », pour deux effets opposés.
- `mode.basculer` : « Animer ce plan » / « Retour au storyboard » (Storyboard, dessin, barres : `src/panels.rs:225-229`, `src/canvas.rs:394`, `src/barres.rs:286`, `294`), « Retour aux Vignettes » (`src/panels.rs:383`), et la bulle de l'onglet **« Storyboard »** (`src/panels.rs:348`) : clic droit sur l'onglet Storyboard assigne une touche à « entrer dans le plan », pas à l'onglet.
- Exports : « Exporter le film » (barre, `src/barres.rs:157`) = « Film (MP4, ProRes, DNxHR)… » (Fichier, `src/exportmenu.rs:22`) = « Exporter le film (vidéo) » (palette, `src/keymap.rs:70`) ; de même « Exporter pour le montage » / « Pour le montage (EDL, XML, FCPXML, OpenTimelineIO)… », « Exporter la fiche technique » / « Fiche technique (XLSX, CSV)… », « Exporter pour relecture » / « Relecture (la page à envoyer au réalisateur)… », « Exporter la planche des vignettes… » / « Planche des vignettes… ». La grande fenêtre Exporter (`fichier.exporter`) n'a plus d'entrée de menu, seulement la palette et Préférences (`src/panels.rs:150`, `src/prefsui.rs:827`).
- `aide.diagnostic` : « Régler FEANOR pour cette machine… » (Aide, `src/panels.rs:309`) et « Diagnostic des performances… » (`src/diagnosticui.rs:590`).
- `panneau.proprietes` : « Fiche du plan » (Storyboard, `src/panels.rs:193`), « Nom et fiche du plan… » (timeline du Storyboard, `src/timelinesb.rs:218`).
- `camera.reinitialiser` : « Réinitialiser la caméra du plan » (Affichage, `src/panels.rs:264`), « Réinitialiser la caméra » (`src/camtools.rs:699`, `src/handles.rs:1170`), « Réinitialiser le cadrage » (Vignettes, `src/boardui.rs:2741`), « Remettre la caméra à zéro » (`src/timelinesb.rs:296`).
- `vignettes.marie_louise` : « Marie-louise (ne montrer que les vignettes) » / « Retirer la marie-louise » (`src/annotationsui.rs:965`, `989`) et « Passe-partout des cases » (`src/vignettesprefs.rs:103`) — à ne pas confondre avec le « Passe-partout » de la caméra (`camera.masque`, `src/viewbar.rs:413`).
- `espace3d.basculer` : « Espace 3D » (Affichage ▸ Plateau, `src/panels.rs:267`), « 3D » (barre du dessin, `src/viewbar.rs:554`), « Agrandir » (vue à côté, `src/espace3dcote.rs:244`) ; distinct de l'onglet « 3D » (`espace.3d`, `src/panels.rs:337`).
- Plans : « Couper le plan à l'image courante » / « Couper le plan » / « Couper le plan ici » (`src/panels.rs:97`, `src/barres.rs:389`, `src/storyboard.rs:2053`) ; « Réunir avec le plan suivant » / « Réunir avec le suivant » / « Réunir le plan et le suivant » (`src/panels.rs:98`, `src/barres.rs:390`, `src/storyboard.rs:610`) ; « Raccourcir le début jusqu'à la tête de lecture » / « Rogner le début » / « Raccourcir le début du plan jusqu'ici » (`src/rognageui.rs:557`, `src/barres.rs:391`, `src/storyboard.rs:2057`).
- Le menu du clic droit sur un plan du montage refait à la main, **sans action**, « Animer ce plan », « Nouveau plan après », « Dupliquer le plan », « Couper le plan à l'image courante », « Réunir avec le plan suivant » (`src/storyboard.rs:782-806`) ; idem dans les Vignettes (`src/boardui.rs:2680-2719`) : pas de raccourci affiché, pas d'« Assigner un raccourci… » au clic droit, alors que les mêmes commandes l'ont dans Storyboard ▸ Plans.
- `edition.coller_sur_place` « Coller sur place » (palette seulement) fait exactement `edition.coller` (`src/app.rs:4690`, `src/panels.rs:173-174`). `pinceau.remplir` est un double caché de `pinceau.fond` (`src/keymap.rs:177`, `840`).
- `signet.basculer` : « Poser / retirer un signet » (Repères, `src/cues.rs:286`), « Signet ici (poser / retirer) » (règle, `src/handles.rs:1133`), « Signet du plan ici (poser / retirer) » (`src/storyboard.rs:2054`).
- `nuancier.ajouter` : « Ajouter la couleur d'avant-plan au nuancier » / « Ajouter au nuancier » / « Ajouter la couleur d'avant-plan » (`src/colorui.rs:388`, `685`, `src/nuancierui.rs:202`).
- « Éditer les groupes… » existe deux fois dans le même menu de calque : Organiser ▸ Groupes (action `groupes.editer`, `src/groupsui.rs:107`) et Apparence ▸ Groupe · Couleur d'étiquette (bouton sans action, `src/layerheads.rs:1146`).

### 10.3 Entrées qui semblent ne rien faire, désactivées en dur ou liées à du code retiré

- **Raccourcis en réserve, sans fonction** (la touche ne fait rien, la commande est absente de la palette) : `Ctrl+Maj+A` Sélectionner tous les dessins du plan, `Alt+Z` Outil zoom, `Ctrl+Alt` Rotation temporaire, `Ctrl+Espace` / `Alt+Espace` Zoom temporaire, `Ctrl+G` Grille, `Alt+Y` Peindre les zones non peintes, `O` Épaisseur du crayon, `Alt+Maj+F` Aplatir, `Ctrl+Alt+;` Aimanter au contour, `Ctrl+Maj+R` Remplir les zones auto, `Ctrl+Maj+F` Chercher dans la légende ; sans touche : Supprimer et resserrer, Décharger la référence (`src/keymap.rs:117-241`, `817-818` ; `keymap.toml:167-656`, `2310-2315`).
- **Afficher dans l'Explorateur** (Fichier ▸ « Afficher le projet dans l'Explorateur », clic droit sur un onglet, boutons « révéler » des exports, de la fiche, des frictions, de l'À propos) : tout passe par `reveal_in_explorer`, qui lance `explorer.exe` sans condition et ignore l'erreur (`src/exportui.rs:1082-1099`). Sur Mac, rien ne doit s'ouvrir, sans message.
- **Préférences ▸ Raccourcis ▸ Modifier le fichier à la main** : lance `notepad.exe` (`src/app.rs:5677`) ; sur Mac, message d'erreur attendu.
- **Assigner un raccourci…** (clic droit partout) : le fichier personnel vit dans `%APPDATA%\SHOT` (`src/donnees.rs:22-24`, `src/keymap.rs:1357-1359`) ; sans cette variable (cas normal sur Mac), l'enregistrement échoue avec « Dossier %APPDATA% introuvable : raccourci non enregistré. » (`src/raccourcisui.rs:104-108`), sauf si un `keymap.toml` est posé à côté de l'exécutable.
- **Fichier ▸ Importer ▸ Une image (calque bitmap)…** n'est reliée à aucune action (`src/panels.rs:133-136`) : pas de raccourci possible, et **introuvable dans la palette**, alors que le commentaire de `src/panels.rs:114-115` promet que toute commande y est.
- **Palette et 3D** : hors de la vue Espace 3D, la palette cache toute commande dont le nom contient « 3D » ou « carte » (`src/keymap.rs:831-834`, `src/espace3ddirect.rs:26-46`), dont « Espace de travail : 3D », « Espace 3D à côté du dessin », « Ajouter un objet 3D au décor de la séquence », « Enlever le décor 3D », « Toujours devant le décor 3D », alors que l'onglet 3D, la case « Espace 3D à côté » et Fichier ▸ Importer les montrent toujours.
- **Menu Storyboard** : deux séparateurs de suite (`src/panels.rs:190-191`), trace d'entrées retirées.
- **Vignettes** : le commentaire de la barre d'outils dit que les autres outils « restent à leurs raccourcis (Alt + lettre) » (`src/toolbars.rs:26-28`), mais `run_action` les refuse dans les Vignettes avec un message (`src/app.rs:4450-4474`).
- **Onglet Montage** : sa bulle dit « Tab : retour au storyboard » (`src/panels.rs:329`) ; `Tab` est `mode.basculer`, qui bascule Storyboard ⇄ Animation (`src/app.rs:5044`). À vérifier à l'écran.
- **Labo** : menu entier caché tant que Préférences ▸ Général ▸ « Afficher les fonctions expérimentales » est éteint (`src/panels.rs:287`).
- **Anciens identifiants 3D** : `vue.3d`, `scene3d.*`, `outil3d.*`, `vue3d.recadrer` existent encore comme doubles de `espace3d.*` (`src/keymap.rs:847-863`) ; les commandes d'objets 3D d'origine (`scene3d.ajouter_*`, `importer_modele`, `outil.repere3d`, `fichier.importer_modele3d`) sont bien retirées (`src/keymap.rs:670-674`, aucune trace dans `keymap.toml`). L'espace « Animation » n'a pas d'action : `Espace::Animation.action()` rend `espace.animation`, absent de `ACTIONS` (`src/workspace.rs:277`).
- **Ce que le stylet transmet** (Préférences / palette `aide.stylet_transmet`) lit le stylet « auprès de Windows » ; ce code n'existe que sous Windows 64 bits (`src/stylet.rs:369`, `543`) : sur Mac, banc vide attendu.

### 10.4 Raccourcis écrits pour Windows : comportement sur Mac

- **Ctrl = ⌘** pour tous les raccourcis (`src/keymap.rs:892` compare `Modifiers::command`) ; les libellés disent « Ctrl+ » (`src/keymap.rs:903-905`) et les bulles d'aide écrivent « Ctrl+K », « Ctrl + glisser », « Ctrl+Z » en dur (ex. `src/panels.rs:9-34`, `src/accueil.rs:258` qui écrit « Ctrl+O » en dur). La touche ⌃ seule ne déclenche aucun raccourci, mais quand on **assigne** un raccourci, ⌃ et ⌘ sont tous deux enregistrés comme « Ctrl » (`src/keymap.rs:1035`) : appuyer ⌃+X à l'assignation donne un raccourci qui ne répond qu'à ⌘+X.
- Gestes à la souris : la plupart acceptent ⌃ **ou** ⌘ (`src/canvas.rs:576-637`, `src/handles.rs:115`, `src/boardui.rs:941`, `969`) ; exceptions qui ne lisent que ⌃ : valider une note de planche par `Ctrl+Entrée` (`src/boardui.rs:2083`), ajouter à la sélection de la planche par `Ctrl`/`Maj` + clic (`src/boardui.rs:1628`, `1983`). Sur Mac, ⌃ + clic peut aussi être lu comme un clic droit.
- **`Ctrl+Tab` / `Ctrl+Maj+Tab`** (onglet suivant / précédent, `keymap.toml:76`, `81`) deviennent ⌘+Tab : macOS les prend pour changer d'application. Aucun moyen clavier de changer d'onglet de projet sur Mac.
- **`Ctrl+Q`** (Quitter) = ⌘+Q, que macOS et le menu d'application de winit traitent aussi : vérifier que la boîte « Modifications non enregistrées » (`src/panels.rs:910-945`) apparaît bien avant de quitter.
- **Touches de fonction** : `F1` Liste des raccourcis, `F4` Nouvelle clé vide, `F5` Nouvelle clé, `Maj+F5` Supprimer l'image du plan, `F6` Couper le plan, `F9` Noter une friction, `F11` Mode concentration. Sur un Mac elles sont d'abord luminosité / Launchpad / dictée / son : il faut `fn`. `F11` est en plus « Afficher le bureau » dans macOS.
- **`Inser`** n'existe pas sur un clavier Mac : « Nouveau dessin » (`dessin.nouveau`, `keymap.toml:895`) n'a **aucune autre touche** ; « Nouveau plan » garde `P`.
- **`Suppr`, `Origine`, `Fin`, `PgPréc`, `PgSuiv`** : fn+⌫, fn+←, fn+→, fn+↑, fn+↓. `Retour arrière` (⌫) supprime bien (`keymap.toml:156`), mais le panneau Références teste `Key::Delete` en dur (`src/referencesui.rs:755`).
- **`Alt` + lettre** (outils `Alt+B/P/E/I/G/D/S/T/R`, `Alt+H`, `Alt+V`, `Alt+L`, `Alt+O`, `Alt+M`, `Alt+Maj+V`, `Alt+Maj+C`, `Alt+Entrée`, `Alt+1/2/3`, `Alt++`, `Alt+-`) : sur Mac, ⌥ + lettre compose un caractère (∫, π, €…) ou une touche morte. egui-winit 0.31 retombe alors sur la **position** de la touche (QWERTY) ; en AZERTY, `Alt+M` (Montage) et toute lettre qui n'est pas à la même place qu'en QWERTY risquent de ne pas répondre, ou de répondre à une autre touche. À essayer une par une.
- **Ponctuation par position** : `[` / `]` (taille du pinceau) = touches ^ et $ en AZERTY ; ^ est une touche morte sur Mac. `,` / `.` = touches ; et : (`keymap.toml:17-21`).
- `Ctrl+Espace` et `Alt+Espace` (en réserve) : sur Mac, ⌘+Espace ouvre Spotlight.
- Le code ne contient **aucune** branche `target_os = "macos"` ; les seules branches de plateforme sont `cfg(windows)` (stylet, presse-papiers d'images, Kitsu, matériel, `src/stylet.rs`, `src/kitsu.rs`, `src/materiel.rs`).

## 11. Décompte par zone

| Zone | Lignes de commandes |
|---|---|
| Barre de menus | 180 |
| Onglets et modes | 15 |
| Barres d'icônes | 167 |
| Outils | 57 |
| Barres du dessin | 37 |
| Clic droit (dessin, montage, timeline, planche) | 122 |
| Clic droit (calques, couleur, panneaux) | 136 |
| Palette (toutes les actions) | 731 |
| Raccourcis | 155 |
| **Total des lignes** (une commande peut paraître dans plusieurs zones) | 1600 |