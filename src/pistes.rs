//! Pistes du montage (Anthony, 2026-10-20 : « un système de dépliage / repliage des calques dans la timeline pour gagner de la place
//! tout en gardant cela lisible » ; 2026-10-28, branche `timeline-repliee` : « seul le calque actif est déplié »).
//!
//! Dans le Storyboard et le Montage (l'onglet Animation garde sa timeline, tous calques dépliés), une piste a deux hauteurs :
//! - dépliée (`LAYER_H`, 40) : celle du calque **actif** (le calque courant) ; ses dessins en blocs hauts, sa barre, ses clés ;
//! - repliée (`FOLDED_H`, 11) : une **bande fine** pour tous les autres. Les dessins y sont des barres de la couleur du calque (un peu
//!   atténuée), les clés de position (vertes) et d'effets (violettes) de petits traits.
//!
//! Dans la colonne de gauche, une piste ne porte que l'œil, une pastille de la couleur de la sorte du calque (vectoriel, bitmap…) et le
//! nom : l'opacité, le verrou, le mode de fusion… sont au clic droit sur la piste (`montage_layer_menu`) et dans le panneau Calques.
//! Cliquer un calque (son nom ou l'un de ses blocs) le rend actif : il se déplie, l'ancien se replie, et le panneau Calques suit.
//!
//! **Un réglage du poste** (Préférences > Montage, `Inactifs`) : les calques non actifs en bandes fines (d'office) ou **cachés**, derrière
//! une ligne « ▾ N autres calques » qui les montre.
//!
//! **Rien n'est écrit dans le projet** : le calque actif et les pistes dépliées à la main sont de l'affichage (par poste, par séance). Le
//! drapeau `Layer::collapsed` n'est plus lu par le montage — il reste celui des « lignes réduites » de la timeline de l'animation et des
//! dossiers fermés (un dossier fermé n'a plus de piste pour ses calques, comme dans le panneau Calques).
//!
//! Un calque de couleur lié à un calque de trait (l'aplat sous le trait) n'a pas de piste à lui : `layer_in_row` l'ignore, un liseré rose
//! sous le bloc de son trait dit qu'il existe (`montagerows.rs`).
//!
//! Une piste (rangée r) traverse tous les plans : chaque plan a ses calques dans son ordre (6f). Le calque actif se déplie quand la souris
//! est relâchée (jamais pendant un geste : sinon les pistes d'en dessous glisseraient sous le pointeur).

use std::collections::BTreeSet;

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Response, RichText, Sense, Ui};
use egui_phosphor::regular as ph;
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::document::Panel;
use crate::montagerows::{layer_in_row, row_of_layer, LAYER_H};
use crate::theme;

/// Hauteur d'une piste repliée : une bande fine (9 points de bande, une fois les deux filets retirés).
pub(crate) const FOLDED_H: f32 = 11.0;
/// Hauteur de la ligne « ▾ N autres calques » (mode « cachés »).
pub(crate) const MORE_H: f32 = 14.0;
/// La « rangée » de la ligne « N autres calques » : aucun calque n'y répond.
pub(crate) const MORE_ROW: usize = usize::MAX;
/// Place de l'œil à gauche d'une piste.
const EYE_W: f32 = 15.0;
/// Place de la pastille de couleur, après l'œil.
const DOT_W: f32 = 10.0;

/// Une piste visible : la rangée de calques (0 : en haut de la pile), son haut (depuis le haut des
/// pistes de calques) et sa hauteur.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lane {
    pub row: usize,
    pub top: f32,
    pub h: f32,
    pub folded: bool,
}

impl Lane {
    /// La ligne « N autres calques » (pas une piste de calque).
    pub fn is_more(&self) -> bool {
        self.row == MORE_ROW
    }

