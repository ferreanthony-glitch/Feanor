//! Partie 202 : les cyclos dans l'interface (`cyclos.rs`, `docs/INTERFACE.md` partie 202).
//!
//! - **Propriétés > Monde** (onglet 3D), section « Cyclo » : la galerie des choix (leurs vignettes, faites par le code et gardées),
//!   « Aucun » (l'aspect d'avant), puis les réglages : l'heure, les nuages, la brume, la rotation, la graine (« Autre »), la couleur
//!   dominante. Un choix est une étape d'annulation (« Cyclo ») ; un réglage tiré, une étape au lâcher (la retouche du décor).
//! - **Scène 3D** : sous Lumières, la ligne « Cyclo » (l'ancien « Ciel ») mène à cette section.
//! - **Le Storyboard** : dans la barre au-dessus du dessin, à côté de « Décor » (partie 199), « Cyclo : … » et la liste des choix.
//! - Partie 203 : « Dessiner mon cyclo… » (« Partir de » : Vide ou un choix), « Mon cyclo » et « Redessiner » (`cyclodessin.rs`) ;
//!   un choix qui retirerait le cyclo dessiné le demande d'abord (« Retirer le cyclo dessiné ? », Ctrl+Z le rend).
//! - Avec un cyclo, le soleil du décor est le sien (sa hauteur à l'heure du cyclo, son azimut et sa rotation) : ses ombres (partie
//!   185) suivent. « Aucun » retrouve le soleil du décor tel qu'il était.

use eframe::egui::{self, vec2, RichText};

use crate::app::App;
use crate::cyclodessin::Depart;
use crate::cyclos::{Cyclo, Preset};
use crate::decor3d::Decor3d;

/// Ce qu'on demande au cyclo de la séquence (partie 203 : un choix qui retirerait le dessin attend sa confirmation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ActionCyclo {
    /// Un cyclo du choix, ou aucun.
    Choisir(Option<Cyclo>),
    /// Un dessin neuf (« Dessiner mon cyclo… »).
    Dessiner(Depart),
    /// Le dessin de la séquence, à reprendre (« Redessiner »).
    Redessiner,
}

/// La case des cartes (Propriétés, la carte choisie).
pub(crate) const NOM_PREND: &str = "Prend la lumière du cyclo";
pub(crate) const PHRASE_PREND: &str = "La carte de ce calque prend la lumière du cyclo de la séquence, comme le décor : à peine changée en plein jour, assombrie et bleutée la nuit, réchauffée au coucher du soleil ; les ombres et les lampes s'y ajoutent. Décoché : elle reste telle qu'elle est dessinée.";

/// La taille des vignettes de la galerie, à l'écran (points).
const VIGNETTE_PTS: [f32; 2] = [96.0, 40.0];

/// Le cyclo `c` (rien : aucun) dans le décor `d`. Le soleil du décor suit alors celui du cyclo (`Aspect::soleil`), sans toucher au
/// sien : « Aucun » le retrouve tel quel. Un cyclo choisi allume le ciel du décor (son œil, dans Scène 3D).
pub(crate) fn poser(d: &mut Decor3d, c: Option<Cyclo>) {
    if let Some(c) = c {
        if d.cyclo.is_none() {
            d.aspect.ciel = true;
        }
        d.cyclo = Some(c.borne());
        // (Partie 203 : un cyclo du choix n'a pas de dessin.)
        if c.dessin.is_none() {
            d.dessin_cyclo = None;
        }
    } else {
        d.cyclo = None;
        d.dessin_cyclo = None;
    }
}

impl App {
    /// Le cyclo de la séquence du plan courant.
    pub(crate) fn cyclo_courant(&self) -> Option<Cyclo> {
        if self.doc.panels.is_empty() {
            return None;
        }
        self.doc.decors3d.par_sequence.get(&self.doc.env_cle(self.cur_panel)).and_then(|d| d.cyclo)
    }

    /// Le décor de la séquence du plan courant avec le cyclo `c` (rien : aucun) et le soleil qu'il donne.
    fn decor_avec_cyclo(&self, c: Option<Cyclo>) -> (u64, Decor3d) {
        let seq = self.doc.env_cle(self.cur_panel);
        let mut d = self.doc.decors3d.par_sequence.get(&seq).cloned().unwrap_or_else(|| Decor3d::new(Default::default()));
        poser(&mut d, c);
        (seq, d)
    }

