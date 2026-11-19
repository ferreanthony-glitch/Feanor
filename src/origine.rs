//! **L'origine d'un calque** (`docs/INTERFACE.md`, partie 182) : un point du dessin du calque, que l'on place, et qui devient
//! son vrai pivot. Sa pose (fixe, ou de chaque clé de position) se déplace, s'agrandit, s'étire et tourne autour de lui :
//! entre deux clés de rotation, une tête tourne autour du cou. Sans origine (`Layer::origine` = rien), le pivot est le
//! centre de la feuille, exactement comme avant.
//!
//! - **Déplacer l'origine ne déplace jamais le dessin** (`deplacer_origine`) : la pose fixe et chaque clé sont recalculées
//!   pour garder la même image, sur la feuille (une affinité : xy2 = xy1 + A·Δ − Δ) comme dans l'Espace 3D (une carte
//!   inclinée : son placement 3D, inclinaison et profondeur comprises, reste le même) ; le chemin d'une trajectoire dont
//!   l'origine suit le chemin (`Ancre::Origine`) se déplace avec elle.
//! - Les **origines toutes faites** (`Preset`) se lisent sur la boîte du contenu du dessin montré à l'image courante.
//! - **Aimanté au sol** (`aimanter_au_sol`) : l'origine de la carte repose sur le sol de l'Espace 3D, à la pose fixe et à
//!   chaque clé. Le sol se lit par une seule fonction, `hauteur_du_sol_sous`. Refait après chaque modification, jamais
//!   à la lecture d'un fichier ni à chaque image de la lecture. Décocher garde les hauteurs que l'aimant a données.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, Stroke, Vec2};

use crate::app::App;
use crate::document::{Layer, LayerTransform};
use crate::espace3d::Monde;
use crate::trajectoire::Ancre;

/// Le nom de l'option d'aimant, partout le même.
pub(crate) const AIMANT: &str = "Aimanté au sol";
/// Sa phrase (bulles).
pub(crate) const AIMANT_PHRASE: &str = "L'origine du calque repose sur le sol de l'Espace 3D, à sa pose fixe et à chacune de ses clés. Le réglage de hauteur est alors tenu par l'aimant. Décoché, le calque garde la hauteur où l'aimant l'a posé.";
/// La commande de l'aimant (palette, menus, raccourci à assigner).
pub(crate) const AIMANT_ACTION: &str = "calque.aimante_au_sol";

/// Les origines qu'un menu propose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    CentreDessin,
    BasCentre,
    HautGauche,
    HautDroite,
    BasGauche,
    BasDroite,
    /// Le point de la feuille sous le dernier clic (ou le pointeur), ramené dans le dessin.
    Ici,
    /// Plus d'origine : le centre de la feuille, comme avant.
    Feuille,
}

impl Preset {
    pub const ALL: [Preset; 8] = [Preset::CentreDessin, Preset::BasCentre, Preset::HautGauche, Preset::HautDroite, Preset::BasGauche, Preset::BasDroite, Preset::Ici, Preset::Feuille];

