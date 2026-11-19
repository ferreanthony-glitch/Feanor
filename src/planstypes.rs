//! Les plans types (lot 4 des préréglages, partie 176 ; Anthony, 2026-11-18 : « Oui » à P1, P2, P3, P5, P6, P7, P8, P13, P14,
//! P16, P17, P18 de la liste de la partie 170 ; partie 177 : « Oui » aux six restés sans réponse, P4, P9, P10, P11, P12, P15).
//!
//! Un plan type se glisse depuis le panneau Préréglages (onglet Montage) **entre deux plans du montage** : c'est un plan neuf,
//! tout prêt (color card, carton, titre, intertitre, générique de fin, logo, écran partagé, « Précédemment », « À suivre »,
//! carte avec trajet, inserts d'écran). Deux se posent **sur un plan** existant : le lieu et la date (une incrustation qui
//! s'écrit lettre à lettre) et le générique de début (les noms sur les images, l'un après l'autre).
//!
//! Tout est fait de ce que FEANOR a déjà : des calques Couleur unie, des dessins vectoriels avec du texte (l'outil Texte les
//! retouche : double-clic sur le texte), des clés d'opacité pour les fondus, des clés de position pour le générique qui monte,
//! des calques bitmap vides où dessiner. Rien de neuf dans le fichier, sauf le lettre à lettre d'une incrustation (format 60).

use eframe::egui::{pos2, vec2, Pos2, Rect};

use crate::document::{CelId, Document, Layer, LayerKind, LayerTransform};
use crate::text::FALLBACK;
use crate::vector::{Affine, Element, Trace, VPoint, VectorDrawing};

/// Un plan type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlanType {
    /// P1 : un plan de couleur unie.
    CouleurUnie,
    /// P2 : un carton noir, ou blanc.
    CartonNoir,
    CartonBlanc,
    /// P3 : le titre du film, qui apparaît et s'éteint en fondu.
    Titre,
    /// P5 : « Trois jours plus tard… ».
    Intertitre,
    /// P6 : le lieu et la date en bas d'un plan, lettre à lettre (sur un plan).
    LieuEtDate,
    /// P7 : les noms posés sur les images, l'un après l'autre (sur un plan).
    GeneriqueDebut,
    /// P8 : le générique de fin qui monte, à la vitesse que donne la durée du plan.
    GeneriqueFin,
    /// P13 : le logo du studio, qui apparaît en douceur.
    Logo,
    /// P14 : deux, trois ou quatre cases dans le cadre.
    Partage2,
    Partage3,
    Partage4,
    /// P16 : les cartons de série, avec quelques images du film.
    Precedemment,
    ASuivre,
    /// P17 : une carte, une ligne qui se dessine d'un point à l'autre.
    Carte,
    /// P18 : les inserts d'écran.
    Telephone,
    Ordinateur,
    Journal,
    /// P4 (partie 177) : le titre de la séquence, rempli tout seul (une incrustation du nom de la séquence).
    TitreSequence,
    /// P9 : « PLAN 12 — À VENIR », avec sa durée, pour un animatic incomplet.
    PlanManquant,
    /// P10 : l'ardoise d'en-tête : titre, série, épisode, version, date, durée, remplis depuis le projet.
    Ardoise,
    /// P11 : le compte à rebours de livraison, 8, 7, 6… et le bip à 2.
    CompteARebours,
    /// P12 : la mire de couleur et un son à 1 kHz.
    Mire,
    /// P15 : « Fin de l'acte 1 », pour les séries.
    FinDActe,
}

