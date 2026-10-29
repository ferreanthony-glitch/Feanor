//! Incrustations : du texte posé sur l'image finale par un calque d'effet d'incrustation
//! (`calqueffet::Contenu::Incrustation`) : timecode HH:MM:SS:FF à la cadence du projet, numéro
//! d'image, nom du plan, de la séquence, titre du projet, date, ou un texte libre.
//!
//! Le texte est écrit dans le cadre de la caméra, à l'image finale (pas sur la feuille) : il ne suit
//! ni la caméra ni les calques. Il est tracé avec la mise en page des lettres de l'outil Texte
//! (`text.rs`, contours des polices) et la même peinture que les éléments texte des dessins
//! vectoriels : l'écran et l'export posent donc les mêmes pixels. L'export le grave dans l'image
//! (`burn`) quand le calque est visible, sauf s'il est réglé « Seulement à l'écran » ; l'écran le pose
//! par-dessus le dessin dans le cadre, sauf s'il est réglé « Seulement à l'export ».

use eframe::egui::Color32;

use crate::document::Document;
use crate::raster::{premultiply, Raster, TILE};

/// Ce que dit le texte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Timecode,
    NumeroImage,
    NomPlan,
    NomSequence,
    Titre,
    Date,
    /// Le texte de l'incrustation, avec ces mots remplacés : {timecode} {image} {plan} {sequence}
    /// {titre} {date}.
    Libre,
}

impl Source {
    pub const ALL: [Source; 7] =
        [Source::Timecode, Source::NumeroImage, Source::NomPlan, Source::NomSequence, Source::Titre, Source::Date, Source::Libre];

    pub fn label(self) -> &'static str {
        match self {
            Source::Timecode => "Timecode (HH:MM:SS:IM)",
            Source::NumeroImage => "Numéro d'image",
            Source::NomPlan => "Nom du plan",
            Source::NomSequence => "Nom de la séquence",
            Source::Titre => "Titre du projet",
            Source::Date => "Date",
            Source::Libre => "Texte libre",
        }
    }
}

/// D'où on compte les images du timecode et du numéro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Portee {
    /// Le film entier : l'image 0 est la première du premier plan.
    Montage,
    /// Le plan : l'image 0 est la première du plan.
    Plan,
}

/// Les neuf places du texte dans le cadre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ancre {
    HautGauche,
    HautCentre,
    HautDroite,
    MilieuGauche,
    Centre,
    MilieuDroite,
    BasGauche,
    BasCentre,
    BasDroite,
}

impl Ancre {
    /// Rangées de haut en bas, de gauche à droite.
    pub const ALL: [Ancre; 9] = [
        Ancre::HautGauche,
        Ancre::HautCentre,
        Ancre::HautDroite,
        Ancre::MilieuGauche,
        Ancre::Centre,
        Ancre::MilieuDroite,
        Ancre::BasGauche,
        Ancre::BasCentre,
        Ancre::BasDroite,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Ancre::HautGauche => "En haut à gauche",
            Ancre::HautCentre => "En haut au centre",
            Ancre::HautDroite => "En haut à droite",
            Ancre::MilieuGauche => "Au milieu à gauche",
            Ancre::Centre => "Au centre",
            Ancre::MilieuDroite => "Au milieu à droite",
            Ancre::BasGauche => "En bas à gauche",
            Ancre::BasCentre => "En bas au centre",
            Ancre::BasDroite => "En bas à droite",
        }
    }

    /// (colonne, rangée) de 0 à 2.
    fn cell(self) -> (usize, usize) {
        let i = Ancre::ALL.iter().position(|a| *a == self).unwrap_or(7);
        (i % 3, i / 3)
    }
}

/// Quand l'incrustation se voit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quand {
    Toujours,
    /// Seulement à l'écran : l'export ne la grave pas.
    EcranSeulement,
    /// Seulement à l'export : l'écran ne la montre pas.
    ExportSeulement,
}

impl Quand {
    pub const ALL: [Quand; 3] = [Quand::Toujours, Quand::EcranSeulement, Quand::ExportSeulement];

    pub fn label(self) -> &'static str {
        match self {
            Quand::Toujours => "À l'écran et à l'export",
            Quand::EcranSeulement => "Seulement à l'écran",
            Quand::ExportSeulement => "Seulement à l'export",
        }
    }
}

/// Noms sous lesquels les choix d'une incrustation sont enregistrés dans le projet (stables).
const SOURCES: [&str; 7] = ["timecode", "numero_image", "nom_plan", "nom_sequence", "titre", "date", "libre"];
const PORTEES: [&str; 2] = ["montage", "plan"];
const ANCRES: [&str; 9] = ["haut_gauche", "haut_centre", "haut_droite", "milieu_gauche", "centre", "milieu_droite", "bas_gauche", "bas_centre", "bas_droite"];
const QUANDS: [&str; 3] = ["toujours", "ecran_seulement", "export_seulement"];

impl Source {
    pub fn name(self) -> &'static str {
        SOURCES[Source::ALL.iter().position(|s| *s == self).unwrap_or(0)]
    }

    pub fn from_name(name: &str) -> Option<Source> {
        SOURCES.iter().position(|n| *n == name).map(|i| Source::ALL[i])
    }
}

