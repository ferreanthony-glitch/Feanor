//! Aide > Diagnostic des performances (`aide.diagnostic`) : la fenêtre, la proposition du premier
//! lancement, la section « Performance » de Fichier > Préférences… et la pose des réglages dans
//! le programme (`diagnostic.rs` dit quoi, `diagbanc.rs` mesure, `materiel.rs` détecte).
//!
//! La fenêtre ne bloque rien : le banc tourne sur un autre fil et la fenêtre lit son avancement ;
//! on peut annuler, fermer la fenêtre, ou continuer à travailler pendant ce temps. Rien n'est
//! appliqué sans un clic sur « Appliquer ».

use std::time::Duration;

use eframe::egui::{self, Align2, RichText};
use eframe::egui_wgpu::wgpu;

use crate::app::App;
use crate::diagbanc::Banc;
use crate::diagnostic::{self as d, Choix, Interrupteur, Mesures, PerfReglages, Profil, TraitsTextures, Vignettes};
use crate::materiel::{self, Ecran, Materiel};
use crate::theme;

/// Combien de temps sans rien faire avant que « rendu rapide jusqu'au repos » lance le rendu exact.
pub const REPOS_S: f64 = 2.0;

/// Ce que le rendu exact des dessins ouverts en rendu rapide doit faire maintenant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RendusExacts {
    /// Les demander tout de suite.
    Maintenant,
    /// Attendre encore `s` secondes de calme.
    Attendre(f64),
    /// Ne pas les demander (ils ne viennent que pour l'image affichée).
    Jamais,
}

/// La décision pour le réglage `mode`, `calme` secondes après la dernière activité.
pub fn rendus_exacts(mode: TraitsTextures, calme: f64, lecture: bool) -> RendusExacts {
    match mode {
        TraitsTextures::Exact => RendusExacts::Maintenant,
        TraitsTextures::Rapide => RendusExacts::Jamais,
        TraitsTextures::Repos if !lecture && calme >= REPOS_S => RendusExacts::Maintenant,
        TraitsTextures::Repos => RendusExacts::Attendre((REPOS_S - calme).max(0.0)),
    }
}

/// Un diagnostic terminé.
pub(crate) struct Termine {
    pub mesures: Mesures,
    pub materiel: Materiel,
    pub choix: Choix,
    pub date: u64,
}

/// L'état de la fenêtre.
pub(crate) struct DiagnosticUi {
    pub ouvert: bool,
    /// La proposition du premier lancement est affichée.
    pub offre: bool,
    adapter: Option<wgpu::AdapterInfo>,
    carte: Option<(wgpu::Device, wgpu::Queue)>,
    pub(crate) materiel: Option<Materiel>,
    pub(crate) banc: Option<Banc>,
    pub(crate) fait: Option<Termine>,
    /// Le profil regardé, et les réglages cochés.
    profil: Profil,
    coches: [bool; 11],
    message: String,
    /// Durée d'animation d'egui avant qu'on la coupe.
    animation: f32,
}

impl DiagnosticUi {
    pub(crate) fn new(offre: bool, rs: Option<&eframe::egui_wgpu::RenderState>) -> Self {
        DiagnosticUi {
            ouvert: false,
            offre,
            adapter: rs.map(|rs| rs.adapter.get_info()),
            carte: rs.map(|rs| (rs.device.clone(), rs.queue.clone())),
            materiel: None,
            banc: None,
            fait: None,
            profil: Profil::Equilibre,
            coches: [false; 11],
            message: String::new(),
            animation: egui::Style::default().animation_time,
        }
    }

    /// Choisit le profil regardé : coche les réglages qu'il change.
    pub(crate) fn regarder(&mut self, profil: Profil, courant: &PerfReglages) {
        self.profil = profil;
        if let Some(t) = &self.fait {
            let change = d::differences(courant, &d::reglages_du_profil(profil, &t.materiel));
            for (k, i) in Interrupteur::TOUS.iter().enumerate() {
                self.coches[k] = change.contains(i);
            }
        }
    }
}

