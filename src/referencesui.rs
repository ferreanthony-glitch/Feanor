//! Le panneau Références (`references.rs`) : un tableau libre d'images et de vidéos à regarder pendant
//! qu'on dessine. Ouvert par son icône dans la barre des panneaux (`vue.references`) ; il se détache
//! dans sa propre fenêtre (clic droit sur l'onglet > « Dans une fenêtre à part ») pour aller sur un
//! deuxième écran, et peut y rester « toujours au-dessus ».
//!
//! - **Ajouter** : boutons (images, vidéo, coller), fichiers lâchés sur le panneau, Ctrl+V quand il a
//!   la main. Les images sont copiées dans `references/` du projet, sur un autre fil ; une vidéo est
//!   extraite en images par ffmpeg (le chemin de l'import des calques vidéo), en arrière-plan aussi.
//! - **Tableau** : glisser le fond (ou le bouton du milieu) pour se déplacer, la molette pour zoomer,
//!   glisser une référence pour la placer, ses coins pour l'agrandir, la poignée ronde pour la
//!   tourner (Maj : par 15 degrés). Maj+clic ajoute à la sélection. Alt+clic (ou Alt+glisser) prend la
//!   couleur vue sous la souris, comme la pipette du dessin. Grille et liste : deux autres façons de
//!   parcourir les mêmes références.
//! - **Vidéo** : lecture, image par image, entrée et sortie, boucle, vitesse ; « suivre la tête de
//!   lecture » montre l'image de la vidéo qui tombe sur l'image courante de SHOT, avec un décalage.
//! - **Plans** : une référence se lie au plan courant ; le filtre « références du plan courant »
//!   ne montre que celles-là.
//! - **Rapidité** : les images se décodent sur deux fils à elles, à la définition dont l'écran a
//!   besoin, et restent en mémoire graphique dans un cache borné (les moins récemment vues partent
//!   d'abord) ; le budget est une part du réglage « cache de la carte graphique » du diagnostic des
//!   performances, la définition suit le réglage « vignettes ». Au repos, rien ne repeint l'écran.
//! - **Annulation** : ajouter, retirer, déplacer, redimensionner, tourner, lier… sont des étapes de
//!   l'historique, avec un nom en français (`Edit::Structure`).
//! - **Planche** (2026-10-30, `planchref.rs`) : en plus des images, des post-it, des textes, des titres et des
//!   cadres ; un rectangle de sélection (Maj ou Ctrl+glisser sur le fond, ou l'icône « Rectangle »), ranger,
//!   recadrer, opacité, devant / derrière. Les commandes, les menus, l'import d'une autre planche et la fenêtre à part
//!   (plein écran, écran, transparence) sont dans `planchrefui.rs`.

#[path = "planchrefui.rs"]
pub(crate) mod planchrefui;
#[path = "planchdessinui.rs"]
pub(crate) mod planchdessinui;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Key, PointerButton, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, TextureHandle, TextureId, TextureOptions, Vec2};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::history::Edit;
use crate::references::{self, Clip, Filter, Imported, Kind, Lru, Reference, View};
use crate::theme;
use crate::video;

const IGNORED: &str = "les références sont des images PNG ou JPEG, ou des vidéos.";

/// Les trois façons de parcourir les références.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Layout {
    #[default]
    Board,
    Grid,
    List,
}

// ---------------------------------------------------------------------- décodage en arrière-plan

/// Une image décodée à une définition : fichier, côté le plus long (0 : tel quel), niveaux de gris.
type Key2 = (PathBuf, u32, bool);

struct Tex {
    #[allow(dead_code)] // (la poignée garde la texture en vie ; elle la libère en partant)
    handle: TextureHandle,
    id: TextureId,
    size: [usize; 2],
    rgba: Arc<Vec<u8>>,
}

struct Done {
    key: Key2,
    image: Option<(usize, usize, Vec<u8>)>,
    skipped: bool,
}

/// Deux fils qui lisent et réduisent les images demandées. Une demande dont l'écran n'a plus besoin
/// quand son tour vient est sautée (`wanted`).
struct Decoder {
    jobs: Sender<(Key2, egui::Context)>,
    done: Receiver<Done>,
    wanted: Arc<Mutex<HashSet<Key2>>>,
}

impl Decoder {
    fn new() -> Decoder {
        let (jobs, jobs_rx) = channel::<(Key2, egui::Context)>();
        let (done_tx, done) = channel::<Done>();
        let jobs_rx = Arc::new(Mutex::new(jobs_rx));
        let wanted: Arc<Mutex<HashSet<Key2>>> = Arc::default();
        for n in 0..2 {
            let (jobs_rx, done_tx, wanted) = (jobs_rx.clone(), done_tx.clone(), wanted.clone());
            let spawned = std::thread::Builder::new().name(format!("références-{n}")).spawn(move || loop {
                let job = jobs_rx.lock().unwrap_or_else(|e| e.into_inner()).recv();
                let Ok((key, ctx)) = job else { break };
                let still = wanted.lock().map(|w| w.contains(&key)).unwrap_or(true);
                // (Un fichier qui fait planter le décodeur ne fait pas tomber le fil : l'image est illisible.)
                let image = if still { std::panic::catch_unwind(|| references::decode(&key.0, key.1, key.2)).ok().flatten() } else { None };
                if done_tx.send(Done { key, image, skipped: !still }).is_err() {
                    break;
                }
                ctx.request_repaint();
            });
            if spawned.is_err() {
                break;
            }
        }
        Decoder { jobs, done, wanted }
    }
}

/// Ce que le dessin d'une référence reçoit : la texture (la meilleure déjà en mémoire), sa taille et
/// ses pixels (pour la pipette).
#[derive(Clone)]
struct Shown {
    id: TextureId,
    size: [usize; 2],
    rgba: Arc<Vec<u8>>,
}

// ---------------------------------------------------------------------- l'état

/// Une saisie en cours à la souris sur le tableau.
enum Grab {
    Pan,
    /// Les références prises (numéro, place au départ), et celle sous la souris.
    Move(Vec<(u64, Pos2)>, u64),
    Scale { id: u64, start: f32, center: Pos2, dist: f32 },
    Rotate { id: u64, start: f32, center: Pos2, angle: f32 },
    /// Un coin de cadre tiré : le cadre change de taille (son contenu ne change pas), `(hx, hy)` : le coin.
    FrameSize { id: u64, start: Reference, hx: i8, hy: i8 },
    /// Une poignée du recadrage tirée.
    Crop { id: u64, start: Reference, hx: i8, hy: i8 },
    /// Un rectangle de sélection : ce qui était choisi avant (si Maj), et si on y ajoute.
    Marquee { base: Vec<u64>, add: bool },
}

struct Drag {
    grab: Grab,
    /// Point d'écran de l'appui.
    press: Pos2,
    /// L'étape d'annulation de tout le geste (prise avant que rien ne bouge), et les références
    /// d'avant (pour Échap et pour savoir si quelque chose a bougé).
    snap: Option<Edit>,
    before: Vec<Reference>,
    serial: u64,
    epoch: u64,
    moved: bool,
}

/// Une image à ajouter : un fichier, ou des pixels (collés).
pub(crate) enum RefSource {
    File(PathBuf),
    Rgba { name: String, size: [usize; 2], rgba: Vec<u8> },
}

/// Des images qu'un fil copie dans le projet.
struct Batch {
    handle: JoinHandle<Vec<(u64, Result<Imported, String>)>>,
    dest: PathBuf,
    /// Le plan auquel lier les nouvelles références (le filtre « du plan courant » est allumé).
    link: Option<u64>,
    epoch: u64,
    /// Fichiers lâchés ou choisis en même temps qui n'étaient ni des images ni des vidéos.
    skipped: usize,
}

/// Une vidéo qu'ffmpeg extrait.
struct VideoJob {
    job: video::ImportJob,
    link: Option<u64>,
    epoch: u64,
}

/// La lecture d'une vidéo du tableau.
#[derive(Default)]
struct Vid {
    pos: f64,
    playing: bool,
    last: f64,
}

#[derive(Default)]
pub(crate) struct RefUi {
    pub layout: Layout,
    pub filter: Filter,
    /// Les références choisies ; la dernière est celle dont on règle les détails.
    pub sel: Vec<u64>,
    /// Le dernier clic était dans le panneau : Ctrl+V colle une image, Suppr supprime.
    pub focus: bool,
    /// Le fond du tableau, tiré, dessine un rectangle de sélection (sinon il déplace la vue ; Maj ou Ctrl le font
    /// aussi, et l'espace ou le bouton du milieu déplacent toujours la vue).
    pub marquee_default: bool,
    /// L'image dont on règle le recadrage.
    pub crop_mode: Option<u64>,
    /// Le texte de cet élément prend la saisie (double-clic, ou élément neuf).
    pub text_focus: Option<u64>,
    /// Le texte qu'on écrit sur place, dans le bloc (`planchrefui.rs`).
    pub(crate) edit: Option<planchrefui::EnPlace>,
    /// L'outil de la planche (sélection, crayon, gomme), l'encre, la taille et le trait en cours (`planchdessinui.rs`).
    pub(crate) pen: planchdessinui::Crayon,
    /// Échap ou Ctrl+Entrée ont été tapés pendant l'écriture (retirés des touches avant egui) : le texte est posé.
    pub(crate) edit_end: bool,
    /// Où le dernier clic droit a eu lieu sur le tableau (pour poser un post-it ou un cadre là).
    pub menu_at: Option<Pos2>,
    /// Images d'écran à attendre avant d'envoyer le plein écran (la fenêtre doit d'abord changer d'écran).
    pub fs_delay: u8,
    /// Les écrans connus, et quand on les a lus (secondes).
    pub screens: (f64, Vec<crate::planchref::Ecran>),
    /// Une planche d'un autre projet qu'un fil copie dans celui-ci.
    pub board_import: Option<planchrefui::BoardImport>,
    /// Les commandes de la fenêtre à part ont été reprises des réglages (une fois au démarrage).
    pub window_restored: bool,
    /// Images d'écran à attendre avant de relire si la planche est détachée (au démarrage, la fenêtre se refait).
    pub sync_wait: u8,
    /// La place du panneau dans la fenêtre principale si la dernière image l'y a dessiné : les
    /// fichiers lâchés dessus sont des références.
    pub pane_rect: Option<Rect>,
    pub(crate) view: Option<View>,
    /// La vue de la dernière image d'écran (celle de `view`, ou le cadrage automatique).
    pub(crate) shown_view: Option<View>,
    /// La place du tableau à la dernière image (pour cadrer une référence).
    board: Option<Rect>,
    drag: Option<Drag>,
    picking: bool,
    menu_target: Option<u64>,
    tags_buf: (u64, String),
    decoder: Option<Decoder>,
    cache: Lru<Key2, Tex>,
    inflight: HashSet<Key2>,
    failed: HashSet<Key2>,
    want: HashSet<Key2>,
    batches: Vec<Batch>,
    video_queue: VecDeque<PathBuf>,
    video_job: Option<VideoJob>,
    vids: HashMap<u64, Vid>,
    playing_every: Option<f64>,
    epoch: u64,
}

