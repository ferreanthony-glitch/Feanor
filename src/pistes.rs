//! Pistes du montage (Anthony, 2026-10-20 : « un système de dépliage / repliage des calques dans la timeline pour gagner de la place
//! tout en gardant cela lisible » ; 2026-10-28, branche `timeline-repliee` : « seul le calque actif est déplié » ; 2026-11-11, branche
//! `calques-timeline` : « trois calques maximum affichés sur la timeline », les dossiers « jamais traités comme des calques »).
//!
//! Dans le Storyboard et le Montage (l'onglet Animation garde sa timeline, tous calques dépliés), deux façons de montrer la pile, au
//! choix dans la barre du bas de la bande et dans Préférences > Montage (`Pile`, réglage du poste) :
//! - **trois calques** (d'office) : le calque **actif** au milieu, toujours à la même place (`LAYER_H`, 40), entre son voisin du dessus
//!   et celui du dessous (`VOISIN_H`, 19). Au-dessus et au-dessous, une **fine bande** compte les autres calques (« 4 au-dessus ») et les
//!   montre en traits ; un clic y saute de trois calques. La fenêtre fait toujours la même hauteur (`MARGE_H` + `LAYER_H` + `MARGE_H`) ;
//! - **toute la pile** : le calque actif déplié, tous les autres en bandes (`FOLDED_H`, 17).
//!
//! **Un dossier n'est jamais un calque** : c'est une ligne fine (`DOSSIER_H`, 13), plus petite que toute piste, qu'un clic ferme ou
//! ouvre. Elle ne compte pas dans les trois et n'est jamais le calque actif. Quand elle tombe entre deux calques de la fenêtre, c'est la
//! fine bande du haut ou du bas qui lui cède sa place : le calque actif ne bouge pas. **Un dossier fermé se traverse** : « calque du
//! dessus / du dessous » entre dedans, il se montre ouvert tant que le calque courant est l'un des siens (`dossier_de_passage`) et se
//! referme quand on en sort ; rien n'est écrit pour cela.
//!
//! Dans la colonne de gauche, une piste porte l'œil, une pastille de la couleur de la sorte du calque (vectoriel, bitmap…), le nom et le
//! cadenas d'un calque verrouillé ; un **filet de la couleur de son dossier** court à sa gauche (`folders::couleur_du_rangement` : la
//! bande dit le type, le filet dit le rangement). L'opacité, le mode de fusion… sont au clic droit sur la piste (`montage_layer_menu`)
//! et dans le panneau Calques. Cliquer un calque (son nom ou l'un de ses blocs) le rend actif, et le panneau Calques suit.
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
use crate::montagerows::{layer_in_row, row_count, row_of_layer, LAYER_H};
use crate::theme;

/// Hauteur d'une piste repliée (« Toute la pile ») : une bande de 15 points, le nom en 11.
pub(crate) const FOLDED_H: f32 = 17.0;
/// Hauteur de la ligne d'un dossier : jamais un calque, toujours plus petite qu'une piste.
pub(crate) const DOSSIER_H: f32 = 13.0;
/// « Trois calques » : la piste d'un voisin du calque actif.
pub(crate) const VOISIN_H: f32 = 19.0;
/// « Trois calques » : la place au-dessus (et au-dessous) du calque actif — la fine bande, le voisin, et la ligne d'un dossier quand
/// il y en a une entre les deux. Toujours la même : le calque actif ne bouge pas.
pub(crate) const MARGE_H: f32 = 45.0;
/// La « rangée » d'une fine bande (« N au-dessus », « N au-dessous ») : aucun calque n'y répond.
pub(crate) const MORE_ROW: usize = usize::MAX;
/// De combien de calques un clic sur une fine bande fait avancer.
pub(crate) const SAUT: usize = 3;
/// Place de l'œil à gauche d'une piste.
const EYE_W: f32 = 15.0;
/// Place de la pastille de couleur, après l'œil.
const DOT_W: f32 = 10.0;
/// Retrait d'un calque qui a un filet de couleur à sa gauche (celui de son dossier).
pub(crate) const RETRAIT: f32 = 6.0;
/// Place du cadenas d'un calque verrouillé, à droite du nom.
const VERROU_W: f32 = 13.0;

/// Ce qu'une ligne des pistes est.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Sorte {
    /// La piste d'un calque (dépliée, repliée, ou voisine du calque actif).
    #[default]
    Calque,
    /// La ligne fine d'un dossier.
    Dossier,
    /// « Trois calques » : la fine bande des calques au-dessus de la fenêtre.
    Dessus,
    /// « Trois calques » : la fine bande des calques au-dessous de la fenêtre.
    Dessous,
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
    /// Une fine bande : les rangées qu'elle résume (de la première à la dernière, exclue). Les autres lignes : rien.
    pub rangees: (usize, usize),
    /// Une fine bande : combien de calques du plan courant elle compte (jamais les dossiers ; ceux d'un dossier fermé compris).
    pub compte: usize,
}

