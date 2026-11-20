//! Partie 198 : l'onglet 3D (`Espace::TroisD`, `docs/INTERFACE.md` partie 198, étape 3 sur 5 de la maquette `onglet-3d-v8`). La
//! séquence courante en trois dimensions : son décor 3D, les caméras de tous ses plans, les cartes (les calques) du plan courant.
//!
//! - **Le centre** est la vue Espace 3D d'aujourd'hui, en grand (`espace3dview.rs`, `espace3dvues.rs`, `decor3d*.rs`) : l'onglet
//!   l'ouvre en y entrant, et la referme en sortant si elle l'était. Sa barre (`o3_barre_de_vue`) choisit le point de vue de la vue
//!   active (Perspective, Dessus, Face, Côté, Caméra du plan), le nombre de vues (1, 2 ou 4 : `espace3dvues::Disposition`), et ce
//!   qu'elle montre (ombres, grille, cartes, caméras des autres plans, soleil) ; Recadrer. Une vue orthographique seule est la vue
//!   agrandie des quatre (`Vues::agrandie`).
//! - **La boussole** (`o3_boussole`) en haut à droite de chaque vue en perspective : X rouge, Y vert, Z bleu ; un clic sur un bout
//!   d'axe tourne la vue pour regarder le long de lui, en un quart de seconde adouci (`Vise`).
//! - **La barre d'outils de gauche** n'a que les outils de la vue (`App::espace3d_outils_de`) : choisir, déplacer, tourner, mettre à
//!   l'échelle, modifier l'origine, aimanter au sol.
//! - **La colonne** a sa pile (`DockLayout::pour_la_3d`) : « Scène 3D » (`onglet3dui.rs`) puis Propriétés, dont l'onglet devant suit
//!   ce qui est choisi (`o3_suivre`).
//! - **La bande du bas**, sur toute la largeur : la timeline de la séquence et la banque 3D.
//!
//! Sans latence (ESPACE3D.md) : rien ne se redessine au repos ; seule l'animation de la boussole demande des images, le temps
//! qu'elle dure.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::dock::Pane;
use crate::espace3dgizmo::CAMERA_ID;
use crate::espace3dmath::{self as m, View, V3, MAX_ELEVATION};
use crate::espace3dvues::{Disposition, PointDeVue};
use crate::workspace::Espace;

/// La durée de l'animation de la boussole (secondes), celle de la maquette.
pub const DUREE_VISE: f64 = 0.26;
/// Le rayon de la boussole (points) et celui d'un bout d'axe.
pub const RAYON_BOUSSOLE: f32 = 28.0;
pub const RAYON_BOUT: f32 = 8.0;
/// Les couleurs des axes X, Y, Z.
pub const COULEURS_AXES: [Color32; 3] = [Color32::from_rgb(232, 80, 80), Color32::from_rgb(100, 205, 100), Color32::from_rgb(86, 138, 245)];

/// Les points de vue de la barre de l'onglet, dans son ordre (« Côté » : la vue de droite).
pub const POINTS: [(PointDeVue, &str); 5] = [
    (PointDeVue::Perspective, "Perspective"),
    (PointDeVue::Dessus, "Dessus"),
    (PointDeVue::Face, "Face"),
    (PointDeVue::Droite, "Côté"),
    (PointDeVue::Camera, "Caméra du plan"),
];

/// Les quatre vues de l'onglet : dessus, perspective, face, la caméra du plan (la maquette).
pub const QUATRE: [PointDeVue; 4] = [PointDeVue::Dessus, PointDeVue::Perspective, PointDeVue::Face, PointDeVue::Camera];

/// La vue qui tourne vers un axe : depuis où, vers où (azimut, hauteur, degrés), à partir de quand. `cadre` : rien, la vue seule ;
/// sinon le cadre des vues multiples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vise {
    pub cadre: Option<usize>,
    pub de: (f32, f32),
    pub vers: (f32, f32),
    pub t0: f64,
}

