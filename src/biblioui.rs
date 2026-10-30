//! Le panneau Bibliothèque à l'écran (`biblio.rs` pour les dossiers, `bibliovignettes.rs` pour les vignettes,
//! `biblioapercu.rs` pour l'aperçu) : un explorateur de fichiers en lecture seule, avec un aperçu intégré.
//!
//! - **Explorer** : en haut, retour, dossier parent, actualiser, favori, et le fil d'Ariane (un clic sur un dossier de
//!   la liste y mène) ; les favoris (le projet, Documents, Bureau, les siens) et les lecteurs de Windows ; le filtre par
//!   type, la recherche dans le dossier, la grille de vignettes (taille réglable) ou la liste (nom, type, durée, taille,
//!   date, tri au clic sur l'en-tête). Double-clic sur un dossier : y entrer ; Retour arrière : remonter.
//! - **Aperçu** : sous la liste (ou à sa droite quand le panneau est large), redimensionnable ; il n'existe que quand un
//!   fichier lisible est choisi. Un seul à la fois ; fermer le panneau (ou le cacher derrière un autre onglet) l'arrête.
//! - **Utiliser un fichier** : on le glisse sur le dessin, les vignettes, le montage ou les références (le même résultat
//!   que depuis l'Explorateur de Windows), ou clic droit : importer dans le plan, ajouter aux références, ajouter au
//!   montage, ouvrir le dossier dans l'Explorateur, copier le chemin.
//! - **Jamais d'écriture** : le panneau ne modifie, ne déplace et ne supprime aucun fichier ; il ne touche au
//!   presse-papiers que sur le clic explicite « Copier le chemin ».
//! - **Sans latence** : la liste d'un dossier, les vignettes, les images, les sons et les vidéos se lisent sur des fils
//!   à part ; l'interface ne fait que dessiner ce qui est prêt. Les touches du panneau (Espace, flèches, Retour arrière,
//!   Entrée) lui appartiennent quand la souris le survole : elles ne vont pas au dessin.

use std::path::{Path, PathBuf};

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Key, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle, Vec2};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::biblio::{self, date_texte, duree_texte, taille_texte, Entree, FavoriFixe, Filtre, Genre, Navigateur, Tri, Vue};
use crate::biblioapercu::{Apercu, ApercuImage, ApercuSon, ApercuVideo, Moteur, Muet, SortieAudio};
use crate::bibliovignettes::{Demande, Ffmpeg, Meta, Vignettes};
use crate::dock::Pane;
use crate::theme;

/// Hauteur d'une ligne de la liste, en points.
const LIGNE_H: f32 = 22.0;
/// Hauteur du nom sous une vignette.
const NOM_H: f32 = 18.0;
/// Écart entre deux vignettes.
const ECART: f32 = 6.0;
/// Largeur du panneau à partir de laquelle l'aperçu passe à droite de la liste.
const LARGEUR_DEUX_COLONNES: f32 = 760.0;

/// Un fichier glissé depuis la bibliothèque (la charge du glisser-déposer d'egui).
#[derive(Clone, Debug)]
pub(crate) struct BiblioDrag {
    pub chemin: PathBuf,
    pub genre: Genre,
}

impl BiblioDrag {
    pub fn est_un_son(&self) -> bool {
        self.genre == Genre::Son
    }
}

/// Une étape d'un geste sur la barre de temps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Etape {
    Debut,
    Pendant,
    Fin,
    /// Un simple clic sur la barre.
    Clic,
}

/// Ce que l'écran demande ; exécuté après le dessin, là où `App` est libre (`App::biblio_commande`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Commande {
    /// Un clic : choisir (et ouvrir l'aperçu d'un fichier lisible).
    Choisir(PathBuf),
    /// Entrer dans un dossier.
    Entrer(PathBuf),
    Remonter,
    Retour,
    Actualiser,
    /// Ajouter (ou retirer) le dossier courant des favoris.
    Favori,
    RetirerFavori(PathBuf),
    Trier(Tri),
    /// Un fichier glissé : le dépôt se fait au lâcher.
    Glisser(PathBuf, Genre),
    Importer(PathBuf),
    AjouterReferences(PathBuf),
    AjouterMontage(PathBuf),
    Explorateur(PathBuf),
    CopierChemin(PathBuf),
    // --- l'aperçu
    JouerPause,
    Chercher { t: f64, etape: Etape },
    Pas(i64),
    Volume(f32),
    Boucle(bool),
    FermerApercu,
    /// Passer au fichier lisible suivant (ou précédent) de la liste.
    Voisin(i32),
}

/// L'état du panneau (un seul champ dans `App`).
#[derive(Default)]
pub(crate) struct BiblioUi {
    pub nav: Navigateur,
    /// Créées à la première ouverture du panneau (jamais au lancement).
    pub vignettes: Option<Vignettes>,
    pub apercu: Option<Apercu>,
    /// La place du panneau docké à la dernière image (pour y reconnaître le survol et un dépôt).
    pub rect: Option<Rect>,
    /// Le dernier clic était dans le panneau.
    pub focus: bool,
    /// Le numéro de la dernière image d'écran où le panneau s'est dessiné.
    pub vu: u64,
    pub lecteurs: Option<Vec<PathBuf>>,
    /// Où s'ouvrir si le dossier voulu ne se lit pas.
    candidats: std::collections::VecDeque<PathBuf>,
    ffmpeg: Option<Ffmpeg>,
    /// On tient la barre de temps d'une vidéo qui jouait : elle reprend au lâcher.
    pub reprise: bool,
    /// Les favoris d'office, pour le projet de cette image (recalculés quand le projet change).
    fixes: Option<(Option<PathBuf>, Vec<FavoriFixe>)>,
    /// Le poids du fichier de l'aperçu, lu dans la liste une fois à l'ouverture (jamais cherché à chaque image).
    pub apercu_octets: Option<u64>,
}

impl BiblioUi {
    /// La souris survole-t-elle le panneau (docké, dessiné à l'instant) ?
    pub fn survol(&self, ctx: &egui::Context) -> bool {
        self.rect.is_some_and(|r| ctx.pointer_hover_pos().is_some_and(|p| r.contains(p))) && self.vu + 6 >= ctx.cumulative_pass_nr()
    }

    /// Cette touche (sans modificateur) est-elle celle du panneau ? Les raccourcis du reste de SHOT ne la voient pas
    /// (Retour arrière supprime, les flèches changent d'image : pas ici).
    pub fn touche_prise(&self, ctx: &egui::Context, key: Key, mods: egui::Modifiers) -> bool {
        !mods.any() && matches!(key, Key::Space | Key::Backspace | Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Enter) && !ctx.wants_keyboard_input() && self.survol(ctx)
    }
}

/// « 1920 × 1080 ».
fn dims_texte(d: (u32, u32)) -> String {
    format!("{} × {}", d.0, d.1)
}

impl App {
    // ------------------------------------------------------------------ à chaque image d'écran

    fn biblio_ffmpeg(&mut self) -> Ffmpeg {
        let reglage = self.settings.ffmpeg.clone();
        self.biblio.ffmpeg.get_or_insert_with(|| Ffmpeg::new(reglage)).clone()
    }

