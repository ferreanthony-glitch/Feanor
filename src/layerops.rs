//! Opérations sur les calques du plan (13b) : calques choisis (Ctrl + clic, Maj + clic dans
//! l'en-tête de la timeline ; le calque courant en fait toujours partie), dupliquer, supprimer,
//! fusionner les choisis / les visibles / tous, supprimer les calques vides ou cachés, tout
//! afficher, tout cacher, le calque courant seul, pochoirs, rang du calque, premier / dernier.
//!
//! Les fonctions libres travaillent sur le document (testées dans `layerflags_tests.rs`) ; les
//! méthodes d'`App` y ajoutent l'étape d'annulation et le calque courant.

use std::collections::HashMap;

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
        // (Les griffonnages d'un calque de couleur comptent : ils remplissent des zones.)
        && layer.couleur.as_ref().is_none_or(|c| c.dessins_griffonnages().all(|g| doc.cels.get(&g).is_none_or(|c| c.is_empty())))
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

/// Un nom de calque qui n'est pas dans `pris` : `nom` s'il est libre, sinon « Base (2) », « Base (3) »… (`Base` : `nom` sans un
/// « (n) » final, pour que la copie de « Trait (2) » soit « Trait (3) » et non « Trait (2) (2) »).
pub(crate) fn nom_libre(pris: &std::collections::HashSet<String>, nom: &str) -> String {
    if !pris.contains(nom) {
        return nom.to_string();
    }
    let base = match nom.rsplit_once(" (") {
        Some((b, fin)) if fin.ends_with(')') && fin[..fin.len() - 1].chars().all(|c| c.is_ascii_digit()) && fin.len() > 1 && !b.is_empty() => b,
        _ => nom,
    };
    (2..).map(|n| format!("{base} ({n})")).find(|c| !pris.contains(c)).unwrap_or_else(|| nom.to_string())
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
    // (La copie du calque d'un personnage reste liée au personnage, Anthony 2026-10-29 : choisir sa brosse reste dans la copie si c'est le calque
    // courant, sinon va au dernier calque du personnage utilisé dans le plan, `calquespastilles.rs`.)
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
/// rang. Rien ne change si l'une des fusions est refusée (calque caché, verrouillé, vidéo…) : tout est vérifié avant
/// (`fusioncalques::blocker_list`), et les dessins faits en route sont retirés si la fusion échoue quand même.
pub(crate) fn merge_layers(doc: &mut Document, index: usize, lis: &[usize]) -> Result<usize, String> {
    merge_layers_ctl(doc, index, lis, None)
}

/// `merge_layers`, qui regarde `ctl` à chaque image (la tâche de fond : « Arrêter » et la progression).
pub(crate) fn merge_layers_ctl(doc: &mut Document, index: usize, lis: &[usize], ctl: Option<&crate::fusioncalques::Controle>) -> Result<usize, String> {
    // Un dossier choisi compte pour ses calques ; un dossier ne se fusionne pas lui-même.
    let layers = doc.panels.get(index).map_or(&[][..], |p| &p.layers[..]);
    let lis = crate::fusioncalques::rangs_a_fusionner(layers, lis);
    if let Some(raison) = crate::fusioncalques::blocker_list(doc, index, &lis) {
        return Err(raison);
    }
    let before = doc.panels[index].layers.clone();
    let cels_avant: std::collections::HashSet<CelId> = doc.cels.keys().copied().collect();
    let low = lis[0];
    let layers = &mut doc.panels[index].layers;
    let mut picked: Vec<Layer> = lis.iter().rev().map(|&l| layers.remove(l)).collect();
    picked.reverse();
    let count = picked.len();
    for (k, l) in picked.into_iter().enumerate() {
        layers.insert(low + k, l);
    }
    for _ in 1..count {
        if let Err(e) = crate::fusioncalques::merge_down_ctl(doc, index, low + 1, ctl) {
            doc.panels[index].layers = before;
            doc.cels.retain(|id, _| cels_avant.contains(id));
            return Err(e);
        }
    }
    Ok(low)
}

impl App {
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
                None => {
                    // Le calque courant est inscrit dans le choix : il y reste quand un autre calque choisi devient le courant (un
                    // clic dans sa piste pour y placer la tête de lecture ne réduit plus le choix à un seul calque, 2026-11-11).
                    self.inscrire_le_calque_courant();
                    self.layer_sel.push(id);
                }
            }
            // Plus que le calque courant : il n'y a plus de choix de plusieurs calques.
            let courant = self.layers().get(self.cur_layer).map(|l| l.id);
            if self.layer_sel.len() == 1 && self.layer_sel.first().copied() == courant {
                self.layer_sel.clear();
            }
        } else {
            self.layer_sel.clear();
            self.select_layer(li);
            // Un clic sur le calque (panneau Calques, timeline) : on dessine de nouveau le trait, pas l'aplat.
            self.aplat_cible = None;
        }
    }

    /// Le calque courant est inscrit parmi les calques choisis (`layer_sel`) : il le reste quand un autre devient le courant.
    fn inscrire_le_calque_courant(&mut self) {
        if let Some(id) = self.layers().get(self.cur_layer).map(|l| l.id) {
            if !self.layer_sel.contains(&id) {
                self.layer_sel.push(id);
            }
        }
    }

    /// Un clic dans la piste du calque `li` du plan courant (montage) : il devient le calque courant. Avec plusieurs calques choisis :
    /// sur l'un d'eux, ou avec Ctrl ou Maj (`garde` : on prend une clé de plus), le choix reste entier, le calque qu'on quitte compris
    /// (on vient placer la tête de lecture ou prendre une clé, pas défaire le choix ; Anthony, 2026-11-11 : une nouvelle clé sur
    /// plusieurs calques ne se posait que sur un seul après un tel clic) ; sans touche sur un calque hors du choix, lui seul reste
    /// choisi, comme après un clic sur son nom.
    pub(crate) fn clic_dans_la_piste(&mut self, li: usize, garde: bool) {
        if !self.layer_sel.is_empty() && li != self.cur_layer && li < self.layers().len() {
            if garde || self.is_chosen_layer(li) {
                self.inscrire_le_calque_courant();
            } else {
                self.layer_sel.clear();
            }
        }
        self.select_layer(li);
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
        // Les dessins que la suppression emporte (dits dans la barre d'état ; seul Ctrl+Z les rend).
        let dessins = {
            let layers = self.layers();
            let mut vus = std::collections::BTreeSet::new();
            for &li in lis {
                vus.extend(layers[li].frames.iter().flatten().copied());
            }
            vus.len()
        };
        let (plan_id, calques): (u64, Vec<(usize, crate::document::Layer)>) = (self.panel().id, lis.iter().map(|&li| (li, self.layers()[li].clone())).collect());
        for &li in lis.iter().rev() {
            self.layers_mut().remove(li);
        }
        // Ce qui sort du film reste récupérable : les calques vont à la corbeille (`corbeille.rs`), dans la même étape d'annulation.
        let envoyes = self.doc.jeter_calques(plan_id, calques, "Calque supprimé");
        // Le courant devient le calque resté juste en dessous de l'ancien courant (sinon le plus bas).
        let below = cur - lis.iter().filter(|&&l| l <= cur).count().min(cur);
        self.cur_layer = below.min(self.layers().len() - 1);
        self.layer_sel.clear();
        self.vsel = None;
        self.commit_structure(snap);
        let n = lis.len();
        let perte = if dessins == 0 { String::new() } else { format!(" ({})", crate::rognage::dessins_texte(dessins)) };
        self.status = match (n, envoyes) {
            (1, true) => format!("Calque envoyé à la corbeille{perte}."),
            (n, true) => format!("{n} calques envoyés à la corbeille{perte}."),
            (1, false) => "Calque supprimé.".into(),
            (n, false) => format!("{n} calques supprimés."),
        };
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
        self.fusionner(lis);
    }

    /// Fusionne des calques du plan courant en un seul. Ce qui se calcule en moins d'une image d'écran (peu de dessins, ni effet
    /// ni aplat) se pose tout de suite ; le reste part en tâche de fond (`fusiontache.rs`) : l'interface reste fluide, la barre
    /// d'état montre l'avancement et « Arrêter », le résultat est posé en une seule étape d'annulation.
    pub(crate) fn fusionner(&mut self, lis: Vec<usize>) {
        if self.content_refuses() {
            return;
        }
        if self.fusion.is_some() {
            self.status = "Une fusion de calques est déjà en cours : attends sa fin, ou arrête-la (barre d'état).".into();
            return;
        }
        self.end_stroke();
        self.drop_bitmap_selection();
        if let Some(raison) = crate::fusioncalques::blocker_list(&self.doc, self.cur_panel, &lis) {
            self.status = raison;
            return;
        }
        if !crate::fusioncalques::est_rapide(&self.doc, self.cur_panel, &lis) {
            self.lancer_fusion(lis);
            return;
        }
        let snap = self.snapshot();
        let nombre = crate::fusioncalques::rangs_a_fusionner(self.layers(), &lis).len();
        let resultat = crate::fusioncalques::resultat_de(&self.doc, self.cur_panel, &lis);
        match merge_layers(&mut self.doc, self.cur_panel, &lis) {
            Ok(at) => {
                self.cur_layer = at;
                self.layer_sel.clear();
                self.vsel = None;
                self.commit_structure(snap);
                self.status = format!("{nombre} calques fusionnés en un. {resultat} Ctrl+Z les rend comme avant.");
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
        if self.basculer_calques(li, "Pelure d'oignon", |l| l.onion, |l, v| l.onion = v, false) >= 2 {
            return;
        }
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
        let choisis = self.chosen_layers();
        self.copy_layer();
        self.delete_layers(&choisis, "coupé");
        self.status = "Calque coupé : X pour le coller.".into();
    }

    /// Colle le calque copié à la place du calque courant.
    pub(crate) fn paste_layer_replacing(&mut self) {
        if self.layer_clipboard.is_none() {
            self.status = "Aucun calque copié (W pour copier le calque courant).".into();
            return;
        }
        let old = self.layer().id;
        let nom_voulu = self.layer_clipboard.as_ref().map(|c| c.layer.name.clone());
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
        // Le calque remplacé n'est plus là : le collé reprend le nom d'origine (il avait pris « Nom (2) » à cause de lui).
        if let Some(nom) = nom_voulu.filter(|n| !self.layers().iter().any(|l| &l.name == n)) {
            if let Some(l) = self.layers_mut().iter_mut().find(|l| l.id == pasted) {
                l.name = nom;
            }
        }
        self.cur_layer = self.layers().iter().position(|l| l.id == pasted).unwrap_or(0);
        self.renderer.invalidate_all();
        self.status = "Calque collé à la place du calque courant.".into();
    }
}
