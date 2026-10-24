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
    layer.frames.iter().flatten().all(|id| doc.cels.get_ou_absent(id).is_none_or(|c| c.is_empty()))
        // (Les griffonnages d'un calque de couleur comptent : ils remplissent des zones.)
        && layer.couleur.as_ref().is_none_or(|c| c.dessins_griffonnages().all(|g| doc.cels.get_ou_absent(&g).is_none_or(|c| c.is_empty())))
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
    // La copie d'un calque de toute la scène est un calque du seul plan ; les noms d'instance
    // suivent les copies des dessins.
    let names = source.names.iter().map(|(c, n)| (copies.get(c).copied().unwrap_or(*c), n.clone())).collect();
    let color_marks = source.color_marks.iter().map(|(c, k)| (copies.get(c).copied().unwrap_or(*c), *k)).collect();
    // (La copie du calque Décor est un calque ordinaire : le plan n'a qu'un décor.)
    let mut copy = Layer { id: fresh_layer_id(), frames, scene_wide: None, names, color_marks, decor: false, ..source.clone() };
    // Un calque de couleur copié seul garde son trait, avec les copies de ses dessins de couleur et de ses griffonnages.
    doc.copier_griffonnages(std::slice::from_ref(source), copies);
    if let Some(c) = copy.couleur.as_mut() {
        c.relink(&HashMap::new(), copies);
    }
    copy
}