impl RefUi {
    /// Un autre projet est ouvert.
    fn reset_for_project(&mut self) {
        self.sel.clear();
        self.view = None;
        self.drag = None;
        self.picking = false;
        self.menu_target = None;
        self.focus = false;
        self.vids.clear();
        self.batches.clear();
        self.video_queue.clear();
        self.video_job = None;
        self.tags_buf = (0, String::new());
        self.failed.clear();
        self.crop_mode = None;
        self.text_focus = None;
        self.edit = None;
        self.board_import = None;
        self.pen.abandon();
    }

    /// Demande l'image `path` à la définition `level` ; rend la meilleure qui soit déjà en mémoire
    /// (celle demandée, sinon une autre définition du même fichier). L'image manquante se décode sur un
    /// autre fil, et l'écran se repeint à son arrivée.
    fn texture(&mut self, ctx: &egui::Context, path: PathBuf, level: u32, grey: bool) -> Option<Shown> {
        let key = (path, level, grey);
        self.want.insert(key.clone());
        let of = |t: &Tex| Shown { id: t.id, size: t.size, rgba: t.rgba.clone() };
        if let Some(t) = self.cache.get(&key) {
            return Some(of(t));
        }
        if !self.inflight.contains(&key) && !self.failed.contains(&key) {
            let d = self.decoder.get_or_insert_with(Decoder::new);
            d.wanted.lock().unwrap_or_else(|e| e.into_inner()).insert(key.clone());
            if d.jobs.send((key.clone(), ctx.clone())).is_ok() {
                self.inflight.insert(key.clone());
            }
        }
        let best = self.cache.keys().filter(|k| k.0 == key.0 && k.2 == grey).max_by_key(|k| if k.1 == 0 { u32::MAX } else { k.1 }).cloned();
        best.and_then(|k| self.cache.get(&k).map(of))
    }

    /// Le fichier n'a pas pu être lu.
    fn is_broken(&self, path: &PathBuf, level: u32, grey: bool) -> bool {
        self.failed.contains(&(path.clone(), level, grey))
    }

    /// Reçoit les images décodées (trois au plus par image d'écran : le téléversement à la carte
    /// graphique ne doit pas saccader l'interface).
    fn poll(&mut self, ctx: &egui::Context) {
        let Some(d) = &self.decoder else { return };
        let mut n = 0;
        while n < 3 {
            let Ok(done) = d.done.try_recv() else { return };
            self.inflight.remove(&done.key);
            match done.image {
                Some((w, h, rgba)) => {
                    let image = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
                    let name = format!("référence {} ({})", done.key.0.display(), done.key.1);
                    let handle = ctx.load_texture(name, image, TextureOptions::LINEAR);
                    let id = handle.id();
                    // (La carte graphique et la copie pour la pipette.)
                    self.cache.insert(done.key, Tex { handle, id, size: [w, h], rgba: Arc::new(rgba) }, w * h * 8);
                    n += 1;
                }
                None if !done.skipped => {
                    self.failed.insert(done.key);
                }
                None => {}
            }
        }
        // Il en reste peut-être : encore une image d'écran.
        ctx.request_repaint();
    }

    /// Fin de l'image d'écran : ce qui n'a pas servi part si le cache dépasse son budget, et le fil
    /// ne décode plus que ce que cette image a demandé.
    fn end_frame(&mut self, budget: usize) {
        if let Some(d) = &self.decoder {
            *d.wanted.lock().unwrap_or_else(|e| e.into_inner()) = self.want.clone();
        }
        self.want.clear();
        self.cache.evict(budget);
        self.cache.tick();
    }

    pub(crate) fn cache_bytes(&self) -> usize {
        self.cache.total()
    }

    /// La place du tableau à l'écran et sa vue, pour les essais qui visent à la souris.
    #[cfg(test)]
    pub(crate) fn geometry(&self) -> Option<(Rect, View)> {
        self.board.zip(self.shown_view)
    }
}

/// Le budget des textures de références : un huitième du cache de la carte graphique du diagnostic
/// des performances (128 Mo avec le réglage d'origine), entre 32 et 512 Mo.
pub(crate) fn budget_bytes(cache_carte_mo: u32) -> usize {
    (cache_carte_mo as usize / 8).clamp(32, 512) << 20
}

// ---------------------------------------------------------------------- l'application

impl App {
    /// Une étape d'annulation nommée, à prendre AVANT le changement.
    pub(crate) fn refs_step(&mut self, name: &str) {
        let before = self.history.context.replace(name.to_string());
        self.record_step();
        self.history.context = before;
    }

    /// `App::edited` avec un nom d'étape : la saisie dans un champ.
    fn refs_edited(&mut self, resp: &egui::Response, name: &str) -> bool {
        let before = self.history.context.replace(name.to_string());
        let changed = self.edited(resp);
        self.history.context = before;
        changed
    }

    /// Pose l'étape `snap`, prise avant un geste, sous le nom `name`.
    fn refs_push(&mut self, snap: Edit, name: &str) {
        let before = self.history.context.replace(name.to_string());
        self.history.push(snap);
        self.history.context = before;
        self.modified = true;
    }

    fn refs_plan_id(&self) -> Option<u64> {
        self.doc.panels.get(self.cur_panel).map(|p| p.id)
    }

    /// Le plan auquel lier ce qu'on ajoute : celui du filtre « du plan courant », s'il est allumé.
    fn refs_link_target(&self) -> Option<u64> {
        self.refs.filter.current_plan.then(|| self.refs_plan_id()).flatten()
    }

    /// Les références que le filtre laisse voir, dans l'ordre de la pile.
    fn refs_filtered(&self) -> Vec<Reference> {
        let plan = self.refs_plan_id();
        self.doc.references.iter().filter(|r| self.refs.filter.matches(r, plan)).cloned().collect()
    }

    /// Les références choisies qui existent encore.
    fn refs_selected(&self) -> Vec<u64> {
        self.refs.sel.iter().copied().filter(|id| self.doc.reference(*id).is_some()).collect()
    }

    /// Le dossier où copier les nouveaux fichiers : `references/` du projet s'il a déjà un dossier,
    /// sinon un dossier temporaire d'où ils seront recopiés à l'enregistrement.
    pub(crate) fn refs_dest_dir(&self) -> PathBuf {
        match self.path.as_ref().and_then(|m| m.parent()) {
            Some(dir) => dir.join(references::DIR),
            None => references::staging_dir(),
        }
    }

    /// Où poser une référence neuve : au centre de ce que le tableau montre.
    fn refs_spot(&self) -> Pos2 {
        references::free_spot(&self.doc.references, self.refs.view.map_or(Pos2::ZERO, |v| v.center))
    }

    /// Les images ajoutées (fichiers ou pixels) : copiées dans le projet sur un autre fil, puis posées
    /// sur le tableau (`tick_references`).
    pub(crate) fn refs_add_sources(&mut self, sources: Vec<RefSource>) {
        self.refs_add_sources_skipping(sources, 0);
    }

    /// `refs_add_sources`, en rappelant à la fin que `skipped` fichiers n'étaient pas des images.
    fn refs_add_sources_skipping(&mut self, sources: Vec<RefSource>, skipped: usize) {
        if sources.is_empty() {
            return;
        }
        let dest = self.refs_dest_dir();
        let items: Vec<(u64, RefSource)> = sources.into_iter().map(|s| (self.doc.new_reference_id(), s)).collect();
        let (d, ctx) = (dest.clone(), self.ctx.clone());
        let handle = std::thread::spawn(move || {
            let out: Vec<(u64, Result<Imported, String>)> = items
                .into_iter()
                .map(|(id, s)| {
                    let r = match s {
                        RefSource::File(p) => references::import_image(&p, &d, id),
                        RefSource::Rgba { name, size, rgba } => references::import_rgba(&name, size, &rgba, &d, id),
                    };
                    (id, r)
                })
                .collect();
            ctx.request_repaint();
            out
        });
        self.refs.batches.push(Batch { handle, dest, link: self.refs_link_target(), epoch: self.doc_epoch, skipped });
        self.status = "Import des références…".into();
    }

    /// Des fichiers (boîte de choix, fichiers lâchés) : les images deviennent des références, les
    /// vidéos s'extraient une à une.
    pub(crate) fn refs_add_paths(&mut self, paths: Vec<PathBuf>) {
        let mut images = Vec::new();
        let mut skipped = 0;
        for p in paths {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if references::IMAGE_EXTS.contains(&ext.as_str()) {
                images.push(RefSource::File(p));
            } else if crate::videoui::VIDEO_EXTS.contains(&ext.as_str()) {
                self.refs.video_queue.push_back(p);
            } else {
                skipped += 1;
            }
        }
        let has_images = !images.is_empty();
        self.refs_add_sources_skipping(images, skipped);
        if skipped > 0 && !has_images {
            self.status = format!("{skipped} fichier(s) ignoré(s) : {IGNORED}");
        }
    }

    /// Colle l'image du presse-papiers de Windows (si elle ne vient pas de SHOT).
    pub(crate) fn refs_paste(&mut self) {
        match crate::pasteimage::clipboard_image() {
            Some((w, h, rgba)) => {
                let n = self.doc.references.len() + 1;
                self.refs_add_sources(vec![RefSource::Rgba { name: format!("Image collée {n}"), size: [w, h], rgba }]);
            }
            None => self.status = "Pas d'image dans le presse-papiers de Windows.".into(),
        }
    }

    fn refs_import_images_dialog(&mut self) {
        let Some(paths) = rfd::FileDialog::new().set_title("Importer des références (images)").add_filter("Images", &references::IMAGE_EXTS).pick_files() else { return };
        self.refs_add_paths(paths);
    }

