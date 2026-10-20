//! Onglets de projets (Anthony, 2026-10-20 : « ouvrir plusieurs fichiers dans la même fenêtre avec un
//! système d'onglets »). Conception complète : `docs/ARCHITECTURE.md`, « Onglets de projets ».
//!
//! Un onglet est un projet ouvert, en mémoire, avec tout ce qui ne vaut que pour lui : le document,
//! son historique, la tête de lecture, la sélection, la vue, les caches, les tâches de fond. Tous ces champs
//! restent des champs d'`App` — des dizaines de milliers de lignes écrivent `self.doc`, `self.history`… —
//! et **l'onglet affiché est celui qui est dans ces champs** ; les autres sont rangés dans un
//! `EtatProjet`, que l'on échange avec les champs d'`App` (`std::mem::swap`, quelques microsecondes) au
//! changement d'onglet. La macro `etat_projet!` liste chaque champ de projet une seule fois : elle écrit
//! la structure, l'échange et l'état d'un onglet neuf.
//!
//! Ce qui ne dépend pas d'un projet (réglages, raccourcis, thème, panneaux, outil, pinceaux, couleurs,
//! presse-papiers, carte graphique, son) n'est pas dans l'échange : un onglet ne change pas d'outil, de
//! couleur ni de disposition, et ce qu'on copie dans l'un se colle dans l'autre.
//!
//! Les caches indexés par numéro de dessin (`CelId`, qui naît d'un compteur par projet : deux projets ont
//! tous deux un dessin 3) suivent leur onglet : effets calculés (`fxcache::Reserve`) et textures de la
//! carte graphique (`gpu::TexturesDeProjet`) sont rangés avec lui et rendus à son retour ; les demandes en
//! cours des vignettes sont abandonnées à la sortie.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use eframe::egui::{self, Pos2, Vec2};

use crate::app::{App, Mode, Pending};
use crate::document::{CelId, Document};
use crate::view::View;

/// Compteur des époques de projet (`App::doc_epoch`) : une époque n'est jamais donnée deux fois, deux
/// onglets n'ont donc jamais la même. Les tâches de fond qui notent l'époque du projet pour lequel
/// elles travaillent (enregistrement, références, crayonnés, envois, comparaison) ne répondent ainsi
/// qu'à lui, qu'il soit affiché ou non.
static EPOQUE: AtomicU64 = AtomicU64::new(1);

/// Une époque neuve.
pub(crate) fn nouvelle_epoque() -> u64 {
    EPOQUE.fetch_add(1, Ordering::Relaxed)
}

/// Liste, une fois, les champs d'`App` qui sont propres à un projet : écrit `EtatProjet` (ces champs, plus
/// ce que l'onglet range à côté), `EtatProjet::vierge` (l'état d'un onglet neuf) et `App::echanger_etat`.
macro_rules! etat_projet {
    ($gpu:ident; $($champ:ident : $ty:ty = $init:expr),* $(,)?) => {
        /// Tout ce qu'un projet ouvert tient en mémoire en dehors du document lui-même, rangé pendant que
        /// son onglet n'est pas affiché.
        pub(crate) struct EtatProjet {
            $(pub(crate) $champ: $ty,)*
            /// Ses dessins dérivés des effets (cache global rangé avec l'onglet, `fxcache::Reserve`).
            pub(crate) reserve_fx: Option<crate::fxcache::Reserve>,
            /// Ses textures sur la carte graphique.
            pub(crate) textures_gpu: Option<crate::gpu::TexturesDeProjet>,
        }

        impl EtatProjet {
            /// L'état d'un onglet neuf : un projet vide (l'accueil est montré), rien en mémoire.
            /// `gpu` : la composition passe par la carte graphique.
            pub(crate) fn vierge($gpu: bool) -> EtatProjet {
                EtatProjet {
                    $($champ: $init,)*
                    reserve_fx: None,
                    textures_gpu: None,
                }
            }
        }

        impl App {
            /// Échange les champs de projet d'`App` avec `e` : l'onglet qui était affiché se retrouve dans
            /// `e`, celui qui y était rangé est affiché.
            pub(crate) fn echanger_etat(&mut self, e: &mut EtatProjet) {
                $(std::mem::swap(&mut self.$champ, &mut e.$champ);)*
            }
        }

        /// Les noms des champs échangés (l'essai de garde les compare à ceux d'`App`).
        #[cfg(test)]
        pub(crate) const CHAMPS_DE_PROJET: &[&str] = &[$(stringify!($champ)),*];
    };
}