    /// Le cyclo `c` (rien : aucun) pour la séquence du plan courant, en une étape d'annulation nommée `nom`.
    pub(crate) fn changer_cyclo(&mut self, nom: &str, c: Option<Cyclo>) {
        if self.doc.panels.is_empty() || self.cyclo_courant() == c.map(Cyclo::borne) {
            return;
        }
        self.decor3d_retouche_finir();
        let snap = self.snapshot();
        let (seq, d) = self.decor_avec_cyclo(c);
        self.decor3d_appliquer(seq, d);
        self.commit_named(snap, nom);
        self.renderer.invalidate_all();
        self.status = match c {
            Some(c) => format!("Cyclo « {} » pour la séquence.", c.preset.nom()),
            None => "Plus de cyclo : le ciel d'avant.".into(),
        };
    }

    /// Un réglage du cyclo qu'on tire : la retouche du décor (une étape au lâcher, `decor3d_retouche_finir`).
    pub(crate) fn retoucher_cyclo(&mut self, c: Cyclo) {
        if self.doc.panels.is_empty() || self.cyclo_courant() == Some(c.borne()) {
            return;
        }
        let (seq, d) = self.decor_avec_cyclo(Some(c));
        self.decor3d_retoucher(seq, d);
        self.renderer.invalidate_all();
    }

