//! Partie 206 (Anthony, 2026-11-21) : ce que montre une clé de dessin sur la ligne de son calque, dans la timeline du Storyboard (et
//! le montage, qui peint les mêmes pistes, `montagerows::paint_layer_cell`) et dans celle de l'Animation (`timeline.rs`). Le look ne
//! change pas (les deux tons qui alternent, `cles_alternees.rs`, les couleurs du thème) ; s'y ajoutent :
//!
//! - **B, la coupure** : un trait foncé au début de chaque clé (toujours) ;
//! - **F, la clé vide** (F4, un dessin sans trait) : partie 208 (Anthony : « Plutôt que des rayures, une clé vide n'affiche rien,
//!   si ce n'est le gris du fond de la timeline ») : ni couleur, ni vignette, ni durée, ni soulignement (`cle_vide` ; les timelines
//!   ne l'habillent pas). Elle n'est vide qu'à l'affichage (Anthony : « On peut agir dessus comme une clé pleine ») : ses deux
//!   limites restent lisibles (la coupure à son début, `peindre_coupure`, et celle de la clé suivante), choisie elle a le même
//!   cadre que les autres, son bord s'allume au survol ;
//! - **G, la clé sous la tête de lecture** soulignée, sur chaque calque (toujours) ;
//! - **A, la vignette du dessin** dans la clé assez large (option, éteinte d'origine : `Settings::cles_vignettes`), prise dans le
//!   cache des vignettes de calque (`layersui::layer_thumb`, composées sur un fil de fond ; rien n'est rendu sur le fil de l'écran) ;
//! - **C, la durée** écrite dans la clé assez large, « 12 i » (option, éteinte d'origine : `Settings::cles_durees`) ;
//! - **E, pendant un glisser** de bord ou de fin de plan : le fantôme en pointillés de la place d'avant (`fantomes`) et une petite
//!   bulle près du pointeur (`App::bulle_du_geste`).
//!
//! (D, le bord qui s'allume au survol et pendant tout le geste, est peint par chaque timeline avec sa poignée :
//! `peindre_bord_allume`, partie 208 : une bande claire de `BORD_ALLUME` points bordée de sombre.)

use std::cell::RefCell;

use eframe::egui::{self, pos2, vec2, Color32, FontId, Painter, Rect, Stroke};

use crate::app::App;
use crate::document::{CelId, Document, Layer};

/// B : le trait de la coupure au début de chaque clé.
pub(crate) const COUPURE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 150);
pub(crate) const COUPURE_W: f32 = 1.5;
/// G : le soulignement de la clé sous la tête de lecture.
pub(crate) const SOULIGNE_H: f32 = 2.0;
/// D : la largeur de la bande claire du bord qui s'allume au survol et pendant le geste (partie 208 : 4 points pleins, au lieu de
/// 5 points à peine éclaircis).
pub(crate) const BORD_ALLUME: f32 = 4.0;
/// D : la bande du bord allumé (le contour du thème, presque opaque) et le liseré sombre de chaque côté, qui la détache de la clé.
const BORD_ALLUME_ALPHA: f32 = 0.92;
const BORD_LISERE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 170);

/// Ce qu'une timeline donne à `habiller` pour les clés d'un calque.
pub(crate) struct Habit<'a> {
    /// F (partie 208) : la clé de ce dessin est vide (`cle_vide`) : la timeline n'en peint rien, ni fond, ni habit.
    pub vide: &'a dyn Fn(CelId) -> bool,
    /// L'image de la tête de lecture dans ce plan (rien : la tête n'est pas dans ce plan).
    pub tete: Option<usize>,
    /// C : écrire la durée.
    pub durees: bool,
    /// A : la vignette du dessin ; les cases où la poser (le rectangle, le dessin, le calque visible) s'ajoutent ici, la timeline les
    /// peint après les clés.
    pub vignettes: Option<&'a RefCell<Vec<(Rect, CelId, bool)>>>,
    /// Une bande fine (piste repliée) : ni vignette, ni durée, juste la coupure et le soulignement.
    pub fine: bool,
}

/// F (partie 208) : la clé de `cel` sur le calque `layer` est vide, elle n'affiche rien : un dessin sans trait (F4). Pas sur un calque
/// d'effet (ses dessins sont tous vides, ses clés se montrent), ni quand un calque de couleur y a des griffonnages (ils remplissent
/// des zones).
pub(crate) fn cle_vide(doc: &Document, layer: &Layer, cel: CelId) -> bool {
    layer.effet.is_none()
        && doc.cels.get(&cel).is_none_or(|c| c.is_empty())
        && layer.couleur.as_ref().and_then(|c| c.griffonnages.get(&cel)).and_then(|g| doc.cels.get(g)).is_none_or(|c| c.is_empty())
}

