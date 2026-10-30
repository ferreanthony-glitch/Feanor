# Pelure — pourquoi les brosses « sentent » moins bien que TVPaint 12

Étude en lecture seule (2026-10-18). Code lu : `src/brush.rs`, `canvas.rs`, `app.rs`, `vector.rs`, `vtools.rs`, `gpu.rs`, `banc.rs`, `brushtex.rs`, `lib.rs`, sources de winit 0.30.13 et egui-winit 0.31.1. Côté TVPaint : `%APPDATA%\TVPaint Animation 12 Pro\default\config.ini` (tes réglages réels) et `english.tvpaint.loc`. Chaque affirmation est **observée** (fichier:ligne ou URL), **déduite** ou **à vérifier**.

## 1. Résumé

1. La chaîne stylet → pixel est saine : winit lit l'historique WM_POINTER (tous les échantillons, position sous-pixel, pression 0–1024), egui-winit apparie pression et position, Pelure interpole la pression entre deux événements. Ce n'est pas là que ça pèche.
2. Ce qui pèche, c'est la **mise en forme du geste** : pression → taille linéaire avec taille mini 0 (bouts en fil, départs fantômes), crayon **crénelé** par défaut, lissage « Suivi » dépendant de la cadence et du zoom, aucun lissage différé (TVPaint lisse *après* le trait, sans retard perçu), rattrapage en ligne droite au lâcher.
3. Le modèle d'empreinte (bord 1 px linéaire, « éclaircir sous 0,75 px ») donne des petites tailles grises et molles là où TVPaint garde du corps.
4. Latence : NoVsync + une image d'avance + « élastique » de la pointe, c'est bien ; reste à vérifier la précision de `thread::sleep` (2 ms) sous Windows.
5. Aucun trait n'a encore été enregistré avec le banc (`%APPDATA%\Pelure\traits` n'existe pas) : c'est l'étape 0, tout le reste se règle dessus.

## 2. Causes probables dans Pelure (de la plus à la moins probable)

### 2.1 Réponse à la pression nue : taille mini 0, plancher 3 %, pas de courbe de « toucher »
- **Preuve (observé)** : `brush.rs:444-483` — « Pinceau rond » : `size 6`, `pressure_size true`, `min_size 0.0`, `pressure_opacity false`. `brush.rs:504-507` — `size_factor` plancher à 3 %. `brush.rs:1039-1045` — sous 0,75 px de rayon, on **éclaircit** le débit au lieu d'amincir. `Connection::response` (`brush.rs:351-356`) est linéaire par défaut.
- **Effet ressenti** : au posé et au levé (pression Windows Ink à 2–10 %), le trait devient un fil à 0,18 px de rayon, affiché gris clair : « début fantôme », « bouts en pointe d'aiguille », trait qui semble maigre et sans corps à pression moyenne.
- **Ce que font les autres** : ton outil TVPaint courant a `consize2=10;2 0 1 0 0 1 1` (`config.ini:8980`) : taille connectée avec un **minimum 10 %** et une courbe linéaire (interprétation du format **déduite**, voir §3).
- **Correction** : taille mini par défaut 15–20 % pour le pinceau rond ; une **courbe de pression globale** (préférence) appliquée avant les connexions ; plancher du facteur à ~0,1 au lieu de 0,03 ; au-dessous d'un pixel, garder un rayon ≥ 0,5 et une couverture pleine plutôt que d'éclaircir.
- **Coût/risque** : faible (valeurs + une courbe), migration `#[serde(default)]`.

### 2.2 Crayon crénelé et nu par défaut
- **Preuve (observé)** : `brush.rs:541-550` — `pencil()` : `size 2`, `hardness 1`, `smoothing 0`, **`aliased: true`**. `app.rs:660` : c'est l'outil « Crayon » de la barre. Pas persisté : `reglages.toml` ne contient aucun réglage de pinceau, donc on repart de ces valeurs à chaque lancement (déduit).
- **Effet ressenti** : escaliers sur toute diagonale, courbes « en marches », trait de 2 px qui saute entre 1 et 3 px. Rien ne ressemble moins à un crayon TVPaint.
- **TVPaint** : tes outils ont `aliasing=0` (`config.ini:8764`, `8974`) — anticrénelage actif ; le profil « Crayon » n'est qu'un profil de pointe, pas un rendu pixel (observé).
- **Correction** : `aliased: false` par défaut, dureté 1,0, taille 2–3, `pressure_opacity true` avec `min_opacity ≈ 0,3` (un crayon mord plus ou moins), `smoothing` léger ; garder un réglage « pixels nets » à part pour le pixel art.
- **Coût/risque** : très faible.

