//! Les images d'accueil (Anthony, 2026-10-28 : « à la place du grand logo SHOT, une image de storyboard aléatoire
//! s'affiche », puis : « tu peux ajouter aux paramètres du logiciel, dans l'onglet de personnalisation, la possibilité
//! d'importer ses propres images »).
//!
//! - **Le dossier** : un réglage du poste (pas du projet), `Settings::dossier_accueil` dans `reglages.toml` ; vide :
//!   `<Documents>\SHOT\Identité\Accueil`. Il se change dans Préférences > Apparence > Images d'accueil.
//! - **Le tirage** : une fois par apparition de l'accueil (pas à chaque image). Un fil de travail liste le dossier, tire
//!   une image (pas la même que la fois d'avant s'il y en a plusieurs), la décode, la réduit à la taille de l'écran, et
//!   l'envoie ; l'interface en fait une texture, gardée jusqu'à ce que l'accueil s'en aille. En attendant, la carte
//!   garde la place du logo, vide. Sans dossier, sans image ou si rien ne se lit : le logo, sans aucune erreur.
//! - **Les miniatures** des Préférences : décodées une fois, sur un fil, et gardées (une texture par image).
//! - **Importer** copie les images choisies dans le dossier (créé au besoin) ; un nom pris reçoit « _2 », « _3 »…
//!   **Retirer** supprime l'image du dossier après une question (SHOT n'a pas de fonction pour la Corbeille) et ne touche
//!   jamais à autre chose qu'un fichier png ou jpeg directement dans ce dossier.
//!
//! L'état d'exécution (tirage, miniatures) est dans `Confort::accueil` : partagé par tous les onglets de projets.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use eframe::egui::{self, vec2, RichText};

use crate::app::App;
use crate::theme;

/// Les formats lus.
pub(crate) const EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

/// Plus grande largeur gardée pour l'image de l'accueil, en pixels (le bandeau fait 1000 points de large : 1600 pixels le
/// remplissent à 150 % sans le flou d'une image agrandie).
const LARGEUR_MAX: u32 = 1600;

/// Plus grand côté d'une miniature des Préférences, en pixels.
const MINIATURE_MAX: u32 = 160;

/// Combien d'images le tirage essaie au plus avant de se rabattre sur le logo.
const ESSAIS: usize = 5;

/// Un fichier plus gros que cela n'est pas décodé (rien de ce poids n'est une image de storyboard).
const POIDS_MAX: u64 = 120 * 1024 * 1024;

/// Combien de temps la liste du dossier reste valable dans les Préférences avant d'être relue.
const LISTE_VALIDE: Duration = Duration::from_secs(3);

/// Des pixels décodés, prêts à devenir une texture.
pub(crate) struct Pixels {
    pub taille: [usize; 2],
    pub rgba: Vec<u8>,
}

impl Pixels {
    fn image(&self) -> egui::ColorImage {
        egui::ColorImage::from_rgba_unmultiplied(self.taille, &self.rgba)
    }
}

/// Le dossier d'origine : `<Documents>\SHOT\Identité\Accueil`. (Les essais ne regardent jamais le vrai dossier :
/// celui de leur bac à sable.)
pub(crate) fn dossier_par_defaut() -> PathBuf {
    if cfg!(test) {
        crate::donnees::local().unwrap_or_else(std::env::temp_dir).join("Accueil")
    } else {
        crate::nouveau::default_folder().join("SHOT").join("Identité").join("Accueil")
    }
}

/// Ce fichier est-il une image lue (png, jpg, jpeg, majuscules comprises) ?
pub(crate) fn est_image(chemin: &Path) -> bool {
    chemin.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Les images du dossier (pas des sous-dossiers), rangées par nom. Un dossier absent ou illisible : rien.
pub(crate) fn lister(dossier: &Path) -> Vec<PathBuf> {
    let Ok(entrees) = std::fs::read_dir(dossier) else { return Vec::new() };
    let mut images: Vec<PathBuf> = entrees.filter_map(Result::ok).map(|e| e.path()).filter(|p| est_image(p) && p.is_file()).collect();
    images.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    images
}

/// Où poser un fichier de ce nom dans `dossier` : le nom lui-même s'il est libre, sinon « nom_2.png », « nom_3.png »…
pub(crate) fn nom_libre(dossier: &Path, nom: &std::ffi::OsStr) -> PathBuf {
    let direct = dossier.join(nom);
    if !direct.exists() {
        return direct;
    }
    let chemin = Path::new(nom);
    let base = chemin.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let extension = chemin.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..)
        .map(|n| dossier.join(format!("{base}_{n}{extension}")))
        .find(|p| !p.exists())
        .expect("un nom libre existe toujours")
}

