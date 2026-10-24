//! Fusionner deux plans en glissant l'un sur l'autre, dans le montage (`storyboard.rs`).
//!
//! Dans la bande du montage, glisser un plan le **déplace** : la place où il tombe est celle qu'indique le repère
//! (avant ou après le plan visé, selon la moitié sur laquelle le pointeur est). Ici, le **milieu** d'un plan (la moitié
//! centrale de sa largeur) veut dire autre chose : **fusionner**. Le plan visé s'entoure et porte « Fusionner » ; en
//! lâchant, le plan glissé est réuni à lui. Les deux quarts de bord gardent leur sens : « déplacer ici ».
//!
//! La fusion est celle qui existe déjà (`Document::join_next_plan_without_mark`, donc `append_plan` : les calques se
//! réunissent par nom et réglage, les marques d'image, les notes et les textes suivent, aucun dessin n'est perdu).
//! Comme « Réunir avec le plan précédent », **le plan qui vient le premier garde son identité** (son numéro, son nom,
//! ses versions) et reçoit les images de l'autre à la suite : si le plan glissé venait avant le plan visé, ses images
//! passent avant ; sinon, après. Si les deux ne se touchent pas, le plan glissé quitte sa place et vient se coller au
//! plan visé (les plans entre eux ne bougent pas, sauf qu'il n'y est plus). Une seule étape d'annulation.
//!
//! Les deux plans doivent être de la même séquence (le plan glissé ne change jamais de séquence : une séquence vidée
//! par un plan qui s'en va perdrait son nom). Plusieurs plans choisis qu'on glisse ensemble se déplacent, sans fusion.

use eframe::egui::{Pos2, Rect};

use crate::app::App;
use crate::document::Document;

/// La part de la largeur d'un plan, de chaque côté, qui veut dire « déplacer ici » (le reste, au milieu : fusionner).
pub(crate) const BORD: f32 = 0.25;

/// La zone du milieu d'un bloc de plan : celle où lâcher un plan le fusionne (la moitié centrale de la largeur).
pub(crate) fn zone_du_milieu(bloc: Rect) -> Rect {
    let w = bloc.width();
    Rect::from_min_max(Pos2::new(bloc.left() + w * BORD, bloc.top()), Pos2::new(bloc.right() - w * BORD, bloc.bottom()))
}

/// Le plan que le pointeur `p` vise pour une fusion, quand on glisse le plan `glisse` : celui dont le milieu est sous le
/// pointeur, s'il est un autre plan de la même séquence. `seul` : on ne glisse qu'un plan (un groupe se déplace, il ne fusionne pas).
pub(crate) fn plan_vise(doc: &Document, blocs: &[Rect], p: Pos2, glisse: u64, seul: bool) -> Option<u64> {
    if !seul || doc.storyboard_locked {
        return None;
    }
    let (i, _) = blocs.iter().enumerate().find(|(_, b)| zone_du_milieu(**b).contains(p))?;
    let x = doc.panels.iter().position(|q| q.id == glisse)?;
    if i == x || i >= doc.panels.len() || doc.sequence_of(i) != doc.sequence_of(x) {
        return None;
    }
    Some(doc.panels[i].id)
}

impl Document {
    /// Réunit le plan `x` au plan `y` (voir le haut de ce fichier). Le plan qui vient le premier, une fois les deux voisins, garde
    /// son identité. Renvoie l'indice du plan réuni, ou rien (et alors rien n'a bougé) si l'un n'existe plus, s'ils sont le même
    /// plan ou s'ils ne sont pas de la même séquence.
    pub fn merge_plan_into(&mut self, x: u64, y: u64) -> Option<usize> {
        let (px, py) = (self.panels.iter().position(|p| p.id == x)?, self.panels.iter().position(|p| p.id == y)?);
        if px == py || self.sequence_of(px) != self.sequence_of(py) {
            return None;
        }
        if px + 1 != py && py + 1 != px {
            // Pas voisins : le plan glissé vient se poser contre le plan visé, du côté d'où il venait.
            let devant = if px < py { Some(y) } else { self.panels.get(py + 1).map(|p| p.id) };
            self.move_plans(&[x], devant)?;
        }
        let (px, py) = (self.panels.iter().position(|p| p.id == x)?, self.panels.iter().position(|p| p.id == y)?);
        if px + 1 != py && py + 1 != px {
            return None;
        }
        self.join_next_plan_without_mark(px.min(py))
    }
}

impl App {
    /// Le plan `x`, lâché au milieu du plan `y` : fusionné avec lui, en une étape d'annulation (`fusionglisse.rs`).
    pub(crate) fn fusionner_en_glissant(&mut self, x: u64, y: u64) {
        if self.storyboard_refuses() || !self.partage_peut_toucher(&[x, y]) {
            return;
        }
        let nom = "Fusionner deux plans";
        let avant = self.history.context.replace(nom.to_string());
        let fait = self.structure_op_dessins(|d, _| d.merge_plan_into(x, y));
        self.history.context = avant;
        if fait {
            self.board_sel.clear();
            self.status = format!("Plans fusionnés : {}.", crate::storyboard::panel_title(&self.doc, self.cur_panel));
        } else {
            self.status = "Ces deux plans ne peuvent pas être fusionnés : ils doivent être de la même séquence.".into();
        }
    }
}