    fn refs_import_video_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new().set_title("Importer une vidéo de référence").add_filter("Vidéo", &crate::videoui::VIDEO_EXTS).pick_file() else { return };
        self.refs_add_paths(vec![path]);
    }

    /// Les commandes des références ; vrai si `action` en était une (`vue.references`, qui ouvre le
    /// panneau, est dans la barre des panneaux).
    pub(crate) fn references_action(&mut self, action: &str) -> bool {
        match action {
            "references.importer_image" => {
                self.show_pane(crate::dock::Pane::References);
                self.refs_import_images_dialog();
            }
            "references.importer_video" => {
                self.show_pane(crate::dock::Pane::References);
                self.refs_import_video_dialog();
            }
            "references.coller" => self.refs_paste(),
            "references.supprimer" => self.refs_delete_selected(),
            "references.lier_plan" => self.refs_toggle_link(),
            "references.filtre_plan" => {
                self.refs.filter.current_plan = !self.refs.filter.current_plan;
                self.status = if self.refs.filter.current_plan { "Références : seulement celles du plan courant." } else { "Références : toutes." }.into();
            }
            "references.tout_voir" => {
                self.refs.layout = Layout::Board;
                self.refs.view = None;
            }
            _ => return self.planchref_action(action) || self.planchdessin_action(action),
        }
        true
    }

    /// Le dernier clic était dans le panneau Références : Suppr supprime la sélection.
    pub(crate) fn refs_has_focus(&self) -> bool {
        self.refs.focus && !self.refs_selected().is_empty()
    }

    /// Des fichiers lâchés sur la fenêtre : s'ils tombent sur le panneau Références (docké dans cette
    /// fenêtre), ce sont des références. Faux sinon.
    pub(crate) fn refs_take_drop(&mut self, ctx: &egui::Context, paths: &[PathBuf]) -> bool {
        let Some(rect) = self.refs.pane_rect else { return false };
        let over = ctx.input(|i| i.pointer.latest_pos()).is_some_and(|p| rect.contains(p));
        if over {
            self.refs_add_paths(paths.to_vec());
        }
        over
    }

    /// À chaque image d'écran, panneau ouvert ou non : les imports qui se terminent.
    pub(crate) fn tick_references(&mut self, ctx: &egui::Context) {
        if self.refs.epoch != self.doc_epoch {
            self.refs.reset_for_project();
            self.refs.epoch = self.doc_epoch;
        }
        self.planchref_tick(ctx);
        let mut i = 0;
        while i < self.refs.batches.len() {
            if self.refs.batches[i].handle.is_finished() {
                let b = self.refs.batches.remove(i);
                self.refs_finish_batch(b);
            } else {
                i += 1;
            }
        }
        if !self.refs.batches.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        // Les vidéos, une à la fois.
        if let Some(v) = self.refs.video_job.as_mut() {
            match v.job.finished() {
                None => {
                    let n = v.job.progress.load(std::sync::atomic::Ordering::Relaxed);
                    self.status = format!("Références : extraction de « {} » : {n} images…", v.job.name);
                    ctx.request_repaint_after(Duration::from_millis(250));
                    return;
                }
                Some(result) => {
                    if let Some(v) = self.refs.video_job.take() {
                        self.refs_finish_video(v, result);
                    }
                }
            }
        }
        if self.refs.video_job.is_none() {
            if let Some(path) = self.refs.video_queue.pop_front() {
                let ffmpeg = crate::export::find_ffmpeg(self.settings.ffmpeg.as_deref());
                let job = video::start_import(ffmpeg, video::Source::Video(path), references::VIDEO_FRAME, self.doc.fps);
                self.status = format!("Références : extraction de « {} »…", job.name);
                self.refs.video_job = Some(VideoJob { job, link: self.refs_link_target(), epoch: self.doc_epoch });
                ctx.request_repaint_after(Duration::from_millis(250));
            }
        }
    }

    fn refs_finish_batch(&mut self, b: Batch) {
        let results = b.handle.join().unwrap_or_default();
        if b.epoch != self.doc_epoch {
            return;
        }
        let (mut added, mut errors) = (Vec::new(), Vec::new());
        for (id, r) in results {
            match r {
                Ok(imp) => {
                    let spot = references::free_spot(&self.doc.references.iter().cloned().chain(added.iter().cloned()).collect::<Vec<_>>(), self.refs.view.map_or(Pos2::ZERO, |v| v.center));
                    let mut r = Reference::image(id, &imp.name, &imp.file, imp.size, spot);
                    r.home = Some(b.dest.clone());
                    r.plans.extend(b.link);
                    r.stamp(&crate::commentaires::author_name(&self.settings.comment_author));
                    added.push(r);
                }
                Err(e) => errors.push(e),
            }
        }
        let n = added.len();
        if n > 0 {
            self.refs_step(if n == 1 { "Ajouter une référence" } else { "Ajouter des références" });
            self.refs.sel.clear();
            for r in added {
                self.refs.sel.push(r.id);
                self.doc.add_reference(r);
            }
            self.status = format!("{n} référence(s) ajoutée(s){}.", if b.link.is_some() { ", liée(s) au plan courant" } else { "" });
        }
        if b.skipped > 0 {
            self.status = format!("{}{} fichier(s) ignoré(s) : {IGNORED}", if n > 0 { format!("{} ", self.status) } else { String::new() }, b.skipped);
        }
        if let Some(e) = errors.first() {
            self.status = format!("{}{e}", if n > 0 || b.skipped > 0 { format!("{} Mais : ", self.status) } else { String::new() });
        }
    }

    fn refs_finish_video(&mut self, v: VideoJob, result: Result<video::Imported, String>) {
        if v.epoch != self.doc_epoch {
            return;
        }
        match result {
            Err(e) => self.status = format!("Référence vidéo impossible : {e}"),
            Ok(imp) => {
                let dir = video::temp_dir(imp.clip.id);
                // (Le son extrait avec les images n'est pas gardé.)
                let _ = std::fs::remove_file(dir.join("son.wav"));
                let first = video::frame_file(&dir, 0, &imp.clip.ext);
                let size = image::image_dimensions(&first).map(|(w, h)| [w, h]).unwrap_or([references::VIDEO_FRAME[0] as u32, references::VIDEO_FRAME[1] as u32]);
                let id = self.doc.new_reference_id();
                let name = std::path::Path::new(&imp.clip.name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| imp.clip.name.clone());
                let spot = self.refs_spot();
                let mut r = Reference::image(id, &name, &imp.clip.id.to_string(), size, spot);
                r.video = Some(Clip::new(imp.clip.frames, imp.clip.fps, &imp.clip.ext));
                r.home = dir.parent().map(std::path::Path::to_path_buf);
                r.plans.extend(v.link);
                r.stamp(&crate::commentaires::author_name(&self.settings.comment_author));
                self.refs_step("Ajouter une vidéo de référence");
                self.refs.sel = vec![id];
                self.doc.add_reference(r);
                self.status = format!("Vidéo « {name} » ajoutée aux références : {} images à {:.3} i/s.", imp.clip.frames, imp.clip.fps);
            }
        }
    }

    /// Supprime les références choisies (Ctrl+Z les rend).
    pub(crate) fn refs_delete_selected(&mut self) {
        let ids = self.refs_selected();
        if ids.is_empty() {
            self.status = "Aucune référence choisie.".into();
            return;
        }
        self.refs_step(if ids.len() == 1 { "Supprimer la référence" } else { "Supprimer les références" });
        for id in &ids {
            self.doc.remove_reference(*id);
            self.refs.vids.remove(id);
        }
        self.refs.sel.clear();
        self.status = format!("{} référence(s) supprimée(s) (Ctrl+Z les rétablit).", ids.len());
    }

    /// Change les références choisies (une étape d'annulation nommée) ; faux s'il n'y en a pas.
    pub(crate) fn refs_apply(&mut self, name: &str, mut f: impl FnMut(&mut Reference)) -> bool {
        let ids = self.refs_selected();
        if ids.is_empty() {
            self.status = "Aucune référence choisie.".into();
            return false;
        }
        self.refs_step(name);
        let now = crate::planchref::now_secs();
        for id in ids {
            if let Some(r) = self.doc.reference_mut(id) {
                f(r);
                r.touch(now);
            }
        }
        true
    }

    /// Lie les références choisies au plan courant, ou les en délie si elles y sont toutes liées.
    pub(crate) fn refs_toggle_link(&mut self) {
        let Some(plan) = self.refs_plan_id() else { return };
        let ids = self.refs_selected();
        if ids.is_empty() {
            self.status = "Aucune référence choisie.".into();
            return;
        }
        let all = ids.iter().all(|id| self.doc.reference(*id).is_some_and(|r| r.plans.contains(&plan)));
        self.refs_step(if all { "Délier la référence du plan" } else { "Lier la référence au plan" });
        for id in &ids {
            self.doc.link_reference(*id, plan, !all);
        }
        self.status = if all { "Références déliées du plan courant." } else { "Références liées au plan courant : le filtre « du plan courant » les montre." }.into();
    }

    /// Ajuste la vue du tableau sur une référence (ou sur son pixel : 100 %).
    fn refs_zoom_to(&mut self, id: u64, actual: bool, ppp: f32) {
        let Some(r) = self.doc.reference(id) else { return };
        self.refs.layout = Layout::Board;
        let rect = self.refs.board.filter(|b| b.width() > 10.0).unwrap_or_else(|| Rect::from_min_size(Pos2::ZERO, vec2(600.0, 400.0)));
        self.refs.view = Some(if actual { View { center: r.center(), zoom: View::actual_zoom(r, ppp) } } else { View::fit(r.bounds(), rect, 24.0) });
    }

    // ------------------------------------------------------------------ le panneau

    pub(crate) fn references_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let detached = ctx.viewport_id() != egui::ViewportId::ROOT;
        self.refs.poll(&ctx);
        let alive = self.refs_selected();
        self.refs.sel.retain(|id| alive.contains(id));
        // Un clic ailleurs, un autre affichage ou une fenêtre qui perd la main : le texte écrit sur place est posé.
        self.refs_edit_leave(&ctx, detached);
        if detached {
            self.refs_detached_input(&ctx);
        }
        // Fenêtre transparente : la barre d'outils garde un fond, pour rester lisible.
        let backdrop = (detached && self.settings.references_fenetre.transparent).then(|| ui.painter().add(Shape::Noop));
        let top = ui.max_rect().top();
        if detached {
            // (La fenêtre à part n'a pas de marge : le canevas blanc va jusqu'aux bords, la barre d'outils garde la sienne.)
            egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
                self.refs_toolbar(ui, true);
                self.refs_selection_bar(ui, &ctx);
            });
        } else {
            self.refs_toolbar(ui, detached);
            self.refs_selection_bar(ui, &ctx);
        }
        if let Some(i) = backdrop {
            let bar = Rect::from_min_max(pos2(ui.max_rect().left(), top), pos2(ui.max_rect().right(), ui.cursor().top() + 2.0));
            ui.painter().set(i, Shape::rect_filled(bar, 0.0, theme::fond()));
        }
        ui.add_space(2.0);
        match self.refs.layout {
            Layout::Board => self.refs_board(ui, &ctx),
            Layout::Grid => self.refs_grid(ui, &ctx),
            Layout::List => self.refs_list(ui, &ctx),
        }
        // Un clic dans le panneau lui donne la main (Ctrl+V, Suppr).
        if ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(ui.max_rect()) {
            self.refs.focus = true;
        }
        let budget = budget_bytes(self.settings.perf.cache_carte_mo);
        self.refs.end_frame(budget);
        if let Some(every) = self.refs.playing_every.take() {
            ctx.request_repaint_after(Duration::from_secs_f64(every.clamp(0.008, 0.1)));
        }
    }

    /// Dans la fenêtre à part, le clavier est celui de cette fenêtre : Ctrl+V, Suppr et Ctrl+Z y
    /// servent aux références.
    fn refs_detached_input(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
        self.refs_window_keys(ctx, focused);
        if !focused || ctx.wants_keyboard_input() {
            return;
        }
        let (paste, delete, undo, redo) = ctx.input(|i| {
            let paste = i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))) || (i.modifiers.command && i.key_released(Key::V));
            let undo = i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::Z);
            let redo = (i.modifiers.command && i.modifiers.shift && i.key_pressed(Key::Z)) || (i.modifiers.command && i.key_pressed(Key::Y));
            (paste, i.key_pressed(Key::Delete), undo, redo)
        });
        if paste {
            self.refs_paste();
        }
        if delete {
            self.refs_delete_selected();
        }
        if undo {
            self.run_action("edition.annuler");
        }
        if redo {
            self.run_action("edition.retablir");
        }
    }

    /// Fichiers lâchés sur la fenêtre à part (ceux de la fenêtre principale passent par
    /// `refs_take_drop`).
    fn refs_detached_drop(&mut self, ctx: &egui::Context) {
        let paths: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !paths.is_empty() {
            self.refs_add_paths(paths);
        }
    }

    fn refs_toolbar(&mut self, ui: &mut egui::Ui, detached: bool) {
        use crate::bulles;
        ui.horizontal_wrapped(|ui| {
            if self.bouton(ui, ph::IMAGE, "Importer des images", "PNG ou JPEG, copiés dans le dossier references/ du projet. On peut aussi les déposer sur le panneau.", "references.importer_image").clicked() {
                self.run_action("references.importer_image");
            }
            if self.bouton(ui, ph::FILM_STRIP, "Importer une vidéo", "ffmpeg en extrait les images en arrière-plan ; elles restent dans le projet.", "references.importer_video").clicked() {
                self.run_action("references.importer_video");
            }
            if self.bouton(ui, ph::CLIPBOARD, "Coller une image", "L'image du presse-papiers de Windows (copiée d'un navigateur, d'une capture…). Ctrl+V fait de même quand le panneau a la main.", "references.coller").clicked() {
                self.run_action("references.coller");
            }
            ui.separator();
            self.refs_add_icons(ui);
            ui.separator();
            self.refs_pen_icons(ui);
            ui.separator();
            let mut layout = self.refs.layout;
            bulles::choix(ui, &mut layout, Layout::Board, ph::FRAME_CORNERS, "Tableau", "Les références disposées librement : on les déplace, on les agrandit, on les tourne.", "");
            bulles::choix(ui, &mut layout, Layout::Grid, ph::SQUARES_FOUR, "Grille", "Toutes les références côte à côte, à la même taille.", "");
            bulles::choix(ui, &mut layout, Layout::List, ph::LIST, "Liste", "Une ligne par référence : nom, étiquettes, taille, plans.", "");
            self.refs.layout = layout;
            ui.separator();
            let resp = ui.add(egui::TextEdit::singleline(&mut self.refs.filter.text).hint_text("Chercher…").desired_width(110.0));
            bulles::bulle(resp, "Chercher", "Des mots du nom ou des étiquettes (sans tenir compte des accents ni des majuscules).", "");
            let tags = references::all_tags(&self.doc.references);
            if !tags.is_empty() || self.refs.filter.tag.is_some() {
                let shown = self.refs.filter.tag.clone().unwrap_or_else(|| "Étiquette".into());
                egui::ComboBox::from_id_salt("refs_etiquette").selected_text(shown).width(90.0).show_ui(ui, |ui| {
                    if ui.selectable_label(self.refs.filter.tag.is_none(), "Toutes").clicked() {
                        self.refs.filter.tag = None;
                    }
                    for t in tags {
                        if ui.selectable_label(self.refs.filter.tag.as_deref() == Some(t.as_str()), &t).clicked() {
                            self.refs.filter.tag = Some(t);
                        }
                    }
                });
            }
            let on = self.refs.filter.current_plan;
            if self.bouton_etat(ui, on, ph::FUNNEL, "Références du plan courant", "Ne montre que les références liées au plan courant (voir « Lier au plan courant »).", "references.filtre_plan").clicked() {
                self.run_action("references.filtre_plan");
            }
            if self.bouton(ui, ph::CORNERS_OUT, "Tout voir", "Cadre le tableau sur toutes les références visibles.", "references.tout_voir").clicked() {
                self.run_action("references.tout_voir");
            }
            self.refs_board_icons(ui);
            self.refs_window_icons(ui, detached);
            // (Les traits de crayon ne se comptent pas : ce sont des dessins sur la planche, pas des références.)
            let shown = self.refs_filtered().iter().filter(|r| !r.kind.is_stroke()).count();
            let all = self.doc.references.iter().filter(|r| !r.kind.is_stroke()).count();
            let (used, budget) = (self.refs.cache_bytes() >> 20, budget_bytes(self.settings.perf.cache_carte_mo) >> 20);
            ui.label(RichText::new(if shown == all { format!("{all}") } else { format!("{shown} / {all}") }).small().weak())
                .on_hover_text(format!("Références montrées / en tout. Images gardées en mémoire graphique : {used} Mo sur {budget} Mo (une part du réglage « cache de la carte graphique » du diagnostic des performances)."));
        });
    }

    /// Réglage d'une vidéo (sans changer la lecture), en une étape d'annulation nommée.
    fn refs_set_clip(&mut self, id: u64, name: &str, f: impl FnOnce(&mut Clip)) {
        self.refs_step(name);
        if let Some(c) = self.doc.reference_mut(id).and_then(|r| r.video.as_mut()) {
            f(c);
        }
    }

    /// La lecture d'une vidéo : lecture, images, boucle, vitesse, tête de lecture.
    fn refs_transport(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, r: &Reference) {
        use crate::bulles;
        let Some(c) = r.video.clone() else { return };
        let id = r.id;
        let idx = self.refs_shown_frame(r, ctx.input(|i| i.time));
        let free = !c.follow;
        ui.horizontal_wrapped(|ui| {
            let playing = self.refs.vids.get(&id).is_some_and(|v| v.playing);
            if bulles::etat_si(ui, free, playing, if playing { ph::PAUSE } else { ph::PLAY }, "Lecture / pause", "Joue la vidéo à sa cadence, entre l'entrée et la sortie.", "").clicked() {
                let start = c.range().0 as f64;
                let v = self.refs.vids.entry(id).or_default();
                if !v.playing && v.pos >= c.range().1 as f64 + 1.0 - 1e-3 {
                    v.pos = start;
                }
                v.playing = !v.playing;
                v.last = ctx.input(|i| i.time);
            }
            if bulles::icone_si(ui, free, ph::SKIP_BACK, "Image précédente", "Recule d'une image de la vidéo.", "").clicked() {
                let v = self.refs.vids.entry(id).or_default();
                v.playing = false;
                v.pos = idx.saturating_sub(1).max(c.range().0) as f64;
            }
            if bulles::icone_si(ui, free, ph::SKIP_FORWARD, "Image suivante", "Avance d'une image de la vidéo.", "").clicked() {
                let v = self.refs.vids.entry(id).or_default();
                v.playing = false;
                v.pos = (idx + 1).min(c.range().1) as f64;
            }
            let mut k = idx;
            ui.spacing_mut().slider_width = (ui.available_width() - 150.0).clamp(60.0, 260.0);
            let resp = ui.add_enabled(free, egui::Slider::new(&mut k, 0..=c.frames.saturating_sub(1)).show_value(false));
            if resp.changed() {
                let v = self.refs.vids.entry(id).or_default();
                v.playing = false;
                v.pos = k as f64;
            }
            ui.label(RichText::new(format!("{} / {}  {:.2} s", idx + 1, c.frames, idx as f64 / c.fps)).small().monospace());
        });
        ui.horizontal_wrapped(|ui| {
            if bulles::icone(ui, ph::ARROW_LINE_RIGHT, "Entrée", "L'image montrée devient le début de la lecture et de la boucle.", "").clicked() {
                self.refs_set_clip(id, "Entrée de la vidéo", |c| {
                    c.in_point = idx;
                    if c.out_point.is_some_and(|o| o < idx) {
                        c.out_point = None;
                    }
                });
            }
            if bulles::icone(ui, ph::ARROW_LINE_LEFT, "Sortie", "L'image montrée devient la fin de la lecture et de la boucle.", "").clicked() {
                self.refs_set_clip(id, "Sortie de la vidéo", |c| {
                    c.out_point = Some(idx.max(c.in_point));
                });
            }
            if bulles::etat(ui, c.looping, ph::REPEAT, "Boucle", "À la sortie, la vidéo repart de l'entrée.", "").clicked() {
                let on = !c.looping;
                self.refs_set_clip(id, "Boucle de la vidéo", |c| c.looping = on);
            }
            let mut speed = c.speed;
            let resp = ui.add(egui::DragValue::new(&mut speed).speed(0.02).range(references::SPEED_MIN..=references::SPEED_MAX).prefix("× ").max_decimals(2));
            bulles::bulle(resp.clone(), "Vitesse", "Multiplie la vitesse de lecture (aussi quand elle suit la tête de lecture).", "");
            if self.refs_edited(&resp, "Vitesse de la vidéo") {
                if let Some(x) = self.doc.reference_mut(id).and_then(|r| r.video.as_mut()) {
                    x.speed = speed.clamp(references::SPEED_MIN, references::SPEED_MAX);
                }
            }
            if bulles::etat(ui, c.follow, ph::LINK, "Suivre la tête de lecture", "L'image montrée est celle de la vidéo qui tombe sur l'image courante de FEANOR (avec le décalage, la vitesse, l'entrée, la sortie et la boucle).", "").clicked() {
                let on = !c.follow;
                self.refs_set_clip(id, "Vidéo : suivre la tête de lecture", |c| c.follow = on);
            }
            let mut offset = c.offset;
            let resp = ui.add_enabled(c.follow, egui::DragValue::new(&mut offset).speed(0.2).range(-100_000..=100_000).prefix("décalage ").suffix(" i."));
            bulles::bulle(resp.clone(), "Décalage", "En images du projet : la vidéo est à son entrée quand la tête de lecture est à l'image − décalage. Positif : la vidéo a de l'avance.", "");
            if self.refs_edited(&resp, "Décalage de la vidéo") {
                if let Some(x) = self.doc.reference_mut(id).and_then(|r| r.video.as_mut()) {
                    x.offset = offset;
                }
            }
        });
    }

    /// L'image de la vidéo `r` à montrer maintenant : celle de la tête de lecture si elle la suit, sinon
    /// celle de sa lecture propre (qui avance ici, une fois par image d'écran).
    fn refs_shown_frame(&mut self, r: &Reference, now: f64) -> usize {
        let Some(c) = &r.video else { return 0 };
        if c.follow {
            return c.follow_index(self.global_frame() as i64, self.doc.fps);
        }
        let v = self.refs.vids.entry(r.id).or_insert_with(|| Vid { pos: c.range().0 as f64, playing: false, last: now });
        if v.playing {
            let (pos, ended) = c.advance(v.pos, (now - v.last).clamp(0.0, 0.25));
            v.pos = pos;
            v.playing = !ended;
            if v.playing {
                let every = 1.0 / (c.fps * c.speed as f64).max(1.0);
                self.refs.playing_every = Some(self.refs.playing_every.map_or(every, |e| e.min(every)));
            }
        }
        v.last = now;
        c.index_of(v.pos)
    }

    /// La texture de ce qu'on voit de `r` (image, ou image de la vidéo) à `shown_px` pixels d'écran de
    /// côté ; `None` : en attente, ou illisible (`broken`).
    fn refs_texture(&mut self, ctx: &egui::Context, r: &Reference, frame: usize, shown_px: f32) -> (Option<Shown>, bool) {
        let Some(path) = r.shown_path(frame) else { return (None, true) };
        let level = references::level_for(shown_px, r.size[0].max(r.size[1]), crate::thumbs::scale_pct());
        let shown = self.refs.texture(ctx, path.clone(), level, r.grey);
        let broken = shown.is_none() && self.refs.is_broken(&path, level, r.grey);
        (shown, broken)
    }

    // ------------------------------------------------------------------ le tableau

    fn refs_board(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let size = ui.available_size().max(vec2(60.0, 60.0));
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        self.refs.board = Some(rect);
        let ppp = ctx.pixels_per_point();
        // Fenêtre à part transparente : tout ce que le tableau peint prend l'opacité choisie.
        let win_alpha = if ctx.viewport_id() != egui::ViewportId::ROOT { self.settings.references_fenetre.alpha() } else { 1.0 };
        let mut items = self.refs_filtered();
        // Écrire sur place : ce qu'on demande (double-clic, Entrée, élément neuf), et ce qu'on écrit en ce moment.
        self.refs_edit_start(ctx, &items);
        let editing = self.refs_editing();
        if let Some(r) = items.iter_mut().find(|r| Some(r.id) == editing) {
            // (Le texte qu'on écrit est droit : le bloc se redresse le temps de l'écriture, sans que rien ne change dans le fichier.)
            r.rotation = 0.0;
        }
        // Tant que personne n'a touché à la vue, elle suit la taille du panneau et ce qu'il contient (« tout voir »).
        let auto_view = self.refs.view.is_none();
        let mut view = match self.refs.view {
            Some(v) => v,
            None => match references::bounds_of(&items) {
                Some(b) => View::fit(b, rect, 30.0),
                None => View::default(),
            },
        };
        let initial_view = view;
        let painter = ui.painter_at(rect);
        // Le canevas est blanc dans tous les thèmes (fenêtre à part et plein écran compris) ; l'interface autour ne change pas.
        painter.rect_filled(rect, 0.0, crate::planchdessin::CANVAS.gamma_multiply(win_alpha));
        self.refs_pen_events(ctx);

        // Fichiers lâchés sur la fenêtre à part, et leur repère.
        if ctx.viewport_id() != egui::ViewportId::ROOT {
            self.refs_detached_drop(ctx);
        }
        let dropping = ctx.input(|i| !i.raw.hovered_files.is_empty());

        let (hover, latest) = ctx.input(|i| (i.pointer.hover_pos(), i.pointer.latest_pos()));
        let hover = hover.filter(|p| rect.contains(*p) && resp.hovered());
        let (pressed, down, mods) = ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.modifiers));
        let escape = ctx.input(|i| i.key_pressed(Key::Escape));
        let space = ctx.input(|i| i.key_down(Key::Space)) && !ctx.wants_keyboard_input();

        // Zoom à la molette, autour de la souris ; le bouton du milieu déplace le tableau.
        if let Some(p) = hover {
            let (scroll, zoom) = ctx.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let f = (scroll * 0.0025).exp() * zoom;
            if (f - 1.0).abs() > 1e-5 {
                view.zoom_about(rect, p, f);
            }
            if space {
                ctx.set_cursor_icon(egui::CursorIcon::Grab);
            }
        }
        if resp.dragged_by(PointerButton::Middle) {
            view.pan(resp.drag_delta());
        }
        // Tout sélectionner (Ctrl+A) quand le panneau a la main.
        if self.refs.focus && !ctx.wants_keyboard_input() && ctx.input(|i| i.modifiers.command && i.key_pressed(Key::A)) {
            self.refs.sel = items.iter().map(|r| r.id).collect();
        }

        // Un appui : la pipette (Alt), une poignée, une référence, ou le fond.
        let edit_rect = self.refs.edit.as_ref().and_then(|e| e.rect);
        if pressed && self.refs.drag.is_none() && !self.refs.picking {
            // (Un appui sur le bloc qu'on écrit est pour son texte.)
            if let Some(p) = hover.filter(|p| !edit_rect.is_some_and(|r| r.contains(*p))) {
                self.refs.focus = true;
                if self.refs.pen.outil != planchdessinui::Outil::Selection {
                    // Crayon ou gomme : l'appui trace ou gomme (la sélection, la pipette et les poignées dorment).
                    self.refs_pen_press(&view, rect, p, space);
                } else if mods.alt {
                    self.refs.picking = true;
                } else {
                    self.refs_press(&items, &view, rect, p, mods, space);
                }
            }
        }
        // La pipette : tant que le bouton est tenu avec Alt, la couleur suit la souris.
        if self.refs.picking {
            if down {
                if let Some(p) = latest {
                    self.refs_pick(ctx, &items, &view, rect, p);
                }
            } else {
                self.refs.picking = false;
                let c = self.color;
                self.color_used(c);
            }
        }
        // Échap ou Entrée ferment le recadrage.
        if self.refs.drag.is_none() && self.refs.crop_mode.is_some() && (escape || (self.refs.focus && ctx.input(|i| i.key_pressed(Key::Enter)) && !ctx.wants_keyboard_input())) {
            self.refs.crop_mode = None;
        }
        // Le crayon ou la gomme : le geste suit la souris ; Échap abandonne le trait, ou, sans rien d'autre à finir, rend la sélection.
        if self.refs.pen.busy() {
            self.refs_pen_update(&view, rect, down, latest, escape);
        } else if escape && self.refs.pen.outil != planchdessinui::Outil::Selection && self.refs.focus && self.refs.drag.is_none() && self.refs.edit.is_none() && self.refs.crop_mode.is_none() && !ctx.wants_keyboard_input() {
            self.refs_set_tool(planchdessinui::Outil::Selection);
        }
        // Le geste en cours.
        if let Some(mut d) = self.refs.drag.take() {
            let valid = d.epoch == self.doc_epoch && d.serial == self.history.serial();
            if !valid {
                // (Annuler ou ouvrir un autre projet pendant le geste : il est abandonné.)
            } else if escape {
                self.doc.references = d.before;
                self.status = "Geste annulé.".into();
            } else if down {
                if let Some(cur) = latest {
                    self.refs_drag_to(&mut d, &mut view, rect, cur, mods.shift, ctx.input(|i| i.pointer.delta()));
                    // Une référence qu'on sort du tableau se glisse vers la planche des Vignettes : rien ne bouge ici (`deposer.rs`).
                    if let Grab::Move(starts, _) = &d.grab {
                        if !rect.contains(cur) && starts.iter().any(|s| self.doc.reference(s.0).is_some_and(|r| !r.file.is_empty())) {
                            self.doc.references = d.before.clone();
                            let ids = starts.iter().map(|s| s.0).collect();
                            egui::DragAndDrop::set_payload(ctx, crate::deposer::RefsDrag { ids });
                        }
                    }
                }
                self.refs.drag = Some(d);
            } else {
                self.refs_end_drag(d);
            }
        }

        // Les éléments : les cadres au fond, le reste dans l'ordre de la pile.
        let now = ctx.input(|i| i.time);
        let hovered_id = hover.and_then(|p| references::hit_test_near(items.iter(), view.to_board(rect, p), 6.0 / view.zoom.max(1e-3)));
        let crop_id = self.refs.crop_mode.filter(|id| self.refs.sel == [*id]);
        if crop_id.is_none() {
            self.refs.crop_mode = None;
        }
        for pass in 0..2 {
            for r in &items {
                if (r.kind == Kind::Frame) != (pass == 0) || r.kind == Kind::Stroke {
                    continue;
                }
                let corners = r.corners().map(|c| view.to_screen(rect, c));
                let bounds = screen_bounds(&corners);
                if !bounds.intersects(rect) {
                    continue;
                }
                let alpha = (r.opacity * win_alpha).clamp(0.0, 1.0);
                if r.kind != Kind::Image {
                    paint_block(&painter, r, &corners, view.zoom, alpha, editing != Some(r.id));
                    continue;
                }
                let frame = self.refs_shown_frame(r, now);
                let shown_px = r.size[0].max(r.size[1]) as f32 * r.scale * view.zoom * ppp;
                let (shown, broken) = self.refs_texture(ctx, r, frame, shown_px);
                if crop_id == Some(r.id) {
                    // Le recadrage : l'image entière, pâle, derrière ce qui reste visible.
                    let full = r.full_local();
                    let fc = [full.left_top(), full.right_top(), full.right_bottom(), full.left_bottom()].map(|p| view.to_screen(rect, r.from_local(p.to_vec2())));
                    paint_quad(&painter, r, &fc, shown.as_ref().map(|s| s.id), broken, 0.32 * win_alpha, [0.0, 0.0, 1.0, 1.0]);
                }
                paint_quad(&painter, r, &corners, shown.as_ref().map(|s| s.id), broken, alpha, r.crop_rect());
                if let (Some(c), true) = (&r.video, self.refs.vids.get(&r.id).is_some_and(|v| v.playing)) {
                    // Les deux images d'après, pour que la lecture ne bute pas sur le décodage.
                    for ahead in 1..=2 {
                        let next = c.index_of(self.refs.vids.get(&r.id).map_or(0.0, |v| v.pos) + ahead as f64);
                        let _ = self.refs_texture(ctx, r, next, shown_px);
                    }
                }
                if r.is_video() {
                    let badge = Rect::from_min_size(bounds.left_top() + vec2(4.0, 4.0), vec2(18.0, 16.0));
                    if badge.width() < bounds.width() {
                        painter.rect_filled(badge, 3.0, Color32::from_black_alpha(150));
                        painter.text(badge.center(), Align2::CENTER_CENTER, ph::FILM_STRIP, FontId::proportional(12.0), Color32::WHITE);
                    }
                }
                if !r.plans.is_empty() && bounds.width() > 36.0 {
                    let pin = Rect::from_min_size(bounds.right_top() + vec2(-22.0, 4.0), vec2(18.0, 16.0));
                    painter.rect_filled(pin, 3.0, Color32::from_black_alpha(150));
                    painter.text(pin.center(), Align2::CENTER_CENTER, ph::PUSH_PIN, FontId::proportional(12.0), Color32::WHITE);
                }
            }
        }
        // Les traits de crayon, par-dessus tout, puis celui qu'on trace en ce moment.
        for r in items.iter().filter(|r| r.kind == Kind::Stroke) {
            let corners = r.corners().map(|c| view.to_screen(rect, c));
            if screen_bounds(&corners).expand(r.width * r.scale * view.zoom).intersects(rect) {
                crate::planchdessin::paint_stroke(&painter, r, &view, rect, (r.opacity * win_alpha).clamp(0.0, 1.0));
            }
        }
        self.refs_pen_paint(ctx, &painter, &view, rect, hover, win_alpha);
        // La sélection et ses poignées.
        let selected = self.refs_selected();
        for id in &selected {
            if let Some(r) = items.iter().find(|r| r.id == *id) {
                let corners = r.corners().map(|c| view.to_screen(rect, c));
                painter.add(Shape::closed_line(corners.to_vec(), Stroke::new(2.0_f32, crate::planchdessin::SELECTION)));
            }
        }
        if let (&[id], true) = (selected.as_slice(), editing.is_none() && self.refs.pen.outil == planchdessinui::Outil::Selection && self.refs.drag.as_ref().is_none_or(|d| matches!(d.grab, Grab::Scale { .. } | Grab::Rotate { .. } | Grab::FrameSize { .. } | Grab::Crop { .. } | Grab::Move(..)))) {
            if let Some(r) = items.iter().find(|r| r.id == id) {
                let moving = self.refs.drag.as_ref().is_some_and(|d| matches!(d.grab, Grab::Move(..)));
                if !moving {
                    for h in handles_for(r, &view, rect, crop_id == Some(id)) {
                        // Le curseur dit ce que la poignée fait.
                        if hover.is_some_and(|p| (p - h.pos).length() <= 8.0) {
                            ctx.set_cursor_icon(match h.kind {
                                Handle::Rotate => egui::CursorIcon::Grab,
                                Handle::Scale => egui::CursorIcon::ResizeNwSe,
                                Handle::Frame(hx, hy) | Handle::Crop(hx, hy) => match (hx, hy) {
                                    (0, _) => egui::CursorIcon::ResizeVertical,
                                    (_, 0) => egui::CursorIcon::ResizeHorizontal,
                                    (a, b) if a == b => egui::CursorIcon::ResizeNwSe,
                                    _ => egui::CursorIcon::ResizeNeSw,
                                },
                            });
                        }
                        let sq = Rect::from_center_size(h.pos, vec2(9.0, 9.0));
                        if matches!(h.kind, Handle::Rotate) {
                            painter.circle_filled(h.pos, 5.5, crate::planchdessin::SELECTION);
                            painter.circle_stroke(h.pos, 5.5, Stroke::new(1.5_f32, Color32::WHITE));
                        } else {
                            painter.rect_filled(sq, 1.5, crate::planchdessin::SELECTION);
                            painter.rect_stroke(sq, 1.5, Stroke::new(1.5_f32, Color32::WHITE), StrokeKind::Inside);
                        }
                    }
                }
            }
        }
        // Le texte qu'on écrit, par-dessus le bloc.
        if let Some(r) = items.iter().find(|r| Some(r.id) == editing) {
            self.refs_edit_widget(ui, ctx, r, &view, rect, (r.opacity * win_alpha).clamp(0.0, 1.0));
        }
        // Le rectangle de sélection.
        if let (Some(Drag { grab: Grab::Marquee { .. }, press, moved: true, .. }), Some(cur)) = (self.refs.drag.as_ref(), latest) {
            let area = Rect::from_two_pos(*press, cur);
            painter.rect_filled(area, 0.0, crate::planchdessin::SELECTION.gamma_multiply(0.14));
            painter.rect_stroke(area, 0.0, Stroke::new(1.0_f32, crate::planchdessin::SELECTION), StrokeKind::Inside);
        }
        // Le texte d'accueil, ou celui d'un filtre qui ne laisse rien.
        if items.is_empty() {
            let text = if self.doc.references.is_empty() {
                "Planche vide.\nDépose des images ou une vidéo ici, colle une image (Ctrl+V),\nposes-y un post-it, un texte ou un cadre avec les boutons du haut."
            } else {
                "Aucune référence ne correspond au filtre."
            };
            painter.text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(13.0), crate::planchdessin::CANVAS_TEXT_SOFT);
        }
        // La pipette : une pastille de la couleur prise près de la souris.
        if let (true, Some(p)) = (self.refs.picking, latest) {
            let c = self.color;
            painter.circle_filled(p + vec2(16.0, -16.0), 11.0, Color32::from_rgb(c[0], c[1], c[2]));
            painter.circle_stroke(p + vec2(16.0, -16.0), 11.0, Stroke::new(1.5_f32, Color32::WHITE));
            painter.circle_stroke(p + vec2(16.0, -16.0), 12.2, Stroke::new(1.0_f32, Color32::from_black_alpha(170)));
        }
        if dropping {
            painter.rect_stroke(rect.shrink(3.0), 6.0, Stroke::new(2.5_f32, theme::accent()), StrokeKind::Inside);
            painter.text(rect.center_top() + vec2(0.0, 18.0), Align2::CENTER_CENTER, "Déposer pour ajouter aux références", FontId::proportional(14.0), crate::planchdessin::SELECTION);
        }
        self.refs.shown_view = Some(view);
        if !(auto_view && view == initial_view) {
            self.refs.view = Some(view);
        }

        // Double-clic : sur un texte, l'écrire sur place ; sur une image, la cadrer ; sur le fond, tout voir.
        if resp.double_clicked() && self.refs.pen.outil == planchdessinui::Outil::Selection && !edit_rect.zip(hover).is_some_and(|(r, p)| r.contains(p)) {
            match hovered_id {
                Some(id) if self.doc.reference(id).is_some_and(|r| r.kind.has_text()) => self.refs.text_focus = Some(id),
                Some(id) => self.refs_zoom_to(id, false, ppp),
                None => self.refs.view = None,
            }
        }
        // Clic droit : le menu de la référence dessous (qui devient choisie), ou du fond.
        if resp.secondary_clicked() {
            self.refs.menu_target = hovered_id;
            self.refs.menu_at = hover.map(|p| view.to_board(rect, p));
            if let Some(id) = hovered_id {
                if !self.refs.sel.contains(&id) {
                    self.refs.sel = vec![id];
                }
            }
        }
        resp.context_menu(|ui| self.refs_menu(ui, ppp));
        // La bulle d'une référence : son nom, ses étiquettes, ses plans.
        if let (Some(id), None, None, true) = (hovered_id, self.refs.drag.as_ref(), editing, self.refs.pen.outil == planchdessinui::Outil::Selection) {
            if let Some(r) = self.doc.reference(id) {
                let tip = reference_tip(r);
                let title = r.title();
                resp.on_hover_ui_at_pointer(|ui| {
                    ui.label(RichText::new(title).strong());
                    ui.label(RichText::new(tip).small().color(theme::texte_doux()));
                });
            }
        }
    }

    /// Un appui sur le tableau (hors pipette) : une poignée de l'élément choisi, un élément, ou le fond (qui lâche
    /// la sélection et déplace la vue, ou dessine un rectangle de sélection).
    fn refs_press(&mut self, items: &[Reference], view: &View, rect: Rect, p: Pos2, mods: egui::Modifiers, space: bool) {
        let shift = mods.shift;
        let ids = self.refs_selected();
        if let [id] = ids.as_slice() {
            if let Some(r) = items.iter().find(|r| r.id == *id) {
                let crop = self.refs.crop_mode == Some(*id);
                let hs = handles_for(r, view, rect, crop);
                if let Some(h) = hs.iter().find(|h| (h.pos - p).length() <= 8.0) {
                    let center = r.center();
                    let board_p = view.to_board(rect, p);
                    let grab = match h.kind {
                        Handle::Rotate => Grab::Rotate { id: *id, start: r.rotation, center, angle: (board_p - center).angle() },
                        Handle::Scale => Grab::Scale { id: *id, start: r.scale, center, dist: (board_p - center).length().max(1e-3) },
                        Handle::Frame(hx, hy) => Grab::FrameSize { id: *id, start: r.clone(), hx, hy },
                        Handle::Crop(hx, hy) => Grab::Crop { id: *id, start: r.clone(), hx, hy },
                    };
                    self.refs_begin_drag(grab, p);
                    return;
                }
            }
        }
        // (Un appui ailleurs referme le recadrage.)
        self.refs.crop_mode = None;
        let board_p = view.to_board(rect, p);
        match references::hit_test_near(items.iter(), board_p, 6.0 / view.zoom.max(1e-3)) {
            Some(id) => {
                if shift {
                    if let Some(i) = self.refs.sel.iter().position(|x| *x == id) {
                        self.refs.sel.remove(i);
                        return;
                    }
                    self.refs.sel.push(id);
                } else if !self.refs.sel.contains(&id) {
                    self.refs.sel = vec![id];
                }
                // Un cadre emporte ce qu'il contient.
                let moving = self.doc.with_frame_contents(&self.refs_selected());
                let starts: Vec<(u64, Pos2)> = moving.into_iter().filter_map(|i| self.doc.reference(i).map(|r| (i, r.center()))).collect();
                self.refs_begin_drag(Grab::Move(starts, id), p);
            }
            None => {
                if !space && (self.refs.marquee_default || shift || mods.command || mods.ctrl) {
                    let base = if shift { self.refs_selected() } else { Vec::new() };
                    if !shift {
                        self.refs.sel.clear();
                    }
                    self.refs_begin_drag(Grab::Marquee { base, add: shift }, p);
                } else {
                    if !shift {
                        self.refs.sel.clear();
                    }
                    self.refs_begin_drag(Grab::Pan, p);
                }
            }
        }
    }

    fn refs_begin_drag(&mut self, grab: Grab, press: Pos2) {
        // (Un geste fige la vue : elle ne doit pas se recadrer pendant qu'on déplace.)
        if self.refs.view.is_none() {
            self.refs.view = self.refs.shown_view;
        }
        let no_step = matches!(grab, Grab::Pan | Grab::Marquee { .. });
        self.refs.drag = Some(Drag {
            grab,
            press,
            snap: (!no_step).then(|| self.snapshot()),
            before: self.doc.references.clone(),
            serial: self.history.serial(),
            epoch: self.doc_epoch,
            moved: false,
        });
    }

    /// La souris est en `cur` : la saisie suit (rien ne bouge avant 3 points de course).
    fn refs_drag_to(&mut self, d: &mut Drag, view: &mut View, rect: Rect, cur: Pos2, shift: bool, step: Vec2) {
        if !d.moved && (cur - d.press).length() < 3.0 {
            return;
        }
        d.moved = true;
        match &d.grab {
            Grab::Pan => view.pan(step),
            Grab::Move(starts, top) => {
                self.doc.raise_reference(*top);
                for (id, start) in starts {
                    if let Some(r) = self.doc.reference_mut(*id) {
                        let c = references::dragged_center(*start, cur - d.press, view.zoom);
                        (r.x, r.y) = (c.x, c.y);
                    }
                }
            }
            Grab::Scale { id, start, center, dist } => {
                let at = view.to_board(rect, cur);
                if let Some(r) = self.doc.reference_mut(*id) {
                    r.scale = references::dragged_scale(*start, *center, *dist, at);
                }
            }
            Grab::Rotate { id, start, center, angle } => {
                let at = view.to_board(rect, cur);
                if let Some(r) = self.doc.reference_mut(*id) {
                    r.rotation = references::dragged_rotation(*start, *center, *angle, at, shift);
                }
            }
            Grab::FrameSize { id, start, hx, hy } => {
                let at = view.to_board(rect, cur);
                let (c, size) = start.frame_resized(*hx, *hy, at);
                if let Some(r) = self.doc.reference_mut(*id) {
                    (r.x, r.y, r.size) = (c.x, c.y, size);
                }
            }
            Grab::Crop { id, start, hx, hy } => {
                let at = view.to_board(rect, cur);
                let mut n = start.clone();
                n.set_crop(Some(start.crop_from_drag(*hx, *hy, at)));
                if let Some(r) = self.doc.reference_mut(*id) {
                    (r.x, r.y, r.crop) = (n.x, n.y, n.crop);
                }
            }
            Grab::Marquee { base, add } => {
                let area = Rect::from_two_pos(view.to_board(rect, d.press), view.to_board(rect, cur));
                let visible: Vec<u64> = self.refs_filtered().iter().map(|r| r.id).collect();
                let mut sel = if *add { base.clone() } else { Vec::new() };
                for id in self.doc.references_in(area, &visible) {
                    if !sel.contains(&id) {
                        sel.push(id);
                    }
                }
                self.refs.sel = sel;
            }
        }
    }

    /// Le bouton est lâché : une étape d'annulation si quelque chose a bougé.
    fn refs_end_drag(&mut self, d: Drag) {
        let name = match d.grab {
            Grab::Pan | Grab::Marquee { .. } => return,
            Grab::Move(ref s, _) if s.len() > 1 => "Déplacer les références",
            Grab::Move(..) => "Déplacer la référence",
            Grab::Scale { .. } => "Redimensionner la référence",
            Grab::Rotate { .. } => "Tourner la référence",
            Grab::FrameSize { .. } => "Redimensionner le cadre",
            Grab::Crop { .. } => "Recadrer l'image",
        };
        if d.moved && self.doc.references != d.before {
            if let Some(snap) = d.snap {
                self.refs_push(snap, name);
            }
            let now = crate::planchref::now_secs();
            let before = d.before;
            for r in &mut self.doc.references {
                if before.iter().find(|b| b.id == r.id).is_some_and(|b| b != r) {
                    r.touch(now);
                }
            }
        }
    }

    /// Alt+clic : la couleur vue sous la souris devient la couleur courante de SHOT.
    pub(crate) fn refs_pick(&mut self, ctx: &egui::Context, items: &[Reference], view: &View, rect: Rect, p: Pos2) {
        if !rect.contains(p) {
            return;
        }
        let board_p = view.to_board(rect, p);
        let Some(r) = references::hit_test(items.iter(), board_p).and_then(|id| items.iter().find(|r| r.id == id)) else { return };
        if r.kind == Kind::Stroke {
            self.color = r.ink;
            self.status = format!("Couleur prise dans un trait : #{:02X}{:02X}{:02X}", r.ink[0], r.ink[1], r.ink[2]);
            return;
        }
        let now = ctx.input(|i| i.time);
        let frame = self.refs_shown_frame(r, now);
        let shown_px = r.size[0].max(r.size[1]) as f32 * r.scale * view.zoom * ctx.pixels_per_point();
        let (shown, _) = self.refs_texture(ctx, r, frame, shown_px);
        let Some(s) = shown else { return };
        if let Some(rgb) = r.sample(&s.rgba, s.size, board_p) {
            self.color = rgb;
            self.status = format!("Couleur prise dans « {} » : #{:02X}{:02X}{:02X}", r.name, rgb[0], rgb[1], rgb[2]);
        }
    }

    // ------------------------------------------------------------------ la grille et la liste

    /// La vignette d'un élément dans la grille ou la liste : l'image (recadrée, à son opacité) ou le bloc de texte, dans `pic`.
    fn refs_paint_thumb(&mut self, ctx: &egui::Context, painter: &egui::Painter, r: &Reference, pic: Rect, shown_px: f32, now: f64) {
        let c = r.crop_rect();
        let (w, h) = (r.size[0].max(1) as f32 * (c[2] - c[0]), r.size[1].max(1) as f32 * (c[3] - c[1]));
        let fit = (pic.width() / w).min(pic.height() / h);
        let quad = Rect::from_center_size(pic.center(), vec2(w, h) * fit);
        let corners = [quad.left_top(), quad.right_top(), quad.right_bottom(), quad.left_bottom()];
        let upright = Reference { rotation: 0.0, ..r.clone() };
        if r.kind == Kind::Image {
            let frame = self.refs_shown_frame(r, now);
            let (shown, broken) = self.refs_texture(ctx, r, frame, shown_px);
            paint_quad(painter, &upright, &corners, shown.as_ref().map(|s| s.id), broken, r.opacity, c);
        } else {
            paint_block(painter, &upright, &corners, fit / r.scale.max(1e-3), 1.0, true);
        }
    }

    /// Une référence de la grille ou de la liste qu'on prend pour la glisser vers la planche des Vignettes (celles qui sont choisies
    /// avec elle partent ensemble).
    fn refs_glisser(&mut self, ctx: &egui::Context, resp: &egui::Response, id: u64) {
        if resp.drag_started() {
            let ids = if self.refs.sel.contains(&id) { self.refs_selected() } else { vec![id] };
            egui::DragAndDrop::set_payload(ctx, crate::deposer::RefsDrag { ids });
        }
    }

    fn refs_choose(&mut self, id: u64, shift: bool) {
        self.refs.focus = true;
        if shift {
            if let Some(i) = self.refs.sel.iter().position(|x| *x == id) {
                self.refs.sel.remove(i);
            } else {
                self.refs.sel.push(id);
            }
        } else {
            self.refs.sel = vec![id];
        }
    }

    fn refs_grid(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let items: Vec<Reference> = self.refs_filtered().into_iter().filter(|r| !r.kind.is_stroke()).collect();
        if items.is_empty() {
            ui.label(RichText::new(if self.doc.references.is_empty() { "Aucune référence." } else { "Aucune référence ne correspond au filtre." }).weak());
            return;
        }
        let ppp = ctx.pixels_per_point();
        let (cell, gap, label_h) = (150.0, 8.0, 18.0);
        let now = ctx.input(|i| i.time);
        let mut open: Option<u64> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let width = ui.available_width();
            let (_, total) = references::grid_cell(0, items.len(), width, cell, gap);
            let (area, _) = ui.allocate_exact_size(vec2(width, total.max(1.0)), Sense::hover());
            let clip = ui.clip_rect();
            for (i, r) in items.iter().enumerate() {
                let rect = references::grid_cell(i, items.len(), width, cell, gap).0.translate(area.min.to_vec2());
                if !rect.intersects(clip) {
                    continue;
                }
                let resp = ui.interact(rect, ui.id().with(("reference", r.id)), Sense::click_and_drag());
                self.refs_glisser(ctx, &resp, r.id);
                let painter = ui.painter_at(rect);
                let on = self.refs.sel.contains(&r.id);
                painter.rect_filled(rect, 4.0, if on { theme::eleve() } else { theme::champ() });
                let pic = Rect::from_min_max(rect.min + vec2(4.0, 4.0), rect.max - vec2(4.0, label_h));
                self.refs_paint_thumb(ctx, &painter, r, pic, cell * ppp, now);
                painter.text(rect.left_bottom() + vec2(6.0, -label_h * 0.5), Align2::LEFT_CENTER, r.title(), FontId::proportional(11.0), theme::texte());
                if r.is_video() {
                    painter.text(rect.right_bottom() + vec2(-6.0, -label_h * 0.5), Align2::RIGHT_CENTER, ph::FILM_STRIP, FontId::proportional(12.0), theme::texte_doux());
                }
                if on {
                    painter.rect_stroke(rect, 4.0, Stroke::new(2.0_f32, theme::contour()), StrokeKind::Inside);
                }
                if resp.clicked() {
                    let shift = ui.input(|i| i.modifiers.shift);
                    self.refs_choose(r.id, shift);
                }
                if resp.double_clicked() {
                    open = Some(r.id);
                }
                if resp.secondary_clicked() {
                    self.refs.menu_target = Some(r.id);
                    if !self.refs.sel.contains(&r.id) {
                        self.refs.sel = vec![r.id];
                    }
                }
                resp.context_menu(|ui| self.refs_menu(ui, ppp));
            }
        });
        if let Some(id) = open {
            self.refs_zoom_to(id, false, ppp);
        }
    }

    fn refs_list(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let items: Vec<Reference> = self.refs_filtered().into_iter().filter(|r| !r.kind.is_stroke()).collect();
        if items.is_empty() {
            ui.label(RichText::new(if self.doc.references.is_empty() { "Aucune référence." } else { "Aucune référence ne correspond au filtre." }).weak());
            return;
        }
        let ppp = ctx.pixels_per_point();
        let row_h = 46.0;
        let now = ctx.input(|i| i.time);
        let mut open: Option<u64> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row_h, items.len(), |ui, range| {
            for r in &items[range] {
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h - 2.0), Sense::click_and_drag());
                self.refs_glisser(ctx, &resp, r.id);
                let painter = ui.painter_at(rect);
                let on = self.refs.sel.contains(&r.id);
                if on {
                    painter.rect_filled(rect, 3.0, theme::eleve());
                } else if resp.hovered() {
                    painter.rect_filled(rect, 3.0, theme::champ());
                }
                let pic = Rect::from_min_size(rect.min + vec2(3.0, 3.0), vec2(56.0, rect.height() - 6.0));
                self.refs_paint_thumb(ctx, &painter, r, pic, 56.0 * ppp, now);
                let x = pic.right() + 8.0;
                painter.text(pos2(x, rect.top() + 12.0), Align2::LEFT_CENTER, r.title(), FontId::proportional(12.5), theme::texte());
                let mut sub = if r.kind == Kind::Image { format!("{}×{}", r.size[0], r.size[1]) } else { crate::planchref::kind_name(r.kind).to_string() };
                if let Some(c) = &r.video {
                    sub.push_str(&format!("  ·  vidéo, {} images", c.frames));
                }
                if !r.plans.is_empty() {
                    sub.push_str(&format!("  ·  {} plan(s)", r.plans.len()));
                }
                if !r.tags.is_empty() {
                    sub.push_str(&format!("  ·  {}", r.tags.join(", ")));
                }
                painter.text(pos2(x, rect.top() + 30.0), Align2::LEFT_CENTER, sub, FontId::proportional(10.5), theme::texte_doux());
                if on {
                    painter.rect_stroke(rect, 3.0, Stroke::new(1.5_f32, theme::contour()), StrokeKind::Inside);
                }
                if resp.clicked() {
                    let shift = ui.input(|i| i.modifiers.shift);
                    self.refs_choose(r.id, shift);
                }
                if resp.double_clicked() {
                    open = Some(r.id);
                }
                if resp.secondary_clicked() {
                    self.refs.menu_target = Some(r.id);
                    if !self.refs.sel.contains(&r.id) {
                        self.refs.sel = vec![r.id];
                    }
                }
                resp.context_menu(|ui| self.refs_menu(ui, ppp));
            }
        });
        if let Some(id) = open {
            self.refs_zoom_to(id, false, ppp);
        }
    }
}