/// L'azimut et la hauteur (degrés) d'où regarder le long de l'axe `axe` (0 : X, 1 : Y, 2 : Z), du côté `positif` : l'œil est sur
/// le bout cliqué et regarde vers le centre (comme la maquette, comme Blender). `azimut` : celui de la vue, gardé pour Y.
pub fn regard_d_axe(axe: usize, positif: bool, azimut: f32) -> (f32, f32) {
    match (axe, positif) {
        (0, true) => (90.0, 0.0),
        (0, false) => (-90.0, 0.0),
        (1, true) => (azimut, MAX_ELEVATION),
        (1, false) => (azimut, -MAX_ELEVATION),
        (_, true) => (0.0, 0.0),
        (_, false) => (180.0, 0.0),
    }
}

/// L'azimut `vers` ramené au tour le plus proche de `de` (la vue tourne par le plus court chemin).
pub fn azimut_proche(de: f32, vers: f32) -> f32 {
    let mut v = vers;
    while v - de > 180.0 {
        v -= 360.0;
    }
    while de - v > 180.0 {
        v += 360.0;
    }
    v
}

/// L'adoucissement de l'animation (début et fin lents).
pub fn adoucir(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// Les six bouts d'axe de la boussole d'une vue, centrée en `c` : (axe, positif, où à l'écran, profondeur : plus grand, plus loin).
pub fn bouts(view: &View, c: Pos2) -> Vec<(usize, bool, Pos2, f32)> {
    let mut out = Vec::with_capacity(6);
    for axe in 0..3 {
        for positif in [true, false] {
            let s = if positif { 1.0 } else { -1.0 };
            let x = view.right[axe] * s;
            let y = view.up[axe] * s;
            let z = view.forward[axe] * s;
            out.push((axe, positif, pos2(c.x + x * RAYON_BOUSSOLE, c.y - y * RAYON_BOUSSOLE), z));
        }
    }
    // (Le plus loin d'abord : ce qui vient vers nous passe devant.)
    out.sort_by(|a, b| b.3.total_cmp(&a.3));
    out
}

/// Où se pose la boussole d'une vue de rectangle `r` : en haut à droite, sous l'étiquette du nombre de cartes.
pub fn centre_boussole(r: Rect) -> Pos2 {
    pos2(r.right() - RAYON_BOUSSOLE - 18.0, r.top() + RAYON_BOUSSOLE + 30.0)
}

/// Ce que le sommaire « Scène 3D » range ensemble.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Groupe3d {
    Decor,
    Cameras,
    Lumieres,
    Cartes,
}

impl Groupe3d {
    pub const TOUS: [Groupe3d; 4] = [Groupe3d::Decor, Groupe3d::Cameras, Groupe3d::Lumieres, Groupe3d::Cartes];

    pub fn nom(self) -> &'static str {
        match self {
            Groupe3d::Decor => "Décor",
            Groupe3d::Cameras => "Caméras",
            Groupe3d::Lumieres => "Lumières",
            Groupe3d::Cartes => "Cartes du plan",
        }
    }
}

/// Une ligne du sommaire « Scène 3D ».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ligne3d {
    /// L'en-tête d'un groupe, et combien il a de lignes.
    Groupe(Groupe3d, usize),
    /// Un objet du décor de la séquence.
    Objet(u64),
    /// La caméra du plan `pi` (un plan de la séquence courante).
    Camera(usize),
    Soleil,
    Ciel,
    /// La carte du calque `li` du plan courant (`id` : pour vérifier qu'il y est toujours).
    Carte(usize, u64),
}

/// Le nom d'une ligne (ce que la recherche lit).
pub fn nom_de(doc: &crate::document::Document, cur_panel: usize, l: Ligne3d) -> String {
    match l {
        Ligne3d::Groupe(g, _) => g.nom().to_string(),
        Ligne3d::Objet(id) => doc.decor_du_plan(cur_panel).and_then(|d| d.objet(id)).map_or_else(String::new, |o| o.nom.clone()),
        Ligne3d::Camera(pi) => format!("Caméra · {}", crate::storyboard::panel_title(doc, pi)),
        Ligne3d::Soleil => "Soleil".into(),
        Ligne3d::Ciel => "Ciel".into(),
        Ligne3d::Carte(li, _) => doc.panels.get(cur_panel).and_then(|p| p.layers.get(li)).map_or_else(String::new, |l| l.name.clone()),
    }
}

