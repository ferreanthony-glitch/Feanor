# Architecture de Pelure

Document tenu à jour à chaque étape. Dernière mise à jour : phase 2d (octobre 2026).

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
  = aucun effet. Le zoom et la rotation de la caméra n'en tiennent pas compte (zoom d'objectif,
  pas de travelling avant). C'est le choix le moins coûteux et réversible : un vrai rendu en
  perspective (taille qui change avec la distance) pourra venir avec la vue 3D.
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
- Manifeste : `"camera": [{"image", "x", "y", "zoom", "rotation", "doux"}]` par panneau (absent si
  aucune clé).
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

## Format de projet (version 5, dossier)

```
MonFilm/
  MonFilm.pelure      manifeste JSON : {"format": "pelure-projet", "version": 5, "largeur",
                      "hauteur" (feuille), "cadre": [l, h], réglages,
                      "sequences": [{ "scenes": [{ "panneaux": [{ "calques": [{ "expositions": … }]}]}]}],
                      "sons": [{"id", "nom", "fichier"}], "pistes_son": [{ "extraits": [{"son", "debut",
                      "decalage", "duree"}] }], "modeles": [{"id", "nom"}]}
  dessins/<id>.png    dessin bitmap (RGBA non prémultiplié)
  dessins/<id>.json   dessin vectoriel (éléments sérialisés)
  sons/<id>.<ext>     fichier son importé, copié tel quel (wav, mp3, flac, ogg)
  modeles/<id>.pelure3d  modèle 3D soudé : "PELURE3D", version, nombre de sommets et de
                      triangles, positions (3 × f32) puis triangles (3 × u32), petit-boutiste
```

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
| Repère 3D : lecteur glTF et fil de fer écrits pour Pelure, sur le processeur (2d) | glTF = format ouvert que Blender exporte ; pas de bibliothèque ni de passe GPU à ajouter pour un guide en fil de fer | ≈ 900 lignes ; calcul 0,15 s par vue sur un gros décor (sur un autre fil) | faible : mauvais fichier → message | oui : affichage par la carte graphique ou rendu ombré possibles plus tard, le format `.pelure3d` ne change pas |