### 2.3 Lissage : dépendant de la cadence et du zoom, jamais différé, rattrapage en ligne droite
- **Preuve (observé)** : `brush.rs:751` — « Suivi » = `prev + (pos − prev) × (1 − 0,3)` **par événement**, sans tenir compte du temps ni de la distance. `brush.rs:753-761` — la corde (`smooth_px`) est en **pixels du dessin**, pas de l'écran. `brush.rs:882-888` — `finish()` tire toujours un segment droit du dernier point lissé à la pointe. Lissage par défaut 0,3 (`brush.rs:455`).
- **Effet ressenti** : à 200–260 Hz de stylet, un facteur 0,7 par événement ne lisse presque rien (constante de temps ≈ 15 ms) mais ajoute un petit retard ; à faible cadence (grosse brosse, image lente), il lisse soudain beaucoup. Zoomé à 400 %, la corde de 8 px fait 32 px d'écran (trait qui traîne) ; à 25 %, 2 px (plus aucun effet). Au levé, un petit « crochet » droit termine chaque trait.
- **TVPaint (observé)** : `config.ini:9005-9011` — lissage actif, `smoothtype=2`, `smoothvalue=2.0`, **`smoothrealtime=0`**. Doc (*Line Smoothing*) : Real Time désactivé → la ligne est lissée **une fois le trait fini**. TVPaint t'affiche donc le trait brut sans retard, puis le remplace par le trait lissé : zéro latence perçue *et* ligne propre. Le mode derrière le code 2 : **à vérifier**.
- **Correction** : (a) « Suivi » converti en constante de temps (`slow_tracking × dt`, comme libmypaint) ; (b) `smooth_px` exprimé en pixels **d'écran** (option « distance selon le zoom », comme Krita) ; (c) **lissage différé** : pendant le geste, trait brut (ou lissage léger) ; au levé, on rejoue `raw_log` avec le lissage choisi sur les tuiles `before` déjà gardées (`StrokeState` a tout ce qu'il faut : `raw_log`, `before`, `mask`) ; (d) rattrapage optionnel, et quand il a lieu, prolonger la courbe (dernière direction lissée) plutôt qu'un segment droit.
- **Coût/risque** : (a)(b) faibles ; (c) moyen (une image de recalcul au levé, invisible pour un trait de taille normale ; à borner pour les très grosses brosses).

### 2.4 Modèle d'empreinte aux petites tailles : bord 1 px linéaire, « éclaircir » sous le pixel, dureté par `min()`
- **Preuve (observé)** : `brush.rs:1342-1352` — `coverage()` : rampe linéaire `radius − d + 0,5`, puis `min()` avec une chute smoothstep entre `inner` et `radius` ; `:1060` — `inner = min(r × dureté, r − 0,5)` ; `:1041-1045` — rayon < 0,75 → rayon forcé à 0,75 et débit multiplié par `r/0,75`. `vector.rs:463-485` : même modèle.
- **Effet ressenti** : une ligne de 1–2 px est un dégradé gris de 2–3 px de large ; les traits fins paraissent flous et pâles ; la jonction `min()` fait un léger « cran » sur les brosses demi-dures.
- **Références** : libmypaint « pixel feather » (fondu minimal à rayon optique conservé) ; Krita précision 1–5 du masque (§3).
- **Correction** : couverture par aire (ou 4 sous-échantillons) sous 2 px de rayon ; une seule courbe de bord au lieu du `min()` ; sous le pixel, alpha plein sur le pixel central.
- **Coût/risque** : moyen (coût CPU seulement sur les petites tailles ; tests d'image à mettre à jour).

### 2.5 Horodatage par image et vitesse bruitée
- **Preuve (observé)** : `app.rs:1388`, `1431` — tous les points d'une image d'écran reçoivent le même `i.time` ; `brush.rs:848-862` — vitesse lissée à 50 % par mesure. winit n'expose pas `PerformanceCount` de `POINTER_INFO`.
- **Effet ressenti** : seulement si une connexion « vitesse » est active (pas par défaut) : réponse en escalier. Latent, pas prioritaire.
- **Correction** : horodater à l'arrivée de l'événement (impossible sans toucher egui-winit) ou lire l'historique WM_POINTER soi-même (même chantier que l'inclinaison, à faire avec toi à la tablette).