    /// Ce que la bibliothèque fait à chaque image d'écran, panneau ouvert ou non : reprendre ce que les fils ont fait, faire
    /// avancer l'aperçu, le fermer quand le panneau l'est (ou caché derrière un autre onglet), et recevoir un fichier lâché.
    pub(crate) fn poll_bibliotheque(&mut self, ctx: &egui::Context) {
        // Le panneau est fermé ou caché : plus d'aperçu, plus de lecture.
        // (Une image d'écran compte une ou deux passes : on ne ferme qu'après plusieurs images sans dessin.)
        let dessine = self.biblio.vu + 5 >= ctx.cumulative_pass_nr();
        if self.biblio.apercu.is_some() && !(self.pane_shown(Pane::Bibliotheque) && dessine) {
            self.biblio_fermer_apercu();
        }
        if !self.pane_shown(Pane::Bibliotheque) {
            self.biblio.rect = None;
            self.biblio.focus = false;
        }
        let mut actif = false;
        if self.biblio.nav.poll() {
            actif = true;
            // Le dossier voulu ne se lit pas : le suivant des dossiers de départ.
            if self.biblio.nav.erreur.is_some() {
                if let Some(suivant) = self.biblio.candidats.pop_front() {
                    self.biblio.nav.aller(suivant);
                }
            } else {
                self.biblio.candidats.clear();
            }
        }
        if self.biblio.nav.lit() {
            actif = true;
        }
        if let Some(v) = self.biblio.vignettes.as_mut() {
            if v.relever(ctx) || v.occupe() {
                actif = true;
            }
        }
        // L'aperçu avance ; le projet qui joue prend la sortie audio.
        if self.biblio.apercu.is_some() {
            let projet_joue = self.playing;
            let ctx2 = ctx.clone();
            self.biblio_avec_sortie(|b, sortie| {
                if let Some(a) = b.apercu.as_mut() {
                    if projet_joue && a.joue() {
                        a.rendre_la_sortie(sortie);
                    }
                    match a {
                        Apercu::Image(i) => {
                            i.suivre(&ctx2);
                        }
                        Apercu::Son(s) => {
                            s.suivre();
                            let fini = s.lecture.suivre(sortie, s.duree);
                            let _ = fini;
                        }
                        Apercu::Video(v) => {
                            v.suivre(&ctx2, sortie);
                        }
                    }
                }
            });
            if self.biblio.apercu.as_ref().is_some_and(Apercu::actif) {
                actif = true;
            }
        }
        // Un fichier glissé depuis la bibliothèque, lâché ailleurs : même résultat que depuis l'Explorateur de Windows.
        if let Some(d) = egui::DragAndDrop::payload::<BiblioDrag>(ctx) {
            if ctx.input(|i| i.pointer.any_released()) {
                // (Les cibles du montage ont pris le leur pendant le dessin : s'il reste, personne ne l'a pris.)
                let pos = ctx.input(|i| i.pointer.interact_pos().or(i.pointer.latest_pos()));
                let _ = egui::DragAndDrop::take_payload::<BiblioDrag>(ctx);
                let sur_la_bibliotheque = pos.zip(self.biblio.rect).is_some_and(|(p, r)| r.contains(p));
                if let (Some(pos), false) = (pos, sur_la_bibliotheque) {
                    if !self.modal_open() {
                        self.biblio_lacher(ctx, d.chemin.clone(), d.genre, pos);
                    }
                }
            }
            actif = true;
        }
        if actif {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    /// Exécute `f` avec la sortie audio de SHOT (ou sans rien qui sonne, quand il n'y en a pas).
    fn biblio_avec_sortie<R>(&mut self, f: impl FnOnce(&mut BiblioUi, &mut dyn SortieAudio) -> R) -> R {
        match self.audio.as_ref().filter(|e| e.ok()) {
            Some(e) => f(&mut self.biblio, &mut Moteur(e)),
            None => f(&mut self.biblio, &mut Muet),
        }
    }

    /// Ferme l'aperçu : la lecture s'arrête, la sortie audio est rendue, les fils et ffmpeg s'arrêtent.
    pub(crate) fn biblio_fermer_apercu(&mut self) {
        if let Some(a) = self.biblio.apercu.take() {
            let projet_joue = self.playing;
            self.biblio_avec_sortie(|_, sortie| {
                // (Un projet qui joue a pris la sortie audio : on n'y touche pas.)
                if projet_joue {
                    sortie_sans_toucher(a);
                } else {
                    a.fermer(sortie);
                }
            });
        }
        self.biblio.reprise = false;
    }

    // ------------------------------------------------------------------ opérations

    /// Ouvre le dossier `dossier` (sa liste se lit sur un fil à part).
    pub(crate) fn biblio_aller(&mut self, dossier: PathBuf) {
        self.biblio_fermer_apercu();
        if let Some(v) = self.biblio.vignettes.as_mut() {
            v.abandonner_la_file();
        }
        self.biblio.candidats.clear();
        self.settings.bibliotheque.dernier = Some(dossier.clone());
        self.settings_a_ecrire = true;
        self.biblio.nav.aller(dossier);
    }

    /// Le dossier du projet (là où est enregistré le projet), s'il l'est.
    fn biblio_dossier_projet(&self) -> Option<PathBuf> {
        self.path.as_ref().and_then(|p| p.parent()).map(Path::to_path_buf).filter(|p| !p.as_os_str().is_empty())
    }

    /// La première ouverture du panneau : le dossier de départ (jamais au lancement de SHOT).
    fn biblio_demarrer(&mut self) {
        let projet = self.biblio_dossier_projet();
        let mut candidats: std::collections::VecDeque<PathBuf> = biblio::candidats_de_depart(&self.settings.bibliotheque, projet.as_deref()).into();
        let Some(premier) = candidats.pop_front() else { return };
        self.biblio.candidats = candidats;
        self.biblio.lecteurs = Some(biblio::lecteurs());
        self.biblio.nav.aller(premier);
    }

    /// Choisit un élément de la liste : un fichier lisible ouvre son aperçu, le reste ferme celui qui était là.
    pub(crate) fn biblio_choisir(&mut self, chemin: PathBuf) {
        let genre = self.biblio.nav.entrees.iter().find(|e| e.chemin == chemin).map(|e| (e.genre, e.dossier));
        self.biblio.nav.selection = Some(chemin.clone());
        match genre {
            Some((Some(g), false)) => self.biblio_ouvrir_apercu(chemin, g),
            _ => self.biblio_fermer_apercu(),
        }
    }

    /// Ouvre l'aperçu de `chemin` (celui d'avant se ferme : un seul à la fois).
    pub(crate) fn biblio_ouvrir_apercu(&mut self, chemin: PathBuf, genre: Genre) {
        if self.biblio.apercu.as_ref().is_some_and(|a| a.chemin() == chemin) {
            return;
        }
        self.biblio_fermer_apercu();
        self.biblio.apercu_octets = self.biblio.nav.entrees.iter().find(|e| e.chemin == chemin).map(|e| e.octets);
        let (volume, boucle) = (self.settings.bibliotheque.volume, self.settings.bibliotheque.boucle);
        let apercu = match genre {
            Genre::Image => Apercu::Image(ApercuImage::ouvrir(chemin, self.ctx.input(|i| i.max_texture_side).clamp(1024, 4096) as u32)),
            Genre::Son => Apercu::Son(ApercuSon::ouvrir(chemin, volume, boucle)),
            Genre::Video => {
                let f = self.biblio_ffmpeg();
                Apercu::Video(Box::new(ApercuVideo::ouvrir(chemin, f, volume, boucle)))
            }
        };
        self.biblio.apercu = Some(apercu);
    }

    /// Lecture / pause de l'aperçu (Espace). Le projet qui joue s'arrête : la sortie audio est à l'aperçu.
    pub(crate) fn biblio_jouer_pause(&mut self) {
        let Some(a) = self.biblio.apercu.as_ref() else { return };
        if matches!(a, Apercu::Image(_)) {
            return;
        }
        if !a.joue() {
            if self.playing {
                self.playing = false;
                self.stop_audio();
            }
            // (La sortie audio ne s'ouvre qu'ici, au premier « jouer ».)
            self.audio_engine();
        }
        self.biblio_avec_sortie(|b, sortie| match b.apercu.as_mut() {
            Some(Apercu::Video(v)) => v.jouer_ou_pause(sortie),
            Some(Apercu::Son(s)) => s.jouer_ou_pause(sortie),
            _ => {}
        });
    }

    /// Exécute une commande de l'écran.
    pub(crate) fn biblio_commande(&mut self, c: Commande) {
        match c {
            Commande::Choisir(p) => self.biblio_choisir(p),
            Commande::Entrer(d) => self.biblio_aller(d),
            Commande::Remonter => {
                if let Some(p) = self.biblio.nav.parent() {
                    self.biblio_aller(p);
                }
            }
            Commande::Retour => {
                self.biblio_fermer_apercu();
                if self.biblio.nav.retour() {
                    self.settings.bibliotheque.dernier = Some(self.biblio.nav.dossier.clone());
                    self.settings_a_ecrire = true;
                }
            }
            Commande::Actualiser => {
                self.biblio_fermer_apercu();
                if let Some(v) = self.biblio.vignettes.as_mut() {
                    v.tout_oublier();
                }
                self.biblio.lecteurs = Some(biblio::lecteurs());
                self.biblio.nav.actualiser();
            }
            Commande::Favori => {
                let d = self.biblio.nav.dossier.clone();
                let est = self.settings.bibliotheque.basculer_favori(&d);
                self.settings_a_ecrire = true;
                self.status = if est { format!("Favori ajouté : {}", d.display()) } else { format!("Favori retiré : {}", d.display()) };
            }
            Commande::RetirerFavori(d) => {
                if self.settings.bibliotheque.est_favori(&d) {
                    self.settings.bibliotheque.basculer_favori(&d);
                    self.settings_a_ecrire = true;
                }
            }
            Commande::Trier(t) => {
                let r = &mut self.settings.bibliotheque;
                if r.tri == t {
                    r.tri_inverse = !r.tri_inverse;
                } else {
                    r.tri = t;
                    r.tri_inverse = false;
                }
                self.settings_a_ecrire = true;
            }
            Commande::Glisser(p, g) => egui::DragAndDrop::set_payload(&self.ctx, BiblioDrag { chemin: p, genre: g }),
            Commande::Importer(p) => self.biblio_importer(&p),
            Commande::AjouterReferences(p) => self.biblio_ajouter_references(&p),
            Commande::AjouterMontage(p) => self.biblio_ajouter_montage(&p),
            Commande::Explorateur(p) => crate::exportui::reveal_in_explorer(&p),
            Commande::CopierChemin(p) => {
                // (Le seul endroit où la bibliothèque touche au presse-papiers : l'utilisateur vient de le demander.)
                self.ctx.copy_text(p.display().to_string());
                self.status = "Chemin copié.".into();
            }
            Commande::JouerPause => self.biblio_jouer_pause(),
            Commande::Chercher { t, etape } => self.biblio_chercher(t, etape),
            Commande::Pas(n) => self.biblio_avec_sortie(|b, sortie| {
                if let Some(Apercu::Video(v)) = b.apercu.as_mut() {
                    v.pas(n, sortie);
                }
            }),
            Commande::Volume(v) => {
                self.settings.bibliotheque.volume = v;
                self.settings_a_ecrire = true;
                let boucle = self.settings.bibliotheque.boucle;
                self.biblio_regler(v, boucle);
            }
            Commande::Boucle(b) => {
                self.settings.bibliotheque.boucle = b;
                self.settings_a_ecrire = true;
                let v = self.settings.bibliotheque.volume;
                self.biblio_regler(v, b);
            }
            Commande::FermerApercu => {
                self.biblio_fermer_apercu();
            }
            Commande::Voisin(sens) => {
                if let Some(p) = self.biblio.nav.voisin(sens) {
                    self.biblio_choisir(p);
                }
            }
        }
    }

    fn biblio_regler(&mut self, volume: f32, boucle: bool) {
        self.biblio_avec_sortie(|b, sortie| match b.apercu.as_mut() {
            Some(Apercu::Son(s)) => s.lecture.regler(volume, boucle, sortie),
            Some(Apercu::Video(v)) => v.regler(volume, boucle, sortie),
            _ => {}
        });
    }

    /// Un geste sur la barre de temps : la vidéo qui jouait s'arrête pendant qu'on tire, et reprend au lâcher.
    fn biblio_chercher(&mut self, t: f64, etape: Etape) {
        let reprise = self.biblio.reprise;
        let mut nouvelle_reprise = reprise;
        self.biblio_avec_sortie(|b, sortie| match b.apercu.as_mut() {
            Some(Apercu::Son(s)) if s.pret() => s.lecture.aller(t.clamp(0.0, s.duree), sortie),
            Some(Apercu::Video(v)) => {
                match etape {
                    Etape::Debut => {
                        if v.lecture.joue {
                            v.pause(sortie);
                            nouvelle_reprise = true;
                        }
                        v.voir(t);
                    }
                    Etape::Pendant => v.voir(t),
                    Etape::Clic => v.aller(t, sortie),
                    Etape::Fin => {
                        if reprise {
                            v.jouer(t, sortie);
                            nouvelle_reprise = false;
                        } else {
                            v.voir(t);
                        }
                    }
                }
            }
            _ => {}
        });
        self.biblio.reprise = nouvelle_reprise;
    }

    /// Le fichier choisi, s'il y en a un : son chemin et son genre.
    fn biblio_fichier_choisi(&mut self) -> Option<(PathBuf, Genre)> {
        let r = self.biblio.nav.choisie().and_then(|e| e.genre.filter(|_| !e.dossier).map(|g| (e.chemin.clone(), g)));
        if r.is_none() {
            self.status = "Choisis d'abord une image, une vidéo ou un son dans le panneau Bibliothèque.".into();
            self.show_pane(Pane::Bibliotheque);
        }
        r
    }

    /// « Importer dans le plan » : une image se pose sur le calque courant, une vidéo devient un calque vidéo (avec son
    /// son), un son se pose sur une piste à la tête de lecture.
    pub(crate) fn biblio_importer(&mut self, p: &Path) {
        match Genre::de(p) {
            Some(Genre::Image) => self.import_image_fichier(p),
            Some(Genre::Video) => {
                self.import_dropped_video(p);
            }
            Some(Genre::Son) => self.insert_bruitage(p.to_path_buf(), None, None),
            None => self.status = format!("« {} » : format non géré par SHOT.", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
        }
    }

    pub(crate) fn biblio_ajouter_references(&mut self, p: &Path) {
        match Genre::de(p) {
            Some(Genre::Image | Genre::Video) => self.refs_add_paths(vec![p.to_path_buf()]),
            _ => self.status = "Seules les images et les vidéos vont dans les références.".into(),
        }
    }

    pub(crate) fn biblio_ajouter_montage(&mut self, p: &Path) {
        match Genre::de(p) {
            Some(Genre::Son) => self.insert_bruitage(p.to_path_buf(), None, None),
            Some(Genre::Video) => {
                self.import_dropped_video(p);
            }
            _ => self.status = "Seuls les sons et les vidéos vont dans le montage.".into(),
        }
    }

    /// Un fichier glissé depuis la bibliothèque, lâché en `pos` : comme depuis l'Explorateur de Windows. Une image va sur la
    /// planche des Vignettes ou dans les références si c'est là qu'on la lâche, sinon sur le dessin.
    pub(crate) fn biblio_lacher(&mut self, ctx: &egui::Context, chemin: PathBuf, genre: Genre, pos: egui::Pos2) {
        match genre {
            Genre::Image => {
                if self.board_take_drop(std::slice::from_ref(&chemin), Some(pos)) || self.refs_take_drop(ctx, std::slice::from_ref(&chemin)) {
                    return;
                }
                if self.canvas_rect.contains(pos) {
                    self.import_image_fichier(&chemin);
                } else {
                    self.status = "Lâche l'image sur le dessin, sur la planche des vignettes ou sur les références.".into();
                }
            }
            // (Un son se pose sur une piste du montage par `bruitage_drop`, qui a déjà pris le lâcher ; ailleurs, à la tête de lecture.)
            Genre::Son => self.insert_bruitage(chemin, None, None),
            Genre::Video => {
                self.deposer_chemins(ctx, vec![chemin], Some(pos));
            }
        }
    }

    /// Les commandes `bibliotheque.*` ; vrai si `action` en était une.
    pub(crate) fn bibliotheque_action(&mut self, action: &str) -> bool {
        match action {
            "bibliotheque.retour" => self.biblio_commande(Commande::Retour),
            "bibliotheque.dossier_parent" => self.biblio_commande(Commande::Remonter),
            "bibliotheque.actualiser" => self.biblio_commande(Commande::Actualiser),
            "bibliotheque.favori" => self.biblio_commande(Commande::Favori),
            "bibliotheque.lecture" => self.biblio_commande(Commande::JouerPause),
            "bibliotheque.importer_plan" => {
                if let Some((p, _)) = self.biblio_fichier_choisi() {
                    self.biblio_importer(&p);
                }
            }
            "bibliotheque.ajouter_references" => {
                if let Some((p, _)) = self.biblio_fichier_choisi() {
                    self.biblio_ajouter_references(&p);
                }
            }
            "bibliotheque.ajouter_montage" => {
                if let Some((p, _)) = self.biblio_fichier_choisi() {
                    self.biblio_ajouter_montage(&p);
                }
            }
            "bibliotheque.afficher_explorateur" => {
                let p = self.biblio.nav.selection.clone().unwrap_or_else(|| self.biblio.nav.dossier.clone());
                self.biblio_commande(Commande::Explorateur(p));
            }
            "bibliotheque.copier_chemin" => {
                let p = self.biblio.nav.selection.clone().unwrap_or_else(|| self.biblio.nav.dossier.clone());
                self.biblio_commande(Commande::CopierChemin(p));
            }
            _ => return false,
        }
        true
    }

    // ------------------------------------------------------------------ le panneau

    pub(crate) fn bibliotheque_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        // La première ouverture du panneau lance la lecture d'un dossier (jamais au lancement de SHOT).
        if !self.biblio.nav.demarre {
            self.biblio_demarrer();
        }
        self.biblio.vu = ctx.cumulative_pass_nr();
        let dans_la_fenetre_principale = ctx.viewport_id() == egui::ViewportId::ROOT;
        let rect = ui.max_rect();
        self.biblio.rect = dans_la_fenetre_principale.then_some(rect);
        if ctx.input(|i| i.pointer.any_pressed()) {
            self.biblio.focus = ctx.pointer_interact_pos().is_some_and(|p| rect.contains(p));
        }
        let mut cmds: Vec<Commande> = Vec::new();
        self.biblio_touches(ui, &mut cmds);
        let r = self.settings.bibliotheque.clone().nettoye();
        let (w, h) = (rect.width(), rect.height());
        if self.biblio.apercu.is_some() {
            let deux_colonnes = w >= LARGEUR_DEUX_COLONNES;
            let montre = if deux_colonnes {
                egui::SidePanel::right("biblio_apercu")
                    .resizable(true)
                    .default_width(w * r.part_apercu)
                    .width_range(w * 0.2..=w * 0.8)
                    .show_inside(ui, |ui| self.biblio_apercu_ui(ui, &mut cmds))
                    .response
                    .rect
                    .width()
                    / w.max(1.0)
            } else {
                egui::TopBottomPanel::bottom("biblio_apercu")
                    .resizable(true)
                    .default_height(h * r.part_apercu)
                    .height_range(h * 0.2..=h * 0.8)
                    .show_inside(ui, |ui| self.biblio_apercu_ui(ui, &mut cmds))
                    .response
                    .rect
                    .height()
                    / h.max(1.0)
            };
            // (La part de l'aperçu se retient quand on lâche la poignée.)
            if (montre - r.part_apercu).abs() > 0.01 && !ctx.input(|i| i.pointer.any_down()) && montre.is_finite() {
                self.settings.bibliotheque.part_apercu = montre.clamp(0.15, 0.85);
                self.settings_a_ecrire = true;
            }
        }
        self.biblio_explorateur_ui(ui, &r, &mut cmds);
        self.biblio_etiquette_glisse(&ctx);
        for c in cmds {
            self.biblio_commande(c);
        }
    }

    /// Les touches du panneau, quand la souris le survole (et qu'aucun champ n'attend du texte).
    fn biblio_touches(&mut self, ui: &egui::Ui, cmds: &mut Vec<Commande>) {
        let ctx = ui.ctx();
        if ctx.wants_keyboard_input() || !ui.rect_contains_pointer(ui.max_rect()) {
            return;
        }
        let video = matches!(self.biblio.apercu, Some(Apercu::Video(_)));
        let (espace, retour, gauche, droite, haut, bas, entree) = ctx.input(|i| {
            let n = |k: Key| i.key_pressed(k) && !i.modifiers.any();
            (n(Key::Space), n(Key::Backspace), n(Key::ArrowLeft), n(Key::ArrowRight), n(Key::ArrowUp), n(Key::ArrowDown), n(Key::Enter))
        });
        if espace {
            cmds.push(Commande::JouerPause);
        }
        if retour {
            cmds.push(Commande::Remonter);
        }
        if gauche || droite {
            let sens = if droite { 1 } else { -1 };
            cmds.push(if video { Commande::Pas(sens as i64) } else { Commande::Voisin(sens) });
        }
        if haut || bas {
            cmds.push(Commande::Voisin(if bas { 1 } else { -1 }));
        }
        if entree {
            if let Some(e) = self.biblio.nav.choisie() {
                if e.dossier {
                    cmds.push(Commande::Entrer(e.chemin.clone()));
                }
            }
        }
    }

    /// Le nom du fichier glissé, qui suit le pointeur.
    fn biblio_etiquette_glisse(&self, ctx: &egui::Context) {
        if let Some(d) = egui::DragAndDrop::payload::<BiblioDrag>(ctx) {
            if let Some(p) = ctx.pointer_latest_pos() {
                egui::Area::new(egui::Id::new("biblio_glisse")).order(egui::Order::Tooltip).fixed_pos(p + vec2(14.0, 10.0)).interactable(false).show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(format!("{} {}", d.genre.icone(), d.chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
                    });
                });
            }
        }
    }

