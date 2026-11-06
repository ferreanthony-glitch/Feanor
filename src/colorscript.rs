//! Exporter un « color script » (Anthony, 2026-10-21 ; sortie dédiée pour la V1, 2026-11-05) : voir d'un coup d'œil comment la couleur et la
//! lumière évoluent dans le film. Fichier > Exporter > « Color script ». La fenêtre et la tâche de fond : `colorscriptui.rs` ; le calcul, ici,
//! sans interface.
//!
//! - **Trois présentations** (`Presentation`) : la **planche** (une petite image par plan, rangées par séquence sur un fond uni, avec ses titres
//!   et sa bande de cinq couleurs facultative) ; la **bande continue** (une colonne par plan sur une seule ligne, large comme la durée du plan
//!   ou toutes égales, chaque image écrasée dans sa colonne comme un « code-barres » de film) ; la **bande de couleurs seule** (une couleur par
//!   plan, sans vignette : la dominante ou la moyenne).
//! - **L'image d'un plan** (`Prise`) : la première, celle du milieu, la **moyenne** de quelques images réparties sur la durée du plan (cinq au
//!   plus), ou la **première clé** (la première image où un dessin commence sur un calque visible ; un plan qui commence vide prend l'image où
//!   son premier dessin arrive). L'option « une image par case » en prend une par case de la planche (le plan et chacune de ses marques).
//! - **Sorties** (`Sortie`) : un PNG d'ensemble, un PNG par séquence, ou un PDF d'ensemble au format d'impression (A3 ou A4 paysage) avec son
//!   bloc de titre (titre du projet, version, date). Repères facultatifs : titres de séquence, numéros de plan, minutage.
//! - **Données** (`Donnees`) : un CSV ou un JSON des couleurs dominantes de chaque image (hex, part, temps d'entrée en secondes, plan,
//!   séquence), écrit à côté de l'image.
//! - **Noms** : les fichiers suivent le modèle de noms du projet (`nommage.rs`) : voir `nom_modele`.
//!
//! Une image trop grande est réduite aux bornes de `planche.rs`, et le compte rendu le dit. Le projet n'est jamais modifié : les images sont
//! rendues par la tâche de fond d'export, le reste ne lit que les durées et les calques.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use image::{Rgb, RgbImage};
use tiny_skia::{Color, Pixmap};

use crate::document::Document;
use crate::export::ExportJob;
use crate::nommage::{self, Jeton, Piece};
use crate::planche::{MAX_COTE, MAX_PIXELS};

/// Combien d'images, au plus, font la moyenne d'un plan.
pub const ECHANTILLONS_MOYENNE: usize = 5;

/// Comment le color script est présenté.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Presentation {
    /// Une petite image par plan, rangées par séquence.
    #[default]
    Planche,
    /// Une colonne par plan, sur une seule ligne.
    Bande,
    /// Une couleur par plan, sans vignette.
    Couleurs,
}

/// Quelle image représente un plan (ou une case).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prise {
    Premiere,
    Milieu,
    /// La moyenne de quelques images réparties sur la durée.
    Moyenne,
    /// La première image où un dessin commence.
    PremiereCle,
}

/// Le fichier écrit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sortie {
    /// Un PNG d'ensemble.
    Png,
    /// Un PNG par séquence.
    PngParSequence,
    /// Un PDF d'ensemble au format d'impression.
    Pdf,
}

impl Sortie {
    pub fn extension(self) -> &'static str {
        match self {
            Sortie::Png | Sortie::PngParSequence => "png",
            Sortie::Pdf => "pdf",
        }
    }
}

/// La page du PDF (paysage).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Papier {
    A4,
    A3,
}

impl Papier {
    /// Largeur et hauteur de la page, en points (paysage).
    pub fn page(self) -> (f32, f32) {
        match self {
            Papier::A4 => (841.89, 595.28),
            Papier::A3 => (1190.55, 841.89),
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            Papier::A4 => "A4 paysage",
            Papier::A3 => "A3 paysage",
        }
    }
}

/// Les couleurs de chaque image, écrites à côté.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Donnees {
    Aucune,
    Csv,
    Json,
}

impl Donnees {
    pub fn extension(self) -> &'static str {
        match self {
            Donnees::Aucune => "",
            Donnees::Csv => "csv",
            Donnees::Json => "json",
        }
    }
}

/// La couleur d'un plan dans la bande de couleurs seule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CouleurPlan {
    Dominante,
    Moyenne,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub sortie: Sortie,
    /// Une image par case (le plan et chacune de ses marques d'image) au lieu d'une par plan.
    pub par_case: bool,
    pub prise: Prise,
    /// Largeur d'une image, en pixels, et écart entre deux images (ou entre deux séquences d'une bande).
    pub largeur: u32,
    pub espace: u32,
    /// Images par rangée, au plus.
    pub par_ligne: u32,
    pub fond: [u8; 3],
    /// Les titres des séquences.
    pub titres: bool,
    /// Les numéros de plan.
    pub numeros: bool,
    /// La bande des couleurs dominantes sous chaque image (planche et bande continue) : éteinte d'office.
    pub bande: bool,
    pub presentation: Presentation,
    /// Bande continue et bande de couleurs : la largeur d'un plan suit sa durée (sinon, toutes égales).
    pub proportionnelle: bool,
    /// Bande continue et bande de couleurs : la largeur totale des colonnes, et la hauteur de la bande, en pixels.
    pub bande_largeur: u32,
    pub bande_hauteur: u32,
    /// Le temps d'entrée de chaque image (« 1:02 »).
    pub minutage: bool,
    pub couleur: CouleurPlan,
    pub papier: Papier,
    /// La version écrite dans le bloc de titre du PDF ; vide : le nom du dernier envoi du projet, s'il y en a un.
    pub version: String,
    pub donnees: Donnees,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            sortie: Sortie::Png,
            par_case: false,
            prise: Prise::Premiere,
            largeur: 160,
            espace: 2,
            par_ligne: 12,
            fond: [0, 0, 0],
            titres: true,
            numeros: false,
            bande: false,
            presentation: Presentation::Planche,
            proportionnelle: true,
            bande_largeur: 3000,
            bande_hauteur: 240,
            minutage: false,
            couleur: CouleurPlan::Dominante,
            papier: Papier::A3,
            version: String::new(),
            donnees: Donnees::Aucune,
        }
    }
}

