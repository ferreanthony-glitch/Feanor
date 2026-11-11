//! Partie 141 : un calque **accroché à la caméra** (Anthony, 2026-11-11 : « une option de calque qui permet d'accrocher le calque à la
//! caméra. Quand la caméra bouge, le calque suit naturellement car il y est accroché »).
//!
//! **La règle.** Un calque accroché (`Layer::accroche`) garde sa place **dans l'image filmée**, quoi que fasse la caméra du plan : elle
//! se déplace, zoome, tourne ou avance, et le calque reste au même endroit du cadre, à la même taille. Sur la feuille, on le voit donc
//! suivre le cadre de la caméra. Un personnage qui marche sur place devant un décor qui défile, l'intérieur d'une voiture, un titre, un
//! cache : on le dessine une fois, on l'accroche.
//!
//! **Comment.** La pose d'un calque accroché (`Layer::transform`, ses clés de position) se lit **dans le cadre de la caméra** au lieu de
//! se lire sur la feuille : c'est la pose qu'il aurait sous la caméra par défaut (au centre de la feuille, sans zoom ni rotation). À
//! chaque image, `CameraSeen::accrochee` la pose là où la caméra de l'image filme : avec `o` le décalage de la caméra depuis le centre de
//! la feuille, `m` son grossissement (l'objectif et l'avancée) et `θ` sa rotation, la pose (décalage `t`, échelle `s`, angle `a`) devient
//! le décalage `o + R(θ)·t / m`, l'échelle `s / m` et l'angle `a + θ`. C'est encore un « décalage + échelle + rotation » : tout ce qui pose
//! un calque (l'affichage, le stylet, la sélection, les pochoirs, les vignettes, les exports) passe par `Layer::transform_seen_at`, donc
//! par là, sans rien savoir de l'accroche. Le multiplan (`Layer::depth`) ne fait plus défiler un calque accroché.
//!
//! **Accrocher et décrocher ne déplacent rien** à l'image courante : en accrochant, la pose du calque est ramenée dans le cadre de la
//! caméra de cette image (`Layer::accrocher`) ; en décrochant, elle est reposée sur la feuille là où on le voit (`Layer::decrocher`). Les
//! clés de position d'un calque animé suivent toutes, de la même façon : son mouvement reste le sien.
//!
//! **L'outil Transformer** travaille sur ce qu'on voit (`App::pose_du_geste`, `App::poser_du_geste`, `layertools.rs`) : la boîte suit le
//! pointeur même sous une caméra zoomée ou tournée.
//!
//! **Les limites** (dites dans `docs/INTERFACE.md`, partie 141) : la bascule et le panoramique de la caméra (9f) ne sont pas suivis (le
//! calque accroché est filmé en perspective avec la feuille) ; une carte inclinée de l'Espace 3D et un calque à trajectoire ne
//! s'accrochent pas ; l'Espace 3D et la vue de dessus montrent un calque accroché à sa pose dans le cadre.
//!
//! Le fichier : `accroche_camera` du calque (format 53).

use eframe::egui::{self, pos2, Pos2, RichText, Vec2};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::camera::{self, Camera};
use crate::document::{CameraSeen, Layer, LayerTransform};

/// La commande (palette, menus, raccourci à assigner).
pub(crate) const ACTION: &str = "calque.accrocher_camera";
/// Le nom de l'option, partout le même.
pub(crate) const NOM: &str = "Accrocher à la caméra";
/// Sa phrase (bulles).
pub(crate) const PHRASE: &str = "Le calque garde sa place dans l'image, quoi que fasse la caméra : elle se déplace, zoome ou tourne, il la suit. Rien ne bouge à l'image où tu l'accroches.";

impl CameraSeen {
    /// La caméra `c` vue par les calques : son décalage depuis le `centre` de la feuille, son avancée, la distance de référence du
    /// `cadre` ; et, pour les calques accrochés, son grossissement et sa rotation.
    pub fn de(c: &Camera, centre: Pos2, cadre: [usize; 2]) -> CameraSeen {
        CameraSeen { offset: pos2(c.x, c.y) - centre, dolly: c.dolly, d0: camera::ref_distance(cadre), grossissement: c.magnification(cadre), angle: c.angle }
    }

    fn grossi(&self) -> f32 {
        if self.grossissement.is_finite() && self.grossissement > 1e-4 { self.grossissement } else { 1.0 }
    }

    /// La pose `t` d'un calque accroché, lue dans le cadre de la caméra, telle qu'on la voit sur la feuille sous cette caméra.
    pub fn accrochee(&self, t: LayerTransform) -> LayerTransform {
        let m = self.grossi();
        let (s, c) = self.angle.to_radians().sin_cos();
        LayerTransform {
            x: self.offset.x + (t.x * c - t.y * s) / m,
            y: self.offset.y + (t.x * s + t.y * c) / m,
            scale: t.scale / m,
            angle: t.angle + self.angle,
            ..t
        }
    }

