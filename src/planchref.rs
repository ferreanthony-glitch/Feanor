//! La planche de références (Anthony, 2026-10-30) : le tableau des références (`references.rs`) devient une vraie
//! planche infinie, à la façon des planches d'inspiration (images, vidéos) et des tableaux de notes (post-it, textes,
//! titres, cadres). Rien n'est repris d'un autre logiciel : seulement l'idée de ce qu'on veut y faire. (Ne pas
//! confondre avec `planche.rs`, la planche des Vignettes du storyboard.)
//!
//! - **Éléments** : une `Reference` est une image, une vidéo, un post-it, un texte, un titre ou un cadre (`Kind`).
//!   Un élément qui n'est pas une image n'a pas de fichier ; sa taille (`size`) est celle du bloc sur le tableau.
//!   Chaque élément a en plus : une opacité, un recadrage non destructif (images), un identifiant unique au monde
//!   (`uid`), son auteur et ses dates.
//! - **Ranger** : `pack` empile serré une sélection (ou tout) en gardant les tailles ; un cadre rangé range ce qu'il
//!   contient et se referme dessus (`Document::arrange_frame`).
//! - **Cadre** : il contient ce dont le centre est dans son rectangle ; le déplacer emporte le contenu
//!   (`Document::frame_contents`).
//! - **Fichier** : les mêmes `references.json` et `references/` que les références d'avant. Le fichier passe en
//!   format 2 seulement s'il contient du nouveau ; `project::VERSION` ne change pas (un ancien projet s'ouvre à
//!   l'identique, et le projet d'une planche avec des post-it ne perd rien dans une ancienne version : elle met le
//!   fichier de côté sans le toucher).
//! - **Fenêtre à part** (`FenetreReglage`) : détachée, plein écran, écran choisi, transparente, toujours au-dessus ;
//!   retenue dans les réglages de l'application.
//! - **Travail en équipe, plus tard** : voir `docs/INTERFACE.md` §81 (`uid`, auteur, dates et `modified` sont là pour
//!   fusionner deux planches ; le dossier partagé et les verrous restent à faire).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::references::{self, Kind, Reference};

/// Les couleurs d'un post-it, d'un texte et d'un cadre (la première : jaune, d'origine).
pub(crate) const NOTE_COLORS: [[u8; 3]; 6] = [[255, 226, 110], [255, 172, 190], [172, 226, 150], [150, 202, 246], [255, 190, 112], [208, 182, 242]];
/// Le nom de chaque couleur (bulles et menus).
pub(crate) const NOTE_COLOR_NAMES: [&str; 6] = ["Jaune", "Rose", "Vert", "Bleu", "Orange", "Mauve"];
/// Longueur maximale d'un texte de la planche, en caractères.
pub(crate) const TEXT_MAX: usize = 20_000;
/// Hauteur du bandeau de titre d'un cadre, en unités du tableau (à l'échelle 1).
pub(crate) const FRAME_STRIP: f32 = 34.0;
/// Espace laissé entre deux éléments rangés, en unités du tableau.
pub(crate) const GAP: f32 = 12.0;
/// Marge d'un cadre autour de ce qu'il contient quand il se referme dessus.
pub(crate) const FRAME_MARGIN: f32 = 26.0;
/// Plus petit côté d'un recadrage, en part de l'image.
pub(crate) const CROP_MIN: f32 = 0.02;

// ---------------------------------------------------------------------- identité

static UID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Un identifiant unique au monde : l'heure (nanosecondes), le numéro du processus, un compteur et un grain
/// pris à l'adresse d'une variable, en hexadécimal. Deux postes ne tirent pas le même.
pub(crate) fn new_uid() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let n = UID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let probe = 0u8;
    let grain = (&probe as *const u8 as usize as u64).rotate_left(17) ^ (nanos as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    format!("{:x}-{:x}-{:x}-{:x}", nanos, std::process::id(), n, grain & 0xFFFF_FFFF)
}

/// Les secondes depuis 1970.
pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Une date en français (`30/10/2026`) à partir des secondes depuis 1970 (calendrier grégorien, heure UTC).
pub(crate) fn date_fr(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    // (Jours depuis 1970 -> année, mois, jour : l'algorithme classique de calendrier civil.)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{day:02}/{month:02}/{year}")
}