/// Copie du calque `li` du plan `index`, avec ses propres copies des dessins, posée juste
/// au-dessus de lui ; un dossier est copié avec ses calques, au-dessus de lui et de ses calques
/// (13b-5). Renvoie le rang de la copie (du dossier copié).
pub(crate) fn duplicate_layer(doc: &mut Document, index: usize, li: usize) -> usize {
    let layers = &doc.panels[index].layers;
    let source = layers[li].clone();
    let children: Vec<Layer> = crate::folders::children_of(layers, li).into_iter().map(|i| layers[i].clone()).collect();
    // Les calques de couleur du trait partent avec lui : leurs copies colorent la copie.
    let colours: Vec<Layer> = crate::calquecouleur::couleurs_de(layers, li).into_iter().map(|i| layers[i].clone()).collect();
    let mut copies: HashMap<CelId, CelId> = HashMap::new();
    let mut copy = layer_copy(doc, &source, &mut copies);
    copy.name = format!("{} copie", source.name);
    let mut block: Vec<Layer> = Vec::new();
    for c in &children {
        let mut child = layer_copy(doc, c, &mut copies);
        child.parent = Some(copy.id);
        block.push(child);
    }
    for c in &colours {
        let mut colour = layer_copy(doc, c, &mut copies);
        colour.name = format!("{} copie", c.name);
        colour.parent = copy.parent;
        if let Some(link) = colour.couleur.as_mut() {
            link.relink(&HashMap::from([(source.id, copy.id)]), &copies);
        }
        block.push(colour);
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

/// D'où s'ouvrent les commandes des calques : ce qui s'y voit déjà n'est pas répété (Anthony,
/// 2026-10-17 : le clic droit « ne doit pas faire doublon avec les options déjà disponibles dans
/// la fenêtre de calques »).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpsFrom {
    /// Menu Calques de la barre, ☰ de la timeline : tout.
    Everywhere,
    /// Sous-menu du menu d'un calque, qui a déjà dupliquer, supprimer, verrouiller, pelure.
    LayerMenu,
    /// Menu « … » du panneau Calques et sous-menu de son clic droit : sans non plus ce que font ses cinq
    /// icônes (le « + » des nouveaux calques, dupliquer, supprimer, monter, descendre) et son verrou ;
    /// copier, coller et fusionner n'ont plus d'icône : ils sont ici.
    LayersPanel,
}

impl App {
    /// Menu des opérations sur les calques (menu Calques, et bouton ☰ de l'en-tête de la timeline).
    pub(crate) fn layer_ops_menu(&mut self, ui: &mut egui::Ui) {
        self.layer_ops_menu_from(ui, OpsFrom::Everywhere);
    }

    /// `layer_ops_menu`, sans ce que l'endroit `from` montre déjà.
    pub(crate) fn layer_ops_menu_from(&mut self, ui: &mut egui::Ui, from: OpsFrom) {
        // Menu court (Anthony, 2026-10-18 : « certains menus contiennent trop de choses ») : ce qui
        // sert souvent en premier niveau, le reste sous « Plus ». Verrou et pelure d'oignon ont leur
        // icône sur chaque ligne de calque (et Alt+L, Ctrl+L).
        let n = self.chosen_layers().len();
        let (all, panel) = (from == OpsFrom::Everywhere, from == OpsFrom::LayersPanel);
        // Les nouveaux calques : le menu du « + » (`calquemenu.rs`). Le panneau Calques a son « + » : pas de doublon.
        if !panel {
            ui.menu_button("Nouveau calque", |ui| self.layer_new_menu(ui));
        }
        let many = |one: &str, several: &str| if n > 1 { format!("{several} ({n})") } else { one.to_string() };
        if all {
            self.action_item(ui, &many("Dupliquer le calque", "Dupliquer les calques choisis"), "calque.dupliquer");
            self.action_item(ui, &many("Supprimer le calque", "Supprimer les calques choisis"), "calque.supprimer");
        }
        ui.menu_button("Presse-papiers", |ui| {
            self.action_item(ui, "Couper le calque", "calque.couper");
            self.action_item(ui, "Copier le calque", "calque.copier");
            self.action_item(ui, "Coller le calque", "calque.coller");
            self.action_item(ui, "Coller à la place du calque courant", "calque.coller_remplacer");
        });
        ui.menu_button("Retourner le calque", |ui| {
            self.action_item(ui, "De gauche à droite", "calque.miroir_h");
            self.action_item(ui, "De haut en bas", "calque.miroir_v");
        });
        ui.menu_button("Fusionner", |ui| {
            self.action_item(ui, "Avec le calque du dessous", "calque.fusionner_dessous");
            self.action_item(ui, &format!("Les calques choisis ({n})"), "calque.fusionner_choisis");
            self.action_item(ui, "Les calques visibles", "calque.fusionner_visibles");
            self.action_item(ui, "Tous les calques", "calque.fusionner_tout");
        });
        // Aplat sous le trait (`fondui.rs`) : un calque, les calques choisis, tout le plan.
        ui.menu_button("Aplat sous le trait", |ui| self.fond_menu(ui, all));
        if all {
            // La propagation (façon EbSynth) du calque courant, aussi dans le menu Calques.
            self.propagation_item(ui, self.cur_layer);
            self.coloriage_item(ui, self.cur_layer);
        }
        ui.separator();
        ui.menu_button("Ordre", |ui| {
            if !panel {
                self.action_item(ui, "Monter le calque", "calque.monter");
                self.action_item(ui, "Descendre le calque", "calque.descendre");
                ui.separator();
            }
            self.action_item(ui, "Aller au calque du dessus", "calque.dessus");
            self.action_item(ui, "Aller au calque du dessous", "calque.dessous");
            self.action_item(ui, "Aller au premier calque", "calque.premier");
            self.action_item(ui, "Aller au dernier calque", "calque.dernier");
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
        ui.menu_button("Plus", |ui| {
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
            ui.menu_button("Pochoirs", |ui| {
                self.action_item(ui, "Éteindre tous les pochoirs", "calque.pochoirs_eteindre");
                self.action_item(ui, "Inverser tous les pochoirs", "calque.pochoirs_inverser");
            });
            ui.menu_button("Nettoyer", |ui| {
                self.action_item(ui, "Supprimer les calques vides", "calque.supprimer_vides");
                self.action_item(ui, "Supprimer les calques cachés", "calque.supprimer_caches");
            });
            // Calque de couleur : ses réglages (griffonner, fermeture des trous, sources).
            let cur = self.cur_layer;
            ui.menu_button("Calque de couleur", |ui| self.couleur_menu(ui, cur));
        });
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
            // Un clic sur le calque (panneau Calques, timeline) : on dessine de nouveau le trait, pas l'aplat.
            self.aplat_cible = None;
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
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.drop_bitmap_selection();
        let mut chosen = self.chosen_layers();
        // Un calque dont le dossier est aussi choisi part avec la copie du dossier.
        let all = chosen.clone();
        chosen.retain(|&li| crate::folders::parent_index(self.layers(), li).is_none_or(|fi| !all.contains(&fi)));
        // De même un calque de couleur dont le trait est aussi choisi : il part avec la copie du trait.
        chosen.retain(|&li| crate::calquecouleur::trait_de(self.layers(), li).is_none_or(|ti| !all.contains(&ti)));
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
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.drop_bitmap_selection();
        let mut lis: Vec<usize> = lis.iter().flat_map(|&li| crate::folders::block_of(self.layers(), li)).collect();
        lis.sort_unstable();
        lis.dedup();
        // Un calque de trait emmène ses calques de couleur.
        let lis = crate::calquecouleur::avec_couleurs(self.layers(), &lis);
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
        if self.content_refuses() {
            return;
        }
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
        // (Un calque de trait garde ses calques de couleur : ils font un seul dessin.)
        let mut keep = crate::calquecouleur::couleurs_de(self.layers(), cur);
        keep.push(cur);
        self.set_all_layers(move |i, l| l.visible = keep.contains(&i), "Seul le calque courant est visible.");
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
        if self.content_refuses() {
            return;
        }
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
