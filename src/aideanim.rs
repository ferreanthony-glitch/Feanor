//! L'aide animée (partie 153, Anthony, 2026-11-13) : de petites animations schématiques, peintes en vecteurs par egui, à la place
//! des textes gris qui expliquaient les outils dans les panneaux.
//!
//! - **Une animation** (`Anim`) : un identifiant stable (retenu dans les réglages quand elle a été vue), un titre, la rubrique du
//!   registre, les contextes où elle joue (l'outil en main, l'espace de travail : `App::aide_contexte`), une durée, une fonction de
//!   dessin et le texte du registre (celui qui était dans le panneau, avec les raccourcis lus dans le fichier de raccourcis).
//! - **Le joueur** (`Joueur`, dans `Confort` : partagé par les onglets) : la première prise d'un outil joue ses animations dans une
//!   petite fenêtre flottante posée à côté de son icône (`aideflottante.rs`, partie 155), puis la fenêtre s'efface. Le bouton ▶ du
//!   titre de « Réglages » et le clic droit sur l'icône (« Revoir l'explication ») les rejouent. Aide > « Afficher les explications »
//!   (`Settings::afficher_explications`) : elles rejouent à chaque prise. Pendant qu'elles jouent, l'écran est repeint ; à l'arrêt,
//!   plus rien n'est demandé.
//! - **Le registre** (`aideregistre.rs`) : toutes les animations rangées par rubrique, chacune rejouable avec son texte, et les
//!   textes des Préférences et des fenêtres (`aidetextes.rs`).
//!
//! Le style (validé par Anthony sur six essais) : une vignette 240 × 150, le fond `canvas` dans un cadre `ground`, les traits à
//! l'encre, le secondaire en encre douce et ligne forte, le geste de la main en jade (curseur, choix, clés, poignée tenue). Aucun
//! rouge, aucun dégradé, aucune ombre, aucun mot sauf les touches tenues, dessinées comme des touches de clavier. Le curseur : un
//! cercle de rayon 5 cerné de jade, un point plein au centre quand le bouton est appuyé. De 4 à 6 secondes, des mouvements adoucis.
//!
//! Le catalogue est dans `aidecatalogue.rs` (les outils) et `aidevignettes.rs` (l'espace Vignettes).

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

use crate::app::{App, Tool};
use crate::theme;

/// La taille logique d'une vignette : les animations sont écrites dans ce repère.
pub(crate) const LARGEUR: f32 = 240.0;
pub(crate) const HAUTEUR: f32 = 150.0;

/// Les couleurs d'une vignette, lues dans la charte (`theme.rs`) selon que le thème est sombre ou clair.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Teintes {
    /// Le fond de la vignette (`canvas`) et son cadre (`ground`).
    pub fond: Color32,
    pub cadre: Color32,
    /// Les traits et les dessins.
    pub encre: Color32,
    /// Le secondaire : encre douce, ligne forte.
    pub douce: Color32,
    pub forte: Color32,
    /// Le geste de la main.
    pub jade: Color32,
}

impl Teintes {
    pub(crate) fn pour(claire: bool) -> Teintes {
        let c = |v: theme::Rvb| Color32::from_rgb(v[0], v[1], v[2]);
        if claire {
            Teintes { fond: c(theme::PAPIER_CANEVAS), cadre: c(theme::PAPIER_SOL), encre: c(theme::PAPIER_ENCRE), douce: c(theme::PAPIER_ENCRE_DOUCE), forte: c(theme::PAPIER_LIGNE_FORTE), jade: c(theme::PAPIER_JADE_TEXTE) }
        } else {
            Teintes { fond: c(theme::ATELIER_CANEVAS), cadre: c(theme::ATELIER_SOL), encre: c(theme::ATELIER_ENCRE), douce: c(theme::ATELIER_ENCRE_DOUCE), forte: c(theme::ATELIER_LIGNE_FORTE), jade: c(theme::JADE) }
        }
    }

    /// Celles du thème en cours.
    pub(crate) fn du_theme() -> Teintes {
        Teintes::pour(theme::palette().claire)
    }
}

/// Une fonction de dessin : le peintre, la vignette à l'écran (aux proportions 240 × 150), le temps en secondes, les couleurs.
pub(crate) type Dessin = fn(&Painter, Rect, f32, &Teintes);

