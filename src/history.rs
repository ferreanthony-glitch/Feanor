use crate::document::{CelId, Document, Structure};

// Une seule règle : toute modification du document enregistre son étape AVANT d'avoir lieu.
// Les champs de saisie et curseurs passent par `App::edited` (une étape par geste).
use crate::raster::Tile;
use crate::vector::VectorDrawing;

const LIMIT: usize = 200;

/// Plan et calque courants, rétablis avec la structure.
#[derive(Clone, Copy)]
pub struct Nav {
    pub panel: usize,
    pub layer: usize,
}

/// Un état antérieur du document. L'appliquer restaure cet état et renvoie l'état
/// qu'il remplace : la même opération sert donc à annuler et à rétablir.
pub enum Edit {
    /// Tuiles d'un dessin bitmap avant modification.
    Tiles {
        cel: CelId,
        tiles: Vec<(usize, Option<Tile>)>,
    },
    /// Dessin vectoriel complet avant modification.
    Vector { cel: CelId, drawing: VectorDrawing },
    /// Tout le document hors dessins : séquences, scènes, panneaux et leurs textes, calques
    /// (images, expositions, réglages), pistes son, guides, papier, cadence.
    Structure { structure: Structure, nav: Nav },
    Group(Vec<Edit>),
}

impl Edit {
    pub fn apply(self, doc: &mut Document, nav: &mut Nav) -> Edit {
        match self {
            Edit::Tiles { cel, tiles } => {
                let Some(c) = doc.cels.get_mut(&cel) else {
                    return Edit::Group(Vec::new());
                };
                let tiles = tiles
                    .into_iter()
                    .map(|(i, t)| (i, std::mem::replace(&mut c.raster.tiles[i], t)))
                    .collect();
                c.version += 1;
                Edit::Tiles { cel, tiles }
            }
            Edit::Vector { cel, drawing } => {
                let Some(c) = doc.cels.get_mut(&cel) else {
                    return Edit::Group(Vec::new());
                };
                let old = std::mem::replace(c.vector.get_or_insert_with(VectorDrawing::default), drawing);
                c.rerender();
                Edit::Vector { cel, drawing: old }
            }
            Edit::Structure { structure, nav: saved } => {
                let old = doc.replace_structure(structure);
                let old_nav = std::mem::replace(nav, saved);
                Edit::Structure {
                    structure: old,
                    nav: old_nav,
                }
            }
            // Défait dans l'ordre inverse ; l'inverse obtenu est lui aussi rangé à rebours.
            Edit::Group(edits) => Edit::Group(
                edits
                    .into_iter()
                    .rev()
                    .map(|e| e.apply(doc, nav))
                    .collect(),
            ),
        }
    }
}

/// Regroupe les modifications d'un même geste (taper dans un champ, tirer un curseur, glisser un
/// point) en une seule étape d'annulation.
#[derive(Default)]
pub struct Gesture {
    /// Geste en cours et numéro de l'historique juste après son étape.
    current: Option<(u64, u64)>,
    /// Le geste a été prolongé pendant cette image d'écran.
    alive: bool,
}

impl Gesture {
    /// Faut-il enregistrer une étape avant cette modification ? Oui pour un nouveau geste, ou si
    /// une autre étape (ou une annulation) s'est glissée depuis celle du geste.
    pub fn needs_step(&self, key: u64, serial: u64) -> bool {
        self.current != Some((key, serial))
    }

    /// Le geste `key` a enregistré son étape ; l'historique en est maintenant au numéro `serial`.
    pub fn started(&mut self, key: u64, serial: u64) {
        self.current = Some((key, serial));
        self.alive = true;
    }

    /// Le geste continue pendant cette image (champ encore actif, curseur encore tenu…).
    pub fn keep(&mut self, key: u64) {
        if self.current.is_some_and(|(k, _)| k == key) {
            self.alive = true;
        }
    }

    /// Fin d'image : un geste que rien n'a prolongé est terminé (sauf si `hold`).
    pub fn end_frame(&mut self, hold: bool) {
        if !std::mem::take(&mut self.alive) && !hold {
            self.current = None;
        }
    }

    pub fn reset(&mut self) {
        self.current = None;
    }
}

#[derive(Default)]
pub struct History {
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// Augmente à chaque changement de l'historique : un geste en cours (saisie dans un champ…)
    /// sait ainsi si une autre étape s'est glissée depuis la sienne.
    serial: u64,
}

impl History {
    pub fn push(&mut self, edit: Edit) {
        self.undo.push(edit);
        self.redo.clear();
        self.serial += 1;
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
    }

    pub fn undo(&mut self, doc: &mut Document, nav: &mut Nav) -> bool {
        let Some(edit) = self.undo.pop() else {
            return false;
        };
        self.redo.push(edit.apply(doc, nav));
        self.serial += 1;
        true
    }

    pub fn redo(&mut self, doc: &mut Document, nav: &mut Nav) -> bool {
        let Some(edit) = self.redo.pop() else {
            return false;
        };
        self.undo.push(edit.apply(doc, nav));
        self.serial += 1;
        true
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.serial += 1;
    }

    pub fn serial(&self) -> u64 {
        self.serial
    }
}
