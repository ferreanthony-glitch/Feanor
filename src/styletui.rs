//! Le banc du stylet : la fenêtre « Ce que le stylet transmet » (Aide ; Anthony, 2026-11-06, « le stylet en entier »).
//!
//! Elle montre, côte à côte, ce que **Windows dit du stylet** (lu par l'écoute de `stylet.rs` : contact, appui,
//! inclinaison, rotation, bouton du fût, stylet retourné) et ce que **FEANOR reçoit aujourd'hui** par le chemin habituel
//! (winit puis egui : un appui, un bouton principal ou secondaire). L'écart entre les deux est ce qu'il reste à brancher.
//!
//! - Une liste de gestes à faire ; chaque ligne se coche dès que l'écoute l'a vu.
//! - Un cadre où faire les gestes sans rien dessiner dans le projet.
//! - « Enregistrer le relevé » : le résultat en clair, dans `%LOCALAPPDATA%\SHOT\stylet-releve.txt` et dans le
//!   presse-papiers.
//!
//! La fenêtre ne change RIEN au dessin : elle lit. L'écoute n'est posée sur la fenêtre de Windows qu'à sa première
//! ouverture (`App::stylet_basculer`).

use eframe::egui::{self, vec2, Align2, Event, PointerButton, RichText, Sense, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::stylet::{self, Jalon, Recu, Releve};
use crate::theme;

/// La commande qui ouvre et referme le banc.
pub const ACTION: &str = "aide.stylet_transmet";

/// Le fichier du relevé, dans `%LOCALAPPDATA%\SHOT`.
pub const FICHIER: &str = "stylet-releve.txt";

const LARGEUR: f32 = 440.0;
const CADRE: f32 = 120.0;

/// L'état du banc, le même quel que soit l'onglet.
#[derive(Default)]
pub struct Etat {
    pub fenetre: bool,
    /// La fenêtre de FEANOR chez Windows (0 : inconnue, comme dans les essais).
    pub hwnd: isize,
    /// L'écoute est-elle posée ? Rien tant que le banc n'a jamais été ouvert.
    pub ecoute: Option<bool>,
    /// Ce que FEANOR a reçu par le chemin habituel depuis l'ouverture (ou « Recommencer »).
    pub recu: Recu,
    /// La phrase sous les boutons, après « Enregistrer le relevé ».
    pub message: Option<String>,
}

/// Compte ce qu'un événement d'egui apporte : un toucher (stylet ou doigt) et son appui, un bouton du pointeur.
pub fn noter_recu(recu: &mut Recu, e: &Event) {
    match e {
        Event::Touch { force, .. } => {
            recu.touchers += 1;
            if let Some(f) = force {
                recu.appui_max = recu.appui_max.max(f.clamp(0.0, 1.0));
            }
        }
        Event::PointerButton { button, pressed: true, .. } => match button {
            PointerButton::Primary => recu.principal += 1,
            PointerButton::Secondary => recu.secondaire += 1,
            PointerButton::Middle => recu.milieu += 1,
            _ => {}
        },
        _ => {}
    }
}

/// Où le relevé est écrit.
pub fn fichier() -> Option<std::path::PathBuf> {
    crate::donnees::local().map(|d| d.join(FICHIER))
}

/// La pointe, en clair.
fn pointe(l: &stylet::Lecture) -> &'static str {
    match (l.dos(), l.contact, l.portee) {
        (true, true, _) => "gomme posée",
        (true, false, _) => "gomme en l'air",
        (false, true, _) => "pointe posée",
        (false, false, true) => "pointe en l'air",
        (false, false, false) => "hors de portée",
    }
}

impl App {
    /// Ouvre ou referme le banc. La première ouverture pose l'écoute sur la fenêtre.
    pub(crate) fn stylet_basculer(&mut self) {
        self.stylet.fenetre = !self.stylet.fenetre;
        if self.stylet.fenetre && self.stylet.ecoute.is_none() {
            self.stylet.ecoute = Some(stylet::ecouter(self.stylet.hwnd));
        }
    }