/// Une animation du catalogue.
pub(crate) struct Anim {
    /// Identifiant stable, retenu dans les réglages quand l'animation a été vue : on en ajoute, on n'en renomme pas.
    pub id: &'static str,
    pub titre: &'static str,
    /// La rubrique du registre (l'outil ou la fonction).
    pub rubrique: &'static str,
    /// Les contextes où elle joue à la prise (`App::aide_contexte`).
    pub contextes: &'static [&'static str],
    /// Durée en secondes.
    pub duree: f32,
    pub dessin: Dessin,
    /// Le texte du registre. `{outil.selection}` s'y écrit « (S) », le raccourci de la commande lu dans le fichier de raccourcis
    /// (rien si elle n'en a pas).
    pub texte: &'static str,
}

/// Toutes les animations, dans l'ordre du registre.
pub(crate) fn toutes() -> impl Iterator<Item = &'static Anim> {
    crate::aidecatalogue::OUTILS.iter().chain(crate::aidegestes::GESTES.iter()).chain(crate::aidevignettes::VIGNETTES.iter())
}

pub(crate) fn anim(id: &str) -> Option<&'static Anim> {
    toutes().find(|a| a.id == id)
}

/// Les animations d'un contexte, dans l'ordre du catalogue.
pub(crate) fn du_contexte(contexte: &str) -> Vec<&'static Anim> {
    toutes().filter(|a| a.contextes.contains(&contexte)).collect()
}

/// Le texte d'une animation, ses `{commande}` remplacés par ` (touche)` (`raccourci` rend la touche, vide sans touche).
pub(crate) fn avec_touches(texte: &str, raccourci: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(texte.len());
    let mut reste = texte;
    while let Some(i) = reste.find('{') {
        out.push_str(&reste[..i]);
        let suite = &reste[i + 1..];
        match suite.find('}') {
            // (Un identifiant de commande a toujours un point : `{plan}`, un champ d'un modèle de nom, reste tel quel.)
            Some(j) if j > 0 && suite[..j].contains('.') && suite[..j].chars().all(|c| c.is_ascii_lowercase() || c == '.' || c == '_' || c.is_ascii_digit()) => {
                out.push_str(&crate::panels::between_parens(&raccourci(&suite[..j])));
                reste = &suite[j + 1..];
            }
            _ => {
                out.push('{');
                reste = suite;
            }
        }
    }
    out.push_str(reste);
    out
}

// ------------------------------------------------------------------------------------------------------ le temps

/// 0 avant `a`, 1 après `b`, linéaire entre les deux.
pub(crate) fn phase(t: f32, a: f32, b: f32) -> f32 {
    if b <= a {
        return if t >= b { 1.0 } else { 0.0 };
    }
    ((t - a) / (b - a)).clamp(0.0, 1.0)
}

/// Adoucissement à l'entrée et à la sortie (ease in-out).
pub(crate) fn adoucir(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) / 2.0 }
}

/// `phase` adoucie.
pub(crate) fn doux(t: f32, a: f32, b: f32) -> f32 {
    adoucir(phase(t, a, b))
}

pub(crate) fn mix(a: f32, b: f32, s: f32) -> f32 {
    a + (b - a) * s
}

pub(crate) fn mixp(a: Pos2, b: Pos2, s: f32) -> Pos2 {
    pos2(mix(a.x, b.x, s), mix(a.y, b.y, s))
}

/// Une couleur qui apparaît ou s'efface (`s` de 0 à 1).
pub(crate) fn fondu(c: Color32, s: f32) -> Color32 {
    c.gamma_multiply(s.clamp(0.0, 1.0))
}

// ------------------------------------------------------------------------------------------------------ les chemins

/// Un chemin en points du repère 240 × 150, mesuré : de quoi en tracer une partie, ou y placer un point à une fraction de sa longueur.
#[derive(Clone, Debug)]
pub(crate) struct Chemin {
    pub pts: Vec<Pos2>,
    cumul: Vec<f32>,
}