    // ------------------------------------------------------------------ l'explorateur

    fn biblio_explorateur_ui(&mut self, ui: &mut egui::Ui, r: &biblio::BiblioReglages, cmds: &mut Vec<Commande>) {
        // (La largeur du panneau, lue avant toute rangée : une rangée qui déborde agrandit la place que `ui` croit avoir,
        // et la grille aurait alors des colonnes hors du cadre.)
        let largeur = ui.available_width();
        let dossier = self.biblio.nav.dossier.clone();
        // En haut : retour, parent, actualiser, favori, fil d'Ariane.
        ui.horizontal(|ui| {
            if self.bouton_si(ui, self.biblio.nav.peut_revenir(), ph::ARROW_LEFT, "Retour", "Le dossier visité avant celui-ci.", "bibliotheque.retour").clicked() {
                cmds.push(Commande::Retour);
            }
            if self.bouton_si(ui, self.biblio.nav.parent().is_some(), ph::ARROW_UP, "Dossier parent", "Remonter d'un dossier (Retour arrière).", "bibliotheque.dossier_parent").clicked() {
                cmds.push(Commande::Remonter);
            }
            if self.bouton(ui, ph::ARROWS_CLOCKWISE, "Actualiser", "Relire le dossier (fichiers ajoutés ou retirés depuis) et refaire les vignettes.", "bibliotheque.actualiser").clicked() {
                cmds.push(Commande::Actualiser);
            }
            let favori = r.est_favori(&dossier);
            if self.bouton_etat(ui, favori, ph::STAR, if favori { "Retirer des favoris" } else { "Ajouter aux favoris" }, "Garde ce dossier dans les favoris, sous la barre d'adresse. Il est retenu d'une séance à l'autre.", "bibliotheque.favori").clicked() {
                cmds.push(Commande::Favori);
            }
            egui::ScrollArea::horizontal().id_salt("biblio_fil").auto_shrink([false, true]).stick_to_right(true).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (i, (nom, chemin)) in biblio::fil_d_ariane(&dossier).into_iter().enumerate() {
                        if i > 0 {
                            ui.label(RichText::new(ph::CARET_RIGHT).size(11.0).color(theme::texte_doux()));
                        }
                        let resp = ui.add(egui::Button::new(RichText::new(nom).size(12.5)).frame(false));
                        if resp.on_hover_text(chemin.display().to_string()).clicked() {
                            cmds.push(Commande::Entrer(chemin));
                        }
                    }
                });
            });
        });
        // Les favoris et les lecteurs.
        let projet = self.biblio_dossier_projet();
        let fixes = match &self.biblio.fixes {
            Some((p, f)) if *p == projet => f.clone(),
            _ => {
                let f = biblio::favoris_fixes(projet.as_deref(), biblio::dossier_profil().as_deref());
                self.biblio.fixes = Some((projet.clone(), f.clone()));
                f
            }
        };
        let lecteurs = self.biblio.lecteurs.clone().unwrap_or_default();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 2.0);
            for f in &fixes {
                let sel = biblio::meme_chemin(&f.chemin, &dossier);
                if ui.add(egui::SelectableLabel::new(sel, RichText::new(format!("{} {}", f.icone, f.nom)).size(12.0))).on_hover_text(f.chemin.display().to_string()).clicked() {
                    cmds.push(Commande::Entrer(f.chemin.clone()));
                }
            }
            for f in &r.favoris {
                let sel = biblio::meme_chemin(f, &dossier);
                let nom = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| f.display().to_string());
                let resp = ui.add(egui::SelectableLabel::new(sel, RichText::new(format!("{} {}", ph::STAR, nom)).size(12.0))).on_hover_text(f.display().to_string());
                if resp.clicked() {
                    cmds.push(Commande::Entrer(f.clone()));
                }
                resp.context_menu(|ui| {
                    if ui.button("Retirer des favoris").clicked() {
                        cmds.push(Commande::RetirerFavori(f.clone()));
                        ui.close_menu();
                    }
                });
            }
            if !lecteurs.is_empty() {
                ui.separator();
            }
            for l in &lecteurs {
                let sel = dossier.starts_with(l);
                let nom = l.to_string_lossy().trim_end_matches(['\\', '/']).to_string();
                if ui.add(egui::SelectableLabel::new(sel, RichText::new(format!("{} {}", ph::HARD_DRIVE, nom)).size(12.0))).on_hover_text(format!("Le lecteur {nom}")).clicked() {
                    cmds.push(Commande::Entrer(l.clone()));
                }
            }
        });
        // Le filtre, la recherche, la vue (à la ligne quand le panneau est étroit).
        ui.horizontal_wrapped(|ui| {
            let mut filtre = r.filtre;
            for f in Filtre::TOUS {
                if ui.add(egui::SelectableLabel::new(filtre == f, RichText::new(f.nom()).size(12.0))).clicked() {
                    filtre = f;
                }
            }
            if filtre != r.filtre {
                self.settings.bibliotheque.filtre = filtre;
                self.settings_a_ecrire = true;
            }
            let champ = (largeur - 200.0).clamp(90.0, 200.0);
            ui.add(egui::TextEdit::singleline(&mut self.biblio.nav.recherche).hint_text("Chercher dans ce dossier…").desired_width(champ));
            let mut vue = r.vue;
            crate::bulles::choix(ui, &mut vue, Vue::Grille, ph::SQUARES_FOUR, "Vignettes", "Les fichiers en grille de vignettes.", "");
            crate::bulles::choix(ui, &mut vue, Vue::Liste, ph::LIST, "Liste", "Les fichiers en liste : nom, type, durée, taille, date.", "");
            if vue != r.vue {
                self.settings.bibliotheque.vue = vue;
                self.settings_a_ecrire = true;
            }
            let mut tout = r.tout_montrer;
            let icone_tout = if tout { ph::EYE } else { ph::EYE_SLASH };
            if crate::bulles::interrupteur(ui, &mut tout, icone_tout, "Tout montrer", "Montre aussi les fichiers que SHOT ne sait pas lire (grisés).", "").changed() {
                self.settings.bibliotheque.tout_montrer = tout;
                self.settings_a_ecrire = true;
            }
            if r.vue == Vue::Grille {
                let mut taille = r.taille;
                if ui.add(egui::Slider::new(&mut taille, biblio::TAILLE_MIN..=biblio::TAILLE_MAX).show_value(false)).on_hover_text("Taille des vignettes").changed() {
                    self.settings.bibliotheque.taille = taille;
                    self.settings_a_ecrire = true;
                }
            }
        });
        ui.add_space(2.0);
        self.biblio.nav.rafraichir(r);
        // Le compte, ou ce qui manque.
        let n = self.biblio.nav.rangs().len();
        if let Some(e) = self.biblio.nav.erreur.clone() {
            ui.add_space(8.0);
            ui.label(RichText::new(e).strong());
            if self.biblio.nav.parent().is_some() && ui.button("Remonter d'un dossier").clicked() {
                cmds.push(Commande::Remonter);
            }
            return;
        }
        if n == 0 {
            ui.add_space(8.0);
            if self.biblio.nav.lit() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Lecture du dossier…").weak());
                });
            } else if self.biblio.nav.entrees.is_empty() {
                ui.label(RichText::new("Ce dossier est vide.").weak());
            } else if self.biblio.nav.recherche.trim().is_empty() {
                ui.label(RichText::new("Aucune image, vidéo ou son ici.").weak());
                ui.label(RichText::new("Le bouton « Tout montrer » (l'œil) affiche aussi les autres fichiers.").weak().small());
            } else {
                ui.label(RichText::new("Rien ne correspond à cette recherche.").weak());
            }
            return;
        }
        ui.horizontal(|ui| {
            if self.biblio.nav.lit() {
                ui.spinner();
            }
            let texte = if self.biblio.nav.tronque { format!("{n} éléments (liste coupée)") } else { format!("{n} élément{}", if n > 1 { "s" } else { "" }) };
            ui.label(RichText::new(texte).weak().small());
        });
        let ffmpeg = self.biblio_ffmpeg();
        match r.vue {
            Vue::Grille => self.biblio_grille_ui(ui, largeur, r, ffmpeg, cmds),
            Vue::Liste => self.biblio_liste_ui(ui, largeur, r, ffmpeg, cmds),
        }
    }

    /// La grille de vignettes.
    fn biblio_grille_ui(&mut self, ui: &mut egui::Ui, largeur: f32, r: &biblio::BiblioReglages, ffmpeg: Ffmpeg, cmds: &mut Vec<Commande>) {
        ui.set_max_width(largeur);
        let cote = r.taille as f32;
        // (Une marge de sécurité : la barre de défilement et le bord du cadre. Une case hors du cadre ne se dessinerait pas.)
        let par_ligne = (((largeur - 18.0 + ECART) / (cote + ECART)).floor() as usize).max(1);
        let total = self.biblio.nav.rangs().len();
        let lignes = total.div_ceil(par_ligne);
        let mut demandes: Vec<Demande> = Vec::new();
        {
            let b = &mut self.biblio;
            let vignettes = b.vignettes.get_or_insert_with(|| Vignettes::new(ffmpeg));
            vignettes.debut_image();
            let (nav, apercu) = (&b.nav, &b.apercu);
            let joue = apercu.as_ref().map(|a| a.chemin().to_path_buf());
            let keymap = &self.keymap;
            ui.spacing_mut().item_spacing = vec2(ECART, ECART);
            egui::ScrollArea::vertical().id_salt("biblio_grille").auto_shrink([false, false]).show_rows(ui, cote + NOM_H, lignes, |ui, plage| {
                for ligne in plage {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = ECART;
                        for col in 0..par_ligne {
                            let k = ligne * par_ligne + col;
                            let Some(&rang) = nav.rangs().get(k) else { break };
                            let e = &nav.entrees[rang];
                            if let (Some(g), false) = (e.genre, e.dossier) {
                                demandes.push(Demande { chemin: e.chemin.clone(), genre: g });
                            }
                            let choisi = nav.selection.as_ref() == Some(&e.chemin);
                            let meta = vignettes.meta.get(&e.chemin).copied();
                            let texture = vignettes.texture(&e.chemin).cloned();
                            let ouvert = joue.as_deref() == Some(e.chemin.as_path());
                            cellule(ui, e, cote, texture.as_ref(), meta, choisi, ouvert, keymap, cmds);
                        }
                    });
                }
            });
            vignettes.souhaiter(&demandes);
            vignettes.nettoyer();
        }
    }

    /// La liste : nom, type, durée, taille, date.
    fn biblio_liste_ui(&mut self, ui: &mut egui::Ui, largeur: f32, r: &biblio::BiblioReglages, ffmpeg: Ffmpeg, cmds: &mut Vec<Commande>) {
        ui.set_max_width(largeur);
        // Les colonnes qui tiennent : le nom d'abord, puis la durée, la taille, le type, la date.
        let (type_w, duree_w, taille_w, date_w) = (96.0, 52.0, 64.0, 74.0);
        let mut libre = largeur - 160.0;
        let mut montrer = |w: f32| {
            if libre >= w {
                libre -= w;
                true
            } else {
                false
            }
        };
        let (avec_duree, avec_taille, avec_type, avec_date) = (montrer(duree_w), montrer(taille_w), montrer(type_w), montrer(date_w));
        // L'en-tête (clic : trier).
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let mut entete = |ui: &mut egui::Ui, nom: &str, tri: Option<Tri>, w: Option<f32>| {
                let marque = if tri == Some(r.tri) { if r.tri_inverse { " ▾" } else { " ▴" } } else { "" };
                let texte = RichText::new(format!("{nom}{marque}")).small().color(theme::texte_doux());
                let taille = vec2(w.unwrap_or_else(|| ui.available_width()), 18.0);
                let (rect, resp) = ui.allocate_exact_size(taille, if tri.is_some() { Sense::click() } else { Sense::hover() });
                ui.painter().text(rect.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, texte.text(), FontId::proportional(11.0), theme::texte_doux());
                if let Some(t) = tri {
                    if resp.on_hover_text("Trier").clicked() {
                        cmds.push(Commande::Trier(t));
                    }
                }
            };
            let reste = largeur - (if avec_duree { duree_w } else { 0.0 }) - (if avec_taille { taille_w } else { 0.0 }) - (if avec_type { type_w } else { 0.0 }) - (if avec_date { date_w } else { 0.0 });
            entete(ui, "Nom", Some(Tri::Nom), Some(reste.max(60.0) - 8.0));
            if avec_type {
                entete(ui, "Type", Some(Tri::Type), Some(type_w));
            }
            if avec_duree {
                entete(ui, "Durée", None, Some(duree_w));
            }
            if avec_taille {
                entete(ui, "Taille", Some(Tri::Taille), Some(taille_w));
            }
            if avec_date {
                entete(ui, "Date", Some(Tri::Date), Some(date_w));
            }
        });
        let total = self.biblio.nav.rangs().len();
        let mut demandes: Vec<Demande> = Vec::new();
        let b = &mut self.biblio;
        let vignettes = b.vignettes.get_or_insert_with(|| Vignettes::new(ffmpeg));
        let nav = &b.nav;
        let keymap = &self.keymap;
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical().id_salt("biblio_liste").auto_shrink([false, false]).show_rows(ui, LIGNE_H, total, |ui, plage| {
            for k in plage {
                let Some(&rang) = nav.rangs().get(k) else { continue };
                let e = &nav.entrees[rang];
                // Dans la liste, seules les durées des vidéos et des sons sont lues (pas de vignettes d'images).
                if let (Some(g @ (Genre::Video | Genre::Son)), false) = (e.genre, e.dossier) {
                    demandes.push(Demande { chemin: e.chemin.clone(), genre: g });
                }
                let meta = vignettes.meta.get(&e.chemin).copied();
                let choisi = nav.selection.as_ref() == Some(&e.chemin);
                ligne(ui, e, meta, choisi, [avec_type, avec_duree, avec_taille, avec_date], [type_w, duree_w, taille_w, date_w], keymap, cmds);
            }
        });
        vignettes.souhaiter(&demandes);
    }

    // ------------------------------------------------------------------ l'aperçu

    fn biblio_apercu_ui(&mut self, ui: &mut egui::Ui, cmds: &mut Vec<Commande>) {
        let Some(chemin) = self.biblio.apercu.as_ref().map(|a| a.chemin().to_path_buf()) else { return };
        let r = self.settings.bibliotheque.clone();
        ui.horizontal(|ui| {
            let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            ui.add(egui::Label::new(RichText::new(nom).strong()).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::bulles::icone(ui, ph::X, "Fermer l'aperçu", "Ferme l'aperçu et arrête la lecture.", "").clicked() {
                    cmds.push(Commande::FermerApercu);
                }
            });
        });
        let taille_fichier = self.biblio.apercu_octets.map(taille_texte);
        // (La sortie audio, pour lire l'horloge : jamais pour jouer ici.)
        let moteur;
        let muet = Muet;
        let sortie: &dyn SortieAudio = match self.audio.as_ref().filter(|e| e.ok()) {
            Some(e) => {
                moteur = Moteur(e);
                &moteur
            }
            None => &muet,
        };
        let raccourci_lecture = self.raccourci("bibliotheque.lecture");
        match self.biblio.apercu.as_mut() {
            Some(Apercu::Image(a)) => apercu_image(ui, a, taille_fichier),
            Some(Apercu::Son(s)) => apercu_son(ui, s, sortie, &r, &raccourci_lecture, cmds),
            Some(Apercu::Video(v)) => apercu_video(ui, v, sortie, &r, &raccourci_lecture, taille_fichier, cmds),
            None => {}
        }
    }
}