    pub fn label(self) -> &'static str {
        match self {
            Preset::CentreDessin => "Au centre du dessin",
            Preset::BasCentre => "En bas au centre",
            Preset::HautGauche => "En haut à gauche",
            Preset::HautDroite => "En haut à droite",
            Preset::BasGauche => "En bas à gauche",
            Preset::BasDroite => "En bas à droite",
            Preset::Ici => "Ici",
            Preset::Feuille => "Au centre de la feuille",
        }
    }

    /// La commande (`keymap.rs`), sans touche d'office.
    pub fn action(self) -> &'static str {
        match self {
            Preset::CentreDessin => "origine.centre_dessin",
            Preset::BasCentre => "origine.bas_centre",
            Preset::HautGauche => "origine.haut_gauche",
            Preset::HautDroite => "origine.haut_droite",
            Preset::BasGauche => "origine.bas_gauche",
            Preset::BasDroite => "origine.bas_droite",
            Preset::Ici => "origine.ici",
            Preset::Feuille => "origine.feuille",
        }
    }

    pub fn de_l_action(action: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.action() == action)
    }

    pub fn aide(self) -> &'static str {
        match self {
            Preset::Ici => "L'origine passe au point de la feuille sous le dernier clic (ou le pointeur), sur le dessin.",
            Preset::Feuille => "Plus d'origine à soi : le calque tourne et s'agrandit autour du centre de la feuille, comme d'origine.",
            _ => "Un point de la boîte du dessin montré à cette image. Le dessin ne bouge pas : seule l'origine change.",
        }
    }

    /// Le point de la boîte `b` du contenu (coordonnées du dessin) ; rien pour « Ici » et « Au centre de la feuille ».
    pub fn dans_la_boite(self, b: Rect) -> Option<[f32; 2]> {
        let p = match self {
            Preset::CentreDessin => b.center(),
            Preset::BasCentre => b.center_bottom(),
            Preset::HautGauche => b.left_top(),
            Preset::HautDroite => b.right_top(),
            Preset::BasGauche => b.left_bottom(),
            Preset::BasDroite => b.right_bottom(),
            Preset::Ici | Preset::Feuille => return None,
        };
        Some([p.x, p.y])
    }
}

/// Les points où l'origine tirée avec Maj s'aimante : le centre de la boîte du contenu, ses coins et les milieux de ses côtés.
pub fn points_d_aimantation(b: Rect) -> [Pos2; 9] {
    [b.center(), b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom(), b.center_top(), b.right_center(), b.center_bottom(), b.left_center()]
}

/// Le point d'aimantation de `b` le plus proche de `p` (coordonnées du dessin).
pub fn aimanter_a_la_boite(b: Rect, p: Pos2) -> Pos2 {
    points_d_aimantation(b).into_iter().min_by(|a, c| a.distance_sq(p).total_cmp(&c.distance_sq(p))).unwrap_or(p)
}

/// Une origine lue d'un fichier : finie et raisonnable, sinon rien (le centre de la feuille).
pub fn assainie(o: Option<[f32; 2]>) -> Option<[f32; 2]> {
    o.filter(|o| o.iter().all(|v| v.is_finite() && v.abs() < crate::trajectoire::COORDONNEE_MAX))
}

/// La profondeur totale d'une pose dont l'écart est `dz`, comme la lisent la vue de la scène et l'image (`Layer::depth_libre_at`).
fn profondeur(layer: &Layer, dz: f32) -> f32 {
    if layer.profondeur_bornee {
        (layer.depth + dz).max(Layer::MIN_DEPTH)
    } else {
        Layer::clamp_depth_libre(layer.depth + dz)
    }
}

/// La pose `t` (profondeur totale `depth`) d'une carte dont l'origine passe de `e1` à `e2` (écarts au centre de la feuille,
/// coordonnées du dessin), avec la même image : sur la feuille pour une carte à plat (une affinité gardée), dans l'espace pour
/// une carte inclinée (son placement 3D gardé ; sa profondeur, celle de l'origine, peut changer, et son échelle avec elle).
/// Rend la pose et la profondeur totale.
pub fn pose_pour_l_origine(world: &Monde, t: LayerTransform, depth: f32, e1: Vec2, e2: Vec2) -> (LayerTransform, f32) {
    if e1 == e2 {
        return (t, depth);
    }
    if !t.is_tilted() {
        return (t.origine_deplacee(e2 - e1), depth);
    }
    let xf = world.card_xf_o(&t, depth, e1);
    let (mut t2, d2) = world.card_from_xf_libre_o(&t, depth, &xf, e2);
    t2.dz = t.dz;
    (t2, d2)
}

