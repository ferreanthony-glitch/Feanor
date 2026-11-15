//! Des panneaux moins dans les chiffres, plus visuels (partie 163, Anthony : « des panneaux moins dans les chiffres, plus visuels, plus
//! facilement compréhensibles, qui donnent envie d'être manipulés »). Ce module tient ce qui ne dépend pas de l'application : les
//! tuiles dessinées (un pictogramme peint au `Painter`, son nom dessous, allumée quand elle dit l'état d'aujourd'hui), le repli
//! « Précis » où restent les champs en chiffres, les pictogrammes eux-mêmes, et les petits calculs que ces dessins appliquent au projet
//! (le cadre B d'un mouvement de caméra des Vignettes, les adoucis d'une courbe de démarrage, l'échelle de la barre de durée). Les
//! méthodes qui écrivent dans le projet sont dans `panneauxvisuelsui.rs`.
//!
//! Aucune image : tout est peint, aux couleurs du thème (`theme.rs`), l'accent pour ce qui est allumé.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};

use crate::camera::{self, Camera, CameraKey};
use crate::theme;

// ------------------------------------------------------------------ les tuiles

/// Hauteur du nom écrit sous le pictogramme d'une tuile.
const NOM_H: f32 = 15.0;

/// Une tuile : un pictogramme peint par `dessin` (dans la zone qu'on lui donne, avec l'encre du moment), son nom dessous. Allumée
/// (`on`) : le fond de sélection et un filet d'accent. Elle se clique ; sa bulle est posée par l'appelant.
pub(crate) fn tuile(ui: &mut Ui, largeur: f32, haut_dessin: f32, on: bool, nom: &str, dessin: impl FnOnce(&Painter, Rect, Color32)) -> Response {
    let h = haut_dessin + NOM_H + 6.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(largeur.max(24.0), h), Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let fond = if on {
            theme::selection()
        } else if resp.hovered() {
            theme::controle_survol()
        } else {
            theme::controle()
        };
        p.rect_filled(rect, 5.0, fond);
        // (Un filet de 1,25 : celui de 1,5 en accent est le liseré d'un plan choisi sur la planche.)
        let bord = if on { Stroke::new(1.25_f32, theme::accent()) } else { Stroke::new(1.0_f32, theme::bord_fort()) };
        p.rect_stroke(rect, 5.0, bord, StrokeKind::Inside);
        let encre = encre(on);
        let zone = Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2(rect.width() - 8.0, haut_dessin));
        dessin(p, zone, encre);
        p.text(pos2(rect.center().x, rect.bottom() - 3.0), Align2::CENTER_BOTTOM, nom, FontId::proportional(11.0), encre);
    }
    resp
}

/// L'encre d'un dessin posé sur une tuile allumée ou non.
pub(crate) fn encre(on: bool) -> Color32 {
    if on {
        theme::sur_selection()
    } else {
        theme::texte()
    }
}

/// La largeur de chacune de `n` tuiles côte à côte dans la place qui reste (écart `ecart`).
pub(crate) fn largeur_tuiles(ui: &Ui, n: usize, ecart: f32) -> f32 {
    let n = n.max(1) as f32;
    ((ui.available_width() - ecart * (n - 1.0)) / n).floor().max(30.0)
}

/// Le repli « Précis » : ce qui se règle en chiffres, fermé d'origine. `id` le distingue des autres replis (son état ouvert ou fermé
/// est retenu pendant la séance).
pub(crate) fn precis<R>(ui: &mut Ui, id: &str, contenu: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let cle = cle_precis(id);
    let ouvert = ui.ctx().data(|d| d.get_temp::<bool>(cle)).unwrap_or(false);
    #[cfg(test)]
    let ouvert = ouvert || TOUT_OUVERT.with(std::cell::Cell::get);
    // (Les flèches de la police des icônes : celles du texte n'ont pas de glyphe dans la police de l'interface.)
    let fleche = if ouvert { egui_phosphor::regular::CARET_DOWN } else { egui_phosphor::regular::CARET_RIGHT };
    let texte = egui::RichText::new(format!("{fleche} Précis")).small().color(theme::texte_doux());
    let resp = ui.add(egui::Button::new(texte).frame(false));
    let resp = crate::bulles::bulle(resp, &format!("Précis : {id}"), if ouvert { "Replier les réglages en chiffres." } else { "Les mêmes réglages, en chiffres (et ceux qui n'ont pas de dessin)." }, "");
    if resp.clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(cle, !ouvert));
    }
    if !ouvert {
        return None;
    }
    Some(ui.indent(cle, contenu).inner)
}