impl Portee {
    pub fn name(self) -> &'static str {
        PORTEES[self as usize]
    }

    pub fn from_name(name: &str) -> Option<Portee> {
        match PORTEES.iter().position(|n| *n == name)? {
            0 => Some(Portee::Montage),
            _ => Some(Portee::Plan),
        }
    }
}

impl Ancre {
    pub fn name(self) -> &'static str {
        ANCRES[Ancre::ALL.iter().position(|a| *a == self).unwrap_or(7)]
    }

    pub fn from_name(name: &str) -> Option<Ancre> {
        ANCRES.iter().position(|n| *n == name).map(|i| Ancre::ALL[i])
    }
}

impl Quand {
    pub fn name(self) -> &'static str {
        QUANDS[Quand::ALL.iter().position(|q| *q == self).unwrap_or(0)]
    }

    pub fn from_name(name: &str) -> Option<Quand> {
        QUANDS.iter().position(|n| *n == name).map(|i| Quand::ALL[i])
    }
}

/// Le réglage d'une incrustation.
#[derive(Clone, Debug, PartialEq)]
pub struct Incrustation {
    pub source: Source,
    /// Le texte d'une incrustation libre.
    pub texte: String,
    pub portee: Portee,
    pub ancre: Ancre,
    /// Hauteur des lettres, en % de la hauteur du cadre.
    pub taille: f32,
    /// Couleur du texte (rouge, vert, bleu, opacité).
    pub couleur: [u8; 4],
    /// Boîte derrière le texte (couleur et opacité) ; rien : pas de boîte.
    pub fond: Option<[u8; 4]>,
    /// Distance au bord du cadre, en % de sa hauteur.
    pub marge: f32,
    pub quand: Quand,
    /// Police (voir `text::font_names`) ; vide : la police intégrée.
    pub police: String,
}

impl Default for Incrustation {
    fn default() -> Self {
        Incrustation {
            source: Source::Timecode,
            texte: String::new(),
            portee: Portee::Montage,
            ancre: Ancre::BasGauche,
            taille: 5.0,
            couleur: [255, 255, 255, 255],
            fond: Some([0, 0, 0, 160]),
            marge: 3.0,
            quand: Quand::Toujours,
            police: String::new(),
        }
    }
}

// ---------------------------------------------------------------------- le texte