### 2.6 Latence : bon en principe, deux points à vérifier
- **Preuve (observé)** : `lib.rs:333-336` — `AutoNoVsync`, une image d'avance ; `app.rs:3234-3245` — plafond 500 images/s par `thread::sleep(2 ms − écoulé)` ; `canvas.rs:158-171` — « élastique » entre dernier point posé et pointe réelle ; les empreintes sont calculées sur le fil de l'interface dans `update`.
- **À vérifier** : précision de `std::thread::sleep` sous Windows (minuterie 15,6 ms si le processus ne la relève pas ; les Rust récents utilisent une minuterie haute résolution — à mesurer) ; durée d'image avec une grosse brosse texturée (dispersion, papier, `count`).
- **Correction** : mesurer (étape 0) ; au besoin remplacer le `sleep` ; si le rendu CPU dépasse 4–5 ms, fil dédié ou carte graphique.

### 2.7 Calques vectoriels : le trait change au lâcher
- **Preuve (observé)** : `vtools.rs:235` — `simplify(path, 0.25)` ; `vector.rs:525-533` — re-rendu avec la **dureté seule** (pas de pression-opacité ni de grain) sauf si la brosse est texturée (`brushtex.rs:498-503`) ; `vector.rs:560-575` — la pression est **reconstituée depuis le rayon**. Calque par défaut : bitmap (`newlayer.rs:75`), donc secondaire.

### Ce qui n'est **pas** en cause (vérifié)
- Entrée : winit lit la pression 0–1024 (`event_loop.rs:2087-2099`), la position sous-pixel (`2103-2104`), **tout l'historique** d'une image (`1999-2030`) et renvoie 0 sans `DefWindowProc` (`2129`) : pas de doublons souris. egui-winit émet un `PointerMoved` par échantillon juste après son `Touch` (`egui-winit lib.rs:625-669`), que `canvas.rs:255-264`, `326-337` apparient. Pression 0 → `None` (`event_loop.rs:994-999`) : bénin.
- Moteur : position **et pression** interpolées entre deux événements (`brush.rs:970-1001`), pas 8 % du diamètre (`:454`) ; masque f32 recalculé depuis `before` (`:1255-1272`), pas d'accumulation d'arrondis u8 ; affichage sur-échantillonné au dézoom (`gpu.rs:258-274`).

## 3. Ce que fait TVPaint