/// Le sommaire de la séquence du plan `cur_panel` : le décor (ses objets), les caméras de ses plans, les lumières (le soleil, le
/// ciel), les cartes du plan courant (du haut de la pile vers le bas, comme la colonne de la timeline). `recherche` : seules les
/// lignes dont le nom la contient (sans casse) ; un groupe sans ligne trouvée n'est pas montré. `replies` : les groupes repliés.
pub fn sommaire(doc: &crate::document::Document, cur_panel: usize, recherche: &str, replies: &[Groupe3d]) -> Vec<Ligne3d> {
    let mut out = Vec::new();
    if doc.panels.is_empty() {
        return out;
    }
    let cur_panel = cur_panel.min(doc.panels.len() - 1);
    let cherche = recherche.trim().to_lowercase();
    let garde = |l: Ligne3d| cherche.is_empty() || nom_de(doc, cur_panel, l).to_lowercase().contains(&cherche);
    for g in Groupe3d::TOUS {
        let lignes: Vec<Ligne3d> = match g {
            Groupe3d::Decor => doc.decor_du_plan(cur_panel).map_or_else(Vec::new, |d| d.objets.iter().map(|o| Ligne3d::Objet(o.id)).collect()),
            Groupe3d::Cameras => {
                let (a, z) = doc.sequence_run(cur_panel);
                (a..z).map(Ligne3d::Camera).collect()
            }
            Groupe3d::Lumieres => vec![Ligne3d::Soleil, Ligne3d::Ciel],
            Groupe3d::Cartes => {
                let layers = &doc.panels[cur_panel].layers;
                (0..layers.len()).rev().filter(|&li| crate::espace3d::is_card(&layers[li])).map(|li| Ligne3d::Carte(li, layers[li].id)).collect()
            }
        };
        let lignes: Vec<Ligne3d> = lignes.into_iter().filter(|l| garde(*l)).collect();
        if lignes.is_empty() && !cherche.is_empty() {
            continue;
        }
        out.push(Ligne3d::Groupe(g, lignes.len()));
        if !replies.contains(&g) {
            out.extend(lignes);
        }
    }
    out
}

/// Ce que l'onglet 3D retient pendant la séance (pas dans le projet).
#[derive(Debug)]
pub(crate) struct Onglet3d {
    /// La vue Espace 3D était-elle ouverte avant d'entrer dans l'onglet (elle le redevient en sortant) ?
    pub vue_avant: Option<bool>,
    /// La vue qui tourne vers un axe de la boussole.
    pub vise: Option<Vise>,
    /// Les caméras des autres plans de la séquence, dans la vue.
    pub montrer_cameras: bool,
    /// Le soleil du décor, dans la vue.
    pub montrer_lumieres: bool,
    /// La recherche du sommaire « Scène 3D ».
    pub recherche: String,
    /// Les groupes repliés du sommaire.
    pub replies: Vec<Groupe3d>,
    /// Les caméras cachées dans la vue (identifiants des plans).
    pub cameras_cachees: Vec<u64>,
    /// L'onglet de Propriétés que ce qui est choisi a mis devant, à la dernière image (il ne le remet devant que s'il change).
    pub sujet: Option<Pane>,
    /// Les panneaux du groupe de la bande du bas et celui qui est devant, lus avant que les panneaux ne se peignent (ses onglets en tête).
    pub bande: (Vec<Pane>, Option<Pane>),
    /// Un onglet de la bande cliqué dans sa tête : il passe devant à l'image suivante.
    pub bande_demandee: Option<Pane>,
    /// Les bouts de la boussole peints à la dernière image, par vue (pour les essais) : le cadre, puis (axe, positif, centre).
    #[cfg(test)]
    pub boussoles: Vec<(Option<usize>, Vec<(usize, bool, Pos2)>)>,
}

impl Default for Onglet3d {
    fn default() -> Onglet3d {
        Onglet3d {
            vue_avant: None,
            vise: None,
            montrer_cameras: true,
            montrer_lumieres: true,
            recherche: String::new(),
            replies: Vec::new(),
            cameras_cachees: Vec::new(),
            sujet: None,
            bande: (Vec::new(), None),
            bande_demandee: None,
            #[cfg(test)]
            boussoles: Vec::new(),
        }
    }
}

impl App {
    /// On est dans l'onglet 3D.
    pub(crate) fn dans_l_onglet_3d(&self) -> bool {
        self.espace == Espace::TroisD
    }

