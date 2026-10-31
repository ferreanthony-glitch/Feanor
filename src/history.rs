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
    /// Tuiles de la strate Fond d'un dessin avant modification (`fond.rs`) : sa peinture, ou sa
    /// coupe (`coupe`).
    FondTiles {
        cel: CelId,
        tiles: Vec<(usize, Option<Tile>)>,
        coupe: bool,
    },
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
                    .map(|(i, t)| (i, c.raster.replace_slot(i, t)))
                    .collect();
                c.version += 1;
                Edit::Tiles { cel, tiles }
            }
            Edit::FondTiles { cel, tiles, coupe } => {
                let Some(c) = doc.cels.get_mut(&cel) else {
                    return Edit::Group(Vec::new());
                };
                let (w, h) = (c.raster.width, c.raster.height);
                let s = c.fond.get_or_insert_with(|| Box::new(crate::document::Strate::new(w, h)));
                let target = if coupe { &mut s.coupe } else { &mut s.peinture.raster };
                let tiles = tiles.into_iter().map(|(i, t)| (i, target.replace_slot(i, t))).collect();
                s.peinture.version += 1;
                c.version += 1;
                Edit::FondTiles { cel, tiles, coupe }
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

    /// L'historique a avancé de `avant` à `apres` par une prolongation de l'étape du geste (`History::extend_last`) : le geste en
    /// cours, s'il en était à `avant`, reste le même geste (sinon son champ, encore ouvert, enregistrerait une étape de plus).
    pub fn suivre(&mut self, avant: u64, apres: u64) {
        if let Some((key, serial)) = self.current {
            if serial == avant {
                self.current = Some((key, apres));
            }
        }
    }
}

/// Les étapes retirées d'un historique (`History::take_steps`), à lâcher ailleurs.
pub struct Steps(#[allow(dead_code)] Vec<Edit>, #[allow(dead_code)] Vec<Edit>);

#[derive(Default)]
pub struct History {
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// Augmente à chaque changement de l'historique : un geste en cours (saisie dans un champ…)
    /// sait ainsi si une autre étape s'est glissée depuis la sienne.
    serial: u64,
    /// Le nom de chaque étape, à côté de `undo` et `redo` (panneau Historique, `historyui.rs`).
    undo_names: Vec<String>,
    redo_names: Vec<String>,
    /// Le nom des étapes enregistrées maintenant : la commande lancée, sinon le panneau ou l'outil
    /// en cours (posé par l'application) ; à défaut, la sorte de modification.
    pub context: Option<String>,
}

/// Le nom d'une étape sans contexte : ce qu'elle a changé.
fn kind_name(edit: &Edit) -> &'static str {
    match edit {
        Edit::Tiles { .. } => "Dessin",
        Edit::Vector { .. } => "Dessin vectoriel",
        Edit::FondTiles { .. } => "Aplat",
        Edit::Structure { .. } => "Modification",
        Edit::Group(edits) => edits.iter().map(kind_name).find(|n| *n != "Modification").unwrap_or("Modification"),
    }
}

