//! Préférences > Vignettes, la suite (Anthony, 2026-10-30) : les numéros de plan et les lignes de texte, l'aspect des cases, le zoom
//! d'ouverture. Les réglages eux-mêmes sont dans `vignettesreglages.rs` ; les trois groupes de la fenêtre (`prefsonglets.rs`) sont
//! dessinés ici, `prefsui.rs` les appelle.

use eframe::egui::{self, RichText};

use crate::app::App;
use crate::board::{self, BoardLayout};
use eframe::egui::{pos2, vec2, Rect, Vec2};

/// Le zoom et le décalage de la planche à l'ouverture d'un projet (Anthony, 2026-10-30 : « ouvre toujours un document avec un zoom
/// équivalent à 2 rangées de 6 vignettes ») : `colonnes` cases par rangée et `rangees` rangées tiennent dans la vue `view`, leurs textes
/// compris, cadrées sur le haut de la planche ou, si la case `courante` (rang dans l'ordre de lecture) est plus loin, sur sa rangée.
/// Rend (zoom, décalage), au sens de `App::board_zoom` et `App::board_pan` (la planche est posée à `view.min + décalage`).
pub(crate) fn ouverture_cadre(layout: &BoardLayout, view: Rect, colonnes: usize, rangees: usize, courante: usize, zoom_min: f32, zoom_max: f32, lignes_texte: usize) -> (f32, Vec2) {
    // Chaque case : son image (pour ranger en rangées) et tout ce qu'elle occupe (numéro, tuile d'un mouvement, textes). Les cases vides, ensuite.
    let sous = board::NOTE_LINE_H * lignes_texte as f32 + 6.0;
    // Sur la planche par séquences : la séquence de la case courante seule (et son titre), pas les autres.
    let (cartes, vides, titre) = match layout.groupe_de(courante) {
        Some(g) => (g.cartes.clone(), g.vides.clone(), g.titre),
        None => (0..layout.cards.len(), 0..layout.empty.len(), None),
    };
    let mut cases: Vec<(Rect, Rect)> = layout.cards[cartes].iter().map(|c| (c.thumb, c.card.union(c.foot).union(c.thumb).union(c.note))).collect();
    cases.extend(layout.empty[vides].iter().map(|r| (*r, Rect::from_min_max(pos2(r.left(), r.top() - board::LABEL_H), pos2(r.right(), r.bottom() + sous)))));
    if cases.is_empty() || !(view.width() > 1.0 && view.height() > 1.0) {
        return (1.0, Vec2::ZERO);
    }
    let japonais = layout.sens.japonais();
    // Les cases groupées en rangées (en colonnes à la japonaise) : une nouvelle rangée quand la case recule sur l'axe d'avancée.
    let (axe, travers): (fn(&Rect) -> f32, fn(&Rect) -> f32) = if japonais { (|r| r.top(), |r| r.left()) } else { (|r| r.left(), |r| r.top()) };
    let mut groupes: Vec<Vec<(Rect, Rect)>> = Vec::new();
    let mut prec = f32::NEG_INFINITY;
    for c in &cases {
        let r = &c.0;
        if groupes.is_empty() || axe(r) < prec - 0.5 || travers(r) > travers(&groupes[groupes.len() - 1][0].0) + if japonais { r.width() } else { r.height() } * 0.5 {
            groupes.push(Vec::new());
        }
        prec = axe(r);
        groupes.last_mut().expect("un groupe").push(*c);
    }
    let rangees = rangees.max(1);
    let colonnes = colonnes.max(1);
    // La rangée de la case courante : le cadrage commence là si elle est au-delà des premières rangées.
    let rang_courant = layout.cards.get(courante).and_then(|c| groupes.iter().position(|g| g.iter().any(|x| x.0 == c.thumb))).unwrap_or(0);
    let premiere = if rang_courant >= rangees { rang_courant } else { 0 };
    let mut zone = Rect::NOTHING;
    // (Le titre de la séquence est dans la vue quand la première rangée l'est.)
    if let (0, Some(t)) = (premiere, titre) {
        zone = zone.union(t);
    }
    for g in groupes.iter().skip(premiere).take(rangees) {
        for c in g.iter().take(colonnes) {
            zone = zone.union(c.1);
        }
    }
    // Une petite marge de chaque côté ; en haut, huit points d'écran entre le haut de la vue et le numéro de la première rangée.
    // (Le dessin de la case courante déborde de 12 % de sa largeur : la marge le garde entier.)
    let largeur_case = groupes.first().and_then(|g| g.first()).map_or(100.0, |c| c.0.width());
    let zone = zone.expand2(vec2(0.13 * largeur_case + 6.0, 0.0));
    let haut = 8.0;
    let zoom = (view.width() / zone.width().max(1.0)).min((view.height() - haut) / zone.height().max(1.0)).clamp(zoom_min, zoom_max);
    // Marges égales à gauche et à droite ; le haut de la zone juste sous le haut de la vue.
    let reste_x = (view.width() - zone.width() * zoom).max(0.0);
    (zoom, vec2(-zone.min.x * zoom + reste_x * 0.5, -zone.min.y * zoom + haut))
}

