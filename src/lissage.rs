//! Lissage de la caméra à la main (Anthony, 2026-11-06 : « je voudrais pouvoir mieux contrôler le
//! lissage de caméra. Quand un lissage se crée, une autre clé apparaît sur la timeline de la caméra
//! et contrôle le moment où le lissage commence ou se termine »).
//!
//! Le modèle existait déjà : chaque clé de caméra a un adouci d'entrée (`ease_in`) et de sortie
//! (`ease_out`), en images. Rien ne change dans le projet : on les montre, et on les tire.
//!
//! - Une clé montre, sur la piste de caméra, deux **poignées de lissage** : petits ronds creux,
//!   reliés à leur clé par un trait fin. À droite, à `image + sortie` : là où la caméra a fini
//!   d'accélérer en repartant. À gauche, à `image − entrée` : là où elle commence à ralentir. Une
//!   poignée existe de chaque côté où il y a un mouvement (une clé voisine), **même à 0** (Anthony,
//!   2026-11-07 : « un mouvement crée immédiatement des clés de lissage visibles sur la timeline, il
//!   suffit de les étirer ensuite ») : elle attend alors au repos, juste à côté de sa clé (`repos`),
//!   plus petite et plus pâle, hors de la prise de la clé, pour être tirée.
//! - Tirer une poignée change l'adouci, à l'image près, sans jamais passer la clé voisine ni la
//!   poignée d'en face : ce qu'on voit est toujours ce qui se passe (`camera::interval_eases` donne
//!   les valeurs après le partage éventuel, `camera::ease` ne partage plus en silence ce qu'on a
//!   réglé ici). Ramenée sur sa clé, elle met ce côté à 0 et disparaît : vitesse constante.
//! - Le bouton « Adoucir » pose des valeurs réelles, en demi-intervalle de chaque côté où la caméra
//!   bouge, ou les retire toutes (`adoucir`).
//! - Une ancienne clé `smooth` montre ses poignées aux valeurs effectives ; le premier réglage
//!   convertit ses deux côtés en valeurs explicites (`convertir`), sans changer le mouvement.
//! - Une poignée n'est pas une clé : elle n'est ni choisie, ni copiée, ni comptée par clé
//!   suivante / précédente. Si une clé et une poignée se recouvrent, la clé passe avant.

use eframe::egui::{self, Color32, Pos2, Stroke};

use crate::app::App;
use crate::camera::{self, CameraKey};
use crate::history::Edit;

/// De quel côté de sa clé est la poignée : l'entrée (à gauche, la caméra ralentit en arrivant) ou la
/// sortie (à droite, elle accélère en repartant).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cote {
    Entree,
    Sortie,
}

/// Demi-largeur de la zone où l'on attrape une poignée, en points : plus large que le dessin (3 points
/// de rayon), pour le stylet.
pub(crate) const PRISE: f32 = 7.0;

/// Rayon d'une poignée dessinée, au repos (adouci nul), et quand elle est éclairée.
const RAYON: f32 = 3.0;
pub(crate) const RAYON_REPOS: f32 = 2.5;
const RAYON_CLAIR: f32 = 4.5;

/// Moins que cela, un adouci est nul (la poignée est au repos).
const NUL: f32 = 1e-3;

/// La zone de prise d'une clé de caméra, de chaque côté de son losange, la plus large des deux pistes (timeline :
/// `cell / 2 + 2` ; montage : `cell / 2`, 5 au moins).
pub(crate) fn tol_cle(cell: f32) -> f32 {
    (cell * 0.5 + 2.0).max(5.0)
}

/// Où attend une poignée au repos (adouci nul), en points de sa clé : juste hors de la prise de la clé, sa propre prise
/// comprise, pour qu'un appui dessus ne prenne jamais la clé.
pub(crate) fn repos(cell: f32) -> f32 {
    tol_cle(cell) + PRISE
}

// ---------------------------------------------------------------------- valeurs

/// La clé a un lissage : l'ancien réglage, ou un adouci d'un côté au moins.
pub(crate) fn est_adoucie(k: &CameraKey) -> bool {
    k.smooth || k.ease_in > 0.0 || k.ease_out > 0.0
}

/// Les adoucis de la clé `i` tels qu'ils agissent (entrée, sortie), en images, partage compris. Du côté
/// où il n'y a pas de clé voisine, la valeur gardée par la clé.
pub(crate) fn valeurs(keys: &[CameraKey], i: usize) -> (f32, f32) {
    let entree = if i > 0 { camera::interval_eases(&keys[i - 1], &keys[i]).1 } else { keys[i].ease_in };
    let sortie = if i + 1 < keys.len() { camera::interval_eases(&keys[i], &keys[i + 1]).0 } else { keys[i].ease_out };
    (entree, sortie)
}

