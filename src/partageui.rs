//! La planche-mère à l'écran (`partage.rs` : le modèle) : dans le menu d'un calque, le sous-menu **« Réutiliser dans… »** (le plan suivant,
//! le précédent, toute la séquence, des plans choisis… ; et **« Détacher »** quand il est partagé) ; dans les menus des plans,
//! **« Réutiliser le plan »** et **« Détacher le plan »** ; la petite fenêtre des plans à cocher ; le maillon dans la ligne du calque
//! (`layersui.rs`) ; la ligne d'état. Le mot « planche-mère » ne sert que dans les bulles et la ligne d'état (Anthony : rien de plus dans
//! le menu Aide).

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::partage::Cibles;
use crate::storyboard::panel_title;

/// La fenêtre « Réutiliser le calque dans… » : le calque visé et, pour chaque plan du film (son numéro), s'il doit en avoir un exemplaire.
pub(crate) struct Choix {
    pub(crate) pi: usize,
    pub(crate) li: usize,
    pub(crate) plans: Vec<(u64, bool)>,
}

/// Pourquoi un calque ne se partage pas.
pub(crate) const PAS_PARTAGEABLE: &str = "Un dossier, un calque de couleur, un calque vidéo, un calque d'effet ou un calque de toute la scène ne se partage pas : seul un calque de dessin fait une planche-mère.";

impl App {
    /// « Plan 12, Plan 13 » : les plans `rangs`.
    fn noms_des_plans(&self, rangs: &[usize]) -> String {
        rangs.iter().map(|&i| panel_title(&self.doc, i)).collect::<Vec<_>>().join(", ")
    }

    /// Le calque courant réutilisé dans le plan suivant, le précédent, ou tous les plans de la séquence (`calque.reutiliser_*`) : une étape.
    pub(crate) fn reutiliser_calque_vers(&mut self, quoi: Cibles) {
        if self.content_refuses() {
            return;
        }
        let (pi, li) = (self.cur_panel, self.cur_layer);
        if !self.layers().get(li).is_some_and(crate::document::Layer::partageable) {
            self.status = PAS_PARTAGEABLE.into();
            return;
        }
        let cibles = self.doc.cibles(pi, quoi);
        if cibles.is_empty() {
            self.status = match quoi {
                Cibles::Suivant => "Pas de plan après celui-ci.".into(),
                Cibles::Precedent => "Pas de plan avant celui-ci.".into(),
                Cibles::Sequence => "La séquence n'a pas d'autre plan.".into(),
            };
            return;
        }
        let snap = self.snapshot();
        if self.doc.reutiliser_calque(pi, li, &cibles).is_none() {
            return;
        }
        self.doc.sync_partages(pi);
        self.apres_partage(snap, "Réutiliser le calque");
        let nom = self.layers()[li].name.clone();
        let plans = self.doc.plans_du_partage(pi, li).into_iter().filter(|&i| i != pi).collect::<Vec<_>>();
        self.status = format!("« {nom} » est partagé avec {} : dessiner dans l'un, c'est dessiner dans tous, chacun sous sa caméra (planche-mère).", self.noms_des_plans(&plans));
    }

    /// Ouvre la fenêtre des plans à cocher pour le calque courant (`calque.reutiliser_choisir`).
    pub(crate) fn ouvrir_choix_reutiliser(&mut self) {
        let (pi, li) = (self.cur_panel, self.cur_layer);
        if !self.layers().get(li).is_some_and(crate::document::Layer::partageable) {
            self.status = PAS_PARTAGEABLE.into();
            return;
        }
        let lies = self.doc.plans_du_partage(pi, li);
        let plans = self.doc.panels.iter().enumerate().map(|(i, p)| (p.id, i == pi || lies.contains(&i))).collect();
        self.reutiliser = Some(Choix { pi, li, plans });
    }

    /// Le calque courant quitte sa planche-mère, avec une copie de ses dessins (`calque.detacher_partage`) : une étape.
    pub(crate) fn detacher_calque_partage(&mut self) {
        if self.content_refuses() {
            return;
        }
        let (pi, li) = (self.cur_panel, self.cur_layer);
        let snap = self.snapshot();
        if !self.doc.detacher_partage(pi, li) {
            self.status = "Ce calque n'est pas partagé.".into();
            return;
        }
        self.doc.sync_partages(pi);
        self.apres_partage(snap, "Détacher le calque");
        self.status = format!("« {} » est détaché : il garde une copie du dessin, les autres plans gardent le leur.", self.layers()[li].name);
    }

