//! Calque CTG dans l'interface (13j) : nouveau calque CTG (sous le calque courant, qui devient
//! sa source), transformer le calque courant en calque CTG ou le redevenir ordinaire, choisir
//! ses sources, charger sa structure à partir des sources. Voir `ctg.rs`.

use eframe::egui;

use crate::app::App;
use crate::ctg::{self, Ctg};
use crate::document::{Layer, LayerKind};

impl App {
    /// Nouveau calque CTG juste sous le calque courant ; le calque courant (s'il porte des
    /// dessins) devient sa source.
    pub(crate) fn new_ctg_layer(&mut self) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let cur = self.layer();
        let sources = if cur.folder || cur.ctg.is_some() { Vec::new() } else { vec![cur.id] };
        let parent = cur.parent;
        let n = self.layers().iter().filter(|l| l.ctg.is_some()).count() + 1;
        let snap = self.snapshot();
        let mut layer = Layer::new(format!("CTG {n}"), LayerKind::Bitmap);
        layer.ctg = Some(Ctg { sources });
        layer.parent = parent;
        let at = self.cur_layer;
        self.layers_mut().insert(at, layer);
        self.cur_layer = at;
        self.layer_sel.clear();
        self.vsel = None;
        self.commit_structure(snap);
        self.status = "Calque CTG : griffonne des couleurs dans les zones des traits, elles se remplissent.".into();
    }

    /// Le calque courant devient un calque CTG (sources : tous les autres calques de dessin) ;
    /// ses dessins deviennent des graffitis.
    pub(crate) fn convert_to_ctg(&mut self) {
        if self.layer().folder || self.layer().ctg.is_some() {
            self.status = "Choisis un calque de dessin ordinaire.".into();
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.record_step();
        let li = self.cur_layer;
        self.layers_mut()[li].ctg = Some(Ctg::default());
        self.status = "Calque transformé en calque CTG : ses dessins sont des graffitis.".into();
    }

    /// Le calque CTG courant redevient un calque ordinaire (ses graffitis restent tels quels).
    pub(crate) fn disable_ctg(&mut self) {
        if self.layer().ctg.is_none() {
            self.status = "Ce calque n'est pas un calque CTG.".into();
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.record_step();
        let li = self.cur_layer;
        self.layers_mut()[li].ctg = None;
        self.status = "Calque CTG désactivé : ses graffitis restent, sans remplissage.".into();
    }

    /// « Charger la structure à partir des sources » du calque CTG courant (`clear` : en
    /// repartant de graffitis vides).
    pub(crate) fn load_ctg_structure(&mut self, clear: bool) {
        let li = self.cur_layer;
        if self.layer().ctg.is_none() {
            self.status = "Ce calque n'est pas un calque CTG.".into();
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let len = self.frame_count();
        let sources: Vec<Layer> = ctg::source_layers(self.layers(), li).into_iter().map(|k| self.layers()[k].clone()).collect();
        if sources.is_empty() {
            self.status = "Ce calque CTG n'a pas de source.".into();
            return;
        }
        let kind = self.layer().kind;
        let old = self.layer().frames.clone();
        let snap = self.snapshot();
        let refs: Vec<&Layer> = sources.iter().collect();
        let doc = &mut self.doc;
        let frames = ctg::structure(len, &refs, &old, clear, || doc.new_cel(kind));
        let n = crate::images::drawings_in_order(&frames).len();
        self.layers_mut()[li].frames = frames;
        self.commit_structure(snap);
        self.status = format!("Structure chargée à partir des sources : {n} dessin{} de graffitis.", if n > 1 { "s" } else { "" });
    }

    /// Les sources du calque CTG `li` (menu de la ligne du calque).
    pub(crate) fn ctg_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some(ctg) = self.layers().get(li).and_then(|l| l.ctg.clone()) else { return };
        ui.label(egui::RichText::new("Sources (leurs traits ferment les zones) :").weak());
        let all = ctg.sources.is_empty();
        if ui.radio(all, "Tous les autres calques de dessin").clicked() && !all {
            self.record_step();
            self.layers_mut()[li].ctg = Some(Ctg::default());
        }
        let candidates: Vec<(u64, String)> = self
            .layers()
            .iter()
            .enumerate()
            .rev()
            .filter(|(k, l)| *k != li && !l.folder && l.ctg.is_none())
            .map(|(_, l)| (l.id, l.name.clone()))
            .collect();
        for (id, name) in candidates {
            let mut on = !all && ctg.sources.contains(&id);
            if ui.checkbox(&mut on, name).changed() {
                self.record_step();
                let mut sources = if all { Vec::new() } else { ctg.sources.clone() };
                if on {
                    sources.push(id);
                } else {
                    sources.retain(|s| *s != id);
                }
                self.layers_mut()[li].ctg = Some(Ctg { sources });
            }
        }
        ui.separator();
        self.action_item(ui, "Charger la structure à partir des sources", "ctg.charger_structure");
        self.action_item(ui, "Charger la structure à partir des sources (effacer)", "ctg.charger_structure_effacer");
        ui.separator();
        self.action_item(ui, "Désactiver le calque CTG", "calque.desactiver_ctg");
    }
}