/// Ce que l'import a fait.
#[derive(Default, Debug)]
pub(crate) struct Importe {
    /// Les copies faites, dans le dossier.
    pub copies: Vec<PathBuf>,
    /// Les fichiers laissés de côté : pas une image png ou jpeg, ou déjà dans le dossier.
    pub ignores: usize,
    /// Les copies qui ont échoué, avec la raison.
    pub erreurs: Vec<String>,
}

/// Copie `sources` dans `dossier` (créé au besoin). Les sources ne sont jamais déplacées ni modifiées.
pub(crate) fn importer(dossier: &Path, sources: &[PathBuf]) -> Importe {
    let mut bilan = Importe::default();
    let candidates: Vec<&PathBuf> = sources
        .iter()
        .filter(|s| {
            let ok = est_image(s) && s.is_file() && s.parent() != Some(dossier);
            if !ok {
                bilan.ignores += 1;
            }
            ok
        })
        .collect();
    if candidates.is_empty() {
        return bilan;
    }
    if let Err(e) = std::fs::create_dir_all(dossier) {
        bilan.erreurs.push(format!("{} : {e}", dossier.display()));
        return bilan;
    }
    for source in candidates {
        let Some(nom) = source.file_name() else { continue };
        let cible = nom_libre(dossier, nom);
        match std::fs::copy(source, &cible) {
            Ok(_) => bilan.copies.push(cible),
            Err(e) => bilan.erreurs.push(format!("{} : {e}", source.display())),
        }
    }
    bilan
}

/// Supprime une image du dossier. Refuse tout ce qui n'est pas un fichier png ou jpeg posé directement dans `dossier`.
pub(crate) fn retirer(dossier: &Path, image: &Path) -> Result<(), String> {
    if image.parent() != Some(dossier) || !est_image(image) || !image.is_file() {
        return Err(format!("« {} » n'est pas une image de ce dossier.", image.display()));
    }
    std::fs::remove_file(image).map_err(|e| format!("{} : {e}", image.display()))
}

/// Un nombre au hasard (la graine de chaque séance de l'OS, mêlée à l'heure).
fn au_hasard() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    h.finish()
}

/// Tire une image au hasard, pas `derniere` s'il y en a une autre. `hasard` : le nombre tiré.
pub(crate) fn tirer<'a>(images: &'a [PathBuf], derniere: Option<&Path>, hasard: u64) -> Option<&'a PathBuf> {
    let autres: Vec<&PathBuf> = images.iter().filter(|p| Some(p.as_path()) != derniere).collect();
    let choix = if autres.is_empty() { images.iter().collect() } else { autres };
    (!choix.is_empty()).then(|| choix[(hasard % choix.len() as u64) as usize])
}

/// Décode une image et la réduit pour qu'elle tienne dans `max` pixels. Rien si elle ne se lit pas.
pub(crate) fn decoder(chemin: &Path, max: u32) -> Option<Pixels> {
    if std::fs::metadata(chemin).ok()?.len() > POIDS_MAX {
        return None;
    }
    let mut image = image::open(chemin).ok()?;
    if image.width() == 0 || image.height() == 0 {
        return None;
    }
    if image.width() > max || image.height() > max {
        image = image.thumbnail(max, max);
    }
    let rgba = image.to_rgba8();
    Some(Pixels { taille: [rgba.width() as usize, rgba.height() as usize], rgba: rgba.into_raw() })
}

/// Le travail du fil de tirage : lister, tirer, décoder (quelques essais si une image ne se lit pas).
fn tirer_et_decoder(dossier: &Path, derniere: Option<&Path>) -> Option<(PathBuf, Pixels)> {
    let mut images = lister(dossier);
    let mut hasard = au_hasard();
    let mut precedente = derniere.map(Path::to_path_buf);
    for _ in 0..ESSAIS {
        let chemin = tirer(&images, precedente.as_deref(), hasard)?.clone();
        if let Some(pixels) = decoder(&chemin, LARGEUR_MAX) {
            return Some((chemin, pixels));
        }
        images.retain(|p| *p != chemin);
        precedente = None;
        hasard = hasard.rotate_left(17) ^ au_hasard();
    }
    None
}

