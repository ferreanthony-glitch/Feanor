//! Pistes du montage (Anthony, 2026-10-20 : « un système de dépliage / repliage des calques dans la timeline pour gagner de la place
//! tout en gardant cela lisible » ; 2026-10-28, branche `timeline-repliee` : « seul le calque actif est déplié » ; 2026-11-11, branche
//! `calques-timeline` : « trois calques maximum affichés sur la timeline », les dossiers « jamais traités comme des calques »).
//!
//! Dans le Storyboard et le Montage (l'onglet Animation garde sa timeline, tous calques dépliés), toute la pile est montrée : le calque
//! actif déplié (`LAYER_H`, 44 depuis la partie 160 : deux rangées d'options), tous les autres en bandes (`FOLDED_H`, 17). (Le mode
//! « Trois calques », sa fenêtre et ses fines bandes « N au-dessus / au-dessous » sont retirés à la demande d'Anthony, partie 165.7 :
//! « supprime l'option et le bouton 3 calques ».)
//!
//! **Un dossier n'est jamais un calque** : c'est une ligne fine (`DOSSIER_H`, 13), plus petite que toute piste, qu'un clic ferme ou
//! ouvre. Elle n'est jamais le calque actif. **Un dossier fermé se traverse** : « calque du
//! dessus / du dessous » entre dedans, il se montre ouvert tant que le calque courant est l'un des siens (`dossier_de_passage`) et se
//! referme quand on en sort ; rien n'est écrit pour cela.
//!
//! Dans la colonne de gauche, une piste porte l'œil, une pastille de la couleur de la sorte du calque (vectoriel, bitmap…), le nom et le
//! cadenas d'un calque verrouillé ; un **filet de la couleur de son dossier** court à sa gauche (`folders::couleur_du_rangement` : la
//! bande dit le type, le filet dit le rangement). La piste du calque actif porte en plus toutes ses options sur deux rangées (`piste_active_head`, partie 160 :
//! le panneau Calques n'existe plus) ; tout est aussi au clic droit sur la piste (`montage_layer_menu`). Cliquer un calque (son nom ou l'un de ses blocs) le rend actif, et le panneau Calques suit.
//!
//! **Rien n'est écrit dans le projet** pour la fenêtre : le calque actif et les pistes dépliées à la main sont de l'affichage (par
//! poste, par séance). Le drapeau `Layer::collapsed` reste celui des « lignes réduites » de la timeline de l'animation et des dossiers
//! fermés (un clic sur la ligne d'un dossier le change, comme dans le panneau Calques).
//!
//! Un calque de couleur lié à un calque de trait (l'aplat sous le trait) n'a pas de piste à lui : `layer_in_row` l'ignore, un liseré rose
//! sous le bloc de son trait dit qu'il existe (`montagerows.rs`).
//!
//! Une piste (rangée r) traverse tous les plans : chaque plan a ses calques dans son ordre (6f) ; c'est le plan courant qui décide de
//! ce qu'une rangée est (un calque, la ligne d'un dossier). Le calque actif change quand la souris est relâchée (jamais pendant un
//! geste : sinon les pistes glisseraient sous le pointeur).

use std::collections::BTreeSet;

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Response, RichText, Sense, Stroke, Ui};
use egui_phosphor::regular as ph;
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::document::{Layer, Panel};
use crate::montagerows::{layer_in_row, row_of_layer, LAYER_H};
use crate::theme;

/// Hauteur d'une piste repliée : une bande de 15 points, le nom en 11.
pub(crate) const FOLDED_H: f32 = 17.0;
/// Hauteur de la ligne d'un dossier : jamais un calque, toujours plus petite qu'une piste.
pub(crate) const DOSSIER_H: f32 = 13.0;
/// Place de l'œil à gauche d'une piste.
const EYE_W: f32 = 15.0;
/// Place de la pastille de couleur, après l'œil.
const DOT_W: f32 = 10.0;
/// Retrait d'un calque qui a un filet de couleur à sa gauche (celui de son dossier).
pub(crate) const RETRAIT: f32 = 6.0;
/// Place du cadenas d'un calque verrouillé, à droite du nom.
const VERROU_W: f32 = 13.0;
/// Une rangée de la piste du calque actif (partie 160 : deux rangées, `LAYER_H` = 2 × 22).
pub(crate) const RANGEE_ACTIVE: f32 = 22.0;
/// La colonne doit avoir au moins cette largeur pour que la piste du calque actif montre aussi la vignette de son dessin (à la
/// largeur d'origine, 104 points, les réglages prennent toute la rangée du bas).
pub(crate) const LARGEUR_VIGNETTE: f32 = 150.0;
/// La liste du mode de fusion de la piste active (partie 173) : sa largeur au plus (« Superposition ▾ » en entier) et au moins
/// (la flèche et deux lettres), et ce qu'elle laisse au moins à la barre d'opacité (« 100 % » lisible).
const LISTE_MAX: f32 = 96.0;
const LISTE_MIN: f32 = 26.0;
const BARRE_MIN: f32 = 34.0;