/// **Déplace l'origine du calque** vers `nouvelle` (coordonnées du dessin ; rien : le centre de la feuille) sans rien changer
/// de son image : sa pose fixe et chacune de ses clés de position sont recalculées (`pose_pour_l_origine`), et le chemin
/// d'une trajectoire que suit l'origine (`Ancre::Origine`) se déplace avec elle (sur la feuille : du déplacement de
/// l'origine sous la pose du départ ; dans l'espace : de celui de l'origine de la carte). Les autres ancrages ne dépendent
/// pas de l'origine : rien n'y change. Vrai si quelque chose a changé.
pub fn deplacer_origine(layer: &mut Layer, nouvelle: Option<[f32; 2]>, world: &Monde) -> bool {
    let nouvelle = assainie(nouvelle);
    if layer.origine == nouvelle {
        return false;
    }
    let centre = pos2(world.stage[0] / 2.0, world.stage[1] / 2.0);
    let e1 = layer.ecart_origine(centre);
    let e2 = match nouvelle {
        Some([x, y]) => vec2(x - centre.x, y - centre.y),
        None => Vec2::ZERO,
    };
    // Le chemin d'une trajectoire que suit l'origine : il suit l'origine, mesurée sous la pose du départ (avant le changement).
    if let Some(tr) = layer.trajectoire.clone().filter(|t| t.ancre == Ancre::Origine) {
        match tr.support {
            None => {
                let posee = crate::poskeys::pose_at(&layer.pos_keys, tr.depart, layer.transform);
                let d = posee.lineaire(e2 - e1);
                if let Some(t) = layer.trajectoire.as_mut() {
                    t.deplacer(d.x, d.y);
                }
            }
            Some(_) => {
                // (La pose propre de la carte, celle d'où partent ses clés : sa pose fixe et sa profondeur.)
                let base = LayerTransform { dz: 0.0, ..layer.transform };
                let d = Layer::clamp_depth_libre(layer.depth + layer.transform.dz);
                let xf = world.card_xf_o(&base, d, e1);
                let glisse = crate::espace3dmath::sub(world.origine_monde(&xf, e2), world.origine_monde(&xf, e1));
                if let Some(s) = layer.trajectoire.as_mut().and_then(|t| t.support.as_mut()) {
                    *s = s.deplace(glisse);
                }
            }
        }
    }
    let base = layer.depth;
    if layer.pos_keys.is_empty() {
        let (t, d) = pose_pour_l_origine(world, layer.transform, profondeur(layer, layer.transform.dz), e1, e2);
        layer.transform = t;
        if t.is_tilted() {
            // (Un calque fixe n'a pas d'écart de profondeur : la profondeur de l'origine est la sienne.)
            layer.depth = d;
            layer.transform.dz = 0.0;
        }
    } else {
        let (t, d) = pose_pour_l_origine(world, layer.transform, profondeur(layer, layer.transform.dz), e1, e2);
        layer.transform = LayerTransform { dz: if t.is_tilted() { d - base } else { t.dz }, ..t };
        let poses: Vec<(LayerTransform, f32)> = layer.pos_keys.iter().map(|k| pose_pour_l_origine(world, k.t, profondeur(layer, k.t.dz), e1, e2)).collect();
        for (k, (t, d)) in layer.pos_keys.iter_mut().zip(poses) {
            k.t = LayerTransform { dz: if t.is_tilted() { d - base } else { t.dz }, ..t };
        }
    }
    layer.origine = nouvelle;
    true
}

/// **La hauteur du sol** (mètres, l'axe Y du monde) sous le point (`x`, `z`) de l'Espace 3D du monde `world`. C'est par
/// cette seule fonction que l'aimant lit le sol : aujourd'hui le plan du sol du monde (`Monde::sol_y`), partout le même ;
/// demain aussi le dessus des objets du décor 3D, à brancher ici.
pub fn hauteur_du_sol_sous(world: &Monde, x: f32, z: f32) -> f32 {
    let _ = (x, z);
    world.sol_y()
}