impl Chemin {
    pub(crate) fn de(pts: Vec<Pos2>) -> Chemin {
        let mut cumul = Vec::with_capacity(pts.len());
        let mut l = 0.0;
        for (i, p) in pts.iter().enumerate() {
            if i > 0 {
                l += pts[i - 1].distance(*p);
            }
            cumul.push(l);
        }
        Chemin { pts, cumul }
    }

    /// Une courbe de Bézier cubique (comme `M a C b c d` en SVG).
    pub(crate) fn bezier(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> Chemin {
        let n = 48;
        let pts = (0..=n)
            .map(|i| {
                let t = i as f32 / n as f32;
                let u = 1.0 - t;
                let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                pos2(w0 * a.0 + w1 * b.0 + w2 * c.0 + w3 * d.0, w0 * a.1 + w1 * b.1 + w2 * c.1 + w3 * d.1)
            })
            .collect();
        Chemin::de(pts)
    }

    pub(crate) fn droite(a: (f32, f32), b: (f32, f32)) -> Chemin {
        Chemin::de(vec![pos2(a.0, a.1), pos2(b.0, b.1)])
    }

    pub(crate) fn longueur(&self) -> f32 {
        self.cumul.last().copied().unwrap_or(0.0)
    }

    /// Le point à la fraction `s` (0 à 1) de la longueur, et la direction du chemin là.
    pub(crate) fn point(&self, s: f32) -> (Pos2, egui::Vec2) {
        if self.pts.len() < 2 {
            return (self.pts.first().copied().unwrap_or(Pos2::ZERO), vec2(1.0, 0.0));
        }
        let cible = s.clamp(0.0, 1.0) * self.longueur();
        let i = self.cumul.partition_point(|&c| c < cible).clamp(1, self.pts.len() - 1);
        let (a, b) = (self.pts[i - 1], self.pts[i]);
        let seg = (self.cumul[i] - self.cumul[i - 1]).max(1e-6);
        let k = ((cible - self.cumul[i - 1]) / seg).clamp(0.0, 1.0);
        (mixp(a, b, k), (b - a).normalized())
    }

    /// Les points du début jusqu'à la fraction `s`.
    pub(crate) fn jusqu_a(&self, s: f32) -> Vec<Pos2> {
        if s <= 0.0 || self.pts.len() < 2 {
            return Vec::new();
        }
        let cible = s.clamp(0.0, 1.0) * self.longueur();
        let mut out: Vec<Pos2> = self.pts.iter().zip(&self.cumul).take_while(|(_, c)| **c < cible).map(|(p, _)| *p).collect();
        out.push(self.point(s).0);
        out
    }

    /// Les points de la fraction `a` à la fraction `b`.
    pub(crate) fn entre(&self, a: f32, b: f32) -> Vec<Pos2> {
        if b <= a || self.pts.len() < 2 {
            return Vec::new();
        }
        let (la, lb) = (a.clamp(0.0, 1.0) * self.longueur(), b.clamp(0.0, 1.0) * self.longueur());
        let mut out = vec![self.point(a).0];
        out.extend(self.pts.iter().zip(&self.cumul).filter(|(_, c)| **c > la && **c < lb).map(|(p, _)| *p));
        out.push(self.point(b).0);
        out
    }
}

/// Le temps égal entre deux marques : à la fraction de temps `u` (0 à 1), la fraction de longueur où l'on est, quand les marques
/// (fractions de longueur, croissantes) partagent le temps en parts égales.
pub(crate) fn par_les_marques(u: f32, marques: &[f32]) -> f32 {
    let mut jalons = vec![0.0];
    jalons.extend_from_slice(marques);
    jalons.push(1.0);
    let n = jalons.len() - 1;
    let x = u.clamp(0.0, 1.0) * n as f32;
    let i = (x.floor() as usize).min(n - 1);
    mix(jalons[i], jalons[i + 1], x - i as f32)
}

// ------------------------------------------------------------------------------------------------------ la toile

/// Le repère d'une vignette à l'écran : les animations écrivent en 240 × 150, la toile met à l'échelle.
pub(crate) struct Toile<'a> {
    pub p: &'a Painter,
    pub r: Rect,
    pub k: f32,
    pub t: Teintes,
}