/// Rend la sortie audio sans y toucher (un projet qui joue l'a prise) : l'aperçu s'arrête seul.
fn sortie_sans_toucher(mut a: Apercu) {
    a.rendre_la_sortie(&Muet);
}

// ---------------------------------------------------------------------------------- les cases

/// Le texte de survol d'un fichier.
fn survol_texte(e: &Entree, meta: Option<Meta>) -> String {
    let mut t = e.chemin.display().to_string();
    if !e.dossier {
        t.push_str(&format!("\n{} · {}", e.type_texte(), taille_texte(e.octets)));
        if let Some(m) = meta {
            if let Some(d) = m.dims {
                t.push_str(&format!(" · {}", dims_texte(d)));
            }
            if let Some(d) = m.duree {
                t.push_str(&format!(" · {}", duree_texte(d)));
            }
        }
        if e.genre.is_none() {
            t.push_str("\nSHOT ne sait pas lire ce fichier.");
        }
    }
    t
}

/// Le menu d'un fichier (clic droit) : ses commandes sont aussi des actions de la palette.
fn menu_fichier(ui: &mut egui::Ui, e: &Entree, keymap: &crate::keymap::Keymap, cmds: &mut Vec<Commande>) {
    let mut item = |ui: &mut egui::Ui, texte: &str, action: &str, c: Commande| {
        let raccourci = keymap.label(action);
        if ui.add(egui::Button::new(texte).shortcut_text(raccourci)).clicked() {
            cmds.push(c);
            ui.close_menu();
        }
    };
    if e.dossier {
        item(ui, "Ouvrir ce dossier", "", Commande::Entrer(e.chemin.clone()));
    }
    if let (Some(g), false) = (e.genre, e.dossier) {
        let p = e.chemin.clone();
        item(ui, "Importer dans le plan", "bibliotheque.importer_plan", Commande::Importer(p.clone()));
        if g != Genre::Son {
            item(ui, "Ajouter aux références", "bibliotheque.ajouter_references", Commande::AjouterReferences(p.clone()));
        }
        if g != Genre::Image {
            item(ui, "Ajouter au montage", "bibliotheque.ajouter_montage", Commande::AjouterMontage(p));
        }
        ui.separator();
    }
    item(ui, "Ouvrir le dossier dans l'Explorateur", "bibliotheque.afficher_explorateur", Commande::Explorateur(e.chemin.clone()));
    item(ui, "Copier le chemin", "bibliotheque.copier_chemin", Commande::CopierChemin(e.chemin.clone()));
}

