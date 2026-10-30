//! La pile des versions d'un plan, dans le montage (Anthony, 2026-10-20) : sur le bloc d'un plan qui a
//! plusieurs versions, une petite étiquette « V2 » ; la survoler ouvre, **au-dessus du bloc**, une carte par
//! version (un aperçu, son nom, une croix pour la supprimer) ; un clic sur une carte fait passer le plan à cette
//! version.
//!
//! Ce module est la partie **sans interface** : quelles cartes, où poser l'étiquette sur un bloc (large, étroit,
//! avec ou sans la pastille ambre des envois), où poser la pile, et la petite machine qui dit quand elle
//! s'ouvre et se ferme. Le dessin et les gestes sont dans `pileversui.rs`.

use eframe::egui::{pos2, vec2, Rect};

use crate::document::Document;

/// Largeur et hauteur d'une carte.
pub const CARTE_L: f32 = 172.0;
pub const CARTE_H: f32 = 46.0;
/// Entre deux cartes, et entre les cartes et le bord de la pile.
pub const ECART: f32 = 3.0;
pub const BORD: f32 = 4.0;
/// Hauteur de l'étiquette sur le bloc.
pub const ETIQ_H: f32 = 15.0;
/// Le plus de cartes montrées d'un coup (au-delà, la molette fait défiler).
pub const CARTES_MAX: usize = 6;
/// Secondes que la pile reste ouverte quand le pointeur la quitte (on traverse un petit vide sans la perdre).
pub const GRACE: f64 = 0.25;
/// Marge de chaque côté de l'étiquette (la poignée de bord du bloc, une marque d'image).
const MARGE: f32 = 10.0;
/// De combien la pastille des envois (centrée à 18 points du bord droit, rayon 7) entame le bloc.
const PASTILLE: f32 = 28.0;

/// Une carte de la pile : une version du plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carte {
    pub numero: u32,
    /// La version active : celle que montre le film.
    pub actuelle: bool,
    pub date: u64,
    /// La durée de cette version, en images : chaque version garde la sienne (changer de version change la
    /// durée du plan dans le montage).
    pub duree: usize,
}

/// L'étiquette « V2 » se montre dès qu'un plan a plusieurs versions (un plan à une seule reste propre).
pub fn etiquette_visible(nombre_de_versions: usize) -> bool {
    nombre_de_versions >= 2
}

/// Les cartes du plan `plan`, de haut en bas : par numéro croissant, de sorte que la plus récente est la plus
/// proche du bloc. Vide pour un plan à une seule version.
pub fn cartes(doc: &Document, plan: u64) -> Vec<Carte> {
    let Some(v) = doc.versions_du_plan(plan) else { return Vec::new() };
    let mut out: Vec<Carte> = v.autres.iter().map(|r| Carte { numero: r.numero, actuelle: false, date: r.date, duree: r.panel.frame_count() }).collect();
    let active = doc.panels.iter().find(|p| p.id == plan).map_or(1, crate::document::Panel::frame_count);
    out.push(Carte { numero: v.actuelle, actuelle: true, date: v.date_actuelle, duree: active });
    out.sort_by_key(|c| c.numero);
    out
}

/// Où et comment se montre l'étiquette sur un bloc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Etiquette {
    /// « V2 » dans une pastille.
    Texte(Rect),
    /// Le bloc est trop étroit : un simple point (qui s'ouvre de la même façon).
    Point(Rect),
    /// Rien ne tient (bloc minuscule).
    Aucune,
}

impl Etiquette {
    /// La zone où la souris la survole.
    pub fn zone(&self) -> Option<Rect> {
        match self {
            Etiquette::Texte(r) | Etiquette::Point(r) => Some(*r),
            Etiquette::Aucune => None,
        }
    }
}