/// Ce qu'une ligne des pistes est.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Sorte {
    /// La piste d'un calque (dépliée ou repliée).
    #[default]
    Calque,
    /// La ligne fine d'un dossier.
    Dossier,
}

/// Une ligne visible des pistes : la rangée de calques (0 : en haut de la pile), son haut (depuis le haut des pistes de calques) et sa
/// hauteur.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lane {
    pub row: usize,
    pub top: f32,
    pub h: f32,
    pub folded: bool,
    pub sorte: Sorte,
}

impl Lane {
    /// La piste d'un calque.
    pub(crate) fn calque(row: usize, top: f32, h: f32, folded: bool) -> Lane {
        Lane { row, top, h, folded, sorte: Sorte::Calque }
    }

    /// La ligne fine d'un dossier.
    pub(crate) fn dossier(row: usize, top: f32) -> Lane {
        Lane { row, top, h: DOSSIER_H, folded: true, sorte: Sorte::Dossier }
    }

    /// La ligne fine d'un dossier (pas une piste de calque).
    pub fn est_dossier(&self) -> bool {
        self.sorte == Sorte::Dossier
    }

    /// Les losanges d'effets (violets) se prennent jusqu'à cette hauteur depuis le haut de la piste ;
    /// sous elle, ceux de position, puis le dessin. Une bande n'en a pas : leurs clés y sont de petits traits, qu'on ne prend pas
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

    /// Depuis le haut de la piste, la hauteur où commence la barre qui décale le calque (aucune dans une bande : un clic y choisit le
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
    Lane::calque(0, 0.0, LAYER_H, false)
}

/// Le calque `(plan, calque)` — ids — qui est actif : celui qui était courant au dernier relâchement de la souris.
pub(crate) type AutoOpen = Option<(u64, u64)>;

/// La hauteur de la rangée des plans dans le Storyboard et le Montage (réglage du poste, `reglages.toml` : `hauteur_plans`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HauteurPlans {
    /// Une vignette, le nom et la durée dessous (d'office).
    #[default]
    Moyenne,
    /// Un bandeau : une petite vignette, le nom et la durée sur une ligne.
    Bandeau,
}

impl HauteurPlans {
    pub fn est_moyenne(&self) -> bool {
        *self == HauteurPlans::Moyenne
    }

    /// La hauteur de la rangée, en points.
    pub fn points(self) -> f32 {
        match self {
            HauteurPlans::Moyenne => 56.0,
            HauteurPlans::Bandeau => 28.0,
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            HauteurPlans::Moyenne => "Moyenne",
            HauteurPlans::Bandeau => "Bandeau",
        }
    }
}

/// Ce que le montage retient pour dire quelles pistes sont dépliées : le réglage et ce que l'utilisateur a fait à la main.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Replis {
    /// Les rangées dépliées à la main (en plus du calque actif).
    pub ouvertes: BTreeSet<usize>,
    /// La rangée du calque actif est repliée à la main, jusqu'au prochain changement de calque.
    pub active_fermee: bool,
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
}

/// Le dossier de rang `fi` est réduit, et le calque courant `cur_layer` est l'un des siens : il se montre ouvert le temps du passage.
pub(crate) fn dossier_de_passage(layers: &[Layer], fi: usize, cur_layer: usize) -> bool {
    layers.get(fi).is_some_and(|f| f.folder && f.collapsed && layers.get(cur_layer).is_some_and(|c| c.parent == Some(f.id)))
}

/// Le dossier de rang `fi` est fermé à l'écran : réduit, sans le calque courant dedans (un dossier fermé s'ouvre quand on y entre et
/// se referme quand on en sort).
pub(crate) fn dossier_ferme(layers: &[Layer], fi: usize, cur_layer: usize) -> bool {
    layers.get(fi).is_some_and(|f| f.folder && f.collapsed) && !dossier_de_passage(layers, fi, cur_layer)
}

/// La rangée `row` est cachée : son calque, dans le plan courant `cur`, est rangé dans un dossier fermé à l'écran.
pub(crate) fn row_hidden(panels: &[Panel], cur: usize, cur_layer: usize, row: usize) -> bool {
    let Some(layers) = panels.get(cur).map(|p| &p.layers) else { return false };
    let Some(k) = layer_in_row(layers, row) else { return false };
    crate::folders::parent_index(layers, k).is_some_and(|fi| dossier_ferme(layers, fi, cur_layer))
}

/// La rangée du calque actif `auto` (celle de son plan ; un calque de couleur lié est à la rangée de son trait).
pub(crate) fn active_row(panels: &[Panel], auto: AutoOpen) -> Option<usize> {
    let (panel, layer) = auto?;
    let p = panels.iter().find(|p| p.id == panel)?;
    let k = p.layers.iter().position(|l| l.id == layer)?;
    row_of_layer(&p.layers, k)
}

/// La rangée `row` est repliée — elle ne l'est pas si c'est celle du calque actif (sauf repliée à la main) ou si
/// on l'a dépliée à la main.
pub(crate) fn row_folded_in(panels: &[Panel], row: usize, auto: AutoOpen, replis: &Replis) -> bool {
    let active = active_row(panels, auto) == Some(row) && !replis.active_fermee;
    !active && !replis.ouvertes.contains(&row)
}