/// Les gestes communs à une case et à une ligne : clic, double-clic, glisser, clic droit.
fn gestes(resp: egui::Response, e: &Entree, keymap: &crate::keymap::Keymap, cmds: &mut Vec<Commande>) {
    if resp.drag_started_by(egui::PointerButton::Primary) {
        if let (Some(g), false) = (e.genre, e.dossier) {
            cmds.push(Commande::Glisser(e.chemin.clone(), g));
            cmds.push(Commande::Choisir(e.chemin.clone()));
        }
    } else if resp.double_clicked() && e.dossier {
        cmds.push(Commande::Entrer(e.chemin.clone()));
    } else if resp.clicked() || resp.secondary_clicked() {
        cmds.push(Commande::Choisir(e.chemin.clone()));
    }
    resp.context_menu(|ui| menu_fichier(ui, e, keymap, cmds));
}

/// Une case de la grille.
#[allow(clippy::too_many_arguments)]
fn cellule(ui: &mut egui::Ui, e: &Entree, cote: f32, texture: Option<&TextureHandle>, meta: Option<Meta>, choisi: bool, ouvert: bool, keymap: &crate::keymap::Keymap, cmds: &mut Vec<Commande>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(cote, cote + NOM_H), Sense::click_and_drag());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let peintre = ui.painter();
    let zone = Rect::from_min_size(rect.min, vec2(cote, cote));
    let grise = e.genre.is_none() && !e.dossier;
    if choisi {
        peintre.rect_filled(rect.expand(1.0), 5.0, ui.visuals().selection.bg_fill.gamma_multiply(0.55));
    } else if resp.hovered() {
        peintre.rect_filled(rect.expand(1.0), 5.0, theme::eleve());
    }
    peintre.rect_filled(zone.shrink(2.0), 4.0, theme::apercu_trait());
    match (texture, e.dossier) {
        (Some(t), false) => {
            let taille = t.size_vec2();
            let k = ((cote - 4.0) / taille.x).min((cote - 4.0) / taille.y);
            let r = Rect::from_center_size(zone.center(), taille * k);
            peintre.image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        _ => {
            let (icone, couleur) = if e.dossier {
                (ph::FOLDER_SIMPLE, theme::accent_texte())
            } else if let Some(g) = e.genre {
                (g.icone(), theme::texte_doux())
            } else {
                (ph::FILE, theme::texte_doux().gamma_multiply(0.6))
            };
            peintre.text(zone.center(), Align2::CENTER_CENTER, icone, FontId::proportional((cote * 0.4).clamp(18.0, 64.0)), couleur);
        }
    }
    // L'insigne du genre et la durée, sur un fond sombre.
    if let (Some(g), false) = (e.genre, e.dossier) {
        let mut texte = String::new();
        if let Some(d) = meta.and_then(|m| m.duree) {
            texte = duree_texte(d);
        }
        if g != Genre::Image || !texte.is_empty() {
            let galerie = FontId::proportional(10.5);
            let t = if texte.is_empty() { g.icone().to_string() } else { format!("{} {texte}", g.icone()) };
            let largeur = ui.fonts(|f| f.layout_no_wrap(t.clone(), galerie.clone(), Color32::WHITE).size().x) + 8.0;
            let pastille = Rect::from_min_size(pos2(zone.right() - largeur - 4.0, zone.bottom() - 18.0), vec2(largeur, 14.0));
            peintre.rect_filled(pastille, 3.0, Color32::from_black_alpha(170));
            peintre.text(pastille.center(), Align2::CENTER_CENTER, t, galerie, Color32::WHITE);
        }
        if meta.is_some_and(|m| m.illisible) {
            peintre.text(zone.left_top() + vec2(6.0, 4.0), Align2::LEFT_TOP, ph::WARNING, FontId::proportional(13.0), Color32::from_rgb(230, 170, 60));
        }
    }
    if ouvert {
        peintre.rect_stroke(zone.shrink(1.0), 4.0, Stroke::new(2.0_f32, theme::contour()), StrokeKind::Inside);
    }
    let nom = Rect::from_min_max(pos2(rect.left() + 2.0, zone.bottom() + 1.0), pos2(rect.right() - 2.0, rect.bottom()));
    let encre = if grise { theme::texte_doux().gamma_multiply(0.7) } else if choisi { theme::texte_fort() } else { theme::texte() };
    peintre.with_clip_rect(nom).text(nom.center_top() + vec2(0.0, 1.0), Align2::CENTER_TOP, &e.nom, FontId::proportional(11.5), encre);
    let resp = resp.on_hover_ui(|ui| {
        ui.label(survol_texte(e, meta));
    });
    gestes(resp, e, keymap, cmds);
}

