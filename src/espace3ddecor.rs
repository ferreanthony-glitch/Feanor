//! L'environnement de l'Espace 3D et les calques 3D (`docs/INTERFACE.md`, partie 157). Anthony, 2026-11-14 : « j'aimerais que
//! l'environnement soit paramétrable. La couleur du fond, le maillage, la possibilité d'ajouter une grille pour le calque de sol,
//! le mur et le ciel (ces calques sont disponibles dans la création des calques dans un regroupement de calques 3D). Les outils
//! de dessin dans l'environnement 3D et la façon de les utiliser doivent rester exactement pareils. »
//!
//! - **L'environnement** (`Environnement3d`) : la couleur du fond de la vue Espace 3D, et le maillage du sol de la vue (montré
//!   ou non, sa couleur, son opacité, son pas). Il se règle dans le panneau Espace 3D (un réglage de la VUE, règle d'Anthony
//!   du 2026-11-12) et **le projet le retient** (champ facultatif `environnement_3d` du manifeste, sans changer le format),
//!   comme les vues (`vues_3d`) : un décor de nuit pour un film, un fond clair pour un autre ; le projet emporte son aspect
//!   sur une autre machine. Hors de l'annulation, comme les vues : il ne change jamais l'image du film. D'origine
//!   (`Environnement3d::default`) : l'aspect d'avant, exactement, et rien n'est écrit dans le manifeste.
//! - **Les calques 3D** (`Calque3d`) : « Nouveau calque > Calques 3D > Sol, Mur, Ciel ». Chacun est un calque de dessin
//!   ordinaire (vectoriel ou bitmap, comme le calque courant) posé dans l'espace par `Monde::surface_xf` (`Surface::Sol`,
//!   `Surface::Mur`, `Surface::Ciel`) : la commande « Nouvelle surface » du panneau, avec un nom court et une marque. Une
//!   seule étape d'annulation, aucune fenêtre. Un plan qui n'est pas en Espace 3D refuse, comme « Nouvelle surface » : la
//!   barre d'état le dit d'abord.
//! - **La grille de repère** d'un calque 3D (`Calque3d::grille`, `Calque3d::pas`) : des traits bleus peints **par-dessus**
//!   la carte dans la vue Espace 3D, et par-dessus l'image quand ce calque est le calque courant dans le dessin. Elle n'est
//!   jamais dans le dessin : ni dans les exports, ni sous la gomme, ni dans les traits. Le dessin, lui, ne change pas d'un
//!   pixel : mêmes outils, mêmes gestes, même rendu que sur n'importe quel calque.

use eframe::egui::{self, pos2, Color32, Pos2, RichText, Stroke};
use egui_phosphor::regular as ph;
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::espace3d::Surface;
use crate::espace3dview::GridLine;

// ------------------------------------------------------------------------------------ l'environnement

/// La couleur du fond de la vue Espace 3D d'origine (le gris de toujours).
pub const FOND_D_ORIGINE: [u8; 3] = [26, 26, 26];
/// La couleur des traits ordinaires du maillage d'origine (rouge, vert, bleu de 0 à 1 ; les axes ont la leur).
const GRIS_DU_MAILLAGE: [f32; 3] = [0.7, 0.7, 0.72];
/// Le pas du maillage le plus fin et le plus grand qu'on règle (mètres).
pub const PAS_MIN: f32 = 0.05;
pub const PAS_MAX: f32 = 1000.0;

/// L'environnement de la vue Espace 3D, retenu par le projet (`Document::environnement_3d`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Environnement3d {
    /// La couleur du fond de la vue.
    #[serde(default = "fond_d_origine")]
    pub fond: [u8; 3],
    /// Le maillage du sol est montré (le bouton « Grille du sol » du panneau, `App::espace3d.show_grid`).
    #[serde(default = "oui")]
    pub maillage: bool,
    /// La couleur des traits ordinaires du maillage ; rien : le gris d'origine. Les axes gardent leur couleur.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maillage_couleur: Option<[u8; 3]>,
    /// L'opacité du maillage, de 0 à 1, qui multiplie celle de chaque trait (1 : celle d'origine).
    #[serde(default = "un")]
    pub maillage_opacite: f32,
    /// Le pas du maillage, en mètres ; 0 : automatique, à l'échelle de ce qu'on regarde (comme avant).
    #[serde(default)]
    pub maillage_pas: f32,
}