/// Les poignées à montrer : (rang de la clé, côté, images), de chaque côté où il y a un mouvement (une clé
/// voisine) ; 0 : la poignée est au repos, à côté de sa clé.
pub(crate) fn poignees(keys: &[CameraKey]) -> Vec<(usize, Cote, f32)> {
    let mut out = Vec::new();
    for i in 0..keys.len() {
        if i > 0 {
            let v = camera::interval_eases(&keys[i - 1], &keys[i]).1;
            out.push((i, Cote::Entree, if v > NUL { v } else { 0.0 }));
        }
        if i + 1 < keys.len() {
            let v = camera::interval_eases(&keys[i], &keys[i + 1]).0;
            out.push((i, Cote::Sortie, if v > NUL { v } else { 0.0 }));
        }
    }
    out
}

/// Écrit en valeurs explicites l'ancien lissage (`smooth`) de la clé `i`, côté par côté, puis le
/// coupe : le mouvement ne change pas. Du côté sans voisine, la valeur gardée ne change pas non plus.
pub(crate) fn convertir(keys: &mut [CameraKey], i: usize) {
    if !keys[i].smooth {
        return;
    }
    let k = keys[i];
    if i > 0 {
        let span = (k.frame - keys[i - 1].frame).max(1) as f32;
        keys[i].ease_in = k.eases(span).0;
    }
    if i + 1 < keys.len() {
        let span = (keys[i + 1].frame - k.frame).max(1) as f32;
        keys[i].ease_out = k.eases(span).1;
    }
    keys[i].smooth = false;
}

/// Règle l'adouci du côté `cote` de la clé `i` à `voulu` images (arrondi à l'image), ramené entre 0 et
/// ce que laissent la clé voisine et la poignée d'en face. Une ancienne clé adoucie est d'abord
/// convertie (`convertir`) ; si le partage avait réduit la poignée d'en face, elle prend sa valeur
/// effective, pour que ce qu'on voit soit ce qui se passe. Rend la valeur posée.
pub(crate) fn regler(keys: &mut [CameraKey], i: usize, cote: Cote, voulu: f32) -> f32 {
    let voulu = voulu.round().max(0.0);
    convertir(keys, i);
    let (a, b) = match cote {
        Cote::Sortie if i + 1 < keys.len() => (i, i + 1),
        Cote::Entree if i > 0 => (i - 1, i),
        // Pas de clé voisine : aucun mouvement de ce côté, la valeur est gardée telle quelle.
        _ => {
            match cote {
                Cote::Entree => keys[i].ease_in = voulu,
                Cote::Sortie => keys[i].ease_out = voulu,
            }
            return voulu;
        }
    };
    let span = (keys[b].frame - keys[a].frame).max(1) as f32;
    let (sortie_a, entree_b) = camera::interval_eases(&keys[a], &keys[b]);
    let brut = (keys[a].eases(span).1, keys[b].eases(span).0);
    if brut.0 + brut.1 > span + NUL {
        match cote {
            Cote::Sortie => keys[b].ease_in = entree_b,
            Cote::Entree => keys[a].ease_out = sortie_a,
        }
    }
    let en_face = match cote {
        Cote::Sortie => entree_b,
        Cote::Entree => sortie_a,
    };
    let v = voulu.min((span - en_face).max(0.0).floor());
    match cote {
        Cote::Sortie => keys[a].ease_out = v,
        Cote::Entree => keys[b].ease_in = v,
    }
    v
}

/// Ce qu'a fait le bouton « Adoucir ».
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Adouci {
    /// Le lissage posé : (entrée, sortie), en images (0 : pas de mouvement de ce côté, ou plus de place).
    Cree(f32, f32),
    /// Tout le lissage de la clé est retiré.
    Retire,
    /// La clé n'a aucune clé voisine : pas de mouvement à lisser.
    SansMouvement,
}

/// Le bouton « Adoucir » : sur une clé sans lissage, pose des valeurs réelles de chaque côté où la
/// caméra bouge (un demi-intervalle, comme l'ancien réglage, sans prendre la place de la poignée d'en
/// face) ; sur une clé déjà adoucie, retire tout.
pub(crate) fn adoucir(keys: &mut [CameraKey], i: usize) -> Adouci {
    if est_adoucie(&keys[i]) {
        keys[i].smooth = false;
        keys[i].ease_in = 0.0;
        keys[i].ease_out = 0.0;
        return Adouci::Retire;
    }
    if keys.len() < 2 {
        return Adouci::SansMouvement;
    }
    let cote = |span: f32, en_face: f32| (span * 0.5).min(span - en_face).max(0.0);
    let entree = (i > 0).then(|| {
        let span = (keys[i].frame - keys[i - 1].frame).max(1) as f32;
        cote(span, camera::interval_eases(&keys[i - 1], &keys[i]).0)
    });
    let sortie = (i + 1 < keys.len()).then(|| {
        let span = (keys[i + 1].frame - keys[i].frame).max(1) as f32;
        cote(span, camera::interval_eases(&keys[i], &keys[i + 1]).1)
    });
    keys[i].ease_in = entree.unwrap_or(0.0);
    keys[i].ease_out = sortie.unwrap_or(0.0);
    Adouci::Cree(entree.unwrap_or(0.0), sortie.unwrap_or(0.0))
}

