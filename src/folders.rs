//! Dossiers de calques (13b-5). Un dossier est un calque sans dessin (`Layer::folder`) ; ses
//! calques (`Layer::parent` = son id) sont rangés juste sous lui dans la pile, d'un seul tenant.
//! Un seul niveau : un dossier n'est jamais dans un dossier (décision d'Anthony, 2026-10-15).
//!
//! Le dossier n'a pas de mode de fusion : chaque calque garde le sien ; un dossier caché cache
//! ses calques, et son opacité s'ajoute à la leur (elle les multiplie). Un dossier verrouillé
//! verrouille ses calques. Réduit (`collapsed`), il cache leurs lignes dans la timeline.

use std::collections::HashSet;

use crate::app::App;
use crate::document::Layer;

/// Rang du dossier du calque `li`, s'il est dans un dossier.
pub fn parent_index(layers: &[Layer], li: usize) -> Option<usize> {
    let parent = layers.get(li)?.parent?;
    layers.iter().position(|l| l.folder && l.id == parent)
}

/// Rangs des calques du dossier `fi`, du plus bas au plus haut.
pub fn children_of(layers: &[Layer], fi: usize) -> Vec<usize> {
    let Some(f) = layers.get(fi).filter(|l| l.folder) else { return Vec::new() };
    (0..layers.len()).filter(|&i| layers[i].parent == Some(f.id)).collect()
}

/// Le dossier `fi` et ses calques (rangs, du plus bas au plus haut) ; un calque seul s'il n'est
/// pas un dossier.
pub fn block_of(layers: &[Layer], fi: usize) -> Vec<usize> {
    let mut out = children_of(layers, fi);
    out.push(fi);
    out
}

/// Comment le calque `li` entre dans l'image : rien (lui ou son dossier est caché, ou c'est un
/// dossier), ou son opacité, multipliée par celle de son dossier.
pub fn effect(layers: &[Layer], li: usize) -> Option<f32> {
    let l = layers.get(li)?;
    if l.folder || !l.visible {
        return None;
    }
    match parent_index(layers, li).map(|fi| &layers[fi]) {
        Some(f) if !f.visible => None,
        Some(f) => Some(l.opacity.clamp(0.0, 1.0) * f.opacity.clamp(0.0, 1.0)),
        None => Some(l.opacity.clamp(0.0, 1.0)),
    }
}

/// Comme `effect`, à l'image `frame` du plan : l'opacité du calque et celle de son dossier suivent leurs clés d'opacité
/// (`opacitecles.rs`). Tout ce qui compose une image (écran, vignettes, lecture, exports, PDF) passe par ici.
pub fn effect_at(layers: &[Layer], li: usize, frame: usize) -> Option<f32> {
    let l = layers.get(li)?;
    if l.folder || !l.visible {
        return None;
    }
    match parent_index(layers, li).map(|fi| &layers[fi]) {
        Some(f) if !f.visible => None,
        Some(f) => Some(l.opacity_at(frame) * f.opacity_at(frame)),
        None => Some(l.opacity_at(frame)),
    }
}

/// Le calque `li` est verrouillé par son dossier.
pub fn locked_by_folder(layers: &[Layer], li: usize) -> bool {
    parent_index(layers, li).is_some_and(|fi| layers[fi].locked)
}

/// Remet la pile en ordre : un `parent` qui ne désigne pas un dossier du plan est oublié, un
/// dossier n'a pas de dossier, et les calques de chaque dossier sont rassemblés juste sous lui
/// (dans leur ordre). Les autres calques gardent leur ordre.
pub fn normalize(layers: &mut Vec<Layer>) {
    let folders: HashSet<u64> = layers.iter().filter(|l| l.folder).map(|l| l.id).collect();
    for l in layers.iter_mut() {
        if l.folder || l.parent.is_some_and(|p| !folders.contains(&p)) {
            l.parent = None;
        }
    }
    // Du haut vers le bas : chaque calque hors dossier, et sous chaque dossier ses calques.
    let top_down: Vec<Layer> = layers.drain(..).rev().collect();
    let mut out: Vec<Layer> = Vec::with_capacity(top_down.len());
    for l in top_down.iter().filter(|l| l.parent.is_none()) {
        out.push(l.clone());
        if l.folder {
            out.extend(top_down.iter().filter(|c| c.parent == Some(l.id)).cloned());
        }
    }
    out.reverse();
    *layers = out;
}