fn cle_precis(id: &str) -> egui::Id {
    egui::Id::new(("precis_163", id))
}

#[cfg(test)]
thread_local! {
    static TOUT_OUVERT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Les essais d'avant la partie 163, qui lisent les réglages en chiffres dans leur fenêtre : tous les « Précis » ouverts (sur ce fil).
#[cfg(test)]
pub(crate) fn tout_ouvrir(on: bool) {
    TOUT_OUVERT.with(|c| c.set(on));
}

/// Ouvre ou ferme le repli « Précis » `id` sans clic (les essais).
#[cfg(test)]
pub(crate) fn ouvrir_precis(ctx: &egui::Context, id: &str, ouvert: bool) {
    ctx.data_mut(|d| d.insert_temp(cle_precis(id), ouvert));
}

/// Un petit titre gris au-dessus d'une rangée de dessins.
pub(crate) fn titre(ui: &mut Ui, texte: &str) {
    ui.label(egui::RichText::new(texte).small().color(theme::texte_doux()));
}

// ------------------------------------------------------------------ des formes

fn trait_(c: Color32) -> Stroke {
    Stroke::new(1.2_f32, c)
}

fn cadre(p: &Painter, r: Rect, c: Color32) {
    p.rect_stroke(r, 0.0, trait_(c), StrokeKind::Middle);
}

fn fleche(p: &Painter, a: Pos2, b: Pos2, c: Color32) {
    p.line_segment([a, b], trait_(c));
    let d = (b - a).normalized();
    let n = vec2(-d.y, d.x);
    let t = 4.0;
    p.add(Shape::convex_polygon(vec![b, b - d * t + n * t * 0.6, b - d * t - n * t * 0.6], c, Stroke::NONE));
}

/// Les quatre coins d'un rectangle de centre `centre`, de taille `taille`, tourné de `degres`.
fn coins_tournes(centre: Pos2, taille: Vec2, degres: f32) -> Vec<Pos2> {
    let (s, co) = degres.to_radians().sin_cos();
    let h = taille * 0.5;
    [vec2(-h.x, -h.y), vec2(h.x, -h.y), vec2(h.x, h.y), vec2(-h.x, h.y)].iter().map(|v| centre + vec2(v.x * co - v.y * s, v.x * s + v.y * co)).collect()
}

/// Le plus grand rectangle de rapport `rapport` (largeur / hauteur) centré dans `r`.
pub(crate) fn au_rapport(r: Rect, rapport: f32) -> Rect {
    let rapport = rapport.max(0.05);
    let (w, h) = if r.width() / r.height().max(1e-3) > rapport { (r.height() * rapport, r.height()) } else { (r.width(), r.width() / rapport) };
    Rect::from_center_size(r.center(), vec2(w, h))
}

// ------------------------------------------------------------------ 163.2 : les mouvements de caméra des Vignettes

/// Les mouvements qu'une vignette sait poser d'un clic (partie 163.2) : seulement ceux que la caméra de FEANOR fait déjà (`Camera` :
/// position, zoom, rotation du cadre, panoramique). Le tremblement n'y est pas : il existe comme effet de caméra (outil Caméra), mais
/// il pose une clé par secousse, autant de cadres sur la case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mouvement {
    Travelling,
    ZoomAvant,
    ZoomArriere,
    Pano,
    Rotation,
}

impl Mouvement {
    pub(crate) const TOUS: [Mouvement; 5] = [Mouvement::Travelling, Mouvement::ZoomAvant, Mouvement::ZoomArriere, Mouvement::Pano, Mouvement::Rotation];

