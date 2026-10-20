//! « Colorier par zones… » dans l'interface (`coloriage.rs`). Trois temps :
//! - la fenêtre dit ce qui va se passer (les clés, les images à colorier) et propose le calque de
//!   trait, le calque où sont peintes les clés, celui où écrire les aplats (le même, un nouveau
//!   calque « Couleur » vectoriel ou bitmap, un autre), la plage, la fermeture des trous, et ce
//!   qu'on fait des zones incertaines ;
//! - la tâche tourne sur un autre fil (le document ne le quitte pas : on lui donne les dessins, qui
//!   se partagent à la copie) : progression et « Arrêter » ;
//! - à la fin, les aplats sont posés en une seule étape d'annulation, « Colorier par zones », et la
//!   liste des zones à vérifier s'affiche : un clic va à l'image.
//!
//! Le calque de trait n'est jamais modifié.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use eframe::egui::{self, vec2, Align2};

use crate::app::App;
use crate::coloriage::{self, Bilan, Cadre, ConseillerCouleur, Demande, Doute, Incertain, Parametres, SansConseil};
use crate::document::{Cel, CelId, Layer, LayerKind};
use crate::roles::Role;
use crate::zones::ZoneCache;

/// Mémoire que garde le cache des découpes.
const CACHE_BYTES: usize = 384 << 20;
/// Nom du calque créé.
const NOM_COULEUR: &str = "Couleur";

/// Où écrire les aplats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dest {
    /// Dans le calque des clés lui-même (les clés ne bougent pas).
    Same,
    /// Dans un nouveau calque « Couleur », juste sous le calque de trait.
    New(LayerKind),
    /// Dans un autre calque (identifiant).
    Layer(u64),
}

/// Réglages de la fenêtre et état du coloriage.
pub(crate) struct Coloriage {
    window: bool,
    /// Calque de trait choisi (identifiant) ; aucun : le calque courant.
    line: Option<u64>,
    /// Calque où sont peintes les clés ; aucun : celui qu'on devine (`guess_source`).
    source: Option<u64>,
    pub(crate) dest: Dest,
    /// Clés : celles du calque (peintes), ou les images tapées dans `keys_text`.
    pub(crate) keys_typed: bool,
    pub(crate) keys_text: String,
    /// Plage : tout le plan, ou de `range.0` à `range.1` (comprise).
    pub(crate) whole_plan: bool,
    pub(crate) range: (usize, usize),
    pub(crate) params: Parametres,
    cache: Arc<ZoneCache>,
    job: Option<Job>,
    report: Option<Report>,
    advisor: Arc<dyn ConseillerCouleur>,
}

impl Coloriage {
    pub(crate) fn new() -> Coloriage {
        Coloriage {
            window: false,
            line: None,
            source: None,
            dest: Dest::Same,
            keys_typed: false,
            keys_text: String::new(),
            whole_plan: true,
            range: (0, 0),
            params: Parametres::default(),
            cache: Arc::new(ZoneCache::new(CACHE_BYTES)),
            job: None,
            report: None,
            advisor: Arc::new(SansConseil),
        }
    }

    /// Un coloriage tourne : le reste de l'interface attend.
    pub(crate) fn running(&self) -> bool {
        self.job.is_some()
    }

    /// Le conseiller de couleur (aucun avis pour l'instant ; voir `coloriage::ConseillerCouleur`).
    #[allow(dead_code)]
    pub(crate) fn set_advisor(&mut self, advisor: Arc<dyn ConseillerCouleur>) {
        self.advisor = advisor;
    }

    #[cfg(test)]
    pub(crate) fn cache(&self) -> &Arc<ZoneCache> {
        &self.cache
    }

    #[cfg(test)]
    pub(crate) fn window_open(&self) -> bool {
        self.window
    }

    #[cfg(test)]
    pub(crate) fn report_len(&self) -> Option<usize> {
        self.report.as_ref().map(|r| r.entries.len())
    }

    #[cfg(test)]
    pub(crate) fn report_entries(&self) -> Vec<(usize, u16, coloriage::Raison)> {
        self.report.as_ref().map(|r| r.entries.iter().map(|e| (e.frame, e.zone, e.raison)).collect()).unwrap_or_default()
    }
}