impl<'a> Toile<'a> {
    pub(crate) fn new(p: &'a Painter, r: Rect, t: &Teintes) -> Toile<'a> {
        Toile { p, r, k: r.width() / LARGEUR, t: *t }
    }

    pub(crate) fn pt(&self, x: f32, y: f32) -> Pos2 {
        self.r.min + vec2(x, y) * self.k
    }

    pub(crate) fn ecran(&self, p: Pos2) -> Pos2 {
        self.pt(p.x, p.y)
    }

    fn trait_(&self, w: f32, c: Color32) -> Stroke {
        Stroke::new(w * self.k, c)
    }

    /// Le fond de la vignette.
    pub(crate) fn fond(&self) {
        self.p.rect_filled(self.r, 4.0 * self.k, self.t.fond);
    }

    pub(crate) fn ligne(&self, a: Pos2, b: Pos2, w: f32, c: Color32) {
        self.p.line_segment([self.ecran(a), self.ecran(b)], self.trait_(w, c));
    }

    pub(crate) fn polyligne(&self, pts: &[Pos2], w: f32, c: Color32) {
        if pts.len() >= 2 {
            self.p.add(Shape::line(pts.iter().map(|p| self.ecran(*p)).collect(), self.trait_(w, c)));
        }
    }

    pub(crate) fn ferme(&self, pts: &[Pos2], w: f32, c: Color32) {
        if pts.len() >= 3 {
            self.p.add(Shape::closed_line(pts.iter().map(|p| self.ecran(*p)).collect(), self.trait_(w, c)));
        }
    }

    pub(crate) fn cadre_pointille(&self, r: Rect, w: f32, c: Color32) {
        self.pointilles(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], w, c);
    }

    /// Une fine flèche de `a` à `b`.
    pub(crate) fn fleche(&self, a: Pos2, b: Pos2, w: f32, col: Color32) {
        self.ligne(a, b, w, col);
        let d = (b - a).normalized();
        let n = vec2(-d.y, d.x);
        self.polyligne(&[b - d * 5.0 + n * 3.0, b, b - d * 5.0 - n * 3.0], w, col);
    }

    pub(crate) fn plein(&self, pts: &[Pos2], c: Color32) {
        if pts.len() >= 3 {
            self.p.add(Shape::convex_polygon(pts.iter().map(|p| self.ecran(*p)).collect(), c, Stroke::NONE));
        }
    }

    pub(crate) fn pointilles(&self, pts: &[Pos2], w: f32, c: Color32) {
        let pts: Vec<Pos2> = pts.iter().map(|p| self.ecran(*p)).collect();
        self.p.extend(Shape::dashed_line(&pts, self.trait_(w, c), 4.0 * self.k, 3.0 * self.k));
    }

    pub(crate) fn cercle(&self, c: Pos2, r: f32, w: f32, col: Color32) {
        self.p.circle_stroke(self.ecran(c), r * self.k, self.trait_(w, col));
    }

    pub(crate) fn disque(&self, c: Pos2, r: f32, col: Color32) {
        self.p.circle_filled(self.ecran(c), r * self.k, col);
    }

    pub(crate) fn cadre(&self, r: Rect, w: f32, c: Color32) {
        self.p.rect_stroke(Rect::from_min_max(self.ecran(r.min), self.ecran(r.max)), 1.0 * self.k, self.trait_(w, c), StrokeKind::Middle);
    }

    pub(crate) fn pave(&self, r: Rect, c: Color32) {
        self.p.rect_filled(Rect::from_min_max(self.ecran(r.min), self.ecran(r.max)), 1.0 * self.k, c);
    }

    /// Le curseur : un cercle de rayon 5 cerné de jade ; un point plein au centre quand le bouton est appuyé.
    pub(crate) fn curseur(&self, c: Pos2, appuye: bool) {
        self.disque(c, 5.0, fondu(self.t.fond, 0.6));
        self.cercle(c, 5.0, 1.6, self.t.jade);
        if appuye {
            self.disque(c, 2.2, self.t.jade);
        }
    }