/// Les lignes du matériel, en deux colonnes.
fn machine_grille(ui: &mut egui::Ui, m: &Materiel) {
    egui::Grid::new("diag_materiel").num_columns(2).spacing([14.0, 3.0]).show(ui, |ui| {
        for (nom, valeur) in m.lignes() {
            ui.label(RichText::new(nom).color(theme::texte_doux()));
            ui.add(egui::Label::new(valeur).wrap());
            ui.end_row();
        }
    });
}

impl App {
    // ------------------------------------------------------------------ poser les réglages

    /// Pose dans le programme les réglages de performance dès qu'ils ont changé : valeurs lues par
    /// les modules, cache de la carte graphique, vignettes à refaire, style d'egui.
    pub(crate) fn appliquer_performance(&mut self, ctx: &egui::Context) {
        let p = self.settings.perf;
        if p != self.perf_applied {
            let avant = self.perf_applied;
            let c = p.cibles();
            c.appliquer_globales();
            if let Some(gpu) = &self.gpu {
                gpu.set_cache_bytes(c.cache_carte);
                gpu.set_effects_gpu(c.effets_gpu);
            }
            if avant.vignettes != p.vignettes {
                self.thumbs.clear();
                self.layer_thumbs.clear();
            }
            self.perf_applied = p;
            ctx.request_repaint();
        }
        let animation = if p.animations_coupees { 0.0 } else { self.diag.animation };
        if (ctx.style().animation_time - animation).abs() > f32::EPSILON {
            ctx.style_mut(|s| s.animation_time = animation);
        }
    }

    /// À chaque image : le rendu exact des dessins restés en rendu rapide, selon le réglage.
    pub(crate) fn rejeu_idle_tick(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if self.playing || ctx.input(|i| !i.events.is_empty() || i.pointer.any_down()) {
            self.activity = now;
        }
        if self.rejeu_idle.is_empty() || self.open_job.is_some() {
            return;
        }
        match rendus_exacts(self.settings.perf.traits_textures, now - self.activity, self.playing) {
            RendusExacts::Maintenant => {
                for id in std::mem::take(&mut self.rejeu_idle) {
                    self.rejeu_with(id, false);
                }
            }
            RendusExacts::Attendre(s) => ctx.request_repaint_after(Duration::from_secs_f64(s + 0.05)),
            RendusExacts::Jamais => {}
        }
    }