impl PlanType {
    pub const ALL: [PlanType; 24] = [
        PlanType::CouleurUnie,
        PlanType::CartonNoir,
        PlanType::CartonBlanc,
        PlanType::Titre,
        PlanType::TitreSequence,
        PlanType::Intertitre,
        PlanType::FinDActe,
        PlanType::LieuEtDate,
        PlanType::GeneriqueDebut,
        PlanType::GeneriqueFin,
        PlanType::Logo,
        PlanType::Ardoise,
        PlanType::CompteARebours,
        PlanType::Mire,
        PlanType::PlanManquant,
        PlanType::Partage2,
        PlanType::Partage3,
        PlanType::Partage4,
        PlanType::Precedemment,
        PlanType::ASuivre,
        PlanType::Carte,
        PlanType::Telephone,
        PlanType::Ordinateur,
        PlanType::Journal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PlanType::CouleurUnie => "Color card",
            PlanType::CartonNoir => "Carton noir",
            PlanType::CartonBlanc => "Carton blanc",
            PlanType::Titre => "Titre du film",
            PlanType::Intertitre => "Intertitre",
            PlanType::LieuEtDate => "Lieu et date",
            PlanType::GeneriqueDebut => "Générique de début",
            PlanType::GeneriqueFin => "Générique de fin",
            PlanType::Logo => "Logo studio",
            PlanType::Partage2 => "Écran partagé en deux",
            PlanType::Partage3 => "Écran partagé en trois",
            PlanType::Partage4 => "Écran partagé en quatre",
            PlanType::Precedemment => "Précédemment",
            PlanType::ASuivre => "À suivre",
            PlanType::Carte => "Carte avec trajet",
            PlanType::Telephone => "Insert : téléphone",
            PlanType::Ordinateur => "Insert : ordinateur",
            PlanType::Journal => "Insert : journal",
            PlanType::TitreSequence => "Titre de séquence",
            PlanType::PlanManquant => "Plan manquant",
            PlanType::Ardoise => "Ardoise d'en-tête",
            PlanType::CompteARebours => "Compte à rebours",
            PlanType::Mire => "Mire de couleur",
            PlanType::FinDActe => "Fin d'acte",
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            PlanType::CouleurUnie => "Un plan de couleur unie (sa couleur se change sur la ligne du calque, dans la timeline).",
            PlanType::CartonNoir => "Un carton noir, pour respirer ou réserver une durée.",
            PlanType::CartonBlanc => "Un carton blanc, pour respirer ou réserver une durée.",
            PlanType::Titre => "Le titre du film (celui des propriétés du projet) apparaît et s'éteint en fondu ; l'outil Texte le retouche.",
            PlanType::Intertitre => "« Trois jours plus tard… » sur un carton noir ; l'outil Texte le retouche.",
            PlanType::LieuEtDate => "En bas du plan, le lieu et la date s'écrivent lettre à lettre (une incrustation : son texte se change dans les réglages du calque).",
            PlanType::GeneriqueDebut => "Les noms posés sur les images du plan, l'un après l'autre, en fondu ; l'outil Texte les retouche, un dessin par nom.",
            PlanType::GeneriqueFin => "Les noms montent sur un fond noir ; la vitesse suit la durée du plan (allonger le plan les fait monter moins vite).",
            PlanType::Logo => "Un carton noir et un calque Logo qui apparaît en douceur : choisis ton image, elle s'y pose.",
            PlanType::Partage2 => "Deux cases côte à côte, chacune son calque où dessiner.",
            PlanType::Partage3 => "Trois cases côte à côte, chacune son calque où dessiner.",
            PlanType::Partage4 => "Quatre cases, deux en haut, deux en bas, chacune son calque où dessiner.",
            PlanType::Precedemment => "Le carton « Précédemment… » sur quelques images des plans d'avant.",
            PlanType::ASuivre => "Le carton « À suivre… » sur quelques images des plans d'après.",
            PlanType::Carte => "Une carte, et une ligne qui se dessine d'un point à l'autre.",
            PlanType::Telephone => "Un téléphone et ses messages, qui arrivent l'un après l'autre.",
            PlanType::Ordinateur => "Un écran d'ordinateur, une fenêtre et son texte.",
            PlanType::Journal => "La une d'un journal : le nom, le grand titre, les colonnes.",
            PlanType::TitreSequence => "Le nom de la séquence qui commence, rempli tout seul (renommer la séquence le change) ; il ouvre la séquence du plan d'après.",
            PlanType::PlanManquant => "« Plan 12 — À VENIR » et sa durée, remplis tout seuls : la place d'un plan pas encore dessiné, dans un animatic.",
            PlanType::Ardoise => "Le titre, la série, l'épisode, la date et la durée du film, remplis depuis le projet (Fichier > Propriétés) ; la version se tape.",
            PlanType::CompteARebours => "8, 7, 6… jusqu'à 2, avec le bip d'une image sur le 2, puis du noir : le compte à rebours des livraisons.",
            PlanType::Mire => "Les barres de couleur, et un son à 1 kHz sur toute la durée du plan.",
            PlanType::FinDActe => "« Fin de l'acte 1 » sur un carton noir, en fondu (le numéro suit les fins d'acte d'avant).",
        }
    }

    /// Son numéro dans la liste de la partie 170.
    pub fn numero(self) -> &'static str {
        match self {
            PlanType::CouleurUnie => "P1",
            PlanType::CartonNoir | PlanType::CartonBlanc => "P2",
            PlanType::Titre => "P3",
            PlanType::Intertitre => "P5",
            PlanType::LieuEtDate => "P6",
            PlanType::GeneriqueDebut => "P7",
            PlanType::GeneriqueFin => "P8",
            PlanType::Logo => "P13",
            PlanType::Partage2 | PlanType::Partage3 | PlanType::Partage4 => "P14",
            PlanType::Precedemment | PlanType::ASuivre => "P16",
            PlanType::Carte => "P17",
            PlanType::Telephone | PlanType::Ordinateur | PlanType::Journal => "P18",
            PlanType::TitreSequence => "P4",
            PlanType::PlanManquant => "P9",
            PlanType::Ardoise => "P10",
            PlanType::CompteARebours => "P11",
            PlanType::Mire => "P12",
            PlanType::FinDActe => "P15",
        }
    }

    /// Se pose sur un plan existant (des calques de plus), au lieu d'être un plan neuf.
    pub fn sur_un_plan(self) -> bool {
        matches!(self, PlanType::LieuEtDate | PlanType::GeneriqueDebut)
    }

    /// La durée d'un plan neuf de ce type, en secondes.
    pub fn secondes(self) -> f32 {
        match self {
            PlanType::CartonNoir | PlanType::CartonBlanc => 1.0,
            PlanType::CouleurUnie => 2.0,
            PlanType::Intertitre | PlanType::Logo | PlanType::TitreSequence | PlanType::PlanManquant | PlanType::FinDActe => 3.0,
            PlanType::CompteARebours => 8.0,
            PlanType::Mire => 5.0,
            PlanType::GeneriqueFin => 10.0,
            PlanType::Precedemment | PlanType::ASuivre | PlanType::Carte => 5.0,
            _ => 4.0,
        }
    }
}

/// Un plan type glissé depuis le panneau Préréglages : il se pose sur la coupe (ou sur le plan) où on le lâche.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PlanTypeDrag {
    pub plan: PlanType,
}

// ------------------------------------------------------------------------------------------ les briques

const BLANC: [u8; 3] = [250, 250, 248];
const NOIR: [u8; 3] = [8, 8, 10];
const GRIS: [u8; 3] = [165, 165, 170];

/// Un rectangle aux coins arrondis, en points (pour un remplissage).
fn arrondi(r: Rect, rayon: f32) -> Vec<[f32; 2]> {
    let k = rayon.min(r.width() / 2.0).min(r.height() / 2.0).max(0.0);
    let mut out = Vec::with_capacity(28);
    for (c, a0) in [(pos2(r.right() - k, r.top() + k), -90.0_f32), (pos2(r.right() - k, r.bottom() - k), 0.0), (pos2(r.left() + k, r.bottom() - k), 90.0), (pos2(r.left() + k, r.top() + k), 180.0)] {
        for i in 0..=6 {
            let a = (a0 + i as f32 * 15.0).to_radians();
            out.push([c.x + k * a.cos(), c.y + k * a.sin()]);
        }
    }
    out
}

fn remplissage(contour: Vec<[f32; 2]>, color: [u8; 3]) -> Element {
    Element::Fill { contours: vec![contour], color, opacity: 1.0, gradient: None }
}