/// Place l'étiquette près du haut, au milieu du bloc `bloc`. `texte` : la largeur du texte « V2 » ; `pastille` :
/// le bloc porte la pastille « modifié depuis l'envoi » en haut à droite (une simple bande ambre le long du bord
/// haut ne gêne pas : l'étiquette est dessous). Si le milieu est pris par la pastille, l'étiquette se décale
/// à gauche ; si rien ne tient, c'est un point ; si même un point ne tient pas, rien.
pub fn placer_etiquette(bloc: Rect, texte: f32, pastille: bool) -> Etiquette {
    if bloc.height() < 30.0 {
        return Etiquette::Aucune;
    }
    let haut = bloc.top() + 8.0;
    let gauche = bloc.left() + MARGE;
    let droite = bloc.right() - MARGE - if pastille { PASTILLE - MARGE } else { 0.0 };
    let largeur = texte + 10.0;
    let libre = droite - gauche;
    if libre >= largeur {
        let x0 = (bloc.center().x - largeur / 2.0).clamp(gauche, droite - largeur);
        return Etiquette::Texte(Rect::from_min_size(pos2(x0, haut), vec2(largeur, ETIQ_H)));
    }
    if libre >= 8.0 {
        let c = pos2((gauche + droite) / 2.0, haut + ETIQ_H / 2.0);
        return Etiquette::Point(Rect::from_center_size(c, vec2(libre.min(14.0), 14.0)));
    }
    Etiquette::Aucune
}

/// Marge de l'étiquette d'une case des Vignettes, au bord haut et au bord droit de l'image.
#[allow(dead_code)]
pub const COIN: f32 = 4.0;

/// Place l'étiquette en **haut à droite** de la case `case` (une image de la planche des Vignettes) ; `texte` : la largeur du texte « V2 ».
/// Une case trop étroite ne garde qu'un point ; trop petite, rien.
#[allow(dead_code)]
pub fn placer_etiquette_coin(case: Rect, texte: f32) -> Etiquette {
    if case.height() < 2.0 * COIN + ETIQ_H + 6.0 {
        return Etiquette::Aucune;
    }
    let largeur = texte + 10.0;
    let libre = case.width() - 2.0 * COIN;
    if libre >= largeur {
        return Etiquette::Texte(Rect::from_min_size(pos2(case.right() - COIN - largeur, case.top() + COIN), vec2(largeur, ETIQ_H)));
    }
    if libre >= 8.0 {
        let largeur = libre.min(14.0);
        return Etiquette::Point(Rect::from_center_size(pos2(case.right() - COIN - largeur / 2.0, case.top() + COIN + ETIQ_H / 2.0), vec2(largeur, 14.0)));
    }
    Etiquette::Aucune
}

/// Où la pile se pose : son cadre, et le rectangle de chaque carte visible (de haut en bas).
#[derive(Clone, Debug, PartialEq)]
pub struct Dispo {
    pub cadre: Rect,
    pub cartes: Vec<Rect>,
    /// Le cadre plus le petit vide qui le sépare de l'étiquette : tant que le pointeur y est, la pile reste ouverte.
    pub zone: Rect,
}

/// Combien de cartes tiennent au-dessus de l'étiquette `etiquette` sur l'écran `ecran` (au moins une, au plus
/// `CARTES_MAX` et `total`).
pub fn cartes_visibles(total: usize, etiquette: Rect, ecran: Rect) -> usize {
    let place = (etiquette.top() - ecran.top() - 2.0 * BORD - 6.0).max(0.0);
    let n = ((place + ECART) / (CARTE_H + ECART)).floor() as usize;
    n.clamp(1, CARTES_MAX).min(total.max(1))
}

/// Pose la pile de `visibles` cartes juste au-dessus de `etiquette`, centrée dessus, dans `ecran`. S'il n'y a pas
/// la place au-dessus, elle est repoussée vers le bas (elle recouvre alors le bloc).
pub fn disposer(etiquette: Rect, visibles: usize, ecran: Rect) -> Dispo {
    let n = visibles.max(1);
    let largeur = CARTE_L + 2.0 * BORD;
    let hauteur = n as f32 * CARTE_H + (n as f32 - 1.0) * ECART + 2.0 * BORD;
    let x = (etiquette.center().x - largeur / 2.0).clamp(ecran.left() + 2.0, (ecran.right() - largeur - 2.0).max(ecran.left() + 2.0));
    let bas = etiquette.top() - 4.0;
    let y = (bas - hauteur).max(ecran.top() + 2.0);
    let cadre = Rect::from_min_size(pos2(x, y), vec2(largeur, hauteur));
    let cartes = (0..n).map(|k| Rect::from_min_size(pos2(x + BORD, y + BORD + k as f32 * (CARTE_H + ECART)), vec2(CARTE_L, CARTE_H))).collect();
    let zone = Rect::from_min_max(cadre.min, pos2(cadre.right(), cadre.bottom().max(etiquette.top()) + 1.0)).union(etiquette.expand(2.0));
    Dispo { cadre, cartes, zone }
}