fn fond_d_origine() -> [u8; 3] {
    FOND_D_ORIGINE
}

fn oui() -> bool {
    true
}

fn un() -> f32 {
    1.0
}

impl Default for Environnement3d {
    fn default() -> Environnement3d {
        Environnement3d { fond: FOND_D_ORIGINE, maillage: true, maillage_couleur: None, maillage_opacite: 1.0, maillage_pas: 0.0 }
    }
}

impl Environnement3d {
    /// Lu en souple depuis le manifeste : une valeur qui n'a pas la bonne forme ne donne rien (l'aspect d'origine), et les
    /// nombres abîmés reviennent dans leurs bornes.
    pub fn lire(v: serde_json::Value) -> Option<Environnement3d> {
        serde_json::from_value::<Environnement3d>(v).ok().map(Environnement3d::saine)
    }

    /// Les nombres dans leurs bornes : opacité de 0 à 1 (abîmée : 1), pas de 0 (automatique) à `PAS_MAX`.
    pub fn saine(self) -> Environnement3d {
        let opacite = if self.maillage_opacite.is_finite() { self.maillage_opacite.clamp(0.0, 1.0) } else { 1.0 };
        let pas = if self.maillage_pas.is_finite() && self.maillage_pas > 0.0 { self.maillage_pas.clamp(PAS_MIN, PAS_MAX) } else { 0.0 };
        Environnement3d { maillage_opacite: opacite, maillage_pas: pas, ..self }
    }

    /// L'aspect d'origine : le projet n'écrit rien.
    pub fn est_d_origine(&self) -> bool {
        *self == Environnement3d::default()
    }

    pub fn couleur_du_fond(&self) -> Color32 {
        let [r, g, b] = self.fond;
        Color32::from_rgb(r, g, b)
    }

    /// Le pas réglé du maillage, s'il n'est pas automatique.
    pub fn pas_du_maillage(&self) -> Option<f32> {
        (self.maillage_pas > 0.0).then_some(self.maillage_pas)
    }

    /// La couleur des traits ordinaires du maillage, telle que le panneau la montre.
    pub fn couleur_du_maillage(&self) -> [u8; 3] {
        self.maillage_couleur.unwrap_or(GRIS_DU_MAILLAGE.map(|c| (c * 255.0) as u8))
    }

    /// Les traits du maillage à la couleur et à l'opacité de l'environnement. D'origine, ils ne changent pas d'un bit.
    pub(crate) fn teinter_maillage(&self, mut lignes: Vec<GridLine>) -> Vec<GridLine> {
        if self.maillage_couleur.is_none() && self.maillage_opacite == 1.0 {
            return lignes;
        }
        for l in &mut lignes {
            if let Some(c) = self.maillage_couleur {
                // (Les axes, rouge et bleu, gardent leur couleur : ils disent où est l'origine.)
                if l.color[..3] == GRIS_DU_MAILLAGE {
                    l.color = [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, l.color[3]];
                }
            }
            l.color[3] *= self.maillage_opacite;
        }
        lignes
    }
}

// ------------------------------------------------------------------------------------ les calques 3D

/// La sorte d'un calque 3D.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sorte3d {
    #[serde(rename = "sol")]
    Sol,
    #[serde(rename = "mur")]
    Mur,
    #[serde(rename = "ciel")]
    Ciel,
}

impl Sorte3d {
    /// Dans l'ordre du menu « Calques 3D ».
    pub const TOUTES: [Sorte3d; 3] = [Sorte3d::Sol, Sorte3d::Mur, Sorte3d::Ciel];