    /// Les losanges d'effets (violets) se prennent jusqu'à cette hauteur depuis le haut de la piste ;
    /// sous elle, ceux de position, puis le dessin. Une bande fine n'en a pas : leurs clés y sont de petits traits, qu'on ne prend pas
    /// (les dessins, leurs limites et leurs bords, eux, se prennent comme avant).
    pub fn fx_hit_max(&self) -> f32 {
        if self.folded {
            -1.0
        } else {
            crate::fxkeys::STRIP
        }
    }

    /// Hauteur des losanges de position, depuis le haut de la piste (le milieu d'une piste dépliée).
    pub fn pos_y(&self) -> f32 {
        if self.folded {
            0.0
        } else {
            App::pos_marker_y(0.0, self.h)
        }
    }

    /// Distance verticale à laquelle on prend un losange de position.
    pub fn pos_hit(&self) -> f32 {
        if self.folded {
            -1.0
        } else {
            crate::poskeysui::HIT_H
        }
    }

    /// Depuis le haut de la piste, la hauteur où commence la barre qui décale le calque (aucune dans une bande fine : un clic y choisit le
    /// calque, il ne le décale pas par mégarde).
    pub fn bar_from(&self) -> f32 {
        if self.folded {
            self.h
        } else {
            self.h - crate::montagerows::BAR_OFF - crate::montagerows::BAR_H - 2.0
        }
    }
}

/// Une piste dépliée (pour les essais).
#[cfg(test)]
pub(crate) fn full_lane() -> Lane {
    Lane { row: 0, top: 0.0, h: LAYER_H, folded: false }
}

/// Le calque `(plan, calque)` — ids — qui est actif (déplié) : celui qui était courant au dernier relâchement de la souris.
pub(crate) type AutoOpen = Option<(u64, u64)>;

/// Que fait-on des calques qui ne sont pas actifs (réglage du poste, `reglages.toml` : `calques_non_actifs`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Inactifs {
    /// En bandes fines, dessins et clés visibles (d'office).
    #[default]
    Replies,
    /// Cachés : seul le calque actif a une piste, avec une ligne « ▾ N autres calques » qui montre les autres.
    Caches,
}

impl Inactifs {
    pub fn est_replie(&self) -> bool {
        *self == Inactifs::Replies
    }
}

/// Ce que le montage retient pour dire quelles pistes sont dépliées : le réglage et ce que l'utilisateur a fait à la main.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Replis {
    pub mode: Inactifs,
    /// Les rangées dépliées à la main (en plus du calque actif).
    pub ouvertes: BTreeSet<usize>,
    /// La rangée du calque actif est repliée à la main, jusqu'au prochain changement de calque.
    pub active_fermee: bool,
    /// Mode « cachés » : les autres calques sont montrés (clic sur la ligne « N autres calques »).
    pub autres_montres: bool,
}

/// Ce que le montage retient des pistes en dehors du document (affichage seulement).
#[derive(Default)]
pub(crate) struct PistesUi {
    /// Le calque actif, tel qu'il était courant au dernier relâchement de la souris.
    pub auto_open: AutoOpen,
    /// Le dernier calque courant vu : le calque actif ne change qu'à ce moment.
    seen: AutoOpen,
    /// Les rangées dépliées à la main.
    pub ouvertes: BTreeSet<usize>,
    /// La rangée du calque actif est repliée à la main.
    pub active_fermee: bool,
    /// Les calques cachés sont montrés.
    pub autres_montres: bool,
}

/// La rangée `row` est cachée : son calque, dans le plan courant `cur`, est rangé dans un dossier fermé
/// (sauf s'il est le calque courant `cur_layer` : comme dans le panneau Calques, on ne le perd pas de vue).
pub(crate) fn row_hidden(panels: &[Panel], cur: usize, cur_layer: usize, row: usize) -> bool {
    let Some(layers) = panels.get(cur).map(|p| &p.layers) else { return false };
    let Some(k) = layer_in_row(layers, row) else { return false };
    k != cur_layer && layers[k].parent.is_some_and(|p| layers.iter().any(|f| f.folder && f.id == p && f.collapsed))
}