etat_projet! {
    gpu;
    // --- le document et ce qui l'accompagne
    doc: Document = Document::with_frame(1920, 1080, 24).created_now(),
    history: crate::history::History = Default::default(),
    gesture: crate::history::Gesture = Default::default(),
    path: Option<PathBuf> = None,
    import_source: Option<PathBuf> = None,
    written: std::collections::HashMap<CelId, u64> = std::collections::HashMap::new(),
    modified: bool = false,
    doc_epoch: u64 = nouvelle_epoque(),
    welcome: bool = true,
    // --- où l'on est
    mode: Mode = Mode::Storyboard,
    cur_panel: usize = 0,
    cur_layer: usize = 0,
    cur_frame: usize = 0,
    heard_frame: Option<usize> = Some(0),
    // --- lecture
    playing: bool = false,
    play_montage: bool = false,
    play_accum: f64 = 0.0,
    last_time: Option<f64> = None,
    // --- vue
    view: View = View::default(),
    fit_pending: bool = true,
    onion_offset: Vec2 = Vec2::ZERO,
    timeline_cell: f32 = crate::timeline::CELL_W,
    timeline_fit: Option<(usize, usize)> = None,
    montage_zoom: f32 = 4.0,
    strip_geo: [crate::zoombande::StripGeo; 2] = Default::default(),
    strip_anchor: [Option<crate::zoombande::Anchor>; 2] = [None, None],
    montage_fit: bool = false,
    camera_view: bool = false,
    stage_view: Option<View> = None,
    format_custom: bool = false,
    in_card: bool = false,
    board_zoom: f32 = 0.0,
    board_pan: Vec2 = Vec2::ZERO,
    xsheet_seen_frame: Option<usize> = None,
    plan_len_seen: Option<(u64, usize)> = None,
    // --- sélections
    crayonnes: crate::crayonnesui::Etat = Default::default(),
    layer_sel: Vec<u64> = Vec::new(),
    frame_sel: Option<(usize, usize)> = None,
    key_sel: Vec<crate::montagekeys::KeyRef> = Vec::new(),
    key_anchor: Option<crate::montagekeys::KeyRef> = None,
    cam_key_sel: Vec<crate::camkeys::CamKeyRef> = Vec::new(),
    cam_key_anchor: Option<crate::camkeys::CamKeyRef> = None,
    fx_choice: Option<crate::fxkeys::FxSel> = None,
    fx_key_sel: Vec<crate::fxkeys::FxKeyRef> = Vec::new(),
    fx_key_anchor: Option<crate::fxkeys::FxKeyRef> = None,
    board_sel: Vec<u64> = Vec::new(),
    board_notes: Vec<crate::board::NoteRef> = Vec::new(),
    sel_clip: Option<u64> = None,
    vsel: Option<crate::vtools::VSelection> = None,
    bsel: Option<crate::pixtools::BSelection> = None,
    sel_box: Option<crate::seltools::SelBox> = None,
    sel_contours: std::cell::RefCell<Option<crate::contours::Contours>> = Default::default(),
    multi: crate::multisel::Multi = Default::default(),
    flottante: Option<crate::flottante::Flottante> = None,
    guides: crate::guides::GuideState = Default::default(),
    comment_ui: crate::commentaires::CommentUi = Default::default(),
    hole: crate::images::Hole = Default::default(),
    edge_sel: Option<crate::rognageui::EdgeSel> = None,
    trans: crate::transitionsui::TransUi = Default::default(),
    sfx: crate::audiofxui::SoundFxUi = Default::default(),
    menu_target: Option<crate::soundtrack::MenuTarget> = None,
    panel_focus: bool = false,
    images_focus: bool = false,
    key_focus: bool = false,
    // --- gestes en cours
    stroke: Option<crate::brush::StrokeState> = None,
    vstroke: Option<crate::vtools::VectorStroke> = None,
    verase: Option<crate::vtools::VectorErase> = None,
    vdrag: Option<crate::vtools::VDrag> = None,
    bdrag: Option<crate::pixtools::BDrag> = None,
    fond_stroke: Option<CelId> = None,
    fond_cut: Option<crate::brush::StrokeState> = None,
    cam_drag: Option<crate::camtools::CamDrag> = None,
    layer_drag: Option<crate::layertools::LayerDrag> = None,
    head_drag: Option<usize> = None,
    panel_layer_drag: Option<usize> = None,
    montage_layer_drag: Option<usize> = None,
    keys_drag: Option<crate::montagekeys::KeysDrag> = None,
    key_rect: Option<crate::montagekeys::KeyRect> = None,
    key_drag_delta: isize = 0,
    key_drag_mode: crate::handles::KeyMove = crate::handles::KeyMove::Ripple,
    cam_keys_drag: Option<crate::camkeys::CamKeysDrag> = None,
    cam_rect: Option<crate::camkeys::CamRect> = None,
    fx_keys_drag: Option<crate::fxkeys::FxKeysDrag> = None,
    ctrl_lasso: Option<crate::springtools::CtrlLasso> = None,
    image_drag: Option<(usize, usize)> = None,
    grid_select: Option<crate::handles::GridSelect> = None,
    handle_drag: Option<crate::handles::HandleDrag> = None,
    grid_menu: Option<crate::handles::GridMenu> = None,
    montage_drag: Option<crate::storyboard::MontageDrag> = None,
    board_drag: Option<crate::boardui::BoardDrag> = None,
    board_text: Option<crate::boardui::NoteTextEdit> = None,
    top_drag: Option<crate::topview::TopDrag> = None,
    shape_drag: Option<(Pos2, Pos2)> = None,
    shape_edit: Option<crate::shapes::ShapeEdit> = None,
    text_edit: Option<crate::texttools::TextEditState> = None,
    cut_gesture: Option<Vec<Pos2>> = None,
    envelope: Option<crate::envelopetools::EnvelopeEdit> = None,
    persp: Option<crate::perstools::PerspEdit> = None,
    length_origin: Option<crate::rognageui::PlanBackup> = None,
    // --- fenêtres et saisies qui visent ce projet
    tween_dialog: Option<crate::tweenui::TweenDialog> = None,
    layer_rename: Option<crate::layersui::LayerRename> = None,
    instance_rename: Option<crate::images::InstanceRename> = None,
    smart_rename: Option<crate::images::SmartRename> = None,
    res_edit: Option<crate::projetui::ResolutionEdit> = None,
    duration_dialog: Option<crate::rognageui::DurationDialog> = None,
    trim_menu: Option<(u64, usize)> = None,
    goto_plan: Option<String> = None,
    renumber_confirm: bool = false,
    new_layer_dialog: Option<crate::newlayer::NewLayerForm> = None,
    effets_ui: crate::prereglagesui::EffetsUi = Default::default(),
    pending: Option<Pending> = None,
    timeline_sound_track: Option<usize> = None,
    flip_run: Option<crate::flipsui::FlipRun> = None,
    flip_manual: Option<(usize, usize)> = None,
    // --- vue 3D, références, envois, versions
    ref3d: crate::ref3dtool::Ref3dState = Default::default(),
    stage3d: crate::stage3dui::Stage3dState = Default::default(),
    scene3d: crate::scene3dui::Scene3dState = Default::default(),
    refs: crate::referencesui::RefUi = Default::default(),
    envois_ui: crate::envoisui::EnvoisUi = Default::default(),
    comparaison: Option<crate::comparerui::Comparaison> = None,
    pile_versions: crate::pilevers::Pile = Default::default(),
    purge_versions: Option<Option<u64>> = None,
    // Le tracé droit en cours (Maj tenue) et les clés de position choisies sur la timeline.
    droit: crate::tracedroit::Droit = Default::default(),
    pos_ui: crate::poskeysui::PosUi = Default::default(),
    // La relecture : sa fenêtre, sa page en calcul et les notes lues visent ce projet.
    relecture: crate::relectureui::RelectureUi = Default::default(),
    // Le film partagé de ce projet : sa session (son fil de fond continue pour un onglet caché) et son interface.
    partage: Option<crate::cooperation::Session> = None,
    equipe_ui: crate::cooperationui::EquipeUi = Default::default(),
    // --- caches (rangés avec l'onglet : rien ne s'en échappe vers un autre projet)
    renderer: crate::render::Renderer = {
        let mut r = crate::render::Renderer::new();
        r.set_gpu(gpu);
        r
    },
    monde: crate::monde::Monde = crate::monde::Monde::new(),
    thumbs: crate::thumbs::Thumbs = Default::default(),
    layer_thumbs: crate::layersui::LayerThumbs = Default::default(),
    murs: crate::calquecouleur::Cache = Default::default(),
    rejeu: crate::rejeu::Rejeu = crate::rejeu::Rejeu::new(),
    rejeu_again: std::collections::BTreeSet<CelId> = Default::default(),
    rejeu_warm: std::collections::HashMap<CelId, u64> = Default::default(),
    rejeu_deferred: Vec<crate::rejeu::Done> = Vec::new(),
    rejeu_idle: Vec<CelId> = Vec::new(),
    // --- tâches de fond
    saving: Option<crate::enregistrement::Enregistrement> = None,
    save_queue: Option<(PathBuf, Option<Pending>)> = None,
    after_save: Option<Pending> = None,
    save_error: Option<String> = None,
    open_job: Option<crate::ouverture::OpenJob> = None,
    video_import: Option<crate::videoui::VideoImport> = None,
    recovery: Option<crate::recovery::Recovery> = None,
}