impl Lane {
    /// La piste d'un calque.
    pub(crate) fn calque(row: usize, top: f32, h: f32, folded: bool) -> Lane {
        Lane { row, top, h, folded, sorte: Sorte::Calque, rangees: (0, 0), compte: 0 }
    }

    /// La ligne fine d'un dossier.
    pub(crate) fn dossier(row: usize, top: f32) -> Lane {
        Lane { row, top, h: DOSSIER_H, folded: true, sorte: Sorte::Dossier, rangees: (0, 0), compte: 0 }
    }

    /// Une fine bande (« N au-dessus », « N au-dessous »).
    pub(crate) fn bande(sorte: Sorte, top: f32, h: f32, rangees: (usize, usize), compte: usize) -> Lane {
        Lane { row: MORE_ROW, top, h, folded: true, sorte, rangees, compte }
    }

    /// Une fine bande de la fenêtre des trois calques (pas une piste de calque).
    pub fn is_more(&self) -> bool {
        matches!(self.sorte, Sorte::Dessus | Sorte::Dessous)
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

/// Comment la pile des calques se montre dans la timeline du Storyboard et du Montage (réglage du poste, `reglages.toml` :
/// `calques_timeline`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pile {
    /// Trois calques à la fois : le calque actif au milieu, ses deux voisins, et deux fines bandes qui comptent les autres (d'office).
    #[default]
    Trois,
    /// Toute la pile : le calque actif déplié, les autres en bandes.
    Toute,
}

impl Pile {
    pub fn est_trois(&self) -> bool {
        *self == Pile::Trois
    }

    pub fn nom(self) -> &'static str {
        match self {
            Pile::Trois => "Trois calques",
            Pile::Toute => "Toute la pile",
        }
    }
}

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
    pub mode: Pile,
    /// « Toute la pile » : les rangées dépliées à la main (en plus du calque actif).
    pub ouvertes: BTreeSet<usize>,
    /// « Toute la pile » : la rangée du calque actif est repliée à la main, jusqu'au prochain changement de calque.
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

/// « Toute la pile » : la rangée `row` est repliée — elle ne l'est pas si c'est celle du calque actif (sauf repliée à la main) ou si
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