    /// On entre dans l'onglet 3D (`set_espace`) : la vue Espace 3D s'ouvre en grand (on retient si elle l'était), le pointeur est
    /// aux outils de la vue (pas de dessin sur la carte), la caméra de la vue du dessin n'est pas celle du plan.
    pub(crate) fn o3_entrer(&mut self) {
        self.o3.vue_avant = Some(self.espace3d.on);
        if !self.espace3d.on {
            self.set_espace3d(true);
        }
        self.set_camera_view(false);
        self.e3_manipuler();
        self.o3.sujet = None;
    }

    /// On quitte l'onglet 3D : la vue Espace 3D redevient ce qu'elle était avant lui.
    pub(crate) fn o3_quitter(&mut self) {
        self.o3.vise = None;
        if let Some(avant) = self.o3.vue_avant.take() {
            if !avant && self.espace3d.on {
                self.set_espace3d(false);
            }
        }
    }

    /// À chaque image d'écran, dans l'onglet 3D, avant les panneaux : l'animation de la boussole, et l'onglet de Propriétés qui suit
    /// ce qui est choisi (la caméra : Caméra ; un objet ou une carte : Objet), seulement quand ce choix change.
    pub(crate) fn o3_suivre(&mut self) {
        if !self.espace3d.on {
            self.set_espace3d(true);
        }
        self.o3_avancer_vise();
        // La tête de la bande du bas : l'onglet cliqué passe devant ; puis ce qu'elle montre, pour ses boutons.
        if let Some(p) = self.o3.bande_demandee.take() {
            if let Some(state) = self.dock.as_mut() {
                if let Some(at) = state.find_tab(&p) {
                    state.set_active_tab(at);
                }
            }
        }
        self.o3.bande = self.o3_lire_la_bande();
        let sel = &self.espace3d.sel;
        let sujet = if sel.contains(&CAMERA_ID) {
            Some(Pane::Camera3d)
        } else if sel.iter().any(|&s| crate::poignees3d::objet_of(s).is_some() || crate::espace3d::card_of(s).is_some()) {
            Some(Pane::Objet3d)
        } else {
            None
        };
        if sujet != self.o3.sujet {
            self.o3.sujet = sujet;
            if let Some(p) = sujet {
                if self.colonne.contient(p) {
                    self.colonne.montrer(p);
                }
            }
        }
    }

    // ------------------------------------------------------------------------ les points de vue et les vues

    /// Le point de vue de la vue active : la vue seule (perspective, ou la caméra du plan) ; une vue agrandie ou la vue active des
    /// vues multiples, le sien.
    pub(crate) fn o3_point_actif(&self) -> PointDeVue {
        if self.e3_vues_multiples() {
            let v = &self.espace3d.vues;
            v.agrandie.or(v.active_montree()).map_or(PointDeVue::Perspective, |i| v.cadres[i].point)
        } else if self.espace3d.through_camera {
            PointDeVue::Camera
        } else {
            PointDeVue::Perspective
        }
    }

    /// Combien de vues on voit : 1 (la vue seule, ou une vue agrandie), 2 ou 4.
    pub(crate) fn o3_nombre_de_vues(&self) -> u8 {
        let v = &self.espace3d.vues;
        match v.disposition {
            Disposition::Une => 1,
            _ if v.agrandie.is_some() => 1,
            Disposition::CoteACote | Disposition::LUneSurLAutre => 2,
            Disposition::Quatre => 4,
        }
    }