/// Une image du color script.
#[derive(Clone, Debug, PartialEq)]
pub struct Entree {
    /// Le plan, dans le projet, et l'image du plan qui le représente.
    pub plan: usize,
    pub frame: usize,
    /// Les images qui en font la moyenne (`Prise::Moyenne`) ; sinon, la seule `frame`.
    pub echantillons: Vec<usize>,
    pub etiquette: Option<String>,
    /// Où elle est posée (coin haut gauche) et la taille de l'image elle-même.
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    /// Sa première image dans le film, sa durée en images, son rang dans le plan (1 : le plan ; 2… : les cases d'après).
    pub debut: usize,
    pub duree: usize,
    pub case: usize,
    /// Le minutage écrit (« 1:02 »), si on le demande.
    pub minutage: Option<String>,
    /// Le nom du plan (modèle de noms), si les données sont demandées.
    pub nom: String,
}

/// Une séquence et ses images.
#[derive(Clone, Debug, PartialEq)]
pub struct Bloc {
    pub titre: String,
    pub titre_x: i32,
    pub titre_y: i32,
    /// La largeur dont le titre dispose (une bande étroite le réduit).
    pub largeur: u32,
    pub entrees: Vec<Entree>,
}

/// Le color script rangé, sans pixel.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub blocs: Vec<Bloc>,
    /// La taille d'une image (la hauteur d'une colonne de bande), la hauteur de sa bande, et celle de l'image entière.
    pub cellule: [u32; 2],
    pub bande_h: u32,
    pub marge: i32,
    pub titre_taille: f32,
    pub taille: [u32; 2],
    pub options: Options,
    pub reduit: Option<String>,
    pub fps: u32,
    /// Bande continue : la taille du texte du minutage, et sa rangée sous la bande.
    pub repere_taille: f32,
    pub repere_y: i32,
}

/// Une image à rendre : à quelle entrée (rang dans l'ordre des blocs) elle sert, laquelle du projet, et à quelle taille.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Echantillon {
    pub entree: usize,
    pub plan: usize,
    pub frame: usize,
    pub cible: [u32; 2],
}

impl Spec {
    /// Les images qui représentent chaque entrée : (plan, image du plan), dans l'ordre du rangement.
    #[cfg(test)]
    pub fn images(&self) -> Vec<(usize, usize)> {
        self.blocs.iter().flat_map(|b| b.entrees.iter().map(|e| (e.plan, e.frame))).collect()
    }

    pub fn nombre(&self) -> usize {
        self.blocs.iter().map(|b| b.entrees.len()).sum()
    }

    /// Toutes les images à rendre (plusieurs par entrée pour une moyenne), dans l'ordre.
    pub fn echantillons(&self) -> Vec<Echantillon> {
        let mut out = Vec::new();
        for (k, e) in self.blocs.iter().flat_map(|b| b.entrees.iter()).enumerate() {
            for &f in &e.echantillons {
                out.push(Echantillon { entree: k, plan: e.plan, frame: f, cible: self.taille_de_travail(e) });
            }
        }
        out
    }

    /// La taille de l'image calculée pour l'entrée `e` : celle qu'on pose, ou une petite image pour la bande de couleurs.
    pub fn taille_de_travail(&self, e: &Entree) -> [u32; 2] {
        match self.options.presentation {
            Presentation::Couleurs => [64, ((64.0 * self.cellule[1] as f32 / self.cellule[0].max(1) as f32).round() as u32).clamp(8, 64)],
            _ => [e.w.max(1), e.h.max(1)],
        }
    }

    /// Les couleurs dominantes de chaque image sont-elles à calculer (bande, données, ou couleur dominante des plans) ?
    pub fn palette_utile(&self) -> bool {
        let o = &self.options;
        o.donnees != Donnees::Aucune || (o.presentation != Presentation::Couleurs && o.bande) || (o.presentation == Presentation::Couleurs && o.couleur == CouleurPlan::Dominante)
    }
}

