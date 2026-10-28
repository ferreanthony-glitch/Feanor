//! La couleur de l'aplat et sa **portée** (Anthony, 2026-10-28) : à côté de la couleur, « Changer la couleur de l'aplat sur : la
//! clé / le plan / tout le storyboard ». Changer la couleur recolore alors les aplats de la portée qui avaient l'ancienne
//! couleur, en gardant leur opacité ; le nombre de dessins changés est dit dans la barre d'état ; une seule étape d'annulation.
//!
//! - **La clé** (d'office) : l'aplat du dessin courant, ou de chaque clé choisie si plusieurs le sont (celle qu'on voit parmi elles) :
//!   les pixels peints dans l'aplat (strate, et calques de couleur du calque) qui avaient l'ancienne couleur prennent la
//!   nouvelle ; la forme de l'aplat (déduite du trait, couleur du calque) prend la nouvelle couleur **pour ce dessin seul** : sa
//!   forme est peinte de la nouvelle couleur dans la strate, sur un fil de travail (le calcul de la forme coûte). La couleur du
//!   calque ne change pas : les autres clés gardent la leur.
//! - **Le plan** / **tout le storyboard** : en plus, la couleur de chaque calque de ces plans qui était l'ancienne devient la nouvelle
//!   (tous ses dessins), et les calques de couleur de ces plans sont recolorés de même.
//!
//! La couleur ne se pose pas à chaque image du glissé du sélecteur (il y aurait de quoi brûler le processeur sur un grand
//! storyboard) : elle est retenue pendant le glissé et posée au lâcher (`tick_aplat_couleur`, à chaque image d'écran, rien au repos).

use eframe::egui;

use crate::app::App;
use crate::document::CelId;
use crate::history::Edit;
use crate::raster::{premultiply, unpremultiply, Raster};

/// Sur quoi porte le changement de couleur de l'aplat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AplatPortee {
    #[default]
    Cle,
    Plan,
    Storyboard,
}

impl AplatPortee {
    pub const TOUS: [AplatPortee; 3] = [AplatPortee::Cle, AplatPortee::Plan, AplatPortee::Storyboard];

    pub fn label(self) -> &'static str {
        match self {
            AplatPortee::Cle => "La clé",
            AplatPortee::Plan => "Le plan",
            AplatPortee::Storyboard => "Tout le storyboard",
        }
    }

    fn aide(self) -> &'static str {
        match self {
            AplatPortee::Cle => "Seul l'aplat du dessin courant (ou de chaque clé choisie) change de couleur ; les autres clés gardent la leur.",
            AplatPortee::Plan => "Les aplats du plan qui avaient l'ancienne couleur prennent la nouvelle.",
            AplatPortee::Storyboard => "Les aplats de tout le storyboard qui avaient l'ancienne couleur prennent la nouvelle.",
        }
    }
}

/// Un changement de couleur retenu pendant le glissé du sélecteur.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CouleurAttente {
    layer: u64,
    old: [u8; 3],
    pub(crate) new: [u8; 3],
    portee: AplatPortee,
    /// L'image d'écran du dernier changement.
    image: u64,
}

impl CouleurAttente {
    /// Essais : l'ancienne couleur du changement retenu.
    #[cfg(test)]
    pub(crate) fn set_old_for_test(&mut self, old: [u8; 3]) {
        self.old = old;
    }
}

/// Les pixels de `r` qui ont la couleur `old` (à deux niveaux près : l'arrondi du prémultiplié) prennent `new`, leur opacité gardée.
/// Rend les tuiles d'avant (annulation) ; rien si rien n'a changé.
pub(crate) fn recolorer(r: &mut Raster, old: [u8; 3], new: [u8; 3]) -> Vec<(usize, Option<crate::raster::Tile>)> {
    let near = |u: [u8; 4]| (0..3).all(|k| u[k].abs_diff(old[k]) <= 2);
    let mut before = Vec::new();
    for idx in 0..r.tiles.len() {
        let Some(tile) = r.tiles[idx].as_ref() else { continue };
        if !tile.iter().any(|c| c.a() > 0 && near(unpremultiply(*c))) {
            continue;
        }
        before.push((idx, r.tiles[idx].clone()));
        if let Some(tile) = r.tiles[idx].as_mut() {
            for c in tile.iter_mut() {
                if c.a() > 0 && near(unpremultiply(*c)) {
                    *c = premultiply(new[0], new[1], new[2], c.a());
                }
            }
        }
    }
    before
}