    /// Le point de vue `p` pour la vue active. Une seule vue : la perspective et la caméra du plan sont la vue seule d'avant ; une vue
    /// orthographique (Dessus, Face, Côté) est la vue agrandie des quatre, à ce point de vue. Plusieurs : celui de la vue active.
    pub(crate) fn o3_choisir_point(&mut self, p: PointDeVue) {
        self.o3.vise = None;
        if self.o3_nombre_de_vues() > 1 {
            let i = self.espace3d.vues.active_montree().unwrap_or(self.espace3d.vues.disposition.cadres()[0]);
            self.e3_vues_point(i, p);
            return;
        }
        if p.est_ortho() {
            // (La vue seule devient la vue agrandie des quatre, son premier cadre à ce point de vue.)
            if self.espace3d.vues.disposition != Disposition::Quatre {
                self.e3_vues_poser(Disposition::Quatre);
            }
            let i = self.espace3d.vues.agrandie.unwrap_or(0);
            self.e3_vues_point(i, p);
            self.espace3d.vues.agrandie = Some(i);
            self.espace3d.vues.active = i;
        } else {
            if self.e3_vues_multiples() {
                // (Le cadre agrandi prend ce point de vue, puis la vue seule le reprend : `e3_vues_poser`.)
                let i = self.espace3d.vues.agrandie.unwrap_or(self.espace3d.vues.active);
                self.e3_vues_point(i, p);
                self.espace3d.vues.agrandie = None;
                self.espace3d.vues.active = i;
                self.e3_vues_poser(Disposition::Une);
            }
            self.espace3d.through_camera = p == PointDeVue::Camera;
        }
        self.status = format!("3D : vue « {} ».", POINTS.iter().find(|x| x.0 == p).map_or(p.nom(), |x| x.1));
    }

    /// Une, deux ou quatre vues : deux, la perspective et la caméra du plan ; quatre, dessus, perspective, face et la caméra du plan.
    pub(crate) fn o3_poser_vues(&mut self, n: u8) {
        self.o3.vise = None;
        match n {
            1 => {
                if self.espace3d.vues.agrandie.is_some() || self.e3_vues_multiples() {
                    // (La vue active devient la vue seule ; orthographique, elle reste agrandie.)
                    let i = self.espace3d.vues.agrandie.or(self.espace3d.vues.active_montree()).unwrap_or(0);
                    if self.espace3d.vues.cadres[i].point.est_ortho() {
                        if self.espace3d.vues.disposition != Disposition::Quatre {
                            let p = self.espace3d.vues.cadres[i].point;
                            self.e3_vues_poser(Disposition::Quatre);
                            self.e3_vues_point(0, p);
                            self.espace3d.vues.agrandie = Some(0);
                        } else {
                            self.espace3d.vues.agrandie = Some(i);
                        }
                    } else {
                        self.espace3d.vues.active = i;
                        self.espace3d.vues.agrandie = None;
                        self.e3_vues_poser(Disposition::Une);
                    }
                }
            }
            2 => {
                self.e3_vues_poser(Disposition::CoteACote);
                self.e3_vues_point(4, PointDeVue::Perspective);
                self.e3_vues_point(5, PointDeVue::Camera);
                self.espace3d.vues.active = 4;
            }
            _ => {
                self.e3_vues_poser(Disposition::Quatre);
                for (i, p) in QUATRE.into_iter().enumerate() {
                    self.e3_vues_point(i, p);
                }
                self.espace3d.vues.active = 1;
            }
        }
        self.status = format!("3D : {} vue{}.", self.o3_nombre_de_vues(), if self.o3_nombre_de_vues() > 1 { "s" } else { "" });
    }

    // ------------------------------------------------------------------------ la boussole

    /// La vue `cadre` (rien : la vue seule) tourne pour regarder le long de l'axe `axe`, du côté `positif`, en un quart de seconde.
    pub(crate) fn o3_viser(&mut self, cadre: Option<usize>, axe: usize, positif: bool) {
        let Some(orbite) = self.o3_orbite(cadre) else { return };
        let vers = regard_d_axe(axe, positif, orbite.azimuth);
        let vers = (azimut_proche(orbite.azimuth, vers.0), vers.1);
        let t0 = self.ctx.input(|i| i.time);
        self.o3.vise = Some(Vise { cadre, de: (orbite.azimuth, orbite.elevation), vers, t0 });
        let nom = ["X", "Y", "Z"][axe];
        self.status = format!("Vue le long de l'axe {nom} ({}).", if positif { "depuis le côté positif" } else { "depuis le côté négatif" });
        self.ctx.request_repaint();
    }

    /// L'orbite de la vue `cadre` (rien : la vue seule, si elle est en perspective).
    fn o3_orbite(&mut self, cadre: Option<usize>) -> Option<m::Orbite> {
        match cadre {
            None => {
                if self.espace3d.cam.is_none() {
                    self.espace3d_fit();
                }
                self.espace3d.cam
            }
            Some(i) => self.espace3d.vues.cadres.get(i).filter(|c| c.point == PointDeVue::Perspective).map(|c| c.orbite),
        }
    }