/// Une image à traiter, côté document.
pub(crate) struct Run {
    /// Les images du plan que ce dessin de trait occupe.
    pub first: usize,
    /// Les images à qui donner le dessin de couleur fabriqué.
    pub write: Vec<usize>,
    /// Une clé tenue : les images à qui donner le dessin de couleur de la clé (déjà dans le calque
    /// des clés).
    pub alias: Vec<usize>,
    pub alias_cel: Option<CelId>,
}

/// Tout ce qu'il faut pour colorier, relevé sur le document au départ.
pub(crate) struct Plan {
    pub panel: u64,
    pub line: u64,
    pub source: u64,
    pub dest: Dest,
    pub kind: LayerKind,
    pub demande: Demande,
    pub runs: Vec<Run>,
    /// Images des clés.
    pub keys: Vec<usize>,
    pub size: (usize, usize),
    /// Images à colorier.
    pub targets: usize,
}

/// Une coloriage en cours.
struct Job {
    plan: Plan,
    done: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
    handle: JoinHandle<Result<Output, String>>,
    started: Instant,
}

struct Output {
    /// Le dessin fabriqué de chaque image (rang dans la demande).
    cels: Vec<(usize, Cel)>,
    doutes: Vec<Doute>,
    bilan: Bilan,
}

/// Les zones à vérifier, après un coloriage.
struct Report {
    panel: u64,
    entries: Vec<Entry>,
    open: bool,
}

struct Entry {
    frame: usize,
    zone: u16,
    raison: coloriage::Raison,
    aire: u32,
    confiance: f32,
}

