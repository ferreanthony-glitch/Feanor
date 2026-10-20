# Interface de SHOT : simplicité et cohérence (passe du 2026-10-19)

Cette passe répond à la demande d'Anthony : « un logiciel qui réponde toujours très rapidement,
sans latence et avec une interface simple et intuitive ». Un petit outil de storyboard façon
Storyboard Pro, avec le dessin et l'animation façon TVPaint. En deux jours, beaucoup de fonctions sont
arrivées vite, de plusieurs mains : cette passe relit l'interface d'un bloc, corrige ce qui peut l'être
sans changer le comportement, et propose le reste (partie 6).

Branche `simplicite` (coupée de `main` à `9a33cd5`). Rien n'est fusionné.

Sommaire : 1 raccourcis, 2 vocabulaire, 3 doublons et impasses, 4 cohérence des gestes (clés),
5 encombrement, 6 propositions, 7 à vérifier à l'écran, 8 transitions, effets sonores et bruitages
(branche `transitions-son`, vocabulaire ajouté), 9 suggestions sonores (branche `generation-son`).

---------------------------------------------------------------------------------------------------

## 1. Raccourcis

### Ce qui a été contrôlé

`keymap.toml` compte 358 blocs (418 commandes dans `keymap::ACTIONS`). L'essai
`keymap_tests::no_two_default_bindings_share_a_key_in_the_same_context` compare maintenant TOUTES les
touches par défaut deux à deux, celles des commandes « en réserve » comprises, dans les contextes
(storyboard / animation / partout) qui se recouvrent.

### Conflits trouvés et corrigés

| Touche | Qui la prenait | Réglé |
|---|---|---|
| Suppr | `edition.supprimer` et `reference.decharger` (en réserve, Storyboard Pro `DRAWING_UNLOAD_MODEL_KEY`) | la commande en réserve perd sa touche (la source reste notée) |
| Maj+Suppr | `dessin.supprimer_hors_selection` et `edition.supprimer_et_resserrer` (en réserve, `DELETE_ALT_KEY`) | idem |

(**Mis à jour le 2026-10-20, partie 24** : à la demande d'Anthony, Maj+Suppr est passée à `selection.remplir`
— remplir une zone bitmap de la couleur courante ; sans zone bitmap choisie elle fait encore « effacer hors de
la sélection », qui garde Maj+Retour arrière.)
Aucune touche active ne changeait de sens : Suppr et Maj+Suppr font ce qu'elles faisaient. **Aucun
raccourci d'Anthony n'a été changé.**

### Touches partagées qui sont voulues (un contexte chacune)

`Inser` (nouveau plan en storyboard, nouveau dessin en animation), `Ctrl+Suppr` (supprimer le plan /
retirer le dessin), `Ctrl+Origine` et `Ctrl+Fin` (début, fin du montage / premier, dernier calque).
Le contexte suit le panneau qui a la main (`key_mode_of`) : la timeline a la main même en storyboard.

### Touches qui ne passent pas par le fichier

Ces gestes sont codés en dur ; ils doivent rester cohérents avec le fichier :

- **Flèches** : déplacent la sélection du dessin (1 px, Maj : 10) AVANT `calque.dessus` / `dessous`,
  `image.precedente` / `suivante` (`nudge.rs`). Ctrl ou Alt + flèche gardent leurs raccourcis.
- **Entrée, Échap, Retour arrière** : d'abord à l'outil en cours (forme à finir, texte à valider),
  ensuite seulement aux raccourcis (`tool_key`). Échap abandonne aussi un glisser (avec Ctrl+Z).
- **Tab / Maj+Tab** : retirés à egui avant le focus (`pending_keys`) pour servir de raccourcis.
- **Ctrl tenu seul** : lasso temporaire (outils de dessin) ; **Ctrl+Maj tenus** avec l'outil
  Sélection : tous les calques du plan (nouveau, partie 4.4).
- **Espace tenu** : déplacer la vue (`vue.pan_libre`, maintenir).

### Commandes importantes sans touche par défaut

Elles sont dans la palette (Ctrl+K) et au clic droit « Assigner un raccourci… », mais pas au clavier :
`fichier.exporter`, `camera.vue` (vue à travers la caméra), `commentaire.ajouter`, `marque.basculer`,
`sequence.nouvelle`, `storyboard.verrouiller`, `outil.camera`, `outil.transformer`,
`outil.perspective` (Déformer), `outil.texte`, `outil.rectangle` / `ellipse` / `ligne`, `espace3d.basculer` (Espace 3D ; `vue.3d` et `scene3d.basculer` en sont des doubles),
`fx.panneau`, `vue.calques`, `vue.couleur`, `lecture.boucle`, `plan.avancer` / `reculer`,
`panneau.dupliquer`, `dessin.vider`. **Rien n'a été posé** : ce sont des choix d'Anthony (partie 6.2).

Le panneau Références (`vue.references`, `references.importer_image`, `references.importer_video`,
`references.coller`, `references.supprimer`, `references.lier_plan`, `references.filtre_plan`,
`references.tout_voir`) n'a pas de touche non plus ; Ctrl+V et Suppr y agissent quand le panneau a la
main (le dernier clic y était), comme pour les panneaux Effets et Commentaires.

### Fichier complété

60 commandes n'avaient pas de bloc dans `keymap.toml` (exports, panneaux, commentaires, aides au
cadrage, clés d'effets…). Elles y sont, sans touche, pour que le fichier les nomme toutes (liste F1,
fichier personnel). L'essai `every_action_has_a_block_in_the_file` garde la liste complète.

### Ctrl+K

