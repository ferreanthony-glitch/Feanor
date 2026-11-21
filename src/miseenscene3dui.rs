//! Partie 200 : la mise en scène 3D dans l'interface (`miseenscene3d.rs`, `docs/INTERFACE.md` partie 200).
//!
//! - **Les lampes** : « Ajouter une lampe » (outil de gauche de l'onglet 3D, et Propriétés > Lumières), au-dessus du point que la vue
//!   regarde ; dans « Scène 3D » sous Lumières (l'œil, enlever) ; dans la vue, leur marque se choisit et se déplace (l'outil Déplacer,
//!   les poignées de la partie 184) ; leurs réglages dans Propriétés > Lumières. Une étape d'annulation par geste.
//! - **Les clés d'un objet** : « ◆ Clé » en tête de la bande du bas et dans Propriétés > Objet (Animation) ; la rangée « Objet choisi »
//!   de la timeline de la séquence montre ses clés (un clic y mène, un clic droit l'enlève, un glisser la déplace dans le temps).
//!   Retoucher l'objet suit la règle des caméras de plan (`miseenscene3d::retoucher`), dans le chemin commun des retouches du décor
//!   (`decor3d_appliquer` : `places_retouchees`, `reporter_les_cles`). Avec des clés, sa place est celle de l'image courante
//!   (`decor3d_suivre_l_image`, à chaque image d'écran).
//! - **Caché dans le plan** : Propriétés > Objet, « Dans les plans ».
//! - **Nouveau plan depuis cette vue** : outil de gauche de l'onglet 3D.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, RichText, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::decor3d::Decor3d;
use crate::espace3d::Xf;
use crate::espace3dmath::{self as m, Orbite, Ray, View, V3};
use crate::espace3dvues::PointDeVue;
use crate::miseenscene3d::{self as ms, Lampe3d, MAX_LAMPES};

/// La marque des numéros de sélection des lampes (celle des objets est `poignees3d::OBJET_TAG`, des cartes `espace3d::CARD_TAG`).
pub const LAMPE_TAG: u64 = 1 << 60;
/// Le rayon (mètres) autour d'une lampe où un clic la prend, au moins ; plus loin de l'œil, un angle (`PRISE_ANGLE`, radians).
pub const PRISE_RAYON: f32 = 0.12;
pub const PRISE_ANGLE: f32 = 0.025;

pub fn lampe_selection(id: u64) -> u64 {
    LAMPE_TAG | id
}

pub fn lampe_of(selection: u64) -> Option<u64> {
    (selection != crate::espace3dgizmo::CAMERA_ID && selection & crate::espace3d::CARD_TAG == 0 && selection & crate::poignees3d::OBJET_TAG == 0 && selection & LAMPE_TAG != 0).then_some(selection & !LAMPE_TAG)
}

/// La focale des conventions des caméras de plan (les objectifs proposés, `camera::FOCAL_PRESETS`, sans l'anamorphique) la plus
/// proche de `focal`.
pub fn focale_proche(focal: f32) -> f32 {
    crate::camera::FOCAL_PRESETS.iter().filter(|p| !p.anamorphic).map(|p| p.mm).min_by(|a, b| (a - focal).abs().total_cmp(&(b - focal).abs())).unwrap_or(crate::camera::REF_FOCAL)
}

/// La caméra de la scène qui filme ce que montre la vue `view` (perspective) : sa place, son panoramique et sa bascule, sans rotation,
/// à la focale proposée la plus proche de `focal`.
pub fn camera_de_la_vue(view: &View, focal: f32) -> crate::espace3d::SceneCam {
    let f = m::normalize(view.forward);
    let tilt = f[1].clamp(-1.0, 1.0).asin().to_degrees();
    let pan = f[0].atan2(-f[2]).to_degrees();
    crate::espace3d::SceneCam { pos: view.eye, pan, tilt, roll: 0.0, focal: focale_proche(focal) }.sanitized()
}

/// Les objets animés du décor `d` dont la place a été retouchée par rapport au décor d'avant `avant` (pas par un changement de leur
/// origine, qui a déjà reporté la place à chacune de leurs clés : `Objet3d::changer_origine`).
pub fn places_retouchees(avant: Option<&Decor3d>, d: &Decor3d) -> Vec<u64> {
    let Some(avant) = avant else { return Vec::new() };
    d.objets.iter().filter(|o| o.anime()).filter(|o| avant.objet(o.id).is_some_and(|a| a.place != o.place && a.origine == o.origine)).map(|o| o.id).collect()
}

/// Après l'aimant : la place retouchée des objets `retouches` va dans leurs clés (la règle des caméras de plan, à l'image `image` de la
/// séquence) ; les autres objets animés reprennent la place de leurs clés à cette image (l'aimant ne change pas une clé).
pub fn reporter_les_cles(d: &mut Decor3d, retouches: &[u64], image: usize) {
    for o in d.objets.iter_mut().filter(|o| o.anime()) {
        if retouches.contains(&o.id) {
            ms::retoucher(&mut o.cles, image, o.place);
        } else {
            o.place = o.place_a(image);
        }
    }
}