    /// Le nom du calque créé et de l'étape d'annulation.
    pub fn nom(self) -> &'static str {
        match self {
            Sorte3d::Sol => "Sol",
            Sorte3d::Mur => "Mur",
            Sorte3d::Ciel => "Ciel",
        }
    }

    /// La surface qu'il pose (`Monde::surface_xf`).
    pub fn surface(self) -> Surface {
        match self {
            Sorte3d::Sol => Surface::Sol,
            Sorte3d::Mur => Surface::Mur,
            Sorte3d::Ciel => Surface::Ciel,
        }
    }

    pub fn icone(self) -> &'static str {
        match self {
            Sorte3d::Sol => ph::ALIGN_BOTTOM,
            Sorte3d::Mur => ph::WALL,
            Sorte3d::Ciel => ph::CLOUD,
        }
    }

    /// Une phrase, dans le menu.
    pub fn phrase(self) -> &'static str {
        match self {
            Sorte3d::Sol => "Couché au sol, devant la caméra.",
            Sorte3d::Mur => "Debout sur le sol, face à la caméra.",
            Sorte3d::Ciel => "Tout au fond, face à la caméra, centré sur l'horizon.",
        }
    }

    /// La commande (palette, raccourci qu'on lui donne).
    pub fn action(self) -> &'static str {
        match self {
            Sorte3d::Sol => "calque.nouveau_sol",
            Sorte3d::Mur => "calque.nouveau_mur",
            Sorte3d::Ciel => "calque.nouveau_ciel",
        }
    }

    /// Le pas d'origine de sa grille (mètres) : un mètre au sol et au mur, dix au ciel (il est vingt fois plus loin).
    pub fn pas_d_origine(self) -> f32 {
        match self {
            Sorte3d::Sol | Sorte3d::Mur => 1.0,
            Sorte3d::Ciel => 10.0,
        }
    }
}

/// Ce qu'un calque 3D garde (`Layer::calque_3d`) : sa sorte, et sa grille de repère.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calque3d {
    pub sorte: Sorte3d,
    /// La grille de repère est montrée (jamais dans le dessin).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub grille: bool,
    /// Le pas de la grille, en mètres de la carte dans l'espace (absent ou abîmé : celui d'origine de la sorte, `lire`).
    #[serde(default)]
    pub pas: f32,
}

impl Calque3d {
    /// Un calque 3D neuf : grille éteinte (on l'ajoute si on veut), au pas d'origine de sa sorte.
    pub fn neuf(sorte: Sorte3d) -> Calque3d {
        Calque3d { sorte, grille: false, pas: sorte.pas_d_origine() }
    }

    /// Lu en souple depuis un calque du manifeste : une valeur mal formée (une sorte inconnue d'une version plus récente…)
    /// ne donne rien, le calque s'ouvre comme un calque ordinaire ; un pas abîmé revient à celui d'origine.
    pub fn lire(v: serde_json::Value) -> Option<Calque3d> {
        let c = serde_json::from_value::<Calque3d>(v).ok()?;
        let pas = if c.pas.is_finite() && c.pas > 0.0 { c.pas.clamp(PAS_MIN, PAS_MAX) } else { c.sorte.pas_d_origine() };
        Some(Calque3d { pas, ..c })
    }
}

// ------------------------------------------------------------------------------------ la grille de repère

/// Au plus tant de traits dans chaque sens : au-delà, le pas montré est multiplié par cinq (jusqu'à tenir).
pub const TRAITS_MAX: f32 = 200.0;

/// Un trait de la grille de repère, en pixels du dessin de la carte (la feuille) ; `fort` : un trait sur cinq, compté
/// depuis le milieu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LigneGrille {
    pub a: Pos2,
    pub b: Pos2,
    pub fort: bool,
}

/// Les traits de la grille de repère d'une carte : la feuille (`stage`, pixels) mesure `taille_m` mètres dans l'espace, un
/// trait tous les `pas` mètres, depuis le milieu de la feuille. Rien si une mesure est abîmée.
pub fn lignes_de_grille(stage: [f32; 2], taille_m: [f32; 2], pas: f32) -> Vec<LigneGrille> {
    let [w, h] = stage;
    let (sx, sy) = (pas * w / taille_m[0], pas * h / taille_m[1]);
    if !(w > 0.0 && h > 0.0 && sx.is_finite() && sy.is_finite() && sx > 0.0 && sy > 0.0) {
        return Vec::new();
    }
    let mut f = 1.0;
    for _ in 0..20 {
        if w / (sx * f) <= TRAITS_MAX && h / (sy * f) <= TRAITS_MAX {
            break;
        }
        f *= 5.0;
    }
    let (sx, sy) = (sx * f, sy * f);
    // (Une carte démesurée, que cent millions de fois le pas ne bornent pas : pas de grille plutôt qu'une boucle sans fin.)
    if w / sx > TRAITS_MAX || h / sy > TRAITS_MAX {
        return Vec::new();
    }
    let (cx, cy) = (w / 2.0, h / 2.0);
    let mut out = Vec::new();
    let n = (cx / sx).floor() as i64;
    for i in -n..=n {
        let x = cx + i as f32 * sx;
        out.push(LigneGrille { a: pos2(x, 0.0), b: pos2(x, h), fort: i % 5 == 0 });
    }
    let n = (cy / sy).floor() as i64;
    for i in -n..=n {
        let y = cy + i as f32 * sy;
        out.push(LigneGrille { a: pos2(0.0, y), b: pos2(w, y), fort: i % 5 == 0 });
    }
    out
}

