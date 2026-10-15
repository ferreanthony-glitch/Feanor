//! Opérations sur les calques du plan (13b) : calques choisis (Ctrl + clic, Maj + clic dans
//! l'en-tête de la timeline ; le calque courant en fait toujours partie), dupliquer, supprimer,
//! fusionner les choisis / les visibles / tous, supprimer les calques vides ou cachés, tout
//! afficher, tout cacher, le calque courant seul, pochoirs, rang du calque, premier / dernier.
//!
//! Les fonctions libres travaillent sur le document (testées dans `layerflags_tests.rs`) ; les
//! méthodes d'`App` y ajoutent l'étape d'annulation et le calque courant.

use std::collections::HashMap;

use eframe::egui;

use crate::app::App;
use crate::document::{fresh_layer_id, CelId, Document, Layer};

/// Quels calques choisir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choose {
    All,
    Visible,
    SameGroup,
    OtherGroups,
    Nothing,
}

/// Le calque n'a aucun dessin, ou seulement des dessins vides.
pub(crate) fn is_empty_layer(doc: &Document, layer: &Layer) -> bool {
    layer.frames.iter().flatten().all(|id| doc.cels.get(id).is_none_or(|c| c.is_empty()))
}

/// Calques choisis par `how` (ids), pour un plan dont le calque courant est `cur`.
pub(crate) fn choose(layers: &[Layer], cur: usize, how: Choose) -> Vec<u64> {
    let group = layers.get(cur).map_or(0, |l| l.group);
    layers
        .iter()
        .filter(|l| match how {
            Choose::All => true,
            Choose::Visible => l.visible,
            Choose::SameGroup => l.group == group,
            Choose::OtherGroups => l.group != group,
            Choose::Nothing => false,
        })
        .map(|l| l.id)
        .collect()
}

/// Copie d'un calque avec ses propres copies des dessins (`copies` : un même dessin n'est copié
/// qu'une fois).
fn layer_copy(doc: &mut Document, source: &Layer, copies: &mut HashMap<CelId, CelId>) -> Layer {
    let frames = source
        .frames
        .iter()
        .map(|f| {
            f.map(|id| match copies.get(&id) {
                Some(&c) => c,
                None => {
                    let c = doc.duplicate_cel(id);
                    copies.insert(id, c);
                    c
                }
            })
        })
        .collect();
    // La copie d'un calque de toute la scène est un calque du seul plan.
    Layer { id: fresh_layer_id(), frames, scene_wide: None, ..source.clone() }
}

/// Copie du calque `li` du plan `index`, avec ses propres copies des dessins, posée juste
/// au-dessus de lui ; un dossier est copié avec ses calques, au-dessus de lui et de ses calques
/// (13b-5). Renvoie le rang de la copie (du dossier copié).
pub(crate) fn duplicate_layer(doc: &mut Document, index: usize, li: usize) -> usize {
    let layers = &doc.panels[index].layers;
    let source = layers[li].clone();
    let children: Vec<Layer> = crate::folders::children_of(layers, li).into_iter().map(|i| layers[i].clone()).collect();
    let mut copies: HashMap<CelId, CelId> = HashMap::new();
    let mut copy = layer_copy(doc, &source, &mut copies);
    copy.name = format!("{} copie", source.name);
    let mut block: Vec<Layer> = Vec::new();
    for c in &children {
        let mut child = layer_copy(doc, c, &mut copies);
        child.parent = Some(copy.id);
        block.push(child);
    }
    block.push(copy);
    let n = block.len();
    for (k, l) in block.into_iter().enumerate() {
        doc.panels[index].layers.insert(li + 1 + k, l);
    }
    li + n
}