impl App {
    /// L'image courante, dans la séquence du plan courant (les clés des objets du décor y sont posées).
    pub(crate) fn image_de_sequence_courante(&self) -> usize {
        self.doc.image_de_sequence(self.cur_panel, self.cur_frame)
    }

    /// À chaque image d'écran : les objets animés du décor de la séquence courante sont à la place de leurs clés à l'image courante
    /// (la vue, les poignées et Propriétés > Objet les lisent là). Rien à faire pour un décor immobile.
    pub(crate) fn decor3d_suivre_l_image(&mut self) {
        if self.doc.panels.is_empty() {
            return;
        }
        let seq = self.doc.env_cle(self.cur_panel);
        let anime = self.doc.decors3d.par_sequence.get(&seq).is_some_and(Decor3d::anime);
        if !anime {
            return;
        }
        let image = self.image_de_sequence_courante();
        if let Some(d) = self.doc.decors3d.par_sequence.get_mut(&seq) {
            for o in d.objets.iter_mut().filter(|o| o.anime()) {
                let p = o.place_a(image);
                if p != o.place {
                    o.place = p;
                }
            }
        }
    }

    /// Une étape d'annulation nommée `nom` sur le décor de la séquence courante : `f` le change (faux : rien n'a changé, pas d'étape).
    pub(crate) fn o3_etape_decor(&mut self, nom: &str, f: impl FnOnce(&mut Decor3d) -> bool) -> bool {
        let seq = self.doc.env_cle(self.cur_panel);
        let Some(mut d) = self.doc.decor_du_plan(self.cur_panel).cloned() else { return false };
        if !f(&mut d) {
            return false;
        }
        self.decor3d_retouche_finir();
        let snap = self.snapshot();
        self.decor3d_appliquer(seq, d);
        self.commit_named(snap, nom);
        true
    }

    // ------------------------------------------------------------------------ les lampes

    /// Le point que regarde la vue en perspective de l'onglet 3D (la vue seule, ou la perspective des vues multiples).
    pub(crate) fn o3_orbite_perspective(&self) -> Option<Orbite> {
        if self.e3_vues_multiples() {
            let v = &self.espace3d.vues;
            let montres = v.disposition.cadres();
            let active = v.agrandie.or(v.active_montree()).filter(|i| v.cadres[*i].point == PointDeVue::Perspective);
            if let Some(i) = active.or_else(|| montres.iter().copied().find(|i| v.cadres[*i].point == PointDeVue::Perspective)) {
                return Some(v.cadres[i].orbite);
            }
        }
        self.espace3d.cam
    }

    /// « Ajouter une lampe » : une lampe neuve au-dessus du point que regarde la vue, choisie ; Propriétés > Lumières passe devant.
    pub(crate) fn o3_ajouter_lampe(&mut self) {
        let Some(d) = self.doc.decor_du_plan(self.cur_panel) else {
            self.status = "Une lampe éclaire le décor 3D de la séquence : pose d'abord un objet 3D (la Banque 3D, en bas).".into();
            return;
        };
        if d.lampes.len() >= MAX_LAMPES {
            self.status = format!("Une séquence a au plus {MAX_LAMPES} lampes : enlèves-en une avant d'en ajouter une autre.");
            return;
        }
        let n = d.lampes.len() + 1;
        let cible = self.o3_orbite_perspective().map_or([0.0; 3], |o| o.target);
        let sol = crate::espace3d::Monde::of(&self.doc).sol_y();
        let id = self.doc.alloc();
        let lampe = Lampe3d::neuve(id, format!("Lampe {n}"), [cible[0], sol + ms::HAUTEUR, cible[2]]);
        if self.o3_etape_decor("Ajouter une lampe", |d| {
            d.lampes.push(lampe);
            true
        }) {
            self.espace3d.sel = vec![lampe_selection(id)];
            self.o3.montrer_lumieres = true;
            if self.colonne.contient(crate::dock::Pane::Lumieres3d) {
                self.colonne.montrer(crate::dock::Pane::Lumieres3d);
            }
            let seq = self.doc.sequence_label(self.doc.sequence_of(self.cur_panel));
            self.status = format!("Nouvelle lampe dans la séquence {seq} : règle-la dans Propriétés > Lumières, déplace-la dans la vue.");
        }
    }

    pub(crate) fn o3_lampe(&self, id: u64) -> Option<&Lampe3d> {
        self.doc.decor_du_plan(self.cur_panel)?.lampe(id)
    }