    pub(crate) fn nom(self) -> &'static str {
        match self {
            Mouvement::Travelling => "Travelling",
            Mouvement::ZoomAvant => "Zoom avant",
            Mouvement::ZoomArriere => "Zoom arrière",
            Mouvement::Pano => "Pano",
            Mouvement::Rotation => "Rotation",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Mouvement::Travelling => "Le cadre B glisse à côté de A, d'une demi-largeur : la caméra se déplace sur la feuille.",
            Mouvement::ZoomAvant => "Le cadre B, plus petit, au centre de A : la caméra se rapproche.",
            Mouvement::ZoomArriere => "Le cadre B, plus grand, autour de A : la caméra s'éloigne.",
            Mouvement::Pano => "La caméra tourne sur elle-même vers la droite (panoramique de 20°) : B se déforme sur la feuille.",
            Mouvement::Rotation => "Le cadre B tourne de 15° sur lui-même.",
        }
    }

    /// Le pictogramme.
    pub(crate) fn dessiner(self, p: &Painter, r: Rect, c: Color32) {
        let r = au_rapport(r, 1.8);
        match self {
            Mouvement::Travelling => {
                let w = r.width() * 0.38;
                let h = w * 0.7;
                let a = Rect::from_center_size(pos2(r.left() + w * 0.5 + 2.0, r.center().y), vec2(w, h));
                let b = a.translate(vec2(r.width() - w - 4.0, 0.0));
                cadre(p, a, c.gamma_multiply(0.6));
                cadre(p, b, c);
                fleche(p, pos2(a.right() + 2.0, r.center().y), pos2(b.left() - 2.0, r.center().y), c);
            }
            Mouvement::ZoomAvant | Mouvement::ZoomArriere => {
                let grand = Rect::from_center_size(r.center(), vec2(r.width() * 0.8, r.width() * 0.8 / 1.7));
                let petit = Rect::from_center_size(r.center(), grand.size() * 0.42);
                let avant = self == Mouvement::ZoomAvant;
                cadre(p, grand, if avant { c.gamma_multiply(0.6) } else { c });
                cadre(p, petit, if avant { c } else { c.gamma_multiply(0.6) });
                for (g, q) in [(grand.left_top(), petit.left_top()), (grand.right_top(), petit.right_top()), (grand.right_bottom(), petit.right_bottom()), (grand.left_bottom(), petit.left_bottom())] {
                    let (a, b) = (g + (q - g) * 0.15, g + (q - g) * 0.8);
                    if avant {
                        fleche(p, a, b, c);
                    } else {
                        fleche(p, b, a, c);
                    }
                }
            }
            Mouvement::Pano => {
                let centre = pos2(r.center().x, r.bottom() + r.height() * 0.15);
                let rayon = r.height() * 0.85;
                let arc: Vec<Pos2> = (0..=16).map(|k| {
                    let t = (-60.0 + 120.0 * k as f32 / 16.0_f32).to_radians();
                    centre + vec2(t.sin(), -t.cos()) * rayon
                }).collect();
                p.add(Shape::line(arc.clone(), trait_(c.gamma_multiply(0.6))));
                let haut = arc[8];
                let w = r.width() * 0.26;
                cadre(p, Rect::from_center_size(haut + vec2(0.0, 6.0), vec2(w, w * 0.65)), c);
                let fin = arc[14];
                fleche(p, arc[11], fin, c);
            }
            Mouvement::Rotation => {
                let w = r.width() * 0.5;
                let taille = vec2(w, w * 0.62);
                p.add(Shape::closed_line(coins_tournes(r.center(), taille, 0.0), trait_(c.gamma_multiply(0.6))));
                p.add(Shape::closed_line(coins_tournes(r.center(), taille, -15.0), trait_(c)));
            }
        }
    }
}

/// Le cadre B que pose le mouvement `m` en partant du cadre A (`a`), sur une feuille au cadre final `frame`.
pub(crate) fn cadre_b(m: Mouvement, frame: [usize; 2], a: Camera) -> Camera {
    match m {
        Mouvement::Travelling => {
            let taille = vec2(frame[0] as f32, frame[1] as f32) / a.magnification(frame).max(1e-3);
            crate::vignettecam::cadre_suivant(frame, a, taille)
        }
        Mouvement::ZoomAvant => Camera { zoom: a.zoom * ZOOM, ..a },
        Mouvement::ZoomArriere => Camera { zoom: (a.zoom / ZOOM).max(0.05), ..a },
        Mouvement::Pano => Camera { pan: (a.pan + 20.0).clamp(-85.0, 85.0), ..a }.limited(frame),
        Mouvement::Rotation => Camera { angle: a.angle + 15.0, ..a },
    }
}

/// Le facteur de zoom des pictogrammes Zoom avant et Zoom arrière.
pub(crate) const ZOOM: f32 = 1.6;