// ---------------------------------------------------------------------- les éléments

/// La taille d'un élément neuf sur le tableau (à l'échelle 1).
pub(crate) fn block_size(kind: Kind) -> [u32; 2] {
    match kind {
        Kind::Note => [220, 190],
        Kind::Text => [340, 96],
        Kind::Title => [560, 110],
        Kind::Frame => [640, 420],
        Kind::Image => [360, 360],
    }
}

/// La taille de la lettre d'un élément, en unités du tableau (à l'échelle 1).
pub(crate) fn font_size(kind: Kind) -> f32 {
    match kind {
        Kind::Title => 52.0,
        Kind::Text => 24.0,
        Kind::Frame => 17.0,
        _ => 19.0,
    }
}

/// Marge du texte dans son bloc, en unités du tableau (à l'échelle 1) : en haut, en bas, à gauche et à droite.
pub(crate) const TEXT_PAD: f32 = 10.0;
/// Le plus petit facteur de taille qu'un post-it donne à son texte pour tout y faire tenir : en dessous, il s'allonge.
pub(crate) const FIT_MIN: f32 = 0.6;
/// Nombre de crans entre la taille d'origine (1) et `FIT_MIN`.
const FIT_STEPS: u32 = 16;
/// Les mesures se font sur une ligne un peu plus étroite que la vraie : à un autre zoom, les lettres tombent un peu
/// autrement, et le texte ne doit jamais déborder du bas du bloc.
const FIT_SLACK: f32 = 0.98;
/// Marge de sécurité sous le texte, en unités du tableau.
const FIT_BOTTOM: f32 = 2.0;

/// Fait tenir le texte de `r` dans son bloc, d'après le bloc `before` d'avant l'écriture (le même résultat pour le même
/// texte, quoi qu'on ait tapé avant) : un post-it réduit la taille de ses lettres jusqu'à `FIT_MIN`, puis s'allonge vers le
/// bas ; un texte ou un titre s'allonge vers le bas ; tous s'élargissent si un seul mot ne tient pas. Le haut et la gauche
/// du bloc ne bougent pas. `measure(texte, taille de lettre, largeur de ligne)` rend la largeur et la hauteur du texte
/// posé. Un cadre n'est pas touché.
pub(crate) fn autofit(r: &mut Reference, before: &Reference, measure: &mut dyn FnMut(&str, f32, f32) -> (f32, f32)) {
    if !matches!(r.kind, Kind::Note | Kind::Text | Kind::Title) {
        return;
    }
    let base = font_size(r.kind);
    let pads = 2.0 * TEXT_PAD;
    let (w0, h0) = (before.size[0] as f32, before.size[1] as f32);
    let text = r.text.clone();
    let mut fit = 1.0_f32;
    if r.kind == Kind::Note {
        let avail = h0 - pads - FIT_BOTTOM;
        let step = (1.0 - FIT_MIN) / FIT_STEPS as f32;
        let mut tenable = |i: u32| {
            let (mw, mh) = measure(&text, base * (1.0 - i as f32 * step), (w0 - pads) * FIT_SLACK);
            mw <= w0 - pads + 0.5 && mh <= avail
        };
        // Le plus petit cran qui tient (la taille ne fait que diminuer quand on grossit le texte : recherche par moitiés).
        let i = if tenable(0) {
            0
        } else if !tenable(FIT_STEPS) {
            FIT_STEPS
        } else {
            let (mut lo, mut hi) = (0, FIT_STEPS);
            while hi - lo > 1 {
                let mid = (lo + hi) / 2;
                if tenable(mid) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            hi
        };
        fit = 1.0 - i as f32 * step;
    }
    let px = base * fit;
    let mut w = w0;
    let (mut mw, mut mh) = measure(&text, px, (w - pads) * FIT_SLACK);
    if mw > w - pads + 0.5 {
        // Un mot plus large que le bloc : le bloc s'élargit.
        w = (mw / FIT_SLACK + pads).ceil();
        (mw, mh) = measure(&text, px, (w - pads) * FIT_SLACK);
        let _ = mw;
    }
    let h = h0.max((mh + pads + FIT_BOTTOM).ceil());
    let size = [(w.round() as u32).clamp(8, 1 << 16), (h.round() as u32).clamp(8, 1 << 16)];
    // Le haut et la gauche restent en place : le centre glisse de la moitié de ce que le bloc a gagné.
    let s = r.scale;
    let shift = vec2((size[0] as f32 - r.size[0] as f32) * s * 0.5, (size[1] as f32 - r.size[1] as f32) * s * 0.5);
    if shift != Vec2::ZERO {
        let c = r.from_local(shift);
        (r.x, r.y) = (c.x, c.y);
    }
    r.size = size;
    r.text_fit = fit;
}

/// Le nom d'un genre d'élément, au singulier.
pub(crate) fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Image => "image",
        Kind::Note => "post-it",
        Kind::Text => "texte",
        Kind::Title => "titre",
        Kind::Frame => "cadre",
    }
}