    /// « Réutiliser le plan » (`plan.reutiliser`) : un plan lié juste après, à cadrer avec sa caméra.
    pub(crate) fn reutiliser_plan(&mut self) {
        if self.structure_op_dessins(|d, i| Some(d.reutiliser_plan_apres(i))) {
            self.doc.sync_partages(self.cur_panel);
            self.status = format!("{} réutilisé : même dessin, à cadrer avec sa caméra (planche-mère). « Détacher le plan » le rend indépendant.", panel_title(&self.doc, self.cur_panel));
        }
    }

    /// « Détacher le plan » (`plan.detacher`) : tous ses calques partagés gardent une copie ; une étape.
    pub(crate) fn detacher_plan_courant(&mut self) {
        if self.content_refuses() {
            return;
        }
        let pi = self.cur_panel;
        let snap = self.snapshot();
        let n = self.doc.detacher_plan(pi);
        if n == 0 {
            self.status = "Ce plan ne partage aucun calque.".into();
            return;
        }
        self.doc.sync_partages(pi);
        self.apres_partage(snap, "Détacher le plan");
        self.status = format!("{} détaché : {n} calque{} garde{} une copie, les autres plans gardent le leur.", panel_title(&self.doc, pi), if n > 1 { "s" } else { "" }, if n > 1 { "nt" } else { "" });
    }

    /// Une étape d'annulation `nom` depuis `snap`, et tout se redessine.
    fn apres_partage(&mut self, snap: crate::history::Edit, nom: &str) {
        self.modified = true;
        self.renderer.invalidate_all();
        self.commit_named(snap, nom);
    }

