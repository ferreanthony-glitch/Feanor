//! Les versions d'un plan dans la timeline (partie 168, `versiondepliee.rs` pour la carte film ↔ écran).
//!
//! Sur le bloc d'un plan qui a plusieurs versions, à côté de son nom « Plan 1 · V1 », les **autres versions** en étiquettes texte
//! (« V2 », « V3 »…), chacune suivie d'une flèche :
//!
//! - **un clic sur « V2 »** : la V2 devient la version en cours (le « Passer à la version » de toujours : une étape d'annulation, le
//!   message de la barre d'état d'avant) ;
//! - **un clic sur la flèche ▸** : la V2 se **déplie** juste après la V1, en bloc à part (bordé, hachuré, marqué « V2 dépliée »), les
//!   plans suivants glissent à l'écran ; la flèche devient ◂ et la replie. La lecture du film traverse la V1, puis la V2, puis la
//!   suite. Rien n'est écrit dans le projet, rien ne s'annule, le montage et les exports ne changent pas ;
//! - **un clic droit** sur une étiquette : une petite palette (dix couleurs de repère et « Aucune ») qui colore l'étiquette, une
//!   étape d'annulation par couleur ; la couleur s'enregistre avec la version (`planvers.rs`) et se voit aussi sur « V1 » dans le
//!   nom du plan quand elle est la version en cours, et sur le bloc déplié.
//!
//! Faute de place (plan court), les étiquettes qui ne tiennent pas se résument en « +2 » : un clic ouvre la pile des versions
//! (`pileversui.rs`), un clic droit le menu de chacune. Jamais deux étiquettes l'une sur l'autre.
//!
//! La version dépliée est en **lecture seule** : un clic dedans pose seulement la tête de lecture ; la vue montre alors son image
//! (composée par la file des vignettes, `thumbs.rs`, jamais sur le fil de l'interface), avec « lecture seule » écrit dessus. Le
//! son pendant une version dépliée : celui du film, repris au début du plan (on l'entend sous la V2 comme sous la V1).

use std::sync::Arc;
use std::time::Instant;

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Galley, Painter, Pos2, Rect, Sense, Stroke, StrokeKind};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::planvers::etiquette;
use crate::theme;
use crate::versiondepliee::{Carte, Lieu};

/// Les couleurs de repère des étiquettes : dix teintes douces (moins saturées et un peu plus claires que les couleurs de plan, à
/// la demande d'Anthony : « plus douces »), lisibles avec le texte de l'étiquette sur le fond sombre et distinctes entre elles. Un
/// projet enregistre le **rang** de la teinte (0 à 9), pas sa valeur : changer une teinte ici change toutes les étiquettes qui l'ont.
pub(crate) const PALETTE: [(&str, [u8; 3]); 10] = [
    ("Bleu doux", [122, 152, 208]),
    ("Vert doux", [128, 182, 132]),
    ("Cyan doux", [118, 182, 192]),
    ("Rouge doux", [208, 124, 118]),
    ("Rose doux", [200, 130, 182]),
    ("Orange doux", [216, 158, 108]),
    ("Violet doux", [158, 134, 204]),
    ("Brun doux", [172, 138, 104]),
    ("Jaune doux", [200, 196, 108]),
    ("Gris bleu", [142, 156, 172]),
];

/// La teinte de rang `k` (une valeur hors de la palette, venue d'un fichier abîmé : un gris).
pub(crate) fn couleur(k: u8) -> Color32 {
    let [r, g, b] = PALETTE.get(k as usize).map_or([150, 150, 150], |c| c.1);
    Color32::from_rgb(r, g, b)
}

/// Hauteur d'une étiquette de version.
const ETIQ_H: f32 = 15.0;
/// Largeur de la flèche après une étiquette.
const FLECHE_W: f32 = 12.0;
/// Écart entre deux étiquettes.
const ECART: f32 = 5.0;

/// Ce qu'on peut viser sur le bloc d'un plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sorte {
    /// Le nom d'une version (« V2 ») : un clic y passe.
    Nom,
    /// La flèche ▸ ou ◂ : un clic déplie ou replie.
    Fleche,
    /// « +2 » : les versions qui n'ont pas trouvé de place.
    Plus,
}

/// Une étiquette posée sur la bande à cette image d'écran.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ZoneVersion {
    pub zone: Rect,
    pub plan: u64,
    pub index: usize,
    pub numero: u32,
    pub sorte: Sorte,
}

/// La tête de lecture posée dans une version dépliée : l'image `frame` de la version `numero` du plan `plan`. `garde` : le plan
/// courant et l'image courante quand elle y a été posée (le plan, à sa dernière image) ; dès qu'ils changent (une flèche, un clic
/// ailleurs, un Ctrl+Z…), la tête quitte la version dépliée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Tete {
    pub plan: u64,
    pub numero: u32,
    pub frame: usize,
    garde: (usize, usize),
}