    /// Écrit le relevé dans son fichier et le copie dans le presse-papiers.
    pub(crate) fn stylet_enregistrer(&mut self) {
        let quand = crate::remplacer::date_locale(Some(crate::recovery::now_secs()));
        let texte = stylet::lire().texte(&self.stylet.recu, self.stylet.ecoute, &quand);
        self.ctx.copy_text(texte.clone());
        let ecrit = fichier().and_then(|f| {
            if let Some(d) = f.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            std::fs::write(&f, &texte).ok()
        });
        self.stylet.message = Some(if ecrit.is_some() { "Relevé enregistré, et copié dans le presse-papiers.".into() } else { "Relevé copié dans le presse-papiers (le fichier n'a pas pu être écrit).".into() });
    }

    /// Chaque image d'écran, le banc s'il est ouvert.
    pub(crate) fn stylet_fenetre(&mut self, ctx: &egui::Context) {
        if !self.stylet.fenetre {
            return;
        }
        ctx.input(|i| i.events.iter().for_each(|e| noter_recu(&mut self.stylet.recu, e)));
        let r = stylet::lire();
        let recu = self.stylet.recu;
        let ecoute = self.stylet.ecoute;
        let message = self.stylet.message.clone();
        let (mut ouverte, mut recommencer, mut enregistrer) = (true, false, false);
        egui::Window::new("Ce que le stylet transmet").open(&mut ouverte).collapsible(false).resizable(false).default_pos([70.0, 96.0]).show(ctx, |ui| {
            ui.set_width(LARGEUR);
            let doux = theme::texte_doux();
            ui.add(egui::Label::new(RichText::new("FEANOR écoute ici le stylet auprès de Windows, sans rien changer au dessin. Fais chaque geste de la liste dans le cadre : la ligne se coche dès qu'il est vu.").color(doux)).wrap());
            if ecoute == Some(false) {
                ui.add(egui::Label::new(RichText::new("Windows n'a pas laissé FEANOR écouter le stylet sur cette fenêtre : seule la fin (ce que FEANOR reçoit aujourd'hui) est juste.").color(theme::avertissement())).wrap());
            } else if r.messages == 0 {
                ui.add(egui::Label::new(RichText::new("Aucun message de stylet pour l'instant : approche le stylet de la tablette. Si rien ne vient, le pilote de la tablette n'envoie pas le stylet par Windows Ink.").color(doux)).wrap());
            }
            ui.add_space(4.0);
            cadre(ui, &r);
            ui.add_space(2.0);
            let l = r.lecture;
            ui.columns(2, |c| {
                c[0].label(format!("Stylet : {}", pointe(&l)));
                c[0].label(format!("Appui : {:.0} %", l.appui * 100.0));
                c[0].label(format!("Bouton du stylet : {}", if l.bouton { "enfoncé" } else { "relâché" }));
                c[1].label(format!("Inclinaison gauche-droite : {}°", l.penche[0]));
                c[1].label(format!("Inclinaison avant-arrière : {}°", l.penche[1]));
                c[1].label(format!("Rotation : {}°", l.rotation));
            });
            ui.separator();
            ui.label(RichText::new("À essayer").strong().color(theme::texte_fort()));
            for j in Jalon::TOUS {
                let vu = r.vu(j);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if vu { ph::CHECK_CIRCLE } else { ph::CIRCLE }).size(16.0).color(if vu { theme::accent_texte() } else { doux }));
                    let detail = match j {
                        Jalon::Appui if vu => format!(" (jusqu'à {:.0} %)", r.appui_max * 100.0),
                        Jalon::Penche if vu => format!(" (jusqu'à {}°)", r.penche_max),
                        _ => String::new(),
                    };
                    ui.label(RichText::new(format!("{}{detail}", j.consigne())).color(if vu { theme::texte_fort() } else { doux }));
                });
            }
            ui.separator();
            ui.label(RichText::new("Ce que FEANOR reçoit aujourd'hui").strong().color(theme::texte_fort()));
            ui.label(format!("Stylet : {} événements, appui jusqu'à {:.0} %", recu.touchers, recu.appui_max * 100.0));
            ui.label(format!("Bouton principal : {}  ·  clic droit : {}  ·  bouton du milieu : {}", recu.principal, recu.secondaire, recu.milieu));
            ui.label(RichText::new(format!("Clics de souris vus par Windows : gauche {} (dont {} du stylet), droit {} (dont {} du stylet)", r.clics.gauche, r.clics.gauche_du_stylet, r.clics.droit, r.clics.droit_du_stylet)).color(doux));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                enregistrer = ui.button("Enregistrer le relevé").on_hover_text("Écrit ce que le banc a vu dans un fichier de FEANOR et le copie dans le presse-papiers.").clicked();
                recommencer = ui.button("Recommencer").on_hover_text("Décoche tout et remet les compteurs à zéro.").clicked();
            });
            if let Some(m) = &message {
                ui.label(RichText::new(m).color(doux));
            }
        });
        if recommencer {
            stylet::effacer();
            self.stylet.recu = Recu::default();
            self.stylet.message = None;
        }
        if enregistrer {
            self.stylet_enregistrer();
        }
        if !ouverte {
            self.stylet.fenetre = false;
        }
    }
}

