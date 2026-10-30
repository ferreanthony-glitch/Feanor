# Doublons : une commande, un endroit principal (2026-10-21, branche `simpl-barres`)

La demande d'Anthony : « toujours rendre l'interface la plus simple possible ». Plusieurs commandes existaient en deux ou trois endroits (barre du haut, barre
sous le dessin, barres du montage et de la timeline, menus, clic droit). Cette passe a relu les barres une à une.

**La règle.** Chaque commande a UN endroit principal (une icône, ou son menu), plus le clic droit quand il est utile, plus la palette Ctrl+K (toutes les
commandes y sont, avec leur raccourci). Les menus du haut (Fichier, Édition, Storyboard, Animation, Affichage…) sont le **catalogue** : une entrée par commande, avec
son raccourci ; une icône de barre et son entrée de menu ne sont pas comptées comme un doublon (voir la question 1).

Rien n'est perdu : chaque commande retirée d'une barre reste dans la palette, dans son menu, et souvent sous le « Plus… » de la barre qui la porte (là, grisée avec
la raison quand elle ne s'applique pas). L'essai `barres_tests::every_action_moved_off_a_bar_is_still_reachable` et ceux de ce fichier le gardent.

---------------------------------------------------------------------------------------------------

## 1. L'inventaire (commande → avant → après)

| Commande | Avant | Après (endroit principal en gras) |
|---|---|---|
| Transport : première image, précédente, lecture / pause, suivante, dernière, boucle | barre de commandes (sous « Plus… »), barre du montage, barre de la timeline, menu Animation | **barre de la timeline** (le plan) et **barre du montage** (le film, avec « Début du montage ») ; menu Animation ; Ctrl+K. Retiré de la barre de commandes. |
| Son pendant la lecture | barre de commandes (« Plus… »), barre de l'espace Montage, menu Son | **barre de l'espace Montage** ; sous « Plus… » de la barre du montage et de la timeline ; menu Son |
| Nouveau plan, Dupliquer le plan, Supprimer les plans | barre de commandes (« Plus… »), barre du montage, menu Storyboard, clic droit d'un plan | **barre du montage** (« Plans ▸ » pour le reste) ; clic droit d'un plan ; menu Storyboard. Retiré de la barre de commandes. |
| Pelure d'oignon : allumer, dessins avant et après, tous les réglages | barre sous le dessin **et** barre de la timeline (quatre commandes d'office) | **barre sous le dessin** (l'icône allume ; la petite flèche ouvre « Avant / Après / Tous les réglages… ») ; sous « Plus… » de la timeline ; Ctrl+K |
| Panneau Effets | barre des panneaux **et** barre de la timeline | **barre des panneaux** ; Ctrl+K |
| Panneau Bruitages | barre des panneaux **et** barre de l'espace Montage | **barre des panneaux** ; Ctrl+K |
| Animer ce plan / Retour au storyboard | onglets Animation / Storyboard, barre du montage, barre de la timeline, clic droit du dessin et d'un plan, menu Storyboard | **onglets** (haut à droite) ; double-clic sur un plan ; clic droit ; menu. Les deux boutons des bandes sont sous « Plus… ». |
| Vue du plateau / vue caméra | barre au-dessus du dessin **et** barre sous le dessin (même action) | **barre au-dessus du dessin** |
| Espace 3D | barre sous le dessin | **barre au-dessus du dessin**, avec les deux autres vues |
| Dessin ↔ vignettes (`vue.planches`) | barre du montage (deux boutons) **et** barre sous le dessin | **barre du montage** (la planche n'a pas de barre sous le dessin, c'est le seul endroit d'où l'on en revient) |
| Zones de sécurité | bouton de la barre au-dessus du dessin, menu « Cadrage », Affichage > Caméra | **menu « Cadrage »** : une icône, tout à gauche de la barre SOUS le dessin (2026-10-21) ; Affichage > Caméra |
| Format du cadre (16:9, 2,39…) | liste déroulante de la barre au-dessus du dessin **et** menu « Cadrage » | **menu « Cadrage »** de la barre sous le dessin (avec les zones de sécurité et l'aspect des traits) |
| Guides (perspective) et repères de composition (tiers, spirale…) | panneau Guides, menu « Cadrage » (repères) | **la règle**, barre au-dessus du dessin : un seul menu pour les deux (le panneau Guides n'existe plus) |
| Pastilles des personnages | barre de 26 points au-dessus du dessin et de la planche | **colonne ronde au bord droit** du dessin et de la planche, sous le titre du plan (plus de texte « Aucun personnage » : la bulle) |
| Zoom : ajuster, 100 %, plus, moins | quatre icônes sous le dessin, menu Affichage, texte « Zoom 35 % » de la barre d'état | **icône « Ajuster » + loupe** sous le dessin (la loupe ouvre : pourcentage, −, +, 100 %, Ajuster) ; menu Affichage ; « 35 % » en lecture dans la barre d'état |
| Rotation de la vue | deux icônes + curseur sous le dessin, « Rotation 12° » dans la barre d'état | **une icône** (allumée quand la vue est tournée ; elle ouvre l'angle, ± 15°, « Remettre droite ») |
| Miroir de la vue | icône sous le dessin, « Miroir » dans la barre d'état | **icône** (allumée) |
| Table lumineuse | icône + curseur sous le dessin | **icône + petite flèche** (force des autres calques) |
| Titre du plan (« Séquence 1 — plan 1 ») | barre d'état, coin du dessin, barre de la timeline | **coin du dessin** et en-tête de la timeline ; retiré de la barre d'état |
| Espace de travail courant (« Storyboard · … ») | onglet allumé **et** barre d'état | **onglet** |
| Clé de caméra « ici » / « supprimer » | les deux dans la barre de la timeline, l'une toujours grisée | **une seule des deux** (ici s'il n'y a pas de clé à l'image, supprimer s'il y en a une) |
| Légende des clés | — | **icône « ? »** au bout de l'en-tête du montage et de la timeline (nouveau) |

Ce qui reste volontairement à plusieurs endroits :

- **Annuler, Rétablir, Couper, Copier, Coller** : barre de commandes + menu Édition + clic droit du dessin. Le clic droit est « là où l'on travaille » ; voir la question 3.
- **Commentaire, marque d'image, signet** : leur bouton est sous « Plus… » (timeline, montage), leur entrée dans le menu Storyboard et le clic droit de la règle. Trois façons de les poser, une seule d'office dans la barre (barre de l'espace Montage).
- **Exporter le film** : barre de commandes et Fichier > Exporter. Voir la question 2.
- **Les couleurs d'avant-plan et d'arrière-plan** : en bas de la barre d'outils, panneau Couleur, pastilles des personnages (trois façons de prendre une couleur, trois usages).

---------------------------------------------------------------------------------------------------

## 2. Les panneaux de la barre de droite, par espace de travail

La barre ne montre plus les douze icônes partout (le panneau Guides est parti : ses réglages sont dans la règle, 2026-10-21) : seulement les panneaux utiles au travail de l'espace. Les autres restent dans la palette (Ctrl+K) et dans Affichage >
Espaces de travail > Panneaux. Un panneau que tu ouvres à la main a son icône tant qu'il est ouvert, et chaque espace retient sa disposition : il revient avec l'espace.

| Espace | Icônes d'office |
|---|---|
| Vignettes | Réglages, Pinceaux, Couleur, Références (4) |
| Storyboard | Réglages, Pinceaux, Couleur, Personnages, Calques, Effets, Fiche du plan, Références (8) |
| Animation | Réglages, Pinceaux, Couleur, Calques, Effets, Fiche du plan, Feuille d'exposition, Références (8) |
| Montage | Fiche du plan, Commentaires, Historique, Bruitages (4) |

(`barrepanneaux::panneaux_d_office`, essais `barrepanneaux_tests`.)

---------------------------------------------------------------------------------------------------

## 3. « Cacher plutôt que griser »

**Cachés quand ils ne s'appliquent pas** (barres) : « Coller les plans » (rien de copié), « Clé de caméra ici » (une clé est déjà là), « Supprimer la clé de caméra » et
« Adoucir » (pas de clé à cette image), « Plan modifié précédent / suivant » (aucun envoi), « Renuméroter » (numérotation non verrouillée). Sous « Plus… » ils restent **grisés
avec la raison dans la bulle** (« Rien à coller : copie d'abord des plans. »…), pour qu'on sache qu'ils existent. Menu du clic droit de la ligne Caméra : « ici », « adoucir »
et « supprimer » suivent la même règle.

**Gardés grisés** (menus, où voir l'entrée aide à comprendre) : Coller dans les menus des clés et des plans (on voit que coller existe, qu'il n'y a rien à coller),
« Tremblement du trait » sur un calque bitmap (la raison est dans la bulle), « Propager la peinture… » quand le calque ne peut pas l'être (la raison est dans la bulle),
« Effacer les annotations… » sans annotation, « Supprimer les versions non affichées… » sans version à supprimer.

---------------------------------------------------------------------------------------------------

## 4. Les menus du clic droit : une dizaine d'entrées au plus

L'essai `menus_tests::the_right_click_menus_have_about_ten_entries_at_most` peint chaque menu et compte ses lignes (au plus 10 au premier niveau). Le rare est dans un
sous-menu (« Plus », « Couper et réunir », « Version du plan »…), rien n'est retiré.

| Menu | Avant | Après |
|---|---|---|
| Dessin | 8 | 8 (déjà court) |
| Plan, dans le montage | 18 | 9 : animer, nouveau, dupliquer, copier / coller / supprimer, Couper et réunir ▸, Version du plan ▸, Plus ▸ (couleur, transitions, rogner, son) |
| Vignette (clic droit) | 11 | 8 : mêmes gestes ; Version du plan ▸ ; Plus ▸ (couleur, recadrer, réinitialiser le cadrage) |
| Fond de la planche des vignettes | 11 (19 avec des notes choisies) | 7 : Annotations ▸, Notes choisies ▸ |
| Calque (montage, en-tête de la timeline, panneau Calques, « … », menu Calques : le même menu, `menucalque.rs`) | 19, 12, 12 | 11 (§82) : Nouveau calque ▸, Dupliquer, Renommer, Supprimer ; Fusionner vers le bas, Fusionner les choisis ; Organiser ▸, Apparence ▸, Effets ▸, Aplat ; Plus ▸ |
| Images (case de la timeline) | 14 | 10 |
| Ligne de la caméra | 14 | 9 (« ici », « adoucir » et « supprimer » : seulement ce qui s'applique) |
| Règle de la timeline | 11 | 9 |
| Extrait de son | long | « La piste ▸ » porte les réglages de la piste (nom, muette, solo, verrou, volume) |
| Menus « Repères » et « Affichage » (barre de la timeline, clic gauche) | 20 et 11 | 4 et 5 |
| Transition, coupe, clé de caméra, d'effet, de position, ligne du son, dessin | 2 à 10 | inchangés |

---------------------------------------------------------------------------------------------------

## 5. Questions pour Anthony (là où le bon endroit n'est pas évident, rien n'a changé)

1. **Les menus du haut répètent les icônes.** Fichier, Édition, Animation… listent aussi les commandes qui ont une icône (c'est leur rôle de catalogue, avec le raccourci
   écrit). Veux-tu que les menus ne gardent que ce qui n'a PAS d'icône ? (Plus court, mais on y trouverait moins.)
2. **Exporter le film** est dans la barre de commandes ET dans Fichier > Exporter… Garder l'icône du haut ?
3. **Annuler / Rétablir / Couper / Copier / Coller** : barre de commandes, menu Édition, clic droit du dessin. Retirer ceux du clic droit (ne garder que « Ajuster » et
   « Animer ») ?
4. **« Animer ce plan »** n'a plus de bouton dans les bandes (l'onglet Animation et le double-clic le font). Le remettre dans la barre du montage ?
5. **Personnages** est d'office en Storyboard et retiré d'office en Animation (la colonne des pastilles n'y est pas non
   plus). Bien ? (Le panneau **Guides** n'existe plus : la règle les porte.)
6. **Les clés de dessin ne se cachent pas** dans le filtre : ce sont les dessins mêmes des pistes. Un mode « seulement les débuts de dessin » (une fine barre au lieu des
   blocs) t'intéresse-t-il ?
7. **Commentaire, marque d'image, signet** : trois commandes proches, trois mots (« Signet ici », « Marque d'image ici », « Commentaire ici »). Une seule icône « Repères »
   qui ouvre les trois ?
8. **Barre d'état** : j'ai retiré le titre du plan, l'espace de travail, la rotation, le miroir et la position du curseur (x, y). Il reste le format et la cadence,
   le zoom, le panneau qui a la main, l'état d'enregistrement et le message. Un regret ?
