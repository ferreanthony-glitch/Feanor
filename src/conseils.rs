//! Les conseils d'un seul passage (passe « simplicité », 2026-10-21) : une petite bulle, discrète, la première fois
//! qu'on ouvre un endroit de SHOT qui mérite une phrase d'explication. Une phrase, un bouton « Compris », et la bulle ne
//! revient plus : ce qui a été vu est retenu dans les réglages de l'application (`reglages.toml`, clé `conseils`).
//! Préférences > Général > Conseils : « Revoir les conseils » les oublie tous.
//!
//! Cinq conseils au plus, un par endroit :
//! - l'onglet Vignettes, l'onglet Montage, l'onglet Animation (la première fois qu'on y entre) ;
//! - l'Espace 3D (la première fois qu'on y entre) ;
//! - le mode concentration et l'aide-mémoire des raccourcis (la première fois qu'on a modifié un projet dans l'onglet
//!   Storyboard : c'est le moment où l'on a les mains sur le dessin).
//!
//! Jamais deux bulles à la fois : un conseil attend que l'écran d'accueil soit parti, que la proposition « Régler SHOT
//! pour cette machine ? » ait eu sa réponse et qu'aucune fenêtre ne tienne la main. Il se montre une fois, au moment
//! où il apparaît il est noté comme vu (même si l'on passe à autre chose sans cliquer : « Revoir les conseils » le rend).
//! Il quitte l'écran avec son endroit.

use eframe::egui::{self, Align2, RichText};
use egui_phosphor::regular as ph;
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::theme;
use crate::workspace::Espace;

/// Un conseil, c'est-à-dire un endroit de SHOT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conseil {
    Vignettes,
    Montage,
    Animation,
    Espace3d,
    Raccourcis,
}

impl Conseil {
    /// Dans l'ordre où on les rencontre.
    pub const TOUS: [Conseil; 5] = [Conseil::Vignettes, Conseil::Montage, Conseil::Animation, Conseil::Espace3d, Conseil::Raccourcis];

    /// Identifiant stable, écrit dans les réglages.
    pub fn id(self) -> &'static str {
        match self {
            Conseil::Vignettes => "vignettes",
            Conseil::Montage => "montage",
            Conseil::Animation => "animation",
            Conseil::Espace3d => "espace3d",
            Conseil::Raccourcis => "raccourcis",
        }
    }

    /// Le mot en gras de la bulle.
    pub fn titre(self) -> &'static str {
        match self {
            Conseil::Vignettes => "Les Vignettes",
            Conseil::Montage => "Le Montage",
            Conseil::Animation => "L'Animation",
            Conseil::Espace3d => "L'Espace 3D",
            Conseil::Raccourcis => "Concentration et raccourcis",
        }
    }
}

/// Ce qui a été vu : les identifiants des conseils déjà montrés.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReglageConseils {
    #[serde(default)]
    pub vus: Vec<String>,
}

impl ReglageConseils {
    /// Rien de vu : la clé n'est pas écrite dans le fichier de réglages.
    pub fn est_vide(&self) -> bool {
        self.vus.is_empty()
    }

    pub fn a_vu(&self, c: Conseil) -> bool {
        self.vus.iter().any(|v| v == c.id())
    }

    /// Note ce conseil comme vu. Vrai s'il ne l'était pas.
    pub fn marquer(&mut self, c: Conseil) -> bool {
        if self.a_vu(c) {
            return false;
        }
        self.vus.push(c.id().to_string());
        true
    }

    /// « Revoir les conseils » : tout est à revoir.
    pub fn tout_oublier(&mut self) {
        self.vus.clear();
    }

    /// Combien des conseils d'aujourd'hui sont vus (les identifiants inconnus d'un autre SHOT ne comptent pas).
    pub fn nombre_vus(&self) -> usize {
        Conseil::TOUS.iter().filter(|c| self.a_vu(**c)).count()
    }
}

/// Le conseil à faire apparaître : celui de l'endroit où l'on est, s'il n'a jamais été vu et si l'écran est libre.
pub fn a_montrer(endroit: Option<Conseil>, vus: &ReglageConseils, libre: bool) -> Option<Conseil> {
    endroit.filter(|c| libre && !vus.a_vu(*c))
}

impl App {
    /// L'endroit où l'on est, pour les conseils.
    pub(crate) fn conseil_de_l_endroit(&self) -> Option<Conseil> {
        if self.espace3d.on && !self.camera_view {
            return Some(Conseil::Espace3d);
        }
        match self.espace {
            Espace::Vignettes => Some(Conseil::Vignettes),
            Espace::Montage => Some(Conseil::Montage),
            Espace::Animation => Some(Conseil::Animation),
            Espace::Storyboard => self.modified.then_some(Conseil::Raccourcis),
        }
    }

    /// Les touches de deux commandes, écrites comme le fichier de raccourcis les donne aujourd'hui (celles de l'utilisateur
    /// si elles ont changé) ; vide si la commande n'a pas de touche.
    fn touche_ou_rien(&self, action: &str) -> String {
        self.keymap.label(action)
    }