/// L'état d'affichage des versions dépliées : jamais enregistré, ni dans le projet ni dans l'historique.
#[derive(Default)]
pub(crate) struct Depli {
    /// Les versions dépliées : (plan, numéro).
    pub liste: Vec<(u64, u32)>,
    pub tete: Option<Tete>,
    /// Les étiquettes et les colonnes des versions dépliées posées à la dernière image d'écran (gestes, essais).
    pub zones: Vec<ZoneVersion>,
    pub colonnes: Vec<(Rect, usize)>,
}

/// Le texte lisible sur la couleur `c`.
fn encre_sur(c: Color32) -> Color32 {
    let (sombre, clair) = (Color32::from_gray(24), Color32::WHITE);
    if theme::contraste(sombre, c) >= theme::contraste(clair, c) {
        sombre
    } else {
        clair
    }
}


impl App {
    // ------------------------------------------------------------------ l'état

    /// La carte film ↔ écran des versions dépliées.
    pub(crate) fn depli_carte(&self) -> Carte {
        Carte::de(&self.doc, &self.depli.liste)
    }

    pub(crate) fn est_deplie(&self, plan: u64, numero: u32) -> bool {
        self.depli.liste.contains(&(plan, numero))
    }

    /// Oublie les versions dépliées qui ne sont plus des versions rangées d'un plan du film, et la tête posée dans une version
    /// dépliée quand le plan courant ou l'image courante ont changé depuis.
    pub(crate) fn depli_nettoyer(&mut self) {
        if self.depli.liste.is_empty() && self.depli.tete.is_none() {
            return;
        }
        let doc = &self.doc;
        self.depli.liste.retain(|&(plan, numero)| doc.panels.iter().any(|p| p.id == plan) && doc.versions_du_plan(plan).is_some_and(|v| v.rangee(numero).is_some()));
        if let Some(t) = self.depli.tete {
            let host = self.doc.panels.iter().position(|p| p.id == t.plan);
            let len = self.doc.versions_du_plan(t.plan).and_then(|v| v.rangee(t.numero)).map(|r| r.panel.frame_count().max(1));
            let ok = host == Some(self.cur_panel) && t.garde == (self.cur_panel, self.cur_frame) && self.est_deplie(t.plan, t.numero) && len.is_some_and(|n| t.frame < n);
            if !ok {
                self.depli.tete = None;
            }
        }
    }

