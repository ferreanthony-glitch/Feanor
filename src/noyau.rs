//! Noyau de Pelure pour d'autres interfaces (10a) : ce dont l'appli compagnon Tauri a besoin, en
//! données simples (chaînes, nombres, octets PNG), sans rien d'egui. Les fonctions s'appuient
//! sur le modèle (`document.rs`), le projet en dossier (`project.rs`), le rendu (`render.rs`) et
//! l'export PDF (`export.rs`, `pdf.rs`) : les mêmes que l'application de dessin.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::document::{CelId, Document};
use crate::storyboard::{duration_label, timecode};
use crate::{export, pdf, project, render};

/// Un projet ouvert : le document, son manifeste, et les dessins déjà écrits dans son dossier
/// (pour n'enregistrer que ce qui change).
pub struct Projet {
    pub doc: Document,
    pub chemin: Option<PathBuf>,
    written: HashMap<CelId, u64>,
    /// Modifié depuis l'ouverture ou le dernier enregistrement.
    pub modifie: bool,
}

/// Un panneau, tel qu'une interface l'affiche.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ResumePanneau {
    pub id: u64,
    /// Rang dans le film (0 = premier).
    pub index: usize,
    /// « Sc 1 · P 2 ».
    pub titre: String,
    pub images: usize,
    /// « 2s 04 ».
    pub duree: String,
    /// Début dans le film, « 0:12:05 ».
    pub debut: String,
    pub action: String,
    pub dialogue: String,
    pub notes: String,
    pub calques: usize,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ResumeScene {
    pub id: u64,
    /// Numéro ou nom (« 3 », « Poursuite »).
    pub nom: String,
    pub panneaux: Vec<ResumePanneau>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ResumeSequence {
    pub id: u64,
    pub nom: String,
    pub scenes: Vec<ResumeScene>,
}

/// Tout le projet, pour l'affichage.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ResumeProjet {
    pub chemin: Option<String>,
    /// Nom du projet (celui du manifeste), ou « Sans titre ».
    pub nom: String,
    pub cadre: [usize; 2],
    pub fps: u32,
    pub images: usize,
    pub duree: String,
    pub modifie: bool,
    pub sequences: Vec<ResumeSequence>,
}

impl Projet {
    /// Projet neuf : cadre `largeur`×`hauteur`, `fps` images par seconde.
    pub fn nouveau(largeur: usize, hauteur: usize, fps: u32) -> Projet {
        let (w, h) = (largeur.clamp(16, 4096), hauteur.clamp(16, 4096));
        Projet { doc: Document::with_frame(w, h, fps.clamp(1, 120)), chemin: None, written: HashMap::new(), modifie: false }
    }

    /// Ouvre le projet dont `manifeste` est le fichier `.pelure` ; les problèmes non bloquants
    /// (dessin manquant…) sont rendus à part.
    pub fn ouvrir(manifeste: &Path) -> Result<(Projet, Vec<String>), String> {
        let loaded = project::load(manifeste)?;
        let projet = Projet { doc: loaded.doc, chemin: Some(manifeste.to_path_buf()), written: loaded.written, modifie: false };
        Ok((projet, loaded.warnings))
    }

    /// Enregistre dans le dossier du projet (seulement les dessins qui ont changé).
    pub fn enregistrer(&mut self) -> Result<(), String> {
        let chemin = self.chemin.clone().ok_or("Ce projet n'a pas encore de dossier.")?;
        project::save(&self.doc, &chemin, &mut self.written)?;
        self.modifie = false;
        Ok(())
    }

    pub fn resume(&self) -> ResumeProjet {
        let doc = &self.doc;
        let mut sequences: Vec<ResumeSequence> = Vec::new();
        for (index, panel) in doc.panels.iter().enumerate() {
            let seq = doc.sequence_of(index);
            if sequences.last().is_none_or(|s| s.id != seq) {
                sequences.push(ResumeSequence { id: seq, nom: doc.sequence_label(seq), scenes: Vec::new() });
            }
            let scenes = &mut sequences.last_mut().expect("séquence").scenes;
            if scenes.last().is_none_or(|s| s.id != panel.scene) {
                scenes.push(ResumeScene { id: panel.scene, nom: doc.scene_label(panel.scene), panneaux: Vec::new() });
            }
            let images = panel.frame_count();
            scenes.last_mut().expect("scène").panneaux.push(ResumePanneau {
                id: panel.id,
                index,
                titre: crate::storyboard::panel_title(doc, index),
                images,
                duree: duration_label(images, doc.fps),
                debut: timecode(doc.panel_start(index), doc.fps),
                action: panel.action.clone(),
                dialogue: panel.dialogue.clone(),
                notes: panel.notes.clone(),
                calques: panel.layers.len(),
            });
        }
        let images = doc.total_frames();
        ResumeProjet {
            chemin: self.chemin.as_ref().map(|p| p.display().to_string()),
            nom: self
                .chemin
                .as_ref()
                .and_then(|p| p.file_stem())
                .map_or("Sans titre".to_string(), |s| s.to_string_lossy().into_owned()),
            cadre: doc.frame,
            fps: doc.fps,
            images,
            duree: duration_label(images, doc.fps),
            modifie: self.modifie,
            sequences,
        }
    }