/// Les lignes visibles de haut en bas, pour `count` rangées au plus : la fenêtre des trois calques, ou toute la pile (les pistes d'un
/// dossier fermé en moins).
pub(crate) fn lanes_in(panels: &[Panel], cur: usize, cur_layer: usize, count: usize, auto: AutoOpen, replis: &Replis) -> Vec<Lane> {
    if replis.mode == Pile::Trois {
        if let Some(lanes) = fenetre(panels, cur, cur_layer, count, auto) {
            return lanes;
        }
    }
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

/// « Trois calques » : la fine bande du dessus, le voisin du dessus, la ligne d'un dossier s'il y en a une entre lui et le calque
/// actif, le calque actif, puis la même chose en miroir. Rien si le plan courant n'a aucun calque à montrer.
fn fenetre(panels: &[Panel], cur: usize, cur_layer: usize, count: usize, auto: AutoOpen) -> Option<Vec<Lane>> {
    let plan = panels.get(cur)?;
    let layers = &plan.layers;
    let n = row_count(layers);
    let calque = |row: usize| sorte_de_rangee(layers, row) == Some(Sorte::Calque);
    let dossier = |row: usize| sorte_de_rangee(layers, row) == Some(Sorte::Dossier);
    let visibles: Vec<usize> = (0..n).filter(|&r| calque(r) && !row_hidden(panels, cur, cur_layer, r)).collect();
    // Le centre : la rangée du calque actif s'il est de ce plan, sinon celle du calque courant. Un dossier courant (choisi dans le
    // panneau Calques) : le premier calque visible à partir de sa ligne, sinon le dernier.
    let voulu = auto.filter(|(p, _)| *p == plan.id).and_then(|a| active_row(panels, Some(a))).or_else(|| row_of_layer(layers, cur_layer))?;
    let centre = visibles.iter().copied().find(|&r| r >= voulu).or_else(|| visibles.last().copied())?;
    let ic = visibles.iter().position(|&r| r == centre)?;
    let haut = ic.checked_sub(1).map(|i| visibles[i]);
    let bas = visibles.get(ic + 1).copied();
    // La ligne de dossier la plus proche du calque actif, entre lui et chaque voisin (ou le bout de la pile).
    let dossier_haut = (haut.map_or(0, |h| h + 1)..centre).rev().find(|&r| dossier(r));
    let dossier_bas = (centre + 1..bas.unwrap_or(n)).find(|&r| dossier(r));
    let (borne_haut, borne_bas) = (haut.unwrap_or(centre), bas.unwrap_or(centre) + 1);
    let compte = |a: usize, b: usize| (a..b.min(n)).filter(|&r| calque(r)).count();
    // La fine bande cède sa place au voisin et à la ligne de dossier.
    let h_bande = |voisin: bool, dos: bool| MARGE_H - if voisin { VOISIN_H } else { 0.0 } - if dos { DOSSIER_H } else { 0.0 };
    let mut out: Vec<Lane> = Vec::with_capacity(7);
    let mut top = 0.0;
    let mut pose = |mut l: Lane| {
        l.top = top;
        top += l.h;
        out.push(l);
    };
    pose(Lane::bande(Sorte::Dessus, 0.0, h_bande(haut.is_some(), dossier_haut.is_some()), (0, borne_haut), compte(0, borne_haut)));
    if let Some(r) = haut {
        pose(Lane::calque(r, 0.0, VOISIN_H, true));
    }
    if let Some(r) = dossier_haut {
        pose(Lane::dossier(r, 0.0));
    }
    pose(Lane::calque(centre, 0.0, LAYER_H, false));
    if let Some(r) = dossier_bas {
        pose(Lane::dossier(r, 0.0));
    }
    if let Some(r) = bas {
        pose(Lane::calque(r, 0.0, VOISIN_H, true));
    }
    pose(Lane::bande(Sorte::Dessous, 0.0, h_bande(bas.is_some(), dossier_bas.is_some()), (borne_bas, count.max(n)), compte(borne_bas, n)));
    Some(out)
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

    /// Le calque actif : toujours dans la fenêtre des trois calques ; dans « Toute la pile », si le réglage « Déplier le calque
    /// courant » est allumé.
    pub(crate) fn pistes_auto_open(&self) -> AutoOpen {
        if self.settings.montage_pile == Pile::Trois {
            return self.pistes.auto_open;
        }
        self.settings.montage_auto_open.then_some(self.pistes.auto_open).flatten()
    }

    /// La rangée du calque `li` du plan courant (celle de son trait pour un calque de couleur lié).
    pub(crate) fn track_row_of(&self, li: usize) -> usize {
        row_of_layer(self.layers(), li).unwrap_or(0)
    }

    /// Comment les rangées se montrent maintenant : le réglage du poste et ce qu'on a déplié ou replié à la main.
    pub(crate) fn pistes_replis(&self) -> Replis {
        Replis { mode: self.settings.montage_pile, ouvertes: self.pistes.ouvertes.clone(), active_fermee: self.pistes.active_fermee }
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
        self.status = if self.settings.montage_pile == Pile::Trois {
            "La timeline montre trois calques : « Toute la pile », dans la barre du bas, pour replier ou déplier des pistes.".into()
        } else {
            format!("Toutes les pistes sont {}.", if folded { "repliées" } else { "dépliées" })
        };
        changed
    }

    /// Trois calques, ou toute la pile (réglage du poste, enregistré avec les réglages ; rien dans le projet).
    pub(crate) fn set_pile(&mut self, mode: Pile) {
        if self.settings.montage_pile == mode {
            return;
        }
        self.settings.montage_pile = mode;
        if let Err(e) = self.settings.save() {
            self.status = format!("Réglages non enregistrés : {e}");
            return;
        }
        self.status = match mode {
            Pile::Trois => "Timeline : trois calques à la fois, le calque actif au milieu.".into(),
            Pile::Toute => "Timeline : toute la pile.".into(),
        };
    }

    /// « Toute la pile » : allumée, éteinte (`piste.toute_la_pile`).
    pub(crate) fn basculer_pile(&mut self) {
        self.set_pile(if self.settings.montage_pile == Pile::Trois { Pile::Toute } else { Pile::Trois });
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

    /// « Calque du dessus » (`pas` = +1) ou « du dessous » (−1), `fois` fois de suite (un clic sur une fine bande : trois).
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

    /// Une fine bande de la fenêtre des trois calques, dans `r` : « N au-dessus », « N au-dessous » (un clic saute de `SAUT` calques),
    /// ou le bout de la pile.
    pub(crate) fn bande_head(&mut self, ui: &mut Ui, r: Rect, lane: &Lane) {
        let dessus = lane.sorte == Sorte::Dessus;
        let resp = (lane.compte > 0).then(|| ui.interact(r, ui.id().with(("bande_pile", dessus)), Sense::click()));
        let survol = resp.as_ref().is_some_and(Response::hovered);
        let ink = if survol { theme::texte_fort() } else { ui.visuals().weak_text_color() };
        let texte = match (lane.compte, dessus) {
            (0, true) => "Haut de la pile".to_string(),
            (0, false) => "Bas de la pile".to_string(),
            (n, true) => format!("{} {n} au-dessus", ph::CARET_UP),
            (n, false) => format!("{} {n} au-dessous", ph::CARET_DOWN),
        };
        let p = ui.painter().with_clip_rect(r);
        p.rect_filled(r, 3.0, theme::etiquette());
        p.text(pos2(r.left() + 8.0, r.center().y), Align2::LEFT_CENTER, texte, FontId::proportional(10.5), ink);
        let Some(resp) = resp else { return };
        let aide = if dessus { "Les calques au-dessus de la fenêtre des trois." } else { "Les calques au-dessous de la fenêtre des trois." };
        let resp = crate::bulles::bulle(resp, if dessus { "Au-dessus" } else { "Au-dessous" }, &format!("{aide} Un clic saute de trois calques ; « Toute la pile », dans la barre du bas, les montre tous."), "");
        if resp.clicked() {
            self.aller_au_calque(if dessus { 1 } else { -1 }, SAUT);
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

    // ------------------------------------------------------------------ les menus et les réglages

    /// « Calques dans la timeline » : dans le menu du calque `li` (clic droit sur sa piste).
    pub(crate) fn pistes_menu(&mut self, ui: &mut Ui, li: usize) {
        let row = self.track_row_of(li);
        let folded = self.track_is_folded(row);
        ui.menu_button("Calques dans la timeline", |ui| {
            for (mode, aide) in [(Pile::Trois, PILE_TROIS), (Pile::Toute, PILE_TOUTE)] {
                let on = self.settings.montage_pile == mode;
                let resp = ui.selectable_label(on, mode.nom());
                if crate::bulles::bulle(resp, mode.nom(), aide, "").clicked() {
                    ui.close_menu();
                    self.set_pile(mode);
                }
            }
            if self.settings.montage_pile == Pile::Toute {
                ui.separator();
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
            }
        });
    }

    /// Les deux façons de montrer la pile, en deux icônes à choisir : petites dans la barre du bas de la bande (`petit`), comme les
    /// autres choix dans les Préférences.
    pub(crate) fn pile_choix_ui(&mut self, ui: &mut Ui, petit: bool) {
        let touche = self.raccourci("piste.toute_la_pile");
        let mut mode = self.settings.montage_pile;
        let a = choix_en_icone(ui, petit, &mut mode, Pile::Trois, ph::ROWS, Pile::Trois.nom(), PILE_TROIS, &touche);
        let b = choix_en_icone(ui, petit, &mut mode, Pile::Toute, ph::STACK, Pile::Toute.nom(), PILE_TOUTE, &touche);
        if a || b {
            self.set_pile(mode);
        }
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

    /// La section « Montage » de Fichier > Préférences… : « Calques dans la timeline : trois calques / toute la pile », « Hauteur des
    /// plans : moyenne / bandeau » (deux icônes à choisir, une phrase de bulle pour chacune) et « Déplier le calque courant ».
    pub(crate) fn montage_prefs_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let label = ui.label("Calques dans la timeline");
            crate::bulles::bulle(label, "Calques dans la timeline", &format!("{PILE_TROIS} {PILE_TOUTE}"), "");
            self.pile_choix_ui(ui, false);
            ui.label(RichText::new(self.settings.montage_pile.nom().to_lowercase()).small().weak());
        });
        ui.horizontal(|ui| {
            let label = ui.label("Hauteur des plans");
            crate::bulles::bulle(label, "Hauteur des plans", &format!("{PLANS_MOYENS} {PLANS_BANDEAU}"), "");
            self.plans_choix_ui(ui, false);
            ui.label(RichText::new(self.settings.montage_plans.nom().to_lowercase()).small().weak());
        });
        let mut auto = self.settings.montage_auto_open;
        let resp = ui.checkbox(&mut auto, "Toute la pile : déplier le calque courant tout seul").on_hover_text("Dans « Toute la pile », seul le calque actif est déplié ; les autres sont en bandes.");
        if resp.clicked() {
            self.run_action("piste.auto_deplier");
        }
        self.hauteur_pistes_pref_ui(ui);
    }
}

/// Une phrase pour chaque façon de montrer la pile (bulles, Préférences, menu).
pub(crate) const PILE_TROIS: &str = "Trois calques : le calque actif au milieu, toujours à la même place, entre son voisin du dessus et celui du dessous ; deux fines bandes comptent les autres.";
pub(crate) const PILE_TOUTE: &str = "Toute la pile : le calque actif déplié, tous les autres en bandes. Pour comparer deux calques éloignés ou en choisir plusieurs.";
/// Une phrase pour chaque hauteur de la rangée des plans.
pub(crate) const PLANS_MOYENS: &str = "Moyenne : une vignette, le nom et la durée du plan dessous.";
pub(crate) const PLANS_BANDEAU: &str = "Bandeau : une petite vignette, le nom et la durée sur une ligne.";