    /// L'inverse d'`accrochee` : la pose `vue` sur la feuille, ramenée dans le cadre de cette caméra.
    pub fn decrochee(&self, vue: LayerTransform) -> LayerTransform {
        let m = self.grossi();
        let (s, c) = self.angle.to_radians().sin_cos();
        let d = Vec2::new(vue.x - self.offset.x, vue.y - self.offset.y) * m;
        LayerTransform { x: d.x * c + d.y * s, y: -d.x * s + d.y * c, scale: vue.scale * m, angle: vue.angle - self.angle, ..vue }
    }
}

impl Layer {
    /// Pourquoi ce calque ne s'accroche pas à la caméra (rien : il le peut).
    pub fn raison_sans_accroche(&self) -> Option<&'static str> {
        if self.folder {
            Some("dossier")
        } else if self.effet.is_some() {
            Some("calque d'effet, il n'a pas de place à lui")
        } else if self.decor {
            Some("calque Décor, il reste sur la feuille")
        } else if self.keys_follow_line() {
            Some("calque de couleur, il suit son calque de trait")
        } else if self.trajectoire.is_some() {
            Some("il a une trajectoire, tracée sur la feuille")
        } else if self.transform.is_tilted() || self.pos_keys.iter().any(|k| k.t.is_tilted()) {
            Some("carte inclinée de l'Espace 3D")
        } else {
            None
        }
    }

    /// La pose qui, vue par la caméra `seen` avec le multiplan de ce calque, donne `vue` : l'inverse de `transform_seen` pour un calque
    /// qui n'est pas accroché (à la profondeur 0, ou sous la caméra au centre : `vue` elle-même).
    fn sans_multiplan(&self, vue: LayerTransform, seen: &CameraSeen) -> LayerTransform {
        let depth = self.depth + vue.dz;
        if depth == 0.0 || (seen.offset == Vec2::ZERO && seen.dolly == 0.0) {
            return vue;
        }
        let d = depth.max(Self::MIN_DEPTH);
        let dist = seen.d0 * (1.0 + d) - seen.dolly;
        if dist <= seen.d0 * 0.02 {
            return vue;
        }
        let s = (seen.d0 - seen.dolly) / dist;
        let k = s * (1.0 + d);
        LayerTransform { x: (vue.x - seen.offset.x * (1.0 - s)) / k, y: (vue.y - seen.offset.y * (1.0 - s)) / k, scale: vue.scale / k, ..vue }
    }

    /// Accroche le calque à la caméra, vue en `seen` à l'image `frame` : sa pose (et chacune de ses clés de position) est ramenée dans
    /// le cadre de cette caméra, il ne bouge donc pas à cette image. Rien s'il est déjà accroché.
    pub fn accrocher(&mut self, seen: &CameraSeen, frame: usize) {
        if self.accroche {
            return;
        }
        // (Tel qu'on le voit maintenant, multiplan compris : c'est cette place-là qu'il garde.)
        let vu = |l: &Layer, t: LayerTransform| l.pose_seen(t, seen).unwrap_or(t);
        self.transform = seen.decrochee(vu(self, self.transform));
        let poses: Vec<LayerTransform> = self.pos_keys.iter().map(|k| seen.decrochee(vu(self, k.t))).collect();
        for (k, t) in self.pos_keys.iter_mut().zip(poses) {
            k.t = t;
        }
        let _ = frame;
        self.accroche = true;
    }

    /// Décroche le calque de la caméra, vue en `seen` à l'image `frame` : sa pose (et chacune de ses clés) est reposée sur la feuille,
    /// là où on le voit à cette image. Rien s'il n'est pas accroché.
    pub fn decrocher(&mut self, seen: &CameraSeen, frame: usize) {
        if !self.accroche {
            return;
        }
        self.accroche = false;
        self.transform = self.sans_multiplan(seen.accrochee(self.transform), seen);
        let poses: Vec<LayerTransform> = self.pos_keys.iter().map(|k| self.sans_multiplan(seen.accrochee(k.t), seen)).collect();
        for (k, t) in self.pos_keys.iter_mut().zip(poses) {
            k.t = t;
        }
        let _ = frame;
    }
}