/// La taille (points d'écran) du numéro de plan au zoom zoom de la planche : proportionnelle au zoom, bornée entre 15 et 72 points (à
/// 100 %) pour rester lisible de très loin comme de très près ; echelle : le réglage « Taille des numéros ».
pub(crate) fn taille_numero(zoom: f32, echelle: f32) -> f32 {
    (board::LABEL_H * board::NUMERO_FRACTION * zoom * echelle).clamp(15.0 * echelle, 72.0 * echelle)
}

/// La hauteur de la barre du bas de la planche : celle de la barre du bas du dessin.
pub(crate) const BARRE_BASSE_H: f32 = 30.0;

impl App {
    /// Cadre la planche comme à l'ouverture d'un projet : ouverture_colonnes × ouverture_rangees cases dans la vue (ouverture_cadre).
    pub(crate) fn ouverture_cadrer(&mut self, layout: &BoardLayout, view: Rect) {
        let r = &self.settings.vignettes;
        let courant = layout.cards.iter().position(|c| c.panel == self.cur_panel).unwrap_or(0);
        let (zoom, pan) = ouverture_cadre(layout, view, usize::from(r.ouverture_colonnes), usize::from(r.ouverture_rangees), courant, 0.2, 6.0, self.layout.board_textes.lignes());
        self.board_zoom = zoom;
        self.board_pan = pan;
        self.vignettes.ouverture_suivi = Some((view.size(), zoom, pan, true));
        if self.vignettes.ouverture_images == 0 {
            self.vignettes.ouverture_images = 12;
        }
    }

