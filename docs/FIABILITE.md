# Fiabilité de SHOT

Passe du 2026-10-19 (branche `fiabilite`, partie de `main` b9aa2f0). Le point de départ : le 2026-10-18, SHOT ne
s'ouvrait plus alors que les 768 essais passaient. Après la fusion des effets en arrière-plan, la vignette d'un
projet rouvert lisait l'aperçu réduit d'un effet comme une feuille entière : index hors des tuiles
(`raster.rs:57`), arrêt au lancement. Corrigé depuis (f3d94a1, `blend_tests::a_smaller_drawing_is_read_to_scale`),
mais aucun essai n'ouvrait un projet qui contienne tout ce que SHOT sait enregistrer. Et, sur une machine chargée
par des compilations, la fenêtre est restée noire plus de 14 s au lancement.

Objectif : ne plus jamais livrer une version qui n'ouvre pas les projets d'Anthony ou ne peint pas.

Le filet de sécurité, en une commande : `.\scripts\verifier.ps1` (voir plus bas).

## 1. Le projet d'essai (`src/fiabilite_fixture.rs`)

Un projet fabriqué par le code (pas un fichier d'Anthony), qui contient :

| Domaine | Contenu |
|---|---|
| Structure | 2 séquences nommées, 5 plans (un nommé), image de départ 1001, damier, papier A et B, propriétés, groupes de couleur renommés |
| Calques | bitmap, vectoriel, **vidéo** (10 images PNG minuscules fabriquées sur disque, début négatif), **CTG**, **dossier** et calque rangé, calque caché et verrouillé, transformation (décalage, échelle, rotation, étirements), profondeur, opacité, mode de fusion (Multiplication, Écran, Différence), pré / post-comportement, pochoir, position verrouillée, pelure d'oignon, noms d'instance, marques de couleur, rôles d'images |
| Dessin | traits vectoriels **texturés avec leur trace** (appui, instants, toucher) en gouache, feutre, fusain ; remplissage à trous ; trait lisse ; texte (son image n'est pas comparée : elle dépend des polices du poste) |
| Fond sous le trait | réglages du calque ; strate peinte et coupe d'un dessin |
| Effets | flou (clés), tremblement du trait (clés), teinte-saturation (clés), lueur (clés), bruit (éteint, puis avec clés dans le plan rogné) |
| Calques d'effets (format 27) | un **ajustement** (opacité, mode Écran, expositions aux images 6 à 8, image d'impact + secousse + lueur diffuse à clés) et une **incrustation** réglée à fond (texte libre à mots remplacés, boîte, « seulement à l'export », police) : `Layer::effet` entre dans l'instantané, donc dans la comparaison de chaque aller-retour |
| Caméra | clés avec adoucis, bascule, panoramique, avancée ; **effet de caméra posé** (explosion) ; repère 3D (un cube) |
| Plans | marques d'image avec textes et couleurs, signets, entrée / sortie, notes dessinées, couleur de plan, textes d'action / dialogue / notes |
| Montage | plan **rogné** au début (6 images) et à la fin (8) avec sa **réserve** (dessins, clés d'effets, caméra, marques, signets, commentaires) |
| Commentaires | 5 (dont 2 rangés dans la réserve), résolus et non, couleurs |
| Son | un WAV fabriqué, deux pistes (une muette, verrouillée, volume 1,25, en solo), deux extraits ; effets du premier (`effets_son.json` : volume, fondus, enchaîné, clés de volume, égaliseur, écho) |
| Transitions | fondu enchaîné centré, volet décalé, fondu depuis le blanc au début du film, fondu au noir à la fin (`transitions.json`) |
| Sons proposés | un bruitage à générer, une voix rejetée et modifiée, une ambiance acceptée (son posé), confirmation d'envoi en ligne (`sons_proposes.json`) |
| Références | une image étiquetée, tournée, en gris, liée à deux plans, et une vidéo de trois images aux réglages de lecture changés (`references.json` et `references/`) |
| Crayonnés | la planche de crayonnés (`crayonnes.json`) : 2 pages de 4 × 3 cases, deux traits lisses dans une case, une case peinte au pinceau et cochée, une légende seule |
| Planche, guides | notes dessinées et texte de la planche (une attachée à un plan), guide de perspective |
| Scène 3D | `scenes3d.json` : toutes les formes, un mannequin à la pose libre, un groupe, un modèle importé, un décor de séquence, la caméra du plateau (aller-retour, fumée, fichier coupé et muté ; protections de `write_sidecar`) |

Manque (dit, pas caché) : les calques « toute la scène » (d'avant les plans, devenus des calques ordinaires à
l'ouverture), le texte vectoriel comparé pixel à pixel (polices du poste). `the_fixture_really_holds_everything_the_project_can_store`
fait échouer la suite si le projet d'essai maigrit, et `the_comparison_notices_a_difference` si la comparaison cesse
de voir un écart.

`instantane(doc)` donne tout ce qui doit survivre, une ligne par chose. Ce qui n'y est pas est non persistant par
définition : numéros de calques (neufs à chaque lecture, remplacés par leur rang), versions de dessins, pixels en cache
des traits (le `Debug` d'un trait le dirait : le vecteur est comparé dans sa forme enregistrée), ordre de la liste des
commentaires (le fichier les range dans l'ordre du film), compteur interne des numéros.

## 2. Aller-retour (`fiabilite_aller_retour_tests.rs`)

Enregistrer → rouvrir → enregistrer ailleurs → rouvrir, par **trois chemins d'enregistrement** : une seule boucle
(`parallele::EN_SERIE`, un crochet d'essai : la machine à un cœur), plusieurs fils, copie de secours (écrite sur un
autre fil). À chaque lecture : aucun avertissement, même projet, et les deux dossiers ont les mêmes fichiers, octet pour
octet. Réenregistrer sur place sans rien changer ne touche à rien. Lectures : directe (`project::load`), rapide
(`load_quick_progress`, celle de l'ouverture sur un autre fil ; l'image des traits texturés est d'abord approchée, elle
n'est pas comparée), et par l'application elle-même (ouverture, attente des rendus exacts, « enregistrer sous »).
Durée : moins d'une seconde pour les quatre.

## 3. Fumée de toute l'application (`fiabilite_fumee_tests.rs`)

`App::sans_fenetre` + `App::frame`, 1280 × 720, sur le projet d'essai : 195 images à travers l'accueil, « Nouveau projet »,
Préférences, « À propos », storyboard et animation avec tous les panneaux (`Pane::ALL`, 16 dont Bruitages, Suggestions sonores et Références), les réglages des 18 outils, les repères de
cadrage, une lecture de 48 images, un défilement aller-retour, **tout sélectionner puis déplacer à la souris par la boîte
de sélection**, un effet de caméra, et l'annulation / le rétablissement de chacun. Chaque image : pas de plantage,
de l'interface produite, et un budget de temps large (15 s d'interface ; au-delà de 500 ms, « ATTENTION » est affiché ;
sur une machine lente — un petit calcul de référence qui dépasse 400 ms, ou `SHOT_FUMEE_SOUPLE=1` — le budget n'est
qu'un avertissement). Deux essais : avec la carte graphique de la machine (composition comme à l'écran), sinon sur le
processeur (moteur de secours, `calques par le processeur`). Un troisième refait l'incident du 2026-10-18 : toutes les
vignettes du projet pendant que les effets calculent, avec et sans aperçu réduit gardé. Durée : ~1 s chacun.

## 4. Entrées abîmées

Règle : un message clair, jamais de plantage, jamais de blocage, **aucun fichier effacé ni modifié** par l'ouverture, ni
par l'enregistrement qui suit pour ce que cette version n'a pas su lire.

| Essai | Ce qu'il vérifie |
|---|---|
| `fiabilite_manifestes_tests` | manifeste vide, coupé, octets quelconques, objet vide, autre format, version plus récente, feuille nulle ou démesurée, aucun plan, absent, BOM |
| `fiabilite_entrees_tests` | version plus récente (par l'application aussi : message dans la barre d'état, projet en cours intact), 120 troncatures du manifeste, 90 manifestes aux valeurs changées au hasard, 80 fichiers annexes changés ou coupés |
| `fiabilite_dessins_tests` | dessin manquant, JSON tronqué, PNG tronqué, pas une image, mauvaise taille, en-tête démesuré : un avertissement chacun, fichiers intacts ; 122 PNG coupés ou aux bits changés ; 180 dessins vectoriels aux valeurs changées ; coordonnée de 3,4e38 |
| `fiabilite_annexes_tests` | `commentaires.json` / `rognages.json` / `references.json` d'une version plus récente ou abîmés ; sons et modèles 3D illisibles ; images de `references/` jamais effacées quand le fichier est illisible |
| `fiabilite_raster_tests` | `Raster::get` hors du dessin ; dessins de 5 tailles par toutes les compositions |

### Défauts trouvés par ces essais, et corrigés (un commit chacun)

1. **`Raster::get` sortait des tuiles** hors du dessin (la cause de l'incident du 2026-10-18, resté possible pour tout
   appelant) : transparent hors du dessin. **`composite_tile_into`** (export, composition processeur, secours) lisait
   `cel.raster.tiles[idx]` avec le numéro de tuile de la feuille : plantage `render.rs:203` avec un dessin plus petit ;
   lu à l'échelle maintenant.
2. **Perte de données** : un `commentaires.json` ou `rognages.json` d'une version plus récente (ou abîmé) s'ouvrait sans
   ses commentaires, et l'enregistrement suivant **effaçait le fichier** ; même chose pour un son ou un modèle 3D
   illisible. Ils restent en place ; un fichier neuf à écrire met l'ancien de côté sous `<fichier>.illisible`.
3. **Manifeste** : message anglais (`stream did not contain valid UTF-8`) ; une feuille de milliards de pixels faisait
   tomber SHOT sans un mot (échec d'allocation) ; refusée au-delà de 16 384 pixels de côté.
4. **Dessin vectoriel abîmé** : une coordonnée de 3,4e38 faisait rejouer le trait sans fin, l'ouverture ne finissait
   jamais (trouvé à l'essai, plus de 60 s) ; le dessin est maintenant « illisible » (avertissement, fichier intact). Un
   PNG dont l'en-tête annonce 70 000 × 70 000 demandait 19 Go de tampon (arrêt brutal) ; refusé avant d'allouer. Durée
   d'un plan et nombre d'images d'un calque bornés à 100 000.

Pas corrigé, noté : un son ou un modèle illisible sort du projet, et le manifeste enregistré ne le cite plus (le fichier
reste dans le dossier, mais ses extraits sont perdus du montage) ; un dessin illisible laisse son image vide, de même.

## 5. Démarrage (`demarrage.rs`, `fiabilite_demarrage_tests.rs`)

La fenêtre d'eframe reste cachée jusqu'à la première image peinte : tout `App::new` et ce que le lancement demande se
paient avant. Minutage : `SHOT_DEMARRAGE=1` (variable d'environnement) ajoute le rapport étape par étape à
`%LOCALAPPDATA%\SHOT\demarrage.log`.

Mesuré (banc `perf_startup_under_load`, 160 pointes + 60 papiers importés, temps jusqu'à la première interface
construite, sans la fenêtre ni la carte graphique d'eframe) :

| Machine | Avant | Après |
|---|---|---|
| au repos | 205 ms | 95 ms |
| un calcul par cœur en parallèle (16) | 738 ms | 192 ms |
| deux par cœur (32) | **3 695 ms** | **324 ms** |

Cause : `BrushLibrary::load` (décodage, réductions et hachage de chaque pointe et papier importé, des centaines) tenait
90 % du temps avant la première image ; sous charge, ses huit fils se disputent les cœurs. Elle se lit maintenant sur un
autre fil (`App::poll_brushes`) et les pinceaux de SHOT sont là tout de suite ; tant qu'elle n'est pas arrivée, rien ne
modifie ni n'enregistre la bibliothèque (`brushes_busy` : sinon la lecture écraserait le changement, ou l'enregistrement
effacerait les importés pas encore arrivés). Deuxième cause possible : `p.exists()` du dernier projet sur le fil de
l'interface (disque réseau éteint, disque externe qui dort) ; la question part sur un autre fil (`reopen_last_project`).

Reste avant la première image : `BrushLibrary::builtin()` (les pinceaux de SHOT : 74 ms au repos, ~250 ms sous charge),
polices, réglages, raccourcis, tri des séances de secours, pipelines de la carte graphique (`GpuCanvas::new`, non mesuré
sans fenêtre) et tout ce qu'eframe fait avant d'appeler `App::new` (fenêtre, choix de la carte, Direct3D / Vulkan). À lire
sur la vraie machine avec `SHOT_DEMARRAGE`. Si la fenêtre noire revient, ce journal dit quelle étape a pris le temps.

## 6. Essais déterministes

Les essais d'un même processus tournent ensemble et partageaient l'état global du cache des effets, de son fil de fond,
de l'horloge de l'aperçu et des réglages. Résultat avant correction : sur 6 passages de la suite, 1 à 7 essais échouaient
à chaque passage (`fxcache::tests::*`, `perf_tests::an_idle_application_does_not_repaint`, et le mien).

- **Réglages et dossiers de données** : en test, `donnees::dossier` donne à chaque fil d'essai son propre dossier neuf
  (jamais `%APPDATA%` ni le dossier d'un autre essai) ; plus besoin de `bac_a_sable` pour être isolé. `Settings::load()`
  d'un essai ne lit plus ce qu'un autre écrit.
- **État commun des effets** (`fxcache::monde`) : `lire()` (pris tout seul par `App::sans_fenetre`) pour qui fait tourner
  l'application ; `ecrire()` pour qui compte sur le cache, le fil de fond, l'aperçu ou l'inactivité (tous les essais
  `fxcache_tests` à état global, les essais du tremblement qui calculent, l'essai d'inactivité et celui de la timeline
  zoomée). Un écrivain attend la fin des lecteurs, démarre sur une page blanche (`forget_all`, aperçu remis à zéro,
  calculs en cours finis) et personne ne démarre pendant ce temps.
- **Attentes** : `fxcache::settle()` attend la condition « calcul fini » et non un sommeil ; l'aperçu d'un quart de seconde
  se teste avec une horloge qu'on avance (`fx::advance_clock`), pas par 330 ms de sommeil ; l'essai d'inactivité attend le
  repos (`se_reposer`) au lieu d'un nombre d'images fixe. Les `sleep` restant dans les essais sont des attentes de fin avec
  délai maximal ou des « rien n'arrive » (qui ne peuvent pas échouer par lenteur).
- Mesure : `.\scripts\verifier.ps1 -Repetitions 12` (voir plus bas) ; le résultat est dans le compte rendu de la passe.

## 7. Le script de vérification

```
.\scripts\verifier.ps1                       # tout, une fois
.\scripts\verifier.ps1 -Repetitions 5        # la suite complète 5 fois : nomme les essais intermittents
.\scripts\verifier.ps1 -SansAvertissements   # saute le contrôle des avertissements de compilation
```

Il lance : `cargo check --release` (aucun avertissement), la suite complète (autant de fois que demandé), puis chaque
famille d'essais de fiabilité par son nom ; il affiche OK / ÉCHEC pour chacun et un résumé, et sort avec le code 0 ou 1.
Il **ne lance pas** `shot.exe` (les lancements visuels sont faits à part) et marche pendant que `shot.exe` tourne
(`cargo check` ne produit pas d'exécutable). PowerShell 5.1.

## 8. À regarder à l'écran

- Lancer SHOT avec `SHOT_DEMARRAGE=1` et lire `%LOCALAPPDATA%\SHOT\demarrage.log` : quelle étape pèse, avec et sans
  compilation en parallèle.
- Au lancement avec beaucoup de pinceaux importés : la liste des pinceaux grossit après quelques secondes ; enregistrer
  un réglage dans « Mes brosses » juste au lancement dit « se chargent encore ».
- Le dernier projet sur un disque éteint : SHOT s'ouvre sur l'accueil, sans attente, sans message.

## 9. Envois, versions de plans, comparaison (2026-10-20)

Trois annexes de plus, écrites par le même `SaveJob` que les autres (voir `docs/ARCHITECTURE.md`) ; le format du projet
reste 27, un projet sans envoi ni version n'a ni leurs fichiers ni leur travail.

| Annexe | Protection | Essai |
|---|---|---|
| `envois.json` | illisible, d'une version plus récente, octets quelconques : le projet s'ouvre sans envois (avertissement), l'enregistrement n'y touche pas ; marquer un envoi met l'ancien de côté en `envois.json.illisible` ; un nom de fichier qui sort de `versions/` est oublié, un numéro en double renuméroté | `a_damaged_envois_file_is_never_deleted`, `a_newer_envois_file_is_never_deleted`, `a_hand_damaged_file_is_cleaned_not_trusted` |
| `versions/` | jamais effacé, même quand un envoi est retiré de la liste ; un fichier déjà là n'est pas réécrit ; un instantané est écrit **avant** la liste qui le cite ; « Enregistrer sous » le recopie ; un instantané ou un dessin manquant ou abîmé est dit en clair (la comparaison montre l'autre côté) | `an_envoi_is_written_once_and_unchanged_drawings_are_not_written_again`, `removing_an_envoi_from_the_list_never_deletes_its_files`, `save_as_takes_the_snapshots_along`, `a_missing_snapshot_says_so_in_plain_words` |
| `plans_versions.json` | illisible ou plus récent : le projet s'ouvre sans versions, **aucun dessin de `dessins/` n'est effacé** tant que le fichier est là, et créer une version met l'ancien de côté en `.illisible` ; sans autre version, le fichier et les dessins de la version supprimée partent | `a_damaged_versions_file_is_never_deleted`, `a_newer_versions_file_is_never_deleted`, `deleting_the_last_other_version_removes_the_file_and_its_drawings` |

- **Un projet rouvert n'a pas changé** : les empreintes d'un projet complet (`fiabilite_fixture`) sont les mêmes avant et
  après enregistrement et réouverture (lecture complète et rapide), et un instantané se relit avec l'empreinte qu'il avait
  (`the_digests_of_a_complete_project_survive_save_and_reopen`,
  `the_complete_project_snapshot_reads_back_with_every_plan_intact`, `a_reopened_project_shows_no_modified_plan_until_something_changes`).
  C'est pour cela que l'empreinte d'un dessin est celle de ce que le PNG garde (`fingerprints_survive_the_png_round_trip`).
- **La comparaison ne change rien** : plans, versions des dessins, numéro d'historique, drapeau « modifié » identiques
  avant et après (`the_window_shows_the_images_and_changes_nothing_in_the_project`).
- **Au repos, rien ne repeint** : avec ou sans envoi, avec la fenêtre « Comparer » ouverte
  (`once_the_plans_are_followed_the_application_goes_back_to_rest`, `the_comparison_window_does_not_keep_the_application_awake`).

À regarder à l'écran : voir `docs/INTERFACE.md`, partie 15.

## 10. Onglets de projets (2026-10-20, branche `onglets`)

Plusieurs projets ouverts dans la même fenêtre : le risque nouveau est qu'un projet voie, ou écrase, ce qui est à un autre. Les
essais de `onglets_tests.rs` (`docs/ARCHITECTURE.md`, « Onglets de projets ») :

| Risque | Garde | Essai |
|---|---|---|
| Un champ de projet oublié dans l'échange fuit d'un onglet à l'autre | l'essai lit `app.rs` : chaque champ d'`App` est de projet (`etat_projet!`) ou partagé (`PARTAGES`), jamais les deux, jamais aucun | `every_field_of_the_app_is_either_a_project_field_or_shared` |
| Une annulation touche un autre projet | chaque onglet a son `History` | `undo_is_isolated_per_tab` |
| Un enregistrement écrit le mauvais projet, ou donne son chemin à un autre | `Enregistrement` rangé avec son onglet, époque unique | `saving_goes_to_the_right_project`, `a_save_started_in_a_tab_is_adopted_by_that_tab_after_switching`, `epochs_are_unique_so_a_job_only_answers_its_own_tab` |
| Une fermeture ou un « Quitter » perd un projet | boîte par onglet, boîte unique pour plusieurs, attente des écritures | `closing_*`, `quitting_with_several_modified_tabs_asks_once_and_lists_them`, `quitting_waits_for_a_save_in_a_hidden_tab`, `a_close_waiting_for_a_save_follows_it_…` |
| Une vignette, un effet calculé ou une texture montre les pixels d'un autre projet (mêmes numéros de dessin) | caches rangés par onglet, demandes de fond périmées à la sortie | `thumbnails_never_show_another_projects_pixels`, `thumbnails_asked_before_a_switch_never_answer_the_new_tab`, `derived_effect_drawings_never_cross_from_one_project_to_the_other`, `gpu_textures_follow_their_tab_and_are_not_sent_again` |
| Un onglet caché travaille pour rien | rien n'est en file, lecture arrêtée ; trois onglets au repos ne repeignent pas | `hidden_tabs_run_nothing_and_hold_no_pending_work`, `three_open_tabs_at_rest_ask_for_no_repaint` |
| Un collage entre onglets abîme un projet (feuille d'une autre taille) | la copie est adaptée à la feuille qui reçoit (`adaptation.rs`) ; cadence signalée ; voir la section 12 | `plans_from_a_sheet_of_another_size_are_adapted_…`, `a_copied_layer_pastes_into_…`, `copied_images_paste_into_…` |
| Un plantage perd un onglet | une copie de secours par onglet ; chacune proposée au démarrage | `each_tab_has_its_own_rescue_copy_and_a_crash_offers_every_project`, `closing_a_tab_removes_its_rescue_copy` |
| La session écrasée pendant qu'elle revient | `retenir_la_session` attend `session_prete` | `the_session_is_not_overwritten_while_it_comes_back`, `the_session_is_remembered_and_comes_back_in_order` |

## 11. Relecture par le réalisateur (2026-10-20)

Une entrée venue d'un inconnu, donc traitée comme telle (`relecture.rs`) : le fichier de notes que le relecteur renvoie.

| Protection | Essai |
|---|---|
| Fichier de plus de 40 Mo refusé sans être lu ; lecture bornée même si sa taille annoncée ment | `a_too_big_file_is_refused_before_it_is_read` |
| Pas du JSON, pas notre format, version plus récente, pas de l'UTF-8 : un message en français, rien n'est posé | `a_file_that_is_not_a_notes_file_says_so` |
| Textes nettoyés (contrôles, inversions de sens d'écriture) et coupés, identifiants restreints à `A-Za-z0-9-_.`, doublons ignorés, 3000 notes au plus ; **aucun nom de fichier du fichier n'est utilisé** | `a_notes_file_is_untrusted_texts_ids_and_counts_are_bounded`, `at_most_a_few_thousand_notes_are_read`, `nothing_touches_the_disk_while_importing_notes` |
| Dessins : PNG seulement (pas d'adresse, pas d'autre format), 1,6 Mo de base64, 4096 points par côté, 8 millions de pixels, mémoire du décodeur bornée, réduits à 1280 de large et réécrits (les octets d'origine ne sont pas gardés) ; un dessin refusé ne fait pas perdre sa note | `a_drawing_is_decoded_under_limits_and_rewritten_clean`, `a_bad_drawing_is_refused_without_stopping_the_import` |
| `commentaires.json` : le dessin est relu avec les mêmes bornes ; un dessin abîmé est laissé de côté, le commentaire est lu ; un fichier d'avant (sans les champs `note`, `dessin`) se relit et se réécrit à l'identique | `the_note_key_and_the_drawing_survive_saving_and_reopening_the_project`, `old_comment_files_without_the_new_fields_still_load_and_write_the_same` |
| Le projet d'essai de fiabilité porte une note de relecture avec son dessin : tous les allers-retours la comparent | `the_fixture_really_holds_everything_the_project_can_store` |

La page exportée est une sortie : aucune adresse du dehors, un seul script (le même pour tous les projets, sans texte du projet),
politique de contenu `default-src 'none'`, tout texte du projet échappé (`the_page_works_offline_and_forbids_anything_else`,
`hostile_titles_and_texts_cannot_break_out_of_the_page`, `the_script_is_the_same_for_every_project_and_calls_nothing_outside`).
L'export écrit `<nom>.html.part` puis renomme : un arrêt ou une erreur ne laisse pas de demi-fichier et ne détruit pas une page
exportée avant (`stopping_the_export_leaves_no_file_and_keeps_an_older_page`). Le format du projet ne change pas (`VERSION` 32).

## 12. Coller depuis un projet d'une autre taille (2026-10-20, branche `coller-adapter`)

Une copie qui vient d'une autre feuille est adaptée (`docs/ARCHITECTURE.md`, « Coller depuis un projet d'une autre taille »). Ce qui
pourrait mal tourner, et ce qui le garde :

| Risque | Garde | Essai |
|---|---|---|
| Un collage d'une feuille de même taille change, ou coûte un calcul de fond | identité de la règle : la copie est collée telle quelle, sans question, sans fil de fond, une seule étape d'annulation sans « adapté » | `a_project_of_the_same_size_pastes_exactly_as_before`, `the_same_frame_and_sheet_change_nothing_and_a_bigger_sheet_only_shifts` |
| Un plan 4K collé en 1080 (ou l'inverse) n'a pas la bonne taille : pixels, traits, caméra, poses, effets | une similitude unique pour tout, vérifiée dans les deux sens | `a_4k_plan_pasted_into_a_1080_project_is_scaled_and_scaled_back`, `a_bitmap_is_resampled_with_its_area_and_its_colour_kept`, `upscaling_keeps_flat_colours_flat_and_edges_soft`, `vector_strokes_scale_…`, `camera_keys_follow_the_rule_…`, `layer_poses_position_keys_and_pixel_effect_parameters_follow_the_rule` |
| Ce qui est hors de la feuille (toile infinie) se perd | les tuiles hors-feuille sont lues et écrites comme les autres | `what_is_drawn_off_the_sheet_follows_the_rule_too` |
| Un effet neuf en pixels oublié par la règle | chaque effet est passé en revue par son unité (`px`) | `every_pixel_parameter_of_every_effect_is_recognised_by_its_unit` |
| Proportions ou cadence différentes : un collage surprenant | la fenêtre, annulable ; les choix retenus ; « contenir » et « remplir » | `different_proportions_ask_…`, `contain_keeps_the_whole_source_frame_visible`, `a_different_rate_asks_…`, `contain_and_fill_differ_only_when_the_proportions_differ` |
| Le rééchantillonnage fige l'interface | fil de fond, lancement < 60 ms et image d'écran < 400 ms bornés ; pas de changement d'onglet pendant le calcul | `a_big_paste_never_blocks_the_interface_thread` |
| Le presse-papiers perd sa copie d'origine après un collage adapté | les fonctions de collage clonent ; la copie d'origine sert encore | `the_original_stays_in_the_clipboard_and_pastes_at_its_own_size_elsewhere` |
| Un collage adapté s'annule en plusieurs étapes | une étape, nommée ; annuler / rétablir d'un coup | `a_4k_plan_pasted_into_…` (annuler, rétablir), `a_layer_pasted_from_a_smaller_project_is_scaled` |
| Une cadence recalculée perd des clés ou des marques | une règle d'arrondi commune avec « Changer la cadence » ; marques après la première image, une par image | `keeping_the_duration_in_seconds_rescales_every_frame_count`, `space_and_time_adapt_together` |

## 13. Travailler à plusieurs (2026-10-20, branche `cooperatif`)

Le risque nouveau : deux personnes écrivent le même film par un dossier qui n'est pas un disque (Dropbox, Drive, NAS). Règle :
**rien ne se perd, rien n'est supposé présent, aucun fichier d'un autre poste n'est jamais effacé.** Les essais sont dans
`cooperation_tests.rs` (deux postes, dossiers synchronisés imités par `Synchro`), `cooperation_ui_tests.rs`,
`cooperation_modele_tests.rs`, `cooperation_mesures_tests.rs`.

| Risque | Garde | Essai |
|---|---|---|
| Un projet partagé n'est pas le même projet que l'original | le projet complet d'essai (`fiabilite_fixture`) partagé puis rouvert donne le même instantané ; l'original n'est pas touché | `a_complete_project_shared_and_reopened_is_the_same_project`, `sharing_leaves_the_original_untouched_…` |
| Un SHOT sans partage ouvre le film sans plans | structure au format 34 : refusée comme « version plus récente » | `the_shared_manifest_is_format_34_…` |
| Un fichier de plan manquant, abîmé ou plus récent fait écraser le plan | coquille « en attente » : ni prise, ni écriture, fichier intact | `a_plan_file_that_is_missing_or_unreadable_…`, `…from_a_newer_version…` |
| Deux numéros de dessin identiques | préfixe par équipier, ≤ 2⁵² | `numbers_of_two_members_never_meet` |
| Deux personnes dans le même plan | une prise : la plus ancienne prise vivante, même résultat sur tous les postes | `a_plan_is_taken_by_one_person_at_a_time`, `the_oldest_live_prise_wins_…` |
| La course (deux prises avant de se voir) | le perdant est prévenu, son plan se ferme, son travail va dans un brouillon (dessins sous des numéros neufs) | `two_people_taking_the_same_plan_before_seeing_each_other_…` |
| Une prise provisoire écrit par-dessus le gagnant | elle n'est pas écrite avant 8 s de confirmation (sauf en quittant) | `a_prise_still_being_confirmed_is_not_written` |
| Un poste disparu bloque le plan | bail 10 min, reprise avec confirmation, la prise reprise reste écartée | `a_lease_that_runs_out_…`, `a_holder_that_keeps_its_lease_alive_…`, `a_fiche_seen_for_the_first_time_…` |
| Un plan est pris avant d'être à jour | la prise attend la révision que le porteur d'avant a rendue ; « prendre tel quel » après 60 s | `taking_a_plan_waits_for_the_file_…` |
| Le plan arrive avant ses dessins / avant la structure / la structure avant le plan | empreintes des dessins ; plan gardé pour la structure ; coquille jusqu'à l'arrivée | `a_plan_file_that_arrives_before_its_drawings_…`, `…before_the_structure…`, `the_structure_that_arrives_before_a_new_plan_…` |
| On enregistre ce qu'on ne tient pas | l'enregistrement n'écrit que les plans tenus qui ont changé ; une retouche « sans prise » est mise de côté avant d'être remplacée | `saving_writes_only_the_plans_one_holds_…`, `a_change_in_a_plan_one_does_not_hold_…` |
| Un pixel retouché ne change pas le fichier du plan | l'empreinte du dessin fait partie du contenu | `what_another_saved_arrives_at_the_next_refresh_…` |
| Annuler ressuscite un vieux plan d'un autre | `rafraichir_etranger` après annuler / rétablir | `undo_never_brings_back_an_old_copy_…` |
| Un plan supprimé alors qu'on y dessine | brouillon `.supprime.json` avant qu'il disparaisse | `a_plan_deleted_by_the_montage_holder_…` |
| Le jeton perdu | montage mis de côté, celui du gagnant relu | `the_loser_of_the_token_race_…` |
| Commentaires de deux personnes | un fichier par auteur, le plus récent gagne, indépendant de l'ordre ; résoudre / supprimer le commentaire d'un autre marche | `comments_of_two_people_merge_…`, `comments_merge_the_same_way_…` |
| Copies en conflit du client de synchronisation | repérées (noms de Dropbox, Drive, OneDrive), importables comme version, rangées et jamais effacées | `file_names_made_by_sync_clients_…`, `the_scan_reports_only_what_changed_…`, `a_conflict_copy_of_a_plan_the_holder_has_…`, `two_copies_of_the_same_plan_made_in_two_folders_…` |
| Réseau coupé | message dans la barre d'état, travail gardé en mémoire ; au retour, la prise est gardée ou le travail est en brouillon | `working_offline_…`, `work_done_offline_on_a_plan_that_was_taken_over_…` |
| Un fil de fond qui réveille l'écran pour rien | aucune repeinture quand rien ne change | `an_idle_application_with_a_shared_project_open_…`, `the_thread_never_asks_for_a_repaint_…` |
| L'interface lit le dossier partagé | non : tout est sur le fil de la session, le fil d'ouverture ou celui d'écriture ; l'interface lit un instantané | mesures (`docs/PERFORMANCE.md`, 18) |
| Fermer en tenant des plans | la fermeture rend les prises et écrit la fiche finale ; un plantage laisse le bail expirer | `closing_the_project_gives_back_what_one_held`, `a_session_that_died_…` |
| « Enregistrer sous » d'un film partagé | copie à plat au format 33 ; le projet quitte le partage | `save_as_makes_a_flat_classic_copy_…` |
| Un champ de projet oublié dans l'échange d'onglet | `partage` et `equipe_ui` sont dans `etat_projet!` ; l'essai de garde des onglets passe | `every_field_of_the_app_is_either_a_project_field_or_shared` |

Le format du projet classique ne change pas (`VERSION` 33) ; la suite complète et `scripts\verifier.ps1` (nouvelle famille « travail à
plusieurs ») passent.