/// Ce que fait le passage du cadre A au cadre B : chaque sorte de mouvement qui y est (un mouvement composé en a plusieurs).
pub(crate) fn mouvements_de(frame: [usize; 2], a: &Camera, b: &Camera) -> Vec<Mouvement> {
    let mut out = Vec::new();
    let largeur = frame[0] as f32 / a.magnification(frame).max(1e-3);
    if (b.x - a.x).hypot(b.y - a.y) > largeur * 0.02 {
        out.push(Mouvement::Travelling);
    }
    let (ma, mb) = (a.magnification(frame), b.magnification(frame));
    if mb > ma * 1.01 {
        out.push(Mouvement::ZoomAvant);
    } else if mb < ma / 1.01 {
        out.push(Mouvement::ZoomArriere);
    }
    if (b.pan - a.pan).abs() > 0.5 || (b.tilt - a.tilt).abs() > 0.5 {
        out.push(Mouvement::Pano);
    }
    if (b.angle - a.angle).abs() > 0.5 {
        out.push(Mouvement::Rotation);
    }
    out
}

// ------------------------------------------------------------------ 163.2 : comment ça démarre et s'arrête

/// Les trois façons de démarrer et de s'arrêter d'un mouvement de caméra (les adoucis des clés, `CameraKey::ease_in`, `ease_out`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Courbe {
    /// Part lentement, arrive lentement (adoucis d'une demi-durée de chaque côté).
    Doux,
    /// Vitesse constante (aucun adouci).
    Constant,
    /// Part d'un coup, ralentit jusqu'à l'arrêt (adouci d'arrivée sur toute la durée).
    Brusque,
}

impl Courbe {
    pub(crate) const TOUTES: [Courbe; 3] = [Courbe::Doux, Courbe::Constant, Courbe::Brusque];

    pub(crate) fn nom(self) -> &'static str {
        match self {
            Courbe::Doux => "Doux",
            Courbe::Constant => "Constant",
            Courbe::Brusque => "Brusque",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Courbe::Doux => "La caméra part lentement, accélère, puis ralentit avant de s'arrêter.",
            Courbe::Constant => "La caméra va à la même vitesse du début à la fin.",
            Courbe::Brusque => "La caméra part d'un coup, puis ralentit jusqu'à l'arrêt.",
        }
    }

    /// Les adoucis (sortie de la clé de départ, entrée de la clé d'arrivée), en images, d'un intervalle de `span` images.
    pub(crate) fn adoucis(self, span: f32) -> (f32, f32) {
        match self {
            Courbe::Doux => (span * 0.5, span * 0.5),
            Courbe::Constant => (0.0, 0.0),
            Courbe::Brusque => (0.0, span),
        }
    }

    /// La courbe que jouent les clés `a` puis `b` (la suivante), si c'est l'une des trois.
    pub(crate) fn de(a: &CameraKey, b: &CameraKey) -> Option<Courbe> {
        let span = (b.frame.saturating_sub(a.frame)).max(1) as f32;
        let (out, inn) = camera::interval_eases(a, b);
        let tol = span * 0.02 + 0.01;
        Courbe::TOUTES.into_iter().find(|c| {
            let (o, i) = c.adoucis(span);
            let (o, i) = camera::shared_eases(span, o, i);
            (out - o).abs() <= tol && (inn - i).abs() <= tol
        })
    }

    /// Le pictogramme : la vraie courbe d'avancement (`camera::ease`), du départ (en bas à gauche) à l'arrivée (en haut à droite).
    pub(crate) fn dessiner(self, p: &Painter, r: Rect, c: Color32) {
        let r = r.shrink2(vec2(r.width() * 0.12, 3.0));
        let (o, i) = self.adoucis(1.0);
        let points: Vec<Pos2> = (0..=24).map(|k| {
            let t = k as f32 / 24.0;
            let v = camera::ease(t, 1.0, o, i);
            pos2(r.left() + t * r.width(), r.bottom() - v * r.height())
        }).collect();
        p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0_f32, c.gamma_multiply(0.35)));
        p.add(Shape::line(points, Stroke::new(2.0_f32, c)));
    }
}

/// Les clés `keys` dont chaque intervalle de `rangs` (deux clés qui se suivent) joue la courbe `c`. Les autres adoucis ne changent pas.
pub(crate) fn avec_la_courbe(keys: &[CameraKey], rangs: std::ops::Range<usize>, c: Courbe) -> Vec<CameraKey> {
    let mut out = keys.to_vec();
    let fin = rangs.end.min(out.len());
    for i in rangs.start..fin.saturating_sub(1) {
        let span = (out[i + 1].frame.saturating_sub(out[i].frame)).max(1) as f32;
        let (o, n) = c.adoucis(span);
        out[i].ease_out = o;
        out[i + 1].ease_in = n;
        // (L'ancien « adoucir » vaudrait un demi-intervalle là où un adouci est nul : il cède la place aux adoucis en images.)
        out[i].smooth = false;
        out[i + 1].smooth = false;
    }
    out
}