/// Une ligne de la liste.
#[allow(clippy::too_many_arguments)]
fn ligne(ui: &mut egui::Ui, e: &Entree, meta: Option<Meta>, choisi: bool, colonnes: [bool; 4], largeurs: [f32; 4], keymap: &crate::keymap::Keymap, cmds: &mut Vec<Commande>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), LIGNE_H), Sense::click_and_drag());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let peintre = ui.painter_at(rect);
    if choisi {
        peintre.rect_filled(rect, 3.0, ui.visuals().selection.bg_fill.gamma_multiply(0.55));
    } else if resp.hovered() {
        peintre.rect_filled(rect, 3.0, theme::eleve());
    }
    let grise = e.genre.is_none() && !e.dossier;
    let encre = if grise { theme::texte_doux().gamma_multiply(0.7) } else if choisi { theme::texte_fort() } else { theme::texte() };
    let (avec_type, avec_duree, avec_taille, avec_date) = (colonnes[0], colonnes[1], colonnes[2], colonnes[3]);
    let [type_w, duree_w, taille_w, date_w] = largeurs;
    // De droite à gauche : date, taille, durée, type ; le nom prend le reste.
    let mut droite = rect.right();
    let mut colonne = |texte: String, w: f32, monospace: bool| {
        let x = droite - w;
        let police = if monospace { FontId::monospace(10.5) } else { FontId::proportional(11.5) };
        peintre.with_clip_rect(Rect::from_min_max(pos2(x, rect.top()), pos2(droite, rect.bottom()))).text(pos2(x + 4.0, rect.center().y), Align2::LEFT_CENTER, texte, police, theme::texte_doux());
        droite = x;
    };
    if avec_date {
        colonne(date_texte(e.secondes_modifie()), date_w, false);
    }
    if avec_taille {
        colonne(if e.dossier { String::new() } else { taille_texte(e.octets) }, taille_w, false);
    }
    if avec_duree {
        colonne(meta.and_then(|m| m.duree).map(duree_texte).unwrap_or_default(), duree_w, true);
    }
    if avec_type {
        colonne(e.type_texte(), type_w, false);
    }
    let icone = if e.dossier { ph::FOLDER_SIMPLE } else { e.genre.map_or(ph::FILE, Genre::icone) };
    let couleur = if e.dossier { theme::accent_texte() } else { theme::texte_doux() };
    peintre.text(pos2(rect.left() + 4.0, rect.center().y), Align2::LEFT_CENTER, icone, FontId::proportional(14.0), couleur);
    let nom = Rect::from_min_max(pos2(rect.left() + 24.0, rect.top()), pos2(droite - 4.0, rect.bottom()));
    peintre.with_clip_rect(nom).text(pos2(nom.left(), rect.center().y), Align2::LEFT_CENTER, &e.nom, FontId::proportional(12.5), encre);
    let resp = resp.on_hover_ui(|ui| {
        ui.label(survol_texte(e, meta));
    });
    gestes(resp, e, keymap, cmds);
}