    /// L'animation de la boussole : l'orbite de la vue entre son départ et son arrivée ; une image de plus tant qu'elle dure.
    fn o3_avancer_vise(&mut self) {
        let Some(v) = self.o3.vise else { return };
        let now = self.ctx.input(|i| i.time);
        let k = ((now - v.t0) / DUREE_VISE).clamp(0.0, 1.0) as f32;
        let e = adoucir(k);
        let az = v.de.0 + (v.vers.0 - v.de.0) * e;
        let el = v.de.1 + (v.vers.1 - v.de.1) * e;
        match v.cadre {
            None => {
                if let Some(c) = self.espace3d.cam.as_mut() {
                    c.azimuth = az;
                    c.elevation = el;
                }
            }
            Some(i) => {
                if let Some(c) = self.espace3d.vues.cadres.get_mut(i) {
                    c.orbite.azimuth = az;
                    c.orbite.elevation = el;
                }
            }
        }
        if k >= 1.0 {
            self.o3.vise = None;
        } else {
            self.ctx.request_repaint();
        }
    }

    /// Peint la boussole de la vue `cadre` (rectangle `r`, prise de vue `view`) et prend ses clics : un bout d'axe tourne la vue vers
    /// lui. (Posée après la vue, elle prend le pointeur avant elle.)
    pub(crate) fn o3_boussole(&mut self, ui: &mut egui::Ui, r: Rect, view: &View, cadre: Option<usize>) {
        let c = centre_boussole(r);
        if !r.contains(c) || r.width() < 4.0 * RAYON_BOUSSOLE || r.height() < 4.0 * RAYON_BOUSSOLE {
            return;
        }
        let painter = ui.painter_at(r);
        let zone = Rect::from_center_size(c, vec2(2.0 * (RAYON_BOUSSOLE + RAYON_BOUT), 2.0 * (RAYON_BOUSSOLE + RAYON_BOUT)));
        let fond = ui.interact(zone, ui.id().with(("o3_boussole", cadre)), Sense::click());
        if fond.hovered() {
            painter.circle_filled(c, RAYON_BOUSSOLE + 10.0, Color32::from_white_alpha(14));
        }
        let b = bouts(view, c);
        let mut viser = None;
        for &(axe, positif, p, _) in &b {
            let col = COULEURS_AXES[axe];
            if positif {
                painter.line_segment([c, p], Stroke::new(2.0_f32, col));
                painter.circle_filled(p, RAYON_BOUT, col);
                painter.text(p, Align2::CENTER_CENTER, ["X", "Y", "Z"][axe], FontId::proportional(10.0), Color32::from_gray(17));
            } else {
                painter.circle(p, RAYON_BOUT - 2.0, Color32::from_rgba_unmultiplied(30, 30, 30, 215), Stroke::new(1.5_f32, col));
            }
            let bout = ui.interact(Rect::from_center_size(p, vec2(2.0 * RAYON_BOUT, 2.0 * RAYON_BOUT)), ui.id().with(("o3_bout", cadre, axe, positif)), Sense::click());
            let nom = format!("Regarder le long de {}{}", if positif { "+" } else { "−" }, ["X", "Y", "Z"][axe]);
            if crate::bulles::bulle(bout, &nom, "La vue tourne pour regarder le long de cet axe, depuis ce bout.", "").clicked() {
                viser = Some((axe, positif));
            }
        }
        crate::bulles::bulle(fond, "Boussole", "Un clic sur un bout d'axe (X rouge, Y vert, Z bleu) tourne la vue pour regarder le long de lui.", "");
        #[cfg(test)]
        self.o3.boussoles.push((cadre, b.iter().map(|x| (x.0, x.1, x.2)).collect()));
        if let Some((axe, positif)) = viser {
            self.o3_viser(cadre, axe, positif);
        }
    }

    // ------------------------------------------------------------------------ ce que la vue montre de plus

