# Architecture de Pelure

Document tenu à jour à chaque étape. Dernière mise à jour : phase 1b (octobre 2026).

## Vue d'ensemble

Application de bureau en Rust. Une seule fenêtre, interface **egui 0.31** (via eframe, affichage OpenGL).

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
- **Panneau** : durée, textes (action, dialogue, notes) et calques. Un calque associe à chaque image
  un dessin ; plusieurs images consécutives sur le même dessin forment une exposition.
- **Dessins** : bitmap = tuiles RGBA prémultipliées allouées à la demande (`raster.rs`) ;
  vectoriel = traits à épaisseur variable et zones remplies (`vector.rs`), re-rendus en pixels à chaque
  modification. Toute la composition ne voit donc que des pixels.
- **Rendu** (`render.rs`) : sur le processeur. Les calques et la pelure d'oignon sont empilés dans une
  image envoyée à la carte graphique comme une seule texture ; seules les tuiles modifiées sont
  recalculées. Vignettes des plans en cache (`thumbs.rs`).
- **Annuler / rétablir** (`history.rs`) : chaque modification enregistre l'état d'avant (tuiles,
  dessin vectoriel, ou structure : séquences, scènes, panneaux, calques, pistes son et extraits). Les
  noms, textes, réglages de calque et de piste (muette, volume) n'y sont pas encore (prévu en
  phase 1d), mais une annulation ne les fait jamais revenir en arrière (conservés par identifiant).
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

## Format de projet (version 3, dossier)

```
MonFilm/
  MonFilm.pelure      manifeste JSON : {"format": "pelure-projet", "version": 3, réglages,
                      "sequences": [{ "scenes": [{ "panneaux": [{ "calques": [{ "expositions": … }]}]}]}],
                      "sons": [{"id", "nom", "fichier"}], "pistes_son": [{ "extraits": [{"son", "debut",
                      "decalage", "duree"}] }]}
  dessins/<id>.png    dessin bitmap (RGBA non prémultiplié)
  dessins/<id>.json   dessin vectoriel (éléments sérialisés)
  sons/<id>.<ext>     fichier son importé, copié tel quel (wav, mp3, flac, ogg)
```

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
  ce qu'il redéfinit. Menu « Aide > Modifier / Recharger les raccourcis ».
- Les raccourcis viennent de Storyboard Pro 27 (65, par défaut) et des réglages perso (8). Ceux dont
  la fonction n'existe pas encore sont gardés avec `etat = "non implémenté"`. Les ajouts de Pelure
  sont marqués `source = "proposition"`.
- Lettres = caractère tapé ; chiffres et ponctuation = position physique (clavier AZERTY).
  Détails : `docs/reference/sbpro_presets/README.md`.
- Un fichier illisible ou une action inconnue ne bloque rien : remarques affichées dans l'aide (F1).

## Compilation

Toolchain Rust `x86_64-pc-windows-gnu` (MinGW minimal de rustup). Conséquences :

- `Cargo.lock` fige `parking_lot` 0.12.3, `parking_lot_core` 0.9.10, `libloading` 0.8.6 (les
  versions récentes exigent un `dlltool` complet). Ne pas lancer `cargo update` sans vérifier
  `cargo tree -i windows-link --target x86_64-pc-windows-gnu`.
- `link/libshlwapi.a` + `build.rs` fournissent une bibliothèque d'import manquante.

## Décisions (coût / risque / réversibilité)

| Décision | Pourquoi | Coût | Risque | Réversible ? |
|---|---|---|---|---|
| egui 0.31, pas de mise à jour pour l'instant | stable, déjà en place | nul | moyen à terme (retard sur les versions) | oui, coût croissant avec la taille du code |
| Rendu sur processeur en phase 1, carte graphique (wgpu) en phase 2 | la 3D et le multiplan l'exigent ; inutile avant | réécriture de `render.rs` en phase 2 | moyen | oui (le modèle de données ne change pas) |
| Toolchain GNU → **outils Microsoft (MSVC) avant la phase 2** (validé) | plus de contournements à chaque bibliothèque | 5–7 Go, une fois | faible | oui |
| **Projet = dossier** (validé, fait en 1a) | sons, 3D, sauvegarde auto incrémentale | lecture des anciens `.pelure` gardée | moyen | format versionné : les évolutions futures se font par ajout de champs |
| **Séquence > Scène > Panneau** (validé, fait en 1a) | vocabulaire de Storyboard Pro | refonte du modèle | moyen | l'arbre du fichier est stable ; la liste à plat en mémoire peut changer sans toucher au fichier |
| Raccourcis en TOML, identifiants stables | modifiables sans recompiler | faible | faible | oui |
| Son : **symphonia** (décodage) + **cpal** (sortie) (fait en 1b) | en Rust pur, sans DLL à fournir ; formats courants (WAV, MP3, FLAC, OGG) ; compilent avec la toolchain actuelle sans toucher aux versions figées | ≈ 3,5 Mo d'exécutable en plus (17 → 20,5 Mo) | faible (bibliothèques très utilisées) ; pas d'AAC/M4A | oui : tout passe par `audio.rs` |
| Son décodé entier en mémoire, 16 bits | simple, lecture et défilement instantanés | ≈ 11 Mo par minute de son | élevé seulement pour des heures de son | oui : lecture par morceaux possible plus tard sans changer le format |
| Horloge de lecture = position du son | l'image ne peut pas dériver du son | nul | faible (repli sur l'horloge si pas de sortie) | oui |
| Sons en temps de montage (pas liés aux panneaux) | comportement le plus simple et prévisible | nul | moyen : un panneau rallongé décale l'image par rapport au son | oui : le lien extrait-panneau pourra s'ajouter (champ en plus) |
| Export vidéo via ffmpeg installé à part (phase 3) | seule voie fiable sans souci de licence ni de compilation | dépendance externe | faible | oui |
| Pas d'export XML Toon Boom | format propriétaire non documenté | — | — | — |