/// B : la coupure au début de la clé peinte dans `seg` (aussi au début d'une clé vide, partie 208, qui n'a rien d'autre).
pub(crate) fn peindre_coupure(painter: &Painter, seg: Rect) {
    if seg.width() <= 0.0 || !seg.intersects(painter.clip_rect()) {
        return;
    }
    painter.line_segment([pos2(seg.left() + COUPURE_W * 0.5, seg.top()), pos2(seg.left() + COUPURE_W * 0.5, seg.bottom())], Stroke::new(COUPURE_W, COUPURE));
}

/// D (partie 208) : le bord allumé à l'abscisse `x`, de `haut` à `bas` : une bande claire de `BORD_ALLUME` points, un liseré sombre
/// de chaque côté. Les clés gardent leurs couleurs.
pub(crate) fn peindre_bord_allume(painter: &Painter, x: f32, haut: f32, bas: f32) {
    let demi = BORD_ALLUME * 0.5;
    let bande = Rect::from_min_max(pos2(x - demi, haut), pos2(x + demi, bas));
    painter.rect_filled(bande, 1.0, crate::theme::contour().gamma_multiply(BORD_ALLUME_ALPHA));
    for bord in [bande.left() - 0.5, bande.right() + 0.5] {
        painter.line_segment([pos2(bord, haut), pos2(bord, bas)], Stroke::new(1.0_f32, BORD_LISERE));
    }
}

/// La bulle du geste sur la fin d'un plan (partie 208, la même dans l'Animation, la timeline du Storyboard et le montage) :
/// « Durée du plan : 1s 16 (-8 i) », et « · toutes les clés en proportion » avec Maj ou « Étirer en proportion ».
pub(crate) fn bulle_fin_du_plan(n: usize, fps: u32, ecart: isize, proportion: bool) -> String {
    let mut t = format!("Durée du plan : {} ({ecart:+} i)", crate::storyboard::duration_label(n, fps));
    if proportion {
        t += " · toutes les clés en proportion";
    }
    t
}

/// Le texte de la durée d'une clé de `n` images.
pub(crate) fn texte_duree(n: usize) -> String {
    format!("{n} i")
}

/// La clé (dessin `cel`, images `start` à `end` exclue) peinte dans `seg` : sa coupure, son soulignement sous la tête de lecture, et,
/// en option, sa vignette et sa durée. (Une clé vide, `cle_vide`, n'est pas habillée : la timeline ne l'appelle pas.)
#[allow(clippy::too_many_arguments)]
pub(crate) fn habiller(painter: &Painter, seg: Rect, start: usize, end: usize, cel: CelId, visible: bool, encre: Color32, h: &Habit) {
    if seg.width() <= 0.0 || !seg.intersects(painter.clip_rect()) {
        return;
    }
    let mut libre = seg.left() + 2.0;
    if let Some(cases) = h.vignettes.filter(|_| !h.fine) {
        let th = seg.height() - 4.0;
        let tw = th * 16.0 / 9.0;
        // (Assez large : au moins deux fois la hauteur de la vignette, et la place de la vignette entière.)
        if th >= 8.0 && seg.width() >= (2.0 * th).max(tw + 6.0) {
            let at = Rect::from_min_size(pos2(seg.left() + 3.0, seg.top() + 2.0), vec2(tw, th));
            cases.borrow_mut().push((at, cel, visible));
            libre = at.right() + 3.0;
        }
    }
    if h.durees && !h.fine {
        let texte = texte_duree(end - start);
        let galley = painter.layout_no_wrap(texte, FontId::proportional(9.5), encre);
        if galley.size().x + 4.0 <= seg.right() - libre {
            painter.galley(pos2(seg.right() - 3.0 - galley.size().x, seg.center().y - galley.size().y * 0.5), galley, encre);
        }
    }
    // B : la coupure (par-dessus la vignette).
    peindre_coupure(painter, seg);
    // G : la clé sous la tête de lecture.
    if h.tete.is_some_and(|f| f >= start && f < end) {
        let r = Rect::from_min_max(pos2(seg.left(), seg.bottom() - SOULIGNE_H), seg.right_bottom());
        painter.rect_filled(r, 0.0, crate::theme::image_courante());
    }
}

/// E : les clés de `avant` qui n'ont plus la même place dans `apres` (images de début et de fin exclue) : leur fantôme en pointillés.
pub(crate) fn fantomes(avant: &Layer, apres: &Layer) -> Vec<(usize, usize)> {
    crate::handles::runs(avant)
        .into_iter()
        .filter(|r| r.2.is_some())
        .filter(|r| {
            let (s, e) = apres.run_bounds(r.0);
            !(s == r.0 && e == r.1 && apres.frames.get(r.0).copied().flatten() == r.2)
        })
        .map(|r| (r.0, r.1))
        .collect()
}

/// Le fantôme en pointillés d'une clé dans `seg`.
pub(crate) fn peindre_fantome(painter: &Painter, seg: Rect) {
    let stroke = Stroke::new(1.0_f32, crate::theme::contour());
    let pts = [seg.left_top(), seg.right_top(), seg.right_bottom(), seg.left_bottom(), seg.left_top()];
    for w in pts.windows(2) {
        painter.extend(egui::Shape::dashed_line(&[w[0], w[1]], stroke, 3.0, 2.0));
    }
}