    /// Dans l'onglet 3D, ce que la vue montre en plus de celle d'aujourd'hui : les caméras des autres plans de la séquence (leur
    /// cadre, le nom du plan) et le soleil du décor (sa direction depuis le centre de la scène).
    pub(crate) fn o3_peindre_la_scene(&self, painter: &egui::Painter, proj: &dyn Fn(V3) -> Option<Pos2>, view: &View) {
        if self.o3.montrer_cameras {
            let (a, z) = self.doc.sequence_run(self.cur_panel);
            for pi in (a..z).filter(|&pi| pi != self.cur_panel) {
                if self.o3.cameras_cachees.contains(&self.doc.panels[pi].id) {
                    continue;
                }
                let world = crate::espace3d::Monde::du_plan(&self.doc, pi);
                let cam = world.scene_cam(&self.doc.panel_camera(pi, 0));
                let corners = self.e3_camera_corners(&cam, view);
                let col = crate::theme::camera().gamma_multiply(0.45);
                let line = |a: V3, b: V3| {
                    if let (Some(p), Some(q)) = (proj(a), proj(b)) {
                        painter.line_segment([p, q], Stroke::new(1.0_f32, col));
                    }
                };
                for k in 0..4 {
                    line(cam.pos, corners[k]);
                    line(corners[k], corners[(k + 1) % 4]);
                }
                if let Some(p) = proj(cam.pos) {
                    painter.circle_filled(p, 3.5, col);
                    painter.text(p + vec2(7.0, -3.0), Align2::LEFT_BOTTOM, crate::storyboard::panel_title(&self.doc, pi), FontId::proportional(10.5), col);
                }
            }
        }
        if self.o3.montrer_lumieres {
            if let Some(d) = self.doc.decor_du_plan(self.cur_panel) {
                let centre: V3 = [0.0, crate::espace3d::Monde::of(&self.doc).sol_y(), 0.0];
                let loin = m::add(centre, m::scale(d.aspect.soleil(), 4.0));
                if let (Some(p), Some(q)) = (proj(centre), proj(loin)) {
                    let jaune = Color32::from_rgb(250, 205, 80);
                    painter.extend(egui::Shape::dashed_line(&[p, q], Stroke::new(1.0_f32, jaune.gamma_multiply(0.7)), 5.0, 4.0));
                    painter.circle(q, 7.0, jaune, Stroke::new(1.0_f32, Color32::from_black_alpha(140)));
                    painter.text(q + vec2(10.0, 0.0), Align2::LEFT_CENTER, format!("{} Soleil", ph::SUN), FontId::proportional(11.0), jaune);
                }
            }
        }
    }

    /// La caméra du plan courant est cachée dans la vue (son œil, dans « Scène 3D »).
    pub(crate) fn o3_camera_cachee(&self) -> bool {
        self.dans_l_onglet_3d() && self.o3.cameras_cachees.contains(&self.doc.panels[self.cur_panel].id)
    }

    // ------------------------------------------------------------------------ la barre de la vue, les outils