/// « HH:MM:SS:IM » : le temps de l'image `frame` (à partir de 0) à `fps` images par seconde (entier :
/// SHOT n'a pas de cadence à 29,97, donc pas de timecode « drop frame »).
pub fn timecode(frame: usize, fps: u32) -> String {
    let fps = fps.max(1) as usize;
    let seconds = frame / fps;
    format!("{:02}:{:02}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60, frame % fps)
}

/// Jours depuis le 1er janvier 1970 → (année, mois, jour) du calendrier grégorien.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// « AAAA-MM-JJ » (date UTC) pour `secs` secondes depuis 1970.
pub fn date_text(secs: u64) -> String {
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Secondes depuis 1970 (l'heure de la machine).
pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Le texte de l'incrustation `inc` pour l'image `frame` du plan `panel`, à la date `now` (secondes
/// depuis 1970).
pub fn text_for(inc: &Incrustation, doc: &Document, panel: usize, frame: usize, now: u64) -> String {
    let counted = match inc.portee {
        Portee::Montage => doc.panel_start(panel) + frame,
        Portee::Plan => frame,
    };
    let tc = || timecode(counted, doc.fps);
    let number = || format!("{:04}", counted + doc.start_frame);
    let plan = || crate::storyboard::panel_title(doc, panel.min(doc.panels.len().saturating_sub(1)));
    let sequence = || format!("Séquence {}", doc.sequence_label(doc.sequence_of(panel.min(doc.panels.len().saturating_sub(1)))));
    let title = || doc.properties.title.clone();
    let date = || date_text(now);
    match inc.source {
        Source::Timecode => tc(),
        Source::NumeroImage => number(),
        Source::NomPlan => plan(),
        Source::NomSequence => sequence(),
        Source::Titre => title(),
        Source::Date => date(),
        Source::Libre => inc
            .texte
            .replace("{timecode}", &tc())
            .replace("{image}", &number())
            .replace("{plan}", &plan())
            .replace("{sequence}", &sequence())
            .replace("{titre}", &title())
            .replace("{date}", &date()),
    }
}

// ---------------------------------------------------------------------- la boîte posée

/// L'incrustation d'une image, prête à poser : ses pixels (prémultipliés) et la place de son coin
/// haut-gauche dans le cadre.
pub struct Boite {
    pub raster: Raster,
    pub x: i32,
    pub y: i32,
}

/// Dessine le texte `text` comme `inc` le règle, dans un cadre de `frame` pixels : rien si le texte
/// est vide.
pub fn render_box(inc: &Incrustation, text: &str, frame: [usize; 2]) -> Option<Boite> {
    let text = text.trim_end();
    if text.is_empty() || frame[0] == 0 || frame[1] == 0 {
        return None;
    }
    let size = (inc.taille.clamp(0.5, 60.0) / 100.0 * frame[1] as f32).max(4.0);
    let font = if inc.police.is_empty() { crate::text::FALLBACK } else { inc.police.as_str() };
    let laid = crate::text::layout(text, font, size);
    let pad = if inc.fond.is_some() { (size * 0.3).ceil() } else { 1.0 };
    let (bw, bh) = ((laid.bounds.width() + 2.0 * pad).ceil() as usize + 1, (laid.bounds.height() + 2.0 * pad).ceil() as usize + 1);
    let mut raster = Raster::new(bw, bh);
    if let Some(bg) = inc.fond {
        let c = premultiply(bg[0], bg[1], bg[2], bg[3]);
        for y in 0..bh {
            for x in 0..bw {
                let i = raster.tile_index_of(x, y);
                raster.tile_mut(i)[(y % TILE) * TILE + x % TILE] = c;
            }
        }
    }
    let place = crate::vector::Affine::translate(eframe::egui::vec2(pad, pad));
    let mut elem = crate::vector::Element::text(text.to_string(), font.to_string(), size, [inc.couleur[0], inc.couleur[1], inc.couleur[2]], place);
    if let crate::vector::Element::Text { opacity, .. } = &mut elem {
        *opacity = inc.couleur[3] as f32 / 255.0;
    }
    crate::vector::render_element(&mut raster, &elem);
    let margin = (inc.marge.clamp(0.0, 40.0) / 100.0 * frame[1] as f32).round() as i32;
    let (col, row) = inc.ancre.cell();
    let place = |index: usize, inner: usize, outer: usize| -> i32 {
        let (inner, outer) = (inner as i32, outer as i32);
        match index {
            0 => margin,
            1 => (outer - inner) / 2,
            _ => outer - inner - margin,
        }
    };
    Some(Boite { x: place(col, bw, frame[0]), y: place(row, bh, frame[1]), raster })
}

/// Les incrustations visibles à l'image `frame` du plan `panel`, du calque le plus bas au plus haut.
/// `export` : pour l'export (sinon : pour l'écran). Un calque caché (lui ou son dossier), sans
/// exposition à cette image, ou réglé pour l'autre sortie, n'est pas là.
pub fn boxes_for(doc: &Document, panel: usize, frame: usize, export: bool, now: u64) -> Vec<Boite> {
    let Some(p) = doc.panels.get(panel) else { return Vec::new() };
    boxes_in(doc, &p.layers, panel, frame, export, now)
}

/// Comme `boxes_for`, d'après une pile de calques donnée (celle du plan `panel`, ou la même avec le
/// calque d'un préréglage en cours de réglage).
pub fn boxes_in(doc: &Document, layers: &[crate::document::Layer], panel: usize, frame: usize, export: bool, now: u64) -> Vec<Boite> {
    let mut out = Vec::new();
    for (li, l) in layers.iter().enumerate() {
        let Some(inc) = l.overlay() else { continue };
        if !l.effect_active(frame) || crate::folders::effect(layers, li).is_none() {
            continue;
        }
        match (inc.quand, export) {
            (Quand::EcranSeulement, true) | (Quand::ExportSeulement, false) => continue,
            _ => {}
        }
        let opacity = crate::folders::effect_at(layers, li, frame).unwrap_or(1.0);
        let mut shown = inc.clone();
        shown.couleur[3] = (shown.couleur[3] as f32 * opacity).round() as u8;
        if let Some(f) = &mut shown.fond {
            f[3] = (f[3] as f32 * opacity).round() as u8;
        }
        if let Some(b) = render_box(&shown, &text_for(&shown, doc, panel, frame, now), doc.frame) {
            out.push(b);
        }
    }
    out
}

/// Grave les boîtes dans l'image finale `pixels` (prémultipliée, `frame` pixels).
pub fn burn(pixels: &mut [Color32], frame: [usize; 2], boxes: &[Boite]) {
    for b in boxes {
        for y in 0..b.raster.height {
            let fy = b.y + y as i32;
            if fy < 0 || fy >= frame[1] as i32 {
                continue;
            }
            for x in 0..b.raster.width {
                let fx = b.x + x as i32;
                if fx < 0 || fx >= frame[0] as i32 {
                    continue;
                }
                let s = b.raster.get(x, y);
                if s.a() != 0 {
                    let d = &mut pixels[fy as usize * frame[0] + fx as usize];
                    *d = crate::render::over(*d, s, 255, None);
                }
            }
        }
    }
}

/// Le plan `panel`, image `frame`, a-t-il des incrustations pour cette sortie ? (Pour ne rien
/// copier quand il n'y en a pas.)
pub fn any_for(doc: &Document, panel: usize, frame: usize, export: bool) -> bool {
    let Some(p) = doc.panels.get(panel) else { return false };
    p.layers.iter().enumerate().any(|(li, l)| {
        l.overlay().is_some_and(|i| {
            l.effect_active(frame)
                && crate::folders::effect(&p.layers, li).is_some()
                && !matches!((i.quand, export), (Quand::EcranSeulement, true) | (Quand::ExportSeulement, false))
        })
    })
}