// ---------------------------------------------------------------------------------- l'aperçu

/// Le rectangle où tient une image de `taille` dans `zone`, avec le zoom `zoom` et le décalage `decalage`.
fn rect_image(zone: Rect, taille: Vec2, zoom: f32, decalage: Vec2) -> Rect {
    let k = (zone.width() / taille.x.max(1.0)).min(zone.height() / taille.y.max(1.0)) * zoom;
    Rect::from_center_size(zone.center() + decalage, taille * k)
}

fn apercu_image(ui: &mut egui::Ui, a: &mut ApercuImage, taille_fichier: Option<String>) {
    let info = ui.available_height().min(22.0);
    let zone_h = (ui.available_height() - info).max(40.0);
    let (zone, resp) = ui.allocate_exact_size(vec2(ui.available_width(), zone_h), Sense::click_and_drag());
    let peintre = ui.painter_at(zone);
    peintre.rect_filled(zone, 4.0, theme::apercu_trait());
    if let Some(e) = &a.erreur {
        peintre.text(zone.center(), Align2::CENTER_CENTER, e, FontId::proportional(13.0), theme::texte_doux());
    } else if let Some(t) = a.texture.clone() {
        // Molette : zoom autour du pointeur ; glisser : déplacer ; double-clic : ajuster.
        if resp.hovered() {
            let molette = ui.input(|i| i.smooth_scroll_delta.y);
            if molette != 0.0 {
                let ancre = ui.input(|i| i.pointer.hover_pos()).map_or(Vec2::ZERO, |p| p - zone.center());
                a.zoomer((molette * 0.004).exp(), ancre);
            }
        }
        if resp.dragged() && a.zoom > 1.0 {
            a.decalage += resp.drag_delta();
        }
        if resp.double_clicked() {
            a.recentrer();
        }
        let r = rect_image(zone, t.size_vec2(), a.zoom, a.decalage);
        peintre.image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    } else {
        peintre.text(zone.center(), Align2::CENTER_CENTER, "Lecture de l'image…", FontId::proportional(13.0), theme::texte_doux());
    }
    ui.horizontal(|ui| {
        let mut t = String::new();
        if let Some(d) = a.dims {
            t.push_str(&dims_texte(d));
            t.push_str(" px");
        }
        if let Some(s) = taille_fichier {
            if !t.is_empty() {
                t.push_str(" · ");
            }
            t.push_str(&s);
        }
        if a.zoom > 1.0 {
            t.push_str(&format!(" · zoom {:.0} %", a.zoom * 100.0));
        } else if a.texture.is_some() {
            t.push_str(" · molette : zoom");
        }
        ui.label(RichText::new(t).small().weak());
    });
}