/// Fusionne les calques `lis` du plan `index` en un seul, à la place du plus bas, dans l'ordre
/// de la pile (les calques non choisis qui étaient entre eux passent au-dessus) ; renvoie son
/// rang. Rien ne change si l'une des fusions est refusée (calque caché, verrouillé, transformé…).
pub(crate) fn merge_layers(doc: &mut Document, index: usize, lis: &[usize]) -> Result<usize, String> {
    let n = doc.panels.get(index).map_or(0, |p| p.layers.len());
    // Un dossier choisi compte pour ses calques ; un dossier ne se fusionne pas lui-même.
    let layers = doc.panels.get(index).map_or(&[][..], |p| &p.layers[..]);
    let mut lis: Vec<usize> = lis
        .iter()
        .copied()
        .filter(|&l| l < n)
        .flat_map(|l| if layers[l].folder { crate::folders::children_of(layers, l) } else { vec![l] })
        .collect();
    lis.sort_unstable();
    lis.dedup();
    if lis.len() < 2 {
        return Err("Il faut au moins deux calques à fusionner (Ctrl + clic dans la timeline pour en choisir).".into());
    }
    let before = doc.panels[index].layers.clone();
    let low = lis[0];
    let layers = &mut doc.panels[index].layers;
    let mut picked: Vec<Layer> = lis.iter().rev().map(|&l| layers.remove(l)).collect();
    picked.reverse();
    let count = picked.len();
    for (k, l) in picked.into_iter().enumerate() {
        layers.insert(low + k, l);
    }
    for _ in 1..count {
        if let Err(e) = doc.merge_layer_down(index, low + 1) {
            doc.panels[index].layers = before;
            return Err(e);
        }
    }
    Ok(low)
}

impl App {
    /// Menu des opérations sur les calques (menu Calques, et bouton ☰ de l'en-tête de la timeline).
    pub(crate) fn layer_ops_menu(&mut self, ui: &mut egui::Ui) {
        let n = self.chosen_layers().len();
        self.action_item(ui, "Nouveau calque…", "calque.nouveau");
        self.action_item(ui, "Nouveau calque vectoriel", "calque.nouveau_vectoriel");
        self.action_item(ui, "Nouveau calque bitmap", "calque.nouveau_bitmap");
        self.action_item(ui, "Nouveau calque de fond", "calque.nouveau_fond");
        ui.separator();
        let many = |one: &str, all: &str| if n > 1 { format!("{all} ({n})") } else { one.to_string() };
        self.action_item(ui, &many("Dupliquer le calque", "Dupliquer les calques choisis"), "calque.dupliquer");
        self.action_item(ui, &many("Supprimer le calque", "Supprimer les calques choisis"), "calque.supprimer");
        self.action_item(ui, "Couper le calque", "calque.couper");
        self.action_item(ui, "Copier le calque", "calque.copier");
        self.action_item(ui, "Coller le calque", "calque.coller");
        self.action_item(ui, "Coller à la place du calque courant", "calque.coller_remplacer");
        ui.separator();
        ui.menu_button("Fusionner", |ui| {
            self.action_item(ui, "Avec le calque du dessous", "calque.fusionner_dessous");
            self.action_item(ui, &format!("Les calques choisis ({n})"), "calque.fusionner_choisis");
            self.action_item(ui, "Les calques visibles", "calque.fusionner_visibles");
            self.action_item(ui, "Tous les calques", "calque.fusionner_tout");
        });
        ui.menu_button("Choisir", |ui| {
            self.action_item(ui, "Tous les calques", "calque.choisir_tout");
            self.action_item(ui, "Les calques visibles", "calque.choisir_visibles");
            self.action_item(ui, "Les calques du même groupe", "calque.choisir_groupe");
            self.action_item(ui, "Les calques des autres groupes", "calque.choisir_autres_groupes");
            self.action_item(ui, "Aucun (le calque courant seul)", "calque.choisir_aucun");
        });
        ui.menu_button("Afficher", |ui| {
            self.action_item(ui, "Tous les calques", "calque.tous_visibles");
            self.action_item(ui, "Aucun calque", "calque.tous_caches");
            self.action_item(ui, "Le calque courant seul", "calque.courant_seul");
        });
        ui.menu_button("Nettoyer", |ui| {
            self.action_item(ui, "Supprimer les calques vides", "calque.supprimer_vides");
            self.action_item(ui, "Supprimer les calques cachés", "calque.supprimer_caches");
        });
        ui.menu_button("Pochoirs", |ui| {
            self.action_item(ui, "Éteindre tous les pochoirs", "calque.pochoirs_eteindre");
            self.action_item(ui, "Inverser tous les pochoirs", "calque.pochoirs_inverser");
        });
        ui.menu_button("Groupes", |ui| self.groups_menu(ui));
        ui.menu_button("Dossiers", |ui| {
            self.action_item(ui, "Nouveau dossier", "dossier.nouveau");
            self.action_item(ui, "Placer les calques choisis dans un dossier", "dossier.ranger");
            self.action_item(ui, "Déposer les calques choisis dans le dossier", "dossier.deposer");
            self.action_item(ui, "Sortir les calques choisis de leur dossier", "dossier.sortir");
            ui.separator();
            self.action_item(ui, "Joindre les dossiers choisis dans le dossier courant", "dossier.joindre");
            self.action_item(ui, "Supprimer uniquement le dossier", "dossier.supprimer_seul");
            self.action_item(ui, "Supprimer le dossier et ses calques", "calque.supprimer");
        });
        ui.separator();
        let lock = if self.layer().locked { "Déverrouiller le calque" } else { "Verrouiller le calque" };
        self.action_item(ui, lock, "calque.verrouiller");
        self.action_item(ui, "Pelure d'oignon du calque", "calque.pelure");
        ui.separator();
        self.action_item(ui, "Calque du dessus", "calque.dessus");
        self.action_item(ui, "Calque du dessous", "calque.dessous");
        self.action_item(ui, "Premier calque", "calque.premier");
        self.action_item(ui, "Dernier calque", "calque.dernier");
        self.action_item(ui, "Monter le calque", "calque.monter");
        self.action_item(ui, "Descendre le calque", "calque.descendre");
    }

