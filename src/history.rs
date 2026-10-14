use crate::document::{CelId, Document, Structure};
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
    /// Séquences, scènes, panneaux, calques (images, expositions, durées, ordre…) et pistes son.
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
            Edit::Structure { mut structure, nav: saved } => {
                keep_current_metadata(&mut structure, doc);
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

/// Les noms, les textes des panneaux et les réglages des calques ne passent pas (encore) par
/// l'historique : annuler une opération de structure ne doit pas les faire revenir en arrière.
fn keep_current_metadata(restored: &mut Structure, doc: &Document) {
    for seq in &mut restored.sequences {
        if let Some(now) = doc.sequences.iter().find(|s| s.id == seq.id) {
            seq.name.clone_from(&now.name);
        }
    }
    for scene in &mut restored.scenes {
        if let Some(now) = doc.scenes.iter().find(|s| s.id == scene.id) {
            scene.name.clone_from(&now.name);
        }
    }
    for panel in &mut restored.panels {
        let Some(now) = doc.panels.iter().find(|s| s.id == panel.id) else { continue };
        panel.action.clone_from(&now.action);
        panel.dialogue.clone_from(&now.dialogue);
        panel.notes.clone_from(&now.notes);
        for layer in &mut panel.layers {
            if let Some(l) = now.layers.iter().find(|l| l.id == layer.id) {
                layer.name.clone_from(&l.name);
                layer.opacity = l.opacity;
                layer.visible = l.visible;
            }
        }
    }
    for track in &mut restored.audio_tracks {
        if let Some(now) = doc.audio_tracks.iter().find(|t| t.id == track.id) {
            track.name.clone_from(&now.name);
            track.muted = now.muted;
            track.volume = now.volume;
        }
    }
}

#[derive(Default)]
pub struct History {
    undo: Vec<Edit>,
    redo: Vec<Edit>,
}

impl History {
    pub fn push(&mut self, edit: Edit) {
        self.undo.push(edit);
        self.redo.clear();
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
    }

    pub fn undo(&mut self, doc: &mut Document, nav: &mut Nav) -> bool {
        let Some(edit) = self.undo.pop() else {
            return false;
        };
        self.redo.push(edit.apply(doc, nav));
        true
    }

    pub fn redo(&mut self, doc: &mut Document, nav: &mut Nav) -> bool {
        let Some(edit) = self.redo.pop() else {
            return false;
        };
        self.undo.push(edit.apply(doc, nav));
        true
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