/// La rangée du calque actif `auto` (celle de son plan ; un calque de couleur lié est à la rangée de son trait).
pub(crate) fn active_row(panels: &[Panel], auto: AutoOpen) -> Option<usize> {
    let (panel, layer) = auto?;
    let p = panels.iter().find(|p| p.id == panel)?;
    let k = p.layers.iter().position(|l| l.id == layer)?;
    row_of_layer(&p.layers, k)
}

/// La rangée `row` est repliée : elle ne l'est pas si c'est celle du calque actif (sauf repliée à la main) ou si on l'a dépliée à la main.
pub(crate) fn row_folded_in(panels: &[Panel], row: usize, auto: AutoOpen, replis: &Replis) -> bool {
    let active = active_row(panels, auto) == Some(row) && !replis.active_fermee;
    !active && !replis.ouvertes.contains(&row)
}

/// Les pistes visibles de haut en bas, `count` rangées au plus, celles d'un dossier fermé en moins. Mode « cachés » : seules les pistes
/// dépliées, puis la ligne « N autres calques » (qui montre les autres quand `replis.autres_montres`).
pub(crate) fn lanes_in(panels: &[Panel], cur: usize, cur_layer: usize, count: usize, auto: AutoOpen, replis: &Replis) -> Vec<Lane> {
    let mut top = 0.0;
    let mut out = Vec::new();
    let mut others = 0;
    for row in 0..count {
        if row_hidden(panels, cur, cur_layer, row) {
            continue;
        }
        let folded = row_folded_in(panels, row, auto, replis);
        if folded {
            others += 1;
            if replis.mode == Inactifs::Caches && !replis.autres_montres {
                continue;
            }
        }
        let h = if folded { FOLDED_H } else { LAYER_H };
        out.push(Lane { row, top, h, folded });
        top += h;
    }
    if replis.mode == Inactifs::Caches && others > 0 {
        out.push(Lane { row: MORE_ROW, top, h: MORE_H, folded: true });
    }
    out
}

/// Combien de calques la ligne « N autres calques » cache ou montre.
pub(crate) fn others_count(panels: &[Panel], cur: usize, cur_layer: usize, count: usize, auto: AutoOpen, replis: &Replis) -> usize {
    (0..count).filter(|&row| !row_hidden(panels, cur, cur_layer, row) && row_folded_in(panels, row, auto, replis)).count()
}

/// Icône sans cadre, qui s'éclaire sous la souris.
fn mini_icon(ui: &mut Ui, rect: Rect, id: egui::Id, icon: &str, size: f32, color: Color32) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    let ink = if resp.hovered() { theme::texte_fort() } else { color };
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, FontId::proportional(size), ink);
    resp
}

/// Où va chaque chose dans la colonne de gauche d'une piste de rectangle `r` : l'œil, la pastille, le nom.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Head {
    pub eye: Rect,
    pub dot: egui::Pos2,
    pub dot_r: f32,
    pub name: Rect,
}

pub(crate) fn head_layout(r: Rect, folded: bool) -> Head {
    let eye = Rect::from_min_size(pos2(r.left() + 2.0, r.top()), vec2(EYE_W, r.height()));
    let dot = pos2(eye.right() + DOT_W * 0.5, r.center().y);
    let name = Rect::from_min_max(pos2(eye.right() + DOT_W + 2.0, r.top()), pos2((r.right() - 2.0).max(eye.right() + DOT_W + 3.0), r.bottom()));
    Head { eye, dot, dot_r: if folded { 2.5 } else { 3.5 }, name }
}

/// Taille du nom d'une piste.
pub(crate) fn name_size(folded: bool) -> f32 {
    if folded {
        9.0
    } else {
        12.0
    }
}

impl App {
    // ------------------------------------------------------------------ l'état