    /// Rangs des calques choisis du plan, du plus bas au plus haut ; le calque courant en est.
    pub(crate) fn chosen_layers(&self) -> Vec<usize> {
        let mut out: Vec<usize> = self
            .layers()
            .iter()
            .enumerate()
            .filter(|(i, l)| *i == self.cur_layer || self.layer_sel.contains(&l.id))
            .map(|(i, _)| i)
            .collect();
        out.dedup();
        out
    }

    /// Le calque `li` est choisi (en plus du courant).
    pub(crate) fn is_chosen_layer(&self, li: usize) -> bool {
        li == self.cur_layer || self.layers().get(li).is_some_and(|l| self.layer_sel.contains(&l.id))
    }

    /// Clic dans l'en-tête : seul (devient le calque courant, plus aucun autre choisi), avec Ctrl
    /// (ajouté ou retiré du choix), avec Maj (tous ceux entre le courant et lui).
    pub(crate) fn click_layer_head(&mut self, li: usize, ctrl: bool, shift: bool) {
        let Some(id) = self.layers().get(li).map(|l| l.id) else { return };
        if shift {
            let (a, b) = (li.min(self.cur_layer), li.max(self.cur_layer));
            self.layer_sel = self.layers()[a..=b].iter().map(|l| l.id).collect();
        } else if ctrl {
            if li == self.cur_layer {
                return;
            }
            match self.layer_sel.iter().position(|&x| x == id) {
                Some(k) => {
                    self.layer_sel.remove(k);
                }
                None => self.layer_sel.push(id),
            }
        } else {
            self.layer_sel.clear();
            self.select_layer(li);
        }
    }

