//! Pelure Compagnon (10b) : appli Tauri dont l'interface est en React (`src-frontend/`). Elle
//! partage le noyau de Pelure (`pelure::noyau`) : ouvrir un projet, voir et réorganiser le
//! storyboard, écrire les textes, régler les durées, enregistrer, exporter le PDF. Le dessin
//! reste dans Pelure.
//!
//! Chaque fonction marquée `#[tauri::command]` peut être appelée du frontend par
//! `invoke("nom", { arguments })` ; elles sont enregistrées dans `main` (`generate_handler!`).
//! Les arguments arrivent du JSON par leur nom ; les résultats y repartent (serde).

// Pas de fenêtre de console en plus sous Windows, en version finale.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;

use pelure::noyau::{Projet, ResumeProjet};
use serde::Serialize;
use tauri::ipc::Response;
use tauri::State;

/// Le projet ouvert (un seul à la fois), partagé par toutes les commandes.
struct Etat(Mutex<Option<Projet>>);

/// Applique `f` au projet ouvert.
fn avec<T>(etat: &State<Etat>, f: impl FnOnce(&mut Projet) -> Result<T, String>) -> Result<T, String> {
    let mut garde = etat.0.lock().map_err(|_| "Projet indisponible (erreur précédente).".to_string())?;
    let projet = garde.as_mut().ok_or("Aucun projet ouvert.")?;
    f(projet)
}

/// Applique `f`, puis rend le projet tel qu'il est devenu.
fn changer(etat: &State<Etat>, f: impl FnOnce(&mut Projet) -> Result<(), String>) -> Result<ResumeProjet, String> {
    avec(etat, |p| {
        f(p)?;
        Ok(p.resume())
    })
}

/// Résultat de l'ouverture : le projet, et les problèmes non bloquants (dessin manquant…).
#[derive(Serialize)]
struct Ouverture {
    projet: ResumeProjet,
    avertissements: Vec<String>,
}

#[tauri::command]
fn nouveau_projet(etat: State<Etat>, largeur: usize, hauteur: usize, fps: u32) -> Result<ResumeProjet, String> {
    let projet = Projet::nouveau(largeur, hauteur, fps);
    let resume = projet.resume();
    *etat.0.lock().map_err(|_| "Projet indisponible.".to_string())? = Some(projet);
    Ok(resume)
}

/// Ouvre `chemin` (un fichier .shot, ou l'ancien .pelure), ou celui choisi dans la fenêtre du système. Rien si on
/// annule. Commande asynchrone : un gros projet se lit sans figer la fenêtre.
#[tauri::command]
async fn ouvrir_projet(etat: State<'_, Etat>, chemin: Option<String>) -> Result<Option<Ouverture>, String> {
    let chemin = match chemin {
        Some(c) => PathBuf::from(c),
        None => match rfd::AsyncFileDialog::new().add_filter("Projet FEANOR", &["shot", "pelure"]).pick_file().await {
            Some(f) => f.path().to_path_buf(),
            None => return Ok(None),
        },
    };
    let (projet, avertissements) = Projet::ouvrir(&chemin)?;
    let resume = projet.resume();
    *etat.0.lock().map_err(|_| "Projet indisponible.".to_string())? = Some(projet);
    Ok(Some(Ouverture { projet: resume, avertissements }))
}

#[tauri::command]
fn resume_projet(etat: State<Etat>) -> Result<ResumeProjet, String> {
    avec(&etat, |p| Ok(p.resume()))
}

/// Vignette PNG du panneau `index`, `largeur` pixels de large, en octets bruts (le frontend en
/// fait une image). Asynchrone : le rendu se fait hors de la boucle de la fenêtre.
#[tauri::command]
async fn vignette(etat: State<'_, Etat>, index: usize, largeur: usize) -> Result<Response, String> {
    let png = avec(&etat, |p| p.vignette_png(index, largeur.clamp(32, 1024)))?;
    Ok(Response::new(png))
}

#[tauri::command]
fn ajouter_panneau(etat: State<Etat>, index: usize) -> Result<ResumeProjet, String> {
    changer(&etat, |p| p.ajouter_panneau(index).map(|_| ()))
}

#[tauri::command]
fn dupliquer_panneau(etat: State<Etat>, index: usize) -> Result<ResumeProjet, String> {
    changer(&etat, |p| p.dupliquer_panneau(index).map(|_| ()))
}

#[tauri::command]
fn supprimer_panneau(etat: State<Etat>, index: usize) -> Result<ResumeProjet, String> {
    changer(&etat, |p| p.supprimer_panneau(index).map(|_| ()))
}

/// Déplace le panneau `index` à la place `place` (0 = au début).
#[tauri::command]
fn deplacer_panneau(etat: State<Etat>, index: usize, place: usize) -> Result<ResumeProjet, String> {
    changer(&etat, |p| p.deplacer_panneau(index, place).map(|_| ()))
}

#[tauri::command]
fn modifier_textes(etat: State<Etat>, index: usize, action: String, dialogue: String, notes: String) -> Result<ResumeProjet, String> {
    changer(&etat, |p| p.modifier_textes(index, action, dialogue, notes))
}

#[tauri::command]
fn regler_duree(etat: State<Etat>, index: usize, images: usize) -> Result<ResumeProjet, String> {
    changer(&etat, |p| p.regler_duree(index, images).map(|_| ()))
}

#[tauri::command]
fn enregistrer_projet(etat: State<Etat>) -> Result<ResumeProjet, String> {
    changer(&etat, |p| p.enregistrer())
}

/// Exporte le storyboard en PDF à l'endroit choisi ; rend son chemin (rien si on annule).
#[tauri::command]
async fn exporter_pdf(etat: State<'_, Etat>) -> Result<Option<String>, String> {
    let nom = avec(&etat, |p| Ok(p.resume().nom))?;
    let Some(fichier) = rfd::AsyncFileDialog::new()
        .add_filter("PDF", &["pdf"])
        .set_file_name(format!("{nom}.pdf"))
        .save_file()
        .await
    else {
        return Ok(None);
    };
    let sortie = fichier.path().to_path_buf();
    avec(&etat, |p| p.exporter_pdf(&sortie, &nom))?;
    Ok(Some(sortie.display().to_string()))
}

fn main() {
    // Un projet passé en argument (un .shot, ou l'ancien .pelure) est ouvert au démarrage ;
    // le frontend le demande par `resume_projet`.
    let initial = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .and_then(|p| Projet::ouvrir(&p).map_err(|e| eprintln!("{} : {e}", p.display())).ok())
        .map(|(p, _)| p);
    tauri::Builder::default()
        .manage(Etat(Mutex::new(initial)))
        // Les commandes que le frontend peut appeler.
        .invoke_handler(tauri::generate_handler![
            nouveau_projet,
            ouvrir_projet,
            resume_projet,
            vignette,
            ajouter_panneau,
            dupliquer_panneau,
            supprimer_panneau,
            deplacer_panneau,
            modifier_textes,
            regler_duree,
            enregistrer_projet,
            exporter_pdf,
        ])
        .run(tauri::generate_context!())
        .expect("Pelure Compagnon n'a pas pu démarrer");
}