// ---------------------------------------------------------------------- à l'écran

/// L'abscisse de la poignée (`v` images de sa clé, qui est à `x_cle`) ; jamais plus près de la clé que le repos
/// (`repos`) : une poignée nulle, ou trop courte pour sortir de la prise de la clé à ce zoom, attend là.
pub(crate) fn x_poignee(x_cle: f32, cote: Cote, v: f32, cell: f32) -> f32 {
    let d = (v * cell).max(repos(cell));
    match cote {
        Cote::Sortie => x_cle + d,
        Cote::Entree => x_cle - d,
    }
}

/// La poignée sous l'abscisse `x` : (rang de la clé, côté). Rien si une clé est là (`tol_cle` : sa zone
/// de prise) : la clé passe avant. `x_of` : l'abscisse du milieu d'une image ; `cell` : la largeur d'une
/// image.
pub(crate) fn sous_pointeur(keys: &[CameraKey], x: f32, x_of: &dyn Fn(usize) -> f32, cell: f32, tol_cle: f32) -> Option<(usize, Cote)> {
    if keys.iter().any(|k| (x_of(k.frame) - x).abs() <= tol_cle) {
        return None;
    }
    poignees(keys)
        .into_iter()
        .map(|(i, c, v)| (i, c, (x_poignee(x_of(keys[i].frame), c, v, cell) - x).abs()))
        .filter(|&(_, _, d)| d <= PRISE)
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(i, c, _)| (i, c))
}

/// Les valeurs voulues par la souris à l'abscisse `x`, en images, pour la poignée `cote` d'une clé
/// qui est à `x_cle` (arrondi à l'image).
pub(crate) fn images_voulues(x: f32, x_cle: f32, cote: Cote, cell: f32) -> f32 {
    let d = match cote {
        Cote::Sortie => x - x_cle,
        Cote::Entree => x_cle - x,
    };
    (d / cell.max(1e-3)).round().max(0.0)
}

/// Peint les poignées de lissage de `keys` sur la ligne `y` : un trait fin de la clé à la poignée, puis un
/// petit rond creux ; au repos (adouci nul), un rond plus petit et plus pâle, sans trait. `chaude` : celle
/// qu'on survole ou qu'on tient. À peindre avant les losanges, qui passent par-dessus.
#[allow(clippy::too_many_arguments)]
pub(crate) fn peindre(painter: &egui::Painter, keys: &[CameraKey], y: f32, x_of: &dyn Fn(usize) -> f32, cell: f32, couleur: Color32, fond: Color32, chaude: Option<(usize, Cote)>) {
    let clip = painter.clip_rect();
    for (i, cote, v) in poignees(keys) {
        let x_cle = x_of(keys[i].frame);
        let x = x_poignee(x_cle, cote, v, cell);
        if x.max(x_cle) < clip.left() - 8.0 || x.min(x_cle) > clip.right() + 8.0 {
            continue;
        }
        let clair = chaude == Some((i, cote));
        let au_repos = v <= NUL;
        if !au_repos {
            painter.line_segment([Pos2::new(x_cle, y), Pos2::new(x, y)], Stroke::new(1.0_f32, couleur.gamma_multiply(0.7)));
        }
        let (r, trait_) = if clair {
            (RAYON_CLAIR, Stroke::new(2.0_f32, crate::theme::contour()))
        } else if au_repos {
            (RAYON_REPOS, Stroke::new(1.0_f32, couleur.gamma_multiply(0.55)))
        } else {
            (RAYON, Stroke::new(1.5_f32, couleur))
        };
        painter.circle(Pos2::new(x, y), r, fond, trait_);
    }
}

// ---------------------------------------------------------------------- geste

/// Une poignée de lissage prise à la souris : sa clé (plan et image), son côté, les clés du plan telles
/// qu'elles étaient, la valeur posée, l'étape d'annulation de tout le geste.
pub(crate) struct EaseDrag {
    pub panel: u64,
    pub frame: usize,
    pub cote: Cote,
    before: Vec<CameraKey>,
    snap: Edit,
}