// ---------------------------------------------------------------------- dessin

/// Le rectangle d'écran qui contient les quatre coins.
fn screen_bounds(c: &[Pos2; 4]) -> Rect {
    let mut r = Rect::from_min_max(c[0], c[0]);
    for p in &c[1..] {
        r.extend_with(*p);
    }
    r
}

/// Ce qu'une poignée fait.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Handle {
    Scale,
    Rotate,
    /// Un coin de cadre (changer sa taille), `(hx, hy)` de -1 à 1.
    Frame(i8, i8),
    /// Un coin ou un milieu de côté du recadrage.
    Crop(i8, i8),
}

struct HandleAt {
    pos: Pos2,
    kind: Handle,
}

/// Les poignées de l'élément choisi, en points d'écran. Image ou texte : les quatre coins (agrandir) puis le rond au-dessus
/// du bord haut (tourner). Cadre : ses quatre coins (changer sa taille), pas de rotation. Recadrage en cours : les huit
/// poignées du rectangle visible.
fn handles_for(r: &Reference, view: &View, rect: Rect, crop: bool) -> Vec<HandleAt> {
    let at = |v: Vec2| view.to_screen(rect, r.from_local(v));
    if crop {
        let mut out = Vec::new();
        for hy in -1..=1_i8 {
            for hx in -1..=1_i8 {
                if hx != 0 || hy != 0 {
                    out.push(HandleAt { pos: at(r.crop_handle_local(hx, hy)), kind: Handle::Crop(hx, hy) });
                }
            }
        }
        return out;
    }
    if r.kind == Kind::Frame {
        return [(-1, -1), (1, -1), (1, 1), (-1, 1)].into_iter().map(|(hx, hy)| HandleAt { pos: at(r.crop_handle_local(hx, hy)), kind: Handle::Frame(hx, hy) }).collect();
    }
    let mut out: Vec<HandleAt> = r.corners().iter().map(|c| HandleAt { pos: view.to_screen(rect, *c), kind: Handle::Scale }).collect();
    let top = view.to_screen(rect, r.from_local(vec2(0.0, -r.half().y)));
    let center = view.to_screen(rect, r.center());
    let away = (top - center).normalized();
    let away = if away.is_finite() { away } else { vec2(0.0, -1.0) };
    out.push(HandleAt { pos: top + away * 22.0, kind: Handle::Rotate });
    out
}