/// « 1:02 » (minutes:secondes), « 1:02:03 » à partir d'une heure : le temps de l'image `frame` à `fps` images par seconde.
pub fn temps_court(frame: usize, fps: u32) -> String {
    let s = frame / fps.max(1) as usize;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// La première image de `[a, b)` du plan `p` où commence une clé, au sens des « Images à la clé » (`exportcles::cles_du_plan` : un dessin qui
/// commence sur un calque que l'export des calques sépare) ; `a` s'il n'y en a pas. Une clé veut dire la même chose ici et dans l'export.
pub fn premiere_cle(doc: &Document, p: usize, a: usize, b: usize) -> usize {
    crate::exportcles::cles_du_plan(doc, p, false).into_iter().map(|c| c.frame).find(|f| (a..b).contains(f)).unwrap_or(a)
}

/// Les images qui représentent la partie `[a, b)` du plan `p` : l'image montrée, et celles de la moyenne.
fn prise_de(doc: &Document, prise: Prise, p: usize, a: usize, b: usize) -> (usize, Vec<usize>) {
    let n = doc.panels[p].frame_count();
    let b = b.clamp(a + 1, n.max(a + 1));
    let milieu = (a + b - 1) / 2;
    let (frame, v) = match prise {
        Prise::Premiere => (a, vec![a]),
        Prise::Milieu => (milieu, vec![milieu]),
        Prise::PremiereCle => {
            let f = premiere_cle(doc, p, a, b);
            (f, vec![f])
        }
        Prise::Moyenne => {
            let len = b - a;
            let v: Vec<usize> = if len <= ECHANTILLONS_MOYENNE { (a..b).collect() } else { (0..ECHANTILLONS_MOYENNE).map(|k| a + k * (len - 1) / (ECHANTILLONS_MOYENNE - 1)).collect() };
            (milieu, v)
        }
    };
    let top = n.saturating_sub(1);
    (frame.min(top), v.into_iter().map(|f| f.min(top)).collect())
}

struct Brut {
    plan: usize,
    frame: usize,
    echantillons: Vec<usize>,
    etiquette: Option<String>,
    case: usize,
    debut: usize,
    duree: usize,
}

/// Ce que montre le color script pour les plans `plans` (indices du projet, dans l'ordre du film).
pub fn construire(doc: &Document, plans: &[usize], opt: &Options) -> Spec {
    let mut opt = opt.clone();
    opt.largeur = opt.largeur.clamp(24, 2000);
    opt.par_ligne = opt.par_ligne.clamp(1, 200);
    opt.espace = opt.espace.min(200);
    opt.bande_hauteur = opt.bande_hauteur.clamp(8, 4000);
    opt.bande_largeur = opt.bande_largeur.clamp(100, 15_900);
    let fps = doc.fps.max(1);
    // Le début de chaque plan dans le film (une passe).
    let mut debuts = Vec::with_capacity(doc.panels.len());
    let mut at = 0usize;
    for p in &doc.panels {
        debuts.push(at);
        at += p.frame_count();
    }
    let noms = if opt.donnees != Donnees::Aucune { doc.noms_de_tous_les_plans() } else { Vec::new() };
    // Les images, séquence par séquence (dans l'ordre du film).
    let mut groupes: Vec<(u64, Vec<Brut>)> = Vec::new();
    for &p in plans.iter().filter(|&&p| p < doc.panels.len()) {
        let seq = doc.sequence_of(p);
        let n = doc.panels[p].frame_count();
        let prises: Vec<(usize, usize, bool)> = if opt.par_case {
            crate::board::plan_boards(doc, p).into_iter().map(|(_, a, b)| (a, b, a == 0)).collect()
        } else {
            vec![(0, n, true)]
        };
        for (rang, (a, b, debut)) in prises.into_iter().enumerate() {
            let (frame, echantillons) = prise_de(doc, opt.prise, p, a, b);
            let etiquette = (opt.numeros && debut).then(|| doc.scene_label(doc.panels[p].scene));
            let brut = Brut { plan: p, frame, echantillons, etiquette, case: rang + 1, debut: debuts[p] + a, duree: b.saturating_sub(a).max(1) };
            match groupes.last_mut() {
                Some((s, v)) if *s == seq => v.push(brut),
                _ => groupes.push((seq, vec![brut])),
            }
        }
    }
    let nom_de = |p: usize| noms.get(p).cloned().unwrap_or_default();
    let avec_minutage = opt.minutage;
    let minutage_de = move |b: &Brut| avec_minutage.then(|| temps_court(b.debut, fps));
    let ratio = doc.frame[1].max(1) as f32 / doc.frame[0].max(1) as f32;
    match opt.presentation {
        Presentation::Planche => planche(doc, groupes, opt, ratio, fps, &nom_de, &minutage_de),
        Presentation::Bande | Presentation::Couleurs => bande(doc, groupes, opt, ratio, fps, &nom_de, &minutage_de),
    }
}

type NomDe<'a> = &'a dyn Fn(usize) -> String;
type MinutageDe<'a> = &'a dyn Fn(&Brut) -> Option<String>;

fn entree(b: &Brut, x: i32, y: i32, w: u32, h: u32, nom: NomDe, minutage: MinutageDe) -> Entree {
    Entree { plan: b.plan, frame: b.frame, echantillons: b.echantillons.clone(), etiquette: b.etiquette.clone(), x, y, w, h, debut: b.debut, duree: b.duree, case: b.case, minutage: minutage(b), nom: nom(b.plan) }
}

/// La planche : des rangées d'images par séquence.
fn planche(doc: &Document, groupes: Vec<(u64, Vec<Brut>)>, mut opt: Options, ratio: f32, fps: u32, nom: NomDe, minutage: MinutageDe) -> Spec {
    let colonnes = groupes.iter().map(|g| g.1.len()).max().unwrap_or(1).min(opt.par_ligne as usize).max(1);
    // Les bornes d'une image : la largeur d'une image est réduite jusqu'à y entrer.
    let mut reduit = None;
    let mut largeur = opt.largeur;
    let mesure = |largeur: u32| -> (u32, [u32; 2]) {
        let h = (largeur as f32 * ratio).round().max(1.0) as u32;
        let bande = if opt.bande { (h as f32 * 0.14).round().max(3.0) as u32 } else { 0 };
        let marge = (opt.espace * 4).max(8);
        let titre_h = if opt.titres { ((largeur as f32 * 0.14).clamp(12.0, 40.0) * 1.5).ceil() as u32 + opt.espace * 2 } else { 0 };
        let cell_h = h + bande;
        let mut y = marge;
        for (_, v) in &groupes {
            y += titre_h;
            let rangees = v.len().div_ceil(colonnes) as u32;
            y += rangees * (cell_h + opt.espace) - opt.espace + (opt.espace * 3).max(10);
        }
        let w = marge * 2 + colonnes as u32 * largeur + (colonnes as u32 - 1) * opt.espace;
        (bande, [w, y - (opt.espace * 3).max(10) + marge])
    };
    let (mut bande_h, mut taille) = mesure(largeur);
    let voulue = taille;
    while (taille[0] as f32 > MAX_COTE || taille[1] as f32 > MAX_COTE || taille[0] as f32 * taille[1] as f32 > MAX_PIXELS) && largeur > 24 {
        largeur = (largeur as f32 * 0.9) as u32;
        (bande_h, taille) = mesure(largeur);
    }
    if largeur != opt.largeur {
        reduit = Some(format!("Images réduites à {largeur} pixels de large (l'image aurait fait {} × {} pixels).", voulue[0], voulue[1]));
        opt.largeur = largeur;
    }
    let h = (largeur as f32 * ratio).round().max(1.0) as u32;
    let marge = (opt.espace * 4).max(8) as i32;
    let titre_taille = (largeur as f32 * 0.14).clamp(12.0, 40.0);
    let titre_h = if opt.titres { (titre_taille * 1.5).ceil() as i32 + opt.espace as i32 * 2 } else { 0 };
    let cell_h = (h + bande_h) as i32;
    let esp = opt.espace as i32;
    let mut y = marge;
    let mut blocs = Vec::new();
    for (seq, v) in &groupes {
        let titre = doc.sequence_label(*seq);
        let titre_y = y;
        y += titre_h;
        let entrees = v
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let (r, c) = ((i / colonnes) as i32, (i % colonnes) as i32);
                entree(b, marge + c * (largeur as i32 + esp), y + r * (cell_h + esp), largeur, h, nom, minutage)
            })
            .collect();
        let rangees = v.len().div_ceil(colonnes) as i32;
        y += rangees * (cell_h + esp) - esp + (esp * 3).max(10);
        blocs.push(Bloc { titre, titre_x: marge, titre_y, largeur: taille[0].saturating_sub(marge as u32 * 2), entrees });
    }
    Spec { blocs, cellule: [largeur, h], bande_h, marge, titre_taille, taille, options: opt, reduit, fps, repere_taille: 0.0, repere_y: 0 }
}