    /// Enlève la lampe `id` du décor (une étape).
    pub(crate) fn o3_enlever_lampe(&mut self, id: u64) {
        if self.o3_etape_decor("Enlever une lampe", |d| {
            let n = d.lampes.len();
            d.lampes.retain(|l| l.id != id);
            d.lampes.len() != n
        }) {
            let s = lampe_selection(id);
            self.espace3d.sel.retain(|x| *x != s);
            self.status = "Lampe enlevée du décor (Ctrl+Z la rend).".into();
        }
    }

    /// L'œil d'une lampe : éteinte ou allumée (une étape).
    pub(crate) fn o3_basculer_lampe(&mut self, id: u64) {
        let cache = self.o3_lampe(id).is_some_and(|l| l.cache);
        self.o3_etape_decor(if cache { "Allumer une lampe" } else { "Éteindre une lampe" }, |d| d.lampe_mut(id).map(|l| l.cache = !cache).is_some());
    }

    /// Les lampes choisies dans la vue.
    pub(crate) fn o3_lampes_choisies(&self) -> Vec<u64> {
        self.espace3d.sel.iter().filter_map(|&s| lampe_of(s)).filter(|&id| self.o3_lampe(id).is_some()).collect()
    }

    /// La lampe montrée la plus proche que touche le rayon (dans l'onglet 3D, quand la vue montre les lumières).
    pub(crate) fn e3_pick_lampe(&self, ray: &Ray) -> Option<(f32, u64)> {
        if !self.dans_l_onglet_3d() || !self.o3.montrer_lumieres {
            return None;
        }
        let d = self.doc.decor_du_plan(self.cur_panel)?;
        d.lampes
            .iter()
            .filter_map(|l| {
                let v = m::sub(l.position, ray.o);
                let t = m::dot(v, ray.d);
                if t <= 0.0 {
                    return None;
                }
                let ecart = m::length(m::sub(v, m::scale(ray.d, t)));
                (ecart <= PRISE_RAYON.max(PRISE_ANGLE * t)).then_some((t, l.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// Le placement d'une lampe pour le manipulateur : sa place seule.
    pub(crate) fn o3_xf_de_la_lampe(&self, id: u64) -> Option<Xf> {
        self.o3_lampe(id).map(|l| Xf { pos: l.position, rot: [0.0; 3], size: [1.0; 3] })
    }

    /// Pose les places des lampes de `new` (le geste du manipulateur, qui a son étape d'annulation).
    pub(crate) fn o3_poser_lampes(&mut self, new: &[(u64, Xf)]) {
        let lampes: Vec<(u64, V3)> = new.iter().filter_map(|(s, xf)| lampe_of(*s).map(|id| (id, xf.pos))).filter(|(_, p)| p.iter().all(|v| v.is_finite())).collect();
        if lampes.is_empty() {
            return;
        }
        let seq = self.doc.env_cle(self.cur_panel);
        if let Some(d) = self.doc.decors3d.par_sequence.get_mut(&seq) {
            for (id, p) in lampes {
                if let Some(l) = d.lampe_mut(id) {
                    l.position = p;
                }
            }
            self.modified = true;
        }
    }

    /// Les lampes dans la vue (onglet 3D, « Lumières ») : une marque à leur place, leur nom ; la choisie à la couleur de la sélection,
    /// une éteinte en gris ; un trait jusqu'au sol, pour voir sa hauteur.
    pub(crate) fn o3_peindre_lampes(&self, painter: &egui::Painter, proj: &dyn Fn(V3) -> Option<Pos2>) {
        let Some(d) = self.doc.decor_du_plan(self.cur_panel) else { return };
        let choisies = self.o3_lampes_choisies();
        let sol = crate::espace3d::Monde::of(&self.doc).sol_y();
        for l in &d.lampes {
            let Some(p) = proj(l.position) else { continue };
            let couleur = if choisies.contains(&l.id) {
                crate::theme::POIGNEE_SELECTION
            } else if l.cache {
                Color32::from_gray(120)
            } else {
                crate::couleurs3d::color32(l.couleur)
            };
            if let Some(q) = proj([l.position[0], sol, l.position[2]]) {
                painter.extend(egui::Shape::dashed_line(&[p, q], Stroke::new(1.0_f32, couleur.gamma_multiply(0.5)), 4.0, 4.0));
            }
            let r = 7.0;
            painter.add(egui::Shape::closed_line(vec![p + vec2(0.0, -r), p + vec2(r, 0.0), p + vec2(0.0, r), p + vec2(-r, 0.0)], Stroke::new(1.5_f32, couleur)));
            painter.line_segment([p + vec2(-r * 0.5, -r * 0.5), p + vec2(r * 0.5, r * 0.5)], Stroke::new(1.0_f32, couleur.gamma_multiply(0.7)));
            painter.line_segment([p + vec2(r * 0.5, -r * 0.5), p + vec2(-r * 0.5, r * 0.5)], Stroke::new(1.0_f32, couleur.gamma_multiply(0.7)));
            painter.text(p + vec2(10.0, -2.0), Align2::LEFT_BOTTOM, format!("{} {}", ph::LIGHTBULB, l.nom), FontId::proportional(11.0), couleur);
        }
    }

    // ------------------------------------------------------------------------ les clés d'un objet

    /// L'objet du décor dont la timeline de la séquence montre les clés : le premier choisi dans la vue.
    pub(crate) fn o3_objet_des_cles(&self) -> Option<u64> {
        self.e3_objets_choisis().first().copied()
    }

    /// « ◆ Clé » : la place de l'objet `id` à l'image courante devient (ou remplace) sa clé de cette image (une étape).
    pub(crate) fn o3_poser_cle(&mut self, id: u64) {
        let image = self.image_de_sequence_courante();
        let nom = self.e3_objet(id).map(|o| o.nom.clone()).unwrap_or_default();
        if self.o3_etape_decor("Clé d'un objet du décor", |d| {
            let Some(o) = d.objet_mut(id) else { return false };
            let place = o.place_a(image);
            ms::poser_cle(&mut o.cles, image, place);
            o.place = place;
            true
        }) {
            let n = self.e3_objet(id).map_or(0, |o| o.cles.len());
            self.status = format!("Clé posée à l'image {} sur « {nom} » ({n} clé{}).", image + 1, if n > 1 { "s" } else { "" });
        }
    }

    /// « Effacer les clés » : l'objet redevient immobile, à sa place de l'image courante (une étape).
    pub(crate) fn o3_effacer_cles(&mut self, id: u64) {
        let image = self.image_de_sequence_courante();
        if self.o3_etape_decor("Effacer les clés", |d| {
            let Some(o) = d.objet_mut(id).filter(|o| o.anime()) else { return false };
            o.place = o.place_a(image);
            o.cles.clear();
            true
        }) {
            self.status = "Clés effacées : l'objet est immobile, à sa place de l'image courante.".into();
        }
    }

    /// Enlève la clé de l'image `image` de l'objet `id` (une étape) ; sans clé, l'objet reste à sa place de l'image courante.
    pub(crate) fn o3_enlever_cle(&mut self, id: u64, image: usize) {
        let courante = self.image_de_sequence_courante();
        if self.o3_etape_decor("Enlever une clé", |d| {
            let Some(o) = d.objet_mut(id) else { return false };
            let ici = o.place_a(courante);
            let n = o.cles.len();
            o.cles.retain(|k| k.image != image);
            if o.cles.len() == n {
                return false;
            }
            o.place = if o.cles.is_empty() { ici } else { o.place_a(courante) };
            true
        }) {
            self.status = format!("Clé de l'image {} enlevée.", image + 1);
        }
    }

    /// Le glisser d'une clé dans la timeline : la clé de l'image `de` de l'objet `id` passe à l'image `vers` (pas sur une autre clé) ;
    /// sans étape (le geste a la sienne, `o3_cle_lachee`). Rend l'image où elle est.
    pub(crate) fn o3_deplacer_cle(&mut self, id: u64, de: usize, vers: usize) -> usize {
        let seq = self.doc.env_cle(self.cur_panel);
        let courante = self.image_de_sequence_courante();
        let Some(o) = self.doc.decors3d.par_sequence.get_mut(&seq).and_then(|d| d.objet_mut(id)) else { return de };
        if de == vers || o.cles.iter().any(|k| k.image == vers) {
            return de;
        }
        let Some(i) = o.cles.iter().position(|k| k.image == de) else { return de };
        let k = o.cles.remove(i);
        ms::poser_cle(&mut o.cles, vers, k.place);
        o.place = o.place_a(courante);
        self.modified = true;
        vers
    }

    /// Le glisser d'une clé lâché : une étape « Déplacer une clé » si elle a bougé.
    pub(crate) fn o3_cle_lachee(&mut self) {
        if let Some(g) = self.o3.cle_tiree.take() {
            if g.ici != g.de {
                self.commit_named(g.snap, "Déplacer une clé");
                self.status = format!("Clé déplacée à l'image {}.", g.ici + 1);
            }
        }
    }

    /// Va à l'image `image` de la séquence courante (son plan, son image).
    pub(crate) fn o3_aller_a_l_image(&mut self, image: usize) {
        let (pi, f) = self.doc.plan_de_l_image(self.cur_panel, image);
        let g = self.doc.panel_start(pi) + f;
        self.playing = false;
        // (Les objets et les lampes choisis restent choisis : ils sont à la séquence, pas au plan ; la vue ne choisit pas la carte du
        // calque courant du plan où l'on arrive.)
        let garder: Vec<u64> = self.espace3d.sel.iter().copied().filter(|&s| crate::poignees3d::objet_of(s).is_some() || lampe_of(s).is_some()).collect();
        if g != self.global_frame() {
            self.goto_global(g);
        }
        if !garder.is_empty() {
            self.espace3d.sel = garder;
            self.espace3d.last_layer = self.layer().id;
        }
    }

    // ------------------------------------------------------------------------ caché dans le plan

    /// « Caché dans le plan » : l'objet `id` n'est plus dans l'image du plan courant (`oui`), ou y revient (une étape).
    pub(crate) fn o3_cacher_dans_le_plan(&mut self, id: u64, oui: bool) {
        let plan = self.doc.panels[self.cur_panel].id;
        let titre = crate::storyboard::panel_title(&self.doc, self.cur_panel);
        let nom = self.e3_objet(id).map(|o| o.nom.clone()).unwrap_or_default();
        if self.o3_etape_decor(if oui { "Cacher un objet dans le plan" } else { "Montrer un objet dans le plan" }, |d| {
            let Some(o) = d.objet_mut(id) else { return false };
            if o.cache_dans(plan) == oui {
                return false;
            }
            if oui {
                o.caches_dans.push(plan);
            } else {
                o.caches_dans.retain(|p| *p != plan);
            }
            true
        }) {
            self.status = if oui { format!("« {nom} » n'est plus dans l'image du {}, il reste dans les autres plans.", titre.to_lowercase()) } else { format!("« {nom} » revient dans l'image du {}.", titre.to_lowercase()) };
        }
    }

    // ------------------------------------------------------------------------ nouveau plan depuis la vue

    /// « Nouveau plan depuis cette vue » : un plan neuf à la fin de la séquence courante (le chemin d'un nouveau plan, sa durée celle
    /// des Préférences), sa caméra à la place et dans la direction de la vue en perspective de l'onglet 3D, à la focale proposée la plus
    /// proche ; on y va, sa caméra choisie. Une étape d'annulation. Une vue orthographique, ou « Caméra du plan » : la perspective de
    /// l'onglet (la vue libre).
    pub(crate) fn o3_nouveau_plan_depuis_la_vue(&mut self) {
        if self.doc.panels.is_empty() {
            return;
        }
        if self.espace3d.cam.is_none() {
            self.espace3d_fit();
        }
        let Some(orbite) = self.o3_orbite_perspective() else { return };
        let sc = camera_de_la_vue(&orbite.view(1000.0), orbite.focal);
        let (_, z) = self.doc.sequence_run(self.cur_panel);
        let devant = self.settings.calque_devant_la_camera;
        let avant = self.history.context.replace("Nouveau plan depuis la vue".into());
        let fait = self.structure_op(|d, _| {
            let pi = d.add_scene_after(z - 1);
            let monde = crate::espace3d::Monde::du_plan(d, pi);
            let cam = monde.camera_from_scene(&sc);
            let libre = !d.camera_filmee_par_la_feuille(&cam);
            let p = &mut d.panels[pi];
            p.camera = vec![crate::camera::CameraKey::new(0, cam)];
            p.camera_libre = libre;
            // (Partie 201 : son premier calque, devant sa caméra, si elle a quitté la feuille.)
            if devant {
                for li in 0..d.panels[pi].layers.len() {
                    crate::calquedevant::poser_devant(d, pi, li, 0);
                }
            }
            Some(pi)
        });
        self.history.context = avant;
        if fait {
            self.renderer.invalidate_all();
            self.e3_choisir_la_camera(false);
            self.status = format!("Nouveau plan {}, filmé depuis cette vue ({} mm).", crate::storyboard::panel_title(&self.doc, self.cur_panel), sc.focal);
        }
    }

    // ------------------------------------------------------------------------ l'interface

    /// Les outils de l'onglet 3D de la partie 200 (barre de gauche) : « Ajouter une lampe », « Nouveau plan depuis cette vue ».
    pub(crate) fn o3_outils_mise_en_scene(&mut self, ui: &mut egui::Ui) {
        let decor = self.doc.decor_du_plan(self.cur_panel).map(|d| d.lampes.len());
        let possible = decor.is_some_and(|n| n < MAX_LAMPES);
        let pourquoi = match decor {
            None => "Ajouter une lampe : la séquence n'a pas encore de décor 3D (pose d'abord un objet de la Banque 3D).".to_string(),
            Some(_) => format!("Ajouter une lampe : la séquence a déjà ses {MAX_LAMPES} lampes."),
        };
        let r = ui.add_enabled_ui(possible, |ui| self.case_icone(ui, ph::LIGHTBULB, None, "Ajouter une lampe", "Une lampe ponctuelle au-dessus du point que regarde la vue, à régler dans Propriétés > Lumières.", "")).inner;
        let r = r.on_disabled_hover_text(pourquoi);
        if r.clicked() {
            self.o3_ajouter_lampe();
        }
        let r = self.case_icone(ui, ph::FILM_SLATE, None, "Nouveau plan depuis cette vue", "Un plan neuf à la fin de la séquence : sa caméra filme ce que montre la vue en perspective (une vue orthographique ou « Caméra du plan » : la perspective de l'onglet).", "");
        if r.clicked() {
            self.o3_nouveau_plan_depuis_la_vue();
        }
    }

    /// Propriétés > Lumières, sous le soleil et le ciel : chaque lampe (place, intensité, portée, teinte, enlever), puis « Ajouter une
    /// lampe ». La lampe choisie dans la vue est ouverte.
    pub(crate) fn o3_lampes_ui(&mut self, ui: &mut egui::Ui) {
        let Some(d) = self.doc.decor_du_plan(self.cur_panel) else { return };
        let seq = self.doc.env_cle(self.cur_panel);
        let mut nd = d.clone();
        let avant = d.lampes.clone();
        let choisies = self.o3_lampes_choisies();
        let mut enlever = None;
        ui.separator();
        for l in nd.lampes.iter_mut() {
            let ouverte = choisies.contains(&l.id);
            egui::CollapsingHeader::new(RichText::new(format!("{} {}", ph::LIGHTBULB, l.nom)).strong()).id_salt(("lampe3d", l.id)).default_open(ouverte).open(ouverte.then_some(true)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Nom");
                    ui.add(egui::TextEdit::singleline(&mut l.nom).desired_width(f32::INFINITY));
                });
                egui::Grid::new(("lampe3d_grille", l.id)).num_columns(2).spacing([8.0, 2.0]).show(ui, |ui| {
                    ui.label("Position (m)");
                    ui.horizontal(|ui| {
                        for v in l.position.iter_mut() {
                            ui.add(egui::DragValue::new(v).speed(0.02).max_decimals(2));
                        }
                    });
                    ui.end_row();
                    ui.label("Intensité");
                    let r = ui.add(egui::Slider::new(&mut l.intensite, 0.0..=ms::INTENSITE_MAX).step_by(0.05));
                    crate::bulles::bulle(r, &format!("Intensité de « {} »", l.nom), "La force de la lampe : 0, éteinte ; 1, à peu près celle du soleil tout près d'elle.", "");
                    ui.end_row();
                    ui.label("Portée");
                    let r = ui.add(egui::Slider::new(&mut l.portee, ms::PORTEE_MIN..=ms::PORTEE_MAX).suffix(" m").step_by(0.5));
                    crate::bulles::bulle(r, &format!("Portée de « {} »", l.nom), "La lampe n'éclaire rien plus loin, en s'adoucissant jusque-là.", "");
                    ui.end_row();
                    ui.label("Teinte");
                    ui.horizontal_wrapped(|ui| {
                        for (nom, c) in ms::TEINTES {
                            let on = l.couleur == c;
                            let r = ui.selectable_label(on, crate::bulles::encre_etat(RichText::new(nom), on));
                            if crate::bulles::bulle(r, &format!("Teinte : {nom}"), "La couleur de la lumière de la lampe.", "").clicked() {
                                l.couleur = c;
                            }
                        }
                        let r = crate::champcouleur::srgb(ui, &mut l.couleur);
                        crate::bulles::bulle(r, "Autre teinte", "Une autre couleur pour la lumière de la lampe.", "");
                    });
                    ui.end_row();
                });
                ui.label(RichText::new("Une lampe ne porte pas d'ombres et n'éclaire que le décor 3D (pas les cartes).").small().weak());
                let r = ui.button(format!("{}  Enlever la lampe", ph::TRASH));
                if crate::bulles::bulle(r, &format!("Enlever « {} »", l.nom), "Enlève la lampe du décor (Ctrl+Z la rend).", "").clicked() {
                    enlever = Some(l.id);
                }
            });
        }
        let plein = nd.lampes.len() >= MAX_LAMPES;
        let r = ui.add_enabled(!plein, egui::Button::new(format!("{}  Ajouter une lampe", ph::LIGHTBULB)));
        let r = crate::bulles::bulle(r, "Ajouter une lampe (Lumières)", "Une lampe ponctuelle au-dessus du point que regarde la vue.", "").on_disabled_hover_text(format!("La séquence a déjà ses {MAX_LAMPES} lampes."));
        let ajouter = r.clicked();
        if let Some(id) = enlever {
            self.o3_enlever_lampe(id);
            return;
        }
        if ajouter {
            self.o3_ajouter_lampe();
            return;
        }
        let lampes: Vec<Lampe3d> = nd.lampes.into_iter().map(Lampe3d::bornee).collect();
        if lampes != avant {
            if let Some(mut d) = self.doc.decor_du_plan(self.cur_panel).cloned() {
                d.lampes = lampes;
                self.decor3d_retoucher(seq, d);
            }
        }
        let tenu = ui.input(|i| i.pointer.any_down()) || ui.ctx().memory(|mem| mem.focused().is_some());
        if !tenu {
            self.decor3d_retouche_finir();
        }
    }

    /// Propriétés > Objet, sous les champs de l'objet : « Dans les plans » (caché dans le plan courant) et « Animation » (ses clés).
    pub(crate) fn o3_objet_mise_en_scene_ui(&mut self, ui: &mut egui::Ui, id: u64) {
        let Some(o) = self.e3_objet(id) else { return };
        let (nom, cache, cles): (String, bool, Vec<usize>) = (o.nom.clone(), o.cache_dans(self.doc.panels[self.cur_panel].id), o.cles.iter().map(|k| k.image).collect());
        let titre = crate::storyboard::panel_title(&self.doc, self.cur_panel);
        ui.separator();
        ui.label(RichText::new("Dans les plans").strong());
        let mut c = cache;
        let r = ui.checkbox(&mut c, format!("Caché dans le {}", titre.to_lowercase()));
        if crate::bulles::bulle(r, "Caché dans le plan", "Pour tricher un plan : l'objet n'est plus dans l'image de ce plan (Storyboard, Montage, vignettes, export, « Caméra du plan ») ; il reste dans le décor de la séquence et dans les autres plans.", "").clicked() {
            self.o3_cacher_dans_le_plan(id, c);
        }
        ui.label(RichText::new("L'objet reste dans le décor de la séquence ; la vue le montre en pointillés dans ce plan.").small().weak());
        ui.separator();
        ui.label(RichText::new("Animation").strong());
        let image = self.image_de_sequence_courante();
        let texte = match cles.len() {
            0 => "Pas de clé : l'objet est immobile dans toute la séquence.".to_string(),
            1 => format!("1 clé : image {}. Déplacer l'objet règle cette clé.", cles[0] + 1),
            n => format!("{n} clés : images {}. Déplacer l'objet règle la clé de l'image courante.", cles.iter().map(|k| (k + 1).to_string()).collect::<Vec<_>>().join(", ")),
        };
        ui.label(RichText::new(texte).small().weak());
        ui.horizontal_wrapped(|ui| {
            // (Partie 201.6 : le losange des clés de la timeline, `ph::DIAMOND` ; « ◆ » n'est pas dans les polices de FEANOR.)
            let r = ui.button(format!("{} Clé à l'image {}", ph::DIAMOND, image + 1));
            if crate::bulles::bulle(r, "Clé à l'image courante", &format!("Retient la place de « {nom} » à l'image courante."), "").clicked() {
                self.o3_poser_cle(id);
            }
            if !cles.is_empty() {
                let r = ui.button("Effacer les clés");
                if crate::bulles::bulle(r, "Effacer les clés", "L'objet redevient immobile, à sa place de l'image courante.", "").clicked() {
                    self.o3_effacer_cles(id);
                }
            }
        });
    }

    /// « ◆ Clé » en tête de la bande du bas (la ligne de la timeline de la séquence) : pour l'objet choisi.
    pub(crate) fn o3_bouton_cle(&mut self, ui: &mut egui::Ui) {
        let objet = self.o3_objet_des_cles();
        let nom = objet.and_then(|id| self.e3_objet(id)).map(|o| o.nom.clone());
        let texte = nom.as_ref().map_or(format!("{} Clé (choisis un objet)", ph::DIAMOND), |n| format!("{} Clé sur {n}", ph::DIAMOND));
        let r = ui.add_enabled(objet.is_some(), egui::Button::new(texte));
        let r = crate::bulles::bulle(r, "Poser une clé", "Retient la place de l'objet du décor choisi à l'image courante.", "");
        if r.clicked() {
            if let Some(id) = objet {
                self.o3_poser_cle(id);
            }
        }
    }

    /// La rangée « Objet choisi » de la timeline de la séquence : les clés de l'objet choisi en losanges (le segment animé entre la
    /// première et la dernière). Un clic y mène ; un clic droit l'enlève ; un glisser la déplace dans le temps.
    pub(crate) fn o3_rangee_des_cles(&mut self, ui: &mut egui::Ui, rangee: egui::Rect, piste: egui::Rect, fen: crate::timelineseq::Fenetre) {
        let p = ui.painter_at(rangee.union(piste));
        let p_piste = ui.painter_at(piste);
        let doux = ui.visuals().weak_text_color();
        let Some(id) = self.o3_objet_des_cles() else {
            p.text(pos2(piste.left() + 6.0, rangee.center().y), Align2::LEFT_CENTER, "Choisis un objet du décor pour voir ou poser ses clés.", FontId::proportional(10.0), doux.gamma_multiply(0.8));
            #[cfg(test)]
            crate::onglet3dui::CLES_SEQ.with(|c| c.borrow_mut().clear());
            return;
        };
        let (a, _) = self.doc.sequence_run(self.cur_panel);
        let base = self.doc.panel_start(a);
        // (Partie 201 : à travers la fenêtre de la timeline, zoomée et défilée.)
        let x_de = |image: usize| fen.x_de(base + image);
        let image_de = |x: f32| (fen.f_de(x + fen.points_par_image() * 0.5)).saturating_sub(base);
        let nom = self.e3_objet(id).map(|o| o.nom.clone()).unwrap_or_default();
        let cles: Vec<usize> = self.e3_objet(id).map(|o| o.cles.iter().map(|k| k.image).collect()).unwrap_or_default();
        p.text(pos2(rangee.left() + 6.0, rangee.center().y), Align2::LEFT_CENTER, &nom, FontId::proportional(11.0), ui.visuals().strong_text_color());
        if cles.is_empty() {
            p.text(pos2(piste.left() + 6.0, rangee.center().y), Align2::LEFT_CENTER, format!("Pas de clé : « {} Clé » en haut retient sa place à l'image courante.", ph::DIAMOND), FontId::proportional(10.0), doux.gamma_multiply(0.8));
        }
        if cles.len() > 1 {
            let (x0, x1) = (x_de(cles[0]), x_de(*cles.last().unwrap_or(&0)));
            p_piste.rect_filled(egui::Rect::from_min_max(pos2(x0, rangee.center().y - 2.0), pos2(x1, rangee.center().y + 2.0)), 1.0, crate::theme::accent().gamma_multiply(0.35));
        }
        let courante = self.image_de_sequence_courante();
        #[cfg(test)]
        let mut vus = Vec::new();
        let mut action: Option<(usize, u8, f32)> = None;
        for &k in &cles {
            let c = pos2(x_de(k) + 0.5, rangee.center().y);
            // (Partie 201 : une clé hors de la partie de la séquence qu'on voit n'est ni peinte ni prise, sauf celle qu'on glisse.)
            if !(piste.left() - 6.0..=piste.right() + 6.0).contains(&c.x) && self.o3.cle_tiree.as_ref().is_none_or(|g| g.ici != k) {
                continue;
            }
            let zone = egui::Rect::from_center_size(c, vec2(12.0, 14.0));
            // (Pendant qu'on la glisse, la clé garde le numéro de l'image d'où elle part : egui suit le même geste.)
            let wid = match &self.o3.cle_tiree {
                Some(g) if g.objet == id && g.ici == k => g.de,
                _ => k,
            };
            let r = ui.interact(zone, ui.id().with(("cle_objet", id, wid)), egui::Sense::click_and_drag());
            let ici = k == courante;
            let couleur = if ici || r.hovered() { crate::theme::POIGNEE_SELECTION } else { Color32::from_rgb(230, 200, 120) };
            p_piste.add(egui::Shape::convex_polygon(vec![c + vec2(0.0, -5.5), c + vec2(5.5, 0.0), c + vec2(0.0, 5.5), c + vec2(-5.5, 0.0)], couleur, Stroke::new(1.0_f32, Color32::from_black_alpha(140))));
            #[cfg(test)]
            vus.push((k, zone));
            let r = crate::bulles::bulle(r, &format!("Clé de {nom}, image {}", k + 1), "Un clic : y aller. Clic droit : enlever cette clé. Glisser : la déplacer dans le temps.", "");
            if r.drag_started() {
                action = Some((k, 0, 0.0));
            } else if r.dragged() {
                if let Some(pos) = r.interact_pointer_pos() {
                    action = Some((k, 1, pos.x));
                }
            } else if r.drag_stopped() {
                action = Some((k, 2, 0.0));
            } else if r.secondary_clicked() {
                action = Some((k, 3, 0.0));
            } else if r.clicked() {
                action = Some((k, 4, 0.0));
            }
        }
        #[cfg(test)]
        crate::onglet3dui::CLES_SEQ.with(|c| *c.borrow_mut() = vus);
        match action {
            Some((k, 0, _)) => {
                self.o3.cle_tiree = Some(crate::onglet3d::CleTiree { objet: id, de: k, ici: k, snap: self.snapshot() });
            }
            Some((_, 1, x)) => {
                if let Some(g) = self.o3.cle_tiree.as_ref().filter(|g| g.objet == id).map(|g| g.ici) {
                    let vers = image_de(x);
                    let ici = self.o3_deplacer_cle(id, g, vers);
                    if let Some(t) = self.o3.cle_tiree.as_mut() {
                        t.ici = ici;
                    }
                }
            }
            Some((_, 2, _)) => self.o3_cle_lachee(),
            Some((k, 3, _)) => self.o3_enlever_cle(id, k),
            Some((k, 4, _)) => self.o3_aller_a_l_image(k),
            _ => {}
        }
    }
}