/// Où est le pointeur par rapport à la pile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pointeur {
    /// Sur l'étiquette du plan `plan`.
    Etiquette(u64),
    /// Sur la pile ouverte (ou dans le petit vide entre elle et l'étiquette).
    Pile,
    /// Ailleurs (ou hors de la fenêtre).
    Ailleurs,
}

/// L'état de la pile : laquelle est ouverte, et ce qu'on y fait.
#[derive(Clone, Debug, Default)]
pub struct Pile {
    /// Le plan dont la pile est ouverte.
    pub plan: Option<u64>,
    /// Depuis quand (heure d'egui) le pointeur n'est ni sur l'étiquette ni sur la pile.
    pub quitte_a: Option<f64>,
    /// Ouverte par un clic sur l'étiquette (stylet sans survol) : elle reste jusqu'à un appui ailleurs.
    pub epinglee: bool,
    /// La version dont on confirme la suppression.
    pub confirme: Option<u32>,
    /// Première carte montrée (quand il y en a plus que de place).
    pub premiere: usize,
    /// Où la pile a été posée à l'image d'avant (pour savoir si le pointeur y est).
    pub zone: Option<Rect>,
    /// Les étiquettes « V2 » posées à l'image d'avant : leur zone et leur plan (essais, gestes).
    pub etiquettes: Vec<(Rect, u64)>,
}

impl Pile {
    /// Un pas : `pointeur`, `appui` (un bouton vient d'être pressé), `occupe` (un geste de la bande est en cours :
    /// la pile se ferme), `now` (heure d'egui). Rend vrai si la pile vient de s'ouvrir.
    pub fn avancer(&mut self, pointeur: Pointeur, appui: bool, occupe: bool, now: f64) -> bool {
        if occupe {
            self.fermer();
            return false;
        }
        match pointeur {
            Pointeur::Etiquette(plan) => {
                self.quitte_a = None;
                if self.plan != Some(plan) {
                    self.ouvrir(plan);
                    return true;
                }
            }
            Pointeur::Pile => self.quitte_a = None,
            Pointeur::Ailleurs => {
                if self.plan.is_none() {
                    return false;
                }
                if self.epinglee {
                    if appui {
                        self.fermer();
                    }
                } else {
                    let depuis = *self.quitte_a.get_or_insert(now);
                    if now - depuis >= GRACE {
                        self.fermer();
                    }
                }
            }
        }
        false
    }

    pub fn ouvrir(&mut self, plan: u64) {
        // (`premiere` : tout en bas d'abord, les versions les plus récentes sont les plus proches du bloc.)
        *self = Pile { plan: Some(plan), premiere: usize::MAX, etiquettes: std::mem::take(&mut self.etiquettes), ..Pile::default() };
    }

    pub fn fermer(&mut self) {
        *self = Pile { etiquettes: std::mem::take(&mut self.etiquettes), ..Pile::default() };
    }

    /// Le plan dont la pile est ouverte.
    pub fn ouverte(&self) -> Option<u64> {
        self.plan
    }

    /// Faut-il redemander une image pour fermer la pile après le délai de grâce ? (Dans combien de secondes.)
    pub fn reveil(&self, now: f64) -> Option<f64> {
        let depuis = self.quitte_a?;
        self.plan?;
        Some((GRACE - (now - depuis)).max(0.0))
    }

    /// Fait défiler la liste de `delta` cartes (molette), entre 0 et `total - visibles`.
    pub fn defiler(&mut self, delta: i32, total: usize, visibles: usize) {
        let max = total.saturating_sub(visibles);
        self.premiere = (self.premiere.min(max) as i64 + delta as i64).clamp(0, max as i64) as usize;
    }
}
