//! Brosses dédiées : **avec quoi dessine une brosse dédiée** (§107 de `docs/INTERFACE.md`, Anthony, 2026-11-05 : « elles doivent toutes par
//! défaut être alignées sur la brosse d'origine (pinceau). Les options permettent d'assigner une brosse spécifique (pinceau, crayon, feutre,
//! taille, texture...) »).
//!
//! # Deux modes, pour les trois types (Personnage, Classique, Décor)
//!
//! - **Suit le pinceau** (d'origine) : la brosse ne garde que son rôle et ses couleurs (trait, aplat). Elle dessine avec **le pinceau courant de
//!   l'utilisateur**, tel qu'il l'a réglé (forme, taille, texture, opacité) ; s'il change de pinceau ou de taille, elle suit. La choisir ou la
//!   lâcher ne touche jamais à ce réglage : ici il n'y a rien à rendre.
//! - **Brosse spécifique** (`personnages::Specifique`, un champ facultatif `brosse` de la brosse) : un outil (pinceau ou crayon) et un réglage
//!   complet, à elle. Choisie, elle **prend** le pinceau (ou le crayon) à la place de celui de l'utilisateur, qui est mis de côté (`Pris`) ;
//!   lâchée (clic sur elle, autre brosse qui suit le pinceau, couleur prise à la main, suppression…), elle le **rend tel qu'il était**. Entre
//!   deux brosses spécifiques, rien n'est rendu ni repris : le pinceau de l'utilisateur reste de côté. Ce que l'utilisateur règle pendant
//!   qu'elle est choisie (taille avec les crochets, panneau Réglages de l'outil, choix d'une brosse enregistrée) est **gardé pour elle** et
//!   non pour son pinceau : c'est le même éditeur que d'habitude, sans second éditeur ; `suivre_sorte_pinceau` (le pinceau retenu par sorte de
//!   calque) attend qu'elle soit lâchée, pour ne pas retenir son réglage comme celui de l'utilisateur.
//!
//! Sans latence : rien ne se calcule ici, une copie de réglage au choix, une comparaison par image. Le choix d'une brosse est instantané.

use eframe::egui;

use crate::app::{App, Tool};
use crate::brush::BrushSettings;
use crate::brushtex::Target;
use crate::personnages::Specifique;
use crate::personnagesui::Choix;

/// Ce que le choix d'une brosse spécifique a pris à l'utilisateur et lui rendra : l'outil, et le pinceau et le crayon d'avant.
#[derive(Clone, Debug)]
pub(crate) struct Pris {
    pub tool: Tool,
    pub brush: BrushSettings,
    pub pencil: BrushSettings,
}

/// Ce que la brosse spécifique a posé : l'outil (vrai : crayon) et le réglage, pour voir quand l'éditeur, l'annulation ou l'utilisateur le changent.
pub(crate) type Pose = (bool, BrushSettings);

impl App {
    /// La brosse spécifique de ce choix ; rien : elle suit le pinceau.
    pub(crate) fn dediee_specifique(&self, c: Choix) -> Option<Specifique> {
        match c {
            Choix::Personnage(id) => self.doc.personnages.iter().find(|p| p.id == id).and_then(|p| p.brosse.clone()),
            Choix::Classique(i) => self.settings.brosses_classiques.get(i).and_then(|b| b.specifique.clone()),
            Choix::Decor(i) => self.settings.brosses_decor.get(i).and_then(|b| b.specifique.clone()),
        }
    }

    /// Cette brosse est-elle spécifique, et avec quel outil (vrai : crayon) ? Sans copier le réglage (les pastilles, à chaque image).
    pub(crate) fn dediee_outil(&self, c: Choix) -> Option<bool> {
        match c {
            Choix::Personnage(id) => self.doc.personnages.iter().find(|p| p.id == id).and_then(|p| p.brosse.as_ref().map(|s| s.crayon)),
            Choix::Classique(i) => self.settings.brosses_classiques.get(i).and_then(|b| b.specifique.as_ref().map(|s| s.crayon)),
            Choix::Decor(i) => self.settings.brosses_decor.get(i).and_then(|b| b.specifique.as_ref().map(|s| s.crayon)),
        }
    }