    /// Une touche de clavier à tenir : rectangle arrondi, texte en Plex Mono 11 ; tenue, elle s'allume en jade. `centre` : son centre.
    pub(crate) fn touche(&self, centre: Pos2, texte: &str, tenue: bool) {
        let police = FontId::monospace(11.0 * self.k);
        let galet = self.p.layout_no_wrap(texte.to_string(), police.clone(), self.t.encre);
        let taille = vec2(galet.size().x + 10.0 * self.k, 17.0 * self.k);
        let r = Rect::from_center_size(self.ecran(centre), taille);
        let (fond, encre, bord) = couleurs_touche(&self.t, tenue);
        self.p.rect_filled(r, 3.0 * self.k, fond);
        self.p.rect_stroke(r, 3.0 * self.k, Stroke::new(1.0 * self.k, bord), StrokeKind::Inside);
        self.p.text(r.center(), Align2::CENTER_CENTER, texte, police, encre);
    }

    /// Une poignée carrée (boîte, cadre).
    pub(crate) fn poignee(&self, c: Pos2, tenue: bool) {
        let r = Rect::from_center_size(self.ecran(c), vec2(5.0, 5.0) * self.k);
        self.p.rect_filled(r, 0.0, if tenue { self.t.jade } else { self.t.fond });
        self.p.rect_stroke(r, 0.0, Stroke::new(1.2 * self.k, self.t.jade), StrokeKind::Middle);
    }

    /// Un losange de clé.
    pub(crate) fn losange(&self, c: Pos2, r: f32, col: Color32) {
        let pts = [pos2(c.x, c.y - r), pos2(c.x + r, c.y), pos2(c.x, c.y + r), pos2(c.x - r, c.y)];
        self.plein(&pts, col);
    }

    /// Une petite carte (un calque à plat, ou debout dans l'espace) : un rectangle à l'encre, un trait dedans ; `choisie` la cerne de jade.
    pub(crate) fn carte(&self, c: Pos2, w: f32, h: f32, choisie: bool) {
        let r = Rect::from_center_size(c, vec2(w, h));
        self.pave(r, self.t.cadre);
        self.cadre(r, 1.2, self.t.encre);
        self.polyligne(&[pos2(c.x - w * 0.25, c.y + h * 0.2), pos2(c.x, c.y - h * 0.25), pos2(c.x + w * 0.25, c.y + h * 0.2)], 1.0, self.t.encre);
        if choisie {
            self.cadre(r.expand(2.5), 1.4, self.t.jade);
        }
    }

    /// Un personnage en traits, debout, la tête en haut ; `h` : sa hauteur ; `pas` : l'écart des jambes (de -1 à 1).
    pub(crate) fn bonhomme(&self, pied: Pos2, h: f32, pas: f32, col: Color32) {
        let tete = pos2(pied.x, pied.y - h * 0.82);
        let cou = pos2(pied.x, pied.y - h * 0.66);
        let bassin = pos2(pied.x, pied.y - h * 0.32);
        self.cercle(tete, h * 0.14, 1.3, col);
        self.ligne(cou, bassin, 1.3, col);
        self.ligne(pos2(cou.x, cou.y + h * 0.05), pos2(cou.x - h * 0.18, cou.y + h * 0.26), 1.3, col);
        self.ligne(pos2(cou.x, cou.y + h * 0.05), pos2(cou.x + h * 0.18, cou.y + h * 0.24), 1.3, col);
        let ecart = h * (0.1 + 0.08 * pas);
        self.ligne(bassin, pos2(pied.x - ecart, pied.y), 1.3, col);
        self.ligne(bassin, pos2(pied.x + ecart, pied.y), 1.3, col);
    }

    /// Un arbre simple : un tronc et une couronne.
    pub(crate) fn arbre(&self, pied: Pos2, h: f32, col: Color32) {
        self.ligne(pied, pos2(pied.x, pied.y - h * 0.5), 1.3, col);
        self.cercle(pos2(pied.x, pied.y - h * 0.72), h * 0.26, 1.3, col);
    }

    /// Une petite flèche courbe autour de `c` (la rotation), d'un quart de tour environ.
    pub(crate) fn fleche_courbe(&self, c: Pos2, r: f32, col: Color32) {
        let pts: Vec<Pos2> = (0..=12).map(|i| {
            let a = -0.2 + 1.5 * i as f32 / 12.0;
            pos2(c.x + r * a.cos(), c.y - r * a.sin())
        }).collect();
        self.polyligne(&pts, 1.2, col);
        let bout = *pts.last().unwrap_or(&c);
        self.polyligne(&[pos2(bout.x - 3.0, bout.y - 2.0), bout, pos2(bout.x + 2.5, bout.y - 3.0)], 1.2, col);
    }
}