/// Un onglet : un projet ouvert.
pub(crate) struct Onglet {
    /// Numéro de l'onglet, unique pour la séance (il ne change pas quand on les déplace).
    pub id: u64,
    /// L'état du projet quand l'onglet n'est pas affiché ; `None` pour l'onglet affiché, dont l'état est
    /// dans les champs d'`App`.
    pub etat: Option<Box<EtatProjet>>,
    /// La dernière fois qu'il a été affiché (pour lâcher les caches des plus anciens).
    pub vu_le: Instant,
}

/// Ce que la bande d'onglets a besoin de savoir d'un onglet.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InfosOnglet {
    pub id: u64,
    pub nom: String,
    pub chemin: Option<PathBuf>,
    /// Modifié depuis le dernier enregistrement.
    pub modifie: bool,
    /// Une écriture est en cours (le projet n'est pas encore sur le disque).
    pub ecriture: bool,
    pub actif: bool,
    /// Jamais servi : l'accueil (« Nouvel onglet »).
    pub vierge: bool,
}

impl InfosOnglet {
    /// Le projet a quelque chose à perdre (modifié, ou pas encore écrit) : la pastille « • » de l'onglet.
    pub(crate) fn a_perdre(&self) -> bool {
        self.modifie || self.ecriture
    }
}

/// Le nom d'un projet pour son onglet : celui de son fichier (ou de l'ancien fichier importé).
pub(crate) fn nom_de(path: Option<&Path>, import_source: Option<&Path>, vierge: bool) -> String {
    match path.or(import_source).and_then(|p| p.file_stem()) {
        Some(s) => s.to_string_lossy().into_owned(),
        None if vierge => "Nouvel onglet".into(),
        None => "Sans titre".into(),
    }
}