    /// La barre du bas de la planche des Vignettes (Anthony, 2026-10-30), comme celle de la vue Dessin : le passe-partout des cases et son
    /// opacité, le zoom de la planche (pourcentage, plus, moins, cadrer tout, le zoom d'ouverture « 6 × 2 »).
    /// (La rotation de la planche n'existe pas : voir `docs/INTERFACE.md`, partie des Vignettes du 2026-10-30.)
    pub(crate) fn planche_barre_basse(&mut self, ui: &mut egui::Ui, barre: Rect) {
        use crate::bulles::bulle;
        use egui_phosphor::regular as ph;
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(barre).layout(egui::Layout::left_to_right(egui::Align::Center)));
        child.painter().rect_filled(barre, 0.0, crate::theme::entete());
        child.spacing_mut().item_spacing = vec2(crate::barres::ECART, 0.0);
        child.add_space(4.0);
        let ui = &mut child;
        // Le passe-partout des cases : l'interrupteur (la commande `vignettes.marie_louise`) et l'opacité, comme celui de la caméra.
        let on = self.vignettes.annot.marie;
        self.command_button(ui, ph::SELECTION_INVERSE, "vignettes.marie_louise", "Passe-partout des cases", "Couvrir tout ce qui est hors des cases pour les voir comme des images finies. Un affichage : le dessin et les exports ne changent pas.", Some(on));
        let mut pct = f32::from(self.settings.vignettes.passe_partout_cases_opacite);
        let resp = ui
            .scope(|ui| {
                ui.spacing_mut().slider_width = 64.0;
                ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %").fixed_decimals(0))
            })
            .inner;
        let resp = bulle(resp, "Opacité du passe-partout des cases", "À quel point le passe-partout cache ce qui est hors des cases : 100 %, opaque ; 0 %, rien n'est couvert.", "");
        if resp.changed() {
            self.settings.vignettes.passe_partout_cases_opacite = pct.round().clamp(0.0, 100.0) as u8;
        }
        if resp.drag_stopped() || (resp.changed() && !resp.dragged()) {
            self.settings_a_ecrire = true;
        }
        ui.add_space(crate::barres::MARGE_TRAIT);
        ui.separator();
        ui.add_space(crate::barres::MARGE_TRAIT);
        // Le zoom de la planche, la seule commande de zoom de la vue (Anthony, 2026-10-30) : moins, le pourcentage réel, plus.
        let (colonnes, rangees) = (self.settings.vignettes.ouverture_colonnes, self.settings.vignettes.ouverture_rangees);
        if ui.add(egui::Button::new(ph::MAGNIFYING_GLASS_MINUS).frame(false)).on_hover_text("Zoom arrière").clicked() {
            self.board_zoom_step(1.0 / 1.25);
        }
        let mut p = (self.board_zoom * 100.0).clamp(20.0, 600.0);
        let avant = p;
        let r = ui.add(egui::DragValue::new(&mut p).range(20.0..=600.0).speed(1.0).max_decimals(0).suffix(" %"));
        if bulle(r, "Zoom de la planche", "Le zoom réel de la planche. Glisser, ou double-clic pour taper un pourcentage. Ctrl + molette aussi.", "").changed() && avant > 0.0 {
            self.board_zoom_step(p / avant);
        }
        if ui.add(egui::Button::new(ph::MAGNIFYING_GLASS_PLUS).frame(false)).on_hover_text("Zoom avant").clicked() {
            self.board_zoom_step(1.25);
        }
        let r = ui.button(format!("{} × {}", colonnes, rangees));
        if bulle(r, "Zoom d'ouverture", "Recadrer la planche comme à l'ouverture d'un projet : ce nombre de cases par rangée, ce nombre de rangées. Réglable dans Préférences > Vignettes.", "").clicked() {
            self.board_zoom = 0.0;
            self.vignettes.ouverture_a_faire = true;
        }
        let r = ui.button(ph::ARROWS_OUT.to_string());
        if bulle(r, "Cadrer tout", "Toute la planche dans la vue, en largeur et en hauteur (jusqu'au zoom le plus petit, 20 %).", "").clicked() {
            self.planche_cadrer_tout();
        }
    }

    /// Vignettes > Numéros et textes : la taille des numéros de plan (et leur fond), la pastille de version, la durée, les lignes de texte
    /// d'un projet neuf et la couleur de chaque ligne.
    pub(crate) fn numeros_textes_prefs_ui(&mut self, ui: &mut egui::Ui) {
        use crate::boardsens::Champ;
        use crate::bulles::bulle;
        ui.horizontal(|ui| {
            ui.label("Taille des numéros de plan");
            let mut v = self.settings.vignettes.taille_numeros;
            let resp = ui.add(egui::Slider::new(&mut v, crate::vignettesreglages::NUMEROS_MIN..=crate::vignettesreglages::NUMEROS_MAX).suffix(" %"));
            let resp = bulle(resp, "Taille des numéros de plan", "Le numéro suit le zoom de la planche, mais reste lisible de très loin comme de très près (15 à 72 points à l'écran, à 100 %). Ce réglage l'agrandit ou le réduit. 100 % d'origine.", "");
            if resp.changed() {
                self.settings.vignettes.taille_numeros = v;
                self.settings_a_ecrire = true;
            }
            // (Sur la même ligne : la page de l'onglet garde sa hauteur.)
            let mut numeros = self.settings.vignettes.numeros_visibles;
            let resp = ui.checkbox(&mut numeros, "Montrer les numéros");
            if bulle(resp, "Montrer les numéros de plan", "Les numéros dans la marge au-dessus des cases, sur la planche et dans son export. La touche N les cache ou les montre.", "vignettes.numeros").changed() {
                self.vignettes_basculer_numeros();
            }
        });
        ui.horizontal_wrapped(|ui| {
        let mut fond = self.settings.vignettes.fond_numeros;
        let resp = ui.checkbox(&mut fond, "Fond clair derrière les numéros");
        if bulle(resp, "Fond clair derrière les numéros", "Une pastille claire derrière le numéro de plan : il se lit sur n'importe quel dessin qui déborde.", "").changed() {
            self.settings.vignettes.fond_numeros = fond;
            self.settings_a_ecrire = true;
        }
        let mut version = self.settings.vignettes.pastille_version;
        let resp = ui.checkbox(&mut version, "Pastille de version à côté du numéro");
        if bulle(resp, "Pastille de version", "« V2 » à côté du numéro de plan, hors du cadre, pour un plan qui a plusieurs versions. Survolée, elle ouvre la pile des versions.", "").changed() {
            self.settings.vignettes.pastille_version = version;
            self.settings_a_ecrire = true;
        }
        let mut duree = self.settings.vignettes.afficher_duree;
        let resp = ui.checkbox(&mut duree, "Afficher la durée du plan");
        if bulle(resp, "Afficher la durée du plan", "La durée (secondes + images) à côté du numéro de plan.", "").changed() {
            self.settings.vignettes.afficher_duree = duree;
            self.settings_a_ecrire = true;
        }
        });
        ui.horizontal(|ui| {
            ui.label("Lignes de texte d'un projet neuf");
            let mut t = self.settings.vignettes.textes_defaut;
            let mut change = false;
            for c in Champ::TOUS {
                let mut on = t.montre(c);
                let resp = ui.checkbox(&mut on, c.etiquette());
                if bulle(resp, c.nom(), "Cette ligne se montre sous les cases d'un projet neuf. Décochée d'origine ; le choix déjà fait dans le panneau Vignettes n'est pas changé.", "").changed() {
                    t.regler(c, on);
                    change = true;
                }
            }
            if change {
                self.settings.vignettes.textes_defaut = t;
                self.settings_a_ecrire = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Couleur des lignes de texte");
            for (k, c) in Champ::TOUS.into_iter().enumerate() {
                let mut rvb = self.settings.vignettes.couleurs_textes[k];
                ui.label(c.etiquette());
                let resp = crate::champcouleur::srgb(ui, &mut rvb);
                if bulle(resp, c.nom(), "La couleur de l'intitulé et du filet de cette ligne sous les cases.", "").changed() {
                    self.settings.vignettes.couleurs_textes[k] = rvb;
                    self.settings_a_ecrire = true;
                }
            }
        });
    }

    /// Vignettes > Aspect des cases : le filet (épaisseur et couleur), l'opacité du dessin hors des cases et le passe-partout des cases.
    pub(crate) fn aspect_cases_prefs_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::bulle;
        ui.horizontal(|ui| {
            ui.label("Épaisseur du filet");
            let mut e = self.settings.vignettes.epaisseur_cadre;
            let resp = ui.add(egui::Slider::new(&mut e, 0.5..=6.0).suffix(" pt").fixed_decimals(1));
            if bulle(resp, "Épaisseur du filet", "Le trait qui entoure chaque case, en points (1 d'origine).", "").changed() {
                self.settings.vignettes.epaisseur_cadre = e;
                self.settings_a_ecrire = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Couleur du filet");
            let mut propre = self.settings.vignettes.couleur_filet.is_some();
            let resp = ui.checkbox(&mut propre, "Choisir");
            let mut changed = bulle(resp, "Couleur du filet", "Décoché : le gris d'origine (plus foncé pour la case courante).", "").changed();
            let mut c = self.settings.vignettes.couleur_filet.unwrap_or([154, 154, 154]);
            if propre {
                changed |= crate::champcouleur::srgb(ui, &mut c).changed();
            }
            if changed {
                self.settings.vignettes.couleur_filet = propre.then_some(c);
                self.settings_a_ecrire = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Dessin hors des cases");
            let mut v = self.settings.vignettes.opacite_hors_champ;
            let resp = ui.add(egui::Slider::new(&mut v, 0..=100).suffix(" %"));
            if bulle(resp, "Dessin hors des cases", "L'opacité de ce qui est dessiné sur la planche en dehors des cases (il n'appartient à aucun plan). Un affichage : rien ne change dans le dessin.", "").changed() {
                self.settings.vignettes.opacite_hors_champ = v;
                self.settings_a_ecrire = true;
            }
        });
        ui.horizontal(|ui| {
            let mut on = self.settings.vignettes.passe_partout_cases;
            let resp = ui.checkbox(&mut on, "Passe-partout des cases");
            if bulle(resp, "Passe-partout des cases", "Un voile qui cache tout ce qui est hors des cases : on les voit comme des images finies. Un affichage, sans effet sur le dessin ni sur les exports. Aussi dans la barre du bas de la planche.", "").changed() {
                self.settings.vignettes.passe_partout_cases = on;
                self.vignettes.annot.marie = on;
                self.settings_a_ecrire = true;
            }
            let mut v = self.settings.vignettes.passe_partout_cases_opacite;
            let resp = ui.add(egui::Slider::new(&mut v, 0..=100).suffix(" %"));
            if bulle(resp, "Opacité du passe-partout des cases", "100 % : opaque ; 0 % : rien n'est couvert.", "").changed() {
                self.settings.vignettes.passe_partout_cases_opacite = v;
                self.settings_a_ecrire = true;
            }
        });
    }

    /// Vignettes > À l'ouverture : le zoom de la planche à l'ouverture d'un projet.
    pub(crate) fn ouverture_cases_prefs_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::bulle;
        ui.horizontal(|ui| {
            ui.label("Zoom d'ouverture");
            let mut c = self.settings.vignettes.ouverture_colonnes;
            let mut r = self.settings.vignettes.ouverture_rangees;
            let a = ui.add(egui::DragValue::new(&mut c).range(1..=crate::board::MAX_COLUMNS as u8).speed(0.05).suffix(" cases"));
            let a = bulle(a, "Cases par rangée à l'ouverture", "Combien de cases d'une rangée tiennent dans la vue quand on ouvre un projet (6 d'origine).", "");
            ui.label("×");
            let b = ui.add(egui::DragValue::new(&mut r).range(1..=crate::vignettesreglages::OUVERTURE_MAX).speed(0.05).suffix(" rangées"));
            let b = bulle(b, "Rangées à l'ouverture", "Combien de rangées de cases tiennent dans la vue quand on ouvre un projet (2 d'origine).", "");
            if a.changed() || b.changed() {
                self.settings.vignettes.ouverture_colonnes = c;
                self.settings.vignettes.ouverture_rangees = r;
                self.settings_a_ecrire = true;
            }
        });
        ui.label(RichText::new("À l'ouverture et à la création d'un projet seulement : le zoom ne change pas quand on revient sur l'onglet.").small().weak());
    }
}