fn rectangle(r: Rect, color: [u8; 3]) -> Element {
    remplissage(vec![[r.left(), r.top()], [r.right(), r.top()], [r.right(), r.bottom()], [r.left(), r.bottom()]], color)
}

/// Un trait d'épaisseur `rayon` (rayon de la pointe) par les points `points`.
fn trait_par(points: &[Pos2], rayon: f32, color: [u8; 3]) -> Element {
    Element::Stroke { points: points.iter().map(|p| VPoint { x: p.x, y: p.y, r: rayon }).collect(), color, opacity: 1.0, hardness: 0.85, brush: None, trace: Trace::default() }
}

/// Un texte de taille `taille` (hauteur des lettres, en pixels de la feuille), posé `ancre` : son centre (`Some(0.5)`), son bord
/// gauche (`0.0`) ou son bord droit (`1.0`) au point `p`, verticalement centré.
fn texte(t: &str, taille: f32, color: [u8; 3], p: Pos2, ancre: f32) -> Element {
    let laid = crate::text::layout(t, FALLBACK, taille);
    let b = laid.bounds;
    let origine = pos2(p.x - b.left() - b.width() * ancre, p.y - b.center().y);
    Element::text(t.to_string(), FALLBACK.to_string(), taille, color, Affine::translate(origine.to_vec2()))
}

/// La largeur du texte `t` à la taille `taille`.
fn largeur(t: &str, taille: f32) -> f32 {
    crate::text::layout(t, FALLBACK, taille).bounds.width()
}

/// La taille qui fait tenir `t` dans `w` pixels, sans dépasser `taille`.
fn taille_qui_tient(t: &str, taille: f32, w: f32) -> f32 {
    let l = largeur(t, taille);
    if l > w && l > 0.0 {
        taille * w / l
    } else {
        taille
    }
}

impl Document {
    /// Un dessin vectoriel neuf fait de `elements`.
    fn dessin_vectoriel(&mut self, elements: Vec<Element>) -> CelId {
        let id = self.new_cel(LayerKind::Vector);
        if let Some(cel) = self.cels.get_mut(&id) {
            cel.vector = Some(VectorDrawing { elements });
            cel.rerender();
        }
        id
    }

    /// Un calque vectoriel nommé `nom` qui montre le dessin `elements` pendant `n` images.
    fn calque_dessine(&mut self, nom: &str, elements: Vec<Element>, n: usize) -> Layer {
        let id = self.dessin_vectoriel(elements);
        let mut l = Layer::new(nom, LayerKind::Vector);
        l.frames = vec![Some(id); n];
        l
    }

    /// Un calque bitmap nommé `nom`, un dessin vide montré pendant les `n` images : ce qu'on y dessine tient tout le plan (le
    /// logo, une case d'écran partagé).
    fn calque_vide(&mut self, nom: &str, n: usize) -> Layer {
        let id = self.new_cel(LayerKind::Bitmap);
        let mut l = Layer::new(nom, LayerKind::Bitmap);
        l.frames = vec![Some(id); n];
        l
    }

    /// Un point du cadre filmé (au repos), en parts de sa largeur et de sa hauteur, sur la feuille.
    fn au_cadre(&self, x: f32, y: f32) -> Pos2 {
        let fr = self.frame_rect();
        pos2(fr.left() + x * fr.width(), fr.top() + y * fr.height())
    }

    /// Un plan neuf de `n` images sur la coupe `cut` (0 : au début du film), sans calque ; rend son rang. `avec_la_suite` : il
    /// est de la séquence du plan d'après (un titre de séquence l'ouvre), sinon de celle du plan d'avant.
    fn plan_neuf(&mut self, cut: usize, n: usize, avec_la_suite: bool) -> usize {
        let i = if cut == 0 || self.panels.is_empty() || (avec_la_suite && cut < self.panels.len()) {
            self.add_scene_before(cut.min(self.panels.len().saturating_sub(1)))
        } else {
            self.add_scene_after(cut.min(self.panels.len()) - 1)
        };
        self.panels[i].layers.clear();
        self.panels[i].length = n.max(1);
        i
    }

    /// Les images finales de quelques plans (`plans`, au milieu de chacun), à poser dans le cadre d'un plan neuf : des dessins
    /// bitmap à la taille de la feuille.
    fn images_du_film(&mut self, plans: &[usize]) -> Vec<CelId> {
        let fr = self.frame_rect();
        let (fx, fy) = (fr.left().round() as i64, fr.top().round() as i64);
        let [w, h] = self.frame;
        let mut out = Vec::new();
        for &p in plans {
            let Some(panel) = self.panels.get(p) else { continue };
            let f = panel.frame_count() / 2;
            let plan = crate::render::panel_plan_at(self, p, f, self.render_paper());
            let img = crate::render::camera_image(self, &plan, &self.panel_camera(p, f));
            let id = self.new_cel(LayerKind::Bitmap);
            let (sw, sh) = (self.width as i64, self.height as i64);
            if let Some(cel) = self.cels.get_mut(&id) {
                for y in 0..h {
                    for x in 0..w {
                        let (sx, sy) = (fx + x as i64, fy + y as i64);
                        if sx < 0 || sy < 0 || sx >= sw || sy >= sh {
                            continue;
                        }
                        let (sx, sy) = (sx as usize, sy as usize);
                        let idx = cel.raster.tile_index_of(sx, sy);
                        cel.raster.tile_mut(idx)[(sy % crate::raster::TILE) * crate::raster::TILE + sx % crate::raster::TILE] = img[y * w + x];
                    }
                }
                cel.version += 1;
            }
            out.push(id);
        }
        out
    }