impl App {
    /// A : les vignettes des clés retenues en peignant les pistes (`Habit::vignettes`), posées par-dessus. Prises dans le cache des
    /// vignettes de calque, composées sur un fil de fond (`layer_thumb`) : rien n'est rendu ici.
    pub(crate) fn peindre_vignettes_des_cles(&mut self, ctx: &egui::Context, painter: &Painter) {
        let cases = std::mem::take(&mut *self.cles_vignettes_a_peindre.borrow_mut());
        for (at, cel, visible) in cases {
            if let Some(t) = self.layer_thumb(ctx, cel) {
                let tint = if visible { Color32::WHITE } else { Color32::from_gray(120) };
                painter.image(t.id(), at, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
                painter.rect_stroke(at, 0.0, Stroke::new(1.0_f32, Color32::from_black_alpha(90)), egui::StrokeKind::Inside);
            }
        }
    }

    /// Préférences > Apparence > Tailles, après les lignes de la timeline : les deux cases de la partie 206 (A et C), éteintes d'origine,
    /// écrites dans les réglages du poste seulement quand elles sont cochées.
    pub(crate) fn cles_timeline_prefs_ui(&mut self, ui: &mut egui::Ui) {
        for (nom, aide, vignettes) in [
            ("Vignette dans les clés", "Les lignes de calques de la timeline montrent le dessin de chaque clé assez large (Storyboard et Animation). Éteint : les clés restent en couleurs unies.", true),
            ("Durée dans les clés", "Les clés assez larges écrivent leur durée en images (« 12 i »).", false),
        ] {
            ui.label(nom);
            let valeur = if vignettes { &mut self.settings.cles_vignettes } else { &mut self.settings.cles_durees };
            let r = ui.checkbox(valeur, "");
            if crate::bulles::bulle(r, nom, aide, "").changed() {
                let _ = self.settings.save();
            }
            ui.label("");
            ui.end_row();
        }
    }

    /// Le fantôme de la place d'avant des clés du calque `li` du plan d'id `panel`, pendant un glisser de bord de clé ou de fin de plan
    /// (`HandleDrag::before`) : les clés qui ont changé de place, leur ancien emplacement en pointillés. `x_of` : l'abscisse d'une image ;
    /// `haut`, `bas` : la hauteur des clés.
    pub(crate) fn fantomes_du_geste(&self, painter: &Painter, panel: u64, li: usize, x_of: &dyn Fn(usize) -> f32, haut: f32, bas: f32) {
        let Some((avant, apres)) = self.handle_drag.as_ref().filter(|d| d.panel_id() == panel && d.montre_un_fantome()).and_then(|d| {
            let p = self.doc.panels.iter().find(|p| p.id == panel)?;
            Some((d.before_panel().layers.get(li)?, p.layers.get(li)?))
        }) else {
            return;
        };
        for (s, e) in fantomes(avant, apres) {
            let seg = Rect::from_min_max(pos2(x_of(s) + 1.0, haut), pos2(x_of(e) - 1.0, bas));
            if seg.width() > 0.0 && seg.intersects(painter.clip_rect()) {
                peindre_fantome(painter, seg);
            }
        }
    }

    /// E : la petite bulle près du pointeur pendant un glisser de bord de clé ou de fin de plan (« Clé 2 : 12 i (+4) », « Durée du
    /// plan : 1 s (+4 i) ») : le début de la phrase de la barre d'état, jusqu'au premier « · » ou « ( » d'explication.
    pub(crate) fn bulle_du_geste(&self, ctx: &egui::Context) {
        let Some(texte) = self.bulle_geste.as_ref() else { return };
        let Some(p) = ctx.input(|i| i.pointer.latest_pos()) else { return };
        egui::Area::new(egui::Id::new("bulle_du_geste"))
            .order(egui::Order::Tooltip)
            .fixed_pos(p + vec2(14.0, 18.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    // (Partie 208 : sur une ligne. La zone garde la largeur de ses premières images ; le texte qui s'allonge pendant
                    // le geste passait à la ligne, « Clé 2 : 10 i / (+4) ».)
                    ui.add(egui::Label::new(egui::RichText::new(texte.as_str()).size(12.0)).extend());
                });
            });
    }
}

/// Le début d'une phrase de la barre d'état, pour la bulle du geste : jusqu'au premier « · » (bord d'une clé) ou au premier « ( »
/// suivi d'une explication (fin du plan : « Durée du plan : 1 s (+4 i) » garde son écart).
pub(crate) fn debut_de_phrase(status: &str) -> String {
    let coupe = status.find(" · ").unwrap_or(status.len());
    let s = &status[..coupe];
    // « Durée du plan : 1 s (+4 i) (dernières clés…) » : on garde la première parenthèse (l'écart), pas la seconde.
    let s = match s.find(" i) (") {
        Some(k) => &s[..k + 3],
        None => s,
    };
    s.trim_end_matches('.').to_string()
}
