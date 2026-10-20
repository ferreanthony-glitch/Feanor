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
| Les objets 3D posés pour le blocage | **scène 3D** (« scène » n'est plus le mot du plan) ; **décor** (commun à la séquence) ; **caméra du plateau** (celle de la scène 3D, distincte de la **caméra** 2D du plan) ; **calque « Fond 3D »** | maquette, set, vue 3D (la « vue 3D » est le plateau en 3D) |
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

## 13. Scène 3D de blocage (2026-10-19, branche `scene3d`)

Une vue de plus (icône cube, à côté de « Plateau en 3D » dans la barre sous le dessin ; les deux s'excluent ; depuis
2026-10-20 la barre au-dessus du dessin ne les répète plus, elle ne montre que leurs outils quand l'une est active),
avec ses réglages dans le panneau **Réglages** (qui montre la scène 3D tant qu'elle est ouverte). Tout y est
en icônes avec bulle (nom, description, raccourci), comme le reste.

- **Commandes** (`scene3d.*`, aucune touche par défaut, toutes dans Ctrl+K) : afficher / quitter, outils
  (choisir, déplacer, tourner, échelle), ajouter (cube, plan, cylindre, sphère, cône, escalier, mannequin),
  importer un modèle, dupliquer, supprimer (Suppr y suffit), grouper, dégrouper, vue par la caméra, cadrer,
  envoyer vers un calque de fond, caler la caméra sur la caméra 2D, aimantation, grille, décor / plan.
- **Souris dans la vue** : clic = choisir (Maj ou Ctrl : ajouter) ; glisser un objet = le poser sur le sol (Maj :
  en hauteur) ; poignées du manipulateur pour déplacer, tourner, mettre à l'échelle ; clic droit ou Alt + glisser =
  tourner la vue ; bouton du milieu = la glisser ; molette = avancer. En « Vue par la caméra », ces gestes
  déplacent la caméra du plateau. Mêmes gestes que le plateau en 3D (clic droit, milieu, molette).
- **Annulation** : une étape par geste, au nom français : « Ajouter : cube », « Déplacer dans la scène 3D »,
  « Tourner dans la scène 3D », « Mettre à l'échelle dans la scène 3D », « Supprimer des objets 3D », « Dupliquer… »,
  « Grouper… », « Envoyer la scène 3D vers un calque de fond »…
- **Aucun repaint au repos** : la vue ne demande une image que si on la touche ou si un rendu vers le calque est en
  cours (toutes les 40 ms, jusqu'à sa fin). Le dessin n'est jamais ralenti : le rendu part sur un autre fil (la moitié
  des cœurs), et son résultat n'est posé ni pendant un trait, ni pendant un geste dans la vue 3D, ni en lecture.
- **À vérifier à l'écran (par Anthony)** : lisibilité des poignées et de la boîte de sélection dans les trois
  thèmes ; le décor estompé quand on modifie le plan ; la taille des poignées à toutes les distances ; le panneau
  Réglages dans un panneau étroit ; les conventions empruntées aux logiciels 3D (rouge / vert / bleu pour X / Y / Z,
  flèches / cercles / cubes, tourner au clic droit) **à vérifier** avec ce qu'il attend.

## 14. Espaces de travail liés aux modes, barres minces et « Plus… » (2026-10-19, branche `espaces-barres`)

Les propositions 6.1 et 6.3, approuvées par Anthony. Rien n'a été regardé à l'écran dans cette séance (aucun
lancement de SHOT) : voir « À vérifier » plus bas.

### Espaces de travail (6.1)

- **Trois espaces, chacun sa disposition** : Storyboard, Animation, Montage (`workspace::Espace`). Changer de mode
  (boutons « Storyboard » / « Animation », Tab, « Animer ce plan ») pose la disposition de l'espace d'arrivée ;
  ce qu'on change dans un espace se retient pour lui seul (réglages `disposition`, `disposition_animation`,
  `disposition_montage`, `espace_storyboard`, toutes avec une valeur d'origine : un fichier ancien se lit tel quel).
- **Montage est une disposition du mode Storyboard, pas un troisième mode** (choix expliqué) : mêmes outils,
  mêmes raccourcis (les touches du montage), même dessin, et le bouton « Animer ce plan » entre en animation depuis lui.
  Un troisième mode aurait doublé les contextes de raccourcis, les menus qui changent selon le mode et les essais qui
  les suivent, pour une différence qui n'est qu'une disposition. On y va par Affichage > Espaces de travail ou
  Ctrl+K ; le bouton « Storyboard » (ou l'entrée Storyboard du même sous-menu) en revient. Quitter l'animation
  ramène à celui des deux espaces du côté storyboard où l'on était (`espace_storyboard`).
- **Dispositions d'origine** : Storyboard = la disposition standard ; Animation = la bande (la timeline) plus haute
  (340 au lieu de 290) et les calques plus larges (250 au lieu de 230) ; Montage = la bande à 420, ni calques, ni
  effets, ni réglages de l'outil, ni couleur, la fiche du plan, **Bruitages et Suggestions sonores** à droite (on pose
  les sons dans la bande). Références, Historique, Commentaires restent à la demande partout ; la feuille d'exposition
  n'existe qu'en animation. Les valeurs sont modestes à dessein : l'essai de fumée déplace une sélection au pointeur
  dans une fenêtre de 1280 × 720, et une bande à 400 laissait trop peu de dessin.
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
- **Actions** (`keymap.toml`, un bloc chacune, sans touche) : `espace.storyboard`, `espace.animation`,
  `espace.montage`, `espace.retablir`.

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
- **Pas touchées** : la barre des panneaux à droite ; la barre sous le dessin (vue, caméra, plateau 3D,
  **Scène 3D** — le cube) et celle au-dessus (vue du plateau / caméra, zones de sécurité, format ; plus de boutons 3D
  depuis 2026-10-20 : ils n'étaient que des doublons de ceux du bas, seuls les outils 3D y restent) : ce sont des vues, pas des commandes de ces trois barres ; la
  scène 3D reste aussi dans Affichage > Plateau et Ctrl+K. Proposition 4 de la partie 5 (miroir et table lumineuse sous
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
4. Les trois dispositions d'origine sur ton écran (la bande à 340 en animation, à 420 en Montage) ; ajuste si elles
   mangent trop le dessin.
5. Passer de Storyboard à Animation et retour : le dessin ne saute pas, les panneaux ajustés se recalent en une ou
   deux images ; l'espace Montage depuis Ctrl+K ; le bouton « Storyboard » qui en revient.
6. Un panneau détaché (Références) reste sur son écran en changeant d'espace.

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
- **Pendant l'export** : le projet est verrouillé tant que les images se calculent (fenêtre modale, comme l'export du film),
  puis libre pendant que le son finit.
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