impl Reference {
    /// Un post-it, un texte, un titre ou un cadre neuf, centré en `at`.
    pub(crate) fn block(kind: Kind, id: u64, at: Pos2, text: &str, color: u8) -> Reference {
        let mut r = Reference::image(id, "", "", block_size(kind), at);
        r.scale = 1.0;
        r.kind = kind;
        r.text = text.chars().take(TEXT_MAX).collect();
        r.color = color.min(NOTE_COLORS.len() as u8 - 1);
        r
    }

    /// Pose l'identité d'un élément qu'on crée : identifiant unique, auteur, dates.
    pub(crate) fn stamp(&mut self, author: &str) {
        let now = now_secs();
        if self.uid.is_empty() {
            self.uid = new_uid();
        }
        self.author = author.chars().take(200).collect();
        self.created = now;
        self.modified = now;
    }

    /// L'élément vient d'être modifié (à l'instant `now`) : sa date change ; un ancien élément, lu sans identifiant, en
    /// reçoit un à ce moment-là (un fichier relu à l'identique n'est jamais réécrit pour ça).
    pub(crate) fn touch(&mut self, now: u64) {
        self.modified = now;
        if self.uid.is_empty() {
            self.uid = new_uid();
        }
    }

    /// Le titre d'un élément dans une liste : le nom d'une image, la première ligne d'un texte, ou le genre d'élément.
    pub(crate) fn title(&self) -> String {
        if self.kind == Kind::Image {
            return self.name.clone();
        }
        let first: String = self.text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(60).collect();
        if first.is_empty() {
            let mut name = kind_name(self.kind).to_string();
            if let Some(c) = name.get_mut(0..1) {
                c.make_ascii_uppercase();
            }
            name
        } else {
            first
        }
    }

    /// Le cadre quand son coin `(hx, hy)` est tiré jusqu'au point `at` du tableau (le coin opposé ne bouge pas) : son
    /// nouveau centre et sa nouvelle taille (à l'échelle de l'élément), au moins 40 unités de côté.
    pub(crate) fn frame_resized(&self, hx: i8, hy: i8, at: Pos2) -> (Pos2, [u32; 2]) {
        let l = self.to_local(at);
        let h = self.half();
        let anchor = vec2(-(hx as f32) * h.x, -(hy as f32) * h.y);
        let min = 40.0 * self.scale;
        let (w, hh) = (((l.x - anchor.x) * hx as f32).max(min), ((l.y - anchor.y) * hy as f32).max(min));
        let s = self.scale.max(references::SCALE_MIN);
        let size = [(w / s).round().clamp(8.0, 65_536.0) as u32, (hh / s).round().clamp(8.0, 65_536.0) as u32];
        // (Le centre se calcule sur la taille arrondie : le coin opposé reste exactement où il était.)
        let center = anchor + vec2(hx as f32 * size[0] as f32 * s * 0.5, hy as f32 * size[1] as f32 * s * 0.5);
        (self.from_local(center), size)
    }

    /// La couleur de fond du post-it ou du cadre.
    pub(crate) fn tint(&self) -> [u8; 3] {
        NOTE_COLORS[(self.color as usize).min(NOTE_COLORS.len() - 1)]
    }