/// La couleur de la grille : le bleu du crayon de repère, qu'on distingue du trait.
pub fn couleur_de_grille(fort: bool) -> Color32 {
    Color32::from_rgba_unmultiplied(96, 160, 230, if fort { 190 } else { 110 })
}

/// Peint ces traits de grille à l'écran : `vers_ecran` mène un point du dessin de la carte à l'écran (rien s'il ne se voit
/// pas). Chaque trait est coupé en morceaux : ce qui ne se voit pas (derrière l'œil) est laissé.
pub fn peindre_grille(painter: &egui::Painter, lignes: &[LigneGrille], vers_ecran: &dyn Fn(Pos2) -> Option<Pos2>) {
    const MORCEAUX: usize = 12;
    for l in lignes {
        let stroke = Stroke::new(1.0_f32, couleur_de_grille(l.fort));
        let mut avant = vers_ecran(l.a);
        for k in 1..=MORCEAUX {
            let ici = vers_ecran(l.a + (l.b - l.a) * (k as f32 / MORCEAUX as f32));
            if let (Some(p), Some(q)) = (avant, ici) {
                painter.line_segment([p, q], stroke);
            }
            avant = ici;
        }
    }
}

// ------------------------------------------------------------------------------------ dans l'application

impl App {
    /// L'environnement de la vue Espace 3D du projet (d'origine s'il n'en dit rien).
    pub(crate) fn e3_environnement(&self) -> Environnement3d {
        self.doc.environnement_3d.unwrap_or_default()
    }

    /// Le projet retient cet environnement (rien s'il est d'origine). Hors de l'annulation, comme les vues : une façon de
    /// regarder, pas une retouche du film.
    pub(crate) fn e3_poser_environnement(&mut self, env: Environnement3d) {
        let env = env.saine();
        let v = (!env.est_d_origine()).then_some(env);
        if self.doc.environnement_3d != v {
            self.doc.environnement_3d = v;
            self.modified = true;
        }
        self.espace3d.show_grid = env.maillage;
    }

    /// Montre ou cache le maillage du sol (bouton « Grille du sol », commande `espace3d.grille`) : le projet le retient.
    pub(crate) fn e3_maillage_basculer(&mut self) {
        let env = Environnement3d { maillage: !self.espace3d.show_grid, ..self.e3_environnement() };
        self.e3_poser_environnement(env);
    }

    /// « Nouveau calque > Calques 3D » : un Sol, un Mur ou un Ciel posé dans l'espace du plan courant (`espace3d_poser_surface`),
    /// le calque courant, une étape d'annulation. Hors d'un plan en Espace 3D : refusé, comme « Nouvelle surface », et la
    /// barre d'état le dit d'abord.
    pub(crate) fn nouveau_calque_3d(&mut self, sorte: Sorte3d) {
        let nom = sorte.nom();
        if !self.doc.plan_en_espace(self.cur_panel) {
            self.status = format!("{nom} refusé : ce plan n'est pas en Espace 3D. Coche d'abord « Plan en Espace 3D » dans le panneau Espace 3D, puis recommence.");
            return;
        }
        let distance = self.espace3d_distance_surface();
        // (Les bornes de `Monde::surface_xf` : le nombre dit est celui de la pose.)
        let d0 = self.e3_world().d0;
        let distance = distance.clamp(d0 * 0.05, d0 * 200.0);
        let recul = if sorte == Sorte3d::Ciel { format!("{:.0} m", (distance * crate::espace3d::CIEL_RECUL).min(d0 * 200.0)) } else { format!("{distance:.1} m") };
        match self.espace3d_poser_surface(sorte.surface(), nom, Some(Calque3d::neuf(sorte))) {
            Some((pose, plus_petit)) => {
                self.status = if plus_petit {
                    format!("« {pose} » posé à {recul}, mais plus petit que le cadre : près de la caméra d'origine du plan et derrière elle, une carte ne dépasse pas dix fois la feuille.")
                } else {
                    format!("« {pose} » posé à {recul} : c'est le calque courant, dessine dessus comme sur tout calque. Sa grille : panneau Espace 3D.")
                };
            }
            None => {
                if self.status.is_empty() {
                    self.status = format!("{nom} refusé : à {recul} devant cette caméra, il tomberait trop loin de la feuille pour être une carte. Rapproche la caméra de la feuille.");
                }
            }
        }
    }