/// Les images tapées (« 1, 5-8, 12 »), comptées depuis 0.
pub(crate) fn parse_frames(text: &str, app: &App) -> Result<Vec<usize>, String> {
    let one = |s: &str| {
        crate::cues::parse_frame(s, app.doc.start_frame, app.doc.fps, app.time_display).ok_or_else(|| format!("« {} » n'est pas une image.", s.trim()))
    };
    let mut out = Vec::new();
    for token in text.split([',', ';', ' ']).map(str::trim).filter(|t| !t.is_empty()) {
        match token.split_once('-') {
            Some((a, b)) if !a.is_empty() && !b.is_empty() => {
                let (a, b) = (one(a)?, one(b)?);
                out.extend(a.min(b)..=a.max(b));
            }
            _ => out.push(one(token)?),
        }
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

/// Les suites d'images consécutives d'un calque qui montrent le même dessin : (début, fin exclue, dessin).
fn runs_of(layer: &Layer, count: usize) -> Vec<(usize, usize, CelId)> {
    let mut out: Vec<(usize, usize, CelId)> = Vec::new();
    for f in 0..count {
        let Some(c) = layer.cel_at(f) else { continue };
        match out.last_mut() {
            Some(last) if last.1 == f && last.2 == c => last.1 = f + 1,
            _ => out.push((f, f + 1, c)),
        }
    }
    out
}

/// Un nom de calque qui commence comme « Couleur ».
fn is_colour_name(name: &str) -> bool {
    name.trim().to_lowercase().starts_with("couleur")
}

fn frame_field<'a>(app: &'a App, value: &'a mut usize, span: &Range<usize>) -> egui::DragValue<'a> {
    egui::DragValue::new(value)
        .range(span.start..=span.end.saturating_sub(1).max(span.start))
        .speed(0.2)
        .custom_formatter(move |v, _| app.frame_label(v.max(0.0) as usize))
        .custom_parser(move |s| crate::cues::parse_frame(s, app.doc.start_frame, app.doc.fps, app.time_display).map(|f| f as f64))
}

impl App {
    /// Le calque de trait choisi (index dans le plan courant).
    fn coloriage_line(&self) -> usize {
        match self.coloriage.line.and_then(|id| self.layers().iter().position(|l| l.id == id)) {
            Some(i) => i,
            None => self.cur_layer,
        }
    }

    /// Le calque des clés : celui choisi s'il est encore possible, sinon un calque « Couleur »,
    /// sinon le voisin du calque de trait (dessous, sinon dessus).
    fn guess_source(&self, line: usize) -> Option<usize> {
        let layers = self.layers();
        let ok = |i: usize| i != line && !layers[i].folder && layers[i].video.is_none();
        if let Some(i) = self.coloriage.source.and_then(|id| layers.iter().position(|l| l.id == id)).filter(|&i| ok(i)) {
            return Some(i);
        }
        if let Some(i) = (0..layers.len()).filter(|&i| ok(i)).find(|&i| is_colour_name(&layers[i].name)) {
            return Some(i);
        }
        let below = line.checked_sub(1).filter(|&i| ok(i));
        below.or_else(|| (line + 1 < layers.len()).then_some(line + 1).filter(|&i| ok(i)))
    }

    /// Relève ce que le coloriage ferait, sans rien changer : les clés, les images à colorier, les
    /// dessins à donner à la tâche. Une erreur dit en français ce qui manque.
    pub(crate) fn coloriage_plan(&self) -> Result<Plan, String> {
        self.coloriage_plan_with(false)
    }

    /// `coloriage_plan`, sans les dessins (`light`) : pour le résumé de la fenêtre, refait à chaque
    /// image d'affichage, qui n'a besoin que des numéros.
    fn coloriage_plan_with(&self, light: bool) -> Result<Plan, String> {
        let c = &self.coloriage;
        let panel = self.panel();
        let layers = &panel.layers;
        let li = self.coloriage_line();
        let line = layers.get(li).ok_or("Aucun calque de trait.")?;
        if line.folder || line.video.is_some() {
            return Err("Le calque de trait est un dossier ou une vidéo : choisis un calque de dessin.".into());
        }
        let si = self.guess_source(li).ok_or("Il faut un calque de couleur, où sont peintes les clés : crée-en un sous le calque de trait.")?;
        let source = &layers[si];
        let count = panel.frame_count();
        // Le calque où écrire.
        let (dest, kind) = match c.dest {
            Dest::Same => (Dest::Same, source.kind),
            Dest::New(k) => (Dest::New(k), k),
            Dest::Layer(id) => {
                let l = layers.iter().find(|l| l.id == id && l.id != line.id && !l.folder && l.video.is_none());
                match l {
                    Some(l) => (Dest::Layer(l.id), l.kind),
                    None => return Err("Le calque où écrire n'existe plus : choisis-en un autre.".into()),
                }
            }
        };
        let dest_layer: Option<&Layer> = match dest {
            Dest::Same => Some(source),
            Dest::Layer(id) => layers.iter().find(|l| l.id == id),
            Dest::New(_) => None,
        };
        if dest_layer.is_some_and(|l| l.locked || l.position_locked) {
            return Err("Le calque où écrire est verrouillé.".into());
        }
        // Les clés.
        let has_colour = |f: usize| source.cel_at(f).and_then(|id| self.doc.cels.get(&id)).is_some_and(|cel| !cel.is_empty());
        let mut key_frames: Vec<usize> = if c.keys_typed {
            parse_frames(&c.keys_text, self)?.into_iter().filter(|&f| f < count && has_colour(f)).collect()
        } else {
            let marked: Vec<usize> = (0..count).filter(|&f| source.roles.get(&f) == Some(&Role::Key) && has_colour(f)).collect();
            if marked.is_empty() {
                (0..count).filter(|&f| has_colour(f) && (f == 0 || source.cel_at(f - 1) != source.cel_at(f))).collect()
            } else {
                marked
            }
        };
        let line_runs = runs_of(line, count);
        if line_runs.is_empty() {
            return Err("Le calque de trait est vide.".into());
        }
        // Une clé sans trait à cette image ne sert à rien ; deux clés sur le même dessin de trait : la première.
        key_frames.retain(|&f| line.cel_at(f).is_some());
        let mut key_runs: Vec<usize> = Vec::new();
        let mut key_of_run: Vec<Option<usize>> = vec![None; line_runs.len()];
        for &f in &key_frames {
            if let Some(r) = line_runs.iter().position(|&(a, b, _)| f >= a && f < b) {
                if key_of_run[r].is_none() {
                    key_of_run[r] = Some(f);
                    key_runs.push(r);
                }
            }
        }
        if key_runs.is_empty() {
            return Err(if c.keys_typed {
                "Aucune des images tapées n'a de couleur sur le calque des clés (et du trait au même endroit).".to_string()
            } else {
                format!("Aucune clé : peins des aplats sur le calque « {} », sous le trait, à au moins une image.", source.name)
            });
        }
        // La plage.
        let (lo, hi) = if c.whole_plan { (0, count) } else { (c.range.0.min(c.range.1).min(count), (c.range.0.max(c.range.1) + 1).min(count)) };
        if hi <= lo {
            return Err("La plage choisie est vide.".into());
        }
        let in_range = |r: usize| line_runs[r].0 < hi && line_runs[r].1 > lo;
        let same = dest == Dest::Same;
        // Les dessins de trait à colorier (dans la plage, et pas une clé si les aplats vont dans le
        // calque des clés).
        let is_target = |r: usize| in_range(r) && !(same && key_of_run[r].is_some());
        let Some(first_target) = (0..line_runs.len()).find(|&r| is_target(r)) else {
            return Err("Toutes les images de la plage sont des clés : rien à colorier.".into());
        };
        let last_target = (0..line_runs.len()).rfind(|&r| is_target(r)).unwrap_or(first_target);
        // De la clé d'avant la première image à colorier jusqu'à la clé d'après la dernière.
        let from = key_runs.iter().copied().filter(|&r| r <= first_target).max().unwrap_or(first_target).min(first_target);
        let to = key_runs.iter().copied().filter(|&r| r >= last_target).min().unwrap_or(last_target).max(last_target);
        let mut cadres = Vec::new();
        let mut runs = Vec::new();
        let mut targets = 0;
        let mut size: Option<(usize, usize)> = None;
        for r in from..=to {
            let (a, b, cel) = line_runs[r];
            let ligne = self.doc.cels.get(&cel).ok_or("Un dessin de trait manque.")?;
            let key = key_of_run[r].and_then(|f| source.cel_at(f).and_then(|id| self.doc.cels.get(&id).map(|k| (id, k))));
            let target = is_target(r);
            targets += usize::from(target);
            let empty = || crate::raster::Raster::new(0, 0);
            cadres.push(Cadre {
                cel,
                version: ligne.version,
                ligne: if light { empty() } else { ligne.raster.clone() },
                cle: key.map(|(_, k)| if light { empty() } else { k.raster.clone() }),
                cible: target,
            });
            if size.is_none() {
                size = Some((ligne.raster.width, ligne.raster.height));
            }
            let frames: Vec<usize> = (a.max(lo)..b.min(hi)).collect();
            let kf = key_of_run[r];
            runs.push(Run {
                first: a.max(lo).min(b - 1),
                write: if target { frames.clone() } else { Vec::new() },
                alias: if same && !target { frames.iter().copied().filter(|&f| Some(f) != kf && source.cel_at(f) != key.map(|k| k.0)).collect() } else { Vec::new() },
                alias_cel: key.map(|k| k.0),
            });
        }
        let size = size.unwrap_or((self.doc.width, self.doc.height));
        Ok(Plan {
            panel: panel.id,
            line: line.id,
            source: source.id,
            dest,
            kind,
            demande: Demande { cadres, parametres: c.params },
            runs,
            keys: key_runs.iter().filter_map(|&r| key_of_run[r]).collect(),
            size,
            targets,
        })
    }

    /// Action « calque.colorier_zones » : la fenêtre.
    pub(crate) fn open_coloriage(&mut self) {
        if self.layer().folder || self.layer().video.is_some() {
            self.status = "Le coloriage par zones se fait sur un calque de dessin (trait).".into();
            return;
        }
        self.coloriage.line = None;
        let count = self.frame_count();
        if self.coloriage.range.1 == 0 || self.coloriage.range.1 >= count {
            self.coloriage.range = (0, count.saturating_sub(1));
        }
        self.coloriage.window = true;
    }

    /// Dans les menus des calques : « Colorier par zones… » pour le calque `li` (le calque de trait).
    pub(crate) fn coloriage_item(&mut self, ui: &mut egui::Ui, li: usize) {
        let usable = self.layers().get(li).is_some_and(|l| !l.folder && l.video.is_none());
        let shortcut = self.keymap.label("calque.colorier_zones");
        let resp = ui.add_enabled(usable, egui::Button::new("Colorier par zones…").shortcut_text(&shortcut));
        if !usable {
            resp.on_disabled_hover_text("Choisis un calque de dessin (le trait) : SHOT reporte les couleurs des clés sur les autres images.");
        } else if resp.clicked() {
            ui.close_menu();
            self.select_layer(li);
            self.run_action("calque.colorier_zones");
        } else if resp.secondary_clicked() {
            ui.close_menu();
            self.start_assign("calque.colorier_zones");
        }
    }

    /// À chaque image d'affichage : la fenêtre, la tâche en cours, la liste des zones à vérifier.
    pub(crate) fn coloriage_ui(&mut self, ctx: &egui::Context) {
        if self.coloriage.job.is_some() {
            self.coloriage_progress(ctx);
        } else if self.coloriage.window {
            self.coloriage_window(ctx);
        }
        self.coloriage_report(ctx);
    }

    fn coloriage_window(&mut self, ctx: &egui::Context) {
        let li = self.coloriage_line();
        let plan = self.coloriage_plan_with(true);
        let layer_names: Vec<(u64, String, bool)> = self.layers().iter().map(|l| (l.id, l.name.clone(), l.folder || l.video.is_some())).collect();
        let line_id = self.layers().get(li).map(|l| l.id);
        let source_id = self.guess_source(li).map(|i| self.layers()[i].id);
        let line_name = self.layers().get(li).map(|l| l.name.clone()).unwrap_or_default();
        let transforms_differ = source_id
            .and_then(|id| self.layers().iter().find(|l| l.id == id))
            .zip(self.layers().get(li))
            .is_some_and(|(s, l)| (s.transform != l.transform || s.pos_keys != l.pos_keys) && self.coloriage.dest == Dest::Same);
        let span = 0..self.frame_count();
        let (mut open, mut go) = (true, false);
        let (mut line_choice, mut source_choice, mut dest, mut keys_typed) = (line_id, source_id, self.coloriage.dest, self.coloriage.keys_typed);
        let mut keys_text = self.coloriage.keys_text.clone();
        let (mut whole, mut range, mut params) = (self.coloriage.whole_plan, self.coloriage.range, self.coloriage.params);
        let mut mark = matches!(params.incertain, Incertain::Marquer(_));
        let summary = plan.as_ref().map(|p| {
            let labels: Vec<String> = p.keys.iter().map(|&k| self.frame_label(k)).collect();
            format!(
                "{} {} ({}) ; {} {} à colorier. Le trait ne change pas.",
                p.keys.len(),
                if p.keys.len() > 1 { "clés" } else { "clé" },
                labels.join(", "),
                p.targets,
                if p.targets > 1 { "dessins" } else { "dessin" }
            )
        });
        let app: &App = self;
        egui::Window::new("Colorier par zones")
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.screen_rect().center() - vec2(230.0, 200.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| {
                ui.set_width(480.0);
                ui.label(
                    egui::RichText::new(
                        "Colorie quelques images clés (des aplats sous le trait, dans les zones fermées) : SHOT retrouve les mêmes zones \
                         sur les autres images et y pose les mêmes aplats, nets et modifiables.",
                    )
                    .weak()
                    .small(),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Trait");
                    egui::ComboBox::from_id_salt("coloriage_trait").width(360.0).selected_text(&line_name).show_ui(ui, |ui| {
                        for (id, name, skip) in &layer_names {
                            if !skip {
                                ui.selectable_value(&mut line_choice, Some(*id), name);
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Clés peintes sur");
                    let current = layer_names.iter().find(|(id, ..)| Some(*id) == source_choice).map_or("(aucun calque)", |(_, n, _)| n.as_str());
                    egui::ComboBox::from_id_salt("coloriage_cles").width(320.0).selected_text(current).show_ui(ui, |ui| {
                        for (id, name, skip) in &layer_names {
                            if !skip && Some(*id) != line_choice {
                                ui.selectable_value(&mut source_choice, Some(*id), name);
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.radio_value(&mut keys_typed, false, "les images où ce calque est peint");
                    ui.radio_value(&mut keys_typed, true, "ces images :");
                    ui.add_enabled(keys_typed, egui::TextEdit::singleline(&mut keys_text).desired_width(150.0).hint_text("1, 5-8, 12"));
                });
                ui.horizontal(|ui| {
                    ui.label("Écrire les aplats");
                    let name = |d: Dest| match d {
                        Dest::Same => "dans le calque des clés".to_string(),
                        Dest::New(LayerKind::Vector) => format!("dans un nouveau calque « {NOM_COULEUR} » vectoriel"),
                        Dest::New(LayerKind::Bitmap) => format!("dans un nouveau calque « {NOM_COULEUR} » bitmap"),
                        Dest::Layer(id) => format!("dans « {} »", layer_names.iter().find(|(i, ..)| *i == id).map_or("?", |(_, n, _)| n.as_str())),
                    };
                    egui::ComboBox::from_id_salt("coloriage_dest").width(320.0).selected_text(name(dest)).show_ui(ui, |ui| {
                        ui.selectable_value(&mut dest, Dest::Same, name(Dest::Same));
                        ui.selectable_value(&mut dest, Dest::New(LayerKind::Vector), name(Dest::New(LayerKind::Vector)));
                        ui.selectable_value(&mut dest, Dest::New(LayerKind::Bitmap), name(Dest::New(LayerKind::Bitmap)));
                        for (id, n, skip) in &layer_names {
                            if !skip && Some(*id) != line_choice {
                                ui.selectable_value(&mut dest, Dest::Layer(*id), format!("dans « {n} »"));
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Images");
                    ui.radio_value(&mut whole, true, "tout le plan");
                    ui.radio_value(&mut whole, false, "de");
                    let (mut first, mut last) = range;
                    let a = ui.add_enabled(!whole, frame_field(app, &mut first, &span)).changed();
                    ui.label("à");
                    let b = ui.add_enabled(!whole, frame_field(app, &mut last, &span)).changed();
                    if a || b {
                        range = (first, last);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Fermer les trous du trait jusqu'à");
                    let mut closes = params.zones.trous as u32 * 2;
                    ui.add(egui::DragValue::new(&mut closes).range(0..=24).speed(0.2).suffix(" px"));
                    params.zones.trous = (closes / 2) as u8;
                    ui.label("· zone sous le trait");
                    let mut under = params.zones.debord as u32;
                    ui.add(egui::DragValue::new(&mut under).range(0..=8).speed(0.1).suffix(" px"));
                    params.zones.debord = under as u8;
                });
                ui.horizontal(|ui| {
                    ui.label("Zone incertaine");
                    ui.radio_value(&mut mark, false, "la laisser sans couleur");
                    ui.radio_value(&mut mark, true, "la marquer en magenta (à vérifier)");
                });
                ui.add(egui::Slider::new(&mut params.seuil, 0.2..=0.9).text("Confiance minimale"));
                if transforms_differ {
                    ui.colored_label(ui.visuals().warn_fg_color, "Le trait et le calque des clés n'ont pas la même transformation : les zones suivent le dessin brut.");
                }
                ui.separator();
                match &summary {
                    Ok(text) => {
                        ui.label(text);
                    }
                    Err(e) => {
                        ui.label(e.as_str());
                    }
                }
                ui.label(egui::RichText::new("Une seule étape d'annulation. Les zones incertaines sont listées pour que tu les vérifies.").weak().small());
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    go = ui.add_enabled(summary.is_ok(), egui::Button::new("Colorier")).clicked();
                    if ui.button("Fermer").clicked() {
                        open = false;
                    }
                });
            });
        params.incertain = if mark { Incertain::Marquer(coloriage::REPERE) } else { Incertain::Laisser };
        let c = &mut self.coloriage;
        (c.window, c.line, c.source, c.dest, c.keys_typed, c.keys_text) = (open, line_choice, source_choice, dest, keys_typed, keys_text);
        (c.whole_plan, c.range, c.params) = (whole, range, params);
        if go {
            self.start_coloriage(ctx);
        }
    }

    /// Lance la tâche avec les réglages de la fenêtre.
    pub(crate) fn start_coloriage(&mut self, ctx: &egui::Context) {
        if self.content_refuses() || self.coloriage.job.is_some() {
            return;
        }
        self.end_stroke();
        self.playing = false;
        let plan = match self.coloriage_plan() {
            Ok(p) => p,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        self.coloriage.window = false;
        self.coloriage.report = None;
        let (done, total, cancel) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(1)), Arc::new(AtomicBool::new(false)));
        let (d, t, stop, wake) = (done.clone(), total.clone(), cancel.clone(), ctx.clone());
        let (cache, advisor) = (self.coloriage.cache.clone(), self.coloriage.advisor.clone());
        // La tâche reçoit les dessins (partagés à la copie) ; le document reste ici.
        let mut plan = plan;
        let demande = Demande { cadres: std::mem::take(&mut plan.demande.cadres), parametres: plan.demande.parametres };
        let (kind, size) = (plan.kind, plan.size);
        let handle = std::thread::spawn(move || run_job(&demande, &cache, advisor.as_ref(), &stop, &d, &t, &wake, kind, size));
        self.coloriage.job = Some(Job { plan, done, total, cancel, handle, started: Instant::now() });
    }

    fn coloriage_progress(&mut self, ctx: &egui::Context) {
        let Some(job) = self.coloriage.job.as_ref() else { return };
        if !job.handle.is_finished() {
            let (done, total) = (job.done.load(Ordering::Relaxed), job.total.load(Ordering::Relaxed).max(1));
            let stopping = job.cancel.load(Ordering::Relaxed);
            let mut stop = false;
            egui::Modal::new(egui::Id::new("progression_coloriage")).show(ctx, |ui| {
                ui.set_width(420.0);
                ui.heading("Coloriage par zones");
                ui.add_space(6.0);
                ui.add(egui::ProgressBar::new(done as f32 / total as f32).show_percentage());
                let mut text = format!("{} images à colorier, {} clés", job.plan.targets, job.plan.keys.len());
                if done > 0 && done < total {
                    let rest = job.started.elapsed().as_secs_f32() / done as f32 * (total - done) as f32;
                    text += &format!(" · reste environ {}:{:02}", rest as u64 / 60, rest as u64 % 60);
                }
                ui.label(text);
                ui.add_space(6.0);
                stop = ui.add_enabled(!stopping, egui::Button::new(if stopping { "Arrêt…" } else { "Arrêter" })).clicked();
            });
            if stop {
                job.cancel.store(true, Ordering::Relaxed);
            }
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        self.finish_coloriage();
    }

    /// Attend la fin de la tâche et pose ses aplats (les essais).
    #[cfg(test)]
    pub(crate) fn finish_coloriage_blocking(&mut self) {
        while self.coloriage.job.as_ref().is_some_and(|j| !j.handle.is_finished()) {
            std::thread::sleep(Duration::from_millis(5));
        }
        self.finish_coloriage();
    }

    /// La tâche est finie : on pose ses aplats (une étape d'annulation).
    pub(crate) fn finish_coloriage(&mut self) {
        let Some(job) = self.coloriage.job.take() else { return };
        let stopped = job.cancel.load(Ordering::Relaxed);
        let seconds = job.started.elapsed().as_secs_f32();
        let output = match job.handle.join() {
            Err(_) => {
                self.status = "Le coloriage s'est arrêté sur une erreur ; rien n'a changé.".into();
                return;
            }
            Ok(Err(_)) if stopped => {
                self.status = "Coloriage arrêté : rien n'a changé.".into();
                return;
            }
            Ok(Err(e)) => {
                self.status = format!("Coloriage impossible : {e}");
                return;
            }
            Ok(Ok(o)) => o,
        };
        let plan = job.plan;
        let Some(pi) = self.doc.panels.iter().position(|p| p.id == plan.panel) else {
            self.status = "Coloriage : le plan a disparu, rien n'a changé.".into();
            return;
        };
        let Some(line) = self.doc.panels[pi].layers.iter().position(|l| l.id == plan.line) else {
            self.status = "Coloriage : le calque de trait a disparu, rien n'a changé.".into();
            return;
        };
        let Some(source) = self.doc.panels[pi].layers.iter().position(|l| l.id == plan.source) else {
            self.status = "Coloriage : le calque des clés a disparu, rien n'a changé.".into();
            return;
        };
        if pi != self.cur_panel {
            self.set_panel(pi);
        }
        let snap = self.snapshot();
        // Le calque où écrire : le même, un autre, ou un neuf juste sous le calque de trait.
        let dest = match plan.dest {
            Dest::Same => source,
            Dest::Layer(id) => match self.layers().iter().position(|l| l.id == id) {
                Some(i) => i,
                None => {
                    self.status = "Coloriage : le calque où écrire a disparu, rien n'a changé.".into();
                    return;
                }
            },
            Dest::New(kind) => {
                let mut layer = Layer::new(self.new_colour_name(), kind);
                let l = &self.layers()[line];
                (layer.parent, layer.transform, layer.depth) = (l.parent, l.transform, l.depth);
                layer.pos_keys = l.pos_keys.clone();
                self.layers_mut().insert(line, layer);
                // Le calque de trait et tout ce qui est au-dessus montent d'un rang.
                if self.cur_layer >= line {
                    self.cur_layer += 1;
                }
                line
            }
        };
        let mut made: Vec<Option<CelId>> = vec![None; plan.runs.len()];
        let by_cadre: std::collections::HashMap<usize, Cel> = output.cels.into_iter().collect();
        for (k, cel) in by_cadre {
            made[k] = Some(self.doc.add_cel(cel));
        }
        let layer = &mut self.doc.panels[pi].layers[dest];
        for (k, run) in plan.runs.iter().enumerate() {
            if let Some(id) = made[k] {
                for &f in &run.write {
                    layer.set(f, Some(id));
                }
            }
            if let Some(id) = run.alias_cel {
                for &f in &run.alias {
                    layer.set(f, Some(id));
                }
            }
        }
        for &k in &plan.keys {
            layer.roles.insert(k, Role::Key);
        }
        let before = self.history.context.replace("Colorier par zones".to_string());
        self.commit_structure(snap);
        self.history.context = before;
        // La liste des zones à vérifier : le rang de l'image dans la demande donne la première
        // image de son dessin de trait.
        let mut entries: Vec<Entry> = output
            .doutes
            .iter()
            .filter_map(|d| plan.runs.get(d.cadre).map(|r| Entry { frame: r.first, zone: d.zone, raison: d.raison, aire: d.aire, confiance: d.confiance }))
            .collect();
        entries.sort_by_key(|e| (e.frame, e.zone));
        let doubts = entries.len();
        self.coloriage.report = (doubts > 0).then(|| Report { panel: plan.panel, entries, open: true });
        let Bilan { images, zones_coloriees, .. } = output.bilan;
        self.status = format!(
            "Coloriage par zones : {images} dessins, {zones_coloriees} zones coloriées en {seconds:.1} s{}.",
            if doubts > 0 { format!(", {doubts} à vérifier") } else { String::new() }
        );
    }

    /// Un nom neuf pour le calque créé.
    fn new_colour_name(&self) -> String {
        let taken = |n: &str| self.layers().iter().any(|l| l.name == n);
        if !taken(NOM_COULEUR) {
            return NOM_COULEUR.into();
        }
        (2..).map(|k| format!("{NOM_COULEUR} {k}")).find(|n| !taken(n)).unwrap_or_else(|| NOM_COULEUR.into())
    }

    /// La liste « à vérifier » : un clic va à l'image.
    fn coloriage_report(&mut self, ctx: &egui::Context) {
        let Some(report) = self.coloriage.report.as_ref() else { return };
        if !report.open {
            return;
        }
        let rows: Vec<(String, usize)> = report
            .entries
            .iter()
            .map(|e| (format!("Image {} · zone {} · {} ({} px, confiance {:.0} %)", self.frame_label(e.frame), e.zone, e.raison.label(), e.aire, e.confiance * 100.0), e.frame))
            .collect();
        let panel = report.panel;
        let title = format!("Coloriage par zones : {} à vérifier", rows.len());
        let (mut open, mut goto) = (true, None);
        egui::Window::new(title).id(egui::Id::new("coloriage_rapport")).default_width(380.0).default_pos(ctx.screen_rect().right_top() + vec2(-420.0, 90.0)).open(&mut open).show(ctx, |ui| {
            ui.label(egui::RichText::new("Zones laissées sans couleur (ou marquées en magenta) faute d'être sûres. Un clic va à l'image.").weak().small());
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for (text, frame) in &rows {
                    if ui.selectable_label(false, text).clicked() {
                        goto = Some(*frame);
                    }
                }
            });
        });
        if let Some(frame) = goto {
            if let Some(pi) = self.doc.panels.iter().position(|p| p.id == panel) {
                self.select_panel(pi);
                self.goto_frame(frame);
            }
        }
        if !open {
            if let Some(r) = self.coloriage.report.as_mut() {
                r.open = false;
            }
        }
    }
}

/// La tâche : les aplats de chaque image à colorier, en dessins (vectoriels ou bitmap).
#[allow(clippy::too_many_arguments)]
fn run_job(
    demande: &Demande,
    cache: &ZoneCache,
    advisor: &dyn ConseillerCouleur,
    cancel: &AtomicBool,
    done: &AtomicUsize,
    total: &AtomicUsize,
    wake: &egui::Context,
    kind: LayerKind,
    size: (usize, usize),
) -> Result<Output, String> {
    let mut cels = Vec::new();
    let mut doutes = Vec::new();
    let progress = |d: usize, t: usize| {
        done.store(d, Ordering::Relaxed);
        total.store(t, Ordering::Relaxed);
        wake.request_repaint();
    };
    let bilan = coloriage::colorier(demande, cache, advisor, cancel, &progress, &mut |image| {
        let cel = match kind {
            LayerKind::Bitmap => Cel::bitmap(coloriage::fill_raster(&image.seg, &image.couleurs)),
            LayerKind::Vector => {
                let drawing = coloriage::fill_vector(&image.seg, &image.couleurs);
                Cel { raster: drawing.render(size.0, size.1), vector: Some(drawing), version: 0, fond: None }
            }
        };
        cels.push((image.cadre, cel));
        doutes.extend(image.doutes);
    })?;
    Ok(Output { cels, doutes, bilan })
}