/// Dessine la texture sur le quadrilatère `corners` (haut-gauche, haut-droite, bas-droite,
/// bas-gauche), miroirs compris, à l'opacité `alpha` ; `crop` : la part visible de l'image (0 à 1) ; à défaut de texture,
/// un cadre qui attend (ou signale l'image illisible).
fn paint_quad(painter: &egui::Painter, r: &Reference, corners: &[Pos2; 4], tex: Option<TextureId>, broken: bool, alpha: f32, crop: [f32; 4]) {
    match tex {
        Some(id) => {
            let (u0, u1) = if r.flip_h { (crop[2], crop[0]) } else { (crop[0], crop[2]) };
            let (v0, v1) = if r.flip_v { (crop[3], crop[1]) } else { (crop[1], crop[3]) };
            let uvs = [pos2(u0, v0), pos2(u1, v0), pos2(u1, v1), pos2(u0, v1)];
            let tint = Color32::from_white_alpha((alpha.clamp(0.0, 1.0) * 255.0).round() as u8);
            let mut mesh = egui::Mesh::with_texture(id);
            for (p, uv) in corners.iter().zip(uvs) {
                mesh.vertices.push(egui::epaint::Vertex { pos: *p, uv, color: tint });
            }
            mesh.indices.extend([0, 1, 2, 0, 2, 3]);
            painter.add(Shape::mesh(mesh));
        }
        None => {
            painter.add(Shape::convex_polygon(corners.to_vec(), theme::champ(), Stroke::new(1.0_f32, theme::bord())));
            let c = screen_bounds(corners).center();
            painter.text(c, Align2::CENTER_CENTER, if broken { ph::IMAGE_BROKEN } else { ph::IMAGE }, FontId::proportional(22.0), theme::texte_doux());
            if broken {
                painter.text(c + vec2(0.0, 20.0), Align2::CENTER_CENTER, "fichier illisible ou introuvable", FontId::proportional(10.5), theme::texte_doux());
            }
        }
    }
}