    /// Le calque actif change quand le calque courant change, pas pendant un geste : `pointer_down` (la souris est tenue) le retient.
    pub(crate) fn pistes_follow_current(&mut self, pointer_down: bool) {
        if pointer_down {
            return;
        }
        let cur = self.doc.panels.get(self.cur_panel).and_then(|p| p.layers.get(self.cur_layer).map(|l| (p.id, l.id)));
        if cur == self.pistes.seen {
            return;
        }
        self.pistes.seen = cur;
        self.pistes.auto_open = cur;
        // Un nouveau calque actif : il se déplie, même si l'on avait replié l'ancien à la main.
        self.pistes.active_fermee = false;
    }

    /// Le calque actif, si le réglage « Déplier le calque courant » est allumé.
    pub(crate) fn pistes_auto_open(&self) -> AutoOpen {
        self.settings.montage_auto_open.then_some(self.pistes.auto_open).flatten()
    }

    /// La rangée du calque `li` du plan courant (celle de son trait pour un calque de couleur lié).
    pub(crate) fn track_row_of(&self, li: usize) -> usize {
        row_of_layer(self.layers(), li).unwrap_or(0)
    }

    /// Comment les rangées se replient maintenant : le réglage du poste et ce qu'on a déplié ou replié à la main.
    pub(crate) fn pistes_replis(&self) -> Replis {
        Replis {
            mode: self.settings.montage_inactifs,
            ouvertes: self.pistes.ouvertes.clone(),
            active_fermee: self.pistes.active_fermee,
            autres_montres: self.pistes.autres_montres,
        }
    }

    /// La rangée `row` est repliée à l'écran.
    pub(crate) fn track_is_folded(&self, row: usize) -> bool {
        row_folded_in(&self.doc.panels, row, self.pistes_auto_open(), &self.pistes_replis())
    }

    /// Replie ou déplie la piste de la rangée `row` (affichage seulement : rien dans le projet). Faux si rien n'existe à cette rangée.
    pub(crate) fn set_track_folded(&mut self, row: usize, folded: bool) -> bool {
        if self.doc.panels.get(self.cur_panel).is_none_or(|p| layer_in_row(&p.layers, row).is_none()) {
            return false;
        }
        let active = active_row(&self.doc.panels, self.pistes_auto_open()) == Some(row);
        if folded {
            self.pistes.ouvertes.remove(&row);
            if active {
                self.pistes.active_fermee = true;
            }
        } else {
            self.pistes.ouvertes.insert(row);
            if active {
                self.pistes.active_fermee = false;
            }
        }
        true
    }

    /// Le triangle, le double-clic ou le raccourci d'une piste : replie ou déplie cette piste (`all` : toutes les pistes).
    pub(crate) fn toggle_track_fold(&mut self, row: usize, all: bool) {
        let folded = self.track_is_folded(row);
        if all {
            self.set_all_tracks_folded(!folded);
        } else {
            self.set_track_folded(row, !folded);
        }
    }

    /// Replie toutes les pistes (même celle du calque actif, jusqu'au prochain changement de calque), ou déplie tout. Renvoie le nombre de
    /// pistes dont l'état a changé.
    pub(crate) fn set_all_tracks_folded(&mut self, folded: bool) -> usize {
        let count = self.doc.panels.iter().map(|p| p.layers.len()).max().unwrap_or(0);
        let before: Vec<bool> = (0..count).map(|r| self.track_is_folded(r)).collect();
        if folded {
            self.pistes.ouvertes.clear();
            self.pistes.active_fermee = true;
        } else {
            self.pistes.ouvertes = (0..count).collect();
            self.pistes.active_fermee = false;
        }
        let changed = before.iter().enumerate().filter(|(r, b)| **b != self.track_is_folded(*r)).count();
        self.status = format!("Toutes les pistes sont {}.", if folded { "repliées" } else { "dépliées" });
        changed
    }