/// La pose `t` (profondeur totale `depth`, origine à `ecart` du centre de la feuille) dont l'origine est posée sur le sol :
/// seule sa hauteur change (la place de l'origine en Y, `Monde::card_xf_o`), sa profondeur et tout le reste demeurent.
pub fn pose_au_sol(world: &Monde, t: LayerTransform, depth: f32, ecart: Vec2) -> LayerTransform {
    let s = 1.0 + depth.max(Layer::MIN_DEPTH);
    let k = s * world.k;
    if !(k.is_finite() && k.abs() > 1e-12) {
        return t;
    }
    let x = (ecart.x + t.x) * k;
    let z = -world.d0 * depth;
    let sol = hauteur_du_sol_sous(world, x, z);
    LayerTransform { y: -sol / k - ecart.y, ..t }
}

/// **Aimante au sol** le calque `layer` s'il l'est (`Layer::aimante_au_sol`) : l'origine de sa carte repose sur le sol, à sa
/// pose fixe et à chacune de ses clés. Rien pour un calque accroché à la caméra (sa pose se lit dans le cadre). Vrai si
/// quelque chose a changé.
pub fn aimanter_au_sol(layer: &mut Layer, world: &Monde) -> bool {
    if !layer.aimante_au_sol || layer.accroche || layer.folder {
        return false;
    }
    let e = world.ecart(layer);
    let avant = (layer.transform, layer.pos_keys.clone());
    layer.transform = pose_au_sol(world, layer.transform, profondeur(layer, layer.transform.dz), e);
    let poses: Vec<LayerTransform> = layer.pos_keys.iter().map(|k| pose_au_sol(world, k.t, profondeur(layer, k.t.dz), e)).collect();
    for (k, t) in layer.pos_keys.iter_mut().zip(poses) {
        k.t = t;
    }
    (layer.transform, &layer.pos_keys) != (avant.0, &avant.1)
}

/// Le petit repère de l'origine : un cercle et sa croix, à la manière du pivot de la boîte de la Sélection (`selbox.rs`).
pub fn peindre_repere(painter: &egui::Painter, c: Pos2, couleur: Color32, survol: bool) {
    let r = crate::selbox::PIVOT_RADIUS + 1.0;
    if survol {
        painter.circle_filled(c, r, couleur.gamma_multiply(0.3));
    }
    painter.circle_stroke(c, r + 1.0, Stroke::new(3.0_f32, Color32::from_black_alpha(100)));
    painter.circle_stroke(c, r, Stroke::new(1.5_f32, couleur));
    let k = r + 3.0;
    for (a, b) in [(vec2(-k, 0.0), vec2(k, 0.0)), (vec2(0.0, -k), vec2(0.0, k))] {
        painter.line_segment([c + a, c + b], Stroke::new(1.0_f32, couleur));
    }
}

impl App {
    /// La feuille d'où viennent l'aimant et le déplacement de l'origine : le monde du plan courant.
    fn monde_de_l_origine(&self) -> Monde {
        self.e3_world()
    }

    /// Pose l'origine du calque `li` du plan courant à `nouvelle` (rien : le centre de la feuille), sans que son dessin bouge ;
    /// l'aimant est refait s'il est coché. Une étape d'annulation, au nom `nom`. Faux si rien n'a changé (ou si le calque ne
    /// le permet pas : la raison est dans la ligne d'état).
    pub(crate) fn poser_origine(&mut self, li: usize, nouvelle: Option<[f32; 2]>, nom: &str) -> bool {
        let Some(l) = self.layers().get(li) else { return false };
        if l.folder || l.effet.is_some() {
            self.status = "Un dossier ou un calque d'effet n'a pas d'origine : il n'a pas de dessin à lui.".into();
            return false;
        }
        if l.keys_follow_line() {
            self.status = "Un calque de couleur a l'origine de son calque de trait : place celle du trait, la couleur suit.".into();
            return false;
        }
        if l.position_locked {
            self.status = crate::app::POSITION_LOCKED.into();
            return false;
        }
        self.end_stroke();
        let snap = self.snapshot();
        if !self.origine_sans_etape(li, nouvelle) {
            return false;
        }
        self.commit_named(snap, nom);
        self.renderer.invalidate_all();
        self.status = match self.layers()[li].origine {
            Some([x, y]) => format!("Origine du calque en x {x:.0}, y {y:.0} (le dessin ne bouge pas)."),
            None => "Origine au centre de la feuille, comme d'origine (le dessin ne bouge pas).".into(),
        };
        true
    }

