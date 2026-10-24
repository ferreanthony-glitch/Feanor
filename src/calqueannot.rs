//! Le calque d'annotation d'un plan (Anthony, 2026-10-24) : le même mot et la même idée que les calques d'annotation de la planche des
//! Vignettes (`annotations.rs`), mais **un vrai calque du plan**, dans la vue Dessin et dans la planche du Storyboard.
//!
//! # Deux endroits, une seule règle
//!
//! - **Sur la planche des Vignettes**, une annotation n'appartient à aucun plan : elle est attachée à une case de la planche, qui s'étend sur
//!   tous les plans (un trait qui entoure deux cases, une flèche d'une case à l'autre). Ses calques sont ceux de la planche (`Annotations`).
//! - **Dans un plan**, un calque d'annotation est un calque vectoriel du plan (`Layer::annotation`), comme les autres pour le dessin, le
//!   lasso, la transformation, la gomme, l'annulation, l'œil et le cadenas. Il n'a pas de planche : ses traits sont dans le plan, à la place
//!   où on les trace sur l'image.
//!
//! Les deux se reconnaissent à la même teinte rouge-rose (`theme::dessin_annotation`) et à la même icône ; la commande « Nouveau calque
//! d'annotation » crée celui du lieu où l'on est (la planche dans les Vignettes, le plan dans la vue Dessin). Ils ne se mélangent pas : une
//! annotation de la planche ne devient pas un calque du plan, et inversement.
//!
//! # Ce qu'un calque d'annotation de plan est
//!
//! - **Vectoriel**, avec son dessin tenu sur tout le plan (avant la première image et après la dernière, il montre son dessin) : une feuille de
//!   notes posée sur le plan, qu'on retouche à n'importe quelle image. Il se crée tout au-dessus.
//! - **Peint par-dessus tout, à l'écran** : même un calque d'effet au-dessus de lui ne le couvre pas (`App::build_plan`).
//! - **Jamais dans le film** : ni dans les exports (vidéo, images, PDF, GIF, montage, envois), ni dans les vignettes de la planche et de la
//!   timeline, ni dans la lecture de fond (`render::panel_plan_at`, que tous ces chemins empruntent : il saute les calques d'annotation).
//!   `render::panel_plan_at_avec` les laisse venir quand on le demande expressément : seule la case « Avec les annotations » des fenêtres
//!   d'export le demande (décochée d'office, `ExportJob::avec_annotations`).
//! - **Pas de clé de case** : une case de plus dans le plan ne copie pas ses traits (`cases.rs`), ce sont des notes sur le plan, pas un dessin
//!   de la case.
//! - **Enregistré** avec le calque (`annotation` dans le manifeste, sans changer `VERSION`) ; une version d'avant l'ouvre comme un calque
//!   vectoriel ordinaire.
//! - **Se fusionne seulement avec un autre calque d'annotation** (`Document::merge_layer_down`) : les notes n'entrent pas dans le dessin.

use crate::app::App;
use crate::document::{Behaviour, Layer, LayerKind};

/// Nom du premier calque d'annotation d'un plan.
pub const NOM: &str = "Annotations";

impl Layer {
    /// Un calque d'annotation de plan : vectoriel, son dessin tenu sur tout le plan.
    pub fn new_annotation(name: impl Into<String>) -> Layer {
        Layer { annotation: true, pre: Behaviour::Hold, post: Behaviour::Hold, auto_break: false, ..Layer::new(name, LayerKind::Vector) }
    }
}

/// L'ordre où poser les calques d'un plan : les calques ordinaires dans l'ordre de la pile, puis, si `annotations`, les calques d'annotation
/// (par-dessus tout, même au-dessus d'un calque d'effet). Sans `annotations` : les calques d'annotation n'y sont pas.
pub fn ordre(layers: &[Layer], annotations: bool) -> Vec<usize> {
    let ordinaires = (0..layers.len()).filter(|&i| !layers[i].annotation);
    if annotations {
        ordinaires.chain((0..layers.len()).filter(|&i| layers[i].annotation)).collect()
    } else {
        ordinaires.collect()
    }
}

/// « Annotations », puis « Annotations 2 »… : un nom que les calques du plan n'ont pas.
pub fn nom_libre(layers: &[Layer]) -> String {
    let pris = |n: &str| layers.iter().any(|l| l.name == n);
    if !pris(NOM) {
        return NOM.to_string();
    }
    (2..).map(|k| format!("{NOM} {k}")).find(|n| !pris(n)).expect("un nom libre")
}

impl App {
    /// Sur la planche des Vignettes, les cases ne montrent pas les calques d'annotation du plan : si le calque courant en est un, le dessin
    /// d'une case y partirait sans qu'on le voie. Le calque courant passe alors au plus haut calque qui se dessine et n'est pas une annotation.
    pub(crate) fn ecarter_le_calque_d_annotation(&mut self) {
        if !self.layers().get(self.cur_layer).is_some_and(|l| l.annotation) {
            return;
        }
        if let Some(i) = self.layers().iter().rposition(|l| !l.annotation && l.drawable()) {
            self.cur_layer = i;
            self.vsel = None;
        }
    }

    /// « Nouveau calque d'annotation » : sur la planche des Vignettes, un calque d'annotation de la planche ; dans la vue Dessin, un calque
    /// d'annotation du plan courant, tout au-dessus (une étape d'annulation).
    pub(crate) fn nouveau_calque_d_annotation(&mut self) {
        if self.board_shown() {
            self.annot_calque_nouveau();
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let name = nom_libre(self.layers());
        let at = self.layers().len();
        self.layers_mut().insert(at, Layer::new_annotation(name.clone()));
        self.cur_layer = at;
        self.layer_sel.clear();
        self.vsel = None;
        self.commit_structure(snap);
        self.status = format!("Calque d'annotation « {name} » : par-dessus tout, jamais dans les exports.");
    }
}