/// La bande continue et la bande de couleurs : toutes les images sur une seule ligne, séparées par un écart à chaque nouvelle séquence.
fn bande(doc: &Document, groupes: Vec<(u64, Vec<Brut>)>, mut opt: Options, ratio: f32, fps: u32, nom: NomDe, minutage: MinutageDe) -> Spec {
    let n_total: usize = groupes.iter().map(|g| g.1.len()).sum();
    let marge = (opt.espace * 4).max(8) as i32;
    let esp = opt.espace;
    let ecarts = groupes.len().saturating_sub(1) as u32 * esp;
    // Chaque colonne a au moins un pixel ; le reste se partage selon la durée (ou à parts égales), sans que la somme s'écarte du total.
    let dispo = opt.bande_largeur.max(n_total as u32 + ecarts) - ecarts;
    let reste = (dispo - n_total as u32) as u64;
    let poids: Vec<u64> = groupes.iter().flat_map(|g| g.1.iter()).map(|b| if opt.proportionnelle { b.duree.max(1) as u64 } else { 1 }).collect();
    let total_poids: u64 = poids.iter().sum::<u64>().max(1);
    let (mut cumul, mut acquis) = (0u64, 0u64);
    let largeurs: Vec<u32> = poids
        .iter()
        .map(|p| {
            cumul += p;
            let cible = (cumul * reste + total_poids / 2) / total_poids;
            let part = cible - acquis;
            acquis = cible;
            1 + part as u32
        })
        .collect();
    let mut reduit = None;
    let mut hauteur = opt.bande_hauteur;
    let seul_les_couleurs = opt.presentation == Presentation::Couleurs;
    let mesure = |h: u32| -> (u32, f32, f32, i32, [u32; 2]) {
        let titre_taille = (h as f32 * 0.12).clamp(11.0, 28.0);
        let titre_h = if opt.titres { (titre_taille * 1.5).ceil() as i32 + esp as i32 * 2 } else { 0 };
        let bande_h = if opt.bande && !seul_les_couleurs { (h as f32 * 0.14).round().max(3.0) as u32 } else { 0 };
        let repere_taille = (h as f32 * 0.08).clamp(9.0, 16.0);
        let repere_h = if opt.minutage { (repere_taille * 1.6).ceil() as i32 } else { 0 };
        let w = marge as u32 * 2 + dispo + ecarts;
        let total = marge + titre_h + h as i32 + bande_h as i32 + repere_h + marge;
        (bande_h, titre_taille, repere_taille, titre_h, [w, total as u32])
    };
    let (mut bande_h, mut titre_taille, mut repere_taille, mut titre_h, mut taille) = mesure(hauteur);
    let voulue = taille;
    while (taille[0] as f32 > MAX_COTE || taille[1] as f32 > MAX_COTE || taille[0] as f32 * taille[1] as f32 > MAX_PIXELS) && hauteur > 8 {
        hauteur = (hauteur as f32 * 0.9) as u32;
        (bande_h, titre_taille, repere_taille, titre_h, taille) = mesure(hauteur);
    }
    if hauteur != opt.bande_hauteur {
        reduit = Some(format!("Bande réduite à {hauteur} pixels de haut (l'image aurait fait {} × {} pixels).", voulue[0], voulue[1]));
        opt.bande_hauteur = hauteur;
    }
    let y0 = marge + titre_h;
    let repere_y = y0 + hauteur as i32 + bande_h as i32 + 2;
    let mut x = marge;
    let mut k = 0usize;
    let mut blocs = Vec::new();
    for (seq, v) in &groupes {
        let titre_x = x;
        let mut larg_bloc = 0u32;
        let entrees: Vec<Entree> = v
            .iter()
            .map(|b| {
                let w = largeurs[k];
                k += 1;
                let e = entree(b, x, y0, w, hauteur, nom, minutage);
                x += w as i32;
                larg_bloc += w;
                e
            })
            .collect();
        x += esp as i32;
        blocs.push(Bloc { titre: doc.sequence_label(*seq), titre_x, titre_y: marge, largeur: larg_bloc, entrees });
    }
    let moyenne = (dispo / n_total.max(1) as u32).max(1);
    let _ = ratio;
    Spec { blocs, cellule: [moyenne, hauteur], bande_h, marge, titre_taille, taille, options: opt, reduit, fps, repere_taille, repere_y }
}

/// Les séquences, une par une : les plans de `plans` regroupés par séquence (consécutifs, dans l'ordre du film).
pub fn plans_par_sequence(doc: &Document, plans: &[usize]) -> Vec<Vec<usize>> {
    let mut out: Vec<(u64, Vec<usize>)> = Vec::new();
    for &p in plans.iter().filter(|&&p| p < doc.panels.len()) {
        let s = doc.sequence_of(p);
        match out.last_mut() {
            Some((q, v)) if *q == s => v.push(p),
            _ => out.push((s, vec![p])),
        }
    }
    out.into_iter().map(|g| g.1).collect()
}

/// Les images à écrire : une seule (planche, bande, PDF), ou une par séquence.
pub fn specs(doc: &Document, plans: &[usize], opt: &Options) -> Vec<Spec> {
    if opt.sortie == Sortie::PngParSequence {
        plans_par_sequence(doc, plans).iter().map(|g| construire(doc, g, opt)).collect()
    } else {
        vec![construire(doc, plans, opt)]
    }
}

// ------------------------------------------------------------------------------------------------------- les noms

/// Le nom que le modèle de noms du projet (`nommage.rs`) donne au film, ou, avec `avec_sequence`, à la séquence du plan `p` : le modèle sans ce
/// qui désigne un plan (`{plan}`, `{version}`, `{case}`, `{calque}`, et le texte qui les précède : « _PL » de « SQ01_PL012A »), sans le numéro
/// de séquence pour le film. Rien de reste (le modèle d'origine, pour le film) : le nom du projet, ou le titre de la séquence.
pub fn nom_modele(doc: &Document, p: usize, avec_sequence: bool) -> String {
    let mut garde: Vec<Piece> = Vec::new();
    for piece in doc.modele_de_noms() {
        let retire = matches!(&piece, Piece::Jeton { jeton: Jeton::Plan | Jeton::Version | Jeton::Case | Jeton::Calque, .. }) || (!avec_sequence && matches!(&piece, Piece::Jeton { jeton: Jeton::Sequence, .. }));
        if retire {
            if matches!(garde.last(), Some(Piece::Texte(_))) {
                garde.pop();
            }
        } else {
            garde.push(piece);
        }
    }
    let nom = match doc.panels.get(p) {
        Some(_) => nommage::remplir(&garde, &doc.contexte_de_nom(p)),
        None => String::new(),
    };
    let nom = nom.trim_matches(['_', '-', '.', ' ']).to_string();
    if !nom.is_empty() {
        return crate::editorial::file_safe(&nom);
    }
    if avec_sequence && p < doc.panels.len() {
        crate::editorial::file_safe(&doc.sequence_label(doc.sequence_of(p)))
    } else {
        crate::paquet::base_du_film(doc)
    }
}