impl App {
    /// Les calques que vise l'accroche demandée sur le calque `li` : les calques du geste (`calques_du_geste`), un dossier comptant
    /// pour ses calques.
    fn calques_a_accrocher(&self, li: usize) -> Vec<usize> {
        let layers = self.layers();
        let mut v: Vec<usize> = self
            .calques_du_geste(li)
            .into_iter()
            .filter(|&k| k < layers.len())
            .flat_map(|k| if layers[k].folder { crate::folders::children_of(layers, k) } else { vec![k] })
            .filter(|&k| !layers[k].folder)
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Le calque `li` est accroché à la caméra ; un dossier : tous ses calques le sont (et il en a).
    pub(crate) fn est_accroche(&self, li: usize) -> bool {
        let layers = self.layers();
        match layers.get(li) {
            Some(l) if l.folder => {
                let kids: Vec<usize> = crate::folders::children_of(layers, li).into_iter().filter(|&k| !layers[k].folder).collect();
                !kids.is_empty() && kids.iter().all(|&k| layers[k].accroche)
            }
            Some(l) => l.accroche,
            None => false,
        }
    }

    /// « Accrocher à la caméra » demandé sur le calque `li` : lui et, s'il fait partie du choix, tous les calques choisis prennent
    /// l'inverse de son état (règle « la sélection agit sur tout »). Un dossier : ses calques. Rien ne bouge à l'image courante. Une
    /// étape d'annulation.
    pub(crate) fn basculer_accroche(&mut self, li: usize) {
        if li >= self.layers().len() {
            return;
        }
        // (Un calque de couleur suit son calque de trait, accroche comprise : demandée sur lui, la commande agit sur son trait.)
        let li = crate::calquecouleur::trait_de(self.layers(), li).filter(|_| self.layers()[li].keys_follow_line()).unwrap_or(li);
        let on = !self.est_accroche(li);
        let lis = self.calques_a_accrocher(li);
        let mut ok = Vec::new();
        let mut laisses = Vec::new();
        for &k in &lis {
            let l = &self.layers()[k];
            if l.accroche == on {
                continue;
            }
            // (Un calque de couleur suit son trait, accroche comprise, `calquecouleur::refleter` : il n'est ni à faire ni à dire.)
            if l.keys_follow_line() {
                continue;
            }
            match l.raison_sans_accroche().filter(|_| on) {
                Some(r) => laisses.push(format!("{} ({r})", l.name)),
                None => ok.push(k),
            }
        }
        let laisses = crate::choixcalques::laisses_de_cote(&laisses);
        if ok.is_empty() {
            self.status = if laisses.is_empty() { format!("{NOM} : rien à changer.") } else { format!("{NOM} : aucun calque ne le prend.{laisses}") };
            return;
        }
        self.end_stroke();
        self.record_step();
        let (f, seen) = (self.cur_frame, self.doc.camera_seen(self.cur_panel, self.cur_frame));
        for &k in &ok {
            let l = &mut self.layers_mut()[k];
            if on {
                l.accrocher(&seen, f);
            } else {
                l.decrocher(&seen, f);
            }
        }
        // (Les calques de couleur des traits changés prennent tout de suite la même pose et la même accroche.)
        crate::calquecouleur::refleter(&mut self.doc, self.cur_panel);
        self.renderer.invalidate_all();
        let n = ok.len();
        self.status = match (on, n) {
            (true, 1) => format!("« {} » est accroché à la caméra : il garde sa place dans l'image, quoi que fasse la caméra.{laisses}", self.layers()[ok[0]].name),
            (true, _) => format!("{n} calques accrochés à la caméra : ils gardent leur place dans l'image, quoi que fasse la caméra.{laisses}"),
            (false, 1) => format!("« {} » est décroché de la caméra : il reste sur la feuille, là où il est à cette image.", self.layers()[ok[0]].name),
            (false, _) => format!("{n} calques décrochés de la caméra : ils restent sur la feuille, là où ils sont à cette image."),
        };
    }

    /// Le petit bouton d'un calque du panneau Calques : allumé s'il est accroché à la caméra ; c'est là qu'on l'accroche d'un clic.
    /// Rien pour un calque qui ne s'accroche pas (et n'est pas accroché).
    pub(crate) fn bouton_accroche(&mut self, ui: &mut egui::Ui, li: usize) -> Option<egui::Response> {
        let l = self.layers().get(li)?;
        let on = l.accroche;
        if l.folder || (!on && l.raison_sans_accroche().is_some()) {
            return None;
        }
        let key = self.raccourci(ACTION);
        let about = if on {
            "Ce calque est accroché à la caméra : il garde sa place dans l'image, quoi que fasse la caméra (déplacement, zoom, rotation). Clic : le décrocher ; il reste sur la feuille, là où il est à cette image."
        } else {
            PHRASE
        };
        let resp = ui.add(egui::Button::new(crate::bulles::encre_etat(RichText::new(ph::VIDEO_CAMERA).size(12.0), on)).selected(on).frame(on).min_size(egui::vec2(18.0, 0.0)));
        let resp = crate::bulles::bulle(resp, if on { "Accroché à la caméra" } else { NOM }, about, &key);
        crate::raccourcisui::shortcut_menu(&resp, ACTION, &key);
        Some(resp)
    }
}
