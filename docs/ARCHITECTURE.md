# Architecture de Pelure

Document tenu à jour à chaque étape. Dernière mise à jour : phase 0 (octobre 2026).

## Vue d'ensemble

Application de bureau en Rust. Une seule fenêtre, interface **egui 0.31** (via eframe, affichage OpenGL).

```
Document ─ plans (Shot) ─ calques (Layer : bitmap ou vectoriel) ─ une case par image → dessin (Cel)
                                                                        │
Cel = image en tuiles 64×64 (+ éléments vectoriels si calque vectoriel, rendus dans l'image)
```

- **Modèle** (`document.rs`) : le projet contient des plans montés bout à bout. Chaque plan a sa
  durée, ses textes (action, dialogue, notes) et ses calques. Un calque associe à chaque image un
  dessin ; plusieurs images consécutives sur le même dessin forment une exposition.
- **Dessins** : bitmap = tuiles RGBA prémultipliées allouées à la demande (`raster.rs`) ;
  vectoriel = traits à épaisseur variable et zones remplies (`vector.rs`), re-rendus en pixels à chaque
  modification. Toute la composition ne voit donc que des pixels.
- **Rendu** (`render.rs`) : sur le processeur. Les calques et la pelure d'oignon sont empilés dans une
  image envoyée à la carte graphique comme une seule texture ; seules les tuiles modifiées sont
  recalculées. Vignettes des plans en cache (`thumbs.rs`).
- **Annuler / rétablir** (`history.rs`) : chaque modification enregistre l'état d'avant (tuiles,
  dessin vectoriel, ou structure des plans). Les textes et réglages de calque n'y sont pas encore
  (prévu en phase 1d), mais une annulation ne les fait jamais revenir en arrière.
- **Fichier** (`io.rs`) : `.pelure` = en-tête + manifeste JSON versionné + PNG. Les versions 1 et 2 se lisent.
- **Raccourcis** (`keymap.rs`, `keymap.toml`) : voir plus bas.
- **Interface** : `app.rs` (état, actions, `run_action`), `canvas.rs` (dessin à la souris/stylet),
  `panels.rs` (menus, outils, calques, dialogues), `timeline.rs` (animation), `storyboard.rs` (montage,
  planches, fiche du plan), `vtools.rs` (outils vectoriels).

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
| **Projet = dossier** (validé, mise en œuvre phase 1a) | sons, 3D, sauvegarde auto incrémentale | migration des `.pelure` (lecture gardée) | moyen | coûteux une fois en usage : format versionné dès le départ |
| **Séquence > Scène > Panneau** (validé, phase 1a) | vocabulaire de Storyboard Pro | refonte du modèle | moyen | difficile après la phase 1a |
| Raccourcis en TOML, identifiants stables | modifiables sans recompiler | faible | faible | oui |
| Export vidéo via ffmpeg installé à part (phase 3) | seule voie fiable sans souci de licence ni de compilation | dépendance externe | faible | oui |
| Pas d'export XML Toon Boom | format propriétaire non documenté | — | — | — |