    /// Comme `poser_origine`, sans étape d'annulation (un geste en cours, ou l'appelant s'en charge). Les calques de couleur
    /// liés au calque suivent (`calquecouleur::sync`, au prochain passage).
    pub(crate) fn origine_sans_etape(&mut self, li: usize, nouvelle: Option<[f32; 2]>) -> bool {
        let world = self.monde_de_l_origine();
        let Some(l) = self.layers_mut().get_mut(li) else { return false };
        if !deplacer_origine(l, nouvelle, &world) {
            return false;
        }
        crate::origine::aimanter_au_sol(l, &world);
        self.modified = true;
        true
    }

    /// L'origine toute faite `preset` pour le calque courant : la boîte du dessin montré à l'image courante, ou le point
    /// d'« Ici ». Une étape d'annulation.
    pub(crate) fn origine_preset(&mut self, preset: Preset) {
        let li = self.cur_layer;
        let nouvelle = match preset {
            Preset::Feuille => None,
            Preset::Ici => {
                let Some(stage) = self.origine_ici.or(self.hover_canvas) else {
                    self.status = "Origine « Ici » : clique d'abord le point de la feuille où la mettre.".into();
                    return;
                };
                let Some(p) = self.to_layer_opt(stage) else {
                    self.status = "Origine « Ici » : ce point de la feuille n'est pas sur le calque.".into();
                    return;
                };
                Some([p.x, p.y])
            }
            p => match self.layer_content_box(li, self.cur_frame).and_then(|b| p.dans_la_boite(b)) {
                Some(o) => Some(o),
                None => {
                    self.status = format!("Origine « {} » : le calque n'a rien de dessiné à cette image.", p.label());
                    return;
                }
            },
        };
        self.poser_origine(li, nouvelle, &format!("Origine : {}", preset.label().to_lowercase()));
    }

    /// Refait l'aimant du calque `li` s'il est aimanté au sol (après un geste, un champ, une clé, une trajectoire). Sans étape
    /// d'annulation : il fait partie de l'étape du geste.
    pub(crate) fn aimanter_si(&mut self, li: usize) {
        let world = self.monde_de_l_origine();
        if let Some(l) = self.layers_mut().get_mut(li) {
            if aimanter_au_sol(l, &world) {
                self.renderer.invalidate_all();
            }
        }
    }

    /// Coche ou décoche « Aimanté au sol » sur le calque `li` : coché, l'origine de la carte se pose sur le sol (pose fixe et
    /// clés) ; décoché, la carte reste exactement où l'aimant l'avait mise. Une étape d'annulation.
    pub(crate) fn basculer_aimant(&mut self, li: usize) {
        let Some(l) = self.layers().get(li) else { return };
        if !crate::espace3d::is_card(l) {
            self.status = "Un dossier ou un calque d'effet n'a pas de carte à poser au sol.".into();
            return;
        }
        if l.accroche && !l.aimante_au_sol {
            self.status = "Un calque accroché à la caméra garde sa place dans l'image : il ne se pose pas au sol.".into();
            return;
        }
        if l.position_locked {
            self.status = crate::app::POSITION_LOCKED.into();
            return;
        }
        let on = !l.aimante_au_sol;
        self.end_stroke();
        let snap = self.snapshot();
        let world = self.monde_de_l_origine();
        let l = &mut self.layers_mut()[li];
        l.aimante_au_sol = on;
        // (Décoché : rien d'autre ne change, les hauteurs que l'aimant a données restent.)
        if on {
            aimanter_au_sol(l, &world);
        }
        self.commit_named(snap, if on { "Aimanter au sol" } else { "Ne plus aimanter au sol" });
        self.renderer.invalidate_all();
        self.status = if on { "Aimanté au sol : l'origine du calque repose sur le sol, à chaque clé.".into() } else { "Plus aimanté : le calque garde la hauteur où l'aimant l'avait posé.".into() };
    }