    /// Pose le plan type `t` sur la coupe `cut` (0 à `panels.len()`) : un plan neuf, tout prêt. Rend son rang ; rien pour un type
    /// qui se pose sur un plan (`PlanType::sur_un_plan`).
    pub fn poser_plan_type(&mut self, t: PlanType, cut: usize) -> Option<usize> {
        if t.sur_un_plan() || cut > self.panels.len() {
            return None;
        }
        let n = ((t.secondes() * self.fps.max(1) as f32).round() as usize).max(1);
        let fps = self.fps.max(1) as f32;
        let fr = self.frame_rect();
        let (fw, fh) = (fr.width(), fr.height());
        let secondes = |s: f32| ((s * fps).round() as usize).max(1);
        // Ce qui se lit des plans d'à côté, avant que le plan neuf ne décale les rangs.
        let images = match t {
            PlanType::Precedemment => self.images_du_film(&(cut.saturating_sub(4)..cut).collect::<Vec<_>>()),
            PlanType::ASuivre => self.images_du_film(&(cut..(cut + 4).min(self.panels.len())).collect::<Vec<_>>()),
            _ => Vec::new(),
        };
        // (Une fin d'acte compte celles d'avant.)
        let acte = 1 + self.panels[..cut.min(self.panels.len())].iter().filter(|p| p.layers.iter().any(|l| l.name == NOM_FIN_D_ACTE)).count();
        let i = self.plan_neuf(cut, n, t == PlanType::TitreSequence);
        let mut calques: Vec<Layer> = Vec::new();
        let fond = |c: [u8; 3], nom: &str| Layer::new_unie(nom, c);
        match t {
            PlanType::CouleurUnie => calques.push(fond(crate::couleurunie::DEFAUT, crate::couleurunie::NOM)),
            PlanType::CartonNoir => calques.push(fond(NOIR, "Carton")),
            PlanType::CartonBlanc => calques.push(fond(BLANC, "Carton")),
            PlanType::Titre | PlanType::Intertitre => {
                calques.push(fond(NOIR, "Carton"));
                let (mot, taille, fondu, nom) = if t == PlanType::Titre {
                    let titre = self.properties.title.trim();
                    (if titre.is_empty() { "Titre du film".to_string() } else { titre.to_string() }, 0.13, secondes(0.5), "Titre")
                } else {
                    ("Trois jours plus tard…".to_string(), 0.075, secondes(0.25), "Intertitre")
                };
                let taille = taille_qui_tient(&mot, taille * fh, 0.86 * fw);
                let mut l = self.calque_dessine(nom, vec![texte(&mot, taille, BLANC, self.au_cadre(0.5, 0.5), 0.5)], n);
                fondu_entree_sortie(&mut l, n, fondu, fondu);
                calques.push(l);
            }
            PlanType::GeneriqueFin => {
                calques.push(fond(NOIR, "Carton"));
                let auteur = self.properties.author.trim().to_string();
                let nom = |s: &str| if s.is_empty() { "Prénom Nom".to_string() } else { s.to_string() };
                let titre = self.properties.title.trim().to_string();
                let lignes: Vec<(String, f32, [u8; 3])> = vec![
                    (if titre.is_empty() { "Générique".into() } else { titre }, 0.08, BLANC),
                    (String::new(), 0.05, BLANC),
                    ("Réalisation".into(), 0.04, GRIS),
                    (nom(&auteur), 0.055, BLANC),
                    (String::new(), 0.05, BLANC),
                    ("Storyboard".into(), 0.04, GRIS),
                    (nom(&auteur), 0.055, BLANC),
                    (String::new(), 0.05, BLANC),
                    ("Animatique".into(), 0.04, GRIS),
                    ("Prénom Nom".into(), 0.055, BLANC),
                    (String::new(), 0.05, BLANC),
                    ("Musique".into(), 0.04, GRIS),
                    ("Prénom Nom".into(), 0.055, BLANC),
                    (String::new(), 0.05, BLANC),
                    ("Merci à toute l'équipe".into(), 0.045, BLANC),
                ];
                let mut y = fr.top();
                let mut elements = Vec::new();
                for (mot, taille, c) in &lignes {
                    let taille = taille * fh;
                    y += taille * 0.75;
                    if !mot.is_empty() {
                        elements.push(texte(mot, taille_qui_tient(mot, taille, 0.9 * fw), *c, pos2(fr.center().x, y), 0.5));
                    }
                    y += taille * 0.75;
                }
                let hauteur = y - fr.top();
                let mut l = self.calque_dessine("Générique", elements, n);
                // Il monte du bas du cadre jusqu'à sortir par le haut, à vitesse constante.
                let debut = LayerTransform { y: fh, ..LayerTransform::default() };
                let fin = LayerTransform { y: -hauteur, ..LayerTransform::default() };
                l.transform = debut;
                l.pos_keys = vec![crate::poskeys::PosKey::new(0, debut), crate::poskeys::PosKey::new(n.saturating_sub(1), fin)];
                calques.push(l);
            }
            PlanType::Logo => {
                calques.push(fond(NOIR, "Carton"));
                let mut l = self.calque_vide("Logo", n);
                fondu_entree_sortie(&mut l, n, secondes(0.6), secondes(0.4));
                calques.push(l);
            }
            PlanType::Partage2 | PlanType::Partage3 | PlanType::Partage4 => {
                let cases = match t {
                    PlanType::Partage2 => 2,
                    PlanType::Partage3 => 3,
                    _ => 4,
                };
                let e = (0.012 * fw).max(2.0);
                let mut traits = Vec::new();
                let colonne = |x: f32| rectangle(Rect::from_min_max(pos2(fr.left() + x * fw - e / 2.0, fr.top()), pos2(fr.left() + x * fw + e / 2.0, fr.bottom())), NOIR);
                match cases {
                    2 => traits.push(colonne(0.5)),
                    3 => traits.extend([colonne(1.0 / 3.0), colonne(2.0 / 3.0)]),
                    _ => {
                        traits.push(colonne(0.5));
                        traits.push(rectangle(Rect::from_min_max(pos2(fr.left(), fr.center().y - e / 2.0), pos2(fr.right(), fr.center().y + e / 2.0)), NOIR));
                    }
                }
                for k in 1..=cases {
                    let l = self.calque_vide(&format!("Case {k}"), n);
                    calques.push(l);
                }
                calques.push(self.calque_dessine("Séparations", traits, n));
            }
            PlanType::Precedemment | PlanType::ASuivre => {
                calques.push(fond(NOIR, "Carton"));
                if !images.is_empty() {
                    let mut l = Layer::new("Images du film", LayerKind::Bitmap);
                    let par = (n / images.len()).max(1);
                    l.frames = (0..n).map(|f| Some(images[(f / par).min(images.len() - 1)])).collect();
                    calques.push(l);
                }
                let mot = if t == PlanType::Precedemment { "PRÉCÉDEMMENT…" } else { "À SUIVRE…" };
                let bande = rectangle(Rect::from_min_max(pos2(fr.left(), fr.top() + 0.06 * fh), pos2(fr.left() + 0.5 * fw, fr.top() + 0.2 * fh)), NOIR);
                let mut l = self.calque_dessine("Carton de série", vec![bande, texte(mot, 0.07 * fh, BLANC, self.au_cadre(0.05, 0.13), 0.0)], n);
                fondu_entree_sortie(&mut l, n, secondes(0.3), 0);
                calques.push(l);
            }
            PlanType::Carte => {
                calques.push(fond([232, 220, 192], "Papier de la carte"));
                // Un rivage, une rivière, deux villes.
                let brun = [120, 92, 60];
                let rivage: Vec<Pos2> = (0..=24).map(|k| k as f32 / 24.0).map(|u| self.au_cadre(0.08 + 0.3 * u + 0.04 * (u * 9.0).sin(), 0.05 + 0.9 * u)).collect();
                let riviere: Vec<Pos2> = (0..=16).map(|k| k as f32 / 16.0).map(|u| self.au_cadre(0.55 + 0.35 * u, 0.9 - 0.25 * u + 0.05 * (u * 7.0).sin())).collect();
                let (a, b) = (self.au_cadre(0.42, 0.7), self.au_cadre(0.76, 0.32));
                let ville = |p: Pos2| remplissage(arrondi(Rect::from_center_size(p, vec2(0.025 * fh, 0.025 * fh)), 0.0125 * fh), [150, 40, 30]);
                let carte = vec![
                    trait_par(&rivage, 0.004 * fh, brun),
                    trait_par(&riviere, 0.003 * fh, [80, 110, 150]),
                    ville(a),
                    ville(b),
                    texte("Départ", 0.045 * fh, brun, a + vec2(0.03 * fw, 0.06 * fh), 0.0),
                    texte("Arrivée", 0.045 * fh, brun, b + vec2(0.0, -0.06 * fh), 0.5),
                ];
                calques.push(self.calque_dessine("Carte", carte, n));
                // La ligne du trajet : une courbe de A à B, en tirets, qui se dessine sur les deux tiers du plan, puis reste.
                let milieu = self.au_cadre(0.45, 0.25);
                let courbe = |u: f32| {
                    let v = 1.0 - u;
                    pos2(v * v * a.x + 2.0 * v * u * milieu.x + u * u * b.x, v * v * a.y + 2.0 * v * u * milieu.y + u * u * b.y)
                };
                let tirets = 18;
                let dessin = (n * 2 / 3).max(1);
                let etapes = tirets.min(dessin);
                let mut cels = Vec::with_capacity(etapes);
                for e in 1..=etapes {
                    let jusqu_a = e as f32 / etapes as f32;
                    let mut elements = Vec::new();
                    for k in 0..tirets {
                        let (u0, u1) = (k as f32 / tirets as f32, (k as f32 + 0.55) / tirets as f32);
                        if u0 >= jusqu_a {
                            break;
                        }
                        let u1 = u1.min(jusqu_a);
                        let pts: Vec<Pos2> = (0..=4).map(|j| courbe(u0 + (u1 - u0) * j as f32 / 4.0)).collect();
                        elements.push(trait_par(&pts, 0.006 * fh, [150, 40, 30]));
                    }
                    cels.push(self.dessin_vectoriel(elements));
                }
                let mut l = Layer::new("Trajet", LayerKind::Vector);
                l.frames = (0..n).map(|f| Some(cels[((f * etapes) / dessin).min(etapes - 1)])).collect();
                calques.push(l);
            }
            PlanType::Telephone => {
                calques.push(fond([196, 204, 214], "Fond"));
                let corps = Rect::from_center_size(self.au_cadre(0.5, 0.5), vec2(0.46 * fh, 0.9 * fh));
                let ecran = corps.shrink2(vec2(0.025 * fh, 0.06 * fh));
                calques.push(self.calque_dessine("Téléphone", vec![remplissage(arrondi(corps, 0.06 * fh), [28, 28, 32]), remplissage(arrondi(ecran, 0.02 * fh), [246, 246, 248])], n));
                // Trois messages qui arrivent l'un après l'autre, chacun son calque (l'outil Texte retouche le texte).
                let messages = [("Tu es où ?", false), ("J'arrive.", true), ("Dépêche-toi…", false)];
                for (k, (mot, moi)) in messages.iter().enumerate() {
                    let taille = 0.04 * fh;
                    let w = largeur(mot, taille) + 0.04 * fh;
                    let y = ecran.top() + (0.14 + 0.13 * k as f32) * fh;
                    let x0 = if *moi { ecran.right() - 0.02 * fh - w } else { ecran.left() + 0.02 * fh };
                    let bulle = Rect::from_min_size(pos2(x0, y - 0.035 * fh), vec2(w, 0.07 * fh));
                    let (fond_bulle, encre) = if *moi { ([52, 120, 246], BLANC) } else { ([226, 226, 232], NOIR) };
                    let mut l = self.calque_dessine(&format!("Message {}", k + 1), vec![remplissage(arrondi(bulle, 0.03 * fh), fond_bulle), texte(mot, taille, encre, bulle.center(), 0.5)], n);
                    let arrivee = secondes(0.5 + k as f32);
                    if arrivee > 0 && arrivee < n {
                        l.set_opacity_keys(&[(arrivee - 1, 0.0), (arrivee, 1.0)]);
                    }
                    calques.push(l);
                }
            }
            PlanType::Ordinateur => {
                calques.push(fond([58, 62, 70], "Fond"));
                let moniteur = Rect::from_center_size(self.au_cadre(0.5, 0.45), vec2(0.62 * fw, 0.66 * fh));
                let ecran = moniteur.shrink(0.025 * fh);
                let pied = Rect::from_center_size(pos2(moniteur.center().x, moniteur.bottom() + 0.08 * fh), vec2(0.12 * fw, 0.14 * fh));
                let socle = Rect::from_center_size(pos2(moniteur.center().x, pied.bottom()), vec2(0.26 * fw, 0.03 * fh));
                let fenetre = ecran.shrink2(vec2(0.06 * fw, 0.06 * fh));
                let barre = Rect::from_min_max(fenetre.min, pos2(fenetre.right(), fenetre.top() + 0.06 * fh));
                let mut elements = vec![
                    rectangle(pied, [40, 40, 44]),
                    remplissage(arrondi(socle, 0.01 * fh), [40, 40, 44]),
                    remplissage(arrondi(moniteur, 0.02 * fh), [24, 24, 28]),
                    rectangle(ecran, [70, 110, 160]),
                    rectangle(fenetre, [244, 244, 246]),
                    rectangle(barre, [210, 212, 218]),
                    texte("Message", 0.035 * fh, NOIR, pos2(barre.left() + 0.02 * fw, barre.center().y), 0.0),
                    texte("Objet : rendez-vous", 0.04 * fh, NOIR, pos2(fenetre.left() + 0.03 * fw, barre.bottom() + 0.07 * fh), 0.0),
                ];
                for k in 0..5 {
                    let y = barre.bottom() + (0.15 + 0.065 * k as f32) * fh;
                    let fin = if k == 4 { 0.45 } else { 0.85 };
                    elements.push(rectangle(Rect::from_min_max(pos2(fenetre.left() + 0.03 * fw, y), pos2(fenetre.left() + fin * fenetre.width(), y + 0.018 * fh)), [190, 192, 198]));
                }
                calques.push(self.calque_dessine("Écran", elements, n));
            }
            PlanType::Journal => {
                calques.push(fond([66, 66, 70], "Fond"));
                let page = Rect::from_center_size(self.au_cadre(0.5, 0.52), vec2(0.62 * fh, 0.94 * fh));
                let marge = 0.04 * fh;
                let papier = [240, 236, 224];
                let encre = [30, 28, 26];
                let nom = "LE QUOTIDIEN";
                let titre = "LA UNE DU JOUR";
                let mut elements = vec![
                    rectangle(page, papier),
                    texte(nom, taille_qui_tient(nom, 0.08 * fh, page.width() - 2.0 * marge), encre, pos2(page.center().x, page.top() + 0.08 * fh), 0.5),
                    rectangle(Rect::from_min_max(pos2(page.left() + marge, page.top() + 0.135 * fh), pos2(page.right() - marge, page.top() + 0.142 * fh)), encre),
                    texte(titre, taille_qui_tient(titre, 0.07 * fh, page.width() - 2.0 * marge), encre, pos2(page.center().x, page.top() + 0.21 * fh), 0.5),
                ];
                // Les colonnes : des lignes grises ; la photo sur les deux premières, le texte dessous.
                let colonnes = 3;
                let cw = (page.width() - 2.0 * marge) / colonnes as f32;
                elements.push(rectangle(Rect::from_min_max(pos2(page.left() + marge, page.top() + 0.27 * fh), pos2(page.left() + marge + 2.0 * cw - marge / 2.0, page.top() + 0.5 * fh)), [170, 166, 156]));
                for c in 0..colonnes {
                    let x0 = page.left() + marge + c as f32 * cw;
                    let haut = if c < 2 { page.top() + 0.53 * fh } else { page.top() + 0.27 * fh };
                    let mut y = haut;
                    while y < page.bottom() - marge {
                        elements.push(rectangle(Rect::from_min_max(pos2(x0, y), pos2(x0 + cw - marge / 2.0, y + 0.01 * fh)), [150, 146, 138]));
                        y += 0.025 * fh;
                    }
                }
                calques.push(self.calque_dessine("Journal", elements, n));
            }
            PlanType::TitreSequence => {
                calques.push(fond(NOIR, "Carton"));
                let inc = incrustation_au_centre(crate::incrustation::Source::NomSequence, "", 9.0);
                calques.push(self.calque_incrustation("Titre de séquence", inc, n));
            }
            PlanType::PlanManquant => {
                calques.push(fond([92, 94, 100], "Carton"));
                let gris = [128, 130, 138];
                let croix = vec![trait_par(&[fr.left_top(), fr.right_bottom()], 0.004 * fh, gris), trait_par(&[fr.right_top(), fr.left_bottom()], 0.004 * fh, gris)];
                calques.push(self.calque_dessine("Croix", croix, n));
                let inc = incrustation_au_centre(crate::incrustation::Source::Libre, "{plan} — À VENIR\n{duree}", 7.0);
                calques.push(self.calque_incrustation("Plan manquant", inc, n));
            }
            PlanType::Ardoise => {
                calques.push(fond(NOIR, "Carton"));
                let cadre = Rect::from_min_max(self.au_cadre(0.05, 0.08), self.au_cadre(0.95, 0.92));
                calques.push(self.calque_dessine("Bord", vec![trait_par(&[cadre.left_top(), cadre.right_top(), cadre.right_bottom(), cadre.left_bottom(), cadre.left_top()], 0.003 * fh, GRIS)], n));
                let mut inc = incrustation_au_centre(
                    crate::incrustation::Source::Libre,
                    "{titre}\nSérie : {serie}   Épisode : {episode}\nVersion : 1\nDate : {date}\nDurée du film : {duree_film}",
                    5.5,
                );
                inc.ancre = crate::incrustation::Ancre::MilieuGauche;
                inc.marge = 10.0;
                calques.push(self.calque_incrustation("Ardoise", inc, n));
            }
            PlanType::CompteARebours => {
                // Huit secondes : 8 à 3 une seconde chacun, le 2 une seule image avec le bip, puis du noir.
                let f = self.fps.max(1) as usize;
                calques.push(fond([112, 112, 112], "Fond"));
                let c = fr.center();
                let rond = |r: f32| -> Vec<Pos2> { (0..=48).map(|k| k as f32 / 48.0 * std::f32::consts::TAU).map(|a| pos2(c.x + r * a.cos(), c.y + r * a.sin())).collect() };
                let encre = [24, 24, 24];
                let mire = vec![
                    trait_par(&rond(0.40 * fh), 0.006 * fh, BLANC),
                    trait_par(&rond(0.34 * fh), 0.004 * fh, BLANC),
                    trait_par(&[pos2(fr.left(), c.y), pos2(fr.right(), c.y)], 0.003 * fh, encre),
                    trait_par(&[pos2(c.x, fr.top()), pos2(c.x, fr.bottom())], 0.003 * fh, encre),
                ];
                calques.push(self.calque_dessine("Cercles", mire, n));
                // L'aiguille : un tour par seconde, de 8 à 3.
                let aiguille = self.dessin_vectoriel(vec![trait_par(&[c, pos2(c.x, c.y - 0.40 * fh)], 0.008 * fh, encre)]);
                let mut l = Layer::new("Aiguille", LayerKind::Vector);
                l.frames = (0..n).map(|k| (k < 6 * f).then_some(aiguille)).collect();
                let mut cles = Vec::new();
                for s in 0..6 {
                    let tour = 360.0 * (f.saturating_sub(1)) as f32 / f as f32;
                    cles.push(crate::poskeys::PosKey::new(s * f, LayerTransform::default()));
                    cles.push(crate::poskeys::PosKey::new(s * f + f - 1, LayerTransform { angle: tour, ..LayerTransform::default() }));
                }
                l.pos_keys = cles;
                calques.push(l);
                let chiffres: Vec<CelId> = (2..=8).rev().map(|k| self.dessin_vectoriel(vec![texte(&k.to_string(), 0.5 * fh, encre, c, 0.5)])).collect();
                let mut l = Layer::new("Chiffres", LayerKind::Vector);
                l.frames = (0..n).map(|k| if k < 6 * f { Some(chiffres[k / f]) } else if k == 6 * f { Some(chiffres[6]) } else { None }).collect();
                calques.push(l);
                let mut noir = fond(NOIR, "Noir");
                if 6 * f + 1 < n {
                    noir.set_opacity_keys(&[(6 * f, 0.0), (6 * f + 1, 1.0)]);
                }
                calques.push(noir);
            }
            PlanType::Mire => {
                // Les barres : sept bandes à 75 %, une rangée de rappel, puis le bas.
                let barres: [[u8; 3]; 7] = [[191, 191, 191], [191, 191, 0], [0, 191, 191], [0, 191, 0], [191, 0, 191], [191, 0, 0], [0, 0, 191]];
                let rappel: [[u8; 3]; 7] = [[0, 0, 191], [16, 16, 16], [191, 0, 191], [16, 16, 16], [0, 191, 191], [16, 16, 16], [191, 191, 191]];
                let bas: [([u8; 3], f32); 6] = [([0, 33, 76], 1.25), ([255, 255, 255], 1.25), ([50, 0, 106], 1.25), ([16, 16, 16], 1.25), ([8, 8, 8], 0.5), ([16, 16, 16], 1.5)];
                let mut elements = Vec::new();
                let bande = |x0: f32, x1: f32, y0: f32, y1: f32| Rect::from_min_max(self.au_cadre(x0, y0), self.au_cadre(x1, y1));
                for k in 0..7 {
                    let (x0, x1) = (k as f32 / 7.0, (k + 1) as f32 / 7.0);
                    elements.push(rectangle(bande(x0, x1, 0.0, 0.67), barres[k]));
                    elements.push(rectangle(bande(x0, x1, 0.67, 0.75), rappel[k]));
                }
                let mut x = 0.0;
                for (c, part) in bas {
                    let w = part / 7.0;
                    elements.push(rectangle(bande(x, (x + w).min(1.0), 0.75, 1.0), c));
                    x += w;
                }
                calques.push(self.calque_dessine("Mire", elements, n));
            }
            PlanType::FinDActe => {
                calques.push(fond(NOIR, "Carton"));
                let mot = format!("Fin de l'acte {acte}");
                let mut l = self.calque_dessine(NOM_FIN_D_ACTE, vec![texte(&mot, taille_qui_tient(&mot, 0.09 * fh, 0.86 * fw), BLANC, self.au_cadre(0.5, 0.5), 0.5)], n);
                fondu_entree_sortie(&mut l, n, secondes(0.4), secondes(0.4));
                calques.push(l);
            }
            PlanType::LieuEtDate | PlanType::GeneriqueDebut => {}
        }
        self.panels[i].layers = calques;
        // Le son du compte à rebours (un bip d'une image sur le 2) et de la mire (1 kHz tout le plan) : dans le montage, au temps
        // du plan neuf (une étape d'annulation avec lui).
        let debut = self.panel_start(i);
        let f = self.fps.max(1) as usize;
        match t {
            PlanType::CompteARebours => self.poser_un_son("Bip du compte à rebours (1 kHz)", 1.0 / f as f32, 0.5, debut + 6 * f),
            PlanType::Mire => self.poser_un_son("Son de la mire (1 kHz)", n as f32 / f as f32, 0.1, debut),
            _ => {}
        }
        Some(i)
    }