/// Le fond, l'encre et le bord d'une touche. Tenue : le jade de la marque et l'encre posée sur lui, dans les deux thèmes (le jade
/// « texte » du thème clair, plus sombre, ne laissait que 2,4:1 à l'encre sombre ; le jade de la marque en laisse plus de 4,5:1).
pub(crate) fn couleurs_touche(t: &Teintes, tenue: bool) -> (Color32, Color32, Color32) {
    let c = |v: theme::Rvb| Color32::from_rgb(v[0], v[1], v[2]);
    if tenue {
        (c(theme::JADE), c(theme::SUR_JADE), c(theme::JADE))
    } else {
        (t.cadre, t.douce, t.forte)
    }
}

/// Peint une animation dans `rect` (aux proportions 240 × 150), au temps `t`, sur son fond, sans rien déborder.
pub(crate) fn peindre(painter: &Painter, rect: Rect, a: &Anim, t: f32, teintes: &Teintes) {
    let p = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    p.rect_filled(rect.expand(3.0), 6.0, teintes.cadre);
    (a.dessin)(&p, rect, t.clamp(0.0, a.duree), teintes);
}

// ------------------------------------------------------------------------------------------------------ le joueur

/// L'état de l'aide animée, partagé par les onglets (dans `Confort`).
#[derive(Default)]
pub(crate) struct Joueur {
    /// Les animations jouent d'elles-mêmes à la prise d'un outil (vrai dans l'application lancée par l'utilisateur ; faux pour les essais,
    /// qui ne veulent rien qui bouge sauf s'ils le demandent).
    pub auto: bool,
    /// Le contexte de la dernière image où l'aide a été suivie, et l'outil qui était en main : avec « Afficher les
    /// explications », seule une prise d'outil (un autre `App::tool`) rejoue ce qui a déjà été vu.
    contexte: Option<&'static str>,
    outil: Option<Tool>,
    /// La lecture en cours dans la fenêtre flottante (`aideflottante.rs`).
    pub(crate) lecture: Option<crate::aideflottante::Lecture>,
    /// La fenêtre « Registre des explications » est ouverte, et l'animation qui y joue (avec l'heure de son début).
    pub registre: bool,
    pub registre_joue: Option<(&'static str, f64)>,
}

impl Joueur {
    pub(crate) fn new(auto: bool) -> Joueur {
        Joueur { auto, ..Default::default() }
    }

    /// Une animation joue (ou s'efface) dans la fenêtre flottante.
    #[cfg(test)]
    pub(crate) fn joue(&self) -> bool {
        self.lecture.is_some()
    }

    /// L'animation qui joue dans la fenêtre flottante.
    pub(crate) fn en_cours(&self) -> Option<&'static str> {
        self.lecture.as_ref().and_then(|l| l.ids.get(l.index).copied())
    }

    /// Où la fenêtre flottante a été peinte à la dernière image (rien si aucune animation ne joue).
    #[cfg(test)]
    pub(crate) fn fenetre(&self) -> Option<Rect> {
        self.lecture.as_ref().and_then(|l| l.rect)
    }
}