    /// Une poignée du recadrage : un coin ou un milieu de côté du rectangle visible, `(hx, hy)` de -1 à 1.
    pub(crate) fn crop_handle_local(&self, hx: i8, hy: i8) -> Vec2 {
        let h = self.half();
        vec2(h.x * hx as f32, h.y * hy as f32)
    }

    /// Le recadrage quand la poignée `(hx, hy)` (côtés de l'écran : -1 gauche/haut, 1 droite/bas, 0 aucun) est tirée
    /// jusqu'au point `at` du tableau. Les miroirs sont rendus : le côté gauche de l'écran est le côté droit de
    /// l'image quand elle est retournée.
    pub(crate) fn crop_from_drag(&self, hx: i8, hy: i8, at: Pos2) -> [f32; 4] {
        let l = self.to_local(at);
        let c = self.crop_rect();
        let (w, h) = (self.size[0].max(1) as f32 * self.scale, self.size[1].max(1) as f32 * self.scale);
        let (cx, cy) = ((c[0] + c[2]) * 0.5, (c[1] + c[3]) * 0.5);
        let (sx, sy) = (if self.flip_h { -1.0 } else { 1.0 }, if self.flip_v { -1.0 } else { 1.0 });
        let u = (cx + sx * l.x / w).clamp(0.0, 1.0);
        let v = (cy + sy * l.y / h).clamp(0.0, 1.0);
        let mut out = c;
        // Quel côté de l'image est tiré : la gauche de l'écran est le début de l'image sans miroir.
        let first_x = (hx < 0 && !self.flip_h) || (hx > 0 && self.flip_h);
        let first_y = (hy < 0 && !self.flip_v) || (hy > 0 && self.flip_v);
        if hx != 0 {
            if first_x {
                out[0] = u.min(c[2] - CROP_MIN);
            } else {
                out[2] = u.max(c[0] + CROP_MIN);
            }
        }
        if hy != 0 {
            if first_y {
                out[1] = v.min(c[3] - CROP_MIN);
            } else {
                out[3] = v.max(c[1] + CROP_MIN);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------- le document

impl Document {
    /// Ce que le cadre `frame` contient : les éléments (autres que lui) dont le centre est dans son rectangle ; un
    /// autre cadre n'y est que s'il est plus petit. Dans l'ordre de la pile.
    pub(crate) fn frame_contents(&self, frame: u64) -> Vec<u64> {
        let Some(f) = self.reference(frame).filter(|f| f.kind == Kind::Frame) else { return Vec::new() };
        let area = |r: &Reference| {
            let h = r.half();
            h.x * h.y
        };
        self.references.iter().filter(|r| r.id != frame && f.contains(r.center()) && (r.kind != Kind::Frame || area(r) < area(f))).map(|r| r.id).collect()
    }

    /// Les éléments qui bougent avec `ids` : eux, et le contenu de chacun des cadres parmi eux. Sans doublon.
    pub(crate) fn with_frame_contents(&self, ids: &[u64]) -> Vec<u64> {
        let mut out: Vec<u64> = Vec::new();
        for id in ids {
            if !out.contains(id) {
                out.push(*id);
            }
            for c in self.frame_contents(*id) {
                if !out.contains(&c) {
                    out.push(c);
                }
            }
        }
        out
    }

    /// Les éléments (parmi `visible`) que le rectangle du tableau touche ; un cadre n'y est que s'il y tient en entier.
    pub(crate) fn references_in(&self, rect: Rect, visible: &[u64]) -> Vec<u64> {
        self.references.iter().filter(|r| visible.contains(&r.id) && r.bounds().intersects(rect) && (r.kind != Kind::Frame || rect.contains_rect(r.bounds()))).map(|r| r.id).collect()
    }

    /// Range les éléments `ids` en les empilant serré (`pack`), sans changer ni leurs tailles ni leurs angles, autour du
    /// centre de ce qu'ils couvraient. Un seul cadre dans `ids` : on range son contenu et il se referme dessus. Les
    /// cadres d'une sélection plus grande ne sont pas rangés. Faux s'il n'y avait rien à ranger.
    pub(crate) fn arrange(&mut self, ids: &[u64]) -> bool {
        if let [id] = ids {
            if self.reference(*id).is_some_and(|r| r.kind == Kind::Frame) {
                return self.arrange_frame(*id);
            }
        }
        let items: Vec<(u64, Rect)> = ids.iter().filter_map(|id| self.reference(*id)).filter(|r| r.kind != Kind::Frame).map(|r| (r.id, r.bounds())).collect();
        if items.len() < 2 {
            return false;
        }
        let union = items.iter().skip(1).fold(items[0].1, |a, (_, r)| a.union(*r));
        let (placed, total) = pack(&items, GAP);
        let shift = union.center() - total.center();
        for (id, to) in placed {
            let from = items.iter().find(|i| i.0 == id).map(|i| i.1).unwrap_or(to);
            if let Some(r) = self.reference_mut(id) {
                let c = r.center() + (to.center() + shift - from.center());
                (r.x, r.y) = (c.x, c.y);
            }
        }
        true
    }

    /// Range ce que contient le cadre `frame` à partir de son coin haut-gauche, puis le referme dessus (sa
    /// taille s'ajuste, avec une marge et le bandeau de titre). Le cadre perd sa rotation.
    pub(crate) fn arrange_frame(&mut self, frame: u64) -> bool {
        let inside = self.frame_contents(frame);
        let items: Vec<(u64, Rect)> = inside.iter().filter_map(|id| self.reference(*id)).map(|r| (r.id, r.bounds())).collect();
        let Some(f) = self.reference(frame).cloned() else { return false };
        if items.is_empty() {
            return false;
        }
        let strip = FRAME_STRIP * f.scale;
        let origin = f.bounds().min + vec2(FRAME_MARGIN, FRAME_MARGIN + strip);
        let (placed, total) = pack(&items, GAP);
        for (id, to) in &placed {
            let from = items.iter().find(|i| i.0 == *id).map(|i| i.1).unwrap_or(*to);
            if let Some(r) = self.reference_mut(*id) {
                let c = r.center() + (to.translate(origin.to_vec2()).center() - from.center());
                (r.x, r.y) = (c.x, c.y);
            }
        }
        let area = Rect::from_min_size(f.bounds().min, total.size() + vec2(2.0 * FRAME_MARGIN, 2.0 * FRAME_MARGIN + strip));
        if let Some(fr) = self.reference_mut(frame) {
            fr.rotation = 0.0;
            let s = fr.scale.max(references::SCALE_MIN);
            fr.size = [(area.width() / s).round().max(8.0) as u32, (area.height() / s).round().max(8.0) as u32];
            (fr.x, fr.y) = (area.center().x, area.center().y);
        }
        true
    }

    /// Un cadre (numéro `id`) autour des éléments `ids`, avec une marge et le bandeau de titre, derrière eux.
    pub(crate) fn frame_around(&mut self, ids: &[u64], id: u64, author: &str) -> Option<u64> {
        let items: Vec<Rect> = ids.iter().filter_map(|i| self.reference(*i)).map(|r| r.bounds()).collect();
        let first = *items.first()?;
        let union = items.iter().skip(1).fold(first, |a, r| a.union(*r));
        let area = Rect::from_min_max(union.min - vec2(FRAME_MARGIN, FRAME_MARGIN + FRAME_STRIP), union.max + vec2(FRAME_MARGIN, FRAME_MARGIN));
        let mut f = Reference::block(Kind::Frame, id, area.center(), "", 0);
        f.size = [area.width().round().max(8.0) as u32, area.height().round().max(8.0) as u32];
        f.stamp(author);
        self.add_reference(f);
        Some(id)
    }
}

/// Empile serré des rectangles `(numéro, étendue)` : rangées de hauteurs voisines, les plus hautes d'abord, dans un
/// bloc presque carré, `gap` entre deux. Rend, pour chacun, sa nouvelle étendue (de même taille) dans un repère où le
/// bloc commence en (0, 0), et l'étendue du bloc. Les tailles relatives sont gardées : rien n'est agrandi.
pub(crate) fn pack(items: &[(u64, Rect)], gap: f32) -> (Vec<(u64, Rect)>, Rect) {
    if items.is_empty() {
        return (Vec::new(), Rect::from_min_size(Pos2::ZERO, Vec2::ZERO));
    }
    let area: f32 = items.iter().map(|(_, r)| (r.width() + gap) * (r.height() + gap)).sum();
    let widest = items.iter().map(|(_, r)| r.width()).fold(0.0_f32, f32::max);
    let target = (area.sqrt() * 1.12).max(widest);
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|a, b| items[*b].1.height().total_cmp(&items[*a].1.height()).then(items[*a].0.cmp(&items[*b].0)));
    let (mut x, mut y, mut row_h) = (0.0_f32, 0.0_f32, 0.0_f32);
    let mut out = Vec::with_capacity(items.len());
    let mut total: Option<Rect> = None;
    for i in order {
        let (id, r) = &items[i];
        let (w, h) = (r.width(), r.height());
        if x > 0.0 && x + w > target {
            x = 0.0;
            y += row_h + gap;
            row_h = 0.0;
        }
        let placed = Rect::from_min_size(pos2(x, y), vec2(w, h));
        total = Some(total.map_or(placed, |t| t.union(placed)));
        out.push((*id, placed));
        x += w + gap;
        row_h = row_h.max(h);
    }
    (out, total.unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::ZERO)))
}

// ---------------------------------------------------------------------- importer la planche d'un autre projet

/// La planche lue dans le dossier d'un autre projet : ses éléments, prêts à être ajoutés à côté de ceux qui existent
/// déjà (numéros neufs, identifiants neufs, plans oubliés, fichiers renommés d'après le nouveau numéro), et les copies de
/// fichiers à faire (sur un autre fil).
pub(crate) struct Imported {
    pub items: Vec<Reference>,
    pub copies: Vec<(PathBuf, PathBuf)>,
    pub warnings: Vec<String>,
}

/// Lit la planche du projet `manifest` pour l'ajouter à une planche dont les fichiers sont dans `dest_dir`. `after` : le
/// rectangle du tableau à ne pas recouvrir (la planche importée se pose à sa droite) ; `next_id` donne un numéro neuf.
pub(crate) fn read_other_board(manifest: &Path, dest_dir: &Path, after: Option<Rect>, mut next_id: impl FnMut() -> u64) -> Result<Imported, String> {
    let dir = manifest.parent().ok_or_else(|| "ce n'est pas un fichier de projet".to_string())?;
    if !dir.join(references::FILE).exists() {
        return Err("ce projet n'a pas de planche de références.".into());
    }
    let mut warnings = Vec::new();
    let list = references::load(dir, &mut warnings);
    if list.is_empty() {
        return Err(format!("la planche de ce projet est vide ou illisible{}", warnings.first().map(|w| format!(" ({w})")).unwrap_or_default()));
    }
    let b = references::bounds_of(&list).unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::ZERO));
    let shift = match after {
        Some(a) => vec2(a.max.x + 90.0 - b.min.x, a.min.y - b.min.y),
        None => Vec2::ZERO,
    };
    let (mut items, mut copies) = (Vec::new(), Vec::new());
    for mut r in list {
        let new_id = next_id();
        if !r.file.is_empty() {
            let ext = Path::new(&r.file).extension().and_then(|e| e.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
            let new_file = if r.is_video() { new_id.to_string() } else { format!("{new_id}{ext}") };
            if let Some(from) = r.path() {
                if from.exists() {
                    copies.push((from, dest_dir.join(&new_file)));
                }
            }
            r.file = new_file;
        }
        r.id = new_id;
        r.uid = new_uid();
        r.plans.clear();
        r.x += shift.x;
        r.y += shift.y;
        r.home = Some(dest_dir.to_path_buf());
        items.push(r);
    }
    Ok(Imported { items, copies, warnings })
}

