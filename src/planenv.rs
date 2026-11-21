//! Partie 204 (`docs/INTERFACE.md` partie 204 ; Anthony : « on doit pouvoir créer un plan qui s'extrait de l'environnement global de
//! la séquence ») : **l'environnement d'un plan**.
//!
//! - Un plan partage d'office l'environnement de sa séquence (son décor 3D, ses lampes, son cyclo et son dessin, son soleil et son
//!   ciel), comme avant. Il peut en **sortir** : un environnement **vide**, une **copie** retouchable de celui de sa séquence, ou
//!   **celui d'une autre séquence**, **lié** (en direct : une chaise déplacée dans la séquence 2 bouge dans ce plan). « Détacher »
//!   fait du lien une copie à lui ; « Revenir à l'environnement de la séquence » le rend à sa séquence (demandé d'abord si son
//!   environnement propre a été retouché). Chaque changement est une étape d'annulation.
//! - **Un seul chemin** : `Document::env_cle` (`decor3d.rs`) donne la clé de l'environnement d'un plan dans `Decors3d::par_sequence`
//!   (sa séquence, `cle_de_plan`, ou la séquence liée). L'image des plans, l'export, les vignettes, les ombres, les lampes et la
//!   lumière du cyclo sur les cartes, « Caché dans le plan », le budget et les caches (`DecorCle`, par le décor que la clé désigne),
//!   l'onglet 3D (Scène 3D, Propriétés, les vues : ils retouchent l'environnement du plan courant) passent tous par lui.
//! - **Les clés des objets animés** (partie 200) se lisent à l'image de la séquence du plan (`image_de_sequence`), quel que soit son
//!   environnement : une copie s'anime comme l'original ; un plan lié montre l'environnement de l'autre séquence à la place qu'il a
//!   dans la sienne.
//! - **Le seul signe** : une petite marque sur le bloc du plan (le Storyboard, la timeline de la séquence de l'onglet 3D) ; sa bulle
//!   dit « propre » ou « lié à la séquence N ».

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::decor3d::{cle_de_plan, empreinte_env, EnvPlan};

/// Ce qu'on demande pour l'environnement d'un plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChoixEnv {
    /// Revenir à celui de sa séquence.
    Sequence,
    /// Un environnement propre vide.
    Vide,
    /// Une copie retouchable de celui de sa séquence.
    Copie,
    /// Celui de la séquence `0`, lié.
    Lie(u64),
    /// Le lien devient une copie à lui.
    Detacher,
}

/// La marque d'un plan sorti de l'environnement de sa séquence : propre.
pub(crate) const ICONE_PROPRE: &str = ph::CUBE_FOCUS;
/// … lié à une autre séquence.
pub(crate) const ICONE_LIE: &str = ph::LINK_SIMPLE;

