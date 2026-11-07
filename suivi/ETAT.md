# FEANOR (ex-SHOT) — état réel du projet (relevé du 2026-10-30, mise à jour du 2026-11-05 ci-dessous)

## Mise à jour du 2026-11-06, branche `plan-annule` (pas fusionnée)
- Format de projet : **48** (`plans_annules.json`). Partie 114 d'INTERFACE : un clic sur le numéro écarte le plan (« Écarter », « Reprendre »), sa case reste barrée dans les Vignettes, il est absent partout ailleurs. À tester par Anthony.
- Essais de repos intermittents dans les passages complets (`vignettes_tests`, `aidetouches_tests`, `montageonglet_tests`, `concentration_tests`, `griffonnage_tests`) : verts seuls, vus aussi sans les essais de cette branche ; cause non trouvée.

## Mise à jour du 2026-11-06 au soir (main f5caddd) — elle prime sur la suite
- Format de projet : **47** (corbeille). Essais : **3981 réussis, 0 échec**, deux passages. Parties 101 à 113 d'INTERFACE en attente du test d'Anthony.
- Traits coupés : reproduits en pixels et corrigés (partie 111). Morceaux pâles des cases 15 et 16 : non reproduits, à confirmer à l'écran.
- Analyse des Vignettes : `suivi/ANALYSE-VIGNETTES.md`. Décision 6 (glisser dans B déplace B) en attente.

## Mise à jour du 2026-11-05 au soir (main 80738d2)
- Essais : **3751 réussis, 0 échec**, deux passages. Contenu de la V1 et écarts : `suivi/V1.md`.
- Gris sombre est le thème de base. L'animation n'a plus d'onglet (partie 100). ffmpeg allégé dans `Logiciel\ffmpeg\allege`.

## Mise à jour du 2026-11-05 (main 9c3e516)
- Nom : **FEANOR** ; exécutable `target\release\feanor.exe`. Essais : **3744 réussis, 0 échec**. Format de projet : 46.
- Fusionnés, en attente du test d'Anthony : identité FEANOR (INTERFACE partie 96), mémoire d'une longue séance (partie 97), sortes de calques en couleurs FEANOR (partie 98).

## Mise à jour du 2026-10-30 au soir (main 1df74a1) — elle prime sur la suite

- **Storyboard Pro et Toon Boom retirés** (17dda2a) : import `.sboard`, export, import de pinceaux, raccourcis et presets.
  Tout ce qui les cite plus bas (§ « Échange avec Storyboard Pro », pinceaux Toon Boom, keymap SBP, essais SBP) est **périmé**.
- Format de projet : **44** (dégradé avancé). Essais : **3425 réussis, 0 échec** à la fusion de 1df74a1.
- Fusionnés aujourd'hui, en attente du test d'Anthony : panneau Scénario + recherche, ajustements Vignettes, Vignettes par
  séquences, hors-cadres texturé + passe-partout à l'export, Dégradé avancé, planche de Références.