/// Dessine un post-it, un texte, un titre ou un cadre sur le quadrilatère `corners`, à l'opacité `alpha` ; `zoom` : les points
/// d'écran par unité du tableau. Le texte tourne avec l'élément ; il n'est pas dessiné quand il serait illisible.
fn paint_block(painter: &egui::Painter, r: &Reference, corners: &[Pos2; 4], zoom: f32, alpha: f32, show_text: bool) {
    let a = alpha.clamp(0.0, 1.0);
    let tint = r.tint();
    let paint = |c: [u8; 3], op: f32| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], (op * a * 255.0).round() as u8);
    let s = r.scale * zoom;
    let u = (corners[1] - corners[0]).normalized();
    let v = (corners[3] - corners[0]).normalized();
    let (u, v) = if u.is_finite() && v.is_finite() { (u, v) } else { (vec2(1.0, 0.0), vec2(0.0, 1.0)) };
    let bounds = screen_bounds(corners);
    let (text_color, hint) = block_text_look(r, a);
    match r.kind {
        Kind::Note => {
            let shadow = corners.map(|c| c + vec2(3.0, 4.0) * s.clamp(0.25, 1.5));
            painter.add(Shape::convex_polygon(shadow.to_vec(), Color32::from_black_alpha((70.0 * a) as u8), Stroke::NONE));
            painter.add(Shape::convex_polygon(corners.to_vec(), paint(tint, 1.0), Stroke::NONE));
        }
        Kind::Frame => {
            // (Le trait du cadre est la teinte assombrie : la teinte pleine est trop pâle sur le blanc.)
            let edge = crate::planchdessin::ink_on_canvas(tint).map(|v| (v as f32 * 1.55).min(255.0) as u8);
            painter.add(Shape::convex_polygon(corners.to_vec(), paint(tint, 0.09), Stroke::new(1.5_f32, paint(edge, 0.9))));
            let h = crate::planchref::FRAME_STRIP * s;
            let strip = [corners[0], corners[1], corners[1] + v * h, corners[0] + v * h];
            painter.add(Shape::convex_polygon(strip.to_vec(), paint(tint, 0.85), Stroke::NONE));
        }
        _ => {}
    }
    let px = block_font_px(r, s);
    if !show_text || px < 3.0 || bounds.width() < 8.0 {
        return;
    }
    let pad = crate::planchref::TEXT_PAD * s;
    let (text, color) = if r.text.is_empty() { (hint.to_string(), text_color.gamma_multiply(0.45)) } else { (r.text.clone(), text_color) };
    let font = FontId::proportional(px);
    let (origin, galley) = if r.kind == Kind::Frame {
        let h = crate::planchref::FRAME_STRIP * s;
        (corners[0] + u * pad * 0.8 + v * ((h - px * 1.25) * 0.5).max(0.0), painter.layout_no_wrap(text, font, color))
    } else {
        let wrap = ((r.size[0] as f32 - 2.0 * crate::planchref::TEXT_PAD) * s).max(px);
        (corners[0] + u * pad + v * pad, painter.layout(text, font, color, wrap))
    };
    let clipped = painter.with_clip_rect(bounds.intersect(painter.clip_rect()).expand(1.0));
    clipped.add(egui::epaint::TextShape::new(origin, galley, color).with_angle(r.rotation.to_radians()));
}