/// Le même projet, comparé comme Windows le fait (casse ignorée).
pub(crate) fn meme_chemin(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

/// Le message d'un collage refusé parce que la copie vient d'une feuille d'une autre taille (un dessin n'a de sens
/// qu'à sa taille). `quoi` : le sujet et le verbe (« Ce calque vient », « Ces plans viennent »).
pub(crate) fn refus_de_taille(quoi: &str, de: [usize; 2], vers: [usize; 2]) -> String {
    format!(
        "Collage refusé : {quoi} d'un projet dont la feuille fait {}×{} px, celle-ci fait {}×{} px (un dessin n'a de sens qu'à sa taille).",
        de[0], de[1], vers[0], vers[1]
    )
}

/// Plusieurs onglets à enregistrer ou à fermer d'un coup (quitter, fermer les autres) : la boîte qui les liste.
pub(crate) struct Groupe {
    pub action: ActionGroupe,
    /// Les onglets modifiés (numéro, nom) au moment de la demande.
    pub onglets: Vec<(u64, String)>,
    /// « Enregistrer tout » a été choisi (ou rien n'était à décider, des écritures finissent) : on attend la
    /// fin des écritures avant l'action.
    pub attente: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActionGroupe {
    /// Quitter SHOT.
    Quitter,
    /// Fermer tous les onglets sauf celui-ci (son numéro).
    FermerAutres(u64),
}

impl App {
    // ------------------------------------------------------------------ l'état de départ

    /// Au démarrage : un seul onglet, l'état d'`App`.
    pub(crate) fn onglets_init(&mut self) {
        // (Les champs d'`App` deviennent ceux d'un onglet neuf : `vierge` est l'unique source des valeurs de départ.)
        let mut vierge = EtatProjet::vierge(self.gpu.is_some());
        self.echanger_etat(&mut vierge);
        drop(vierge);
        // (Époque 0, celle que les états remis à zéro d'un projet à l'autre — références, crayonnés, envois — ont au
        // départ : le premier projet n'a rien à remettre. Toutes les suivantes viennent du compteur, à partir de 1.)
        self.doc_epoch = 0;
        self.onglets = vec![Onglet { id: 1, etat: None, vu_le: Instant::now() }];
        self.actif = 0;
        self.prochain_onglet = 2;
    }

    // ------------------------------------------------------------------ renseignements

    /// Le projet n'a jamais servi : l'accueil, rien d'ouvert ni de dessiné. Un projet qu'on ouvre y prend la place.
    fn est_vierge(welcome: bool, path: &Option<PathBuf>, import: &Option<PathBuf>, modified: bool, intact: bool, saving: bool, ouvre: bool) -> bool {
        welcome && path.is_none() && import.is_none() && !modified && intact && !saving && !ouvre
    }

    /// L'onglet affiché n'a servi à rien.
    pub(crate) fn onglet_vierge(&self) -> bool {
        Self::est_vierge(self.welcome, &self.path, &self.import_source, self.modified, self.history.is_empty(), self.saving.is_some(), self.open_job.is_some())
    }

    /// Ce que la bande d'onglets montre de l'onglet `i`.
    pub(crate) fn infos_onglet(&self, i: usize) -> InfosOnglet {
        let t = &self.onglets[i];
        match &t.etat {
            None => {
                let vierge = self.onglet_vierge();
                InfosOnglet {
                    id: t.id,
                    nom: nom_de(self.path.as_deref(), self.import_source.as_deref(), vierge),
                    chemin: self.path.clone(),
                    modifie: self.modified,
                    ecriture: self.saving.is_some(),
                    actif: true,
                    vierge,
                }
            }
            Some(e) => {
                let vierge = Self::est_vierge(e.welcome, &e.path, &e.import_source, e.modified, e.history.is_empty(), e.saving.is_some(), e.open_job.is_some());
                InfosOnglet {
                    id: t.id,
                    nom: nom_de(e.path.as_deref(), e.import_source.as_deref(), vierge),
                    chemin: e.path.clone(),
                    modifie: e.modified,
                    ecriture: e.saving.is_some(),
                    actif: false,
                    vierge,
                }
            }
        }
    }

    /// Les onglets, dans l'ordre de la bande.
    pub(crate) fn infos_onglets(&self) -> Vec<InfosOnglet> {
        let mut infos: Vec<InfosOnglet> = (0..self.onglets.len()).map(|i| self.infos_onglet(i)).collect();
        // Plusieurs projets pas encore enregistrés ne portent pas le même nom : « Sans titre », « Sans titre 2 »…
        let mut sans_titre = 0;
        for info in infos.iter_mut().filter(|i| i.chemin.is_none() && !i.vierge && i.nom == "Sans titre") {
            sans_titre += 1;
            if sans_titre > 1 {
                info.nom = format!("Sans titre {sans_titre}");
            }
        }
        infos
    }

    /// Le rang de l'onglet de numéro `id`.
    pub(crate) fn rang_onglet(&self, id: u64) -> Option<usize> {
        self.onglets.iter().position(|t| t.id == id)
    }

    /// L'onglet qui a ce projet ouvert, s'il y en a un.
    pub(crate) fn onglet_du_projet(&self, path: &Path) -> Option<usize> {
        (0..self.onglets.len()).find(|&i| {
            let p = match &self.onglets[i].etat {
                None => self.path.as_deref(),
                Some(e) => e.path.as_deref(),
            };
            p.is_some_and(|p| meme_chemin(p, path))
        })
    }

    /// Les onglets modifiés (numéro, nom).
    pub(crate) fn onglets_modifies(&self) -> Vec<(u64, String)> {
        self.infos_onglets().into_iter().filter(|i| i.modifie).map(|i| (i.id, i.nom)).collect()
    }

    /// Pourquoi on ne peut pas changer d'onglet maintenant, ou rien. Les travaux qui prennent le projet
    /// entier (export, propagation, coloriage) sont modaux ; ceux qui parlent à l'interface (suggestions
    /// sonores, bruitages en cours de pose) ne peuvent pas suivre un projet qu'on quitte.
    pub(crate) fn onglet_occupe(&self) -> Option<&'static str> {
        if self.export_job.is_some() {
            return Some("l'export");
        }
        if self.propagation.running() {
            return Some("la propagation");
        }
        if self.coloriage.running() {
            return Some("le coloriage par zones");
        }
        if self.sb_export.as_ref().is_some_and(crate::sbexportui::SbExportWindow::rendering) || self.editorial.as_ref().is_some_and(crate::editorialui::EditorialWindow::rendering) {
            return Some("l'export pour le montage");
        }
        if self.fiche.as_ref().is_some_and(crate::ficheui::FicheWindow::busy) {
            return Some("l'export de la fiche technique");
        }
        if self.open_job.is_some() {
            return Some("l'ouverture du projet");
        }
        if self.collage_occupe() {
            return Some("le collage d'une copie d'un autre projet");
        }
        if self.relecture.occupe() {
            return Some("la relecture (page ou notes)");
        }
        if self.sug.occupe() {
            return Some("la génération des sons proposés");
        }
        if !self.brt_pending_is_empty() {
            return Some("la pose d'un bruitage");
        }
        None
    }

    /// On peut changer d'onglet (aucun travail n'attend le projet affiché) ; sinon la barre d'état dit pourquoi.
    /// (Les boîtes de dialogue qui tiennent la main sont l'affaire de l'interface : `ongletsui.rs`.)
    pub(crate) fn onglet_libre(&mut self) -> bool {
        if let Some(quoi) = self.onglet_occupe() {
            self.status = format!("Attends la fin de {quoi} avant de changer d'onglet.");
            return false;
        }
        true
    }

    // ------------------------------------------------------------------ changer d'onglet

    /// L'onglet affiché s'en va : les gestes se terminent, la lecture s'arrête, ses caches sont rangés et ses
    /// demandes de fond abandonnées. Rend ce qu'il faut ranger avec lui.
    fn sortir_de_l_onglet(&mut self) -> (crate::fxcache::Reserve, Option<crate::gpu::TexturesDeProjet>) {
        self.finish_text_edit();
        self.finish_field_edit();
        self.end_stroke();
        self.playing = false;
        self.stop_audio();
        self.thumbs.abandonner();
        self.layer_thumbs.abandonner();
        // Les sons proposés (choix, portée) visaient les plans de ce projet ; rien n'est en cours (`onglet_occupe`).
        self.reset_suggestions();
        // Les fenêtres qui montrent un calcul fait sur ce projet (export pour le montage, vers Storyboard Pro,
        // scénario à importer) se ferment : elles se rouvrent sur le projet affiché.
        if !self.sb_export.as_ref().is_some_and(crate::sbexportui::SbExportWindow::rendering) {
            self.sb_export = None;
        }
        if !self.editorial.as_ref().is_some_and(crate::editorialui::EditorialWindow::rendering) {
            self.editorial = None;
        }
        // (La fiche technique ne s'écrit pas à cet instant : `onglet_occupe` l'a vérifié.)
        self.fiche = None;
        self.scenario_window = None;
        // (Les ressources de la scène 3D sont communes à la carte graphique : un modèle importé a un numéro propre
        // à son projet. Elles se refont à la demande.)
        self.scene3d.gpu = None;
        let reserve = crate::fx::mettre_de_cote();
        let textures = self.gpu.as_ref().map(|g| g.prendre_textures());
        (reserve, textures)
    }

    /// Le nouvel onglet est là : ses caches reviennent, le mode et les panneaux suivent.
    fn entrer_dans_l_onglet(&mut self, reserve: Option<crate::fxcache::Reserve>, textures: Option<crate::gpu::TexturesDeProjet>) {
        if let Some(r) = reserve {
            crate::fxcache::rendre(r);
        }
        if let (Some(t), Some(g)) = (textures, self.gpu.as_ref()) {
            g.poser_textures(t);
        }
        self.pending_keys.clear();
        self.follow_frame = true;
        self.clamp_nav();
        self.onglets[self.actif].vu_le = Instant::now();
        // Le projet était en animation (ou en storyboard) : l'espace de travail va avec son mode.
        let voulu = self.mode;
        let espace = match voulu {
            Mode::Animation => crate::workspace::Espace::Animation,
            Mode::Storyboard => self.settings.storyboard_space,
        };
        if espace.mode() != self.espace.mode() {
            self.aller_a_l_espace(espace);
            self.masquer_exposition();
        }
        self.mode = voulu;
        self.session_a_ecrire = true;
        self.borner_la_memoire_des_onglets();
    }

    /// Les caches des onglets cachés ne grossissent pas sans fin : au-delà de la moitié de la limite des effets et
    /// de celle des textures, tous onglets cachés ensemble, les plus anciennement vus sont lâchés (ils se recalculent au retour, sans rien perdre).
    fn borner_la_memoire_des_onglets(&mut self) {
        // (La moitié de la réserve : l'onglet affiché garde la sienne entière.)
        let limite_fx = crate::fxcache::limite_octets() / 2;
        let limite_gpu = self.gpu.as_ref().map_or(0, |g| g.limite_octets()) / 2;
        let mut ordre: Vec<usize> = (0..self.onglets.len()).filter(|&i| i != self.actif).collect();
        ordre.sort_by_key(|&i| std::cmp::Reverse(self.onglets[i].vu_le));
        let (mut fx, mut gpu) = (0usize, 0u64);
        for i in ordre {
            let Some(e) = self.onglets[i].etat.as_mut() else { continue };
            if let Some(r) = e.reserve_fx.as_ref() {
                fx += r.bytes();
                if fx > limite_fx {
                    e.reserve_fx = None;
                }
            }
            if let Some(t) = e.textures_gpu.as_ref() {
                gpu += t.bytes();
                if gpu > limite_gpu {
                    e.textures_gpu = None;
                }
            }
        }
    }

    /// Affiche l'onglet `vers`. Instantané : le projet est en mémoire, ses caches aussi. Faux si on ne peut pas
    /// changer d'onglet maintenant.
    pub(crate) fn activer_onglet(&mut self, vers: usize) -> bool {
        if vers == self.actif {
            return true;
        }
        if vers >= self.onglets.len() || !self.onglet_libre() {
            return false;
        }
        self.bascule(vers, false);
        // (Le message d'état d'avant parlait d'un autre projet.)
        self.status = format!("Onglet « {} ».", nom_de(self.path.as_deref(), self.import_source.as_deref(), self.onglet_vierge()));
        true
    }

    /// Le changement lui-même. `fermeture` : l'ancien onglet est jeté après (il ne range pas ses caches).
    fn bascule(&mut self, vers: usize, fermeture: bool) {
        let (reserve, textures) = self.sortir_de_l_onglet();
        let mut e = self.onglets[vers].etat.take().expect("un onglet inactif a son état rangé");
        let (reserve_b, textures_b) = (e.reserve_fx.take(), e.textures_gpu.take());
        self.echanger_etat(&mut e);
        let ancien = self.actif;
        if !fermeture {
            e.reserve_fx = Some(reserve);
            e.textures_gpu = textures;
        }
        self.onglets[ancien].etat = Some(e);
        self.actif = vers;
        self.entrer_dans_l_onglet(reserve_b, textures_b);
    }

    /// Onglet suivant (`delta` > 0) ou précédent, en boucle.
    pub(crate) fn onglet_voisin(&mut self, delta: isize) {
        let n = self.onglets.len() as isize;
        if n < 2 {
            return;
        }
        let vers = (self.actif as isize + delta).rem_euclid(n) as usize;
        self.activer_onglet(vers);
    }

    // ------------------------------------------------------------------ ouvrir un onglet

    /// Un onglet neuf, vierge (l'accueil), qui devient l'onglet affiché. Si l'onglet affiché est déjà un onglet
    /// vierge, il n'en ouvre pas un second.
    pub(crate) fn nouvel_onglet(&mut self) -> bool {
        if self.onglet_vierge() {
            return true;
        }
        if !self.onglet_libre() {
            return false;
        }
        let (reserve, textures) = self.sortir_de_l_onglet();
        let mut e = Box::new(EtatProjet::vierge(self.gpu.is_some()));
        self.echanger_etat(&mut e);
        e.reserve_fx = Some(reserve);
        e.textures_gpu = textures;
        self.onglets[self.actif].etat = Some(e);
        let id = self.prochain_onglet;
        self.prochain_onglet += 1;
        self.onglets.push(Onglet { id, etat: None, vu_le: Instant::now() });
        self.actif = self.onglets.len() - 1;
        self.demarrer_secours();
        self.entrer_dans_l_onglet(None, None);
        self.status = "Nouvel onglet : Nouveau, Ouvrir ou un projet récent.".into();
        true
    }

    /// Juste avant qu'un projet prenne la place du projet affiché (Ouvrir, Nouveau, récents, imports) : dans un
    /// onglet neuf si le réglage « Ouvrir dans un nouvel onglet » est allumé et que l'onglet affiché sert déjà.
    /// Rend faux si l'onglet neuf n'a pas pu s'ouvrir (un travail tient le projet affiché) : rien ne doit alors
    /// remplacer ce projet.
    pub(crate) fn cible_projet(&mut self) -> bool {
        if self.onglet_vierge() || !self.settings.open_in_new_tab {
            return true;
        }
        self.nouvel_onglet()
    }

    /// Un fichier de projet (`.pelure`) lâché sur la fenêtre : il s'ouvre (dans un onglet, ou son onglet passe à
    /// l'écran s'il est déjà ouvert). Vrai si le fichier en était un.
    pub(crate) fn import_dropped_project(&mut self, path: &Path) -> bool {
        let est_un_projet = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pelure"));
        if est_un_projet {
            self.open_path(path.to_path_buf());
        }
        est_un_projet
    }

    /// Met `doc` à la place du projet de l'onglet qui doit le recevoir (`cible_projet`). Faux, sans rien changer,
    /// si l'onglet n'a pas pu s'ouvrir.
    pub(crate) fn charger_projet(&mut self, doc: Document) -> bool {
        if !self.cible_projet() {
            return false;
        }
        self.load_doc(doc);
        self.session_a_ecrire = true;
        true
    }

    /// Faut-il demander avant de remplacer le projet affiché (modifié) ? Non quand le projet qui arrive ira dans
    /// un onglet à lui (ou dans l'onglet vierge).
    pub(crate) fn remplace_le_projet(&self) -> bool {
        !self.settings.open_in_new_tab && !self.onglet_vierge()
    }

    /// Le projet `path` est déjà ouvert : son onglet est affiché, et la demande n'a plus lieu d'être.
    pub(crate) fn montrer_si_deja_ouvert(&mut self, path: &Path) -> bool {
        let Some(i) = self.onglet_du_projet(path) else { return false };
        if self.activer_onglet(i) {
            self.status = format!("« {} » est déjà ouvert : voici son onglet.", nom_de(Some(path), None, false));
        }
        true
    }

    /// Déplace l'onglet `de` à la place `vers`.
    pub(crate) fn deplacer_onglet(&mut self, de: usize, vers: usize) {
        let n = self.onglets.len();
        if de >= n || vers >= n || de == vers {
            return;
        }
        let t = self.onglets.remove(de);
        self.onglets.insert(vers, t);
        // L'onglet affiché suit son déplacement.
        self.actif = if self.actif == de {
            vers
        } else if de < self.actif && vers >= self.actif {
            self.actif - 1
        } else if de > self.actif && vers <= self.actif {
            self.actif + 1
        } else {
            self.actif
        };
        self.session_a_ecrire = true;
    }

    // ------------------------------------------------------------------ fermer

    /// Ferme l'onglet affiché sans rien demander (la boîte « Modifications non enregistrées » a déjà parlé). Le
    /// dernier onglet ne se ferme pas : un projet vide du même format le remplace, et l'accueil revient.
    pub(crate) fn fermer_l_onglet_affiche(&mut self) {
        if self.onglets.len() == 1 {
            let (w, h, fps) = (self.doc.frame[0], self.doc.frame[1], self.doc.fps);
            self.load_doc(Document::with_frame(w, h, fps).created_now());
            self.path = None;
            self.welcome = true;
            self.status = "Projet fermé : un projet vide du même format le remplace.".into();
            self.session_a_ecrire = true;
            return;
        }
        let i = self.actif;
        // Le voisin de droite devient l'onglet affiché (celui de gauche pour le dernier), comme dans un navigateur.
        let voisin = if i + 1 < self.onglets.len() { i + 1 } else { i - 1 };
        self.bascule(voisin, true);
        // (Après le changement, l'état de l'onglet fermé est rangé dans sa case.)
        let ferme = self.onglets.remove(i);
        if voisin > i {
            self.actif -= 1;
        }
        let nom = self.jeter_onglet(ferme);
        self.status = format!("Onglet « {nom} » fermé.");
        self.session_a_ecrire = true;
    }

    /// Le projet d'un onglet qui s'en va : sa copie de secours est effacée (il a été enregistré, ou on a choisi de
    /// ne pas le faire) et ce qu'il tenait est rendu.
    fn jeter_onglet(&mut self, t: Onglet) -> String {
        let Some(mut e) = t.etat else { return String::new() };
        let nom = nom_de(e.path.as_deref(), e.import_source.as_deref(), false);
        if let Some(r) = e.recovery.take() {
            // (Attendre une copie en cours ne doit pas figer l'interface.)
            let _ = std::thread::Builder::new().name("secours-fermeture".into()).spawn(move || r.close());
        }
        if let Some(s) = e.saving.take() {
            // Une écriture en cours finit toute seule : on ne la coupe jamais.
            let _ = std::thread::Builder::new().name("enregistrement-fermeture".into()).spawn(move || s.attendre());
        }
        // Un film partagé qu'on ferme : ses prises sont rendues (sur un fil, pour ne rien figer).
        if let Some(mut s) = e.partage.take() {
            let _ = std::thread::Builder::new().name("cooperation-fermeture".into()).spawn(move || s.fermer());
        }
        nom
    }

    /// Ferme l'onglet `i`. Un onglet modifié passe d'abord à l'écran (la boîte « Modifications non enregistrées »).
    pub(crate) fn fermer_onglet(&mut self, i: usize) {
        if i >= self.onglets.len() {
            return;
        }
        if i == self.actif {
            self.request(Pending::Close);
            return;
        }
        if self.infos_onglet(i).a_perdre() {
            if self.activer_onglet(i) {
                self.request(Pending::Close);
            }
            return;
        }
        // Un onglet qui n'a rien à perdre se ferme sans passer à l'écran.
        let ferme = self.onglets.remove(i);
        if i < self.actif {
            self.actif -= 1;
        }
        let nom = self.jeter_onglet(ferme);
        self.status = format!("Onglet « {nom} » fermé.");
        self.session_a_ecrire = true;
    }

    /// Ferme tous les onglets sauf `garde` (son numéro) ; si certains sont modifiés, une seule boîte les liste.
    pub(crate) fn fermer_les_autres(&mut self, garde: u64) {
        let modifies: Vec<(u64, String)> = self.onglets_modifies().into_iter().filter(|(id, _)| *id != garde).collect();
        if modifies.is_empty() {
            self.fermer_autres_sans_demander(garde);
        } else {
            self.groupe = Some(Groupe { action: ActionGroupe::FermerAutres(garde), onglets: modifies, attente: false });
        }
    }

    fn fermer_autres_sans_demander(&mut self, garde: u64) {
        let Some(r) = self.rang_onglet(garde) else { return };
        if r != self.actif {
            if !self.onglet_libre() {
                return;
            }
            self.bascule(r, false);
        }
        let (gardes, autres): (Vec<Onglet>, Vec<Onglet>) = std::mem::take(&mut self.onglets).into_iter().partition(|t| t.id == garde);
        self.onglets = gardes;
        self.actif = 0;
        for t in autres {
            self.jeter_onglet(t);
        }
        self.status = "Les autres onglets sont fermés.".into();
        self.session_a_ecrire = true;
    }

    // ------------------------------------------------------------------ copie de secours par onglet

    /// Un onglet neuf a sa copie de secours (un dossier de séance à lui, avec son verrou).
    pub(crate) fn demarrer_secours(&mut self) {
        if !self.secours_actif || self.recovery.is_some() {
            return;
        }
        self.recovery = crate::recovery::root().and_then(|r| crate::recovery::Recovery::start(&r).ok());
    }

    // ------------------------------------------------------------------ tâches des onglets qui ne sont pas affichés

    /// À chaque image d'écran : ce que les onglets cachés ont à faire. Presque toujours rien (aucun calcul, aucune
    /// repeinture) ; seule une écriture ou une copie de secours en cours les réveille : l'écriture finie est adoptée
    /// (l'onglet est échangé un instant pour que le code d'enregistrement fasse son travail, puis rangé de nouveau).
    pub(crate) fn tick_onglets(&mut self, ctx: &egui::Context) {
        let mut a_regarder = false;
        let mut revoir: Option<Duration> = None;
        let mut a_montrer: Option<usize> = None;
        let interval = self.autosave_interval();
        for i in 0..self.onglets.len() {
            if i == self.actif {
                continue;
            }
            let Some(e) = &self.onglets[i].etat else { continue };
            let fini = e.saving.as_ref().is_some_and(crate::enregistrement::Enregistrement::fini);
            if e.saving.is_some() && !fini {
                a_regarder = true;
            }
            if fini {
                let suite = e.saving.as_ref().is_some_and(crate::enregistrement::Enregistrement::a_une_suite) || e.after_save.is_some();
                if suite {
                    // Une action attend cette écriture (fermer l'onglet…) : elle se fait dans l'onglet affiché.
                    a_montrer = Some(i);
                } else if self.adopter_ecriture(i) {
                    a_montrer = Some(i);
                }
            }
            // La copie de secours d'un onglet caché suit le même rythme que celle de l'onglet affiché.
            if let Some(d) = self.tick_secours_cache(i, interval) {
                revoir = Some(revoir.map_or(d, |r: Duration| r.min(d)));
            }
        }
        if let Some(i) = a_montrer {
            // Une erreur à lire, ou une suite à faire : l'onglet passe à l'écran.
            self.activer_onglet(i);
        }
        if a_regarder {
            revoir = Some(revoir.map_or(Duration::from_millis(200), |r| r.min(Duration::from_millis(200))));
        }
        if let Some(d) = revoir {
            ctx.request_repaint_after(d);
        }
    }

    /// Échange l'onglet `i` avec l'affiché le temps de `f`, puis les remet comme ils étaient. Les caches communs
    /// (effets, textures) ne bougent pas : `f` ne dessine rien et ne calcule rien.
    fn avec_onglet_cache<R>(&mut self, i: usize, f: impl FnOnce(&mut App) -> R) -> R {
        let mut e = self.onglets[i].etat.take().expect("onglet rangé");
        let (reserve, textures) = (e.reserve_fx.take(), e.textures_gpu.take());
        self.echanger_etat(&mut e);
        let r = f(self);
        self.echanger_etat(&mut e);
        e.reserve_fx = reserve;
        e.textures_gpu = textures;
        self.onglets[i].etat = Some(e);
        r
    }

    /// Adopte l'écriture finie de l'onglet caché `i` (chemin, « modifié », récents). Rend vrai s'il faut le montrer :
    /// une erreur est à lire.
    fn adopter_ecriture(&mut self, i: usize) -> bool {
        let nom = self.infos_onglet(i).nom;
        let (erreur, status) = self.avec_onglet_cache(i, |app| {
            let avant = std::mem::take(&mut app.status);
            app.finish_job();
            let status = std::mem::replace(&mut app.status, avant);
            (app.save_error.is_some(), status)
        });
        // Le message de l'écriture est celui d'un autre onglet : on dit lequel.
        self.status = format!("{nom} — {status}");
        self.session_a_ecrire = true;
        erreur
    }

    /// La copie de secours de l'onglet caché `i` : termine celle qui s'écrit, ou en lance une si le délai est passé et
    /// que le projet a changé (comme `tick_recovery` pour l'onglet affiché : l'échange d'onglet, lui, ne prend jamais
    /// de cliché). Rend dans combien de temps revenir la regarder (copie en cours : bientôt ; projet modifié à copier au
    /// prochain délai : au plus dix secondes, comme l'onglet affiché).
    pub(crate) fn tick_secours_cache(&mut self, i: usize, interval: Option<Duration>) -> Option<Duration> {
        let Some(e) = self.onglets[i].etat.as_mut() else { return None };
        let Some(rec) = e.recovery.as_mut() else { return None };
        let snap = crate::recovery::Snapshot {
            doc: &e.doc,
            serial: e.history.serial(),
            modified: e.modified,
            title: nom_de(e.path.as_deref(), e.import_source.as_deref(), false),
            project: e.path.clone(),
            project_written: &e.written,
        };
        // (Pas pendant que le projet s'écrit dans son dossier : la copie recopierait des dessins qu'il n'a pas encore.)
        rec.tick(snap, interval, e.saving.is_some());
        let erreur = rec.error.take();
        let revoir = if rec.busy() {
            Some(Duration::from_millis(200))
        } else {
            interval.filter(|_| e.modified).map(|i| i.min(Duration::from_secs(10)))
        };
        if let Some(erreur) = erreur {
            self.status = format!("Copie de secours impossible : {erreur}");
        }
        revoir
    }

    // ------------------------------------------------------------------ quitter, fermer plusieurs onglets

    /// Un autre onglet que celui qui est affiché est modifié ou s'écrit encore.
    pub(crate) fn d_autres_onglets_a_perdre(&self) -> bool {
        (0..self.onglets.len()).any(|i| i != self.actif && self.infos_onglet(i).a_perdre())
    }

    /// Une demande de fermeture de la fenêtre ou « Quitter » : tout de suite si rien n'est à perdre, sinon une
    /// seule boîte qui liste les projets modifiés ; des écritures en cours sont attendues.
    pub(crate) fn demander_a_quitter(&mut self) {
        self.end_stroke();
        self.playing = false;
        let modifies = self.onglets_modifies();
        let ecritures = self.infos_onglets().iter().any(|i| i.ecriture);
        if modifies.is_empty() && !ecritures {
            self.perform(Pending::Quit);
        } else {
            self.groupe = Some(Groupe { action: ActionGroupe::Quitter, onglets: modifies.clone(), attente: modifies.is_empty() });
        }
    }

    /// « Enregistrer tout » : chaque projet modifié est enregistré (un projet sans nom demande où, l'un après
    /// l'autre). Rend faux si Anthony renonce. L'écriture se poursuit sur les fils.
    fn enregistrer_les_onglets(&mut self, sauf: Option<u64>) -> bool {
        let garde = self.onglets[self.actif].id;
        let a_faire: Vec<usize> = (0..self.onglets.len()).filter(|&i| self.infos_onglet(i).modifie && Some(self.onglets[i].id) != sauf).collect();
        let mut ok = true;
        for i in a_faire {
            if i != self.actif {
                self.bascule(i, false);
            }
            if !self.save() {
                ok = false;
                break;
            }
        }
        // On revient où l'on était.
        if let Some(r) = self.rang_onglet(garde) {
            if r != self.actif {
                self.bascule(r, false);
            }
        }
        ok
    }

    /// La boîte « Projets non enregistrés » (quitter, fermer les autres).
    pub(crate) fn groupe_fenetre(&mut self, ctx: &egui::Context) {
        let Some(g) = self.groupe.take() else { return };
        // Les écritures lancées par « Enregistrer tout » : on attend la dernière, puis l'action.
        if g.attente {
            self.groupe = Some(g);
            self.suivre_les_ecritures(ctx);
            return;
        }
        let mut choix = None;
        egui::Window::new("Projets non enregistrés")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_max_width(440.0);
                ui.label(match g.action {
                    ActionGroupe::Quitter => "Ces projets ont été modifiés depuis leur dernier enregistrement :",
                    ActionGroupe::FermerAutres(_) => "Ces autres projets ont été modifiés depuis leur dernier enregistrement :",
                });
                ui.add_space(4.0);
                for (_, nom) in &g.onglets {
                    ui.label(format!("•  {nom}"));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Enregistrer tout").clicked() {
                        choix = Some(0);
                    }
                    if ui.button("Ne pas enregistrer").clicked() {
                        choix = Some(1);
                    }
                    if ui.button("Annuler").clicked() {
                        choix = Some(2);
                    }
                });
            });
        self.groupe = Some(g);
        if let Some(c) = choix {
            self.groupe_choisir(c);
        }
    }

    /// Le choix de la boîte « Projets non enregistrés » : 0 enregistrer tout, 1 ne pas enregistrer, 2 annuler.
    pub(crate) fn groupe_choisir(&mut self, choix: u8) {
        let Some(g) = self.groupe.take() else { return };
        match choix {
            0 => {
                let sauf = match g.action {
                    ActionGroupe::FermerAutres(garde) => Some(garde),
                    ActionGroupe::Quitter => None,
                };
                if self.enregistrer_les_onglets(sauf) {
                    self.groupe = Some(Groupe { attente: true, ..g });
                }
            }
            1 => self.faire_l_action_du_groupe(g.action),
            _ => {}
        }
    }

    /// Les écritures de « Enregistrer tout » sont-elles finies ? Alors l'action suit ; une erreur l'annule.
    fn suivre_les_ecritures(&mut self, ctx: &egui::Context) {
        let Some(g) = self.groupe.take() else { return };
        let epargne = |id: u64| matches!(g.action, ActionGroupe::FermerAutres(garde) if garde == id);
        let encore = self.infos_onglets().iter().any(|i| i.ecriture && !epargne(i.id));
        let erreur = self.save_error.is_some() || self.onglets.iter().any(|t| t.etat.as_ref().is_some_and(|e| e.save_error.is_some()));
        if erreur {
            // L'erreur est dite par sa propre boîte ; rien n'est fermé.
            return;
        }
        if encore {
            let noms: Vec<String> = self.infos_onglets().into_iter().filter(|i| i.ecriture && !epargne(i.id)).map(|i| i.nom).collect();
            egui::Window::new("Enregistrement")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!("Enregistrement de {}…", noms.join(", ")));
                    ui.add(egui::ProgressBar::new(0.0).desired_width(280.0).animate(true));
                });
            ctx.request_repaint_after(Duration::from_millis(100));
            self.groupe = Some(g);
            return;
        }
        // Un projet modifié pendant l'écriture redemande : la boîte revient.
        let restes: Vec<(u64, String)> = self.onglets_modifies().into_iter().filter(|(id, _)| !epargne(*id)).collect();
        if restes.is_empty() {
            self.faire_l_action_du_groupe(g.action);
        } else {
            self.groupe = Some(Groupe { action: g.action, onglets: restes, attente: false });
        }
    }

    fn faire_l_action_du_groupe(&mut self, action: ActionGroupe) {
        match action {
            ActionGroupe::Quitter => self.perform(Pending::Quit),
            ActionGroupe::FermerAutres(garde) => self.fermer_autres_sans_demander(garde),
        }
    }

    /// L'application se ferme : toutes les écritures se terminent, toutes les copies de secours sont effacées.
    pub(crate) fn fermer_tous_les_onglets(&mut self) {
        for i in 0..self.onglets.len() {
            if i == self.actif {
                self.finish_save_on_exit();
                if let Some(r) = self.recovery.take() {
                    r.close();
                }
                // Un film partagé : ce qu'on tient est rendu avant de quitter.
                if let Some(mut s) = self.partage.take() {
                    s.fermer();
                }
            } else if let Some(e) = self.onglets[i].etat.as_mut() {
                if let Some(s) = e.saving.take() {
                    s.attendre();
                }
                if let Some(r) = e.recovery.take() {
                    r.close();
                }
                if let Some(mut s) = e.partage.take() {
                    s.fermer();
                }
            }
        }
    }

    // ------------------------------------------------------------------ session

    /// Les projets ouverts, dans l'ordre, et l'onglet affiché : retenus dans les réglages pour le prochain lancement.
    pub(crate) fn retenir_la_session(&mut self) {
        // (Pas avant que les onglets de la séance d'avant soient revenus : la liste serait effacée.)
        if !self.session_prete {
            return;
        }
        let chemins: Vec<Option<PathBuf>> = (0..self.onglets.len()).map(|i| self.infos_onglet(i).chemin).collect();
        let affiche = chemins.get(self.actif).cloned().flatten();
        let tabs: Vec<PathBuf> = chemins.into_iter().flatten().collect();
        let actif = affiche.and_then(|p| tabs.iter().position(|t| meme_chemin(t, &p))).unwrap_or(0);
        if tabs != self.settings.session_tabs || actif != self.settings.session_active {
            self.settings.session_tabs = tabs;
            self.settings.session_active = actif;
            let _ = self.settings.save();
        }
    }

    /// À chaque image d'écran : la session est retenue quand elle a changé (jamais avant que celle de la séance
    /// d'avant soit rouverte : elle serait effacée).
    pub(crate) fn tick_session(&mut self) {
        if std::mem::take(&mut self.session_a_ecrire) && self.session_prete {
            self.retenir_la_session();
        }
    }

    /// Au lancement : les onglets de la séance d'avant reviennent, un projet à la fois (chacun s'ouvre pendant
    /// que la fenêtre est modale), l'onglet qui était affiché à la fin.
    pub(crate) fn rouvrir_ces_projets(&mut self, projets: Vec<PathBuf>, affiche: Option<PathBuf>) {
        self.session_file = projets.into_iter().collect::<VecDeque<_>>();
        self.session_affichee = affiche;
        self.session_prete = false;
    }

    /// À chaque image d'écran : le projet suivant de la session, quand le précédent est ouvert.
    pub(crate) fn tick_session_ouverture(&mut self) {
        if self.session_file.is_empty() {
            if self.session_prete {
                return;
            }
            // Tous les projets de la séance d'avant sont ouverts (ou ont disparu) : l'onglet affiché revient.
            if self.open_job.is_some() {
                return;
            }
            if let Some(p) = self.session_affichee.take() {
                if let Some(i) = self.onglet_du_projet(&p) {
                    self.activer_onglet(i);
                }
            }
            self.session_prete = true;
            self.session_a_ecrire = true;
            return;
        }
        if self.open_job.is_some() || self.modal_open() {
            return;
        }
        if let Some(path) = self.session_file.pop_front() {
            if path.exists() && self.onglet_du_projet(&path).is_none() {
                self.open_path(path);
            }
        }
    }
}
