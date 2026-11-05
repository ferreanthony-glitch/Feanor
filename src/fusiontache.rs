//! La fusion de calques en tâche de fond (règle « zéro latence » de SHOT, comme les exports, partie 28) : une fusion qui demande
//! plus d'une image d'écran (effets, aplat, animation longue : `fusioncalques::est_rapide`) ne se calcule pas sur le fil de
//! l'interface.
//!
//! - **Départ** : l'interface copie le nécessaire (un petit document avec les seuls calques fusionnés et leurs dessins, partagés
//!   à la copie : `Document::mini_pour_fusion`), lance un fil et rend la main tout de suite.
//! - **Pendant** : la barre d'état montre « Fusion des calques » avec sa progression et « Arrêter » ; le reste de SHOT reste
//!   utilisable.
//! - **Fin** : le résultat est posé **en une seule étape d'annulation**, à la place des calques fusionnés. Si on a arrêté, rien ne
//!   change. Si les calques ont changé entre-temps (dessin dessus, suppression, annulation, autre fusion…), la fusion est
//!   **abandonnée avec un message** plutôt que d'écraser ce travail.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::app::App;
use crate::document::{Cel, CelId, Layer};
use crate::fusioncalques::{self, Controle};

/// Ce que la tâche rend : le calque fusionné, et les dessins neufs qu'il montre.
pub(crate) struct Sortie {
    layer: Layer,
    cels: Vec<(CelId, Cel)>,
}

/// Une fusion en cours.
pub(crate) struct Tache {
    ctl: Arc<Controle>,
    handle: JoinHandle<Result<Sortie, String>>,
    /// Le plan, les calques (du plus bas au plus haut) et leurs dessins tels qu'ils étaient au départ : s'ils ne sont plus pareils
    /// à la fin, la fusion est abandonnée.
    panel: u64,
    avant: Vec<Layer>,
    versions: Vec<(CelId, u64)>,
    depuis: Instant,
    nombre: usize,
    resultat: &'static str,
}

impl Tache {
    /// Le calcul est fini (le résultat attend d'être posé).
    #[cfg(test)]
    pub(crate) fn est_finie(&self) -> bool {
        self.handle.is_finished()
    }
}

impl App {
    /// Lance la fusion de `lis` (déjà vérifiée) en tâche de fond.
    pub(crate) fn lancer_fusion(&mut self, lis: Vec<usize>) {
        let index = self.cur_panel;
        let lis = fusioncalques::rangs_a_fusionner(self.layers(), &lis);
        let resultat = fusioncalques::resultat_de(&self.doc, index, &lis);
        let images = fusioncalques::images_a_calculer(&self.doc, index, &lis);
        let mut mini = self.doc.mini_pour_fusion(index, &lis);
        let avant: Vec<Layer> = lis.iter().map(|&l| self.doc.panels[index].layers[l].clone()).collect();
        let cels: BTreeSet<CelId> = avant.iter().flat_map(|l| l.frames.iter().flatten().copied()).collect();
        let versions = cels.iter().filter_map(|id| self.doc.cels.get(id).map(|c| (*id, c.version))).collect();
        let panel = self.doc.panels[index].id;
        let ctl = Arc::new(Controle::default());
        ctl.total.store((images * (avant.len() - 1)).max(1), Ordering::Relaxed);
        let (c, nombre) = (ctl.clone(), avant.len());
        let handle = std::thread::spawn(move || {
            // Un travail de fond laisse des cœurs libres à l'interface.
            crate::render::set_thread_cap((crate::parallele::coeurs() / 2).max(1));
            let connus: std::collections::HashSet<CelId> = mini.cels.keys().copied().collect();
            let tous: Vec<usize> = (0..nombre).collect();
            crate::layerops::merge_layers_ctl(&mut mini, 0, &tous, Some(&c))?;
            let layer = mini.panels[0].layers.remove(0);
            let cels = mini.cels.into_iter().filter(|(id, _)| !connus.contains(id)).collect();
            Ok(Sortie { layer, cels })
        });
        self.status = "Fusion des calques en cours… (barre d'état : Arrêter)".into();
        self.fusion = Some(Tache { ctl, handle, panel, avant, versions, depuis: Instant::now(), nombre, resultat });
    }