/// La barre de temps : un clic ou un glisser donne la fraction (0 à 1) ; rien tant qu'on n'y touche pas.
fn barre_de_temps(ui: &mut egui::Ui, fraction: f32, actif: bool) -> (Option<(f32, Etape)>, Rect) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), if actif { Sense::click_and_drag() } else { Sense::hover() });
    let piste = Rect::from_center_size(rect.center(), vec2(rect.width(), 5.0));
    let p = ui.painter();
    p.rect_filled(piste, 2.5, theme::controle());
    let x = piste.left() + piste.width() * fraction.clamp(0.0, 1.0);
    p.rect_filled(Rect::from_min_max(piste.min, pos2(x, piste.bottom())), 2.5, theme::accent());
    p.circle_filled(pos2(x, rect.center().y), 6.0, if resp.hovered() || resp.dragged() { theme::texte_fort() } else { theme::texte() });
    let frac = |resp: &egui::Response| resp.interact_pointer_pos().map(|q| ((q.x - piste.left()) / piste.width().max(1.0)).clamp(0.0, 1.0));
    let mut sortie = None;
    if resp.drag_started() {
        sortie = frac(&resp).map(|f| (f, Etape::Debut));
    } else if resp.dragged() {
        sortie = frac(&resp).map(|f| (f, Etape::Pendant));
    } else if resp.drag_stopped() {
        sortie = frac(&resp).map(|f| (f, Etape::Fin));
    } else if resp.clicked() {
        sortie = frac(&resp).map(|f| (f, Etape::Clic));
    }
    (sortie, rect)
}

/// Volume, boucle : communs au son et à la vidéo.
fn reglages_lecture(ui: &mut egui::Ui, r: &biblio::BiblioReglages, cmds: &mut Vec<Commande>) {
    let mut boucle = r.boucle;
    if crate::bulles::interrupteur(ui, &mut boucle, ph::REPEAT, "Boucle", "Rejoue au début quand c'est fini.", "").changed() {
        cmds.push(Commande::Boucle(boucle));
    }
    ui.label(RichText::new(if r.volume > 0.0 { ph::SPEAKER_HIGH } else { ph::SPEAKER_SLASH }).size(15.0).color(theme::texte_doux()));
    let mut volume = r.volume;
    if ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false)).on_hover_text("Volume de l'aperçu").changed() {
        cmds.push(Commande::Volume(volume));
    }
}

fn apercu_son(ui: &mut egui::Ui, s: &mut ApercuSon, sortie: &dyn SortieAudio, r: &biblio::BiblioReglages, raccourci: &str, cmds: &mut Vec<Commande>) {
    let controles = 44.0;
    let zone_h = (ui.available_height() - controles).max(50.0);
    let (zone, resp) = ui.allocate_exact_size(vec2(ui.available_width(), zone_h), Sense::click_and_drag());
    let peintre = ui.painter_at(zone);
    peintre.rect_filled(zone, 4.0, theme::apercu_trait());
    let t = s.lecture.temps(sortie).min(s.duree.max(0.0));
    if let Some(e) = &s.erreur {
        peintre.text(zone.center(), Align2::CENTER_CENTER, e, FontId::proportional(13.0), theme::texte_doux());
    } else if let Some(pcm) = s.lecture.pcm.clone() {
        // La forme d'onde : une barre par colonne de pixels, centrée.
        let colonnes = zone.width().floor().max(1.0) as usize;
        let n = pcm.frames().max(1);
        let milieu = zone.center().y;
        let demi = zone.height() * 0.5 - 6.0;
        for c in 0..colonnes {
            let (a, b) = (c * n / colonnes, ((c + 1) * n / colonnes).max(c * n / colonnes + 1));
            let h = pcm.peak_between(a, b).clamp(0.0, 1.0) * demi;
            let x = zone.left() + c as f32 + 0.5;
            let passe = (c as f64 + 0.5) / colonnes as f64 <= t / s.duree.max(1e-6);
            peintre.line_segment([pos2(x, milieu - h.max(0.5)), pos2(x, milieu + h.max(0.5))], Stroke::new(1.0_f32, if passe { theme::accent_texte() } else { theme::texte_doux() }));
        }
        let x = zone.left() + zone.width() * (t / s.duree.max(1e-6)) as f32;
        peintre.line_segment([pos2(x, zone.top()), pos2(x, zone.bottom())], Stroke::new(1.5_f32, theme::texte_fort()));
        // Un clic ou un glisser sur l'onde : aller là.
        if resp.clicked() || resp.dragged() {
            if let Some(q) = resp.interact_pointer_pos() {
                let f = ((q.x - zone.left()) / zone.width().max(1.0)).clamp(0.0, 1.0) as f64;
                cmds.push(Commande::Chercher { t: f * s.duree, etape: Etape::Pendant });
            }
        }
    } else {
        peintre.text(zone.center(), Align2::CENTER_CENTER, "Lecture du son…", FontId::proportional(13.0), theme::texte_doux());
    }
    ui.horizontal(|ui| {
        let joue = s.lecture.joue;
        let (icone, nom) = if joue { (ph::PAUSE, "Pause") } else { (ph::PLAY, "Lecture") };
        if crate::bulles::icone_si(ui, s.pret(), icone, nom, "Écouter le son (ou le mettre en pause). Espace, la souris sur le panneau.", raccourci).clicked() {
            cmds.push(Commande::JouerPause);
        }
        ui.label(RichText::new(format!("{} / {}", duree_texte(t), duree_texte(s.duree))).monospace().small().color(theme::texte_doux()));
        reglages_lecture(ui, r, cmds);
    });
}

#[allow(clippy::too_many_arguments)]
fn apercu_video(ui: &mut egui::Ui, v: &mut ApercuVideo, sortie: &dyn SortieAudio, r: &biblio::BiblioReglages, raccourci: &str, taille_fichier: Option<String>, cmds: &mut Vec<Commande>) {
    let controles = 62.0;
    let zone_h = (ui.available_height() - controles).max(50.0);
    let (zone, resp) = ui.allocate_exact_size(vec2(ui.available_width(), zone_h), Sense::click());
    let peintre = ui.painter_at(zone);
    peintre.rect_filled(zone, 4.0, Color32::BLACK);
    if let Some(e) = &v.erreur {
        peintre.text(zone.center(), Align2::CENTER_CENTER, e, FontId::proportional(12.5), Color32::from_gray(190));
    } else if let Some(t) = &v.texture {
        let r = rect_image(zone, t.size_vec2(), 1.0, Vec2::ZERO);
        peintre.image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    } else {
        peintre.text(zone.center(), Align2::CENTER_CENTER, "Lecture de la vidéo…", FontId::proportional(12.5), Color32::from_gray(190));
    }
    if resp.clicked() {
        cmds.push(Commande::JouerPause);
    }
    let duree = v.duree();
    let t = v.temps(sortie);
    let fraction = if duree > 0.0 { (t / duree) as f32 } else { 0.0 };
    let pret = v.infos.is_some();
    let (geste, _) = barre_de_temps(ui, fraction, pret && duree > 0.0);
    if let Some((f, etape)) = geste {
        let tt = f as f64 * duree;
        cmds.push(Commande::Chercher { t: tt, etape });
    }
    ui.horizontal(|ui| {
        if crate::bulles::icone_si(ui, pret, ph::SKIP_BACK, "Image précédente", "Recule d'une image (flèche gauche, la souris sur le panneau).", "").clicked() {
            cmds.push(Commande::Pas(-1));
        }
        let joue = v.lecture.joue;
        let (icone, nom) = if joue { (ph::PAUSE, "Pause") } else { (ph::PLAY, "Lecture") };
        if crate::bulles::icone_si(ui, pret, icone, nom, "Lire la vidéo, avec son son (ou la mettre en pause). Espace, la souris sur le panneau.", raccourci).clicked() {
            cmds.push(Commande::JouerPause);
        }
        if crate::bulles::icone_si(ui, pret, ph::SKIP_FORWARD, "Image suivante", "Avance d'une image (flèche droite, la souris sur le panneau).", "").clicked() {
            cmds.push(Commande::Pas(1));
        }
        ui.label(RichText::new(format!("{} / {}", duree_texte(t), if duree > 0.0 { duree_texte(duree) } else { "—".into() })).monospace().small().color(theme::texte_doux()));
        reglages_lecture(ui, r, cmds);
    });
    ui.horizontal(|ui| {
        let mut info = String::new();
        if let Some(i) = &v.infos {
            info = format!("{} · {:.2} i/s", dims_texte((i.largeur, i.hauteur)), i.fps).replace('.', ",");
            if !i.son {
                info.push_str(" · sans son");
            }
        }
        if let Some(s) = taille_fichier {
            info.push_str(&format!(" · {s}"));
        }
        if let Some(e) = &v.son_erreur {
            if v.infos.as_ref().is_some_and(|i| i.son) {
                info.push_str(&format!(" · son indisponible ({e})"));
            }
        }
        ui.label(RichText::new(info).small().weak());
    });
}