#[cfg(test)]
thread_local! {
    /// Les marques peintes à la dernière image : le plan (`Panel::id`), où, et sa bulle (essais).
    pub(crate) static MARQUES: std::cell::RefCell<Vec<(u64, egui::Rect, String)>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl App {
    /// L'environnement du plan `index` (rien : celui de sa séquence).
    pub(crate) fn env_du_plan(&self, index: usize) -> Option<EnvPlan> {
        self.doc.panels.get(index).and_then(|p| self.doc.decors3d.envs.get(&p.id).copied())
    }

    /// Le nom d'une séquence pour les phrases : « séquence 2 », ou « séquence 2 · La cuisine ».
    pub(crate) fn nom_court_de_sequence(&self, seq: u64) -> String {
        self.o3_nom_de_sequence(seq).replacen("Séquence", "séquence", 1)
    }

    /// La marque du bloc du plan `index` et sa bulle ; rien pour un plan qui partage l'environnement de sa séquence.
    pub(crate) fn marque_env(&self, index: usize) -> Option<(&'static str, String)> {
        match self.env_du_plan(index)? {
            EnvPlan::Propre { .. } => Some((ICONE_PROPRE, "Environnement 3D propre à ce plan (son décor, ses lampes, son cyclo).".to_string())),
            EnvPlan::Lie(s) => Some((ICONE_LIE, format!("Environnement 3D lié à la {} : ce qu'on y change se voit ici.", self.nom_court_de_sequence(s)))),
        }
    }

    /// Peint la marque du plan `index` à droite de son bloc `rect`, centrée sur la hauteur `cy` (la ligne du nom et de la durée) ;
    /// rend la zone de la marque et sa bulle. (Partie 205, vu à l'écran : en bas à droite, elle couvrait la fin de la durée
    /// « 2s 00 » ; le nom et la durée se rangent maintenant à sa gauche, `zone.left()`.)
    pub(crate) fn peindre_marque_env(&self, painter: &egui::Painter, rect: egui::Rect, index: usize, cy: f32) -> Option<(egui::Rect, String)> {
        let (icone, phrase) = self.marque_env(index)?;
        let taille = 12.0_f32.min(rect.height() - 8.0).max(7.0);
        let rayon = taille * 0.72;
        let c = egui::pos2(rect.right() - 7.0 - rayon, cy.clamp(rect.top() + rayon + 1.0, rect.bottom() - rayon - 1.0));
        let zone = egui::Rect::from_center_size(c, egui::vec2(rayon * 2.0 + 2.0, rayon * 2.0 + 2.0));
        painter.circle(c, rayon, crate::theme::panneau(), egui::Stroke::new(1.0_f32, crate::theme::accent().gamma_multiply(0.55)));
        painter.text(c, egui::Align2::CENTER_CENTER, icone, egui::FontId::proportional(taille), crate::theme::accent());
        #[cfg(test)]
        MARQUES.with(|m| m.borrow_mut().push((self.doc.panels[index].id, zone, phrase.clone())));
        Some((zone, phrase))
    }

    /// L'environnement propre du plan `index` a été retouché depuis qu'il a été fait (ou on ne le sait plus : après une ouverture,
    /// s'il a quelque chose).
    pub(crate) fn env_propre_retouche(&self, index: usize) -> bool {
        let Some(EnvPlan::Propre { base }) = self.env_du_plan(index) else { return false };
        let d = self.doc.decors3d.par_sequence.get(&cle_de_plan(self.doc.panels[index].id));
        match base {
            Some(b) => empreinte_env(d) != b,
            None => d.is_some_and(|d| !d.est_vide()),
        }
    }

    /// Demande `choix` pour le plan `index` ; « Revenir » sur un environnement propre retouché attend une confirmation
    /// (`env_a_jeter_window`).
    pub(crate) fn demander_env(&mut self, index: usize, choix: ChoixEnv) {
        if choix == ChoixEnv::Sequence && self.env_propre_retouche(index) {
            self.o3.env_a_jeter = self.doc.panels.get(index).map(|p| p.id);
            return;
        }
        self.changer_env(index, choix);
    }

    /// Fait `choix` pour le plan `index`, en une étape d'annulation (« Environnement du plan »). Rend vrai si quelque chose a changé.
    pub(crate) fn changer_env(&mut self, index: usize, choix: ChoixEnv) -> bool {
        let Some(pid) = self.doc.panels.get(index).map(|p| p.id) else { return false };
        let seq = self.doc.sequence_of(index);
        let avant = self.env_du_plan(index);
        let neuf = match choix {
            ChoixEnv::Sequence => None,
            ChoixEnv::Vide | ChoixEnv::Copie => Some(EnvPlan::Propre { base: None }),
            ChoixEnv::Lie(s) if s == seq => None,
            ChoixEnv::Lie(s) => Some(EnvPlan::Lie(s)),
            ChoixEnv::Detacher => match avant {
                Some(EnvPlan::Lie(_)) => Some(EnvPlan::Propre { base: None }),
                _ => return false,
            },
        };
        if avant == neuf && !matches!(choix, ChoixEnv::Vide | ChoixEnv::Copie) {
            return false;
        }
        if matches!(choix, ChoixEnv::Lie(s) if !self.doc.sequences.iter().any(|q| q.id == s)) {
            return false;
        }
        self.decor3d_retouche_finir();
        self.end_stroke();
        let snap = self.snapshot();
        let cle = cle_de_plan(pid);
        match choix {
            ChoixEnv::Sequence | ChoixEnv::Lie(_) => {
                self.doc.decors3d.par_sequence.remove(&cle);
            }
            ChoixEnv::Vide => {
                self.doc.decors3d.par_sequence.remove(&cle);
            }
            ChoixEnv::Copie => self.doc.copier_env(seq, cle, |p| p == pid),
            ChoixEnv::Detacher => {
                let Some(EnvPlan::Lie(s)) = avant else { unreachable!() };
                self.doc.copier_env(s, cle, |p| p == pid);
            }
        }
        match neuf {
            None => {
                self.doc.decors3d.envs.remove(&pid);
            }
            Some(EnvPlan::Propre { .. }) => {
                let base = empreinte_env(self.doc.decors3d.par_sequence.get(&cle));
                self.doc.decors3d.envs.insert(pid, EnvPlan::Propre { base: Some(base) });
            }
            Some(e) => {
                self.doc.decors3d.envs.insert(pid, e);
            }
        }
        // (Les objets choisis dans la vue étaient ceux de l'environnement d'avant.)
        self.espace3d.sel.retain(|&s| crate::poignees3d::objet_of(s).is_none());
        self.decor3d.choisi = None;
        let nom = match choix {
            ChoixEnv::Sequence => "Revenir à l'environnement de la séquence",
            ChoixEnv::Vide => "Environnement propre vide",
            ChoixEnv::Copie => "Environnement propre (copie)",
            ChoixEnv::Lie(_) => "Environnement lié",
            ChoixEnv::Detacher => "Détacher l'environnement",
        };
        self.commit_named(snap, nom);
        self.renderer.invalidate_all();
        self.status = match choix {
            ChoixEnv::Sequence => "Le plan partage de nouveau l'environnement de sa séquence (Ctrl+Z le rend).".into(),
            ChoixEnv::Vide => "Le plan a son environnement à lui, vide : ce qu'on y pose ne se voit que dans ce plan.".into(),
            ChoixEnv::Copie => "Le plan a son environnement à lui, une copie de celui de sa séquence : on peut le retoucher sans toucher aux autres plans.".into(),
            ChoixEnv::Lie(s) => format!("Le plan montre l'environnement de la {}, en direct.", self.nom_court_de_sequence(s)),
            ChoixEnv::Detacher => "Le lien est devenu un environnement à ce plan : ce qu'on change dans l'autre séquence ne s'y voit plus.".into(),
        };
        true
    }

    /// Le sous-menu « Environnement du plan » (le clic droit d'un bloc de plan au Storyboard).
    pub(crate) fn menu_env_du_plan(&mut self, ui: &mut egui::Ui, index: usize) {
        if index >= self.doc.panels.len() {
            return;
        }
        ui.menu_button("Environnement du plan", |ui| self.menu_env_items(ui, index));
    }

    /// Partie 205 : le clic droit d'un bloc de plan dans la timeline de la séquence (onglet 3D) : ses entrées directement, sous un
    /// titre (le menu n'avait qu'une entrée, ce sous-menu).
    pub(crate) fn menu_env_du_plan_a_plat(&mut self, ui: &mut egui::Ui, index: usize) {
        if index >= self.doc.panels.len() {
            return;
        }
        ui.label(RichText::new(format!("Environnement du {}", crate::storyboard::panel_title(&self.doc, index).to_lowercase())).weak());
        ui.separator();
        self.menu_env_items(ui, index);
    }

    /// Propriétés > Décor (onglet 3D) : « Environnement du plan : celui de la séquence / propre / lié à la séquence N », et ses
    /// commandes.
    pub(crate) fn env_du_plan_ui(&mut self, ui: &mut egui::Ui) {
        if self.doc.panels.is_empty() {
            return;
        }
        let i = self.cur_panel;
        let texte = match self.env_du_plan(i) {
            None => "celui de la séquence".to_string(),
            Some(EnvPlan::Propre { .. }) => "propre à ce plan".to_string(),
            Some(EnvPlan::Lie(s)) => format!("lié à la {}", self.nom_court_de_sequence(s)),
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Environnement du plan :").strong());
            ui.label(texte);
        });
        ui.horizontal_wrapped(|ui| {
            let r = ui.menu_button("Changer…", |ui| {
                self.menu_env_items(ui, i);
            });
            crate::bulles::bulle(r.response, "Environnement du plan", "Ce plan partage l'environnement 3D de sa séquence, ou en a un à lui (vide, ou une copie), ou montre celui d'une autre séquence (lié).", "");
            match self.env_du_plan(i) {
                Some(EnvPlan::Lie(_)) => {
                    let r = ui.button("Détacher");
                    if crate::bulles::bulle(r, "Détacher l'environnement", "Le lien devient une copie à ce plan : ce qu'on change dans l'autre séquence ne s'y voit plus.", "").clicked() {
                        self.demander_env(i, ChoixEnv::Detacher);
                    }
                }
                Some(EnvPlan::Propre { .. }) => {}
                None => {}
            }
            if self.env_du_plan(i).is_some() {
                let r = ui.button("Revenir à l'environnement de la séquence");
                if crate::bulles::bulle(r, "Revenir à l'environnement de la séquence", "Le plan partage de nouveau l'environnement de sa séquence (Ctrl+Z le rend).", "").clicked() {
                    self.demander_env(i, ChoixEnv::Sequence);
                }
            }
        });
    }

    /// Au-delà de ce nombre d'autres séquences, « Lié à la séquence N » va dans un sous-menu (le menu reste court, §82).
    const LIENS_A_PLAT: usize = 5;

    /// Les entrées du sous-menu (sans le titre). Partie 205 : l'état du plan d'abord (« Celui de la séquence », ou « Propre (celui-ci) »
    /// juste dessous, cochés), puis les autres choix ; plus de cinq autres séquences : « Lié à une autre séquence ▸ ».
    fn menu_env_items(&mut self, ui: &mut egui::Ui, index: usize) {
        let env = self.env_du_plan(index);
        let seq = self.doc.sequence_of(index);
        let mut choix = None;
        if ui.selectable_label(env.is_none(), "Celui de la séquence").clicked() {
            choix = Some(ChoixEnv::Sequence);
            ui.close_menu();
        }
        if matches!(env, Some(EnvPlan::Propre { .. })) && ui.selectable_label(true, "Propre (celui-ci)").clicked() {
            ui.close_menu();
        }
        if ui.selectable_label(false, "Propre : vide").clicked() {
            choix = Some(ChoixEnv::Vide);
            ui.close_menu();
        }
        if ui.selectable_label(false, "Propre : copie de celui de la séquence").clicked() {
            choix = Some(ChoixEnv::Copie);
            ui.close_menu();
        }
        let autres: Vec<(u64, String)> = self.doc.sequence_order().into_iter().filter(|&q| q != seq).map(|q| (q, format!("Lié à la {}", self.nom_court_de_sequence(q)))).collect();
        let mut liens = |ui: &mut egui::Ui, choix: &mut Option<ChoixEnv>| {
            for (q, nom) in &autres {
                if ui.selectable_label(env == Some(EnvPlan::Lie(*q)), nom.as_str()).clicked() {
                    *choix = Some(ChoixEnv::Lie(*q));
                    ui.close_menu();
                }
            }
        };
        if autres.len() > Self::LIENS_A_PLAT {
            ui.menu_button("Lié à une autre séquence", |ui| liens(ui, &mut choix));
        } else {
            liens(ui, &mut choix);
        }
        if matches!(env, Some(EnvPlan::Lie(_))) && ui.button("Détacher (en faire une copie à ce plan)").clicked() {
            choix = Some(ChoixEnv::Detacher);
            ui.close_menu();
        }
        if let Some(c) = choix {
            self.demander_env(index, c);
        }
    }

    /// « Revenir à l'environnement de la séquence ? » : l'environnement propre du plan a été retouché.
    pub(crate) fn env_a_jeter_window(&mut self, ctx: &egui::Context) {
        let Some(pid) = self.o3.env_a_jeter else { return };
        let Some(index) = self.doc.panels.iter().position(|p| p.id == pid) else {
            self.o3.env_a_jeter = None;
            return;
        };
        let (mut oui, mut non) = (false, false);
        let modal = egui::Modal::new(egui::Id::new("env_a_jeter")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading("Revenir à l'environnement de la séquence ?");
            ui.add_space(6.0);
            ui.label("L'environnement propre de ce plan a été retouché : il sera retiré, et le plan montrera celui de sa séquence.");
            ui.label(RichText::new("Ctrl+Z le rend.").weak());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                oui = ui.button("Revenir à la séquence").clicked();
                non = ui.button("Annuler").clicked();
            });
        });
        if oui {
            self.o3.env_a_jeter = None;
            self.changer_env(index, ChoixEnv::Sequence);
        } else if non || modal.should_close() {
            self.o3.env_a_jeter = None;
        }
    }

    /// Partie 204 : supprimer les plans `ids` viderait des séquences auxquelles d'autres plans sont liés : la liste de ces séquences.
    pub(crate) fn sequences_liees_videes(&self, ids: &[u64]) -> Vec<u64> {
        // (Partie 205 : seuls les plans du film comptent ; un plan lié qui est dans la corbeille devient une copie sans question.)
        let liees: std::collections::BTreeSet<u64> = self.doc.decors3d.envs.iter().filter(|(p, _)| !ids.contains(p) && self.doc.panels.iter().any(|q| q.id == **p)).filter_map(|(_, e)| if let EnvPlan::Lie(s) = e { Some(*s) } else { None }).collect();
        liees
            .into_iter()
            .filter(|&s| {
                let plans: Vec<u64> = (0..self.doc.panels.len()).filter(|&i| self.doc.sequence_of(i) == s).map(|i| self.doc.panels[i].id).collect();
                !plans.is_empty() && plans.iter().all(|p| ids.contains(p))
            })
            .collect()
    }

    /// « Supprimer une séquence à laquelle d'autres plans sont liés ? » : oui, leurs liens deviennent des copies (`normalize`).
    pub(crate) fn suppression_liee_window(&mut self, ctx: &egui::Context) {
        let Some(ids) = self.o3.suppression_liee.clone() else { return };
        let seqs = self.sequences_liees_videes(&ids);
        if seqs.is_empty() {
            self.o3.suppression_liee = None;
            return;
        }
        let noms: Vec<String> = seqs.iter().map(|&s| self.nom_court_de_sequence(s)).collect();
        let (mut oui, mut non) = (false, false);
        let modal = egui::Modal::new(egui::Id::new("suppression_liee")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.heading("Supprimer une séquence dont l'environnement sert ailleurs ?");
            ui.add_space(6.0);
            ui.label(format!("D'autres plans montrent l'environnement de la {}. Ils en garderont chacun une copie à eux.", noms.join(", de la ")));
            ui.label(RichText::new("Ctrl+Z rend la séquence et les liens.").weak());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                oui = ui.button("Supprimer").clicked();
                non = ui.button("Annuler").clicked();
            });
        });
        if oui {
            self.o3.suppression_liee = None;
            self.delete_panels_confirmes(&ids);
        } else if non || modal.should_close() {
            self.o3.suppression_liee = None;
        }
    }
}
