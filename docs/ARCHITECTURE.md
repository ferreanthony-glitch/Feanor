# Architecture de SHOT (anciennement Pelure)

Document tenu à jour à chaque étape. Dernière mise à jour : phase 9 (octobre 2026).

> Le logiciel s'appelle SHOT depuis le 2026-10-18. Ce document garde l'ancien nom dans son
> historique : lire `%APPDATA%\Pelure` et `%LOCALAPPDATA%\Pelure` comme `...\SHOT` (voir
> `src/donnees.rs`, qui reprend l'ancien dossier une fois). La caisse reste `pelure`, l'exécutable
> est `shot.exe`, le logo se trace dans `src/logo.rs`. Voir `docs/ROADMAP.md` (« Pelure devient SHOT »).

## Vue d'ensemble

Application de bureau en Rust. Une seule fenêtre, interface **egui 0.31** (via eframe). Affichage
par la carte graphique avec **wgpu** (Vulkan ou Direct3D 12) depuis la phase 2a ; si wgpu ne démarre
pas, Pelure repart en OpenGL (forçable avec `PELURE_AFFICHAGE=opengl`). Le moteur utilisé est
indiqué dans le menu Aide.

```
Document ─ séquences ─ scènes ─ panneaux ─ calques (bitmap ou vectoriel) ─ une case par image → dessin (Cel)
                                                                                  │
Cel = image en tuiles 64×64 (+ éléments vectoriels si calque vectoriel, rendus dans l'image)
```

- **Modèle** (`document.rs`) : en mémoire, les panneaux sont une **liste à plat dans l'ordre du film** ;
  chaque panneau désigne sa scène, chaque scène sa séquence. Les panneaux d'une même scène se suivent,
  comme les scènes d'une même séquence : `normalize()` rétablit cette règle après chaque opération et
  supprime les scènes ou séquences vides. Ce choix garde simples la bande de montage, la lecture et
  l'annulation ; le fichier, lui, est rangé en arbre (voir plus bas). Les numéros de scène (1, 2, 3…
  dans l'ordre du film) et de panneau (dans la scène) sont calculés, sauf nom ou numéro choisi.
- **Opérations de montage** (`Document::add_panel_after`, `add_scene_after`, `add_sequence_after`,
  `move_panel`, `remove_scene`…) : dans le modèle, testées sans interface. Un panneau déplacé garde sa
  scène s'il reste à côté d'un panneau de sa scène ; sinon il rejoint celle du panneau devant lequel
  on le pose. Comportement de Storyboard Pro non vérifié : c'est une interprétation.
  Couper un panneau (`split_panel`) duplique les dessins de la seconde partie (chaque panneau
  possède ses dessins) ; le réunir (`merge_with_next`) met les calques bout à bout par rang et par
  sorte. Fusionner deux calques (`merge_layer_down`) crée de **nouveaux** dessins : les anciens
  restent intacts, et l'annulation (copie de la structure) y revient sans rien recalculer.
- **Verrouillage** : `Layer::locked`, enregistré dans le manifeste (`"verrouille"`, absent si faux).
  `App::layer_editable` refuse alors tout ce qui touche aux dessins ; la timeline reste modifiable.
- **Panneau** : durée, textes (action, dialogue, notes) et calques. Un calque associe à chaque image
  un dessin ; plusieurs images consécutives sur le même dessin forment une exposition.
- **Dessins** : bitmap = tuiles RGBA prémultipliées allouées à la demande (`raster.rs`) ;
  vectoriel = traits à épaisseur variable et zones remplies (`vector.rs`), re-rendus en pixels à chaque
  modification. Toute la composition ne voit donc que des pixels. Pour que le coût ne grandisse
  pas avec le nombre de traits : un nouveau trait est posé par-dessus l'image d'avant son aperçu
  (`Cel::render_on_top`) ; la gomme et le déplacement d'une sélection ne refont que leur zone
  (`Cel::rerender_region`, éléments rendus coupés aux tuiles touchées). Les deux sont testés
  identiques, pixel pour pixel, à un rendu complet.
- **Rendu à l'écran** (`gpu.rs`, depuis 2a-2) : par la carte graphique, en deux temps.
  1. *Composer* : papier puis calques (opacité, teinte de pelure, table lumineuse) empilés dans
     une image à la taille du projet, pixel pour pixel. C'est là que viendront la caméra et le
     multiplan.
  2. *Afficher* : cette image posée à l'écran avec la vue (zoom, rotation, miroir), lissée comme
     egui lisse ses images. Affichage sans attente de la synchro de l'écran
     (`PresentMode::AutoNoVsync`, une image d'avance au plus) : chaque mouvement du stylet est
     affiché aussitôt ; en fenêtre, Windows compose l'image à la synchro (pas de déchirement).
     `App::update` limite le rythme : 500 images/s quand il y a des entrées, 125 sinon (lecture,
     export), rien au repos. Pendant un trait, `paint_stroke_tip` relie à l'écran le dernier
     point posé à la pointe du stylet (le lissage fait sinon traîner le trait) ; le curseur garde
     la croix du système, déplacée par Windows sans attendre l'image. Superposer chaque calque directement à l'écran donnait des bords de
     trait plus foncés en vue réduite.

  Chaque dessin affiché est une texture à la taille de la feuille. Quand sa version change, on
  n'envoie que ses tuiles dessinées, et on efface celles qui l'étaient (`CelTexture::filled`) ;
  pendant un trait, seulement les tuiles touchées (signal `Renderer::invalidate_tiles`). Toutes
  les tuiles d'une image d'écran partent dans un seul tampon et une seule commande de copie
  (`Uploads`) : `write_texture` tuile par tuile créait un tampon par appel, trop lent par
  milliers. Au-delà d'1 Go de textures, les plus anciennes sont libérées.
- **Lecture fluide** (`playback.rs`, 3a) : à chaque image d'écran, les dessins des images qui
  viennent (la seconde suivante en lecture, en suivant le montage et la boucle ; les images et
  panneaux voisins à l'arrêt) sont envoyés d'avance, 4 Mo au plus par image, un dessin entamé
  reprenant à sa prochaine tuile (`CelTexture::partial`). Leur nombre est borné pour ne pas
  chasser les textures affichées. `PlayMeter` mesure la cadence réellement affichée (barre
  d'état). Mesure de référence (montage lourd) : changements de panneau de 45–75 ms → moins de
  5 ms.
- **Rendu sur processeur** (`render.rs`) : export, vignettes (`thumbs.rs`), pipette, pot de
  peinture, et affichage de secours (OpenGL, ou `PELURE_COMPOSITION=processeur`). Même calcul,
  même résultat : c'est la référence de comparaison. `camera_image` (export) ne compose que la
  zone filmée, en tuiles entières avec 2 px de marge pour l'interpolation, répartie sur les
  cœurs (`par_chunks`, fils `std::thread::scope`) ; testée identique, pixel pour pixel, à
  l'ancien calcul (feuille entière, un fil).
- **Exports** (`export.rs`, `exportui.rs`, 3b) : les images sont calculées sur le fil de
  l'interface, 40 ms par image d'écran, derrière une fenêtre modale (le document ne peut pas
  changer : pas besoin de le copier, ce qui doublerait la mémoire). Une image tenue (même plan,
  même caméra) n'est pas recalculée. Elles partent par une file bornée (8 images) vers des fils
  d'écriture PNG, ou vers un fil qui les verse dans l'entrée de ffmpeg (images RGBA brutes). Le
  son est mixé d'avance (`MixState`, hors carte son) dans un WAV temporaire que ffmpeg lit à
  côté. Couleurs converties et étiquetées BT.709 ; tailles impaires rognées d'un pixel (les codecs
  veulent des tailles paires). Le PDF (3c) passe par la même file : un fil réduit chaque image
  (800 px, Lanczos) en JPEG, puis `pdf.rs` met en page et écrit le fichier à la fin.
- **PDF** (`pdf.rs`) : écrit à la main (en-tête, objets, table des positions), PDF 1.4. Polices
  standard Helvetica / Helvetica-Bold en WinAnsi : rien à embarquer, lisibles partout, accents
  français compris ; largeurs des caractères d'après leurs métriques publiées, pour couper les
  lignes. Coût : pas de police au choix ni d'écritures non latines — il faudrait alors embarquer
  une police TrueType (bibliothèque à ajouter, réversible : seul `pdf.rs` change). ffmpeg n'est pas fourni : cherché sur le poste, ou choisi
  (`settings.rs`, `%APPDATA%\Pelure\reglages.toml`), lancé sans fenêtre de console. Annuler ou
  quitter pendant un export arrête ffmpeg et efface la vidéo inachevée.
- **Annuler / rétablir** (`history.rs`) : chaque modification enregistre l'état d'avant, **avant**
  d'avoir lieu : tuiles d'un dessin bitmap, dessin vectoriel, ou « structure ». La structure, c'est
  tout le document hors dessins et sons : séquences, scènes, panneaux et leurs textes, calques
  (expositions, nom, opacité, visibilité), pistes son et extraits, guides, papier, cadence.
  - Les champs et curseurs modifient une **copie** de la valeur ; s'ils changent, `App::edited`
    enregistre l'étape puis on reporte la valeur. Un **geste** = une étape : taper tout un texte
    tant que le champ reste actif, tirer un curseur, glisser un point de fuite, choisir une couleur
    tant que la fenêtre est ouverte (`history::Gesture`). Si une autre étape s'intercale (bouton,
    annulation), la suite du geste en ouvre une nouvelle.
  - Les clics qui changent un réglage (œil d'un calque, M d'une piste) font une étape chacun.
  - Ne s'annulent pas : les réglages de l'application (outil, couleur, zoom, pelure, table
    lumineuse, affichage des guides), qui ne sont pas dans le projet.
  - Coût : une étape de structure copie toute la structure (pas les dessins). Négligeable pour un
    storyboard ; à revoir (copies partagées) si un projet atteint des milliers de panneaux.
- **Son** : voir « Son » plus bas.
- **Fichier** : voir « Format de projet ».
- **Raccourcis** (`keymap.rs`, `keymap.toml`) : voir plus bas.
- **Interface** : `app.rs` (état, actions, `run_action`), `canvas.rs` (dessin à la souris/stylet),
  `panels.rs` (menus, outils, calques, dialogues), `timeline.rs` (animation), `storyboard.rs` (montage,
  vignettes, fiche du panneau), `soundtrack.rs` (pistes son), `vtools.rs` (outils vectoriels).
- **Vues du storyboard** : « Dessin » (le panneau courant, au centre), « Vignettes » (tous les
  panneaux rangés par scène) et la bande de montage en bas, qui sert de timeline (règle, barres de
  séquences et de scènes, panneaux, pistes son). Les noms « Dessin / Vignettes » évitent la confusion
  avec la « scène » du découpage.

## Copie de secours (4a)

- `project.rs` : l'enregistrement est en deux temps. `prepare_save` (fil de l'interface) copie
  ce qui a changé — dessins bitmap clonés (tuiles), vectoriels déjà convertis, ou à recopier du
  dossier du projet s'ils n'ont pas changé depuis (`Reuse`) — et le manifeste. `SaveJob::run`
  écrit (encodage PNG : la partie lente), manifeste en dernier, puis efface l'inutile. Un échec
  vide la mémoire des dessins écrits : le suivant réécrit tout.
- `recovery.rs` : un dossier par séance (`%LOCALAPPDATA%\Pelure\secours\<heure>-<processus>`),
  avec un `verrou` ouvert sans partage (Windows refuse de l'effacer tant que la séance vit),
  `seance.json` et la copie `projet\secours.pelure`. Copie lancée quand le délai est passé, que
  le projet est modifié et que l'historique a bougé depuis la précédente, jamais pendant un
  geste ; écrite sur un fil. Au démarrage, les séances dont le verrou s'efface sont
  interrompues : avec copie, proposées (`recoveryui.rs`) ; sans, effacées. `on_exit` efface le
  dossier de la séance.

## Espaces de travail (4b)

- `workspace.rs` : une disposition (`Layout`) = panneaux affichés, largeur de la fiche, hauteurs
  du montage et de la timeline, vue vignettes, vue de dessus. Dispositions de Pelure, espaces
  enregistrés et dernière disposition dans `reglages.toml` (`[disposition]`, `[[espaces]]`).
- `workspaceui.rs` : les panneaux egui gardent leur taille d'une image à l'autre ; pour imposer
  celle d'une disposition, leur identifiant change (`layout_gen`) et ils repartent de la taille
  demandée. Les tailles tirées à la main sont relues chaque image et retenues quand la souris
  est relâchée.

## Fenêtres modulables (5b)

- Affichage et gestes par **egui_dock 0.16** (onglets, boutons d'aimantation, panneaux
  flottants dans la fenêtre). `dock.rs` : les neuf panneaux (`Pane`) et la disposition dans un
  **format propre à Pelure** (`DockLayout` : arbre `onglets` / `cote_a_cote` / `dessus_dessous`
  avec la part de chaque côté, plus les panneaux flottants et leur place), converti vers l'état
  d'egui_dock (`to_state`) et retour (`from_state`). Les espaces de travail ne dépendent donc pas
  de la version de la bibliothèque, et tout cela se teste sans interface (`dock_tests.rs`).
- Réparation à la lecture (`sanitized`) : chaque panneau une fois, le dessin toujours dans la
  fenêtre principale, parts bornées, groupes vides retirés. Rouvrir un panneau : avec les
  panneaux de son groupe (gauche : outils, couleur, réglages, vue, guides ; droite : fiche,
  calques ; bas : bande), sinon dans une colonne recréée à côté du dessin.
- `dockui.rs` : le contenu de chaque onglet (les fonctions de panneau existantes), les menus.
  egui_dock 0.16 ne retient pas la place des panneaux flottants : on la lit dans la mémoire
  d'egui (`dock::window_id`), sans le cadre (sinon la taille grandirait à chaque ouverture).
- `ajuste.rs` (panneaux qui s'ajustent) : `Panes::ui` note la hauteur du contenu des panneaux
  mesurés (là où le curseur s'arrête : egui_dock étend la zone à tout l'onglet avant de la
  donner) ; après `DockArea::show`, chaque nœud `Vertical` dont une partie s'ajuste reçoit une
  nouvelle `fraction` (calcul pur `top_height`, testé), par petits pas. Une paire (premier
  panneau du haut, premier du bas) dont la part n'est plus celle qu'on a posée a été tirée à la
  main : elle rejoint `panneaux_tires_a_la_main`. `remember_layout` attend que le bord soit
  arrêté (`fit_busy`) ; `set_dock_layout` oublie les parts posées.
- Historique nommé (`history.rs`, `historyui.rs`) : `History` garde un nom par étape à côté de
  `undo` / `redo` ; le nom vient de `History::context`, que `run_action` (nom de la commande) et
  `pane_ui` (outil sur le dessin, sinon nom du panneau) posent le temps de leur travail, puis
  rendent. Les ~60 endroits qui appellent `push` n'en savent rien.
- Fond sous le trait (`fond.rs`, `fondui.rs`) : réglages par calque (`Layer::fond`), strate Fond
  par dessin (`Cel::fond` : `Strate { peinture, coupe }`). Le rendu ne suit pas en direct un
  dessin dérivé (effets, CTG) pendant un trait : le fond est donc un élément à part, juste sous
  le trait (`render::fond_item`, `FondPart::Forme`, dérivé et gardé dans le cache de `fx`,
  recalculé au lâcher). Pendant qu'on peint ou gomme dans le fond, `fx::fond_patch` refait ses
  seules tuiles touchées (forme gardée dans `COVERS`, même version), que `gpu::sync` envoie
  tuile par tuile. La peinture est gardée dans la forme par un pochoir au moment de peindre, et
  de nouveau à la composition ; la coupe suit le geste dans un second `StrokeState`.
- `workspace::Layout` garde ses anciens champs (panneaux affichés, tailles en pixels) : ils
  décrivent les dispositions de Pelure et sont convertis (`DockLayout::from_legacy`) quand
  `panneaux` manque (espaces enregistrés avant 5b).
- L'accessibilité d'egui (accesskit) est coupée : elle plantait quand un panneau devenait
  flottant (`Option::unwrap` dans accesskit_consumer 0.26). À réactiver quand ce sera corrigé.
- **Fenêtre à part** (5g) : `DockLayout::detachees` ; chaque panneau détaché est affiché par
  `ctx.show_viewport_immediate` (une vraie fenêtre de Windows, même contenu que l'onglet, via
  `App::pane_ui`). Le clavier de ces fenêtres ne passe pas par les raccourcis.

## Outils de la phase 5

- **Formes** (`shapes.rs` géométrie testée, `shapetools.rs` gestes) : `ShapeEdit` pour les formes
  en plusieurs gestes (courbe à courber, polyligne) ; Entrée, Échap, Retour arrière leur sont
  donnés avant les raccourcis (`App::tool_key`). Courbe : Bézier cubique ; une courbure fait passer
  la courbe par le pointeur à mi-chemin, la seconde aux deux tiers.
- **Texte** (`text.rs`, `texttools.rs`) : polices de Windows lues par ttf-parser ; seule la table
  des noms est lue au démarrage (sur un autre fil). Élément vectoriel `Text` (texte, police,
  taille, couleur, placement affine), dessiné par les contours des lettres (cache des mises en
  page) en règle non nulle, lettre par lettre.
- **Tampons** : retirés le 2026-10-17 à la demande d'Anthony (tampon et tampon de duplication ;
  le code est dans l'historique git, `stamp.rs` / `stamptools.rs` avant ce retrait).
- **Perspective** (`persp.rs`, `perstools.rs`) : homographie rectangle → quadrilatère ; l'image
  est refaite depuis l'original à chaque lâcher ; éléments vectoriels : points transformés
  (le texte prend l'affine la plus proche).

## Interface de la phase 6

- `toolbars.rs` : barre de commandes (`TopBottomPanel`) et barre d'outils verticale (`SidePanel`
  de 44 points), outils groupés par `TOOL_GROUPS` ; elles ne sont pas des panneaux du dock.
- `colorui.rs` : conversions teinte / saturation / valeur testées, curseurs dégradés (depuis la
  passe couleur, les conversions sont dans `couleur.rs` : voir « Couleur »).
- `layersui.rs` : panneau Calques ; vignettes de calque rendues sur le processeur (cadre du
  projet, 64 points de large), en cache par dessin et par `version`. `layers_panel` : mode,
  opacité et `layer_commands` en haut, puis `layer_list` dans sa propre `ScrollArea` (l'onglet
  n'a plus de barre de défilement, `scroll_bars`). Chaque rang pose sa zone cliquable et
  glissable (`glisser_calque`, rectangle de l'image d'écran d'avant, en mémoire egui) avant ses
  boutons : posée après, elle prenait le dessus dans le test d'egui (un widget du dessus qui ne
  sent que le glisser annule les clics de ceux du dessous) et rendait l'œil, le verrou et le
  clic droit muets. Menus : `layer_full_menu(…, in_montage = false)`.
- `brushui.rs` + `brush::stroke_preview` : l'aperçu d'un réglage est un vrai trait du moteur dans
  une petite image (taille ramenée à la hauteur), en cache par réglage et échelle d'écran.
- Vue Vignettes (`storyboard::panels_view`) : la vignette du panneau choisi appelle `canvas_panel`
  dans un `Ui` enfant, en vue caméra (`in_card`) ; vue, vue caméra et rectangle du dessin sont
  remis après. Pas de second chemin de dessin : outils, annulation et rendu sont les mêmes.
- Affichage GPU (`gpu.rs`, `fs_display`) : en réduction, n × n lectures par pixel d'écran
  (n = réduction / 1,5, 8 au plus), à partir des dérivées des coordonnées de texture.
- **Calques de toute la scène** (`document.rs`, 6f) : pas de calque « hors panneau » ; chaque
  panneau de la scène porte un exemplaire (`Layer::scene_wide = SceneWide { key, scene }`) qui
  pointe sur le même dessin pour toutes ses images. `Document::sync_scene_layers`, appelé à chaque
  image, accorde les exemplaires sur le modèle (celui du panneau courant, sinon le premier) et
  retire ceux égarés dans une autre scène. Ainsi le rendu, l'export, les vignettes, la pelure
  d'oignon et les outils n'ont rien à savoir. Les opérations qui créent une scène depuis une autre
  (couper, copier) donnent au fond une nouvelle clé et sa propre copie du dessin
  (`adopt_scene_layers`) ; supprimer le calque le retire de toute la scène ; il ne se fusionne pas.
- `montagerows.rs` : rangées de la bande de montage (`Rows` : caméra, panneaux, calques, fond,
  sons), leur dessin, leurs noms à gauche et le clic sur un calque.

## Planche et pinceaux vectoriels texturés (phase 7)

- `board.rs` (sans interface, testé dans `board_tests.rs`) : disposition des vignettes en
  coordonnées de la planche (points au zoom 1, retour à la ligne à la largeur de la vue), place
  visée par des panneaux lâchés (`drop_target`), notes (`Board`, `NoteStroke`). Les notes sont
  dans `Structure` : l'annulation et l'enregistrement (`planche`) les couvrent.
- `boardui.rs` : la planche (zoom, décalage, gestes : déplacer, choisir, noter, gommer, glisser
  des panneaux), menus. `Document::move_panels` / `remove_panels` agissent sur plusieurs panneaux.
  La vignette du panneau courant appelle `canvas_panel` dans un `Ui` enfant, agrandi de la marge
  de la vue caméra et coupé à la vignette ; `canvas_input` n'accepte que la partie visible
  (`interact_rect`). Vignettes : une par plan et par largeur affichée (`thumbs.rs`).
- Traits vectoriels texturés : `Element::Stroke.brush` garde le réglage, pointe et papier
  désignés par une **clé tirée du contenu** (`brushtex::content_key`), stable d'une machine à
  l'autre. `brushtex` tient un registre partagé (bibliothèque + textures des projets ouverts),
  car le rendu vectoriel n'a pas accès à l'application. Rendu : `vector::paint_textured` passe
  les points au moteur bitmap (`StrokeState`) sur une feuille à part, la pression retrouvée
  d'après le rayon, puis pose le résultat dans la zone demandée. Transformer un trait met à
  l'échelle la taille du pinceau.

## Planche en pages et modes de fusion (phase 8)

- `board::layout` range les vignettes d'après `BoardSettings` (taille, colonnes ≤ 6, lignes par
  page, écart, scène à la ligne), sans dépendre de la vue. Notes (`Board`) : traits et textes ;
  une note attachée (`anchor` : id du panneau) garde ses coordonnées relatives à la vignette, en
  largeurs de vignette — elle suit la vignette et change d'échelle avec elle. Les opérations qui
  retirent des panneaux libèrent leurs notes à leur dernière place (`release_removed_notes`,
  appelé par `structure_op` et la suppression multiple).
- `blend.rs` : `BlendMode` (17 modes, 27 depuis 9a ; formules W3C en valeurs gamma) et `blend_over` pour la
  composition sur le processeur (`render.rs` : export, vignettes, pipette ; `merged_cel`). Sur la
  carte graphique, un calque en mode de fusion arrête la passe de composition, copie l'image
  composée dans une texture « fond », puis est dessiné par `fs_blend` (lecture du fond au même
  pixel, résultat qui remplace). Coût : une copie de l'image par calque concerné.

## Phase 9 : fusion complète, caméra continue, cadres, plateau en 3D

- **Modes de fusion** (9a) : 27 dans `BlendMode::ALL`, dont l'ordre est le numéro passé au
  shader (Normal 0, Fondu 1… Luminosité 26) ; `FAMILY_STARTS` sépare les familles dans le menu.
  Fondu : un bruit entier par pixel de la feuille (`dissolve_noise`), le même sur le processeur et
  la carte graphique ; le pixel du calque est pris si le bruit est sous son opacité.
- **Calques et fiche** (9b) : `layersui.rs` sans zone de réglages en bas (menu du calque au clic
  droit, renommage en place). La fiche est un onglet de la colonne des réglages
  (`dock::SETTINGS_TABS`) ; `LAYOUT_VERSION` 2, `DockLayout::upgraded` y range la fiche des
  dispositions enregistrées avant.
- **Caméra continue** (9c, `camera.rs`, `Document::panel_camera`) : un panneau sans clé prend la
  caméra du panneau précédent de la scène, à sa dernière image (sinon celle du suivant, à sa
  première image). Les panneaux qui ont des clés ne changent pas : c'est le choix le moins risqué
  pour les projets existants (une interpolation à l'échelle de la scène aurait tout modifié).
  `Camera::dolly` : avancée vers le décor, en pixels de la feuille ; la caméra de référence est à
  D = largeur du cadre × 35 / 36 (`ref_distance`, focale de référence 35 mm pour 36 mm de large) ;
  grossissement = zoom × D / (D − avancée), focale = 35 mm × zoom. Multiplan : un calque de
  profondeur d, vu de la caméra décalée de o et avancée de z, est agrandi de k = s(1 + d) et
  décalé de (1 − s) o, avec s = (D − z) / (D(1 + d) − z) ; derrière la caméra, il n'est pas
  dessiné (`Layer::transform_seen` → `None`). Adoucis en images (`CameraKey::ease_in/out`) : vitesse
  en trapèze (accélération, palier, freinage), `camera::ease` ; l'ancien « doux » vaut des adoucis
  d'une demi-durée. Fenêtre de la caméra : `camtools::camera_panel`.
- **Barre de la vue et cadres** (9d, `viewbar.rs`, `frames.rs`) : au-dessus du dessin (pas dans
  les vignettes). Plateau / Caméra : `App::set_camera_view` garde la vue du plateau
  (`stage_view`) et la rend au retour. Cadres tracés par-dessus, dans le repère du cadre de la
  caméra (`Camera::to_stage`) : ils suivent sa position, son zoom, sa rotation. Format : le plus
  grand rectangle du rapport voulu, centré (`format_rect`) ; zones de sécurité 93 % / 90 % du
  cadre du format s'il y en a un, sinon du cadre de la caméra. Réglages de la vue : gardés avec la
  disposition (`Layout::safe_frames`, `frame_format`), pas dans le projet ; changer d'espace de
  travail ne les change pas.
- **Plateau en 3D** (9e, `stage3d.rs` : géométrie testée ; `stage3dui.rs` : affichage et
  gestes). Monde en pixels de la feuille, x à droite, y en haut, z vers la caméra ; plan de
  référence z = 0, caméra de référence en z = D. Un calque de profondeur d est le plan
  z = −D·d, agrandi de (1 + d) : vu de l'œil de la caméra (`World::eye`, z = D − avancée), il
  tombe exactement là où le rendu 2D le met (testé avec décalage, avancée, transformation du
  calque). Affichage par egui : plans en maillages subdivisés 16 × 16 (la texture est plaquée
  par triangle ; sans subdivision la perspective tordrait le dessin), peints du plus loin au plus
  proche de l'œil de la vue (plans parallèles : cet ordre suffit) ; morceaux derrière l'œil
  retirés, segments coupés (`Projector::segment`). Textures : chaque dessin réduit à 1024 px de
  large sur le processeur (`render::cel_image`, réparti sur les cœurs), en cache par dessin et
  version ; la transformation du calque est appliquée aux sommets, pas à la texture (déplacer un
  calque ne la refait pas). La vue réutilise `view3d::Cam3d` (orbite, glisser, avancer,
  `fit_at`). Outil Déplacer : rayon de la souris (`Projector::ray`) coupé avec les plans
  (`stage3d::pick` : d'abord un plan dessiné à cet endroit, sinon la feuille la plus proche) ;
  dans son plan, le déplacement monde ÷ (1 + d) va dans `Layer::transform` ; avec Maj, la
  profondeur suit la souris le long de l'axe z (`z_motion`). La caméra se prend par son œil
  (déplacement dans son plan, Maj : avancée, Ctrl : panoramique et bascule). Une étape
  d'annulation au lâcher. Les modes de fusion ne sont pas appliqués dans la vue 3D.
- **Bascule et panoramique** (9f, `camera.rs`) : `Camera::tilt`, `Camera::pan` (degrés). Le
  multiplan ramène déjà chaque calque sur le plan de référence tel que l'œil le voit, et cette
  projection ne dépend que de la place de l'œil, pas de son orientation : une caméra qui tourne
  sur elle-même filme donc ce plan en perspective. Le passage image finale → feuille devient une
  homographie (`Camera::homography` : centrage, roulis, focale f = zoom × D, bascule autour de X,
  panoramique autour de la verticale, rencontre du plan z = 0 depuis l'œil ; w > 0 devant la
  caméra), son inverse est composé des inverses de chaque étape (`homography_inverse`). Caméra
  droite : l'ancien calcul affine, inchangé (`FrameMap::Level`). Rien d'autre ne change dans le
  rendu : `camera_image` et `thumbnail` lisent la feuille composée par `FrameMap::at`.
  Affichage : la vue caméra est une `View` avec `persp` (homographies feuille ↔ écran,
  `camtools::turned_view`) ; tous les `to_screen` / `to_canvas` (outils, repères, curseur) y
  passent. Carte graphique : le w de chaque coin de la feuille va au shader (`color` de l'élément
  d'affichage, `params.w` = 1), qui sort `(xy·w, 0, w)` : interpolation de la texture en
  perspective et découpe de ce qui passe derrière la caméra par le matériel. Secours sans carte
  graphique : maillage 24 × 24 (approché). Sur le plateau, le cadre filmé est un trapèze ; le
  hors-champ estompé est la feuille moins le cadre coupé à la feuille (`viewbar::clip_to_rect`,
  `nested_band`). `Camera::limited` (appelé par `set_current_camera`) réduit bascule et
  panoramique tant qu'un coin du cadre ne voit pas le plan sous au moins `MIN_GRAZING` = 5°.
  Testé : caméra droite = calcul d'avant, inverse, visée (E·tan φ), trapèze, vue 3D « par la
  caméra » (repère `Cam3d` calculé à part) qui voit le cadre aux coins de l'écran, image filmée.

## Appli compagnon Tauri (phase 10)

- **Bibliothèque** (10a) : `src/lib.rs` déclare tous les modules et `pelure::lancer()` démarre
  l'application egui ; `src/main.rs` ne fait que l'appeler. Les modules restent privés à la
  bibliothèque ; seul `noyau` est public.
- **Noyau** (`noyau.rs`) : `Projet` (document, manifeste, dessins déjà écrits pour
  l'enregistrement incrémental, « modifié ») et des fonctions en données simples, sérialisables
  (`ResumeProjet` > `ResumeSequence` > `ResumeScene` > `ResumePanneau`) : ouvrir, nouveau,
  enregistrer, résumé, vignette PNG (`render::thumbnail` + `io::encode_png`), panneaux (ajouter,
  dupliquer, déplacer, supprimer), textes, durée, PDF (même chemin que l'export de Pelure, sur un
  seul fil). Aucune dépendance à egui dans son API.
- **`src-tauri/`** (10b) : projet Cargo indépendant (`[workspace]` vide : son `Cargo.lock`, son
  `target`), qui dépend de `pelure` par chemin. État : `Mutex<Option<Projet>>` géré par Tauri ;
  commandes `#[tauri::command]` enregistrées par `generate_handler!` ; les lentes sont
  `async` ; la vignette part en octets (`ipc::Response`). Fenêtres de fichiers par `rfd`.
- **`src-frontend/`** : Vite 8, React 19, TypeScript 5.9, Tailwind 4 (extension Vite). `api.ts`
  type chaque commande ; `App.tsx` : accueil, planche par séquence et scène, fiche du panneau.
  Après chaque commande, Rust renvoie tout le résumé (quelques Ko) : pas d'état à synchroniser.
  Détails et lancement : `docs/COMPAGNON.md`.

## Moteur de dessin partagé (phase 11)

- `moteur.rs` (public, testé dans `moteur_tests.rs`) : le moteur de pinceau de Pelure pour une
  autre interface — Pelure Studio (`..\pelure-studio`, projet à part qui en dépend par chemin).
  `Pinceaux` (la `BrushLibrary`, avec les pinceaux importés de `%APPDATA%\Pelure\pinceaux`, lus
  seulement), `Reglage` (nom, gomme, taille, opacité, texture) et `apercu` (`stroke_preview`) ;
  `Dessin` (un `Raster` : tuiles 64 × 64, `tuile_rgba`) ; `Trait` (`StrokeState` : `commencer`
  avec un `Outil` — réglage, couleur, taille / opacité / pression retouchées, gomme —, `point`,
  `pointe` pour relier le trait au stylet, `finir`) ; `Annulation` (les tuiles d'avant ;
  `appliquer` défait et rend de quoi refaire). Le moteur lui-même n'a pas changé.
- Panneau **Pinceaux** (11c, repris du Studio) : `Pane::Brushes`, en onglet avec la couleur
  (`dock::COLOR_TABS` ; `open` l'y remet). `LAYOUT_VERSION` 3 : `upgraded` l'ajoute aux
  dispositions enregistrées avant (s'il manque) ; fermé ensuite, il le reste.
  `brushui::brushes_panel` : réglages de la cible de l'outil (gomme, sinon pinceau), en liste ;
  grands aperçus (`stroke_preview`) en cache (`BrushPreviews::wide`) par réglage et taille en
  pixels de l'écran, largeur par paliers de 24 points pour ne pas les refaire en tirant un bord.

## Thème (phase 12)

- `theme.rs` : la palette (gris 25 à 900 et bleus 800 / 900 du thème sombre de Spectrum, valeurs
  de @spectrum-css/tokens 16) en constantes ; `visuels` (egui : panneaux, champs, contrôles au
  repos / survolés / enfoncés, sélection), `densifier` (espacements, tailles de texte),
  `polices` (Segoe UI lue dans `%WINDIR%\Fonts`, en tête de la famille proportionnelle ; les
  polices d'egui et les icônes restent derrière), `dock` (onglets et séparations d'egui_dock).
  Appliqué au démarrage (`App::new`) et à chaque image pour les onglets (`show_dock`). Les fonds
  dessinés à la main (autour de la feuille, barre de la vue, masque de la vue caméra) prennent
  `theme::FOND` / `ENTETE`. `contraste` (WCAG) sert aux tests.
- Aperçus des pinceaux (12b) : la texture d'aperçu reste une couverture en gris (en cache) ; la
  couleur est donnée au dessin (`preview_colors` : papier du projet, couleur courante, gris pour
  une gomme) — changer de couleur ne refait aucun aperçu.
- Timeline et montage (12c) : leurs couleurs dessinées à la main viennent toutes de `theme.rs`
  (`REGLE`, `RANGEE` / `RANGEE_SOMBRE`, `ETIQUETTE`, `DESSIN_*` dans la timeline et `CALQUE_*`,
  plus sourdes, sous les panneaux, `IMAGE_COURANTE`, `CAMERA`, `SON_*`, `barre(séquence,
  impaire)`) : `timeline.rs`, `montagerows.rs`, `soundtrack.rs`, `storyboard.rs`. Les deux
  barres d'outils sont une `ScrollArea` horizontale autour d'une seule ligne ;
  les boutons viennent de `bulles.rs` (12d).
- Panneaux choisis et presse-papiers (12e) : le choix est `App::board_sel` (partagé par le
  montage et la planche ; `board_selection()` y ajoute toujours le panneau courant).
  `Document::copy_panels` copie les panneaux et leurs dessins dans un `PanelClip` (avec la taille
  du projet) ; `paste_panels` les insère après un panneau, dans sa scène, avec de nouveaux ids et
  de nouveaux dessins (un calque de toute la scène n'est gardé que dans sa scène :
  `sync_scene_layers`). `App::panel_focus` dit où vont Ctrl+C / Ctrl+V / Ctrl+X / Suppr : vrai
  après un appui dans la bande de montage ou sur une vignette, faux après un appui dans le dessin
  (`canvas_panel`). Glisser un panneau choisi déplace le groupe (`move_chosen_panels`, par
  `Document::move_panels`).
- Glisser une clé de caméra (12e) : `camera::move_key` (bornée entre ses voisines, testée),
  `camtools::CamKeyDrag` (panneau, clé, image de départ, état d'avant pour une seule étape
  d'annulation), dans la rangée Caméra de la timeline et la piste Caméra du montage. Échap,
  Ctrl+Z ou un changement de mode pendant le geste remettent la clé où elle était
  (`cancel_montage_drag`).
- Boutons icônes et bulles (12d) : `bulles.rs`. Fonctions libres (`icone`, `icone_si`, `etat`,
  `etat_si`, `interrupteur`, `choix`, `etiquette`, `bulle`) qui prennent le texte du raccourci ;
  méthodes d'`App` (`bouton`, `bouton_si`, `bouton_etat`, `aide`, `raccourci`) qui prennent
  l'identifiant de l'action et lisent son raccourci dans le fichier de raccourcis. Une bulle :
  nom, raccourci, description. Délai d'une seconde : `theme::densifier` règle
  `interaction.tooltip_delay` sur `bulles::DELAI`. Descriptions des outils :
  `Tool::description`, `Nav3d::description`. Un test parcourt les sources et vérifie que chaque
  action citée par un bouton existe dans le fichier de raccourcis.

## Séquences et plans (phase 13a)

- Un plan est une `Scene` qui a exactement un `Panel` (`Document::is_plans`) : le modèle et ses
  fonctions (calques, caméra, rendu, lecture, enregistrement) restent ceux d'un panneau ; à
  l'écran, « plan » partout. Les opérations de plan gardent ce rang : `add_scene_after` (nouveau
  plan), `duplicate_scene_after`, `paste_plans` (chaque plan collé dans sa propre scène),
  `split_plan` (F5 : la suite devient un nouveau plan), `join_next_plan` (réunir),
  `move_plans` (un plan garde sa séquence s'il retombe à côté d'elle, sinon rejoint celle du
  plan devant lequel on le pose), `remove_panels`. Les anciennes fonctions qui créaient plusieurs
  panneaux par scène (`add_panel_after`, `split_panel`, `merge_with_next`, `move_panel(s)`,
  `split_scene`, `join_previous_scene`) ne servent plus qu'en interne et aux tests.
- Marques d'image : `Panel::marks` (`ImageMark` : id unique dans le document, image, couleur,
  textes), triées, après la première image. Une image de storyboard = le début du plan, ou une
  marque ; `board::plan_boards` les donne avec leur étendue. La planche fait une vignette par
  image (`CardPlace` : plan, id de la vignette, [frame, end)), la fiche édite les textes de
  l'image sous la tête de lecture (`Document::board_mark_at`), `export::storyboard_of` fait le
  PDF et la page HTML, `Thumbs::texture_at` les vignettes à une image donnée. Les notes de la
  planche s'attachent à l'id d'une vignette (plan ou marque).
- Conversion (`Document::into_plans`, appelée par `project::load` si `!is_plans()`) : les
  calques de toute la scène deviennent ordinaires (`flatten_scene_layers`), puis chaque scène est
  réunie en un plan par `append_plan` : caméra tenue aux jointures (`held_keys`, clés au début
  et à la fin de chaque partie), calques appariés par nom puis par rang s'ils sont réglés pareil
  (sinon un calque de plus, juste au-dessus de celui où va le calque d'en dessous), début de
  chaque partie = marque d'image avec ses textes, notes ré-attachées (`reanchor_notes`), repère
  3D du premier gardé (avertissement sinon). Durée totale inchangée : le son reste calé.
  `split_plan` fait l'inverse (une marque à l'image de coupe redevient le début du plan).
- Les calques de toute la scène (6f) ne se créent plus : « Nouveau calque de fond » fait un
  calque ordinaire dont un seul dessin est tenu sur tout le plan. `sync_scene_layers` reste pour
  les projets en cours d'ouverture, sans effet sur un projet en plans.

## Réglages des calques (phase 13b)

- `Layer` : `onion` (pelure d'oignon du calque : `build_plan` montre ses voisines comme pour le
  calque courant), `keep_alpha`, `stencil` / `stencil_negative`, `position_locked`, `group`
  (0 à 12), `collapsed`. `Layer::drawable` = ni verrouillé ni de position verrouillée ;
  `layer_editable`, Transformer (`transform_press`, `transform_fields`, Ctrl+R), sélection,
  vue de dessus, plateau 3D et profondeur du menu de calque le respectent
  (`app::POSITION_LOCKED`).
- `Document::groups` (`groups.rs`, toujours 13, dans `Structure` pour l'annulation).
- Préserver la transparence : `StrokeState::on_layer(keep_alpha, …)` ; `brush::paint_over`
  change la couleur sans toucher à l'opacité du pixel ; une tuile vide est sautée ; la gomme
  n'est pas concernée.
- Pochoir (`stencil.rs`) : `App::stencil_for` construit au début du trait un `Stencil`, facteur
  0–255 par pixel du dessin peint, produit de tous les calques-pochoirs visibles du plan (sauf le
  calque peint) à l'image courante ; une tuile entière vaut une seule valeur tant qu'aucune forme
  ne la touche. Un pochoir déplacé (transformation, multiplan) est lu à travers
  `affine(peint) puis inverse(affine(pochoir))`. Le facteur multiplie la couverture de chaque
  empreinte, pour le pinceau, le crayon et la gomme bitmap. Calques vectoriels, pot de peinture,
  formes et texte ne sont pas encore concernés.
- En-tête de la timeline (`layerheads.rs`) : `rows_of` donne les lignes (calque, haut, hauteur :
  `ROW_FULL` ou `ROW_SMALL` si `collapsed`), utilisées par l'en-tête et par la grille
  (`timeline.rs`) pour rester alignées. Chaque ligne : un `interact` de fond (choisir, glisser,
  menu, double-clic sur le nom), puis les boutons posés dessus (`ui.put`, enfants `new_child`
  pour les menus et le mode). Glisser : `App::head_drag`, `drop_index`, `move_layer_to` (une
  étape). Renommage partagé avec le panneau Calques (`LayerRename::in_timeline`).
- Opérations (`layerops.rs`) : `App::layer_sel` (ids) + le calque courant = calques choisis
  (`chosen_layers`) ; fonctions sur le document `duplicate_layer`, `merge_layers` (rassemble les
  choisis au-dessus du plus bas puis `merge_layer_down` en chaîne ; tout est remis si une fusion
  est refusée), `choose`, `is_empty_layer` ; `layer_ops_menu` sert au menu Calques et au ☰ de
  l'en-tête. `newlayer.rs` : fenêtre « Nouveau calque… » et `insert_index` (place choisie).
- Groupes (`groupsui.rs`) : `LayerGroup::shown` est un filtre de la timeline seulement
  (`rows_of` saute ces calques ; `drop_index` pose un calque glissé juste au-dessus du calque de
  la ligne d'en dessous, les calques sans ligne ne bougent pas). Commandes en masse par
  `Scope` (tous / groupe courant / autres) et `layers_in`. Les groupes sont dans `Structure` :
  noms, couleurs et filtre s'annulent comme le reste.
- Dossiers (`folders.rs`, 13b-5) : un dossier est un `Layer` avec `folder` (aucun dessin) ;
  un calque rangé a `parent` = l'id du dossier. Invariant tenu par `normalize` : un seul niveau,
  les calques d'un dossier juste sous lui, d'un seul tenant (chaque opération qui touche à la
  pile le rappelle). `effect` donne l'opacité d'un calque à l'image (la sienne × celle de son
  dossier, rien si l'un des deux est caché) : `build_plan`, `render::plan_with_camera`, la
  lecture anticipée, les pochoirs et le plateau 3D s'en servent. `move_step` déplace par
  unités (un dossier et ses calques, ou un calque seul) ; `folder_at` décide si un calque
  glissé entre dans un dossier. `drawable` est faux pour un dossier ; `refuse_on_folder` garde
  les commandes de dessins et d'expositions.
- Images (`images.rs`, 13c) : `App::frame_sel` [début, fin) sur les calques choisis ;
  `images_focus` (un clic dans la grille) fait viser les images par Ctrl+X / C / V et Suppr,
  comme `panel_focus` pour le montage. Fonctions sur une timeline de calque (`delete_frames`
  et son `Hole`, `insert_frames`, `reverse_frames`, `frames_in`) ; `shift_panel` décale les
  clés de caméra et les marques quand tout le plan change de durée. `ImageClip` garde une
  copie des dessins ; chaque collage en refait. Instances (`next_instance`, `prev_instance`,
  `break_instance`, `change_exposures`), faux-fixe (`fake_hold`, par instances, hasard
  déterministe à graine), `stretch_frames` (le post-comportement posé dans la timeline),
  `drop_frames` (déplacer / copier / insérer). La cassure automatique se fait dans
  `ensure_cel`, que tous les outils de dessin appellent ; la création automatique est gardée
  par `layer_editable`. Noms d'instance : `Layer::names` (dessin → nom), le renommage
  intelligent dans `smart_rename`. Retourner : `Document::flipped_cel` (une copie).
- Copies de calques et de plans : `document::relink_copies` remet les `parent` (dossiers) et
  les noms d'instance sur les nouveaux id (dupliquer / couper / coller un plan, dupliquer ou
  coller un calque).
- Repères (`cues.rs`, 13d) : `play_range` (entrée / sortie) sert à `toggle_play`,
  `tick_playback`, au son (`play_range` de `soundtrack.rs`) et à la lecture anticipée
  (`playback::Position::range`). `frame_label` / `parse_frame` : image, time code, trame,
  depuis `Document::start_frame`. `images::shift_panel` déplace aussi signets et points.
  Zoom : `App::timeline_cell` (largeur d'une image), `timeline_fit` en attente pour
  « Adapter » (posé à l'image suivante, quand la largeur visible est connue).
- Onglets du bas (`bottomtabs.rs`, 13e) : un `TopBottomPanel::show_inside` en bas du panneau
  Timeline (une mise en page « de bas en haut » ne connaît pas la hauteur à l'avance). Le
  damier est un maillage à texture 2 × 2 répétée (`TextureWrapMode::Repeat`) sous les calques ;
  « aucun » : un quadrilatère noir. Notes (`notes.rs`) : une rangée de la grille (sous la
  caméra) quand l'onglet Notes est ouvert ; un geste qui y commence ne touche ni la tête de
  lecture ni les images ; `paint_notes_overlay` dessine une bande centrée sur l'image courante.
- Feuille d'exposition (`xsheet.rs`, `xsheetui.rs`, 13g-1) : un panneau ancrable de plus
  (`Pane::XSheet`, `"feuille_expo"`, groupe des calques, absent de la disposition par défaut ;
  `toggle_pane` rend actif un panneau qu'on ouvre). Rien de nouveau dans le projet : chaque case
  se calcule à partir du plan (`layer_cells`, `camera_cells`, `story_rows`, `audio_levels`).
  Affichage par `ScrollArea::show_viewport` : seules les lignes visibles sont peintes, l'en-tête
  est peint en haut de la partie visible. `XSheetSettings` est un réglage de l'application
  (`[feuille_expo]`).
- Rôles des images (`roles.rs`, `rolesui.rs`, 13g-2) : `Layer::roles`, image → rôle, comme une
  case de la feuille (même vide, même après la fin du calque). Ils ne suivent pas tout seuls les
  images : chaque opération qui en déplace le fait (`roles::shift`, `reverse`, `take` / `put`),
  ainsi que `Layer::insert`, `Layer::append_at` et la coupe / réunion de plans. Les autres
  réarrangements (une exposition de plus à chaque instance) les laissent à leur image.
  Décision : rangés par image plutôt que par dessin, parce qu'un intervalle « à faire » se pose
  là où rien n'est encore dessiné (choix d'Anthony).
- Flips (`flips.rs`, `flipsui.rs`, 13h) : `FlipScope` calcule les limites autour de l'image de
  départ et les checkpoints (calcul pur, testé) ; `next_point`, `step` et `played` donnent
  l'image suivante. Le flip en cours (`App::flip_run`) avance dans `tick_flip`, à chaque
  image d'affichage :
  - basique : selon le temps écoulé ;
  - directionnel : selon le déplacement de la souris.

  Il finit dès que sa touche (`Keymap::held`) ou le bouton de la souris n'est plus tenu. Un
  flip manuel garde son image de départ (`flip_manual`) tant qu'on n'a pas bougé l'image
  courante autrement. `FlipSettings` est un réglage de l'application (`[flips]`), pas du
  projet.
- Pile d'effets (`fx.rs`, `fxui.rs`, 13i) : `Layer::fx`, effets vivants. `PlanItem::fx` porte
  les effets à appliquer à cette image (`steps_at`). Les moteurs lisent les dessins par
  `fx::shown`, qui rend :
  - le dessin lui-même s'il n'y a pas d'effet ;
  - sinon un dessin dérivé, calculé une fois et gardé dans un cache (`fxcache.rs` : 768 Mo de
    pixels, les plus anciens partent ; 48 entrées au plus avant 2026-10-18, ce qui vidait le
    cache à chaque tour d'une animation d'effet). Le cache est indexé par `fx_id` (dessin +
    paramètres, bit de poids fort mis) et refait quand la version du dessin change ; ouvrir un
    autre projet l'oublie (`fx::forget_all`, dans `App::load_doc`).
  - `fx::shown` calcule en bloquant (export, vignettes enregistrées, tests) : un seul calcul
    même si plusieurs fils le demandent (`fxcache::get_or_compute`). L'écran, lui, passe par
    `fx::display` (GPU, vignettes en mode doux `fxcache::soft_scope`) : jamais de calcul sur le
    fil de l'interface. Un dessin non prêt est demandé au fil de fond (`fxcache::request`, file
    par priorité : l'image affichée, puis les 24 qui viennent par `fx::prefetch`, puis les
    vignettes ; une demande non renouvelée en 6 images d'écran est abandonnée ; le fil laisse
    deux cœurs libres, `render::set_thread_cap`) et on montre son dernier résultat
    (`fxcache::fallback`, par dessin d'origine) ou le dessin sans effet ; `gpu::sync` rend le
    numéro de texture réellement composé pour chaque élément (celui du dernier résultat, donc
    sans nouvel envoi). Le fil de fond redessine l'écran à chaque résultat.
  - Aperçu (`fxproxy.rs`, `fx::draft_level`) : lecture, défilement et gestes sur un curseur
    (`fx::note_interaction`) calculent sur un dessin réduit `1 << niveau` fois (niveau selon le
    zoom : moitié sous 0,8, quart sous 0,35), numéro de dessin dérivé à part, texture plus
    petite à la carte graphique (la composition agrandit). Admis seulement sans transformation
    du calque et pour les effets qui suivent l'échelle (`proxyable`) ; rayons et décalages
    réduits (`scaled_values`), bruit = moyenne du hasard pleine taille (`noise_scaled`).
    Quand rien ne bouge depuis 250 ms, la pleine résolution est demandée et remplace l'aperçu.
  - Calcul des effets : tous exacts à l'octet près avec l'ancien code (empreintes dans
    `fxexact_tests.rs`), répartis sur les cœurs par `render::par_chunks` (rangées de tuiles ;
    flou : lignes, transposition par bandes de 16 colonnes, lignes, transposition).
  - Au-delà des flous (2026-10-17) : `fxfilters.rs` (purs, testés) — réglages de couleur sur
    les pixels dessinés (`map_colors`, couverture inchangée), netteté (différence avec
    `fx::gaussian`), bruit (hasard fixe par pixel), mosaïque, ombre portée, lueur (débordent :
    `fx::region_of` / `into_raster`). `EffectKind::family` / `FAMILIES` rangent les effets ;
    `changes` dit quand un effet ne change rien (il n'est pas calculé). Menu du calque :
    `App::fx_menu_items` (familles en cases à cocher), `toggle_fx` (ajoute avec les réglages
    par défaut, sinon active ou désactive), `fx_on` ; fenêtre « Effets » (`fxui.rs`).
  - Déformation et Rendu : `fxwarp.rs` (purs, testés). Déformations par lecture inverse
    bilinéaire sur toute la feuille (`warp`, parallèle) : onde, ondulation, tourbillon,
    pincement, centres en % de la feuille. Rendu sur la couleur des pixels dessinés (`shade`) :
    nuages (bruit lissé, 4 octaves, graine), éclairage, dégradé.

  La carte graphique garde la texture du dérivé sous ce même numéro. Les flous ne travaillent
  que sur la zone dessinée, marge comprise :
  - flou gaussien : trois flous en boîte par sommes glissantes, en lignes puis en colonnes
    (transposition par bandes, en parallèle) ;
  - flou de mouvement : jusqu'à 64 échantillons bilinéaires le long du segment, répartis sur
    les cœurs.

  Décision : coût — le calcul se fait à chaque nouveau jeu de paramètres (en fond), et pendant
  un trait le flou n'est refait qu'à la fin. Risque faible : les dessins ne changent jamais.
  Réversible : retirer l'effet suffit. Les clés suivent les images insérées / supprimées
  (`Layer::shift_frames`, avec les rôles).
- Calque CTG (`ctg.rs`, `ctgui.rs`, 13j) : `Layer::ctg` (sources : id de calques). Le plan
  porte, pour un calque CTG, les dessins de ses sources à cette image (`PlanItem::ctg`).
  `fx::shown` en fait un dessin dérivé (le remplissage, puis les effets), gardé dans le même
  cache sous `shown_id`, et refait quand la version du dessin de graffitis ou d'une source
  change.

  Le remplissage, `ctg::fill` :
  - un trait est un pixel des sources d'opacité au moins 128 ;
  - fermeture des trous (`Ctg::gap`, 4 par défaut) : les traits sont épaissis de `gap`
    pixels (deux balayages de distance par sens, carré) pour chercher les zones ;
  - depuis chaque graffiti, une zone est remplie de proche en proche (4 voisins) hors des
    traits épaissis, et prend la couleur la plus présente parmi ses graffitis ;
  - la couleur regagne la bande d'épaississement (`gap` pas, sans traverser un vrai trait),
    puis 2 pixels sous les traits voisins.

  Seules les zones touchées sont parcourues. Sources enregistrées par leur rang dans le plan
  (format 20), refaites en id au chargement, et suivies par `relink_copies` quand on copie un
  plan.
- Calque vidéo (`video.rs`, `videoui.rs`, 13k) : `Layer::video` (`VideoClip` : id, nom,
  nombre d'images, cadence, début, extension) ; pas de dessins. `video::cel_at` donne, à une
  image du plan, un numéro de dessin propre à cette image de la vidéo (`frame_id`, bit 62),
  retenu dans une table. `fx::shown`, ne le trouvant pas dans `doc.cels`, lit l'image
  (`video::shown` : fichier → feuille, image centrée dans le cadre ; 24 images gardées au plus).
  Où sont les images de chaque vidéo : une table (`register_dir`) — dossier temporaire après
  l'import, `videos/<id>/` du projet au chargement. `SaveJob` recopie dans le projet celles qui
  y manquent.

  Décision : coût — décoder une fois à l'import (ffmpeg, sur un autre fil) plutôt qu'à chaque
  image : la lecture d'une image JPEG est rapide, et rien ne reste en mémoire. Les images
  prennent de la place sur le disque (environ 100 à 200 ko par image HD). Risque : ffmpeg doit
  être là (celui de l'export). Réversible : « Transformer en calque d'animation ».
- Interpolation (`tween.rs`, `tweenui.rs`, 13l). Calcul pur :
  - `tween_vector` : les éléments sont appariés par rang. Les traits sont rééchantillonnés à
    longueur égale (le plus grand nombre de points des deux, au plus 512) ; leur sens est
    comparé par les extrémités. Les remplissages sont appariés contour par contour, et les
    éléments sans correspondant voient leur opacité multipliée par t ou 1 − t.
  - `blend_raster` : un fondu prémultiplié, tuile par tuile.
  - `resample` : n images étirées sur m, au 1/64 d'image près. Un intervalle seulement entre
    deux dessins différents qui se suivent ; jamais avec une image vide.

  Les intervalles sont des dessins neufs (vectoriels si les deux sont vectoriels), donc
  modifiables. Changer la cadence passe tout le projet au même facteur (`rescale_frame`) en
  une étape d'annulation : la cadence et les extraits de son sont dans `Structure`.
- Propagation façon EbSynth (`propager.rs` sans interface, `propagerui.rs` fenêtre et tâche,
  moteur dans la caisse `synthese/`, sur la carte graphique d'eframe) : voir
  `docs/PROPAGATION.md`.
- Poignées et menus (passe interface, `handles.rs`).
  - `Handle` désigne ce que l'on tient : une marque, un signet, l'entrée, la sortie, une clé
    d'effet ou le bord d'une exposition.
  - `HandleDrag` garde le plan d'avant (Échap via `cancel_montage_drag`) et l'étape d'annulation
    du geste, poussée au lâcher seulement si quelque chose a changé : le modèle des clés de
    caméra (`CamKeyDrag`).
  - Les déplacements sont des fonctions pures testées (`between`, `move_mark`, `move_bookmark`,
    `move_fx_key`, `set_run_length`) ; chaque élément reste entre ses voisins, l'ordre ne change
    pas.
  - Une exposition repart à chaque mouvement du calque d'origine (comme le `Trim` du montage), et
    `Layer::insert` / `shift_frames` font glisser rôles et clés d'effets.
  - La timeline cherche une poignée (`timeline_handle_at`) avant le glisser d'images et le
    défilement.
  - Clic droit : `GridMenu` (règle, caméra, clé d'effet, son, noms des lignes) dans la timeline,
    et de nouvelles cibles dans `MenuTarget` (piste de calque, caméra, séquence, règle) dans le
    montage. Le panneau Calques et le dessin ont aussi leurs menus. Les entrées passent par
    `action_item`, donc chacune peut recevoir un raccourci.
  - Glisser un calque : `layerheads::drop_index` et `move_layer_to` (déjà ceux de l'en-tête de
    la timeline), avec les lignes de l'image en cours (`panel_layer_drag`, `montage_layer_drag`).
- Connexions des brosses (moteur de dessin, étape 2).
  - `BrushSettings::dynamics : Dynamics` contient une `Connection` (source, inverser, courbe,
    minimum) pour la taille, l'opacité, l'angle et la rondeur, plus la vitesse max et la
    longueur du fondu.
  - `Curve` est de taille fixe (8 points au plus) : `BrushSettings` reste `Copy`, on le copie
    partout tel quel. Elle s'enregistre en liste de points (`serde(from/into)`), ordonnée et
    bornée à la lecture.
  - `StrokeState` mesure les entrées (`Inputs`) à chaque empreinte :
    - l'appui, interpolé ;
    - la vitesse, en pixels par seconde, d'après les instants passés par `add_point_at` (l'image
      d'écran), lissée de moitié ;
    - la direction du segment (un tour = 1) et la longueur déjà tracée (fondu) ;
    - le hasard, tiré seulement si une connexion s'en sert, pour garder le même trait qu'avant.
  - `size_connection` / `opacity_connection` rendent la connexion effective : celle de la
    dynamique, sinon la pression simple d'avant (droite de `min` à 1). C'est ce qui garantit le
    même trait pour les anciens réglages (test).
  - Panneau : `dynui.rs` (source, Inverser, Mini, éditeur de courbe : glisser, double-clic,
    clic droit), à la fin des réglages du pinceau.

    Décision : coût — courbe linéaire par morceaux plutôt que spline, simple et sûre, et TVPaint
    en a aussi. Risque : une courbe douce demande plus de points. Réversible : un mode
    d'interpolation pourra s'ajouter au format.
- Banc de traits (moteur de dessin, étape 1) : `banc.rs`.
  - `StrokeState::raw_log` garde les points reçus (x, y, appui, instant ; −1 : instant inconnu).
  - `end_stroke` ajoute le trait au fichier quand le banc est allumé (`App::record_strokes`,
    commande `pinceau.enregistrer_traits`).
  - `replay` rejoue un trait sur une feuille à sa mesure, avec ses instants : la vitesse est la
    même.
- Sensation des brosses (étude du 2026-10-18, `docs/SENSATION-BROSSES.md`) : `brush.rs`.
  - `Toucher` (réglage commun à tous les pinceaux : `Settings::toucher`, copie de travail
    `App::toucher` enregistrée dans `update` hors glissé, `StrokeState::with_feel`) :
    - `pressure_curve`, appliquée dans `add_point` au stylet seulement, après le journal du banc
      (qui garde l'appui brut ; `Recorded::feel` garde le toucher, `replay` le rend) ;
    - `threshold` : au-dessous, le point met à jour `raw` (la pointe affichée) mais ni `contact`,
      ni le lissage, ni les empreintes ; `finish` rattrape `contact`, pas `raw` ;
    - `size_floor`, par `StrokeState::size_factor` ;
    - `thin_full` et `soft_edge`, dans `dab` : `coverage_soft` est une seule smoothstep de
      rayon × dureté au bord `rayon + 0,5`, large d'un pixel au moins ; l'ancienne `coverage`
      reste pour `Toucher::ancien()` et pour les traits vectoriels (`vector::Mask::stamp`).
  - Horodatage : `spread_time` (pure) étale les événements du pointeur d'une image d'écran entre
    `App::last_frame_time` et l'heure de l'image ; `App::input_time` est lu par `begin_stroke`,
    `stroke_to` et `begin_vector_stroke`.
  - Lissage : `follow_alpha(smoothing, dt)` (constante de temps, `FOLLOW_REF_HZ` = 200) ; `dt`
    vient de `add_point_at` (`last_time`), aucun dans `add_point` ; `with_zoom` pour la corde et
    la distance en pixels d'écran (`smooth_screen`) ; `catch_up` selon `CatchUp` (aucun, droit,
    courbe : Bézier quadratique partant de la dernière direction du chemin lissé) ; lissage
    différé (`smooth_deferred`) : `smooth` rend le point brut pendant le geste, et `finish`
    appelle `replay_smoothed` — les tuiles d'avant reprennent leur place, `reset_gesture` remet
    l'état (graine du hasard comprise), puis les points du journal sont rejoués avec `replaying`
    ; au-delà de 2000 tuiles touchées, le trait reste brut.
  - Tests : `toucher_tests.rs`, `lissage_tests.rs` ; `latency_tests::frame_pacing_sleep_is_precise`
    (ignoré) mesure le sommeil du rythme des images.

    Décision : coût — les réglages de toucher sont globaux (`Toucher`) plutôt que par brosse (un
    seul endroit, les réglages de brosses enregistrés ne changent pas), les options de lissage
    par brosse avec `#[serde(default)]` (les anciens fichiers se relisent). Risque : la cadence de
    référence du Suivi et les défauts sont des choix à confirmer à la tablette. Réversible :
    « Ancien toucher » et les cases rendent le comportement d'avant.
- Audit du rendu des brosses (2026-10-18, `docs/AUDIT-RENDU-BROSSES.md`, étude
  `docs/VECTORIEL-COMME-BITMAP.md`) : `rendu_audit_tests.rs` (essai ignoré
  `planches_rendu_audit`) trace quatre gestes synthétiques (rampe d'appui, hachures, S, passages
  croisés) avec chaque réglage, en bitmap (`StrokeState`) et en vectoriel (`vector::simplify` +
  `render_element`, comme `commit_vector_stroke`), et écrit des planches PNG dans `audit-rendu/`
  (ignoré par git) avec `mesures.txt` (alpha le long de la rampe, aplat du S, accumulation,
  écart bitmap / vectoriel). Les imports TVPaint (`.tvpx` d'Anthony, panneaux de l'installation),
  Krita et MyPaint y passent aussi, en lecture seule.
  - `tvpbrush::read_ini` met les sections en minuscules (`[ToolBox]` des exports) ; `base_tool`
    prend `step` en pour cent de la taille (deviné, à vérifier) et note le `grain` du `tv_pencil`.
  - `GrayImage::equalized` (rang, valeurs égales au rang moyen) remplace `normalized` pour les
    papiers importés (`add_paper`, `load`) : le seuil de `grain_factor` suppose un relief réparti
    uniformément, comme celui de `procedural_paper`.
- Rendu des brosses, étapes 1 à 3 (2026-10-18, `docs/AUDIT-RENDU-BROSSES.md` §6 bis).
  - Textures fabriquées (`brushtex.rs`) : `multiscale_paper` (somme de bruits adoucis sur 1, 2,
    4, 8 texels, chacun ramené à l'écart-type 1, puis `equalized`), `canvas_paper`, `laid_paper`,
    `tip_grains`, `tip_stick` (fusain / craie : bord déchiqueté par trois sinus, trous par un bruit
    adouci), `tip_bristles_flat` (poils posés par `stamp_hair`), `tip_bristles_round` ; tirages
    déterministes (`Rng` xorshift, `hash01`), donc la même clé de contenu partout. Numéros fixes
    des textures de Pelure : papiers 1–5, pointes 10–14 (`builtin_tip_cells`), importés ≥ 100.
  - `BrushSettings` : `paper_softness` (largeur de transition de `grain_factor`, défaut 0,12 =
    l'ancien seuil, `GRAIN_SOFTNESS_OLD`), `paper_follow` (relief lu depuis la première empreinte
    du trait, `StrokeState::origin`), `pencil_grain : PencilGrain` (force et douceur par niveau,
    papier fin partagé `pencil_grain_paper`, appliqué dans `dab` en plus du papier) ; tous en
    `#[serde(default)]`, un vieux réglage rend la même image.
  - Débit accumulé : `dab` divise le nombre d'empreintes par `Tip::mean_coverage` (dépôt moyen de
    la pointe sur son carré) pour qu'une pointe clairsemée atteigne le débit réglé.
  - Réglages : `BrushSettings::graphite` (le départ de l'outil Crayon, `App::new`),
    `colour_pencil`, `ink`, `felt`, `charcoal`, `chalk`, `gouache` ; `pencil` = « Crayon net ».
    Le Pinceau rond reste le premier de `builtin()` et le départ de l'outil Pinceau.
  - `pinceauxui::paper_ui` et `tip_ui` (panneau Pinceaux, catégories Grain et papier, Forme de la pointe) : listes « Pointe » et « Papier », « Douceur du grain », « Grain du
    crayon », « Le grain suit le trait ».
  - Imports : `kritabrush` met `opaque` (MyPaint) et Flow (Krita) en débit accumulé ;
    `tvpbrush::base_tool` met le `grain` du `tv_pencil` en `PencilGrain` (ordre deviné).
  - `vector::pressure_for_radius` : l'appui d'un point vectoriel retrouvé pour toute connexion de
    taille (dichotomie sur la réponse), utilisé par le rejeu des traits sans appuis.
  - Tests : `matiere_tests.rs`.
- Rendu des brosses, étape 5 : le trait vectoriel trait pour trait comme le bitmap (2026-10-18,
  `docs/VECTORIEL-COMME-BITMAP.md` §3 bis).
  - `vector::Trace` dans `Element::Stroke { trace }` (`#[serde(default)]`) : `pressure` et `time`
    par point du chemin (vides : inconnus), `feel : Option<Toucher>`, `secondary : Option<[u8; 3]>`
    (couleur B), et `cache : StrokeCache` (`#[serde(skip)]`). Le moteur les enregistre dans
    `StrokeState::path_pressure` / `path_time` (`record`, avec `cur_time`) ;
    `App::commit_vector_stroke(vs, path, pressure, time, preview)` les garde avec le réglage complet
    (`BrushLibrary::for_vector` rend toujours le réglage) et ne simplifie plus le chemin.
  - Rendu (`render_element_clipped`) : un trait est rendu seul sur une feuille
    (`render_stroke_alone` : rejeu par `StrokeState` sans lissage, toucher à courbe droite et seuil
    nul, appui de la trace ou `pressure_for_radius`, instant de la trace ou `add_point` ; sans
    réglage, `Mask::stamp` comme avant), ses pixels gardés (`CachedPixels::from_sheet` : les tuiles
    64 × 64 dessinées, creuses, RGBA prémultiplié, plafond `MAX_CACHE_TILES` = 1024) avec une
    signature (`stroke_signature` : couleur, opacité, dureté, réglage, trace), puis posés
    (`blit_cache` : copie tuile à tuile pour une translation entière, rééchantillonnage bilinéaire
    sinon ; `blit_sheet` sans cache ; toujours bornés à `bounds() + 1,5`). `StrokeCache { cell :
    Arc<RwLock<Option<Rendered>>>, xf : Affine }` : partagé entre copies (l'historique ne duplique
    rien), jamais comparé (`PartialEq` toujours vrai) ni enregistré. Budget global
    `CACHE_BUDGET_BYTES` (512 Mo) tenu par `Registry` (octets et dernier usage par cache, `Weak` ;
    au-dessus : caches des traits disparus oubliés, puis les moins récents vidés jusqu'à 90 %).
  - Aperçu / exact : `Element::transformed` (et `set_color`, `scale_width`, `invalidate_cache`)
    **vide** le cache ; `Element::transformed_preview` le garde et compose `xf` (translation
    arrondie au pixel ; un aperçu — `xf` ≠ identité — pose son cache sans vérifier la signature, et
    s'il le perd en plein geste le refait avec `xf` pour base). `Trace::sample` / `densify_traced` /
    `piece_traced` / `with_samples` : les morceaux (gomme, cutter, enveloppe, tween) gardent appuis
    et instants interpolés ; `without_samples` seulement quand ils ne peuvent pas suivre.
    `render_split` (reste / choisis depuis les caches), `render_quick` (traits lisses sans rejeu).
  - `rejeu.rs` : `Rejeu` = fil « rejeu » + `submit(cel, drawing, w, h) → gen` (copie aux caches
    vidés), `cancel` / `cancel_all` (rendu direct, annulation), `poll` (résultats encore valables),
    `wait` (essais). `App::rejeu_cel` demande, `App::poll_rejeu` (chaque image) adopte : image du
    dessin remplacée et `Element::adopt_cache` sur chaque trait, si le dessin est toujours le même
    (`same_as`). `refresh_cel*` annulent la demande du dessin ; `after_history` et `load_doc`, toutes.
  - Gestes : `vtools::select_drag` → `transformed_preview` ; `finish_vdrag` et `nudge_selection`
    → aperçu gardé + `rejeu_cel`. `envelopetools` / `perstools` : à l'ouverture, `Original::split`
    (deux feuilles : `envelope::split_rect` / `split_masked` / `render_split`) ; à chaque image,
    `envelope::warp_raster_fast(sheet, src, map)` (grille de 8 px, plus proche voisin par blocs
    couvrant l'agrandissement local) posé sur le reste (`Raster::composite_over`) ; au lâcher,
    bitmap exact (`warp_raster`, historique des tuiles contre l'original) ou géométrie vectorielle +
    `rejeu_cel`. `project::load_quick` (appli) rend les dessins texturés avec `render_quick` et les
    liste dans `Loaded::approx` ; `project::load` reste exact.
  - `project::VERSION` 24 (`appui` u16, `instants` u32 relatifs). Tests : `vecto_tests.rs`,
    `revue_tests.rs` (un essai par constat de la revue ; ignorés :
    `envelope_then_drag_is_fast_on_the_ui_thread`, `loading_a_big_project_is_measured`),
    `vector::budget_tests`.
- Peinture humide (moteur de dessin, étape 11) : `BrushSettings::mix` (« melange ») et `charge`.
  - Avec du mélange, ou une charge incomplète, l'empreinte prend un autre chemin :
    1. la moyenne pondérée des pixels courants sous l'empreinte ;
    2. la couleur de la brosse (`wet_rgb`) qui s'en approche de `mélange × couverture` ;
    3. une pose directe par `wet_blend`, en lerp prémultiplié vers la couleur de la brosse, de
       couverture `charge + (1 − charge) × couverture prise × mélange`.
  - Il n'y a pas de masque : le grain, les pochoirs et « préserver la transparence » restent
    pris en compte. L'annulation garde les tuiles d'avant, comme pour un trait ordinaire.
  - La charge décroît en `exp(−0,3 × (1 − charge) × pas)`, le pas étant compté en diamètres :
    l'épuisement suit la distance parcourue et non le nombre d'empreintes, ce qu'un test
    vérifie.
- Import Procreate (moteur de dessin, étape 10) : `procreate.rs`.
  - `read_bplist` lit une liste de propriétés binaire (« bplist00 ») : bloc final, table des
    positions, marqueurs de type, et une garde contre les boucles.
  - `keyed_root` déroule une archive NSKeyedArchiver (`$top` → `root` → `$objects`).
  - `brush_settings` convertit les clés relevées par la communauté. Les dossiers `Reset/`
    (réglages d'usine) et `Sub01/` (seconde brosse d'une double brosse) sont ignorés.
- Import Photoshop (moteur de dessin, étape 10) : `abr.rs`.
  - `Reader` lit en gros-boutiste. `sampled` lit une pointe : bornes, profondeur 8 ou 16 bits,
    brute ou PackBits par ligne.
  - `read_v12` lit les versions 1 et 2 ; `read_v6` les sections `8BIM`. Dans `samp`,
    l'identifiant est une chaîne Pascal de 37 octets, suivie d'un préambule de 10 ou 264
    octets, comme chez GIMP.
  - `descriptor` / `value` lisent le descripteur d'actions de Photoshop (Objc, VlLs, doub,
    UntF, long, bool, TEXT, enum…) ; un préréglage retrouve sa pointe par `sampledData`.
- Import Krita et MyPaint (moteur de dessin, étape 10) : `kritabrush.rs`, `zipread.rs`.
  - `kpp_text` lit le bloc PNG `preset` (zTXt, iTXt ou tEXt ; zlib par flate2), `parse_preset`
    le XML (`crate::xml`, avec les CDATA), `sensor` les capteurs, `read_gbr` les pointes GIMP
    (retournées si elles sont noires sur fond blanc).
  - `import_kpp_with` cherche une pointe en image par une fonction, pour couvrir le réglage, le
    dossier `brushes` et les paquets ; `import_bundle` passe tous les réglages d'un paquet.
  - `myb_settings` convertit un réglage MyPaint : l'appui sur le rayon logarithmique devient
    une courbe de la taille (exponentielle échantillonnée sur 8 points, ramenée au maximum).
  - `zipread::Zip` : le répertoire central, méthodes 0 et 8.
  - flate2 est maintenant une dépendance directe : la version 1.1.10 était déjà dans
    `Cargo.lock`, par image et png, donc rien n'est téléchargé.
- Import des brosses TVPaint (moteur de dessin, étape 10) : `dip.rs` et `tvpbrush.rs`.
  - `dip::read` lit une image `.dip` : blocs IFF `DGBL`, `DPEL` (ordre des composantes, alpha
    = 17), `DLOC`, `DBOD`, puis `DBAN` en différences, avec les sauts `0x80 k`.
    `dip::tip_strip` met les images côte à côte en gris de dépôt (alpha), d'où une pointe en
    plusieurs cases.
  - `tvpbrush` :
    - `read_tar` (ustar) pour `.tvpx`, `read_ini` (avec la marque UTF-8 des fichiers de
      TVPaint), `george_args` pour les commandes `tv_*` ;
    - `connection` (code de source, courbe), `custom_brush` (réglage `.ini`), `base_tool`
      (outils de base) ;
    - `import_panel` assemble les boutons du panneau en préréglages (`source` = le fichier) ;
    - `find_paper` cherche les papiers dans l'installation (`tvpaint_resources`,
      `PELURE_TVPAINT`).
  - Essais ignorés (lecture seule de l'installation) : décodage exact des pointes, import des
    panneaux Pencils, Chalks, Pens. Avec `PELURE_DIP_PNG`, ils écrivent des images à regarder.

    Décision : coût — lire les fichiers de TVPaint plutôt que demander des exports, parce que
    ses préréglages ne sortent qu'en `.tvpx`, `.bin` et `.ini`. Risque : quelques règles sont
    devinées (dites dans le compte rendu). Réversible : chaque règle est une fonction testée, à
    corriger après comparaison.
- Profil de pointe (moteur de dessin, étape 3) : `BrushSettings::profile : Option<Curve>`.
  - Pour chaque empreinte, une table de 65 valeurs (`profile_lut`), lue par `profile_coverage`,
    avec le bord anticrénelé sur un pixel (franc en pixels nets).
  - `profile_from_hardness` reproduit la dureté (plein, puis smoothstep), à quelques niveaux
    près (test).
  - `for_vector` garde le réglage d'un trait vectoriel qui a un profil, comme pour une texture.
- Débit et accumulation (moteur de dessin, étape 4) : `BrushSettings::power` (« debit ») et
  `build_up` (« accumuler »).
  - Sans accumuler, le masque du trait garde le maximum de `couverture × débit` : le modèle
    d'avant.
  - En accumulant, `masque += (1 − masque) × dépôt`, avec `dépôt = 1 − (1 − débit)^(1/n)`, où
    `n` = diamètre ÷ pas est le nombre d'empreintes qui recouvrent un point.
  - Le pixel est toujours recalculé depuis son état d'avant le trait, avec `masque × opacité` :
    l'opacité reste le plafond.
- Le montage comme banc principal (5 bis, 2026-10-17).
  - `handles.rs` : `key_at` (une clé : coupe l'exposition, ou remplit des images vides jusqu'au
    dessin suivant ou la fin du plan ; refusée là où un dessin commence, `key_start_taken`),
    `App::montage_new_key` (une étape d'annulation, plan, calque et image courants).
  - `runs` (les expositions bout à bout, plages vides comprises), `reorder_slot` (le rang où
    tombe le dessin : les autres expositions dont le milieu est avant le sien) et `reorder_run`
    (la suite refaite, chacune gardant sa durée, les rôles suivant leurs images) :
    `Handle::Key`. `move_boundary` (la limite entre deux expositions déplacée, chacune gardant
    au moins une image si c'est un dessin, le rôle de début suivant) : `Handle::Boundary`.
  - Clés choisies et leurs commandes : `montagekeys.rs`. `KeyRef` (plan, calque, début de
    l'exposition), `App::key_sel` et `key_anchor` ; `choose` (pur, testé) applique un clic, Ctrl
    ou Maj ; `keys_between` suit la piste (`TrackKey`) d'un plan à l'autre. `copy_keys`
    (`KeyClip` : écarts en images du montage, durées, dessins copiés), `paste_keys`
    (`overwrite_run`), `delete_keys(gap)` (`remove_run`, de la dernière à la première),
    `duplicate_keys` (`duplicate_run`) visent les clés choisies, sinon celle de l'image
    courante. `App::key_focus` (un clic sur la piste d'un calque du montage) passe avant la main
    des plans dans `run_action`, et `key_mode_of` donne alors les raccourcis de l'animation.
  - Outils à ressort : `springtools.rs`. `handle_shortcuts` appelle `spring_after` quand la
    touche d'une action `outil.*` change d'outil ; `update_springs` (avant les raccourcis, à
    chaque image d'écran) lit les touches relâchées et Ctrl ; `back_on_release` (testée) décide
    du retour ; `spring_used` (appui sur le dessin). Ctrl seul : `CtrlLasso`, gâché par une
    autre touche ou la molette, jamais au milieu d'un trait. Actions `dessin.nouvelle_cle` (F5)
    et `dessin.nouvelle_cle_vide` (F4) : `montage_new_key(…, copy)` à l'image courante ; avec
    `copy`, la clé est un `duplicate_cel` de `handles::key_source` (le dessin montré, sinon le
    dernier d'avant, sinon le premier d'après ; pur, testé). `panneau.couper` : F6.
  - Choix au rectangle : `montagekeys::KeyRect` (coin de départ, choix d'avant, ajout),
    `keys_in_rect` / `drag_key_rect` ; `App::key_rect` connu de `dragging`, `cancel_drags`
    (rend le choix d'avant) et `drop_stale_drags`.
  - Zoom de la bande : `montage_zoom_bar` (petite barre en bas de `montage_panel`), actions
    `bande.zoom_avant` / `bande.zoom_arriere` (`App::zoom_strip` : `montage_zoom` ou
    `timeline_cell` selon le mode) et `bande.tout_voir` (`montage_fit`, appliqué dans
    `montage_strip` qui connaît la largeur ; en animation, `fit_timeline`) ; Ctrl + molette sur
    la bande (`zoom_delta`).
  - Repère de pose : rien ne bouge pendant le glisser d'une clé (`Handle::Key` garde son décalage
    dans `App::key_drag_delta`, posée par `end_handle_drag`) ou de plusieurs (`KeysDrag`, posées
    par `end_keys_drag`). `handles::drop_marks` (la limite où chaque bloc déplacé viendra, vue
    dans le calque tel qu'il est ; partage `shifted_units` avec `shift_runs`) ;
    `montagekeys::drop_view` → `DropCell` (fantômes, repères) ; `montagerows::paint_drop`.
  - Glisser plusieurs clés : `handles::shift_runs` (blocs de clés choisies qui se suivent, placés
    par leur milieu déplacé ; une seule clé est un bloc d'une clé) ; `montagekeys::
    KeysDrag` garde chaque calque touché tel qu'avant (Échap, `cancel_keys_drag`) et l'étape
    d'annulation (`end_keys_drag`, seulement si quelque chose a bougé) ; `dragging`,
    `cancel_drags` et `drop_stale_drags` le connaissent.
  - Ordre des calques propre à chaque plan (2026-10-17, choix d'Anthony) : `Rows::count` (le plan
    qui a le plus de calques) et `layer_in_row` (la rangée r d'un plan : son r-ième calque en
    partant du haut) remplacent les pistes par nom (`layer_tracks` n'est plus). `row_at`,
    `layer_row_hit`, `montage_layer_handle_at`, `keys_between` (Maj + clic) et `keys_in_rect`
    (rectangle) vont par rangée. `TrackKey`, `track_of` et `layer_on_track` (par nom) ne servent
    plus qu'au collage sur plusieurs calques (`ensure_tracks`).
  - Piste « Fond » : les calques de toute la scène ont leur rangée comme les autres (la rangée
    « Fond » et `Rows::background` ne sont plus) ; `sync_scene_layers` copie
    les images du modèle (coupées à la durée du panneau, le dernier dessin tenu s'il est plus
    court) au lieu d'un dessin tenu.
  - `montagerows::cell_handle` : `Boundary` sur une limite intérieure, `Key` au milieu d'un
    dessin (le montage ne donne plus `Exposure`, que garde la timeline) ; `LAYER_H` = 40.
    `montage_strip` : double-clic sur une piste de calque, entrée « Nouveau dessin ici » du
    menu, bouton « Nouveau calque » de la barre.
  - Colonne de gauche étirable (2026-10-17, demande d'Anthony) : `montagehead.rs`.
    `App::montage_head_w` (lu et écrit dans `Settings::montage_head_w`,
    `largeur_entetes_montage`, borné par `clamp_head_w` entre `HEAD_MIN` = `HEADERS_W` et
    `HEAD_MAX`) donne sa largeur à `track_headers` ; `row_headers` en tire celle des étiquettes.
    `head_width_grip` (après la colonne, pour rester au-dessus) : tirer, double-clic
    (`HEAD_ALL` / `HEAD_MIN`), enregistrement au lâcher.
  - Rangée d'un calque : `head_layout` (pur, testé) place le nom, jusqu'à 3 icônes (afficher,
    verrouiller, pelure), le menu ⋮, puis dessous l'opacité, le mode de fusion et le
    post-comportement, selon la largeur. `montage_layer_head` les dessine dans une zone à part
    (`new_child`) : les `ui.put` des icônes ne doivent pas faire avancer la mise en page, sinon la
    bande recouvre la fin de la colonne. Il réutilise `layerheads` : `opacity_bar` (identifiée par
    un `egui::Id`), `eye_button`, `lock_button`, `icon_at`, `blend_combo` (sorti de `layer_head`).
  - Clic droit sur la rangée, ⋮, ou « Calque » au clic droit d'une piste : `montage_layer_menu`
    (réglages, opacité, mode, `behaviour_items` sorti de `layer_head_menu`, groupe,
    `layer_menu_where(…, in_montage)`, CTG, vidéo, propagation, puis `layer_ops_menu`).
    `LayerRename::in_montage` : le renommage se fait dans la colonne (double-clic sur le nom).
  - `paint_layer_cell` montre le pré / post-comportement (boucle, va-et-vient, maintenir) en
    pâle avant le premier dessin et après le dernier, avec `behaviour_icon`.
  - Nouveau dessin : `ensure_cel` prend ses images de `images::new_drawing_span` (pur, testé) :
    en storyboard, tout le plan si le calque n'y a aucun dessin.
  - Clé précédente / suivante : `montagekeys::key_starts` (débuts de dessins de tous les
    calques d'un plan, sauf dossiers et vidéos) et `step_key` (purs, testés) ;
    `App::goto_key` (montage entier en storyboard, plan en animation) ; actions `cle.*`.
  - Timeline : `on_playhead` (la case de la tête de lecture dans la règle) passe avant
    `timeline_handle_at` au début d'un glisser et pour le curseur.
  - Peint derrière / remplir la forme : `brushfill.rs` (purs, testés : `closed_outline` — bout
    près du début, ou boucle d'un trait qui se recoupe —, `paint_under`, `over_premul`,
    `fill_under_stroke`). `StrokeState::with_options(behind, fill_closed, close_tol)` : en
    peinture normale `paint_under` ; en humide, la peinture sous le pixel ; `fill_shape` au début
    de `finish` (le remplissage s'insère entre le trait, de couverture connue par son masque, et
    le dessin d'avant ; tuiles ajoutées à l'annulation du trait). En vectoriel,
    `commit_vector_stroke` insère un `Element::Fill` sous le trait (les deux à l'indice 0 s'il
    peint derrière) et refait la zone. `App::paint_behind`, `fill_shapes` (retenus dans
    `Settings`, enregistrés avec la pelure quand ils changent), `close_tolerance` (15 points
    d'écran) ; actions `pinceau.derriere`, `pinceau.remplir` (sans touche).
  - Enveloppe de Bézier : `envelope.rs` (pur, testé : `Envelope` = coins + 8 poignées, `point`,
    surface de Coons ; `warp_raster` par maillage de triangles et `persp::sample` ; `warp_drawing`
    densifie puis `persp::warp_element_with`) ; `envelopetools.rs` (`App::envelope`, comme
    `perstools` : `envelope_press` avant `transform_press`, `_drag`, `_release`, `_key`,
    `check_envelope`, `paint_envelope_overlay`) ; aussi dans l'outil Sélection (`envelope_press` avant
    `select_press`) : `Original::Vector` avec les éléments choisis, `Original::Masked` (zone bitmap,
    `envelope::warp_raster_masked`, `rasterize` partagé). `springtools` : pas de lasso de Ctrl avec
    Transformer.
  - Aller à un plan (`aller.rs`) : `find_plan` (pur, testé : nom du plan, de préférence dans la
    séquence courante, puis « séquence/plan », puis le rang), fenêtre `goto_window` (`App::goto_plan`).
    `Pending::Close` (Ctrl+W) ; `dockui::show_pane` (ouvert, onglet devant).
  - Retourner (`flip.rs`) : `mirror_transform_x` (miroir de la feuille : angle opposé, largeur
    négative, translation recalée sur le centre de la feuille), `flipped_in_place`, `mirror_camera_x`,
    purs et testés ; `LayerTransform::clamp_stretch` garde le signe.
  - Transformer (`layertools.rs`) : `BoxHandle` (déplacer, coins, milieux des côtés, rotation) ;
    `stretched` (pur, testé) change `stretch_x` / `stretch_y` et recale `x`, `y` pour que le point
    opposé garde sa place ; `LayerTransform::affine` = rotation · échelle · étirement.
  - Cadence de la propagation : `propager::Cadence`, `made_frames` (pur, testé : images gardées),
    `held_frames`, `guide_keys` (images où un calque du guide change ; aucune pour une vidéo),
    `hold` (après `apply` : les images tenues reprennent le dessin d'avant). `propagerui` calcule
    tout, ne garde que `made_frames` (`Job::computed` pour la progression).
  - Mémoire de la propagation : `Job::made` tient des `Raster` (`to_raster` sur le fil de la
    tâche, `NOISE`), `Job::latest` la vignette ; `propager::apply_rasters` les pose (`apply`, sur
    des `Image`, ne sert plus qu'aux essais). Dans la synthèse, `from_first` tient des `Sparse`.
  - Journal de plantage (`plantage.rs`) : `install` (dans `lancer`) garde le crochet de panic
    par défaut puis écrit `report_text` (message, endroit, fil, pile `Backtrace::force_capture`) par
    `write_report` dans `%LOCALAPPDATA%\Pelure\plantages\plantage-<s>.txt` et `nouveau.txt` ; au
    démarrage, `take_new` le signale une fois dans la barre d'état. Erreurs de la carte de
    l'affichage : `on_uncaptured_error` / `set_device_lost_callback` (`App::new`) appellent
    `note_gpu_error` (un rapport par séance) au lieu du plantage par défaut de wgpu. Une erreur d'allocation (manque
    de mémoire) arrête le programme sans passer par là.
  - Code couleur des calques : `theme::couleur_calque(kind, ctg)` (vif : repères, barres, bande et
    étiquette du panneau Calques, lettre de l'en-tête de la timeline) et `fond_calque` (sourd :
    dessins sous les panneaux du montage) ; `DESSIN_CTG` / `CALQUE_CTG` roses. Tous les endroits
    qui choisissaient la couleur d'après `layer.kind` passent par eux.
  - Image du presse-papiers (`pasteimage.rs`) : `run_action("edition.coller")` commence par
    `clipboard_image` (arboard : rien si le texte commence par `MARK`, « Pelure : », écrit par les
    copies de Pelure) ; `paste_clipboard_image` : calque bitmap modifiable ou `add_layer`, `fit_size`
    au cadre (`imageops::resize`), `to_floating` (`raster::premultiply` ; sans aucune opacité :
    opaque), puis `paste_floating` (le corps de `bitmap_paste`). `handle_shortcuts` : egui-winit
    garde Ctrl+V sans texte dans le presse-papiers ; le relâché de V avec Ctrl, sans événement
    `Paste` depuis l'appui (`App::paste_event`), compte comme Ctrl+V.
  - Cutter (`cutter.rs`) : `cut_strokes` (pur, testé) : traits que le geste croise (ou, pour un
    clic, le plus proche à `tol` près, `nearest`), coupés aux `crossings` avec les autres traits et
    avec eux-mêmes (segments non voisins) ; les morceaux (`piece`, entre deux coupes) qui
    contiennent un point touché partent. Endroits = indice du segment + fraction. Tout est calculé
    sur les traits d'avant, les morceaux gardés remplacent le trait à sa place dans l'ordre.
    `App::cut_gesture` (points du dessin), `cutter_press` / `cutter_drag` / `finish_cut` (une
    étape d'annulation `push_vector`).
  - Propagation dans les menus : `montagehead::propagation_item` (montage, timeline, panneau
    Calques, menu Calques) montre toujours l'entrée ; grisée avec la raison (vidéo, pas bitmap,
    rien à suivre d'après `propager::guides`), sinon `propagation_menu` de `propagerui` pour le
    calque courant ou, pour un autre calque, `select_layer` puis `calque.propager`.
  - Barre d'outils (`toolbars.rs`) : `TOOL_GROUPS` = groupes de cases ; `slot_tool` (pur, testé)
    donne l'outil montré (courant, sinon le plus récent de `App::tool_recent`, sinon le premier).
    `tool_slot` ouvre `App::tool_flyout` (groupe, case, rectangle) quand le bouton est tenu
    `FLYOUT_HOLD` ; `tool_flyout_ui` (une `Area` au premier plan) prend un outil au clic ou au
    relâché dessus, se ferme sur un appui ailleurs ou Échap. Le relâché du clic maintenu n'est
    pas un choix (`clicked` ignoré tant que la liste est ouverte).
  - Sélection par couleur (`colorsel.rs`) : `Tool::SelectColor`, `Tool::is_select` (la zone
    bitmap et les poignées restent avec les deux outils). Bitmap : `fill::flood` (voisins) ou
    `colorsel::similar` sur le dessin du calque, `PixelMask::from_bools`, contours par
    `vector::trace_contours` ; vectoriel : `same_color` (`Element::color`). Une poignée sous le
    pointeur passe la main à `select_press`.
  - Commandes de la vue : `viewbar::view_tools_bar` (`TOOLS_BAR_H`), sous le dessin dans
    `canvas_panel`, avec `toolbars::command_button` (les mêmes boutons que la barre du haut, qui ne
    les a plus) ; en 3D, seulement planche / caméra / 3D ; `Pane::View` reste pour lire les anciennes dispositions, `sanitized` l'enlève
    (comme `Timeline`), il n'est plus dans `Pane::ALL` ni `SETTINGS_TABS`. Guides : au clic,
    `begin_paint` d'abord (il clôt le trait d'avant, ce qui vide `guides.stroke`), puis
    `begin_guided_stroke`, seulement si un trait ou un gommage a commencé.
  - Panneaux rétrécis : `dockui::pane_ui` appelle `fit_to_pane` pour les panneaux de côté
    (`slider_width`, `combo_width` d'après la largeur) ; `fit_w(ui, max)` pour les listes et
    champs à largeur fixe ; `theme::dock` : `show_scroll_bar_on_overflow` pour les onglets.
  - `montage_panel` dessine la colonne de gauche dans un `new_child` à elle : chaque
    `new_child` / `ui.put` fait avancer le compteur d'identifiants automatiques de la zone qui
    le contient, et la bande (`allocate_painter`, identifiant automatique) changeait d'identité
    quand le plan courant n'avait pas autant de calques, ce qui coupait le glisser en cours.
  - Flèches : `nudge.rs`. `arrow_step` (pur, testé : 1 px, `BIG_STEP` avec Maj, rien avec Ctrl
    ou Alt), `App::nudge_ready` (traits choisis quel que soit l'outil, ou zone bitmap avec
    l'outil Sélection ; le dessin a la main ; `update_springs` garde l'outil Sélection au relâché
    de Ctrl quand une zone bitmap est choisie),
    `nudge_selection` : en vectoriel, éléments translatés et `gesture_change(GESTURE, …)` (une
    étape par flèche tenue, gardée vivante par `keep_gesture` dans `handle_shortcuts`) ; en
    bitmap, `lift_selection` puis `place_floating` (étape au dépôt).
- Pistes du montage modifiables (chantier 5).
  - `montagerows.rs` : `TrackKey { name, nth }` et `layer_tracks` remplacent la rangée par rang
    (`layer_in_row`, 8 au plus). Les plans sont parcourus dans l'ordre, chacun du haut de sa
    pile : un nom nouveau s'insère sous la piste du calque qui le précède dans son plan.
    `layer_on_track` retrouve le calque d'un plan sans rien allouer (appelé pour chaque case à
    l'écran).
  - `montage_layer_handle_at` / `cell_handle` : début, fin, barre, bord d'exposition, avec les
    mêmes `Handle` que la timeline ; le geste passe par `begin_handle_drag` / `drag_handle` /
    `end_handle_drag` (plan d'avant, une étape d'annulation, Échap). Le montage donne
    maintenant à `drag_handle` le bord d'image le plus proche (arrondi), pas l'image sous la
    souris.
  - `paint_layer_cell` dessine la barre (`BAR_H`) et éclaire la poignée survolée ou tenue
    (`lit`). `LAYER_H` passe de 18 à 20 points.

    Décision : coût — les pistes par nom réutilisent les poignées de la timeline telles
    quelles. Risque : deux plans dont les calques ont des noms différents donnent des pistes
    distinctes (c'est voulu). Réversible : l'ordre des pistes n'est pas enregistré, il se
    recalcule.
- Import Storyboard Pro corrigé après l'essai d'Anthony (2026-10-17).
  - `sbscript.rs` : `pelureTraits` désigne le dessin par `{elementId, exposure: nom}` passé par
    `Drawing.Key`, jamais par `drawingId` ; il lit `Drawing.query.getData` et écrit
    `{"format": "getData", box, arts}`. Les couleurs vont une fois dans `couleurs.json`
    (`COLORS`), relu par `read_manifest` dans `Drawings::palette`.
  - `sbvector.rs` : `palette` lit les couleurs simples (`[r,g,b,a]` ou `{r,g,b,a}`) et les
    textures (`texture_color` : PNG en base64 décodé par `kritabrush::base64`, moyenne pondérée
    par l'alpha, opacité = couverture). `raw_elements(json, palette)` lit les deux formats ; en
    `getData`, les zones d'un calque sont rassemblées par couleur en un remplissage pair-impair
    (calques plans), puis les traits de crayon (`pencil`, partagé avec l'ancien format).
  - `sboard.rs` : `sequences_exist` lit la méta `sequenceExists` ; vide, `sequenceName` est
    ignoré ; absente, il est gardé.

    Décision : coût — un seul appel `getData` par dessin au lieu de quatre `getStrokes` et d'une
    lecture de couleur par identifiant. Risque : la couleur moyenne d'une texture n'est qu'une
    approche du grain (l'image du calque reste exacte s'il n'est pas gardé en vecteurs).
    Réversible : l'ancien format se lit toujours.
- Timeline et montage ensemble (chantier 6).
  - `Pane::Strip` est devenu `Pane::Montage` (nom enregistré inchangé, « bande »). Une
    `Pane::Timeline` à part a existé le 2026-10-17 (versions 4 et 5 de la disposition : l'un sous
    l'autre, puis en onglets) ; retirée le même jour à la demande d'Anthony, elle n'est gardée que
    pour lire ces dispositions : `sanitized` l'enlève, `upgraded` remet le montage s'il manquait,
    `strips_as_tabs` rend à la bande sa hauteur de 290 points. `pane_ui` montre dans la bande le
    montage en storyboard, la timeline en animation (version 6, comme avant le chantier).
  - `pane_ui` montre chacun dans les deux modes. `panels_have_focus` et `images_have_focus` ne
    regardent plus le mode : la main est au dernier des deux cliqués (chaque clic retire l'autre).
    `focus_mark` trace un trait en haut du panneau qui a la main.
  - `toggle_play_scope(montage)` : le bouton du montage lit le film, celui de la timeline le
    plan ; `toggle_play` (la touche) suit le mode des raccourcis.
  - Les raccourcis suivent le panneau qui a la main (choix d'Anthony) : `key_mode_of(mode,
    panel_focus, images_focus)` (testée) donne le mode des raccourcis, storyboard si le montage
    ou la planche a la main, animation si c'est la timeline, sinon le mode du dessin.
    `handle_shortcuts` cherche les touches dans ce mode ; dans `run_action`, `board` (flèches,
    suppression d'un extrait son) le suit aussi, tandis que `board_shown` (Tab, vue Vignettes)
    reste le mode du dessin. `set_mode` rend la main au dessin.
  - `follow_frame` est pris une fois par image d'écran dans `following` au début de
    `show_dock` : les deux bandes amènent l'image courante en vue.

    Décision : coût — deux panneaux du dock plutôt qu'une bande à deux étages. Risque : la place
    du dessin diminue (la bande du bas passe de 290 à 420 points de référence). Réversible : chaque
    panneau se ferme ou se détache, et la disposition se retient.
- Poignées de calque, fin de plan, Échap, choix au rectangle (chantiers 1 à 4 de l'audit).
  - Nouvelles poignées : `Handle::LayerStart`, `LayerEnd`, `LayerMove` (la barre dessinée sous les
    dessins, `BAR_H`) et `PlanEnd` (prise au bout de la règle). Leurs fonctions pures sont testées :
    `content_bounds`, `set_layer_start`, `set_layer_end`, `move_layer` (par `Layer::insert` /
    `shift_frames`, rôles et clés d'effets suivent).
  - Chaque mouvement repart du plan d'avant le geste.
  - `put_layer` garde au plan sa durée d'avant : `length` est relevé si le calque raccourci
    était le plus long.
  - Un geste sans effet rend le plan exactement comme avant.

    Décision : coût — une borne simple, la fin du plan, plutôt qu'un mode « décaler la suite du
    montage ». Risque : allonger un calque au-delà demande d'allonger d'abord le plan. Réversible :
    une touche pourra lever la borne.
  - `App::dragging` réunit tous les glissers. `handle_shortcuts` les teste avant tout autre
    raccourci : Échap, ou la touche liée à `edition.annuler`, appelle `cancel_drags`, les autres
    touches attendent. `drop_stale_drags`, après l'interface, oublie un glisser dont la vue
    n'est plus montrée, bouton relâché : sinon il couperait les raccourcis.
  - Choix au rectangle : `GridSelect` garde la case du départ et le choix d'avant (rendu par
    Échap). Il remplit `frame_sel` et `layer_sel` avec les lignes couvertes, sans les dossiers. Le
    défilement à la souris reste dans la règle.
  - `sound_shift_note` dit si changer la durée d'un plan décale les plans suivants par rapport
    au son. Elle sert à la poignée de fin, au `Trim` du montage et à `watch_plan_length`, qui
    compare la durée du plan courant d'une image d'écran à l'autre ; l'annulation est exclue.
    Un geste de glisser seul : les clics restent aux boutons.
- Taille de l'image (passe interface, Fichier > Réglages du projet, onglet Résolution).
  `Document::resize_frame(w, h, scale)` fait l'une de deux choses :
  - avec `scale` : `scale_stage(s)`, où `s` est le plus grand des deux rapports (le dessin
    remplit le cadre). Il multiplie les dessins bitmap (`Raster::scaled` : seul le rectangle des
    tuiles dessinées est recalculé, filtre triangle en réduction, Catmull-Rom en agrandissement),
    les éléments vectoriels (`Affine::scale_about`, épaisseurs comprises), les guides, les clés
    de caméra (position, avancée) et le placement des calques ;
  - sans `scale` : seul le cadre change. Puis la feuille est agrandie (`enlarge_stage`, centrée)
    pour rester au moins le double du cadre.

  Décision : coût — ne pas passer par l'historique (tous les dessins changent) ;
  `App::after_stage_change` vide l'historique et oublie ce qui pointait dans l'ancienne feuille.
  Risque : pas de Ctrl+Z ; la fenêtre le dit avant « Appliquer ». Restent à leur taille : les
  images d'un calque vidéo (lues à la taille de l'import), le rayon des flous et la fermeture des
  trous du CTG (en pixels). Réversible : on peut revenir à l'ancienne taille, au prix d'un
  rééchantillonnage.
- Export vers Storyboard Pro (14b-1).
  - `sbexport::prepare` décrit ce qui part (`SbOut`) : plans → plans, images de storyboard
    (`board::plan_boards`) → panneaux avec leurs textes, calques sans les dossiers (visibilité et
    opacité du dossier reportées), extraits de son. Il note ce qui n'est pas repris.
  - `SbExportJob` calcule les images comme l'export d'images : quelques-unes par image d'écran
    (le projet est figé pendant ce temps, `modal_open`), PNG écrits par des fils. Chaque image est
    `render::layer_plan_at` (le calque seul, mode normal, pleine opacité, même caché) vu par
    `doc.default_camera()`. Les images vides sont sautées ; deux images identiques (même hachage)
    ne font qu'un fichier.
  - Le script : les données en littéral JavaScript (`serde_json`, U+2028/2029 échappés pour
    ECMAScript 3), puis des fonctions documentées seulement. Chaque calque créé prend d'abord un
    nom provisoire (`pelure~rang`), retrouvé en comparant les listes de noms avant et après.
    Ainsi rien ne suppose où Storyboard Pro range un calque neuf. Les calques vides d'origine du
    panneau sont enlevés ensuite. Un nom de plan déjà pris par le modèle passe par un nom
    provisoire, rétabli après la suppression des plans du modèle.
  - `sbbatch` est devenu générique : `Batch<T>::spawn` (travail sur un autre fil) et `launch`
    (Storyboard Pro en batch, journal, arrêt). L'avancement vient de `avancement.txt`, le
    résultat de `resultat.txt` (`parse_result`).
  - Réglages : `modele_storyboard_pro` et `dossier_export_storyboard_pro` dans `reglages.toml`.
    `sboard::read_template` lit un modèle sans panneau (l'import le refuse toujours).

    Décision : coût — des images plutôt que des vecteurs pour commencer, parce que l'import d'image
    est documenté et que l'écriture de traits ne l'est que pour Harmony. Risque : calques non
    modifiables en traits dans Storyboard Pro. Réversible : 14b-2 remplacera l'image par les
    traits, calque par calque, sans changer le reste.
- Ouverture automatique et dessins vectoriels de Storyboard Pro (14a-2).
  - `sbbatch` : `find_storyboard_pro` cherche l'installation la plus récente. `copy_project`
    copie le projet sans `logs/` ni `.lock`. `Batch::start` lance, sur un autre fil,
    `StoryboardPro -scene … -batch -compile …` dans un dossier temporaire, avec le journal
    (sortie) et l'arrêt possible.
  - `PELURE_SANS_STORYBOARD_PRO` le désactive (essais), `PELURE_STORYBOARD_PRO` désigne
    l'exécutable.
  - `sbvector` lit le JSON des traits. Bézier : 0, 1 ou 2 points de contrôle par segment. Une
    zone = les traits dont un seul côté porte sa couleur, mis bout à bout par leurs jointures ; en
    pair-impair, l'appariement aux jointures ne change pas la zone.
  - Calage (`fit_to_image`) : boîtes du dessin et des pixels opaques de l'image, échelle commune
    (proportions à 20 % près), axe vertical essayé dans les deux sens. On garde le sens au meilleur
    recouvrement (intersection sur union), et les vecteurs seulement à 60 % ou plus.

    Décision : coût — caler sur l'image plutôt que convertir les unités Toon Boom (caméra du
    panneau, placement du calque, champs) qu'on ne peut pas vérifier ici. Risque : un calque
    tourné ou très déformé reste en image. Réversible : on pourra ajouter la conversion exacte
    plus tard.
- Import Storyboard Pro (14a) :
  - `xml.rs` lit un document XML en arbre (`Element` : nom, attributs décodés, enfants), avec
    quick-xml. quick-xml était déjà dans `Cargo.lock`, par wayland : rien n'est téléchargé.
  - `sboard.rs` en tire un `SbProject`. Plans dans l'ordre du film (colonne de type 0 de la
    scène racine, `warpSeq`), panneaux, légendes (`caption_text` : le HTML de Qt devient du texte),
    modules `READ` = calques, pistes son (`soundSequence`, effets `WSOLATempo` et `Resample`).
  - `sbscript.rs` écrit le script d'export (ECMAScript 3, fonctions documentées de l'interface de
    script de Storyboard Pro seulement) et relit sa liste `pelure.txt`, en lignes à tabulations :
    le moteur de script n'a pas de JSON.
  - `sbimport::build` construit des panneaux à l'ancienne (un par panneau de Storyboard Pro, plan
    = scène), puis appelle `Document::into_plans` (13a). Les panneaux mis bout à bout, les marques
    avec leurs textes et les calques réunis par nom viennent ainsi du code déjà testé de 13a.
  - Panneaux du script ↔ panneaux du `.sboard` : par identifiant, sinon par rang.
  - Une image identique (même contenu) ne fait qu'un dessin. Un calque de fond répété d'un panneau
    à l'autre est donc tenu, pas copié.
  - Son : un extrait simple garde le son d'origine (début arrondi à l'image). Un extrait à effet
    ou à volume devient son propre son, en WAV : `stretch::resample` (vitesse et hauteur), puis
    `stretch::wsola` (tempo seul), puis `stretch::gain`.

    Décision : coût — calculer une fois à l'import plutôt qu'ajouter vitesse et volume par
    extrait au modèle, au mixeur et au format. Risque : ces extraits ne se rallongent plus au-delà
    de ce qui a été calculé. Réversible : un vrai réglage par extrait pourra venir ensuite.
  - `sbui.rs` : la fenêtre d'import. L'import tourne sur un autre fil (`Document` est `Send`) et
    passe par `Pending::SbImport` (« enregistrer ? »). Un `.sboard` ou un `pelure.txt` lâché sur la
    fenêtre y est pris.
- Vue Projet (`projetui.rs`, 13e-4) : `Document::storyboard_locked` est dans `Structure` (une
  annulation le rétablit avec le reste ; le basculer est une étape). `App::storyboard_refuses`
  garde `structure_op` et les autres chemins qui changent l'ordre, le nombre ou la durée des
  plans (suppression, durée, glissés du montage et de la planche) ; `App::content_refuses`,
  appelé par `layer_editable`, `refuse_on_folder` et les commandes d'images et de calques, garde
  le contenu (choix d'Anthony : on ne dessine pas sous le cadenas). Les
  propriétés aussi sont dans `Structure` (champs suivis par `edited`, une étape par geste) ;
  `modified` est posé par `save_to`, `created` par `Document::created_now` (projet neuf). Les
  dates s'affichent en temps universel (`civil_from_days`) : l'heure locale demanderait un appel
  au système, sans dépendance ni code `unsafe` aujourd'hui. `Document::from_plans` construit le
  projet des plans choisis par `from_parts` (mêmes id de plans, calques et dessins) ;
  `Pending::FromChosen` le fait passer par « enregistrer ? ».
- Marques de couleur (`marks.rs`, 13d) : `Layer::color_marks` (dessin → couleur), comme les
  noms d'instance (elles suivent les copies via `relink_copies`) ; `marked_starts`,
  `step_mark`.

## Couleur (passe couleur, demandée par Anthony le 2026-10-16)

- Modules :
  - `couleur.rs` (sans interface, testé dans `couleur_tests.rs`) : RVB ↔ TSV (venues de
    `colorui.rs`, qui les réexporte), TSV ↔ TSL en nombres (la teinte reste telle quelle), Lab
    (D50) → RVB, code hexadécimal (#RRGGBB, #RGB), `hsv_following` (un gris ou le noir garde la
    teinte qu'on réglait), couleurs récentes (`remember` : en tête, sans doublon, 24 au plus),
    `ColorSettings` (table `[couleurs]` de `reglages.toml`).
  - `roue.rs` (testé dans `roue_tests.rs`) : la roue posée à l'écran (`Wheel` : centre, rayons de
    l'anneau). Teinte 0 en haut, sens des aiguilles d'une montre. Carré : saturation de gauche à
    droite, valeur de bas en haut. Triangle : sommets teinte pure (pointée vers sa place sur
    l'anneau), blanc à +120°, noir à +240° ; point = noir + v (blanc − noir) + v s (teinte −
    blanc), inversé par un système 2 × 2 puis borné. `zone` : anneau, intérieur ou rien.
    Maillages colorés : anneau en 96 morceaux (multiple de 6 : les teintes pures sur des
    sommets), carré et triangle en 24 × 24, chaque sommet avec sa couleur exacte.
  - `nuancier.rs` (testé dans `nuancier_tests.rs`) : `Palette` (nom, `Swatch` : couleur, nom),
    opérations sur la liste (créer, dupliquer, supprimer en gardant au moins un nuancier,
    renommer sans doublon), `move_item` (fentes comptées avant le déplacement), `drop_slot`
    (pastille la plus proche, moitié gauche ou droite), formats et fichiers.
  - `colorui.rs` : `ColorState` (dans `App::couleurs`), les deux carrés, `wheel_ui` (testée
    dans egui sans fenêtre : clic, glissés, clic droit), curseurs (`Channel`, `channel_rows`),
    opacité, couleurs récentes, commandes `couleur.*` et `pastille.avant_plan` /
    `pastille.arriere_plan`, enregistrement (`color_tick`), fenêtre de la barre d'outils, menu
    Édition > Couleur.
  - `nuancierui.rs` : le nuancier montré, ses commandes (`nuancier.*`, `pastille.renommer` /
    `remplacer` / `retirer`), les noms en cours de saisie, la confirmation de suppression,
    l'import et l'export (`rfd`).
- Avant-plan = `App::color`, inchangé : tous les outils le lisaient déjà. `App::hsv` garde sa
  teinte, et ne suit `color` que si elle a changé ailleurs (`sync_hsv`). L'arrière-plan
  (`prefs.back`, et sa TSV `back_hsv`) et le reste sont dans `App::couleurs` ; l'échange échange
  aussi les TSV. La roue et les curseurs sont redessinés à chaque image d'après `App::hsv` : ils
  suivent la pipette, le nuancier, le code, le texte rouvert.
- Roue : la partie visée est retenue à l'appui dans les données d'egui (clé de la roue), pour
  tout le geste ; seul le bouton gauche change la couleur (le droit ouvre le menu).
- Opacité : la couleur reste RVB (`StrokeMode::Paint([u8; 3])`, éléments vectoriels). La ligne
  Opacité règle `BrushSettings::opacity` du pinceau ou du crayon : la même valeur que leurs
  réglages, sans toucher au moteur de pinceau.
- Couleurs récentes : `App::color_used`, appelé au début d'un trait de pinceau ou de crayon
  (`begin_paint`), d'un remplissage (`fill_at`), à la pose d'une forme (`commit_shape_points`)
  et d'un texte (`finish_text_edit`). Les notes de la planche n'y passent pas.
- Pastille visée (commandes `pastille.*`) : celle dont le menu est ouvert, sinon celle sous la
  souris, vues à l'image précédente (`ColorState::menu` / `hovered`, renouvelés par
  `color_tick` en fin d'image ; un panneau fermé n'en laisse donc aucune). Un raccourci agit
  ainsi sur la pastille survolée. Retirer et nommer prennent, à défaut, la pastille de la
  couleur d'avant-plan (c'est ce que fait le bouton −).
- Glisser une pastille : `ColorState::drag` = (grille où le geste a commencé, rang). Le panneau
  et la fenêtre de la barre d'outils ont chacun une grille ; chacune ne suit que ses glissés.
- Enregistrement : `color_tick` recopie l'avant-plan dans `prefs`, compare à `Settings::colors`
  et écrit `reglages.toml` quand ils diffèrent, hors glissé et hors saisie (comme la pelure).
  `ColorSettings::sanitized` répare un fichier relu (au moins un nuancier, rang montré valide).
  Ordre du TOML : valeurs simples (`avant_plan`, `arriere_plan`, `roue_visible`, `roue`,
  `curseurs`, `recentes`, `nuancier_affiche`) puis `[[couleurs.nuanciers]]` et leurs
  `[[couleurs.nuanciers.pastilles]]`. L'ancien nuancier (`App::swatches`, jamais enregistré)
  devient le nuancier « Pelure », nommé couleur par couleur.
- Annulation : les nuanciers sont des réglages de l'application, comme les espaces de travail ;
  ils ne passent pas par l'historique. Supprimer un nuancier demande donc une confirmation.
- Format du projet : inchangé (version 21) ; aucune couleur de ce panneau n'y entre.
- Formats de nuancier :
  - GIMP `.gpl` (texte) : `GIMP Palette`, `Name:`, `Columns:`, commentaires `#`, puis une ligne
    `R G B<tab>nom` par couleur. Lecture tolérante : une colonne de plus après une tabulation
    est ignorée, `Untitled` compte comme pas de nom, une ligne illisible est comptée et sautée.
    Écrit comme GIMP (une couleur sans nom : rien après les chiffres).
  - Photoshop `.aco`, d'après la spécification d'Adobe (« Photoshop File Formats
    Specification », section « Color Swatches ») : entiers 16 bits, octet de poids fort
    d'abord ; version 1 (espace et quatre valeurs par couleur), suivie de la version 2 (les
    mêmes, plus le nom en UTF-16, longueur sur 32 bits, zéro final compris). Espaces lus : RVB,
    TSV, CMJN (0 = 100 % d'encre ; produit simple, sans profil), Lab (L en centièmes, a et b
    signés en centièmes ; blanc D50, matrice XYZ → sRGB adaptée par Bradford, valeurs publiées
    par Bruce Lindbloom), gris (0 à 10000, 10000 = noir : la spécification ne dit pas le sens,
    c'est la lecture de GIMP). Les nuanciers de fabricants (Pantone…) sont comptés et laissés de
    côté. Écrit en version 1 puis 2, en RVB, comme Photoshop.
- Raccourcis : D = `couleur.defaut` (touche de Photoshop, libre dans les deux modes, source
  « proposition ») ; X, la touche d'échange de Photoshop, est `calque.coller` (réglage perso
  PASTE_LAYERS_KEY) : `couleur.intervertir` n'a pas de touche. `nuancier.ajouter` correspond
  par le nom à COLOR_ADD (« New Colour Swatch ») de Storyboard Pro 27, qui n'a pas de touche.
  Les autres commandes sont de Pelure, sans touche. Un test vérifie que chaque commande des
  couleurs citée dans `colorui.rs` et `nuancierui.rs` est déclarée et traitée.

## Pinceaux texturés (3d-2)

- `brush.rs` : chaque empreinte calcule une couverture par pixel — disque (dureté) ou pointe en
  image (`Tip`, échantillonnée dans le repère de la pointe : rondeur, angle, hasard), multipliée
  par l'opacité (pression) et par le grain. Le trait garde le maximum par pixel (pas
  d'accumulation dans un même trait), comme avant.
- `brushtex.rs` : grain = papier en motif fixe sur le dessin (coordonnées du dessin, pas du
  trait) ; un pixel reçoit la couleur si son relief dépasse un seuil qui baisse quand on appuie
  (`grain_factor`). Papiers de Pelure fabriqués au démarrage (bruit fin réparti uniformément,
  `procedural_paper`). Pointes : réductions successives (moyenne 2 × 2) pour qu'une petite pointe
  reste lisse. `BrushLibrary` : textures et réglages nommés ; les importés sont gardés dans
  `%APPDATA%\Pelure\pinceaux` (`pinceaux.json` + textures PNG en gris). Les réglages
  (`BrushSettings`, sérialisés en français) ne citent les textures que par numéro.
- `tbbrush.rs` : lecture des préréglages Toon Boom — petit lecteur XML maison (le format n'a que
  des balises et des attributs `value`), textures `bundledFile` en hexadécimal (en-tête de
  18 octets, gris 8 bits, lignes alignées sur 4 octets). Correspondance des réglages et
  hypothèses en tête du fichier. Coût d'un vrai lecteur XML : une bibliothèque à télécharger,
  inutile ici ; réversible.

## Pelure d'oignon (3d-1)

- `onion.rs` : voisins du dessin courant, sur le calque courant, posés sous lui dans le plan de
  composition (même chemin que le reste : carte graphique ou processeur), teintés (`PlanItem::tint`)
  ou non, opacité dégressive (`fade`). Deux façons de compter : par dessin (les dessins différents
  les plus proches) ou par image (les images voisines ; un dessin vu plusieurs fois n'est montré
  qu'une fois, au plus près). Réglage de l'application, pas du projet : `[pelure]` dans
  `%APPDATA%\Pelure\reglages.toml`, enregistré quand il change (pas pendant un glissé).

## Sélection bitmap, table lumineuse, guides

- **Sélection bitmap** (`pixsel.rs` : calcul, testé sans interface ; `pixtools.rs` : outil). Une
  zone = un masque anticrénelé (lasso ou rectangle, réunion avec Maj). Au premier déplacement, les
  pixels sont **soulevés** : on garde le dessin sans eux (`base`), le dessin d'avant (`before`) et
  les pixels eux-mêmes. Chaque mouvement repose les pixels d'origine avec la transformation totale
  (interpolation bilinéaire ; déplacement arrondi au pixel, donc sans flou). En quittant la
  sélection (Échap, autre outil, autre image, Ctrl+Z…), l'annulation reçoit une seule étape : les
  tuiles qui diffèrent de `before`.
- **Table lumineuse** : dans l'affichage de travail seulement (pas à l'export ni dans les
  vignettes), les calques autres que le calque courant sont composés avec une opacité réduite.
  C'est notre lecture de la « Light Table » de Storyboard Pro (autres calques estompés) ; la table
  lumineuse de TVPaint, qui montre d'autres images, correspond plutôt à notre pelure d'oignon.
- **Pelure d'oignon complète** (13f, `onion.rs`, `onionui.rs`) : `OnionSettings` reste un
  réglage de l'application (`reglages.toml`, table `[pelure]`), pas du projet, comme TVPaint
  garde son panneau. Par rang (0 = l'image la plus proche, jusqu'à `MAX_NEIGHBOURS`) :
  `show_before/after` et `before/after_opacity` ; liés, seule l'opacité du rang 0 compte et les
  autres suivent `ramp` (en ligne droite), ce qui redonne exactement l'ancien `fade` ; délier
  recopie ce qu'on voyait. Opacité finale = globale × celle du rang ; couleur : `tint_of`
  (dégradé linéaire de la couleur 1 au rang 0 à la couleur 2 au dernier rang). `migrate` reprend l'ancien
  `tinted` des deux côtés. Les modes signet et marque ne retiennent que les images aux signets
  du plan ou dont le dessin porte une marque de couleur. « Devant » pose les images de la
  pelure après le dessin du calque dans `build_plan`. Le décalage (`App::onion_offset`, en
  pixels de la feuille) s'ajoute au `LayerTransform` des seules images de la pelure ; il n'est
  pas enregistré.
- **Guides** (`guides.rs`) : un guide = 1, 2 ou 3 points de fuite en pixels du dessin, enregistrés
  dans le manifeste (`"guides"`) et annulables comme le reste. Réglages d'affichage (actif, aligné,
  verrouillé, lignes de référence) gardés dans l'application, pas dans le projet. Trait aligné : au départ du trait on
  attend 6 pixels d'écran de mouvement, puis on choisit la direction du guide la plus proche (vers un
  point de fuite, verticale, horizontale) et chaque point du trait est projeté sur cette droite.
  Les types de guides de Storyboard Pro autres que la perspective à 1, 2 ou 3 points (lignes,
  isométrique…) ne sont pas faits.

## Son

- **Décodage** (`audio.rs`, bibliothèque symphonia) : à l'import ou à l'ouverture, le fichier entier
  est converti en stéréo 48 kHz, 16 bits, en mémoire (≈ 11 Mo par minute), avec un résumé de forme
  d'onde (une valeur toutes les 240 échantillons) pour l'affichage.
- **Modèle** : `Document.sounds` (fichiers importés, jamais retirés pendant la session, comme les
  dessins) et `Document.audio_tracks` (pistes, chacune avec des extraits qui ne se chevauchent pas).
  Un extrait = un son, une image de départ dans le montage, un décalage dans le son, une durée.
  Les pistes sont en **temps de montage** : elles ne suivent pas les panneaux (le lien
  extrait-panneau de Storyboard Pro, `LINK_CLIP_AND_PANEL`, reste en réserve).
- **Lecture** : la sortie audio (bibliothèque cpal, WASAPI) est ouverte à la première lecture avec
  du son. Le mixage tourne dans le fil de la carte son ; **sa position sert d'horloge** à l'image,
  qui ne peut donc pas dériver. Sans sortie audio, ou si elle tombe en panne, la lecture continue à
  l'horloge de l'ordinateur, avec un message.
- **Son au défilement** : quand l'image courante change hors lecture, une image de son est jouée.
- Le mixage (`MixState`) se teste sans carte son.
- **Effets** (`audiofx.rs`, 2026-10-19) : `build_mix(doc, wait)` est le seul endroit où les pistes deviennent des
  `MixClip` (lecture, défilement, WAV, export vidéo, export vers Storyboard Pro). Un `MixClip` porte un gain fixe
  (piste × général × extrait) et, s'il y a lieu, un `ClipEnv` (fondus, fondu enchaîné en puissance égale, clés de
  volume) que le fil de la carte son évalue échantillon par échantillon ; l'égaliseur (étagères à 200 Hz et 4 kHz,
  cloche à 1 kHz) et l'écho refont le son de l'extrait sur un fil à part (`processed`, 24 résultats gardés), jamais
  sur le fil de l'interface ni celui de la carte son. Le fondu enchaîné relie deux extraits qui se touchent et utilise
  les poignées de son de part et d'autre de la coupe. Réglages dans `SoundClip::fx` et `AudioTrack::solo` (donc
  annulables), fichier `effets_son.json` sans changer `VERSION`.

## Transitions du montage (2026-10-19)

- `transitions.rs` : `Transition { sortant, entrant, sorte, images, calage }` rangées dans `Document` et `Structure`
  (annulation), `zones(doc)` (où tombe chaque zone, bornée aux deux plans qu'elle relie, ignorée si les plans ne se
  suivent plus), `Zone::mix(g)` (quelles images de quels plans, poids du plan entrant, volet, couche de couleur).
  **La durée du film ne change pas** ; une image manquante est tenue. `blend_images` mélange sur le processeur
  (export) ; fichier `transitions.json` (même protection que `commentaires.json` et `rognages.json`).
- Écran (`gpu.rs`, `GpuCanvas::paint_mix`) : le second plan est composé dans une seconde image (`frame2`, gardée
  comme la première tant que rien ne change : un plan tenu ne se recompose pas), les deux sont posées l'une sur
  l'autre par l'affichage (opacité, volet par colonne, aplat de couleur : `fs_display`), chaque plan avec sa
  propre caméra en vue caméra. `canvas.rs` construit les deux plans (`mix_setup`) ; sans carte graphique, pas
  d'aperçu (l'export, lui, mélange sur le processeur).
- Export (`export.rs`) : `ExportJob::with_transitions` ; les images d'une zone sont le mélange de deux images
  calculées comme les autres (cache de quatre images pour ne pas recalculer le plan tenu).

## Bibliothèque de bruitages (2026-10-19)

- `bruitages.rs` : réglages (`[bruitages] dossiers` dans `reglages.toml`), parcours des dossiers sur un fil à part
  (`Bibliotheque::scan`, paquets repris à chaque image), durées lues à la demande par une sonde (en-tête symphonia),
  écoute en trois états (repos, chargement, lecture) sur un trait `Sortie` (la sortie audio de SHOT, ou un faux).
- `bruitagesui.rs` : le panneau (`Pane::Bruitages`, fermé par défaut), le glisser-déposer d'egui (`SoundDrag`)
  vers la bande de montage et la ligne du son de la timeline, la pose d'un son lu et décodé à part
  (`place_imported_sound`, le chemin de l'import).

## Feuille et cadre (depuis 2b-1)

- **Feuille** (`Document.width/height`) : la surface de tous les dessins, deux fois plus grande que
  le cadre pour un nouveau projet. Tout le dessin (pinceaux, sélection, guides…) travaille en
  pixels de feuille, sans changement. Une zone vide ne coûte pas de mémoire (tuiles à la demande).
- **Cadre** (`Document.frame`) : la taille de l'image finale ; centré sur la feuille
  (`Document::frame_rect`). L'export, les vignettes et la page de storyboard montrent le cadre
  (`render::frame_image`).
- **Conversion** : un projet au format 3 (ou un ancien `.pelure`) avait feuille = cadre. À
  l'ouverture, `Document::enlarge_stage` double la feuille et décale tout le contenu au centre
  (dessins bitmap et vectoriels, guides) : le cadrage ne change pas.
- Taille maximale du cadre : 4096 px (feuille 8192 px, limite des textures de la carte graphique).

## Transformation de calque (depuis 2c-1)

- `Layer::transform` (`LayerTransform`) : décalage, échelle uniforme, rotation, autour du centre
  de la feuille. Les dessins ne sont jamais recalculés : la transformation est appliquée à la
  composition (`PlanItem::transform` ; processeur : chaque pixel va chercher le point du dessin,
  bilinéaire ; carte graphique : le calque est posé tourné / agrandi dans l'image composée, lissage
  bilinéaire qui tombe pile sur les pixels pour un calque non transformé).
- Dessiner sur un calque transformé : la souris est convertie dans le repère du dessin
  (`App::to_layer`), et les repères affichés (sélection, formes, curseur) le sont dans l'autre sens
  (`App::layer_to_screen`, `layer_zoom`). Guides, caméra et pipette restent sur la feuille.
- Échelle et rotation à la souris se font autour du centre de la boîte du contenu
  (`LayerTransform::around` garde la forme « décalage + échelle + rotation »).

## Profondeur et multiplan (depuis 2c-2)

- `Layer::depth` (z) : 0 = plan de référence, celui qui défile exactement avec la caméra. Un plan
  à z est vu à la distance 1 + z : quand le centre de la caméra s'écarte de C − C0 du centre de la
  feuille, le calque glisse de (C − C0) × (1 − 1/(1 + z)) dans le même sens (`transform_seen`).
  Résultat dans le cadre : z = 1 défile deux fois moins vite, z très grand reste fixe (ciel),
  z négatif défile plus vite (premier plan). Borne : z ≥ −0,9.
- Ce glissement s'ajoute à la transformation du calque au moment de composer
  (`render::panel_plan_at`, `App::build_plan`) : rien n'est enregistré en plus, et caméra centrée
  = aucun effet. Le zoom et la rotation de la caméra n'en tiennent pas compte (zoom d'objectif).
  Depuis 9c, l'avancée de la caméra (travelling avant) agrandit chaque calque selon sa
  distance (voir « Phase 9 »).
- Dessiner sur un calque en profondeur : la souris passe par la transformation **vue**
  (`App::layer_seen`), le trait tombe donc sous le curseur quelle que soit la position de la
  caméra à l'image courante.
- Manifeste : `"profondeur"` par calque, absent si 0. Fusion refusée entre profondeurs différentes.
- **Vue de dessus** (`topview.rs`, 2c-3) : un calque à la distance D = 1 + z est un trait ; un
  point x de son dessin y est à X = D × (x − centre de la feuille). La caméra est en X = C − centre,
  son champ s'ouvre de D × cadre / zoom. Ce sténopé redonne exactement le défilement multiplan
  (testé) : la vue montre ce que filme la caméra. Distances au-delà de 12 posées au bord haut.
  Glisser un calque : copie de la structure au départ, une étape d'annulation au lâcher (comme
  l'outil Transformer) ; le cadrage de la vue est figé pendant le glisser.

## Caméra (depuis 2b-2)

- `camera.rs` : une caméra = centre du cadre sur la feuille, zoom (1 = taille de l'image finale),
  rotation en degrés. Chaque panneau a des images clés (`Panel.camera`, triées) : aucune = caméra
  au centre ; une = caméra fixe ; plusieurs = interpolation (positions et rotation en ligne
  droite, zoom à vitesse régulière, c'est-à-dire géométrique). Couper un panneau coupe le
  mouvement (`split_keys`), les réunir le recolle (`join_keys`) : l'image filmée ne change pas.
- Régler la caméra (`camtools.rs`) : caméra fixe → on change l'unique clé ; caméra animée → on
  règle (ou crée) la clé de l'image courante. Annulable comme le reste (clés dans la structure).
- **Vue caméra** : n'est pas un rendu à part, c'est une vue (zoom, rotation, décalage) calculée
  pour que le cadre remplisse l'écran ; dessiner y reste possible, la souris est convertie comme
  d'habitude. Hors du cadre : masque opaque.
- **Export, vignettes, planches** : `render::camera_image` prend chaque pixel de l'image finale au
  point filmé de la feuille (bilinéaire) ; caméra centrée sans zoom ni rotation = pixels exacts.
- Manifeste : `"camera": [{"image", "x", "y", "zoom", "rotation", "doux", "avance",
  "adoucir_entree", "adoucir_sortie"}]` par panneau (absent si aucune clé ; les trois derniers
  depuis le format 10, absents si nuls).
- **Courbes** (2b-3) : entre deux clés, l'avancement suit une courbe de Hermite dont la pente
  vaut 0 au départ et/ou à l'arrivée si la clé correspondante est « adoucie », 1 sinon (deux clés
  non adoucies = mouvement régulier). C'est un choix de Pelure : la façon dont Storyboard Pro règle
  les vitesses de caméra n'a pas été vérifiée.

## Repère 3D (2d)

- `model3d.rs` : lecteur **glTF 2.0** écrit pour Pelure (pas de bibliothèque ajoutée) : `.glb`, ou
  `.gltf` avec tampons intégrés (base64) ou à côté (jamais hors de son dossier). On prend les
  triangles de la scène (listes, bandes, éventails ; indices 8, 16, 32 bits), avec les
  transformations des nœuds (matrice ou translation, rotation, échelle). Matières, textures,
  animations ignorées ; accesseurs « clairsemés », Draco et données quantifiées refusés avec un
  message. Sommets **soudés** (les exports dupliquent les sommets aux coutures), puis **arêtes
  utiles** : bord, cassure de plus de 25° entre deux faces (en valeur absolue : une face retournée
  ne fait pas apparaître de diagonale), arête partagée par plus de deux faces. Tri plutôt que
  tables de hachage : 1,77 million de triangles en 0,33 s. Arêtes rangées des plus longues aux
  plus courtes (pour l'aperçu).
- `view3d.rs` : caméra en orbite (`Cam3d` : point visé, azimut, hauteur, distance, focale en mm pour
  36 mm de large). Fil de fer en pixels du cadre ; arêtes cachées par un **tampon de profondeur**
  calculé sur le processeur (1/z, ≈ 1300 pixels sur la largeur du cadre ; une arête est visible si
  aucun des 9 pixels autour n'est nettement plus proche). Tout est réparti sur les cœurs (bandes du
  tampon, paquets d'arêtes). `rasterize` trace les segments, lissés, dans un masque.
- `ref3dtool.rs` : `Panel.ref3d` = modèle, caméra, visible, arêtes cachées, couleur, opacité ;
  modèles dans `Document::models` (partagés entre panneaux, jamais retirés pendant la séance). Le
  repère couvre **toute la feuille** (le décor continue dans le hors-champ). Affichage : le fil de
  fer complet est calculé sur un autre fil et tracé dans **une image** (taille de la feuille × le
  zoom de l'écran, par paliers), posée sur la feuille comme un dessin : une seule texture par
  image d'écran, le pinceau ne ralentit pas. Pendant qu'on tourne, ou le temps du calcul, un
  aperçu des 6 000 plus longues arêtes est tracé directement. Le repère n'entre ni dans les
  exports ni dans les vignettes ; « Dessiner dans un calque » le trace dans un calque bitmap.
- Annulation : le repère fait partie de la structure (copie au début du geste, une étape au
  lâcher, comme l'outil Caméra).

## Format de projet (version 17, dossier)

```
MonFilm/
  MonFilm.pelure      manifeste JSON : {"format": "pelure-projet", "version": 17, "largeur",
                      "hauteur" (feuille), "cadre": [l, h], réglages,
                      "sequences": [{ "scenes": [{ "panneaux": [{ "calques": [{ "expositions": … }],
                      "marques": [{"id", "image", "action", …}] }]}]}]  (un panneau par scène),
                      "sons": [{"id", "nom", "fichier"}], "pistes_son": [{ "extraits": [{"son", "debut",
                      "decalage", "duree"}] }], "modeles": [{"id", "nom"}]}
  dessins/<id>.png    dessin bitmap (RGBA non prémultiplié)
  dessins/<id>.json   dessin vectoriel (éléments sérialisés)
  sons/<id>.<ext>     fichier son importé, copié tel quel (wav, mp3, flac, ogg)
  modeles/<id>.pelure3d  modèle 3D soudé : "PELURE3D", version, nombre de sommets et de
                      triangles, positions (3 × f32) puis triangles (3 × u32), petit-boutiste
```

- Format 21 (13k) : `video` sur un calque, { `id`, `nom`, `images`, `ips`, `debut`, `ext` } ; ses
  images dans `videos/<id>/000001.<ext>`… ; absent : calque ordinaire.
- Format 20 (13j) : `ctg` sur un calque, { `sources` : rangs des calques sources dans le plan
  (vide : tous les autres calques de dessin), `trou` : fermeture des trous en pixels (absent :
  4) } ; absent : calque ordinaire.
- Format 19 (13i) : `effets` sur un calque, une liste de { `sorte` (`flou_gaussien`,
  `flou_mouvement`), `actif`, `valeurs`, `cles` : [{ `image`, `valeurs` }] } ; absent : aucun.
- Format 18 (13g-2) : `roles` sur un calque (image → `cle`, `breakdown`, `intervalle`,
  `intervalle_complete`, `intervalle_special`, `intervalle_special_complete` ; absent : aucun).
- Format 17 (13e) : `fond` (`sorte` : `aucun`, `couleur`, `damier` ; `couleur_b`) dans le
  manifeste ; `notes_dessinees` (couleur, épaisseur, points [image, hauteur]) sur un panneau.
  En mémoire : `Document::background`, `paper_b`, `render_paper()` (le papier seulement pour
  une couleur), `Cues::notes`. Aussi `volume_general`, `verrouillee` sur une piste son,
  `storyboard_verrouille` et `proprietes` (`titre`, `informations`, `auteur`, `cree`,
  `modifie` en secondes depuis 1970, `notes`, `copyright` ; absents : vides, date inconnue).
- Format 16 (13d) : sur un panneau `signets`, `entree`, `sortie` (images du plan) ; dans le
  manifeste `image_de_depart` (absent : 1). En mémoire : `Panel::cues` (`Cues`) et
  `Document::start_frame` (dans `Structure`).
- Format 15 (13c) : `avant`, `apres` sur un calque (`rien`, `boucle`, `va_et_vient`,
  `maintenir` ; absents : rien). Le rendu lit `Layer::shown_at` ; les outils, `cel_at`.
- Format 14 (13b-5) : `dossier` (le calque est un dossier) et `dans_dossier` (rang, dans le
  plan, du dossier qui le range ; les id de calque ne sont pas enregistrés).
- Format 13 (13b) : sur un calque `pelure`, `transparence`, `pochoir`, `pochoir_negatif`,
  `position_verrouillee`, `replie` (absents : faux), `groupe` (absent : 0) ; `groupes` (13
  groupes : `nom`, `couleur`, `montre`), absent tant qu'ils sont ceux d'un projet neuf.
- Format 12 (13a) : un panneau par scène ; `marques` sur un panneau.
- Format 11 (9f) : `bascule`, `panoramique` (degrés) sur une clé de caméra, absents si nuls.
- Format 10 (9c) : `avance`, `adoucir_entree`, `adoucir_sortie` sur une clé de caméra ; `fusion`
  peut valoir les 10 nouveaux modes (identifiants en français, ex. `lumiere_vive`).
- Format 9 (8) : `fusion` sur un calque (absent : normal) ; `attache` sur une note (id du
  panneau ; points relatifs à la vignette) ; `planche_textes` (texte, position, taille, couleur).
- Format 8 (7) : `planche` (notes : couleur et points x, y, rayon) ; `pinceau` sur un trait
  vectoriel texturé ; `textures/pointe_<clé>.png`, `textures/papier_<clé>.png` (gris 8 bits),
  écrites seulement si elles manquent (la clé vient du contenu), effacées quand plus rien ne les cite.
- Format 7 (6f) : `toute_la_scene` sur un calque (un numéro, le même pour les exemplaires d'une
  scène) ; le dessin partagé n'est écrit qu'une fois. Format 6 (5d) : éléments texte.
- Format 5 (2d) : `modeles`, et `repere3d` par panneau (`modele`, `cible`, `azimut`,
  `elevation`, `distance`, `focale`, `visible`, `aretes_cachees`, `couleur`, `opacite`). Comme
  les sons, un modèle n'est écrit que s'il manque, et effacé quand plus aucun panneau ne s'en sert.
  Un modèle manquant est signalé à l'ouverture, ses repères sont retirés, le reste s'ouvre.

- Le champ `guides` (1c) suit la même règle.
- Les champs `sons` et `pistes_son` ont été ajoutés en 1b sans changer de version : absents, ils
  valent « aucun son », et une version précédente de Pelure les ignore (elle perdrait le son en
  réenregistrant, mais rien d'autre).
- Un son n'est écrit qu'une fois (il ne change jamais) ; les sons qui ne servent plus sont effacés de
  `sons/` à l'enregistrement. Un son manquant ou illisible est signalé à l'ouverture, ses extraits
  sont retirés, le reste s'ouvre. Un nom de fichier qui sortirait de `sons/` est refusé.
- Clés en français, expositions écrites en plages (`{"dessin": 12, "images": 24}`) : le manifeste
  reste lisible et court.
- **Enregistrement incrémental** (`project.rs`) : un dessin n'est réécrit que si sa `version` a changé
  depuis le dernier enregistrement dans ce dossier. Écriture « à côté puis renommage » pour chaque
  fichier ; manifeste écrit après les dessins ; dessins inutilisés effacés en dernier (seulement les
  fichiers `<nombre>.png/json` de `dessins/`). Ce schéma servira à la sauvegarde automatique (phase 4).
- **Garde-fous** : « Enregistrer sous » refuse un dossier qui contient déjà un autre projet ou un
  dossier `dessins` ou `sons`. À l'ouverture, un dessin manquant ou illisible est signalé, le reste s'ouvre.
- **Compatibilité** : un champ inconnu est ignoré, un champ absent prend sa valeur par défaut ; un
  format plus récent que le logiciel est refusé avec un message. Les anciens fichiers à fichier unique
  (formats 1 et 2, `io.rs`) s'ouvrent : chaque ancien « plan » devient une scène d'un panneau.

## Raccourcis clavier

- Chaque action a un **identifiant stable** (`keymap::ACTIONS`, ex. `edition.annuler`). On en ajoute,
  on n'en renomme jamais : les fichiers personnels de l'utilisateur restent valables.
- `keymap.toml` (racine du dépôt) est le fichier par défaut, **intégré au programme**. Un fichier
  personnel (à côté de l'exécutable, sinon `%APPDATA%\Pelure\keymap.toml`) remplace action par action
  ce qu'il redéfinit ; si une de ses touches est aussi prise par le fichier par défaut, la sienne
  l'emporte (et l'aide signale le conflit). Menu « Aide > Modifier / Recharger les raccourcis ».
- Les raccourcis viennent de Storyboard Pro 27 (65 par défaut, plus 4 du module Guides) et des
  réglages perso (8). Ceux dont
  la fonction n'existe pas encore sont gardés avec `etat = "non implémenté"`. Les ajouts de Pelure
  sont marqués `source = "proposition"`.
- Lettres = caractère tapé ; chiffres et ponctuation = position physique (clavier AZERTY).
  Détails : `docs/reference/sbpro_presets/README.md`.
- Un fichier illisible ou une action inconnue ne bloque rien : remarques affichées dans l'aide (F1).
- Raccourcis choisis dans Pelure (`raccourcisui.rs`, passe interface).
  - Les boutons liés à une action (`App::bouton*`, `App::aide`) ont un menu au clic droit.
    `action_item` fait de même pour les menus (clic droit direct).
  - Une commande dessinée avec `&self` seulement dépose sa demande dans les données d'egui
    (`ask_assign`) ; `shortcut_windows` la lit à l'image suivante.
  - Pendant l'assignation, `capture_shortcut` prend la première touche, avant les raccourcis, y
    compris Tab mis de côté pour egui. `keymap::combo_text` l'écrit comme le fichier : lettres et
    touches nommées par ce qu'elles écrivent, chiffres et signes par leur position.
  - `Keymap::assign_changes` retire la touche aux autres actions du même contexte.
    `edited_user_file` réécrit le fichier personnel avec le crate `toml` : le contexte suit celui
    du raccourci par défaut, les autres entrées sont gardées, les commentaires ne le sont pas. Puis
    `reload_keymap`.
  - Décision : coût — réécrire le fichier entier plutôt que le modifier ligne à ligne (simple,
    testé). Risque : les commentaires d'un fichier écrit à la main se perdent, et un fichier
    illisible n'est jamais écrasé. Réversible : le fichier reste du texte.

## Compilation

Toolchain Rust **MSVC** (`stable-x86_64-pc-windows-msvc`, fixée par `rust-toolchain.toml`) avec les
« Visual Studio Build Tools 2022 » (charge C++), installés au début de la phase 2.

L'ancienne toolchain `x86_64-pc-windows-gnu` reste utilisable en secours (supprimer
`rust-toolchain.toml`) : `Cargo.lock` fige encore `parking_lot` 0.12.3, `parking_lot_core` 0.9.10,
`libloading` 0.8.6, et `link/libshlwapi.a` + `build.rs` fournissent une bibliothèque d'import
(sans effet avec MSVC). À retirer quand ce secours ne servira plus.

## Décisions (coût / risque / réversibilité)

| Décision | Pourquoi | Coût | Risque | Réversible ? |
|---|---|---|---|---|
| egui 0.31, pas de mise à jour pour l'instant | stable, déjà en place | nul | moyen à terme (retard sur les versions) | oui, coût croissant avec la taille du code |
| Rendu sur processeur en phase 1, carte graphique (wgpu) en phase 2 | la 3D et le multiplan l'exigent ; inutile avant | réécriture de `render.rs` en phase 2 | moyen | oui (le modèle de données ne change pas) |
| Moteur d'affichage egui : wgpu par défaut, OpenGL en secours (fait en 2a-1) | wgpu permet nos propres passes de rendu (calques, caméra, 3D) | ≈ 1 min de compilation en plus ; mémoire 112 → 335 Mo au démarrage (pilote) | faible : repli automatique sur OpenGL | oui : une option au lancement |
| Toolchain GNU → **outils Microsoft (MSVC)** (validé, fait au début de la phase 2) | plus de contournements à chaque bibliothèque | 5–7 Go, une fois ; exécutable 20,5 → 8,8 Mo | faible | oui : supprimer `rust-toolchain.toml` |
| **Projet = dossier** (validé, fait en 1a) | sons, 3D, sauvegarde auto incrémentale | lecture des anciens `.pelure` gardée | moyen | format versionné : les évolutions futures se font par ajout de champs |
| **Séquence > Scène > Panneau** (validé, fait en 1a) | vocabulaire de Storyboard Pro | refonte du modèle | moyen | l'arbre du fichier est stable ; la liste à plat en mémoire peut changer sans toucher au fichier |
| Raccourcis en TOML, identifiants stables | modifiables sans recompiler | faible | faible | oui |
| Son : **symphonia** (décodage) + **cpal** (sortie) (fait en 1b) | en Rust pur, sans DLL à fournir ; formats courants (WAV, MP3, FLAC, OGG) ; compilent avec la toolchain actuelle sans toucher aux versions figées | ≈ 3,5 Mo d'exécutable en plus (17 → 20,5 Mo) | faible (bibliothèques très utilisées) ; pas d'AAC/M4A | oui : tout passe par `audio.rs` |
| Son décodé entier en mémoire, 16 bits | simple, lecture et défilement instantanés | ≈ 11 Mo par minute de son | élevé seulement pour des heures de son | oui : lecture par morceaux possible plus tard sans changer le format |
| Horloge de lecture = position du son | l'image ne peut pas dériver du son | nul | faible (repli sur l'horloge si pas de sortie) | oui |
| Sons en temps de montage (pas liés aux panneaux) | comportement le plus simple et prévisible | nul | moyen : un panneau rallongé décale l'image par rapport au son | oui : le lien extrait-panneau pourra s'ajouter (champ en plus) |
| Export vidéo via ffmpeg installé à part (phase 3) | seule voie fiable sans souci de licence ni de compilation | dépendance externe | faible | oui |
| Pas d'export XML Toon Boom | format propriétaire non documenté | — | — | — |
| Icônes Phosphor (egui-phosphor, MIT) (5a, choix d'Anthony) | jeu complet et cohérent, dans la police de l'interface | ≈ 1 Mo d'exécutable | faible | oui : `Tool::icon`, `Pane::icon` |
| Panneaux par egui_dock 0.16 (MIT), disposition dans un format de Pelure (5b) | aimantation, onglets, panneaux flottants déjà faits et éprouvés ; format à nous pour ne pas dépendre de la version | refonte de l'affichage des panneaux | moyen : accessibilité coupée (plantage d'accesskit) | oui : le format `DockLayout` ne dépend pas de la bibliothèque |
| Calque de toute la scène = un exemplaire par panneau, accordés à chaque image (6f) | rendu, export, vignettes, outils et annulation inchangés | une passe sur les calques à chaque image (négligeable) ; règles à suivre dans les opérations de scène | moyen : une opération de montage oubliée pourrait mélanger deux fonds (couvert par `scenelayer_tests.rs`) | oui : le fichier ne garde qu'un numéro par calque ; un vrai calque de scène pourrait le remplacer |
| Modes de fusion : copie de l'image composée avant chaque calque concerné, plutôt qu'un mélange matériel (8b) | seuls quelques modes existent en mélange matériel ; la copie permet les 17 avec les mêmes formules que le processeur | une copie de l'image (quelques Mo) par calque en mode de fusion | faible | oui : un calque normal ne coûte rien de plus |
| Notes attachées en coordonnées relatives à la vignette (8a) | la note suit et grandit avec la vignette, quel que soit le réglage de la planche | nul | faible | oui : détacher la remet en coordonnées de la planche |
| Notes de la planche libres, pas attachées aux vignettes (7, choix d'Anthony) | la planche est un espace de travail à part entière | nul | une note peut ne plus montrer le bon panneau après un déplacement | oui : une ancre par note pourrait s'ajouter (champ en plus) |
| Textures des traits vectoriels par clé de contenu, embarquées dans le projet (7b) | les numéros de la bibliothèque changent d'une machine à l'autre ; les dessins déjà écrits restent valables | un petit PNG par texture utilisée | faible (collision de clé sur 31 bits négligeable) | oui |
| Dessin dans la vignette = le même `canvas_panel` dans un `Ui` enfant (6e) | aucun second chemin de dessin | nul | faible | oui |
| Caméra d'un panneau sans clé = celle du voisin dans la scène (9c) | la caméra ne saute plus d'un panneau à l'autre, sans toucher aux panneaux qui ont des clés | nul | faible : un panneau sans clé change si l'on règle son voisin (c'est le but) | oui : poser une clé fige le panneau |
| Avancée de la caméra dans le multiplan, pas de bascule ni de panoramique (9c) | le rendu reste un empilement de plans 2D, exact et rapide | nul | les mouvements X/Y de la caméra manquent | oui : un rendu en perspective pourra s'ajouter (champs en plus) |
| Appli compagnon Tauri à côté de Pelure, pas à la place de l'interface egui (10, choix d'Anthony) | le storyboard profite d'une interface web ; la toile de dessin (carte graphique, stylet, latence) reste où elle est rapide | Node.js + ≈ 300 crates de plus à la compilation du compagnon seulement ; un noyau à tenir | faible : Pelure inchangée ; deux applis peuvent enregistrer le même projet l'une sur l'autre | oui : supprimer `src-tauri/` et `src-frontend/` |
| **Pelure egui reste le logiciel principal** ; Pelure Studio (Tauri) = banc d'essai d'interface (11, choix d'Anthony) | la toile native (egui + wgpu, même processus, affichage réglé) garde la meilleure latence et partage la carte graphique avec le rendu ; dans un WebView, chaque trait traverse l'IPC (1,6 à 5,6 ms mesurés) puis le compositeur de WebView2, et les pixels sont copiés (8 Mo par calque pour une opération sur toute l'image) | nul | faible : les idées d'interface du Studio sont à reporter dans egui à la main | oui : `moteur.rs` reste disponible pour une autre interface |
| Thème : couleurs de Spectrum reprises en constantes, police Segoe UI du système (12a) | l'allure dense et sombre du Studio, sans dépendance ni fichier ajouté | nul (police lue au démarrage, ≈ 1 Mo) | faible : sans Segoe UI (autre système), la police d'egui reste | oui : `theme::appliquer` et `polices` à retirer |
| Ctrl+C / Ctrl+V / Ctrl+X / Suppr visent les panneaux après un clic dans le montage ou sur une vignette, le dessin après un clic dans le dessin (12e) | les touches habituelles servent pour les panneaux, sans nouveau raccourci à apprendre | un booléen (`App::panel_focus`) | moyen : Suppr après un clic dans le montage supprime des panneaux (Ctrl+Z les rend) | oui : retirer les gardes `panels_have_focus` dans `run_action` ; `panneau.copier` / `panneau.coller` restent |
| Un plan = une scène à un seul panneau, plutôt qu'un nouveau type (13a) | tout ce qui travaille sur un panneau (rendu, caméra, calques, enregistrement, lecture) reste valable | un rang à tenir (`is_plans`, testé) | faible : une opération oubliée pourrait remettre deux panneaux dans une scène (le chargement les réunirait) | oui : le modèle n'a pas changé de forme |
| Dossier = calque marqué `folder`, calques rangés par `parent`, gardés d'un seul tenant sous lui (13b-5) | rendu, timeline, enregistrement et annulation restent ceux d'une pile de calques ; pas d'arbre à parcourir | `normalize` à rappeler après chaque opération sur la pile (testé dans `folders_tests.rs`) | faible : un oubli remet au pire un calque hors de son dossier | oui : passer à un arbre ne changerait que `folders.rs` et le format (`dans_dossier`) |
| Les débuts d'anciens panneaux deviennent des marques d'image avec leurs textes (13a, choix d'Anthony) | aucun dessin ni texte de storyboard perdu ; vignettes et PDF gardent leurs images | un champ par plan (`marques`, format 12) | faible | oui : retirer une marque ne touche pas aux dessins |
| Copie de panneaux avec une copie de leurs dessins, au moment de la copie (12e) | coller donne ce qu'on a copié, même si l'original a changé ou disparu | mémoire des dessins copiés, tant que le presse-papiers les garde | faible | oui |
| `src-tauri` projet Cargo à part, pas membre d'un espace de travail (10b) | Tauri ne change aucune version des dépendances de Pelure | le noyau est compilé deux fois (deux dossiers `target`) | faible | oui : en faire un membre plus tard |
| Bascule et panoramique = homographie du plan de référence déjà composé (9f) | exact (le multiplan ne dépend que de l'œil), sans toucher à la composition ni aux calques | une homographie par image ; vue caméra par la carte graphique avec un w par coin | faible : au ras de l'horizon, la feuille est très étirée (bornée à 5°) ; hors de la feuille, rien | oui : caméra droite = calcul d'avant ; deux champs du manifeste |
| Cadres de composition dans la disposition, pas dans le projet (9d) | ce sont des réglages de la vue, comme le zoom | nul | un format de diffusion propre au projet n'est pas transmis avec lui | oui : un champ du manifeste pourrait s'ajouter |
| Plateau 3D dessiné par egui (maillages subdivisés, textures réduites), pas par une passe wgpu à part (9e) | aucun shader ni pipeline de plus ; marche aussi en OpenGL | ≈ 600 triangles par calque ; texture refaite (≈ 10 ms) quand un dessin change | faible : pas de tampon de profondeur (plans parallèles triés, suffisant) | oui : `stage3dui.rs` seul à remplacer |
| Repère 3D : lecteur glTF et fil de fer écrits pour Pelure, sur le processeur (2d) | glTF = format ouvert que Blender exporte ; pas de bibliothèque ni de passe GPU à ajouter pour un guide en fil de fer | ≈ 900 lignes ; calcul 0,15 s par vue sur un gros décor (sur un autre fil) | faible : mauvais fichier → message | oui : affichage par la carte graphique ou rendu ombré possibles plus tard, le format `.pelure3d` ne change pas |
| Couleurs et nuanciers dans les réglages de l'application, pas dans le projet (passe couleur) | ce sont les outils de celui qui dessine, d'un film à l'autre, comme les pinceaux importés ; le format du projet ne change pas | nul | un nuancier propre à un film ne voyage pas avec lui (on l'exporte en .gpl ou .aco) | oui : un champ du manifeste pourrait s'ajouter (format versionné) |
| Avant-plan = `App::color`, l'existant ; arrière-plan à côté (passe couleur) | aucun outil à modifier : ils lisaient déjà `App::color` | nul | faible | oui |
| Opacité = celle du pinceau, pas une couleur RVBA (passe couleur) | le moteur de pinceau (repris bientôt) et les éléments vectoriels ne connaissent qu'une couleur RVB, et le pinceau a déjà son opacité | nul | la même valeur se règle à deux endroits (panneau Couleur, réglages du pinceau) | oui : une couleur RVBA demanderait de changer `StrokeMode` et le format des dessins vectoriels |
| Roue dessinée par le peintre d'egui en maillages subdivisés (passe couleur) | aucun shader ni texture ; couleurs exactes aux sommets ; marche aussi en OpenGL | ≈ 1 400 sommets par image d'écran quand le panneau est montré | faible | oui : `roue.rs` et `wheel_ui` seuls |
| Commandes de pastille sur la pastille du menu ouvert, sinon celle sous la souris (passe couleur) | chaque commande passe par `action_item` et peut recevoir un raccourci, qui agit sur la pastille survolée | deux champs mis à jour à chaque image | faible : la souris doit être sur la pastille au moment de la touche | oui |
| Nuanciers hors de l'annulation, suppression confirmée (passe couleur) | comme les espaces de travail : ce ne sont pas des données du projet | nul | une erreur sur une pastille ne se défait pas par Ctrl+Z | oui : un historique propre aux nuanciers pourrait s'ajouter |
| `.aco` lu sans profil de couleur (CMJN simple, Lab D50) (passe couleur) | aucune bibliothèque de gestion des couleurs à ajouter ; RVB, TSV et gris exacts | nul | une couleur CMJN ou Lab peut différer un peu de ce que montre Photoshop | oui : seul `nuancier::aco_color` changerait |