// ---------------------------------------------------------------------- la fenêtre à part

fn full() -> u8 {
    100
}

fn is_full(n: &u8) -> bool {
    *n >= 100
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

/// Ce que l'application retient de la fenêtre à part des références : on la règle dans la fenêtre elle-même.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FenetreReglage {
    /// La planche est dans sa propre fenêtre.
    #[serde(default, rename = "detachee", skip_serializing_if = "std::ops::Not::not")]
    pub detached: bool,
    /// La fenêtre est en plein écran.
    #[serde(default, rename = "plein_ecran", skip_serializing_if = "std::ops::Not::not")]
    pub fullscreen: bool,
    /// L'écran du plein écran : 0 celui où est la fenêtre, 1 le premier écran, 2 le deuxième…
    #[serde(default, rename = "ecran", skip_serializing_if = "is_zero")]
    pub screen: u8,
    /// Le fond de la fenêtre est transparent (la fenêtre est refaite pour que ça prenne).
    #[serde(default, rename = "transparente", skip_serializing_if = "std::ops::Not::not")]
    pub transparent: bool,
    /// Opacité de la fenêtre transparente, de 20 à 100 pour cent.
    #[serde(default = "full", rename = "opacite", skip_serializing_if = "is_full")]
    pub opacity: u8,
    /// La fenêtre reste devant les autres.
    #[serde(default, rename = "dessus", skip_serializing_if = "std::ops::Not::not")]
    pub on_top: bool,
}