    /// La barre au-dessus de la vue, dans l'onglet 3D : le point de vue, le nombre de vues, ce que la vue montre, Recadrer.
    pub(crate) fn o3_barre_de_vue(&mut self, ui: &mut egui::Ui) {
        let actif = self.o3_point_actif();
        for (p, nom) in POINTS {
            let on = actif == p;
            let r = ui.selectable_label(on, crate::bulles::encre_etat(egui::RichText::new(nom), on));
            let description = match p {
                PointDeVue::Perspective => "La vue libre : clic droit ou Alt + glisser la tourne, bouton du milieu la glisse, molette pour avancer. La boussole, en haut à droite, la met le long d'un axe.",
                PointDeVue::Camera => "Ce que filme la caméra du plan, au format du cadre : glisser ou tourner la vue déplace la caméra elle-même.",
                _ => "Une vue orthographique (sans perspective), le long d'un axe : elle se glisse et se zoome.",
            };
            if crate::bulles::bulle(r, &format!("Vue {nom}"), description, "").clicked() {
                self.o3_choisir_point(p);
            }
        }
        ui.separator();
        let n = self.o3_nombre_de_vues();
        for (k, action, description) in [
            (1u8, "espace3d.vues_une", "Une vue."),
            (2, "espace3d.vues_cote", "Deux vues côte à côte : la perspective et la caméra du plan."),
            (4, "espace3d.vues_quatre", "Quatre vues : dessus, perspective, face et la caméra du plan."),
        ] {
            let mot = k.to_string();
            if self.case_mot(ui, &mot, Some(n == k), &format!("{k} vue{}", if k > 1 { "s" } else { "" }), description, action).clicked() {
                self.o3_poser_vues(k);
            }
        }
        ui.separator();
        // (Partie 199 : l'aspect de la vue, Matières · Couleurs · Argile.)
        self.aspect_de_la_vue_ui(ui);
        ui.separator();
        let decor = self.doc.decor_du_plan(self.cur_panel).map(|d| d.aspect.ombres);
        let r = ui.add_enabled_ui(decor.is_some(), |ui| self.case_icone(ui, ph::CLOUD_SUN, Some(decor == Some(true)), "Ombres", "Les ombres du décor 3D et des cartes sous le soleil (selon le budget de l'ordinateur).", "")).inner;
        if r.clicked() {
            self.o3_basculer_ombres();
        }
        if self.case_icone(ui, ph::GRID_FOUR, Some(self.espace3d.show_grid), "Grille du sol", "Quadrillage au sol, un trait plus marqué tous les cinq.", "espace3d.grille").clicked() {
            self.e3_maillage_basculer();
        }
        if self.case_icone(ui, ph::CARDS, Some(self.espace3d.show_cards), "Cartes", "Les calques du plan courant, en cartes dans l'espace.", "").clicked() {
            self.espace3d.show_cards = !self.espace3d.show_cards;
        }
        if self.case_icone(ui, ph::VIDEO_CAMERA, Some(self.o3.montrer_cameras), "Caméras", "Les caméras des autres plans de la séquence, en plus de celle du plan courant.", "").clicked() {
            self.o3.montrer_cameras = !self.o3.montrer_cameras;
        }
        if self.case_icone(ui, ph::SUN, Some(self.o3.montrer_lumieres), "Lumières", "Le soleil du décor : sa direction depuis le centre de la scène.", "").clicked() {
            self.o3.montrer_lumieres = !self.o3.montrer_lumieres;
        }
        if self.case_icone(ui, ph::CORNERS_OUT, None, "Recadrer", "Tout l'espace dans la vue : le décor, les cartes et la caméra.", "espace3d.cadrer").clicked() {
            self.espace3d_fit();
        }
    }

    /// Les ombres du décor de la séquence, allumées ou éteintes (une étape d'annulation, la retouche du décor).
    pub(crate) fn o3_basculer_ombres(&mut self) {
        let seq = self.doc.sequence_of(self.cur_panel);
        let Some(mut d) = self.doc.decor_du_plan(self.cur_panel).cloned() else { return };
        d.aspect.ombres = !d.aspect.ombres;
        self.decor3d_retoucher(seq, d);
        self.decor3d_retouche_finir();
    }

    /// « Aimanter au sol » (barre d'outils de l'onglet 3D) : ce qui est choisi, une carte ou des objets du décor, repose sur le sol,
    /// ou ne l'est plus (une étape d'annulation).
    pub(crate) fn o3_basculer_aimant(&mut self) {
        let objets = self.e3_objets_choisis();
        if !objets.is_empty() {
            let seq = self.doc.sequence_of(self.cur_panel);
            let Some(mut d) = self.doc.decor_du_plan(self.cur_panel).cloned() else { return };
            let oui = !objets.iter().all(|id| d.objet(*id).is_some_and(|o| o.aimante));
            for id in &objets {
                if let Some(o) = d.objet_mut(*id) {
                    o.aimante = oui;
                }
            }
            self.decor3d_retoucher(seq, d);
            self.decor3d_retouche_finir();
            return;
        }
        let cartes: Vec<usize> = self.espace3d.sel.iter().filter_map(|&s| self.e3_card_layer(s)).collect();
        if let Some(&li) = cartes.first() {
            self.basculer_aimant(li);
        } else {
            self.status = "Aimanter au sol : choisis d'abord une carte ou un objet du décor.".into();
        }
    }

    /// Ce qui est choisi est aimanté au sol.
    pub(crate) fn o3_aimant_allume(&self) -> Option<bool> {
        let objets = self.e3_objets_choisis();
        if !objets.is_empty() {
            return Some(objets.iter().all(|id| self.e3_objet(*id).is_some_and(|o| o.aimante)));
        }
        let li = self.espace3d.sel.iter().find_map(|&s| self.e3_card_layer(s))?;
        Some(self.layers()[li].aimante_au_sol)
    }
}