impl History {
    pub fn push(&mut self, edit: Edit) {
        let name = self.context.clone().unwrap_or_else(|| kind_name(&edit).to_string());
        // (Le rapport d'une erreur dit ce qu'on venait de faire, `plantage.rs`.)
        crate::plantage::noter_action(&name);
        self.undo.push(edit);
        self.undo_names.push(name);
        self.redo.clear();
        self.redo_names.clear();
        self.serial += 1;
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
            self.undo_names.remove(0);
        }
    }

    /// Ajoute `edit` à la dernière étape, qui devient un groupe (aux flèches tenues : chaque
    /// appui garde son propre retour en arrière, et une seule étape les annule tous). Sans étape
    /// à prolonger, c'est un `push`.
    pub fn extend_last(&mut self, edit: Edit) {
        let Some(last) = self.undo.pop() else {
            self.push(edit);
            return;
        };
        let mut edits = match last {
            Edit::Group(edits) => edits,
            single => vec![single],
        };
        edits.push(edit);
        self.undo.push(Edit::Group(edits));
        self.redo.clear();
        self.redo_names.clear();
        self.serial += 1;
    }

    /// Le nombre d'étapes à annuler.
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Regroupe les `n` dernières étapes à annuler (deux au moins) en une seule, qui garde le nom de la plus
    /// ancienne : un plan fait en dessinant dans une case vide s'annule d'un coup.
    pub fn merge_last(&mut self, n: usize) {
        let len = self.undo.len();
        if n < 2 || n > len {
            return;
        }
        let name = self.undo_names[len - n].clone();
        let tail = self.undo.split_off(len - n);
        self.undo_names.truncate(len - n);
        self.undo.push(Edit::Group(tail));
        self.undo_names.push(name);
        self.serial += 1;
    }

    /// Défait la dernière étape et l'oublie : elle ne se rétablit pas (un geste qui n'a rien fait).
    pub fn undo_and_forget(&mut self, doc: &mut Document, nav: &mut Nav) -> bool {
        if !self.undo(doc, nav) {
            return false;
        }
        self.redo.pop();
        self.redo_names.pop();
        true
    }

    pub fn undo(&mut self, doc: &mut Document, nav: &mut Nav) -> bool {
        let Some(edit) = self.undo.pop() else {
            return false;
        };
        self.redo.push(edit.apply(doc, nav));
        self.redo_names.push(self.undo_names.pop().unwrap_or_default());
        self.serial += 1;
        true
    }

    pub fn redo(&mut self, doc: &mut Document, nav: &mut Nav) -> bool {
        let Some(edit) = self.redo.pop() else {
            return false;
        };
        self.undo.push(edit.apply(doc, nav));
        self.undo_names.push(self.redo_names.pop().unwrap_or_default());
        self.serial += 1;
        true
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.undo_names.clear();
        self.redo_names.clear();
        self.serial += 1;
    }

    /// Comme `clear`, mais rend les étapes au lieu de les lâcher ici : celles d'un gros projet tiennent des dizaines de
    /// milliers de tuiles, que l'appelant fait rendre sur un autre fil (`lacher.rs`).
    pub fn take_steps(&mut self) -> Steps {
        let steps = Steps(std::mem::take(&mut self.undo), std::mem::take(&mut self.redo));
        self.clear();
        steps
    }

    /// Les étapes à annuler (la plus ancienne d'abord, la dernière faite à la fin) et celles à
    /// rétablir (la prochaine d'abord).
    pub fn names(&self) -> (Vec<&str>, Vec<&str>) {
        (self.undo_names.iter().map(String::as_str).collect(), self.redo_names.iter().rev().map(String::as_str).collect())
    }

    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// Aucune étape à annuler ni à rétablir (un projet qu'on n'a pas encore touché, `onglets.rs`).
    pub fn is_empty(&self) -> bool {
        self.undo.is_empty() && self.redo.is_empty()
    }

    /// Tuiles que tiennent les étapes : (dont les pixels sont en mémoire, en tout, octets tenus).
    pub fn tuiles(&self) -> (usize, usize, usize) {
        let mut r = (0, 0, 0);
        for e in self.undo.iter().chain(self.redo.iter()) {
            e.compter(&mut r);
        }
        r
    }

    /// Range les tuiles des étapes les plus anciennes, au plus `max` tuiles (une étape de 3 000 tuiles ne se range pas d'un coup) :
    /// les `garder` dernières étapes à annuler et la prochaine à rétablir restent en pixels (Ctrl+Z reste instantané) ; les autres se
    /// rangent en tampons exacts et se rouvrent quand on les applique. Rend le nombre de tuiles rangées.
    pub fn geler_les_vieilles_etapes(&mut self, garder: usize, max: usize) -> usize {
        let mut fait = 0;
        let n_undo = self.undo.len().saturating_sub(garder);
        for e in self.undo[..n_undo].iter_mut() {
            if fait >= max {
                return fait;
            }
            fait += e.geler(max - fait);
        }
        // (Les étapes à rétablir : la prochaine, la dernière du vecteur, reste ouverte.)
        let n_redo = self.redo.len().saturating_sub(1);
        for e in self.redo[..n_redo].iter_mut() {
            if fait >= max {
                return fait;
            }
            fait += e.geler(max - fait);
        }
        fait
    }
}

impl Edit {
    fn compter(&self, r: &mut (usize, usize, usize)) {
        match self {
            Edit::Tiles { tiles, .. } | Edit::FondTiles { tiles, .. } => {
                for t in tiles.iter().filter_map(|(_, t)| t.as_ref()) {
                    r.0 += usize::from(!t.est_rangee());
                    r.1 += 1;
                    r.2 += t.octets_en_memoire();
                }
            }
            Edit::Group(edits) => edits.iter().for_each(|e| e.compter(r)),
            Edit::Vector { .. } | Edit::Structure { .. } => {}
        }
    }

    /// Range au plus `max` tuiles de cette étape (celles qui ne sont pas déjà rangées et que l'étape est seule à tenir) ; rend le nombre rangé.
    fn geler(&mut self, max: usize) -> usize {
        let mut fait = 0;
        match self {
            Edit::Tiles { tiles, .. } | Edit::FondTiles { tiles, .. } => {
                for t in tiles.iter_mut().filter_map(|(_, t)| t.as_mut()) {
                    if fait >= max {
                        break;
                    }
                    // (`geler` : fait le tampon s'il manque, puis lâche les pixels si l'étape est seule à tenir la tuile.)
                    if t.geler() {
                        fait += 1;
                    }
                }
            }
            Edit::Group(edits) => {
                for e in edits.iter_mut() {
                    if fait >= max {
                        break;
                    }
                    fait += e.geler(max - fait);
                }
            }
            Edit::Vector { .. } | Edit::Structure { .. } => {}
        }
        fait
    }
}
