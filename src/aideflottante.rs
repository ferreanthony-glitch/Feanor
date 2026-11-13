//! La fenêtre flottante de l'aide animée (partie 155, Anthony, 2026-11-13 : « Les animations n'apparaissent pas dans réglages mais
//! dans des petites fenêtres flottantes à côté de l'outil ou la fonction concernée »).
//!
//! - **Où** : une petite fenêtre (la vignette 240 × 150 dans un cadre `panel` arrondi en `radius-lg`, sans ombre), au-dessus des
//!   panneaux et des fenêtres (sous les menus et les bulles), posée à côté de ce qui la déclenche : l'icône de l'outil dans la barre
//!   de gauche, l'onglet « Vignettes », l'outil d'annotation pris, l'icône ou le bouton qui ouvre la vue Espace 3D. Chacun retient
//!   son rectangle à l'image où il est peint (`retenir`) ; la fenêtre prend le premier de ses `ancres` qui est à l'écran, sinon la
//!   barre d'outils, sinon un coin, puis suit cette ancre-là ou reste à sa place. Elle reste entièrement dans l'écran (`placer`).
//! - **Quand** : elle joue une fois (une suite, comme celle des Vignettes, l'une après l'autre dans la même fenêtre), puis
//!   s'efface en un fondu court. Un appui dessus la ferme aussitôt ; pendant un geste, le pointeur qui s'en approche la ferme
//!   aussi : elle ne couvre jamais le trait qu'on fait.
//! - **Jamais de clic volé** : elle est peinte sur une couche qui n'a aucune zone de clic (une `egui::Area`
//!   déclare toujours la sienne, qui cache aux outils ce qu'il y a dessous) ; un appui du stylet sur la toile dessine donc
//!   normalement, même sous elle.
//! - **Revoir** : le clic droit sur une icône d'outil, sur l'onglet « Vignettes » ou sur le bouton de l'Espace 3D offre « Revoir
//!   l'explication » (`demander_revoir`, lu à l'image suivante) ; le ▶ du titre de « Réglages » (`aideanim.rs`) ; le registre.
//! - Pendant qu'elle joue, l'écran est repeint ; ensuite, plus rien n'est demandé.

use std::collections::HashMap;

use eframe::egui::{self, pos2, vec2, Id, LayerId, Order, Rect, Stroke, StrokeKind, Vec2};

use crate::aideanim::{anim, du_contexte, peindre, Teintes, HAUTEUR, LARGEUR};
use crate::app::{App, Tool};

/// La marge entre le cadre de la fenêtre et sa vignette.
pub(crate) const MARGE: f32 = 8.0;
/// L'écart entre la fenêtre et ce qui la déclenche, et entre la fenêtre et le bord de l'écran.
pub(crate) const ECART: f32 = 8.0;
pub(crate) const BORD_ECRAN: f32 = 6.0;
/// `radius-lg` de la charte.
const RAYON_LG: f32 = 12.0;
/// L'apparition et le fondu de fin, en secondes ; le temps laissé sur la dernière image d'une animation avant la suivante ou le fondu.
const APPARITION: f64 = 0.15;
const FONDU: f64 = 0.3;
const TENUE: f64 = 0.6;
/// Autour de la fenêtre, la distance où un pointeur appuyé (un geste en cours) la ferme.
const GARDE: f32 = 12.0;

/// Les ancres : la barre d'outils entière, l'onglet « Vignettes », l'outil d'annotation pris, le bouton qui ouvre l'Espace 3D.
/// (Les outils retiennent leur icône sous leur commande, `Tool::action`.)
pub(crate) const BARRE: &str = "barre_outils";
pub(crate) const VIGNETTES: &str = "espace.vignettes";
pub(crate) const ANNOTATION: &str = "annotation";
pub(crate) const ESPACE3D: &str = "espace3d.basculer";

/// La taille de la fenêtre : la vignette et sa marge.
pub(crate) fn taille() -> Vec2 {
    vec2(LARGEUR + 2.0 * MARGE, HAUTEUR + 2.0 * MARGE)
}