impl App {
    /// La ligne « Couleur de l'aplat » et le choix de la portée, dans les réglages de l'aplat du calque `li`.
    pub(crate) fn aplat_couleur_ui(&mut self, ui: &mut egui::Ui, li: usize, couleur: [u8; 3]) {
        let Some(id) = self.layers().get(li).map(|l| l.id) else { return };
        let mut shown = self.aplat_couleur_attente.filter(|a| a.layer == id).map_or(couleur, |a| a.new);
        ui.horizontal(|ui| {
            ui.label("Couleur de l'aplat");
            let resp = ui.color_edit_button_srgb(&mut shown);
            if resp.changed() {
                self.choisir_couleur_aplat(li, shown, ui.ctx().cumulative_pass_nr());
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Changer la couleur de l'aplat sur :").small());
            for p in AplatPortee::TOUS {
                let resp = ui.selectable_label(self.aplat_portee == p, egui::RichText::new(p.label()).small());
                let resp = crate::bulles::bulle(resp, p.label(), p.aide(), "");
                if resp.clicked() {
                    self.aplat_portee = p;
                }
            }
        });
    }

    /// Le sélecteur donne `new` au calque `li` : retenu jusqu'au lâcher (`tick_aplat_couleur`).
    pub(crate) fn choisir_couleur_aplat(&mut self, li: usize, new: [u8; 3], image: u64) {
        let Some(l) = self.layers().get(li) else { return };
        let (id, actuelle) = (l.id, l.fond.map_or(new, |f| f.couleur));
        let old = self.aplat_couleur_attente.filter(|a| a.layer == id).map_or(actuelle, |a| a.old);
        self.aplat_couleur_attente = Some(CouleurAttente { layer: id, old, new, portee: self.aplat_portee, image });
    }

    /// À chaque image d'écran : une couleur retenue dont le sélecteur n'est plus tiré (bouton relâché, pas de nouveau changement
    /// depuis l'image d'avant) est posée. Rien au repos.
    pub(crate) fn tick_aplat_couleur(&mut self, ctx: &egui::Context) {
        let Some(a) = self.aplat_couleur_attente else { return };
        if ctx.input(|i| i.pointer.any_down()) || ctx.cumulative_pass_nr() <= a.image {
            ctx.request_repaint();
            return;
        }
        self.appliquer_couleur_aplat();
    }

    /// Pose la couleur retenue sur sa portée : une étape d'annulation « Aplat : couleur », le nombre de dessins changés dans la
    /// barre d'état. Rend ce nombre.
    pub(crate) fn appliquer_couleur_aplat(&mut self) -> usize {
        let Some(a) = self.aplat_couleur_attente.take() else { return 0 };
        if a.old == a.new || self.content_refuses() {
            return 0;
        }
        let Some(li) = self.layers().iter().position(|l| l.id == a.layer) else { return 0 };
        self.end_stroke();
        let (old, new) = (a.old, a.new);
        let cur_panel = self.cur_panel;
        let panels: Vec<usize> = match a.portee {
            AplatPortee::Storyboard => (0..self.doc.panels.len()).collect(),
            _ => vec![cur_panel],
        };
        // Les dessins visés par la portée « la clé » : celui qu'on voit, ou ceux des clés choisies s'il y en a deux ou plus.
        let frames: Vec<usize> = self.cles_de_la_portee(li);
        // Ce qui change : (plan, rang du calque) dont la couleur de calque change ; (dessin, strate ou non) dont les pixels changent.
        let mut calques: Vec<(usize, usize)> = Vec::new();
        let mut pixels: Vec<(CelId, bool)> = Vec::new();
        let mut cles_teintees: Vec<(usize, CelId)> = Vec::new();
        for &pi in &panels {
            let layers = &self.doc.panels[pi].layers;
            for (k, l) in layers.iter().enumerate() {
                if a.portee == AplatPortee::Cle {
                    if pi != cur_panel {
                        continue;
                    }
                    // Le calque courant (son aplat de strate) et ses calques de couleur, aux images des clés.
                    if k == li {
                        for &f in &frames {
                            if let Some(c) = l.cel_at(f) {
                                if l.fond.is_some() && !pixels.contains(&(c, true)) {
                                    pixels.push((c, true));
                                }
                                if l.fond.is_some_and(|r| r.couleur == old) && !cles_teintees.iter().any(|t| t.1 == c) {
                                    cles_teintees.push((li, c));
                                }
                            }
                        }
                    } else if l.couleur.as_ref().is_some_and(|c| c.trait_id == layers[li].id) {
                        for &f in &frames {
                            if let Some(c) = l.cel_at(f) {
                                if !pixels.contains(&(c, false)) {
                                    pixels.push((c, false));
                                }
                            }
                        }
                    }
                    continue;
                }
                if l.fond.is_some() {
                    for c in l.frames.iter().flatten() {
                        if !pixels.contains(&(*c, true)) {
                            pixels.push((*c, true));
                        }
                    }
                    if l.fond.is_some_and(|r| r.couleur == old) {
                        calques.push((pi, k));
                    }
                }
                if l.couleur.is_some() {
                    for c in l.frames.iter().flatten() {
                        if !pixels.contains(&(*c, false)) {
                            pixels.push((*c, false));
                        }
                    }
                }
            }
        }
        let mut changes: std::collections::BTreeSet<CelId> = std::collections::BTreeSet::new();
        // Les pixels peints.
        let mut edits: Vec<Edit> = Vec::new();
        for (cel, strate) in pixels {
            let Some(c) = self.doc.cels.get_mut(&cel) else { continue };
            let tiles = if strate {
                match c.fond.as_mut() {
                    Some(s) => {
                        let t = recolorer(&mut s.peinture.raster, old, new);
                        if !t.is_empty() {
                            s.peinture.version += 1;
                        }
                        t
                    }
                    None => Vec::new(),
                }
            } else {
                recolorer(&mut c.raster, old, new)
            };
            if tiles.is_empty() {
                continue;
            }
            c.version += 1;
            changes.insert(cel);
            edits.push(if strate { Edit::FondTiles { cel, tiles, coupe: false } } else { Edit::Tiles { cel, tiles } });
        }
        if edits.is_empty() && calques.is_empty() && cles_teintees.is_empty() {
            self.status = format!("Aplat : aucun dessin ({}) n'avait cette couleur d'aplat.", a.portee.label().to_lowercase());
            return 0;
        }
        // Une seule étape : la structure (couleur des calques) puis les pixels, qui la rejoignent.
        self.record_named_step("Aplat : couleur");
        if !edits.is_empty() {
            self.history.extend_last(Edit::Group(edits));
        }
        // La couleur de calque : tous les dessins du calque (ceux de ses images).
        for &(pi, k) in &calques {
            if let Some(f) = self.doc.panels[pi].layers[k].fond.as_mut() {
                f.couleur = new;
            }
            changes.extend(self.doc.panels[pi].layers[k].frames.iter().flatten().copied());
        }
        // « La clé » : la forme de l'aplat de ce dessin prend la nouvelle couleur, pour lui seul (sur un fil de travail).
        if !cles_teintees.is_empty() {
            let serial = self.history.serial();
            for (k, cel) in cles_teintees {
                if self.lancer_teinte(k, cel, new, serial) {
                    changes.insert(cel);
                }
            }
        }
        self.modified = true;
        self.renderer.invalidate_all();
        let n = changes.len();
        self.status = format!("Aplat : couleur changée sur {n} dessin{} ({}).", if n > 1 { "s" } else { "" }, a.portee.label().to_lowercase());
        n
    }

    /// Les images où commencent les clés dont l'aplat du calque `li` change de couleur : celles des clés choisies de ce calque
    /// (deux au moins, avec celle qu'on voit), sinon l'image courante.
    fn cles_de_la_portee(&self, li: usize) -> Vec<usize> {
        let (Some(l), Some(panel)) = (self.layers().get(li), self.doc.panels.get(self.cur_panel)) else { return vec![self.cur_frame] };
        let keys: Vec<usize> = self.key_sel.iter().filter(|k| k.panel == panel.id && k.layer == l.id).map(|k| k.start).collect();
        let voit = l.cel_at(self.cur_frame);
        if keys.len() >= 2 && keys.iter().any(|&s| l.cel_at(s) == voit && voit.is_some()) {
            keys
        } else {
            vec![self.cur_frame]
        }
    }

    /// Essais : la couleur d'un dessin de couleur (`Color32`) reste lisible.
    #[cfg(test)]
    pub(crate) fn couleur_attente(&self) -> Option<[u8; 3]> {
        self.aplat_couleur_attente.map(|a| a.new)
    }
}