    /// Ce qui dessine, en mots : « Suit le pinceau » ou le nom de la brosse spécifique, avec sa taille et son outil (pastilles, bulles).
    pub(crate) fn dediee_etiquette(&self, c: Choix) -> String {
        match self.dediee_specifique(c) {
            None => "Suit le pinceau".to_string(),
            Some(s) => format!("{}, {:.0} px, {}", s.libelle(&self.brushes.presets), s.settings.size, if s.crayon { "crayon" } else { "pinceau" }),
        }
    }

    /// Pose (ou retire, `None` : elle suit le pinceau) la brosse spécifique de ce choix. Un personnage change le projet (l'appelant a
    /// enregistré l'étape d'annulation) ; une classique ou un décor change un réglage de l'application.
    pub(crate) fn dediee_ecrire(&mut self, c: Choix, s: Option<Specifique>) {
        match c {
            Choix::Personnage(id) => {
                if let Some(p) = self.doc.personnages.iter_mut().find(|p| p.id == id) {
                    if p.brosse != s {
                        p.brosse = s;
                        self.modified = true;
                    }
                }
            }
            Choix::Classique(i) | Choix::Decor(i) => {
                let liste = if matches!(c, Choix::Decor(_)) { &mut self.settings.brosses_decor } else { &mut self.settings.brosses_classiques };
                if let Some(b) = liste.get_mut(i) {
                    if b.specifique != s {
                        b.specifique = s;
                        self.settings_a_ecrire = true;
                    }
                }
            }
        }
    }

    /// Le réglage vivant de l'outil de la brosse spécifique.
    fn dediee_vivant(&self, crayon: bool) -> BrushSettings {
        if crayon {
            self.pencil
        } else {
            self.brush
        }
    }

    /// Le pinceau de l'utilisateur, tel qu'il est vraiment : celui mis de côté si une brosse spécifique est choisie, sinon le courant. (Le
    /// point de départ d'une brosse qui devient spécifique ; jamais le réglage d'une autre brosse spécifique.) Rend (crayon, réglage).
    fn dediee_pinceau_utilisateur(&self) -> (bool, BrushSettings) {
        match &self.perso.pris {
            Some(p) => (p.tool == Tool::Pencil, if p.tool == Tool::Pencil { p.pencil } else { p.brush }),
            None => (self.tool == Tool::Pencil, if self.tool == Tool::Pencil { self.pencil } else { self.brush }),
        }
    }

    /// Une fois par image, et à chaque choix de brosse : la brosse choisie pose sa brosse spécifique, ou rend le pinceau de l'utilisateur
    /// quand elle n'en a pas (ou quand plus aucune brosse n'est choisie) ; et ce que l'utilisateur règle pendant ce temps est gardé pour elle.
    /// `forcer_outil` : au choix, l'outil devient celui de la brosse (pinceau ou crayon).
    pub(crate) fn dediee_suivre(&mut self, forcer_outil: bool) {
        let voulu = self.perso.actif.and_then(|c| self.dediee_specifique(c).map(|s| (c, s)));
        let Some((c, s)) = voulu else {
            if self.perso.pris.is_some() {
                self.dediee_rendre();
            }
            return;
        };
        if self.perso.pose != Some((s.crayon, s.settings)) {
            self.dediee_poser(&s, forcer_outil);
            return;
        }
        // Posée : si l'utilisateur a réglé son outil (taille aux crochets, panneau Réglages de l'outil, brosse enregistrée), c'est pour elle.
        let outil_ok = if s.crayon { self.tool == Tool::Pencil } else { self.tool == Tool::Brush };
        let vivant = self.dediee_vivant(s.crayon);
        if outil_ok && vivant != s.settings {
            let mut n = s;
            n.settings = vivant;
            self.perso.pose = Some((n.crayon, vivant));
            self.dediee_ecrire(c, Some(n));
        }
    }

