# Référence : la timeline de TVPaint Animation 12 (phase 13)

Inventaire établi le 2026-10-15 à partir des textes de TVPaint Animation 12 Pro installé chez
Anthony (lecture seule) : `Resources\data\french.tvpaint.loc` (identifiants ci-dessous, sans le `$`),
`english.tvpaint.loc` pour lever les doutes, `newlayerpreset.conf`, `flips.conf`, et sa
configuration `%APPDATA%\TVPaint Animation 12 Pro\default\config.ini` (raccourcis).

Règle : ne rien inventer. **[A]** = le texte d'aide existe (comportement connu) ; **[N]** = nom
seulement, comportement à confirmer avec Anthony avant de l'implanter. Libellés repris pour
l'équivalence des fonctions ; Pelure n'emprunte ni nom de marque ni ressource.

Vocabulaire : « Table lumineuse » = pelure d'oignon ; « Faux-Fixe » = répéter des images ;
« Va et vient » = ping-pong ; « Maintenir » = tenir ; « Cellule d'exposition » = exposition ;
« Tête / queue d'instance » ; « Activer / Désactiver le calque » = visibilité ; « Signet » =
repère simple ; « Marque d'image » = marque de couleur ; « Point d'entrée / de sortie » ;
« Pochoir » = stencil ; « Groupe » = groupe de couleur ; « Dossier » = dossier de calques.

## 1. En-tête d'un calque (gauche de la ligne)

Aide générale 000025318 [A] : bouton vert / rouge = afficher ou cacher ; sous le cadenas =
verrouiller ; sous l'ampoule = table lumineuse (clic droit : son panneau) ; sous le rectangle
dégradé = préserver la transparence ; bouton suivant = pochoir, et deux boutons dessous pour le
rendre positif ou négatif ; dossier multicolore = groupe de couleur ; barre blanche = opacité ;
sous la barre = mode de mélange (petits triangles : mode suivant / précédent) ; double-clic sur
le nom = renommer ; triangle à droite du nom ou clic droit = options du calque ; triangle à
gauche = réduire / agrandir.