/// Où en est le tirage de l'accueil.
#[derive(Default)]
pub(crate) enum Tirage {
    /// Pas encore tiré pour cette apparition.
    #[default]
    AFaire,
    /// Le fil travaille : la carte garde la place du logo.
    Attente(Receiver<Option<(PathBuf, Pixels)>>),
    /// L'image est prête, gardée en texture.
    Image(egui::TextureHandle, [usize; 2]),
    /// Rien à montrer : le logo.
    Logo,
}

/// Ce que la carte d'accueil dessine à la place du logo.
pub(crate) enum Visuel {
    Attente,
    Image(egui::TextureHandle, [usize; 2]),
    Logo,
}

/// L'état d'exécution des images d'accueil (jamais dans le projet) : voir `Confort::accueil`.
#[derive(Default)]
pub(crate) struct AccueilImages {
    pub tirage: Tirage,
    /// Combien de tirages ont été lancés (les essais vérifient qu'il n'y en a pas un par image).
    pub tirages: u32,
    /// L'image montrée la dernière fois : le prochain tirage en prend une autre.
    pub derniere: Option<PathBuf>,
    // ---- Préférences
    /// Les images du dossier, et quand elles ont été lues.
    pub liste: Vec<PathBuf>,
    pub liste_dossier: Option<PathBuf>,
    pub liste_lue: Option<Instant>,
    /// Les miniatures : une texture par image (`None` : elle ne se lit pas).
    pub miniatures: HashMap<PathBuf, Option<egui::TextureHandle>>,
    miniatures_demandees: HashSet<PathBuf>,
    miniatures_canal: Option<(Sender<(PathBuf, Option<Pixels>)>, Receiver<(PathBuf, Option<Pixels>)>)>,
    /// Combien de miniatures ont été décodées (les essais vérifient qu'on ne décode pas à chaque image).
    pub miniatures_decodees: u32,
    /// L'image dont « Retirer » attend la confirmation.
    pub retrait: Option<PathBuf>,
    /// Le dernier compte rendu (import, retrait), montré sous les boutons.
    pub message: Option<String>,
}

impl AccueilImages {
    /// Oublie le tirage : le prochain passage sur l'accueil en refait un.
    pub fn oublier_le_tirage(&mut self) {
        if !matches!(self.tirage, Tirage::AFaire) {
            self.tirage = Tirage::AFaire;
        }
    }

    /// La liste du dossier est à relire.
    pub fn oublier_la_liste(&mut self) {
        self.liste_lue = None;
    }
}

impl App {
    /// Le dossier des images d'accueil : le réglage, ou celui d'origine.
    pub(crate) fn dossier_accueil(&self) -> PathBuf {
        let choisi = self.settings.dossier_accueil.trim();
        if choisi.is_empty() {
            dossier_par_defaut()
        } else {
            PathBuf::from(choisi)
        }
    }

    /// Choisit le dossier des images d'accueil (`None` : celui d'origine) et le retient.
    pub(crate) fn definir_dossier_accueil(&mut self, dossier: Option<PathBuf>) {
        self.settings.dossier_accueil = dossier.map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
        let _ = self.settings.save();
        let a = &mut self.confort.accueil;
        a.oublier_le_tirage();
        a.oublier_la_liste();
        a.retrait = None;
        a.message = None;
    }

    /// Le tirage de l'accueil : lancé une fois par apparition (sur un fil), puis sa texture est gardée. Appelé à chaque image
    /// de l'accueil, il ne fait que regarder si le fil a fini.
    pub(crate) fn accueil_visuel(&mut self, ctx: &egui::Context) -> Visuel {
        if matches!(self.confort.accueil.tirage, Tirage::AFaire) {
            let dossier = self.dossier_accueil();
            let derniere = self.confort.accueil.derniere.clone();
            let (envoi, reception) = channel();
            let ctx2 = ctx.clone();
            std::thread::spawn(move || {
                let _ = envoi.send(tirer_et_decoder(&dossier, derniere.as_deref()));
                ctx2.request_repaint();
            });
            self.confort.accueil.tirages += 1;
            self.confort.accueil.tirage = Tirage::Attente(reception);
        }
        if let Tirage::Attente(reception) = &self.confort.accueil.tirage {
            let fini = match reception.try_recv() {
                Ok(r) => Some(r),
                Err(TryRecvError::Disconnected) => Some(None),
                Err(TryRecvError::Empty) => None,
            };
            if let Some(resultat) = fini {
                self.confort.accueil.tirage = match resultat {
                    Some((chemin, pixels)) => {
                        let texture = ctx.load_texture("shot-accueil-image", pixels.image(), egui::TextureOptions::LINEAR);
                        self.confort.accueil.derniere = Some(chemin);
                        Tirage::Image(texture, pixels.taille)
                    }
                    None => Tirage::Logo,
                };
            }
        }
        match &self.confort.accueil.tirage {
            Tirage::Image(t, taille) => Visuel::Image(t.clone(), *taille),
            Tirage::Logo => Visuel::Logo,
            _ => Visuel::Attente,
        }
    }