    /// La phrase du conseil. Les touches sont lues dans le fichier de raccourcis : une touche changée y est changée aussi.
    pub(crate) fn conseil_texte(&self, c: Conseil) -> String {
        match c {
            Conseil::Vignettes => "Dessine directement dans les Cases, ou dépose des images sur la planche : elles prennent place dans les Cases.".into(),
            Conseil::Montage => "Le film en haut, toutes les Pistes en bas : tire le bord d'un Plan pour changer sa durée.".into(),
            Conseil::Animation => {
                let pelure = self.touche_ou_rien("vue.pelure");
                if pelure.is_empty() {
                    "Ici tu animes le Plan image par image : la pelure d'oignon montre les images voisines.".into()
                } else {
                    format!("Ici tu animes le Plan image par image : la pelure d'oignon ({pelure}) montre les images voisines.")
                }
            }
            Conseil::Espace3d => "Chaque calque est une carte dans l'espace : clique-la et glisse-la ; clic droit pour tourner la vue, molette pour avancer.".into(),
            Conseil::Raccourcis => {
                let (concentration, aide) = (self.touche_ou_rien("vue.concentration"), self.touche_ou_rien("aide.aide_memoire"));
                match (concentration.is_empty(), aide.is_empty()) {
                    (false, false) => format!("{concentration} masque tous les panneaux pour ne garder que le dessin ; maintiens {aide} pour voir les raccourcis."),
                    (false, true) => format!("{concentration} masque tous les panneaux pour ne garder que le dessin."),
                    (true, false) => format!("Maintiens {aide} pour voir les raccourcis."),
                    (true, true) => "Le menu Affichage masque tous les panneaux pour ne garder que le dessin.".into(),
                }
            }
        }
    }

    /// Chaque image d'écran : le conseil de l'endroit, une fois. Rien de lourd (un test d'égalité par image).
    pub(crate) fn conseils_ui(&mut self, ctx: &egui::Context) {
        if !self.confort.conseils_actifs {
            return;
        }
        let endroit = self.conseil_de_l_endroit();
        // Le conseil à l'écran s'en va avec son endroit.
        if self.confort.conseil.is_some() && self.confort.conseil != endroit {
            self.confort.conseil = None;
        }
        // Un seul à la fois, et seulement quand rien d'autre ne demande l'attention.
        // Au compte-gouttes (premier contact, 2026-10-30) : la visite « Premier jour » passe d'abord, et une séance ne montre qu'un seul
        // conseil de plus (« Revoir les conseils » les rend tous, un par séance).
        let place = self.confort.conseils_vus_seance < self.confort.conseils_max_seance && !self.visite_occupe();
        let libre = place && self.confort.conseil.is_none() && !self.welcome_shown() && !self.diag.offre && !self.modal_open() && !self.fenetre_ouverte();
        if let Some(c) = a_montrer(endroit, &self.settings.conseils, libre) {
            self.confort.conseil = Some(c);
            self.confort.conseils_vus_seance += 1;
            if self.settings.conseils.marquer(c) {
                let _ = self.settings.save();
            }
        }
        let Some(c) = self.confort.conseil else { return };
        if self.welcome_shown() || self.diag.offre || self.modal_open() || self.fenetre_ouverte() {
            return;
        }
        let texte = self.conseil_texte(c);
        let mut compris = false;
        egui::Area::new(egui::Id::new("conseil_unique")).order(egui::Order::Foreground).anchor(Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -120.0)).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(theme::eleve()).stroke(egui::Stroke::new(1.0_f32, theme::accent_texte())).show(ui, |ui| {
                ui.set_max_width(340.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(ph::LIGHTBULB).size(16.0).color(theme::accent_texte()));
                    ui.label(RichText::new(c.titre()).strong().color(theme::texte_fort()));
                });
                ui.add(egui::Label::new(RichText::new(texte).color(theme::texte_doux())).wrap());
                ui.add_space(4.0);
                if crate::bulles::bulle(ui.button(RichText::new("Compris").strong()), "Compris", "Ferme ce conseil. Préférences > Général > Revoir les conseils les rend.", "").clicked() {
                    compris = true;
                }
            });
        });
        if compris {
            self.confort.conseil = None;
        }
    }

    /// Préférences > Général > Conseils.
    pub(crate) fn conseils_prefs_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let b = ui.button("Revoir les conseils");
            if crate::bulles::bulle(b, "Revoir les conseils", "Les petites bulles qui expliquent un endroit de FEANOR la première fois reviennent, une par endroit et une par séance.", &self.raccourci("aide.conseils")).clicked() {
                self.revoir_les_conseils();
            }
            ui.label(RichText::new(format!("{} conseil(s) vu(s) sur {}.", self.settings.conseils.nombre_vus(), Conseil::TOUS.len())).small().weak());
        });
    }

    /// « Revoir les conseils » : ils sont tous à revoir, la bulle d'ici s'en va.
    pub(crate) fn revoir_les_conseils(&mut self) {
        self.settings.conseils.tout_oublier();
        self.confort.conseil = None;
        // (Demandé à la main : le conseil de l'endroit où l'on est revient tout de suite, comme avant.)
        self.confort.conseils_vus_seance = 0;
        let _ = self.settings.save();
        self.status = "Les conseils reviendront, un par endroit, la prochaine fois que tu y passes.".into();
    }
}