/// « 1 image », « 6 images », « 2,5 images ».
fn nb_images(v: f32) -> String {
    let nombre = if (v - v.round()).abs() < 0.05 { format!("{}", v.round()) } else { format!("{v:.1}").replace('.', ",") };
    format!("{nombre} image{}", if v > 1.0 { "s" } else { "" })
}

impl App {
    /// Commence à tirer la poignée `cote` de la clé de l'image `frame` du plan de rang `pi`. Faux si la
    /// clé n'existe pas.
    pub(crate) fn begin_ease_drag(&mut self, pi: usize, frame: usize, cote: Cote) -> bool {
        let Some(p) = self.doc.panels.get(pi) else { return false };
        if !p.camera.iter().any(|k| k.frame == frame) {
            return false;
        }
        let (panel, before) = (p.id, p.camera.clone());
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        self.ease_drag = Some(EaseDrag { panel, frame, cote, before, snap });
        true
    }

    /// La clé dont on tire la poignée : (plan, image).
    pub(crate) fn ease_drag_target(&self) -> Option<(u64, usize)> {
        self.ease_drag.as_ref().map(|d| (d.panel, d.frame))
    }

    /// La poignée suit la souris, à l'abscisse `x` (la clé est à `x_cle`, `cell` points par image) : les
    /// clés repartent de ce qu'elles étaient avant le geste.
    pub(crate) fn drag_ease(&mut self, x: f32, x_cle: f32, cell: f32) {
        let Some(d) = self.ease_drag.as_mut() else { return };
        let voulu = images_voulues(x, x_cle, d.cote, cell);
        let mut keys = d.before.clone();
        let Some(i) = keys.iter().position(|k| k.frame == d.frame) else { return };
        let v = regler(&mut keys, i, d.cote, voulu);
        let (panel, cote, frame) = (d.panel, d.cote, d.frame);
        if let Some(p) = self.doc.panels.iter_mut().find(|p| p.id == panel) {
            p.camera = keys;
        }
        let quoi = match cote {
            Cote::Entree => "d'entrée",
            Cote::Sortie => "de sortie",
        };
        let limite = if voulu > v { " (limite : la clé voisine ou l'autre poignée)" } else { "" };
        self.status = format!("Lissage {quoi} de la clé (image {}) : {}{limite}.", self.frame_label(frame), nb_images(v));
    }

    /// Lâche la poignée : une étape d'annulation, « Régler le lissage de la caméra », si quelque chose a changé.
    pub(crate) fn end_ease_drag(&mut self) {
        let Some(d) = self.ease_drag.take() else { return };
        let avant = d.before.clone();
        let change = self.doc.panels.iter().find(|p| p.id == d.panel).is_some_and(|p| p.camera != avant);
        if !change {
            return;
        }
        self.commit_as(d.snap, "Régler le lissage de la caméra");
    }

    /// Échap pendant le geste : les clés reviennent comme avant.
    pub(crate) fn cancel_ease_drag(&mut self) {
        let Some(d) = self.ease_drag.take() else { return };
        if let Some(p) = self.doc.panels.iter_mut().find(|p| p.id == d.panel) {
            p.camera = d.before;
        }
    }

    /// Le bouton « Adoucir » (`camera.cle_adoucir`) : voir `adoucir`.
    pub(crate) fn adoucir_cle_ici(&mut self) {
        let frame = self.cur_frame;
        let Some(i) = self.doc.panels[self.cur_panel].camera.iter().position(|k| k.frame == frame) else {
            self.status = "Place-toi sur une clé de caméra pour l'adoucir.".into();
            return;
        };
        if self.doc.panels[self.cur_panel].camera.len() < 2 && !est_adoucie(&self.doc.panels[self.cur_panel].camera[i]) {
            self.status = "Cette clé est seule dans le plan : la caméra ne bouge pas, rien à adoucir.".into();
            return;
        }
        self.record_step();
        let r = adoucir(&mut self.doc.panels[self.cur_panel].camera, i);
        self.status = match r {
            Adouci::Retire => "Lissage retiré : la caméra va à vitesse constante autour de cette clé.".into(),
            Adouci::SansMouvement => "Cette clé est seule dans le plan : rien à adoucir.".into(),
            Adouci::Cree(e, o) if e <= 0.0 && o <= 0.0 => "Pas de place pour un lissage ici : les clés voisines prennent déjà tout l'intervalle.".into(),
            Adouci::Cree(e, o) => {
                let mut dit = Vec::new();
                if e > 0.0 {
                    dit.push(format!("{} avant la clé", nb_images(e)));
                }
                if o > 0.0 {
                    dit.push(format!("{} après", nb_images(o)));
                }
                format!("Lissage posé : {}. Tire les petits ronds creux pour le régler.", dit.join(", "))
            }
        };
    }
}