/// La courbe que jouent tous les intervalles de `rangs`, si c'est la même (rien sinon, ou s'il n'y a pas d'intervalle).
pub(crate) fn courbe_des(keys: &[CameraKey], rangs: std::ops::Range<usize>) -> Option<Courbe> {
    let fin = rangs.end.min(keys.len());
    if fin < rangs.start + 2 {
        return None;
    }
    let mut c = None;
    for i in rangs.start..fin - 1 {
        let ici = Courbe::de(&keys[i], &keys[i + 1])?;
        if c.is_some_and(|d| d != ici) {
            return None;
        }
        c = Some(ici);
    }
    c
}

// ------------------------------------------------------------------ 163.1 : la barre de durée

/// Combien d'images la barre de durée montre pour un plan de `len` images : de quoi l'allonger de moitié (une seconde au moins),
/// arrondi à la seconde, deux secondes au moins.
pub(crate) fn etendue_barre(len: usize, fps: u32) -> usize {
    let fps = fps.max(1) as usize;
    let v = (len * 3 / 2).max(len + fps).max(2 * fps);
    v.div_ceil(fps) * fps
}

/// Le texte posé sur la barre : « 1 s 20 i · 44 images ».
pub(crate) fn texte_duree(len: usize, fps: u32) -> String {
    let fps = fps.max(1) as usize;
    let images = if len > 1 { "images" } else { "image" };
    if len >= fps {
        format!("{} s {} i · {len} {images}", len / fps, len % fps)
    } else {
        format!("{len} i · {len} {images}")
    }
}

/// La durée (en images, au moins une) que donne un pointeur en `x` sur une barre qui va de `gauche` à `droite` et montre `etendue`
/// images.
pub(crate) fn duree_a(x: f32, gauche: f32, droite: f32, etendue: usize) -> usize {
    let t = (x - gauche) / (droite - gauche).max(1.0);
    ((t * etendue as f32).round().max(1.0) as usize).min(100_000)
}

// ------------------------------------------------------------------ 163.3 : la planche

/// Le pictogramme d'une page lue en lignes (`japonais` faux) ou à la japonaise : les cases, et une flèche qui suit la lecture.
pub(crate) fn dessiner_sens(p: &Painter, r: Rect, c: Color32, japonais: bool) {
    let r = au_rapport(r, 1.8);
    let (cw, ch) = (r.width() * 0.22, r.width() * 0.22 * 0.6);
    let gx = (r.width() - 3.0 * cw) / 4.0;
    let gy = (r.height() - 2.0 * ch) / 3.0;
    let case = |col: f32, rang: f32| Rect::from_min_size(pos2(r.left() + gx + col * (cw + gx), r.top() + gy * 0.6 + rang * (ch + gy)), vec2(cw, ch));
    if japonais {
        // De haut en bas, puis la colonne suivante, à droite.
        let cases = [case(0.0, 0.0), case(0.0, 1.0), case(1.0, 0.0), case(1.0, 1.0), case(2.0, 0.0)];
        for q in cases {
            cadre(p, q, c);
        }
        let x = cases[0].left() - gx * 0.5;
        fleche(p, pos2(x, cases[0].top()), pos2(x, cases[1].bottom()), theme::accent());
    } else {
        let cases = [case(0.0, 0.0), case(1.0, 0.0), case(2.0, 0.0), case(0.0, 1.0), case(1.0, 1.0)];
        for q in cases {
            cadre(p, q, c);
        }
        let y = cases[3].bottom() + gy * 0.45;
        fleche(p, pos2(cases[0].left(), y), pos2(cases[2].right(), y), theme::accent());
    }
}

/// Le nombre de cases par ligne que donne un pointeur en `x` (ou `y` à la japonaise) sur une mini-planche dont la première case
/// commence en `debut` et dont chaque case prend `pas` points, entre une et `max` cases.
pub(crate) fn cases_a(pos: f32, debut: f32, pas: f32, max: usize) -> usize {
    (((pos - debut) / pas.max(1.0)).round().max(1.0) as usize).min(max.max(1))
}

// ------------------------------------------------------------------ 163.4 : les exports

/// Les familles d'export montrées en tuiles dans la fenêtre d'export (partie 163.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Famille {
    /// Le storyboard en PDF (des pages de cases).
    Storyboard,
    /// Le film (MP4, ProRes, DNxHR) ou un GIF animé.
    Film,
    /// Des images une à une (une par image du film, ou par plan, par case, par clé).
    Images,
    /// Pour le montage (EDL, XML, FCPXML, OpenTimelineIO) : sa propre fenêtre.
    Montage,
}

