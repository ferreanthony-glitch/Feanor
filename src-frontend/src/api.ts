// Appels aux commandes Rust de l'appli (src-tauri/src/main.rs, `#[tauri::command]`). Les noms
// des arguments sont ceux des fonctions Rust ; les types suivent `pelure::noyau`.
import { invoke } from "@tauri-apps/api/core";

export type ResumePanneau = {
  id: number;
  index: number;
  titre: string;
  images: number;
  duree: string;
  debut: string;
  action: string;
  dialogue: string;
  notes: string;
  calques: number;
};

export type ResumeScene = { id: number; nom: string; panneaux: ResumePanneau[] };
export type ResumeSequence = { id: number; nom: string; scenes: ResumeScene[] };

export type ResumeProjet = {
  chemin: string | null;
  nom: string;
  cadre: [number, number];
  fps: number;
  images: number;
  duree: string;
  modifie: boolean;
  sequences: ResumeSequence[];
};

export type Ouverture = { projet: ResumeProjet; avertissements: string[] };

export const api = {
  nouveau: (largeur: number, hauteur: number, fps: number) =>
    invoke<ResumeProjet>("nouveau_projet", { largeur, hauteur, fps }),
  /** Sans chemin : la fenêtre « Ouvrir » du système ; null si on annule. */
  ouvrir: (chemin?: string) => invoke<Ouverture | null>("ouvrir_projet", { chemin: chemin ?? null }),
  resume: () => invoke<ResumeProjet>("resume_projet"),
  /** Vignette PNG du panneau, en adresse d'image utilisable par <img> (à libérer après). */
  vignette: async (index: number, largeur: number) => {
    const octets = await invoke<ArrayBuffer>("vignette", { index, largeur });
    return URL.createObjectURL(new Blob([octets], { type: "image/png" }));
  },
  ajouter: (index: number) => invoke<ResumeProjet>("ajouter_panneau", { index }),
  dupliquer: (index: number) => invoke<ResumeProjet>("dupliquer_panneau", { index }),
  supprimer: (index: number) => invoke<ResumeProjet>("supprimer_panneau", { index }),
  /** `place` : 0 = au début ; le nombre de panneaux = à la fin. */
  deplacer: (index: number, place: number) => invoke<ResumeProjet>("deplacer_panneau", { index, place }),
  textes: (index: number, action: string, dialogue: string, notes: string) =>
    invoke<ResumeProjet>("modifier_textes", { index, action, dialogue, notes }),
  duree: (index: number, images: number) => invoke<ResumeProjet>("regler_duree", { index, images }),
  enregistrer: () => invoke<ResumeProjet>("enregistrer_projet"),
  /** Chemin du PDF écrit, ou null si on annule. */
  exporterPdf: () => invoke<string | null>("exporter_pdf"),
};