/// Ce que la rangée `row` est dans la pile `layers` (celle du plan courant) : un calque, la ligne d'un dossier, ou rien (ce plan a
/// moins de calques que d'autres).
pub(crate) fn sorte_de_rangee(layers: &[Layer], row: usize) -> Option<Sorte> {
    layer_in_row(layers, row).map(|k| if layers[k].folder { Sorte::Dossier } else { Sorte::Calque })
}

/// Les lignes visibles de haut en bas, pour `count` rangées au plus : toute la pile (les pistes d'un dossier fermé en moins).
pub(crate) fn lanes_in(panels: &[Panel], cur: usize, cur_layer: usize, count: usize, auto: AutoOpen, replis: &Replis) -> Vec<Lane> {
    let vide: &[Layer] = &[];
    let layers = panels.get(cur).map_or(vide, |p| p.layers.as_slice());
    let mut top = 0.0;
    let mut out = Vec::new();
    for row in 0..count {
        if row_hidden(panels, cur, cur_layer, row) {
            continue;
        }
        let lane = if sorte_de_rangee(layers, row) == Some(Sorte::Dossier) {
            Lane::dossier(row, top)
        } else {
            let folded = row_folded_in(panels, row, auto, replis);
            Lane::calque(row, top, if folded { FOLDED_H } else { LAYER_H }, folded)
        };
        top += lane.h;
        out.push(lane);
    }
    out
}

/// Quand la bande a plus de place que ses pistes n'en prennent, les calques les plus proches du calque actif prennent aussi la grande
/// hauteur (`LAYER_H`, qui reste la plus grande) : celui du dessus, celui du dessous, puis les suivants, tant qu'il reste de la place
/// (Anthony, 2026-11-11 : « quand il y a plus de place pour afficher les calques, les calques les plus proches du calque sélectionné
/// prennent aussi la taille la plus grande », « il faut que ce soit toujours le plus confortable possible »). `place` : la hauteur
/// offerte aux pistes de calques. Les lignes de dossier gardent leur hauteur. Sans calque actif déplié (replié à la main), rien ne grandit. Les hauts sont recalculés.
pub(crate) fn agrandir(lanes: &mut [Lane], place: f32) {
    let calques: Vec<usize> = (0..lanes.len()).filter(|&i| lanes[i].sorte == Sorte::Calque).collect();
    let Some(centre) = calques.iter().position(|&i| !lanes[i].folded) else { return };
    let mut reste = place - lanes.iter().map(|l| l.h).sum::<f32>();
    // Du plus proche au plus loin, le dessus avant le dessous ; le premier qui ne tient pas arrête tout (les grands calques restent
    // d'un seul tenant autour du calque actif).
    let proches = (1..calques.len()).flat_map(|d| [centre.checked_sub(d), Some(centre + d).filter(|&k| k < calques.len())]).flatten();
    for k in proches {
        let l = &mut lanes[calques[k]];
        if !l.folded {
            continue;
        }
        let gain = LAYER_H - l.h;
        if gain > reste {
            break;
        }
        reste -= gain;
        l.h = LAYER_H;
        l.folded = false;
    }
    let mut top = 0.0;
    for l in lanes.iter_mut() {
        l.top = top;
        top += l.h;
    }
}

/// Icône sans cadre, qui s'éclaire sous la souris.
fn mini_icon(ui: &mut Ui, rect: Rect, id: egui::Id, icon: &str, size: f32, color: Color32) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    let ink = if resp.hovered() { theme::texte_fort() } else { color };
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, FontId::proportional(size), ink);
    resp
}

/// Un choix parmi plusieurs, en icône avec sa bulle : petite dans la barre du bas de la bande (`petit`), sinon comme les autres choix
/// des Préférences (`bulles::choix`). Vrai : la valeur vient de changer.
#[allow(clippy::too_many_arguments)]
fn choix_en_icone<T: PartialEq + Copy>(ui: &mut Ui, petit: bool, valeur: &mut T, cette: T, symbole: &str, nom: &str, description: &str, raccourci: &str) -> bool {
    if !petit {
        return crate::bulles::choix(ui, valeur, cette, symbole, nom, description, raccourci).changed();
    }
    let on = *valeur == cette;
    let bouton = egui::Button::new(crate::bulles::encre_etat(RichText::new(symbole).size(11.0), on)).selected(on).min_size(vec2(18.0, 14.0));
    let resp = crate::bulles::bulle(ui.add(bouton), nom, description, raccourci);
    if resp.clicked() && !on {
        *valeur = cette;
        return true;
    }
    false
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
    head_layout_en(r, folded, 0.0)
}