    /// Prend l'outil et le réglage de la brosse spécifique `s` ; l'utilisateur garde les siens de côté (une seule fois, tant qu'une brosse
    /// spécifique est choisie).
    fn dediee_poser(&mut self, s: &Specifique, forcer_outil: bool) {
        self.end_stroke();
        let ancien = self.perso.pose.as_ref().map(|p| p.0);
        match &self.perso.pris {
            None => self.perso.pris = Some(Pris { tool: self.tool, brush: self.brush, pencil: self.pencil }),
            Some(p) => {
                // (Le pinceau devient un crayon, ou l'inverse : celui qu'elle tenait est rendu.)
                if let Some(vieux) = ancien {
                    if vieux != s.crayon {
                        if vieux {
                            self.pencil = p.pencil;
                        } else {
                            self.brush = p.brush;
                        }
                    }
                }
            }
        }
        if s.crayon {
            self.pencil = s.settings;
        } else {
            self.brush = s.settings;
        }
        if forcer_outil || ancien != Some(s.crayon) {
            self.set_tool(if s.crayon { Tool::Pencil } else { Tool::Brush });
        }
        self.perso.pose = Some((s.crayon, s.settings));
        self.brush_previews.memory.clear();
    }

    /// Rend le pinceau de l'utilisateur tel qu'il était (rien à rendre : rien ne se passe). L'outil revient aussi s'il est encore celui que
    /// la brosse avait pris.
    pub(crate) fn dediee_rendre(&mut self) {
        let pose = self.perso.pose.take();
        let Some(p) = self.perso.pris.take() else { return };
        self.end_stroke();
        if let Some((crayon, _)) = pose {
            if crayon {
                self.pencil = p.pencil;
            } else {
                self.brush = p.brush;
            }
            if self.tool == if crayon { Tool::Pencil } else { Tool::Brush } {
                self.set_tool(p.tool);
            }
        }
        self.brush_previews.memory.clear();
    }