    /// La vignette d'un choix, en texture d'egui (faite une fois, gardée).
    fn texture_de_vignette(ctx: &egui::Context, p: Preset) -> egui::TextureHandle {
        let id = egui::Id::new(("vignette_cyclo", p));
        if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
            return t;
        }
        let v = crate::cyclos::vignette(p);
        // (Le haut du ciel n'apprend rien : la bande de l'horizon, ses silhouettes et un peu de sol.)
        let (h0, h1) = (v.hauteur * 3 / 16, v.hauteur * 5 / 8);
        let w = v.largeur;
        let img = egui::ColorImage::from_rgba_unmultiplied([w, h1 - h0], &v.rgba[h0 * w * 4..h1 * w * 4]);
        let t = ctx.load_texture(format!("cyclo-{}", p.nom()), img, egui::TextureOptions::LINEAR);
        ctx.data_mut(|d| d.insert_temp(id, t.clone()));
        t
    }

    /// Propriétés > Monde : la section « Cyclo ».
    pub(crate) fn cyclo_ui(&mut self, ui: &mut egui::Ui) {
        let titre = ui.label(RichText::new("Cyclo").strong());
        if std::mem::take(&mut self.o3.vers_cyclo) {
            titre.scroll_to_me(Some(egui::Align::TOP));
        }
        ui.label(RichText::new("Le ciel et l'horizon autour du décor, et la lumière qu'ils donnent au décor et aux cartes. Dessinés par FEANOR.").small().weak());
        if self.doc.panels.is_empty() {
            return;
        }
        let actuel = self.cyclo_courant();
        let mut choix: Option<Option<Cyclo>> = None;
        // (Une grille d'autant de colonnes que la largeur du panneau en tient : « Aucun », puis les neuf choix.)
        let large = ui.available_width().min(ui.clip_rect().width()).max(VIGNETTE_PTS[0]);
        let cols = (((large + 6.0) / (VIGNETTE_PTS[0] + 8.0 + 6.0)).floor() as usize).max(1);
        egui::Grid::new("cyclo_galerie").spacing([6.0, 6.0]).show(ui, |ui| {
            let mut k = 0;
            let mut suivant = |ui: &mut egui::Ui| {
                k += 1;
                if k % cols == 0 {
                    ui.end_row();
                }
            };
            let r = ui.add_sized(vec2(VIGNETTE_PTS[0] + 8.0, VIGNETTE_PTS[1] + 26.0), egui::SelectableLabel::new(actuel.is_none(), "Aucun"));
            if crate::bulles::bulle(r, "Cyclo : Aucun", "Le ciel d'avant, son dégradé, et le soleil du décor tel qu'il était.", "").clicked() {
                choix = Some(None);
            }
            suivant(ui);
            for p in Preset::TOUS {
                let t = Self::texture_de_vignette(ui.ctx(), p);
                let on = actuel.is_some_and(|c| c.preset == p && c.dessin.is_none());
                let r = ui
                    .vertical(|ui| {
                        // (Une vignette, sans texte : rien à encrer quand elle est choisie.)
                        let mut vignette = egui::ImageButton::new(egui::Image::new((t.id(), vec2(VIGNETTE_PTS[0], VIGNETTE_PTS[1]))));
                        vignette = vignette.selected(on);
                        let b = ui.add(vignette);
                        ui.label(RichText::new(p.nom()).small());
                        b
                    })
                    .inner;
                if crate::bulles::bulle(r, &format!("Cyclo : {}", p.nom()), "Un cyclo dessiné par FEANOR : son ciel, son horizon, sa lumière. Ses réglages ensuite.", "").clicked() {
                    choix = Some(Some(p.cyclo()));
                }
                suivant(ui);
            }
            // (Partie 203 : le cyclo dessiné de la séquence, sa vignette (l'image du cyclo, quand elle est prête).)
            if let Some(c) = actuel.filter(|c| c.dessin.is_some()) {
                let r = ui
                    .vertical(|ui| {
                        let b = match Self::texture_du_cyclo_dessine(ui.ctx(), &c) {
                            Some(t) => {
                                let mut vignette = egui::ImageButton::new(egui::Image::new((t.id(), vec2(VIGNETTE_PTS[0], VIGNETTE_PTS[1]))));
                                vignette = vignette.selected(true);
                                ui.add(vignette)
                            }
                            None => ui.add_sized(vec2(VIGNETTE_PTS[0] + 8.0, VIGNETTE_PTS[1] + 8.0), egui::SelectableLabel::new(true, "…")),
                        };
                        ui.label(RichText::new("Mon cyclo").small());
                        b
                    })
                    .inner;
                crate::bulles::bulle(r, "Cyclo : Mon cyclo", "Le cyclo dessiné de la séquence : « Redessiner » le reprend.", "");
            }
        });
        // (Partie 203 : dessiner son cyclo.)
        let mut action: Option<ActionCyclo> = choix.map(ActionCyclo::Choisir);
        ui.horizontal_wrapped(|ui| {
            let r = ui.button("Dessiner mon cyclo…");
            if crate::bulles::bulle(r, "Dessiner mon cyclo", "Une feuille de 360° autour du décor, dans un onglet à elle : la dessiner avec les outils de FEANOR, puis « Terminer ».", "").clicked() {
                action = Some(ActionCyclo::Dessiner(self.depart_du_cyclo()));
            }
            ui.label("Partir de");
            let depart = self.depart_du_cyclo();
            let nom = |d: Depart| match d {
                Depart::Vide => "Vide",
                Depart::Choix(p) => p.nom(),
            };
            egui::ComboBox::from_id_salt("cyclo_depart").selected_text(nom(depart)).show_ui(ui, |ui| {
                for d in std::iter::once(Depart::Vide).chain(Preset::TOUS.into_iter().map(Depart::Choix)) {
                    if ui.selectable_label(d == depart, nom(d)).clicked() {
                        self.o3.cyclo_depart = Some(d);
                    }
                }
            });
            if actuel.is_some_and(|c| c.dessin.is_some()) {
                let r = ui.button("Redessiner");
                if crate::bulles::bulle(r, "Redessiner le cyclo", "Rouvre la feuille du cyclo dessiné de la séquence, telle qu'elle est.", "").clicked() {
                    action = Some(ActionCyclo::Redessiner);
                }
            }
        });
        if let Some(a) = action {
            self.demander_cyclo(a);
            return;
        }
        let Some(mut c) = actuel else { return };
        let avant = c;
        egui::Grid::new("cyclo_reglages").num_columns(2).spacing([8.0, 3.0]).show(ui, |ui| {
            ui.label("Heure");
            let r = ui.add(egui::Slider::new(&mut c.heure, 0.0..=24.0).step_by(0.25).suffix(" h"));
            crate::bulles::bulle(r, "Heure du cyclo", "La hauteur et la couleur du soleil : le matin, midi, le soir chaud, la nuit.", "");
            ui.end_row();
            ui.label("Nuages");
            let r = ui.add(egui::Slider::new(&mut c.nuages, 0.0..=1.0));
            crate::bulles::bulle(r, "Nuages du cyclo", "La part du ciel couverte.", "");
            ui.end_row();
            ui.label("Brume");
            let r = ui.add(egui::Slider::new(&mut c.brume, 0.0..=1.0));
            crate::bulles::bulle(r, "Brume du cyclo", "Vers l'horizon, le ciel et le lointain se fondent.", "");
            ui.end_row();
            ui.label("Rotation");
            let r = ui.add(egui::Slider::new(&mut c.rotation, -180.0..=180.0).suffix("°"));
            crate::bulles::bulle(r, "Rotation du cyclo", "Tourne le ciel et l'horizon autour du décor, le soleil avec.", "");
            ui.end_row();
            ui.label("Graine");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut c.graine).range(0..=99_999));
                let r = ui.button("Autre");
                if crate::bulles::bulle(r, "Autre graine", "D'autres nuages, d'autres crêtes, une autre ville.", "").clicked() {
                    c.graine = c.graine.wrapping_add(1);
                }
            });
            ui.end_row();
            ui.label("Couleur dominante");
            ui.horizontal(|ui| {
                let mut on = c.couleur.is_some();
                let r = ui.checkbox(&mut on, "");
                if crate::bulles::bulle(r, "Couleur dominante du cyclo", "Une teinte sur tout le ciel et sa lumière ; décochée : celles du choix.", "").changed() {
                    c.couleur = on.then_some([0xe8, 0x9a, 0x6a]);
                }
                if let Some(mut k) = c.couleur {
                    if crate::champcouleur::srgb(ui, &mut k).changed() {
                        c.couleur = Some(k);
                    }
                }
            });
            ui.end_row();
        });
        if c != avant {
            self.retoucher_cyclo(c);
        }
        if !ui.input(|i| i.pointer.any_down()) {
            self.decor3d_retouche_finir();
        }
    }

    /// Le Storyboard, dans la barre au-dessus du dessin (à côté de « Décor », partie 199) : « Cyclo : … » et la liste des choix.
    pub(crate) fn cyclo_picker_ui(&mut self, ui: &mut egui::Ui) {
        if self.doc.panels.is_empty() {
            return;
        }
        let actuel = self.cyclo_courant();
        let dessine = actuel.is_some_and(|c| c.dessin.is_some());
        let nom = if dessine { "Mon cyclo" } else { actuel.map_or("Aucun", |c| c.preset.nom()) };
        let mut choix: Option<ActionCyclo> = None;
        let depart = self.depart_du_cyclo();
        let r = ui.menu_button(format!("Cyclo : {nom}"), |ui| {
            if ui.selectable_label(actuel.is_none(), "Aucun").clicked() {
                choix = Some(ActionCyclo::Choisir(None));
                ui.close_menu();
            }
            for p in Preset::TOUS {
                if ui.selectable_label(actuel.is_some_and(|c| c.preset == p && !dessine), p.nom()).clicked() {
                    choix = Some(ActionCyclo::Choisir(Some(p.cyclo())));
                    ui.close_menu();
                }
            }
            ui.separator();
            // (Partie 203 : le cyclo dessiné.)
            if dessine {
                let _ = ui.selectable_label(true, "Mon cyclo");
                if ui.button("Redessiner").clicked() {
                    choix = Some(ActionCyclo::Redessiner);
                    ui.close_menu();
                }
            }
            if ui.button("Dessiner mon cyclo…").clicked() {
                choix = Some(ActionCyclo::Dessiner(depart));
                ui.close_menu();
            }
        });
        crate::bulles::bulle(
            r.response,
            "Cyclo de la séquence",
            "Le ciel et l'horizon autour du décor, et leur lumière sur les cartes ; ses réglages dans l'onglet 3D, Propriétés > Monde.",
            "",
        );
        if let Some(c) = choix {
            self.demander_cyclo(c);
        }
    }

    /// Le départ d'un dessin neuf : celui choisi dans « Partir de », sinon le choix du cyclo de la séquence (Jour clair sans cyclo).
    pub(crate) fn depart_du_cyclo(&self) -> Depart {
        self.o3.cyclo_depart.unwrap_or_else(|| Depart::Choix(self.cyclo_courant().map_or(Preset::Jour, |c| c.preset)))
    }

    /// Partie 203 : fait `a`, ou, s'il retirerait le cyclo dessiné de la séquence, le demande d'abord (`cyclo_a_jeter_window`).
    pub(crate) fn demander_cyclo(&mut self, a: ActionCyclo) {
        let dessine = self.cyclo_courant().is_some_and(|c| c.dessin.is_some());
        let jette = match a {
            ActionCyclo::Choisir(c) => c.is_none_or(|c| c.dessin.is_none()) && self.cyclo_courant() != c.map(Cyclo::borne),
            ActionCyclo::Dessiner(_) => true,
            ActionCyclo::Redessiner => false,
        };
        if dessine && jette {
            self.o3.cyclo_a_jeter = Some(a);
        } else {
            self.faire_cyclo(a);
        }
    }

    /// Fait `a` sans rien demander.
    pub(crate) fn faire_cyclo(&mut self, a: ActionCyclo) {
        match a {
            ActionCyclo::Choisir(c) => {
                let nom = if self.cyclo_courant().is_some_and(|c| c.dessin.is_some()) { "Retirer le cyclo dessiné" } else { "Cyclo" };
                self.changer_cyclo(nom, c);
            }
            ActionCyclo::Dessiner(d) => {
                self.dessiner_mon_cyclo(Some(d));
            }
            ActionCyclo::Redessiner => {
                self.dessiner_mon_cyclo(None);
            }
        }
    }

    /// « Retirer le cyclo dessiné ? » : un choix, « Aucun » ou un dessin neuf remplaceraient le dessin du cyclo de la séquence.
    pub(crate) fn cyclo_a_jeter_window(&mut self, ctx: &egui::Context) {
        let Some(a) = self.o3.cyclo_a_jeter else { return };
        let (titre, phrase, bouton) = match a {
            ActionCyclo::Dessiner(_) => ("Remplacer le cyclo dessiné ?", "Une feuille neuve s'ouvre ; à « Terminer », elle prend la place du dessin du cyclo de cette séquence.", "Dessiner un cyclo neuf"),
            _ => ("Retirer le cyclo dessiné ?", "Le dessin du cyclo de cette séquence sera retiré.", "Retirer le dessin"),
        };
        let (mut oui, mut non) = (false, false);
        let modal = egui::Modal::new(egui::Id::new("cyclo_a_jeter")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading(titre);
            ui.add_space(6.0);
            ui.label(phrase);
            ui.label(RichText::new("Ctrl+Z le rend.").weak());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                oui = ui.button(bouton).clicked();
                non = ui.button("Annuler").clicked();
            });
        });
        if oui {
            self.o3.cyclo_a_jeter = None;
            self.faire_cyclo(a);
        } else if non || modal.should_close() {
            self.o3.cyclo_a_jeter = None;
        }
    }

    /// La vignette du cyclo dessiné `c` : l'image du cyclo, réduite, quand elle est prête (gardée par egui, à son empreinte).
    fn texture_du_cyclo_dessine(ctx: &egui::Context, c: &Cyclo) -> Option<egui::TextureHandle> {
        let id = egui::Id::new(("vignette_cyclo_dessine", c.cle()));
        if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
            return Some(t);
        }
        let img = crate::cyclos::texture(c.cle())?;
        let [w, h] = crate::cyclos::VIGNETTE;
        // (La même part que les vignettes des choix : la bande de l'horizon.)
        let (h0, h1) = (h * 3 / 16, h * 5 / 8);
        let mut rgba = vec![0u8; w * (h1 - h0) * 4];
        for y in h0..h1 {
            for x in 0..w {
                let (xs, ys) = (x * img.largeur / w, y * img.hauteur / h);
                let (o, i) = (((y - h0) * w + x) * 4, (ys * img.largeur + xs) * 4);
                rgba[o..o + 4].copy_from_slice(&img.rgba[i..i + 4]);
            }
        }
        let t = ctx.load_texture("cyclo-dessine", egui::ColorImage::from_rgba_unmultiplied([w, h1 - h0], &rgba), egui::TextureOptions::LINEAR);
        ctx.data_mut(|d| d.insert_temp(id, t.clone()));
        Some(t)
    }

    /// « Prend la lumière du cyclo » du calque `li`, cochée ou décochée (une étape d'annulation).
    pub(crate) fn basculer_prend_cyclo(&mut self, li: usize) {
        let Some(l) = self.layers().get(li) else { return };
        if !crate::espace3d::is_card(l) || l.annotation {
            self.status = "Un dossier, un calque d'effet ou d'annotation n'a pas de carte dans le décor : il ne prend pas la lumière.".into();
            return;
        }
        let on = !l.prend_cyclo;
        self.end_stroke();
        let snap = self.snapshot();
        self.layers_mut()[li].prend_cyclo = on;
        self.commit_named(snap, if on { "Prend la lumière du cyclo" } else { "Ne prend pas la lumière du cyclo" });
        self.renderer.invalidate_all();
        self.status = if on { "La carte prend la lumière du cyclo.".into() } else { "La carte ne prend plus la lumière du cyclo : elle reste telle qu'elle est dessinée.".into() };
    }
}