/// `head_layout`, décalé de `retrait` points vers la droite (la place du filet de la couleur du dossier).
pub(crate) fn head_layout_en(r: Rect, folded: bool, retrait: f32) -> Head {
    let eye = Rect::from_min_size(pos2(r.left() + 2.0 + retrait, r.top()), vec2(EYE_W, r.height()));
    let dot = pos2(eye.right() + DOT_W * 0.5, r.center().y);
    let name = Rect::from_min_max(pos2(eye.right() + DOT_W + 2.0, r.top()), pos2((r.right() - 2.0).max(eye.right() + DOT_W + 3.0), r.bottom()));
    Head { eye, dot, dot_r: if folded { 3.0 } else { 3.5 }, name }
}

/// Taille du nom d'une piste.
pub(crate) fn name_size(folded: bool) -> f32 {
    if folded {
        11.0
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

    /// Comment les rangées se montrent maintenant : le réglage du poste et ce qu'on a déplié ou replié à la main.
    pub(crate) fn pistes_replis(&self) -> Replis {
        Replis { ouvertes: self.pistes.ouvertes.clone(), active_fermee: self.pistes.active_fermee }
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

    /// Le double-clic ou le raccourci d'une piste : replie ou déplie cette piste (`all` : toutes les pistes).
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

    /// La hauteur de la rangée des plans : moyenne ou en bandeau (réglage du poste).
    pub(crate) fn set_hauteur_plans(&mut self, h: HauteurPlans) {
        if self.settings.montage_plans == h {
            return;
        }
        self.settings.montage_plans = h;
        if let Err(e) = self.settings.save() {
            self.status = format!("Réglages non enregistrés : {e}");
            return;
        }
        self.status = match h {
            HauteurPlans::Moyenne => "Plans : hauteur moyenne.".into(),
            HauteurPlans::Bandeau => "Plans : en bandeau.".into(),
        };
    }

    /// « Plans en bandeau » : allumé, éteint (`bande.plans_en_bandeau`).
    pub(crate) fn basculer_hauteur_plans(&mut self) {
        self.set_hauteur_plans(if self.settings.montage_plans == HauteurPlans::Moyenne { HauteurPlans::Bandeau } else { HauteurPlans::Moyenne });
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

    // ------------------------------------------------------------------ aller d'un calque à l'autre

    /// Le calque d'après le calque courant dans le sens `pas` (+1 : dessus, −1 : dessous). Jamais un dossier : on va de dessin en dessin,
    /// et l'on entre dans un dossier fermé (il se montre ouvert tant qu'on y est, `dossier_de_passage`).
    pub(crate) fn calque_voisin(&self, pas: isize) -> Option<usize> {
        let layers = self.layers();
        let mut k = self.cur_layer as isize + pas;
        while k >= 0 && (k as usize) < layers.len() {
            if !layers[k as usize].folder {
                return Some(k as usize);
            }
            k += pas;
        }
        None
    }

    /// « Calque du dessus » (`pas` = +1) ou « du dessous » (−1), `fois` fois de suite.
    pub(crate) fn aller_au_calque(&mut self, pas: isize, fois: usize) {
        for _ in 0..fois {
            let Some(k) = self.calque_voisin(pas.signum()) else { break };
            self.select_layer(k);
        }
    }

    // ------------------------------------------------------------------ la colonne de gauche

    /// Ce qui va dans la colonne d'une piste (dépliée ou repliée) du calque `li`, dans `r` : le filet de la couleur de son dossier, l'œil,
    /// la pastille de la sorte du calque, le nom (ou son champ quand on le renomme ; sous le nom de son dossier dans une piste dépliée) et
    /// le cadenas d'un calque verrouillé. Le fond de la rangée (clics, glissés, menu) est déjà posé ; l'opacité, la fusion sont au clic
    /// droit.
    pub(crate) fn montage_head(&mut self, ui: &mut Ui, r: Rect, li: usize, name_color: Color32, folded: bool) {
        let layers = self.layers();
        let Some(l) = layers.get(li) else { return };
        let (id, name, visible, dot) = (l.id, l.name.clone(), l.visible, theme::couleur_du_calque(l));
        let filet = crate::folders::couleur_du_rangement(layers, li, &self.doc.groups);
        let dossier = crate::folders::parent_index(layers, li).map(|fi| layers[fi].name.clone());
        let verrou = l.locked || crate::folders::locked_by_folder(layers, li);
        let mut at = head_layout_en(r, folded, if filet.is_some() { RETRAIT } else { 0.0 });
        if verrou {
            at.name.max.x = (at.name.max.x - VERROU_W).max(at.name.min.x + 1.0);
        }
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt(("tete_piste", id)));
        let ui = &mut child;
        let fg = ui.visuals().text_color();
        let off = fg.gamma_multiply(0.35);
        let size = name_size(folded);
        if let Some(c) = filet {
            ui.painter().rect_filled(Rect::from_min_max(pos2(r.left() + 2.0, r.top() + 1.0), pos2(r.left() + 5.0, r.bottom() - 1.0)), 1.0, c);
        }
        let resp = mini_icon(ui, at.eye, egui::Id::new(("oeil_piste", id)), if visible { ph::EYE } else { ph::EYE_SLASH }, if folded { 11.0 } else { 13.0 }, if visible { fg } else { off });
        if crate::bulles::bulle(resp, "Afficher / cacher", "Montrer ou cacher le calque (et sa couleur, si c'est un trait). Verrou, opacité, fusion : clic droit sur la piste.", "").clicked() {
            self.toggle_layer_visible(li);
        }
        ui.painter().circle_filled(at.dot, at.dot_r, if visible { dot } else { dot.gamma_multiply(0.4) });
        if verrou {
            let ou = pos2(r.right() - VERROU_W * 0.5 - 1.0, r.center().y);
            ui.painter().text(ou, Align2::CENTER_CENTER, ph::LOCK_SIMPLE, FontId::proportional(if folded { 10.0 } else { 11.5 }), fg.gamma_multiply(0.75));
        }
        let renaming = self.layer_rename.as_ref().is_some_and(|n| n.id == id && n.in_montage);
        if renaming {
            let mut field = ui.new_child(egui::UiBuilder::new().max_rect(at.name.expand2(vec2(0.0, 2.0))).id_salt(("nom_calque_montage", id)));
            let done = self.layer_rename_field(&mut field, at.name.width());
            self.finish_layer_rename(done);
        } else {
            let ink = if visible { name_color } else { name_color.gamma_multiply(0.5) };
            let clip = ui.painter().with_clip_rect(at.name.expand2(vec2(0.0, 1.0)));
            // Une piste dépliée d'un calque rangé : le nom de son dossier au-dessus du sien, pour savoir où l'on est quand la ligne du
            // dossier n'est pas à l'écran.
            let y = match dossier.filter(|_| !folded && r.height() >= 30.0) {
                Some(nom) => {
                    clip.text(pos2(at.name.left(), at.name.center().y - 8.0), Align2::LEFT_CENTER, nom, FontId::proportional(9.5), filet.unwrap_or(off));
                    at.name.center().y + 5.0
                }
                None => at.name.center().y,
            };
            clip.text(pos2(at.name.left(), y), Align2::LEFT_CENTER, &name, FontId::proportional(size), ink);
        }
    }

    /// La piste du calque actif `li`, dans `r` (deux rangées de 22 points, partie 160 : elle porte toutes les options qui étaient au
    /// panneau Calques et au clic droit). En haut : le filet du dossier, l'œil, la pastille de la sorte, le nom (et celui du dossier),
    /// le cadenas, le ⋮ du menu du calque. En bas : la vignette du dessin (ou la couleur d'un calque Couleur unie) quand la colonne est
    /// assez large, les effets, l'accroche à la caméra, la liste du mode de fusion (« Normal ▾ », partie 173), le losange des clés
    /// d'opacité et l'opacité. La pelure d'oignon du calque est au clic droit (Apparence). Le fond de la rangée (clic, glissé, clic
    /// droit) est déjà posé.
    pub(crate) fn piste_active_head(&mut self, ui: &mut Ui, r: Rect, li: usize, name_color: Color32) {
        let layers = self.layers();
        let Some(l) = layers.get(li) else { return };
        let (id, name, visible, dot, locked, accroche, blend, cel, unie) = (l.id, l.name.clone(), l.visible, theme::couleur_du_calque(l), l.locked, l.accroche, l.blend, l.cel_at(self.cur_frame), l.unie.is_some());
        let opacity = l.opacity_at(self.cur_frame);
        let accrochable = !l.folder && l.effet.is_none() && !l.keys_follow_line();
        let filet = crate::folders::couleur_du_rangement(layers, li, &self.doc.groups);
        let dossier = crate::folders::parent_index(layers, li).map(|fi| layers[fi].name.clone());
        let retrait = if filet.is_some() { RETRAIT } else { 0.0 };
        let haut = Rect::from_min_max(r.min, pos2(r.right(), r.top() + RANGEE_ACTIVE));
        let bas = Rect::from_min_max(pos2(r.left(), r.bottom() - RANGEE_ACTIVE), r.max);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt(("tete_piste_active", id)));
        // (Rien ne sort de la piste, même dans la colonne étroite.)
        child.set_clip_rect(r.intersect(ui.clip_rect()));
        let ui = &mut child;
        let large = r.width() >= LARGEUR_VIGNETTE;
        let fg = ui.visuals().text_color();
        let off = fg.gamma_multiply(0.35);
        let shade = |on: bool| if on { fg } else { off };
        if let Some(c) = filet {
            ui.painter().rect_filled(Rect::from_min_max(pos2(r.left() + 2.0, r.top() + 1.0), pos2(r.left() + 5.0, r.bottom() - 1.0)), 1.0, c);
        }
        // ---- En haut.
        let at = head_layout_en(haut, false, retrait);
        let resp = mini_icon(ui, at.eye, egui::Id::new(("oeil_piste", id)), if visible { ph::EYE } else { ph::EYE_SLASH }, 13.0, if visible { fg } else { off });
        if crate::bulles::bulle(resp, "Afficher / cacher", "Montrer ou cacher le calque (et sa couleur, si c'est un trait).", "").clicked() {
            self.toggle_layer_visible(li);
        }
        // (Colonne étroite : la couleur d'un calque Couleur unie prend la place de la pastille de sa sorte.)
        if unie && !large {
            let v = Rect::from_center_size(at.dot, vec2(DOT_W, 12.0));
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(v).id_salt(("unie_piste", id)));
            c.spacing_mut().interact_size = v.size();
            self.couleur_unie_champ(&mut c, li);
        } else {
            ui.painter().circle_filled(at.dot, at.dot_r, if visible { dot } else { dot.gamma_multiply(0.4) });
        }
        // La sorte du calque (vectoriel, bitmap, couleur…) dans la bulle de sa pastille (partie 160 : l'icône de la sorte était sur
        // sa ligne du panneau Calques).
        let sorte = crate::calquemenu::Sorte::of(&self.layers()[li]);
        let resp = ui.interact(Rect::from_center_size(at.dot, vec2(DOT_W, haut.height())), egui::Id::new(("sorte_piste", id)), Sense::hover());
        crate::bulles::bulle(resp, &format!("{}  {}", sorte.icone_ligne(), sorte.nom()), sorte.phrase(), "");
        let menu = Rect::from_min_size(pos2(haut.right() - 15.0, haut.top() + 2.0), vec2(14.0, haut.height() - 4.0));
        let verrou = Rect::from_min_size(pos2(menu.left() - 15.0, haut.top() + 2.0), vec2(14.0, haut.height() - 4.0));
        self.lock_button(ui, verrou, li, locked, shade(locked));
        let mut m = ui.new_child(egui::UiBuilder::new().max_rect(menu).id_salt(("options_piste", id)));
        let button = egui::Button::new(RichText::new(ph::DOTS_THREE_VERTICAL).size(13.0).color(fg)).frame(false);
        let resp = egui::menu::menu_custom_button(&mut m, button, |ui| self.montage_layer_menu(ui, li)).response;
        crate::bulles::bulle(resp, "Commandes du calque", "Renommer, dupliquer, organiser, apparence, effets, aplat… (aussi : clic droit sur la piste).", "");
        let nom = Rect::from_min_max(at.name.min, pos2((verrou.left() - 2.0).max(at.name.min.x + 1.0), haut.bottom()));
        let renaming = self.layer_rename.as_ref().is_some_and(|n| n.id == id && n.in_montage);
        if renaming {
            let mut field = ui.new_child(egui::UiBuilder::new().max_rect(nom.expand2(vec2(0.0, 2.0))).id_salt(("nom_calque_montage", id)));
            let done = self.layer_rename_field(&mut field, nom.width());
            self.finish_layer_rename(done);
        } else {
            let ink = if visible { name_color } else { name_color.gamma_multiply(0.5) };
            let clip = ui.painter().with_clip_rect(nom.expand2(vec2(0.0, 1.0)));
            let mut x = nom.left();
            // Rangé dans un dossier : son nom d'abord, en petit, pour savoir où l'on est.
            if let Some(d) = dossier {
                let g = clip.text(pos2(x, nom.center().y), Align2::LEFT_CENTER, format!("{d} ›"), FontId::proportional(9.5), filet.unwrap_or(off));
                x = g.right() + 3.0;
            }
            clip.text(pos2(x, nom.center().y), Align2::LEFT_CENTER, &name, FontId::proportional(12.0), ink);
        }
        // ---- En bas.
        let y = bas.center().y;
        let mut x = bas.left() + 2.0 + retrait;
        // (Des cases de 13 points : à la largeur d'origine de la colonne, il reste une vingtaine de points à la barre d'opacité.)
        const PAS: f32 = 13.0;
        let case = |x: f32| Rect::from_center_size(pos2(x + PAS * 0.5, y), vec2(PAS, bas.height() - 4.0));
        // La vignette (ou la couleur unie) : seulement si la colonne est assez large pour tout le reste.
        if large {
            let v = Rect::from_center_size(pos2(x + 13.0, y), vec2(26.0, bas.height() - 6.0));
            if unie {
                let mut c = ui.new_child(egui::UiBuilder::new().max_rect(v).id_salt(("unie_piste", id)));
                c.spacing_mut().interact_size = v.size();
                self.couleur_unie_champ(&mut c, li);
            } else {
                match cel.and_then(|c| self.layer_thumb(ui.ctx(), c)) {
                    Some(t) => {
                        ui.painter().image(t.id(), v, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), if visible { Color32::WHITE } else { Color32::from_gray(120) });
                    }
                    None => {
                        ui.painter().rect_filled(v, 0.0, theme::controle());
                    }
                }
                ui.painter().rect_stroke(v, 0.0, Stroke::new(1.0_f32, theme::bord()), egui::StrokeKind::Inside);
            }
            x = v.right() + 3.0;
        }
        // (La pelure d'oignon du calque n'est plus ici, partie 173 : elle est au clic droit de la piste et au ⋮, Apparence ▸
        // « Pelure d'oignon du calque » (Ctrl+L), avec le panneau de la pelure juste dessous ; sa place va à la liste du mode.)
        if let Some(resp) = self.fx_switch(ui, li, Some(case(x)), fg) {
            if resp.clicked() {
                self.toggle_layer_fx(li);
            }
            x += PAS;
        }
        if accrochable {
            let resp = crate::layerheads::icon_at(ui, case(x), ph::VIDEO_CAMERA, shade(accroche));
            if self.aide(resp, crate::accroche::NOM, crate::accroche::PHRASE, crate::accroche::ACTION).clicked() {
                self.basculer_accroche(li);
            }
            x += PAS;
        }
        // Le mode de fusion (partie 173) : une liste qui écrit le nom du mode, « Normal ▾ », comme dans la timeline. Elle prend la
        // place qui reste, moins le losange et une barre d'opacité lisible ; un nom trop long y est raccourci.
        let losange = if li == self.cur_layer { PAS } else { 0.0 };
        let reste = (bas.right() - 2.0 - x - losange - 1.0).max(0.0);
        let largeur = (reste - BARRE_MIN).clamp(LISTE_MIN, LISTE_MAX).min(reste);
        let liste = Rect::from_min_max(pos2(x, y - 9.0), pos2(x + largeur, y + 9.0));
        if let Some(m) = crate::layerheads::liste_des_modes(ui, liste, egui::Id::new(("fusion_piste", id)), blend) {
            // (Plusieurs calques choisis : tous prennent ce mode, une étape d'annulation, partie 136.)
            self.regler_mode_calques(li, m);
        }
        x += largeur;
        // (Le losange vise le calque courant : sur la piste du trait d'un calque de couleur courant, il n'y est pas.)
        if li == self.cur_layer {
            self.losange_opacite(ui, case(x), fg, off);
            x += PAS;
        }
        let barre = Rect::from_min_max(pos2(x + 1.0, y - 6.5), pos2((bas.right() - 2.0).max(x + 8.0), y + 6.5));
        let painter = ui.painter().clone();
        self.opacity_bar(ui, &painter, barre, r, li, egui::Id::new(("opacite_piste", id)), opacity, true, "Opacité du calque", "Cliquer ou tirer dans la barre.");
    }

    /// La ligne fine du dossier `li`, dans `r` : son triangle (couché : fermé ; droit : ouvert ; de biais : fermé, ouvert le temps du
    /// passage), le carré de sa couleur, son nom, son cadenas et le nombre de ses calques. `survol` : la souris est dessus.
    pub(crate) fn dossier_head(&self, ui: &mut Ui, r: Rect, li: usize, survol: bool) {
        let layers = self.layers();
        let Some(l) = layers.get(li) else { return };
        let ferme = dossier_ferme(layers, li, self.cur_layer);
        let passage = dossier_de_passage(layers, li, self.cur_layer);
        let n = crate::folders::children_of(layers, li).len();
        let couleur = crate::folders::couleur_du_rangement(layers, li, &self.doc.groups).unwrap_or_else(theme::texte_doux);
        let courant = li == self.cur_layer;
        let faible = ui.visuals().weak_text_color();
        let ink = if survol || courant { theme::texte_fort() } else { faible };
        let p = ui.painter().with_clip_rect(r);
        let (c, t) = (pos2(r.left() + 7.0, r.center().y), 3.5);
        let triangle = if ferme {
            vec![pos2(c.x - t * 0.6, c.y - t), pos2(c.x + t * 0.9, c.y), pos2(c.x - t * 0.6, c.y + t)]
        } else if passage {
            vec![pos2(c.x + t, c.y - t), pos2(c.x + t, c.y + t), pos2(c.x - t, c.y + t)]
        } else {
            vec![pos2(c.x - t, c.y - t * 0.6), pos2(c.x + t, c.y - t * 0.6), pos2(c.x, c.y + t * 0.9)]
        };
        p.add(egui::Shape::convex_polygon(triangle, ink, Stroke::NONE));
        let carre = Rect::from_center_size(pos2(r.left() + 18.5, r.center().y), vec2(7.0, 7.0));
        p.rect_filled(carre, 1.5, if l.visible { couleur } else { couleur.gamma_multiply(0.4) });
        let compte = p.layout_no_wrap(n.to_string(), FontId::monospace(9.5), faible);
        let mut droite = r.right() - 4.0 - compte.size().x;
        p.galley(pos2(droite, r.center().y - compte.size().y * 0.5), compte, faible);
        if l.locked {
            droite -= VERROU_W;
            p.text(pos2(droite + VERROU_W * 0.5, r.center().y), Align2::CENTER_CENTER, ph::LOCK_SIMPLE, FontId::proportional(9.5), faible);
        }
        let nom = if passage { format!("{} · de passage", l.name) } else { l.name.clone() };
        let zone = Rect::from_min_max(pos2(r.left() + 25.0, r.top()), pos2((droite - 3.0).max(r.left() + 26.0), r.bottom()));
        p.with_clip_rect(zone).text(pos2(zone.left(), r.center().y - 0.5), Align2::LEFT_CENTER, nom, FontId::proportional(10.0), if l.visible { ink } else { ink.gamma_multiply(0.5) });
    }

    /// Ferme ou ouvre le dossier `li` du plan courant (un clic sur sa ligne) : le même drapeau que dans le panneau Calques.
    pub(crate) fn basculer_dossier(&mut self, li: usize) {
        let Some(l) = self.layers().get(li).filter(|l| l.folder) else { return };
        let (ouvre, nom) = (l.collapsed, l.name.clone());
        let dedans = self.layers().get(self.cur_layer).is_some_and(|c| c.parent == Some(self.layers()[li].id));
        self.layers_mut()[li].collapsed = !ouvre;
        self.modified = true;
        self.status = match (ouvre, dedans) {
            (true, _) => format!("« {nom} » est ouvert."),
            (false, true) => format!("« {nom} » est fermé : il se refermera quand tu en sortiras."),
            (false, false) => format!("« {nom} » est fermé."),
        };
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

    // ------------------------------------------------------------------ les menus et les réglages

    /// « Calques dans la timeline » : dans le menu du calque `li` (clic droit sur sa piste).
    pub(crate) fn pistes_menu(&mut self, ui: &mut Ui, li: usize) {
        let row = self.track_row_of(li);
        let folded = self.track_is_folded(row);
        ui.menu_button("Calques dans la timeline", |ui| {
            if ui.button(if folded { "Déplier cette piste" } else { "Replier cette piste" }).clicked() {
                ui.close_menu();
                self.set_track_folded(row, !folded);
            }
            self.action_item(ui, "Replier toutes les pistes", "piste.replier_tout");
            self.action_item(ui, "Déplier toutes les pistes", "piste.deplier_tout");
            let mut auto = self.settings.montage_auto_open;
            if ui.checkbox(&mut auto, "Déplier le calque courant tout seul").on_hover_text("Seul le calque actif est déplié ; les autres sont en bandes.").clicked() {
                self.run_action("piste.auto_deplier");
            }
        });
    }

    /// Les deux hauteurs de la rangée des plans, en deux icônes à choisir (barre du bas de la bande, Préférences).
    pub(crate) fn plans_choix_ui(&mut self, ui: &mut Ui, petit: bool) {
        let touche = self.raccourci("bande.plans_en_bandeau");
        let mut h = self.settings.montage_plans;
        let a = choix_en_icone(ui, petit, &mut h, HauteurPlans::Moyenne, ph::IMAGE, "Plans de hauteur moyenne", PLANS_MOYENS, &touche);
        let b = choix_en_icone(ui, petit, &mut h, HauteurPlans::Bandeau, ph::MINUS, "Plans en bandeau", PLANS_BANDEAU, &touche);
        if a || b {
            self.set_hauteur_plans(h);
        }
    }

    /// La section « Montage » de Fichier > Préférences… : « Hauteur des plans : moyenne / bandeau » (deux icônes à choisir, une phrase
    /// de bulle pour chacune) et « Déplier le calque courant ». (« Calques dans la timeline : trois calques / toute la pile » est retiré,
    /// partie 165.7 : toute la pile est toujours montrée.)
    pub(crate) fn montage_prefs_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let label = ui.label("Hauteur des plans");
            crate::bulles::bulle(label, "Hauteur des plans", &format!("{PLANS_MOYENS} {PLANS_BANDEAU}"), "");
            self.plans_choix_ui(ui, false);
            ui.label(RichText::new(self.settings.montage_plans.nom().to_lowercase()).small().weak());
        });
        let mut auto = self.settings.montage_auto_open;
        let resp = ui.checkbox(&mut auto, "Déplier le calque courant tout seul").on_hover_text("Seul le calque actif est déplié ; les autres sont en bandes. Éteint, tous les calques restent en bandes tant qu'on ne les déplie pas.");
        if resp.clicked() {
            self.run_action("piste.auto_deplier");
        }
        // Les calques proches du calque actif prennent la grande hauteur quand la bande a de la place (`agrandir`).
        let mut grands = self.settings.montage_grands;
        let resp = ui
            .checkbox(&mut grands, "Agrandir les calques proches quand il y a de la place")
            .on_hover_text("Quand la timeline est plus haute que ses pistes, les calques les plus proches du calque actif prennent aussi la grande hauteur, tant qu'il reste de la place. La grande hauteur ne change pas.");
        if resp.clicked() {
            self.settings.montage_grands = grands;
            if let Err(e) = self.settings.save() {
                self.status = format!("Réglages non enregistrés : {e}");
            } else {
                self.status = if grands { "Les calques proches du calque actif grandissent quand la timeline a de la place." } else { "Seul le calque actif est grand dans la timeline." }.into();
            }
        }
        self.hauteur_pistes_pref_ui(ui);
    }
}

/// Une phrase pour chaque hauteur de la rangée des plans.
pub(crate) const PLANS_MOYENS: &str = "Moyenne : une vignette, le nom et la durée du plan dessous.";
pub(crate) const PLANS_BANDEAU: &str = "Bandeau : une petite vignette, le nom et la durée sur une ligne.";