    /// Allume ou éteint la grille de repère du calque `li` (commande `calque.grille_3d` pour le calque courant) : une étape
    /// d'annulation. Un calque qui n'est pas un calque 3D n'en a pas : la barre d'état le dit.
    pub(crate) fn basculer_grille_3d(&mut self, li: usize) {
        let Some(c) = self.layers().get(li).and_then(|l| l.calque_3d) else {
            self.status = "Grille : seulement sur un calque Sol, Mur ou Ciel (Nouveau calque > Calques 3D).".into();
            return;
        };
        let grille = !c.grille;
        // (La sélection agit sur tout, partie 136 : les calques 3D choisis avec lui prennent le même état ; les autres n'ont pas
        // de grille et ne changent pas.)
        let lis: Vec<usize> = self.calques_du_geste(li).into_iter().filter(|&k| self.layers().get(k).is_some_and(|l| l.calque_3d.is_some())).collect();
        self.end_stroke();
        let before = self.history.context.replace("Grille du calque".to_string());
        self.record_step();
        self.history.context = before;
        for &k in &lis {
            if let Some(c) = self.layers_mut()[k].calque_3d.as_mut() {
                c.grille = grille;
            }
        }
        if lis.len() >= 2 {
            self.status = format!("Grille {} sur {} calques 3D.", if grille { "allumée" } else { "éteinte" }, lis.len());
            return;
        }
        self.status = if grille {
            format!("Grille allumée : un trait tous les {} m sur la carte. Un repère seulement, jamais dans le dessin ni les exports.", nombre(c.pas))
        } else {
            "Grille éteinte.".into()
        };
    }

    /// Le panneau Espace 3D, section « Environnement » : la couleur du fond, le maillage du sol (couleur, opacité, pas).
    pub(crate) fn e3_environnement_ui(&mut self, ui: &mut egui::Ui) {
        let mut env = Environnement3d { maillage: self.espace3d.show_grid, ..self.e3_environnement() };
        let avant = env;
        egui::CollapsingHeader::new(RichText::new(format!("{}  Environnement", ph::SUN_HORIZON)).strong()).id_salt("espace3d_environnement").default_open(false).show(ui, |ui| {
            egui::Grid::new("espace3d_environnement_grille").num_columns(2).spacing([6.0, 4.0]).show(ui, |ui| {
                ui.label("Fond");
                let r = crate::champcouleur::srgb(ui, &mut env.fond);
                crate::bulles::bulle(r, "Couleur du fond", "Le fond de la vue Espace 3D. Une façon de regarder : jamais dans l'image du film.", "");
                ui.end_row();
                ui.label("Maillage");
                ui.horizontal(|ui| {
                    let r = ui.checkbox(&mut env.maillage, "");
                    crate::bulles::bulle(r, "Montrer le maillage", "Le quadrillage du sol de la vue (le bouton « Grille du sol » : la même chose).", "espace3d.grille");
                    let mut c = env.couleur_du_maillage();
                    let r = crate::champcouleur::srgb(ui, &mut c);
                    if crate::bulles::bulle(r, "Couleur du maillage", "Ses traits ordinaires ; les axes gardent leur couleur.", "").changed() {
                        env.maillage_couleur = Some(c);
                    }
                    let mut pct = env.maillage_opacite * 100.0;
                    let r = ui.add(egui::DragValue::new(&mut pct).speed(1.0).range(0.0..=100.0).max_decimals(0).suffix(" %"));
                    if crate::bulles::bulle(r, "Opacité du maillage", "100 % : celle d'origine.", "").changed() {
                        env.maillage_opacite = pct / 100.0;
                    }
                });
                ui.end_row();
                ui.label("Pas");
                ui.horizontal(|ui| {
                    let mut auto = env.maillage_pas <= 0.0;
                    let r = ui.checkbox(&mut auto, "auto");
                    if crate::bulles::bulle(r, "Pas automatique", "Le pas suit ce qu'on regarde : de près des cases fines, de loin de grandes cases.", "").changed() {
                        env.maillage_pas = if auto { 0.0 } else { 1.0 };
                    }
                    let mut pas = if auto { 1.0 } else { env.maillage_pas };
                    let r = ui.add_enabled(!auto, egui::DragValue::new(&mut pas).speed(0.05).range(PAS_MIN..=PAS_MAX).max_decimals(2).suffix(" m"));
                    if crate::bulles::bulle(r, "Taille des cases", "Un trait tous les tant de mètres, un plus marqué tous les cinq.", "").changed() {
                        env.maillage_pas = pas;
                    }
                });
                ui.end_row();
            });
            let origine = Environnement3d { maillage: env.maillage, ..Environnement3d::default() };
            if ui.add_enabled(env != origine, egui::Button::new(format!("{}  Aspect d'origine", ph::ARROW_COUNTER_CLOCKWISE))).on_hover_text("Le fond et le maillage d'origine.").clicked() {
                env = origine;
            }
        });
        if env != avant {
            self.e3_poser_environnement(env);
        }
    }

