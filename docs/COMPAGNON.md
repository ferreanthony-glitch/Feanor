# Pelure Compagnon (Tauri 2 + React)

Une seconde appli, à côté de Pelure : le storyboard d'un projet en interface web moderne (Vite,
React, TypeScript, Tailwind), sur le **même noyau Rust** que Pelure. On y ouvre un projet, on
voit les panneaux par séquence et par scène (vignettes rendues par Pelure), on écrit l'action,
le dialogue et les notes, on règle les durées, on réordonne (glisser, ← →), on ajoute, duplique,
supprime, on enregistre et on exporte le storyboard en PDF. Le dessin reste dans Pelure.

## Lancer en développement

Une fois pour toutes (déjà fait sur ce poste) : Node.js LTS, puis dans `src-frontend` :

```
npm install
```

Ensuite, à chaque fois :

```
cd src-frontend
npm run tauri dev
```

Le script `tauri` remonte à la racine du dépôt (`cd .. && tauri`) : le CLI de Tauri y trouve
`src-tauri/tauri.conf.json`. `tauri dev` lance d'abord `npm run dev` (Vite, port 5173), compile
`src-tauri` puis ouvre la fenêtre. Une modification du React s'affiche aussitôt ; une
modification du Rust (noyau ou commandes) recompile et relance l'appli.

Ouvrir directement un projet : `npm run tauri -- dev -- -- "C:\chemin\Projet.pelure"` (le
premier `--` est pour npm, le second sépare les arguments du CLI de Tauri de ceux de l'appli).
L'exécutable accepte aussi un projet en argument (double-clic sur un `.pelure`). Projet
d'exemple (quatre panneaux dessinés, deux scènes) :
`$env:PELURE_EXEMPLE="C:\dossier\Exemple.pelure"; cargo test --release ecrire_un_projet_exemple -- --ignored`.

Construire l'exécutable : `npm run tauri build` (frontend construit dans `src-frontend/dist`,
puis `src-tauri/target/release/pelure-compagnon.exe` ; pas d'installateur : `bundle.active` est
à `false`).

## Ce qui a été ajouté, pas à pas

1. **Pelure devient aussi une bibliothèque** (`Cargo.toml` : section `[lib]`). Les déclarations
   de modules sont passées de `src/main.rs` à `src/lib.rs`, avec `pelure::lancer()` qui démarre
   l'application egui ; `main.rs` ne fait plus que l'appeler. Pelure est inchangée (mêmes tests).
2. **Le noyau** (`src/noyau.rs`) : `Projet` (document, manifeste, dessins déjà écrits) et ses
   fonctions en données simples — `ouvrir`, `nouveau`, `resume` (séquences > scènes >
   panneaux, avec titres, durées, textes), `vignette_png`, `ajouter_panneau`,
   `dupliquer_panneau`, `supprimer_panneau`, `deplacer_panneau`, `modifier_textes`,
   `regler_duree`, `enregistrer`, `exporter_pdf`. Rien d'egui ; testé (`noyau_tests.rs`).
3. **`src-tauri/`**, un projet Cargo à part (`[workspace]` vide dans son `Cargo.toml`, donc son
   propre `Cargo.lock` et son dossier `target`) : les dépendances de Tauri ne changent aucune
   version de celles de Pelure.
   - `Cargo.toml` : `tauri` 2, `tauri-build` 2 (compilation), `serde`, `rfd` (fenêtres
     Ouvrir / Enregistrer), et `pelure = { path = ".." }`.
   - `build.rs` : `tauri_build::build()` lit la configuration et prépare l'icône Windows.
   - `tauri.conf.json` : nom, identifiant (`fr.anthonyf.pelure.compagnon`), fenêtre (1280 × 820),
     `build.devUrl` (le serveur Vite) et `build.frontendDist` (`../src-frontend/dist`),
     `beforeDevCommand` / `beforeBuildCommand` (lancés dans `src-frontend`).
   - `capabilities/default.json` : la fenêtre `main` a les permissions de base (`core:default`).
     Les commandes propres à l'appli n'ont pas besoin d'autorisation en plus.
   - `src/main.rs` : l'état (`Etat(Mutex<Option<Projet>>)`, un projet à la fois), les fonctions
     `#[tauri::command]`, et leur enregistrement :

     ```rust
     tauri::Builder::default()
         .manage(Etat(Mutex::new(None)))
         .invoke_handler(tauri::generate_handler![nouveau_projet, ouvrir_projet, /* … */])
         .run(tauri::generate_context!())
     ```

     `generate_handler!` fabrique le répartiteur : un `invoke("ouvrir_projet", { chemin })` du
     frontend appelle la fonction du même nom, ses arguments étant lus du JSON par leur nom ; le
     résultat (ou l'erreur, une chaîne) repart en JSON. Les commandes lentes (ouvrir, vignette,
     PDF) sont `async` : elles ne figent pas la fenêtre. La vignette part en octets bruts
     (`tauri::ipc::Response`), pas en JSON.
4. **`src-frontend/`** : `package.json` (React 19, Vite 8, Tailwind 4 par son extension Vite,
   TypeScript 5.9, `@tauri-apps/api` et `@tauri-apps/cli` 2), `vite.config.ts` (port fixe 5173,
   `src-tauri` non surveillé), `src/api.ts` (une fonction typée par commande), `src/App.tsx`
   (l'interface).

## Commandes

| Commande | Arguments | Rend |
|---|---|---|
| `nouveau_projet` | `largeur`, `hauteur`, `fps` | le projet (`ResumeProjet`) |
| `ouvrir_projet` | `chemin` (ou rien : fenêtre du système) | projet + avertissements, ou rien si annulé |
| `resume_projet` | — | le projet |
| `vignette` | `index`, `largeur` | PNG (octets) |
| `ajouter_panneau`, `dupliquer_panneau`, `supprimer_panneau` | `index` | le projet |
| `deplacer_panneau` | `index`, `place` (0 = au début) | le projet |
| `modifier_textes` | `index`, `action`, `dialogue`, `notes` | le projet |
| `regler_duree` | `index`, `images` | le projet |
| `enregistrer_projet` | — | le projet |
| `exporter_pdf` | — (fenêtre du système) | chemin du PDF, ou rien si annulé |

## Attention

- Pelure et le compagnon peuvent ouvrir le même projet, mais chacun enregistre sa version :
  enregistrer dans l'un puis dans l'autre écrase le travail du premier. Fermer l'un avant
  d'enregistrer dans l'autre.
- Un projet neuf créé dans le compagnon n'a pas encore de dossier : « Enregistrer sous » n'y est
  pas (pour l'instant, le créer dans Pelure).
- `csp` est à `null` dans `tauri.conf.json` (comme le modèle officiel de Tauri) : l'appli ne
  charge que son propre frontend ; à resserrer si elle devait afficher du contenu extérieur.