    /// Les calques non actifs : en bandes fines, ou cachés (réglage du poste, enregistré avec les réglages ; rien dans le projet).
    pub(crate) fn set_inactifs(&mut self, mode: Inactifs) {
        if self.settings.montage_inactifs == mode {
            return;
        }
        self.settings.montage_inactifs = mode;
        self.pistes.autres_montres = false;
        if let Err(e) = self.settings.save() {
            self.status = format!("Réglages non enregistrés : {e}");
            return;
        }
        self.status = match mode {
            Inactifs::Replies => "Calques non actifs : en bandes fines.".into(),
            Inactifs::Caches => "Calques non actifs : cachés (« N autres calques » les montre).".into(),
        };
    }

    /// « Déplier le calque courant » : allumé, éteint.
    pub(crate) fn toggle_auto_open(&mut self) {
        self.settings.montage_auto_open = !self.settings.montage_auto_open;
        if self.settings.montage_auto_open {
            // Le calque courant se déplie tout de suite.
            self.pistes.seen = None;
        }
        if let Err(e) = self.settings.save() {
            self.status = format!("Réglages non enregistrés : {e}");
        }
        self.status = format!("Le calque courant {} sa piste tout seul.", if self.settings.montage_auto_open { "déplie" } else { "ne déplie plus" });
    }

    // ------------------------------------------------------------------ la colonne de gauche

