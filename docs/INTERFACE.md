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
`outil.perspective` (Déformer), `outil.texte`, `outil.rectangle` / `ellipse` / `ligne`, `vue.3d`,
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
- **Volontairement hors thème** : les vues techniques (Vue de dessus, plateau 3D : fond sombre et gris
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
leur sens. Ctrl+Maj remplace la sélection (il n'ajoute pas).

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
| Barre au-dessus du dessin | vue plateau / caméra, 3D, zones de sécurité, format, aides au cadrage |
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

Aucune n'est faite ici : c'est la partie 6.3 (barre par défaut plus mince).

---------------------------------------------------------------------------------------------------

## 6. Propositions pour Anthony (non faites)

### 6.1 Espaces de travail par tâche : Storyboard, Animation, Montage

- **Quoi** : trois dispositions enregistrées (panneaux visibles, leurs places, barres visibles), que les
  boutons de mode existants (« Storyboard » / « Animation ») et le menu Affichage > Espaces de travail
  appliquent. « Montage » : la bande du montage large, les panneaux de dessin repliés.
- **Existant** : `workspace.rs` / `workspaceui.rs` ont déjà des espaces (Standard, Dessin, Timeline, Vue
  d'ensemble) et savent enregistrer ses propres dispositions ; ce qui manque est le lien avec le mode.
- **Coût** : petit à moyen, 1 à 2 jours (lier un espace à chaque mode, retenir la dernière disposition
  par mode, trois dispositions livrées, essais d'aller-retour).
- **Ce qu'Anthony voit** : passer en Animation ouvre les panneaux Calques, Effets et la timeline en grand ;
  revenir en Storyboard remet le montage et la fiche du plan. Sa disposition de chaque mode se retient.

### 6.2 Préréglage « raccourcis façon Storyboard Pro »

Seulement ce que `keymap.toml` documente (les 65 raccourcis de `sb_shortcuts.xml` de Storyboard Pro 27,
leur identifiant est dans le champ `source`). Aujourd'hui SHOT s'écarte de Storyboard Pro sur :

| Commande SHOT | Storyboard Pro 27 | SHOT aujourd'hui | Raison notée dans le fichier |
|---|---|---|---|
| plan précédent | `PREV_PANEL_KEY` : A | PgPréc | A / F vont à la clé précédente / suivante (choix d'Anthony) |
| plan suivant | `NEXT_PANEL_KEY` : F | PgSuiv | idem |
| coller sur place | `PASTE_IN_PLACE_KEY` : Ctrl+Maj+V | pas de touche | Ctrl+Maj+V colle à la tête de lecture (choix d'Anthony) |
| couper le plan | `SPLIT_PANEL_KEY` : F5 (ses réglages perso) | F6 | F5 pose une clé |
| supprimer et resserrer | `DELETE_ALT_KEY` : Maj+Suppr | pas de touche | commande pas faite, touche prise |
| décharger la référence | `DRAWING_UNLOAD_MODEL_KEY` : Suppr | pas de touche | commande pas faite, touche prise |
| lier les clips au panneau | `LINK_CLIP_AND_PANEL` : Ctrl+L | retirée (Ctrl+L = pelure du calque) | sans objet depuis les plans |
| pinceau pochoir | `DRAWING_STENCIL_BRUSH_TOOL_KEY` : Alt+X | retirée | le pochoir de calque existe |

Le préréglage serait un fichier `keymap-storyboard-pro.toml` qui redéfinit seulement ces actions
(`panneau.precedent` = A, `panneau.suivant` = F, `cle.precedente` / `suivante` = rien,
`edition.coller_sur_place` = Ctrl+Maj+V, `edition.coller_a_la_tete` = rien, `panneau.couper` = F5,
`dessin.nouvelle_cle` = rien tant que F5 sert à couper), choisi dans Préférences > Raccourcis clavier.

- **À vérifier** (rien d'écrit sans la source) : ce que Storyboard Pro fait sur Tab et Maj+Tab ; les
  touches de la Palette de commandes (Ctrl+K est une proposition de SHOT) ; les touches F4 / F5 pour une
  nouvelle clé (SHOT les a prises de ses réglages perso : « à vérifier » dans Storyboard Pro) ; toutes les
  commandes que SHOT a et que Storyboard Pro 27 n'a pas par défaut (marquées « sans touche par défaut »
  dans le fichier : on n'invente pas).
- **Coût** : petit, 0,5 à 1 jour (le fichier, un sélecteur dans Préférences, un essai qui compare le
  fichier aux sources du `keymap.toml`).
- **Ce qu'Anthony voit** : un choix « Raccourcis : SHOT / Storyboard Pro » ; en Storyboard Pro, A et F
  changent de plan comme dans son logiciel et F5 coupe un plan.

### 6.3 Barre par défaut plus mince

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
- **Préférences** : section « Génération sonore » repliée (les trois rôles ; le serveur local : dossier des modèles,
  Python, case « Démarrer le serveur tout seul… », port, « Démarrer le serveur local », « Arrêter », « Tester » ; les
  clés, « Tester la connexion », une note de licences). La fenêtre Préférences défile quand elle dépasse l'écran.
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
5. **Préférences** : la section repliée ouverte avec les trois rôles en ligne (hauteur, défilement de la fenêtre sur un
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