**TVPaint 12 (tes réglages, `config.ini`, observé)**
- `[system] wintab=0` (`:8932`) ; la loc offre « Wintab / Ink » (`english.tvpaint.loc:5790-5797`) → TVPaint lit **Windows Ink** chez toi (déduit), même source que Pelure : la différence n'est pas dans la tablette.
- Outil courant (`[tablettools0tool]`, `:8973-9013`) : `radius 5.50`, `power 100`, `opacity 100`, `aliasing=0`, `dry=1`, `grain=1`, **`stepon=1 stepval=12.50`** (pas 12,5 % — Pelure 8 %), `consize2=10;…`, `conpower2=0;…`, profil « Crayon » à 3 points `(0,0) (0.0746,0.737) (1,1)` linéaire. Lecture du format `consize2` : `min;N x y points…` d'après l'étude précédente → min 10 %, courbe (0,0)→(1,1) : **déduit**. Sens de l'axe x du profil (centre→bord ou bord→centre) : **à vérifier** (si x = bord→centre, c'est un bord quasi net qui monte vite à 74 % puis doucement à 100 % au centre : un crayon dur avec un cœur un peu plus dense).
- Lissage (doc.tvpaint.com/docs/drawing-basic-tools/main-panel-tools/line-smoothing) : modes Average (Distance) / Pulled string / Average (Points) ; Real Time off = lissage **après** le trait ; HUD = assistant visible ; Catch Up (Average Points) rattrape l'assistant au levé (loc `:6557`).
- Moteur (doc.tvpaint.com, *Settings Cheat Sheet*) : Power = quantité de couleur déposée, Opacity = plafond de transparence, Step = distance entre répétitions, Profile = forme de la pointe, Aliasing on/off (algorithmes TVPaint 11 conservés), Drying. La formule Power/Opacité au chevauchement **n'est pas publiée** : garder celle de Pelure et la comparer sur des traits enregistrés.
- `[main]` `displaymode=Best`, `aapreviewquality=1`, `displaybrushdraw=off` (`:2887`, `2904-2906`) — sens **à vérifier**.

**Références libres (observé)**
- libmypaint (`brushsettings.json`, `mypaint-brush.c`) : sous-pas entre événements (x, y, pression, temps interpolés) ; 2 empreintes par rayon par défaut ; `slow_tracking` pondéré par le **temps écoulé** ; vitesse filtrée puis **log** ; `pressure_gain_log` ; `opaque_linearize 0.9` (ce que fait déjà `brush.rs:1053-1057`) ; `anti_aliasing` = largeur de fondu minimale à rayon optique conservé ; `stroke_threshold`.
- Krita (docs.krita.org, *Freehand Brush*) : Stabilizer avec **Smooth Pressure**, **Scalable Distance** (selon le zoom), Delay (zone morte pour les angles), Finish Line, mode Pixel ; pointe auto : précision 1–5, « Smooth lines » pour l'encrage.

## 4. Plan par étapes courtes (chacune avec un A/B à la tablette)

