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
| L'espace où l'on place les calques et la caméra | **Espace 3D** (le plateau en 3D et la scène 3D d'avant ne font plus qu'un, sans objets depuis le 2026-10-21) ; **carte** (un calque du plan posé dans l'espace) ; **caméra** (celle du plan, la seule) | maquette, set, scène 3D, plateau en 3D, caméra du plateau, objet 3D, repère 3D |
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
- **« Fond »** : fond de la feuille, fond du thème (le « calque de fond » du panneau Calques a disparu le 2026-10-21 : un calque bitmap
  ordinaire fait la même chose). L'aplat sous le trait (fonction du calque) ne s'appelle plus « fond » : c'est l'**« Aplat sous le trait »**
  (« Aplat » sur sa sous-ligne), depuis le 2026-10-21. Ne jamais écrire « Fond sous le trait » ni « Remplissage » dans un texte de l'interface
  pour cette fonction.
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

## 11. Vignettes simples (la vue Vignettes, 2026-10-21, branche `vignettes-simples`)

Demande d'Anthony, tranchée sur des maquettes interactives : la « Planche de crayonnés » (2026-10-19) et la vue
Vignettes faisaient deux fois le même travail. **La planche de crayonnés est supprimée** ; ce qu'elle avait d'utile
est passé dans les Vignettes (`board.rs` pour la mise en page, `boardui.rs` pour l'écran). Vocabulaire : **image**
(une vignette : le début d'un plan, ou l'une de ses marques d'image), **plan**, **case vide** (après la dernière
image), **barre** (entre deux images), **bande de note** (sous une image).

### Ce que l'on voit

- **Rien autour des images** : un filet gris d'un point, plus foncé pour l'image courante ; pas de bande de titre, pas
  de durée, pas de « Plan ». Seul le **numéro du plan**, au-dessus de sa première image, en gris discret. Principe d'Anthony : « la partie Vignettes
  doit être la plus épurée possible, rien de distraction, que du dessin » : **ni pastille d'envoi ni pastille du film partagé ni
  étiquette de version sur la planche** (elles restent au montage), ni compteur de plans choisis. Les réglages (taille, colonnes,
  lignes par page, zoom) sont en haut, petits et à 30 % d'opacité, pleins seulement sous la souris ; « Attacher les notes » est au
  clic droit sur le fond. Plusieurs plans choisis : leurs images (sauf l'image courante) ont un liseré
  d'accent.
- **Écart** : proportionnel à la largeur des images (voir plus bas) : 18 points entre deux images d'un même plan, 53 à une coupe, pour des images de 240. Les lignes ont un nombre fixe d'images
  (« colonnes », 6 au plus) ; une ligne ne se casse jamais à un changement de plan. Ordre de lecture : de gauche à
  droite, puis ligne par ligne.
- **Barre** entre deux images qui se suivent : courte (un tiers de la hauteur de l'image), 1,5 point, **grise = même
  plan, noire = coupe**. Une barre est aussi après la dernière image de chaque ligne (la transition vers la première
  image de la ligne suivante). Sous la souris elle prend l'accent, avec une bulle.
- **Bande de note** sous chaque image, dans l'espace entre les lignes : vide, juste un filet très pâle (aucun mot). Le
  texte passe à la ligne ; il rapetisse un peu avec sa longueur (11 → 10 → 9,5 points) ; **trois lignes au plus**, puis
  « … ». La ligne d'images suivante descend juste de ce qu'il faut pour garder sa marge quand une note prend deux ou
  trois lignes.
- **Case vide** après la dernière image (une seule, un « + » très pâle ; la planche s'arrête là).

### Ce que l'on fait

- **Dessiner** : un trait qui commence dans une image (pleine ou vide) la rend courante **à l'appui** et dessine dedans,
  sans clic de choix avant. C'est le dessin lui-même qui est dans l'image courante : même moteur, même rendu progressif, le
  point reçu est dans l'image d'écran où il arrive. **Un trait qui commence ailleurs ne dessine rien** (ni note libre : on ne
  trace plus de notes au pinceau sur la planche ; textes, gomme et sélection des notes restent). **Pendant le geste, le
  trait déborde à l'écran** (retouche du 2026-10-21, branche `vignettes-retouches`) : on le voit en entier, même par-dessus
  l'écart, les barres, les bandes de note et les images voisines ; il en va de même de l'aperçu d'une ligne, d'un rectangle,
  d'une ellipse, d'une courbe ou d'une polyligne. **Au lâcher**, l'image ne montre plus que son cadre (et sa petite marge,
  comme avant) ; le dessin garde tout le tracé (toile infinie) et l'export cadre comme avant. Comment : `debord.rs` ne
  repeint que **le dehors**, d'après le tracé que le moteur garde de toute façon (ligne médiane lissée et rayon de chaque
  point, plus la pointe jusqu'à la croix) : un trait qui reste dans l'image ne produit aucune forme, un trait qui sort ne
  produit que ses tronçons du dehors (quelques lignes épaisses, quatre fenêtres de coupe autour de l'image) ; le dedans reste
  le rendu exact du moteur. Aucun retard ajouté (calculé dans l'image d'écran où le point arrive) ; pas de débord pour la
  gomme (rien à montrer). Au dehors, le trait est une approximation (couleur, rayon, opacité ; pas la texture du pinceau).
- **Case vide** : dessiner dedans **fait le plan** à l'appui même (les cases vides sautées avant elle deviennent des plans
  vides, pour garder sa place). Si la barre qui précède la première case vide est **grise** (un clic la bascule), la case
  fait **une image de plus du dernier plan** (il s'allonge de la durée de sa dernière image, une marque d'image est posée).
  La création et le trait ne font **qu'une étape d'annulation** (« Nouveau plan dessiné » / « Nouvelle image dessinée ») ;
  un simple clic qui ne trace rien ne laisse aucune trace. Outils qui font un plan : Pinceau, Crayon, Ligne, Rectangle,
  Ellipse (les autres ne font rien dans une case vide).
- **Barre** : un clic sur une barre **grise** coupe le plan à cette image (la vignette de droite commence un plan neuf :
  `split_plan`) ; sur une barre **noire** il réunit les deux plans (le début du plan de droite devient une marque d'image du plan
  de gauche : `join_next_plan`, les deux plans doivent être de la même séquence). Une étape d'annulation, nommée (« Couper le
  plan entre deux vignettes », « Réunir deux vignettes en un plan »).
- **Note** : un clic sur la bande l'ouvre ; Entrée (ou un clic ailleurs) la pose, Maj + Entrée passe à la ligne, Échap
  abandonne. Elle va dans les notes du plan (première image) ou de la marque d'image ; le texte entier est gardé, visible
  pendant la saisie et dans la fiche du plan. Une étape d'annulation, « Note de vignette ».
- **Choisir des plans** : Ctrl ou Maj + clic sur une image (Ctrl ajoute ou retire, Maj prend l'intervalle) ; ou le lasso
  depuis le fond de la page. Clic droit sur une image : le menu de la vignette (animer, nouveau plan, dupliquer, couleur, versions,
  copier / coller…) ; sur le fond : le menu de la planche. Molette : défiler ; Ctrl + molette : zoom ; Espace, bouton du milieu
  ou outil Main : déplacer la planche.
- **Déplacer un plan** (retouche du 2026-10-21) : **Maj + appui sur le CADRE d'une image** (son filet, à cinq points près de
  chaque côté), puis glisser. Maj + appui **dedans** garde son sens (choisir des plans) ; un simple clic sur le cadre ne fait
  rien. Règles validées par Anthony :
  - tirer **n'importe quelle** image d'un plan déplace **tout le plan d'un bloc** (ses images et marques d'image, ses
    dessins, ses clés de caméra, son décor, ses notes, ses versions, ses commentaires…) : c'est l'opération du montage,
    `Document::move_plans` (`App::move_plans_before`). Si le plan pris fait partie des plans choisis, tous les plans choisis
    partent ensemble ;
  - le **repère de pose n'est jamais qu'entre deux plans**, jamais dans un plan (on ne coupe pas un plan par mégarde ; une
    future image de mouvement de caméra, plus large, passera à la ligne d'un bloc). Pour sortir une image d'un plan : on
    **coupe d'abord sa barre**, puis on la déplace ;
  - retours : le curseur devient une **main** sur le cadre quand Maj est tenue (même sur l'image courante, dont le dessin
    demande sinon le pinceau), une **main fermée** pendant le geste ; les images d'origine s'effacent un peu, un **fantôme**
    des images du plan suit le pointeur (leurs places relatives gardées), un **trait d'accent** marque la pose, entre deux
    plans ; près de sa place, aucun repère (lâcher là = ne rien faire) ; **Échap abandonne** ;
  - une seule étape d'annulation, « **Déplacer le plan** » (« Déplacer les plans » pour plusieurs) ; le plan déplacé devient
    courant ;
  - **d'une séquence à l'autre**, comme au montage : le plan rejoint la séquence du plan devant lequel on le pose, la barre
    d'état le dit. Un storyboard verrouillé, ou un plan tenu par un autre dans un film partagé, refuse avec un message
    (`boardmove.rs`).
- **Ce qui n'existe plus** : glisser une image *sans Maj* pour réordonner (le geste est celui du trait), le double-clic pour animer (un clic dessine : « Animer cette image » est au clic droit, Tab),
  les réglages « Écart » et « Plan à la ligne », la durée et « Plan N » dans l'en-tête, l'étiquette de version « · V2 » (le
  menu du clic droit garde les versions).

### Mouvements de caméra dessinés dans les cases (2026-10-21, branche `vignettes-onglet-camera`)

Demande d'Anthony (option 1 validée) : dessiner un mouvement de caméra directement sur la planche. Code : `vignettecam.rs`
(logique sans interface), `vignettecamui.rs` (le geste et les cadres à l'écran), `board.rs` (mise en page), `thumbs.rs`
(vignette d'une zone).

- **Ctrl + Maj + glisser le cadre d'une case** (ou **T tenue** ; depuis le 2026-10-22 : Ctrl seul est le lasso, voir plus bas) :
  - le cadre d'origine reste **A** ; un cadre **B** suit le pointeur et reste là où on le dépose. Au lâcher, **deux clés de
    caméra** sont posées sur le plan : A à la première image, B à la dernière (le modèle de la timeline et du montage : vitesse
    constante, aucun adouci, comme une clé posée au montage) ;
  - en recommençant depuis l'**intérieur du dernier cadre**, on ajoute **C**, puis **D**… Les clés se **répartissent
    régulièrement sur la durée du plan** (`repartition` : 0, mi-plan, fin pour trois ; 0, ⅓, ⅔, fin pour quatre) quand elles
    l'étaient déjà. **Des clés posées à la main (montage, timeline) sont respectées** : la nouvelle va à la dernière image et, si
    la dernière clé y était déjà, elle recule à mi-chemin entre sa voisine et la fin ;
  - un **coin** tiré (d'une case sans mouvement : celui de la vignette) change la taille du cadre, donc le **zoom** de la clé
    (tirer vers l'extérieur élargit) ; le **bord** d'un cadre existant (A, B, C…) déplace cette clé ; le dedans du dernier cadre
    en fait naître un nouveau ; ailleurs, rien ;
  - un plan d'une seule image ne peut pas porter de mouvement (message : allonge-le d'abord) ; un storyboard verrouillé ou un plan
    tenu par un autre refuse ; **Échap** abandonne et remet les clés d'avant ; un **Ctrl + clic qui ne bouge pas** garde son sens
    d'avant (choisir des plans, fait au lâcher) ; le curseur devient une croix fléchée sur le cadre à prendre (Ctrl tenue) ;
  - **une seule étape d'annulation par geste, nommée** : « Mouvement de caméra » (B, de A vers B), « Cadre C », « Cadre D »,
    « Cadre B » (déplacé), « Zoom du cadre B ». Le geste modifie les clés à chaque image (la case grandit en direct) et ne
    prend son étape qu'au lâcher, depuis l'état d'avant.
- **La case grandit** pour montrer toute la zone des cadres. On prend le cadre A comme unité, l'**englobant de tous les cadres**
  sur la feuille, et la case a le nombre entier de cases qui le contient : 1 × 1 (tout tient dans A), 2 × 1 (un panoramique d'une
  largeur de cadre), 1 × 2, 2 × 2, 3 × 1… Au plus les **colonnes** de la planche en largeur et **3 cases** en hauteur. Rien n'est
  enregistré pour cela : la taille se **déduit des clés** (en supprimer au montage fait rétrécir la case, les annuler aussi).
- **Mise en page** (`board::layout_with`) : l'ordre de lecture reste le même ; chaque case prend la première place libre après la
  précédente ; une case qui ne tient pas dans la fin de la rangée (ou de la page) **passe tout entière à la suivante**, jamais
  coupée ; les voisines suivent. Une case de plus d'une rangée de haut aligne les colonnes de la planche (écart de coupe partout,
  barres incluses) pour que les rangées d'en dessous ne la chevauchent pas ; sinon chaque rangée garde ses écarts serrés dans un
  plan. La barre qui suit une grande case est aussi haute qu'elle (zone de clic), le trait reste d'un tiers d'une case.
- **Dans la grande case**, on voit la feuille à l'échelle du cadre A (un cadre A = une largeur de case), centrée sur l'englobant,
  **hors de la feuille aussi** : le décor se dessine partout (toile infinie), en direct comme dans une case ordinaire (même
  moteur, même débord du trait pendant le geste, au lâcher la case coupe à son cadre). Une case qui n'est pas courante est une
  vignette de la zone (`Thumbs::texture_zone`, sur un fil de fond : une caméra droite centrée sur la zone, dont le cadre en a la
  taille). L'outil Caméra, lui, recadre à l'ancienne.
- **Les cadres** se dessinent fins, par-dessus, dans la couleur de la caméra du thème (`theme::camera()`, liseré clair dessous),
  chacun avec sa **lettre** dans une petite étiquette et une **flèche** de centre à centre entre deux cadres qui se suivent (la
  convention du storyboard). Ils apparaissent dès que le plan a deux clés, même si tout tient dans A.
- **Le montage et la timeline** montrent les vraies clés sur la piste Caméra ; la lecture suit (A vers B à vitesse constante).
- **Pastilles des personnages** : la planche montre aussi la colonne des pastilles, au bord droit, quand l'espace de travail la
  montre (d'origine : Vignettes et Storyboard), au lieu de ne la montrer que dans la vue Dessin (depuis le 2026-10-21, une colonne et non plus
  une barre au-dessus : partie 41).
- **Limites** : les calques en profondeur (multiplan) d'une grande case gardent leur place vue du cadre A ; une caméra inclinée ou
  panoramiquée compte par son rectangle englobant (la zone n'est pas inclinée) ; seule la première image d'un plan est une case de
  mouvement (ses marques d'image restent des cases ordinaires, vues à travers la caméra).

### La planche comme la maquette 07 v6 (2026-10-21, branche `vignettes-maquette`)

Retour d'Anthony : « la dernière maquette marchait mieux que le logiciel : le dessin pouvait sortir de la case, l'extérieur en
dessous était plus discret et mieux intégré ; le geste de mouvement de caméra ne marche pas non plus ». Code : `board.rs`
(mise en page), `boardui.rs` (écran), `thumbs.rs` + `render.rs` (la marge des vignettes), `debord.rs`, `vignettecamui.rs`,
`springtools.rs`.

- **Une carte blanche, pas une grille** : la planche est une **carte blanche au bord fin et doux, aux coins arrondis**, sur le
  fond de l'espace de travail ; elle **s'arrête après la dernière rangée utile** (plus de pages complétées par des cases vides, plus
  d'ombre portée). **Une seule case vide**, juste après la dernière image : un cadre très pâle avec un « + » discret, qui
  s'assombrit sous la souris ; dessiner dedans fait le plan (inchangé). `board::BoardLayout::empty` n'a donc plus qu'une case.
- **Proportions de la maquette** : filet d'image gris moyen (154), plus foncé pour l'image courante ; écarts **proportionnels à la
  largeur des vignettes** (10 / 136 dans un plan, 30 / 136 à une coupe : 18 et 53 points pour une vignette de 240, `board::gap_same`
  et `gap_cut`) ; barre dans un plan : **1,5 point, très pâle (230)**, un tiers de la hauteur ; barre de coupe : **2 points, noire** ;
  numéro du plan petit (11 points) tout en haut de sa ligne ; bande de note **sans cadre, un filet très pâle dessous**, texte gris
  petit (trois lignes puis « … », la taille baisse un peu avec la longueur).
- **Le trait qui déborde** : pendant le geste, la partie dehors du trait se voit en entier mais **pâle** (`debord.rs`,
  `GHOST_ALPHA`) ; **au lâcher, la case en garde un peu au-delà de son cadre** (`RING_FRACTION`, 3 % de sa largeur au premier essai, 12 % depuis le 2026-10-22), comme la maquette
  coupe à la case plus une petite marge. Comment : la vignette de chaque case (`Thumbs::texture_case`) est composée par le fil de
  fond avec cette marge autour du cadre (`render::thumbnail_marge_until` : dans la marge, seul ce qui est dessiné est composé, pas le
  papier) ; la case courante, qui est le dessin lui-même, peint seulement les quatre bandes de la marge de sa vignette. Rien de plus sur
  le fil de l'interface. **Ce qui est dessiné hors du cadre se voit donc dans cette marge, aplats de fond compris.**
- **Mouvement de caméra** : Ctrl **tenu avant l'appui** ne prend plus l'outil Sélection (le lasso à ressort) dans les Vignettes : le
  geste marche avec n'importe quel outil ; et il marche aussi sur **une autre case du plan que la première** tant que le plan n'a pas
  de mouvement (la case entière est alors le cadre A) ; sur un plan qui bouge déjà, la barre d'état dit que le mouvement se règle sur
  la première case.
- **Exports** : la planche exportée (PNG, JPEG, PDF) et le color script se servent de la même mise en page : mêmes cases, mais les
  écarts suivent la largeur des vignettes, le numéro de plan a plus de place et la dernière page s'arrête après la dernière rangée.

### Papier infini, cases pâles, empreintes de caméra et geste qui ne crée plus rien (2026-10-22, branche `vignettes-mouvements`)

Demandes d'Anthony (validées avec un croquis et un schéma), après les deux sections ci-dessus : **la marge autour des cases est
bonne**, mais c'était la maquette (12 % de la largeur de la case, de chaque côté) et non les 3 % du premier essai : `RING_FRACTION` vaut maintenant **0,12**.
Les vignettes composées en fond gardent dans la marge le dessin seul, sans papier ; la case courante peint les quatre bandes de sa marge. La marge dépasse
l'écart entre deux cases (10 / 136 dans un plan, 30 / 136 à une coupe) : elle recouvre un peu la voisine, et c'est la case peinte en dernier (ordre de la
planche, toujours le même) dont le trait se voit : rien ne clignote. Ce qui est dessiné hors du cadre (aplats de fond compris) se voit donc plus loin.
Le geste de caméra garde sa propre marge d'appui de 3 % (deux cases voisines ne se disputent pas l'appui). Code : `board.rs` (mise en page), `vignettecam.rs` (empreinte, liens, règles du geste),
`vignettecamui.rs` (geste, curseur, menu, dessin des cadres), `boardui.rs` (écran), `thumbs.rs` (une vignette par tuile), `planche.rs`
(export), `keymap.toml` (la touche), `workspace.rs` + `prefsui.rs` (le réglage). **Ce qui suit remplace** : la carte blanche, la case vide
unique en « + », la case de mouvement qui prend un bloc de cases entières, les cadres couleur du thème et la flèche de centre à centre,
« le dedans du dernier cadre fait naître le suivant » (« Mouvements de caméra dessinés dans les cases » plus haut).

- **Papier blanc infini** : plus de carte, plus de fond de table ; le papier remplit toute la vue. On le **déplace dans tous les sens**
  (molette, Espace, bouton du milieu, outil Main : plus de butée proche, seulement une borne très large) et on zoome comme avant. Les
  réglages de la planche restent la taille des vignettes, les **colonnes** (six d'origine) et le zoom ; **« Lignes par page » a disparu**
  (plus de pages à l'écran : une seule, sans fin ; l'export garde la valeur enregistrée).
- **Cases pâles, toujours visibles** (`BoardLayout::empty`, dans l'ordre de lecture) : un filet très léger, sans « + », un peu plus marqué sous
  la souris. **Au moins trois rangées de six cases en tout** (colonnes de la planche) et **toujours une rangée pâle après la dernière
  rangée occupée** : dessiner dans la dernière fait apparaître la suivante. Le minimum est un réglage : **Préférences > Vignettes > Planche
  des Vignettes > « Rangées de cases pâles »** (trois d'origine, de 1 à 12 ; `Layout::board_min_rows`, `rangees_pales` dans les réglages ; avant
  le 2026-10-22 il était dans l'onglet Montage).
  Dessiner (ou déposer des images) dans **n'importe quelle case pâle** fait le plan là ; **celles qui la précèdent deviennent des plans
  vides**, pour garder l'ordre de lecture ; une seule étape d'annulation (`Cible::Vide(e)`, déjà là, retrouvé).
- **Empreinte d'un mouvement de caméra** (`vignettecam::empreinte`) : la case garde sa taille (**jamais dézoomée**, jamais arrondie à un
  bloc de cases) ; ses cadres se montrent **à la même échelle** que les autres cases (un cadre A = une case). Quatre cas :
  - **travelling** : la case s'élargit **juste ce qu'il faut** dans le sens du mouvement, A et B côte à côte, une **fine flèche bleue au-dessus**
    de A vers B ;
  - **zoom arrière** : B entoure la case ; sa **hauteur est limitée à celle d'une rangée** (la case et sa bande de note, `Place::total`) :
    plus grand, B est dessiné **réduit** (« limite de taille », mêmes proportions, centre gardé) ; des **fines lignes bleues relient les coins**
    de A à ceux de B. Un zoom avant : B est dessiné dedans ;
  - **diagonale** : B passe dans la **rangée suivante** (la précédente si le mouvement monte), les coins reliés comme une boîte en perspective ;
  - **panoramique vertical** : B sous A, dans la rangée suivante, une **fine flèche vers le bas**.
  Une rangée de plus (au plus trois) fait une **tuile** de plus ; chaque tuile est une vignette composée sur un fil de fond
  (`Thumbs::texture_case` avec le numéro de rangée), le dessin de la feuille à l'échelle des cases, **hors de A voilé** (blanc translucide) ;
  la tuile courante est le dessin lui-même (on dessine dans celle où l'on appuie). Cadres : **traits fins bleus (#378ADD)**, lettres A, B, C…
  bleues, plus de boîtes jaunes. Un petit décalage vertical (moins que la bande de note) reste dans la rangée.
- **Les autres plans se rangent** : l'ordre de lecture est gardé (gauche à droite, rangée par rangée) ; chaque case prend la première place libre
  après la précédente, **à côté et dessous** d'une empreinte de deux rangées (les rangées sont des intervalles libres, pas des colonnes
  alignées) ; une empreinte qui ne tient pas dans la fin de la rangée passe tout entière à la suivante. La rangée d'une case à mouvement garde une
  place de plus sous elle (la bande de note descend d'autant), constante : rien ne bouge quand le cadre grandit.
  **Pas de tremblement pendant le geste** : la mise en page se refait en direct, mais (1) la **largeur réservée est arrondie au huitième de case** (un cadre qui dépasse à peine ne coûte rien) et
  (2) elle **ne rétrécit jamais tant que dure le geste** (`board::Reserve`, retenue dans le geste, `CamGeste::hold`) : un cadre qui va et vient
  autour d'un seuil ne fait pas sauter les voisines ; au lâcher tout se range d'après les cadres. (Choix expliqué : geler les voisines jusqu'au
  lâcher les aurait recouvertes par l'empreinte qui grandit ; ne rien retenir les aurait fait osciller.) Mesuré : 200 plans dont 20 mouvements, une
  mise en page prend environ 0,13 ms (essai `the_layout_of_two_hundred_plans_is_cheap`, borne 5 ms) : aucune allocation notable par image.
- **Le geste** (Ctrl + glisser, ou **T tenue**) :
  - **Ctrl + glisser est toujours un geste de caméra** dès que l'appui est sur une case, son empreinte ou sa marge (3 %) : il ne déplace
    jamais un plan et ne choisit jamais. La cause du « souvent ça déplace le plan » : un appui qui ratait la zone d'un cadre retombait sur le choix
    de plans. Maintenant l'appui qui rate prend le cadre le plus proche. Un **Ctrl + clic sans mouvement** choisit des plans comme avant ; **Maj +
    glisser le cadre de la case** déplace le plan, comme avant ;
  - **case sans mouvement** : le glissé crée B (déplacé ; ou zoomé s'il part près d'un coin de la case) ;
  - **case avec des cadres** : le glissé **ne crée jamais rien** : dedans un cadre il le déplace (le **plus petit qui contient le point** :
    B dans A, dedans B on déplace B, dans A hors de B on déplace A), à un coin il change son zoom, hors de tous il déplace le plus proche ;
  - **un cadre de plus** : **Ctrl + Alt + glisser** (ou T + Alt) en crée un qui part du dernier (déplacé, ou zoomé près de son coin), ou
    **« Ajouter un cadre »** au clic droit de la case (il se pose à côté du dernier, on le tire ensuite) ;
  - **retirer** : clic droit sur un cadre, **« Retirer ce cadre »** ; retirer B d'un mouvement à deux cadres enlève le mouvement ; les clés restantes
    se répartissent si elles l'étaient. Une étape d'annulation nommée chaque fois (« Mouvement de caméra », « Cadre C », « Cadre B »,
    « Zoom du cadre B », « Retirer le cadre C »).
- **T tenue** (`vignettes.camera`, comme Espace pour la main) : tant qu'elle est tenue, tout glissé sur une case est le geste de caméra, rien ne dessine,
  l'outil n'a pas changé (il est donc « de retour » au relâché). Pourquoi T : **T comme travelling** ; libre dans `keymap.toml`, dans le jeu « Façon
  Storyboard Pro », dans les touches écrites en dur (`Key::T` n'apparaît nulle part ; Alt+T est le cutter, Ctrl+T un nouvel onglet : d'autres
  modificateurs) et dans le fichier personnel d'Anthony (B, E, L, Maj+1, Maj+2) ; C (rotation de la vue), Z (concentration) et R (aide-mémoire) sont pris.
  Le mécanisme de `springtools.rs` change d'outil au relâché ; ici l'outil ne change pas, on lit la touche à chaque image (`Keymap::held_any`,
  qui ignore les modificateurs, pour que T + Alt reste tenue). **Aucun bouton de plus.**
- **Curseur** : Ctrl (ou T) tenue sur une case : une petite **caméra** bleue suit le pointeur, et le curseur est « déplacer » dans un cadre,
  « redimensionner » à un coin. Messages de la barre d'état courts.
- **Exports** (planche PNG / JPEG / PDF) : la même mise en page (sans les cases pâles, qui ne sont pas exportées) ; l'image de la case est celle de A,
  les **cadres bleus, leurs lettres et leurs liens** sont tracés sur le papier de l'empreinte (le dessin de la feuille hors de A n'y est pas :
  limite connue) ; marie-louise : les fenêtres sont l'image de A et les tuiles de l'empreinte. Le color script ne se sert pas de cette mise en page.
- **Limites** : la tuile d'une rangée de plus montre B à sa place sur la feuille mais alignée sur la rangée (le décalage vertical n'est pas
  conservé) ; trois rangées de haut au plus ; un cadre incliné compte par son rectangle englobant.

### Caméra libre entre les rangées, silhouette seule, tout l'espace se dessine (2026-10-22, branche `vignettes-camera-libre`)

Demandes d'Anthony (trois croquis annotés, puis ses retours sur la version précédente). **Ce qui suit remplace** de la section d'avant : les
quatre cas d'empreinte (travelling, zoom, diagonale, panoramique) rangés par rangées, la tuile par rangée, le voile hors de A, la largeur arrondie au
huitième de case, les liens « coins reliés », la limite « la tuile d'une rangée de plus est alignée sur la rangée ». Code : `vignettecam.rs`
(empreinte, enveloppe, liens, aimant), `board.rs` (`gabarit_de`, `poser`, `layout_with`), `boardui.rs` (`board_tuile`, `suivre_decalage`),
`vignettecamui.rs` (geste, aimant, cadres), `thumbs.rs` + `render.rs` (la tuile et son masque), `debord.rs`, `planche.rs` (export).

- **La caméra se pose n'importe où** : B (C, D…) est à sa **vraie place** par rapport à A, au point près : à mi-rangée, à cheval sur deux rangées, à
  n'importe quel décalage horizontal et vertical, dans tous les sens. Plus aucun arrondi à une rangée, une colonne ou un huitième de case
  (`empreinte` : chaque cadre = ses quatre coins sur la feuille, à l'échelle des cases, centrés sur A). Seuls restent : un cadre plus grand qu'une
  rangée (réduit, « limite de taille » comme avant) et un décalage plus grand que la planche (resserré : au plus une rangée de colonnes de large, trois
  rangées de haut).
- **Aimant, très doux** (`vignettecam::aimante`, `AIMANT` = **6 points d'écran** au zoom du moment) : le bord ou le centre du cadre qu'on déplace
  (B, C… ; ni A, ni un zoom) est attiré par un alignement **seulement quand il en est à moins de 6 points** : les bords et le centre des autres
  cadres, les colonnes de la planche (un pas de case et d'écart de coupe depuis A), le haut et le bas des vignettes de chaque rangée. L'attraction est
  progressive (l'écart devient écart × écart / 6) : le cadre **colle** tout près de l'alignement et rejoint le pointeur exactement au bord du seuil, donc
  **aucun saut** en entrant ni en sortant ; au-delà, le cadre suit le pointeur au point près. Chaque axe pour lui.
- **La case A reste en place pendant le geste ; l'ordre de lecture ne se casse jamais** (règle d'Anthony : de gauche à droite, rangée par rangée, les
  numéros de plan se lisent dans l'ordre ; elle passe avant « A ne bouge pas ») :
  - **pendant un geste de caméra** (B, C… tirés), A reste **exactement** où elle était à l'écran (`board::Reserve::pose`, retenue dans `CamGeste::pose`,
    `poser`) : l'enveloppe peut recouvrir un instant des cases d'avant, rien ne se range sous la main ; l'aimant ne la déplace jamais ;
  - **au lâcher**, tout se range d'après les cadres, dans l'ordre de lecture. La case se pose d'abord d'après sa seule vignette (sa place sans mouvement) :
    si l'enveloppe ne recouvre aucune case d'avant (elle grandit **vers la droite ou le bas**, même au-delà de la rangée : le papier est infini), **A ne
    bouge pas du tout** ; si elle recouvrirait des cases d'avant (elle a besoin de place **vers le haut ou la gauche**), la case passe **une fois**, au
    lâcher, à la première place où elle tient tout entière **après la précédente** : à droite sur la même rangée si elle y tient, sinon sur la suivante.
    Les cases d'avant ne bougent jamais ; celles d'après se rangent à côté et dessous de l'enveloppe ;
  - une enveloppe qui dépasse la page **en haut ou à gauche** fait glisser toute la planche d'autant (`BoardLayout::decalage`) et la vue **compense par
    le défilement** (`suivre_decalage`) : rien ne saute à l'écran ; à droite et en bas, la planche s'agrandit ;
  - deux enveloppes qui se recouvriraient : la plus tardive passe à la place suivante (comme une enveloppe qui recouvre une case d'avant) ;
  - jouer sur A elle-même (Ctrl + Maj + glisser dedans A) déplace le cadre A : le reste de l'empreinte bouge autour d'elle.
- **Prise des cadres qui se recouvrent** (`vignettecam::choisit`) : l'intérieur et les coins de B passent avant les coins de A. Les cadres en jeu sont
  ceux qui contiennent le point ou dont un coin est à portée ; **le plus petit l'emporte** (à aire égale, à un millième près : le dernier), puis, dans ce
  cadre, un coin à portée change son zoom, sinon on le déplace. Saisir près du coin de A en étant dans B déplace donc B (avant, le coin de A
  prenait le zoom de A et changeait l'échelle de toute l'empreinte).- **Les rangées d'une enveloppe** (`gabarit_de`) : pour chaque rangée de la planche que l'enveloppe touche, l'intervalle en x qu'elle y couvre ; la place
  se réserve rangée par rangée, **sans arrondi** : une empreinte peut finir **au milieu d'une rangée** ; les autres cases vont à côté (là où il y a de la place)
  ou une rangée plus bas. La hauteur des rangées dépend des notes (1 à 3 lignes) : la place se déduit en quelques passes (une enveloppe, ses rangées, les notes
  de ces rangées…) qui s'arrêtent dès que rien ne change. Une enveloppe peut mordre sur l'**écart** d'une case voisine (il n'est là que pour séparer deux
  vignettes), jamais sur la case. La note de A descend **sous** l'enveloppe quand elle passe sous la case.
- **Pas de tremblement pendant le geste** : l'étendue de l'enveloppe ne rétrécit pas tant que le geste dure (`board::Reserve` : la boîte déjà couverte, relative à
  A ; `CamGeste::hold`), au lâcher tout se range d'après les cadres.
- **Lignes : la silhouette seule** (`vignettecam::liens`) : les cadres eux-mêmes (A, B, C… avec leurs lettres, fins, bleus) et, entre deux cadres, **seulement les
  côtés de l'enveloppe convexe de tous leurs coins** ; jamais une ligne qui traverse l'intérieur (la diagonale de A à B qui coupe A ou B a disparu ; un zoom qui entoure
  A n'a plus de lignes). Une **flèche** reste pour un travelling pur (même taille, de côté : au-dessus ; de haut en bas avec un espace entre les cadres). Les cadres
  dont les coins se confondent (une caméra à la main : des dizaines de clés presque au même point) ne se redessinent pas : **leurs lettres s'empilaient en un petit carré bleu
  dans le coin de la case** (`cadres_distincts`, seuil 2,5 points).
- **Toute l'enveloppe est la zone de dessin** : A, B et l'espace entre eux se dessinent comme une case (`MoveBoard::contient`, `Hit::Thumb`). Le dessin **vit sur la
  feuille du plan** (coordonnées de la feuille : les cadres ne sont que des fenêtres) : **déplacer un cadre ne déplace pas le dessin**. Une seule **tuile** couvre l'enveloppe
  (`board_tuile`, une vignette composée sur un fil de fond à l'échelle des cases, le papier seulement dans l'enveloppe : `render::thumbnail_marge_until`, paramètre `masque`) ;
  le plan courant y est le dessin lui-même (le trait tombe sous le stylet, sans recomposition sur le fil de l'interface), le trait qui sort de l'enveloppe se voit pâle pendant le
  geste (`debord.rs`, zone polygonale) puis dans la marge de la tuile. **Aucun voile** : le dessin est net partout dans l'enveloppe (A, B et entre eux).
- **Caméra qui tourne ou penche** : un cadre est le quadrilatère de ses quatre coins : une caméra tournée (`Camera::angle`) se voit **tournée** (même taille, 120 × 67,5 à 30° dans
  l'essai), une caméra penchée un trapèze ; l'enveloppe, les liens, la prise des coins et le glisser les suivent. (Avant : le rectangle englobant, plus grand.)
  Le voile corrigé du dessin en grand (`viewbar::nested_band`) n'est plus nécessaire ici : plus de voile ; ses morceaux convexes (`difference_convexe`) servent à masquer les coins
  de la tuile hors de l'enveloppe.
- **Export** (planche) : les cadres sont tracés comme quadrilatères, avec les côtés de l'enveloppe ; la page s'agrandit pour tenir l'empreinte.
- **À confirmer avec Anthony** : (1) A passe après la précédente au lâcher quand l'enveloppe recouvre des cases d'avant (décidé : l'ordre de lecture gagne) ; (2) les silhouettes entre deux
  cadres **non consécutifs** (A–C) se dessinent aussi quand ce sont des côtés de l'enveloppe ; (3) l'aimant à 6 points ; (4) zoomer A change l'échelle de l'ensemble (l'échelle est celle de A : une case) ; (5) le tremblé de caméra (clés presque confondues) montre un seul cadre.

Essais : `vignettecam_tests` (cadres à mi-rangée sans arrondi, caméra tournée, enveloppe, liens = côtés de l'enveloppe sans ligne intérieure, aimant : près / loin / sans saut),
`vignettes_camera_tests` (la case A identique à l'écran avant, pendant et après un glissé de B, en haut à gauche aussi ; l'aimant à l'écran ; rangement autour d'une
empreinte qui finit à mi-rangée ; 200 plans dont 40 mouvements à mi-rangée et des notes de 1 à 3 lignes : aucun recouvrement, moins de 5 ms ; une caméra à la main
ne pose qu'une lettre et ne voile rien). Captures : `vignettes\b1.png` (B à mi-rangée), `b2.png` (traits dans A, entre A et B, dans B), `b3.png` (B déplacé : les traits n'ont
pas bougé), `c1.png` (B vers le haut et la gauche, au lâcher : A passe après la précédente, l'ordre de lecture tient), `c-pendant.png` (même geste, pendant : A n'a pas bougé).
### Deuxième tour : affichage des tuiles, marges, copie d'un plan, lasso, caméra de continuité, coupes (2026-10-22, branche `vignettes-retouches2`)

Demandes d'Anthony (captures `bug-cam-trait`, `demande-marge-serree`, `71`, `72`). Code : `thumbs.rs` (`Geom`), `boardui.rs` (`board_tuile`, `board_card`,
`peint_texture_dans`, `marge_ouverte`), `vignettecam.rs` (`elargi`, `etendue_du_mouvement`, `recale`), `boardmove.rs` (copie), `springtools.rs`
(`prendre_le_lasso`), `storyboard.rs` (le montage). Essais : `vignettes_retouches2_tests`.

- **Les traits coupés à des lignes droites après un déplacement de B — cause et correction.** La tuile d'un mouvement de caméra est une image composée sur un
  fil de fond pour **une géométrie précise** (la part de la feuille qu'elle couvre et l'enveloppe qui borne son papier). À chaque image du geste la géométrie
  change : la tuile se recompose, et pendant ce temps l'écran continuait à poser **l'ancienne texture sur le nouveau rectangle** (`painter.image(c.rect(r))`),
  avec ses bords de coupe, sa marge et son papier d'avant le geste. Maintenant chaque texture **porte sa géométrie** (`thumbs::Geom` : la zone et un
  résumé de l'enveloppe, rangés avec l'image) :
  - **pendant le geste et jusqu'à ce que la tuile soit refaite** : le plan courant est le dessin vivant, qui montre **tout le trait dans la tuile** (rien n'est coupé à une
    ligne d'avant le geste) ; la texture d'avant n'est **pas posée comme si elle était la bonne** (ni le papier hors de l'enveloppe, ni la marge). Rien n'est recomposé
    sur le fil de l'interface ; la marge réapparaît dès que la tuile d'après est prête (quelques images) ;
  - **un plan qui n'est pas courant** : sa texture est posée **là où est la feuille qu'elle montre** (`CaseTex::rect_tuile`), jamais étirée sur la nouvelle tuile ;
  - **au lâcher** : une seule recomposition en fond, et l'affichage est exactement celui de la nouvelle géométrie.
  Le cache est aussi plus juste : la marge d'une tuile est **12 % de la largeur d'une case** (comme celle des autres cases), et non 12 % de la tuile entière (53 points au lieu de 17
  sur une tuile de trois cases : les traits étaient coupés loin de l'enveloppe) ; elle fait partie de la clé de la vignette.
- **La marge de 12 % ne s'ouvre que pour le plan où l'on dessine** (le plan courant). Les autres plans sont **coupés net à leurs cadres** (l'enveloppe, ou la case d'un plan sans mouvement)
  **plus quelques points** (`MARGE_SERREE` = 4 points d'écran, comme le liséré rouge d'Anthony). Même vignette en cache, simplement peinte coupée (`peint_texture_dans` : un maillage dont les
  coordonnées de texture suivent l'enveloppe élargie `vignettecam::elargi`, ou un rectangle de la même texture) : aucune recomposition, la bascule est instantanée quand on commence à dessiner
  dans un plan (il devient courant) comme quand on le quitte.
- **Maj + Alt + glisser le cadre d'une case duplique le plan** et glisse la copie ; l'original reste net et en place, la copie se pose à l'endroit du lâcher **dans l'ordre de lecture**
  (n'importe quel repère entre deux plans, y compris devant l'original ; `board::nearest_slot_copie`), en **une étape d'annulation** (« Dupliquer le plan »), avec la copie de
  « Dupliquer le plan » du menu (`duplicate_scene_after` : dessins, clés, notes). Plusieurs plans choisis : tous dupliqués. Maj + Alt était libre à cet endroit (Alt n'avait pas de sens avec Maj + cadre).
- **Ctrl seul + glisser est le lasso** (comme partout, `springtools.rs`) : il se prend à l'**appui** sur une case (`prendre_le_lasso` : la case devient courante, l'outil Sélection est pris,
  l'outil d'avant revient au relâché de Ctrl), jamais au simple survol, pour qu'un Ctrl tenu avant Maj n'arme jamais le ressort. La boîte de sélection, ses poignées et ses déplacements sont ceux du dessin
  (le dessin de la case est le dessin lui-même, en orange comme ailleurs). Ctrl + clic ne choisit plus de plans : Maj + clic reste le choix.
- **La caméra des Vignettes : Ctrl + Maj + glisser** (les deux ordres, `board_press` lit les touches de l'appui) **ou T tenue** (`vignettes.camera`, réaffectable) ; **Ctrl + Maj + Alt** (ou T + Alt) : un cadre de plus, comme
  « Ajouter un cadre » du clic droit. Ctrl + Maj ne prend ni le lasso ni le geste « tous les calques » ; un Ctrl + Maj + clic qui ne bouge pas ne fait rien. Maj seul + cadre : déplacer le plan.
- **Les cases qui suivent un mouvement montrent le dernier cadre** (« B, C… ») : le geste répartit les clés sur **la durée de la première case** du plan (jusqu'à sa marque d'image suivante,
  `vignettecam::etendue_du_mouvement`) et non plus sur tout le plan ; la caméra tient sa dernière clé, donc les cases suivantes du plan (la vignette, le dessin vivant, la lecture) voient à
  travers le dernier cadre. Des clés posées avant, sur tout le plan, se rangent sur la première case au prochain geste (`recale`) ; des clés à la main restent.
- **Une coupe décochée ne change que celle-là.** Cause probable (non reproduite en logique, que les essais couvrent) : un stylet double l'événement de la souris (deux appuis dans la même image d'écran) ; le second,
  rejoué sur une planche que le premier venait de réorganiser, tombait sur la barre d'à côté. Un seul appui du bouton principal par pression (`board_view`). Essais : coupes une à une avec une ou plusieurs
  cases et un mouvement de caméra, recocher, annuler, appui doublé.
- **Le montage ne dessine plus les limites des cases** (le repère orange avec un losange au bord de la fusion) : la marque d'image reste dans les données (les Vignettes en ont besoin), elle se voit toujours
  dans la timeline du plan (Animation), où on la déplace ; le montage ne la prend plus.
- **Les notes d'un plan réuni** restent à leur case, dans l'ordre du temps : les textes (action, dialogue, notes) de Q vont à la marque d'image qui commence là où Q commençait, ceux de P restent à P ;
  séparer à nouveau les rend à Q ; annuler redonne les originaux. Rien ne se perd, aucun saut de ligne en trop.

### Le décor partagé entre les cases, ses brosses, et l'onglet Vignettes des Préférences (2026-10-22, branche `vignettes-decor`)

Demandes d'Anthony. Code : `decor.rs` (le calque et le partage, sans interface), `decorui.rs` (les pastilles, le choix, l'acheminement des traits),
`vignettesreglages.rs` (les réglages), `prefsui.rs` / `prefsonglets.rs` (l'onglet). Essais : `decor_tests.rs` (33), `cases_tests.rs` et `notes_cases_tests.rs`. Vocabulaire : **Décor** (jamais « Fond » : le mot
est pris par l'aplat sous le trait et par l'ancien calque de fond, retiré le 2026-10-21 ; en anglais ce sera « Background »).

- **Le calque « Décor ».** Un par plan, **tout en bas** de ses calques (sous les personnages et les traits), créé par **le premier trait d'une brosse du décor** dans
  ce plan, et nommé « Décor ». Ce n'est pas l'ancien « calque de fond » (retiré) : c'est un calque **bitmap ordinaire** qu'un drapeau distingue (`Layer::decor`). Il est tenu sur
  toutes les images du plan par **un seul dessin** et porte le post-comportement « Maintenir » (il suit le plan s'il s'allonge).
- **Le même décor dans toutes les cases d'une continuité.** Les cases d'un plan (la première image, puis ses marques d'image) montrent toutes ce dessin : un trait de décor
  fait dans une case se voit dans toutes les autres (c'est le même dessin), et **seulement lui** : les calques des personnages gardent leurs dessins case par case. Les vignettes
  des cases liées se refont en fond par la clé de leur cache (elle porte la version de chaque dessin). Avec un mouvement de caméra, les cases qui suivent voient le décor à
  travers le dernier cadre (la caméra tient sa dernière clé, comme avant).
- **Couper et réunir.** Cocher la coupe (le plan se coupe, `split_panel`) : **le plan d'après a sa propre copie du décor**, modifier l'un laisse l'autre. Décocher la coupe
  (les deux plans se réunissent, `Document::join_next_plan`) : avec **« Report du décor »** allumé (d'origine), le décor du premier plan est tenu sur toute la suite, celui du
  second (s'il en avait un) cède la place ; si le premier n'avait pas de décor, celui du second reste à sa place dans le temps. Éteint, la case suivante repart d'un décor
  **vide** (ou de son propre décor). Le réglage n'est lu qu'à la jonction (rien ne change en le basculant) ; une étape d'annulation par coupe, comme avant.
- **Les brosses : trois pastilles dans la colonne des pastilles de la planche**, après un petit trait, sous les couleurs neutres : **crayon**, **feutre gris**, **aplat**. Des carrés
  arrondis (pour ne pas les prendre pour des personnages, qui sont ronds) de la couleur du décor, **gris-bleu clair #8FA3B8** d'origine, avec le signe de la brosse dedans (un trait
  fin, un trait épais, un carré plein). Pourquoi là : la colonne est déjà le « avec quoi je dessine ? » de la planche (personnage, couleur neutre, couleur libre) ; le décor y est
  un choix de plus, sans rien ajouter à la barre d'outils ni à la planche. Elles n'existent que dans les Vignettes. Clic : choisir (clic sur la choisie : la lâcher) ; clic droit :
  « Régler les brosses du décor… » (ouvre Préférences > Vignettes) ; bulle d'une seconde avec le nom et ce que fait la brosse.
- **Ce que fait le choix.** La brosse prend l'outil qu'il faut (Crayon pour le crayon, Pinceau pour le feutre et l'aplat), son réglage à la taille des Préférences (crayon graphite
  de 5, feutre de 24, grande brosse pleine de 90) et la couleur du décor. L'outil, les deux pinceaux, la couleur et le calque d'avant sont mis de côté et **reviennent quand on lâche** le décor
  (sa pastille, « aucun personnage », un personnage, un autre calque choisi à la main, une autre couleur prise à la main ; quitter les Vignettes). **À l'appui d'un trait dans une
  case**, le calque Décor du plan est créé au besoin et devient courant : le trait va dedans **quel que soit le calque courant**. Le calque et son premier trait ne font qu'une étape
  d'annulation (« Dessiner le décor »), puis une étape par trait ; un plan fait en dessinant dans une case vide avec une brosse du décor reste une seule étape.
- **Fichier.** Aucun changement de `VERSION` (reste 36) : le calque s'écrit comme les autres, plus un champ `decor` (absent : calque ordinaire) dans l'entrée du calque, que lisent aussi
  les fichiers de plan d'un film partagé. Un projet d'avant s'ouvre tel quel (aucun calque Décor) ; une version d'avant ouvre un projet à décor sans y voir de différence (un calque
  « Décor » ordinaire, sans le drapeau : réenregistré par elle, il perdrait son rôle). Le décor d'une continuité est **un seul dessin dans les expositions du calque** (une exposition de
  toute la durée du plan) : c'est ce qui le garde partagé à la réouverture.
- **Préférences > Vignettes (nouvel onglet, 4 carrés)** : ce qui se règle de la planche, et rien d'autre. Rangées de cases pâles (venu de Montage), Cases par rangée, **Débord du trait hors
  des cases** (12 % d'origine, de 0 à 40 %, `thumbs::Thumbs::debord` : il fait partie de la clé des vignettes, elles se refont en fond), **Aimant des cadres de caméra** (6 points d'origine,
  0 : éteint), **Couleur des cadres de caméra** (#378ADD d'origine : à l'écran et dans l'export de la planche), **Report du décor**, **Couleur du décor**, **Taille des brosses du décor**
  (trois valeurs), et « Rétablir les réglages des Vignettes ». Ce sont des **réglages de l'application** (`reglages.toml`, table `[vignettes]`, absente tant qu'on n'y touche pas ; les rangées et
  les colonnes restent dans la disposition, comme avant), jamais du projet : seul « Report du décor » agit sur le projet, au moment de la jonction (`Document::report_decor`, posé par
  `structure_op_porte`). Ils jouent tout de suite.
- **Un défaut trouvé en chemin, corrigé** : la colonne des pastilles était un `ScrollArea` dans une `Area`, et egui ne laisse jamais grandir un `ScrollArea` au-delà de la taille que
  son `Area` avait à l'image d'avant : une colonne qui s'allonge (un personnage de plus ; les brosses du décor en passant du Dessin aux Vignettes, l'`Area` étant la même) restait coupée à
  son ancienne hauteur, ses dernières pastilles (et le « + ») hors de portée du pointeur. Elle ne défile plus que si elle ne tient pas (`pastilles_colonne`).

### Une nouvelle case = une nouvelle clé de dessin ; la case vivante après un travelling (2026-10-22, même branche)

- **Clé de case** (`cases.rs`, `Document::cles_de_case`). Les cases d'un plan sont ses **marques d'image** ; les clés de dessin sont les **expositions des calques**. Deux choses
  séparées : poser une marque ne touchait pas aux calques, la nouvelle case montrait le dessin d'avant, **tenu**, et le retoucher retouchait aussi la case d'avant. Maintenant, quand une
  case est faite — dessiner dans la case pâle qui suit le plan (barre grise), décocher une coupe (deux plans réunis), la commande « Marque d'image » —, **chaque calque de dessin
  reçoit une clé à l'image de la case** : un calque dont le dessin est tenu en reçoit **une copie** (choisi parce que c'est ce que la case montrait déjà : rien ne saute à l'écran ; vide, la
  case aurait perdu son dessin), un calque qui avait un dessin à la case d'avant mais rien ici (le plan réuni n'avait pas ce calque) reçoit un **dessin vide**. Rien pour un calque encore
  vide, un dossier, un calque d'effet ou vidéo, un calque de couleur (il suit son calque de trait) — et **rien pour le calque Décor** (un seul dessin pour toute la continuité). Dans la
  même étape d'annulation que la case : une annulation défait la case et ses clés (et le trait qu'on y a fait en la créant).
- **La case vivante d'une continuité après un travelling** (`canvas.rs`). Cause : la case dont on est en train de dessiner l'image (la seule « vivante ») est un vrai dessin à l'écran, et
  une case des Vignettes **ne composait pas le monde hors de la feuille** (`world`, toile infinie). Les cases qui suivent un mouvement voient à travers le dernier cadre B, qui peut sortir
  de la feuille (qui déborde d'une demi-largeur de cadre) : le dessin s'arrêtait net au bord de la feuille, le reste de la case restait blanc, alors que la vignette (composée par
  `thumbnail_marge_until`, qui lit tout) montrait tout. La case vivante compose maintenant le monde quand elle voit à travers la caméra (`camera_view`) : une seule case à la fois.
  Essai : `cases_tests::the_live_case_after_a_travelling_shows_what_lies_beyond_the_sheet_where_camera_b_looks` ; captures `travelling-avant-c.png` (cassé) et `travelling-apres-c.png`.

### Les notes des cases ne s'écrivent qu'au bouton (2026-10-22, même branche)

« Les notes ne doivent s'activer qu'en cliquant sur un bouton, sinon ça empêche parfois de dessiner. » Avant, un appui sur la bande de note sous une case l'ouvrait ; avec le débord
de 12 %, un trait qui partait du bas d'une case tombait dans la bande et ouvrait la saisie au lieu de dessiner. Maintenant :
- **Un appui sur la bande (ou sur son texte) dessine** dans sa case, comme n'importe où près d'elle (`Hit::Note` se comporte comme `Hit::Thumb`, `Vignettes::appui_bande` fait accepter
  au dessin vivant un appui qui tombe sous le cadre) ; le curseur est celui de l'outil.
- **Pour écrire : une petite icône `NOTE_PENCIL` au bout (à droite) de la bande** (`note_icone_rect`). Elle ne se montre que **pointeur sur la case ou sur sa bande** (pas pendant un
  geste), **et toujours sur la case dont la note s'écrit** : la planche reste nette. Un appui dessus ouvre la note de cette case, texte compris, curseur au bout ; Échap ou un clic ailleurs
  la ferme, comme avant. Bulle d'une seconde par `bulles.rs` : « Écrire la note ».
- **Au clavier** : la commande `vignettes.note` (« Écrire la note de la case courante », sans touche d'origine, comme les autres commandes des Vignettes ; Ctrl+K ou l'assigner dans
  Préférences > Raccourcis).
- Le texte des notes reste visible comme avant (trois lignes, puis « … »). Essais : `notes_cases_tests.rs` (4) ; ceux de `vignettes_tests` qui ouvraient la bande par un clic
  passent par l'icône (`icone_note`).

### Un mouvement de caméra créé sur une case qui a de la continuité derrière elle (2026-10-22, même branche)

Décision d'Anthony : un plan a deux cases (la case 1 et une case de continuité 2), chacune son dessin ; il crée un mouvement (Ctrl + Maj + glisser) sur la case 1. Avant, le dessin de la case 2
disparaissait derrière le cadre B. Maintenant, **les cases qui suivent forment un plan à part** (`Document::scinder_apres_mouvement`, `cases.rs`) :
- **une coupe juste après la case qui porte le mouvement** (le plan se coupe à la première marque d'image, `split_plan`) ; le mouvement ne montre que le dessin de la case 1 ;
- **le plan neuf garde les dessins des autres cases tels quels** (des copies, comme toute coupe) **et la caméra qu'elles montraient avant** (les clés d'avant le mouvement : le plan d'origine
  n'en avait aucune ou une seule) : leurs vignettes ne changent pas d'un pixel, avec une barre de coupe devant ; **le décor** a, dans le plan neuf, une copie indépendante (comme en recochant
  une coupe) ;
- **le mouvement et la coupe ne font qu'une étape d'annulation** (« Mouvement de caméra ») : un Ctrl+Z rend un seul plan de deux cases, sans mouvement ;
- **aucune copie automatique** : si Anthony veut la case 2 dans la continuité du mouvement, il copie son dessin dans le cadre B lui-même ; **déplacer B ensuite ne déplace jamais un dessin** (il
  change seulement ce que la caméra cadre), le plan voisin n'est pas touché ;
- la règle « la case qui suit montre le dernier cadre » **reste** pour les cases de continuité faites **après** le mouvement (dessiner dans la case pâle qui suit l'empreinte, sans coupe) ;
- ça vaut aussi pour « Ajouter un cadre » (clic droit) sur une case sans mouvement ; sur un film verrouillé ou sans le jeton du montage, le geste est refusé avec son message (une coupe change le
  montage).
Essais : `cases_tests` (le plan se coupe, dessin et caméra de la case 2 inchangés, vignette identique pixel pour pixel, une étape d'annulation, déplacer B ne touche pas le plan voisin ; la case
faite après le mouvement voit toujours B). L'essai d'avant `the_case_after_a_camera_move_shows_the_last_frame_and_moving_b_updates_it` ajoute désormais la case de continuité après le mouvement.

### Un zoom ne rétrécit jamais la case A (2026-10-22, même branche)

« Lors d'un zoom, c'est le cadre A qui rétrécit » (un zoom arrière : A devenu petit, B de la taille de la case). **Cause** : un coin tiré sur un plan qui a déjà un mouvement prenait
`Prise::Zoom(0)` quand le coin de A était à portée ; or, après un zoom arrière, B entoure A et son coin est à quelques points de celui de A : « le plus petit cadre l'emporte » donnait A. Le
geste changeait **la caméra de A** (sa clé 0, ici de 1 à 0,62) ; comme **A fixe l'échelle de toute l'empreinte** (A = la case, `vignettecam::empreinte`), tout se redessinait à une autre
échelle : A semblait avoir rétréci, B reprenait la taille de la case. **Règle** (`vignettecam::choisit`) : un coin de A ne zoome pas A quand le point est aussi dans un autre cadre (ou près de
son coin) : on prend le plus petit des autres (son coin le zoome, sinon on le déplace) ; le coin de A reste saisissable quand rien d'autre n'est là. **Sur une case sans mouvement**, un coin
tiré fait B, copie de A zoomée (`NouveauZoom`) autour de son centre, A inchangé (déjà le cas, maintenant couvert). **Un zoom arrière** : B plus grand que A autour de lui, limité à la hauteur de
rangée, jamais A rétréci. Essais : `cases_tests` (zoom arrière, zoom arrière large et zoom avant sur une case sans mouvement : A est exactement la case d'avant, B plus grand ou plus petit ; un
deuxième coin tiré au coin de A que B entoure zoome B, A reste la case et sa caméra ne change pas, feuille deux fois plus grande que le cadre ou de sa taille).

### À regarder à l'écran (le décor, par Anthony)

1. Vignettes, colonne des pastilles : après les trois couleurs neutres et un petit trait, trois carrés arrondis gris-bleu (un trait fin, un trait épais, un carré plein), avant le « + ».
2. Choisir le feutre : la pastille s'allume, le pinceau devient un feutre gris-bleu. Dessiner dans une case : un calque « Décor » apparaît tout en bas du plan (panneau Calques), le trait y est, quel que
   soit le calque d'avant ; Ctrl+Z défait le calque et le trait d'un coup. Cliquer de nouveau sur la pastille : le pinceau, la couleur et le calque d'avant sont revenus.
3. Un plan de plusieurs cases (barre grise entre elles) : un décor dessiné dans la deuxième case se voit aussi dans la première et la troisième, avec leurs vignettes ; les personnages dessinés
   dans une case ne passent pas dans les autres.
4. Cliquer une barre noire entre deux plans (décocher la coupe) : la case de droite montre le décor de la case de gauche. Recocher : la case de droite a sa copie, qu'on peut retoucher sans
   changer l'autre. Préférences > Vignettes > Report du décor éteint, puis décocher : la case de droite repart sans décor.
5. Préférences > Vignettes : changer le débord (les traits qui sortent des cases en montrent plus ou moins), l'aimant à 0 (un cadre de caméra tiré ne se colle plus), la couleur des cadres
   (cadres, lettres et flèche de la planche et de son export), la taille et la couleur des brosses du décor.

### À regarder à l'écran (papier infini et caméra)

Captures de l'essai : `vignettes\m-trav.png` (travelling : A, B, flèche au-dessus, les autres cases décalées), `m-zoom.png` (zoom arrière limité à la
hauteur de la rangée, coins reliés), `m-diag.png` (diagonale : B dans la rangée suivante, boîte en perspective, les cases suivantes se rangent à côté et
dessous), `m-tilt.png` (panoramique vertical : B sous A, flèche vers le bas), `m-vide.png` (projet neuf : trois rangées de six cases pâles).
1. Papier : blanc sans bord, déplaçable partout ; zoom comme avant ; plus de « Lignes par page » dans la petite barre.
2. Dessiner dans une case pâle lointaine : les plans vides avant elle, une étape d'annulation ; dessiner dans la dernière rangée : une rangée pâle de plus.
3. Ctrl + Maj + glisser n'importe où sur une case, y compris sur sa marge ou à côté d'un cadre : jamais un plan qui bouge, jamais un lasso. T tenue : même chose, sans Ctrl ni Maj, avec
   n'importe quel outil ; relâchée, le pinceau dessine de nouveau. Alt en plus : un cadre de plus. Ctrl seul : le lasso.
4. Une fois B posé : Ctrl + Maj + glisser dedans B le déplace (sans créer C), dans A hors de B déplace A, un coin change le zoom ; clic droit : « Ajouter un
   cadre », et sur un cadre « Retirer ce cadre ».
5. La petite caméra bleue près du pointeur (Ctrl + Maj ou T tenue), le curseur « déplacer » dans un cadre et « redimensionner » à un coin.
6. Exporter la planche avec une case à mouvement : cadres bleus, lettres et liens sur le papier de l'empreinte.

### Anciens projets (`crayonnes.json`)

SHOT ne lit plus ce fichier et ne l'écrit plus, mais **ne l'efface jamais** (octet pour octet, après ouverture et
enregistrements) ; « Enregistrer sous » ne le recopie pas (l'original reste dans son dossier). À l'ouverture, **un seul
avertissement** dit combien de cases dessinées n'avaient jamais servi à faire des plans et que le fichier reste en place ;
rien n'est converti tout seul. Pour retrouver ces croquis : les redessiner dans une case vide, ou rouvrir le projet avec la
version d'avant (`crayonnesheritage.rs`).

### À regarder à l'écran

1. Les filets (clair / foncé pour l'image courante) et le numéro du plan dans les trois thèmes : lisibles sur la page blanche, rien
   d'autre autour des images.
2. Les barres : grise très discrète, noire nette, d'un tiers de l'image, centrées dans l'écart ; une barre au bout de chaque ligne ;
   la bulle sous la souris.
3. L'écart : serré (10) dans un plan, large (30) à une coupe ; six colonnes sur un petit écran.
4. Dessiner dans une image qui n'est pas courante, sans clic avant : le premier point est déjà là, aucun saut ; le trait qui
   sort de l'image **se voit en entier pendant le geste** (par-dessus l'écart, la barre, la note, l'image voisine) ; **au lâcher il
   disparaît du dehors** (l'image est coupée à son cadre) ; un trait qui part du fond ne trace rien.
5. Dessiner dans la première case vide : le plan apparaît à sa place ; avec la barre grise, une image de plus ; Ctrl+Z défait
   les deux d'un coup.
6. Cliquer une barre : coupe ou réunion, puis Ctrl+Z.
7. Les bandes de note : une, deux, trois lignes et « … » ; la ligne suivante descend sans toucher la note ; la saisie montre tout le
   texte ; le filet pâle sous une bande vide ; la taille du texte à 100 %, 50 % et 200 % de zoom.
8. À la tablette : la pression et le retard du trait dans une vignette, comparés à la vue Dessin.
9. Le débord : un long trait qui part d'une image, traverse l'écart et finit dans la voisine (et au-delà du bord de la page) :
   il reste attaché à la croix, sans saut au bord de l'image, avec les mêmes couleur et épaisseur dehors que dedans ; à la
   tablette, la pression se voit dehors ; une ligne, un rectangle, une ellipse tirés jusque dans l'image voisine ; au lâcher, plus
   rien dehors. Rouvrir le plan en Dessin : le trait entier y est.
10. Maj tenue au-dessus du cadre d'une image (image courante comprise) : la main, pas le cercle du pinceau ; au-dessus du dedans :
   le curseur ordinaire.
11. Maj + glisser le cadre d'une image d'un plan de **trois** images : tout le plan suit (fantôme de ses trois images), le trait
    d'accent n'apparaît qu'entre deux plans (jamais entre deux images d'un même plan), lâcher : le plan est là, ses trois
    images ensemble, Ctrl+Z puis Ctrl+Y ; Échap en plein geste : rien ne bouge ; lâcher près de sa place : rien ne bouge.
12. Sortir une image d'un plan : cliquer sa barre (elle devient noire), puis Maj + glisser son cadre. Un plan déposé dans une
    autre séquence : la barre d'état le dit. Les trois thèmes : le trait d'accent et le fantôme restent lisibles.
13. Ctrl + glisser le cadre d'une case d'un plan de plusieurs images, vers la droite d'une largeur de case : B apparaît, la case
    s'élargit en direct (2 × 1), les voisines se décalent, la lettre A et la lettre B sont lisibles, la flèche va de A à B ;
    lâcher, Ctrl+Z (la case redevient ordinaire), Ctrl+Y.
14. Recommencer depuis l'intérieur de B : C (3 × 1, ou retour à la ligne de la case entière si elle ne tient pas dans la rangée) ;
    glisser vers le bas : D (rangées de plus). Ouvrir le montage : les clés A, B, C, D sont sur la piste Caméra, réparties ; supprimer
    une clé : la case rétrécit.
15. Tirer un coin d'une case ordinaire (Ctrl) : un cadre plus large ou plus serré, le curseur change aux coins ; tirer le bord d'un
    cadre existant : il se déplace ; Échap en plein geste : tout revient.
16. Dessiner dans la grande case, y compris dans la partie hors de la feuille ; changer de plan courant : la grande case garde son
    dessin (vignette de zone) ; les trois thèmes (cadres et lettres lisibles sur papier blanc comme sur dessin).
17. Maj + glisser le cadre d'une grande case : le plan entier se déplace, ses clés avec lui.

### Essais ajoutés

`vignettes_tests` (23 : la planche de crayonnés n'a plus de commande, un ancien `crayonnes.json` s'ouvre et reste intact,
dessin direct dans une image pleine, dans une marque et dans une case vide, trait hors des images, plans sautés, clic sans
trait, barre grise → image, coupe / réunion nommées et annulables, barre de bout de ligne, notes du plan et de la marque,
trois lignes et « … », numéros seuls, Ctrl / Maj, clic droit, storyboard verrouillé, fusion d'étapes d'annulation, coût du
trait, repos) et `board_tests` (mise en page : écarts, barres, hauteur des lignes, zones touchées). La planche de crayonnés
(38 essais) et sa part du projet d'essai de fiabilité sont partis avec elle.

`vignettes_retouches_tests` (15, retouche du 2026-10-21) : le trait se voit dehors pendant le geste (par-dessus l'écart, la
barre et l'image voisine, jusqu'à la pointe), rien dehors au lâcher et le dessin garde tout (hors de la feuille compris) ; l'aperçu
d'une forme déborde aussi ; coût par image d'un trait qui sort comparé à un trait qui reste dedans (affiché), et calcul du
débord seul ; le cadre est le filet à cinq points près (et pas le centre) ; Maj + clic dedans choisit toujours, sur le cadre il
prend le plan ; la main sur le cadre, image courante comprise ; le repère n'est jamais dans un plan (3, 4 et 6 colonnes, tous
les points de la planche) ; tirer une image d'un plan de trois déplace tout le plan, intact (marques, notes, clés de caméra),
une seule étape « Déplacer le plan » annulable et rétablissable ; plan d'une image, sortie d'un plan par sa barre coupée ;
Échap ; lâcher en place ; autre séquence et storyboard verrouillé ; plans choisis ensemble ; fantôme, repère et curseur ;
repos après un trait qui déborde et un déplacement.

`vignettecam_tests` (10, logique) et `vignettes_camera_tests` (24, écran) : taille de la case déduite de l'union des cadres
(2 × 1, 1 × 2, 2 × 2, 3 × 1, plafonds), zone centrée à l'échelle de A, répartition des clés (B à la fin, C au milieu, D), clés à la
main respectées, prise des coins, bords et intérieur, geste qui ne s'accumule pas, noms des étapes ; Ctrl + glisser crée A et B aux
bornes du plan (une étape, « Mouvement de caméra », annulable), C et D répartis, bord = déplacer, coin = zoom, coin d'une case
ordinaire = B zoomé, Ctrl + clic qui ne bouge pas, Échap, storyboard verrouillé, plan d'une image ; mise en page (largeur, retour à
la ligne entier, voisines, aucun chevauchement en 1 × 2 et 2 × 2, pages, repères de pose, case qui rétrécit) ; dessin hors de la
feuille dans la grande case ; vignette de zone ; le montage voit les clés ; Maj + cadre déplace toujours le plan ; repos.

`vignettecam_tests` (21 : empreintes des quatre mouvements, limite de hauteur, liens, clés, prise des cadres : le plus petit l'emporte, Alt ajoute, retrait),
`vignettes_camera_tests` (écran : le geste crée B, déplace B sans rien créer, déplace A hors de B, coin = zoom, Ctrl + Alt = C, menu, appui qui rate = caméra
jamais plan, T tenue, curseur, mise en page des quatre mouvements et reflux, largeur retenue pendant le geste), `vignettes_maquette_tests` (papier, trois
rangées de six, une rangée pâle après la dernière occupée, dessiner dans la dernière, 200 plans), `vignettes_tests` (case pâle lointaine = plans vides avant).

### Annotations, images déposées, marie-louise et exports de la planche (2026-10-21, branche `vignettes-annot-exports`)

Demandes d'Anthony, validées. Principe inchangé : « la partie Vignettes la plus épurée possible » ; rien de nouveau n'est visible
sur la planche tant qu'on ne prend pas un de ces outils (deux icônes de plus dans la colonne d'outils, et des commandes dans le
menu du clic droit sur le fond et la palette Ctrl+K). Vocabulaire : **annotation** (un trait du stylo), **case** (une vignette ou une
case vide), **marie-louise** (le passe-partout), **Plan**, **Case**, **Vignettes**, **Aplat** (inchangés).

- **Stylo et gomme d'annotation** (`annotations.rs`, `annotationsui.rs`). Deux icônes sous les outils de dessin, **seulement quand la
  planche est à l'écran** : le stylo (icône rouge, rouge d'origine, sa couleur et sa taille dans les Réglages) et la gomme. Aucune touche par
  défaut (`vignettes.annotation_stylo`, `vignettes.annotation_gomme`, réglables dans Préférences > Raccourcis). Prendre un autre outil les range ;
  cliquer deux fois le même aussi.
  - Le trait passe **par-dessus tout** (cases, barres, bandes de note, marges, fond), déborde où l'on veut ; l'appui est pris avant les cases :
    **le dessin de la case courante ne reçoit rien**. Il n'est ni dans un plan, ni dans le film, ni dans les vignettes, ni dans les exports
    vidéo / images / PDF / montage (le cliché d'export ne copie pas les annotations). Une étape d'annulation par trait (« Annotation »).
  - **Attaché à la case la plus proche** du centre du trait ; ses points sont gardés *relativement à cette case* : le trait suit quand les colonnes
    changent, quand les cases bougent ou grossissent (la taille du trait grossit avec la case). Un trait dans une case vide reste dans la même case quand
    elle devient une vignette. **Si la case disparaît** (plan supprimé, plans réunis, marque retirée, annuler), le trait passe à sa **voisine la plus
    proche** (parmi les vignettes qui restent) à la place exacte où il était à l'instant de la disparition ; sans l'ancienne planche (fichier d'une autre
    séance), à la vignette de même rang. **Il n'est jamais perdu** : seules la gomme et « Effacer les annotations » l'enlèvent.
  - **Gomme d'annotation** : le trait entier que touche le cercle ; une étape d'annulation par geste (« Effacer des annotations ») et aucune si rien n'est touché ;
    elle ne touche ni aux dessins ni aux notes libres de la planche.
  - **Cacher / montrer** (`vignettes.annotations_afficher`, clic droit sur le fond, Réglages du stylo) : un affichage, rien n'est effacé ; prendre le stylo les montre.
    **Effacer les annotations…** (`vignettes.annotations_effacer`) demande une confirmation, s'annule d'un Ctrl+Z.
  - **Fichier** : `annotations.json` à côté du manifeste, sans changer `VERSION` du projet (36) ; écrit par `write_sidecar` (jamais détruit s'il est illisible
    ou plus récent : mis de côté en `.illisible` si un neuf doit le remplacer) ; un projet **partagé** l'écrit avec le montage, les équipiers voient les mêmes annotations.
- **Marie-louise** (`marie.rs`, `vignettes.marie_louise`). Un passe-partout noir (ou la couleur choisie) couvre **tout ce qui est hors de l'intérieur des vignettes** :
  pages, barres, numéros, notes, écarts, et les traits d'annotation qui débordent. À l'écran c'est une bascule (clic droit sur le fond, Réglages du stylo, palette) ; à l'export,
  une option de la planche (avec sa couleur).
- **Déposer des images sur la planche** (`deposer.rs`), depuis l'Explorateur de Windows ou en glissant une référence hors du panneau Références :
  - **sur une case** : l'image va dans la case, sur un calque bitmap « Image » **à part, tout en bas (sous le dessin)**, ajustée au cadre de la caméra (tout le cadre, proportions
    gardées). Calque ordinaire : opacité, œil, cadenas. Dans une case qui n'est qu'une marque d'image, elle n'occupe que les images de cette case. Déposer une autre image sur la même case
    remplace celle du calque « Image » ;
  - **plusieurs images** : à la suite, case après case (l'une après l'autre dans l'ordre du dépôt) ; au-delà de la dernière case, de nouveaux plans ;
  - **sur les cases vides** : un plan neuf par image, dans l'ordre (les cases vides sautées avant celle qu'on vise deviennent des plans vides, comme quand on y dessine ; la barre grise devant la
    première case vide fait de la première image une image de plus du dernier plan) ;
  - **sur une barre** (entre deux cases) : un plan neuf par image, insérés là ; entre deux images d'un même plan (barre grise), le plan est d'abord coupé ;
  - **une seule étape d'annulation** pour tout le dépôt (« Déposer des images ») ; storyboard verrouillé ou plan tenu par un autre : refusé avec un message ;
  - lire, réduire et poser l'image se fait **sur un fil de fond** (aucune latence, même pour une grande photo) ; un repère (case encadrée, trait d'insertion, nombre d'images) suit le curseur
    pendant le glisser. Windows ne donne pas la place d'un dépôt de fichiers à la fenêtre : SHOT lit le curseur de l'écran.
- **Exporter la planche des vignettes** (Fichier > Exporter > « Planche des vignettes », `planche.rs`, `plancheui.rs`) : PNG ou JPEG (une grande image) ou PDF (une grande page,
  ou pages A4 / A3 paysage : la planche mise à la largeur de la page et coupée en hauteur, de préférence entre deux pages de la planche, avec un pied de page numéroté).
  Options : numéros de plan, notes, barres de coupe, cadres de caméra A et B (là où la caméra bouge), **annotations (décochées d'office)**, marie-louise (et sa couleur) ;
  résolution : 150 ou 300 dpi (une unité de la planche = 1/96 de pouce) ou une largeur en pixels (réduite aux bornes, 16 000 pixels de côté ou 60 millions de pixels, avec un message) ;
  étendue : tout le film, la séquence courante ou les plans choisis (rangés comme sur une planche qui n'aurait qu'eux). Par défaut : la planche comme à l'écran (pages blanches, mêmes
  colonnes et mêmes réglages), sans les cases vides. **Tâche de fond** (la fenêtre « Export en cours », avec « Arrêter » qui efface ce qui était commencé) ; écriture atomique `.part`.
- **Exporter un color script** (Fichier > Exporter > « Color script », `colorscript.rs`, `colorscriptui.rs`) : une petite image par **plan** (la première ou celle du milieu), ou une par
  **case** (option), rangées **par séquence** en rangées serrées (écart réglable) sur un fond uni (**noir d'office**, ou la couleur choisie), avec le titre de chaque séquence, les numéros de plan
  facultatifs, la largeur de l'image, l'écart, le nombre d'images par rangée ; une **bande des couleurs dominantes** sous chaque image (cinq couleurs au plus, **éteinte d'office**). PNG ou PDF
  en une grande page. Tâche de fond annulable.

#### À regarder à l'écran (annotations, dépôt, exports)

1. Vignettes : deux icônes de plus dans la colonne d'outils (rouges). Rien ailleurs.
2. Stylo : un trait rouge de la case 1 à la case 2, par-dessus l'écart et la barre ; la case courante ne reçoit aucun dessin ; Ctrl+Z défait le trait. Un trait sur le fond gris, hors des pages.
3. Changer les colonnes et la taille des vignettes : les traits suivent leur case. Supprimer un plan : ses traits passent à la voisine. Annuler : ils reviennent.
4. Gomme : n'efface que du rouge ; le dessin et les notes restent. Cacher / montrer ; « Effacer les annotations… » demande confirmation.
5. Marie-louise : tout est noir hors des images ; un trait qui déborde est coupé au bord des images.
6. Glisser une image de l'Explorateur : sur une case (repère encadré, l'image dans un calque « Image » sous le dessin), sur une case vide, sur une barre ; plusieurs images à la suite ; Ctrl+Z défait tout le dépôt ;
   glisser une référence hors du panneau Références vers la planche (tableau, grille et liste). Une grande photo ne fige rien.
7. Exporter la planche : PNG puis PDF A4 ; ouvrir le fichier ; sans annotations d'office, avec elles cochées ; avec marie-louise ; « Arrêter » en plein calcul : aucun fichier, aucun `.part`.
8. Exporter un color script : une image par plan, rangées serrées par séquence sur fond noir ; l'option « une par case » ; la bande des couleurs ; un fond clair (titres sombres).
9. Les trois thèmes : les icônes rouges et le repère de dépôt restent lisibles.

#### Essais ajoutés (partie 11)

`annotations` (14 : attache à la case la plus proche, relatif et suit les colonnes / tailles, case vide, gomme seule, case disparue → voisine, secours par le rang, rangs, fichier aller-retour,
fichier abîmé / plus récent / hostile, projet enregistré et rouvert, fichier jamais détruit, annuler, cliché d'export sans annotations), `annotationsui_tests` (11 : le stylo ne change rien au storyboard,
trait sur le fond et sur une barre, outils rangés, gomme, cacher, tout effacer avec confirmation, suivre les colonnes, plan supprimé, **images de film identiques avec et sans annotation**, marie-louise à l'écran, repos,
coût), `marie` (4), `deposer` (8 : case, marque, suite, cases vides, barre grise, entre deux cases, références et fichiers ignorés, verrouillé et grande image), `planche` (15 : réglages d'origine, étendues, barres,
numéros, notes, annotations décochées d'office, marie-louise, résolutions, coupures, fichiers PNG / JPEG / PDF, cadres A et B, tâche de fond, annuler, PDF, fenêtres), `colorscript` (10 : par plan / par case,
milieu, rangées serrées et titres, numéros, bande éteinte d'office, couleurs dominantes, image composée, bornes, fichiers, tâche de fond).

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

- **Ajouter** : le « + » du panneau Calques > Calque d'effet (passe du 2026-10-21, partie 15 : plus de bouton étincelle) ;
  Calques > Nouveau calque > Calque d'effet (et clic droit dans le vide du panneau) ; « Préréglages » du panneau Effets ; Ctrl+K (`effet.*`, sans touche par défaut, « Assigner un
  raccourci… » au clic droit d'une entrée de menu).
- **Fenêtre du préréglage** (à droite de l'écran, ne cache pas le dessin) : l'effet se voit en direct sur le dessin
  pendant les réglages ; Appliquer (une étape d'annulation « Préréglage d'effet : … »), Annuler ou Échap (rien n'est posé).
- **Distinguer** : un calque d'effet a une étincelle à la place de la lettre B / V, la teinte cyan (`theme::dessin_cfx`,
  `calque_cfx`) dans le panneau Calques, la timeline et le montage, et une étincelle à la place du mot « effet » sous son nom.
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

## 13. Espace 3D (2026-10-21, branche `espace3d-simple` ; avant : l'Espace 3D unifié du 2026-10-20 avec objets 3D, mannequin, rendu dans l'image)

Décision d'Anthony : « On va rester simple et focus sur le dessin. Un espace 3D où je peux bouger les dessins dans
l'environnement suffit. On peut supprimer les objets 3D comme les cubes, cylindres, personnages… ça alourdit le logiciel et
l'interface pour pas grand-chose. » Le **repère 3D** (modèle glTF en fil de fer par-dessus le dessin) est retiré aussi.

**Un seul espace, que des calques et la caméra** : l'icône cube de la barre au-dessus du dessin, avec les vues plateau et caméra (depuis la partie 40 ; `espace3d.basculer`, aussi
Affichage > Plateau et Ctrl+K) ouvre une vue 3D à la place du dessin, avec ses réglages dans le panneau **Réglages** (qui montre
l'Espace 3D tant qu'il est ouvert). Les identifiants d'avant (`vue.3d`, `scene3d.basculer`, `vue3d.recadrer`, `scene3d.cadrer`,
`outil3d.*`, `scene3d.outil_*`, `scene3d.vue_camera`, `scene3d.aimanter`, `scene3d.grille`) sont des doubles
(`keymap::ALIASES`) : un raccourci posé sur l'un d'eux marche toujours. Conception, formules, format, anciens projets :
`docs/ESPACE3D.md`. Tout est en icônes avec bulle d'une seconde (nom, raccourci, description).

- **Ce qu'on y voit** : les **cartes** (les calques visibles du plan, texturés par leur dessin de l'image courante, dans l'ordre
  de la pile, voile clair et contour de leur feuille, nom et profondeur), un sol quadrillé pour l'orientation, la **caméra du plan**
  (pyramide de vision, cadre, haut de l'image, sa course si elle a des clés). Plus d'objet, de décor ni de lumière.
- **Une rangée d'outils** (barre au-dessus de la vue) : **Choisir · Déplacer · Tourner · Mettre à l'échelle · Caméra · Recadrer ·
  Regarder par la caméra**. Commandes (`espace3d.*`, aucune touche par défaut, toutes dans Ctrl+K) : afficher / quitter, les
  quatre outils, choisir la caméra, recadrer, regarder par la caméra, aimantation, grille, **ordonner la pile selon la profondeur**.
- **Souris dans la vue** : clic = choisir une carte ou la caméra (Maj ou Ctrl : en ajouter) ; un clic sur une carte en fait aussi
  le calque courant (et le calque choisi dans le panneau Calques choisit sa carte) ; poignées du manipulateur pour déplacer,
  tourner, mettre à l'échelle ce qui est choisi ; glisser une carte la déplace dans son plan (Maj : en profondeur) ; clic droit
  ou Alt + glisser = tourner la vue ; bouton du milieu = la glisser ; molette = avancer. En « Regarder par la caméra », ces
  gestes déplacent la caméra du plan (sa clé de l'image courante si elle est animée).
- **Réglages** : repère de la carte pour les poignées, aimantation (pas de grille, de rotation, de taille), grille ; la liste des
  **cartes** (le dessus de la pile d'abord ; un verrou si le calque est verrouillé), « Montrer les cartes », « Ordonner la pile
  selon la profondeur » ; les réglages de la carte choisie : position (m), rotation (°) — Z est le tour dans le plan, X et Y
  inclinent —, échelle (%), étirement X / Y (%), profondeur (comme le panneau Calques), « Remettre à plat » ; la **caméra du plan**
  (position en m, panoramique, bascule, rotation, focale et objectifs, « Viser la sélection »).
- **Barre d'état** : « Espace 3D : clic sur une carte (un calque) ou sur la caméra pour la choisir, glisser pour la déplacer, clic
  droit pour tourner la vue, bouton du milieu pour la glisser, molette pour avancer. » — plus un mot sur des objets.
- **Annulation** : une étape par geste, au nom français : « Déplacer dans l'espace 3D », « Tourner dans l'espace 3D »,
  « Mettre à l'échelle dans l'espace 3D », « Déplacer la caméra », « Placer la carte dans l'espace 3D », « Ordonner la pile selon
  la profondeur », « Viser avec la caméra ».
- **Aucun repaint au repos** : la vue ne demande une image que si on la touche. Plus de rendu de fond (plus d'objets).
- **Ce qui a disparu** : cube, plan, cylindre, sphère, cône, escalier, mannequin, modèle glTF comme objet ; « Objets dans l'image » ;
  « Figer les objets dans un calque de fond » ; « Reprendre l'ancienne caméra de blocage » ; décor / plan ; lumière et rendu ;
  Fichier > Importer > « Un modèle 3D (glTF) comme repère » ; l'outil « Repère 3D » (barre d'outils de dessin, Réglages, curseur).
  **Anciens projets** : un calque « Fond 3D » est un calque bitmap ordinaire (mêmes pixels) ; `scenes3d.json` et les fichiers
  de `modeles/` restent dans le dossier du projet, ni lus, ni réécrits, ni effacés.
- **Limites visibles** : une carte inclinée est coupée aux bords de la feuille ; une carte ne se recouvre pas par profondeur mais
  par l'ordre de la pile ; la vue d'édition montre le dessin de la carte sans ses effets ; une carte ne s'incline pas au-delà de 85°.
- **Vérifié par le programme** (pas à l'écran) : voir `docs/ESPACE3D.md`, « Essais ». **À vérifier à l'écran (par
  Anthony)** : la liste de contrôle ci-dessous.

### Liste de contrôle visuelle pour Anthony

1. **Un seul bouton 3D** (cube) dans la barre au-dessus du dessin (partie 40) ; dans Affichage > Plateau une seule entrée « Espace 3D » ; Ctrl+K
   « Espace 3D » ne se trouve qu'une fois ; **aucune** commande de cube, mannequin, modèle ni « repère 3D » dans Ctrl+K.
2. **Une rangée d'outils** en ouvrant l'Espace 3D : Choisir, Déplacer, Tourner, Échelle, Caméra | Recadrer, Regarder par la
   caméra. Icônes distinctes de celles de la barre du haut (la « Vue caméra » du dessin garde sa caméra vidéo) ; bulles à 1 s ;
   l'outil actif est en accent.
3. **Les calques sont là** : ouvrir un plan à plusieurs calques à des profondeurs différentes ; les cartes apparaissent à leur
   profondeur, dans l'ordre de la pile, avec leur dessin, leur nom, leur profondeur ; la pyramide de la caméra est dessinée ; le
   sol quadrillé se lit.
4. **Choisir une carte** : un clic dessus la choisit (contour d'accent, poignées), son calque devient le calque courant ; choisir
   un autre calque dans le panneau Calques change la carte choisie ; le bouton « Caméra » choisit la pyramide.
5. **Déplacer** : les flèches rouge / verte / bleue déplacent la carte en X / Y / Z ; en la reculant elle rapetisse ; le champ
   « Profondeur » du panneau Calques la recule, lui, en gardant sa taille vue de la caméra.
6. **Tourner / incliner** : les cercles inclinent la carte ; le cercle bleu la tourne dans son plan ; revenir en vue normale : la
   carte inclinée est bien déformée en perspective, et on peut dessiner dessus (le trait tombe sous le pointeur).
7. **Échelle** : cube du centre = uniforme ; cubes d'axe = étirement ; le chiffre « Échelle » du panneau suit.
8. **Chiffres** : position, rotation, échelle, étirement, profondeur dans Réglages ; « Remettre à plat » ; « Ordonner la pile
   selon la profondeur ».
9. **Parallaxe** : trois cartes à des profondeurs différentes, bouger la caméra (poignées de la pyramide, ou « Regarder par la
   caméra » puis bouton du milieu) : revenir en vue normale, jouer le plan : le fond bouge moins que le premier plan.
10. **Clés** : sur une carte animée, la tirer à une autre image règle sa clé (losange vert) ; lire le plan : elle s'incline et
    recule en douceur. Avec des clés de caméra, tirer la pyramide à une autre image règle la clé de cette image.
11. **Annuler** : Ctrl+Z après chaque geste rend l'état d'avant, en une étape au nom lisible dans l'historique.
12. **Un ancien projet** (objets 3D, repère 3D, calque « Fond 3D ») s'ouvre sans avertissement, avec la même image qu'avant ; le
    calque « Fond 3D » est là, verrouillé ; l'enregistrer ne supprime pas `scenes3d.json` du dossier.
13. **Menus** : Fichier > Importer n'a plus « Un modèle 3D (glTF) comme repère » ; la barre d'outils de dessin n'a plus d'outil
    « Repère 3D » ; le panneau Calques n'a plus « Nouveau calque de fond » (icône des montagnes).
14. **Fluidité et thèmes** : 10 cartes, tourner la vue et tirer une carte restent fluides ; poignées, contours de cartes et
    pyramide lisibles dans les trois thèmes (l'espace a son fond sombre propre).
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
| Montage (21 → 12) | début, lecture, boucle, position ; nouveau plan, nouvelle séquence, menu Plans ; importer un son, menu Son ; animer ce plan ; vue dessin, vue vignettes | calque vectoriel, copier / coller / supprimer les plans, avancer / reculer le plan, verrou, aimant, **ajouter une transition**, importer une vidéo, nouveau commentaire |
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
  barre d'état (« Montage · plan 3 » ; depuis la partie 40, elle ne montre plus l'espace de travail ni le plan), la vue caméra du lecteur.
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
  comparer, vidéo, vues dessin / vignettes, copier / coller / supprimer les plans, replis des pistes) ; la place
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

### Le quatrième onglet Vignettes (2026-10-21, branche `vignettes-onglet-camera`)

Principe d'Anthony : « la partie Vignettes doit être la plus épurée possible, rien de distraction, que du dessin et des
références visuelles éventuellement ». Il y a maintenant **quatre onglets**, en haut à droite, dans l'ordre du travail :
**Vignettes · Storyboard · Animation · Montage** (même style, `bulles::encre_etat` pour l'onglet allumé, bulle à 1 s, clic droit
« Assigner un raccourci »). Le changement est instantané et ne touche pas au projet (plan, image, plans choisis, calque).

- **Choix : une disposition du mode Storyboard, pas un mode** (comme le Montage) : `Espace::Vignettes.mode() == Mode::Storyboard`,
  mêmes outils et raccourcis, même dessin ; la disposition pose la vue Vignettes (`Layout::vignettes`) comme zone principale.
- **Raccourci : Alt+Maj+V** (`espace.vignettes`, libre ; Alt+V est le miroir vertical de la sélection, Alt+M le Montage ; affiché
  « Maj+Alt+V » dans les bulles) ; Ctrl+K « Espace de travail : Vignettes » ; Affichage > Espaces de travail > Vignettes.
- **Tab depuis les Vignettes : le Storyboard** (décision expliquée : comme le Montage, ce n'est pas un côté de la bascule
  storyboard / animation). « Animer cette image » (clic droit d'une case) entre en animation ; Tab y ramène aux Vignettes (on revient à
  l'espace du côté storyboard où l'on était, `Settings::storyboard_space`). SHOT rouvre dans les Vignettes si on l'a quitté ainsi.
- **Disposition d'origine** (`DockLayout::pour_les_vignettes`) : la planche en grand ; à sa droite une seule colonne, les
  **Réglages** de l'outil (le pinceau) au-dessus, la **Couleur** et les **Pinceaux** en onglets dessous ; la **barre des
  personnages** au-dessus de la planche (`Espace::barre_personnages_par_defaut`, ligne « Vignettes » ajoutée ; la planche la peint
  maintenant) ; une **colonne d'outils minimale** (`TOOL_GROUPS_VIGNETTES` : pinceau, crayon, gomme ; les autres outils restent
  à leurs raccourcis et dans Ctrl+K ; la case du stylo d'annotation de la planche viendra s'y poser). **Ni bande (montage, timeline), ni
  Calques, ni Effets, ni fiche du plan, ni guides** ; les **Références** sont dans la barre des panneaux, fermées, à un clic.
- **Mémoire** : comme les autres espaces (`disposition_vignettes` dans `reglages.toml`, valeur d'origine absente ; un ancien
  fichier se lit tel quel), enregistrable comme espace nommé, « Rétablir la disposition » (Ctrl+K `espace.retablir`) la remet à
  son origine sans toucher aux autres.
- **Mouvements de caméra** : voir la partie 11, « Mouvements de caméra dessinés dans les cases ».

#### À vérifier à l'écran (par Anthony, quatrième onglet)

1. Les quatre onglets en haut à droite, dans l'ordre Vignettes, Storyboard, Animation, Montage, lisibles dans les trois thèmes, le
   bon allumé ; la bulle de Vignettes à 1 s (nom, Maj+Alt+V, description).
2. Un clic sur Vignettes : la planche seule, grande ; à droite Réglages du pinceau, Couleur et Pinceaux ; la colonne des pastilles
   des personnages au bord droit de la planche ; à gauche seulement pinceau, crayon, gomme ; rien en bas. Un clic sur Références dans la barre des panneaux l'ouvre.
3. Le plan courant, l'image et les plans choisis sont les mêmes qu'avant le clic, dans les deux sens.
4. Tab depuis les Vignettes : le Storyboard. Clic droit sur une case > Animer cette image, puis Tab : retour aux Vignettes.
5. Ferme les Réglages dans les Vignettes, passe en Storyboard puis reviens : toujours fermés. Rétablir la disposition : ils reviennent.
6. Au repos : rien ne bouge, le processeur dort.

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
- **Pastille** : 14 points en haut à droite du bloc du plan dans la bande (pas dans la vue Vignettes, épurée) ; un crayon pour un plan modifié, un plus pour un plan ajouté ; bulle « Modifié depuis l'envoi « v2 au réal » ».
  Sur un bloc de moins de 44 points de large (la bande zoomée très loin : la pastille cacherait la poignée du bord) ou de
  moins de 40 de haut, une **bande ambre de 5 points le long du bord haut** la remplace, avec la même bulle. Le « dernier
  envoi » est le dernier marqué (pas le plus tardif par sa date : la date se tape).
- **Fiche du plan, ligne « Version »** : « V2 » en gras (et « sur 3 » s'il y en a plusieurs), les icônes « Nouvelle version » (en dupliquant, en partant d'un plan vide) ; une
  ligne par autre version : son numéro et sa date, puis passer (flèches), comparer (colonnes), restaurer (flèche arrière),
  supprimer (corbeille).
- **Titre** : « Plan 3 · V2 » dans la bande, sur la planche, dans la fiche, la timeline, le titre du dessin (plus la barre d'état, partie 40) et le titre de la
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
  d'Anthony), les textes d'action / dialogue / notes, la couleur de repère, les calques de toute la scène (le
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
est peinte sur la première vignette du bloc (pas dans la vue Vignettes, qui ne montre que le numéro du plan : les versions y sont
au clic droit) ; sur un bloc très étroit seul le point ou le clic droit donnent accès aux versions.

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
  28 et 29 sont ceux d'autres chantiers menés en parallèle. **Format 36** (« On fusionne », ci-dessous) : `griffonnages`,
  `remplissage`, `griffonner`, `sources`, et un calque libre (sans `trait`).
- **Export, PDF, vignettes, montage, GPU** : la couleur est un calque bitmap ordinaire pour eux (rien n'a changé). Export
  Storyboard Pro : calque bitmap sous le trait, avec une note dans la fenêtre d'export (le lien n'y existe pas : à vérifier).
- **Avec l'« Aplat sous le trait »** (`fond.rs`) : les deux coexistent, rien n'a changé dans l'un pour l'autre. L'aplat est
  une strate du même calque, déduite du trait (une forme, une couleur) ; le calque de couleur est un calque à part,
  colorié zone par zone, avec ses propres images. Ordre de composition : aplat du trait, couleur, trait.
- **Limites connues** : couper / coller une clé du trait (même calque) ne remet pas sa couleur (elle reste dans le
  dessin d'origine : glisser la clé la garde) ; un effet du trait (tremblement) ne bouge pas la couleur ; le trait
  doit exister à l'image pour colorier (sinon, un pinceau en pose un vide) ; un calque de toute la scène, un dossier,
  un calque d'effet ou vidéo ne portent pas de couleur.

### Griffonner : le calque CTG est fondu dans le calque de couleur (« On fusionne », 2026-10-21, branche `couleur-unique`)

Décision d'Anthony : **un seul calque de couleur**, qui se peint à la main **et** qui sait remplir des zones d'un coup de
griffonnage ; le calque CTG est retiré (commande « Nouveau calque CTG », menus « Calque CTG », sorte de calque, bloc de
`keymap.toml`, essais). Code : `src/griffonnage.rs` (algorithme, tampon, lecture des anciens projets), `src/calquecouleur.rs`
(données), `src/calquecouleurui.rs` (mode, dessin de griffonnages, menu), `fx.rs` / `fxcache.rs` (calcul de fond).

**Vocabulaire et commandes** : « Griffonner » (mode du calque de couleur), « griffonnage » (ce qu'on pose en ce mode),
« Sources supplémentaires », « calque libre ».
- `calque.griffonner` (« Calque de couleur : griffonner… ») : icône boucle dans les réglages du pinceau, du crayon, de la
  gomme et du pot (allumée = enfoncée, bulle : nom, raccourci, description ; clic droit : assigner un raccourci), case
  « Griffonner » du menu Calque de couleur de la ligne du calque (et de Calques > Plus > Calque de couleur). Aucune touche
  par défaut. Une étape d'annulation par bascule ; sur un calque qui n'est pas de couleur : un message.
- Menu Calque de couleur d'un calque de couleur : Griffonner, Pinceau dans la zone fermée (peinture à la main seulement),
  Fermer les trous jusqu'à N px (peinture **et** remplissage), **Sources supplémentaires** (cases : d'autres calques de
  dessin dont les traits ferment aussi les zones ; le trait du calque, lui, ferme toujours les siennes), Ajouter un autre
  calque de couleur, Détacher du calque de trait.

**Comportement**

| Geste | En mode Griffonner | Mode éteint (peinture à la main) |
|---|---|---|
| Pinceau, crayon, formes | Un griffonnage : la couleur remplit **toute la zone fermée** qu'il touche (la couleur la plus présente dans la zone gagne), sous le trait | Peinture libre, dans la zone fermée où le trait commence (« Pinceau dans la zone ») |
| Pot de peinture | Un tampon de griffonnage (disque de 5 px) au point cliqué | Remplit la zone fermée, à plat (ne se recalcule pas) |
| Gomme | Efface les griffonnages ; une zone sans plus aucun griffonnage n'est plus remplie | Efface la peinture libre, jamais le remplissage |
| Sélection, déplacement, transformation | Agissent sur la **peinture libre**, pas sur les griffonnages (limite connue) | idem |

- **Ordre de composition** (de bas en haut) : remplissage au griffonnage, aplat sous le trait (s'il y en a un), **peinture libre**,
  puis le calque de trait. La peinture libre est donc **au-dessus** du remplissage ; pour retirer du remplissage à un
  endroit, gomme le griffonnage (ou retouche le trait pour que la zone change).
- **Où sont les griffonnages** : un dessin à part par dessin de peinture (`CalqueCouleur::griffonnages`), un vrai dessin
  du projet (`dessins/<n°>.png`) qu'aucune timeline ne cite : il suit son dessin de peinture quand le trait bouge, se copie
  avec le calque ou le plan (jamais partagé), s'enregistre avec le projet. Pendant qu'on griffonne en mode Griffonner, les
  griffonnages restent visibles, **pâles**, par-dessus (à l'écran seulement).
- **Le remplissage suit** : retouche le trait (ou une source), ou un griffonnage : il se **recalcule tout seul, en
  arrière-plan** ; l'écran garde le remplissage d'avant jusqu'à l'arrivée du nouveau (rien ne se calcule sur le fil de
  l'interface pendant un geste ; `fx::display_with`). L'export et l'enregistrement le calculent tout de suite, exactement.
  Un trou du trait jusqu'à 2 × N px (N = 4 : 8 px) ne laisse pas fuir la couleur ; au-delà, la couleur remplit le dehors.
- **Sources** : le calque de trait du calque de couleur, plus les sources supplémentaires. Un calque **libre** (sans calque
  de trait, images à lui — c'est ce que devient un ancien calque CTG que son trait ne suit pas) n'a que ses sources. Détacher
  un calque de couleur qui a des griffonnages donne un calque libre dont l'ancien trait devient une source (l'image ne
  change pas) ; si le trait disparaît, même chose.
- **Les clés ne changent pas** : un calque de couleur lié suit toujours les images de son trait (le miroir de `refleter`) ;
  ses griffonnages suivent leur dessin de peinture. Un calque libre a ses propres images, comme un calque ordinaire.
- **Anciens projets** (format 35 et avant) : un calque CTG s'ouvre converti en calque de couleur qui montre **la même image au
  pixel près** (essai `an_old_project_with_a_ctg_layer_opens_with_the_same_image_and_stays_editable`, qui compare à des
  empreintes prises avant la fusion), Griffonner allumé, griffonnages gardés (les mêmes fichiers : rien n'est réécrit ni
  effacé, le champ `ctg` du manifeste est lu et jamais plus écrit). **Lié** à sa première source quand elle est juste
  au-dessus, dans le même dossier, que leurs images se recouvrent exactement et que pré / post-comportement,
  transformation, profondeur et clés de position sont identiques (sinon l'image changerait) ; **libre** dans tous les
  autres cas, ses sources devenant des sources supplémentaires. Un message le dit à l'ouverture. « Charger la structure
  à partir des sources » n'existe plus : un calque lié suit déjà les images de son trait.
- **Trois façons de colorier sous le trait, devenues deux** : le **calque de couleur** (peinture à la main et
  griffonnage, qui se mêlent) et l'**aplat sous le trait** (`fond.rs` : une forme déduite du trait). La **brosse par
  personnage** (un trait + une couleur d'aplat par personnage, partie 32) s'appuie sur l'aplat
  sous le trait : elle n'a pas changé et ne dépend pas du calque de couleur.
- **Limites** : les images guides de la propagation (`Propager`, `propager.rs`) ne montrent pas les remplissages au
  griffonnage (le trait et la peinture libre, si) ; les effets du calque (flou…) s'appliquent au remplissage et à la peinture
  libre chacun de son côté (un effet qui crée de la matière dans le vide, comme un vignettage, s'y applique deux fois) ;
  sélectionner ou déplacer des griffonnages n'est pas possible ; la zone d'un griffonnage n'est pas bornée (un trait qui
  traverse un trait colorie les deux zones).

### À vérifier à l'écran (Griffonner)

1. Calque de couleur sous un trait fermé (un cercle), outil Pinceau : l'icône boucle apparaît dans ses réglages ; bulle au
   bout d'une seconde ; clic : elle s'enfonce, la phrase dit « la couleur remplit la zone ».
2. Griffonne une couleur au milieu du cercle : toute la zone se remplit (au lâcher, un instant plus tard), jusque sous le
   trait, sans liseré clair ; le griffonnage reste visible en pâle.
3. Retouche le trait (ferme-le en deux avec une cloison) : le remplissage d'avant reste, puis se met à jour tout seul ;
   pendant que tu dessines la cloison, rien ne saccade.
4. Éteins Griffonner et peins une tache d'une autre couleur : elle est **au-dessus** du remplissage ; la gomme l'enlève sans
   toucher au remplissage. Rallume Griffonner : la gomme efface les griffonnages (le remplissage disparaît avec eux).
5. Un petit trou dans le trait (jusqu'à 8 px) : la couleur ne fuit pas.
6. Pot de peinture en mode Griffonner : un clic dans une zone la remplit.
7. Ctrl+Z / Ctrl+Maj+Z après un griffonnage : un geste, une étape.
8. Enregistre, rouvre : même image. Duplique le plan : les deux ne partagent pas leurs griffonnages (retouche l'un).
9. Un projet d'avant qui avait un calque CTG : il s'ouvre avec la même image, un message le dit ; le calque est devenu un
   calque de couleur (lettre C, rose) ; griffonne dessus, c'est modifiable. Plus de menu ni de commande « CTG ».

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
  d'ajustement, l'aplat sous le trait, les calques de couleur, l'enveloppe et la perspective, la propagation,
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
  passe à l'écran (« … est déjà ouvert »). Un fichier `.shot` (ou un ancien `.pelure`) lâché sur la fenêtre ou sur l'accueil
  s'ouvre de même. `Bus.shot` et l'ancien `Bus.pelure` sont le même projet : un seul onglet, une seule ligne dans les récents.
- **Nom du fichier de projet** (2026-10-23) : `MonFilm\MonFilm.shot`. Fichier > Ouvrir propose les `.shot` et les anciens
  `.pelure` ; « Enregistrer sous » et « Nouveau » écrivent un `.shot`. Le premier « Enregistrer » d'un projet `.pelure` écrit son
  `.shot`, puis efface le `.pelure` (la barre d'état le dit : « Le fichier du projet s'appelle maintenant… ») ; dessins et
  dossier ne bougent pas. Un film partagé garde son `.pelure` (la boîte « Partager ce projet » montre un `.shot`, la structure
  écrite dans le dossier partagé s'appelle pourtant `.pelure`).
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
pinceau et du texte, puis rendu exact), aplat sous le trait (peinture et coupe), pose des calques et clés de position, clés de
caméra (position ; l'avancée suit la largeur du cadre ; zoom et angles inchangés), paramètres d'effets en pixels (rayon des flous,
décalage de l'ombre, liseré, grain, tremblement… clés comprises, dans les bornes de l'effet ; la densité de trous du trait sec,
par 100 px de trait, est divisée), débord de l'aplat sous le trait, fermeture des trous des calques de couleur (peinture et remplissage au griffonnage), zones bitmap et
éléments vectoriels collés sur le dessin (ils gardent leur place dans le cadre). **Ne change pas** : les paramètres en pour cent,
les incrustations (en pour cent de la hauteur du cadre), les marques d'image, les textes, et — comme pour un collage de plans dans
le même projet — les références liées, les commentaires, les transitions et les sons des plans.

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
| **Aplat sous le trait** (« Aplat ») | un réglage **du calque**, avec son bouton sur sa ligne et sa sous-ligne dans le panneau Calques (partie 33) | « fond » ou « remplissage » dans un texte de l'interface ; option du pinceau (c'était avant) |
| **Passe-partout** | le voile autour du cadre de la caméra de « Estomper le hors-champ » | masque |
| **GIF animé** | l'export `.gif`, sans son | |

### Où et quels gestes

| Commande | Où | Raccourci |
|---|---|---|
| Effets du trait, de matière (`fx.ecriture`…, `fx.preset_*`) | panneau Effets : boutons « Trait ▾ » et « Matière ▾ » (icône + nom ; section « Préréglages ») ; « Ajouter un effet » ; clic droit sur un calque > Effets ; Ctrl+K | aucun |
| Nouveau dossier (`dossier.nouveau`) | panneau Calques : icône dossier + de ses commandes, clic droit dans le vide de la liste ; en-tête de la timeline (déjà là) ; menu Calques > Dossiers (déjà là) | aucun |
| Ranger les calques choisis dans un dossier (`dossier.ranger`) | panneau Calques : icône (quand plusieurs calques sont choisis) ; menu Calques > Dossiers | aucun |
| Ouvrir / fermer un dossier | triangle de sa ligne dans le panneau Calques et dans la timeline (le même réglage) | aucun |
| Aplat sous le trait du calque (`pinceau.fond`) | bouton carré-utilisateur (« Aplat sous le trait ») sur la ligne de chaque calque de dessin du panneau Calques, et dans ses commandes (calque courant) ; clic droit d'un calque > « Ajouter un aplat » / « Retirer l'aplat » ; menu Calques > Aplat sous le trait (partie 33) | aucun |
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
- **Panneau Calques** : dans les commandes, après « Nouveau calque bitmap » : « Nouveau dossier » (et, avec plusieurs calques choisis,
  « Ranger les calques choisis dans un dossier »), puis le bouton d'aplat et, s'il y a des effets, « fx » pour le calque courant. Chaque
  ligne : [triangle si dossier] œil, cadenas, [fx], vignette (l'icône d'un dossier pour un dossier), nom, puis « vectoriel » (ou « dossier ·
  3 calques ») et le bouton d'aplat ; un calque qui a un aplat a en plus sa sous-ligne fine (partie 33). Un calque rangé est décalé vers la droite ; un dossier fermé n'a plus les lignes de ses calques (sauf le
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
- ~~Les réglages du fond restent sous le pinceau~~ : depuis le 2026-10-21 (partie 33), ils sont dans Réglages quand la sous-ligne de l'aplat est choisie, et au clic droit de la sous-ligne.

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
3. Aplat sous le trait : plus de bouton dans les réglages du pinceau ni du crayon ; sur la ligne d'un calque de dessin, le bouton carré-
   utilisateur (éteint) ; un clic le donne au calque (même un autre que le courant) et une sous-ligne « Aplat » apparaît dessous ;
   Ctrl+Z. Dans le thème Gris clair, l'icône allumée se lit. (La suite : partie 33.)
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
| **Vignettes** (quatre carrés, 2026-10-22) | Planche des Vignettes | Rangées de cases pâles (venu de Montage) ; Cases par rangée ; Débord du trait hors des cases |
| | Cadres de caméra | Aimant des cadres de caméra (0 : éteint) ; Couleur des cadres de caméra |
| | Décor | Report du décor ; Couleur du décor ; Taille des brosses du décor ; Rétablir les réglages des Vignettes |
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

## 32. Brosses par personnage (2026-10-21, branche `brosses-personnages`)

Demande d'Anthony : « On ajoute un personnage et on lui attribue une couleur de trait et une couleur de fond. Quand on sélectionne ce
personnage, on dessine avec la couleur de trait et de fond immédiatement. Ça permet, même avec un dessin très rough, d'identifier le
personnage. J'imagine une barre horizontale au-dessus du canevas avec juste des pastilles de couleurs qu'on sélectionne. Une pastille par
personnage plus des couleurs neutres. Paramétrables. » Précisions : la barre est visible d'office en Storyboard et dans l'espace Vignettes
(à venir), cachée d'office en Animation et en Montage mais allumable ; le panneau « Personnages » est dans la disposition par défaut si elle
le permet, sinon à un clic de la barre. Vocabulaire (décidé ensuite par Anthony) : la couleur sous le trait s'appelle l'**aplat** ; un
personnage a donc un **trait** et un **aplat**. Les textes nouveaux disent « aplat » ; ce qui existait déjà (« Aplat sous le trait » du calque,
`Layer::fond`, la strate Fond) a été renommé **« Aplat sous le trait »** le 2026-10-21 (partie 33) dans tous les textes de l'interface ; le code et
le format gardent `fond`.

### Mots

| Mot | Sens | À éviter |
|---|---|---|
| **Personnage** | un nom, une couleur de trait, une couleur d'aplat (ou aucune), dans un ordre ; il est **du projet** (`personnages.json`) | acteur, rôle |
| **Couleur neutre** | un trait avec ou sans aplat qui n'est pas un personnage (noir, gris, rouge de correction…) ; elle est **de l'application**, commune à tous les projets | couleur par défaut |
| **Pastille** | le rond de la barre : anneau = couleur du trait, dedans = couleur de l'aplat (rayé quand il n'y en a pas) | puce, bouton couleur |
| **Choisir** | cliquer une pastille : le trait prend sa couleur, et l'aplat sous le trait s'allume avec la sienne ; recliquer la pastille choisie la lâche | sélectionner (pour les dessins, c'est autre chose) |
| **Lâcher** | revenir à une couleur libre : la couleur reste, mais n'est plus celle d'un personnage | désélectionner |

### Où et quels gestes

| Geste | Où | Effet |
|---|---|---|
| Clic sur une pastille | colonne des pastilles (au bord droit du dessin et de la planche), panneau Personnages | choisit le personnage (ou la couleur neutre) ; un outil qui ne dessine pas (Sélection, Pipette…, la gomme) cède la place au pinceau ; un clic sur la pastille choisie la lâche ; le clic ne dessine rien |
| Pastille « aucun personnage » (anneau barré, en haut de la colonne) | colonne | lâche le personnage : la couleur libre (plus de texte « Aucun personnage » : la bulle le dit) |
| « + » | colonne (en bas), panneau | nouveau personnage (couleurs de la place suivante), choisi, et la fenêtre « Modifier » s'ouvre : une étape « Nouveau personnage » |
| Clic droit sur une pastille | colonne | Modifier… (nom, trait, aplat), Dupliquer, Monter / Descendre, Supprimer |
| Glisser la poignée d'une ligne | panneau Personnages | change la place du personnage (une étape « Ranger les personnages ») |
| Icône « personnages » au pied de la colonne | colonne | ouvre ou ferme le panneau Personnages (un clic) |
| Affichage > « Montrer / Cacher les pastilles des personnages » ; Ctrl+K `perso.barre` | menu, palette | la colonne de cet espace de travail (chaque espace retient son choix) |
| `perso.suivant`, `perso.precedent`, `perso.aucun`, `perso.ajouter` | Ctrl+K, raccourcis à poser | personnage suivant / précédent (les personnages, puis les neutres, en tournant), lâcher, ajouter |
| Une couleur prise à la main | panneau Couleur, nuancier, pipette (Alt + clic), échange des deux couleurs, code hexadécimal | lâche le personnage (la barre n'en montre plus aucun de choisi) |
| Pot de peinture sur l'aplat | clic sur la sous-ligne « Aplat » du calque (partie 33), pot de peinture | remplit la zone fermée avec la couleur d'**aplat** du personnage choisi (sinon la couleur courante) |
| Choisir un personnage pendant que la sous-ligne d'un aplat est la cible | barre, panneau | **revient au trait** : le personnage dessine son trait et son aplat ; recliquer la sous-ligne reprend l'aplat seul (partie 33) |

### Où vivent les choses (et pourquoi)

- **Les personnages sont du projet.** Ils sont dans `Structure` (avec les commentaires, les transitions…) : ajouter, modifier, ranger, supprimer
  s'annulent (Ctrl+Z). Fichier `personnages.json` à côté du manifeste, écrit par `write_sidecar` : jamais effacé s'il est illisible ou d'une
  version plus récente (mis de côté sous `.illisible` si un fichier neuf doit le remplacer), absent s'il n'y a aucun personnage. **Le format du
  projet ne change pas** (`VERSION` reste 35 ; projets partagés inclus) : un projet d'avant s'ouvre tel quel ; une version qui ne connaît pas
  les personnages ignore le fichier et montre les aplats tels qu'ils ont été peints. Dans un film partagé, les personnages s'écrivent avec le
  montage (celui qui tient le jeton, comme les transitions) ; les équipiers reçoivent la même liste.
- **Les couleurs neutres sont un réglage de l'application** (`reglages.toml`, clé `couleurs_neutres`), parce qu'elles ne sont à aucun film :
  le noir, le gris, le rouge de correction servent partout. D'origine : **Noir** (trait noir, sans aplat), **Gris** (trait gris, aplat gris clair),
  **Rouge de correction** (trait rouge, sans aplat). Huit au plus ; on peut les retirer toutes. Les modifier ne touche pas à l'historique du
  projet (un réglage de l'application ne s'annule pas).
- **La barre se montre par espace de travail** (`Layout::character_bar`, clé `barre_personnages` de la disposition) : rien d'écrit tant qu'on
  n'a pas choisi, et l'espace décide (`Espace::barre_personnages_par_defaut` : Storyboard oui, Animation et Montage non ; l'espace
  Vignettes, quand il viendra, oui : c'est une ligne à ajouter dans ce `match`). La colonne est dessinée par `pastilles_colonne` (une `Area` posée en
  fin d'image par le dessin et par la planche des vignettes ; partie 41).

### L'aplat d'un trait de personnage (où vit la couleur d'aplat)

L'« Aplat sous le trait » du calque (`Layer::fond`) est **une seule forme par dessin, d'une couleur par calque**. Changer la couleur du calque au choix d'un
personnage recolorierait l'aplat de tous les personnages du calque. On garde donc plusieurs personnages sur un même calque ainsi :

- au début d'un trait de pinceau ou de crayon avec un personnage qui a un aplat, le calque prend un « Aplat sous le trait » s'il n'en avait pas
  (une étape « Aplat du personnage » à part) ; **ce qui était déjà dessiné sur ce calque reste sans aplat** (coupé, comme le ferait un personnage
  sans aplat) ;
- à la fin du trait, un fil de fond (`persofond.rs`, jamais le fil de l'interface) calcule la forme avant et après le trait ; **les pixels que le
  trait a fait gagner à la forme sont peints dans la peinture de la strate de l'aplat (`Cel::fond`) de la couleur d'aplat du personnage** (ou coupés, s'il n'en a pas).
  La peinture reste dans la forme et passe sous le trait, comme tout ce qu'on peint sur l'aplat : un autre personnage dessine ensuite sur le même
  calque et colore son propre aplat sans toucher à celui des autres ;
- le résultat est posé à l'image d'écran où il arrive (un dixième de seconde après le trait, comme la forme elle-même) ; s'il arrive avant toute
  autre action, il rejoint l'étape d'annulation du trait : **un trait de personnage = une étape, aplat compris** ; sinon il est posé sans étape à
  lui (la peinture est rognée par la forme : défaire le trait cache aussi son aplat, et le rétablir le rend) ;
- rien ne change dans les dessins (l'aplat existe depuis le format 23) et rien ne se calcule en plus quand on n'utilise pas de personnage.

La gomme et la peinture déjà posée dans l'aplat ne sont pas touchées. Pinceau et crayon, bitmap et vectoriel. **Même strate** que l'aplat peint à la
main depuis la sous-ligne du calque (partie 33) : un personnage et la sous-ligne travaillent la même peinture, la même coupe.

### Ce que voit l'utilisateur

- **Colonne des pastilles** (depuis le 2026-10-21, partie 41 ; avant, une barre de 26 px sous la barre de la vue) : au bord droit du dessin, sous le titre du plan ;
  de haut en bas « aucun personnage » (anneau barré), une pastille par personnage, un trait, les pastilles neutres, un « + » et l'icône du panneau (enfoncée quand
  il est ouvert). La pastille choisie a un halo de la couleur d'accent ; la bulle (une seconde) donne le nom, les codes des couleurs et le clic droit. Sans
  personnage, la bulle de la pastille « aucun personnage » le dit.
- **Panneau Personnages** (en onglet avec Couleur et Pinceaux ; icône dans la barre des panneaux) : « + », « Couleur libre » (lâcher), le nombre de
  personnages ; une ligne par personnage (poignée, pastille, nom, couleur du trait, interrupteur « Avec un aplat », couleur de l'aplat, corbeille) ;
  puis « Couleurs neutres » (mêmes lignes), « + » et « Couleurs neutres d'origine ».
- **Fenêtre « Modifier le personnage »** (clic droit > Modifier…) : la même ligne ; chaque champ ou sélecteur de couleur est une étape nommée « Modifier
  le personnage » par geste. Le pinceau suit la couleur d'un personnage choisi qu'on modifie (ou qu'on annule).
- **Barre d'état** : « « Lou » : le trait et l'aplat de ce personnage. » / « Couleur libre : plus de personnage choisi. ».

### Limites

- Les pixels que la forme d'un autre personnage couvrait déjà gardent la couleur d'avant : un personnage dessiné *dans* l'aplat d'un autre, sur le même
  calque, n'a pas son aplat (on le recolorie avec le pot de peinture sur la sous-ligne de l'aplat, qui prend l'aplat du personnage choisi, ou on dessine sur un autre
  calque). Un trait qui complète un personnage déjà fermé n'ajoute d'aplat que là où la forme grandit.
- Un trait ne se souvient pas de son personnage : supprimer un personnage ne change aucun dessin ; déplacer ou transformer un dessin ne déplace pas
  la peinture de l'aplat qui le colorait (comme toute peinture de l'aplat).
- Seuls les traits de pinceau et de crayon portent un aplat de personnage ; le pot de peinture sur le trait, les formes et le texte prennent la couleur
  de trait du personnage seulement.
- L'aplat du dessin courant est coupé sous ce qui était déjà dessiné quand le calque prend son fond ; les autres dessins du calque (autres images) gardent
  le fond blanc du calque tant qu'on n'y dessine pas avec un personnage.
- Enregistrer dans la fraction de seconde qui suit un trait écrit le projet sans l'aplat de ce trait ; il est posé juste après et le projet est de nouveau
  « modifié ».
- Dans un film partagé, deux équipiers qui modifient la liste en même temps : le dernier à écrire gagne (comme les transitions).

### Essais ajoutés

`personnages_tests.rs` : ajouter, modifier, supprimer, dupliquer et ranger des personnages avec annulation ; noms nets ; choisir règle la couleur du trait
et le trait gagne son aplat (bitmap et vectoriel) ; deux personnages sur un même calque gardent chacun leur aplat ; sans aplat (coupé) et neutre sans aplat ;
un trait et son aplat = une étape ; des traits plus rapides que le fil de fond ; la gomme ne touche à rien ; le pot de peinture sur l'aplat ; une
couleur prise à la main lâche le personnage ; le choix suit la couleur modifiée et part avec le personnage supprimé ; suivant / précédent ; les neutres
(réglages d'origine, aller-retour TOML, ajout, déplacement, borne) ; la barre par espace de travail ; barre et panneau dessinés, application au repos
(avant et après un trait) ; fichier : aller-retour, absent sans personnage, illisible ou plus récent jamais détruit, lecture défensive, fil d'écriture,
aplats enregistrés et relus. Les essais de la barre des panneaux, de la disposition (onglets Couleur, Pinceaux, Personnages) et du fichier de raccourcis
tiennent compte du nouveau panneau et des six commandes.

### À vérifier à l'écran (par Anthony)

1. Espace Storyboard : au bord droit du dessin, sous le titre du plan, la colonne des pastilles (« aucun personnage », trois pastilles neutres : noir, gris, rouge, « + »).
   Animation et Montage : pas de colonne ; Affichage > « Montrer les pastilles des personnages » l'allume dans cet espace seulement ; revenir en Storyboard : elle y est.
2. « + » : une pastille de couleur apparaît, choisie (halo), la fenêtre « Modifier le personnage » s'ouvre ; taper « Lou », changer le trait (rouge) et
   l'aplat (rose) : la pastille suit. Ctrl+Z annule chaque geste ; le panneau Historique les nomme.
3. Dessiner une silhouette fermée au pinceau avec Lou : le trait est rouge et, une fraction de seconde après, le dedans se remplit de rose ; le calque a
   maintenant le bouton « Aplat sous le trait » allumé et sa sous-ligne « Aplat ». Ctrl+Z : le trait et son aplat partent ensemble (l'aplat du calque, lui, est une étape à part).
4. Créer Max (bleu, aplat bleu ciel) ; sur le même calque, dessiner à côté : aplat bleu ciel, celui de Lou n'a pas changé. Chercher une silhouette qui chevauche
   l'autre (limite connue : la partie commune garde l'aplat de Lou).
5. Pastille « Noir » (sans aplat) : le trait est noir et n'a pas d'aplat, même sur un calque qui en a déjà un ; pastille « Gris » : trait gris, aplat gris clair.
6. Prendre une autre couleur (panneau Couleur, pipette Alt + clic, échange des deux couleurs) : le halo disparaît de la barre. Recliquer la pastille : la couleur revient.
7. Cliquer la pastille choisie : elle est lâchée (« Couleur libre »). L'outil Sélection actif, cliquer une pastille : le pinceau est pris.
8. Clic droit sur une pastille : Modifier…, Dupliquer (« Lou (copie) »), Déplacer à gauche / à droite, Supprimer. Les pastilles se rangent ; Ctrl+Z remet l'ordre.
9. Icône « personnages » à droite de la barre, ou barre des panneaux : le panneau s'ouvre (onglet avec Couleur et Pinceaux). Glisser la poignée d'une ligne
   change l'ordre de la barre ; la corbeille supprime ; les couleurs neutres se modifient là aussi, et « Couleurs neutres d'origine » les remet.
10. Fermer et rouvrir SHOT : les couleurs neutres modifiées sont là ; enregistrer le projet, le rouvrir : mêmes personnages, mêmes aplats (la silhouette de Lou garde
    son rose). Ouvrir un autre projet : ses personnages à lui, les mêmes couleurs neutres.
11. Gomme sur un personnage choisi : elle gomme, le personnage reste choisi. Pot de peinture sur la sous-ligne « Aplat » avec Max choisi : la zone prend le bleu ciel. Choisir une pastille pendant que la sous-ligne est choisie : on revient au trait.
12. Au repos : barre et panneau ouverts, rien ne bouge, le processeur reste à zéro ; un trait de personnage ne laisse rien tourner après lui.

## 33. L'aplat sous le trait, depuis le calque (2026-10-21, branche `aplat-souscalque`)

Demande d'Anthony : « Le fond derrière le trait s'active depuis le calque, plus depuis les réglages du pinceau. Une fois activé, un
sous-calque apparaît dans le panneau de calques (uniquement, pas sur la timeline), en petit (plus fin qu'un calque normal). Quand on clique
sur ce calque on peut gommer ou dessiner sur le fond uniquement. Plus simple de cette manière, les outils ou icônes liés à cela avant
disparaissent. Plus simple, plus intuitif. » Et le nom : **« Aplat »**. (Le calque d'avant : partie 25 ; les personnages : partie 32.)

### Mots

| Mot | Sens | À éviter |
|---|---|---|
| **Aplat sous le trait** | la forme déduite du trait, d'une couleur, sous le trait ; un réglage **du calque** (bouton de sa ligne, clic droit) | fond sous le trait, remplissage |
| **Aplat** (sous-ligne) | la ligne fine sous le calque, dans le panneau Calques : pastille de la couleur + « Aplat » ; la **cible** du dessin quand elle est choisie | sous-calque (dans les textes de l'interface), calque d'aplat |
| **Cible** | où dessinent le pinceau, le crayon, la gomme, le pot de peinture et le cutter : le **trait** (d'office) ou l'**aplat** du calque courant | strate (c'est le mot du code) |

### Où et quels gestes

| Geste | Où | Effet |
|---|---|---|
| Ajouter / retirer l'aplat | bouton carré-utilisateur de la ligne du calque (« Aplat sous le trait ») et des commandes du panneau ; clic droit d'un calque > « Ajouter un aplat » / « Retirer l'aplat » ; menu Calques > Aplat sous le trait ; actions `pinceau.fond`, `fond.calques_choisis`, `fond.tout_le_plan`, `fond.retirer_choisis`, `fond.couleur_partout` (Ctrl+K, raccourcis d'avant) | une étape « Aplat : ajouter » / « Aplat : retirer » ; la sous-ligne apparaît ou part ; ni dossier, ni calque CTG ou vidéo |
| Clic sur la sous-ligne | panneau Calques | l'aplat devient la cible (le calque aussi devient courant) ; un outil qui ne dessine pas cède la place au pinceau ; pas d'étape d'annulation |
| Clic sur le calque (ligne, vignette, nom) | panneau Calques, timeline | la cible revient au trait ; changer de calque, retirer l'aplat, annuler : aussi |
| Peindre, gommer, remplir, couper | canevas, la cible étant l'aplat | pinceau, crayon : dans l'aplat, sous le trait, dans sa forme ; gomme : coupe l'aplat ; pot de peinture : remplit la zone fermée par le trait ; cutter : retire la zone ; le trait n'est jamais touché ; « Aplat : peinture », « Aplat : gomme », « Aplat : remplir », « Aplat : retirer la zone » |
| Régler l'aplat | Réglages (tout en haut, « Aplat de <calque> »), clic droit de la sous-ligne | couleur, débord, fermer les trous, le bord de l'image ferme la forme ; « Cet aplat aussi sur » les calques choisis / tout le plan (Réglages) ; une étape « Aplat : réglages » par geste |
| Retirer l'aplat | clic droit de la sous-ligne ; bouton ou clic droit du calque | « Aplat : retirer » |

### Ce qui a disparu

- Dans les réglages du pinceau, du crayon, de la gomme et du pot de peinture : la bascule « Peindre dans : Le trait | Le fond », la couleur du fond,
  le débord, fermer les trous, la case du bord, « Ce fond aussi sur », et la phrase d'aide. Plus rien de l'aplat ne s'y trouve.
- Sur la ligne du calque courant : la petite bascule « Trait | Fond ».
- L'action `pinceau.strate_fond` (et son bloc de `keymap.toml`) : la cible se choisit en cliquant (un raccourci posé dessus par un utilisateur est
  ignoré avec l'avertissement habituel des actions inconnues).

### Règles de cible

- La cible est `App::aplat_cible` : le **numéro du calque** dont l'aplat est visé (`None` : le trait). Elle est **propre au projet** (rangée avec
  l'onglet, `onglets.rs`) et **n'est jamais enregistrée** : un projet rouvert dessine le trait. `App::aplat_en_cible()` dit si l'aplat du calque
  courant est la cible (le calque en a toujours un, et c'est bien lui).
- Elle ne vaut que pour le calque dont on a cliqué la sous-ligne. Elle retombe sur le trait : au clic sur un calque (`click_layer_head`), à tout
  changement de calque (`select_layer`), quand l'aplat est retiré, annulé ou perdu (`tick_aplat`, à chaque image d'écran, sans rien demander au repos).
- **Un personnage** (partie 32) : le choisir (pastille, `perso.suivant`…) **revient au trait**, parce qu'un personnage dessine son trait *et* son aplat,
  comme prévu ; son aplat va dans la **même strate** que l'aplat peint à la main. Cliquer ensuite la sous-ligne reprend l'aplat seul, le personnage
  restant choisi : pinceau et pot de peinture y prennent alors sa couleur d'**aplat**.
- Les outils qui ne travaillent pas l'aplat (sélection, formes, texte, déformation, caméra…) travaillent le trait, quelle que soit la cible ; la
  cible reste en attente et se retrouve au retour d'un outil de dessin. La ligne du bas de la vue ne dit « Aplat de <calque> » que pour les cinq
  outils qui l'utilisent.

### La sous-ligne

- Une ligne de 16 px (un rang de calque en fait 36 + marges, soit moins de la moitié), en retrait, sous le rang de son calque : une pastille de la couleur
  de l'aplat et « Aplat » en petit. Choisie, elle est surlignée comme un calque courant (la couleur de la sorte du calque) et le rang du calque est
  alors surligné plus doucement. Survol : bulle d'une seconde. Clic : cible ; clic droit : réglages et « Retirer l'aplat ».
- **Panneau Calques seulement.** Ni la timeline (`layerheads.rs`, `timeline.rs`) ni les pistes du montage (`montagerows.rs`) n'en ont : leurs rangées ne
  changent pas. Un dossier fermé cache ses calques et donc leurs sous-lignes (le calque courant reste listé, avec la sienne).
- Elle n'entre pas dans le glisser des calques (seuls les rangs des calques sont des places) ni dans le choix de plusieurs calques.

### Vocabulaire

Tous les textes de l'interface (menus, bulles, barre d'état, palette Ctrl+K, Réglages du projet et du nouveau projet, panneau Historique) disent
« aplat » : « Aplat sous le trait », « Ajouter un aplat », « Retirer l'aplat », « Aplat de <calque> », « Aplat : peinture ». Le code et le format gardent
`fond` (`Layer::fond`, `Cel::fond`, `FondReglages`, `fond.rs`, `fondui.rs`, `fondfil.rs`, les actions `pinceau.fond` et `fond.*`) : **le format des projets ne
change pas**, un projet d'avant s'ouvre tel quel (les aplats déjà posés ont leur sous-ligne). Les calques « de fond » du décor (scène entière, « Fond 3D »)
sont autre chose et gardent leur nom.

### Limites

- La sous-ligne n'est pas dans la timeline ni dans le montage : pour peindre l'aplat on passe par le panneau Calques (voulu).
- « Cet aplat aussi sur » n'est que dans Réglages (il part de l'aplat du calque courant), pas dans le clic droit d'une sous-ligne d'un autre calque.
- Ctrl ou Maj + clic sur une sous-ligne n'ajoute rien au choix de calques (c'est un clic simple).
- Les formes (ligne, rectangle…) et le texte ne se posent pas dans l'aplat.

### Essais ajoutés

`aplat_tests.rs` (16 essais) : activation depuis le calque seulement (aucun réglage d'aplat dans les cinq outils, plus d'action de bascule, bouton sur chaque
ligne) ; clic droit du calque (ajouter / retirer, une étape nommée, autre calque que le courant) ; alias `pinceau.fond` et `fond.*` ; sous-ligne fine (moins de la
moitié d'un rang), seulement dans le panneau Calques (pas la timeline ni le montage), une par aplat, suivant les dossiers fermés ; clic sur la sous-ligne = cible
(outil, barre d'état, pas d'étape), clic sur le calque ou autre calque = trait, retrait et annulation ; indication « Aplat de <calque> » dans la ligne de la vue ;
pinceau, gomme et pot de peinture sur l'aplat seul (trait intact, étapes « Aplat : … », tout s'annule) ; le trait de nouveau après un clic sur le calque ; personnage
(retour au trait, aplat dans la même strate, pot de peinture avec son aplat) ; réglages dans Réglages et dans le menu de la sous-ligne (une étape « Aplat :
réglages » par geste, bon calque) ; projet enregistré et relu inchangé, cible non enregistrée et propre à l'onglet ; application au repos avec la sous-ligne choisie.
Les essais des notes de calques (bouton de la ligne, rien sous le pinceau), des personnages, du fichier de raccourcis et de l'état par onglet suivent le nouveau
vocabulaire.

### À vérifier à l'écran (par Anthony)

1. Panneau Calques : sur un calque de dessin, cliquer le bouton carré-utilisateur (bulle « Aplat sous le trait ») : une sous-ligne fine « Aplat » avec une pastille
   blanche apparaît juste sous le calque. Rien de nouveau dans la timeline ni dans les pistes du montage. Réglages du pinceau : plus de bascule, plus de couleur
   de fond.
2. Dessiner un personnage fermé sur ce calque. Cliquer la sous-ligne : elle se surligne, le calque s'adoucit, la ligne du bas de la vue dit « Aplat de … ». Pinceau
   avec une autre couleur : la peinture reste dans la forme, sous le trait, le trait ne bouge pas. Gomme : l'aplat est coupé (on voit à travers), le trait reste. Pot
   de peinture : un clic dans une zone fermée la remplit sous le trait. Ctrl+Z annule chaque geste (panneau Historique : « Aplat : … »).
3. Cliquer le nom du calque : le pinceau redessine le trait ; la gomme efface du trait.
4. Réglages (outil pinceau, sous-ligne choisie) : « Aplat de … » avec couleur, débord, fermer les trous, bord, « Cet aplat aussi sur ». Clic droit sur la sous-ligne : les
   mêmes réglages et « Retirer l'aplat ». Changer la couleur : l'aplat suit, la pastille aussi ; Ctrl+Z.
5. Clic droit sur un calque (même pas courant) : « Ajouter un aplat » / « Retirer l'aplat ». Retirer : la sous-ligne part ; Ctrl+Z la rend, avec ce qu'on y avait peint.
6. Fermer un dossier qui contient un calque avec aplat : sa sous-ligne part avec lui ; le rouvrir : elle revient.
7. Choisir une pastille de personnage pendant que la sous-ligne est choisie : on revient au trait (la sous-ligne n'est plus surlignée) ; dessiner : trait et aplat du
   personnage. Recliquer la sous-ligne : le pot de peinture prend la couleur d'aplat du personnage.
8. Enregistrer, fermer, rouvrir : l'aplat et sa peinture sont là, la sous-ligne aussi, et c'est le trait qu'on dessine. Ouvrir un projet d'avant : ses aplats ont leur sous-ligne.
9. Au repos, sous-ligne choisie : rien ne bouge, le processeur reste à zéro.

## 36. Variantes de séquence : un autre découpage sans perdre le premier (2026-10-21, branche `variantes-sequence`)

Anthony : « essayer un autre découpage d'une séquence (A plus serrée, B avec un plan de plus…) sans perdre le premier ». Ce ne sont ni des versions du film
entier (trop lourd) ni des versions d'un plan (partie 15) : une variante est **un jeu de plans pour la séquence**. Code : `src/variantes.rs` (modèle et fichier),
`src/variantesui.rs` (sélecteur, menus, fenêtres), essais `src/variantes_tests.rs`.

### Ce que c'est

- Une séquence a toujours **une variante choisie** : ses plans, dans le film. Le **montage, la lecture, les exports, la numérotation, l'export pour le montage,
  la relecture et la fiche technique la suivent** sans rien faire de plus. Les **autres variantes sont rangées** : elles ne sont ni dans le montage ni dans
  les exports, mais elles sont enregistrées avec le projet.
- Les variantes se nomment **A, B, C…** (jusqu'à Z) ; un **nom court** peut suivre la lettre (« B · serré »). Une séquence qui n'a jamais eu de variante n'a rien
  de nouveau à l'écran : c'est la A.
- Une variante rangée est complète : ses scènes, ses plans (dessins, marques d'image, clés de caméra, textes, notes), les versions de chaque plan, leur
  contenu rogné, leurs commentaires, leurs transitions entre deux plans de la variante. Rien d'elle ne traîne dans le film.

### Où ça se trouve

| Geste | Où |
|---|---|
| **Sélecteur « A · B · C »** (clic = passer à la variante ; clic droit = renommer, comparer, supprimer) | une rangée discrète en haut de la vue Vignettes, sous les réglages ; elle porte la **séquence du plan courant**. Une séquence sans variante n'a que l'icône (branche), à demi effacée : clic = le menu « Variantes de la séquence » |
| Nouvelle variante, en dupliquant / en partant d'un plan vide (`variante.nouvelle`, `variante.vide`) | menu de l'icône ; Storyboard > Variantes de la séquence ; clic droit sur la barre de la séquence dans le montage ; Ctrl+K |
| Passer à la variante précédente / suivante (`variante.precedente`, `variante.suivante`) | Ctrl+K (sans touche, à choisir) |
| Renommer, supprimer, comparer (`variante.renommer`, `variante.supprimer`, `variante.comparer`) | clic droit sur une lettre ; mêmes menus ; Ctrl+K |
| Variante choisie, lisible dans le montage | la barre de la séquence dit « Séquence 2 · B » dès qu'elle a plusieurs variantes |

### Les gestes

- **Nouvelle variante, en dupliquant** : le film **ne bouge pas** (mêmes plans, mêmes numéros : la sélection, l'historique, les notes de la planche accrochées aux
  plans, les envois suivent). Ses plans prennent la lettre suivante ; c'est une **copie profonde** (numéros neufs de plans, de scènes, de marques et de calques,
  dessins dupliqués, tuiles partagées jusqu'à la première retouche) qui est rangée sous la lettre d'avant. Les versions des plans, les commentaires, les transitions
  internes et les liens des références sont copiés ; **le contenu rogné ne l'est pas**.
- **Nouvelle variante, en partant d'un plan vide** : le jeu de plans en cours est rangé tel quel ; la séquence repart d'une scène et d'un plan vide.
- **Passer à une variante** : les deux jeux de plans **s'échangent**. Les transitions aux bords de la séquence (avec le plan d'avant, avec le plan d'après) restent
  sur la coupe et passent au nouveau premier ou dernier plan. Les notes de la planche attachées à des plans rangés sont cachées jusqu'à leur retour.
- **Supprimer** (après confirmation : « N plans et M images seront supprimés ») : jamais la dernière variante. Supprimer la variante choisie fait passer la séquence à la
  plus petite des autres. La lettre libérée est reprise par la prochaine variante.
- **Renommer** : un nom court (40 lettres au plus, une ligne) ; vide : la lettre seule.
- Un geste = **une étape d'annulation nommée** (panneau Historique) : « Nouvelle variante de la séquence », « Passer à la variante B », « Supprimer la variante B »,
  « Renommer la variante B ». Le storyboard verrouillé les refuse. Aucun ne touche un pixel sur le fil de l'interface : on échange des listes de plans, sans
  rien recopier ni relire (l'étape d'annulation partage les variantes rangées, `Arc`).
- **Copier-coller entre variantes** : copier des plans (ou des cases de la vue Vignettes) dans une variante, passer à l'autre, coller. Le presse-papiers est autonome.

### Comparer

La fenêtre « Comparer » (partie 15) propose, parmi les côtés du plan courant, « **Variante B** » : le **plan de même rang** (le 2e de la séquence, le 3e…) dans
cette variante, contre le plan d'aujourd'hui, côte à côte, en volet, en fondu ou en superposition. Une variante qui n'a pas de plan à ce rang n'est pas proposée
(« la variante B n'a pas de plan au rang 3 de la séquence » si on la demande quand même). Comparer deux jeux de plans entiers côte à côte n'est pas fait : c'est
plan par plan.

### Envois

- Un envoi note la **variante choisie de chaque séquence qui en a plusieurs** ; la liste des envois la rappelle (« séquence 2 : B »).
- Les plans d'une copie ont des numéros neufs : pour que « modifié depuis l'envoi » reste juste, chaque plan copié se souvient du plan dont il vient, et on le
  compare à celui-là (les empreintes sont celles du contenu, pas des numéros). Passer à une copie non touchée ne signale rien ; retoucher un plan ne signale que
  celui-là ; un plan d'une variante vide est « nouveau ».

### Fichier

`variantes.json` à côté du manifeste, **sans changer le format du projet (36)** : par séquence, la variante choisie et, pour chaque variante rangée, ses scènes, ses
plans (l'entrée du manifeste), leurs versions, le contenu rogné, les commentaires, les transitions. Absent : aucune séquence n'a de variante. Les dessins sont dans
`dessins/` comme les autres. Un fichier illisible ou plus récent n'est jamais effacé, le projet s'ouvre sans les variantes rangées (avertissement) et
l'enregistrement ne nettoie pas `dessins/`. Une version de SHOT qui ne connaît pas le fichier l'ignore et ouvre la variante choisie. Détail : en-tête de `src/variantes.rs`.

### Travailler à plusieurs

- **Créer, passer, supprimer ou renommer une variante demande le jeton du montage** (comme changer l'ordre des plans), et les plans que l'échange fait **sortir** du film
  doivent être à moi ou libres (jamais à un autre : « Léa tient ce plan »). Les plans qui entrent sont à moi dès leur arrivée.
- Les variantes rangées sont une **annexe du montage** : `variantes.json` dans le dossier partagé, écrit par celui qui tient le jeton, avec leurs dessins. Les autres
  postes les relisent quand la structure change. Si des dessins de variantes n'étaient pas arrivés à la lecture, le fichier n'est jamais réécrit et les gestes sont
  refusés (« Des dessins de variantes manquent… ») jusqu'à une relecture complète.
- Voir aussi `docs/COOPERATION.md`, §12.

### Limites connues

- Le contenu rogné n'est pas copié par « en dupliquant » (il suit la variante qu'il a quittée) ; les commentaires et les notes de la planche d'un plan rangé ne se voient
  pas tant que sa variante est rangée.
- Compare plan par plan (même rang), pas deux jeux de plans côte à côte.
- Dans un film partagé, chaque changement de structure relit les dessins des variantes rangées (sur un fil de fond) ; un plan qui revient dans le film retrouve son
  ancien fichier de plan comme après un « annuler » de suppression.
- Pas de fusion de deux variantes ; pas de variantes du film entier (voir les envois et les versions de plan).

### À vérifier à l'écran (par Anthony)

1. Vue Vignettes : sous la rangée de réglages, une petite icône de branche à demi effacée (la séquence du plan courant n'a pas de variante). Cliquer un plan d'une autre
   séquence : l'icône suit. Rien d'autre n'est ajouté à la vue.
2. Cliquer l'icône : « Nouvelle variante, en dupliquant » / « en partant d'un plan vide ». Choisir la première : la rangée montre « Séquence 2 · A B » avec **B** enfoncée ;
   la barre de la séquence dans le montage dit « Séquence 2 · B » ; le film n'a pas bougé.
3. Dans la B, ajouter un plan, en raccourcir un : le montage change. Cliquer **A** : la séquence revient comme avant, la B est gardée ; recliquer B : le plan ajouté est là.
4. Ctrl+Z après chaque passage : le panneau Historique dit « Passer à la variante A » ; l'annulation rend l'autre variante.
5. Clic droit sur une lettre : Renommer… (« serré » : le sélecteur garde la lettre, la bulle et le montage disent « B · serré »), Comparer, Supprimer… (confirmation, jamais la dernière).
6. Fenêtre Comparer sur un plan : « Variante A » est un des côtés ; les deux plans se superposent.
7. Copier un plan dans la A, passer à la B, coller : il est là.
8. Marquer un envoi, passer à la A (une copie), laisser le relevé finir : aucun plan n'est signalé « modifié » ; retoucher un plan : seul lui l'est.
9. Enregistrer, fermer, rouvrir : les deux variantes, leurs noms et la choisie sont là ; l'exemple d'avant (sans variante) s'ouvre sans fichier de plus.
10. Film partagé : sans le jeton du montage, créer ou passer à une variante est refusé avec la raison ; avec le jeton, ça marche, et un plan tenu par un autre empêche le passage.
11. Au repos, avec des variantes : rien ne bouge, le processeur reste à zéro.

## 15. Les quatre panneaux plus simples (2026-10-21, branche `simpl-panneaux`)

Demande d'Anthony : « toujours rendre l'interface la plus simple possible ». Quatre panneaux (Calques, Couleur, Effets,
Réglages) et la création d'un calque ; rien n'a disparu : ce qui a quitté l'écran est dans un menu, le clic droit ou
la palette de commandes (Ctrl+K).

### Calques

- **Cinq icônes** en haut : « + » (nouveau calque), dupliquer, supprimer, monter, descendre ; et « … » à droite.
- **« + »** : un petit menu, une icône et une phrase par sorte : dessin vectoriel, dessin bitmap, calque de couleur, calque d'effet
  (ses préréglages, dont « Calque d'effet vide »), vidéo (choisit le fichier), dossier ; en bas « Nom, groupe, place… »
  (la fenêtre `calque.nouveau`). Les mêmes entrées sont dans Calques > Nouveau calque et le clic droit dans le vide du
  panneau. Les raccourcis existants (`calque.nouveau_vectoriel`, `calque.nouveau_bitmap`, `calque.nouveau_couleur`,
  `dossier.nouveau`, `effet.vide`, `fichier.importer_video`) lancent les mêmes actions qu'avant (`calquemenu.rs`).
- **« … »** : l'aplat du calque courant (ajouter ou retirer), « Effets du calque allumés » (s'il en a), puis toutes les
  commandes des calques que le panneau ne montre pas déjà : presse-papiers (couper, copier, coller), retourner, fusionner
  (avec le dessous, les choisis, les visibles, tous), aplat sous le trait (calques choisis, tout le plan), ordre, groupes,
  dossiers (dont « Placer les calques choisis dans un dossier »), choisir, afficher, pochoirs, nettoyer, calque de couleur.
  Le clic droit sur un calque donne les mêmes (sous-menu « Autres commandes des calques »).
- **Une ligne = une icône de sorte** (vectoriel, bitmap, couleur, effet, vidéo, dossier) avec sa bulle, à la place des mots
  « vectoriel » et « bitmap » ; un dossier porte le nombre de ses calques. Le mode de fusion et l'opacité sont ceux du
  calque courant, montrés une seule fois, sous les icônes.

### Couleur

De haut en bas : les deux couleurs (avant-plan, arrière-plan : le mélange de deux couleurs de certains pinceaux le prend) ;
les neutres (noir, cinq gris, blanc : clic = avant-plan, clic droit = arrière-plan) ; les couleurs du nuancier montré ;
puis « Plus de couleurs », **replié d'office** et retenu dans les réglages de l'application (jamais dans le projet) : la roue
chromatique **ou** les curseurs TSV / RVB / TSL (un seul à la fois, retenu aussi), le code hexadécimal, les couleurs
récentes, et la gestion des nuanciers. Ctrl+Z ne défait pas le nuancier. Le nuancier de départ n'a plus de blanc ni de gris
(la rangée des neutres les porte) ; les nuanciers déjà enregistrés ne changent pas.

### Effets

Chaque effet de la pile se plie et se déplie (flèche à gauche de son nom) : en-tête = flèche, case « actif », nom, poubelle ;
corps = réglages, clés, monter et descendre. Un nouvel effet s'ouvre et les autres se replient ; la première fois qu'on voit un
calque, ses un ou deux effets sont ouverts (au-delà : repliés, sauf l'effet choisi). L'état est celui de la séance. Ctrl+C
et Ctrl+V copient et collent les effets choisis (il n'y a pas de bouton Copier : le clic droit sur le nom, aussi).
Ce qui ne peut pas servir se cache : « Tout réinitialiser » et « Tout supprimer » sans effet, les effets du trait sur un
calque bitmap (le menu « Trait » fait place à une ligne qui dit où est le tremblement), « Supprimer la clé » sans clé ici.

### Réglages

Pinceau, crayon, gomme : taille, appui, opacité et nom en haut ; sous l'aperçu du trait, « Plus de réglages » (replié d'office,
retenu) : les catégories de la brosse, « Peint derrière », « Remplir la forme » et le mode d'emploi. Le pot de peinture :
tolérance et contours en haut, débord dans « Plus de réglages ». Sélection : tout sélectionner, copier, coller, supprimer (et
couper, remplir sur un bitmap) en haut, cachés tant qu'ils ne peuvent pas servir ; miroirs, devant / derrière, champs de la
boîte et mode d'emploi dans « Plus de réglages ». Transformer : position, taille et angle en haut ; clés de position et mode
d'emploi dans « Plus de réglages ». Inchangés (déjà deux ou trois réglages) : formes, texte, déformer, caméra, cutter, pipette.
L'état de chaque outil est dans `[panneau_pinceaux] plus_outils`.

### À regarder à l'écran

1. Calques : cinq icônes et « … » ; le « + » ouvre six entrées avec leur phrase ; « … » > Presse-papiers > Copier ; clic droit
   sur une ligne : « Autres commandes des calques » ; les lignes montrent une icône, pas un mot.
2. Couleur : noir, cinq gris, blanc sous les deux carrés, puis les couleurs ; « Plus de couleurs » se déplie, la roue et les
   curseurs ne sont jamais ensemble ; fermer et rouvrir SHOT : l'état est retenu.
3. Effets : ajouter trois effets ; seul le dernier est ouvert ; la flèche plie et déplie ; sur un calque bitmap, pas de menu « Trait ».
4. Réglages : pinceau, pot, sélection : peu de choses, « Plus de réglages » replié ; l'ouvrir, changer d'outil, revenir.

### Essais ajoutés (`panneaux_tests.rs`)

Cinq icônes et « … » ; une icône de sorte par ligne ; le menu « + » (sortes, phrases, effet vide) ; chaque commande retirée de la
barre reste atteignable (« … », « + », palette) ; neutres dans l'ordre ; ordre du panneau Couleur ; roue ou curseurs, jamais
les deux ; état replié et choisi retenu dans les réglages, pas dans le projet ; Ctrl+Z laisse le nuancier ; effets repliés,
nouvel effet ouvert (`fxui::plis_apres`) ; boutons cachés plutôt que grisés ; « Plus de réglages » replié et retenu par outil.

---------------------------------------------------------------------------------------------------

## 40. Barres simplifiées (2026-10-21, branche `simpl-barres`)

« Toujours rendre l'interface la plus simple possible » : huit points, détaillés dans `docs/DOUBLONS.md` (l'inventaire des doublons, les choix « cacher plutôt que griser »,
les questions pour Anthony).

1. **Une commande, un endroit principal.** La barre de commandes ne garde que les neuf icônes du fichier et de l'édition (les plans et le transport n'y sont plus, même
   sous « Plus… »). La pelure d'oignon se règle sous le dessin seulement ; le panneau Effets et les Bruitages ont leur icône dans la barre des panneaux, pas une seconde dans
   les barres du montage ; « Animer ce plan » est l'onglet Animation. Les vues (plateau, caméra, Espace 3D) sont dans la barre AU-DESSUS du dessin, avec un seul menu
   « Cadrage » (les zones de sécurité et le format n'ont plus de bouton ni de liste en plus). Les menus du haut restent le catalogue.
2. **Les panneaux de la barre de droite, par espace.** `barrepanneaux::panneaux_d_office` : Vignettes 4 icônes (Réglages, Pinceaux, Couleur, Références), Storyboard 9,
   Animation 9, Montage 4 (Fiche, Commentaires, Historique, Bruitages). Un panneau ouvert à la main a son icône tant qu'il est ouvert (chaque espace retient sa
   disposition, la barre ne se bat pas contre l'utilisateur) ; les autres : Ctrl+K et Affichage > Espaces de travail > Panneaux.
3. **Légende des clés et filtre** (`legendecles.rs`) : une icône « ? » au bout de l'en-tête du montage et de la timeline ; au survol, la légende (dessin bleu et vert,
   caméra jaune, effet violet, position verte, transition) avec une case par sorte pour cacher caméra, effets, positions, transitions. **Un affichage seulement** : la sorte
   cachée n'est ni dessinée ni saisie à la souris, le projet ne change pas, la lecture et l'export montrent tout. Retenu dans les réglages (`cles_vues`), pas dans le projet.
   Les clés de dessin sont le contenu des pistes : elles ne se cachent pas. L'icône est allumée quand une sorte est cachée.
4. **Barre d'état courte** : à gauche, le format et la cadence, le zoom, le panneau qui a la main (quand il y en a un), l'état d'enregistrement ; plus de titre du plan,
   d'espace de travail, de rotation, de miroir ni de position du curseur. Le message est une astuce d'une ligne (`messages::resume` coupe avant la première parenthèse ou le
   premier point-virgule au-delà de 70 caractères) ; le message entier est dans sa bulle et dans l'historique des messages.
5. **Clic droit : une dizaine d'entrées au plus** (essai `menus_tests`) : plan, vignette, fond de la planche, calque (montage, en-tête de la timeline, panneau Calques), images,
   ligne de la caméra, règle de la timeline, extrait de son, menus « Repères » et « Affichage ». Le rare est dans « Plus » ou un sous-menu, rien n'est retiré.
6. **Cacher plutôt que griser** dans les barres : une commande qui ne s'applique pas disparaît (coller sans rien de copié, clé de caméra ici / supprimer / adoucir, plan
   modifié sans envoi, renuméroter sans verrou) ; sous « Plus… » elle reste, grisée, avec la raison dans la bulle.
7. **Mêmes icônes, mêmes écarts** : `barres::ICONE` (17), `barres::CASE` (28 × 26), `barres::ECART` (2) et `barres::MARGE_TRAIT` (3) pour la barre de commandes, la barre d'outils
   (qui passe de 44 à 40 de large), la barre des panneaux (34), les deux barres de la vue, les en-têtes du montage et de la timeline (`toolbars::case_icone`).
   Les traits entre groupes ont la même marge partout.
8. **Barre sous le dessin réduite à des icônes** : Ajuster, loupe, rotation, miroir, pelure d'oignon, table lumineuse. Les réglages fins (pourcentage du zoom, angle, ± 15°,
   avant et après de la pelure, force de la table lumineuse) sont dans de petits panneaux qui s'ouvrent au clic au-dessus de l'icône (`viewbar::panneau_de_case`) ; la pelure
   et la table lumineuse ont une petite flèche à côté de leur icône. Échap ou un clic ailleurs les ferme.

### Fichiers

`barres.rs` (barres, constantes de taille), `barrepanneaux.rs` (icônes par espace), `legendecles.rs` (légende, filtre), `viewbar.rs` (barres de la vue), `toolbars.rs`
(`case_icone`), `messages.rs` (`resume`), `panels.rs` (barre d'état), `montagehead.rs`, `layerheads.rs`, `storyboard.rs`, `boardui.rs`, `handles.rs`, `images.rs`, `cues.rs`,
`soundtrack.rs`, `annotationsui.rs` (menus du clic droit). Essais : `barres_tests`, `barrepanneaux_tests`, `legendecles_tests`, `menus_tests`, `messages_tests`.

### À vérifier à l'écran (par Anthony)

1. Vignettes : la barre de droite n'a que quatre icônes. Montage : quatre aussi (Fiche, Commentaires, Historique, Bruitages). Ouvrir un panneau par Ctrl+K : son icône apparaît ;
   le refermer : elle repart. Quitter l'espace et y revenir : le panneau est où on l'a laissé.
2. Survoler le « ? » au bout de la barre du montage : la légende s'affiche sous l'icône ; décocher « Clé de caméra » : les losanges jaunes disparaissent de la piste Caméra
   (et ne se saisissent plus), le « ? » s'allume ; recocher : ils reviennent, rien n'a changé dans le projet. Même chose dans la timeline.
3. Barre sous le dessin : cliquer la loupe (pourcentage, − , +, 100 %, Ajuster), l'icône de rotation (angle), la petite flèche de la pelure d'oignon (avant, après, tous les
   réglages), celle de la table lumineuse. Toutes les cases ont la même taille que celles de la barre du haut.
4. Clic droit sur un plan du montage : neuf lignes ; « Plus » contient la couleur, les transitions, le rognage et l'import d'un son.
5. Barre d'état : « 1920×1080 · 24 i/s · 35 % » puis le message court ; le message long (par exemple en entrant dans l'Espace 3D) est entier dans la bulle.

## 17. Accueil, concentration, conseils, taille de l'interface, aide-mémoire et réglage du stylet (2026-10-21, branche `simpl-accueil`)

Passe « simplicité » : *zéro latence* d'abord, puis *toujours rendre l'interface la plus simple possible*. Six choses, un fichier chacune (`accueil.rs`,
`concentration.rs`, `conseils.rs`, `echelle.rs`, `aidetouches.rs`, `reglagestylet.rs`), l'état partagé dans `confort.rs` (un seul champ d'`App`, `confort`,
rangé dans `PARTAGES` : rien de tout cela n'est dans le projet).

### Vocabulaire
Plan, Case, Vignettes, Piste, Papier, Aplat : les mots de ce chantier sont ceux du glossaire (partie 2). Aucun mot n'a été renommé ailleurs.

### Les touches choisies

| Touche | Commande | Pourquoi elle est libre |
|---|---|---|
| **Z** | `vue.concentration` — mode concentration (la même touche remet tout) | Aucune commande de `keymap.toml`, du jeu « Façon Storyboard Pro » ni du fichier personnel d'Anthony (B, E, L, Maj+1, Maj+2) ne porte Z seule. `Maj+Z` (Zoom 100 %) et `Ctrl+Z` (Annuler) gardent leur sens. Tab (mode) et X (coller les calques) sont pris. Z comme « zen ». |
| **R** (tenue) | `aide.aide_memoire` — l'aide-mémoire des raccourcis, tant que la touche est tenue | Idem : R seule est libre ; `Ctrl+R`, `Alt+R` (outil formes) et `Ctrl+Maj+R` sont d'autres combinaisons. R comme « raccourcis ». |

Garde-fous : `keymap_tests::no_two_default_bindings_share_a_key_in_the_same_context` (déjà là) échouerait si l'une des deux touches était prise, et
`concentration_tests::z_and_r_are_free_keys_with_no_conflict_in_either_key_set` vérifie, dans SHOT et dans le jeu Storyboard Pro, que Z et R vont bien à ces deux
commandes, en storyboard comme en animation, et que `Maj+Z` / `Ctrl+Z` n'ont pas bougé. Les touches se changent comme les autres (Préférences > Raccourcis, F1, clic droit >
« Assigner un raccourci… », fichier personnel) ; la carte et les conseils lisent la touche du moment. `aide.conseils` et `aide.regler_stylet` n'ont pas de touche.

### 1. L'accueil
Trois grandes entrées, et rien d'autre de grand : **Dessiner des vignettes** (un projet vide, tout de suite, dans l'onglet Vignettes ; c'est l'ancien « Commencer sans
projet »), **Importer un scénario** (la fenêtre d'import existe, `scenarioui.rs` : depuis l'accueil elle part d'un nouveau projet) et **Ouvrir un projet** (SHOT ou Storyboard Pro :
la fenêtre d'ouverture accepte les deux). Dessous, un petit « Nouveau projet… » (le choix du format), les projets récents, et « Rouvrir le dernier projet au lancement ». Les anciens
boutons « Importer un projet Storyboard Pro » et « Commencer sans projet » ont disparu de la carte (ils sont dans Fichier > Importer et dans Ouvrir). L'accueil s'efface pendant
que la fenêtre d'import d'un scénario ou d'un projet Storyboard Pro est ouverte, et revient si on la ferme sans rien importer.
**Déposer des images** (`deposer.rs`) marche dès la première entrée : l'onglet Vignettes montre la planche, qui prend les fichiers lâchés (essai
`accueil_tests::images_dropped_on_the_board_work_right_after_the_welcome_entry`). Lâcher des images sur la carte d'accueil elle-même ne fait rien (il faut d'abord « Dessiner des vignettes »).

### 2. Le mode concentration (Z)
Cache les menus, les onglets de projets, toutes les barres, la barre d'état, la barre des panneaux et **tous les panneaux** ; le dessin prend la fenêtre, sous une barre
minimale (l'outil et la couleur, le plan, annuler, rétablir, et le bouton de sortie). C'est une **couche posée par-dessus** : `App::frame` ne dessine plus les barres ni le dock, mais le dock,
les dispositions des quatre espaces et les panneaux tirés à la main ne sont **ni lus ni écrits** autrement qu'à l'ordinaire : l'éteindre rend exactement ce qu'on avait (essai
`focus_restores_the_exact_layout_even_after_the_user_arranged_panels`). Les ensembles de panneaux par onglet (autre chantier) ne sont donc pas concernés. **Sans latence** : un booléen ; la
touche agit dans l'image d'écran où elle arrive. Les panneaux posés dans leur propre fenêtre (autre écran) restent. Jamais sur l'accueil. Aussi dans Affichage, dans Ctrl+K.

### 3. Les conseils d'un seul passage
Une bulle discrète, en bas à droite (là où se pose « Régler SHOT pour cette machine ? »), une phrase, un bouton **Compris**. Elle vient la première fois seulement, est notée vue dès qu'elle
apparaît (`reglages.toml`, clé `[conseils]`), s'en va avec son endroit, et **n'est jamais là en même temps qu'une autre** ni avant la réponse à la proposition du premier lancement, ni sur l'accueil, ni
pendant une boîte de dialogue. Cinq conseils : Vignettes, Montage, Animation (la première entrée dans l'onglet), Espace 3D (la première entrée) et « Z / R » (la première modification d'un projet dans
l'onglet Storyboard). Préférences > Général > **Revoir les conseils** (et Aide > Revoir les conseils) les rend tous. Les essais n'en voient jamais (`conseils_actifs` n'est vrai que dans l'application lancée par l'utilisateur).

### 4. La taille de l'interface
Préférences > Apparence > Échelle : 90, 100, 125 ou 150 %, tout de suite, retenue (`echelle_interface`, absente à 100 %). C'est le facteur de zoom d'egui. Les positions de la souris et du stylet sont en points,
divisées par ce même facteur par egui-winit : un point de l'écran reste un point du dessin ; `pixels_per_point` (zoom % de la barre d'état, « 1 pixel = 1 pixel », vignettes, logo) contient déjà le facteur.
Deux corrections au passage : le dépôt de fichiers lisait le curseur avec les pixels par point *de l'écran* au lieu de ceux de l'interface (`deposer::curseur_en_points`), et les raccourcis d'egui
(Ctrl +, Ctrl -, Ctrl 0, qui changeaient la taille sans rien retenir, Ctrl 0 compris) sont coupés. Essais : `echelle_tests` (le trait arrive où la vue le place à 90, 100, 125 et 150 %).

### 5. L'aide-mémoire des raccourcis (R tenue)
Une carte légère au milieu du dessin : trois colonnes — l'outil courant, l'onglet courant, ce qui vaut partout — lues dans le fichier de raccourcis en vigueur (une touche changée se lit changée ; une commande sans touche n'a
pas de ligne ; les gestes de souris, comme Alt + clic, sont écrits tels quels). Relâchée, elle part. La liste complète, avec recherche, reste sous F1.

### 6. Régler le stylet à ma main
Aide > Régler le stylet à ma main… (et Préférences > Dessin) : trois traits (léger, moyen, appuyé), la courbe de pression (`Toucher::pressure_curve`, la même que « Toucher du stylet » dans les
réglages du pinceau) passe par (léger, 0,2), (moyen, 0,55) et (appuyé, 1). L'aperçu redessine les trois traits avec le réglage d'aujourd'hui et avec le réglage proposé, et trace les deux courbes ;
**rien n'est changé avant « Appliquer »**, « Rétablir » rend la courbe d'avant. Trois traits trop semblables : la fenêtre le dit et ne propose rien. **La souris appuie toujours à 100 %** : ses traits ne sont pas
mesurés et la fenêtre le dit.

### À vérifier à l'écran (par Anthony)
1. Lancer SHOT sans projet : trois grandes entrées. « Dessiner des vignettes » ouvre l'onglet Vignettes, vide ; déposer une image de l'Explorateur sur une Case.
2. Z : tout disparaît sauf le dessin et la barre mince ; Z (ou le bouton) remet tout comme avant, dans chaque onglet. Dessiner pendant le mode : rien ne saute.
3. Premier passage dans Montage, Vignettes, Animation, Espace 3D : une bulle, une seule ; « Compris » la ferme ; elle ne revient pas. Préférences > Général > Revoir les conseils.
4. Préférences > Apparence > Échelle : 125 %, 150 %, 90 % tout de suite ; dessiner à la souris et au stylet : le trait part de la pointe. Relancer : la taille est gardée.
5. Tenir R : la carte des raccourcis ; la relâcher : elle part. Changer la touche du pinceau « plus gros » dans F1 : la carte la montre.
6. Aide > Régler le stylet : trois traits, aperçu, Appliquer ; ouvrir « Toucher du stylet » dans les réglages du pinceau : la courbe a changé ; Rétablir.

### Essais ajoutés
`accueil_tests`, `concentration_tests`, `conseils_tests`, `echelle_tests`, `aidetouches_tests`, `reglagestylet_tests`.


---------------------------------------------------------------------------------------------------

## 41. La règle des guides, le menu Cadrage et la colonne des pastilles (2026-10-21, branche `retouches-dessin`)

Trois retouches demandées par Anthony sur une capture annotée de l'onglet Storyboard.

1. **La règle est LE lieu des guides.** L'icône règle de la barre au-dessus du dessin (après le plateau, la caméra et l'Espace 3D) ouvre le menu des guides :
   en haut les repères de composition (tiers, grille d'or, spirale d'or, son orientation, diagonales, croix centrale, et « Éteindre les repères »), dessous « Perspective »
   (tout ce que faisait le panneau Guides : afficher et utiliser, aligner les traits, verrouiller, lignes de référence, aligner aussi la gomme, lignes du curseur jusqu'au bord ;
   la liste des guides du projet ; un guide de plus à 1, 2 ou 3 points de fuite ; le nom du guide courant et le supprimer). L'icône s'allume dès qu'un repère ou le guide de
   perspective est tracé (`guides_any_on`). **Le panneau Guides n'existe plus** : plus d'icône dans la barre des panneaux, plus d'action `vue.guides`, plus d'onglet dans les
   dispositions d'origine ; `Pane::Guides` n'est gardé que pour lire les dispositions enregistrées, que `DockLayout::sanitized` en débarrasse (comme l'ancien panneau Vue).
   Les actions `guide.*` (Ctrl+K, raccourcis) et le sous-menu Affichage > Guides restent.
2. **Le menu Cadrage, tout à gauche de la barre sous le dessin** (icône « cadre », avant « Ajuster »). Un petit panneau au-dessus de l'icône : le **format** (Aucun, 16:9,
   9:16, 4:5, 1:1, 4:3, 1,85:1, 2,39:1, Personnalisé avec sa largeur : sa hauteur) et les **zones de sécurité** (l'interrupteur et les deux jeux de valeurs). Même taille
   d'icône que le reste de la barre (`barres::ICONE`, `CASE`). L'icône s'allume avec un format ou les zones de sécurité (`framing_any_on`). Pas de sous-menu ni de liste
   déroulante dans ce panneau (il se ferme au clic hors de lui, et un sous-menu est « hors de lui ») : des choix en ligne. L'**aspect des traits** des repères (six couleurs,
   « Autre couleur », opacité, épaisseur) ne règle que les repères de la règle (tiers, spirale…) : il est sous la règle, à côté d'eux (Anthony, 2026-10-22).
3. **La colonne des pastilles des personnages** (`pastilles_colonne`, `personnagesui.rs`). Plus de barre au-dessus du dessin : une colonne ronde et discrète au bord droit du
   dessin (46 points sous le haut, donc sous le titre « Séquence 1 — plan 1 ») et de la planche des Vignettes (8 points sous le haut). De haut en bas : « aucun personnage »
   (anneau barré, allumé quand la couleur est libre ; le texte « Aucun personnage » a disparu, la bulle le dit), les personnages, un trait, les couleurs neutres, « + », l'icône du
   panneau Personnages. Mêmes gestes qu'avant (clic : choisir ou lâcher ; clic droit : Modifier, Dupliquer, Monter, Descendre, Supprimer ; bulle d'une seconde avec le nom et
   les couleurs). Fond sombre à moitié transparent, 28 points de large ; la barre de 26 points a disparu, le dessin ne perd rien. C'est une `Area` (ordre « milieu ») posée
   en fin d'image : la couche d'egui au-dessus du dessin reçoit les clics, `layer_id_at` ne rend plus la couche du dessin, donc **cliquer ou glisser sur la colonne ne
   dessine rien** (ni stylo d'annotation sur la planche : la colonne tient le haut du bord droit). Elle défile (sans barre) si la fenêtre est trop basse. Rien de lourd par image :
   une dizaine de cercles ; les cases (`Etat::cases`) sont gardées pour les essais.

### Où est passé chaque réglage du panneau Guides

| Avant (panneau Guides) | Maintenant (menu de la règle, barre au-dessus du dessin) |
|---|---|
| Afficher et utiliser le guide (G) | icône perspective, ligne « Perspective » |
| Aligner les traits (Maj+A), Verrouiller (Maj+L), Lignes de référence (Maj+G), Aligner aussi la gomme, Lignes du curseur jusqu'au bord | les cinq icônes suivantes, même ligne |
| Liste des guides du projet | la liste, sous les icônes |
| Nouveau guide à 1, 2, 3 points de fuite | trois icônes « 1 », « 2 », « 3 » |
| Nom du guide courant, Supprimer | le champ et la corbeille, à droite des trois icônes |

### À vérifier à l'écran (par Anthony)

1. Storyboard : la colonne des pastilles au bord droit du dessin, sous « Séquence 1 — plan 1 » ; plus de barre au-dessus du dessin ; survol 1 s : « Aucun personnage » ; clic sur une
   pastille neutre : le trait prend sa couleur et rien ne se dessine ; « + » : un personnage, sa fenêtre s'ouvre ; clic droit : le menu.
2. Vignettes : la même colonne au bord droit de la planche.
3. La règle (barre du haut) : repères, perspective, guides ; la barre des panneaux n'a plus d'icône Guides ; plus d'onglet Guides à droite.
4. Cadrage (tout à gauche de la barre du bas) : choisir 16:9, allumer les zones de sécurité, régler la couleur : les traits du cadre suivent.

### Essais ajoutés

`regle_cadrage_tests.rs` : chaque fonction de l'ancien panneau Guides est dans le menu de la règle et agit au clic ; la règle ne garde que les guides et les repères ; le menu Cadrage est
la première icône de la barre du bas (même taille, format, zones de sécurité, aspect) ; la colonne des pastilles (au bord droit, mince, sous le titre ; choisir, ajouter, aucun personnage,
neutres, clic droit ; cliquer ou glisser dessus ne dessine rien et ne crée aucune étape d'annulation ; la même sur la planche des Vignettes). `barrepanneaux_tests` : plus de panneau Guides
dans la barre, les jeux d'origine ni les dispositions.

## 42. Retouches d'interface (2026-10-23, branche `retouches-interface`)

Quatre retouches demandées par Anthony sur une capture annotée.

### 1. Le point fixe des poignées, la rotation autour du coin opposé

Deux réglages de l'application (`reglages.toml` : `point_fixe_poignees`, `rotation_autour_de` ; absents : les valeurs d'office), pour toutes les boîtes à poignées : la boîte
de la Sélection (vectorielle, bitmap, plusieurs calques, Sélection par couleur), le Transformer sur une zone bitmap et le Transformer d'un calque (un ou plusieurs calques
choisis). Ils sont au-dessus de « Plus de réglages » des outils Sélection, Sélection par couleur et Transformer (deux petits choix à deux icônes, `point_fixe_ui`) et dans
Préférences > Dessin, groupe « Poignées de la Sélection et du Transformer ».

- **Point fixe des poignées** : « Poignée opposée » (d'office) — un coin ou le milieu d'un côté tiré (mettre à l'échelle) et un bord tiré (incliner) gardent la poignée
  opposée en place ; « Centre » — tout se fait autour du pivot (le centre de la boîte, s'il n'a pas été déplacé ; le centre de la boîte pour le Transformer d'un calque).
  **Ctrl tenu pendant le geste donne l'autre mode** : c'était déjà le geste de Ctrl (« autour du pivot »), il l'est toujours avec le réglage d'office. Le mode « Centre » est
  exactement l'ancien geste avec Ctrl (essai `centre_mode_is_exactly_the_old_ctrl_gesture…`). Maj garde les proportions, comme avant ; « Garder les proportions » aussi.
- **Rotation autour de** : « Pivot » (d'office, comme avant) ou « Coin opposé » — une rotation prise juste hors d'un coin tourne autour du coin opposé de la boîte. Le petit
  cercle du pivot passe sur ce coin dès qu'on survole la zone de rotation, et pendant le geste. La poignée ronde du Transformer tourne toujours autour du centre.
- Le **Transformer d'un calque** a maintenant lui aussi la rotation juste hors d'un coin (comme la boîte de la Sélection, curseur de rotation), en plus de sa poignée ronde.

Remarque : avant ce réglage, le geste sans touche gardait déjà la poignée opposée (Sélection et Transformer) ; seul Ctrl faisait tourner autour du pivot. Le mode
d'office reproduit donc l'ancien geste sans touche, et « Centre » l'ancien geste avec Ctrl.

### 2. Les réglages du pinceau, du crayon et de la gomme

- **« Peint derrière » s'appelle « Peint sur l'aplat »** (nom, bulle, barre d'état, liste des raccourcis, aide-mémoire). Son effet ne change pas encore (la peinture passe
  sous ce qui est déjà dessiné) ; la clé `peint_derriere` des réglages et l'action `pinceau.derriere` gardent leur nom.
- **« Remplir la forme » est retiré** : il faisait double emploi avec l'aplat sous le trait. Choix : **son effet s'arrête**, il n'est pas reporté sur l'aplat. C'était un
  réglage de l'application, global (tous les calques, tous les projets), qui posait sous chaque trait fermé une zone de la couleur du trait ; l'aplat est un réglage du
  **calque**, une seule forme recalculée, de sa propre couleur. Allumer de soi-même un aplat sur des calques parce qu'un ancien fichier de réglages avait cette case cochée
  changerait des projets sans qu'on l'ait demandé. Un ancien `reglages.toml` avec `remplir_la_forme = true` s'ouvre toujours (lu, sans effet, plus écrit) ; les brosses ne
  l'ont jamais porté. L'action `pinceau.remplir` (un raccourci déjà posé dessus) devient un double de `pinceau.fond` : elle ajoute ou retire l'aplat du calque courant.
- **Au-dessus de « Plus de réglages »**, sous l'aperçu du trait, toujours visibles (pinceau et crayon) : l'interrupteur « Peint sur l'aplat » et le bouton d'aplat du calque
  courant (le même que sur sa ligne du panneau Calques : ajouter ou retirer l'aplat).
- **La ligne « Le trait de ce calque bout »** (et son icône de tremblement) n'est plus dans les réglages du pinceau et du crayon : le tremblement du trait se règle dans le
  panneau Effets (et l'action `fx.tremblement`).