/// Le cadre où faire les gestes : rien ne s'y dessine dans le projet. Un rond suit le stylet (plein et plus gros quand
/// on appuie, creux en l'air) ; un trait part du rond du côté où le stylet est couché.
fn cadre(ui: &mut egui::Ui, r: &Releve) {
    let (reponse, painter) = ui.allocate_painter(vec2(LARGEUR, CADRE), Sense::click_and_drag());
    let zone = reponse.rect;
    painter.rect_filled(zone, 6.0, theme::apercu_trait());
    painter.rect_stroke(zone, 6.0, Stroke::new(1.0_f32, theme::bord()), egui::StrokeKind::Inside);
    let l = r.lecture;
    let ici = ui.ctx().input(|i| i.pointer.latest_pos()).filter(|p| zone.contains(*p));
    let Some(centre) = ici.or((l.portee || l.contact).then(|| zone.center())) else {
        painter.text(zone.center(), Align2::CENTER_CENTER, "Fais les gestes ici : rien ne se dessine", egui::FontId::proportional(14.0), theme::texte_doux());
        return;
    };
    let clair = painter.with_clip_rect(zone);
    let teinte = if l.dos() { theme::avertissement() } else { theme::accent_texte() };
    // Le fût du stylet vu de dessus : du côté où il penche, d'autant plus long qu'il est couché.
    let fut = vec2(l.penche[0] as f32, l.penche[1] as f32) / 90.0 * 110.0;
    if fut.length() > 2.0 {
        clair.line_segment([centre, centre + fut], Stroke::new(4.0_f32, teinte.gamma_multiply(0.55)));
    }
    if l.contact {
        clair.circle_filled(centre, 4.0 + 20.0 * l.appui, teinte);
    } else {
        clair.circle_stroke(centre, 7.0, Stroke::new(2.0_f32, teinte));
    }
    if l.bouton {
        clair.circle_stroke(centre, 32.0, Stroke::new(2.0_f32, theme::texte_fort()));
    }
    let legende = match (l.dos(), l.bouton) {
        (true, true) => "gomme, bouton enfoncé",
        (true, false) => "gomme",
        (false, true) => "bouton enfoncé",
        (false, false) => "",
    };
    if !legende.is_empty() {
        painter.text(zone.left_top() + vec2(8.0, 6.0), Align2::LEFT_TOP, legende, egui::FontId::proportional(13.0), theme::texte_fort());
    }
}