- Visibilité 000025334 ; « Afficher/Cacher le calque » 000029840 [A] (feuille d'exposition).
- Verrou 000025335, 000029841 [A] : on ne peut plus dessiner. 000025191/192, 095191001.
- Table lumineuse du calque 000025346, 000029842 [A] (clic gauche bascule, clic droit : panneau).
- Préserver la transparence 000025338 [A], 000028493.
- Pochoir positif / négatif 000025336 / 000025337 [A : noms seulement dans l'aide → à préciser].
- Groupe 000025344 [A] ; calque caché par le filtre de groupe 000025333 [A].
- Opacité 000025342 [A] ; opacité d'un dossier 012025347 [A] (agit sur ses calques).
- Mode de mélange 000025343 [A] ; le calque le plus bas est montré en « couleur » (son vrai mode
  gardé) 011025343 [A] ; en « calque courant seul », seul « couleur » 011025344 [A].
- Nom 000025340 ; options 000025341 [A] ; réduire / agrandir 000025339 [A] ; dossiers 012002561.
- Rangée des clés d'effets 000025345 [A].
- Vignette de l'image courante 011085170 [A] ; « Montrer les icônes » 000025190 ; tailles
  000029721–724.
- Verrouiller la position 000025210/211 (erreurs 095191005, 012002853/854).
- Par calque : création automatique d'image 011085171, cassure automatique d'instance 011085172.

Sens confirmés par Anthony le 2026-10-15 (les textes de TVPaint ne font que les nommer) :
- **Préserver la transparence** : le pinceau ne colore que ce qui est déjà dessiné (le vide reste
  vide) ; la gomme gomme normalement.
- **Pochoir** : le dessin du calque-pochoir limite où l'on peint sur les autres calques ;
  positif = seulement dans sa forme, négatif = seulement hors de sa forme.
- **Verrouiller la position** : l'outil Transformer et le déplacement en 3D ne bougent plus le
  calque, et on ne peut plus dessiner dessus.
- **Dossiers** (choix du 2026-10-15, les textes ne font que nommer les fonctions) : un seul
  niveau ; pas de mode de fusion de dossier, chaque calque garde le sien et l'opacité du dossier
  s'ajoute à la sienne ; « Joindre les dossiers » met les calques des dossiers dans un seul et
  efface les autres (celui qui reste est le dossier courant).

## 2. Sortes de calques

Animation 000025198 ; image (statique) 000025197 ; exposition / feuille d'exposition 000025144
(000029815/816, 000029800 [A]) ; CTG (couleur et texture) 012002845 ; dossier 012002562 ; fond
012002844 ; Puppet 012103645 ; calque caméra 129582142 ; calque vidéo 000020117–119 ; calque
préchargé 000025127. Pas de calque papier ni de calque son (le son a son onglet).

## 3. Opérations sur les calques

- Nouveau (choix de la sorte, importer une séquence) 000025304 [A]. Fenêtre : nom, sorte, groupe,
  mode, pré / post-comportement, position (« Sur / Sous le calque courant », « Tout au dessus /
  dessous », « Sur / Sous le groupe courant », « Groupe du calque courant »).
- Gestionnaire de réglages de calques 011030004 (modèle de nom 011030031 [A] : %Y %m %d %H %M %S,
  %i, %pt, %cln ; structure de départ 011030050–066 ; début 011030095 [A] et durée 011030096 [A]).
- Dupliquer (les calques choisis) 000025104/105 ; supprimer le courant ou les choisis 012002614
  [A] ; supprimer tous les calques vides / inactifs 000028556–559.
- Renommer 000025170 ; copier / couper / coller un calque (au-dessus, en remplaçant) 000025113–117.
- Fusionner : choisis / visibles / tous / avec celui du dessous 000025160–162, 011025162 ;
  000025306 [A] (Ctrl + clic pour choisir) ; panneau de fusion : « Tampon » 000025521 [A],
  « Effacer source » 000025522 [A], garder groupe / marques / noms d'instance 011025525–527 [A].
- Réordonner : monter, descendre, rassembler, inverser l'ordre 011025178–184 [N].
- Choisir des calques : même groupe, autres groupes, visibles, tous 012002841 [A] ; menu
  010525100–112.
- Navigation entre calques : premier / dernier / précédent / suivant, visibles, déverrouillés,
  dans le groupe 000020087–096, 011020093–104.
- Tous visibles / tous cachés 000025070/071 ; voir le calque courant seul 000025073 ; bascule
  projet / calque courant 000025303 [A] ; focus sur le groupe 000025074.
- Tous les pochoirs : désactiver / inverser 000025075/076, 000025332 [A].
- Convertir en calque d'animation 000025126. Diviser / assembler des calques 000025171–174 [N].
- Copier / couper la sélection dans un nouveau calque (derrière le courant) 000024563/564 [A].
- Calque de transition 000025168/169, aligner 000025188/189, chaîne 000020144 [N].
- Recalculer le temps, ratio, images/s, nombre d'images, redimensionner 000020929… [N].
- Agrandir / rétrécir le calque 000025550/551 : interpoler 000025580 [A], étirer 000025581 [A],
  répéter (boucle / va et vient / maintenir) 000025582/583 [A], instances vides 000025584 [A],
  couper les images 000025585 [A], expositions vides 000025588 [A].
- Dossiers : placer dans un dossier 012002553, joindre 012002572 [N].
- Effacer le calque 000020092 ; effacer / remplir les images choisies 000029080 [A].
- Fond : aucun / couleur / damier 000020145–147.

## 4. Images, instances, expositions

- Insérer des images (avant / après) 000025201/202 ; ajouter des instances avant / après
  000028590… [N].
- Supprimer l'image / les images / instances choisies 000025108, 000028593, 011028625 ; portée
  (du calque, du projet, sélection) 011025104–107 ; **modes de trou** 011025108–111 : laisser
  vide, combler depuis la gauche, combler depuis la droite, garder en l'état [N].
- Dupliquer des images 000025135. Choisir des images 000025137/138, 000028605.
- Couper / copier / coller des images 000025118–122 ; coller en insérant 000029825 [A] (avant
  l'image courante) ou en remplaçant 000029826 [A] (efface la sélection puis insère) ; taille
  différente 000025600–603 [A].
- Glisser-déposer : menu Insérer / Copier / Déplacer 010086260–262 ; préférence 011021205 [A] :
  sans menu, Ctrl copie, Maj insère, sans touche déplace.
- Inverser les images choisies 000029821 [A]. Retourner H / V 000028612–614.
- Menu Exposition 000028619 : insérer / supprimer une cellule, toutes les cellules, aller à
  l'instance précédente / suivante, tête / queue, casser l'instance, recalculer les cellules
  (avec seuil) 000028620–628 [N] ; ajouter / supprimer n expositions 000028589/594.
- Cassure automatique / création automatique d'image 000028630/631 [N].
- Noms d'instance : renommer 000028485 ; effacer / tout renommer / renommer intelligemment
  011020080–083 ; renommage intelligent 011074300–355 [A] (préfixe, suffixe ; noms vides,
  numériques ou texte) ; double-clic pour renommer 011021213 [A].
- Faux-Fixe 000025208 : répéter / va et vient / aléatoire 000028474–476 [N].
- Intervalles : insérer des images intermédiaires 000020927/928 [N] ; ajouter / supprimer des
  intervalles 000029817–820 [A].
- Clés d'effets sur la timeline 000025109/110, 000025314/315 [A], clé auto 000025316 [A].
- Ctrl pendant le tracé : chaque coup de pinceau sur l'image suivante 000121132 [A].

## 5. Pré / post-comportement

Libellés 000025195/196 ; valeurs aucun / boucle / va et vient / maintenir 000025129–132. Pas de
texte d'aide : sens confirmé par Anthony le 2026-10-15 — avant la première image et après la
dernière image du calque, il montre rien, la boucle de ses images, un va-et-vient, ou sa
première / dernière image maintenue.

## 6. Repères, entrée / sortie, tête de lecture, affichage

- Signets : basculer, suivant / précédent, tout enlever, enlever les choisis 000028467–470/495.
- Marques d'image (26 couleurs) 010087000–011087022 : ajouter / retirer, sur instances vides /
  toutes / non marquées, retirer celles du calque ou du plan, précédente / suivante (par
  couleur), « Répéter » 010087018 [A], « Continuer sur le calque suivant » 010087019 [A],
  afficher les couleurs utilisées seulement.
- Points d'entrée / sortie 000029682–685 (erreur entrée = sortie 100200200 [A]) ; limites de
  boucle 121029686, boucle entre les images choisies 012175103/104.
- Règle : clic = image, glisser = défiler 000025320 [A] ; champ image courante 000025323 [A].
- Affichage time code / image / trame 000025080–082, 000025317 [A]. Image de départ 000023060 [A].
- Adapter : plan / calque / sélection / image / clé / son / tout / expositions 000025000–008 ;
  zoom de la timeline 000025331 [A].
- Préférences : timeline étendue 000121187 [A], verticale 000121189 [A], instances vides
  différenciées / croix 000021277/278, miniatures 000021243/244, icônes des calques 000021266.
- Atténuation des calques 011086295–308 [N].

## 7. Projet et plans

Onglets « Clip: Timeline », « Clip: XSheet », « Projet » 000029650–652, 000029660 [A] (Projet :
tous les plans ; Timeline : les calques du plan courant). Timeline du projet 009580020 [A] (tous
les plans à la suite, pistes son globales en plus de celles des plans). Scènes / plans : ajouter
avant / après, supprimer, dupliquer, diviser, joindre 009580022–038 ; fusionner les plans [A] ;
renommer, cacher, choisir, couper / copier / coller, nouveau projet à partir des plans choisis.
Propriétés : nom, information, auteur, dates, notes, copyright ; action / dialogue / notes par
plan 009580210–212.

## 8. Onglets du bas

Image, Groupe, Notes, Son, Groupes, Arrière-plan, Image de départ 011085130–148.
- Notes 000072047 [A] : dessinées sur l'animation, vues en lecture, jamais rendues ; crayon,
  gomme (couleur choisie), gomme globale, teinte, position, découper ; bleu / rouge / noir ;
  grille 000072045 [A] ; placement de l'affichage 000072014 [A].
- Son : forme d'onde, Ctrl + glisser = décalage 000025322 [A] ; par piste : affichage, bascule,
  options (verrou, décalage, supprimer, dupliquer), volume, verrou [A] ; piste maître [A] ;
  défilement (jamais / scrub / toujours) [A].
- Groupes 011030100–124 [A] : éditer, rendre un groupe visible, mot de passe, appliquer la
  couleur au calque, couleurs utilisées seulement ; menu de groupe en masse 010086200–256.
  Noms par défaut 000060300–312.
- Arrière-plan : aucun / couleur / damier, couleurs A et B 000025300–302 [A].

## 9. Le reste

Table lumineuse complète 000050000–134 [A] (couleurs avant / après, chaque image, opacité par
image et globale, curseurs liés, « Devant », modes instance / image / signet / marque / clé ;
Bin 000050102–110 ; déplacer temporairement / remettre les images 000050064/065). Modes de
colorisation « Coul. / Image / Dég. » 000050010–019 : couleur et couleur de l'image reprises
(Coloriser oui / non), « Dégradé » [N] : sens choisi par Anthony le 2026-10-15, celui de
Photoshop (deux couleurs, de l'image la plus proche à la plus lointaine). Fait en 13f, sauf le
mode clé de rig.
Caméra : clés par paramètre, courbes de vitesse, interpolation linéaire / spline. Feuille
d'exposition (rôles, colonnes, export PDF / XDTS). Flips 011074000–063 [A].

## 10. Raccourcis (configuration TVPaint 12 d'Anthony, section `[keyboard]`)

`,` choisir un calque ; `a` / Ctrl+Gauche instance précédente ; `f` / Ctrl+Droite instance
suivante ; Haut / Bas calque au-dessus / dessous ; Maj + flèches : avec sélection ; Ctrl+Haut /
Bas déplacer le calque ; Ctrl+Début / Fin premier / dernier calque ; Début / Fin première /
dernière image du calque ; PgPréc / PgSuiv clé précédente / suivante ; Ctrl+L table lumineuse du
calque ; Ctrl+Maj+Haut / Gauche / Droite signet basculer / précédent / suivant ; `=` voir tous
les calques ; `!` notes ; Maj+B mode de fond ; Maj+F aller à l'image ; `i` / Maj+I ajouter /
insérer une image ; Maj+F5 supprimer l'image ; Espace lecture ; Retour arrière effacer ; Suppr
supprimer la sélection ; Ctrl+N nouveau calque d'animation.