    /// Le panneau Espace 3D, sous les réglages de la carte choisie : la grille de repère d'un calque 3D (allumée, pas).
    pub(crate) fn e3_grille_carte_ui(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some(c) = self.layers().get(li).and_then(|l| l.calque_3d) else { return };
        ui.horizontal(|ui| {
            let r = self.bouton_etat(ui, c.grille, ph::GRID_NINE, "Grille", "Une grille de repère sur la carte, dans cette vue et dans le dessin quand ce calque est le calque courant. Un repère seulement : jamais dans le dessin, ni sous la gomme, ni dans les exports.", "calque.grille_3d");
            if r.clicked() {
                self.basculer_grille_3d(li);
            }
            let mut pas = c.pas;
            let r = ui.add(egui::DragValue::new(&mut pas).speed(0.05).range(PAS_MIN..=PAS_MAX).max_decimals(2).suffix(" m"));
            let r = crate::bulles::bulle(r, "Pas de la grille", "Un trait tous les tant de mètres de la carte, un plus marqué tous les cinq.", "");
            let before = self.history.context.replace("Pas de la grille".to_string());
            let change = self.edited(&r);
            self.history.context = before;
            if change {
                if let Some(c) = self.layers_mut()[li].calque_3d.as_mut() {
                    c.pas = pas.clamp(PAS_MIN, PAS_MAX);
                }
            }
            ui.label(RichText::new(format!("{} {}", c.sorte.icone(), c.sorte.nom())).small().weak());
        });
    }

    /// La grille de repère du calque courant, peinte sur l'image du dessin (pas dans la vue Espace 3D : elle a la sienne,
    /// sur chaque carte ; ni dans une case de la planche).
    pub(crate) fn peindre_grille_du_calque(&self, painter: &egui::Painter) {
        // (Ni pendant la lecture : on regarde le film, pas un repère.)
        if self.in_card || self.playing || self.board_shown() || self.espace3d_vue_ouverte() {
            return;
        }
        let l = self.layer();
        // (Caché, ou dans un dossier caché : pas de grille sur un dessin qu'on ne voit pas, comme dans la vue Espace 3D.)
        let visible = crate::folders::effect(self.layers(), self.cur_layer).is_some();
        let Some(g) = l.calque_3d.filter(|g| g.grille && visible) else { return };
        let world = self.e3_world();
        let xf = world.carte(l, self.cur_frame);
        let lignes = lignes_de_grille(world.stage, [xf.size[0].abs(), xf.size[1].abs()], g.pas);
        // (Le chemin de `layer_to_screen`, sans sa roue de secours : un point de la carte qui ne tombe pas sur la feuille, derrière
        // l'œil, n'est pas peint.)
        let penchee = self.tilted_card();
        let vers_ecran = |p: Pos2| -> Option<Pos2> {
            match &penchee {
                Some((w, mx, eye)) => w.card_to_sheet_m(*eye, mx, p).map(|s| self.view.to_screen(self.canvas_rect, self.canvas_size(), s)),
                None => Some(self.layer_to_screen(p)),
            }
        };
        peindre_grille(painter, &lignes, &vers_ecran);
    }
}

/// Un nombre de mètres sans zéros inutiles (« 1 », « 2,5 »).
fn nombre(v: f32) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', ",")
}