/// Une lecture en cours : les animations l'une après l'autre, puis le fondu.
#[derive(Clone, Debug)]
pub(crate) struct Lecture {
    pub ids: Vec<&'static str>,
    pub index: usize,
    /// L'heure d'egui au début de l'animation `index`, et au début de toute la lecture (pour l'apparition).
    pub debut: f64,
    pub ouverture: f64,
    /// Le fondu de fin a commencé à cette heure.
    pub fin: Option<f64>,
    /// Ce à côté de quoi la fenêtre se pose, dans l'ordre de préférence.
    pub ancres: Vec<&'static str>,
    /// Où elle a été peinte à la dernière image, et l'ancre qui l'a posée : elle suit cette ancre tant qu'elle est peinte, et reste
    /// à sa place quand elle disparaît (la vue Espace 3D qui s'ouvre cache son bouton) ; elle ne saute jamais vers une autre.
    pub rect: Option<Rect>,
    pub prise: Option<&'static str>,
}

// ------------------------------------------------------------------------------------------------------ les ancres

/// Les rectangles retenus, avec le numéro de l'image où ils ont été peints.
#[derive(Clone, Default)]
struct Ancres(HashMap<&'static str, (u64, Rect)>);

fn id_ancres() -> Id {
    Id::new("aide_flottante_ancres")
}

fn id_revoir() -> Id {
    Id::new("aide_flottante_revoir")
}

/// `cle` est peinte à l'écran dans `r`, à cette image.
pub(crate) fn retenir(ctx: &egui::Context, cle: &'static str, r: Rect) {
    let passe = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Ancres>(id_ancres()).0.insert(cle, (passe, r));
    });
}

/// Le rectangle de `cle`, s'il a été peint à cette image ou à la précédente.
pub(crate) fn ancre(ctx: &egui::Context, cle: &str) -> Option<Rect> {
    let passe = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<Ancres>(id_ancres())).and_then(|a| a.0.get(cle).copied()).filter(|(p, _)| p + 1 >= passe).map(|(_, r)| r)
}

/// Les ancres d'un contexte de l'aide (`App::aide_contexte`), l'outil en main étant `outil` : ce à côté de quoi sa fenêtre se pose.
pub(crate) fn ancres_du_contexte(contexte: &str, outil: Tool) -> Vec<&'static str> {
    let mut out = match contexte {
        "vignettes" => vec![VIGNETTES],
        "vignettes.annotation" => vec![ANNOTATION, VIGNETTES],
        "vignettes.camera" => vec![Tool::Camera.action(), VIGNETTES],
        "espace3d" => vec![ESPACE3D, outil.action()],
        _ => vec![outil.action()],
    };
    out.push(BARRE);
    out
}

/// Où poser une fenêtre de `taille` à côté de `ancre`, entièrement dans `ecran` : à droite d'une icône (celles de la barre de
/// gauche), sinon à sa gauche, sinon à droite recalée dans l'écran (jamais par-dessus la barre) ; sous un onglet ou un bouton plus
/// large que haut, dessous, sinon dessus, sinon dessous recalée.
pub(crate) fn placer(ancre: Rect, taille: Vec2, ecran: Rect) -> Rect {
    let zone = ecran.shrink(BORD_ECRAN);
    let droite = Rect::from_min_size(pos2(ancre.right() + ECART, ancre.top()), taille);
    let gauche = Rect::from_min_size(pos2(ancre.left() - ECART - taille.x, ancre.top()), taille);
    let dessous = Rect::from_min_size(pos2(ancre.left(), ancre.bottom() + ECART), taille);
    let dessus = Rect::from_min_size(pos2(ancre.left(), ancre.top() - ECART - taille.y), taille);
    let large = ancre.width() > 1.5 * ancre.height();
    let (premiere, seconde) = if large { (dessous, dessus) } else { (droite, gauche) };
    // La première place, recalée : si elle n'a glissé que le long de l'ancre (une icône tout en bas remonte un peu, un onglet à
    // droite de l'écran glisse vers la gauche), elle est encore à côté d'elle.
    let le_long = |avant: Rect, apres: Rect| if large { (apres.top() - avant.top()).abs() < 0.5 } else { (apres.left() - avant.left()).abs() < 0.5 };
    let (r1, r2) = (recaler(premiere, zone), recaler(seconde, zone));
    if le_long(premiere, r1) {
        r1
    } else if le_long(seconde, r2) {
        r2
    } else {
        r1
    }
}