impl Famille {
    pub(crate) const TOUTES: [Famille; 4] = [Famille::Storyboard, Famille::Film, Famille::Images, Famille::Montage];

    pub(crate) fn nom(self) -> &'static str {
        match self {
            Famille::Storyboard => "Storyboard PDF",
            Famille::Film => "Film vidéo",
            Famille::Images => "Images une à une",
            Famille::Montage => "Pour le montage",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Famille::Storyboard => "Des pages de cases, avec leurs textes : à imprimer, à envoyer.",
            Famille::Film => "L'animatique en vidéo (MP4, ProRes, DNxHR) ou en GIF animé.",
            Famille::Images => "Un fichier par image du film, ou par plan, par case, par clé.",
            Famille::Montage => "La liste des plans pour un logiciel de montage (EDL, XML, FCPXML, OpenTimelineIO), dans sa fenêtre.",
        }
    }

    /// La famille d'un format de la fenêtre d'export (rien pour le son, les calques séparés, la caméra : ils gardent leur fenêtre telle
    /// quelle).
    pub(crate) fn de(f: crate::export::Format) -> Option<Famille> {
        use crate::export::Format;
        match f {
            Format::Pdf => Some(Famille::Storyboard),
            Format::Mp4 | Format::ProRes | Format::ProRes4444 | Format::DnxHr | Format::Gif => Some(Famille::Film),
            Format::Png | Format::Stills => Some(Famille::Images),
            _ => None,
        }
    }

    /// Le pictogramme : ce qu'on obtient.
    pub(crate) fn dessiner(self, p: &Painter, r: Rect, c: Color32) {
        let clair = c.gamma_multiply(0.85);
        let sourd = c.gamma_multiply(0.35);
        match self {
            Famille::Storyboard => {
                let page = au_rapport(r, 0.8);
                p.rect_filled(page, 1.0, clair);
                let (w, h) = ((page.width() - 9.0) / 2.0, (page.height() - 12.0) / 3.0);
                for k in 0..6 {
                    let (col, rang) = ((k % 2) as f32, (k / 2) as f32);
                    p.rect_filled(Rect::from_min_size(page.min + vec2(3.0 + col * (w + 3.0), 3.0 + rang * (h + 3.0)), vec2(w, h * 0.7)), 0.0, sourd);
                }
            }
            Famille::Film => {
                let ecran = au_rapport(r.shrink2(vec2(0.0, 4.0)), 1.8);
                p.rect_filled(ecran, 1.0, clair);
                let m = ecran.center();
                let t = ecran.height() * 0.22;
                p.add(Shape::convex_polygon(vec![m + vec2(-t * 0.7, -t), m + vec2(t, 0.0), m + vec2(-t * 0.7, t)], sourd, Stroke::NONE));
                let barre = Rect::from_min_size(pos2(ecran.left(), ecran.bottom() + 2.0), vec2(ecran.width(), 3.0));
                p.rect_filled(barre, 0.0, sourd);
                p.rect_filled(Rect::from_min_size(barre.min, vec2(barre.width() * 0.38, 3.0)), 0.0, theme::accent());
            }
            Famille::Images => {
                let une = au_rapport(r.shrink(6.0), 1.7);
                for (k, a) in [(2.0_f32, 0.35_f32), (1.0, 0.6), (0.0, 0.85)] {
                    let q = une.translate(vec2(-k * 5.0, k * 5.0) + vec2(5.0, -5.0));
                    p.rect_filled(q, 1.0, c.gamma_multiply(a));
                }
            }
            Famille::Montage => {
                let piste = Rect::from_center_size(r.center(), vec2(r.width() * 0.9, r.height() * 0.5));
                p.rect_filled(piste, 1.0, sourd);
                let mut x = piste.left() + 3.0;
                for part in [0.16_f32, 0.28, 0.1, 0.22] {
                    let w = piste.width() * part;
                    p.rect_filled(Rect::from_min_size(pos2(x, piste.top() + 3.0), vec2(w, piste.height() * 0.45)), 0.0, clair);
                    x += w + 3.0;
                }
                let y = piste.bottom() - 4.0;
                p.line_segment([pos2(piste.left() + 3.0, y), pos2(piste.right() - 3.0, y)], Stroke::new(1.0_f32, theme::accent()));
            }
        }
    }
}