    /// Ce qui va dans la colonne d'une piste (dépliée ou repliée) du calque `li`, dans `r` : l'œil, la pastille de la sorte du calque et le nom
    /// (ou son champ quand on le renomme). Le fond de la rangée (clics, glissés, menu) est déjà posé ; l'opacité, le verrou, la fusion sont
    /// au clic droit.
    pub(crate) fn montage_head(&mut self, ui: &mut Ui, r: Rect, li: usize, name_color: Color32, folded: bool) {
        let Some(l) = self.layers().get(li) else { return };
        let (id, name, visible, dot) = (l.id, l.name.clone(), l.visible, theme::couleur_du_calque(l));
        let at = head_layout(r, folded);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt(("tete_piste", id)));
        let ui = &mut child;
        let fg = ui.visuals().text_color();
        let off = fg.gamma_multiply(0.35);
        let size = name_size(folded);
        let resp = mini_icon(ui, at.eye, egui::Id::new(("oeil_piste", id)), if visible { ph::EYE } else { ph::EYE_SLASH }, if folded { 9.0 } else { 13.0 }, if visible { fg } else { off });
        if crate::bulles::bulle(resp, "Afficher / cacher", "Montrer ou cacher le calque (et sa couleur, si c'est un trait). Verrou, opacité, fusion : clic droit sur la piste.", "").clicked() {
            self.toggle_layer_visible(li);
        }
        ui.painter().circle_filled(at.dot, at.dot_r, if visible { dot } else { dot.gamma_multiply(0.4) });
        let renaming = self.layer_rename.as_ref().is_some_and(|n| n.id == id && n.in_montage);
        if renaming {
            let mut field = ui.new_child(egui::UiBuilder::new().max_rect(at.name.expand2(vec2(0.0, 2.0))).id_salt(("nom_calque_montage", id)));
            let done = self.layer_rename_field(&mut field, at.name.width());
            self.finish_layer_rename(done);
        } else {
            let ink = if visible { name_color } else { name_color.gamma_multiply(0.5) };
            ui.painter().with_clip_rect(at.name.expand2(vec2(0.0, 1.0))).text(pos2(at.name.left(), at.name.center().y - if folded { 0.5 } else { 0.0 }), Align2::LEFT_CENTER, &name, FontId::proportional(size), ink);
        }
    }

    /// Un double-clic sur la colonne d'une piste hors du nom la plie ou la déplie (sur le nom : le
    /// renommer, comme avant). `pos` : le point cliqué ; `name_rect` : où est le nom ; `size` : sa taille.
    pub(crate) fn double_click_is_on_name(&self, ui: &Ui, pos: egui::Pos2, name_rect: Rect, name: &str, size: f32) -> bool {
        if !name_rect.contains(pos) {
            return false;
        }
        let width = ui.fonts(|f| f.layout_no_wrap(name.to_string(), FontId::proportional(size), Color32::WHITE).size().x);
        pos.x <= name_rect.left() + width + 4.0
    }

    // ------------------------------------------------------------------ les menus

    /// « Replier / déplier les pistes » : dans le menu du calque `li` (clic droit sur sa piste).
    pub(crate) fn pistes_menu(&mut self, ui: &mut Ui, li: usize) {
        let row = self.track_row_of(li);
        let folded = self.track_is_folded(row);
        ui.menu_button("Replier / déplier les pistes", |ui| {
            if ui.button(if folded { "Déplier cette piste" } else { "Replier cette piste" }).clicked() {
                ui.close_menu();
                self.set_track_folded(row, !folded);
            }
            self.action_item(ui, "Replier toutes les pistes", "piste.replier_tout");
            self.action_item(ui, "Déplier toutes les pistes", "piste.deplier_tout");
            ui.separator();
            let mut auto = self.settings.montage_auto_open;
            if ui.checkbox(&mut auto, "Déplier le calque courant tout seul").on_hover_text("Seul le calque actif est déplié ; les autres sont en bandes fines.").clicked() {
                self.run_action("piste.auto_deplier");
            }
            ui.separator();
            ui.label(RichText::new("Calques non actifs").small().weak());
            self.inactifs_choices(ui);
        });
    }

    /// Les deux façons de montrer les calques non actifs, en deux lignes à choisir (menu du clic droit sur une piste).
    fn inactifs_choices(&mut self, ui: &mut Ui) {
        for (mode, nom, aide) in [(Inactifs::Replies, "Repliés", INACTIFS_REPLIES), (Inactifs::Caches, "Cachés", INACTIFS_CACHES)] {
            let on = self.settings.montage_inactifs == mode;
            let resp = ui.selectable_label(on, nom);
            if crate::bulles::bulle(resp, nom, aide, "").clicked() {
                ui.close_menu();
                self.set_inactifs(mode);
            }
        }
    }

    /// La section « Montage » de Fichier > Préférences… : « Calques non actifs : repliés / cachés » (deux icônes à choisir, une phrase de
    /// bulle pour chacune) et « Déplier le calque courant ».
    pub(crate) fn montage_prefs_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let label = ui.label("Calques non actifs");
            crate::bulles::bulle(label, "Calques non actifs", &format!("{INACTIFS_REPLIES} {INACTIFS_CACHES}"), "");
            let mut mode = self.settings.montage_inactifs;
            let a = crate::bulles::choix(ui, &mut mode, Inactifs::Replies, ph::ROWS, "Repliés", INACTIFS_REPLIES, "");
            let b = crate::bulles::choix(ui, &mut mode, Inactifs::Caches, ph::EYE_SLASH, "Cachés", INACTIFS_CACHES, "");
            if a.changed() || b.changed() {
                self.set_inactifs(mode);
            }
            ui.label(RichText::new(if mode == Inactifs::Replies { "repliés" } else { "cachés" }).small().weak());
        });
        let mut auto = self.settings.montage_auto_open;
        let resp = ui.checkbox(&mut auto, "Déplier le calque courant tout seul").on_hover_text("Seul le calque actif est déplié ; les autres sont en bandes fines.");
        if resp.clicked() {
            self.run_action("piste.auto_deplier");
        }
        self.hauteur_pistes_pref_ui(ui);
    }
}

/// Une phrase pour chaque façon de montrer les calques non actifs (bulles, Préférences, menu).
pub(crate) const INACTIFS_REPLIES: &str = "Repliés : les calques qui ne sont pas actifs sont des bandes fines, avec leurs dessins et leurs clés en petits traits.";
pub(crate) const INACTIFS_CACHES: &str = "Cachés : seul le calque actif est montré, avec une ligne « N autres calques » sur laquelle cliquer pour voir les autres.";