/// `r` déplacé juste assez pour tenir dans `zone` (collé en haut à gauche si elle est trop petite).
fn recaler(r: Rect, zone: Rect) -> Rect {
    let mut d = Vec2::ZERO;
    if r.right() > zone.right() {
        d.x = zone.right() - r.right();
    }
    if r.left() + d.x < zone.left() {
        d.x = zone.left() - r.left();
    }
    if r.bottom() > zone.bottom() {
        d.y = zone.bottom() - r.bottom();
    }
    if r.top() + d.y < zone.top() {
        d.y = zone.top() - r.top();
    }
    r.translate(d)
}

// ------------------------------------------------------------------------------------------------------ revoir

/// Les commandes dont le clic droit offre « Revoir l'explication » : chaque outil, l'onglet « Vignettes », l'Espace 3D.
pub(crate) fn a_une_explication(action: &str) -> bool {
    action == VIGNETTES || action == ESPACE3D || Tool::ALL.iter().any(|t| t.action() == action)
}

/// « Revoir l'explication » choisi dans le menu du clic droit de `action` : la fenêtre s'ouvre à l'image suivante (le menu n'a pas
/// l'application sous la main).
pub(crate) fn demander_revoir(ctx: &egui::Context, action: &str) {
    ctx.data_mut(|d| d.insert_temp(id_revoir(), action.to_string()));
    ctx.request_repaint();
}

impl App {
    /// Rejoue les animations de `action` (un outil, les Vignettes, l'Espace 3D) à côté de son icône. Vrai si quelque chose joue.
    pub(crate) fn aide_revoir(&mut self, action: &str, maintenant: f64) -> bool {
        let (contexte, ancres) = if action == VIGNETTES {
            ("vignettes", ancres_du_contexte("vignettes", self.tool))
        } else if action == ESPACE3D {
            ("espace3d", ancres_du_contexte("espace3d", self.tool))
        } else if let Some(t) = Tool::ALL.into_iter().find(|t| t.action() == action) {
            // (La caméra de la planche a ses animations ; les autres outils, les leurs, même sur la planche.)
            let contexte = if t == Tool::Camera && self.espace == crate::workspace::Espace::Vignettes { "vignettes.camera" } else { self.aide_contexte_outil(t) };
            (contexte, vec![t.action(), BARRE])
        } else {
            return false;
        };
        let ids: Vec<&'static str> = du_contexte(contexte).iter().map(|a| a.id).collect();
        if ids.is_empty() {
            return false;
        }
        self.aide_jouer(ids, ancres, maintenant);
        true
    }

    // -------------------------------------------------------------------------------------------------- la fenêtre