    /// Déplie ou replie la version `numero` du plan `index` (rien n'est écrit dans le projet).
    pub(crate) fn depli_basculer(&mut self, index: usize, numero: u32) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        if self.doc.versions_du_plan(plan).and_then(|v| v.rangee(numero)).is_none() {
            return;
        }
        let actuelle = etiquette(self.doc.numero_de_version(plan));
        if let Some(k) = self.depli.liste.iter().position(|e| *e == (plan, numero)) {
            self.depli.liste.remove(k);
            if self.depli.tete.is_some_and(|t| t.plan == plan && t.numero == numero) {
                self.depli.tete = None;
            }
            self.status = format!("{} repliée.", etiquette(numero));
        } else {
            self.depli.liste.push((plan, numero));
            self.status = format!(
                "{} dépliée après la {actuelle}, en lecture seule : la lecture passe par les deux. Les plans suivants glissent à l'écran seulement ; le montage et les exports ne changent pas. ◂ la replie.",
                etiquette(numero)
            );
        }
    }

    /// L'image de l'écran où est la tête de lecture.
    pub(crate) fn depli_d(&self, carte: &Carte) -> usize {
        if let Some(t) = self.depli.tete {
            if let Some(k) = carte.rang(t.plan, t.numero) {
                return carte.d_de_lieu(Lieu::Deplie { k, frame: t.frame });
            }
        }
        carte.d_of(self.global_frame())
    }

    /// Pose la tête de lecture à l'image `d` de l'écran (sans arrêter la lecture).
    pub(crate) fn depli_aller(&mut self, carte: &Carte, d: usize) {
        let total = self.doc.total_frames().max(1);
        let d = d.min(carte.total(total) - 1);
        match carte.lieu(d) {
            Lieu::Film(g) => {
                let (p, f) = self.doc.locate(g.min(total - 1));
                self.set_panel(p);
                self.cur_frame = f;
                self.depli.tete = None;
            }
            Lieu::Deplie { k, frame } => {
                let i = carte.inserts[k];
                self.set_panel(i.host);
                self.cur_frame = self.doc.panels[i.host].frame_count().saturating_sub(1);
                self.depli.tete = Some(Tete { plan: i.plan, numero: i.numero, frame, garde: (self.cur_panel, self.cur_frame) });
            }
        }
        self.follow_frame = true;
    }

    /// Un clic de l'utilisateur à l'image `d` de l'écran : la lecture s'arrête, la tête y va.
    pub(crate) fn depli_cliquer(&mut self, carte: &Carte, d: usize) {
        self.end_stroke();
        self.playing = false;
        self.depli_aller(carte, d);
    }

    /// La lecture du film avance (ou recule) d'une image de l'écran ; faux au bout (sans `boucle`).
    pub(crate) fn depli_pas(&mut self, sens: i64, boucle: bool) -> bool {
        let carte = self.depli_carte();
        let n = carte.total(self.doc.total_frames().max(1)) as i64;
        let mut d = self.depli_d(&carte) as i64 + sens;
        if d < 0 || d >= n {
            if !boucle {
                return false;
            }
            d = d.rem_euclid(n);
        }
        self.depli_aller(&carte, d as usize);
        true
    }

    /// Des versions sont dépliées : la lecture et les flèches passent par elles.
    pub(crate) fn depli_actif(&self) -> bool {
        !self.depli.liste.is_empty()
    }

    /// Vrai si la tête de lecture est à la dernière image de l'écran.
    pub(crate) fn depli_au_bout(&self) -> bool {
        let carte = self.depli_carte();
        self.depli_d(&carte) + 1 >= carte.total(self.doc.total_frames().max(1))
    }

    /// L'image du film dont on entend le son quand la tête est dans une version dépliée : celle du plan à la même place (le son
    /// reprend au début du plan).
    pub(crate) fn depli_son_global(&self) -> Option<usize> {
        let t = self.depli.tete?;
        let host = self.doc.panels.iter().position(|p| p.id == t.plan)?;
        Some(self.doc.panel_start(host) + t.frame)
    }

    /// Pendant la lecture avec son (`tick_audio_clock`) : la position `g` du son, en images du film. Vrai si les versions dépliées
    /// ont pris la main (la tête est dans une version dépliée, ou vient d'y entrer).
    pub(crate) fn depli_suivre_le_son(&mut self, g: usize) -> bool {
        let carte = self.depli_carte();
        if carte.vide() {
            return false;
        }
        if let Some(t) = self.depli.tete {
            let Some(k) = carte.rang(t.plan, t.numero) else { return false };
            let i = carte.inserts[k];
            let f = g.saturating_sub(self.doc.panel_start(i.host));
            if f < i.len {
                if f != t.frame {
                    self.depli.tete = Some(Tete { frame: f, ..t });
                    self.follow_frame = true;
                }
                return true;
            }
            // Au bout de la version dépliée : la suivante, ou le plan d'après ; le son repart de là.
            let mut d = carte.debut(k) + i.len;
            if d >= carte.total(self.doc.total_frames().max(1)) {
                if !self.looping {
                    self.playing = false;
                    return true;
                }
                d = 0;
            }
            self.depli_aller(&carte, d);
            self.depli_relancer_son();
            return true;
        }
        // Le son vient de passer la fin d'un plan qui a des versions dépliées : la première d'entre elles, le son repart au début du plan.
        let avant = self.global_frame();
        if let Some(k) = carte.inserts.iter().position(|i| avant < i.at && i.at <= g) {
            self.depli_aller(&carte, carte.debut(k));
            self.depli_relancer_son();
            return true;
        }
        false
    }

    fn depli_relancer_son(&mut self) {
        if self.playing {
            self.stop_audio();
            self.start_audio_playback();
        }
    }

    /// Les pistes du film portant le son gardé de la version `numero` du plan `plan` (partie 168), posé à la place du plan dans le
    /// film : ce qu'on entend sous la version dépliée. Rien si la version n'a pas de son gardé (on rejoue alors le son du film depuis
    /// le début du plan). Les extraits d'une piste retirée depuis, ou d'un son qui n'est plus là, sont oubliés.
    pub(crate) fn depli_pistes(&self, plan: u64, numero: u32) -> Option<Vec<crate::document::AudioTrack>> {
        let sons = self.doc.son_de_version(plan, numero).filter(|s| !s.is_empty())?;
        let host = self.doc.panels.iter().position(|p| p.id == plan)?;
        let debut = self.doc.panel_start(host) as i64;
        let mut pistes: Vec<crate::document::AudioTrack> = self.doc.audio_tracks.iter().map(|t| crate::document::AudioTrack { clips: Vec::new(), ..t.clone() }).collect();
        for (k, s) in sons.iter().enumerate() {
            let Some(piste) = pistes.iter_mut().find(|t| t.id == s.piste) else { continue };
            if !self.doc.sounds.contains_key(&s.son) {
                continue;
            }
            let mut a = debut + s.debut;
            let (mut decalage, mut duree) = (s.decalage, s.duree as i64);
            if a < 0 {
                decalage += (-a) as usize;
                duree += a;
                a = 0;
            }
            if duree <= 0 {
                continue;
            }
            // (Des numéros d'extraits à part : ils ne sont jamais ceux du film.)
            piste.clips.push(crate::document::SoundClip::new(u64::MAX - k as u64, s.son, a as usize, decalage, duree as usize));
        }
        Some(pistes)
    }

    /// Le mélange à jouer quand la tête est dans une version dépliée qui a son son gardé.
    pub(crate) fn depli_mix(&self) -> Option<Vec<crate::audio::MixClip>> {
        let t = self.depli.tete?;
        let pistes = self.depli_pistes(t.plan, t.numero)?;
        Some(crate::audiofx::build_mix_pistes(&self.doc, &pistes, false))
    }

    // ------------------------------------------------------------------ changer de version, colorer

    /// Un clic sur l'étiquette « V2 » du plan `index` : elle devient la version en cours. Dépliée, elle se replie (elle est maintenant
    /// le plan) ; aucune autre ne se déplie à sa place.
    pub(crate) fn depli_passer(&mut self, index: usize, numero: u32) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        let ancien = self.doc.numero_de_version(plan);
        if ancien == numero {
            return;
        }
        self.passer_a_la_version(index, numero);
        // (Anthony : « si la version dépliée est sélectionnée, elle remplace la version active et l'autre version se replie » : elle
        // quitte les versions dépliées, et rien ne se déplie à la place de l'ancienne ; les autres versions dépliées le restent.)
        if self.doc.numero_de_version(plan) == numero {
            self.depli.liste.retain(|e| *e != (plan, numero));
            if self.depli.tete.is_some_and(|t| t.plan == plan) {
                self.depli.tete = None;
            }
        }
    }

    /// La couleur de repère de l'étiquette de la version `numero` du plan `index` : une étape d'annulation, rien si rien ne change.
    pub(crate) fn colorer_la_version(&mut self, index: usize, numero: u32, couleur: Option<u8>) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        if self.doc.couleur_de_version(plan, numero) == couleur {
            return;
        }
        let snap = self.snapshot();
        if !self.doc.colorer_version(plan, numero, couleur) {
            return;
        }
        let nom = etiquette(numero);
        let before = std::mem::replace(&mut self.history.context, Some(format!("Couleur de la {nom}")));
        self.commit_structure(snap);
        self.history.context = before;
        self.status = match couleur {
            Some(c) => format!("Étiquette {nom} en {} (Ctrl+Z l'enlève).", PALETTE.get(c as usize).map_or("couleur", |c| c.0).to_lowercase()),
            None => format!("Étiquette {nom} sans couleur (Ctrl+Z la remet)."),
        };
    }

    /// Le menu du clic droit sur l'étiquette d'une version : la palette, puis passer, déplier ou replier.
    pub(crate) fn depli_menu_version(&mut self, ui: &mut egui::Ui, plan: u64, numero: u32) {
        let Some(index) = self.doc.panels.iter().position(|p| p.id == plan) else {
            ui.close_menu();
            return;
        };
        let nom = etiquette(numero);
        let actuelle = self.doc.numero_de_version(plan);
        let existe = numero == actuelle || self.doc.versions_du_plan(plan).is_some_and(|v| v.rangee(numero).is_some());
        if !existe {
            ui.close_menu();
            return;
        }
        let courante = self.doc.couleur_de_version(plan, numero);
        ui.label(egui::RichText::new(format!("Couleur de l'étiquette {nom}")).color(theme::texte_doux()));
        let mut choix: Option<Option<u8>> = None;
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(5.0 * 24.0 + 8.0);
            for k in 0..PALETTE.len() as u8 {
                let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                ui.painter().rect_filled(rect.shrink(2.0), 4.0, couleur(k));
                if courante == Some(k) {
                    ui.painter().rect_stroke(rect, 4.0, Stroke::new(2.0_f32, ui.visuals().strong_text_color()), StrokeKind::Inside);
                } else if resp.hovered() {
                    ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0_f32, ui.visuals().text_color()), StrokeKind::Inside);
                }
                let nom_couleur = PALETTE[k as usize].0;
                if crate::bulles::bulle(resp, nom_couleur, &format!("Colore l'étiquette {nom} : un repère, rien d'autre ne change."), "").clicked() {
                    choix = Some(Some(k));
                }
            }
        });
        if ui.selectable_label(courante.is_none(), "Aucune").clicked() {
            choix = Some(None);
        }
        if let Some(c) = choix {
            ui.close_menu();
            self.colorer_la_version(index, numero, c);
            return;
        }
        if numero != actuelle {
            ui.separator();
            if ui.button(format!("Passer à la {nom}")).clicked() {
                ui.close_menu();
                self.depli_passer(index, numero);
                return;
            }
            let texte = if self.est_deplie(plan, numero) { format!("Replier la {nom}") } else { format!("Déplier la {nom} à côté") };
            if ui.button(texte).clicked() {
                ui.close_menu();
                self.depli_basculer(index, numero);
            }
        }
    }

    /// Le menu du clic droit sur « +2 » : un sous-menu par version qui n'a pas trouvé de place sur le bloc.
    pub(crate) fn depli_menu_toutes(&mut self, ui: &mut egui::Ui, plan: u64) {
        let numeros: Vec<u32> = self.doc.versions_du_plan(plan).map(|v| v.autres.iter().map(|r| r.numero).collect()).unwrap_or_default();
        if numeros.is_empty() {
            ui.close_menu();
            return;
        }
        for n in numeros {
            let marque = if self.est_deplie(plan, n) { " (dépliée)" } else { "" };
            ui.menu_button(format!("{}{marque}", etiquette(n)), |ui| self.depli_menu_version(ui, plan, n));
        }
    }

    /// Un clic sur une étiquette.
    pub(crate) fn depli_clic(&mut self, z: ZoneVersion) {
        match z.sorte {
            Sorte::Nom if z.numero == self.doc.numero_de_version(z.plan) => self.pile_epingler(z.plan),
            Sorte::Nom => self.depli_passer(z.index, z.numero),
            Sorte::Fleche => self.depli_basculer(z.index, z.numero),
            Sorte::Plus => self.pile_epingler(z.plan),
        }
    }

    /// La bulle d'une étiquette survolée (nom, ce que fait le clic, clic droit = couleur).
    pub(crate) fn depli_bulle(&self, resp: &egui::Response, z: ZoneVersion) {
        let nom = etiquette(z.numero);
        let actuelle = self.doc.numero_de_version(z.plan);
        let (titre, aide) = match z.sorte {
            Sorte::Nom if z.numero == actuelle => (format!("{nom}, version en cours"), "C'est celle que montrent le film, l'export et les envois. Clic : la pile des versions ; clic droit : sa couleur.".to_string()),
            Sorte::Nom => (format!("Passer à la {nom}"), format!("Clic : elle devient la version en cours (la {} est gardée, Ctrl+Z revient). Clic droit : couleur de l'étiquette.", etiquette(actuelle))),
            Sorte::Fleche if self.est_deplie(z.plan, z.numero) => (format!("Replier la {nom}"), "Le bloc déplié disparaît ; les plans suivants reprennent leur place à l'écran. Clic droit : couleur de l'étiquette.".to_string()),
            Sorte::Fleche => (format!("Déplier la {nom}"), format!("Elle se pose juste après la {}, en lecture seule, pour la lire et la comparer ; les plans suivants glissent à l'écran seulement (le montage et les exports ne changent pas). Clic droit : couleur de l'étiquette.", etiquette(actuelle))),
            Sorte::Plus => ("D'autres versions".to_string(), "Clic : la pile de toutes les versions du plan ; clic droit : passer, déplier, colorer.".to_string()),
        };
        resp.clone().on_hover_ui_at_pointer(|ui| crate::bulles::contenu(ui, &titre, &aide, ""));
    }

    // ------------------------------------------------------------------ la bande

    /// Le nom du plan `i` sur son bloc `rect`, puis ses autres versions en étiquettes (et la durée, s'il reste de la place). Rend les
    /// étiquettes posées. `gauche` : où commence le nom ; `bandeau` : le bloc est bas (le nom au milieu de sa hauteur).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn peindre_titre_du_bloc(&self, clip: &Painter, rect: Rect, i: usize, titre: String, gauche: f32, bandeau: bool, duree: Arc<Galley>, ink: Color32, weak: Color32) -> Vec<ZoneVersion> {
        let y_de = |h: f32| if bandeau { rect.center().y - h * 0.5 - 1.0 } else { rect.bottom() - 7.0 - h };
        let plan = self.doc.panels[i].id;
        let mut zones = Vec::new();
        let Some(v) = self.doc.versions_du_plan(plan).filter(|v| !v.autres.is_empty()) else {
            // Une seule version : le nom, et la durée si elle tient à côté (comme toujours).
            let title = clip.layout_no_wrap(titre, FontId::proportional(12.0), ink);
            if (gauche - rect.left()) + title.size().x + duree.size().x + 15.0 < rect.width() {
                clip.galley(pos2(rect.right() - 7.0 - duree.size().x, y_de(duree.size().y)), duree, weak);
            }
            clip.galley(pos2(gauche, y_de(title.size().y)), title, ink);
            return zones;
        };
        let pre = clip.layout_no_wrap(format!("{titre} · "), FontId::proportional(12.0), ink);
        let h = pre.size().y;
        let y = y_de(h);
        let cy = y + h * 0.5;
        clip.galley(pos2(gauche, y), pre.clone(), ink);
        let mut x = gauche + pre.size().x;
        // La version en cours, dans le nom (« V1 »), à sa couleur.
        let cur = v.actuelle;
        let g = clip.layout_no_wrap(etiquette(cur), FontId::proportional(12.0), ink);
        let r = Rect::from_min_size(pos2(x - 1.0, cy - ETIQ_H * 0.5), vec2(g.size().x + 2.0 + if v.couleurs.contains_key(&cur) { 6.0 } else { 0.0 }, ETIQ_H));
        let encre = match v.couleurs.get(&cur) {
            Some(&c) => {
                let c = couleur(c);
                clip.rect_filled(r, 3.0, c);
                encre_sur(c)
            }
            None => ink,
        };
        clip.galley(pos2(r.center().x - g.size().x * 0.5, y), g, encre);
        zones.push(ZoneVersion { zone: r, plan, index: i, numero: cur, sorte: Sorte::Nom });
        x = r.right() + ECART + 2.0;
        // Les autres versions : ce qui tient, puis « +N ».
        let autres: Vec<u32> = v.autres.iter().map(|r| r.numero).collect();
        let galleys: Vec<Arc<Galley>> = autres.iter().map(|n| clip.layout_no_wrap(etiquette(*n), FontId::proportional(11.0), ink)).collect();
        let largeur = |g: &Galley| g.size().x + 8.0 + 1.0 + FLECHE_W + ECART;
        let limite = rect.right() - 8.0;
        let plus_w = |reste: usize| clip.layout_no_wrap(format!("+{reste}"), FontId::proportional(11.0), ink).size().x + 8.0 + ECART;
        let mut m = autres.len();
        loop {
            let w: f32 = galleys[..m].iter().map(|g| largeur(g)).sum::<f32>() + if m < autres.len() { plus_w(autres.len() - m) } else { 0.0 };
            if x + w - ECART <= limite || m == 0 {
                break;
            }
            m -= 1;
        }
        for (k, g) in galleys.iter().take(m).enumerate() {
            let n = autres[k];
            let r = Rect::from_min_size(pos2(x, cy - ETIQ_H * 0.5), vec2(g.size().x + 8.0, ETIQ_H));
            let encre = match v.couleurs.get(&n) {
                Some(&c) => {
                    let c = couleur(c);
                    clip.rect_filled(r, 3.0, c);
                    encre_sur(c)
                }
                None => {
                    clip.rect_stroke(r, 3.0, Stroke::new(1.0_f32, theme::bord()), StrokeKind::Inside);
                    ink
                }
            };
            clip.galley(r.center() - g.size() * 0.5, g.clone(), encre);
            zones.push(ZoneVersion { zone: r, plan, index: i, numero: n, sorte: Sorte::Nom });
            let f = Rect::from_min_size(pos2(r.right() + 1.0, r.top()), vec2(FLECHE_W, ETIQ_H));
            let deplie = self.est_deplie(plan, n);
            let (icone, couleur) = if deplie { (ph::CARET_LEFT, theme::contour()) } else { (ph::CARET_RIGHT, ink) };
            clip.text(f.center(), Align2::CENTER_CENTER, icone, FontId::proportional(12.0), couleur);
            zones.push(ZoneVersion { zone: f, plan, index: i, numero: n, sorte: Sorte::Fleche });
            x = f.right() + ECART;
        }
        if m < autres.len() {
            let reste = autres.len() - m;
            let g = clip.layout_no_wrap(format!("+{reste}"), FontId::proportional(11.0), ink);
            let r = Rect::from_min_size(pos2(x, cy - ETIQ_H * 0.5), vec2(g.size().x + 8.0, ETIQ_H));
            if r.right() <= limite {
                clip.rect_stroke(r, 3.0, Stroke::new(1.0_f32, theme::bord()), StrokeKind::Inside);
                clip.galley(r.center() - g.size() * 0.5, g, ink);
                zones.push(ZoneVersion { zone: r, plan, index: i, numero: autres[m], sorte: Sorte::Plus });
                x = r.right() + ECART;
            }
        }
        // La durée, seulement si elle tient après tout cela.
        if x + 8.0 + duree.size().x <= rect.right() - 7.0 {
            clip.galley(pos2(rect.right() - 7.0 - duree.size().x, y_de(duree.size().y)), duree, weak);
        }
        // (Une étiquette coupée par le bord du bloc ou de la bande ne se vise pas.)
        let visible = clip.clip_rect();
        zones.retain(|z| visible.contains_rect(z.zone) && rect.contains_rect(z.zone));
        zones
    }

    /// Les versions dépliées de la bande : leur bloc (bordé, hachuré, « V2 dépliée »), et leur colonne hachurée sous lui (pas de
    /// pistes : rien ne s'y modifie). Rend les colonnes posées et les étiquettes de leurs blocs.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn peindre_versions_depliees(&mut self, ctx: &egui::Context, painter: &Painter, carte: &Carte, left: f32, ppf: f32, strip: Rect, top: f32, block_h: f32, thumb_h: f32, bandeau: bool, lanes_top: f32) -> (Vec<(Rect, usize)>, Vec<ZoneVersion>) {
        let mut colonnes = Vec::new();
        let mut zones = Vec::new();
        let aspect = self.doc.frame[0] as f32 / self.doc.frame[1].max(1) as f32;
        let deadline = Instant::now() + crate::thumbs::THUMB_TIME;
        let fps = self.doc.fps;
        for (k, ins) in carte.inserts.iter().enumerate() {
            let x0 = left + carte.debut(k) as f32 * ppf;
            let x1 = x0 + ins.len as f32 * ppf;
            let colonne = Rect::from_min_max(pos2(x0, strip.top()), pos2(x1, strip.bottom()));
            if !colonne.intersects(painter.clip_rect()) {
                continue;
            }
            colonnes.push((colonne, k));
            // La colonne : un fond léger et des hachures (rien ne s'y modifie), du haut des blocs au bas de la bande (la règle et la barre
            // des séquences restent entières au-dessus).
            let peinte = Rect::from_min_max(pos2(x0, top), pos2(x1, strip.bottom()));
            let p = painter.with_clip_rect(peinte.intersect(painter.clip_rect()));
            p.rect_filled(peinte, 0.0, theme::fenetre().gamma_multiply(0.55));
            let pas = 9.0;
            let hachure = Stroke::new(1.0_f32, theme::bord().gamma_multiply(0.6));
            let mut xh = x0 - peinte.height();
            while xh < x1 {
                p.line_segment([pos2(xh, peinte.bottom()), pos2(xh + peinte.height(), peinte.top())], hachure);
                xh += pas;
            }
            // Sous le bloc, la piste son montre ce qu'on y entend : le son gardé de la version, sinon celui du film depuis le début du
            // plan (posé à la place du plan, décalé jusqu'à la colonne).
            let l = x0 - self.doc.panel_start(ins.host) as f32 * ppf;
            let sous = Rect::from_min_max(pos2(x0, lanes_top), pos2(x1, strip.bottom())).intersect(painter.clip_rect());
            if sous.width() > 0.0 && sous.height() > 0.0 && !self.doc.audio_tracks.is_empty() {
                let pl = painter.with_clip_rect(sous);
                match self.depli_pistes(ins.plan, ins.numero) {
                    Some(pistes) => self.paint_lanes_of(&pl, strip, l, ppf, lanes_top, theme::contour(), &pistes),
                    None => self.paint_lanes(&pl, strip, l, ppf, lanes_top, theme::contour()),
                }
            }
            let couleur = self.doc.couleur_de_version(ins.plan, ins.numero).map(couleur);
            let bloc = Rect::from_min_max(pos2(x0 + 1.0, top + 2.0), pos2(x1 - 1.0, top + block_h));
            let fond = couleur.map_or(theme::bloc(), |c| theme::bloc().lerp_to_gamma(c, 0.3));
            p.rect_filled(bloc, 4.0, fond);
            let clip = p.with_clip_rect(bloc.shrink(2.0).intersect(p.clip_rect()));
            if let Some(r) = self.doc.versions_du_plan(ins.plan).and_then(|v| v.rangee(ins.numero)) {
                if let Some(tex) = self.thumbs.texture_rangee(ctx, &self.doc, &r.panel, crate::thumbs::STRIP_WIDTH, deadline) {
                    let tr = Rect::from_min_size(bloc.min + vec2(3.0, 3.0), vec2(thumb_h * aspect, thumb_h));
                    clip.image(tex, tr, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
            }
            // Des hachures légères sur le bloc aussi : on voit d'un coup d'œil que ce n'est pas un plan du film.
            let mut xh = bloc.left() - bloc.height();
            while xh < bloc.right() {
                clip.line_segment([pos2(xh, bloc.bottom()), pos2(xh + bloc.height(), bloc.top())], Stroke::new(1.0_f32, theme::bord().gamma_multiply(0.35)));
                xh += pas;
            }
            p.rect_stroke(bloc, 4.0, Stroke::new(1.5_f32, couleur.unwrap_or_else(theme::contour)), StrokeKind::Inside);
            // « V2 dépliée ◂ », en bas à gauche (au milieu en bandeau), et la durée de la version.
            let ink = theme::texte_fort();
            let gauche = if bandeau { bloc.left() + 3.0 + thumb_h * aspect + 5.0 } else { bloc.left() + 5.0 };
            let g = clip.layout_no_wrap(etiquette(ins.numero), FontId::proportional(11.0), ink);
            let cy = if bandeau { bloc.center().y - 1.0 } else { bloc.bottom() - 7.0 - ETIQ_H * 0.5 };
            let r = Rect::from_min_size(pos2(gauche, cy - ETIQ_H * 0.5), vec2(g.size().x + 8.0, ETIQ_H));
            let encre = match couleur {
                Some(c) => {
                    clip.rect_filled(r, 3.0, c);
                    encre_sur(c)
                }
                None => {
                    clip.rect_stroke(r, 3.0, Stroke::new(1.0_f32, theme::bord()), StrokeKind::Inside);
                    ink
                }
            };
            clip.galley(r.center() - g.size() * 0.5, g, encre);
            let f = Rect::from_min_size(pos2(r.right() + 1.0, r.top()), vec2(FLECHE_W, ETIQ_H));
            clip.text(f.center(), Align2::CENTER_CENTER, ph::CARET_LEFT, FontId::proportional(12.0), theme::contour());
            let mot = clip.layout_no_wrap("dépliée".into(), FontId::proportional(11.0), theme::texte_doux());
            let mut fin = f.right();
            if f.right() + 4.0 + mot.size().x < bloc.right() - 6.0 {
                clip.galley(pos2(f.right() + 4.0, cy - mot.size().y * 0.5), mot.clone(), theme::texte_doux());
                fin = f.right() + 4.0 + mot.size().x;
            }
            let duree = clip.layout_no_wrap(crate::storyboard::duration_label(ins.len, fps), FontId::monospace(10.0), theme::texte_doux());
            if fin + 10.0 + duree.size().x < bloc.right() - 7.0 {
                clip.galley(pos2(bloc.right() - 7.0 - duree.size().x, cy - duree.size().y * 0.5), duree, theme::texte_doux());
            }
            let visible = clip.clip_rect();
            for (zone, sorte) in [(r, Sorte::Nom), (f, Sorte::Fleche)] {
                if visible.contains_rect(zone) {
                    zones.push(ZoneVersion { zone, plan: ins.plan, index: ins.host, numero: ins.numero, sorte });
                }
            }
        }
        (colonnes, zones)
    }

    // ------------------------------------------------------------------ la vue

    /// La tête de lecture est dans une version dépliée : la vue montre son image, en lecture seule (rien ne s'y dessine). Vrai si
    /// c'est le cas (la vue ordinaire ne se dessine pas).
    pub(crate) fn depli_vue(&mut self, ui: &mut egui::Ui, rect: Rect) -> bool {
        self.depli_nettoyer();
        let Some(t) = self.depli.tete else { return false };
        let Some(host) = self.doc.panels.iter().position(|p| p.id == t.plan) else { return false };
        let ctx = ui.ctx().clone();
        let response = ui.allocate_rect(rect, Sense::click());
        if response.is_pointer_button_down_on() {
            self.panel_focus = false;
            self.images_focus = false;
            self.key_focus = false;
        }
        self.canvas_rect = rect;
        self.hover_canvas = None;
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, theme::fond());
        let deadline = Instant::now() + crate::thumbs::THUMB_TIME;
        // (Une vignette à la largeur de la vue, rangée sous le numéro du plan rangé de la version : la file des vignettes la garde, la
        // refait sur ses fils quand l'image change, et montre la précédente en attendant. Jamais la largeur de la bande.)
        let largeur = crate::thumbs::width_for(rect.width().min(rect.height() * 2.0) * ctx.pixels_per_point()).max(256);
        let tex = match self.doc.versions_du_plan(t.plan).and_then(|v| v.rangee(t.numero)) {
            Some(r) => self.thumbs.texture_rangee_a(&ctx, &self.doc, &r.panel, t.frame, r.panel.id, largeur, deadline),
            None => None,
        };
        let montree = tex;
        let aspect = self.doc.frame[0] as f32 / self.doc.frame[1].max(1) as f32;
        let zone = rect.shrink(16.0);
        let (w, h) = if zone.width() / zone.height().max(1.0) > aspect { (zone.height() * aspect, zone.height()) } else { (zone.width(), zone.width() / aspect) };
        let image = Rect::from_center_size(zone.center(), vec2(w, h));
        match montree {
            Some(id) => {
                painter.image(id, image, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            None => {
                painter.rect_filled(image, 0.0, theme::champ());
                painter.text(image.center(), Align2::CENTER_CENTER, "Image en préparation…", FontId::proportional(14.0), theme::texte_doux());
            }
        }
        let couleur = self.doc.couleur_de_version(t.plan, t.numero).map(couleur);
        painter.rect_stroke(image, 0.0, Stroke::new(2.0_f32, couleur.unwrap_or_else(theme::contour)), StrokeKind::Outside);
        // « Plan 1 · V2 dépliée · lecture seule », en haut à gauche de l'image.
        let titre = format!("{} · {} dépliée · lecture seule · image {}", crate::storyboard::panel_title(&self.doc, host), etiquette(t.numero), t.frame + 1);
        let g = painter.layout_no_wrap(titre, FontId::proportional(12.0), theme::texte_fort());
        let pill = Rect::from_min_size(image.min + vec2(6.0, 6.0), g.size() + vec2(12.0, 6.0));
        painter.rect_filled(pill, 4.0, theme::fenetre().gamma_multiply(0.92));
        painter.rect_stroke(pill, 4.0, Stroke::new(1.0_f32, couleur.unwrap_or_else(theme::bord)), StrokeKind::Inside);
        painter.galley(pill.min + vec2(6.0, 3.0), g, theme::texte_fort());
        if response.hovered() {
            response.on_hover_ui_at_pointer(|ui| {
                crate::bulles::contenu(ui, &format!("{} dépliée, en lecture seule", etiquette(t.numero)), "On la lit pour la comparer ; on n'y dessine pas. Pour la retoucher, passe à cette version (clic sur son étiquette dans la timeline).", "")
            });
        }
        if self.thumbs.pending || tex.is_none() {
            ctx.request_repaint();
        }
        true
    }
}

/// L'image de l'écran sous l'abscisse `x` de la bande (`left` : l'abscisse de l'image 0, `ppf` : la largeur d'une image).
pub(crate) fn d_sous(x: f32, left: f32, ppf: f32) -> usize {
    ((x - left) / ppf).floor().max(0.0) as usize
}

/// La première étiquette sous le point `p`.
pub(crate) fn zone_sous(zones: &[ZoneVersion], p: Pos2) -> Option<ZoneVersion> {
    zones.iter().copied().find(|z| z.zone.contains(p))
}
