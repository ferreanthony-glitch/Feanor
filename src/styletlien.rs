//! Le stylet branché (Anthony, 2026-11-07, d'après son relevé du banc, partie 114) : ce que l'écoute de `stylet.rs` lit
//! auprès de Windows sert enfin au dessin.
//!
//! - **Le bouton du stylet est le clic droit.** winit ne transmet du stylet que la pointe : un appui, un bouton principal.
//!   Le relevé d'Anthony l'a montré : « bouton, pointe posée : vu » chez Windows, « clic droit : 0 » chez FEANOR. Ici, un
//!   appui de la pointe pendant que le bouton est enfoncé devient un appui du **bouton secondaire**, et le relâchement qui
//!   le suit aussi : la fenêtre des réglages de l'outil (clic bref, partie 66) et l'anneau (bouton tenu, partie 111)
//!   marchent au stylet. Le bouton compte à l'instant du contact (`Releve::bouton_au_contact`) : pressé en cours de
//!   trait, il ne change rien. Son pilote ne signale le bouton qu'à la pointe posée (mode « Click & Tap ») : le geste est
//!   donc bouton tenu, poser, partir dans une direction.
//! - **Le stylet retourné prend la Gomme.** Tant que Windows voit le stylet à l'envers (en l'air ou posé), l'outil est la
//!   Gomme ; remis à l'endroit, l'outil d'avant revient, s'il n'a pas été changé entre-temps. Un stylet déjà sur la Gomme
//!   n'y change rien. Parti de la tablette à l'envers, l'écoute l'oublie (`stylet::noter_le_depart`) : l'outil revient.
//!
//! Tout passe par `App::raw_input_hook`, avant qu'egui ne lise l'image : rien n'est ajouté pendant un geste, seulement
//! récrit. Préférences > Dessin > « Bouton et gomme du stylet lus auprès de Windows » éteint les deux
//! (`Settings::stylet_windows`) ; le banc, lui, lit toujours.

use eframe::egui::{Event, PointerButton, RawInput, TouchPhase};

use crate::app::{App, Tool};
use crate::stylet::{self, Releve};

/// Ce qui se poursuit d'une image à l'autre.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lien {
    /// Un appui de la pointe a été récrit en bouton secondaire : son relâchement le sera aussi.
    pub bouton_tenu: bool,
    /// L'outil d'avant la Gomme, tant que le stylet est retourné.
    pub outil_avant: Option<Tool>,
}

/// Récrit les événements d'une image : l'appui de la pointe (un événement tactile puis le bouton principal, comme egui-winit
/// les donne) pendant que le bouton du stylet est enfoncé devient un appui du bouton secondaire, sans l'événement tactile ;
/// le relâchement qui suit aussi. Un appui de la souris (sans événement tactile) n'est jamais touché.
pub fn recrire(events: &mut Vec<Event>, lien: &mut Lien, bouton_au_contact: bool) {
    // L'événement tactile de début, en attente du bouton qui le suit.
    let mut debut: Option<usize> = None;
    let mut a_oter = Vec::new();
    for k in 0..events.len() {
        match &mut events[k] {
            Event::Touch { phase: TouchPhase::Start, .. } => debut = Some(k),
            Event::PointerButton { button, pressed: true, .. } if *button == PointerButton::Primary => {
                if let Some(d) = debut.take() {
                    if bouton_au_contact {
                        *button = PointerButton::Secondary;
                        lien.bouton_tenu = true;
                        a_oter.push(d);
                    }
                }
            }
            Event::PointerButton { button, pressed: false, .. } if *button == PointerButton::Primary && lien.bouton_tenu => {
                *button = PointerButton::Secondary;
                lien.bouton_tenu = false;
            }
            _ => {}
        }
    }
    for d in a_oter.into_iter().rev() {
        events.remove(d);
    }
}

impl App {
    /// Ce que l'écoute a lu ; dans les essais, ce qu'on lui impose (l'écoute est commune à tout le processus).
    pub(crate) fn stylet_releve(&self) -> Releve {
        self.stylet.force.unwrap_or_else(stylet::lire)
    }

    /// Chaque image, avant egui (`raw_input_hook`) : le bouton du stylet devient le clic droit, le stylet retourné prend
    /// la Gomme.
    pub(crate) fn stylet_brancher(&mut self, raw_input: &mut RawInput) {
        if !self.settings.stylet_windows {
            return;
        }
        let r = self.stylet_releve();
        recrire(&mut raw_input.events, &mut self.stylet.lien, r.bouton_au_contact);
        match (r.lecture.dos(), self.stylet.lien.outil_avant) {
            (true, None) => {
                if self.tool != Tool::Eraser {
                    self.stylet.lien.outil_avant = Some(self.tool);
                    self.set_tool(Tool::Eraser);
                }
            }
            (false, Some(avant)) => {
                self.stylet.lien.outil_avant = None;
                if self.tool == Tool::Eraser {
                    self.set_tool(avant);
                }
            }
            _ => {}
        }
    }
}