/// Monte ou descend le calque `li` d'un cran ; faux s'il est déjà au bout. Un calque rangé
/// bouge dans son dossier, et en sort quand il en passe le bord (au-dessus du dossier, ou juste
/// sous ses calques). Un dossier bouge avec ses calques, et un calque hors dossier passe
/// par-dessus un dossier voisin et ses calques d'un seul cran. La pile doit être en ordre
/// (`normalize`).
pub fn move_step(layers: &mut Vec<Layer>, li: usize, up: bool) -> bool {
    if li >= layers.len() {
        return false;
    }
    if let Some(fi) = parent_index(layers, li) {
        let lowest = children_of(layers, fi)[0];
        if up && li + 1 == fi {
            let mut l = layers.remove(li);
            l.parent = None;
            layers.insert(fi, l);
        } else if up {
            layers.swap(li, li + 1);
        } else if li == lowest {
            layers[li].parent = None;
        } else {
            layers.swap(li, li - 1);
        }
        normalize(layers);
        return true;
    }
    // Les unités de la pile, du bas vers le haut : un dossier avec ses calques, ou un calque seul.
    let mut units: Vec<Vec<Layer>> = Vec::new();
    let mut i = 0;
    while i < layers.len() {
        let end = layers[i]
            .parent
            .and_then(|p| layers.iter().position(|f| f.folder && f.id == p))
            .filter(|&fi| fi > i)
            .unwrap_or(i);
        units.push(layers[i..=end].to_vec());
        i = end + 1;
    }
    let id = layers[li].id;
    let Some(u) = units.iter().position(|u| u.iter().any(|l| l.id == id)) else { return false };
    let v = if up { u + 1 } else { u.wrapping_sub(1) };
    if v >= units.len() {
        return false;
    }
    units.swap(u, v);
    *layers = units.concat();
    true
}

/// Dossier dans lequel tombe un calque posé au rang `at` de la pile (une fois retiré de son
/// ancienne place) : celui des deux voisins, s'ils sont dans le même dossier, ou le dossier
/// lui-même si le calque se pose juste sous lui.
pub fn folder_at(layers: &[Layer], at: usize) -> Option<u64> {
    let above = layers.get(at);
    let below = at.checked_sub(1).and_then(|i| layers.get(i));
    match (below, above) {
        (_, Some(a)) if a.folder => Some(a.id),
        (Some(b), Some(a)) if b.parent.is_some() && b.parent == a.parent => b.parent,
        _ => None,
    }
}

impl App {
    /// Pose une nouvelle pile de calques (une étape d'annulation) ; `current` redevient le
    /// calque courant.
    fn set_layers(&mut self, mut layers: Vec<Layer>, current: u64, status: String) {
        normalize(&mut layers);
        let snap = self.snapshot();
        *self.layers_mut() = layers;
        self.cur_layer = self.layers().iter().position(|l| l.id == current).unwrap_or(0);
        self.layer_sel.clear();
        self.vsel = None;
        self.commit_structure(snap);
        self.status = status;
    }

    /// Nouveau nom de dossier pour le plan.
    fn folder_name(&self) -> String {
        format!("Dossier {}", self.layers().iter().filter(|l| l.folder).count() + 1)
    }

    /// Nouveau dossier vide, au-dessus du calque courant (au-dessus de son dossier s'il est rangé).
    pub(crate) fn new_folder(&mut self) {
        self.end_stroke();
        let mut layers = self.layers().to_vec();
        let at = parent_index(&layers, self.cur_layer).unwrap_or(self.cur_layer) + 1;
        let folder = Layer::new_folder(self.folder_name());
        let (id, name) = (folder.id, folder.name.clone());
        layers.insert(at, folder);
        self.set_layers(layers, id, format!("Dossier « {name} » créé."));
    }

    /// Calques choisis qui ne sont pas des dossiers (rangs).
    fn chosen_plain_layers(&self) -> Vec<usize> {
        self.chosen_layers().into_iter().filter(|&li| !self.layers()[li].folder).collect()
    }

    /// Place les calques choisis dans un nouveau dossier, posé juste au-dessus du plus haut d'entre eux.
    pub(crate) fn folder_from_chosen(&mut self) {
        let chosen = self.chosen_plain_layers();
        let Some(&top) = chosen.last() else {
            self.status = "Choisis les calques à ranger (Ctrl + clic dans la timeline).".into();
            return;
        };
        self.end_stroke();
        let current = self.layer().id;
        let mut layers = self.layers().to_vec();
        let folder = Layer::new_folder(self.folder_name());
        let (id, name) = (folder.id, folder.name.clone());
        for &li in &chosen {
            layers[li].parent = Some(id);
        }
        layers.insert(top + 1, folder);
        let n = chosen.len();
        self.set_layers(layers, current, format!("{n} calque{} rangé{} dans « {name} ».", plural(n), plural(n)));
    }

