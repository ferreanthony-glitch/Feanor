//! La fenêtre « Comparer » (`comparer.rs` pour le calcul) : deux côtés d'un même plan, côte à côte ou
//! superposés. Lecture seule : rien ici n'écrit dans le document, l'historique ni les fichiers.
//!
//! Les côtés se choisissent parmi la version d'aujourd'hui, chaque version du plan (`planvers.rs`) et chaque
//! envoi qui contient le plan (`envois.rs`). Ouverte depuis un envoi (fenêtre « Envois ») ou depuis la fiche
//! du plan, elle propose d'office le plan d'aujourd'hui contre l'état d'un envoi, ou contre sa version
//! précédente.
//!
//! Un côté qui n'est que la version d'aujourd'hui ou une version rangée est une copie légère faite ici (les
//! tuiles sont partagées) ; un envoi se relit sur le fil de fond. L'interface ne fait que poser les textures
//! reçues : la glissière des images, le curseur du volet et le fondu ne calculent rien.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::comparer::{self, Entree, Mode, Moteur, Reponse, Requete, Source};
use crate::envois::date_courte;
use crate::planvers::etiquette;
use crate::theme;

/// Largeur des images composées pour la fenêtre.
const LARGEUR: usize = 1280;

/// L'état de la fenêtre (un seul champ dans `App`).
pub(crate) struct Comparaison {
    /// Le projet (`App::doc_epoch`) pour lequel elle vaut.
    epoch: u64,
    /// Le plan comparé (son numéro).
    plan: u64,
    pub(crate) a: Source,
    pub(crate) b: Source,
    pub(crate) mode: Mode,
    frame: usize,
    mix: f32,
    moteur: Option<Moteur>,
    numero: u64,
    prete: bool,
    images: [Option<usize>; 2],
    pub(crate) erreurs: [Option<String>; 2],
    notes: Vec<String>,
    textures: [Option<TextureHandle>; 3],
    difference: Option<f32>,
    /// Les images affichées sont celles de cette image du plan.
    vue_frame: Option<usize>,
    a_demander: bool,
    avec_pelure: bool,
    ouverte: bool,
}

impl Comparaison {
    fn nouvelle(epoch: u64, plan: u64, a: Source, b: Source, frame: usize) -> Self {
        Comparaison {
            epoch,
            plan,
            a,
            b,
            mode: Mode::Cote,
            frame,
            mix: 0.5,
            moteur: None,
            numero: 0,
            prete: false,
            images: [None, None],
            erreurs: [None, None],
            notes: Vec::new(),
            textures: [None, None, None],
            difference: None,
            vue_frame: None,
            a_demander: false,
            avec_pelure: false,
            ouverte: true,
        }
    }

    /// Une image est affichée pour cette image du plan (essais).
    #[cfg(test)]
    pub(crate) fn vue_frame(&self) -> Option<usize> {
        self.vue_frame
    }

    /// La part de l'image qui diffère d'un côté à l'autre (essais).
    #[cfg(test)]
    pub(crate) fn difference(&self) -> Option<f32> {
        self.difference
    }
}

impl App {
    // ------------------------------------------------------------------ les côtés