    /// Le rendu exact de tous les dessins restés en rendu rapide, tout de suite : calculé sur plusieurs cœurs, ce
    /// n'est plus l'affaire d'un export (il refait ces dessins dans sa copie du projet, sur un autre fil : `ExportJob::with_exact`),
    /// mais celle de qui veut le projet exact maintenant.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn exactify_idle(&mut self) {
        if self.rejeu_idle.is_empty() {
            return;
        }
        let ids = std::mem::take(&mut self.rejeu_idle);
        for id in &ids {
            self.rejeu.cancel(*id);
        }
        let done = crate::exportrender::rendre_exacts(&mut self.doc, &ids);
        let n = done.len();
        for (id, before, after) in done {
            self.rejeu_warm.insert(id, after);
            crate::rejeu::keep_saved(&mut self.written, id, before, after);
        }
        self.renderer.invalidate_all();
        self.status = format!("Rendu exact de {n} dessin(s).");
    }

    // ------------------------------------------------------------------ la fenêtre

    /// Aide > Diagnostic des performances : ouvre la fenêtre (le matériel est détecté à l'ouverture).
    pub(crate) fn open_diagnostic(&mut self) {
        self.diag.ouvert = true;
        self.diag.message.clear();
        self.diag.materiel = Some(self.detecter_materiel());
    }

    fn detecter_materiel(&self) -> Materiel {
        let mut m = Materiel::systeme();
        m.carte = self.diag.adapter.as_ref().map(materiel::carte);
        let r = self.ctx.screen_rect();
        m.ecran = Some(Ecran { fenetre: [r.width(), r.height()], moniteur: self.ctx.input(|i| i.viewport().monitor_size).map(|v| [v.x, v.y]), ppp: self.ctx.pixels_per_point() });
        m
    }

    fn lancer_banc(&mut self) {
        if self.diag.banc.is_some() {
            return;
        }
        self.diag.message.clear();
        self.diag.fait = None;
        let ctx = self.ctx.clone();
        self.diag.banc = Some(Banc::lancer(self.diag.carte.clone(), move || ctx.request_repaint_after(Duration::from_millis(50))));
    }

    /// Le banc a fini : le matériel complété, le profil choisi, la date notée (plus de proposition
    /// au lancement).
    fn banc_termine(&mut self) {
        let Some(banc) = self.diag.banc.take() else { return };
        let r = banc.joindre();
        if r.annule {
            self.diag.message = "Diagnostic annulé : rien n'a été changé.".into();
            return;
        }
        let mut materiel = self.diag.materiel.clone().unwrap_or_else(|| self.detecter_materiel());
        materiel.disque = r.disque;
        let choix = d::choisir_profil(&materiel, &r.mesures);
        let date = d::maintenant();
        self.settings.diagnostic.note_fait(date);
        let _ = self.settings.save();
        let profil = choix.profil;
        self.diag.fait = Some(Termine { mesures: r.mesures, materiel, choix, date });
        let courant = self.settings.perf;
        self.diag.regarder(profil, &courant);
    }

    /// « Appliquer » : pose les réglages cochés du profil regardé.
    fn appliquer_profil(&mut self) {
        let Some(t) = &self.diag.fait else { return };
        let cible = d::reglages_du_profil(self.diag.profil, &t.materiel);
        let coches: Vec<Interrupteur> = Interrupteur::TOUS.iter().enumerate().filter(|(k, _)| self.diag.coches[*k]).map(|(_, i)| *i).collect();
        let mut courant = self.settings.perf;
        self.settings.diagnostic.appliquer(&mut courant, &cible, &coches, self.diag.profil, d::maintenant());
        self.settings.perf = courant;
        self.diag.message = format!(
            "Profil {} appliqué : {} réglage(s) posé(s). « Revenir comme avant » remet les valeurs d'avant.",
            self.diag.profil.nom(),
            coches.len()
        );
        self.status = format!("Performance : profil {} appliqué ({} réglages).", self.diag.profil.nom(), coches.len());
        if let Err(e) = self.settings.save() {
            self.status = format!("Réglages non enregistrés : {e}");
        }
    }

    /// « Revenir comme avant ».
    pub(crate) fn revenir_performance(&mut self) {
        let mut courant = self.settings.perf;
        if self.settings.diagnostic.revenir(&mut courant) {
            self.settings.perf = courant;
            self.diag.message = "Les réglages de performance sont revenus comme avant le diagnostic.".into();
            self.status = "Performance : réglages remis comme avant.".into();
            let _ = self.settings.save();
            let c = self.settings.perf;
            self.diag.regarder(self.diag.profil, &c);
        }
    }

    /// Le texte du rapport (profil regardé).
    fn rapport_texte(&self) -> Option<String> {
        let t = self.diag.fait.as_ref()?;
        let cible = d::reglages_du_profil(self.diag.profil, &t.materiel);
        Some(d::rapport(&t.materiel, &t.mesures, &t.choix, &self.settings.perf, &cible, t.date))
    }

    pub(crate) fn diagnostic_window(&mut self, ctx: &egui::Context) {
        if self.diag.banc.as_ref().is_some_and(Banc::fini) {
            self.banc_termine();
        }
        if !self.diag.ouvert {
            return;
        }
        let mut open = true;
        let mut lancer = false;
        let mut annuler = false;
        let mut appliquer = false;
        let mut revenir = false;
        let mut copier = false;
        let mut enregistrer = false;
        let mut profil = None;
        let centre = ctx.screen_rect().center().x;
        // (Une fenêtre qu'on peut étirer garde sa taille d'une image à l'autre : chaque état — avant,
        // pendant, après les mesures — a la sienne, prise sur son contenu la première fois.)
        let etat = if self.diag.banc.is_some() { 1 } else if self.diag.fait.is_some() { 2 } else { 0 };
        // (Une fenêtre étirable démarre à 420 points de haut : on lui donne la taille de son contenu.)
        let haut = match etat {
            1 => 330.0,
            2 => (ctx.screen_rect().height() - 150.0).clamp(300.0, 780.0),
            _ => 470.0,
        };
        egui::Window::new("Diagnostic des performances")
            .id(egui::Id::new(("diagnostic_performances", etat)))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .pivot(Align2::CENTER_TOP)
            .default_pos(egui::pos2(centre, 70.0))
            .default_size([620.0, haut])
            .show(ctx, |ui| {
                // Les résultats : les boutons en pied, la zone qui défile prend le reste.
                let pied = self.diag.fait.is_some() && self.diag.banc.is_none();
                let zone = if pied { (ui.available_height() - 84.0).max(160.0) } else { ui.available_height().max(120.0) };
                egui::ScrollArea::vertical().max_height(zone).auto_shrink([false, !pied]).show(ui, |ui| {
                    ui.set_min_width(590.0);
                    ui.label(RichText::new("SHOT détecte votre machine, la mesure une quinzaine de secondes, puis propose des réglages. Rien n'est appliqué sans votre accord.").color(theme::texte_doux()));
                    ui.add_space(6.0);
                    let resultats = self.diag.fait.is_some() && self.diag.banc.is_none();
                    if let Some(m) = &self.diag.fait.as_ref().map(|t| &t.materiel).or(self.diag.materiel.as_ref()) {
                        if resultats {
                            // (Les résultats d'abord : la machine se déplie, sa ligne reste lue.)
                            egui::CollapsingHeader::new(RichText::new("Votre machine").strong()).default_open(false).show(ui, |ui| machine_grille(ui, m));
                            ui.label(RichText::new(m.resume()).small().color(theme::texte_doux()));
                        } else {
                            ui.label(RichText::new("Votre machine").strong());
                            machine_grille(ui, m);
                        }
                    }
                    // Les dessins en mémoire (`residence.rs`) : ce que le projet tient, et le budget.
                    let s = self.residence.stats;
                    let budget = if s.budget == 0 { "sans limite".to_string() } else { format!("{:.0} Mo", s.budget as f64 / 1e6) };
                    ui.label(
                        RichText::new(format!(
                            "Dessins en mémoire : {:.0} Mo ({} dessins ; {} hors de la mémoire, relus ou refaits à la demande) — budget {budget}. Annulation : {:.0} Mo de tuiles, {:.1} Mo d'anciennes étapes comprimées.",
                            s.octets_presents as f64 / 1e6,
                            s.presents,
                            s.absents,
                            self.history.tuiles_gardees() as f64 * crate::residence::OCTETS_TUILE as f64 / 1e6,
                            self.history.octets_comprimes() as f64 / 1e6
                        ))
                        .small()
                        .color(theme::texte_doux()),
                    );
                    // Le budget de mémoire vidéo du système et ce que le cache des dessins en fait (`memoirevideo.rs`).
                    if let Some(l) = crate::memoirevideo::lecture() {
                        let vise = self.gpu.as_ref().map_or(0, |g| g.limite_octets());
                        let texte = format!("Mémoire vidéo de SHOT : {} Mo utilisés, budget du système {} Mo ; cache des dessins visé : {} Mo.", l.utilise >> 20, l.budget >> 20, vise >> 20);
                        ui.label(RichText::new(texte).small().color(theme::texte_doux()));
                    }
                    for ligne in crate::memoirevideo::journal().iter().rev().take(3) {
                        ui.label(RichText::new(ligne).small().color(theme::texte_doux()));
                    }
                    ui.add_space(8.0);
                    ui.separator();
                    // Le banc en cours.
                    if let Some(banc) = &self.diag.banc {
                        let fraction = banc.avancement.fraction();
                        ui.label(RichText::new("Mesures en cours").strong());
                        ui.add(egui::ProgressBar::new(fraction).desired_width(ui.available_width()).text(format!("{} — {} %", banc.avancement.etape(), (fraction * 100.0).round())));
                        ui.label(RichText::new("L'interface reste utilisable. Les mesures n'ouvrent aucun projet.").small().color(theme::texte_doux()));
                        annuler = crate::bulles::bulle(ui.button("Annuler"), "Annuler", "Arrête les mesures. Rien n'est changé.", "").clicked();
                        ctx.request_repaint_after(Duration::from_millis(100));
                        return;
                    }
                    let Some(t) = &self.diag.fait else {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let go = ui.button(RichText::new("Lancer le diagnostic").strong());
                            lancer = crate::bulles::bulle(go, "Lancer le diagnostic", "Détecte le matériel et mesure SHOT pendant une quinzaine de secondes, sur un autre fil.", "").clicked();
                            if self.settings.diagnostic.peut_revenir() {
                                revenir = crate::bulles::bulle(ui.button("Revenir comme avant"), "Revenir comme avant", "Remet les réglages de performance d'avant le dernier profil appliqué.", "").clicked();
                            }
                        });
                        if !self.diag.message.is_empty() {
                            ui.label(RichText::new(&self.diag.message).color(theme::accent_texte()));
                        }
                        return;
                    };
                    // Les mesures.
                    ui.label(RichText::new("Mesures").strong());
                    for l in d::lignes_mesures(&t.mesures) {
                        ui.label(l);
                    }
                    ui.label(RichText::new(format!("Durée du banc : {} s", format!("{:.1}", t.mesures.duree_s).replace('.', ","))).small().color(theme::texte_doux()));
                    ui.add_space(6.0);
                    ui.separator();
                    // Le profil conseillé et pourquoi.
                    ui.label(RichText::new(format!("Profil conseillé : {}", t.choix.profil.nom())).strong().color(theme::accent_texte()));
                    for r in t.choix.raisons() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label("•");
                            ui.label(r);
                        });
                    }
                    ui.add_space(6.0);
                    // Les trois profils.
                    ui.horizontal(|ui| {
                        for p in Profil::TOUS {
                            let nom = if p == t.choix.profil { format!("{} (conseillé)", p.nom()) } else { p.nom().to_string() };
                            let r = ui.selectable_label(self.diag.profil == p, nom);
                            if crate::bulles::bulle(r, p.nom(), p.resume(), "").clicked() {
                                profil = Some(p);
                            }
                        }
                    });
                    ui.label(RichText::new(self.diag.profil.resume()).small().color(theme::texte_doux()));
                    ui.add_space(6.0);
                    // Les réglages du profil, un par un.
                    let cible = d::reglages_du_profil(self.diag.profil, &t.materiel);
                    ui.label(RichText::new("Réglages à poser (décochez ceux que vous ne voulez pas)").strong());
                    let courant = self.settings.perf;
                    egui::Grid::new("diag_reglages").num_columns(3).spacing([10.0, 4.0]).show(ui, |ui| {
                        for (k, i) in Interrupteur::TOUS.iter().enumerate() {
                            let change = !i.egal(&courant, &cible);
                            if change {
                                ui.checkbox(&mut self.diag.coches[k], "");
                            } else {
                                self.diag.coches[k] = false;
                                ui.add_enabled(false, egui::Checkbox::new(&mut false, ""));
                            }
                            let nom = ui.label(RichText::new(i.nom()).color(if change { ui.visuals().text_color() } else { theme::texte_doux() }));
                            crate::bulles::bulle(nom, i.nom(), i.aide(), "");
                            let valeur = if change {
                                let suite = if i.immediat() { "" } else { " (à la prochaine ouverture d'un projet)" };
                                format!("{} → {}{}", i.texte(&courant), i.texte(&cible), suite)
                            } else {
                                format!("{} (déjà réglé)", i.texte(&courant))
                            };
                            ui.add(egui::Label::new(RichText::new(valeur).color(if change { theme::texte_fort() } else { theme::texte_doux() })).wrap());
                            ui.end_row();
                        }
                    });
                    ui.label(RichText::new("Tout s'applique tout de suite, sans redémarrer, sauf le rendu des traits texturés qui joue à l'ouverture d'un projet. Chaque réglage reste modifiable dans Fichier > Préférences… (section Performance).").small().color(theme::texte_doux()));
                });
                // Les boutons restent sous la zone qui défile : toujours visibles.
                if pied {
                    ui.separator();
                    let rien = !self.diag.coches.iter().any(|c| *c);
                    ui.horizontal_wrapped(|ui| {
                        let ap = ui.add_enabled(!rien, egui::Button::new(RichText::new("Appliquer").strong()));
                        appliquer = crate::bulles::bulle(ap, "Appliquer", "Pose les réglages cochés. Les valeurs d'avant sont gardées.", "").clicked();
                        let rev = ui.add_enabled(self.settings.diagnostic.peut_revenir(), egui::Button::new("Revenir comme avant"));
                        revenir = crate::bulles::bulle(rev, "Revenir comme avant", "Remet exactement les valeurs d'avant la dernière application.", "").clicked();
                        lancer = crate::bulles::bulle(ui.button("Relancer"), "Relancer le diagnostic", "Refait les mesures.", "").clicked();
                        copier = crate::bulles::bulle(ui.button("Copier le rapport"), "Copier le rapport", "Copie le rapport (matériel, mesures, réglages) dans le presse-papiers, en texte.", "").clicked();
                        enregistrer = crate::bulles::bulle(ui.button("Enregistrer le rapport…"), "Enregistrer le rapport", "Écrit le rapport dans un fichier texte.", "").clicked();
                    });
                    if !self.diag.message.is_empty() {
                        ui.label(RichText::new(&self.diag.message).color(theme::accent_texte()));
                    }
                }
            });
        if let Some(p) = profil {
            let courant = self.settings.perf;
            self.diag.regarder(p, &courant);
        }
        if annuler {
            if let Some(b) = &self.diag.banc {
                b.annuler();
            }
        }
        if lancer {
            self.lancer_banc();
        }
        if appliquer {
            self.appliquer_profil();
        }
        if revenir {
            self.revenir_performance();
        }
        if copier {
            if let Some(texte) = self.rapport_texte() {
                ctx.copy_text(texte);
                self.diag.message = "Rapport copié dans le presse-papiers.".into();
            }
        }
        if enregistrer {
            self.enregistrer_rapport();
        }
        if !open {
            if let Some(b) = &self.diag.banc {
                b.annuler();
            }
            self.diag.ouvert = false;
        }
    }

    fn enregistrer_rapport(&mut self) {
        let Some(texte) = self.rapport_texte() else { return };
        let Some(chemin) = rfd::FileDialog::new().add_filter("Texte", &["txt"]).set_file_name("diagnostic-performances-shot.txt").save_file() else { return };
        self.diag.message = match std::fs::write(&chemin, texte) {
            Ok(()) => format!("Rapport enregistré : {}", chemin.display()),
            Err(e) => format!("Rapport non enregistré : {e}"),
        };
    }

    // ------------------------------------------------------------------ le premier lancement

    /// Le petit message du premier lancement : lancer le diagnostic, ou plus tard. Il ne bloque
    /// rien et ne revient pas (la réponse est gardée dans les réglages).
    pub(crate) fn diagnostic_offer(&mut self, ctx: &egui::Context) {
        if !self.diag.offre {
            return;
        }
        let mut reponse = None;
        egui::Area::new(egui::Id::new("diagnostic_offre")).order(egui::Order::Foreground).anchor(Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -120.0)).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(theme::eleve()).stroke(egui::Stroke::new(1.0_f32, theme::accent_texte())).show(ui, |ui| {
                ui.set_max_width(400.0);
                ui.label(RichText::new("Régler SHOT pour cette machine ?").strong().color(theme::texte_fort()));
                ui.add(egui::Label::new(RichText::new("SHOT peut détecter votre matériel, le mesurer une quinzaine de secondes et proposer des réglages de performance. Rien n'est appliqué sans votre accord. Plus tard : Aide > Diagnostic des performances.").color(theme::texte_doux())).wrap());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if crate::bulles::bulle(ui.button(RichText::new("Lancer le diagnostic").strong()), "Lancer le diagnostic", "Ouvre le diagnostic et démarre les mesures.", "").clicked() {
                        reponse = Some(true);
                    }
                    if crate::bulles::bulle(ui.button("Plus tard"), "Plus tard", "Ne plus afficher ce message. Le diagnostic reste dans le menu Aide.", "").clicked() {
                        reponse = Some(false);
                    }
                });
            });
        });
        if let Some(lancer) = reponse {
            self.repondre_offre(lancer);
        }
    }

    /// La réponse au message du premier lancement : il disparaît, la réponse est gardée dans les
    /// réglages (il ne revient pas), et « Lancer » ouvre le diagnostic et démarre les mesures.
    /// Rien n'est appliqué.
    pub(crate) fn repondre_offre(&mut self, lancer: bool) {
        self.diag.offre = false;
        self.settings.diagnostic.propose = true;
        let _ = self.settings.save();
        if lancer {
            self.open_diagnostic();
            self.lancer_banc();
        }
    }

    // ------------------------------------------------------------------ Préférences

    /// La section « Performance » de Fichier > Préférences… : chaque réglage que le diagnostic
    /// pose, modifiable à la main.
    pub(crate) fn performance_prefs_ui(&mut self, ui: &mut egui::Ui) {
        let avant = self.settings.perf;
        let mut p = avant;
        let aide = |ui: &mut egui::Ui, nom: &str, i: Interrupteur| {
            let r = ui.label(nom);
            crate::bulles::bulle(r, i.nom(), i.aide(), "");
        };
        egui::Grid::new("prefs_performance").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
            aide(ui, "Effets en aperçu réduit", Interrupteur::EffetsReduits);
            ui.checkbox(&mut p.effets_reduits, "pendant la lecture et les gestes");
            ui.end_row();
            aide(ui, "Traits vectoriels texturés", Interrupteur::TraitsTextures);
            egui::ComboBox::from_id_salt("prefs_traits_textures").selected_text(p.traits_textures.nom()).show_ui(ui, |ui| {
                for m in TraitsTextures::TOUS {
                    ui.selectable_value(&mut p.traits_textures, m, m.nom());
                }
            });
            ui.end_row();
            aide(ui, "Tremblement du trait", Interrupteur::Tremblement);
            ui.checkbox(&mut p.tremblement_coupe, "couper « Animer à l'arrêt »");
            ui.end_row();
            aide(ui, "Pelure d'oignon", Interrupteur::Pelure);
            ui.add(egui::DragValue::new(&mut p.pelure_max).range(0..=crate::onion::MAX_NEIGHBOURS as u32).custom_formatter(|n, _| if n == 0.0 { "pas de plafond".into() } else { format!("{n} image(s) au plus") }));
            ui.end_row();
            aide(ui, "Cache de la carte graphique", Interrupteur::CacheCarte);
            ui.add(egui::DragValue::new(&mut p.cache_carte_mo).range(128..=16384).speed(32).suffix(" Mo"));
            ui.end_row();
            aide(ui, "Cache des traits vectoriels", Interrupteur::CacheTraits);
            ui.add(egui::DragValue::new(&mut p.cache_traits_mo).range(64..=8192).speed(16).suffix(" Mo"));
            ui.end_row();
            aide(ui, "Cache des effets", Interrupteur::CacheEffets);
            ui.add(egui::DragValue::new(&mut p.cache_effets_mo).range(64..=8192).speed(16).suffix(" Mo"));
            ui.end_row();
            // Mémoire des dessins (`residence.rs`) : 0 automatique, `SANS_LIMITE` : tout reste (comme avant).
            let r = ui.label("Mémoire des dessins");
            crate::bulles::bulle(
                r,
                "Mémoire des dessins",
                "Au-delà, les dessins les moins récemment regardés quittent la mémoire : relus du dossier du projet (dessins bitmap déjà enregistrés), ou refaits d'après leurs traits (dessins vectoriels), quand on y revient. Le plan courant, ses voisins, ce que la lecture va montrer et tout ce qui n'est pas enregistré restent toujours.",
                "",
            );
            ui.horizontal(|ui| {
                let auto = crate::residence::budget_auto_mo();
                let mut mode = match p.memoire_dessins_mo {
                    0 => 0,
                    crate::diagnostic::SANS_LIMITE => 2,
                    _ => 1,
                };
                let noms = [format!("automatique ({auto} Mo)"), "au plus".to_string(), "sans limite".to_string()];
                egui::ComboBox::from_id_salt("prefs_memoire_dessins").selected_text(noms[mode].clone()).show_ui(ui, |ui| {
                    for (k, n) in noms.iter().enumerate() {
                        ui.selectable_value(&mut mode, k, n);
                    }
                });
                p.memoire_dessins_mo = match mode {
                    0 => 0,
                    2 => crate::diagnostic::SANS_LIMITE,
                    _ => {
                        let mut mo = if p.memoire_dessins_mo == 0 || p.memoire_dessins_mo == crate::diagnostic::SANS_LIMITE { auto } else { p.memoire_dessins_mo };
                        ui.add(egui::DragValue::new(&mut mo).range(256..=65536).speed(32).suffix(" Mo"));
                        mo
                    }
                };
                let s = self.residence.stats;
                ui.label(RichText::new(format!("en mémoire : {:.0} Mo ({} dessins, {} hors de la mémoire)", s.octets_presents as f64 / 1e6, s.presents, s.absents)).small().weak());
            });
            ui.end_row();
            aide(ui, "Fils de calcul", Interrupteur::Fils);
            ui.add(egui::DragValue::new(&mut p.fils).range(0..=64).custom_formatter(|n, _| if n == 0.0 { "automatique".into() } else { format!("{n} au plus") }));
            ui.end_row();
            aide(ui, "Vignettes", Interrupteur::Vignettes);
            egui::ComboBox::from_id_salt("prefs_vignettes").selected_text(p.vignettes.nom()).show_ui(ui, |ui| {
                for v in Vignettes::TOUS {
                    ui.selectable_value(&mut p.vignettes, v, v.nom());
                }
            });
            ui.end_row();
            aide(ui, "Animations", Interrupteur::Animations);
            ui.checkbox(&mut p.animations_coupees, "interface sans animations");
            ui.end_row();
            aide(ui, "Effets sur la carte graphique", Interrupteur::EffetsGpu);
            let gpu_ok = self.gpu.as_ref().is_none_or(|g| g.effects_supported());
            ui.add_enabled(gpu_ok, egui::Checkbox::new(&mut p.effets_gpu, if gpu_ok { "flous, lueurs, bruit… dessinés par la carte, à chaque image" } else { "non disponible sur cette carte graphique" }));
            ui.end_row();
        });
        ui.horizontal_wrapped(|ui| {
            if crate::bulles::bulle(ui.button("Diagnostic des performances…"), "Diagnostic des performances", "Détecte la machine, la mesure et propose un profil de réglages.", &self.raccourci("aide.diagnostic")).clicked() {
                self.open_diagnostic();
            }
            if self.settings.diagnostic.peut_revenir() && ui.button("Revenir comme avant").clicked() {
                self.revenir_performance();
                p = self.settings.perf;
            }
            if ui.button("Valeurs d'origine").on_hover_text("Tous ces réglages reviennent à ce qu'ils étaient avant le diagnostic des performances.").clicked() {
                p = PerfReglages::default();
            }
        });
        ui.label(RichText::new("Les réglages jouent tout de suite ; le rendu des traits texturés, à l'ouverture d'un projet.").small().weak());
        if p != avant {
            self.settings.perf = p.sanitized();
            let _ = self.settings.save();
        }
    }
}