    /// Le point de la feuille où est l'origine du calque courant à l'image courante (carte inclinée comprise).
    pub(crate) fn origine_sur_la_feuille(&self) -> Pos2 {
        let o = self.layer().pivot(self.doc.stage_center());
        if let Some((world, mx, eye)) = self.tilted_card() {
            return world.card_to_sheet_m(eye, &mx, o).unwrap_or(o);
        }
        self.layer_affine().apply(o)
    }

    /// Le menu « Origine » (menu du calque, réglages de l'outil Transformer) pour le calque courant.
    pub(crate) fn origine_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        for p in Preset::ALL {
            if p == Preset::Ici || p == Preset::Feuille {
                ui.separator();
            }
            let raison = match p {
                Preset::Feuille if self.layers().get(li).is_some_and(|l| l.origine.is_none()) => Some("L'origine est déjà au centre de la feuille.".to_string()),
                _ => None,
            };
            let texte = p.label();
            let r = ui.add_enabled(raison.is_none(), egui::Button::new(texte));
            let r = crate::bulles::bulle(r, texte, raison.as_deref().unwrap_or(p.aide()), p.action());
            if r.clicked() {
                if self.cur_layer != li {
                    self.select_layer(li);
                }
                self.origine_preset(p);
                ui.close_menu();
            }
        }
    }

    /// Les champs X et Y de l'origine du calque courant (coordonnées de son dessin), avec le menu « Origine ».
    pub(crate) fn origine_fields(&mut self, ui: &mut egui::Ui) {
        let li = self.cur_layer;
        let Some(l) = self.layers().get(li) else { return };
        if l.folder || l.effet.is_some() {
            return;
        }
        let o = l.pivot(self.doc.stage_center());
        let a_soi = l.origine.is_some();
        let actif = !l.position_locked && !l.locked && !l.keys_follow_line();
        let (mut x, mut y) = (o.x, o.y);
        let mut next = None;
        ui.horizontal_wrapped(|ui| {
            ui.label("Origine").on_hover_text("Le point du dessin autour duquel le calque se déplace, s'agrandit et tourne (le petit cercle à croix de l'outil). Le déplacer ne déplace pas le dessin.");
            ui.add_enabled_ui(actif, |ui| {
                let r = ui.add(egui::DragValue::new(&mut x).speed(1.0).prefix("x ").suffix(" px"));
                let r = crate::bulles::bulle(r, "Origine x", "Où est l'origine du calque, en pixels de son dessin. Le dessin ne bouge pas.", "");
                if self.edited(&r) {
                    next = Some([x, o.y]);
                }
                let r = ui.add(egui::DragValue::new(&mut y).speed(1.0).prefix("y ").suffix(" px"));
                let r = crate::bulles::bulle(r, "Origine y", "Où est l'origine du calque, en pixels de son dessin. Le dessin ne bouge pas.", "");
                if self.edited(&r) {
                    next = Some([o.x, y]);
                }
                ui.menu_button(if a_soi { "Origine ▾" } else { "Origine (feuille) ▾" }, |ui| self.origine_menu(ui, li));
            });
        });
        if let Some(n) = next {
            // (Le champ a déjà sa propre étape d'annulation, `edited`.)
            if self.origine_sans_etape(li, Some(n)) {
                self.renderer.invalidate_all();
            }
        }
    }
}