    /// Les côtés possibles pour le plan d'indice `index` : la version d'aujourd'hui, ses autres versions, les
    /// envois où il était.
    pub(crate) fn comparer_sources(&self, index: usize) -> Vec<Source> {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return Vec::new() };
        let mut out = vec![Source::Actuelle];
        if let Some(v) = self.doc.versions_du_plan(plan) {
            out.extend(v.autres.iter().rev().map(|r| Source::Version(r.numero)));
        }
        // Les autres variantes de sa séquence qui ont un plan de même rang (`variantes.rs`).
        let sequence = self.doc.sequence_of(index);
        if let Some(v) = self.doc.variantes_de(sequence) {
            let rang = self.rang_dans_sequence(index);
            out.extend(v.autres.iter().filter(|r| r.contenu.slots.len() > rang).map(|r| Source::Variante(r.numero)));
        }
        let mut envois: Vec<&crate::envois::Envoi> = self.doc.envois.list.iter().filter(|e| self.doc.plan_dans_envoi(e, plan).is_some()).collect();
        envois.sort_by_key(|e| std::cmp::Reverse(e.id));
        out.extend(envois.into_iter().map(|e| Source::Envoi(e.id)));
        out
    }

    /// Le rang du plan `index` dans sa séquence (0 pour le premier).
    fn rang_dans_sequence(&self, index: usize) -> usize {
        self.doc.course_de_sequence(self.doc.sequence_of(index)).map_or(0, |(debut, _)| index.saturating_sub(debut))
    }

    /// Le nom d'un côté pour le plan `plan`.
    pub(crate) fn comparer_libelle(&self, plan: u64, s: &Source) -> String {
        match s {
            Source::Actuelle => format!("Aujourd'hui ({})", etiquette(self.doc.numero_de_version(plan))),
            Source::Version(n) => match self.doc.versions_du_plan(plan).and_then(|v| v.rangee(*n)) {
                Some(r) if r.date > 0 => format!("Version {} ({})", etiquette(*n), date_courte(r.date)),
                _ => format!("Version {}", etiquette(*n)),
            },
            Source::Variante(n) => {
                let sequence = self.doc.panels.iter().position(|p| p.id == plan).map_or(0, |i| self.doc.sequence_of(i));
                format!("Version de séquence {}", self.doc.etiquette_de_variante(sequence, *n))
            }
            Source::Envoi(id) => match self.doc.envois.trouve(*id) {
                Some(e) => {
                    let v = self.doc.plan_dans_envoi(e, plan).and_then(|connu| e.plan(connu)).map_or(1, |p| p.version);
                    let suite = if v > 1 || self.doc.nombre_de_versions(plan) > 1 { format!(", {}", etiquette(v)) } else { String::new() };
                    format!("Envoi « {} » ({}{suite})", e.nom, date_courte(e.date))
                }
                None => "Envoi (retiré de la liste)".to_string(),
            },
        }
    }

    /// Ce qu'il faut donner au fil pour le côté `s` du plan d'indice `index`.
    fn comparer_entree(&self, index: usize, s: &Source) -> Result<Entree, String> {
        let panel = self.doc.panels.get(index).ok_or("le plan n'existe plus")?;
        let plan = panel.id;
        match s {
            Source::Actuelle => Ok(Entree::Doc(Box::new(comparer::petit_projet(&self.doc, panel)))),
            Source::Version(n) if *n == self.doc.numero_de_version(plan) => Ok(Entree::Doc(Box::new(comparer::petit_projet(&self.doc, panel)))),
            Source::Version(n) => {
                let r = self.doc.versions_du_plan(plan).and_then(|v| v.rangee(*n)).ok_or_else(|| format!("la version {} n'existe plus", etiquette(*n)))?;
                Ok(Entree::Doc(Box::new(comparer::petit_projet(&self.doc, &r.panel))))
            }
            Source::Variante(n) => {
                let sequence = self.doc.sequence_of(index);
                let rang = self.rang_dans_sequence(index);
                let r = self
                    .doc
                    .plan_de_variante_au_rang(sequence, *n, rang)
                    .ok_or_else(|| format!("la version de séquence {} n'a pas de plan au rang {} de la séquence", self.doc.etiquette_de_variante(sequence, *n), rang + 1))?;
                Ok(Entree::Doc(Box::new(comparer::petit_projet(&self.doc, r))))
            }
            Source::Envoi(id) => {
                let envoi = self.doc.envois.trouve(*id).ok_or("cet envoi n'est plus dans la liste")?;
                // Un plan copié par une variante se relit sous le numéro du plan dont il vient (`variantes.rs`).
                let plan = self.doc.plan_dans_envoi(envoi, plan).unwrap_or(plan);
                if self.doc.envois.pending.iter().any(|p| p.id == *id) {
                    return Err(format!("l'envoi « {} » s'enregistre encore : réessaie dans un instant.", envoi.nom));
                }
                let dossier = self.doc.envois.dossier.clone().or_else(|| self.path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf()));
                let dossier = dossier.ok_or("enregistre d'abord le projet : l'état d'un envoi est dans son dossier.")?;
                Ok(Entree::Envoi { dossier, envoi: Box::new(envoi.clone()), plan })
            }
        }
    }

    // ------------------------------------------------------------------ ouvrir

    /// La commande `comparer.ouvrir` : ouvre la fenêtre pour le plan courant, ou la ferme si elle est ouverte ;
    /// vrai si `action` en était une.
    pub(crate) fn comparer_action(&mut self, action: &str) -> bool {
        if action != "comparer.ouvrir" {
            return false;
        }
        if self.comparaison.take().is_none() {
            self.comparer_ouvrir(self.cur_panel, None, None);
        }
        true
    }

    /// Ouvre la comparaison du plan `index`. Sans côtés donnés : le plan d'aujourd'hui contre sa version
    /// précédente, sinon contre le dernier envoi où il était.
    pub(crate) fn comparer_ouvrir(&mut self, index: usize, a: Option<Source>, b: Option<Source>) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        let (a, b) = match (a, b) {
            (Some(a), Some(b)) => (a, b),
            (a, b) => {
                let b = b.unwrap_or(Source::Actuelle);
                let a = a.or_else(|| self.comparer_par_defaut(plan));
                match a {
                    Some(a) => (a, b),
                    None => {
                        self.status = "Rien à comparer pour ce plan : crée une version (« Nouvelle version du plan ») ou marque un envoi.".into();
                        return;
                    }
                }
            }
        };
        self.cur_panel = index.min(self.doc.panels.len() - 1);
        let frame = self.cur_frame.min(self.doc.panels[index].frame_count().saturating_sub(1));
        self.comparaison = Some(Comparaison::nouvelle(self.doc_epoch, plan, a, b, frame));
        self.comparer_relancer();
    }

    /// Le côté qu'on compare d'office au plan d'aujourd'hui : sa version rangée la plus récente, sinon le dernier
    /// envoi où il était.
    fn comparer_par_defaut(&self, plan: u64) -> Option<Source> {
        if let Some(v) = self.doc.versions_du_plan(plan) {
            let courante = v.actuelle;
            let avant = v.autres.iter().filter(|r| r.numero < courante).map(|r| r.numero).max();
            if let Some(n) = avant.or_else(|| v.autres.iter().map(|r| r.numero).max()) {
                return Some(Source::Version(n));
            }
        }
        self.doc.envois.list.iter().filter(|e| self.doc.plan_dans_envoi(e, plan).is_some() && !e.fichier.is_empty()).max_by_key(|e| e.id).map(|e| Source::Envoi(e.id))
    }

    /// Relit les deux côtés (autres côtés choisis, ou « Rafraîchir »).
    fn comparer_relancer(&mut self) {
        let ctx = self.ctx.clone();
        let Some(c) = self.comparaison.as_ref() else { return };
        let Some(index) = self.doc.panels.iter().position(|p| p.id == c.plan) else {
            self.comparaison = None;
            return;
        };
        let (a, b) = (self.comparer_entree(index, &c.a), self.comparer_entree(index, &c.b));
        let Some(c) = self.comparaison.as_mut() else { return };
        c.moteur = None;
        c.prete = false;
        c.vue_frame = None;
        c.difference = None;
        c.textures = [None, None, None];
        c.images = [None, None];
        c.notes.clear();
        c.erreurs = [None, None];
        match (a, b) {
            (Ok(a), Ok(b)) => {
                c.moteur = Some(Moteur::demarrer(a, b, ctx));
                c.a_demander = true;
            }
            (a, b) => {
                c.erreurs = [a.err(), b.err()];
                c.prete = true;
            }
        }
    }

    // ------------------------------------------------------------------ la fenêtre

    /// La fenêtre « Comparer », s'il y en a une d'ouverte.
    pub(crate) fn comparer_fenetre(&mut self, ctx: &egui::Context) {
        if self.comparaison.as_ref().is_some_and(|c| c.epoch != self.doc_epoch) {
            self.comparaison = None;
        }
        let Some(mut c) = self.comparaison.take() else { return };
        let Some(index) = self.doc.panels.iter().position(|p| p.id == c.plan) else { return };
        // Les réponses du fil.
        if let Some(m) = &c.moteur {
            while let Some(r) = m.recevoir() {
                match r {
                    Reponse::Prete { images, erreurs, notes } => {
                        c.images = images;
                        c.erreurs = erreurs;
                        c.notes = notes;
                        c.prete = true;
                        let max = images.iter().flatten().max().copied().unwrap_or(1);
                        c.frame = c.frame.min(max.saturating_sub(1));
                    }
                    Reponse::Rendu(r) if r.numero == c.numero => {
                        for (k, image) in [r.a, r.b, r.pelure].into_iter().enumerate() {
                            c.textures[k] = image.map(|i| ctx.load_texture(format!("comparer-{k}"), i, TextureOptions::LINEAR));
                        }
                        c.difference = r.difference;
                        c.vue_frame = Some(r.frame);
                    }
                    Reponse::Rendu(_) => {}
                }
            }
        }
        let mut relancer = false;
        let sources = self.comparer_sources(index);
        let titre = format!("Comparer · {}", crate::planversui::titre_avec_version(&self.doc, index));
        let mut ouverte = c.ouverte;
        egui::Window::new(titre).id(egui::Id::new("fenetre_comparer")).open(&mut ouverte).default_size([900.0, 600.0]).resizable(true).show(ctx, |ui| {
            // Les deux côtés.
            ui.horizontal(|ui| {
                for cote in 0..2 {
                    let courant = if cote == 0 { c.a.clone() } else { c.b.clone() };
                    let mut choix = courant.clone();
                    let texte = self.comparer_libelle(c.plan, &courant);
                    egui::ComboBox::from_id_salt(("comparer_cote", cote)).selected_text(texte).width(250.0).show_ui(ui, |ui| {
                        for s in &sources {
                            let l = self.comparer_libelle(c.plan, s);
                            ui.selectable_value(&mut choix, s.clone(), l);
                        }
                    });
                    if choix != courant {
                        if cote == 0 {
                            c.a = choix;
                        } else {
                            c.b = choix;
                        }
                        relancer = true;
                    }
                    if cote == 0 {
                        if bulles::icone(ui, ph::SWAP, "Échanger les côtés", "Le côté de gauche passe à droite, et inversement.", "").clicked() {
                            std::mem::swap(&mut c.a, &mut c.b);
                            relancer = true;
                        }
                    }
                }
                if bulles::icone(ui, ph::ARROW_CLOCKWISE, "Rafraîchir", "Relire les deux côtés (le plan a pu changer depuis).", "").clicked() {
                    relancer = true;
                }
            });
            // Le mode et la glissière.
            ui.horizontal(|ui| {
                let avant = c.mode;
                bulles::choix(ui, &mut c.mode, Mode::Cote, ph::COLUMNS, "Côte à côte", "Les deux images l'une à côté de l'autre.", "");
                bulles::choix(ui, &mut c.mode, Mode::Volet, ph::SPLIT_HORIZONTAL, "Volet", "Une seule image : un côté à gauche du curseur, l'autre à droite (glisser).", "");
                bulles::choix(ui, &mut c.mode, Mode::Fondu, ph::CIRCLE_HALF, "Fondu", "Une seule image : un côté fondu dans l'autre.", "");
                bulles::choix(ui, &mut c.mode, Mode::Pelure, ph::CIRCLES_THREE, "Superposition", "Le trait qui n'est que d'un côté en rouge (gauche) ou en bleu (droite), celui des deux en noir.", "");
                if c.mode != avant && c.mode == Mode::Pelure && c.textures[2].is_none() {
                    c.avec_pelure = true;
                    c.a_demander = true;
                }
                ui.separator();
                let max = c.images.iter().flatten().max().copied().unwrap_or(1).max(1);
                ui.label("Image");
                let mut f = c.frame + 1;
                if ui.add(egui::Slider::new(&mut f, 1..=max)).changed() {
                    c.frame = f - 1;
                    c.a_demander = true;
                }
                if matches!(c.mode, Mode::Volet | Mode::Fondu) {
                    ui.separator();
                    ui.label(if c.mode == Mode::Volet { "Volet" } else { "Fondu" });
                    ui.add(egui::Slider::new(&mut c.mix, 0.0..=1.0).show_value(false));
                }
            });
            ui.separator();
            let hauteur = (ui.available_height() - 24.0).max(120.0);
            self.comparer_dessin(ui, &mut c, hauteur);
            ui.horizontal(|ui| {
                let message = if let Some(e) = c.erreurs.iter().flatten().next() {
                    RichText::new(e.clone()).color(theme::erreur())
                } else if !c.prete || c.vue_frame.is_none() {
                    RichText::new("Calcul des images…").color(theme::texte_doux())
                } else if let Some(d) = c.difference {
                    let pct = format!("{:.1}", d * 100.0).replace('.', ",");
                    RichText::new(if d < 0.0005 { "Les deux images sont les mêmes.".to_string() } else { format!("{pct} % de l'image diffère.") })
                } else {
                    RichText::new("")
                };
                ui.label(message);
                ui.label(RichText::new("Lecture seule : cette vue ne change pas le plan.").color(theme::texte_doux()));
            });
            for n in &c.notes {
                ui.label(RichText::new(n).small().color(theme::avertissement()));
            }
        });
        c.ouverte = ouverte;
        if relancer {
            self.comparaison = Some(c);
            self.comparer_relancer();
            c = match self.comparaison.take() {
                Some(c) => c,
                None => return,
            };
        }
        // Une demande d'images (changement d'image, de mode, côtés relus), une seule à la fois.
        if c.a_demander {
            if let Some(m) = &c.moteur {
                c.numero += 1;
                m.demander(Requete { numero: c.numero, frame: c.frame, largeur: LARGEUR, avec_pelure: c.avec_pelure || c.mode == Mode::Pelure });
                c.a_demander = false;
            }
        }
        if c.ouverte {
            self.comparaison = Some(c);
        }
    }

    /// Les images, selon le mode, dans une zone de `hauteur` points.
    fn comparer_dessin(&self, ui: &mut egui::Ui, c: &mut Comparaison, hauteur: f32) {
        let aspect = self.doc.frame[0] as f32 / self.doc.frame[1].max(1) as f32;
        let zone = ui.allocate_space(vec2(ui.available_width(), hauteur)).1;
        let painter = ui.painter_at(zone);
        painter.rect_filled(zone, 2.0, theme::fond());
        let ajuste = |r: Rect| {
            let (w, h) = if r.width() / r.height() > aspect { (r.height() * aspect, r.height()) } else { (r.width(), r.width() / aspect) };
            Rect::from_center_size(r.center(), vec2(w, h))
        };
        let plein = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let etiquette_de = |plan: u64, s: &Source| self.comparer_libelle(plan, s);
        let cotes = [(&c.a, &c.textures[0], c.erreurs[0].as_ref()), (&c.b, &c.textures[1], c.erreurs[1].as_ref())];
        match c.mode {
            Mode::Cote => {
                let moitie = (zone.width() - 12.0) / 2.0;
                for (k, (source, tex, erreur)) in cotes.iter().enumerate() {
                    let colonne = Rect::from_min_size(pos2(zone.left() + k as f32 * (moitie + 12.0), zone.top() + 18.0), vec2(moitie, zone.height() - 18.0));
                    painter.text(pos2(colonne.center().x, zone.top() + 2.0), Align2::CENTER_TOP, etiquette_de(c.plan, source), FontId::proportional(12.0), theme::texte());
                    match (tex, erreur) {
                        (Some(t), _) => {
                            let r = ajuste(colonne);
                            painter.image(t.id(), r, plein, Color32::WHITE);
                            painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, theme::bord()), egui::StrokeKind::Outside);
                        }
                        (None, Some(e)) => {
                            painter.text(colonne.center(), Align2::CENTER_CENTER, e, FontId::proportional(12.0), theme::erreur());
                        }
                        (None, None) => {}
                    }
                }
            }
            Mode::Volet | Mode::Fondu | Mode::Pelure => {
                let r = ajuste(zone);
                let (a, b, p) = (&c.textures[0], &c.textures[1], &c.textures[2]);
                match c.mode {
                    Mode::Volet => {
                        if let (Some(a), Some(b)) = (a, b) {
                            painter.image(a.id(), r, plein, Color32::WHITE);
                            let x = r.left() + c.mix * r.width();
                            let droite = Rect::from_min_max(pos2(x, r.top()), r.max);
                            painter.image(b.id(), droite, Rect::from_min_max(pos2(c.mix, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                            painter.line_segment([pos2(x, r.top()), pos2(x, r.bottom())], Stroke::new(2.0_f32, theme::accent_texte()));
                            painter.circle_filled(pos2(x, r.center().y), 7.0, theme::accent_texte());
                            painter.text(pos2(r.left() + 6.0, r.top() + 4.0), Align2::LEFT_TOP, etiquette_de(c.plan, &c.a), FontId::proportional(11.0), theme::texte_fort());
                            painter.text(pos2(r.right() - 6.0, r.top() + 4.0), Align2::RIGHT_TOP, etiquette_de(c.plan, &c.b), FontId::proportional(11.0), theme::texte_fort());
                            let resp = ui.interact(r, ui.id().with("comparer_volet"), Sense::click_and_drag());
                            if let (true, Some(pos)) = (resp.dragged() || resp.clicked(), resp.interact_pointer_pos()) {
                                c.mix = ((pos.x - r.left()) / r.width()).clamp(0.0, 1.0);
                            }
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            }
                        }
                    }
                    Mode::Fondu => {
                        if let (Some(a), Some(b)) = (a, b) {
                            painter.image(a.id(), r, plein, Color32::WHITE);
                            painter.image(b.id(), r, plein, Color32::from_white_alpha((c.mix * 255.0).round() as u8));
                        }
                    }
                    _ => {
                        if let Some(p) = p {
                            painter.image(p.id(), r, plein, Color32::WHITE);
                            painter.text(pos2(r.left() + 6.0, r.top() + 4.0), Align2::LEFT_TOP, format!("rouge : {}", etiquette_de(c.plan, &c.a)), FontId::proportional(11.0), Color32::from_rgb(220, 40, 40));
                            painter.text(pos2(r.left() + 6.0, r.top() + 20.0), Align2::LEFT_TOP, format!("bleu : {}", etiquette_de(c.plan, &c.b)), FontId::proportional(11.0), Color32::from_rgb(50, 80, 235));
                        }
                    }
                }
                if let Some(e) = c.erreurs.iter().flatten().next() {
                    painter.text(zone.center(), Align2::CENTER_CENTER, e, FontId::proportional(12.0), theme::erreur());
                }
            }
        }
    }
}