    /// Les réglages « avec quoi elle dessine » d'une brosse dédiée : « Suit le pinceau » ou « Brosse spécifique » et, dans ce cas, l'outil, la
    /// brosse (le pinceau, le crayon, le feutre… ou toute brosse enregistrée), la taille et la texture. Dans le panneau, dans la fenêtre
    /// « Modifier » du clic droit.
    pub(crate) fn dediee_reglages(&mut self, ui: &mut egui::Ui, c: Choix) {
        let actuel = self.dediee_specifique(c);
        let mut nouveau: Option<Option<Specifique>> = None;
        let texte = if actuel.is_some() { "Brosse spécifique" } else { "Suit le pinceau" };
        let depart = self.dediee_pinceau_utilisateur();
        egui::ComboBox::from_id_salt(("dediee_mode", format!("{c:?}")))
            .width(128.0)
            .selected_text(texte)
            .show_ui(ui, |ui| {
                let r = ui.selectable_label(actuel.is_none(), "Suit le pinceau").on_hover_text("Dessine avec le pinceau courant : sa forme, sa taille, sa texture et son opacité, tels que tu les as réglés. Si tu changes de pinceau ou de taille, elle suit.");
                if r.clicked() && actuel.is_some() {
                    nouveau = Some(None);
                }
                let r = ui.selectable_label(actuel.is_some(), "Brosse spécifique").on_hover_text("Dessine toujours avec sa propre brosse (pinceau, crayon, feutre ou une brosse enregistrée, sa taille, sa texture), sans toucher à ton pinceau : il revient tel quel quand tu la lâches.");
                if r.clicked() && actuel.is_none() {
                    // (Elle part du pinceau de l'utilisateur : rien ne change tant qu'on ne la règle pas.)
                    nouveau = Some(Some(Specifique { crayon: depart.0, settings: depart.1, nom: None }));
                }
            })
            .response
            .on_hover_text("Avec quoi cette brosse dessine : le pinceau courant (elle le suit), ou sa propre brosse.");
        let mut etape = false;
        let mut s = match (&nouveau, actuel.clone()) {
            (Some(n), _) => n.clone(),
            (None, a) => a,
        };
        let mut change = nouveau.is_some();
        etape |= change;
        if let Some(sp) = s.as_mut() {
            let avant = sp.clone();
            // L'outil.
            let mut crayon = sp.crayon;
            ui.selectable_value(&mut crayon, false, "Pinceau").on_hover_text("L'outil Pinceau.");
            ui.selectable_value(&mut crayon, true, "Crayon").on_hover_text("L'outil Crayon.");
            sp.crayon = crayon;
            // La brosse : celles de la bibliothèque (fournies, importées, les miennes).
            let dossiers: Vec<(String, Vec<(usize, String)>)> = crate::pinceaux::ranger(&self.brushes.presets, Target::Brush, "")
                .into_iter()
                .map(|d| (d.nom.clone(), d.rangs.iter().map(|&r| (r, self.brushes.presets[r].name.clone())).collect()))
                .collect();
            let mut prise: Option<usize> = None;
            egui::ComboBox::from_id_salt(("dediee_brosse", format!("{c:?}")))
                .width(150.0)
                .selected_text(sp.libelle(&self.brushes.presets))
                .show_ui(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        for (dossier, rangs) in &dossiers {
                            ui.label(egui::RichText::new(dossier).small().weak());
                            for (rang, nom) in rangs {
                                let choisie = sp.nom.as_deref() == Some(nom.as_str()) && sp.settings == self.brushes.presets[*rang].settings;
                                if ui.selectable_label(choisie, nom).clicked() {
                                    prise = Some(*rang);
                                }
                            }
                        }
                    });
                })
                .response
                .on_hover_text("La brosse de cette brosse dédiée : un pinceau, un crayon, un feutre ou n'importe quelle brosse enregistrée.");
            if let Some(rang) = prise {
                sp.settings = self.brushes.presets[rang].settings;
                sp.nom = Some(self.brushes.presets[rang].name.clone());
            }
            // La taille et la texture.
            let resp = ui.add(egui::DragValue::new(&mut sp.settings.size).range(0.5..=400.0).speed(0.3).max_decimals(0).suffix(" px"));
            let resp = crate::bulles::bulle(resp, "Taille", "La taille de la brosse spécifique, en pixels du dessin.", "");
            if self.perso_edited(&resp, c) {
                change = true;
            }
            let resp = ui.add(egui::DragValue::new(&mut sp.settings.grain).range(0.0..=1.0).speed(0.01).fixed_decimals(2).prefix("Texture "));
            let resp = crate::bulles::bulle(resp, "Texture", "La force du grain du papier de la brosse : 0, lisse ; 1, très texturé. (Les autres réglages de la pointe et du papier : choisis cette brosse, puis règle le pinceau comme d'habitude, ses réglages sont gardés pour elle.)", "");
            if self.perso_edited(&resp, c) {
                change = true;
            }
            if *sp != avant {
                change = true;
                // (Un choix de liste ou de bouton n'a pas de geste : l'étape d'annulation est à poser avant l'écriture ; une taille ou une texture
                // tirées l'ont posée avec `perso_edited`.)
                etape |= prise.is_some() || crayon != avant.crayon;
            }
        }
        if change {
            if etape && matches!(c, Choix::Personnage(_)) {
                self.perso_etape("Modifier la brosse du personnage");
            }
            self.dediee_ecrire(c, s);
            self.dediee_suivre(false);
        }
    }
}