    /// Le sous-menu « Réutiliser dans… » du menu d'un calque (`menucalque.rs`).
    pub(crate) fn reutiliser_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let partage = self.layers().get(li).is_some_and(|l| l.partage.is_some());
        let titre = if partage { format!("{}  Réutiliser dans…  (partagé)", ph::LINK) } else { format!("{}  Réutiliser dans…", ph::LINK) };
        ui.menu_button(titre, |ui| {
            if !self.layers().get(li).is_some_and(crate::document::Layer::partageable) {
                ui.label(RichText::new(PAS_PARTAGEABLE).small().weak());
                return;
            }
            if partage {
                let autres: Vec<usize> = self.doc.plans_du_partage(self.cur_panel, li).into_iter().filter(|&i| i != self.cur_panel).collect();
                ui.label(RichText::new(format!("Même dessin que {} (planche-mère).", self.noms_des_plans(&autres))).small().weak());
                ui.separator();
            }
            self.entree_action(ui, li, "", "Le plan suivant", "calque.reutiliser_suivant");
            self.entree_action(ui, li, "", "Le plan précédent", "calque.reutiliser_precedent");
            self.entree_action(ui, li, "", "Tous les plans de la séquence", "calque.reutiliser_sequence");
            self.entree_action(ui, li, "", "Des plans choisis…", "calque.reutiliser_choisir");
            if partage {
                ui.separator();
                self.entree_action(ui, li, ph::LINK_BREAK, "Détacher (garder une copie)", "calque.detacher_partage");
            }
        });
    }

    /// La fenêtre des plans à cocher (`ouvrir_choix_reutiliser`). « Appliquer » lie les plans cochés et détache les décochés.
    pub(crate) fn reutiliser_window(&mut self, ctx: &egui::Context) {
        let Some(mut choix) = self.reutiliser.take() else { return };
        if self.doc.panels.get(choix.pi).and_then(|p| p.layers.get(choix.li)).is_none() {
            return;
        }
        let nom = self.doc.panels[choix.pi].layers[choix.li].name.clone();
        let mut ouvert = true;
        let mut appliquer = false;
        let mut fermer = false;
        egui::Window::new(format!("Réutiliser « {nom} » dans…")).id(egui::Id::new("reutiliser_calque")).collapsible(false).resizable(false).open(&mut ouvert).show(ctx, |ui| {
            ui.set_width(300.0);
            ui.label(RichText::new("Les plans cochés montrent le même dessin que ce calque, chacun sous sa caméra (planche-mère). Décocher un plan le détache : il garde une copie.").small().weak());
            egui::ScrollArea::vertical().id_salt("reutiliser_plans").max_height(260.0).show(ui, |ui| {
                for (id, coche) in &mut choix.plans {
                    let Some(i) = self.doc.panels.iter().position(|p| p.id == *id) else { continue };
                    let texte = panel_title(&self.doc, i);
                    if i == choix.pi {
                        ui.add_enabled(false, egui::Checkbox::new(coche, format!("{texte} (ce calque)")));
                    } else {
                        ui.checkbox(coche, texte);
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Appliquer").clicked() {
                    appliquer = true;
                }
                if ui.button("Fermer").clicked() {
                    fermer = true;
                }
            });
        });
        if appliquer {
            self.appliquer_choix_reutiliser(&choix);
        }
        if ouvert && !appliquer && !fermer {
            self.reutiliser = Some(choix);
        }
    }

    /// Les plans cochés reçoivent un exemplaire, les décochés qui en avaient un sont détachés ; une étape.
    pub(crate) fn appliquer_choix_reutiliser(&mut self, choix: &Choix) {
        if self.content_refuses() {
            return;
        }
        let (pi, li) = (choix.pi, choix.li);
        let rang = |doc: &crate::document::Document, id: u64| doc.panels.iter().position(|p| p.id == id);
        let coches: Vec<usize> = choix.plans.iter().filter(|(_, c)| *c).filter_map(|(id, _)| rang(&self.doc, *id)).filter(|&i| i != pi).collect();
        let decoches: Vec<usize> = choix.plans.iter().filter(|(_, c)| !*c).filter_map(|(id, _)| rang(&self.doc, *id)).collect();
        let lies = self.doc.plans_du_partage(pi, li);
        let a_lier: Vec<usize> = coches.iter().copied().filter(|i| !lies.contains(i)).collect();
        let a_detacher: Vec<usize> = decoches.iter().copied().filter(|i| lies.contains(i)).collect();
        if a_lier.is_empty() && a_detacher.is_empty() {
            self.status = "Rien à changer.".into();
            return;
        }
        let snap = self.snapshot();
        if !a_lier.is_empty() {
            self.doc.reutiliser_calque(pi, li, &a_lier);
        }
        if let Some(cle) = self.doc.panels[pi].layers[li].partage {
            for p in a_detacher {
                if let Some(k) = self.doc.panels[p].layers.iter().position(|l| l.partage == Some(cle)) {
                    self.doc.detacher_partage(p, k);
                }
            }
        }
        self.doc.sync_partages(pi);
        self.apres_partage(snap, "Réutiliser le calque");
        let plans: Vec<usize> = self.doc.plans_du_partage(pi, li).into_iter().filter(|&i| i != pi).collect();
        let nom = self.doc.panels[pi].layers[li].name.clone();
        self.status = if plans.is_empty() {
            format!("« {nom} » n'est plus partagé.")
        } else {
            format!("« {nom} » est partagé avec {} (planche-mère).", self.noms_des_plans(&plans))
        };
    }

    /// Le maillon d'une ligne de calque partagé (`layersui.rs`) : combien de plans le partagent, et lesquels en bulle.
    pub(crate) fn maillon_partage(&self, ui: &mut egui::Ui, li: usize) {
        let autres: Vec<usize> = self.doc.plans_du_partage(self.cur_panel, li).into_iter().filter(|&i| i != self.cur_panel).collect();
        if autres.is_empty() {
            return;
        }
        ui.label(RichText::new(format!("{} {}", ph::LINK, autres.len())).small().weak()).on_hover_text(format!(
            "Planche-mère : ce calque montre le même dessin que {} (dessiner dans l'un, c'est dessiner dans tous ; chaque plan le cadre avec sa caméra). Clic droit > Réutiliser dans… pour changer, Détacher pour reprendre une copie.",
            self.noms_des_plans(&autres)
        ));
    }
}