/// Les noms de fichiers (sans extension), un par image à écrire : « <nom>_colorscript », le nom du film pour une image d'ensemble, celui de
/// la séquence pour chaque image d'une séquence (deux noms égaux : le second prend « _2 »).
pub fn noms_de_fichiers(doc: &Document, plans: &[usize], opt: &Options) -> Vec<String> {
    let groupes = plans_par_sequence(doc, plans);
    let bruts: Vec<String> = if opt.sortie == Sortie::PngParSequence {
        groupes.iter().map(|g| nom_modele(doc, g[0], true)).collect()
    } else if groupes.len() == 1 {
        vec![nom_modele(doc, groupes[0][0], true)]
    } else {
        vec![nom_modele(doc, plans.first().copied().unwrap_or(0), false)]
    };
    let mut vus: Vec<String> = Vec::new();
    bruts
        .into_iter()
        .map(|b| {
            let base = format!("{b}_colorscript");
            let mut nom = base.clone();
            let mut n = 1;
            while vus.iter().any(|v| v.to_lowercase() == nom.to_lowercase()) {
                n += 1;
                nom = format!("{base}_{n}");
            }
            vus.push(nom.clone());
            nom
        })
        .collect()
}

/// Où écrire : les fichiers d'image et celui des données. `out` : le fichier de l'image d'ensemble ou du PDF ; pour un PNG par séquence, le
/// dossier. Les données vont à côté de l'image (même nom, autre extension) ; pour un PNG par séquence, dans le dossier, au nom du film.
pub fn chemins(doc: &Document, plans: &[usize], opt: &Options, out: &Path) -> (Vec<PathBuf>, Option<PathBuf>) {
    let noms = noms_de_fichiers(doc, plans, opt);
    let ext = opt.donnees.extension();
    if opt.sortie == Sortie::PngParSequence {
        let images = noms.iter().map(|n| out.join(format!("{n}.png"))).collect();
        let film = nom_modele(doc, plans.first().copied().unwrap_or(0), false);
        let donnees = (opt.donnees != Donnees::Aucune).then(|| out.join(format!("{film}_colorscript.{ext}")));
        (images, donnees)
    } else {
        let donnees = (opt.donnees != Donnees::Aucune).then(|| out.with_extension(ext));
        (vec![out.to_path_buf()], donnees)
    }
}

// ----------------------------------------------------------------------------------------------------- les couleurs

/// Les couleurs dominantes d'une image : au plus `n`, avec la part de l'image que chacune représente (la somme fait 1), de la plus
/// présente à la moins présente. Deux couleurs trop proches n'en font qu'une.
pub fn dominantes(img: &RgbImage, n: usize) -> Vec<([u8; 3], f32)> {
    if img.width() == 0 || img.height() == 0 || n == 0 {
        return Vec::new();
    }
    let petit = if img.width() > 64 {
        let h = ((img.height() as f32 * 64.0 / img.width() as f32).round() as u32).max(1);
        image::imageops::resize(img, 64, h, image::imageops::FilterType::Triangle)
    } else {
        img.clone()
    };
    // Un histogramme à quatre bits par canal.
    let mut cases: std::collections::HashMap<u16, (u32, [u32; 3])> = std::collections::HashMap::new();
    for p in petit.pixels() {
        let k = ((p[0] as u16 >> 4) << 8) | ((p[1] as u16 >> 4) << 4) | (p[2] as u16 >> 4);
        let e = cases.entry(k).or_insert((0, [0; 3]));
        e.0 += 1;
        for c in 0..3 {
            e.1[c] += p[c] as u32;
        }
    }
    let mut triees: Vec<(u16, u32, [f32; 3])> = cases.into_iter().map(|(k, (nb, s))| (k, nb, [s[0] as f32 / nb as f32, s[1] as f32 / nb as f32, s[2] as f32 / nb as f32])).collect();
    triees.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let dist = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    let mut centres: Vec<[f32; 3]> = Vec::new();
    for t in &triees {
        if centres.len() >= n {
            break;
        }
        if centres.iter().all(|c| dist(*c, t.2) >= 40.0) {
            centres.push(t.2);
        }
    }
    let mut parts = vec![(0u32, [0f32; 3]); centres.len()];
    for t in &triees {
        let k = (0..centres.len()).min_by(|&a, &b| dist(centres[a], t.2).total_cmp(&dist(centres[b], t.2))).unwrap_or(0);
        parts[k].0 += t.1;
        for c in 0..3 {
            parts[k].1[c] += t.2[c] * t.1 as f32;
        }
    }
    let total: u32 = parts.iter().map(|p| p.0).sum::<u32>().max(1);
    let mut out: Vec<([u8; 3], f32)> = parts
        .into_iter()
        .filter(|p| p.0 > 0)
        .map(|(nb, s)| ([(s[0] / nb as f32).round() as u8, (s[1] / nb as f32).round() as u8, (s[2] / nb as f32).round() as u8], nb as f32 / total as f32))
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
}

/// La couleur moyenne d'une image (moyenne des pixels, canal par canal).
pub fn moyenne(img: &RgbImage) -> [u8; 3] {
    let n = (img.width() as u64 * img.height() as u64).max(1);
    let mut s = [0u64; 3];
    for p in img.pixels() {
        for c in 0..3 {
            s[c] += p[c] as u64;
        }
    }
    [((s[0] + n / 2) / n) as u8, ((s[1] + n / 2) / n) as u8, ((s[2] + n / 2) / n) as u8]
}

/// « #RRGGBB ».
pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// L'image d'une entrée du color script, avec sa palette si elle sert.
pub type Cellule = (RgbImage, Vec<([u8; 3], f32)>);

/// La couleur lisible sur le fond `fond`.
fn encre_sur(fond: [u8; 3]) -> [u8; 3] {
    let l = 0.299 * fond[0] as f32 + 0.587 * fond[1] as f32 + 0.114 * fond[2] as f32;
    if l < 128.0 {
        [235, 235, 235]
    } else {
        [30, 30, 30]
    }
}

fn largeur_texte(t: &str, taille: f32) -> f32 {
    crate::text::layout(t, crate::text::FALLBACK, taille).bounds.width()
}