    /// Un son à 1 kHz de `secondes` secondes, d'amplitude `amplitude` (1 : le plus fort), posé dans le montage à l'image `debut`.
    fn poser_un_son(&mut self, nom: &str, secondes: f32, amplitude: f32, debut: usize) {
        let octets = wav_1khz(secondes, amplitude);
        let Ok(pcm) = crate::audio::decode(&octets, "wav") else { return };
        let son = self.add_sound(nom.to_string(), "wav".into(), std::sync::Arc::new(octets), std::sync::Arc::new(pcm));
        self.place_sound(son, debut, None);
    }

    /// Un calque d'incrustation nommé `nom`, visible pendant les `n` images.
    fn calque_incrustation(&mut self, nom: &str, inc: crate::incrustation::Incrustation, n: usize) -> Layer {
        let pose = self.new_cel(LayerKind::Bitmap);
        let mut l = Layer::new_effect(nom, crate::calqueffet::CalqueEffet::incrustation(inc, nom));
        l.frames = vec![Some(pose); n];
        l
    }

    /// Pose le plan type `t` sur le plan `index` (lieu et date, générique de début) : des calques de plus, en haut de la pile.
    /// Rend le rang du calque posé ; rien pour un type qui fait un plan neuf.
    pub fn poser_sur_le_plan(&mut self, t: PlanType, index: usize) -> Option<usize> {
        if !t.sur_un_plan() || index >= self.panels.len() {
            return None;
        }
        let n = self.panels[index].frame_count().max(1);
        let fps = self.fps.max(1) as f32;
        let fr = self.frame_rect();
        let fh = fr.height();
        let layer = match t {
            PlanType::LieuEtDate => {
                // Une incrustation (dans le cadre, par-dessus tout) qui s'écrit lettre à lettre : son texte se change dans ses
                // réglages, le lettre à lettre suit.
                let inc = crate::incrustation::Incrustation {
                    source: crate::incrustation::Source::Libre,
                    texte: "Paris, 1962".into(),
                    portee: crate::incrustation::Portee::Plan,
                    ancre: crate::incrustation::Ancre::BasGauche,
                    taille: 4.5,
                    couleur: [255, 255, 255, 255],
                    fond: None,
                    marge: 5.0,
                    quand: crate::incrustation::Quand::Toujours,
                    police: String::new(),
                    lettre_a_lettre: ((fps / 12.0).round() as u32).clamp(1, crate::incrustation::LETTRE_A_LETTRE_MAX),
                };
                let pose = self.new_cel(LayerKind::Bitmap);
                let mut l = Layer::new_effect("Lieu et date", crate::calqueffet::CalqueEffet::incrustation(inc, "Lieu et date"));
                l.frames = vec![Some(pose); n];
                l
            }
            _ => {
                // Trois noms en bas du cadre, l'un après l'autre, en fondu : un dessin par nom.
                let auteur = self.properties.author.trim().to_string();
                let noms = [("Un film de", if auteur.is_empty() { "Prénom Nom".to_string() } else { auteur }), ("Avec", "Prénom Nom".to_string()), ("Musique", "Prénom Nom".to_string())];
                let par = (n / noms.len()).max(1);
                let fondu = ((0.3 * fps).round() as usize).clamp(1, (par / 3).max(1));
                let mut cels = Vec::new();
                for (role, nom) in &noms {
                    let elements = vec![texte(role, 0.035 * fh, GRIS, self.au_cadre(0.5, 0.74), 0.5), texte(nom, 0.06 * fh, BLANC, self.au_cadre(0.5, 0.82), 0.5)];
                    cels.push(self.dessin_vectoriel(elements));
                }
                let mut l = Layer::new("Générique de début", LayerKind::Vector);
                l.frames = (0..n).map(|f| Some(cels[(f / par).min(cels.len() - 1)])).collect();
                let mut cles = Vec::new();
                for k in 0..noms.len() {
                    let (a, b) = (k * par, if k + 1 == noms.len() { n } else { (k + 1) * par });
                    if b <= a + 2 * fondu {
                        continue;
                    }
                    cles.extend([(a, 0.0), (a + fondu, 1.0), (b - 1 - fondu, 1.0), (b - 1, 0.0)]);
                }
                l.set_opacity_keys(&cles);
                l
            }
        };
        let layers = &mut self.panels[index].layers;
        layers.push(layer);
        Some(layers.len() - 1)
    }
}