    /// À chaque image d'affichage : la tâche est-elle finie ? Sinon, on la regardera plus tard (rien ne bloque).
    pub(crate) fn fusion_tick(&mut self, ctx: &egui::Context) {
        let Some(t) = &self.fusion else { return };
        if t.handle.is_finished() {
            self.finir_fusion();
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// « Arrêter » : la tâche s'interrompt à la prochaine image, rien ne change.
    pub(crate) fn arreter_fusion(&mut self) {
        if let Some(t) = &self.fusion {
            t.ctl.arret.store(true, Ordering::Relaxed);
        }
    }

    /// La barre d'état : la progression et « Arrêter ».
    pub(crate) fn fusion_status_ui(&mut self, ui: &mut egui::Ui) {
        let Some(t) = &self.fusion else { return };
        let (fait, total) = (t.ctl.fait.load(Ordering::Relaxed), t.ctl.total.load(Ordering::Relaxed).max(1));
        let arret = t.ctl.arret.load(Ordering::Relaxed);
        ui.separator();
        ui.label(format!("Fusion de {} calques", t.nombre));
        ui.add(crate::barre::Barre::new((fait as f32 / total as f32).min(1.0)).desired_width(120.0).show_percentage());
        let bouton = ui.add_enabled(!arret, egui::Button::new(if arret { "Arrêt…" } else { "Arrêter" }));
        if bouton.on_hover_text("Arrête la fusion : rien ne change.").clicked() {
            self.arreter_fusion();
        }
    }

    /// La tâche est finie : on pose son résultat (une étape d'annulation), ou on l'abandonne.
    pub(crate) fn finir_fusion(&mut self) {
        let Some(t) = self.fusion.take() else { return };
        let arretee = t.ctl.arret.load(Ordering::Relaxed);
        let sortie = match t.handle.join() {
            Err(_) => {
                self.status = "La fusion s'est arrêtée sur une erreur ; rien n'a changé.".into();
                return;
            }
            Ok(Err(_)) if arretee => {
                self.status = "Fusion arrêtée : rien n'a changé.".into();
                return;
            }
            Ok(Err(e)) => {
                self.status = e;
                return;
            }
            Ok(Ok(s)) => s,
        };
        if arretee {
            self.status = "Fusion arrêtée : rien n'a changé.".into();
            return;
        }
        // Les calques et leurs dessins sont-ils restés tels qu'au départ ?
        let abandon = "Fusion abandonnée : les calques ont changé pendant le calcul (dessin, suppression, annulation…). Rien n'a été écrasé, relance la fusion.";
        let Some(pi) = self.doc.panels.iter().position(|p| p.id == t.panel) else {
            self.status = abandon.into();
            return;
        };
        let mut positions = Vec::with_capacity(t.avant.len());
        for ancien in &t.avant {
            match self.doc.panels[pi].layers.iter().position(|l| l.id == ancien.id) {
                Some(i) if &self.doc.panels[pi].layers[i] == ancien => positions.push(i),
                _ => {
                    self.status = abandon.into();
                    return;
                }
            }
        }
        if t.versions.iter().any(|(id, v)| self.doc.cels.get(id).map(|c| c.version) != Some(*v)) || self.content_refuses() {
            self.status = abandon.into();
            return;
        }
        // Pose, en une étape : les dessins neufs (sous des numéros de ce document), puis le calque à la place du plus bas.
        let snap = self.snapshot();
        let mut numeros: HashMap<CelId, CelId> = HashMap::new();
        for (ancien, cel) in sortie.cels {
            numeros.insert(ancien, self.doc.add_cel(cel));
        }
        let renum = |id: &CelId| numeros.get(id).copied().unwrap_or(*id);
        let mut layer = sortie.layer;
        for f in layer.frames.iter_mut().flatten() {
            *f = renum(f);
        }
        layer.names = layer.names.iter().map(|(c, n)| (renum(c), n.clone())).collect();
        layer.color_marks = layer.color_marks.iter().map(|(c, k)| (renum(c), *k)).collect();
        let mut tries = positions.clone();
        tries.sort_unstable();
        let bas = tries[0];
        let layers = &mut self.doc.panels[pi].layers;
        for &i in tries.iter().rev() {
            layers.remove(i);
        }
        layers.insert(bas, layer);
        if self.cur_panel == pi {
            self.cur_layer = bas;
        }
        self.layer_sel.clear();
        self.vsel = None;
        self.commit_structure(snap);
        let secondes = t.depuis.elapsed().as_secs_f32();
        self.status = format!("{} calques fusionnés en un ({secondes:.1} s). {} Ctrl+Z les rend comme avant.", t.nombre, t.resultat);
    }

    /// Attend la fin de la tâche et la pose (les essais).
    #[cfg(test)]
    pub(crate) fn fusion_attendre(&mut self) {
        while self.fusion.as_ref().is_some_and(|t| !t.handle.is_finished()) {
            std::thread::sleep(Duration::from_millis(5));
        }
        self.finir_fusion();
    }
}