    /// À chaque image, après les barres et les panneaux : suit l'outil en main, lance ce qu'il y a à jouer, et peint la fenêtre
    /// flottante. Rien (et aucun repaint) quand rien ne joue.
    pub(crate) fn aide_flottante_ui(&mut self, ctx: &egui::Context) {
        let maintenant = ctx.input(|i| i.time);
        if let Some(action) = ctx.data_mut(|d| d.remove_temp::<String>(id_revoir())) {
            self.aide_revoir(&action, maintenant);
        }
        // Sans barre d'outils (l'espace Montage, ou la barre cachée), rien ne joue de soi-même : le contexte n'est pas suivi, et
        // l'animation jouera quand l'icône reparaîtra.
        if self.espace != crate::workspace::Espace::Montage && self.layout.tool_bar {
            self.aide_suivre(maintenant);
        }
        let Some(mut l) = self.confort.aide.lecture.clone() else { return };
        let Some(mut a) = l.ids.get(l.index).and_then(|id| anim(id)) else {
            self.confort.aide.lecture = None;
            return;
        };
        // Un appui sur la fenêtre la ferme tout de suite ; un geste dont le pointeur s'en approche aussi (elle ne couvre jamais le
        // trait). L'appui continue son chemin : il dessine, ou clique ce qu'il y a dessous.
        // Un geste commencé avant qu'elle s'ouvre (un outil pris au clavier pendant un trait) ne la ferme pas, pour ne pas perdre
        // l'animation ; elle se cache le temps qu'il passe près d'elle.
        let mut cachee = false;
        if let Some(r) = l.rect {
            let (appui, pres, avant) = ctx.input(|i| {
                let appui = i.pointer.any_pressed() && i.pointer.press_origin().is_some_and(|p| r.contains(p));
                let pres = i.pointer.any_down() && i.pointer.latest_pos().is_some_and(|p| r.expand(GARDE).contains(p));
                let avant = i.pointer.press_start_time().is_some_and(|t| t < l.ouverture);
                (appui, pres, avant)
            });
            if appui || (pres && !avant) {
                self.confort.aide.lecture = None;
                return;
            }
            cachee = pres;
        }
        // Au bout de l'animation (plus la tenue) : la suivante, dans la même fenêtre, ou le fondu.
        if l.fin.is_none() && maintenant - l.debut > a.duree as f64 + TENUE {
            if l.index + 1 < l.ids.len() {
                l.index += 1;
                l.debut = maintenant;
                self.aide_vue(l.ids[l.index]);
                if let Some(b) = anim(l.ids[l.index]) {
                    a = b;
                }
            } else {
                l.fin = Some(maintenant);
            }
        }
        if l.fin.is_some_and(|f| maintenant - f >= FONDU) {
            self.confort.aide.lecture = None;
            return;
        }
        let opacite = match l.fin {
            Some(f) => 1.0_f32 - ((maintenant - f) / FONDU) as f32,
            None => ((maintenant - l.ouverture) / APPARITION) as f32,
        }
        .clamp(0.0, 1.0);
        // La place : à côté de l'ancre qui l'a posée, tant qu'elle est peinte ; sinon là où elle était. À la première image : la
        // première ancre à l'écran (la barre d'outils en dernier), sinon un coin.
        let ecran = ctx.screen_rect();
        let rect = match (l.prise.and_then(|c| ancre(ctx, c)), l.rect) {
            (Some(r), _) => placer(r, taille(), ecran),
            (None, Some(avant)) => recaler(avant, ecran.shrink(BORD_ECRAN)),
            (None, None) => match l.ancres.iter().find_map(|c| ancre(ctx, c).map(|r| (*c, r))) {
                Some((c, r)) => {
                    l.prise = Some(c);
                    placer(r, taille(), ecran)
                }
                None => placer(Rect::from_min_size(ecran.min + vec2(BORD_ECRAN, 80.0), vec2(40.0, 40.0)), taille(), ecran),
            },
        };
        let t = if l.fin.is_some() { a.duree } else { (maintenant - l.debut) as f32 };
        // Sur une couche des fenêtres (`Order::Middle`) : au-dessus des panneaux et des fenêtres, sous les menus, le menu des outils
        // d'une case et les bulles, qui s'ouvrent souvent au même endroit.
        let mut p = ctx.layer_painter(LayerId::new(Order::Middle, Id::new("aide_flottante")));
        p.set_opacity(if cachee { 0.0 } else { opacite });
        let palette = crate::theme::palette();
        p.rect(rect, RAYON_LG, palette.panneau, Stroke::new(1.0_f32, palette.bord), StrokeKind::Inside);
        peindre(&p, rect.shrink(MARGE), a, t, &Teintes::du_theme());
        l.rect = Some(rect);
        self.confort.aide.lecture = Some(l);
        ctx.request_repaint();
    }
}