    /// Choisit des calques du plan (menu « Choisir »).
    pub(crate) fn choose_layers(&mut self, how: Choose) {
        self.layer_sel = choose(self.layers(), self.cur_layer, how);
        let n = self.chosen_layers().len();
        self.status = format!("{n} calque{} choisi{}.", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" });
    }

    /// Maj + Haut / Bas : le calque du dessus (du dessous) devient le calque courant, et ceux
    /// qu'on quitte restent choisis.
    pub(crate) fn extend_layer_choice(&mut self, up: bool) {
        let to = if up { self.cur_layer + 1 } else { self.cur_layer.wrapping_sub(1) };
        if to >= self.layers().len() {
            return;
        }
        let id = self.layer().id;
        if !self.layer_sel.contains(&id) {
            self.layer_sel.push(id);
        }
        self.select_layer(to);
    }

    /// Duplique les calques choisis ; chaque copie va juste au-dessus de son calque.
    pub(crate) fn duplicate_chosen_layers(&mut self) {
        self.end_stroke();
        self.drop_bitmap_selection();
        let mut chosen = self.chosen_layers();
        // Un calque dont le dossier est aussi choisi part avec la copie du dossier.
        let all = chosen.clone();
        chosen.retain(|&li| crate::folders::parent_index(self.layers(), li).is_none_or(|fi| !all.contains(&fi)));
        let current = self.layer().id;
        let snap = self.snapshot();
        let mut copies = Vec::new();
        let mut current_copy = current;
        for &li in chosen.iter().rev() {
            let at = duplicate_layer(&mut self.doc, self.cur_panel, li);
            copies.push(self.layers()[at].id);
            if self.layers()[li].id == current {
                current_copy = self.layers()[at].id;
            }
        }
        // Les copies deviennent le choix ; le courant est la copie du calque qui l'était (les
        // copies posées dessous ont décalé les rangs : on le retrouve par son id).
        self.cur_layer = self.layers().iter().position(|l| l.id == current_copy).unwrap_or(self.cur_layer);
        self.layer_sel = copies;
        self.vsel = None;
        self.commit_structure(snap);
        let n = chosen.len();
        self.status = if n == 1 { "Calque dupliqué.".into() } else { format!("{n} calques dupliqués.") };
    }

    /// Supprime des calques du plan (il en reste au moins un) ; un dossier part avec ses calques.
    fn delete_layers(&mut self, lis: &[usize], what: &str) {
        self.end_stroke();
        self.drop_bitmap_selection();
        let mut lis: Vec<usize> = lis.iter().flat_map(|&li| crate::folders::block_of(self.layers(), li)).collect();
        lis.sort_unstable();
        lis.dedup();
        let lis = &lis[..];
        if lis.is_empty() {
            self.status = format!("Aucun calque {what}.");
            return;
        }
        if lis.len() >= self.layers().len() {
            self.status = "Il faut garder au moins un calque.".into();
            return;
        }
        let snap = self.snapshot();
        let cur = self.cur_layer;
        for &li in lis.iter().rev() {
            self.layers_mut().remove(li);
        }
        // Le courant devient le calque resté juste en dessous de l'ancien courant (sinon le plus bas).
        let below = cur - lis.iter().filter(|&&l| l <= cur).count().min(cur);
        self.cur_layer = below.min(self.layers().len() - 1);
        self.layer_sel.clear();
        self.vsel = None;
        self.commit_structure(snap);
        let n = lis.len();
        self.status = if n == 1 { "Calque supprimé.".into() } else { format!("{n} calques supprimés.") };
    }

    /// Supprime les calques choisis (Q).
    pub(crate) fn delete_chosen_layers(&mut self) {
        let chosen = self.chosen_layers();
        self.delete_layers(&chosen, "choisi");
    }

    /// Supprime les calques du plan qui n'ont aucun dessin.
    pub(crate) fn delete_empty_layers(&mut self) {
        let empty: Vec<usize> = (0..self.layers().len()).filter(|&i| is_empty_layer(&self.doc, &self.layers()[i])).collect();
        self.delete_layers(&empty, "vide");
    }

    /// Supprime les calques cachés du plan.
    pub(crate) fn delete_hidden_layers(&mut self) {
        let hidden: Vec<usize> = (0..self.layers().len()).filter(|&i| !self.layers()[i].visible).collect();
        self.delete_layers(&hidden, "caché");
    }

    /// Fusionne des calques du plan en un seul.
    pub(crate) fn merge_layer_list(&mut self, lis: Vec<usize>) {
        self.end_stroke();
        self.drop_bitmap_selection();
        let snap = self.snapshot();
        match merge_layers(&mut self.doc, self.cur_panel, &lis) {
            Ok(at) => {
                self.cur_layer = at;
                self.layer_sel.clear();
                self.vsel = None;
                self.commit_structure(snap);
                self.status = format!("{} calques fusionnés en un.", lis.len());
            }
            Err(e) => self.status = e,
        }
    }

    pub(crate) fn merge_chosen_layers(&mut self) {
        let chosen = self.chosen_layers();
        self.merge_layer_list(chosen);
    }

    pub(crate) fn merge_visible_layers(&mut self) {
        let visible = (0..self.layers().len()).filter(|&i| self.layers()[i].visible).collect();
        self.merge_layer_list(visible);
    }

    pub(crate) fn merge_all_layers(&mut self) {
        let all = (0..self.layers().len()).collect();
        self.merge_layer_list(all);
    }

    /// Change un réglage de tous les calques du plan (une étape d'annulation).
    fn set_all_layers(&mut self, change: impl Fn(usize, &mut Layer), status: &str) {
        self.end_stroke();
        self.record_step();
        for (i, l) in self.layers_mut().iter_mut().enumerate() {
            change(i, l);
        }
        self.renderer.invalidate_all();
        self.status = status.into();
    }

    /// Tous les calques visibles.
    pub(crate) fn show_all_layers(&mut self) {
        self.set_all_layers(|_, l| l.visible = true, "Tous les calques sont visibles.");
    }

    /// Tous les calques cachés.
    pub(crate) fn hide_all_layers(&mut self) {
        self.set_all_layers(|_, l| l.visible = false, "Tous les calques sont cachés.");
    }

    /// Le calque courant seul visible.
    pub(crate) fn show_current_layer_only(&mut self) {
        let cur = self.cur_layer;
        self.set_all_layers(move |i, l| l.visible = i == cur, "Seul le calque courant est visible.");
    }

    /// Tous les pochoirs éteints.
    pub(crate) fn stencils_off(&mut self) {
        self.set_all_layers(|_, l| l.stencil = false, "Pochoirs éteints.");
    }

    /// Chaque pochoir change de sens (positif ↔ négatif).
    pub(crate) fn stencils_invert(&mut self) {
        self.set_all_layers(|_, l| if l.stencil { l.stencil_negative = !l.stencil_negative }, "Pochoirs inversés.");
    }

    /// Pelure d'oignon du calque courant, allumée ou éteinte.
    pub(crate) fn toggle_layer_onion(&mut self) {
        let li = self.cur_layer;
        self.set_layer_flag(li, |l| l.onion = !l.onion);
        let on = self.layer().onion;
        self.status = format!("Pelure d'oignon du calque : {}.", if on { "allumée" } else { "éteinte" });
    }

    /// Le premier (le plus bas) ou le dernier (le plus haut) calque devient le calque courant.
    pub(crate) fn select_end_layer(&mut self, last: bool) {
        let to = if last { self.layers().len() - 1 } else { 0 };
        self.select_layer(to);
    }

    /// Coupe le calque courant : copié (W), puis supprimé.
    pub(crate) fn cut_layer(&mut self) {
        if self.layers().len() <= 1 {
            self.status = "Il faut garder au moins un calque.".into();
            return;
        }
        self.copy_layer();
        let li = self.cur_layer;
        self.delete_layers(&[li], "coupé");
        self.status = "Calque coupé : X pour le coller.".into();
    }

    /// Colle le calque copié à la place du calque courant.
    pub(crate) fn paste_layer_replacing(&mut self) {
        if self.layer_clipboard.is_none() {
            self.status = "Aucun calque copié (W pour copier le calque courant).".into();
            return;
        }
        let old = self.layer().id;
        self.paste_layer();
        // L'ancien courant (un dossier : avec ses calques) part, dans la même étape d'annulation.
        let pasted = self.layer().id;
        let Some(oi) = self.layers().iter().position(|l| l.id == old) else { return };
        if pasted == old {
            return;
        }
        let block = crate::folders::block_of(self.layers(), oi);
        if block.len() >= self.layers().len() {
            return;
        }
        for &li in block.iter().rev() {
            self.layers_mut().remove(li);
        }
        crate::folders::normalize(self.layers_mut());
        self.cur_layer = self.layers().iter().position(|l| l.id == pasted).unwrap_or(0);
        self.renderer.invalidate_all();
        self.status = "Calque collé à la place du calque courant.".into();
    }
}