La palette liste maintenant chaque commande une fois, sous son nom : les six doubles `scene.*`
(« Nouveau plan (second raccourci) »…, des doubles de `panneau.*` créés pour poser une seconde touche)
n'y sont plus (`keymap::ALIASES`). Les commandes pas encore faites n'y étaient déjà pas. L'essai
`every_action_has_a_clear_french_label` exige un nom français net (majuscule, pas d'identifiant, pas
d'espace en trop) pour les 418 commandes ; `the_command_palette_lists_each_command_once_under_its_own_name`,
aucun nom en double.

Libellés périmés corrigés : « Couper le plan à l'image courante (comme F5) » (c'est F6), « Afficher /
masquer le panneau Outils » (c'est la colonne des réglages et de la fiche), « Afficher / masquer la fiche
et les calques » (c'est la colonne des calques).

---------------------------------------------------------------------------------------------------

## 2. Vocabulaire

### Glossaire (appliqué aux textes visibles)

| Idée | Mot | À éviter |
|---|---|---|
| Unité du storyboard | **plan** (une séquence regroupe des plans) | scène, shot, panneau (le mot « panneau » reste dans les échanges avec Storyboard Pro : c'est le sien) |
| Zone de l'interface | **panneau** (Calques, Effets…) ; colonne, barre | fenêtre (réservé aux vraies fenêtres) |
| Début d'un dessin sur un calque | **clé** ; **clé de caméra** ; **clé d'effet** | « image clé » (une image est une image du film) |
| Ce que montre une clé | **dessin** ; sa durée : **exposition** | |
| Images, calques, clés, plans qu'on désigne | **choisir** / choix | sélectionner |
| Éléments et pixels dans un dessin | **sélectionner** / sélection (outil Sélection) | choisir |
| Effet | **effet** (pile d'effets) | filtre |
| Valeurs réglables d'un outil, d'un projet, d'un objet | **Réglages** | options, paramètres (sauf noms d'effets) |
| Retirer un objet | **Supprimer** (plan, calque, clé, effet, piste, commentaire) | |
| Retirer une propriété | **Retirer** (raccourci, marque, signet, rôle, pastille, fond) | |
| Vider du contenu dessiné | **Effacer** / **Vider** | |
| Les dessins d'avant et d'après | **pelure d'oignon** (jamais « Pelure » seule dans un titre) | |
| Région fermée par le trait, coloriée d'un aplat | **zone** ; le coloriage s'appelle **Colorier par zones** (calque de **trait**, calque de **couleur**, **clés**) ; ce qu'on ne sait pas colorier : **zone à vérifier**, repérée en **magenta** | cellule, région, masque |
| Vue sur le film | **montage** (storyboard) ; **timeline** (animation) ; **bande** seulement pour les deux ensemble | |
| Traits d'aide du cadre | **aides au cadrage** | repères (réservé aux repères de la timeline : signets, entrée, sortie) |
| Menu ⋮ d'un calque ou d'un groupe | **Commandes du calque** / **du groupe** | options |
| Importer un projet Storyboard Pro | **Importer** (l'accueil disait « Ouvrir ») | |
| L'espace où l'on place les calques, les objets 3D et la caméra | **Espace 3D** (le plateau en 3D et la scène 3D d'avant ne font plus qu'un) ; **carte** (un calque du plan posé dans l'espace) ; **objet** (cube, mannequin, modèle…) ; **décor** (commun à la séquence) ; **caméra** (celle du plan, la seule) ; **calque « Fond 3D »** (les objets figés dans un calque) | maquette, set, scène 3D, plateau en 3D, caméra du plateau |
| Basculer | « (montrer / cacher) » pour les barres et colonnes, « (ouvrir / fermer) » pour les panneaux, « (oui / non) » pour une option, « (allumer / éteindre) » pour la pelure et les pochoirs, « (poser / retirer) » pour les marques et signets | afficher / masquer |

### Changements faits (étiquettes seulement)

- « Image clé de caméra » → « Clé de caméra » (bulles, menus, barre d'état, actions), « Images clés de
  la caméra » → « Clés de caméra », « Retirer la clé » → « Supprimer la clé de caméra ».
- « Pelure : … » → « Pelure d'oignon : … » (9 commandes) ; « Panneau de la pelure » et « Réglages de la
  pelure » (le même bouton, deux noms) → « Réglages de la pelure d'oignon ».
- « Options du calque » → « Réglages du calque » ; « Options du calque » (bulle du ⋮) et « Tout ce qui
  touche au calque » → « Commandes du calque ».
- Aides au cadrage : « Couleur / Opacité des repères », « Éteindre les repères » → « … des aides au cadrage ».
- Effets : « Retirer l'effet » → « Supprimer l'effet » ; réglages rangés de la pelure : « Effacer »
  → « Supprimer ».
- Accueil : « Ouvrir un projet Storyboard Pro » → « Importer un projet Storyboard Pro (.sboard)… ».
- « Nouveau calque » (bouton du montage, qui fait un calque vectoriel) → « Nouveau calque vectoriel ».
- Les doubles `scene.*` ne font plus qu'une ligne dans la palette.
- Chemins de menu périmés dans les messages (« Aide > Recharger les raccourcis », « Calques > Nouveau
  calque vectoriel », « Fichier > Exporter le film », « Fichier > Ouvrir un projet Storyboard Pro »).

### Ajout du 2026-10-19 : « Colorier par zones… »

- Commande `calque.colorier_zones` (sans touche, comme `calque.propager`) : menu Animation, clic droit
  d'un calque (montage, timeline, panneau Calques : la ligne cliquée est le calque de trait), palette
  Ctrl+K, et « Assigner un raccourci… » au clic droit de l'entrée. Elle ouvre la fenêtre « Colorier par
  zones » (`coloriageui.rs`) ; la tâche tourne sur un autre fil (fenêtre de progression, « Arrêter ») ;
  une seule étape d'annulation, « Colorier par zones ».
- Vocabulaire de la fenêtre : « Trait », « Clés peintes sur », « Écrire les aplats », « Images », « Fermer
  les trous du trait jusqu'à », « Zone incertaine », « Confiance minimale ». La liste « N à vérifier » dit
  « Image 12 · zone 3 · nouvelle zone » ; un clic va à l'image.
- Distinct de « Propager la peinture… » (EbSynth : peinture sur une vidéo) : ici on retrouve les mêmes
  zones fermées d'un dessin au suivant et on pose des aplats nets, sans pixels générés.

### Restes à décider (non changés)

- **« Arrière-plan »** désigne à la fois la couleur de la feuille (onglet Arrière-plan, `fond.*`) ET la
  deuxième couleur du couple de couleurs (avant-plan / arrière-plan). Proposition : la feuille devient
  « Fond de la feuille ».
- **« Fond »** : fond sous le trait (fonction du calque), calque de fond (décor commun), fond de la feuille,
  fond du thème. Trois sens produits par l'application ; garder « Fond sous le trait » en entier.
- **« Propriétés du plan »** (action, Alt+Entrée, nom de Storyboard Pro) ouvre le panneau « Fiche du plan »
  (onglet « Fiche »). Trois noms pour une chose. Proposition : « Fiche du plan » partout.
- « Image clé » reste dans les textes de la propagation (EbSynth : les images peintes à la main) ;
  fichiers `propager*.rs` non touchés à la demande d'Anthony.
- Le tutoiement (« Clique », « Choisis ») est partout, sauf une phrase de `diagnosticui.rs` (vouvoiement,
  « votre matériel »). À uniformiser avec la voix de l'application.
- Le texte posé sur un dessin de décor (ocre) dans les thèmes sombres n'a que 1,9:1 de contraste. Comme
  avant ; à voir à l'écran.

---------------------------------------------------------------------------------------------------

## 3. Doublons et impasses

### Une commande, plusieurs endroits, plusieurs noms (avant / après)

| Commande | Noms avant | Après |
|---|---|---|
| Début du montage | bouton « Début du montage » qui annonçait la touche de `image.premiere` (première image du PLAN) | lié à `montage.debut` (Ctrl+Origine) |
| Réglages de la pelure | « Panneau de la pelure » (timeline), « Réglages de la pelure » (barre du dessin) | un seul nom |
| Coller sur place | entrée du menu Édition identique à Coller | remplacée par « Coller à la tête de lecture (clés) » ; la commande reste dans la palette |
| Plan précédent / suivant | `panneau.precedent` (PgPréc) et `scene.precedente` (Ctrl+PgPréc) : la même chose | doubles déclarés, cachés de la palette |
| Nouveau commentaire | « Nouveau commentaire » / « Nouveau commentaire ici » / « Commentaire ici » | à unifier (voir ci-dessous) |
| Marque d'image | « (ajouter / retirer) » et « (poser / retirer) » | à unifier : « (poser / retirer) » |
| Signet | « Signet ici » / « Signet du plan ici » / « Signet à l'image courante » | à unifier |
| Lecture (transport) | barre de commandes du haut ET barre du montage ou de la timeline : les mêmes cinq boutons | voir partie 5 |
| Animer le plan | bouton « Animation » du haut, « Animer ce plan » (menu, montage, timeline), clic droit | trois noms, un geste |
| Vue dessin / vignettes | barre du montage ET barre du dessin (`vue.planches`) | deux boutons, une commande |

### Entrées qui ne font rien ou pas encore

Quatorze commandes sont `implemented: false` (rotation et zoom temporaires, grille, outil zoom, peindre
les zones non peintes, épaisseur du crayon, aplatir, aimanter au contour, remplir les zones, décharger la
référence, chercher dans les légendes, sélectionner les dessins du plan, supprimer et resserrer).
**Aucune n'a d'entrée de menu, de bouton ni d'entrée de palette** (vérifié) ; elles sont dans la liste F1,
section « En réserve » repliée, sans touche active (essai `what_is_not_made_yet_has_no_active_key`).
Rien à masquer de plus.

### Bulles hors `bulles.rs`

194 appels `on_hover_text` / `on_hover_ui` restent (43 fichiers), pour la plupart sur des champs de
réglage et des libellés de panneaux. Passés par `bulles::bulle` (nom en gras, description, raccourci) :
les boutons d'icônes de la timeline, des pistes son (verrou, dupliquer, supprimer), de la pile d'effets
(choisir, monter, descendre, supprimer, clé précédente / suivante, actif), du filtre des raccourcis, des
aides au cadrage, des réglages rangés de la pelure, des menus ⋮ de groupe, des menus du clic droit sur
une clé. Le reste (curseurs et champs des panneaux Pinceaux, Effets, Caméra, Pelure, Flips) garde
l'infobulle simple d'egui : c'est un chantier mécanique pour une prochaine passe.

Un essai (`no_hand_made_selected_button_forgets_its_ink`) interdit désormais un bouton d'état ou un choix
qui force la couleur de son texte.

### Couleurs en dur

- **Dans `theme.rs` désormais** : les poignées de sélection (le même bleu recopié dans six fichiers),
  de déformation et de transformation, les cadres de composition (zones de sécurité, format), la teinte
  des calques de décor (nouvelle couleur de palette `decor` / `calque_decor`, lisible sur les thèmes clairs).
  La caméra prend `theme::camera()`, comme ses clés dans la timeline (avant : un jaune recopié dans
  trois fichiers). Essai `the_overlay_colours_are_written_once_in_the_theme`.
- **Icônes allumées** : une icône de bouton d'état restait claire sur le gris clair de l'accent (thème Gris
  sombre). `egui::Button::selected` dessine son texte avec la couleur du survol, pas celle de la
  sélection. Tous les boutons d'état passent par `bulles::symbole_etat` / `encre_etat`
  (`theme::sur_accent`) : `etat`, `interrupteur`, `choix`, `bouton_etat`, et les boutons faits à la main
  (réglages de la pelure, flips, effets, notes, cadrage). Les noms en gras des choix (nom d'un effet choisi,
  onglets Réglages / Résolution, modes Animation / Storyboard) aussi.
- **Volontairement hors thème** : les vues techniques (Vue de dessus, Espace 3D : fond sombre et gris
  propres), les couleurs du papier de la planche, des notes et de leurs plumes (données du projet), les
  vignettes et leurs teintes de visibilité, les ombres et voiles noirs et blancs de lisibilité sur le dessin,
  les couleurs choisies par l'utilisateur (nuanciers, groupes, commentaires, marques).

### Indications périmées corrigées

- « l'outil Sélection (V) » (V est la rotation de la vue) : la touche est lue dans le fichier (Alt+S).
- « Aide > Modifier / Recharger les raccourcis » : ces entrées n'existent plus (c'est Préférences et la
  liste F1) ; l'en-tête de `keymap.toml` et le message du Bloc-notes le disent.
- README : « Vectoriel (V…) » et « Bitmap (B…) » sont les anciennes touches ; ce sont Maj+S et Maj+B.
- Le message de démarrage (« P pour un nouveau plan », « F1 : raccourcis ») est juste.

---------------------------------------------------------------------------------------------------

## 4. Cohérence des gestes des trois sortes de clés

### 4.1 Tableau (après cette passe)

| Geste | Clés de dessins | Clés de caméra | Clés d'effets |
|---|---|---|---|
| Clic | choisit la clé | idem | idem |
| Ctrl + clic | ajoute / retire, sur n'importe quel calque | idem (d'un plan à l'autre) | idem (sur l'effet choisi) |
| Maj + clic | le rectangle entre la clé de départ et celle-ci, **rangées d'entre comprises** (était : la même rangée) | toutes celles d'entre, d'un plan à l'autre | toutes celles d'entre, sur ce calque |
| Rectangle | oui, sur **toutes les rangées couvertes** ; Maj ajoute, Ctrl bascule (était : Ctrl ajoutait) | oui (piste Caméra) ; Maj ajoute, Ctrl bascule | non |
| Glisser | la clé et TOUTES celles d'après sur son calque suivent (chaque calque touché) ; Maj : seules les choisies ; Alt : réordonner | la clé et toutes celles d'après dans son plan ; Maj : seules les choisies | la clé et toutes celles d'après du même effet ; Maj : seules les choisies |
| Ctrl+C / X | oui | oui | oui |
| Ctrl+V | **insère** après la clé de la tête, repousse la suite | idem | idem |
| Maj+Ctrl+V | colle exactement sur la tête | idem | idem |
| Suppr | la voisine remplace (ou un vide) | supprime | supprime |
| Ctrl+A | toutes les clés du calque courant | **toutes les clés de caméra du plan (nouveau)** | **toutes les clés de l'effet (nouveau)** |
| Ctrl+D | dupliquer sur place | — | — |
| Clic droit | menu commun (4.2) | idem | idem |
| Annulation | une étape par opération | idem | idem |

### 4.2 Le menu du clic droit : même contenu, même ordre, mêmes noms

Avant : les trois menus avaient été écrits à part. La caméra grisait Copier sans clé, les effets non ;
« Choisir toutes… » n'existait que pour la caméra ; l'ordre variait ; les bulles disaient trois fois la
même chose avec trois phrases. Maintenant `clescommun.rs` décrit **cinq commandes** :
Copier la clé…, Coller les clés…, Coller à la tête de lecture, Supprimer la clé…, Choisir toutes les clés…
(+ « (n) » quand plusieurs sont choisies), avec le raccourci de `edition.*` et la même bulle. Chaque
menu a la même forme : poser une clé / **les cinq commandes** / ce qui est propre à la sorte.

- Clés de dessins : Nouvelle clé ici (copie du dessin), Nouvelle clé vide ici | les cinq | Dupliquer sur
  place, Supprimer en laissant un vide | Calque « … ».
- Clés de caméra : Clé de caméra ici / au début / à la fin, Adoucir, Supprimer la clé d'ici | les cinq |
  Effets de caméra, Vue à travers la caméra, Réinitialiser, Adapter la timeline.
- Clés d'effets : les cinq | Pile d'effets du calque, Clés d'effets (montrer / cacher).

Essais : `the_menus_of_the_three_kinds_of_keys_list_the_same_entries_in_the_same_order` et
`the_three_menus_are_drawn_by_the_shared_code_not_by_hand` (aucun menu ne refait une entrée à la main).

### 4.3 Règle de choix partagée

`clescommun::choose_group` (clic, Ctrl, Maj) et `clescommun::rect_choice` (rectangle) remplacent trois
copies de la même règle. Les essais existants de chaque sorte passent sans changement.

### 4.4 Choisir plusieurs clés de plusieurs calques (demande d'Anthony)

Dans le **montage** :

- **Rectangle** : un glisser qui part d'un espace vide des pistes choisit toutes les clés touchées sur
  toutes les rangées couvertes (avant ces rangées étaient déjà couvertes, mais Ctrl ajoutait au lieu de
  basculer).
- **Ctrl + clic** : bascule une clé sur n'importe quel calque.
- **Maj + clic** : le rectangle entre la clé de départ et la clé cliquée : rangées d'entre comprises,
  clés dont le début est entre les deux images.
- **Glisser** : toutes les clés choisies, calque par calque, chacun avec sa propre règle (la clé prise
  et toutes celles d'après suivent ; Maj : seules les choisies) ; une étape d'annulation.
- **Ctrl+C / Ctrl+V** : chaque clé garde son calque RELATIVEMENT au plus haut calque copié : celui-ci va
  sur le calque courant, les autres en dessous, dans le même ordre et avec les mêmes écarts. Les calques
  qui manquent sous la pile sont créés (nom et sorte repris). Avant, la règle cherchait un calque de même
  nom dans le plan d'arrivée (créé s'il manquait). **C'est un changement de règle**, demandé par Anthony ;
  les essais de l'ancienne règle ont été remplacés. Le bloc s'insère à un seul endroit pour tous les
  calques (après la clé de la tête de lecture sur le calque courant), ce qui garde les écarts entre
  calques.
- **Maj+Ctrl+V** : exactement sur la tête ; **Suppr** : toutes ; une étape d'annulation chacune.
- Les clés choisies sont cerclées du contour du thème ; la barre d'état dit « 12 clés sur 3 calques ».

Dans la **timeline de l'animation**, l'unité n'est pas la clé mais l'image : le rectangle choisit déjà
des images sur toutes les rangées couvertes (calques choisis automatiquement), Maj + clic étend la plage
d'images, Suppr, glisser (Ctrl : copier, Maj : insérer) et copier-coller agissent sur ce choix, avec leurs
propres règles. **Je n'ai pas fusionné les deux modèles** (clés / images) : le montage reste le lieu du
choix de clés ; en animation, le montage est visible avec la timeline (chantier 6). Pas d'ambiguïté :
le rectangle prend la sorte de clé de la rangée où il PART (piste Caméra : caméra ; pistes de calques :
dessins ; les clés d'effets sont en haut des pistes, elles se choisissent au clic).

Dans le **dessin** (Storyboard Pro : « Ctrl+Maj + outil de sélection sur le canevas ») : avec l'outil
Sélection, Ctrl+Maj tenus pendant le lasso, le rectangle ou le clic prennent sur TOUS les calques du
plan à l'image courante (visibles, déverrouillés, non vidéo, avec un dessin ; les dossiers comptent par
leurs calques). Les calques où quelque chose a été pris deviennent les calques choisis et leurs clés à
l'image courante sont choisies dans le montage. La boîte entoure tout ; déplacer, échelle, rotation,
inclinaison s'appliquent à tout, une étape d'annulation (c'est la sélection multi-calques existante,
`multisel.rs`). Ctrl seul (lasso temporaire, boîte autour du pivot) et Maj seul (ajouter / retirer) gardent
leur sens. ~~Ctrl+Maj remplace la sélection (il n'ajoute pas).~~ **Depuis le 2026-10-20, Ctrl+Maj cumule** (partie
24) : un objet s'ajoute à la sélection ou en est retiré, un lasso ajoute ce qu'il prend.

### 4.5 Différences qui restent (voulues ou à décider)

1. Pas de rectangle pour les clés d'effets (leurs losanges sont en haut de la piste d'un calque, le
   rectangle prendrait des clés de dessins). À décider si Anthony en veut un (touche de départ différente).
2. Maj + clic des clés d'effets reste sur le calque de l'effet choisi ; celui de la caméra et des dessins
   passe d'un plan à l'autre.
3. Suppr : la clé de dessin laisse une voisine tenir plus longtemps (ou un vide : « Supprimer en laissant
   un vide ») ; la caméra et les effets suppriment simplement.
4. Alt + glisser (réordonner) n'existe que pour les dessins.
5. Ctrl+D (dupliquer sur place) n'existe que pour les dessins.
6. Dans la timeline en animation, le clic droit sur un dessin ouvre « Rôle / Images », pas le menu des clés.

### 4.6 Zoom de la bande calé sur la tête de lecture (demande d'Anthony)

« Le zoom sur la timeline doit zoomer sur la zone de la tête de lecture. » Tout geste de zoom du montage
et de la timeline garde la tête de lecture au même endroit de l'écran (`zoombande.rs`) : le curseur du
bas, les boutons + et −, les raccourcis (Alt+1, Alt+2) et Ctrl + molette. Si la tête n'est pas dans la
vue, elle vient d'abord au milieu, puis le zoom se fait autour d'elle. **Ctrl + molette zoome autour de
la tête de lecture, pas de la souris** (c'est ce qu'il a demandé ; rien n'est gêné : la molette seule
défile toujours). Mécanisme : le geste note la distance de la tête au bord gauche de la vue, et à
l'image d'écran suivante, avec la nouvelle largeur d'image, la bande défile de la différence. Une
demande de défilement de plus de quatre images d'écran (bande cachée) est oubliée.

---------------------------------------------------------------------------------------------------

## 5. Encombrement

Constats (le nombre de commandes visibles en même temps, hors panneaux) :

| Endroit | Éléments |
|---|---|
| Barre de commandes du haut | 19 icônes ; **le transport (5 boutons + boucle + son) refait la barre du bas** |
| Barre du montage | 21 icônes + 3 menus (lecture, plans, calque, copier / coller / supprimer, déplacer, verrou, aimant, son, vidéo, commentaire, animer, vue) |
| Barre de la timeline | une trentaine de contrôles dans une rangée qui défile (transport, repères, affichage, dessins, images, caméra, marque, commentaire, pelure, flips, effets) |
| Barre de la vue (sous le dessin) | 14 icônes + curseurs + une ligne d'aide de l'outil |
| Barre au-dessus du dessin | vue plateau / caméra, outils de l'Espace 3D quand il est ouvert, zones de sécurité, format, aides au cadrage |
| Menus | Fichier 12, Édition 12, Storyboard 10, Calques 10 à 12, Animation 8, Affichage 9 |

À cacher par défaut ou ranger sous « Plus… » sans rien supprimer (suggestions) :

1. **Barre de commandes** : le transport (déjà dans la barre du bas), « Dupliquer le plan » et « Supprimer
   les plans » (barre du montage, menu Plans) ; garder Nouveau / Ouvrir / Enregistrer / Exporter, Annuler /
   Rétablir, Couper / Copier / Coller.
2. **Barre du montage** : « Avancer / Reculer le plan », verrou, aimant, « Importer une vidéo »,
   « Nouveau commentaire » sous « Plus… ».
3. **Barre de la timeline** : « Marque d'image », « Nouveau commentaire », Flips, Effets, la cadence
   (réglage du projet) sous « Plus… » ; la pelure et son réglage restent.
4. **Barre de la vue** : miroir, table lumineuse (et son curseur) sous « Plus… ».
5. **Menus** : Animation (8) pourrait n'être que Lecture et Images ; Fichier : « Réglages du projet… » et
   « Préférences… » côte à côte.

Aucune n'était faite dans cette passe : c'est la partie 6.3 (barre par défaut plus mince), **faite le
2026-10-19** (voir la partie 14).

---------------------------------------------------------------------------------------------------

## 6. Propositions pour Anthony (toutes faites le 2026-10-19 : 6.1 et 6.3 dans la partie 14, 6.2 ci-dessous)

### 6.1 Espaces de travail par tâche : Storyboard, Animation, Montage — ✅ fait (partie 14)

- **Quoi** : trois dispositions enregistrées (panneaux visibles, leurs places, barres visibles), que les
  boutons de mode existants (« Storyboard » / « Animation ») et le menu Affichage > Espaces de travail
  appliquent. « Montage » : la bande du montage large, les panneaux de dessin repliés.
- **Existant** : `workspace.rs` / `workspaceui.rs` ont déjà des espaces (Standard, Dessin, Timeline, Vue
  d'ensemble) et savent enregistrer ses propres dispositions ; ce qui manque est le lien avec le mode.
- **Coût** : petit à moyen, 1 à 2 jours (lier un espace à chaque mode, retenir la dernière disposition
  par mode, trois dispositions livrées, essais d'aller-retour).
- **Ce qu'Anthony voit** : passer en Animation ouvre les panneaux Calques, Effets et la timeline en grand ;
  revenir en Storyboard remet le montage et la fiche du plan. Sa disposition de chaque mode se retient.

### 6.2 Jeu de raccourcis « Façon Storyboard Pro » (FAIT, branche `raccourcis-sbp`)

Seulement ce que `keymap.toml` documente (les 65 raccourcis de `sb_shortcuts.xml` de Storyboard Pro 27,
leur identifiant est dans le champ `source`). Au départ, SHOT s'écartait de Storyboard Pro sur :

| Commande SHOT | Storyboard Pro 27 | SHOT aujourd'hui | Raison notée dans le fichier |
|---|---|---|---|
| plan précédent | `PREV_PANEL_KEY` : A | PgPréc | A / F vont à la clé précédente / suivante (choix d'Anthony) ; **gardé ainsi dans le jeu** (voir plus bas) |
| plan suivant | `NEXT_PANEL_KEY` : F | PgSuiv | idem |
| coller sur place | `PASTE_IN_PLACE_KEY` : Ctrl+Maj+V | pas de touche | Ctrl+Maj+V colle à la tête de lecture (choix d'Anthony) ; **gardé ainsi dans le jeu** |
| couper le plan | `SPLIT_PANEL_KEY` : F5 (ses réglages perso) | F6 | F5 pose une clé ; **gardé ainsi dans le jeu** |
| supprimer et resserrer | `DELETE_ALT_KEY` : Maj+Suppr | pas de touche | commande pas faite, touche prise |
| décharger la référence | `DRAWING_UNLOAD_MODEL_KEY` : Suppr | pas de touche | commande pas faite, touche prise |
| lier les clips au panneau | `LINK_CLIP_AND_PANEL` : Ctrl+L | retirée (Ctrl+L = pelure du calque) | sans objet depuis les plans |
| pinceau pochoir | `DRAWING_STENCIL_BRUSH_TOOL_KEY` : Alt+X | retirée | le pochoir de calque existe |

**Ce qui est fait.** Deux jeux, au choix dans **Fichier > Préférences… > Raccourcis clavier** et en tête
de la liste **Fichier > Raccourcis clavier… (F1)** : « Jeu de raccourcis » = *SHOT (par défaut)* ou
*Façon Storyboard Pro*. Le second est le fichier `keymap-storyboard-pro.toml` (intégré au programme, même
format que `keymap.toml`, avec un champ `source` par bloc). **Pour l'instant il garde toutes les touches de
SHOT** : il ne redéfinit aucune commande (aucun bloc `[[raccourci]]`, le format l'accepte) et se contente de
noter des commandes « à vérifier » (plus bas). Anthony garde A / F, F5 et Ctrl+Maj+V comme il les a réglés ;
les deux jeux donnent donc les mêmes touches à chaque commande. S'il décide un jour de reprendre une touche de
Storyboard Pro, elle s'ajoutera au jeu par un bloc `[[raccourci]]` (avec son `source`).

**Pas pris à Storyboard Pro, volontairement (choix d'Anthony, 2026-10-19).** « A et F ne servent plus à aller
de clé en clé dans ce jeu ; F5 ne crée plus de nouvelle clé : je veux garder ces raccourcis comme je les ai
mis en place. » Idem pour Ctrl+Maj+V. Le jeu ne touche donc pas à :

- **Ctrl+Maj+V** : reste `edition.coller_a_la_tete` (« Coller les clés à la tête de lecture »), comme dans
  SHOT ; `edition.coller_sur_place` (`PASTE_IN_PLACE_KEY` de Storyboard Pro, Ctrl+Maj+V chez lui) reste une
  commande sans touche ;
- **A / F** : restent `cle.precedente` / `cle.suivante` ; le plan précédent / suivant (`panneau.precedent` /
  `panneau.suivant`) garde PgPréc / PgSuiv, au lieu du A / F de Storyboard Pro (`PREV_PANEL_KEY` /
  `NEXT_PANEL_KEY`) ;
- **F5** : reste `dessin.nouvelle_cle` (plus « à vérifier » : elle garde sa touche) ; `panneau.couper` garde F6,
  au lieu du F5 de son Storyboard Pro (`SPLIT_PANEL_KEY`).

Le fichier du jeu le dit en commentaire ; ne les y remettre qu'après une nouvelle décision d'Anthony.

Quatre commandes **gardent la touche de SHOT** (elles ne changent pas) et sont notées « à vérifier »
dans le jeu, parce que la touche de Storyboard Pro n'est établie nulle part : `mode.basculer` (Tab) et
`vue.planches` (Maj+Tab), dont le rôle dans Storyboard Pro n'est pas vérifié ; `aide.palette` (Ctrl+K,
proposition de SHOT) ; `dessin.nouvelle_cle_vide` (F4, réglage perso d'Anthony dans SHOT). Total : **4
« à vérifier »**, toutes sur des touches de SHOT ; la liste F1 les marque d'une étiquette « à vérifier ».

**Pas reprises (rien à poser, la commande n'existe pas dans SHOT)** : `DELETE_ALT_KEY` (Maj+Suppr, supprimer
et resserrer : pas faite), `DRAWING_UNLOAD_MODEL_KEY` (Suppr, décharger la référence : pas faite),
`LINK_CLIP_AND_PANEL` (Ctrl+L : sans objet depuis les plans), `DRAWING_STENCIL_BRUSH_TOOL_KEY` (Alt+X :
retiré, le pochoir de calque existe). Suppr, Maj+Suppr et Ctrl+L gardent donc leur sens dans les deux jeux.
Toutes les autres commandes de Storyboard Pro 27 que `keymap.toml` cite ont déjà la même touche dans SHOT,
ou n'en ont pas dans Storyboard Pro (« sans touche par défaut » : on n'invente pas).

**Règle de fusion** (trois couches, chacune remplace la précédente *commande par commande*) :
`keymap.toml` (SHOT) < jeu choisi < fichier personnel (`%APPDATA%\SHOT\keymap.toml`).

- Un jeu ne touche que ses commandes ; il écrit lui-même les retraits (une touche donnée à une commande est
  retirée à celle qui la portait), donc il n'a aucun conflit en lui (essai
  `keymap_jeux_tests::the_storyboard_pro_set_has_no_internal_conflict`, même contrôle que pour SHOT).
- Les raccourcis de l'utilisateur sont **gardés par-dessus le jeu**, dans les deux sens : une commande qu'il
  a changée garde sa touche quel que soit le jeu ; les autres prennent celles du jeu. Si sa touche est aussi
  dans le jeu, la sienne joue et la liste (F1) signale le conflit.
- Une copie du fichier par défaut, mot pour mot (ce que « Modifier le fichier à la main » crée), ne compte
  pas comme personnelle : sinon elle cacherait le jeu en entier.
- Limite : si l'utilisateur prend à une autre commande une touche du jeu (fenêtre « Assigner »), le retrait
  est écrit dans son fichier ; il reste après un changement de jeu. « Par défaut » (liste F1) rend la touche.
- Changer de jeu demande confirmation quand il y a des raccourcis personnels ; sinon c'est immédiat. Le choix
  est dans les réglages (`jeu_raccourcis`, absent = SHOT : les anciens réglages se lisent comme avant).
- La liste F1 dit, par commande, d'où vient sa touche (*SHOT*, *jeu* ou *perso*) ; la palette Ctrl+K montre
  toutes les touches actives de la commande, avec *jeu* ou *perso* quand ce n'est pas SHOT.

### 6.3 Barre par défaut plus mince — ✅ fait (partie 14)

- **Quoi** : la barre de commandes du haut ne garde que Nouveau / Ouvrir / Enregistrer / Exporter,
  Annuler / Rétablir, Couper / Copier / Coller ; les barres du montage et de la timeline mettent leurs
  commandes rares sous un bouton « Plus… » (liste de la partie 5). Rien n'est supprimé : tout reste dans
  les menus, la palette Ctrl+K et « Plus… ». Un réglage « Barres complètes » les remet toutes.
- **Coût** : moyen, 2 à 3 jours (un composant « Plus… » commun aux barres, le réglage, les essais de
  fumée qui comptent les icônes).
- **Ce qu'Anthony voit** : une barre du haut de 9 icônes au lieu de 19, une barre du montage de 12 icônes
  au lieu de 21, une timeline qui ne défile plus.

---------------------------------------------------------------------------------------------------

## 7. À vérifier à l'écran

Rien de cette passe n'a été regardé : aucun lancement de SHOT (consigne). À voir :

1. **Icônes allumées** dans les trois thèmes : boucle (barre du haut), outil actif, panneau actif de la
   barre de droite, bascule de vue (barre du dessin), aimant (montage), plume / vue dessin (montage),
   réglages de la pelure, flips, effets (timeline), notes, nom d'un effet choisi, modes Storyboard /
   Animation, onglets Réglages / Résolution du projet : toujours lisibles sur l'accent.
2. **Zoom de la bande ancré sur la tête de lecture** : curseur du bas, boutons, Alt+1 / Alt+2, Ctrl +
   molette, dans le montage et dans la timeline ; tête hors de la vue (elle vient au milieu).
   Un défilement de rattrapage se fait à l'image d'écran suivante (une image de décalage possible).
3. **Menus du clic droit** des trois sortes de clés (ordre, noms, grisés).
4. **Choix de clés** : rectangle sur plusieurs rangées, Ctrl bascule, Maj + clic en rectangle ; collage
   sur plusieurs calques (calques créés sous la pile) ; barre d'état « 12 clés sur 3 calques ».
5. **Ctrl+Maj + outil Sélection** sur le canevas : le lasso prend sur tous les calques, la boîte entoure
   tout, les clés se choisissent dans le montage.
6. **Décor** : nom et dessins des calques de toute la scène dans les trois thèmes.
7. **Camera** : le cadre de la caméra a la couleur de la palette (jaune dans les thèmes sombres,
   brun ocre dans Gris clair).
8. **Palette Ctrl+K** : pas de double.
9. **Panneau Références** (icône de la barre des panneaux, une de plus : onze) : déposer des images
   sur le panneau, Ctrl+V d'une image copiée dans un navigateur, glisser / agrandir / tourner une
   référence, Alt+clic = pipette, lecture d'une vidéo et « suivre la tête de lecture », puis « Dans
   une fenêtre à part » sur un deuxième écran avec « Toujours au-dessus » (les fichiers y sont lâchés
   dans la fenêtre à part elle-même). La bulle de chaque icône vient de `bulles.rs`.

---------------------------------------------------------------------------------------------------

## 8. Transitions, effets sonores, bruitages (2026-10-19, branche `transitions-son`)

### Vocabulaire ajouté

| Idée | Mot | À éviter |
|---|---|---|
| La jointure de deux plans dans le montage | **coupe** | raccord, cut |
| Ce qui habille une coupe, ou le bord du film | **transition** | effet (réservé à la pile d'effets d'un calque) |
| Les sortes | **fondu enchaîné**, **fondu au noir**, **fondu au blanc**, **volet**, **noir** | dissolve, fade |
| Où la zone se pose par rapport à la coupe | **calage** : « Centrée sur la coupe », « Finit à la coupe », « Commence à la coupe » | alignement |
| Plan qui part / plan qui arrive | **plan sortant** / **plan entrant** | A / B |
| Le fondu des bords d'un extrait de son | **fondu d'entrée**, **fondu de sortie** | fade in / out |
| Le fondu entre deux extraits qui se touchent | **fondu enchaîné** du son (centré sur la jonction) | crossfade |
| Le son qui existe de l'autre côté d'un bord d'extrait | **poignée** (le mot des logiciels de montage) | marge |
| Les points de la courbe de volume d'un extrait | **clés de volume** (une clé est un début : voir le glossaire de la partie 2) | points, nœuds |
| Seules ces pistes s'entendent | **solo** | — |
| Le panneau des sons de l'utilisateur | **Bruitages** (bibliothèque, dossiers) | effets sonores (réservé à `audiofx`) |

### Ce qui a été posé

- **Montage** : un nœud papillon (fondu enchaîné), un losange (noir ou blanc), une rampe (bord du film), un carré
  divisé (volet) ou un bloc noir, au milieu des plans, sur la coupe. Ses bords sont des poignées de durée ;
  clic = choisir, Suppr = supprimer, clic droit = sorte / durée / calage ; clic droit sur une coupe =
  « Transition ▸ ». Bouton « Ajouter une transition » de la barre du montage (un seul appel, `transition_button`, à
  ranger sous « Plus… » si la barre l'exige).
- **Pistes son** : bouton S à côté de M ; sur l'extrait choisi, deux poignées carrées au coin du haut, une
  ligne de volume (double-clic pour une clé), une croix à la jonction de deux extraits qui se touchent.
  Clic droit sur un extrait > « Effets du son » (volume, fondus, fondu enchaîné, clés, égaliseur, écho).
- **Barre des panneaux** : une icône de plus (onde), groupe des commentaires et de l'historique.
- **Menus** : Son > Bruitages… ; menu d'un plan : « Transition au début du plan » / « … à la fin du plan ».
- **Commandes** (Ctrl+K, sans touche par défaut) : `transition.*` (8), `son.fondu_entree`, `son.fondu_sortie`,
  `son.enchaine`, `son.cle_volume`, `son.solo`, `son.effets_reinit`, `vue.bruitages`, `bruitage.inserer`,
  `bruitage.ecouter`. Suppr supprime la transition choisie, ou la clé de volume choisie, quand le dernier
  clic du montage l'a visée.
- **Historique** : chaque geste a un nom français (« Ajouter une transition : fondu enchaîné », « Durée de la
  transition », « Fondu d'entrée », « Clé de volume », « Piste en solo », « Insérer un bruitage »…).

### À vérifier à l'écran (rien n'a été lancé dans cette séance)

1. **Nœud de transition** : lisible dans les trois thèmes, en zoom arrière (largeur minimale 18 points), sur un plan
   coloré ; ses poignées de bord ne gênent pas celles du rognage (les bords de plan sont plus bas dans la pile des
   gestes : la transition passe d'abord).
2. **Clic droit sur une coupe** : à ± 4 points de la jonction des blocs ; le menu du plan reste ailleurs.
3. **Lecture avec transition** : fondu fluide à la cadence, en vue caméra avec deux caméras différentes ; le volet
   avance sur le cadre par défaut, pas sur celui d'une caméra qui bouge (à l'écran seulement).
4. **Aperçu à l'arrêt** : le dessin dans une zone de transition montre le mélange ; `transition.apercu` le coupe
   (à décider si le défaut doit être l'inverse en mode Animation : il est déjà coupé là).
5. **Poignées de fondu** aux coins d'un extrait : prise à 6 points, bande du haut de 12 points ; la poignée sur le
   coin gauche prend le pas sur le bord (taille de l'extrait) pour l'extrait choisi seulement.
6. **Ligne de volume** : double-clic sur un extrait pas encore choisi (le premier clic le choisit).
7. **Panneau Bruitages** : liste de plusieurs milliers de sons (défilement), glisser vers la bande et vers la
   timeline, repère de dépôt.
8. **Entrée de menu** « Bruitages… » dans le menu Son et l'icône de la barre : bulle et raccourci.

### Essais ajoutés

`transitions_tests` (27), `audiofx_tests` (28), `bruitages_tests` (17) ; les essais de la barre des panneaux et
des dispositions comptent un panneau de plus ; le projet d'essai de fiabilité contient transitions, effets
de sons et solo.

---------------------------------------------------------------------------------------------------

## 9. Suggestions sonores (2026-10-19, branche `generation-son`, phase 1)

### Vocabulaire ajouté

| Idée | Mot | À éviter |
|---|---|---|
| Un son que SHOT propose d'après le storyboard | **proposition** (panneau « Suggestions sonores ») | suggestion de son, cue (réservé aux signets du plan) |
| Les états | **proposée**, **en génération**, **prête**, **acceptée**, **rejetée** | brouillon, validée |
| Les sortes | **bruitage**, **ambiance**, **musique d'intention**, **voix** | SFX, musique du film |
| Poser dans le montage | **accepter** (« Accepter » copie le son dans le projet) | insérer, valider |
| La piste où elles se posent | **« Sons proposés »** (« Sons proposés 2 »…) | — |
| Qui fait le travail d'un rôle | **règles**, **local**, **service en ligne** | sidecar, API (mots du code et du contrat) |
| Lire le texte du plan pour en tirer des sons | **Lire le storyboard** | analyser, scanner |

### Ce qui a été posé

- **Panneau** « Suggestions sonores » : la portée (tout le film, séquence courante, plan courant, des plans choisis) et
  « Lire le storyboard » ; une ligne qui dit qui fait quoi (« Lecture : règles · Sons : local · Voix : local ») et, en
  couleur d'avertissement, ce qui partira en ligne ; l'avancement des tâches ; « Tout générer », « Tout accepter »,
  « Supprimer les rejetées » ; la proposition choisie et ses boutons (Générer / Régénérer, Écouter, Accepter, Rejeter,
  Reprendre, Modifier, Supprimer) ; la liste (icône de la sorte, plan et moment, titre, état), même menu au clic droit.
- **Fenêtres** : « Envoyer vers un service en ligne ? » (une fois par projet), « Modifier la proposition », « Clé de … ».
- **Préférences** : onglet « Génération » (depuis le § 30 ; avant, une section repliée) : les trois rôles ; le serveur
  local : dossier des modèles, Python, case « Démarrer le serveur tout seul… », port, « Démarrer le serveur local »,
  « Arrêter », « Tester » ; les clés, « Tester la connexion », une note de licences. Le contenu de l'onglet défile quand
  il dépasse la fenêtre.
- **Barre des panneaux** : une icône de plus (étincelle), même groupe que les bruitages ; menu Son > Suggestions
  sonores… ; commandes `vue.suggestions_sonores` et `suggestion.lire` (sans touche par défaut).
- **Historique** : « Accepter un son proposé ».

### À vérifier à l'écran (rien n'a été lancé dans cette séance)

1. **Le panneau dans sa colonne** : la hauteur de la liste quand les lignes d'avertissement en ligne et les barres
   d'avancement sont là (le panneau ne défile pas : l'en-tête est fixe, seule la liste défile) ; les lignes de 44 points
   dans les trois thèmes ; les états lisibles (couleurs d'accent, d'avertissement, atténuées).
2. **Les icônes** : étincelle (générer), flèches (régénérer), coches (tout accepter), croix, crayon, corbeille ; choix de
   l'icône de la barre (`SPARKLE`) face à « Effets » (baguette).
3. **Double-clic** sur une ligne : écoute si le son existe, sinon génère ; clic droit : menu complet.
4. **Fenêtres** : confirmation en ligne (texte long, bouton), « Modifier » (champs selon la sorte), saisie de la clé
   (champ masqué).
5. **Préférences** : l'onglet Génération avec les trois rôles en ligne (hauteur, défilement de l'onglet sur un
   écran de 1080 points) ; « Démarrer le serveur local » (le message d'attente, puis la phrase de « Tester ») ;
   « Arrêter » ; « Tester » quand le serveur ne tourne pas (« Le serveur local ne tourne pas… »).
6. **Piste « Sons proposés »** dans la bande de montage et dans la timeline : nom, couleur, extraits posés à la bonne image.
7. **Lire un film de plusieurs dizaines de plans** : l'interface reste fluide pendant les vignettes (8 ms par image
   d'écran) et la lecture.

### Essais ajoutés

`indications_tests` (13), `fournisseurs_tests` (42), `suggestions_tests` (26), `base64_tests` (2), plus les essais de la barre
des panneaux et des dispositions (un panneau de plus). Le projet d'essai de fiabilité contient des sons proposés
(un bruitage, une voix rejetée et modifiée, une ambiance acceptée) et la confirmation d'envoi en ligne.

---------------------------------------------------------------------------------------------------

## 10. Importer un scénario (2026-10-19, branche `import-scenario`)

Fichier > Importer > « Un scénario (Fountain, Final Draft, Word, texte)… », commande `fichier.importer_scenario`
(dans Ctrl+K, sans touche par défaut). Une fenêtre « Importer un scénario » montre, avant tout changement :
le format lu, les séquences et leurs plans, la durée estimée, les personnages, les remarques de lecture, et
les choix (découpage en plans, rythme, destination).

- **Vocabulaire** : un intitulé de scène (« INT. CUISINE – NUIT ») devient une **séquence** ; l'action et les
  répliques deviennent des **plans** (jamais « scène » ni « panneau » dans les textes visibles) ; les
  paragraphes d'un plan « par scène » en sont les **images de storyboard** (marques d'image).
- **Menu** : l'entrée est sous Importer, après « Un projet Storyboard Pro (.sboard)… » (qui ouvre aussi une
  fenêtre de lecture avant d'importer).
- **Annuler** : un import dans le projet ouvert est une seule étape, « Importer un scénario » (panneau
  Historique) ; un import dans un nouveau projet remplace le projet ouvert après la question habituelle
  « Modifications non enregistrées ».
- **À vérifier à l'écran** (rien lancé ici) : la fenêtre en thème clair et sombre (barre d'avancement d'un gros
  fichier, liste des séquences défilante, remarques en couleur d'avertissement), les plans créés dans le montage
  (titres, durées) et la fiche du plan (champs Action, Dialogue, Notes remplis ; notes « Décor : … »).

---------------------------------------------------------------------------------------------------

## 11. Crayonnés rapides (la planche, 2026-10-19, branche `crayonnes`)

Une page de petites cases où l'on enchaîne des croquis, puis chaque case devient un plan
(`crayonnes.rs`, `crayonnesui.rs`). Elle prend la place du dessin en storyboard ; elle se ferme par la croix
de sa barre ou la même commande. Vocabulaire : **case** (un croquis de la planche), **planche** (toutes les
cases), **page** (une grille de cases), **plan** pour ce qu'on en tire.

- **Ouvrir** : Storyboard > Planche de crayonnés…, le bouton de la barre du montage (à côté de Vue dessin /
  Vue vignettes), Ctrl+K > « Planche de crayonnés rapides » (`storyboard.crayonnes`). Ouvrir prend l'outil
  Crayon (sauf si la Gomme est l'outil).
- **Commandes** (palette Ctrl+K, clic droit sur une icône > assigner une touche, sans touche par défaut) :
  `crayonnes.case_suivante`, `case_precedente`, `page_suivante`, `page_precedente`, `ajouter_page`,
  `inserer_case`, `supprimer_case`, `vider_case`, `tout_choisir`, `creer_plans`, `case_auto`.
- **Touches qui ne passent pas par le fichier** (comme les flèches du dessin) : quand la planche a la main (le
  dernier appui y était), Tab / Maj+Tab et les flèches changent de case, Page préc. / suiv. de page ; Ctrl+Z /
  Ctrl+Y, Ctrl+A et Suppr y sont l'annulation de la planche, « choisir toutes les cases dessinées » et « vider
  les cases choisies » (Tab redevient « passer en animation » dès qu'une bande reprend la main).
- **Gestes** : appui dans une case = tracer (le trait appartient à cette case) ; Maj ou Ctrl + appui dans une
  case, ou clic sur son numéro = choisir ; glisser le numéro = changer l'ordre ; clic droit = menu de la case.
- **Annulation** : la planche a sa propre pile ; « Créer les plans » est une seule étape de l'historique du
  projet (nommée « Créer les plans des crayonnés »), que Ctrl+Z dans la planche défait aussi si elle est
  toujours la dernière.

À regarder à l'écran : les cases et leurs numéros dans les trois thèmes (la coche verte, l'anneau de la case
choisie et de la case courante), le trait qui reste collé à la pointe pendant le geste, le glisser d'un numéro
(barre d'insertion devant la case visée), la fenêtre « Créer les plans », la grille 6 × 4 sur un petit écran,
le pinceau et la gomme à la tablette (pression), la barre sur une fenêtre étroite (elle défile).

### Essais ajoutés

`crayonnes_tests` (38 : appartenance, coupe, navigation, ordre, création des plans, fichier, annulation, écran).
Le projet d'essai de fiabilité contient une planche de crayonnés (trois cases, dont une peinte au pinceau).

---------------------------------------------------------------------------------------------------

## 12. Calques d'effets et préréglages d'effets (2026-10-19, branche `calques-effets`)

Demande d'Anthony : « des calques d'effets après des préréglages d'animation d'effet comme des impact frames,
glows, tremblements de caméra effectifs, timecode ». Détail : README (section du même nom) et `docs/ROADMAP.md`.

### Mots

| Idée | Mot | À éviter |
|---|---|---|
| Calque qui agit sur ce qui est dessous | **calque d'effet** (sorte « ajustement ») | calque de réglage, filtre |
| Calque qui écrit du texte sur l'image finale | **incrustation** (timecode, numéro d'image…) | overlay, sous-titre |
| Effets tout réglés avec leurs clés | **préréglage d'effet** (menu **Préréglages**, bouton étincelle) | preset, modèle |
| Durée d'un calque d'effet | son **exposition** (comme un dessin tenu) | durée en secondes cachée |
| Secouer l'image déjà dessinée | **tremblement de l'image** (≠ « Effets de caméra », qui bougent la caméra) | tremblement de caméra effectif |

### Où et quels gestes

- **Ajouter** : bouton étincelle du panneau Calques ; Calques > Nouveau calque > Calque d'effet (et clic droit d'un
  calque) ; « Préréglages » du panneau Effets ; Ctrl+K (`effet.*`, sans touche par défaut, « Assigner un
  raccourci… » au clic droit d'une entrée de menu).
- **Fenêtre du préréglage** (à droite de l'écran, ne cache pas le dessin) : l'effet se voit en direct sur le dessin
  pendant les réglages ; Appliquer (une étape d'annulation « Préréglage d'effet : … »), Annuler ou Échap (rien n'est posé).
- **Distinguer** : un calque d'effet a une étincelle à la place de la lettre B / V, la teinte cyan (`theme::dessin_cfx`,
  `calque_cfx`) dans le panneau Calques, la timeline et le montage, et le mot « effet » sous son nom.
- **Régler après coup** : choisir le calque, panneau Effets (pile, clés en losanges violets, comme tout calque) ;
  une incrustation a son éditeur en tête du panneau. Étirer / couper / déplacer l'exposition dans la timeline change
  les images où l'effet agit.

### À vérifier

- Photoshop et After Effects : le calque de réglage agit sur tout ce qui est dessous, et dans un groupe seulement
  sur ce groupe ; SHOT l'a repris tel quel (**à vérifier** que les dossiers de SHOT se comportent comme les
  groupes de Photoshop — « passer à travers » n'existe pas ici).
- Une incrustation suit le cadre de la caméra (texte à l'écran de l'export) : **à vérifier** dans Storyboard Pro
  (calque d'incrustation / « burn-in » du timecode, nom du plan en surimpression).
- Les valeurs d'ampleur des préréglages (impact, lueur, flash, secousse, vignettage, grain, aberration) sont des choix
  de SHOT, à régler à l'œil.
- Pas vu à l'écran dans cette séance (aucun lancement de SHOT, consigne) : fenêtre du préréglage (largeur, placement),
  icône et cyan dans les trois thèmes, boîte de l'incrustation sur le dessin, aperçu en direct avec un gros dessin.

---------------------------------------------------------------------------------------------------

## 13. Espace 3D (2026-10-20, branche `espace-3d` ; avant : « Scène 3D de blocage » de la branche `scene3d` et « Plateau en 3D » de la phase 9)

**Un seul espace** : l'icône cube de la barre sous le dessin (`espace3d.basculer`, aussi Affichage > Plateau et Ctrl+K) ouvre
une vue 3D à la place du dessin, avec ses réglages dans le panneau **Réglages** (qui montre l'Espace 3D tant qu'il est
ouvert). Le « Plateau en 3D » et la « Scène 3D » d'avant n'existent plus comme vues : leurs identifiants de commande
(`vue.3d`, `scene3d.basculer`, `vue3d.recadrer`, `outil3d.*`) sont des doubles (`keymap::ALIASES`), un raccourci posé sur
l'un d'eux marche toujours. Conception, formules, format : `docs/ESPACE3D.md`. Tout est en icônes avec bulle (nom,
description, raccourci).

- **Ce qu'on y voit** : les **cartes** (les calques visibles du plan, texturés par leur dessin de l'image courante, dans
  l'ordre de la pile, voile clair et contour de leur feuille, nom et profondeur), les **objets** (cube, plan, cylindre,
  sphère, cône, escalier, mannequin, modèle glTF ; le sol quadrillé), la **caméra du plan** (pyramide de vision, cadre, haut
  de l'image, sa course si elle a des clés). Les cartes se dessinent par-dessus les objets : c'est la règle de l'image
  (les objets sont sous tous les calques).
- **Commandes** (`scene3d.*` et `espace3d.*`, aucune touche par défaut, toutes dans Ctrl+K) : afficher / quitter, outils
  (choisir, déplacer, tourner, échelle), ajouter (cube, plan, cylindre, sphère, cône, escalier, mannequin), importer un
  modèle, dupliquer, supprimer (Suppr y suffit), grouper, dégrouper, vue par la caméra, cadrer, figer les objets dans un
  calque de fond, reprendre l'ancienne caméra de blocage, aimantation, grille, décor / plan, **ordonner la pile selon la
  profondeur**.
- **Souris dans la vue** : clic = choisir une carte, un objet ou la caméra (Maj ou Ctrl : en ajouter) ; un clic sur une
  carte en fait aussi le calque courant (et le calque choisi dans le panneau Calques choisit sa carte) ; poignées du
  manipulateur pour déplacer, tourner, mettre à l'échelle ce qui est choisi ; glisser un objet le pose sur le sol (Maj :
  en hauteur), glisser une carte la déplace dans son plan (Maj : en profondeur) ; clic droit ou Alt + glisser = tourner
  la vue ; bouton du milieu = la glisser ; molette = avancer. En « Vue par la caméra », ces gestes déplacent la caméra du
  plan (sa clé de l'image courante si elle est animée).
- **Réglages** : plan ou décor ; « Objets dans l'image » ; formes à ajouter ; outil, repère local, aimantation (pas de
  grille, de rotation, de taille) ; la liste des **cartes** (le dessus de la pile d'abord ; un verrou si le calque est
  verrouillé), « Montrer les cartes », « Ordonner la pile selon la profondeur » ; les réglages de la carte choisie :
  position (m), rotation (°) — Z est le tour dans le plan, X et Y inclinent —, échelle (%), étirement X / Y (%),
  profondeur (comme le panneau Calques), « Remettre à plat » ; la liste des objets ; les réglages d'un objet ; la
  **caméra du plan** (position en m, panoramique, bascule, rotation, focale et objectifs, « Reprendre l'ancienne caméra de
  blocage », « Viser la sélection ») ; lumière et rendu.
- **Annulation** : une étape par geste, au nom français : « Ajouter : cube », « Déplacer dans l'espace 3D », « Tourner
  dans l'espace 3D », « Mettre à l'échelle dans l'espace 3D », « Déplacer la caméra », « Placer la carte dans l'espace 3D »,
  « Ordonner la pile selon la profondeur », « Objets 3D dans l'image », « Figer les objets 3D dans un calque de fond »,
  « Supprimer des objets 3D », « Dupliquer… », « Grouper… ».
- **Aucun repaint au repos** : la vue ne demande une image que si on la touche ou si un rendu (calque figé, objets de
  l'image) est en cours. Le dessin n'est jamais ralenti : ces rendus partent sur un autre fil (la moitié des cœurs), et
  leur résultat n'est posé ni pendant un trait, ni pendant un geste dans la vue 3D, ni en lecture.
- **Limites visibles** : une carte inclinée est coupée aux bords de la feuille ; une carte ne se recouvre pas par
  profondeur mais par l'ordre de la pile ; un objet est toujours derrière les cartes ; la vue d'édition montre le dessin
  de la carte sans ses effets ; une carte ne s'incline pas au-delà de 85°.
- **Vérifié par le programme** (pas à l'écran) : voir `docs/ESPACE3D.md`, « Essais ». **À vérifier à l'écran (par
  Anthony)** : la liste de contrôle ci-dessous.

### Liste de contrôle visuelle pour Anthony

1. **Un seul bouton 3D** (cube) dans la barre sous le dessin ; plus de bouton « Plateau en 3D » ni « Scène 3D » ; dans
   Affichage > Plateau une seule entrée « Espace 3D » ; Ctrl+K « Espace 3D » ne se trouve qu'une fois.
2. **Les calques sont là** : ouvrir un plan à plusieurs calques à des profondeurs différentes (plan d'un ancien projet) ;
   les cartes apparaissent à leur profondeur, dans l'ordre de la pile, avec leur dessin, leur nom, leur profondeur ; la
   pyramide de la caméra est dessinée.
3. **Choisir une carte** : un clic dessus la choisit (contour orange, poignées), son calque devient le calque courant ;
   choisir un autre calque dans le panneau Calques change la carte choisie.
4. **Déplacer** : les flèches rouge / verte / bleue déplacent la carte en X / Y / Z ; en la reculant elle rapetisse ;
   le champ « Profondeur » du panneau Calques la recule, lui, en gardant sa taille vue de la caméra.
5. **Tourner / incliner** : les cercles inclinent la carte ; le cercle bleu la tourne dans son plan ; revenir en vue
   normale : la carte inclinée est bien déformée en perspective, et on peut dessiner dessus (le trait tombe sous le
   pointeur).
6. **Échelle** : cube du centre = uniforme ; cubes d'axe = étirement ; le chiffre « Échelle » du panneau suit.
7. **Chiffres** : position, rotation, échelle, étirement, profondeur dans Réglages ; « Remettre à plat ».
8. **Parallaxe** : placer trois cartes à des profondeurs différentes, bouger la caméra (poignées de la pyramide, ou
   Vue par la caméra puis bouton du milieu) : revenir en vue normale, jouer le plan : le fond bouge moins que le
   premier plan. Un cube dans l'espace : il est dans l'image, sous les cartes, et bouge aussi avec la parallaxe.
9. **Clés** : sur une carte animée (clés de position), la tirer à une autre image règle sa clé (losange vert) ;
   lire le plan : elle s'incline et recule en douceur.
10. **Caméra animée** : avec des clés de caméra, tirer la pyramide à une autre image règle la clé de cette image.
11. **Annuler** : Ctrl+Z après chaque geste rend l'état d'avant, en une étape au nom lisible dans l'historique.
12. **Un ancien projet** s'ouvre identique (même image qu'avant, objets 3D d'avant hors de l'image, calque « Fond 3D »
    intact) ; l'enregistrer le passe au format 35 (un SHOT d'avant le refuse — c'est voulu).
13. **Fluidité** : 10 cartes et 20 objets, tourner la vue et tirer une carte restent fluides ; un trait de pinceau ne
    ralentit pas.
14. **Thèmes** : poignées, contours de cartes et pyramide lisibles dans les trois thèmes (l'espace a son fond sombre
    propre, comme la vue de dessus).
## 14. Espaces de travail liés aux modes, barres minces et « Plus… » (2026-10-19, branche `espaces-barres`)

Les propositions 6.1 et 6.3, approuvées par Anthony. Rien n'a été regardé à l'écran dans cette séance (aucun
lancement de SHOT) : voir « À vérifier » plus bas.

### Espaces de travail (6.1)

- **Trois espaces, chacun sa disposition** : Storyboard, Animation, Montage (`workspace::Espace`). Changer d'espace
  (les trois onglets en haut à droite, Tab, « Animer ce plan ») pose la disposition de l'espace d'arrivée ;
  ce qu'on change dans un espace se retient pour lui seul (réglages `disposition`, `disposition_animation`,
  `disposition_montage`, `espace_storyboard`, toutes avec une valeur d'origine : un fichier ancien se lit tel quel).
- **Montage est une disposition du mode Storyboard, pas un troisième mode** (choix expliqué) : mêmes outils,
  mêmes raccourcis (les touches du montage), même dessin, et le bouton « Animer ce plan » entre en animation depuis lui.
  Un troisième mode aurait doublé les contextes de raccourcis, les menus qui changent selon le mode et les essais qui
  les suivent, pour une différence qui n'est qu'une disposition. On y va par son onglet (depuis le 2026-10-21, voir
  « Le troisième onglet Montage » plus bas), Alt+M, Affichage > Espaces de travail ou Ctrl+K ; l'onglet Storyboard
  (ou Tab) en revient. Quitter l'animation ramène à celui des deux espaces du côté storyboard où l'on était
  (`espace_storyboard`).
- **Dispositions d'origine** : Storyboard = la disposition standard ; Animation = la bande (la timeline) plus haute
  (340 au lieu de 290) et les calques plus larges (250 au lieu de 230) ; Montage = voir « Le troisième onglet
  Montage » (le film en haut, la bande à 440 avec toutes les pistes, la fiche du plan et les panneaux de montage à
  droite). Références et feuille d'exposition restent à la demande ; la feuille d'exposition n'existe qu'en
  animation.
- **Migration** : l'unique disposition d'un ancien `reglages.toml` devient celle du Storyboard sans conversion
  (le champ garde son nom) ; les deux autres commencent par leur disposition d'origine.
- **« Rétablir la disposition »** (Affichage > Espaces de travail, Ctrl+K `espace.retablir`) : l'espace en cours
  reprend sa disposition d'origine, les bords tirés à la main sont oubliés (`panneaux_tires_a_la_main`), les autres espaces
  ne bougent pas. Les quatre dispositions d'avant (Standard, Dessin, Timeline, Vue d'ensemble) et les dispositions
  enregistrées s'appliquent à l'espace en cours.
- **Ce qui ne bouge pas en changeant d'espace** : les cadres de composition, le format et la planche des vignettes
  (réglages de la vue) ; les panneaux dans leur propre fenêtre (posés sur un autre écran) — sauf la feuille
  d'exposition, qui suit l'animation ; l'état des panneaux qui s'ajustent à leur contenu (`ajuste.rs`) repart de la
  disposition posée, comme à chaque nouvelle disposition. Le changement est instantané (aucun calcul d'image, aucun
  rejeu) ; les réglages s'écrivent à l'image d'écran suivante.
- **Correction au passage** : la colonne de droite ne savait recevoir que les calques, l'exposition et les effets ;
  sans calques (c'est le cas de l'espace Montage), Historique, Commentaires, Bruitages, Suggestions sonores et
  Références ne s'ouvraient pas. Ils forment maintenant leurs onglets (`dock.rs`, `right_column`).
- **Actions** (`keymap.toml`, un bloc chacune, sans touche sauf `espace.montage` : Alt+M) : `espace.storyboard`,
  `espace.animation`, `espace.montage`, `espace.retablir`.

### Barres minces (6.3) et « Plus… »

Tout est dans `src/barres.rs` : une table de commandes par barre, dans l'ordre d'affichage, avec le groupe (un trait
entre deux groupes), si la commande est dans la barre d'office, son icône, son nom, sa bulle et son action.

| Barre | Dans la barre | Sous « Plus… » |
|---|---|---|
| Commandes du haut (19 → 9) | Nouveau, Ouvrir, Enregistrer, Exporter ; Annuler, Rétablir ; Couper, Copier, Coller | Nouveau plan, Dupliquer le plan, Supprimer les plans ; le transport (première image, précédente, lecture, suivante, dernière) ; boucle ; son pendant la lecture |
| Montage (21 → 12) | début, lecture, boucle, position ; nouveau plan, nouvelle séquence, menu Plans ; importer un son, menu Son ; animer ce plan ; vue dessin, vue vignettes, planche de crayonnés | calque vectoriel, copier / coller / supprimer les plans, avancer / reculer le plan, verrou, aimant, **ajouter une transition**, importer une vidéo, nouveau commentaire |
| Timeline (ne défile plus) | animer / retour, titre et durée du plan ; transport, boucle, image courante, menus Repères et Affichage ; nouveau dessin, allonger / raccourcir l'exposition, « en N », menu Images ; clés de caméra début / ici / fin ; pelure (bouton, avant, après, réglages) | cadence, dupliquer / supprimer / vider le dessin, supprimer la clé de caméra, adoucir, marque d'image, nouveau commentaire, flips, effets |

- **Le bouton « Plus… »** (`DOTS_THREE`) est au bout de chaque barre ; une ligne par commande avec son icône, son
  nom et son raccourci (clic : elle s'exécute ; clic droit : « Assigner un raccourci » ; les états — boucle, verrou,
  aimant, pelure… — sont enfoncés avec l'encre `theme::sur_accent`, `bulles::symbole_etat`). Les menus (Plans, Son,
  Repères…) y sont des sous-menus, les réglages (cadence…) des champs.
- **Épingler** : à gauche de chaque ligne, une épingle (« Épingler dans la barre ») remet la commande dans la barre ;
  le sous-menu « Dans la barre » liste les commandes montrées d'office pour les retirer, « Rétablir la barre d'origine »
  efface les choix. Le choix est retenu **par barre** (`reglages.toml`, `[barres.commandes]`, `[barres.montage]`,
  `[barres.timeline]`, listes `epinglees` et `retirees` : seul l'écart avec la barre d'origine est écrit ; un fichier
  abîmé est remis d'équerre à la lecture).
- **Jamais de défilement** : les barres du montage et de la timeline n'ont plus de zone de défilement. Quand la place
  manque, la commande qui ne tient plus et les suivantes passent d'elles-mêmes sous « Plus… » (elles restent
  épinglées et reviennent quand la fenêtre s'élargit). Les affichages fixes (titre du plan, image courante, position)
  ne passent jamais dessous.
- **Rien n'est retiré** : chaque commande a son action dans `keymap::ACTIONS` (palette Ctrl+K, raccourci, assignation) ;
  les menus du haut sont inchangés (sauf le menu Plans du montage, dont l'icône « trois points » est devenue une liste
  à puces, pour ne pas ressembler à « Plus… »).
- **Pas touchées** : la barre des panneaux à droite ; la barre sous le dessin (vue, caméra,
  **Espace 3D** — le cube, un seul bouton depuis l'Espace 3D) et celle au-dessus (vue du plateau / caméra, zones de sécurité, format ; seuls les outils
  de l'Espace 3D y viennent quand il est ouvert) : ce sont des vues, pas des commandes de ces trois barres ; l'Espace 3D
  reste aussi dans Affichage > Plateau et Ctrl+K. Proposition 4 de la partie 5 (miroir et table lumineuse sous
  « Plus… » dans la barre sous le dessin) : pas faite, elle n'était pas dans 6.3.
- Le « Réglage Barres complètes » de 6.3 est remplacé par les épingles : épingler tout remet la barre complète.

### Essais ajoutés

`barres_tests.rs` (barres d'origine à 9 / 12 icônes et ce qui est sous « Plus… » ; pas de doublon, chaque commande
d'un seul côté ; chaque action retirée d'une barre existe et est implémentée ; épingles, aller-retour dans les
réglages, fichier abîmé nettoyé, pas de trace quand on revient à l'origine ; la barre dessinée dans 520 points ne
dépasse pas ; les commandes font ce que faisaient leurs boutons) et `workspace_tests.rs` (dispositions d'origine,
changer de mode restitue la disposition de l'espace, écriture dans les réglages, migration d'un ancien fichier,
aller-retour de réglages, Montage et retour par le bouton, rétablir un seul espace, feuille d'exposition,
panneau détaché, chaque panneau de la barre s'ouvre dans chaque espace, changement instantané).

### À vérifier à l'écran

1. Le menu « Plus… » : largeur des lignes, alignement de l'épingle et du raccourci, sous-menus (Plans, Son,
   Repères, Affichage, Images) ouverts depuis une ligne qui porte une épingle, clic sur l'épingle (le menu reste ouvert).
2. Une barre de la timeline dans un panneau étroit : où passe la coupure, le bouton « Plus… » toujours visible à droite.
3. Les trois thèmes : icônes enfoncées lisibles dans la barre et dans « Plus… ».
4. Les trois dispositions d'origine sur ton écran (la bande à 340 en animation, à 440 en Montage) ; ajuste si elles
   mangent trop le dessin.
5. Passer de Storyboard à Animation et retour : le dessin ne saute pas, les panneaux ajustés se recalent en une ou
   deux images ; l'espace Montage depuis son onglet, Alt+M ou Ctrl+K ; l'onglet « Storyboard » (ou Tab) qui en revient.
6. Un panneau détaché (Références) reste sur son écran en changeant d'espace.

### Le troisième onglet Montage (2026-10-21, branche `espace-montage`)

Demande d'Anthony : « en haut à droite, à côté de Storyboard et Animation, un troisième onglet Montage, avec une
organisation des panneaux propre au travail de montage ».

- **Les trois onglets**, en haut à droite, dans l'ordre du travail : **Storyboard · Animation · Montage** (même style,
  l'onglet de l'espace en cours est allumé avec l'encre faite pour l'accent, `bulles::encre_etat`). Un clic, la bulle
  à 1 s (nom, raccourci, description) et le clic droit « Assigner un raccourci » comme pour toute commande. Passer de
  l'un à l'autre est instantané et ne change rien au projet : le plan courant, l'image, la tête de lecture, les plans
  choisis et les calques restent ; seule la disposition (et la barre du montage) change. La lecture s'arrête.
- **Choix : une disposition du mode Storyboard, pas un troisième mode** (`Espace::Montage.mode() == Mode::Storyboard`).
  Le montage garde les outils, les raccourcis et le dessin du storyboard : c'est le même travail sur les plans, vu
  autrement. Un troisième `Mode` aurait doublé les contextes de raccourcis, les menus qui dépendent du mode et leurs
  essais, pour une différence qui est une disposition et une barre. Ce qui s'adapte à l'espace : la disposition
  (`DockLayout::pour_le_montage`), la barre de la bande (`Barre::MontageEdition`), le titre « Lecteur » du dessin, la
  barre d'état (« Montage · plan 3 »), la vue caméra du lecteur.
- **Raccourcis** : **Alt+M** (`espace.montage`, libre : « M » seul reste le miroir ; Alt+lettre est le même geste que
  les outils) ; Ctrl+K « Espace de travail : Montage ». **Tab** : du Storyboard à l'Animation et retour, comme avant ;
  **depuis le Montage, Tab revient au Storyboard** (le Montage n'est pas un côté de la bascule : « Animer ce plan », dans
  sa barre, ou le double-clic sur un plan, entre en animation, et Tab ramène alors au Montage, d'où l'on venait).
- **La disposition d'origine du Montage** (pensée pour monter, pas pour dessiner) :
  - en haut à gauche, le **Lecteur** (le dessin) en grand, en **vue caméra** : on voit le film tel que la caméra le filme
    (à l'entrée, la vue caméra s'allume si elle ne l'était pas ; à la sortie elle s'éteint si c'est l'entrée qui l'avait
    allumée) ;
  - en bas, sur toute la largeur, la **bande du montage** avec toutes ses pistes (plans, pistes de calques, pistes de son,
    transitions), à 440 au lieu de 290 (storyboard) et 340 (animation) ;
  - à droite, la **Fiche du plan** (durée, textes) en haut, et dessous, en onglets : **Bruitages**, **Suggestions sonores**,
    **Commentaires**, **Historique** ;
  - ni Outils, ni Couleur, ni Pinceaux, ni Réglages de l'outil, ni Guides, ni Calques, ni Effets, et la **barre d'outils
    verticale est repliée** ; tous restent à la demande (barre des panneaux, Affichage > Espaces de travail > Panneaux).
  - Les panneaux « Son » et « Transitions » n'existent pas comme panneaux : les pistes son (volumes, fondus, clés,
    solo), leurs effets et les transitions se règlent dans la bande, par la barre et par les menus du clic droit.
    La relecture est faite de fenêtres (exporter / importer les notes), lancées depuis la barre.
- **Mémoire** : comme les autres espaces, la disposition qu'on y fait se retient (`disposition_montage` dans
  `reglages.toml`), se restitue à l'entrée et à l'ouverture suivante (on démarre dans le Montage si on l'a quitté
  ainsi), s'enregistre comme espace de travail nommé (Affichage > Espaces de travail > Enregistrer la disposition…) et
  « Rétablir la disposition » (Ctrl+K `espace.retablir`) la remet à son origine sans toucher aux deux autres.
  L'ancienne disposition d'origine du Montage, restée intacte dans les réglages, est remplacée par la nouvelle à la
  lecture ; une disposition retouchée à la main reste la sienne.
- **La barre du Montage** (`Barre::MontageEdition`, `barres.rs`) montre d'office les commandes d'édition, qui sont sous
  « Plus… » dans la barre du montage du storyboard :
  1. lire : début, lecture, boucle, son pendant la lecture, position ;
  2. plans : nouveau plan, nouvelle séquence ;
  3. couper et rogner : couper le plan, réunir avec le suivant, rogner le début, rogner la fin, durée du plan, avancer,
     reculer, aimantation des bords, menu Plans ;
  4. transitions et son : ajouter une transition, importer un son, nouvelle piste son, couper le son, menu Son,
     panneau Bruitages (interrupteur) ;
  5. pistes : replier toutes, déplier toutes ;
  6. relecture et exports : nouveau commentaire, exporter pour relecture, importer les notes, exporter pour le montage,
     exporter la fiche technique ;
  7. « Animer ce plan ».
  Sous « Plus… » : toutes les autres commandes de la barre du montage (verrous, numérotation, envois, versions,
  comparer, vidéo, vues dessin / vignettes / crayonnés, copier / coller / supprimer les plans, replis des pistes) ; la place
  qui manque les y envoie d'elle-même, comme pour toute barre. Elle a **ses propres épingles** (`[barres.montage_edition]`
  dans `reglages.toml`) : celles de la barre du storyboard n'y changent rien.
- **Limites** : pas de panneaux Son ou Transitions à part ; le lecteur est le dessin (il lit le film entier avec la
  lecture du montage, mais il garde les outils de dessin à portée d'Alt+lettre) ; la hauteur de la bande est modeste
  dans une petite fenêtre (1280 × 720 : le lecteur garde environ la moitié de la hauteur, tire le bord au besoin).

#### Essais ajoutés (`montageonglet_tests.rs`)

Les trois onglets se dessinent dans l'ordre et passent d'un espace à l'autre au clic ; rien ne change dans le projet
en changeant (plan, image, plans choisis, historique) ; seul l'onglet de l'espace est allumé ; Alt+M (aucun conflit) et
Tab depuis le Montage au clavier ; « Animer ce plan » puis Tab ramènent au Montage ; contenu de la disposition du Montage ;
vue caméra à l'entrée et à la sortie ; chaque panneau de la barre des panneaux s'ouvre dans le Montage ; disposition
retenue, restituée, rétablie, enregistrée comme espace, démarrage dans le Montage ; l'ancienne disposition cède la place
et une disposition retouchée reste ; contenu de la barre du Montage et des commandes qui font leur travail ; épingles
propres à la barre ; changement instantané ; repos (aucun réveil à moins d'une seconde) et image du Montage rapide.

#### À vérifier à l'écran (par Anthony, troisième onglet)

1. Les trois onglets en haut à droite : Storyboard, Animation, Montage, lisibles dans les trois thèmes, le bon allumé ;
   les bulles à 1 s (Montage : Alt+M).
2. Un clic sur Montage : le dessin devient « Lecteur », en vue caméra, grand ; la bande en bas, haute, avec toutes les
   pistes ; la Fiche du plan et les onglets Bruitages, Suggestions, Commentaires, Historique à droite ; plus de barre
   d'outils à gauche.
3. Le plan courant, l'image, la tête de lecture et les plans choisis sont les mêmes qu'avant le clic ; pareil dans
   l'autre sens (Storyboard, Animation).
4. La barre en tête de la bande : couper, rogner, durée, avancer/reculer, aimant, transition, sons, bruitages, pistes,
   relecture, exports se voient sans « Plus… » (sur une grande fenêtre ; sur une petite, le bouton « Plus… » prend la suite).
5. Tab depuis le Montage : retour au Storyboard ; « Animer ce plan », puis Tab : retour au Montage.
6. Ferme l'Historique dans le Montage, passe en Storyboard puis reviens : il est toujours fermé. Affichage > Espaces de
   travail > Rétablir la disposition : il revient. Ferme SHOT dans le Montage : il s'ouvre dans le Montage.
7. Au repos, dans le Montage : rien ne bouge, le processeur dort (comme ailleurs).

## 15. Envois, plans modifiés, versions de plans et comparaison (2026-10-20, branche `versions`)

Demande d'Anthony : savoir ce qui a changé entre deux livraisons du storyboard. Rien n'a été regardé à l'écran dans
cette séance (aucun lancement de SHOT) : voir « À vérifier à l'écran » plus bas. Détail du fonctionnement : `README.md`.

### Mots

| Mot | Sens |
|---|---|
| **Envoi** | l'état de tous les plans à un moment, avec un nom (« v2 au réal ») et une date |
| **Marquer un envoi** | la commande qui le crée (aussi « Noter » dans le texte de la fenêtre) |
| **Modifié / ajouté depuis l'envoi** | un plan qui n'est plus comme au dernier envoi / qui n'y était pas ; la pastille ambre |
| **Version** (V1, V2…) | une autre forme du même plan, gardée à côté ; « la version active » (ou affichée) est celle du film |
| **Nouvelle version du plan** | garde le plan tel qu'il est et continue sur une copie (« en dupliquant »), ou sur un calque vide (« en partant d'un plan vide ») |
| **Pile des versions** | les cartes qui s'ouvrent au-dessus du bloc du plan, dans le montage, quand on survole « V2 » |
| **Passer à une version** | elle devient l'active, l'ancienne active est gardée |
| **Restaurer une version** | une copie d'elle devient une nouvelle version active |
| **Comparer** | la fenêtre à deux côtés, en lecture seule |

### Où et quels gestes

| Commande | Où | Raccourci |
|---|---|---|
| Marquer un envoi… (`envoi.marquer`) | barre du montage > Plus… ; Storyboard > Envois ; Ctrl+K | aucun |
| Envois (`envoi.liste`, la fenêtre) | barre du montage > Plus… (icône à l'état) ; Storyboard > Envois ; Ctrl+K | aucun |
| Plan modifié suivant / précédent (`envoi.modifie_suivant`, `envoi.modifie_precedent`) | barre du montage > Plus… ; Storyboard > Envois ; fenêtre « Envois » ; Ctrl+K | aucun |
| Choisir les plans modifiés (`envoi.modifies_choisir`) | Storyboard > Envois ; fenêtre « Envois » ; Ctrl+K | aucun |
| Nouvelle version, en dupliquant (`version.nouvelle`) | barre du montage > Plus… ; Storyboard > Versions du plan ; fiche du plan (icône fourche) ; clic droit sur un plan > Nouvelle version > En dupliquant la version en cours ; Ctrl+K | aucun |
| Nouvelle version, en partant d'un plan vide (`version.vide`) | Storyboard > Versions du plan ; fiche du plan (icône page +) ; clic droit sur un plan > Nouvelle version > En partant d'un plan vide ; Ctrl+K | aucun |
| Supprimer les versions non affichées (`version.purger`, `version.purger_plan`) | Storyboard > Versions du plan (deux entrées, grisées s'il n'y a rien à supprimer) ; Ctrl+K | aucun |
| Version précédente / suivante (`version.precedente`, `version.suivante`) | Storyboard > Versions du plan ; Ctrl+K | aucun |
| Passer à, Restaurer, Supprimer une version | **la pile des versions** (survol de « V2 » sur le bloc du plan : clic sur une carte pour passer, croix pour supprimer) ; fiche du plan (ligne « Version ») ; clic droit sur un plan > Version du plan | aucun |
| Comparer le plan (`comparer.ouvrir`) | barre du montage > Plus… (icône à l'état) ; Storyboard > Versions du plan ; clic droit > Version du plan ; fenêtre « Envois » (une icône par envoi) ; fiche (une icône par version) ; Ctrl+K | aucun |

Tout se fait par la table des barres (`barres.rs`) et les actions (`keymap.rs`, un bloc chacune dans `keymap.toml`, sans
touche) : clic droit sur l'entrée d'un menu ou d'une ligne de « Plus… » pour leur en donner une. Les icônes ont leur
bulle d'une seconde (nom, raccourci, description, `bulles.rs`). La pastille est peinte avec la couleur d'avertissement du
thème (`theme::avertissement`) ; les états allumés (Envois, Comparer) prennent `bulles::symbole_etat`.

### Ce que voit l'utilisateur

- **Fenêtre « Marquer un envoi »** : un nom (« v2 au réal », le numéro suivant est proposé), une date (JJ/MM/AAAA, celle du
  jour ; rouge si elle n'est pas lisible), « Marquer l'envoi » / Annuler, Entrée valide. Un projet pas encore enregistré le
  dit (l'envoi est gardé au premier enregistrement).
- **Pastille** : 14 points en haut à droite du bloc du plan dans la bande et dans le coin de la première vignette du plan dans la
  vue Vignettes ; un crayon pour un plan modifié, un plus pour un plan ajouté ; bulle « Modifié depuis l'envoi « v2 au réal » ».
  Sur un bloc de moins de 44 points de large (la bande zoomée très loin : la pastille cacherait la poignée du bord) ou de
  moins de 40 de haut, une **bande ambre de 5 points le long du bord haut** la remplace, avec la même bulle. Le « dernier
  envoi » est le dernier marqué (pas le plus tardif par sa date : la date se tape).
- **Fiche du plan, ligne « Version »** : « V2 » en gras (et « sur 3 » s'il y en a plusieurs), les icônes « Nouvelle version » (en dupliquant, en partant d'un plan vide) ; une
  ligne par autre version : son numéro et sa date, puis passer (flèches), comparer (colonnes), restaurer (flèche arrière),
  supprimer (corbeille).
- **Titre** : « Plan 3 · V2 » dans la bande, sur la planche, dans la fiche, la timeline, la barre d'état et le titre de la
  fenêtre « Comparer » pour un plan qui a plusieurs versions.
- **Fenêtre « Envois »** : la ligne de commandes (marquer, plan modifié précédent / suivant, choisir les modifiés), le
  résumé (« Depuis « v2 au réal » : 7 plans modifiés, 1 ajouté »), la liste des plans modifiés (un clic y va), puis les
  envois du plus récent au plus ancien avec le nombre de plans qui diffèrent d'aujourd'hui, l'icône de comparaison et la
  corbeille (« Retirer de la liste », avec confirmation ; les fichiers de `versions/` restent).
- **Fenêtre « Comparer · Plan 3 · V2 »** : deux listes (les côtés possibles : Aujourd'hui, Version VN, Envoi « nom » (date)),
  l'icône qui échange, l'icône qui relit ; quatre icônes de mode (côte à côte, volet, fondu, superposition) ; la glissière de
  l'image du plan ; pour le volet et le fondu, la glissière du mélange (le volet se tire aussi à la souris dans l'image) ; en
  bas la part de l'image qui diffère et « Lecture seule : cette vue ne change pas le plan. »

### Les versions dans le montage (2026-10-20, branche `versions-montage`)

Demande d'Anthony : itérer un plan en plusieurs versions depuis le montage, sans ouvrir la fiche. Code : `pilevers.rs`
(logique sans interface), `pileversui.rs` (dessin et gestes), `planvers.rs` / `planversui.rs` (modèle et commandes).
**Aucun changement de format** (`VERSION` reste 30) : tout repose sur `plans_versions.json` d'avant.

**Nomenclature** : « **V1, V2, V3…** » en majuscule partout (étiquette de la bande, cartes, fiche, planche, timeline, barre
d'état, fenêtre « Comparer », messages, étapes d'annulation : « Passer à la version V1 », « Supprimer la version V2 »,
« Nouvelle version du plan, vide »). Les numéros ne se réutilisent pas : le prochain est toujours au-dessus de tous. Les
**envois** gardent leur nom libre (« v2 au réal » est la livraison du film, pas la version d'un plan).

**Étiquette « V2 » sur le bloc du plan** : seulement quand le plan a **au moins deux versions** (un plan à une seule reste
propre). Une pastille arrondie (fond de fenêtre, texte fort, trait de bordure : lisible dans les trois thèmes) près du haut,
au milieu du bloc, 8 points sous le bord ; allumée à l'accent quand sa pile est ouverte. Elle laisse sa place à la
pastille ambre « modifié depuis l'envoi » (en haut à droite) : si le milieu est pris, elle se décale à gauche ; sur un bloc
étroit elle devient un **point** (qui s'ouvre de même) ; sur un bloc trop bas ou trop étroit pour le point, elle disparaît (le
clic droit reste). Toujours à 10 points au moins des poignées de rognage.

**La pile** : survoler l'étiquette ouvre, **au-dessus du bloc**, une carte par version, de haut en bas par numéro croissant
(la plus récente près du bloc). Chaque carte : un aperçu à gauche (la première image de cette version), son nom, la date
(ou « affichée »), et une croix en haut à droite. La version affichée est sur l'accent et n'a pas de croix.
- Elle reste ouverte tant que le pointeur est sur l'étiquette, sur les cartes ou dans le petit vide entre les deux ; elle
  se ferme **0,25 s** après qu'il les a quittés. Un **clic sur l'étiquette l'épingle** (stylet sans survol) : elle reste
  jusqu'à un appui ailleurs. Elle se ferme aussi pendant un geste de la bande (glisser un plan, rogner…) et quand un menu
  s'ouvre.
- Clic sur une carte : « Passer à la version Vn » (une étape d'annulation nommée) ; la pile reste ouverte pour comparer.
- Croix : une **confirmation sur la carte même** (« Supprimer V2 ? » avec Supprimer / Garder), puis « Supprimer la version
  V2 » (annulable, Ctrl+Z). La version affichée n'a pas de croix : on ne supprime jamais la dernière version.
- Plus de six versions : six cartes et la molette fait défiler (petites flèches). Peu de place au-dessus : moins de cartes.
- **Zéro latence** : les aperçus passent par la file des vignettes de fond (`thumbs.rs`, `texture_rangee`), jamais
  composés sur le fil de l'interface ; gabarit gris tant qu'ils ne sont pas prêts ; seules les cartes visibles en demandent.
  Fermée ou ouverte pointeur immobile, la pile ne demande aucune image d'écran (essai d'inactivité).
- Bulles d'une seconde (`bulles.rs`) sur les cartes et les croix ; les couleurs viennent de `theme::`.

**Clic droit sur un plan de la bande ou de la planche** → **Nouvelle version ›** : *En dupliquant la version en cours*
(`version.nouvelle` : le plan tel qu'il est est gardé, on continue sur une copie) ou *En partant d'un plan vide*
(`version.vide`). Les deux sont une étape d'annulation nommée et la nouvelle version devient l'active. Elles sont aussi
dans la fiche (icônes à côté de la version), Storyboard > Versions du plan, Ctrl+K, et ont un bloc sans touche dans
`keymap.toml`.

**Ce qu'une version « plan vide » garde et remet à zéro** :
- gardé : la **durée**, la **taille du cadre** (celle du film), la **caméra** (ses clés : le cadrage ne bouge pas, décision
  d'Anthony), le repère 3D, les textes d'action / dialogue / notes, la couleur de repère, les calques de toute la scène (le
  décor) ;
- remis à zéro : les calques de dessin (un seul calque vectoriel « Dessin » vide, au-dessus du décor, comme dans un plan
  neuf), les marques d'image, les signets et points d'entrée / sortie.
La version d'avant est rangée telle quelle sous son numéro.

**Supprimer les versions non affichées…** (Storyboard > Versions du plan, deux entrées : tous les plans, ou le plan courant ;
`version.purger`, `version.purger_plan`, Ctrl+K) : supprime toutes les versions rangées, la version affichée de chaque plan
reste. Une **confirmation** donne le compte (« 7 versions de 4 plans seront supprimées »), puis **une étape d'annulation**
« Supprimer les versions non affichées » (Ctrl+Z les ramène toutes). Grisées quand il n'y a rien à supprimer (par Ctrl+K :
le message « Aucune version à supprimer »). Les états gardés par les **envois** (`versions/`, `envois.json`) ne sont pas
des versions du plan et **ne sont pas touchés** : un envoi se rouvre et se compare comme avant. Un plan qui n'a plus que sa
version affichée n'a plus de versions : il redevient « V1 » et la prochaine nouvelle version sera « V2 ».

**Envois et comparaison** : passer d'une version à l'autre change le contenu du plan, donc (empreinte existante) le plan est
repéré « modifié depuis l'envoi » s'il diffère de l'état envoyé, et perd son repère quand on revient à la version
envoyée. « Comparer » propose toutes les versions comme côtés.

**Export pour le montage** : un plan qui a plusieurs versions porte celle qui part dans le nom de son clip et de son
fichier : `SQ01_PL012A_V2` (`media/SQ01_PL012A_V2.mov`) ; un plan à une seule version garde `SQ01_PL012A`. La bobine de
l'EDL reste celle du plan (« PL012A », 8 caractères au plus), le titre long dit « · V2 ». Revenir à V1 avant d'exporter
donne `…_V1`.

**Limites connues** : une version rangée ne se lit pas image par image dans la pile (première image seulement) ; l'étiquette
est peinte sur la première vignette du bloc (pas dans la vue Vignettes / planche, qui garde « Plan 3 · V2 » dans son titre et
le clic droit) ; sur un bloc très étroit seul le point ou le clic droit donnent accès aux versions.

### À vérifier à l'écran (versions dans le montage)

1. Un plan à une seule version : aucune étiquette. Clic droit > Nouvelle version > En dupliquant : « V2 » apparaît, près du haut
   au milieu du bloc ; lisible dans les trois thèmes, sur un bloc large et sur un bloc étroit (point), avec la pastille ambre
   (après avoir retouché un plan envoyé) : les deux se voient sans se chevaucher.
2. Survoler « V2 » : les cartes montent **au-dessus du bloc** (pas coupées par la bande) ; les aperçus arrivent un instant
   après (gabarit d'abord) ; la lecture n'accroche pas pendant le survol.
3. Aller de l'étiquette aux cartes sans la perdre ; la quitter : elle se ferme au bout d'un quart de seconde. Clic sur une
   carte : le plan change de version tout de suite (Historique : « Passer à la version V1 ») ; Ctrl+Z.
4. Croix > « Supprimer V1 ? » > Garder, puis Supprimer ; Ctrl+Z la ramène. Pas de croix sur la version affichée.
5. Clic droit > Nouvelle version > En partant d'un plan vide : même durée, même caméra (le cadrage ne bouge pas), un calque
   « Dessin » vide ; Ctrl+Z.
6. Avec 8 versions : six cartes, la molette fait défiler.
7. Storyboard > Versions du plan > Supprimer les versions non affichées… : le compte dans la confirmation, annulation ;
   grisé quand rien à supprimer.
8. Exporter pour le montage avec un plan en V2 : les clips s'appellent `…_V2`.
9. Dire si l'étiquette (place, taille) et l'ordre des cartes (la plus récente près du bloc) conviennent, et si l'on veut aussi
   l'étiquette dans la vue Vignettes.

### Essais ajoutés

`pilevers_tests.rs` (nomenclature, règle de visibilité de l'étiquette, placement sur blocs larges / étroits / avec pastille,
machine d'ouverture et de fermeture, disposition de la pile, nouvelle version vide, étapes d'annulation, dernière version,
suppression des versions non affichées avec compte et annulation, envois intacts, noms de clips de l'export, survol / clic /
croix / confirmation / inactivité dans l'application, aperçus par la file de fond) ; `envois_tests.rs` (dates ; fichier et protections ; empreintes — ce qui compte, ce qui ne compte pas, stabilité après
enregistrement et réouverture sur le projet complet de `fiabilite_fixture` ; instantané écrit une fois, relu à l'identique,
copié par « Enregistrer sous », jamais effacé ; suivi dans l'application : repère, retour à l'état de l'envoi, projet rouvert,
navigation, repos, coût) ; `planvers_tests.rs` (modèle, annulation et noms des étapes, fichier et protections, storyboard
verrouillé) ; `comparer_tests.rs` (composition, superposition, différence, fil, envoi relu, lecture seule, repos).

### À vérifier à l'écran

1. Marquer un envoi sur une copie d'un vrai projet : la fenêtre, le message de la barre d'état, `envois.json` et `versions/` dans
   l'Explorateur ; le temps ressenti (rien ne doit se figer) ; la taille de `versions/` (les dessins sont des liens durs
   quand le disque le permet : elle ne devrait presque rien ajouter à la première fois).
2. Retoucher un plan : la pastille apparaît une demi-seconde après qu'on a lâché le stylet, jamais pendant le trait ; Ctrl+Z
   la retire. Sa place et sa lisibilité dans les trois thèmes, sur des blocs larges et étroits, et sur la vue Vignettes à
   plusieurs zooms ; la bulle au bout d'une seconde.
3. « Plan modifié suivant » : la tête de lecture et la sélection suivent ; « Choisir les plans modifiés » les met tous en
   évidence.
4. Nouvelle version du plan, retoucher, « Passer à » : le dessin change tout de suite ; Ctrl+Z défait chaque geste (le nom
   dans le panneau Historique) ; la ligne « Version » de la fiche tient dans une colonne étroite.
5. Comparer : les quatre modes sur un vrai plan avec des effets ; le volet à la souris ; la superposition rouge / bleu est
   lisible ; la fenêtre se compose en une ou deux secondes sans geler le dessin ; elle reste ouverte quand on change de plan
   sans erreur (elle suit le plan comparé).
6. Fermer et rouvrir le projet : les envois, les pastilles (aucun plan faussement modifié) et les versions sont là.
7. Dire si la couleur ambre de la pastille et son emplacement conviennent, et si « Restaurer » (qui crée une version) est ce
   qu'il attendait, ou s'il préfère que la version choisie remplace l'active.

## 16. Numérotation des plans : suivie ou verrouillée (2026-10-20, branche `numerotation-export`)

Règles et code : `src/numerotation.rs` (module en tête), interface `src/numerotationui.rs`.

- **Réglages du projet > Réglages > « Numérotation des plans »** : « Suivie » (comme avant : le numéro d'un plan est
  son rang dans le film) ou « Verrouillée » (chaque plan garde son numéro). Le bouton « Renuméroter… » n'agit qu'en
  numérotation verrouillée.
- **Verrouillée** : les numéros d'aujourd'hui (1, 2, 3…) sont figés ; ensuite
  - un plan **inséré** entre le 12 et le 13 devient **12A** (puis **12B** juste après le 12A) ; couper un plan, le
    dupliquer, coller, « Nouveau plan » suivent la même règle ;
  - un plan **supprimé** laisse un trou, son numéro n'est **jamais redonné** (un plan inséré à sa place reçoit une
    lettre : le montage a peut-être déjà un « 13 ») ; réunir deux plans en supprime un ;
  - un plan **déplacé** garde son numéro (les numéros ne sont alors plus dans l'ordre du film) ;
  - à la fin du film, un plan prend l'entier suivant le dernier (13 après 12A, si 13 n'existe pas).
- **Forme d'un numéro** : un entier puis une queue de chiffres et de lettres majuscules. Ordre : 12 < 12.5 < 12A <
  12A1 < 12AA < 12B < 13. Entre 12A et 12B qui se touchent : **12A1** (puis 12A2…). Entre 12 et 12A (rien ne tient avec
  des lettres) : **12.5** (le point sépare la base d'une queue qui commence par un chiffre). Avant le premier plan :
  « 0 » puis « -1 ». Un numéro est toujours unique.
- **Séquences** : même mécanique (SQ01, SQ01A…).
- **Renuméroter** (menu Storyboard > « Renuméroter les plans… », « Plus… » du montage, Ctrl+K) : confirmation, puis 1, 2,
  3… dans l'ordre du film, plans et séquences ; **une** étape d'annulation nommée « Renuméroter les plans ».
  Verrouiller / déverrouiller : aussi une étape (« Verrouiller / déverrouiller la numérotation des plans »). Déverrouiller
  oublie les numéros propres.
- **Un nom à soi** (fiche du plan) l'emporte toujours à l'écran ; le numéro reste disponible pour les noms de fichiers
  des exports pour le montage (« SQ01_PL012A »).
- **Partout où un plan est nommé** : bande de montage, planche, titre du plan, fiche, fenêtre « Aller à », commentaires,
  incrustations (nom du plan), PDF et page HTML du storyboard, lecture du storyboard pour les sons, export Storyboard
  Pro (**le nom du plan SBP est le numéro ; la correspondance exacte avec un numéro de plan de Storyboard Pro est à
  vérifier**, SHOT n'écrit que le nom).
- **Fichier** : champs facultatifs du manifeste (`numerotation_verrouillee`, `numero` d'une séquence et d'un plan),
  absents tant que la numérotation est suivie : un ancien projet s'ouvre tel quel et un projet suivi s'enregistre
  exactement comme avant ; `VERSION` reste 27. Une version de SHOT d'avant ouvre un projet verrouillé mais perd les
  numéros à l'enregistrement.

### À voir à l'écran (numérotation)

1. Réglages du projet : « Verrouillée » ; la bande du montage ne change pas. Nouveau plan après le 2 : « Plan 2A ».
2. Supprimer le 3 : les autres gardent leur numéro ; un nouveau plan après le 2 se nomme 2A, pas 3.
3. Glisser le plan 5 devant le 1 : il reste « 5 ». « Renuméroter les plans… » : confirmation, tout repart de 1 ; Ctrl+Z.
4. Les titres (planche, fiche, PDF) disent « Plan 12A ».
## 17. Exporter pour le montage (2026-10-20, branche `numerotation-export`)

Détail, formats et liste de contrôle : `docs/EXPORT_MONTAGE.md`. Code : `src/editorial.rs`, `src/editorialjob.rs`,
`src/editorialui.rs`.

- **Où** : Fichier > Exporter… > « Pour le montage (EDL / XML / OTIO) » (icône `FILM_REEL`), Ctrl+K « Exporter pour le
  montage », « Plus… » de la barre du montage ; action `fichier.exporter_montage` (un bloc dans `keymap.toml`, sans touche).
- **La fenêtre** (comme celle de l'export vers Storyboard Pro) : formats (3 cases), dossier, titre, étendue (tout le film /
  la séquence courante), time code de départ (rouge s'il est illisible), NTSC (pour 24, 30, 60 i/s), vidéos (ProRes /
  H.264, moitié de taille), son, chemins (relatifs / absolus). Dessous : le résumé de ce qui partirait (plans, durée,
  cadence, fondus écrits et poignées, extraits son), l'avertissement « la numérotation est suivie » (les noms des clips
  changeraient), les transitions écrites comme des coupes, ffmpeg. Pendant l'export : le plan en cours, deux barres
  (images, son), le temps restant, « Arrêter » (efface ce qui a été écrit). À la fin : les fichiers écrits et « Ouvrir le
  dossier ».
- **Pendant l'export** : le projet n'est retenu que le temps de la copie (quelques images d'écran), puis libre ; l'export
  utilise le projet tel qu'il était au départ et continue si l'on change d'onglet (§ 28).
- **Numérotation verrouillée** : bouton « Verrouiller la numérotation » et « Renuméroter les plans » sous « Plus… » du montage
  (icônes `LIST_NUMBERS` / `LOCK_KEY` et `HASH`, bulles), menu Storyboard, Réglages du projet.

### À voir à l'écran (export pour le montage)

1. La fenêtre sur un projet de quelques plans : résumé juste (plans, durée, fondus), time code rouge quand on tape n'importe quoi.
2. Un export : la fenêtre reste vivante, les deux barres avancent, « Arrêter » efface le dossier.
3. Le dossier produit : `media/` (un fichier par plan), `audio/`, les trois fichiers de montage, `LISEZ-MOI.txt`.
4. Puis l'import dans Premiere Pro, DaVinci Resolve, Avid : voir la liste de `docs/EXPORT_MONTAGE.md` (« À vérifier par Anthony »).

---------------------------------------------------------------------------------------------------

## 18. Calque de couleur (la couleur sous le trait, 2026-10-20, branche `canevas-couleur`)

Demande d'Anthony : pouvoir colorier sous le trait sans l'abîmer, avec des images qui suivent celles du trait
(l'idée « trait / couleur » de Toon Boom ; le comportement exact de Storyboard Pro ou de Harmony n'a pas été
vérifié : **à vérifier**, rien n'est copié). Code : `src/calquecouleur.rs` (modèle, miroir, zones),
`src/calquecouleurui.rs` (commandes, menu, pot de peinture, pinceau dans la zone).

### Vocabulaire et commandes

- « Calque de couleur », « calque de trait », « sous le trait », « Pinceau dans la zone fermée », « Fermer les
  trous jusqu'à N px », « Détacher du calque de trait ».
- `calque.nouveau_couleur` (« Nouveau calque de couleur (sous le trait du calque courant) ») : icône palette du
  panneau Calques (bulle : nom, raccourci, description), Calques > Nouveau calque > « De couleur », clic droit
  sur un calque (vide du panneau compris) > « Nouveau calque de couleur » et « Calque de couleur » >
  « Ajouter… », Ctrl+K. Aucune touche par défaut (bloc dans `keymap.toml`). `calque.detacher_couleur` : le
  calque de couleur courant redevient un calque bitmap ordinaire.
- Dans le panneau Calques : la ligne d'un calque de couleur est en retrait, avec « couleur » et « sous <trait> » ;
  celle d'un trait en porte le nombre. Dans la timeline : la lettre C à la place de B / V, ligne décalée sous son trait.

### Comportement

| Opération | Sur le calque de trait | Sur le calque de couleur |
|---|---|---|
| Images (clés, expositions, trous) | La couleur les suit (miroir à chaque image d'écran et à chaque étape) | Refusé avec un message (« pose, déplace ou supprime les clés sur le trait ») ; poignées de la timeline et du montage absentes |
| Nouveau dessin / copie / casser l'instance / dupliquer | Un dessin de trait copié emporte la couleur du dessin copié ; un nouveau dessin a une couleur vide | — |
| Monter / descendre / glisser dans la pile | La couleur le suit, toujours juste dessous (dans le même dossier) | Refusé (« reste sous son calque de trait ») |
| Œil | Cache aussi la couleur | Cache la couleur seule |
| Verrou, opacité, mode de fusion, pelure d'oignon, effets, groupe | À chacun (on colorie sous un trait verrouillé) | À chacun |
| Transformation, profondeur, pré / post-comportement | Ceux de la couleur | Recopiés du trait |
| Supprimer | Emporte ses couleurs | La couleur seule |
| Dupliquer, copier / coller | Les couleurs partent avec lui (leurs dessins sont copiés) ; coller dans un autre plan garde le lien | Une copie de plus sur le même trait (ou ordinaire si le trait n'est pas là) |
| Fusionner avec le dessous | Le trait + sa couleur = un seul calque bitmap ordinaire | idem |
| Trait disparu (fusion, coupe) | — | Le lien s'oublie, le calque reste |

- **Annulation** : une commande = une étape (la couleur est accordée dans la même étape).
- **Pot de peinture** : un clic remplit la zone fermée par le trait de l'image courante, sous le trait (la couleur
  passe 2 px sous lui) ; les trous du trait jusqu'à 2 × N px sont fermés (N = 4 : 8 px, réglable). Un clic juste sur un
  trait fin prend la zone la plus proche ; au cœur d'un aplat noir : un message. Si la zone fuit (trou trop grand),
  tout le dehors est colorié et le message le dit (Ctrl+Z). La couleur remplace celle d'avant dans la zone.
- **Pinceau et crayon** : par défaut, le trait reste dans la zone fermée où il commence (un pochoir calculé à partir
  des murs du trait, `Zone::pochoir`) ; « Pinceau dans la zone fermée » le débranche. La gomme n'est pas limitée.
- **Latence** : les murs du trait (distance au trait, `fond::edt`) coûtent le plus ; ils sont gardés par version du
  dessin du trait (`calquecouleur::Cache`) et précalculés **sur un autre fil** dès qu'on est sur un calque de couleur
  avec le pot de peinture, le pinceau ou le crayon. Un clic ou un trait ne calcule plus que la zone (un remplissage
  de la région touchée). Mesure d'un projet 3840 × 2160, un personnage de 1000 × 1500 px : voir
  `filling_a_big_sheet_is_fast_enough_to_click` (affiche les temps).
- **Format** : `calque_couleur` (rang du trait, couples de dessins, trous, zone), **format 30**. Un projet d'avant s'ouvre
  tel quel ; un projet enregistré en 30 n'est pas lisible par une version d'avant (même règle que 25 à 27). Les numéros
  28 et 29 sont ceux d'autres chantiers menés en parallèle.
- **Export, PDF, vignettes, montage, GPU** : la couleur est un calque bitmap ordinaire pour eux (rien n'a changé). Export
  Storyboard Pro : calque bitmap sous le trait, avec une note dans la fenêtre d'export (le lien n'y existe pas : à vérifier).
- **Avec « Fond sous le trait »** (`fond.rs`) : les deux coexistent, rien n'a changé dans l'un pour l'autre. Le fond est
  une strate du même calque, déduite du trait (une forme, une couleur) ; le calque de couleur est un calque à part,
  colorié zone par zone, avec ses propres images. Ordre de composition : fond du trait, couleur, trait.
- **Limites connues** : couper / coller une clé du trait (même calque) ne remet pas sa couleur (elle reste dans le
  dessin d'origine : glisser la clé la garde) ; un effet du trait (tremblement) ne bouge pas la couleur ; le trait
  doit exister à l'image pour colorier (sinon, un pinceau en pose un vide) ; un calque de toute la scène, un dossier,
  un calque d'effet, vidéo ou CTG ne portent pas de couleur.

### À vérifier à l'écran (calque de couleur)

1. Icône palette dans le panneau Calques : bulle (nom, description), calque créé sous le trait, en retrait, « sous Trait ».
2. Pot de peinture sur le calque de couleur : un clic dans un contour fermé le remplit sous le trait, sans liseré clair
   entre la couleur et le trait ; un contour avec un petit trou ne fuit pas ; un grand trou : message + tout le dehors.
3. Pinceau : un trait qui déborde du contour reste dedans ; « Pinceau dans la zone fermée » (clic droit sur le calque >
   Calque de couleur) le laisse déborder.
4. Dans la timeline : poser, allonger, supprimer une clé du trait : la couleur suit ; un dessin dupliqué garde sa couleur.
5. Œil du trait : cache aussi la couleur ; œil de la couleur : la couleur seule.
6. Dupliquer, copier / coller le trait : les couleurs viennent avec lui ; Ctrl+Z revient d'un coup.
7. Export d'images et PDF : la couleur est sous le trait.

## 19. Toile infinie (dessiner hors de la feuille, 2026-10-20, branche `canevas-couleur`)

Ce qui change à l'écran. Le fonctionnement de Storyboard Pro est **à vérifier** : SHOT reprend l'idée d'une toile
sans bord, pas son détail.

- **Plus de page** : l'ombre sous la feuille a disparu. Le papier (ou le damier, ou le noir de l'arrière-plan,
  onglet « Arrière-plan » sous la timeline) remplit tout l'espace de dessin, à tout zoom (de 2 % à 6 400 %), et le défilement
  (bouton du milieu, Espace + glisser, outil Main) n'a pas de bord. Il n'y avait pas de zone hachurée à
  proprement parler dans cette version : ce qui entourait la feuille était l'ombre et le fond sombre.
- **Cadre de la caméra** : un trait fin et discret (gris) quand il se confond avec la feuille ; avec l'outil
  Caméra, ou quand le cadre est déplacé ou tourné, le cadre habituel (blanc, étiqueté « Caméra »). Le bouton
  « Estomper le hors-champ » (réglages de la caméra, `camera.masque`) voile tout ce qui est autour du cadre, y compris
  loin de la feuille (voile plus léger qu'avant à l'écran, 45 au lieu de 70 sur 255).
- **Dessiner** : pinceau, crayon, gomme, formes, texte, lignes vectorielles : partout, y compris à gauche et
  au-dessus de la feuille. Un trait qui traverse le bord de la feuille se continue dehors. Ctrl+Z / Ctrl+Maj+Z :
  d'un coup, comme un trait ordinaire.
- **Pipette** (Alt + clic, ou outil Pipette) : prend la couleur affichée aussi hors de la feuille (papier compris).
- **Pot de peinture** : borné par la feuille ; un clic hors d'elle ne fait rien et le dit dans la barre d'état
  (« Le pot de peinture remplit dans la feuille… »).
- **Ajuster à la fenêtre** (barre sous le dessin, menu du clic droit, `vue.ajuster`) : cadre la **caméra**, pas
  tout ce qui est dessiné (choix de SHOT : prévisible, le même qu'avant). Pour voir tout ce qu'on a dessiné
  loin : dézoomer à la molette.
- **Exports** (images, vidéo, PDF) : le cadre filmé décide ; s'il regarde hors de la feuille il montre ce qui y est
  dessiné, sinon du papier. Rien n'est exporté hors du cadre.
- **Limites** : les zones bitmap sélectionnées (lasso, baguette de couleur) et leur déplacement prennent les
  pixels de la feuille (on peut déplacer ce qu'on a pris vers l'extérieur) ; les effets de calque, les calques
  d'ajustement, le fond sous le trait, les calques de couleur, l'enveloppe et la perspective, la propagation,
  l'import / export Storyboard Pro restent à la feuille. Le zoom arrière extrême (plus de 4 096 px de monde
  dessiné à l'écran) montre ce qui est hors de la feuille un peu plus flou (la texture est réduite).
  Au-delà de 32 768 px de la feuille, rien ne se pose.
- Code : `src/monde.rs`, `src/raster.rs` (tuiles hors-feuille), `src/render.rs`, `src/canvas.rs`,
  `src/bottomtabs.rs` (arrière-plan), `src/camtools.rs` ; `docs/ARCHITECTURE.md` « Toile infinie ».

### À vérifier à l'écran (toile infinie)

1. Ouvrir un projet existant : il s'affiche comme avant dans la feuille ; autour, du papier (même couleur) à la
   place de l'ombre ; aucun dessin ne manque ni ne change.
2. Dézoomer (molette) et déplacer la vue loin (Espace + glisser) : le papier suit sans bord ni trou, sans
   ralentissement. Arrière-plan damier : le damier continue hors de la feuille ; noir : tout est noir.
3. Dessiner au pinceau à gauche de la feuille, au-dessus, puis un trait qui traverse le bord : pas de retard au
   bout du pinceau, pas de coupure au bord de la feuille (zoom 100 % et zoom arrière), pas de scintillement
   pendant le trait.
4. Ctrl+Z / Ctrl+Maj+Z sur ces traits ; enregistrer, fermer, rouvrir : tout est là (hors de la feuille aussi).
5. Gomme hors de la feuille, formes (ligne, rectangle, ellipse), texte, trait vectoriel loin de la feuille.
6. Rotation (Maj + molette) et miroir de la vue avec du dessin hors de la feuille : il tourne avec le reste.
7. Caméra : déplacer le cadre sur du dessin hors de la feuille (outil Caméra) ; l'export d'images montre ce dessin ;
   « Estomper le hors-champ » voile autour du cadre, discrètement.
8. Pot de peinture hors de la feuille : rien ne bouge, message dans la barre d'état. Pipette hors de la feuille :
   prend la couleur du trait.
9. Pelure d'oignon et table lumineuse avec des dessins hors de la feuille : ils apparaissent teintés.
10. Un plan avec des effets de calque et du dessin hors de la feuille : les effets agissent dans la feuille, le
    dessin hors feuille est montré sans effet (limite connue).
11. Les vignettes de la planche (storyboard) montrent le cadre, comme avant.

---------------------------------------------------------------------------------------------------

## 20. Effets du trait (2026-10-20, branche `effets-trait`)

Demande d'Anthony : « le tremblement, j'adore » — d'autres effets qui agissent sur le trait lui-même. Détail : README
(« Effets du trait »), `docs/ROADMAP.md` et `docs/PERFORMANCE.md` (section 13).

### Mots

| Idée | Mot | À éviter |
|---|---|---|
| Le dessin se trace de lui-même | **Écriture du trait** (avancement) ; l'inverse : **effacement** | write-on, animation de tracé |
| Chaque trait redoublé de passes décalées | **Crayonné multiple** (passes, retirage) | rough, doublage |
| L'épaisseur qui pulse | **Respiration** (amplitude, période) | pulsation du trait |
| Vague le long du trait | **Ondulation du trait** (≠ « Onde » et « Ondulation » de la famille Déformation, qui déforment l'image) | wiggle |
| Tirets qui défilent | **Pointillés qui défilent** (tiret, intervalle) | dash |
| Le trait part en poussière | **Désagrégation** (grains, vent) | dissolution, particules |
| Famille d'effets | **Trait** (en tête des familles) | effets de matière (la série d'après) |

### Où et quels gestes

- **(Depuis le 2026-10-20 : un bouton déroulant « Trait ▾ », partie 25 ; la rangée d'icônes décrite ici n'existe plus.)**
  **Rangée d'icônes « Trait »** en tête du panneau Effets : six icônes (signature, gribouillis, battement, vague en dents
  de scie, trois points, grains) cochent et décochent chaque effet sur le calque courant (une étape d'annulation) ; enfoncée =
  l'effet est sur le calque et actif ; grisée, avec la raison dans la bulle, sur un calque bitmap. Clic droit sur une
  icône : lui assigner un raccourci. À droite, la **baguette** : « Apparition écrite », « Croquis nerveux »,
  « Poussière ».
- **Aussi** : « Ajouter un effet » > Trait ; clic droit sur un calque > Effets > Trait (cases à cocher) ; Ctrl+K (« écriture »,
  « crayonné », « respiration », « ondulation », « pointillés », « désagrégation », « préréglage du trait »).
- **Régler** : le nom de l'effet dans le panneau Effets montre ses clés dans la timeline et le montage (losanges violets,
  comme le tremblement) ; chaque réglage a une bulle. L'avancement de l'écriture et de la désagrégation se met en clés (0 % puis
  100 %) ; « Création automatique des clés » marche comme pour les autres effets.
- **Tant qu'on dessine** sur le calque (trait, gomme, sélection, transformation), ses effets du trait et son tremblement se
  taisent : on voit le dessin tel qu'il est.
- **Copier-coller** : comme le reste de la pile ; une seule fois par calque et par sorte (comme le tremblement) ; laissés de
  côté sur un calque bitmap, la barre d'état le dit.

### À vérifier

- Aucune comparaison avec Storyboard Pro, TVPaint ou WigglyPaint n'a été vérifiée (**à vérifier** : l'ordre d'écriture
  des traits dans Storyboard Pro pour son effet de tracé, s'il en a un).
- Les valeurs par défaut (écriture à 60 %, crayonné à 2 passes de 2,5 px, respiration 35 % sur 24 images, ondulation 4 px,
  pointillés 18 / 12 px, poussière 40 % avec 80 px de dispersion) sont des choix de SHOT, à régler à l'œil.

### À vérifier à l'écran (par Anthony)

1. Un calque vectoriel avec quelques traits (un visage) : cliquer l'icône signature. Le dessin est à moitié tracé (60 %),
   la pointe de chaque trait s'efface-t-elle assez doucement ? « Bord doux » à 20 px, « Douceur du rythme » à 100 %.
2. Poser « Apparition écrite » (baguette) à l'image 1 d'un plan de 3 secondes : le dessin se trace en une seconde, les clés
   se voient dans la timeline ; tirer la tête de lecture : l'écriture suit sans retard. À 100 %, le dessin est celui d'avant.
3. « Tous les traits ensemble » : tous avancent à la fois ; « Effacement » : le dessin se défait dans l'ordre ; « Durée
   selon la longueur » : un grand trait met plus de temps qu'un petit.
4. Dessiner un trait pendant que l'écriture est à 30 % : le trait neuf se voit en entier pendant le geste, puis rejoint
   l'effet. Annuler : une étape par effet posé.
5. Crayonné multiple : des passes pâles de chaque côté du trait, qui changent toutes les deux images ; « Vitesse » 1 à 12,
   « Variantes » 2 à 6 ; le trait d'origine reste net par-dessus. « Croquis nerveux » : crayonné + léger tremblement.
6. Respiration : le trait gonfle et se resserre sur 24 images ; « Vague le long du trait » : le gonflement court le long du
   trait ; épaisseur de base à 50 % puis 200 % (par clés).
7. Ondulation du trait : une vague qui avance (vitesse positive puis négative), les bouts restent en place ; « Bouts fixes »
   décoché, les bouts bougent.
8. Pointillés : tirets qui défilent dans les deux sens ; vitesse 0 : tirets fixes ; longueur et intervalle très courts.
9. Désagrégation : l'avancement de 0 à 100 % défait le trait en poussière ; « Sens du trait » à 100 % : du début vers la fin ;
   le vent change la direction ; la graine change la poussière ; « Poussière » (préréglage) en une seconde.
10. Un trait de brosse texturée (gouache, fusain) sous chaque effet : la texture suit la déformation (respiration, ondulation).
11. Lecture avec un calque de beaucoup de traits : l'écran reste fluide, les images arrivent un peu après la première fois
    (calcul de fond), le deuxième tour est servi. Un calque bitmap : les six icônes grisées, la bulle dit pourquoi.
12. Le tremblement du trait (sur la carte graphique maintenant) : même aspect qu'avant ; une pile tremblement + flou : aucun
    clignotement quand on change un réglage.
13. Les trois thèmes : icônes enfoncées lisibles dans la rangée.


---------------------------------------------------------------------------------------------------

## 21. Effets de matière (2026-10-20, branche `effets-matiere`)

Suite des effets du trait, demandée par Anthony après les effets animés. Détail : README (« Effets de matière »),
`docs/ROADMAP.md` et `docs/PERFORMANCE.md` (section 14).

### Mots

| Idée | Mot | À éviter |
|---|---|---|
| Contour d'une autre couleur autour du dessin | **Liseré** (largeur, douceur) | outline, contour (réservé au « Contour blanc » du préréglage) |
| Trous et bord qui s'effrite | **Trait sec** (trous, bord qui s'effrite) | dry brush |
| Bord qui déborde comme sur du buvard | **Encre qui bave** (étalement, bord plus foncé) | bleed |
| La couleur change le long du trait | **Dégradé le long du trait** | gradient (le « Dégradé » de la famille Rendu colore l'image) |
| Redessiner avec un autre pinceau | **Changer de plume** ; **Appliquer pour de bon** | restyle, bake |
| Famille d'effets | **Matière** (juste après Trait) | |

### Où et quels gestes

- **(Depuis le 2026-10-20 : un bouton déroulant « Matière ▾ », partie 25.)** **Deuxième rangée d'icônes « Matière »** dans le panneau Effets, sous celle du Trait : liseré (sélection), trait sec (pinceau
  large), encre (goutte), dégradé, plume. Enfoncée = l'effet est sur le calque et actif ; **grisée avec la raison dans la
  bulle** pour le dégradé et la plume sur un calque bitmap (le liseré, le trait sec et l'encre y marchent). À droite, la
  baguette : « Encre de Chine », « Feutre usé », « Contour blanc », « Arc-en-ciel », « Plume calligraphique ». Clic droit sur
  une icône : lui assigner un raccourci.
- **Aussi** : « Ajouter un effet » > Matière ; clic droit sur un calque > Effets > Matière ; Ctrl+K (« liseré », « trait sec »,
  « encre », « dégradé », « plume », « appliquer la plume », « préréglage de la matière »).
- **Régler** : comme les autres effets (curseurs, clés, losanges violets). Une **couleur** (liseré, arrêts du dégradé) se choisit
  par un sélecteur de couleur et se met en clés d'un bloc. « Changer de plume » a une **liste** (la plume calligraphique de
  SHOT, puis les réglages de pinceau de la bibliothèque) ; ce choix ne s'anime pas.
- **« Appliquer pour de bon »** (bouton sous les réglages de la plume, ou Ctrl+K) : remplace les traits de tous les dessins du
  calque, retire l'effet, une étape d'annulation nommée « Plume appliquée pour de bon » (Ctrl+Z rend les traits et l'effet).
- Tant qu'on dessine sur le calque, le trait sec, le dégradé et la plume se taisent ; le liseré et l'encre (qui ne lisent que
  les pixels) restent.

### À vérifier

- Les valeurs par défaut (liseré blanc de 4 px, trait sec 5 trous pour 100 px de 16 px, bavure de 2,5 px) sont des choix de SHOT,
  à régler à l'œil. Aucune comparaison avec un autre logiciel n'a été vérifiée (**à vérifier**).
- Une plume dont l'opacité suit l'appui (fusain, craie) sur des traits sans appui enregistré (anciens traits, souris) donne un
  trait plus pâle : le rayon d'origine est gardé, l'appui est déduit du rayon.

### À vérifier à l'écran (par Anthony)

1. Un calque vectoriel avec un visage : cliquer l'icône liseré. Un contour blanc de 4 px entoure tout le dessin ; largeur 20 px,
   douceur 10, couleur rouge ; « Derrière le trait » décoché sur un trait translucide : l'anneau ne passe pas dessous.
2. Un calque bitmap (une peinture) : le liseré, le trait sec et l'encre sont utilisables, le dégradé et la plume sont grisés et la
   bulle dit pourquoi.
3. Encre qui bave : étalement 2,5 puis 6 px, irrégularité 0 puis 100 %, bord plus foncé sur un trait de couleur ; le trait reste
   lisible ; une autre graine, une autre bavure ; préréglage « Encre de Chine ».
4. Trait sec : des trous le long des traits, un bord qui s'effrite en stries ; « Changer toutes les 2 images » : les trous
   changent de place comme le tremblement ; sur un bitmap, une version plus simple (bruit sur les bords, plages creuses).
5. Dégradé : un trait de la couleur 1 à la couleur 2 ; « Sur tout le dessin » : le premier trait rouge, le dernier bleu ;
   défilement 5 %/img avec la boucle cochée : le dégradé court le long des traits ; quatre couleurs ; « Arc-en-ciel ».
6. Plume : « Plume calligraphique » (pleins et déliés), Feutre, Fusain, Craie ; le dessin d'origine ne change pas quand on
   décoche l'effet ; la taille de la plume ; lecture sans à-coup (les traits sont rejoués une fois).
7. « Appliquer pour de bon » : les traits sont ceux de la plume, l'effet disparaît du calque, l'historique montre « Plume
   appliquée pour de bon », Ctrl+Z puis Ctrl+Shift+Z.
8. Pile : liseré + flou (le flou passe après le liseré), trait sec + dégradé + liseré ; copier-coller sur un calque bitmap (le
   dégradé et la plume sont laissés de côté, la barre d'état le dit) ; enregistrer, rouvrir.
9. Lecture et export : même image à l'écran, dans l'export et les vignettes ; un calque de 2 000 traits reste fluide.
10. Les trois thèmes : icônes enfoncées lisibles dans les deux rangées.

## 23. Onglets de projets (2026-10-20, branche `onglets`)

Demande d'Anthony : « ouvrir plusieurs fichiers dans la même fenêtre avec un système d'onglets ». Un onglet = un projet ouvert
(son annulation, sa tête de lecture, sa sélection, sa vue, ses caches). Détail : `docs/ARCHITECTURE.md` (« Onglets de projets »),
`docs/PERFORMANCE.md` (section 15), `README.md` (« Onglets de projets »).

### Mots

| Idée | Mot | À éviter |
|---|---|---|
| Un projet ouvert dans la bande | **Onglet** | fenêtre, document |
| Un onglet qui n'a servi à rien (l'accueil) | **Nouvel onglet** | onglet vide |
| Fermer un projet | **Fermer l'onglet** (menu) ; l'action garde son nom `fichier.fermer` | fermer le projet (sauf dans une phrase) |
| Pastille d'un projet pas enregistré | **•** (orange) | astérisque |

### Où et quels gestes

- **La bande** : entre la barre de menus et la barre de commandes. Un onglet par projet : son nom, « • » orange s'il est modifié ou
  s'écrit encore, une croix (sur l'onglet affiché et sous la souris). L'onglet affiché est du ton du panneau, avec un filet à
  l'accent ; les autres sont plus bas. Deux projets pas encore enregistrés : « Sans titre », « Sans titre 2 ».
- **Clic** : passe à l'onglet. **Clic du milieu** ou croix : le ferme. **Clic droit** : Fermer, Fermer les autres, Afficher dans
  l'Explorateur. **Glisser** un onglet : le déplace (un trait d'accent montre où il tombe). **« + »** : un onglet vierge
  (l'accueil). Bulle (une seconde) : le chemin du projet et son état.
- **Raccourcis** : Ctrl+T nouvel onglet, Ctrl+W fermer l'onglet, Ctrl+Tab / Ctrl+Maj+Tab onglet suivant / précédent (en boucle).
  `Tab` et `Maj+Tab` restent le mode et la planche. Tous dans Ctrl+K et assignables (clic droit sur une entrée de menu).
- **Menu Fichier** : Nouvel onglet, Fermer l'onglet, Fermer les autres onglets.
- **Ouvrir, Nouveau, Projets récents, imports** : dans un nouvel onglet (réglable : Fichier > Préférences > Général > « Ouvrir
  dans un nouvel onglet »). Si l'onglet affiché est l'accueil, le projet y prend la place. Un projet déjà ouvert : son onglet
  passe à l'écran (« … est déjà ouvert »). Un fichier `.pelure` lâché sur la fenêtre s'ouvre de même.
- **Fermer un onglet modifié** : la boîte « Modifications non enregistrées » (elle nomme le projet) ; un onglet caché passe d'abord
  à l'écran. Le voisin de droite prend la place (celui de gauche pour le dernier). Le dernier onglet ne se ferme pas : l'accueil
  revient.
- **Quitter, ou Fermer les autres, avec plusieurs onglets modifiés** : une seule boîte « Projets non enregistrés » qui les
  liste : Enregistrer tout / Ne pas enregistrer / Annuler.
- **Au lancement** : tous les onglets de la dernière séance reviennent, dans l'ordre, l'onglet affiché à la fin en dernier
  (« Rouvrir le dernier projet au lancement »). Un projet disparu est sauté.
- **Plantage** : la fenêtre de récupération liste une copie par projet ; « Tout ouvrir » met chacune dans son onglet.
- **Copier-coller** : plans, calques, images, clés, effets, zones et éléments se collent d'un onglet à l'autre ; une feuille d'une
  autre taille refuse (message avec les deux tailles), une autre cadence prévient.
- **Pendant la propagation, le coloriage, la génération des sons proposés**, la copie du projet par un export (quelques images
  d'écran) ou une boîte de dialogue : on ne change pas d'onglet (la barre d'état dit pourquoi). **Un export qui tourne n'empêche
  plus de changer d'onglet** (§ 28).

- **Copier-coller** : plans, calques, images, clés, effets, zones et éléments se collent d'un onglet à l'autre. Un projet d'une
  **autre taille de feuille** n'est plus refusé : la copie est adaptée (voir « Coller depuis un projet d'une autre taille »,
  plus bas) ; une autre cadence est signalée, ou recalculée si on le demande.
- **Pendant un export, la propagation, le coloriage, la génération des sons proposés** ou une boîte de dialogue : on ne change pas
  d'onglet (la barre d'état dit pourquoi).

### Coller depuis un projet d'une autre taille

Demande d'Anthony (2026-10-20) : « Un plan collé dans un projet d'une autre taille de feuille doit s'adapter. » Avant, la copie
était refusée (« un dessin n'a de sens qu'à sa taille »). Maintenant elle est **adaptée** (`src/adaptation.rs` : la règle, pure ;
`src/collage.rs` : la question, le calcul de fond, la reprise du collage).

**La règle** — ce que la caméra voyait reste ce que la caméra voit : le cadre de la caméra du projet d'origine est ajusté au cadre
de la caméra du projet qui reçoit, d'un même facteur en largeur et en hauteur (rien n'est déformé), centre de feuille sur centre
de feuille.

| Cas | Ce qui se passe |
|---|---|
| Même cadre et même feuille | rien n'est changé, aucune question, aucun calcul : comme avant |
| Mêmes proportions (4K → 1080, 1080 → 4K) | échelle = cadre d'arrivée / cadre d'origine (×0,5, ×2…) |
| Proportions différentes (16:9 → carré) | **Contenir** (d'office) : tout le cadre copié reste visible, des bandes de l'autre côté ; **Remplir** : le cadre d'arrivée est rempli, ce qui dépasse sort du cadre mais reste sur la feuille (toile infinie : rien n'est perdu) |
| Même cadre, feuille agrandie à la main | un simple décalage entier : les pixels sont copiés, pas mélangés |
| Cadence différente | **Garder les images** (d'office : la durée en secondes change, la barre d'état le dit) ou **garder la durée en secondes** (images, clés, rôles, marques et repères recalculés : `f' = arrondi(f × cadence d'arrivée / cadence d'origine)`, durées d'au moins 1 image ; plus lent : dessins tenus plus longtemps, plus rapide : des images sautées, un dessin d'une seule image peut disparaître) |

**Ce qui est mis à l'échelle** : dessins bitmap (feuille **et** hors-feuille, rééchantillonnés : triangle en réduction, Catmull-Rom
en agrandissement, sur des pixels prémultipliés : pas de frange de couleur), dessins vectoriels (points, épaisseurs, taille du
pinceau et du texte, puis rendu exact), fond sous le trait (peinture et coupe), pose des calques et clés de position, clés de
caméra (position ; l'avancée suit la largeur du cadre ; zoom et angles inchangés), paramètres d'effets en pixels (rayon des flous,
décalage de l'ombre, liseré, grain, tremblement… clés comprises, dans les bornes de l'effet ; la densité de trous du trait sec,
par 100 px de trait, est divisée), débord du fond sous le trait, fermeture des trous des calques de couleur et CTG, zones bitmap et
éléments vectoriels collés sur le dessin (ils gardent leur place dans le cadre). **Ne change pas** : les paramètres en pour cent,
les incrustations (en pour cent de la hauteur du cadre), les marques d'image, les textes, et — comme pour un collage de plans dans
le même projet — la scène 3D de blocage, les références liées, les commentaires, les transitions et les sons des plans.

**La question** (petite fenêtre « Coller depuis un autre projet ») : seulement pour un plan, un calque, des images ou des clés, et
seulement si les **proportions** ou les **cadences** diffèrent. Elle montre d'où vient la copie et ce qui va se passer ; « Ne plus
demander » applique ensuite les derniers choix sans question. Les choix sont retenus dans les réglages et se changent dans Fichier
> Préférences… > Général > « Coller depuis un autre projet ». Les clés de caméra et d'effets, les effets, les éléments, les zones
et la sélection sur plusieurs calques appliquent ces choix retenus sans poser la question.

**Pendant le calcul** : les dessins sont rééchantillonnés sur des fils de fond (tous les cœurs sauf un) : « Collage en cours… 12/72
dessins » dans la barre d'état, la fenêtre reste vivante ; raccourcis et bande d'onglets sont figés (comme pendant un export) et le
collage se pose dans le plan, le calque et l'image où il a été demandé. Mesures : `docs/PERFORMANCE.md`, section 18.

**Après** : une seule étape d'annulation, nommée « Coller des plans (adapté de 3840×2160) » (ou « un calque », « des images »…) ;
la barre d'état dit ce qui a été adapté : « Collé depuis un projet 3840×2160 : mis à l'échelle ×0,5. Plan collé après le plan 1. »
Le presse-papiers n'est pas touché : la même copie se colle ensuite ailleurs, à sa propre taille ou adaptée à une autre.

**Sélection sur plusieurs calques** : copiée dans un autre projet, aucun numéro de calque ne correspond ; chaque calque copié va
sur le premier calque choisi de même sorte (vectoriel pour des éléments, bitmap pour des pixels) encore libre, dans l'ordre.

### À vérifier

- Les largeurs de la bande (96 à 220 px) et sa hauteur (26 px) sont des choix de SHOT, à régler à l'œil.
- Les raccourcis Ctrl+T / Ctrl+Tab sont ceux d'un navigateur (proposition, pas ceux de Storyboard Pro).
- Les plans copiés dans un projet d'une autre taille sont adaptés (2026-10-20, branche `coller-adapter`) : voir la section
  suivante. Ses choix de départ — « contenir » plutôt que « remplir », « garder les images » plutôt que « garder les secondes » —
  sont à régler à l'œil.

### À vérifier à l'écran (par Anthony)

1. Lancer SHOT, ouvrir trois projets (Ctrl+O, ou les projets récents) : trois onglets, l'un après l'autre à droite, chacun avec
   son nom ; le dernier ouvert est affiché.
2. Passer de l'un à l'autre à la souris puis à Ctrl+Tab / Ctrl+Maj+Tab : instantané, la tête de lecture, la sélection, le
   zoom et le mode (storyboard / animation) de chaque projet sont retrouvés. Dessiner un trait dans un onglet, Ctrl+Z : seul
   cet onglet recule.
3. Modifier un projet : « • » orange sur son onglet ; l'enregistrer (Ctrl+S) : il disparaît. Lancer un enregistrement lourd et passer
   tout de suite à un autre onglet : l'écriture finit seule, le « • » s'en va, la barre d'état dit « Nom — Enregistré… ».
4. Ctrl+T : un onglet « Nouvel onglet » avec l'accueil ; « Nouveau projet » ou un projet récent y prend la place (pas d'onglet de
   plus). Ctrl+W ferme l'onglet affiché ; sur un projet modifié, la boîte nomme le projet.
5. Clic du milieu sur un onglet : il se ferme ; clic droit : Fermer, Fermer les autres, Afficher dans l'Explorateur ; glisser un
   onglet à gauche d'un autre le déplace.
6. Modifier deux onglets (dont un caché), Quitter (Ctrl+Q) : une seule boîte liste les deux projets ; Annuler ne fait rien ;
   Enregistrer tout écrit les deux puis quitte.
7. Copier un plan (clic dans le montage, Ctrl+C), aller dans un autre projet, Ctrl+V : le plan est là, ses dessins aussi, et
   modifier la copie ne touche pas à l'original. Même chose pour un calque (W puis X), des images, des clés de caméra, des effets.
   Coller dans un projet d'une autre taille : voir « Coller depuis un projet d'une autre taille » (vérifications 11 à 17).
8. Deux projets avec le même effet (flou) sur leur premier dessin, de couleurs différentes : chaque onglet montre ses couleurs,
   aussi après plusieurs allers-retours ; les vignettes du montage aussi.
9. Fermer SHOT avec trois onglets, le rouvrir : les trois reviennent, le même est affiché. Désactiver « Rouvrir le dernier projet
   au lancement » : SHOT s'ouvre sur l'accueil.
10. Fichier > Préférences > Général > « Ouvrir dans un nouvel onglet » décoché : Ouvrir remplace le projet affiché (après la question).

### À vérifier à l'écran : coller depuis un projet d'une autre taille (par Anthony)

11. Un projet 1080 et un projet 4K ouverts dans deux onglets. Copier un plan du 4K (clic dans le montage, Ctrl+C), aller dans le
    1080, Ctrl+V : pas de question (mêmes proportions) ; « Collage en cours… » une fraction de seconde, puis le plan est là,
    **deux fois plus petit** : ce que la caméra voyait dans le 4K, elle le voit dans le 1080 (dessins bitmap nets, traits à la
    bonne place et à la bonne épaisseur, flou au même aspect, caméra qui bouge de la même façon). La barre d'état dit
    « Collé depuis un projet 3840×2160 : mis à l'échelle ×0,5. ». Ctrl+Z : tout le plan part d'un coup ; l'historique l'appelle
    « Coller des plans (adapté de 3840×2160) ».
12. Le contraire : copier ce plan du 1080 et le coller dans le 4K : il revient à sa taille (un peu plus doux, il a été réduit).
13. Un projet carré 1080 × 1080 et un projet 16:9 : la fenêtre « Coller depuis un autre projet » apparaît, avec « Contenir » et
    « Remplir ». Contenir : le plan entier tient dans le carré. Remplir : le carré est rempli, les côtés du plan dépassent du cadre
    (ils sont toujours sur la feuille : élargir la vue pour les voir). Annuler : rien n'est collé. Cocher « Ne plus demander » : le
    collage suivant se fait sans fenêtre avec le même choix ; Fichier > Préférences… > Général le change.
14. Un projet à 24 i/s et un à 25 i/s, même taille : la fenêtre propose « Garder les images » ou « Garder la durée en secondes ».
    Le second choix : un plan de 24 images dure 25 images, ses clés de caméra et d'effets suivent.
15. Un calque (W puis X), des images, des clés du montage, des clés de caméra, des effets : même règle, dans un projet d'une autre
    taille. Un trait copié (Ctrl+C / Ctrl+V sur un calque vectoriel) et une zone de pixels gardent leur place dans le cadre.
16. Deux projets de même taille : tout se colle comme avant, sans fenêtre ni message d'adaptation.
17. Pendant « Collage en cours… » (plan lourd) : l'interface répond (barre d'état, souris), mais on ne change pas d'onglet et les
    raccourcis sont figés jusqu'à la fin.
11. Les trois thèmes : la bande est lisible (nom, « • », croix) ; la fenêtre étroite : la bande défile.
12. Laisser SHOT ouvert avec trois projets sans toucher à rien : le processeur reste à zéro (Gestionnaire des tâches).

---------------------------------------------------------------------------------------------------

## 22. Relecture par le réalisateur (2026-10-20, branche `revue-realisateur`)

Demande d'Anthony : son réalisateur / client **n'a pas SHOT** ; il doit pouvoir relire une livraison et renvoyer des notes que
SHOT repose en commentaires sur les bons plans. Principe, format, sécurité : `README.md` « Relecture par le réalisateur ».
Code : `src/relecture.rs`, `src/relecturejob.rs`, `src/relectureui.rs`, `assets/relecture/page.js` et `page.css`. Rien n'a été
regardé à l'écran de SHOT dans cette séance (aucun lancement) ; la page a été essayée dans un navigateur (voir plus bas).

### Mots

| Mot | Sens |
|---|---|
| **Relecture** | l'aller (la page HTML) et le retour (les notes) |
| **Page de relecture** | le fichier `.html` envoyé, qui marche sans internet |
| **Notes de relecture** | le fichier `.json` que la page fait télécharger au relecteur |
| **Importer les notes** | les poser en commentaires de la timeline |
| **Dessin du relecteur** | ce qu'il a tracé par-dessus l'image, gardé avec le commentaire |

### Où et quels gestes

| Commande | Où | Raccourci |
|---|---|---|
| Exporter pour relecture (`fichier.exporter_relecture`) | Fichier > Exporter… > « Pour relecture (page HTML) » ; fenêtre Envois (icône lunettes de chaque envoi) ; « Plus… » du montage ; Ctrl+K | aucun |
| Importer les notes de relecture (`fichier.importer_notes`) | Fichier > Importer > « Des notes de relecture (.json)… » ; « Plus… » du montage ; Ctrl+K ; **ou le `.json` lâché sur la fenêtre** | aucun |
| Montrer le dessin du relecteur | panneau Commentaires, sous l'éditeur d'un commentaire qui a un dessin (icône crayon à l'état ; corbeille : retirer le dessin) | aucun |

Un bloc sans touche par action dans `keymap.toml` (clic droit sur l'entrée d'un menu : lui en donner une) ; icônes et bulles d'une
seconde (`bulles.rs`) ; l'interrupteur allumé prend `bulles::interrupteur` (encre `sur_accent`) ; couleurs de `theme::`.

### Ce que voit l'utilisateur

- **Fenêtre « Exporter pour relecture »** : Titre ; Envoi (liste, « Aucun » par défaut ; son nom est écrit dans la page) ; Plans
  (tout le film, la séquence courante, les plans choisis, « Seulement les plans modifiés depuis l'envoi (n) ») ; dans la page :
  lecture animée, commentaires déjà posés, PDF à imprimer. Dessous : « 9 plans, 12 images de storyboard. », « Exporter… »
  (demande où écrire le `.html`), puis la barre (« image 14 / 60 », temps restant) et « Arrêter ». À la fin : « Terminé : 9 plans, 12
  images », le fichier et sa taille en Mo (avertissement au-delà de 20 Mo), « Montrer dans l'explorateur ».
- **Fenêtre « Notes de relecture »** : « Claire — « Mon film » », la date, « 14 notes, dont 3 avec un dessin. », « 12 notes à poser
  sur 9 plans. », « n retrouvées par leur numéro de plan », « n déjà importées : pas de doublon », la liste des notes sans plan, la case
  « Les poser sur le plan courant, avec leur provenance », l'avertissement et la case « Importer quand même » si le projet n'est
  pas le même, « Importer ». Puis le compte rendu : « 12 notes importées sur 9 plans, 1 note sans plan (plan supprimé). »
- **Panneau Commentaires** : les notes sont des commentaires comme les autres (fanion jaune dans la règle, auteur, date, modifiables,
  résolubles), le premier posé est choisi. Quand un commentaire avec dessin est choisi et que la tête de lecture est sur l'image
  qu'il vise, le dessin est peint **par-dessus l'image** du dessin, au cadre de la caméra.
- **Historique** : une seule étape, « Importer les notes de relecture » ; Ctrl+Z les retire toutes.

### La page (`assets/relecture/page.js`, `page.css`)

En-tête collant (titre, « Votre nom », nombre de notes, « Enregistrer mes notes »), consigne en trois lignes, sommaire (un lien par
plan), un intertitre par séquence, une carte par plan (« Plan 12A », « V2 » s'il y a des versions, durée), la lecture animée
(« Lire le plan », curseur) si demandée, puis une section par image de storyboard : l'image, sa légende (« Image 2 sur 3 · 12 i »),
action / dialogue / notes, « Déjà noté dans SHOT » (commentaires existants), la barre de dessin (« Dessiner », rouge / bleu / vert,
« Gomme », « Annuler le trait », « Tout effacer ») et le cadre jaune de la note. En bas : la note générale. Clair, une colonne sur
téléphone, deux colonnes de cartes sur grand écran, boutons d'au moins 36 points, rien ne défile de côté. La note du relecteur et ses
dessins sont gardés dans le navigateur (`localStorage`, s'il le permet) : rouvrir la page les retrouve.

### Limites

Fichier de notes 40 Mo, 3000 notes, 20 000 caractères par note, nom du relecteur 120 caractères, dessin PNG de 1,6 Mo de base64
au plus, 4096 points par côté, 8 millions de pixels, réduit à 1280 de large à l'import. Page : images de storyboard à 960 de large
(JPEG 80), lecture animée à 480 de large (JPEG 62), 16 images échantillonnées par plan au plus (1500 pour tout le film).

### À vérifier à l'écran (par Anthony)

1. Fichier > Exporter… : la ligne « Pour relecture (page HTML) » est là ; elle ouvre la fenêtre ; les cases et le résumé suivent
   l'étendue choisie ; « Seulement les plans modifiés depuis l'envoi » est grisé sans envoi, et indique le bon nombre après un envoi
   puis des retouches.
2. Exporter 20 à 30 plans : l'interface reste vivante (on peut bouger la fenêtre, la barre avance, « Arrêter » efface tout) ; la page
   se pose où tu l'as choisie, sans fichier `.part` qui traîne ; avec « PDF » : un `.pdf` du même nom à côté, trois plans par page et
   un cadre « Notes » à remplir.
3. **Ouvrir le HTML sur ton ordinateur, puis sur un téléphone** (envoie-le-toi par e-mail ou copie-le sur le téléphone, ouvre-le
   dans Chrome / Safari, **en mode avion**) : les images sont là, le texte se lit sans zoomer, « Dessiner » fonctionne au doigt
   sans faire défiler la page, la gomme et « Annuler le trait » aussi, le cadre de note se remplit, « Enregistrer mes notes »
   télécharge `notes-<titre>-<date>.json` (si rien ne se télécharge, le cadre « Le fichier ne s'est pas téléchargé ? » donne le texte).
   Fermer et rouvrir la page : le nom, les notes et les dessins sont retrouvés. Sans nom : le bouton demande le nom.
4. Lecture animée : « Lire le plan » joue les images échantillonnées à la bonne cadence ; le curseur les parcourt.
5. Importer les notes : lâcher le `.json` sur la fenêtre de SHOT (ou Fichier > Importer) ; la fenêtre annonce le bon nombre de notes
   et de plans ; « Importer » : les fanions jaunes apparaissent dans la règle, le panneau Commentaires s'ouvre sur le premier, le
   dessin du relecteur se voit par-dessus l'image quand son commentaire est choisi (et se cache avec l'interrupteur) ; Historique :
   « Importer les notes de relecture » ; Ctrl+Z puis Ctrl+Shift+Z.
6. **Réimporter le même fichier** : « Rien de nouveau », aucun commentaire de plus ; enregistrer, fermer, rouvrir le projet,
   réimporter : toujours rien de plus.
7. Supprimer un plan puis importer : la note de ce plan est « sans plan (plan supprimé) » ; déplacer ou renuméroter des plans
   avant d'importer : les notes retombent sur les bons plans.
8. PDF (Fichier > Exporter > Storyboard PDF) : la case « Avec les commentaires (les non résolus) » (grisée sans commentaire) écrit chaque
   commentaire sous l'image où il tombe, « Commentaire (Anthony) : … » ; sans la case, le PDF est comme avant.
9. Dire si la place du dessin du relecteur (par-dessus l'image, à la caméra) et le jaune des fanions conviennent.

### Essais ajoutés

`relecture_tests.rs` : échappement et page hors ligne (aucune adresse, politique de contenu, un seul script identique pour tous
les projets, titres et textes hostiles), tous les plans et leurs versions, commentaires en contexte, lecture animée (échantillons,
fusion des images identiques), fichier de notes (format, version, UTF-8, limites, identifiants, doublons, 3000 notes, fichier trop
gros), dessins (décodage sous limites, réduction, transparent, refus), fichier écrit par un vrai navigateur, placement par
identifiant / numéro / sans plan / image perdue, import (commentaires, idempotence, rouvrir le projet, dessin abîmé), compte rendu en
français, une seule étape d'annulation nommée, PDF avec commentaires et cadres de notes, tâche d'export (page + PDF, arrêt, fermeture,
fichier d'avant gardé), fenêtres, dessin montré / caché, aucun fichier créé à l'import.

---------------------------------------------------------------------------------------------------

## 25. Notes d'Anthony — calques, effets, montage, export, caméra (2026-10-20, branche `notes-calques`)

Sept demandes d'une même note. Pour chacune : ce qui existait déjà (rien n'est en double), ce qui a changé. Détail :
`README.md` (« Notes d'Anthony du 2026-10-20 »). Rien n'a été regardé à l'écran dans cette séance (aucun lancement de
SHOT) : voir « À vérifier à l'écran ».

### Mots

| Mot | Sens | À éviter |
|---|---|---|
| **Trait ▾**, **Matière ▾** | les deux boutons déroulants du panneau Effets (famille Trait, famille Matière) | « rangée d'icônes » (c'était avant) |
| **Dossier** | un calque sans dessin qui range des calques (un seul niveau) ; son triangle l'ouvre ou le ferme | groupe (le « groupe de couleur » est autre chose) |
| **fx** | le petit bouton de la ligne d'un calque qui allume ou éteint toute sa pile d'effets | |
| **Fond sous le trait** | un réglage **du calque**, avec son bouton sur sa ligne | option du pinceau (c'était avant) |
| **Passe-partout** | le voile autour du cadre de la caméra de « Estomper le hors-champ » | masque |
| **GIF animé** | l'export `.gif`, sans son | |

### Où et quels gestes

| Commande | Où | Raccourci |
|---|---|---|
| Effets du trait, de matière (`fx.ecriture`…, `fx.preset_*`) | panneau Effets : boutons « Trait ▾ » et « Matière ▾ » (icône + nom ; section « Préréglages ») ; « Ajouter un effet » ; clic droit sur un calque > Effets ; Ctrl+K | aucun |
| Nouveau dossier (`dossier.nouveau`) | panneau Calques : icône dossier + de ses commandes, clic droit dans le vide de la liste ; en-tête de la timeline (déjà là) ; menu Calques > Dossiers (déjà là) | aucun |
| Ranger les calques choisis dans un dossier (`dossier.ranger`) | panneau Calques : icône (quand plusieurs calques sont choisis) ; menu Calques > Dossiers | aucun |
| Ouvrir / fermer un dossier | triangle de sa ligne dans le panneau Calques et dans la timeline (le même réglage) | aucun |
| Fond sous le trait du calque (`pinceau.fond`) | bouton carré-utilisateur sur la ligne de chaque calque de dessin du panneau Calques, et dans ses commandes (calque courant) ; menu Calques > Fond sous le trait | aucun |
| Effets du calque allumés / éteints (`calque.effets`) | bouton « fx » de la ligne du calque (panneau Calques, timeline), seulement s'il a des effets ; « Effets du calque allumés » dans les menus de calque ; Ctrl+K | aucun |
| Exporter un GIF animé (`fichier.exporter_gif`) | Fichier > Exporter… > « Un GIF animé » ; fenêtre d'export (choix « GIF animé ») ; Ctrl+K | aucun |
| Opacité du passe-partout | Fichier > Préférences… > Dessin (curseur 0 à 100 %, « Rétablir ») | |

Les icônes ont leur bulle d'une seconde (nom, raccourci, description : `bulles.rs`), un bloc sans touche par action dans `keymap.toml`
(clic droit sur une icône : lui en donner une), les boutons allumés prennent `bulles::encre_etat`, les couleurs viennent de `theme::`.

### Ce que voit l'utilisateur

- **Panneau Effets** : sous « Ajouter un effet / Préréglages » (le bouton « Copier / coller » a disparu le 2026-10-21, partie 31), deux boutons « Trait ▾ » et « Matière ▾ ». Un effet allumé sur
  le calque est enfoncé dans la liste ; le titre compte les effets allumés (« Trait (2) ▾ »). Sur un calque bitmap, les entrées qu'il
  refuse sont grisées et la bulle dit pourquoi. Les préréglages sont sous un petit titre « Préréglages ». Si les effets du calque sont
  éteints (bouton fx), une ligne le dit, avec « Rallumer ».
- **Panneau Calques** : dans les commandes, après « Nouveau calque de fond » : « Nouveau dossier » (et, avec plusieurs calques choisis,
  « Ranger les calques choisis dans un dossier »), puis le bouton de fond et, s'il y a des effets, « fx » pour le calque courant. Chaque
  ligne : [triangle si dossier] œil, cadenas, [fx], vignette (l'icône d'un dossier pour un dossier), nom, puis « vectoriel » (ou « dossier ·
  3 calques ») et le bouton de fond. Un calque rangé est décalé vers la droite ; un dossier fermé n'a plus les lignes de ses calques (sauf le
  calque courant).
- **Timeline** : « fx » sur la deuxième rangée de réglages (après le groupe), ou à côté du cadenas sur une ligne réduite.
- **Pile des versions** : chaque carte porte la durée de sa version à côté du nom (« V2 3s 00 ») ; la bulle d'une carte dit de combien la
  durée change. La fiche du plan et le menu « Version du plan » montrent aussi la durée de chaque version. En passant à une version, la
  barre d'état dit « Durée du plan : 2s 00 → 3s 00 ; les plans suivants reculent de 24 i, le son reste en place. ».
- **Fenêtre d'export, GIF** : « Étendue » (avec « Plans choisis » pour le GIF, les images et le PDF), « Taille » (480 px, 720 px, 50 %, image
  finale, avec les dimensions), « Cadence » (images par seconde, au plus celles du projet, et le nombre d'images du GIF), « Lecture »
  (« Tourne en boucle », « Tramage »), « Transitions », ffmpeg. La progression est celle du film (« Image 12 sur 48 », reste environ,
  Annuler : le GIF inachevé est effacé).
- **Dessin** : le cadre de la caméra est un trait de 1 pixel sur un liseré sombre fin ; « Estomper le hors-champ » voile avec
  l'opacité des Préférences.

### Limites

- « fx » n'a pas de bouton dans la colonne du montage (pas de place) : son entrée de menu y est.
- Un GIF de plans choisis qui ne se suivent pas est fait de leurs images bout à bout, dans l'ordre du film.
- Le contenu rogné (réserve) reste attaché au plan et non à la version : passer à une version de durée différente ne le change pas.
- Les réglages du fond (couleur, débord, trous) restent sous le pinceau (visibles quand le calque courant a un fond), pas sur la ligne du calque.

### Essais ajoutés

`notes_calques_tests.rs` (39 essais) : les deux menus déroulants (rien d'une rangée d'icônes, toutes les entrées et préréglages listés,
une entrée coche l'effet en une étape d'annulation, le titre compte, entrées grisées sans effet sur un bitmap, un préréglage de matière) ;
le panneau Calques (dossier, calques décalés, bouton, triangle qui plie et déplie, calque courant toujours listé, nouveau dossier en une
étape, glisser dedans et dehors, monter un dossier, œil / verrou / opacité) ; le fond (par calque, une étape, refus sur un dossier, bouton
sur chaque ligne, plus de bouton dans les réglages du pinceau, enregistré avec le calque) ; « fx » (une étape, plan, export, bouton
montré seulement avec des effets, peint dans le panneau et la timeline, enregistré, projet d'avant, copie du calque) ; les durées par
version (ondulation des plans suivants, une étape d'annulation, cartes, tête de lecture, export pour le montage, empreinte, comparaison) ;
le GIF (tailles, cadences, images gardées, ligne de commande, plans choisis, fenêtre et action, vrai GIF avec ffmpeg, annulation) ; la
caméra (opacité du passe-partout, réglage enregistré, curseur des Préférences, cadre de 1 pixel, voile suivant le réglage).

### À vérifier à l'écran (par Anthony)

1. Panneau Effets : « Trait ▾ » et « Matière ▾ » côte à côte ; ouvrir « Trait » : six effets avec leurs icônes, « Préréglages » dessous ;
   cliquer « Écriture du trait » : le menu se ferme, le titre devient « Trait (1) ▾ », l'effet est dans la pile ; Ctrl+Z. Sur un calque
   bitmap : les effets du trait sont grisés, la bulle (une seconde) dit pourquoi ; « Matière » : liseré, trait sec, encre y marchent.
2. Panneau Calques : cliquer l'icône « Nouveau dossier » ; glisser deux calques dedans ; le triangle du dossier le ferme (les lignes
   disparaissent aussi de la timeline) puis le rouvre ; le calque courant dans un dossier fermé reste visible ; choisir le dossier,
   baisser son opacité, cacher son œil, verrouiller son cadenas : ses calques suivent ; monter le dossier avec la flèche : ses calques
   aussi. Plusieurs calques choisis (Ctrl + clic) : l'icône « Ranger les calques choisis dans un dossier ».
3. Fond sous le trait : plus de bouton dans les réglages du pinceau ni du crayon ; sur la ligne d'un calque de dessin, le bouton carré-
   utilisateur (éteint) ; un clic le donne au calque (même un autre que le courant), les réglages du fond apparaissent sous le pinceau
   quand ce calque est courant ; Ctrl+Z. Dans le thème Gris clair, l'icône allumée se lit.
4. « fx » : absent sur un calque sans effet ; poser un flou : « fx » apparaît à côté de l'œil et du cadenas (panneau Calques, timeline) ;
   un clic éteint l'effet à l'écran (barré), le panneau Effets le dit ; un clic le rallume ; Ctrl+Z / Ctrl+Maj+Z ; enregistrer, fermer,
   rouvrir : l'état est gardé.
5. Versions : un plan de 2 s, « Nouvelle version », rallonger V2 à 3 s ; survoler « V2 » : chaque carte donne sa durée ; cliquer V1 : la
   bande se resserre, la barre d'état dit de combien, les plans suivants avancent ; Ctrl+Z les repousse ; la tête de lecture ne reste pas
   après la fin d'une version plus courte ; exporter pour le montage : les clips ont les durées de la version affichée.
6. GIF : Fichier > Exporter… > « Un GIF animé » ; plan courant, 480 px, 12 images par seconde, boucle : ouvrir le `.gif` dans un
   navigateur (il tourne en boucle, couleurs correctes, dessin lisible) ; sans tramage : fichier plus léger, aplats plus durs ; le film
   entier : la barre avance, le dessin reste vivant, Annuler efface le fichier ; avec « Plans choisis » (Ctrl + clic dans la bande).
7. Caméra : le cadre est fin (1 pixel) et se lit sur fond blanc, sur fond noir et sur une peinture sombre ; avec l'outil Caméra il est un
   peu plus épais et de la couleur de la caméra ; Fichier > Préférences… > « Opacité du passe-partout » : à 0 % rien n'est voilé, à
   100 % le hors-champ est noir, 27 % retrouve l'aspect d'avant ; le réglage est gardé au prochain lancement. Dire si 27 % convient
   comme valeur d'origine et si la toile infinie (65 % de la valeur) devrait avoir la même opacité que la feuille.
8. Les trois thèmes : les boutons enfoncés des deux menus, de « fx » et du fond restent lisibles.


---------------------------------------------------------------------------------------------------

## 24. Notes d'Anthony : dessin (2026-10-20, branche `notes-dessin`)

Huit notes du réalisateur, lot « dessin ». Pour chacune : ce qui existait, ce qui a été fait, la règle choisie,
les conflits contrôlés. Essais : `src/dessin_notes_tests.rs`.

### 24.1 Tracé droit (Maj tenue pendant un trait)

- **Existait ?** Non pour les pinceaux. Maj servait déjà aux formes (angle par 15° pour la ligne, carré pour le
  rectangle), à l'outil Sélection (ajouter), à la boîte (proportions), au Transformer (axe, 15°), à la molette
  (rotation de la vue) et aux flèches (10 px). Rien pendant un trait de pinceau, de crayon ou de gomme.
- **Règle.** Pendant un trait, Maj tenue : le moteur n'avance plus, un segment droit part du **dernier point
  posé** (le début du trait si Maj est tenue dès l'appui) jusqu'à la pointe ; on le voit en aperçu (ligne à la
  largeur du pinceau, dans sa couleur et son opacité ; gris pour la gomme). **Maj+Alt** : l'angle du segment est
  aligné par pas de 15° (même fonction que la Maj de la ligne, `shapes::snap_angle`). Maj relâchée (ou bouton
  relâché) : le segment est posé en une fois, en points réguliers (un tous les quarts de la taille du pinceau,
  à vitesse de main rapide), puis le tracé libre reprend de son bout. Une seule étape d'annulation, le trait
  entier. Marche en vectoriel (le segment devient un trait modifiable), en bitmap et avec la gomme (vectorielle ou
  bitmap).
- **Choix.** Alt n'est pas tenu à l'appui : Alt + appui est la pipette (tu le tiens pendant le trait seulement).
  Un trait aligné sur un guide (`guides.rs`) est déjà droit : Maj n'y change rien. **Pas fait** : Maj + clic, puis
  Maj + clic ailleurs pour relier deux points (façon Photoshop) ; le segment n'est pas calculé « en direct » avec
  le vrai pinceau (aperçu en ligne), pour ne rien refaire sur le fil de l'interface pendant le geste.
- **Sans latence.** Pendant le geste : une ligne dessinée par image d'écran. La pose du segment est le travail
  d'un trait ordinaire (au plus 3000 points), au relâché, sur le fil de l'interface comme un trait normal.

### 24.2 Ctrl pour choisir un objet, Ctrl+Maj pour en cumuler

- **Existait ?** En partie. Ctrl tenu seul prend l'outil Sélection (le « lasso temporaire », `springtools.rs`) : un
  clic sur un trait vectoriel le choisit. Ctrl+Maj tenus avec l'outil Sélection : « tous les calques » (Storyboard
  Pro), qui **remplaçait** la sélection. Sur un calque bitmap, un clic ne choisissait rien.
- **Règle finale.**

| Geste | Résultat |
|---|---|
| Ctrl + clic sur un trait ou une zone remplie (vectoriel) | l'objet est choisi seul (comme avant) |
| Ctrl + clic sur un dessin bitmap | **la forme d'un seul tenant** sous la souris : les pixels dessinés qui se touchent (huit voisins) ; un trait fin se vise à 4 pixels d'écran près ; le clic remplace la zone |
| Ctrl + clic dans le vide | la sélection est lâchée |
| Ctrl + glisser | lasso ou rectangle, comme avant (seul le clic sans glisser choisit un objet) |
| Maj + clic (outil Sélection) | ajoute ou retire, sur le calque, comme avant ; vaut aussi pour la forme d'un dessin bitmap |
| **Ctrl + Maj + clic** | **ajoute l'objet à la sélection, sur n'importe quel calque du plan, ou le retire s'il y était** (nouveau) |
| **Ctrl + Maj + lasso / rectangle** | **ajoute** tout ce qu'il prend sur tous les calques (avant : remplaçait) |

  La sélection faite sur un seul calque (traits vectoriels, ou zone bitmap pas soulevée) est reprise dans le
  cumul. Ctrl+Maj ne prend jamais une poignée de la boîte (c'est un geste de choix : un trait tout contre le bord
  de la boîte se choisit quand même). Les calques où quelque chose est pris deviennent les calques choisis, et
  leurs clés à l'image courante se choisissent dans le montage (comme avant). Un dessin bitmap lui-même soulevé
  (déplacé, pas posé) est d'abord posé.
- **Conflits contrôlés.** `is_all_layers_gesture` (Ctrl+Maj, sans Alt) ne change pas ; Alt reste la pipette.
  `update_springs` accepte maintenant Maj avec Ctrl pour prendre l'outil Sélection : sans cela, ajouter Maj à Ctrl
  tenue rendait l'outil d'avant, et le clic peignait. Maj seule garde son sens. Les clés (montage) ont leurs propres
  règles de Ctrl / Maj (partie 4) : rien n'y change.
- **Limites.** Une forme bitmap « d'un seul tenant » : deux traits qui ne se touchent pas sont deux objets ; un
  fond peint d'un seul tenant est un seul objet. Les objets d'un calque de couleur ou vidéo ne sont pas choisis.

### 24.3 Un pinceau par sorte de calque

- **Existait ?** Non : un seul pinceau (`App::brush`) pour tous les calques, pas même retenu d'une séance à l'autre.
- **Règle.** SHOT retient le dernier pinceau utilisé sur un calque **vectoriel** et le dernier sur un calque
  **bitmap**. Changer de calque (ou de plan) vers l'autre sorte rend le pinceau retenu pour cette sorte
  (« Pinceau de calque bitmap retrouvé. » dans la barre d'état). Un calque qui n'a jamais servi garde le pinceau du
  moment et le retient. Un dossier ou un calque d'effet ne change rien. Retenus dans les réglages de l'application
  (`[panneau_pinceaux]` : `pinceau_vecto`, `pinceau_bitmap`), ils reviennent au lancement suivant. Le crayon et la
  gomme gardent chacun leur réglage (ils ne changent pas avec le calque).
- Le pinceau retenu est le réglage complet (taille, opacité, dynamiques…) tel qu'il était, retouches comprises, pas
  seulement le nom de la brosse.

### 24.4 Enregistrer les réglages d'une brosse

- **Existait ?** En partie, caché : « Remplacer par le réglage courant » (clic droit sur une brosse de la
  bibliothèque) et le bouton + (nouvelle brosse). Rien dans le panneau Réglages.
- **Fait.** Une icône disquette à droite du nom du réglage, dans le haut du panneau Réglages de l'outil, ouvre un
  petit menu : **Enregistrer dans « X »** (remplace le réglage de la brosse d'où vient le réglage courant ; grisé si
  rien n'a changé ou si la brosse est une brosse de SHOT) ; **Enregistrer comme nouvelle brosse…** (la ligne du nom
  se montre dans le panneau, OK / Annuler) ; **Rétablir l'ancien réglage de « X »** (juste après un « Enregistrer
  dans », tant que la brosse a encore ce réglage). Quand le réglage a été retouché à la main, le nom est
  « (réglé à la main) » suivi de « d'après « X » » (la dernière brosse choisie dans la bibliothèque ou gardée sous
  un nom) : c'est elle que l'icône remplace. L'enregistrement dans la bibliothèque n'est pas dans l'historique du
  projet (la bibliothèque est à l'application) : d'où le « Rétablir ».

### 24.5 Réglages repliés, fond de l'aperçu

- **Repliés.** Dans le panneau Réglages du pinceau, tout ce qui est **sous l'aperçu du trait** (les catégories et
  leurs réglages) est replié derrière une petite flèche (`▸` replié, `▾` déplié, bulle « Déplier les réglages du
  pinceau »). L'état est retenu (`reglages_ouverts`). Ce qui est au-dessus (taille, opacité, nom, enregistrer) reste
  visible.
- **Fond.** Le fond de l'aperçu n'est plus le papier blanc du projet : c'est `theme::apercu_trait`, un gris un peu
  plus clair que le panneau (le panneau mélangé de 12 % de blanc dans les thèmes sombres, de 55 % dans le thème
  clair), donc pour les trois thèmes. Si la couleur du trait lui ressemble (une brosse grise, la gomme), le fond est
  poussé vers le blanc ou le noir, celui qui fait le mieux ressortir le trait, jusqu'à un contraste de 1,8:1
  (`theme::fond_apercu_trait`). Les vignettes et lignes de la bibliothèque gardent le papier du projet.

### 24.6 Remplir la sélection bitmap (Maj+Suppr)

- **Existait ?** Pas le remplissage. **Maj+Suppr n'était pas libre** : elle était à « Effacer ce qui est hors de la
  sélection » (`dessin.supprimer_hors_selection`, `DRAWING_DELETE_NON_SELECTION_KEY` de Storyboard Pro). Choix : la
  nouvelle commande `selection.remplir` prend Maj+Suppr ; **sans zone bitmap choisie, Maj+Suppr fait encore l'ancien
  geste** (vectoriel : garder seulement la sélection), et « effacer hors de la sélection » garde Maj+Retour arrière
  (son second raccourci de Storyboard Pro). Seul le cas « zone bitmap + Maj+Suppr » change. Les deux jeux de
  raccourcis (SHOT, façon Storyboard Pro) sont pareils.
- **Règle.** La zone prend la couleur courante (bord anticrénelé mélangé), une étape d'annulation « Remplir la
  sélection ». Un calque verrouillé (lui, son dossier ou sa position) refuse avec son message ; « préserver la
  transparence » ne change que la couleur de ce qui est déjà dessiné (le vide reste vide, l'opacité reste) ; les
  pochoirs des autres calques limitent le remplissage comme le pinceau. Une zone soulevée (déplacée, pas posée) : le
  message dit de la poser d'abord. L'icône « Remplir » (seau) est dans les réglages de l'outil Sélection.

### 24.7 Clés de position (outil Transformer)

- **Existait ?** La transformation d'un calque (`Layer::transform` : décalage, échelle, étirement, rotation) était
  **fixe**, sans clé. Les clés de caméra (`camera.rs`, `camkeys.rs`) et d'effets (`fx.rs`, `fxkeys.rs`) existaient.
- **Cible choisie : le calque entier** (comme un « peg »). Un calque a des **clés de position**
  (`Layer::pos_keys`, `poskeys.rs`) : à l'image `n` du plan, sa pose vaut telle valeur. Entre deux clés la pose va de
  l'une à l'autre (décalage, échelle, étirement et rotation en ligne droite), avant la première et après la dernière
  elle ne bouge pas. **Lisser** reprend le modèle des clés de caméra : `smooth` (arrive et repart lentement sur un
  demi-intervalle) ou des adoucis d'entrée et de sortie en images, avec `camera::ease`.
- **Outil Transformer, réglages > « Clés de position ».** Quatre icônes : **Clé de position** (pose une clé à
  l'image courante avec la pose actuelle ; avec plusieurs calques choisis, une clé chacun), **Lisser la clé**
  (bascule sur la clé d'ici), **Supprimer la clé d'ici**, **Supprimer toutes les clés** (le calque garde la pose de
  l'image courante). Avec la clé d'ici : « arrivée » et « départ », les adoucis en images. Trois commandes sans
  touche par défaut dans le fichier : `calque.cle_position`, `calque.cle_position_lisser`,
  `calque.cle_position_supprimer`, `calque.cles_position_supprimer`.
- **Animer.** Un calque qui a des clés est animé : le déplacer, l'agrandir, l'étirer ou le tourner à une image (poignées,
  champs x / y / largeur / hauteur / angle, déplacement dans la vue 3D de la scène) **pose ou met à jour la clé de cette
  image** ; le geste est une étape d'annulation, comme avant. Ctrl+R remet la clé de l'image courante à la pose
  d'origine. Un miroir (calque ou plan) retourne toutes les clés. Les calques de couleur suivent les clés de leur trait.
  Sans clé, rien ne change : le calque a sa pose fixe.
- **Dans la timeline (animation).** Des **losanges verts** au milieu de la ligne du calque (des **ronds** quand la clé
  est adoucie, cerclés de blanc quand elle est choisie) : un clic choisit la clé et y met la tête de lecture (Ctrl :
  ajouter ou retirer, Maj : ajouter celles d'entre la dernière cliquée et celle-ci) ; glisser la déplace (avec les autres
  clés choisies du calque, bloquées dans le plan, une étape « Déplacer des clés de position ») ; Suppr supprime les
  clés choisies ; clic droit : lisser, supprimer. La main (curseur « Grab ») se montre au survol.
- **Dans le montage (Anthony, 2026-10-20 : « ajoute le glisser dans le montage »).** Les mêmes losanges, **au milieu de
  la piste de chaque calque de chaque plan** (la bande des losanges violets d'effets reste en haut, `fxkeys::STRIP` :
  les deux ne se recouvrent pas), avec les mêmes gestes (`montage_pos_key_at` dans `montagerows.rs`, branchés dans
  `montage_strip`, les aides communes dans `poskeysui.rs` : `marker_near`, `choose_pos_marker`, `begin_pos_drag`).
  - **Priorité.** Un appui sur un losange vert passe avant tout ce qui est dessous : poignées du dessin (bords, limite
    entre deux dessins, barre, glisser d'un dessin par son milieu), choix au rectangle (même avec Ctrl ou Maj), choix
    des plans, double-clic qui pose une clé de dessin. Partout ailleurs les gestes du montage sont ceux d'avant ; un
    clic ou un geste qui ne part pas d'un losange vert **lâche** le choix des clés de position. La poignée du dessin
    ne s'allume plus sous un losange.
  - **Clic** : la clé est choisie (Ctrl : ajouter ou retirer ; Maj : ajouter celles d'entre), **la tête de lecture va
    sur la clé** (image du film), son plan et son calque deviennent courants, les clés de dessins, de caméra et d'effets
    choisies sont lâchées et Suppr vise les clés de position (`key_focus`). Les clés choisies sont **d'un seul plan** :
    en prendre une dans un autre plan lâche le choix d'avant.
  - **Glisser** : les clés choisies du calque suivent la souris, à l'image entière (accrochées aux images), sans sortir du
    plan (le décalage est limité par la clé la plus proche d'un bord, le groupe garde ses écarts), même si la souris
    sort du plan. Une clé qui arrive sur une autre du calque **la remplace** (règle de la timeline, `poskeys::moved`) ;
    l'annulation rend les deux. Un losange pris hors du choix devient le choix. Une seule étape d'annulation,
    « Déplacer des clés de position » ; Échap pendant le geste remet les clés comme avant, sans rien enregistrer
    (`cancel_drags`, qui sert aussi à la timeline). Storyboard verrouillé : refusé, comme les autres gestes.
  - **Suppr** supprime les clés de position choisies quand elles ont la main (jamais les dessins) ; **clic droit** sur un
    losange : `pos_key_menu` (lisser, supprimer, poser une clé ici, supprimer toutes les clés du calque), un losange non
    choisi l'est seul.
  - **Bulle** (1 s, réglage général des bulles) : « Clé de position, image N (calque) », glisser pour la déplacer,
    Ctrl / Maj + clic, Suppr, clic droit.
  - **Rien de lourd à chaque image** : la recherche d'un losange ne se fait que sous la souris (les plans à l'écran, les
    clés du calque de la rangée visée) ; la timeline et le montage partagent `marker_near`.
- **Partout où l'image est fabriquée** : l'affichage, la lecture, l'export (film, images, PDF), les vignettes, la pile
  des versions, la vue 3D du plateau (multiplan) et la vue de dessus lisent la pose **à l'image demandée** (`Layer::pose`,
  `transform_seen_at`). Les clés suivent les images insérées ou retirées, la coupe d'un plan (la pose de la coupe est
  gardée de chaque côté, comme les clés de caméra), la réunion de deux plans, le changement de taille de la feuille.
- **Format.** `VERSION` passe de 32 à **33** : `cles_position` d'un calque (liste de clés : `image`, `x`, `y`, et si
  différents de leur défaut `echelle`, `rotation`, `etirement_x`, `etirement_y`, `lisse`, `adoucir_entree`,
  `adoucir_sortie`). Une version précédente refuse le projet (elle le montrerait immobile). Un projet de version 32 ou
  moins s'ouvre tel quel, sans clé. **Fusion avec l'autre lot de notes** : si ce lot a aussi besoin du 33, la fusion
  garde `VERSION = 33` et réunit les deux descriptions dans le commentaire de `project.rs`.
- **Limites.** Pas de courbe de vitesse libre (adoucis seulement) ; pas de clé sur le pivot ni sur l'opacité ; la
  propagation façon EbSynth (`propager*.rs`, non modifié) lit la pose fixe du calque ; les calques « toute la
  scène » ont leurs clés propres à chaque plan ; pas de copier / coller de clés de position ; dans le montage, le choix
  ne passe pas d'un plan à l'autre (un seul plan à la fois) ; glisser un dessin ou une limite entre deux dessins pousse
  la suite du calque **avec ses clés de position** (comme avant).

### 24.8 À regarder à l'écran

1. Pinceau, bitmap : tracer, tenir Maj en plein trait : un segment droit apparaît entre le dernier point et la croix ;
   relâcher : il est posé. Maj dès l'appui : de l'appui à la croix. Maj+Alt : l'angle saute par 15°. Même chose avec la
   gomme et sur un calque vectoriel (le trait reste modifiable avec l'outil Sélection). Ctrl+Z défait tout le trait.
2. Deux taches bitmap : Ctrl + clic sur l'une (le contour la cerne), Ctrl+Maj + clic sur l'autre (les deux sont
   cernées), Ctrl+Maj + clic encore sur la seconde (elle est retirée). Sur deux calques vectoriels : pareil, les deux
   calques deviennent les calques choisis.
3. Pinceau « encre » sur un calque vectoriel, passer sur un calque bitmap et choisir « gouache », revenir : « encre » ;
   revenir au bitmap : « gouache » ; fermer et rouvrir SHOT : même chose.
4. Réglages du pinceau : changer la taille de la brosse « Encre » : « (réglé à la main) d'après « Encre » » ; la
   disquette : « Enregistrer dans « Encre » » est grisé s'il s'agit d'une brosse de SHOT (propose « comme nouvelle ») ;
   avec une brosse de « Mes brosses », il remplace, puis « Rétablir l'ancien réglage ».
5. Sous l'aperçu du trait : une petite flèche, tout est replié ; la déplier, fermer et rouvrir SHOT : l'état est retenu.
   L'aperçu est sur un gris un peu plus clair que le panneau dans « Gris sombre », « Gris clair » et « Sombre » ; choisir
   un gris moyen comme couleur : le fond s'éclaircit ou s'assombrit pour garder le trait lisible.
6. Lasso (outil Sélection) sur un calque bitmap : Maj+Suppr remplit de la couleur courante ; Ctrl+Z défait ; sur un
   calque verrouillé : refusé avec son message ; avec « préserver la transparence » : seul le dessin change de couleur.
7. Outil Transformer : « Clés de position » dans les réglages. Poser une clé à l'image 1, aller à l'image 12, tirer le
   calque : une clé naît, la lecture le déplace d'une clé à l'autre ; losanges verts dans la timeline ; cliquer sur un
   losange, le glisser, Suppr ; clic droit > Lisser : le losange devient rond et le mouvement ralentit à ses bouts ;
   exporter les images : le calque bouge dans l'export. Dans le montage : les mêmes losanges verts au milieu de la piste
   du calque, de chaque plan ; la main au survol puis la bulle ; cliquer (la tête de lecture y va), Ctrl / Maj + clic,
   glisser sur le dessin (le dessin ne bouge pas), tirer au-delà de la fin du plan (bloqué), Échap pendant le geste,
   Suppr, clic droit ; les losanges violets d'effets, juste au-dessus, se prennent toujours à part.

## 26. Fiche technique : le découpage du film en tableur (2026-10-20, branche `fiche-technique`)

Demande d'Anthony : une fiche de découpage pour la production, le layout et le montage, une ligne par plan (ou par image
de storyboard), qui s'ouvre dans Excel, LibreOffice et Google Sheets. Code : `src/fiche.rs` (colonnes, contenu,
mouvement de caméra), `src/ficheformats.rs` (CSV, presse-papiers, XLSX), `src/zipwrite.rs` (l'archive du XLSX),
`src/fichejob.rs` (la tâche), `src/ficheui.rs` (la fenêtre). Essais : `src/fiche_tests.rs`. **Aucun changement de format
du projet** (`VERSION` reste 33).

### 26.1 Où et comment

- **Où** : Fichier > Exporter… > « Fiche technique (tableur) » (icône `TABLE`) ; « Plus… » de la barre du montage ;
  Ctrl+K « Exporter la fiche technique » ; action `fichier.exporter_fiche` (un bloc dans `keymap.toml`, **sans touche**).
- **La fenêtre** : *Plans* (tout le film, la séquence courante, les plans choisis, seulement les plans modifiés depuis un
  envoi : les mêmes choix que « Exporter pour relecture ») ; *Envoi* (l'envoi de référence, s'il y en a) ; *Lignes* (une par
  plan, ou une par image de storyboard) ; *Time code de départ* (01:00:00:00, rouge s'il est illisible) ; *Format*
  (classeur XLSX ou CSV, et pour le CSV point-virgule ou virgule) ; *Colonnes* (cases, flèches pour monter et descendre, tout
  cocher, tout décocher, colonnes d'origine ; une bulle d'une seconde dit ce que contient chaque colonne). Dessous, ce qui
  partirait (« 12 plans, 12 lignes, 15 colonnes, 12 vignettes à dessiner »), puis deux icônes : **Exporter la fiche…**
  (choisir le fichier, écrire en tâche de fond) et **Copier dans le presse-papiers**. Colonnes, ordre, format, séparateur,
  time code et dernier dossier sont retenus d'une séance à l'autre (`reglages.toml`, `[fiche_technique]`).
- **Pendant l'export** : une barre « vignette n / total », le temps restant, la croix « Arrêter » (efface tout ce qui a
  été commencé). Le projet n'est retenu que le temps de la copie (quelques images d'écran), puis libre ; on peut changer
  d'onglet, la fiche continue (§ 28).
- **À la fin** : « Terminé : 12 lignes, 12 vignettes », le chemin et la taille, le dossier des vignettes, les
  avertissements, et l'icône qui montre le dossier dans l'explorateur.

### 26.2 Les colonnes (29, choisies et ordonnées par l'utilisateur)

| Colonne | Contenu |
|---|---|
| Séquence | nom ou numéro de la séquence |
| N° de plan | le numéro de la numérotation (`12A` pour un plan inséré en numérotation verrouillée) |
| Référence | le nom de clip de l'export pour le montage : `SQ01_PL012A`, avec `_V2` si le plan a plusieurs versions |
| Version | `V2` quand le plan a plusieurs versions (la version affichée), sinon vide |
| Nom du plan | le nom donné au plan, s'il en a un |
| Image | « 2 / 3 » en mode « une ligne par image de storyboard » ; vide sinon |
| Vignette | chemin `vignettes/SQ01_PL012A.png` ; dans le XLSX, l'image dans la cellule |
| Durée (images), Durée (s), Durée (TC) | **nombres** pour les deux premières (on calcule dessus), time code en texte pour la troisième |
| TC entrée, TC sortie | time codes du **film entier** (un plan garde son time code même si l'on n'écrit que quelques plans) ; la sortie est l'image qui suit le plan |
| Cadrage | le nom de l'objectif si c'en est un de la liste de la caméra (« Portrait · 85 mm »), sinon « 52,5 mm » ; au début du plan |
| Mouvement de caméra, Détail du mouvement | voir 26.3 |
| Images de storyboard | le plan et ses marques d'image |
| Action, Dialogue, Notes | les textes du plan (ceux de l'image de storyboard en mode « par image ») |
| Transition d'entrée, de sortie | « Fondu enchaîné (12 i) » : la transition posée sur la coupe qui précède / suit |
| Sons | les noms des sons posés sur les pistes pendant le plan (toutes les pistes, séparés par « ; ») |
| Commentaires ouverts, Texte des commentaires | le nombre de commentaires **pas résolus**, puis leur texte avec l'auteur (un par ligne) |
| Modifié depuis l'envoi, Dernier envoi | « Modifié » ou « Ajouté » au regard de l'envoi de référence (vide : inchangé) ; son nom |
| Couleur du plan | la couleur de repère |
| Calques, Effets | nombre de calques (sans les dossiers) ; effets allumés (« Flou gaussien × 2; Lueur ») |

Colonnes cochées d'office : Séquence, N° de plan, Version, Nom du plan, Vignette, Durée (images), Durée (s), TC entrée,
TC sortie, Cadrage, Mouvement de caméra, Action, Dialogue, Notes, Transition d'entrée et de sortie, Sons, Commentaires
ouverts, Modifié depuis l'envoi.

### 26.3 Mouvement de caméra : les règles

Déduit des images clés de la caméra du plan (de la part du plan qui est l'image de storyboard, en mode « par image »).
Entre deux clés chaque réglage va de l'un à l'autre sans détour : on ne regarde donc que la caméra au début, à la fin
et à chaque clé. Un réglage « bouge » quand son **étendue** (le plus grand moins le plus petit) dépasse un seuil ; son
sens est celui du début à la fin, ou « aller-retour » s'il revient presque à son point de départ.

| Réglage de la caméra | Seuil | Mot |
|---|---|---|
| X (la caméra glisse sur la feuille) | 1 % de la largeur du cadre | travelling à droite / à gauche |
| Y | 1 % de la hauteur du cadre | travelling vers le haut / vers le bas |
| Z (avancée vers la feuille) | 2 % de la distance de référence | travelling avant / arrière |
| Panoramique (la caméra tourne sur place) | 0,5° | panoramique à droite / à gauche |
| Bascule | 0,5° | bascule vers le haut / vers le bas |
| Zoom (focale) | rapport de 1,02 | zoom avant / arrière |
| Rotation (roulis) | 0,5° | rotation horaire / antihoraire |

Rien ne bouge (ni clé, ni une seule, ni deux clés égales, ni un tremblement sous les seuils) : « fixe ». Plusieurs
réglages : leurs mots dans l'ordre du tableau, séparés par « + » (« panoramique à droite + zoom avant ») ; X et Y ensemble
font un seul mot (« travelling à droite et vers le bas »). **Choix à confirmer** : un glissement de la caméra sur la feuille
(X, Y) est écrit « travelling », et seul le réglage « panoramique » de la caméra (qui tourne sur place, sans parallaxe)
est écrit « panoramique ». Si en animation vous appelez « panoramique » le glissement sur un décor, c'est un tableau à
changer à un seul endroit (`Mouvement::label`, `fiche.rs`). La colonne de détail dit, pour chaque réglage qui bouge, sa
valeur au début et à la fin (« X : 960 → 1200 px ; Focale : 35 → 49 mm »).

### 26.4 Formats

- **CSV** : UTF-8 **avec BOM** (Excel lit les accents), fins de ligne `\r\n`, séparateur `;` (Excel en français, d'office)
  ou `,`. Un champ est entre guillemets s'il contient le séparateur, un guillemet ou un retour à la ligne (le guillemet
  est doublé, le retour à la ligne est gardé dans la cellule). Les décimales prennent la virgule avec `;` (Excel en
  français les lit comme des nombres) et le point avec `,`. Les textes ne sont jamais raccourcis.
- **Formules (injection CSV)** : un tableur évalue une cellule de texte qui commence par `=`, `+`, `-`, `@` (ou une
  tabulation, un retour chariot). **Choix : une apostrophe `'` est ajoutée devant** ces textes dans le CSV et dans le
  presse-papiers (elle se voit : `'- à refaire`, c'est voulu, la protection ne se cache pas). Les nombres écrits par
  SHOT ne sont pas du texte et ne sont pas touchés. Dans le XLSX les textes sont des chaînes que le tableur ne lit jamais
  comme une formule : rien n'y est ajouté (essayé dans Excel : `=1+1` reste du texte).
- **XLSX** : un classeur Office Open XML écrit à la main puis zippé (aucune caisse de plus). Une feuille « Fiche
  technique » ; en-tête en gras sur fond gris, **figé** (la première ligne reste en haut) avec **filtre automatique** ;
  largeurs de colonnes ; texte qui passe à la ligne dans les colonnes de texte ; durées en nombres, time codes en texte ;
  chaînes partagées (le texte est échappé, les caractères interdits en XML sont retirés) ; mise en page paysage, une page
  de large. Une cellule XLSX tient 32 767 caractères : au-delà, le texte est coupé dans le classeur (avec « … ») et la
  fenêtre le dit ; le CSV garde tout.
- **Vignettes** : un PNG par ligne dans `vignettes/` **à côté de la fiche** (`SQ01_PL012A.png`, `SQ01_PL012A_img02.png`
  pour la 2e image de storyboard), 320 pixels de large au plus, la première image du plan (ou de l'image de storyboard),
  sur fond blanc. Dans le XLSX, chaque image est posée dans sa cellule (160 × 90 pixels au plus, proportions gardées,
  hauteur de ligne réglée) ; le chemin reste écrit dans la cellule, dessous. Une image répétée n'est écrite qu'une fois
  dans le classeur.
- **Presse-papiers** : texte séparé par des tabulations (en-tête d'abord, champs entre guillemets comme les tableurs les
  collent), décimales à la virgule, formules neutralisées, **sans la colonne des vignettes** (les fichiers n'existent pas).
  Rien n'est copié avant le clic sur le bouton.

### 26.5 Sécurité de l'écriture

- **Atomique** : la fiche et chaque vignette sont écrites sous un nom en `.part` puis renommées. Un export arrêté, un
  disque plein ou un fichier verrouillé ne laissent ni demi-fichier, ni fiche d'avant abîmée.
- **Fichier ouvert dans Excel** : Excel tient le classeur verrouillé. SHOT essaie d'ouvrir la fiche existante en
  écriture **avant** de commencer ; s'il ne peut pas : « « fiche.xlsx » est ouvert dans un autre programme (Excel ?) ou
  protégé en écriture : ferme-le puis recommence. Rien n'a été modifié. », sans rien écrire. Si le fichier se verrouille
  pendant l'export, le renommage final échoue avec le même message, la fiche d'avant est intacte, les `.part` et les
  vignettes posées par cet export sont effacés.
- **Arrêter / fermer SHOT** : efface les `.part` et les vignettes que l'export a posées (une vignette d'une fiche
  précédente, remplacée, n'est jamais effacée).
- **Zéro latence** : aucune vignette ne se dessine sur le fil de l'interface (le projet est copié au départ, les images se
  calculent sur d'autres fils, § 28) ; les PNG, le classeur et les fichiers sont écrits à côté. Sans colonne « Vignette »,
  rien n'est dessiné.

### 26.6 Limites

- **Vérifié ici** : Excel (installé sur ce poste) a ouvert un classeur d'essai sans réparation, avec les images dans
  leurs cellules, la première ligne figée, le filtre, l'en-tête en gras, les durées en nombres et `=1+1` resté du texte.
  **Pas vérifié** : LibreOffice et Google Sheets (absents). Google Sheets ignore en général les images posées sur une
  feuille importée : le chemin de la vignette reste alors dans la cellule.
- Le cadrage est celui du **début** du plan ; la colonne « Détail du mouvement » dit la focale du début et de la fin.
- Un plan sans clé de caméra garde celle que le plan d'avant (dans la même scène) a laissée (règle de `panel_camera`).
- Les vignettes sont la première image du plan : pas de choix de l'image.
- « Modifié depuis l'envoi » attend que le relevé des plans soit prêt (quelques secondes après l'ouverture du projet) ; la
  fenêtre le dit.
- Les time codes sont sans saut d'images (comme l'export pour le montage), à la cadence du projet.
- Pas de formules ni de mise en forme conditionnelle dans le classeur : c'est une photo du film à l'instant de l'export.

### 26.7 À regarder à l'écran (liste pour Anthony)

1. Fichier > Exporter… : la ligne « Fiche technique (tableur) » ; la fenêtre s'ouvre ; passer la souris une seconde sur
   une colonne, sur les icônes de format et sur les deux icônes du bas : les bulles.
2. Décocher « Notes », monter « Mouvement de caméra » avant « Action » avec les flèches ; exporter en **XLSX** avec les
   vignettes : la barre avance, le projet ne réagit plus pendant les vignettes (c'est voulu) puis redevient libre.
3. **Ouvrir le .xlsx dans Excel** : pas de message de réparation ; la ligne 1 en gras, grise, restée en haut quand on
   défile ; le filtre sur chaque colonne ; les vignettes dans leurs cellules (pas coupées, pas décalées) ; les durées sont
   des nombres (les additionner : la barre d'état d'Excel donne la somme) ; les time codes restent « 01:00:00:00 » ;
   les textes à plusieurs lignes se lisent ; un dialogue qui commence par `=` ou `-` reste du texte.
4. **Ouvrir le même fichier dans LibreOffice Calc et dans Google Sheets** (import) : mêmes colonnes et mêmes valeurs ; dire
   si les vignettes apparaissent (Google Sheets les perd d'ordinaire : le chemin reste lisible).
5. Exporter en **CSV point-virgule** et l'ouvrir en double-cliquant dans Excel français : accents justes, une colonne par
   rubrique, « 0,5 » reconnu comme nombre ; en **CSV virgule** dans Google Sheets (Fichier > Importer) : pareil.
6. Un plan en « V2 » : la colonne Version, la référence `…_V2`. Un plan inséré en numérotation verrouillée : « 12A ».
7. Un plan avec un panoramique, un zoom, un travelling : les mots de « Mouvement de caméra » et les valeurs du détail
   correspondent à ce que la caméra fait ; dire si « travelling » (glissement) et « panoramique » (rotation) sont les bons
   mots pour vous (26.3).
8. « Une ligne par image de storyboard » sur un plan à plusieurs marques d'image : une ligne par image, ses textes, ses
   time codes, sa vignette.
9. « Seulement les plans modifiés depuis l'envoi » après avoir retouché un plan envoyé : une ligne ; la colonne
   « Modifié depuis l'envoi » dit « Modifié ».
10. **Excel ouvert sur la fiche pendant l'export** : choisir le même fichier, ouvert dans Excel, puis « Exporter » : le
    message « ouvert dans un autre programme (Excel ?) » apparaît tout de suite et rien n'est écrit. Fermer Excel, refaire.
11. Arrêter l'export en cours : aucun fichier ni dossier « vignettes » laissé, la fiche d'avant intacte.
12. « Copier dans le presse-papiers », puis coller dans la case A1 d'un tableur : les colonnes se répartissent ; rien n'est
    dans le presse-papiers avant ce clic.
13. Sur un vrai projet de plusieurs dizaines de plans avec vignettes : le temps de l'export (la barre dit le temps
    restant) et la taille du dossier `vignettes/`.

## 28. Exports : le projet reste libre pendant qu'ils tournent (2026-10-20, branche `exports-fond`)

Demande d'Anthony : « un logiciel qui réponde toujours très rapidement, sans latence ». Avant, le film, les images, le GIF, le PDF,
le montage, la relecture, la fiche et l'export vers Storyboard Pro calculaient leurs images sur le fil de l'interface (40 ms par
image d'écran, projet verrouillé) ; la page HTML, l'image courante et le son seul gelaient SHOT. Maintenant, aucune image ne se
calcule sur le fil de l'interface (mesures : `docs/PERFORMANCE.md` § 17).

- **Ce qui change à l'écran.** Après « Exporter… » et le choix du fichier, une petite fenêtre « Export en cours » apparaît en
  bas à droite, **sans rien bloquer** : phase (« Copie du projet… », « Mixage du son… »), barre d'avancement, « image n sur
  total », temps restant, et la phrase « L'export utilise le projet tel qu'il était au départ : tu peux continuer à
  travailler. » Les boutons sont des icônes avec leur bulle d'une seconde : **Arrêter** (croix ; efface ce qui n'est pas fini),
  puis, à la fin, **Montrer dans l'explorateur** et **Fermer**. La fenêtre de choix rappelle la même chose sous « Exporter… ».
- **Le projet est libre.** On peut dessiner, changer de plan, annuler, enregistrer, ouvrir un autre projet pendant l'export. Ce
  qu'on retouche n'y entre pas (il utilise la copie du départ). Le projet n'est retenu que **le temps de la copie** (quelques
  images d'écran, 8 ms au plus chacune) : pendant ces instants, comme avant pendant tout l'export, les raccourcis sont suspendus
  et on ne change pas d'onglet (« Attends la fin de la copie du projet pour l'export »).
- **Les onglets.** Un export qui tourne continue quand on change d'onglet ou qu'on ouvre un autre projet ; la fenêtre ajoute
  « · « nom du projet » » quand on regarde un autre onglet. Il en va de même pour l'export pour le montage, la fiche technique,
  la page de relecture (suivie dans son onglet caché, sa fin est dite dans la barre d'état) et l'export vers Storyboard Pro.
- **Un seul export à la fois** pour le film, les images, le GIF, le PDF, la page HTML, l'image courante et le son : une
  deuxième demande dit « Un export est déjà en cours ».
- **Quitter.** « Quitter » (ou la croix de la fenêtre) pendant un export le dit : « Quitter arrêterait l'export : attends sa
  fin, ou refais « Quitter » pour l'arrêter. » Un deuxième « Quitter » dans les quinze secondes quitte et arrête l'export (la
  vidéo inachevée est effacée).
- **Annuler** arrête les fils tout de suite (ffmpeg est tué) et efface ce qui n'est pas fini : le film ou le GIF inachevé ; le
  `.part` du PDF, de la page, de l'image, du son ; les planches de la page HTML. **Un PDF, une page ou un son d'avant, sous le
  même nom, n'est jamais touché** : ils s'écrivent sous un nom en `.part` puis sont renommés. Les images déjà écrites d'une suite
  PNG restent (comme avant).
- **Traits texturés en rendu rapide** (Préférences > Performances, « Rapide partout ») : l'export les refait exacts dans sa
  copie, sur un autre fil. Avant, SHOT attendait au moment de cliquer « Exporter ».
- **Priorité.** Les fils de l'export (et ffmpeg) passent **sous la priorité de l'interface**, et laissent deux cœurs libres :
  un export n'a de cœurs que ceux dont l'interface n'a pas besoin.

### À regarder à l'écran (exports)

1. **Film en 4K** (Fichier > Exporter > Film) sur un projet avec des effets : pendant l'export, déplacer la tête de lecture,
   dessiner un trait, zoomer, changer de plan, changer d'onglet : tout répond sans à-coup. La fenêtre « Export en cours » avance.
2. **Le film est celui du départ** : dessiner en gros un trait rouge sur le plan 1 pendant que le film s'encode ; le film fini
   ne contient pas le trait.
3. **Annuler** pendant le film : la fenêtre dit « Export annulé », le fichier .mp4 n'existe pas, et un ancien .mp4 du même nom
   a été remplacé seulement si l'export avait fini. Même chose pour le PDF : annuler laisse l'ancien PDF intact.
4. **Deux projets** : lancer l'export du projet A, ouvrir B dans un autre onglet : la fenêtre dit « · « A » » ; l'export finit ;
   « Montrer dans l'explorateur » ouvre le bon dossier.
5. **Quitter pendant l'export** : le message de la barre d'état au premier « Quitter », la sortie au deuxième.
6. **Image courante**, **page HTML** et **son seul** : plus de gel de SHOT ; un seul clic sur « Fermer » à la fin.
7. **Montage, fiche technique, relecture, Storyboard Pro** : la même fenêtre qu'avant, mais l'interface répond pendant les
   images ; changer d'onglet ne les arrête pas.
8. **Mémoire** (Gestionnaire des tâches) : un film 4K ne fait pas grimper SHOT de plus d'un gigaoctet environ ; la mémoire
   redescend à la fin.

## 27. Travailler à plusieurs (2026-10-20, branche `cooperatif`)

Conception : `docs/COOPERATION.md`. Rien ne change pour un projet ordinaire ; tout ce qui suit n'apparaît que dans un **film partagé**.

### 27.1 Partager

- **Fichier > Partager ce projet…** : une boîte pour choisir où créer le film (le dossier Dropbox / Drive / réseau), puis une fenêtre « Partage »
  avec une barre d'avancement. Le projet ouvert reste tel quel ; le film partagé s'ouvre dans un **nouvel onglet**. (Dans un film déjà partagé, la
  même entrée devient **Équipe…**.)
- **Enregistrer sous…** d'un film partagé en fait une copie ordinaire, à plat, qui n'est plus partagée.

### 27.2 Ce qu'on voit

- **Pastilles** en haut à gauche de chaque plan, dans la bande du montage et sur la planche : un rond à la couleur de celui qui tient le plan, ses
  initiales dedans. À toi : liseré blanc (orange pendant la confirmation de la prise ou la vérification du fichier). Rond **creux** : la prise est
  périmée (sans nouvelles). **Sablier** gris : le fichier du plan, ou un de ses dessins, n'est pas encore arrivé. Petits ronds creux dessous : ceux qui regardent
  ce plan. Sur un bloc trop étroit, une bande à la couleur du porteur le long du bas. La survoler une seconde dit la phrase complète.
- **Barre d'état**, à droite : l'icône « équipe » et le nombre d'autres personnes en ligne ; un triangle d'avertissement s'il y a des brouillons ou des
  copies en conflit ; rouge si le dossier partagé ne répond pas. Un clic ouvre la fenêtre Équipe.
- **Fenêtre Équipe** (Fichier > Équipe…, `equipe.fenetre`) : les icônes prendre / rendre le plan, la séquence, le montage, tout rendre ;
  la liste des équipiers (en ligne ou non, ce qu'ils regardent, ce qu'ils tiennent) ; le dossier ; les plans pas encore arrivés ; les prises qui attendent
  leur fichier (icône « prendre tel quel ») ; les **copies en conflit** (importer comme version / ignorer) ; les **brouillons** (importer comme version) ; les nouveautés.

### 27.3 Prendre et rendre

- Clic droit sur un plan (montage ou planche) : **Prendre ce plan** (ou **Reprendre**, avec confirmation, quand la prise est périmée),
  **Rendre ce plan**, **Prendre la séquence**, **Prendre / Rendre le montage**. Rendre enregistre d'abord.
- Modifier un plan qu'on ne tient pas : refusé, la barre d'état dit à qui il est (« Plan 3 est à Léa. Tu peux le regarder, le jouer et le commenter. »),
  ou « Plan 3 est libre : clic droit > Prendre ce plan ». Changer l'ordre, le nombre, la durée, les noms ou le son sans le jeton : « Il faut le jeton du
  montage… ». Détruire, couper, fusionner ou rogner un plan demande que le plan soit à soi (ou libre : il est pris au passage).
- Préférences > Équipe > « Prendre le plan libre que je commence à modifier » : la première modification d'un plan libre le prend.

### 27.4 À regarder à l'écran

La liste détaillée est en fin de `docs/COOPERATION.md` (deux fenêtres de SHOT sur un poste, puis deux postes sur Dropbox / Drive / réseau).
Points à vérifier en particulier :

1. La pastille et la bulle d'un plan pris par l'autre fenêtre ; le refus de dessiner dedans et sa phrase.
2. Un trait enregistré dans une fenêtre arrive dans l'autre sans qu'on y touche ; la barre d'état le dit une fois.
3. Prendre le même plan dans les deux fenêtres presque en même temps : une des deux perd, est prévenue, et trouve un brouillon dans la fenêtre Équipe.
4. Fermer une fenêtre : ses plans redeviennent libres dans l'autre en quelques secondes.
5. Le montage refusé sans le jeton, permis avec ; un plan créé est à soi.
6. L'application au repos avec un film partagé ouvert : le processeur reste à zéro.
7. Ouvrir le fichier `.pelure` d'un film partagé avec une ancienne version de SHOT : message clair, rien d'écrit.


## 29. Pistes repliables du montage (2026-10-20, branche `pistes-repliables`)

Demande d'Anthony : « un système de dépliage / repliage des calques dans la timeline pour gagner de la place tout en gardant cela lisible ;
je te laisse choisir la taille et l'ergonomie ». Code : `src/pistes.rs` (hauteurs, état, menus), `src/montagerows.rs` (pistes, hits, peinture),
`src/montagehead.rs` (colonne de gauche).

### 29.1 Deux hauteurs

| | Piste dépliée (`LAYER_H`) | Piste repliée (`FOLDED_H`) |
|---|---|---|
| Hauteur | 40 points (comme avant) | **16 points** (celle de la piste Caméra) : 2,5 pistes repliées pour une dépliée |
| Colonne de gauche | triangle, nom (12), œil, verrou, pelure, ⋮, barre d'opacité, mode de fusion, « après » | triangle, nom (11), œil, verrou (selon la largeur de la colonne) ; le reste est au clic droit |
| Dessins | blocs hauts, barre du calque de 5 | barres de 8, barre du calque de 3 ; les limites d'exposition restent visibles ; le nom du calque (10) dans le bloc |
| Losanges | effets (violets, rayon 4,5) en haut, position (verts, 5,5) au milieu | les mêmes couleurs, rayon 3 (4,2 quand choisis), effets en haut, position dessous |

16 points parce que c'est la plus petite hauteur où le nom (11 points, la plus petite taille de texte de SHOT hors barre d'opacité) reste net, et où un
losange se prend encore à la souris (7 points de haut pour les effets, 8 pour la position, 5 pour la barre du calque). Les plans, au-dessus, prennent la place
gagnée (la bande donne au montage tout ce que les pistes ne prennent pas, 170 points au plus).

### 29.2 Replier, déplier

- **Triangle** à gauche de chaque piste : clic = plie ou déplie cette piste ; **Alt + clic** = toutes les pistes. Bulle d'une seconde, clic droit : lui donner une
  touche (`piste.basculer`).
- **Double-clic sur la colonne**, ailleurs que sur le texte du nom : plie ou déplie. (Sur le nom : le renommer, comme avant — Anthony avait demandé le double-clic
  « sur le nom » ; il reste au renommage, le premier geste appris.)
- **Clic droit sur une piste > Replier / déplier les pistes** : cette piste, toutes les pistes, tout déplier, la case « Déplier le calque courant tout seul », puis le choix
  « Repliage des pistes » (commun à tous les plans / propre à chaque plan) ; en mode « propre à chaque plan », deux lignes de plus : « … les pistes de tous les plans ».
- **Barre du montage, sous « Plus… »** : « Replier toutes les pistes » (`piste.replier_tout`), « Déplier toutes les pistes » (`piste.deplier_tout`), « Déplier le calque
  courant » (`piste.auto_deplier`, allumé quand le réglage l'est), « Replis propres à chaque plan » (`piste.replis_par_plan`, allumé en mode par plan ; l'icône est un maillon
  entier en mode commun, un maillon brisé en mode par plan). Aucune touche par défaut (rien de libre sans risque de conflit) : clic droit sur l'icône pour en assigner une.
- **Déplier le calque courant** (réglage `deplier_calque_courant`, allumé d'office) : la piste du calque courant se montre dépliée, les autres gardent leur état
  (celui qu'on a enregistré). Elle se déplie **quand la souris est relâchée**, jamais pendant un geste (sinon les pistes d'en dessous glisseraient sous le pointeur au
  milieu d'un glisser). Replier cette piste à la main, ou « Replier toutes », la laisse repliée jusqu'au prochain changement de calque.

### 29.3 Les pistes traversent les plans

Chaque plan a ses calques dans son ordre (6f) ; la piste n est la rangée n de tous les plans. Deux façons de la replier, au choix d'Anthony (§ 29.8) ; la première est le mode d'origine :

- **Commun à tous les plans** (d'office). La hauteur d'une piste ne doit pas changer quand on clique sur un autre plan, elle ne dépend donc pas du plan courant : **une piste est
  repliée dès qu'un de ses calques l'est** (« replié l'emporte »). Le triangle plie ou déplie d'un coup le calque de cette rangée dans chaque plan.
- **Propre à chaque plan** (Préférences > Montage, § 29.8) : chaque plan retient ses propres replis ; les hauteurs suivent le plan courant.

### 29.4 Dossiers

Un dossier est une piste comme les autres. Son triangle est celui du panneau Calques : **fermé** (= `collapsed`), ses calques n'ont plus de piste (c'est le plan courant qui
décide) ; il se plie seulement dans son plan (la même rangée d'un autre plan est un calque ordinaire, qu'on ne touche pas). Le calque courant d'un dossier fermé garde sa piste
(comme dans le panneau Calques). « Replier toutes » laisse les dossiers comme ils sont (toutes les pistes restent visibles, en bandes fines) ; « Déplier toutes » ouvre aussi les
dossiers fermés.

### 29.5 Un seul état, avec la timeline de l'animation

L'état est `Layer::collapsed`, déjà enregistré avec le projet (`replie`, format inchangé : aucun champ neuf, la version du format ne bouge pas, un projet d'avant s'ouvre tout
déplié). C'est le même que celui des **lignes réduites** de la timeline de l'animation (13b) : un calque réduit là est replié dans le montage, et inversement ; pour un dossier,
c'est « fermé » dans les deux vues et dans le panneau Calques. Le calque courant déplié tout seul est de l'affichage seulement : il n'écrit rien.

**Replier est de l'affichage** : pas d'étape d'annulation (comme les lignes réduites), mais le projet est marqué modifié, puisque l'état s'enregistre. L'état n'est pas compté
comme un changement du plan par les envois (`envois_tests`).

### 29.6 Les gestes sur une piste repliée

Choisir un dessin, tirer son bord, la limite entre deux dessins, le milieu d'un dessin, la barre du calque (les trois derniers points de la piste), les losanges d'effets
et de position (choisir, Ctrl, Maj, glisser), le choix au rectangle (les pistes cachées d'un dossier fermé n'en sont pas), les clés lâchées, le clic droit : comme dans une piste dépliée,
d'après la géométrie de la piste sous la souris (`Lane` : `fx_hit_max`, `pos_y`, `pos_hit`, `bar_from`). Les pistes se calculent en quelques microsecondes (rien de plus par image,
rien qui fasse repeindre au repos).

### 29.7 À regarder à l'écran (liste pour Anthony)

1. Montage : le petit triangle à gauche de chaque nom ; un clic replie la piste en bande fine (le nom, l'œil, le verrou, les dessins en barres, les petits losanges) et les pistes
   d'en dessous remontent ; un autre clic la redéplie.
2. Alt + clic sur un triangle : toutes les pistes se replient (ou se déplient). Même chose : clic droit sur une piste > Replier / déplier les pistes, ou « Plus… » dans la barre du montage.
3. Cliquer sur un dessin d'une piste repliée : le calque devient courant et sa piste se déplie quand on relâche, les autres restent fines. Éteindre « Déplier le calque courant » : plus rien ne s'ouvre seul.
4. Sur une piste repliée : tirer la limite entre deux dessins, le bord d'un dessin, un losange vert ou violet, un rectangle de choix sur plusieurs pistes fines. Tout marche comme sur une piste haute.
5. Les noms restent lisibles à 100 % de zoom d'écran ; les losanges sont petits mais nets ; la barre du calque fine se prend en bas de la bande.
6. Un dossier : son triangle le ferme, ses calques disparaissent du montage (et de la timeline) ; rouvert, ils reviennent.
7. Enregistrer, quitter, rouvrir : les mêmes pistes sont repliées. En animation, les lignes réduites de la timeline sont les mêmes calques.
8. Au repos, l'application ne repeint pas (le processeur reste à zéro).

### 29.8 Deux façons de replier : commun à tous les plans, propre à chaque plan (2026-10-21, branche `pistes-par-plan`)

Anthony garde les deux comportements et en fait un choix : **Fichier > Préférences… > Montage > « Repliage des pistes »** (deux icônes, un maillon entier « Commun à tous les plans »,
un maillon brisé « Propre à chaque plan », une bulle d'une seconde avec une phrase pour chacun) ; aussi au **clic droit sur une piste > Replier / déplier les pistes**, et par la commande
« Replis propres à chaque plan » de « Plus… » dans la barre du montage (`piste.replis_par_plan`, sans touche). C'est un réglage de l'application (`reglages.toml`, clé
`repliage_pistes = "par_plan"`, absente = commun) : **rien n'est écrit dans le projet, format inchangé**. « Déplier le calque courant » reste le même réglage, dans les deux modes (il est
aussi dans Préférences > Montage).

**Ce que chaque geste écrit** (les « drapeaux » sont les `collapsed` des calques, § 29.5) :

| Geste | Commun à tous les plans | Propre à chaque plan |
|---|---|---|
| Hauteur d'une piste (affichage) | repliée si le calque de cette rangée est replié dans **un** plan (« replié l'emporte ») | repliée si le calque de cette rangée **du plan de la mise en page** l'est (plan courant au dernier relâchement de la souris) ; sans calque à cette rangée : dépliée |
| Triangle, double-clic, « Replier / Déplier cette piste », `piste.basculer` | écrit le calque de cette rangée dans **tous** les plans | écrit le calque de cette rangée dans le **plan courant** seulement |
| Alt + triangle, « Replier / Déplier toutes les pistes », `piste.replier_tout` / `piste.deplier_tout` | écrit **tous** les calques de **tous** les plans | écrit tous les calques du **plan courant** seulement |
| « Replier / Déplier les pistes de tous les plans » (clic droit, seulement en mode par plan) | (n'existe pas : c'est déjà le geste ci-dessus) | écrit tous les calques de **tous** les plans |
| Triangle d'un dossier (dans les deux modes) | ferme / ouvre le dossier du plan courant | idem |
| Tout replier : les dossiers | laissés comme ils sont | idem ; **tout déplier** ouvre aussi les dossiers fermés (du plan visé) |
| Le calque courant qui se déplie tout seul | n'écrit rien (affichage) | n'écrit rien (affichage) |
| Changer de mode | n'écrit rien | n'écrit rien |

**Décision pour « tout replier dans tous les plans »** : pas de modificateur sur les boutons (Alt est déjà « toutes les pistes » sur le triangle) ; le menu du clic droit offre les deux lignes
« … de tous les plans » quand le mode est « par plan », puisque ailleurs le geste de tous les plans est le geste normal.

**Passage sans perte** : passer de « par plan » à « commun » n'écrit aucun drapeau (le commun calcule seulement son « replié l'emporte » pour l'affichage, donc une rangée repliée dans un autre
plan apparaît repliée) ; l'inverse non plus. Les replis ne se réécrivent que si l'on plie ou déplie ensuite : la règle du mode en cours s'applique alors (en commun : à tous les plans, comme
avant). Repasser en « par plan » retrouve exactement les hauteurs d'avant, puisque chaque plan avait gardé ses drapeaux.

**Quand les hauteurs changent** (mode par plan) : le plan courant change au clic (ou au clavier, ou à la lecture), mais la mise en page des pistes garde le plan d'avant **tant que la souris est
tenue** et prend le nouveau plan **au relâchement** (`PistesUi::layout_plan`, comme « Déplier le calque courant » qui reste actif dans les deux modes). Les pistes d'en dessous ne glissent donc
jamais sous le pointeur au milieu d'un clic ou d'un glisser ; les cases des autres plans se dessinent dans ces hauteurs. Au clavier ou pendant la lecture (souris relâchée), les hauteurs suivent le plan
à chaque plan franchi. Les dossiers fermés cachent les pistes de leurs calques d'après le plan courant, dans les deux modes (règle d'avant, inchangée).

**Timeline de l'animation (13b)** : elle montre un seul plan, et ses lignes réduites sont les `collapsed` de ce plan. En mode par plan, elle et le montage disent exactement la même chose pour le
plan courant, et un repli fait dans un autre plan ne change ni l'un ni l'autre. En mode commun, comportement d'avant (la timeline lit les drapeaux du plan courant, le montage ajoute le « replié
l'emporte » des autres plans).

**Travail à plusieurs** (`docs/COOPERATION.md`) : en mode par plan, replier une piste n'écrit que les calques du plan courant, donc **jamais dans un plan qu'on ne tient pas** ; en mode commun, replier
une piste écrit dans les calques de la même rangée de tous les plans, y compris ceux d'un équipier (c'est un drapeau d'affichage, mais c'est une écriture dans son plan). Le mode par plan est donc
le plus tranquille sur un film partagé.

Limites : un projet enregistré avec des replis d'un mode s'ouvre dans l'autre sans rien perdre, mais l'affichage peut alors surprendre (en commun, un seul calque replié dans un autre plan replie la
piste partout) ; le mode n'est pas retenu dans le projet, c'est une préférence de la personne qui travaille ; pendant la lecture de plusieurs plans en mode par plan, les pistes changent de
hauteur à chaque plan.

### 29.9 À regarder à l'écran : les deux modes (liste pour Anthony)

1. Préférences > Montage : deux icônes (maillon entier, maillon brisé) ; la bulle de « Repliage des pistes » dit les deux phrases ; celle de chaque icône, la sienne. Même choix dans le clic droit
   sur une piste > Replier / déplier les pistes, et dans « Plus… » du montage (icône allumée = par plan).
2. Mode **commun** (d'origine) : tout se passe comme au § 29.7 ; replier la piste de « Décor » la replie dans chaque plan.
3. Mode **par plan** : replier « Décor » dans le plan 1 ; cliquer sur une case du plan 2 : pendant que la souris est tenue, les pistes ne bougent pas ; au relâchement, elles prennent les hauteurs du
   plan 2 (dépliées si rien n'y est replié) ; revenir au plan 1 : « Décor » est de nouveau fin.
4. Mode par plan : les cases des autres plans se dessinent dans les hauteurs du plan courant (dessins, losanges, barre du calque) et se prennent comme d'habitude.
5. Mode par plan : Alt + triangle et « Replier toutes les pistes » ne touchent que ce plan ; « Replier les pistes de tous les plans » (clic droit) les touche tous. Le calque courant se déplie toujours
   tout seul au relâchement.
6. Basculer de par plan à commun, puis revenir : aucune piste ne change de drapeau (rien n'est marqué modifié dans le titre) ; en commun on voit les rangées repliées dans n'importe quel plan, en par plan
   seulement celles du plan courant.
7. En animation, la timeline du plan courant montre les mêmes lignes réduites que le montage en mode par plan. Enregistrer, quitter, rouvrir : mêmes replis par plan, même mode (c'est une préférence).
8. Au repos, l'application ne repeint pas, dans les deux modes.


## 30. Préférences en onglets (2026-10-21, branche `preferences-onglets`)

Demande d'Anthony : « La fenêtre Préférences est très mal organisée et trop verticale. J'aimerais que tu l'organises par
onglets pour la rendre plus ergonomique et compréhensible. » Code : `src/prefsonglets.rs` (la table des réglages),
`src/prefsui.rs` (la fenêtre), essais dans `src/prefsui_tests.rs`. Rien dans les réglages eux-mêmes n'a changé : leurs
clés de `reglages.toml`, leurs effets et leur moment d'application (tout de suite) sont ceux d'avant ; seule la mise en
page est neuve, plus la clé `onglet_preferences` (l'onglet ouvert).

### 30.1 La mise en page

- **Une liste d'onglets verticale à gauche** (icône Phosphor et nom, 156 points), **le contenu à droite**, **un champ de
  recherche en haut**. Choisie plutôt qu'une rangée d'onglets en haut : neuf noms ne tiennent pas côte à côte dans une
  fenêtre de 800 points sans deuxième ligne ni défilement, et la liste laisse toute la hauteur au contenu (c'est aussi la
  façon de nombreux logiciels de création, Blender par exemple).
- **La fenêtre** : 800 × 540 au départ (plus large que haute), redimensionnable (minimum 560 × 320), ramenée à la taille
  de l'écran si celui-ci est petit, ouverte en haut au centre ; le contenu de l'onglet défile (une barre par onglet, la
  position de chacune est retenue) quand il dépasse. La croix de la barre de titre la ferme ; plus d'ancrage : on la déplace.
- **Un onglet** : son icône et son nom en grand, une phrase, puis des groupes. **Un groupe** : un titre court, un trait, les
  réglages (étiquette à gauche, contrôle à droite quand le réglage a une valeur : couleurs du thème, performance,
  programmes).
- **L'onglet ouvert est retenu** (réglages : `onglet_preferences`, absent pour Général) : la fenêtre rouvre là où on l'a
  laissée, d'une séance à l'autre. Le petit engrenage du panneau Suggestions sonores l'ouvre sur Génération.
- **Les bulles** : une seconde sur un onglet (nom et phrase), sur le champ de recherche, sur chaque réglage (le survol
  d'egui est réglé à 1 s pour toute l'application).

### 30.2 Qui est où

| Onglet (icône) | Groupe | Réglages (étiquette lue à l'écran) |
|---|---|---|
| **Général** (engrenage) | Au lancement et à l'ouverture | Rouvrir le dernier projet au lancement ; Ouvrir dans un nouvel onglet |
| | En cas de plantage | Copie de secours toutes les N min |
| | Coller depuis un autre projet | Proportions différentes (Contenir / Remplir) ; Cadence différente (images / secondes) ; Demander à chaque collage |
| **Apparence** (palette) | Thème et couleurs | Thème + Rétablir ; neuf couleurs (fond des panneaux, barres, fenêtres et menus, boutons et curseurs, texte, accent et sélection, autour du dessin, timeline et montage, bordures) |
| | Panneaux | Ajuster les panneaux à leur contenu ; Réajuster les panneaux |
| **Dessin** (crayon) | Stylet et pinceau | Croix du système sur le pinceau |
| | Caméra | Opacité du passe-partout + Rétablir |
| **Montage** (bobine) | Pistes du montage | Repliage des pistes (commun / par plan) ; Déplier le calque courant tout seul |
| **Raccourcis** (clavier) | Raccourcis clavier | Jeu de raccourcis ; Voir et changer les raccourcis… ; Modifier le fichier à la main… ; Recharger |
| **Export** (export) | Programmes et modèles | ffmpeg (Choisir…, Automatique) ; Modèle Storyboard Pro (Choisir…) ; Ouvrir la fenêtre Exporter… |
| **Équipe** (trois personnes) | Commentaires | Signer mes commentaires |
| | Travail à plusieurs | Mon nom et ma couleur ; Une prise périme après N min ; Prendre le plan libre que je commence à modifier ; Dire dans la barre d'état ce que font les autres |
| **Génération** (étincelle) | Génération sonore | Les trois rôles (lecture, bruitages et ambiances, voix) et leur fournisseur, les clés (inchangées), Dossier des modèles, Python du serveur, Démarrer le serveur tout seul, port, Démarrer / Arrêter / Tester |
| **Performance** (jauge) | Réglages de performance | Les onze réglages du diagnostic (effets en aperçu réduit, traits texturés, tremblement, pelure, trois caches, fils, vignettes, animations, effets sur la carte graphique) ; Diagnostic des performances… ; Revenir comme avant ; Valeurs d'origine |
| | Affichage | Le moteur de rendu et la carte graphique |

« Export » est le seul onglet qui montre des réglages qu'on ne trouvait pas dans les Préférences : le chemin de ffmpeg et le
projet modèle de Storyboard Pro existaient déjà (`ffmpeg`, `modele_storyboard_pro` dans `reglages.toml`) mais ne se
choisissaient que dans les fenêtres d'export, qui gardent leurs boutons. Le dossier du dernier export, la fiche technique
(colonnes, format) et les options propres à chaque export restent dans leur fenêtre.

### 30.3 La recherche

Un ou plusieurs mots (tous doivent répondre), sans tenir compte des majuscules ni des accents, dans l'étiquette du réglage,
quelques mots ajoutés (« voile » pour le passe-partout, « mo » pour les caches…), le titre du groupe et le nom de l'onglet.
Quand le champ n'est pas vide, **les groupes qui répondent, de tous les onglets, remplacent le contenu** : au-dessus de
chacun, un lien « Onglet › » (ouvre l'onglet et vide la recherche) et « trouvé : … » en couleur d'accent (les étiquettes
qui répondent) ; dans la liste, les onglets qui répondent portent le nombre de réponses (les autres sont grisés).
Les groupes de la recherche sont dessinés par le même code que dans leur onglet (même réglage, mêmes bulles). La croix du
champ efface la recherche ; choisir un onglet aussi.

### 30.4 La table des réglages

`REGLAGES` (`prefsonglets.rs`) : une ligne par réglage, `(onglet, groupe, identifiant, étiquette, mots)`. Un groupe n'a qu'un
onglet. `prefsui.rs` dessine un onglet en parcourant les groupes de la table ; la recherche lit la même table. Pour ajouter
un réglage : l'ajouter à la table (et à son dessin dans `groupe_ui` ou dans la fonction que le groupe appelle) ; pour en
retirer un, le retirer des deux. Les essais (`prefsui_tests.rs`) échouent si une étiquette de l'ancienne fenêtre manque dans
la table, si une étiquette est dans deux onglets, si un onglet ne montre pas toutes les siennes (la molette fait défiler
l'onglet jusqu'en bas), ou montre une étiquette d'un autre onglet.

### 30.5 Limites

- Les réglages gardent leur dessin d'avant (étiquette à côté du contrôle sur une ligne) sauf ceux de l'apparence, de la
  performance et des programmes, déjà en deux colonnes : les grouper tous dans une grille demanderait de réécrire les
  fonctions des autres sections.
- Pas de bouton « Rétablir » par groupe là où il n'y en avait pas (thème, passe-partout et performance ont le leur).
- La fenêtre n'a pas de boutons OK / Annuler : comme avant, chaque réglage joue et s'enregistre tout de suite.
- La recherche ne surligne pas le mot dans le réglage : elle montre les groupes et dit « trouvé : … ».
- L'ouverture de la fenêtre n'a pas de raccourci propre (menu Fichier, Ctrl+K : « Préférences »).

### 30.6 À regarder à l'écran (liste pour Anthony)

1. Fichier > Préférences… : une fenêtre plus large que haute, neuf onglets à gauche (icône + nom), champ de recherche en
   haut ; on la tire par un bord (elle grandit), par sa barre de titre (elle se déplace).
2. Les neuf onglets, un à un : titre, phrase, groupes ; chaque groupe a un titre court et un trait ; rien n'est coupé ;
   l'ascenseur n'apparaît que sur Génération (sur un écran de 1080 points, il peut apparaître ailleurs si on rétrécit la
   fenêtre).
3. L'onglet choisi garde son fond d'accent et son texte lisible dans les trois thèmes (Gris sombre, Gris clair, Sombre).
4. Fermer, rouvrir : même onglet. Quitter SHOT, relancer, rouvrir : même onglet. Le petit engrenage de Suggestions sonores :
   l'onglet Génération.
5. Apparence : le thème, une couleur (« Personnalisé »), « Rétablir ». Dessin : le curseur du passe-partout et « Rétablir ».
   Montage : les deux icônes de repliage. Raccourcis : le jeu, les trois boutons. Performance : une case, une valeur.
   Équipe : le nom, la couleur, la durée d'une prise. Général : les deux cases, la copie de secours, les choix du collage.
6. Export : « Choisir… » ffmpeg ouvre le choix de fichier ; une fois choisi, le nom du fichier remplace « cherché tout
   seul » et « Automatique » apparaît ; « Ouvrir la fenêtre Exporter… » ouvre Exporter.
7. Chercher « couleur » (le thème et l'équipe), « cache » (trois caches), « cle » ou « clé » (Génération), « voile »
   (Caméra), « zzz » (rien ne répond) ; cliquer « Dessin › » ; effacer par la croix.
8. Une seconde sur un onglet, le champ de recherche, un réglage : la bulle.
9. Écran de 1366 × 768 ou fenêtre de SHOT réduite : la fenêtre se ramène à l'écran et son contenu défile.
10. Fenêtre ouverte, application au repos : aucun repeint (le processeur reste à zéro).

## 31. Effets au clavier et tremblement dans « Trait » (2026-10-21, branche `effets-clavier`)

Deux demandes d'Anthony : le bouton « Copier / coller » du panneau Effets ne sert à rien (on choisit un effet, Ctrl+C, on clique un
autre calque, Ctrl+V) ; le tremblement du trait est de la famille Trait et doit être avec les autres.

### Mots

| Mot | Sens | À éviter |
|---|---|---|
| **Effet choisi** | l'effet dont le nom est encadré dans le panneau Effets (ses clés se voient dans la timeline et le montage) ; Ctrl / Maj + clic en choisit plusieurs | sélectionné (pour les clés, c'est autre chose) |
| **Coller en fin de pile** | l'effet est ajouté après ceux du calque (il s'applique après eux), comme « Ajouter un effet » ; jamais à la place d'un autre | écraser |

### Où et quels gestes

| Geste | Où | Effet |
|---|---|---|
| Clic sur le nom d'un effet | panneau Effets | le choisit ; le panneau a la main (cadre discret) ; un second clic le lâche |
| Ctrl + clic / Maj + clic sur un nom | panneau Effets | ajoute ou retire un effet du choix / prend la plage jusqu'à l'effet choisi |
| Ctrl+C | panneau Effets a la main, un effet choisi (ou un seul effet sur le calque) | copie les effets choisis : réglages et clés |
| Clic sur un calque | panneau Calques | le calque devient courant ; le panneau a la main (cadre discret s'il y a des effets copiés) |
| Ctrl+V | panneau Effets a la main, ou panneau Calques a la main après une copie d'effets | colle à la fin de la pile du calque courant (ou des calques choisis) : une étape « Coller l'effet » / « Coller 2 effets » |
| « Copier tous les effets du calque », « Coller », « Remplacer les effets » | clic droit sur un calque > Effets ; clic droit sur le nom d'un effet ; Ctrl+K (`fx.copier_tout`, `fx.coller`, `fx.remplacer`) | comme avant |
| Tremblement du trait | bouton « Trait ▾ » du panneau Effets, premier de la liste (icône vague) ; aussi menus de calque, en-tête de la timeline, outils vectoriels, Ctrl+K (`fx.tremblement`) | coche / décoche, grisé avec la raison sur un calque bitmap |

### Ce que voit l'utilisateur

- **Panneau Effets** : « Ajouter un effet » et « Préréglages » ; plus de bouton « Copier / coller ». Chaque effet a toujours son icône « Copier l'effet »
  (elle copie tout le choix si l'effet en fait partie). Les effets choisis ont le cadre de la couleur de sélection.
- **Menu « Trait ▾ »** : « Tremblement du trait », puis écriture, crayonné, respiration, ondulation, pointillés, désagrégation ; le titre compte
  les effets allumés (le tremblement compte), la section « Préréglages » est inchangée (le tremblement n'a pas de préréglage ; ceux de « Tremblement
  de l'image » restent dans « Préréglages » du panneau).
- **« Ajouter un effet »** : plus d'entrée « Tremblement du trait » en tête ; il est dans la famille « Trait » de ce menu, avec les autres.
- **Panneau Calques** : un cadre discret quand Ctrl+V va y coller des effets.
- **Barre d'état** : « 2 effets copiés : Flou gaussien, Lueur. Ctrl+V sur un autre calque… », puis « 2 effets collés sur le calque « B ». ».

### Limites

- Ctrl+V depuis la timeline ou le montage garde son sens d'avant (images, clés, plans) : pour coller des effets, cliquer le calque dans le panneau
  Calques (ou travailler dans le panneau Effets).
- Les effets du trait et de matière, ainsi que le tremblement, ne sont qu'une fois par calque : sur un calque qui l'a déjà, la copie n'est pas ajoutée (la barre d'état le dit).
- Seul le premier effet choisi (le dernier cliqué) montre ses clés dans la timeline.
- Ni Ctrl+X sur les effets, ni glisser-déposer d'un effet sur un calque.
- Le tremblement ne change que de menu : son rendu (processeur et carte graphique) et l'enregistrement des projets sont ceux d'avant (la famille n'est pas enregistrée).

### Essais ajoutés

`fxclavier_tests.rs` (16 essais) : clic sur un nom (choix, main au panneau, second clic), Ctrl / Maj + clic, choix perdu en changeant de calque, Ctrl+C /
Ctrl+V vers un autre calque (réglages et clés identiques, une étape nommée, annulation), plusieurs effets d'un coup, effet de même sorte ajouté et non
remplacé, `fx.remplacer` et `fx.coller` toujours là, Ctrl+V depuis le panneau Calques seulement après une copie d'effets, rien sans la main, un appui ailleurs
rend la main, collage d'une feuille 320×180 dans une 640×360 (pixels doublés), bouton disparu, tremblement premier du menu « Trait » et absent en tête
de « Ajouter un effet », grisé sur un bitmap, projet avec tremblement enregistré et relu à l'identique.

### À vérifier à l'écran (par Anthony)

1. Panneau Effets : plus de bouton « Copier / coller » à côté de « Préréglages ».
2. Sur un calque avec trois effets, cliquer le nom du premier : cadre ; Ctrl + clic sur le troisième : deux cadres ; Maj + clic : la plage ; un clic simple :
   un seul ; re-cliquer l'unique : plus aucun.
3. Choisir deux effets, Ctrl+C ; cliquer un autre calque dans le panneau Calques (un cadre discret apparaît) ; Ctrl+V : les deux effets arrivent
   en bas de sa pile, avec leurs réglages et leurs clés ; Historique : « Coller 2 effets » ; Ctrl+Z les retire d'un coup.
4. Coller deux fois le même flou : deux flous (aucun n'en remplace un autre). Ctrl+K « remplacer les effets » : la pile est remplacée.
5. Copier un effet, puis copier un dessin (outil de sélection, Ctrl+C), cliquer un calque : Ctrl+V colle le dessin, plus l'effet.
6. Copier un effet dans un projet, ouvrir un autre onglet d'une autre taille de feuille, cliquer un calque, Ctrl+V : l'effet arrive, ses pixels adaptés.
7. Menu « Trait ▾ » : « Tremblement du trait » est le premier ; grisé (bulle) sur un calque bitmap ; « Ajouter un effet » : pas d'entrée à part
   en tête, mais « Trait > Tremblement du trait ». Rouvrir un projet d'avant avec un tremblement : il tremble comme avant.