    /// Dossier visé par « déposer » et « joindre » : le dossier choisi le plus haut, sinon le
    /// calque courant s'il est un dossier, sinon son dossier.
    fn target_folder(&self) -> Option<usize> {
        let chosen = self.chosen_layers();
        if self.layer().folder {
            return Some(self.cur_layer);
        }
        chosen.iter().rev().copied().find(|&li| self.layers()[li].folder).or_else(|| parent_index(self.layers(), self.cur_layer))
    }

    /// Dépose les calques choisis dans le dossier visé.
    pub(crate) fn drop_chosen_in_folder(&mut self) {
        let Some(fi) = self.target_folder() else {
            self.status = "Choisis aussi un dossier (Ctrl + clic) où déposer les calques.".into();
            return;
        };
        let chosen = self.chosen_plain_layers();
        if chosen.is_empty() {
            self.status = "Choisis les calques à déposer (Ctrl + clic dans la timeline).".into();
            return;
        }
        self.end_stroke();
        let current = self.layer().id;
        let mut layers = self.layers().to_vec();
        let (id, name) = (layers[fi].id, layers[fi].name.clone());
        for &li in &chosen {
            layers[li].parent = Some(id);
        }
        let n = chosen.len();
        self.set_layers(layers, current, format!("{n} calque{} déposé{} dans « {name} ».", plural(n), plural(n)));
    }

    /// Sort les calques choisis de leur dossier (ils se posent juste sous lui et ses calques).
    pub(crate) fn take_chosen_out(&mut self) {
        let chosen: Vec<usize> = self.chosen_plain_layers().into_iter().filter(|&li| self.layers()[li].parent.is_some()).collect();
        if chosen.is_empty() {
            self.status = "Aucun calque choisi n'est dans un dossier.".into();
            return;
        }
        self.end_stroke();
        let current = self.layer().id;
        let mut layers = self.layers().to_vec();
        for &li in &chosen {
            layers[li].parent = None;
        }
        let n = chosen.len();
        self.set_layers(layers, current, format!("{n} calque{} sorti{} de leur dossier.", plural(n), plural(n)));
    }

    /// Supprime seulement les dossiers choisis : leurs calques restent, hors dossier.
    pub(crate) fn delete_folders_only(&mut self) {
        let folders: Vec<usize> = self.chosen_layers().into_iter().filter(|&li| self.layers()[li].folder).collect();
        if folders.is_empty() {
            self.status = "Aucun dossier choisi.".into();
            return;
        }
        self.end_stroke();
        let mut layers = self.layers().to_vec();
        let ids: Vec<u64> = folders.iter().map(|&fi| layers[fi].id).collect();
        // Le courant devient le plus haut calque du dossier, ou celui d'en dessous.
        let current = children_of(&layers, folders[folders.len() - 1])
            .last()
            .map(|&i| layers[i].id)
            .unwrap_or_else(|| layers[folders[0].saturating_sub(1)].id);
        for l in layers.iter_mut() {
            if l.parent.is_some_and(|p| ids.contains(&p)) {
                l.parent = None;
            }
        }
        layers.retain(|l| !ids.contains(&l.id));
        if layers.is_empty() {
            layers.push(Layer::new("Dessin", crate::document::LayerKind::Vector));
        }
        let current = if layers.iter().any(|l| l.id == current) { current } else { layers[0].id };
        let n = ids.len();
        self.set_layers(layers, current, format!("{n} dossier{} supprimé{} ; leurs calques restent.", plural(n), plural(n)));
    }

    /// Joint les dossiers choisis dans le dossier visé (le courant) : leurs calques y passent et
    /// les autres dossiers sont effacés. On garde celui qu'on veut en le prenant comme courant.
    pub(crate) fn join_chosen_folders(&mut self) {
        let Some(fi) = self.target_folder() else {
            self.status = "Le calque courant doit être un dossier (celui qui reste).".into();
            return;
        };
        let sources: Vec<usize> = self.chosen_layers().into_iter().filter(|&li| li != fi && self.layers()[li].folder).collect();
        if sources.is_empty() {
            self.status = "Choisis d'autres dossiers à joindre à celui-ci (Ctrl + clic).".into();
            return;
        }
        self.end_stroke();
        let mut layers = self.layers().to_vec();
        let (id, name) = (layers[fi].id, layers[fi].name.clone());
        let gone: Vec<u64> = sources.iter().map(|&s| layers[s].id).collect();
        for l in layers.iter_mut() {
            if l.parent.is_some_and(|p| gone.contains(&p)) {
                l.parent = Some(id);
            }
        }
        layers.retain(|l| !gone.contains(&l.id));
        self.set_layers(layers, id, format!("Dossiers joints dans « {name} »."));
    }
}

fn plural(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}