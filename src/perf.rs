//! Mesure de la performance de SHOT : un petit profileur intégré et sa superposition
//! (« Performance », action `vue.performance`, aussi dans la palette Ctrl+K).
//!
//! Éteint, il ne coûte qu'un test de booléen par section (`section` rend une garde vide). Allumé, il
//! note pour chaque image d'écran :
//! - le temps de construction de l'interface (`App::frame`, de bout en bout) ;
//! - le temps passé dans chaque *section* (un panneau, la barre de menus, l'envoi des tuiles à la
//!   carte graphique…). Les sections s'emboîtent : un temps est *inclusif* (le panneau Dessin
//!   contient l'envoi à la carte graphique) ;
//! - ce qui a réveillé SHOT (`Context::repaint_causes` : fichier et ligne de chaque
//!   `request_repaint`), pour voir pourquoi il repeint au repos ;
//! - la file d'attente de l'arrière-plan (rejeux, fonds, vignettes).
//!
//! Le temps d'*après* la construction (découpage en triangles, peinture par la carte graphique,
//! attente de l'écran) n'est pas dans `App::frame` : la superposition le déduit de l'intervalle
//! entre deux images quand SHOT repeint sans cesse (« reste »).
//!
//! L'état est propre au fil de l'interface (`thread_local`) : pas de verrou, et les essais de
//! plusieurs fils ne se mélangent pas. La superposition ne prend aucun clic (`interactable(false)`)
//! et, allumée, rafraîchit ses chiffres deux fois par seconde ; ce réveil-là est étiqueté comme tel
//! dans la liste, pour ne pas le prendre pour un défaut.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::time::Instant;

use eframe::egui;

/// Images gardées en mémoire (environ 4 s à 60 images/s).
const HISTORIQUE: usize = 240;
/// Images sur lesquelles se calculent moyennes et maxima.
const FENETRE: usize = 60;
/// Rafraîchissement de la superposition quand rien d'autre ne repeint.
const RAFRAICHIR: std::time::Duration = std::time::Duration::from_millis(500);