- Branches : `memoire-dessins` **abandonnée** (choix d'Anthony), gardée en étiquette `archive/memoire-dessins` ;
  `sans-logo-barre` à fusionner. Dossiers de travail : `Chantiers\references`, `calques`, `bibliotheque`, `exports`, `vignettes`.
- Point global du jour (ce qui marche, à améliorer, questions ouvertes) : dans la conversation et résumé dans JOURNAL.md.

---

Relevé en lecture seule sur `main` (commit b201dc1). Sources : git, docs/ROADMAP.md, docs/INTERFACE.md (parties 1 à 74),
le code (`src/`), les essais et les commandes lancées ce jour. « à confirmer » = je n'ai pas pu le vérifier.
Note : ROADMAP.md n'est plus tenu après le 2026-10-21 ; depuis, chaque chantier est décrit dans INTERFACE.md.

## 1. Fiche

| | |
|---|---|
| Dernier commit | 2026-10-29 21:38, b201dc1 « Fusion de accueil-fenetre » |
| Premier commit | 2026-10-14 |
| Nombre de commits | 790 (sur main, fusions comprises) |
| Plateforme visée | Windows 64 bits seulement (compilation MSVC, `rust-toolchain.toml`) ; tablette par Windows Ink |
| Taille du code | 624 fichiers dans `src/`, dont 280 fichiers d'essais |
| Format de projet | version 43 (dossier de projet, extension `.shot`) |
| cargo check | OK, aucun avertissement, aucune erreur (24,9 s) |
| cargo test | **3331 réussis, 0 échec, 123 ignorés** (136 s). Les 123 ignorés sont des bancs de mesure et essais lourds lancés à la main (à confirmer un par un) |
| Branches non fusionnées | `memoire-dessins` (2026-10-24, dessins gardés hors de la mémoire), `essai-complet` (2026-10-18, ancienne) |
| Dossiers de travail ouverts | `Chantiers\demarrage`, `Chantiers\importsbp2` (déjà fusionnés, à supprimer avec ton accord) |

## 2. Fonctionnalités par phase

Statuts : Fait / En cours / À faire / Cassé / Retiré. « Fait » = fusionné et couvert par des essais ; le rendu
à l'écran reste à vérifier par toi (section 5). Date = dernière modification du module principal (git).

### Storyboard (plan initial, phases 1, 13)
| Fonction | Statut | Ce que tu peux faire | Preuve | Date |
|---|---|---|---|---|
| Séquences, plans, panneaux | Fait | créer, dupliquer, couper (F5), réunir, déplacer, numéroter (suivie ou verrouillée) | ROADMAP Ph.1, 13a ; INTERFACE §16 | 2026-10-29 (`timeline.rs`) |
| Vignettes (planche) | Fait | planche de cases, sens de lecture, textes à côté, annotations, export de planche, color script | INTERFACE §11, §44, §46 | 2026-10-29 (`vignettesreglages.rs`) |
| Textes du plan (action, dialogue, notes) | Fait | saisir, annuler, exporter en PDF | ROADMAP 1d, 3c | 2026-10-21 (`pdf.rs`) |
| Importer un scénario | Fait | créer les plans depuis un scénario | INTERFACE §10 | 2026-10-19 (`scenario.rs`) |
| Variantes et versions de séquence / de plan | Fait | essayer un autre découpage sans perdre le premier, comparer | INTERFACE §15, §36, §59 | 2026-10-29 (`envois.rs`) |
| Relecture par le réalisateur, envois | Fait | marquer les plans modifiés, envoyer une version | INTERFACE §15, §22 | 2026-10-29 |
| Fiche technique (tableur) | Fait | découpage du film en tableau | INTERFACE §26 | 2026-10-20 (`fiche.rs`) |
| Références visuelles | Fait | fenêtre « Références » | ROADMAP l.2259 | 2026-10-19 |

### Caméra et 3D (plan initial, phase 2)
| Fonction | Statut | Ce que tu peux faire | Preuve | Date |
|---|---|---|---|---|
| Caméra du plan | Fait | cadrer, panoramique, zoom, rotation, clés, adoucis, caméra par paliers, passe-partout | ROADMAP 2b ; INTERFACE §47, §51 | 2026-10-24 (`camera.rs`) |
| Mouvements de caméra dans les vignettes | Fait | voir le mouvement dans la case | INTERFACE §11 ; `vignettecam_tests.rs` | 2026-10-24 |
| Profondeur des calques (parallaxe) | Fait | éloigner un calque, il défile moins vite | ROADMAP 2c | 2026-10-21 (`espace3d.rs`) |
| Espace 3D simplifié | Fait | placer les calques comme des cartes dans l'espace, caméra 3D | ROADMAP l.3429 ; INTERFACE §13 | 2026-10-21 |
| Objets 3D, modèle glTF de repère | Retiré | — (supprimés à ta demande le 2026-10-21) | ROADMAP l.3439 | 2026-10-21 |

### Animatique et export (plan initial, phase 3)
| Fonction | Statut | Ce que tu peux faire | Preuve | Date |
|---|---|---|---|---|
| Lecture avec son | Fait | lire le montage calé sur le son | ROADMAP 3a, 1b | 2026-10-19 (`audio.rs`) |
| Pistes son, transitions, bruitages | Fait | importer WAV/MP3/FLAC/OGG, couper, volume, fondus et transitions | ROADMAP 1b, l.2791 | 2026-10-19 |
| Export film / images / son | Fait | MP4, MOV ProRes, PNG, WAV ; le projet reste utilisable pendant l'export | ROADMAP 3b ; INTERFACE §28 | 2026-10-28 (`export.rs`) |
| Export PDF du storyboard | Fait | A4 paysage, 6 cases par page | ROADMAP 3c | 2026-10-21 |
| Export pour le montage | Fait | EDL, Final Cut XML, OpenTimelineIO | INTERFACE §17 | à confirmer |
| Montage (timeline des plans) | Fait | rogner, Alt+glisser pour dupliquer, fusionner deux plans en glissant, pistes repliables | INTERFACE §29, §52, §55, §60, §68 | 2026-10-29 |
| Génération de sons témoins | En cours | propositions de sons depuis le storyboard ; voix locale essayée | ROADMAP l.2823 | 2026-10-19 — bruitages générés absents, envoi en ligne « à brancher » |

### Dessin et animation (phases 3d, 7, 11, 13, moteur façon TVPaint)
| Fonction | Statut | Ce que tu peux faire | Preuve | Date |
|---|---|---|---|---|
| Pinceaux, brosses perso, brosses par personnage et décor | Fait | dessiner, régler, importer des pinceaux Toon Boom (XML) | INTERFACE §32, §45, §70 | 2026-10-28 (`brush.rs`) |
| Calques bitmap et vectoriels | Fait | dessiner, sélectionner, transformer, copier-coller entre calques | ROADMAP 1c ; INTERFACE §69 | 2026-10-29 (`vector.rs`) |
| Aplat sous le trait (et aplat intelligent) | Fait | colorier sous le trait, l'aplat suit les retouches et les clés suivantes, sans IA | INTERFACE §33, §56, §63 | 2026-10-29 |
| Mélange de couleurs, dégradé, pipette | Fait | pigment qui se mélange, outil Dégradé, pipette partout | INTERFACE §53, §58, §65 | 2026-10-29 |
| Effets du trait et de matière, tremblement | Fait | crayonné, encre qui bave, tremblement par défaut… | INTERFACE §20, §21, §48 | à confirmer |
| Grab et Pull | Fait | déformer le trait au pinceau, sur plusieurs clés | INTERFACE §43 | 2026-10-24 |
| Timeline d'animation à la TVPaint | Fait | images et expositions, signets, entrée/sortie, onglets du bas | ROADMAP 13b–13e | 2026-10-29 |
| Pelure d'oignon / table lumineuse | Fait | 12 images de chaque côté, couleurs au choix | ROADMAP 3d-1, 13f | 2026-10-19 (`onion.rs`) |
| Feuille d'exposition | Fait (13g-1) | colonnes image, son, calques | ROADMAP 13g | 2026-10-19 (`xsheet.rs`) |
| Clés d'opacité, calque « Couleur unie » | Fait | animer l'opacité d'un calque ; calque de couleur pleine sur lequel on dessine | INTERFACE §72 | 2026-10-29 |
| Calques d'effets et préréglages | Fait | effets sur la carte graphique, copier-coller d'effets | INTERFACE §12 | à confirmer |
| Toile infinie | Fait | dessiner hors de la feuille | INTERFACE §19 | à confirmer |
| Propagation façon EbSynth | Fait (Labo) | propager une clé peinte sur les images suivantes | docs/PROPAGATION.md | 2026-10-18 |

### Confort et interface (phases 4, 5, 6, 12, passes d'interface)
| Fonction | Statut | Ce que tu peux faire | Preuve | Date |
|---|---|---|---|---|
| Copie de secours, récupération après plantage | Fait | rouvrir après plantage ; enregistrer par-dessus sans perte | ROADMAP 4a ; INTERFACE §67 | 2026-10-28 (`recovery.rs`) |
| Fenêtre d'accueil compacte, démarrage sans projet | Fait | créer ou ouvrir un projet depuis une seule petite fenêtre | INTERFACE §73, §74 | 2026-10-29 |
| Onglets de projets | Fait | plusieurs projets ouverts | INTERFACE §23 | à confirmer |
| Espaces de travail Vignettes / Storyboard / Animation / Montage | Fait | changer d'espace en haut à droite | INTERFACE §14 | à confirmer |
| Préférences en onglets, thème SHOT | Fait | régler l'interface, les images d'accueil | INTERFACE §30, §61 | à confirmer |
| Clic droit = réglages de l'outil | Fait | régler l'outil sous le curseur | INTERFACE §66 | à confirmer |
| Mon panneau | En cours (Labo) | panneau avec tes commandes ; décision « sortir du Labo ? » en attente | INTERFACE §64 | 2026-10-28 |
| Travailler à plusieurs (dossier partagé) | Fait (cœur) | verrou par plan sur un dossier partagé ; serveur plus tard | INTERFACE §27 | 2026-10-21 |

### Échange avec Storyboard Pro (phase 14, ajoutée)
| Fonction | Statut | Ce que tu peux faire | Preuve | Date |
|---|---|---|---|---|
| Import d'un `.sboard` sans Storyboard Pro | Fait, avec pertes | dessins, calques, caméras, son, groupes en dossiers | INTERFACE §71 ; docs/IMPORT-SBP.md | 2026-10-29 (`sbnative.rs`) |
| Projets importés lourds | Fait (à tester) | mémoire divisée par 2, import 10× plus rapide | merge 6a2ddde | 2026-10-29 |
| Export vers Storyboard Pro (14b-1) | Fait | exporter un projet SHOT | ROADMAP 14b-1 | 2026-10-24 (`sbexport.rs`) |
| Calques vectoriels en vecteurs à l'export (14b-2) | À faire | — | ROADMAP l.818 | — |

### Hors logiciel principal
| Fonction | Statut | Preuve |
|---|---|---|
| Appli compagnon Tauri | Fait (phase 10), banc d'essai d'interface seulement | `src-tauri/`, `src-frontend/` présents |
| Fichier de projet unique `.shot` | À faire (reporté, attend ton feu vert) | décision du 2026-10-28 |
| Installateur Windows | À faire (après V1) | — ; version portable existe |

## 3. Dernières itérations (15, regroupées)

**Import Storyboard Pro (2026-10-29)**
- b8b54b6 : tu peux ouvrir un `.sboard` sans avoir Storyboard Pro ; dessins, calques, caméras et son arrivent.
- e1dff5a : la vue n'est plus blanche sur les grands formats ; les cadrages correspondent aux vignettes de Storyboard Pro.
- d5db5f1 : les couleurs des dessins bitmap importés sont justes, et il manque moins de morceaux.
- 6a2ddde : un gros projet importé s'ouvre en ~17 s au lieu de 3 min et prend 2 fois moins de mémoire.

**Démarrage (2026-10-29)**
- 82feaad : SHOT s'ouvre sans projet ; un seul thème, qui ne suit plus Windows.
- b201dc1 : au lancement, seule une petite fenêtre d'accueil apparaît (nouveau projet, récents).

**Calques (2026-10-29)**
- 27f31ec : l'opacité d'un calque s'anime par clés ; nouveau calque « Couleur unie ».
- 4b42226 : une image collée devient un objet sur un calque vectoriel ; copier-coller de plusieurs calques.
- a289602 : brosses perso rangées en personnage / classique / décor.

**Aplat (2026-10-28/29)**
- 020f8a2 : l'aplat suit le trait quand tu retouches, et d'une clé à l'autre.
- f77782a : la couleur d'aplat d'un personnage se change à un seul endroit, tous ses calques suivent.

**Montage et timeline (2026-10-28/29)**
- 46bde95 : seul le calque actif est déplié dans le montage, plus lisible.
- a5877e1 : Alt+glisser duplique ; nouvelle séquence à partir des plans choisis.

**Sûreté (2026-10-29)**
- 57c04bf : enregistrer par-dessus un projet garde l'ancien à côté ; un échec ne ferme plus rien en silence.
- ba90f0a : cinq essais qui échouaient au hasard sont fiabilisés.

## 4. Points fragiles

- **Essais instables sous charge** (passent seuls) : fondfil, aplat au repos, griffonnage, toile coupée,
  numérotation d'un long film, réserve privée de l'export, annulation des fournisseurs, budget de l'Espace 3D, fumée.
  Cinq ont été réparés le 2026-10-29 ; les autres sont à confirmer.
- **Plantages connus par motif** : changer la structure (supprimer, réunir des plans) pendant qu'un écran garde un
  ancien numéro de plan. Deux cas corrigés le 2026-10-22 ; une passe de débogage complète est prévue après la V1.
- **Fenêtre fermée toute seule** après un enregistrement refusé (2026-10-28) : cause non trouvée.
- **Import Storyboard Pro incomplet** : traits ouverts sans couleur non repris, textures de crayon non lues,
  opacité animée réduite à sa valeur fixe, taches vertes (plan 001), gris rendus blancs (animatic, plan 7).
  L'import ne crée pas encore de clés d'opacité ni de calques Couleur unie.
- **Mémoire** : un gros projet importé prend encore ~6 Go de dessins sur 7 plans carrés. La branche `memoire-dessins`
  (non fusionnée depuis le 2026-10-24) traite ce sujet ; son état exact est à confirmer.
- **Non vérifié à l'écran** : clics dans la fenêtre d'accueil, agrandissement / retour, plusieurs écrans, 150 %.
- **Clés d'opacité** : pas de copier-coller entre calques, l'export vers Storyboard Pro les ignore, la propagation aussi.
- **Génération sonore** : pas de bruitage généré, envoi en ligne non branché.
- **TODO / FIXME dans le code** : aucun (recherche dans `src/`, un seul « XXX » dans `envois.rs`, non vérifié).
- **Décisions difficiles à défaire** : format de projet en dossier (passage au fichier unique reporté) ; feuille
  plafonnée à 8192 px ; 3D retirée (anciens fichiers 3D gardés mais plus lus) ; Windows seulement.

## 5. À vérifier à la main (je ne peux pas le voir)

1. Dessin au stylet : latence, pression, la croix reste collée au trait.
2. Pinceaux texturés, mélange de couleurs, tremblement : rendu « TVPaint » attendu.
3. Aplat intelligent : il suit une retouche et passe bien à la clé suivante.
4. Caméra : mouvement en lecture, paliers, passe-partout, parallaxe dans l'export MP4.
5. Espace 3D : déplacer les cartes, regarder par la caméra.
6. Fenêtre d'accueil : créer un projet, la fenêtre s'agrandit ; fermer, elle revient ; récents ; engrenage.
7. Ton fichier Storyboard Pro de 7 plans : fluidité, mémoire dans le Gestionnaire des tâches, ressemblance des dessins.
8. Lecture du montage avec le son, sans décalage.
9. Export MP4 et PDF : ouvrir les fichiers et regarder.
10. Couleur unie et clés d'opacité : un fondu au noir visible en lecture.
Les listes détaillées « à vérifier à l'écran » sont à la fin de chaque partie de docs/INTERFACE.md.

## 6. Raccourcis

- `keymap.toml` : 602 raccourcis. Les 65 touches d'usine de Storyboard Pro 27 et tes 8 raccourcis perso y sont,
  avec leur origine notée ; aucun raccourci codé en dur. Un essai vérifie qu'aucun n'est oublié ni inventé.
- Presets Storyboard Pro dans le dépôt : `docs/reference/sbpro_presets/` contient `STB_Shortcuts.xml` (tes 8 perso)
  et `STB_Preferences.xml`. Le fichier d'usine `sb_shortcuts.xml` n'est pas copié (il appartient à Toon Boom).
- `keymap-storyboard-pro.toml` (jeu « Façon Storyboard Pro ») : **0 raccourci redéfini** pour l'instant ; il ne
  fait que lister les « à vérifier ».
- Import d'un fichier de raccourcis Storyboard Pro depuis le logiciel : **n'existe pas** (aucun code trouvé).
- Changer un raccourci : clic droit sur une icône ou un menu, « Assigner un raccourci… » ; ta copie va dans
  `%APPDATA%\SHOT\keymap.toml`.

## 7. Prochaines étapes

1. **Tes tests de la fenêtre d'accueil et du projet importé**. Taille : petit. Risque : faible. Débloque le reste.
2. **Import Storyboard Pro → clés d'opacité et Couleur unie**. Le code des deux existe déjà. Taille : petit/moyen.
   Risque : faible.
3. **Reprendre ou abandonner `memoire-dessins`** (dessins hors de la mémoire). Taille : moyen/gros. Risque : moyen,
   la branche a 6 jours de retard sur main.
4. **Passe de stabilisation** (essais instables, plantages par numéro de plan). Taille : moyen. Risque : faible.
5. **Fichier de projet unique `.shot`**, quand tu donnes le feu vert. Taille : gros. Risque : élevé (format des projets).