    /// « Importer des images… » : copie ces fichiers dans le dossier et le dit.
    pub(crate) fn importer_images_accueil(&mut self, sources: &[PathBuf]) {
        let dossier = self.dossier_accueil();
        let bilan = importer(&dossier, sources);
        let mut mots = Vec::new();
        match bilan.copies.len() {
            0 => {}
            1 => mots.push("1 image copiée".to_string()),
            n => mots.push(format!("{n} images copiées")),
        }
        if bilan.ignores > 0 {
            mots.push(format!("{} laissée(s) de côté (pas du png ou du jpeg, ou déjà dans le dossier)", bilan.ignores));
        }
        if let Some(e) = bilan.erreurs.first() {
            mots.push(format!("échec : {e}"));
        }
        let a = &mut self.confort.accueil;
        a.message = Some(if mots.is_empty() { "Rien à copier.".to_string() } else { mots.join(", ") + "." });
        a.oublier_la_liste();
        if !bilan.copies.is_empty() {
            a.oublier_le_tirage();
        }
    }

    /// Retire une image du dossier (déjà confirmé par l'utilisateur).
    pub(crate) fn retirer_image_accueil(&mut self, image: &Path) {
        let dossier = self.dossier_accueil();
        let a = &mut self.confort.accueil;
        a.retrait = None;
        match retirer(&dossier, image) {
            Ok(()) => {
                a.message = Some(format!("« {} » retirée du dossier.", image.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())));
                a.oublier_la_liste();
                a.oublier_le_tirage();
            }
            Err(e) => a.message = Some(e),
        }
    }

    /// Les images du dossier pour les Préférences : relues de temps en temps seulement, jamais à chaque image ; les
    /// miniatures arrivent d'un fil de travail.
    pub(crate) fn rafraichir_images_accueil(&mut self, ctx: &egui::Context) {
        let dossier = self.dossier_accueil();
        let a = &mut self.confort.accueil;
        let perimee = a.liste_dossier.as_ref() != Some(&dossier) || a.liste_lue.is_none_or(|t| t.elapsed() > LISTE_VALIDE);
        if perimee {
            a.liste = lister(&dossier);
            a.liste_dossier = Some(dossier);
            a.liste_lue = Some(Instant::now());
            let presentes: HashSet<&PathBuf> = a.liste.iter().collect();
            a.miniatures.retain(|p, _| presentes.contains(p));
            a.miniatures_demandees.retain(|p| presentes.contains(p));
        }
        let (envoi, reception) = a.miniatures_canal.get_or_insert_with(channel);
        while let Ok((chemin, pixels)) = reception.try_recv() {
            if !a.liste.contains(&chemin) {
                continue;
            }
            let texture = pixels.map(|p| ctx.load_texture(format!("shot-accueil-miniature-{}", chemin.display()), p.image(), egui::TextureOptions::LINEAR));
            a.miniatures_decodees += 1;
            a.miniatures.insert(chemin, texture);
        }
        let manquantes: Vec<PathBuf> = a.liste.iter().filter(|p| !a.miniatures.contains_key(*p) && !a.miniatures_demandees.contains(*p)).cloned().collect();
        if !manquantes.is_empty() {
            a.miniatures_demandees.extend(manquantes.iter().cloned());
            let envoi = envoi.clone();
            let ctx2 = ctx.clone();
            std::thread::spawn(move || {
                for chemin in manquantes {
                    let pixels = decoder(&chemin, MINIATURE_MAX);
                    if envoi.send((chemin, pixels)).is_err() {
                        break;
                    }
                    ctx2.request_repaint();
                }
            });
        }
    }

    /// Préférences > Apparence > Images d'accueil : le dossier, l'import, la liste des images avec « Retirer ».
    pub(crate) fn images_accueil_prefs_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.rafraichir_images_accueil(&ctx);
        let dossier = self.dossier_accueil();
        let par_defaut = self.settings.dossier_accueil.trim().is_empty();
        ui.label(RichText::new("Une de ces images, tirée au hasard, s'affiche sur l'écran d'accueil. Sans image, l'accueil montre le logo.").small().weak());
        let mut choisir = false;
        let mut remettre = false;
        let mut importer_des = false;
        ui.horizontal(|ui| {
            ui.label("Dossier des images d'accueil");
            let chemin = ui.add(egui::Label::new(RichText::new(dossier.display().to_string()).color(theme::texte_doux())).truncate());
            chemin.on_hover_text(dossier.display().to_string());
        });
        ui.horizontal(|ui| {
            let b = ui.button("Changer…");
            if crate::bulles::bulle(b, "Changer le dossier", "Choisis le dossier où SHOT prend les images de l'accueil.", "").clicked() {
                choisir = true;
            }
            let b = ui.add_enabled(!par_defaut, egui::Button::new("Dossier d'origine"));
            if crate::bulles::bulle(b, "Dossier d'origine", "Reprendre le dossier de départ : Documents\\SHOT\\Identité\\Accueil.", "").clicked() {
                remettre = true;
            }
            let b = ui.button("Importer des images…");
            if crate::bulles::bulle(b, "Importer des images", "Choisis des images png ou jpeg : SHOT les copie dans ce dossier (le dossier est créé s'il manque). Tes fichiers d'origine ne bougent pas.", "").clicked() {
                importer_des = true;
            }
        });
        if let Some(m) = &self.confort.accueil.message {
            ui.label(RichText::new(m.as_str()).small().color(theme::accent_texte()));
        }
        let images = self.confort.accueil.liste.clone();
        ui.add_space(4.0);
        ui.label(RichText::new(if images.is_empty() { "Images du dossier : aucune".to_string() } else { format!("Images du dossier : {}", images.len()) }).small().weak());
        let mut retirer_celle: Option<PathBuf> = None;
        for image in &images {
            let nom = image.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(96.0, 54.0), egui::Sense::hover());
                match self.confort.accueil.miniatures.get(image) {
                    Some(Some(t)) => {
                        let [w, h] = t.size();
                        let echelle = (rect.width() / w as f32).min(rect.height() / h as f32);
                        let zone = egui::Rect::from_center_size(rect.center(), vec2(w as f32, h as f32) * echelle);
                        egui::Image::new(egui::load::SizedTexture::new(t.id(), zone.size())).corner_radius(3.0).paint_at(ui, zone);
                    }
                    Some(None) => {
                        ui.painter().rect_filled(rect, 3.0, theme::controle());
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "illisible", egui::FontId::proportional(11.0), theme::texte_doux());
                    }
                    None => {
                        ui.painter().rect_filled(rect, 3.0, theme::controle());
                    }
                }
                ui.add(egui::Label::new(nom.as_str()).truncate()).on_hover_text(image.display().to_string());
                if self.confort.accueil.retrait.as_ref() == Some(image) {
                    ui.label(RichText::new("Supprimer ce fichier du dossier ?").small());
                    if ui.small_button("Supprimer").on_hover_text("Efface l'image du dossier. Elle ne va pas dans la Corbeille.").clicked() {
                        retirer_celle = Some(image.clone());
                    }
                    if ui.small_button("Garder").clicked() {
                        self.confort.accueil.retrait = None;
                    }
                } else {
                    let b = ui.small_button("Retirer");
                    if crate::bulles::bulle(b, "Retirer", "Supprime cette image du dossier d'accueil, après une question. Elle ne va pas dans la Corbeille.", "").clicked() {
                        self.confort.accueil.retrait = Some(image.clone());
                    }
                }
            });
        }
        if choisir {
            if let Some(d) = Self::pick_folder("Dossier des images d'accueil") {
                self.definir_dossier_accueil(Some(d));
            }
        }
        if remettre {
            self.definir_dossier_accueil(None);
        }
        if importer_des {
            if let Some(fichiers) = rfd::FileDialog::new().set_title("Images pour l'accueil").add_filter("Images (png, jpeg)", &EXTENSIONS).pick_files() {
                self.importer_images_accueil(&fichiers);
            }
        }
        if let Some(i) = retirer_celle {
            self.retirer_image_accueil(&i);
        }
    }
}