impl Default for FenetreReglage {
    fn default() -> Self {
        FenetreReglage { detached: false, fullscreen: false, screen: 0, transparent: false, opacity: 100, on_top: false }
    }
}

impl FenetreReglage {
    pub fn est_defaut(&self) -> bool {
        *self == FenetreReglage::default()
    }

    /// Les valeurs ramenées à des valeurs sûres (un fichier de réglages édité à la main ne casse rien).
    pub fn nettoye(mut self) -> Self {
        self.opacity = self.opacity.clamp(20, 100);
        self.screen = self.screen.min(16);
        self
    }

    /// La part d'opacité (0,2 à 1) de tout ce que la fenêtre peint, quand elle est transparente.
    pub fn alpha(&self) -> f32 {
        if self.transparent {
            self.opacity.clamp(20, 100) as f32 / 100.0
        } else {
            1.0
        }
    }
}

/// Un écran de l'ordinateur, en pixels du bureau virtuel de Windows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ecran {
    pub x: i32,
    pub y: i32,
    pub largeur: i32,
    pub hauteur: i32,
}

impl Ecran {
    pub fn contient(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.largeur && y < self.y + self.hauteur
    }
}

/// Les écrans, l'écran principal d'abord puis les autres de gauche à droite (« Écran 1 », « Écran 2 »…). Vide hors de
/// Windows ou si Windows n'en dit rien.
pub fn ecrans() -> Vec<Ecran> {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct Rect32 {
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
        }
        type Rappel = unsafe extern "system" fn(isize, isize, *const Rect32, isize) -> i32;
        #[link(name = "user32")]
        extern "system" {
            fn EnumDisplayMonitors(hdc: isize, clip: *const Rect32, rappel: Rappel, donnees: isize) -> i32;
        }
        unsafe extern "system" fn rappel(_m: isize, _dc: isize, r: *const Rect32, donnees: isize) -> i32 {
            // SAFETY: `donnees` est le `Vec` de l'appelant, vivant pendant tout `EnumDisplayMonitors` ; `r` est valide.
            let (out, r) = unsafe { (&mut *(donnees as *mut Vec<Ecran>), &*r) };
            out.push(Ecran { x: r.left, y: r.top, largeur: r.right - r.left, hauteur: r.bottom - r.top });
            1
        }
        let mut out: Vec<Ecran> = Vec::new();
        // SAFETY: l'appel ne garde ni le pointeur ni le rappel après son retour.
        let ok = unsafe { EnumDisplayMonitors(0, std::ptr::null(), rappel, &mut out as *mut Vec<Ecran> as isize) };
        if ok == 0 {
            return Vec::new();
        }
        out.retain(|e| e.largeur > 0 && e.hauteur > 0);
        out.sort_by_key(|e| (!e.contient(0, 0), e.x, e.y));
        out
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// L'écran « numéro `n` » (1 : le premier) parmi `liste`.
pub fn ecran_numero(liste: &[Ecran], n: u8) -> Option<Ecran> {
    (n >= 1).then(|| liste.get(n as usize - 1).copied()).flatten()
}

/// L'écran qui contient le point `(x, y)` (pixels du bureau), à défaut le premier.
pub fn ecran_de(liste: &[Ecran], x: i32, y: i32) -> Option<usize> {
    liste.iter().position(|e| e.contient(x, y)).or(if liste.is_empty() { None } else { Some(0) })
}