    fn panneau(&self, index: usize) -> Result<usize, String> {
        (index < self.doc.panels.len()).then_some(index).ok_or_else(|| format!("Pas de plan n° {}.", index + 1))
    }

    /// Vignette PNG du panneau `index` (sa première image, vue par sa caméra, sur le papier),
    /// `largeur` pixels de large.
    pub fn vignette_png(&self, index: usize, largeur: usize) -> Result<Vec<u8>, String> {
        let index = self.panneau(index)?;
        let doc = &self.doc;
        let (tw, th) = crate::thumbs::thumb_size(doc, largeur);
        let plan = render::panel_plan_at(doc, index, 0, Some(doc.paper));
        let image = render::thumbnail(doc, &plan, &doc.panel_camera(index, 0), tw, th);
        let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
        crate::io::encode_png(&rgba, tw, th)
    }

    /// Nouveau plan après le plan `index` (13a : un panneau = un plan) ; rend son rang.
    pub fn ajouter_panneau(&mut self, index: usize) -> Result<usize, String> {
        let index = self.panneau(index)?;
        self.modifie = true;
        Ok(self.doc.add_scene_after(index))
    }

    /// Copie du plan `index` juste après lui ; rend son rang.
    pub fn dupliquer_panneau(&mut self, index: usize) -> Result<usize, String> {
        let index = self.panneau(index)?;
        self.modifie = true;
        Ok(self.doc.duplicate_scene_after(index))
    }

    /// Supprime le panneau `index` (jamais le dernier du projet) ; rend le rang du panneau voisin.
    pub fn supprimer_panneau(&mut self, index: usize) -> Result<usize, String> {
        let index = self.panneau(index)?;
        let next = self.doc.remove_panel(index).ok_or("Il faut garder au moins un plan.")?;
        self.modifie = true;
        Ok(next)
    }

    /// Déplace le panneau `index` à la place `place` (0 = au début, nombre de panneaux = à la
    /// fin) ; rend son nouveau rang.
    pub fn deplacer_panneau(&mut self, index: usize, place: usize) -> Result<usize, String> {
        let index = self.panneau(index)?;
        let before = self.doc.panels.get(place).map(|p| p.id);
        let id = self.doc.panels[index].id;
        let at = self.doc.move_plans(&[id], before).ok_or("Déplacement impossible.")?;
        self.modifie = true;
        Ok(at)
    }

    /// Textes du panneau `index`.
    pub fn modifier_textes(&mut self, index: usize, action: String, dialogue: String, notes: String) -> Result<(), String> {
        let index = self.panneau(index)?;
        let p = &mut self.doc.panels[index];
        (p.action, p.dialogue, p.notes) = (action, dialogue, notes);
        self.modifie = true;
        Ok(())
    }

    /// Durée voulue du panneau `index`, en images (il reste au moins aussi long que son animation).
    pub fn regler_duree(&mut self, index: usize, images: usize) -> Result<usize, String> {
        let index = self.panneau(index)?;
        self.doc.panels[index].set_length(images.clamp(1, 100_000));
        self.modifie = true;
        Ok(self.doc.panels[index].frame_count())
    }

    /// Storyboard PDF de tout le projet dans `sortie` (une planche par image de storyboard, comme
    /// le menu Fichier de Pelure).
    pub fn exporter_pdf(&self, sortie: &Path, titre: &str) -> Result<(), String> {
        let doc = &self.doc;
        let all: Vec<usize> = (0..doc.panels.len()).collect();
        let (frames, boards) = export::storyboard_of(doc, &all);
        let mut images = Vec::with_capacity(frames.len());
        for (i, f) in frames {
            let plan = render::panel_plan_at(doc, i, f, Some(doc.paper));
            let img = render::camera_image(doc, &plan, &doc.panel_camera(i, f));
            images.push(export::board_image(&img, doc.frame)?);
        }
        let aspect = doc.frame[0] as f32 / doc.frame[1].max(1) as f32;
        let bytes = pdf::storyboard(titre, &boards, &images, aspect);
        std::fs::write(sortie, bytes).map_err(|e| format!("{} : {e}", sortie.display()))
    }
}