/// La couleur d'un plan dans la bande de couleurs seule.
fn couleur_du_plan(cellule: &Cellule, mode: CouleurPlan) -> [u8; 3] {
    match mode {
        CouleurPlan::Moyenne => moyenne(&cellule.0),
        CouleurPlan::Dominante => cellule.1.first().map(|c| c.0).unwrap_or_else(|| dominantes(&cellule.0, 1).first().map(|c| c.0).unwrap_or_else(|| moyenne(&cellule.0))),
    }
}

/// Le color script dessiné : `cellules[k]` est l'image de la `k`-ième entrée (dans l'ordre de `Spec::images`).
pub fn composer(spec: &Spec, cellules: &[Cellule]) -> Result<RgbImage, String> {
    let [w, h] = spec.taille;
    let mut pix = Pixmap::new(w, h).ok_or("image trop grande pour la mémoire")?;
    let f = spec.options.fond;
    pix.fill(Color::from_rgba8(f[0], f[1], f[2], 255));
    let encre = encre_sur(f);
    let en_bande = spec.options.presentation != Presentation::Planche;
    let (mut fin_numero, mut fin_minutage) = (i32::MIN, i32::MIN);
    let mut k = 0;
    for bloc in &spec.blocs {
        if spec.options.titres && !bloc.titre.trim().is_empty() {
            // Dans une bande étroite, le titre rétrécit pour tenir au-dessus de sa séquence (et disparaît s'il devient illisible).
            let mut t = spec.titre_taille;
            if en_bande {
                let l = largeur_texte(&bloc.titre, t);
                if l > bloc.largeur as f32 {
                    t = (t * bloc.largeur as f32 / l.max(1.0)).floor();
                }
            }
            crate::planche::ecrire(&mut pix, &bloc.titre, bloc.titre_x as f32, bloc.titre_y as f32, t, encre);
        }
        for e in &bloc.entrees {
            let Some(cellule) = cellules.get(k) else {
                k += 1;
                continue;
            };
            k += 1;
            if spec.options.presentation == Presentation::Couleurs {
                crate::planche::remplir(&mut pix, [e.x, e.y, e.x + e.w as i32, e.y + e.h as i32], couleur_du_plan(cellule, spec.options.couleur));
            } else {
                let (img, palette) = cellule;
                let img = if img.dimensions() != (e.w, e.h) { image::imageops::resize(img, e.w, e.h, image::imageops::FilterType::Triangle) } else { img.clone() };
                let mut rgba = Vec::with_capacity((e.w * e.h * 4) as usize);
                for px in img.pixels() {
                    rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
                if let Some(cell) = tiny_skia::IntSize::from_wh(e.w, e.h).and_then(|sz| Pixmap::from_vec(rgba, sz)) {
                    pix.draw_pixmap(e.x, e.y, cell.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
                }
                // La bande des couleurs dominantes, sous l'image.
                if spec.bande_h > 0 && !palette.is_empty() {
                    let (y0, y1) = (e.y + e.h as i32, e.y + (e.h + spec.bande_h) as i32);
                    let mut x = e.x as f32;
                    for (i, (c, part)) in palette.iter().enumerate() {
                        let fin = if i + 1 == palette.len() { (e.x + e.w as i32) as f32 } else { x + part * e.w as f32 };
                        crate::planche::remplir(&mut pix, [x.round() as i32, y0, fin.round() as i32, y1], *c);
                        x = fin;
                    }
                }
            }
            if let Some(num) = &e.etiquette {
                let t = (e.w as f32 * 0.09).clamp(8.0, 18.0);
                let l = (t * (num.chars().count() as f32 * 0.62 + 0.8)) as i32;
                // Dans une bande, un numéro ne se pose que s'il ne recouvre pas le précédent.
                if !en_bande || e.x >= fin_numero {
                    crate::planche::remplir(&mut pix, [e.x, e.y, e.x + l, e.y + (t * 1.3) as i32], [0, 0, 0]);
                    crate::planche::ecrire(&mut pix, num, e.x as f32 + t * 0.3, e.y as f32 + t * 0.05, t, [255, 255, 255]);
                    fin_numero = e.x + l + 2;
                }
            }
            if let Some(temps) = &e.minutage {
                if en_bande {
                    // Sous la bande, à l'aplomb de l'entrée de chaque plan, tant que ça ne recouvre pas le précédent.
                    if e.x >= fin_minutage {
                        crate::planche::ecrire(&mut pix, temps, e.x as f32, spec.repere_y as f32, spec.repere_taille, encre);
                        fin_minutage = e.x + largeur_texte(temps, spec.repere_taille).ceil() as i32 + 4;
                    }
                } else {
                    let t = (e.w as f32 * 0.09).clamp(8.0, 18.0);
                    let l = (t * (temps.chars().count() as f32 * 0.62 + 0.8)) as i32;
                    let bas = e.y + e.h as i32;
                    crate::planche::remplir(&mut pix, [e.x, bas - (t * 1.3) as i32, e.x + l, bas], [0, 0, 0]);
                    crate::planche::ecrire(&mut pix, temps, e.x as f32 + t * 0.3, (bas - (t * 1.3) as i32) as f32 + t * 0.05, t, [255, 255, 255]);
                }
            }
        }
    }
    let mut out = RgbImage::new(w, h);
    for (dst, src) in out.pixels_mut().zip(pix.data().chunks_exact(4)) {
        *dst = Rgb([src[0], src[1], src[2]]);
    }
    Ok(out)
}

// ----------------------------------------------------------------------------------------------------- les fichiers

/// Ce que le bloc de titre du PDF et les données disent du projet.
#[derive(Clone, Debug, PartialEq)]
pub struct Meta {
    pub titre: String,
    pub episode: String,
    pub version: String,
    /// « AAAA-MM-JJ ».
    pub date: String,
    pub papier: Papier,
}

impl Meta {
    /// Le titre du projet (la série à défaut), son épisode, la version (celle des options, sinon le dernier envoi), la date du jour.
    pub fn pour(doc: &Document, opt: &Options) -> Meta {
        let titre = match doc.properties.title.trim() {
            "" => doc.properties.serie.trim().to_string(),
            t => t.to_string(),
        };
        let version = match opt.version.trim() {
            "" => doc.envois.dernier().map(|e| e.nom.trim().to_string()).unwrap_or_default(),
            v => v.to_string(),
        };
        Meta { titre, episode: doc.properties.episode.trim().to_string(), version, date: crate::incrustation::date_text(crate::incrustation::now_secs()), papier: opt.papier }
    }
}

/// Les octets du fichier.
pub fn encoder(img: &RgbImage, sortie: Sortie, meta: &Meta) -> Result<Vec<u8>, String> {
    match sortie {
        Sortie::Png | Sortie::PngParSequence => crate::planche::png(img),
        Sortie::Pdf => pdf(img, meta),
    }
}

/// Le PDF : une page A3 ou A4 paysage, le bloc de titre en haut (titre du projet, version, date), le color script en dessous, aussi grand que la
/// page le permet. L'image est réduite à 200 points par pouce au plus (un PDF d'impression, pas un fichier de travail).
fn pdf(img: &RgbImage, meta: &Meta) -> Result<Vec<u8>, String> {
    use crate::pdf::{op_image, op_text, text_width, PdfFile};
    let (pw, ph) = meta.papier.page();
    let m = 28.0_f32;
    let titre_h = 46.0_f32;
    let (bw, bh) = (pw - 2.0 * m, ph - 2.0 * m - titre_h);
    let s = (bw / img.width().max(1) as f32).min(bh / img.height().max(1) as f32);
    let (iw, ih) = (img.width() as f32 * s, img.height() as f32 * s);
    let px = |pt: f32| ((pt * 200.0 / 72.0).round() as u32).max(1);
    let (cw, ch) = (px(iw), px(ih));
    let jpeg = if img.width() > cw || img.height() > ch { crate::planche::jpeg(&image::imageops::resize(img, cw, ch, image::imageops::FilterType::Triangle), 90)? } else { crate::planche::jpeg(img, 90)? };
    let (jw, jh) = if img.width() > cw || img.height() > ch { (cw, ch) } else { (img.width(), img.height()) };
    let mut f = PdfFile::new();
    let id = f.add_jpeg(jw, jh, &jpeg);
    let titre = if meta.titre.trim().is_empty() { "Color script".to_string() } else { meta.titre.clone() };
    let titre = if meta.episode.trim().is_empty() { titre } else { format!("{titre} — épisode {}", meta.episode) };
    let mut c = String::new();
    c += &op_text(ph, m, m, &titre, 18.0, true, 0.0);
    c += &op_text(ph, m, m + 24.0, "Color script", 10.0, false, 0.35);
    let droite = pw - m;
    let date = format!("Date : {}", meta.date);
    c += &op_text(ph, droite - text_width(&date, false, 10.0), m + 24.0, &date, 10.0, false, 0.2);
    if !meta.version.trim().is_empty() {
        let v = format!("Version : {}", meta.version.trim());
        c += &op_text(ph, droite - text_width(&v, false, 10.0), m + 8.0, &v, 10.0, false, 0.2);
    }
    c += &crate::pdfmise::op_filet(ph, m, droite, m + titre_h - 6.0, 0.6, 0.5);
    c += &op_image(ph, id, m + (bw - iw) / 2.0, m + titre_h, iw, ih);
    f.add_page_sized(pw, ph, c.as_bytes(), &[id]);
    Ok(f.finish(&titre))
}

// -------------------------------------------------------------------------------------------------------- les données

/// Une ligne des données : une image du color script.
#[derive(Clone, Debug, PartialEq)]
pub struct Ligne {
    pub sequence: String,
    pub plan: String,
    pub case: usize,
    pub entree_s: f64,
    pub duree_s: f64,
    pub moyenne: [u8; 3],
    pub couleurs: Vec<([u8; 3], f32)>,
}

fn arrondi(x: f64, decimales: i32) -> f64 {
    let k = 10f64.powi(decimales);
    (x * k).round() / k
}

/// Les lignes des données, dans l'ordre du color script ; `cellules[k]` : l'image (et la palette) de la `k`-ième entrée.
pub fn lignes(spec: &Spec, cellules: &[Cellule]) -> Vec<Ligne> {
    let fps = spec.fps.max(1) as f64;
    let mut out = Vec::new();
    let mut k = 0;
    for b in &spec.blocs {
        for e in &b.entrees {
            let Some((img, palette)) = cellules.get(k) else {
                k += 1;
                continue;
            };
            k += 1;
            out.push(Ligne {
                sequence: b.titre.clone(),
                plan: e.nom.clone(),
                case: e.case,
                entree_s: arrondi(e.debut as f64 / fps, 3),
                duree_s: arrondi(e.duree as f64 / fps, 3),
                moyenne: moyenne(img),
                couleurs: palette.clone(),
            });
        }
    }
    out
}

/// Combien de couleurs une ligne donne, au plus.
pub const COULEURS_PAR_LIGNE: usize = 5;

/// Le CSV (UTF-8, séparateur virgule, lignes terminées par CRLF) : sequence, plan, case, entree_s, duree_s, moyenne, puis couleur_k et part_k
/// (hex, et part de l'image de 0 à 1) pour chacune des cinq couleurs ; les cases des couleurs absentes restent vides.
pub fn csv(lignes: &[Ligne]) -> String {
    use crate::listeplans::cellule_csv;
    let mut en_tete: Vec<String> = ["sequence", "plan", "case", "entree_s", "duree_s", "moyenne"].iter().map(|s| s.to_string()).collect();
    for k in 1..=COULEURS_PAR_LIGNE {
        en_tete.push(format!("couleur_{k}"));
        en_tete.push(format!("part_{k}"));
    }
    let mut out = en_tete.join(",") + "\r\n";
    for l in lignes {
        let mut v = vec![cellule_csv(&l.sequence, ','), cellule_csv(&l.plan, ','), l.case.to_string(), format!("{:.3}", l.entree_s), format!("{:.3}", l.duree_s), hex(l.moyenne)];
        for k in 0..COULEURS_PAR_LIGNE {
            match l.couleurs.get(k) {
                Some((c, part)) => {
                    v.push(hex(*c));
                    v.push(format!("{part:.4}"));
                }
                None => {
                    v.push(String::new());
                    v.push(String::new());
                }
            }
        }
        out += &v.join(",");
        out += "\r\n";
    }
    out
}

/// Le JSON : un objet qui décrit le projet, puis `images`, un objet par image du color script.
pub fn json(meta: &Meta, fps: u32, lignes: &[Ligne]) -> String {
    use serde_json::{json, Value};
    let images: Vec<Value> = lignes
        .iter()
        .map(|l| {
            json!({
                "sequence": l.sequence,
                "plan": l.plan,
                "case": l.case,
                "entree_s": l.entree_s,
                "duree_s": l.duree_s,
                "moyenne": hex(l.moyenne),
                "couleurs": l.couleurs.iter().map(|(c, p)| json!({ "hex": hex(*c), "part": arrondi(*p as f64, 4) })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let racine = json!({
        "format": "feanor-color-script",
        "projet": meta.titre,
        "episode": meta.episode,
        "version": meta.version,
        "date": meta.date,
        "cadence": fps,
        "images": images,
    });
    serde_json::to_string_pretty(&racine).unwrap_or_default() + "\n"
}

// --------------------------------------------------------------------------------------------------------- la tâche

/// La somme des images d'une moyenne, en cours.
struct Cumul {
    taille: [u32; 2],
    somme: Vec<u16>,
    n: u16,
}

impl Cumul {
    fn ajouter(&mut self, img: &RgbImage) {
        if img.dimensions() != (self.taille[0], self.taille[1]) {
            return;
        }
        for (s, v) in self.somme.iter_mut().zip(img.as_raw()) {
            *s += *v as u16;
        }
        self.n += 1;
    }

    fn image(&self) -> RgbImage {
        let n = self.n.max(1) as u32;
        let raw: Vec<u8> = self.somme.iter().map(|&s| ((s as u32 + n / 2) / n) as u8).collect();
        RgbImage::from_raw(self.taille[0], self.taille[1], raw).unwrap_or_else(|| RgbImage::new(self.taille[0], self.taille[1]))
    }
}

/// La tâche d'export du color script : `specs` (une par image à écrire) vers `cibles` (autant de fichiers), les données vers `donnees`. Les
/// images du projet sont rendues par la tâche de fond (`ExportJob::collect_with`, annulable) ; l'assemblage et l'écriture se font ensuite sur un
/// autre fil. Rien n'est écrit si l'on annule ; un fichier déjà écrit est effacé si le suivant échoue. `message` : ce que dit la barre d'état à la fin.
pub fn job(frame: [usize; 2], specs: Vec<Spec>, meta: Meta, cibles: Vec<PathBuf>, donnees: Option<PathBuf>, message: String) -> Result<ExportJob, String> {
    if specs.is_empty() || specs.len() != cibles.len() {
        return Err("Aucun plan dans cette étendue.".into());
    }
    let n_entrees: Vec<usize> = specs.iter().map(Spec::nombre).collect();
    if n_entrees.iter().sum::<usize>() == 0 {
        return Err("Aucun plan dans cette étendue.".into());
    }
    // Chaque image à rendre : (rang de la spec, échantillon), et l'entrée globale qu'elle sert.
    let mut table: Vec<(usize, Echantillon, usize)> = Vec::new();
    let mut base = 0usize;
    for (s, spec) in specs.iter().enumerate() {
        for ech in spec.echantillons() {
            let global = base + ech.entree;
            table.push((s, ech, global));
        }
        base += spec.nombre();
    }
    let total: usize = n_entrees.iter().sum();
    let frames: Vec<(usize, usize)> = table.iter().map(|(_, e, _)| (e.plan, e.frame)).collect();
    let moyenne_demandee = specs.iter().any(|s| s.options.prise == Prise::Moyenne);
    let cumuls: Arc<Mutex<Vec<Option<Cumul>>>> = Arc::new(Mutex::new((0..total).map(|_| None).collect()));
    let table = Arc::new(table);
    let (table_e, cumuls_e) = (table.clone(), cumuls.clone());
    let titre = format!("Color script ({total} image{})", if total > 1 { "s" } else { "" });
    let premier = cibles[0].clone();
    let fps = specs.first().map_or(24, |s| s.fps);
    let specs = Arc::new(specs);
    let job = ExportJob::collect_with(
        titre,
        "Image",
        &premier,
        frames,
        move |k, px| -> Result<Option<RgbImage>, String> {
            let (_, ech, global) = &table_e[k];
            let img = crate::plancheui::rgb_sur_blanc(px, frame, ech.cible)?;
            if moyenne_demandee {
                let mut c = cumuls_e.lock().unwrap_or_else(|e| e.into_inner());
                let slot = &mut c[*global];
                let cumul = slot.get_or_insert_with(|| Cumul { taille: [img.width(), img.height()], somme: vec![0; img.as_raw().len()], n: 0 });
                cumul.ajouter(&img);
                Ok(None)
            } else {
                Ok(Some(img))
            }
        },
        move |images: Vec<Option<RgbImage>>| {
            // L'image de chaque entrée : celle qu'on a gardée, ou la moyenne de ses échantillons.
            let mut par_entree: Vec<Option<RgbImage>> = (0..total).map(|_| None).collect();
            {
                let mut c = cumuls.lock().unwrap_or_else(|e| e.into_inner());
                for (k, img) in images.into_iter().enumerate() {
                    let global = table[k].2;
                    if par_entree[global].is_none() {
                        par_entree[global] = match img {
                            Some(i) => Some(i),
                            None => c[global].take().map(|cu| cu.image()),
                        };
                    }
                }
            }
            let mut ecrits: Vec<PathBuf> = Vec::new();
            let resultat = (|| -> Result<(), String> {
                let mut toutes: Vec<Ligne> = Vec::new();
                let mut base = 0usize;
                for (s, spec) in specs.iter().enumerate() {
                    let utile = spec.palette_utile();
                    let cellules: Vec<Cellule> = (0..spec.nombre())
                        .map(|k| {
                            let img = par_entree[base + k].take().unwrap_or_else(|| RgbImage::new(1, 1));
                            let palette = if utile { dominantes(&img, COULEURS_PAR_LIGNE) } else { Vec::new() };
                            (img, palette)
                        })
                        .collect();
                    base += spec.nombre();
                    if spec.options.donnees != Donnees::Aucune {
                        toutes.extend(lignes(spec, &cellules));
                    }
                    let img = composer(spec, &cellules)?;
                    drop(cellules);
                    let bytes = encoder(&img, spec.options.sortie, &meta)?;
                    drop(img);
                    crate::export::write_atomic(&cibles[s], &bytes)?;
                    ecrits.push(cibles[s].clone());
                }
                if let (Some(chemin), Some(spec)) = (&donnees, specs.first()) {
                    let texte = match spec.options.donnees {
                        Donnees::Csv => csv(&toutes),
                        Donnees::Json => json(&meta, fps, &toutes),
                        Donnees::Aucune => String::new(),
                    };
                    if spec.options.donnees != Donnees::Aucune {
                        crate::export::write_atomic(chemin, texte.as_bytes())?;
                        ecrits.push(chemin.clone());
                    }
                }
                Ok(())
            })();
            if resultat.is_err() {
                for f in &ecrits {
                    let _ = std::fs::remove_file(f);
                }
            }
            resultat
        },
        || {},
    );
    Ok(job.pour_planche(message))
}

#[cfg(test)]
#[path = "colorscript_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "colorscript_sortie_tests.rs"]
mod tests_sortie;