0. **Enregistrer et mesurer** (0,5 j) : Aide > Enregistrer mes traits ; 15 traits typiques (courbe lente, trait rapide, hachures, points, boucles) au crayon et au pinceau rond, à 50 % et 200 % de zoom. Puis `cargo test --release recorded_strokes_replay -- --ignored` avec `PELURE_TRAITS=…\traits.jsonl` (`banc_tests.rs:36-51`) pour les PNG, et un script sur le JSONL : dt entre instants distincts (= durée d'image réelle), pression des premier/dernier échantillons, doublons de position. **A/B** : les mêmes gestes dans TVPaint, exportés en PNG, côte à côte.
1. **Crayon anticrénelé** (0,5 j) : `pencil()` → `aliased false`, taille 2,5, `pressure_opacity` + `min_opacity 0,3`, lissage 0,15. **A/B** : une diagonale et une boucle à 100 %, ancien/nouveau crayon ; comparer au crayon TVPaint 1,5 px.
2. **Toucher du stylet** (1 j) : `min_size` 0,18 par défaut, plancher 0,1, courbe de pression globale (préférence, 4 points, prévisualisation) ; sous le pixel, alpha plein au centre. **A/B** : départs et fins de traits légers ; « est-ce qu'on retrouve le corps du trait TVPaint ? ».
3. **Début de trait** (0,5 j) : ignorer les empreintes tant que la pression < seuil (0,04) ou amorcer sur les deux premiers échantillons ; au levé, ne pas poser l'empreinte du dernier événement si sa pression chute. **A/B** : pointillés rapides, petits traits de hachure (pas de « goutte » au posé ni de pointe au levé).
4. **Lissage différé façon TVPaint** (1–2 j) : option « Lisser après le trait » : brut pendant le geste, rejoué lissé au levé depuis `raw_log` sur `before`. **A/B** : courbe longue et angle vif, avec/sans ; comparer à TVPaint Real Time off.
5. **Lissage indépendant de la cadence et du zoom** (1 j) : « Suivi » en constante de temps ; `smooth_px` en pixels d'écran (option) ; rattrapage courbe ou désactivable. **A/B** : même trait à 25 % et 400 % ; le crochet de fin a disparu ?
6. **Empreinte fine** (1–2 j) : couverture par aire sous 2 px, fondu minimal « pixel feather », une seule courbe de bord. **A/B** : lignes 1, 2, 3 px, diagonales et cercles, à 100 % et 400 %, contre TVPaint.
7. **Latence** (0,5 j mesure, 1 j correctif) : durée d'image depuis le banc ; test sans le `sleep` ; profil d'une grosse brosse texturée. **A/B** : trait rapide, distance pointe–trait à l'écran (photo ou vidéo au ralenti du téléphone).
8. **Calques vectoriels** (1 j, plus tard) : rendu au lâcher identique à l'aperçu (pression-opacité, profil), simplification 0,25 → ajustée au zoom.
9. **Inclinaison + horodatage par événement** (plus tard, avec toi) : lecture directe de WM_POINTER (`PerformanceCount`, `tiltX/Y`).

## 5. Questions pour Anthony / essais à faire

1. Quel outil déçoit le plus : le crayon, le pinceau rond, ou les brosses importées (Procreate « clouds » dans `pinceaux.json`) ? Et quel mot : « mou », « tremblé », « en retard », « maigre », « crénelé », « gris », « bouts pointus », « goutte au posé » ?
2. Zoom habituel pour dessiner (ajusté à la fenêtre ≈ 50 % ? ou 100–200 %) ? Taille de trait habituelle en pixels ?
3. Tablette et pilote : modèle (Wacom ? Cintiq ?), Windows Ink activé dans le pilote, courbe « Toucher de la pointe » du pilote (elle s'applique aux deux logiciels — à vérifier), cadence annoncée.
4. Confirmer dans TVPaint : Préférences > tablette = « Ink » ? Line Smoothing : quel mode affiché pour `smoothtype=2`, Real Time coché ou non ? (Le fichier dit : actif, 2,0, Real Time off.)
5. Essai décisif : enregistrer 15 traits dans Pelure (étape 0), refaire les mêmes dans TVPaint, et dire lesquels des quatre défauts (bouts, crénelage, tremblé/retard, flou des fins lignes) sautent aux yeux en premier — c'est l'ordre dans lequel on fera les étapes 1 à 6.

## 6. Fait / à essayer à la tablette (Fable, 2026-10-18, branche `brosses`)

Chaque changement garde l'ancien comportement à portée de main. Les réglages communs sont dans la section repliée « Toucher du stylet (tous les pinceaux) » au bas des réglages du pinceau, du crayon et de la gomme (gardés dans `reglages.toml`) ; les options de lissage sont sous le type de lissage de chaque pinceau. Rien n'a été vu à l'écran : tout est vérifié par les tests (`toucher_tests.rs`, `lissage_tests.rs`), le reste est à toi.

| # | Fait | Où | A/B à la tablette |
|---|---|---|---|
| 1 | Taille mini du Pinceau rond 0 → 18 % ; plancher de la taille 3 → 10 % (§2.1) | curseur Taille mini ; Toucher > Plancher de taille | Pinceau rond 6 px, traits légers, pointillés : « Ancien toucher » puis « Nouveau toucher ». Les bouts en fil et les départs fantômes ont-ils disparu sans que le trait perde sa finesse ? |
| 2 | Courbe de pression globale (Droite par défaut ; Douce, Ferme, ou à la main) (§2.1) | Toucher > courbe | Le même geste trois fois : Droite / Douce / Ferme. Laquelle rend le corps du trait TVPaint ? (La courbe « Toucher de la pointe » du pilote Wacom s'applique en plus, aux deux logiciels : à vérifier.) |
| 3 | Seuil d'appui 3 % : rien n'est posé au-dessous, au posé comme au levé (étape 3) | Toucher > Seuil d'appui | Pointillés rapides, hachures courtes : plus de point fantôme au posé ni de pointe au levé ? Un effleurement voulu passe-t-il encore ? Sinon baisser (0 = comme avant). |
| 4 | Plus fin qu'un pixel : un pixel plein (avant : un fil gris éclairci) (§2.4) | Toucher > Traits fins pleins | Crayon 1 px et pinceau effleuré, à 100 % et 400 % : la ligne a-t-elle du corps sans baver ? Décocher pour comparer. |
| 5 | Bord des empreintes : une seule chute douce, d'un pixel au moins (avant : rampe d'un pixel coupée par la dureté, un cran) (§2.4) | Toucher > Bord doux | Lignes 1, 2, 3 px, diagonales et cercles, à 100 % et 400 %, contre TVPaint. Une pointe dure fait maintenant sa taille vraie : un demi-pixel de plus qu'avant. |
| 6 | Crayon anticrénelé, 2,5 px, opacité selon l'appui (mini 30 %), lissage 0,15 (§2.2) | Crayon ; « Pixels nets » = l'ancien | Une diagonale et une boucle à 100 % : les escaliers ont disparu ? Comparer au crayon TVPaint 1,5 px. Hachures légères : le crayon mord-il moins, comme un vrai ? |
| 7 | Horodatage par événement : les échantillons d'une image sont étalés dans l'image (§2.5) | automatique | Avec une connexion « Vitesse » : la taille varie-t-elle sans escalier ? Dans le banc (Aide > Enregistrer mes traits), les instants du JSONL ne se répètent plus. |
| 8 | Suivi selon le temps (constante de temps ; cadence de référence 200 Hz, **à vérifier**) (§2.3 a) | Lissage > Selon le temps | Le même trait avec une grosse brosse texturée (image lente) et une petite (image rapide) : le lissage est-il le même ? Décocher = comme avant. Si le Suivi te semble plus mou ou plus raide qu'avant, le banc donne ta cadence réelle (dt entre instants) : on ajustera la référence. |
| 9 | Corde et distance en pixels d'écran (§2.3 b) | Lissage > Pixels d'écran | Corde tirée 12 px, le même trait à 25 % et 400 % : le trait traîne-t-il pareil sous la pointe ? Décocher = pixels du dessin, comme avant. |
| 10 | Rattrapage de la pointe au levé : en courbe (défaut), droit (avant), aucun (§2.3 d) | Lissage > liste « Rattrapage… » | Encrage à la moyenne (points ou distance), traits qui tournent juste avant le levé : le crochet a-t-il disparu ? Avec le Suivi et la corde, courbe = droit (leur dernier pas vise déjà la pointe). « Sans rattrapage » si tu préfères un trait qui s'arrête en retrait. |
| 11 | Lissage après le trait, comme TVPaint avec Real Time décoché (ton réglage) — **décoché par défaut** (§2.3 c) | Lissage > Après le trait | Suivi 0,5, une courbe longue et un angle vif, avec et sans. Le trait brut suit-il la pointe sans retard pendant le geste, puis se lisse-t-il au levé sans saut gênant ? Comparer à TVPaint. Si ça te va, on le cochera par défaut. |
| 12 | Sommeil du rythme des images mesuré sur ce poste : 0,25 ms de dépassement moyen sur 2 ms, 0,82 au pire (§2.6) | — | Rien à faire. Pour remesurer : `cargo test --release frame_pacing_sleep_is_precise -- --ignored --nocapture`. |

Pas fait, et pourquoi : la couverture par aire sous 2 px (le bord doux suffit pour juger ; à faire si les lignes fines paraissent encore molles) ; l'inclinaison et l'heure vraie des échantillons (lecture directe de WM_POINTER, `PerformanceCount`, à faire avec toi à la tablette) ; les traits vectoriels rejoués avec l'opacité selon l'appui et la simplification selon le zoom (§2.7 ; calque par défaut : bitmap). Le trait vectoriel garde l'ancien bord : au lâcher, il peut paraître un demi-pixel plus fin que l'aperçu bitmap — à voir si ça se remarque.

Étape 0, toujours à faire par toi : 15 traits (Aide > Enregistrer mes traits) et les mêmes gestes dans TVPaint. Les traits enregistrés gardent maintenant leur toucher et des instants étalés : le rejeu (`recorded_strokes_replay`) donne la même image qu'à l'écran.