/// Une image d'écran mesurée.
#[derive(Clone, Default)]
struct Image {
    /// Début de l'image, en secondes depuis le premier appel.
    debut: f64,
    /// Temps de `App::frame`, en millisecondes.
    construction: f32,
    /// Temps par section, en millisecondes (inclusif).
    sections: Vec<(&'static str, f32)>,
}

#[derive(Default)]
struct Etat {
    actif: bool,
    origine: Option<Instant>,
    debut: Option<Instant>,
    courante: Image,
    historique: VecDeque<Image>,
    /// Réveils de la dernière fenêtre : (cause, nombre d'images).
    reveils: VecDeque<Vec<String>>,
    /// File d'attente de l'arrière-plan, posée à chaque image par `file_attente`.
    files: Vec<(&'static str, usize)>,
}

thread_local! {
    static ETAT: RefCell<Etat> = RefCell::new(Etat::default());
}

/// La mesure est-elle allumée ?
pub fn actif() -> bool {
    ETAT.with(|e| e.borrow().actif)
}

/// Allume ou éteint la mesure (et repart d'un historique vide).
pub fn activer(on: bool) {
    ETAT.with(|e| {
        let mut e = e.borrow_mut();
        if e.actif != on {
            *e = Etat { actif: on, ..Etat::default() };
        }
    });
}

/// Début de `App::frame`.
pub fn begin_frame(ctx: &egui::Context) {
    ETAT.with(|e| {
        let mut e = e.borrow_mut();
        if !e.actif {
            return;
        }
        let now = Instant::now();
        let origine = *e.origine.get_or_insert(now);
        e.debut = Some(now);
        e.courante = Image { debut: now.duration_since(origine).as_secs_f64(), ..Image::default() };
        e.files.clear();
        let mut causes: Vec<String> = ctx.repaint_causes().iter().map(|c| format!("{}:{} {}", c.file.rsplit(['/', '\\']).next().unwrap_or(c.file), c.line, c.reason).trim().to_string()).collect();
        causes.sort();
        causes.dedup();
        e.reveils.push_back(causes);
        while e.reveils.len() > HISTORIQUE {
            e.reveils.pop_front();
        }
    });
}

/// Fin de `App::frame` : l'image est rangée dans l'historique.
pub fn end_frame(ctx: &egui::Context) {
    let rafraichir = ETAT.with(|e| {
        let mut e = e.borrow_mut();
        if !e.actif {
            return false;
        }
        let Some(debut) = e.debut.take() else { return false };
        e.courante.construction = debut.elapsed().as_secs_f32() * 1e3;
        let image = std::mem::take(&mut e.courante);
        e.historique.push_back(image);
        while e.historique.len() > HISTORIQUE {
            e.historique.pop_front();
        }
        true
    });
    if rafraichir {
        // Les chiffres de la superposition vivent un peu : une image de temps en temps suffit.
        ctx.request_repaint_after(RAFRAICHIR);
    }
}

/// Garde d'une section : le temps écoulé jusqu'à sa destruction est noté sous son nom.
pub struct Garde(Option<(&'static str, Instant)>);

impl Drop for Garde {
    fn drop(&mut self) {
        if let Some((nom, t)) = self.0.take() {
            let ms = t.elapsed().as_secs_f32() * 1e3;
            ETAT.with(|e| {
                let mut e = e.borrow_mut();
                match e.courante.sections.iter_mut().find(|(n, _)| *n == nom) {
                    Some((_, total)) => *total += ms,
                    None => e.courante.sections.push((nom, ms)),
                }
            });
        }
    }
}

/// Mesure ce qui suit, jusqu'à la fin de la portée de la garde rendue :
/// `let _t = perf::section("panneau Montage");`. Éteint : une garde vide.
pub fn section(nom: &'static str) -> Garde {
    if actif() {
        Garde(Some((nom, Instant::now())))
    } else {
        Garde(None)
    }
}

/// La file d'attente `nom` compte `n` travaux à cette image.
pub fn file_attente(nom: &'static str, n: usize) {
    ETAT.with(|e| {
        let mut e = e.borrow_mut();
        if e.actif {
            e.files.push((nom, n));
        }
    });
}

/// Ce que la mesure a vu sur les dernières images.
#[derive(Debug, Default, Clone)]
pub struct Resume {
    pub images: usize,
    /// Images par seconde sur la dernière seconde (0 au repos).
    pub par_seconde: usize,
    pub construction_moy: f32,
    pub construction_max: f32,
    /// Intervalle moyen entre deux images qui se suivent de près (< 250 ms), 0 s'il n'y en a pas.
    pub intervalle_moy: f32,
    /// Par section : (nom, moyenne, maximum) en millisecondes sur les images mesurées, du plus
    /// coûteux au moins coûteux.
    pub sections: Vec<(&'static str, f32, f32)>,
    /// Causes de réveil (fichier:ligne raison) et nombre d'images, de la plus fréquente à la
    /// moins fréquente.
    pub reveils: Vec<(String, usize)>,
    pub files: Vec<(&'static str, usize)>,
}

/// Le résumé des `FENETRE` dernières images.
pub fn resume() -> Resume {
    ETAT.with(|e| resume_de(&e.borrow()))
}

fn resume_de(e: &Etat) -> Resume {
    let n = e.historique.len().min(FENETRE);
    let recent: Vec<&Image> = e.historique.iter().rev().take(n).collect();
    let mut r = Resume { images: n, files: e.files.clone(), ..Resume::default() };
    if let Some(last) = e.historique.back() {
        r.par_seconde = e.historique.iter().filter(|i| last.debut - i.debut < 1.0).count();
    }
    if n == 0 {
        return r;
    }
    r.construction_moy = recent.iter().map(|i| i.construction).sum::<f32>() / n as f32;
    r.construction_max = recent.iter().map(|i| i.construction).fold(0.0, f32::max);
    let gaps: Vec<f64> = recent.windows(2).map(|w| w[0].debut - w[1].debut).filter(|g| *g < 0.25).collect();
    if !gaps.is_empty() {
        r.intervalle_moy = (gaps.iter().sum::<f64>() / gaps.len() as f64 * 1e3) as f32;
    }
    let mut totals: BTreeMap<&'static str, (f32, f32)> = BTreeMap::new();
    for i in &recent {
        for &(nom, ms) in &i.sections {
            let t = totals.entry(nom).or_default();
            t.0 += ms;
            t.1 = t.1.max(ms);
        }
    }
    r.sections = totals.into_iter().map(|(nom, (somme, max))| (nom, somme / n as f32, max)).collect();
    r.sections.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut causes: BTreeMap<&str, usize> = BTreeMap::new();
    for c in e.reveils.iter().rev().take(HISTORIQUE) {
        if c.is_empty() {
            *causes.entry("entrée (souris, clavier, fenêtre)").or_default() += 1;
        }
        for cause in c {
            *causes.entry(cause).or_default() += 1;
        }
    }
    r.reveils = causes.into_iter().map(|(c, k)| (c.to_string(), k)).collect();
    r.reveils.sort_by_key(|(_, k)| std::cmp::Reverse(*k));
    r
}

/// Le texte de la superposition (aussi utile aux essais).
pub fn rapport(r: &Resume) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "Images : {}/s (sur {} mesurées)", r.par_seconde, r.images);
    let _ = writeln!(s, "Construction de l'interface : {:.2} ms en moyenne, {:.2} ms au pire", r.construction_moy, r.construction_max);
    if r.intervalle_moy > 0.0 {
        let _ = writeln!(s, "Intervalle entre images : {:.2} ms (reste : {:.2} ms)", r.intervalle_moy, (r.intervalle_moy - r.construction_moy).max(0.0));
    }
    if !r.files.is_empty() {
        let q: Vec<String> = r.files.iter().map(|(n, k)| format!("{n} {k}")).collect();
        let _ = writeln!(s, "Arrière-plan en attente : {}", q.join(" · "));
    }
    if !r.sections.is_empty() {
        let _ = writeln!(s, "Sections (ms, moyenne / pire, inclusif) :");
        for (nom, moy, max) in r.sections.iter().take(16) {
            let _ = writeln!(s, "  {nom:<28} {moy:>7.2} / {max:>7.2}");
        }
    }
    if !r.reveils.is_empty() {
        let _ = writeln!(s, "Réveils :");
        for (cause, k) in r.reveils.iter().take(6) {
            let _ = writeln!(s, "  {k:>4} × {cause}");
        }
    }
    s
}

/// La superposition, dans un coin de la fenêtre ; ne prend aucun clic.
pub fn overlay(ctx: &egui::Context) {
    if !actif() {
        return;
    }
    let text = rapport(&resume());
    egui::Area::new(egui::Id::new("superposition_performance"))
        .order(egui::Order::Foreground)
        .interactable(false)
        .fixed_pos(egui::pos2(56.0, 60.0))
        .show(ctx, |ui| {
            egui::Frame::new().fill(egui::Color32::from_black_alpha(215)).corner_radius(4.0).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.label(egui::RichText::new("Performance (vue.performance pour fermer)").strong().size(11.0).color(egui::Color32::from_rgb(255, 210, 90)));
                ui.label(egui::RichText::new(text).monospace().size(11.0).color(egui::Color32::from_gray(225)));
            });
        });
}