/// La couleur du texte d'un bloc (à l'opacité `a`) et ce que dit son champ vide ; la même pour le texte dessiné et pour le
/// texte qu'on écrit sur place.
pub(crate) fn block_text_look(r: &Reference, a: f32) -> (Color32, &'static str) {
    let ink = |c: Color32| c.gamma_multiply(a);
    let tint = r.tint();
    // (Un texte coloré se lit sur le blanc du canevas : la teinte du post-it, assombrie.)
    let dark = crate::planchdessin::ink_on_canvas(tint);
    let own_dark = || Color32::from_rgba_unmultiplied(dark[0], dark[1], dark[2], (a * 255.0).round() as u8);
    match r.kind {
        Kind::Note => (ink(Color32::from_rgb(38, 34, 26)), "Double-clic pour écrire"),
        Kind::Frame => (ink(Color32::from_rgb(38, 34, 26)), "Cadre"),
        Kind::Title => (if r.color == 0 { ink(crate::planchdessin::CANVAS_TEXT) } else { own_dark() }, "Titre"),
        _ => (if r.color == 0 { ink(crate::planchdessin::CANVAS_TEXT) } else { own_dark() }, "Texte"),
    }
}

/// La taille de lettre d'un bloc à l'écran, `s` : points d'écran par unité du tableau (échelle de l'élément comprise).
pub(crate) fn block_font_px(r: &Reference, s: f32) -> f32 {
    crate::planchref::font_size(r.kind) * if r.kind == Kind::Frame { 1.0 } else { r.text_fit } * s
}

/// Le texte d'une bulle : taille, étiquettes, plans, auteur et date.
fn reference_tip(r: &Reference) -> String {
    let mut s = if r.kind == Kind::Image { format!("{}×{} pixels", r.size[0], r.size[1]) } else { crate::planchref::kind_name(r.kind).to_string() };
    if let Some(c) = &r.video {
        s.push_str(&format!(", vidéo de {} images à {:.3} i/s", c.frames, c.fps));
    }
    if !r.tags.is_empty() {
        s.push_str(&format!("\nÉtiquettes : {}", r.tags.join(", ")));
    }
    if !r.plans.is_empty() {
        s.push_str(&format!("\nLiée à {} plan(s)", r.plans.len()));
    }
    if !r.author.is_empty() || r.created > 0 {
        let who = if r.author.is_empty() { String::new() } else { format!(" par {}", r.author) };
        let when = if r.created > 0 { format!(", le {}", crate::planchref::date_fr(r.created)) } else { String::new() };
        s.push_str(&format!("\nPosé{who}{when}"));
    }
    s
}