/// Le nom du calque d'une fin d'acte (une fin d'acte compte celles d'avant par lui).
const NOM_FIN_D_ACTE: &str = "Fin d'acte";

/// Une incrustation blanche, sans boîte, au centre du cadre : `source` (son texte `texte` si elle est libre), lettres de
/// `taille` % de la hauteur du cadre.
fn incrustation_au_centre(source: crate::incrustation::Source, texte: &str, taille: f32) -> crate::incrustation::Incrustation {
    crate::incrustation::Incrustation {
        source,
        texte: texte.into(),
        portee: crate::incrustation::Portee::Plan,
        ancre: crate::incrustation::Ancre::Centre,
        taille,
        couleur: [255, 255, 255, 255],
        fond: None,
        marge: 3.0,
        quand: crate::incrustation::Quand::Toujours,
        police: String::new(),
        lettre_a_lettre: 0,
    }
}

/// Un son WAV (48 kHz, 16 bits, mono) à 1 kHz de `secondes` secondes, d'amplitude `amplitude`.
pub(crate) fn wav_1khz(secondes: f32, amplitude: f32) -> Vec<u8> {
    let taux = 48_000u32;
    let n = ((secondes.max(0.0) * taux as f32).round() as u32).max(1);
    let donnees = n * 2;
    let mut out = Vec::with_capacity(44 + donnees as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + donnees).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&taux.to_le_bytes());
    out.extend_from_slice(&(taux * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&donnees.to_le_bytes());
    let a = amplitude.clamp(0.0, 1.0) * i16::MAX as f32;
    for k in 0..n {
        let v = (a * (std::f32::consts::TAU * 1000.0 * k as f32 / taux as f32).sin()).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Un fondu à l'entrée (`entree` images) et à la sortie (`sortie` images) du calque `l`, sur un plan de `n` images, par ses clés
/// d'opacité (que la timeline montre et retouche).
fn fondu_entree_sortie(l: &mut Layer, n: usize, entree: usize, sortie: usize) {
    let last = n.saturating_sub(1);
    let mut cles = Vec::new();
    if entree > 0 && entree < last {
        cles.extend([(0, 0.0), (entree, 1.0)]);
    }
    if sortie > 0 && last > sortie && last - sortie > entree {
        cles.extend([(last - sortie, 1.0), (last, 0.0)]);
    }
    if !cles.is_empty() {
        l.set_opacity_keys(&cles);
    }
}