impl App {
    /// Le contexte de l'aide animée : l'outil en main, précisé par ce qu'il vise (la vue Espace 3D, un calque bitmap, plusieurs
    /// calques), ou l'espace Vignettes.
    pub(crate) fn aide_contexte(&self) -> &'static str {
        if self.espace == crate::workspace::Espace::Vignettes {
            if self.annot_actif() {
                return "vignettes.annotation";
            }
            if self.tool == Tool::Camera {
                return "vignettes.camera";
            }
            return "vignettes";
        }
        // La vue Espace 3D ouverte : ses gestes (la vue, les cartes, la caméra libre), sauf pour la Trajectoire, qui y a les siens.
        if self.e3_vue_montree() && self.tool != Tool::Trajectoire {
            return "espace3d";
        }
        self.aide_contexte_outil(self.tool)
    }

    /// Le contexte d'un outil, précisé par ce qu'il vise (la vue Espace 3D pour la Trajectoire, un calque bitmap, plusieurs
    /// calques) : celui de l'outil en main hors des Vignettes, ou celui d'une icône qu'on veut revoir (`aideflottante.rs`).
    pub(crate) fn aide_contexte_outil(&self, tool: Tool) -> &'static str {
        use crate::document::LayerKind;
        let bitmap = self.layers().get(self.cur_layer).is_some_and(|l| l.kind != LayerKind::Vector);
        match tool {
            Tool::Trajectoire if self.e3_vue_montree() => "trajectoire.espace",
            Tool::Trajectoire => "trajectoire",
            // (Plusieurs calques choisis, ou une sélection qui les couvre ; pas l'aplat visé ni les clés choisies d'un seul calque.)
            Tool::Select if self.chosen_layers().len() >= 2 || self.multi.sel.is_some() => "selection.multi",
            Tool::Select if bitmap => "selection.bitmap",
            Tool::Select => "selection",
            Tool::SelectColor => "selection.couleur",
            Tool::Brush => "pinceau",
            Tool::Pencil => "crayon",
            Tool::Mixer => "melangeur",
            Tool::Eraser if bitmap => "gomme.bitmap",
            Tool::Eraser => "gomme",
            Tool::Cutter => "cutter",
            Tool::Fill => "remplissage",
            Tool::Gradient => "degrade",
            Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline => "formes",
            Tool::Text => "texte",
            Tool::Perspective => "deformer",
            Tool::Grab | Tool::Pull => "sculpter",
            Tool::Picker => "pipette",
            Tool::Pan => "main",
            Tool::Camera => "camera",
            Tool::Transform => "transformer",
        }
    }

    /// Une animation commence vraiment : elle est retenue comme vue (pas avant : une suite arrêtée, un outil changé ou FEANOR quitté
    /// pendant la première laisse les suivantes à voir).
    pub(crate) fn aide_vue(&mut self, id: &'static str) {
        if !self.settings.explications_vues.iter().any(|v| v == id) {
            self.settings.explications_vues.push(id.to_string());
            self.settings_a_ecrire = true;
        }
    }

    /// Lance les animations `ids` dans la fenêtre flottante, l'une après l'autre, depuis le début ; la fenêtre se pose à côté de la
    /// première des `ancres` peintes à l'écran (`aideflottante.rs`).
    pub(crate) fn aide_jouer(&mut self, ids: Vec<&'static str>, ancres: Vec<&'static str>, maintenant: f64) {
        let Some(&premiere) = ids.first() else { return };
        self.aide_vue(premiere);
        // (Une fenêtre déjà ouverte ne réapparaît pas en fondu : elle passe à la nouvelle animation.)
        let ouverture = match &self.confort.aide.lecture {
            Some(l) if l.fin.is_none() => l.ouverture,
            _ => maintenant,
        };
        self.confort.aide.lecture = Some(crate::aideflottante::Lecture { ids, index: 0, debut: maintenant, ouverture, fin: None, ancres, rect: None, prise: None });
    }

    /// Un contexte qui change : ses animations jouent si elles n'ont jamais été vues ; avec Aide > « Afficher les explications », toutes,
    /// mais seulement à la prise d'un autre outil (pas quand le même outil passe d'un calque vectoriel à un calque bitmap, par exemple).
    /// Une animation d'un autre outil qui jouait s'arrête.
    pub(crate) fn aide_suivre(&mut self, maintenant: f64) {
        let contexte = self.aide_contexte();
        if self.confort.aide.contexte == Some(contexte) {
            return;
        }
        self.confort.aide.contexte = Some(contexte);
        let prise = self.confort.aide.outil.replace(self.tool) != Some(self.tool);
        let toutes = self.settings.afficher_explications && prise;
        let ids: Vec<&'static str> = du_contexte(contexte).into_iter().filter(|a| toutes || !self.settings.explications_vues.iter().any(|v| v == a.id)).map(|a| a.id).collect();
        if self.confort.aide.auto && !ids.is_empty() {
            let ancres = crate::aideflottante::ancres_du_contexte(contexte, self.tool);
            self.aide_jouer(ids, ancres, maintenant);
        } else if self.confort.aide.en_cours().is_some_and(|id| anim(id).is_some_and(|a| !a.contextes.contains(&contexte))) {
            self.confort.aide.lecture = None;
        }
    }

    /// Le bouton ▶ du titre de « Réglages » : il rejoue les animations de l'outil en main, dans la fenêtre flottante posée à côté de
    /// son icône. Rien si l'outil n'en a pas.
    pub(crate) fn aide_bouton_ui(&mut self, ui: &mut egui::Ui) {
        let contexte = self.aide_contexte();
        let anims = du_contexte(contexte);
        if anims.is_empty() {
            return;
        }
        let titres: Vec<&str> = anims.iter().map(|a| a.titre).collect();
        let resp = ui.add(egui::Button::new(egui::RichText::new(egui_phosphor::regular::PLAY).small()).frame(false).min_size(vec2(18.0, 18.0)));
        let description = format!("Rejoue l'animation qui montre l'outil, à côté de son icône : {}. Aussi au clic droit sur l'icône ; toutes les explications sont dans Aide > Registre des explications.", titres.join(" ; "));
        if crate::bulles::bulle(resp, "Revoir l'explication", &description, "").clicked() {
            let maintenant = ui.input(|i| i.time);
            self.confort.aide.contexte = Some(contexte);
            let ancres = crate::aideflottante::ancres_du_contexte(contexte, self.tool);
            self.aide_jouer(anims.iter().map(|a| a.id).collect(), ancres, maintenant);
        }
    }

    /// Parmi les calques choisis (deux au moins), ceux qu'un geste de plusieurs calques laisse de côté : cachés ou verrouillés.
    pub(crate) fn calques_laisses_de_cote(&self) -> usize {
        let choisis = self.chosen_layers();
        if choisis.len() < 2 {
            return 0;
        }
        let calques = self.layers();
        choisis.iter().filter(|&&li| calques.get(li).is_some_and(|l| !l.visible || l.locked || l.position_locked || crate::folders::locked_by_folder(calques, li))).count()
    }

    /// L'avertissement de Transformer, de la sélection et de la déformation de plusieurs calques, seulement quand il y a lieu.
    pub(crate) fn avertir_laisses_de_cote(&self, ui: &mut egui::Ui) {
        let n = self.calques_laisses_de_cote();
        if n > 0 {
            let s = if n > 1 { "s" } else { "" };
            theme::avertir_petit(ui, format!("{n} calque{s} caché{s} ou verrouillé{s} : laissé{s} de côté."));
        }
    }

    /// Aide > « Afficher les explications » : cochée, les animations rejouent à chaque prise de l'outil.
    pub(crate) fn aide_basculer_explications(&mut self) {
        self.settings.afficher_explications = !self.settings.afficher_explications;
        self.settings_a_ecrire = true;
        self.status = if self.settings.afficher_explications {
            "Explications : elles rejouent à chaque prise d'un outil.".into()
        } else {
            "Explications : une seule fois par outil. Clic droit sur un outil : Revoir l'explication.".into()
        };
    }

    /// Les deux entrées du menu Aide : la case « Afficher les explications » et le registre.
    pub(crate) fn aide_menu_ui(&mut self, ui: &mut egui::Ui) {
        let mut on = self.settings.afficher_explications;
        let resp = ui.checkbox(&mut on, "Afficher les explications");
        let resp = crate::bulles::bulle(resp, "Afficher les explications", "Cochée : l'animation d'un outil rejoue à côté de son icône chaque fois qu'on le prend. Décochée : une seule fois, la première ; le clic droit sur l'icône (Revoir l'explication) ou le ▶ des Réglages la rejoue.", &self.raccourci("aide.explications"));
        if resp.clicked() {
            ui.close_menu();
            self.usage.source = Some(crate::usage::Source::Menu);
            self.run_action("aide.explications");
        } else if resp.secondary_clicked() {
            ui.close_menu();
            self.start_assign("aide.explications");
        }
        self.action_item(ui, "Registre des explications…", "aide.registre");
    }

    /// Les commandes de l'aide animée. Vrai si `action` en était une.
    pub(crate) fn aide_action(&mut self, action: &str) -> bool {
        match action {
            "aide.explications" => self.aide_basculer_explications(),
            "aide.registre" => self.confort.aide.registre = !self.confort.aide.registre,
            _ => return false,
        }
        true
    }
}
