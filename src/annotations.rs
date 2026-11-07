//! Annotations de la vue Vignettes (Anthony, 2026-10-21) : un stylo (rouge d'origine) et sa gomme, pour
//! montrer, corriger, entourer, flécher sur la planche — **sans aucun effet sur le storyboard**.
//!
//! # Ce que c'est
//!
//! - Un trait d'annotation passe **par-dessus tout** ce que montre la planche (cases, barres, bandes de note,
//!   marges, fond) et peut déborder n'importe où. Il n'est ni dans un plan, ni dans le film, ni dans les
//!   vignettes, ni dans les exports vidéo ou images, ni au montage : les exports ne le lisent jamais (le cliché
//!   du projet, `exportrender.rs`, ne le copie pas). Seul l'export de la planche peut l'inclure, si on le coche.
//! - Chaque trait est **attaché à la case la plus proche** de son centre. Ses points sont gardés *relativement à
//!   cette case* (en largeurs de case, rayon compris), pas en points de la planche : quand les colonnes changent,
//!   que les cases se déplacent ou grossissent, le trait les suit (`Annotations::points_planche`).
//!   La case est désignée par la vignette (le numéro du plan, ou de la marque d'image) et, pour une case vide après
//!   la dernière vignette, par le nombre de cases qui la séparent de cette vignette (`Ancre::apres`) : quand une
//!   case vide devient une vignette, le trait reste dans la même case.
//! - **Si la case disparaît** (plan supprimé, plans réunis, marque retirée) : le trait reste **attaché à sa voisine
//!   la plus proche** (parmi les vignettes qui restent), à la même place sur la planche à l'instant de la
//!   disparition (`rattacher_manquantes`). Il n'est jamais perdu : seule la gomme ou « Effacer les annotations »
//!   l'enlève. Si l'ancienne mise en page manque (projet qui s'ouvre sur un fichier d'une autre séance), il se
//!   rattache à la vignette de même rang que celle d'avant, à son décalage d'origine.
//! - La **gomme d'annotation** n'enlève que des annotations (le trait entier que touche le cercle) ; elle ne touche
//!   ni aux dessins, ni aux notes de la planche.
//! - **Fichier** : `annotations.json`, à côté du manifeste, sans changer `VERSION` du projet (même principe que
//!   `personnages.json` : absent, il n'y a pas d'annotation ; illisible ou d'une version plus récente, il
//!   n'empêche pas d'ouvrir le projet et n'est jamais effacé, `project::write_sidecar`). Un projet partagé l'écrit
//!   avec la structure : les équipiers voient les mêmes annotations. Les annotations sont dans `Structure` :
//!   l'annulation les rétablit avec le reste.
//! - Afficher / cacher et « Effacer les annotations » : `annotationsui.rs`. Cacher n'est qu'un affichage (rien
//!   n'est effacé).
//!
//! # Les calques d'annotation (Anthony, 2026-10-24)
//!
//! - Les traits sont rangés en **calques d'annotation** (`Calque` : un nom, un œil, un rang). Il peut y en avoir plusieurs ;
//!   le dernier de la liste est au-dessus. Un trait appartient à un calque (`Trace::calque`) ; la liste des traits reste à plat
//!   (`Annotations::traces`), le calque dit seulement dans quel ordre et si on les voit (`ordre_de_peinture`).
//! - Ce sont des calques **de la planche** : leurs traits sont attachés aux cases de la planche, comme ci-dessus. Dans la vue
//!   Dessin d'un plan, un calque d'annotation est autre chose, un vrai calque du plan (`Layer::annotation`, `calqueannot.rs`) :
//!   deux endroits, un seul mot, la même couleur et la même règle (jamais dans un export, sauf si on le demande).
//! - **Le lasso** choisit des traits d'un calque (`dans_le_lasso`, `sous_le_point`) ; `transformer` les déplace, les met à
//!   l'échelle, les tourne ou les incline d'un geste, puis les rattache à la case la plus proche de leur nouveau centre : un
//!   trait déplacé d'une case à l'autre suit sa nouvelle case.
//! - **Le fichier** passe en version 2 (`calques`, chacun avec ses traits) ; la version 1 (`traces` à plat, d'avant les calques)
//!   se lit comme **un seul calque**, « Annotations », avec les mêmes traits dans le même ordre. L'enregistrement écrit la
//!   version 2. Un projet sans annotation ni calque particulier n'écrit toujours aucun fichier.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use eframe::egui::{pos2, Pos2, Rect};
use serde::{Deserialize, Serialize};

use crate::board::{self, BoardLayout};
use crate::brush::BrushSettings;
use crate::brushtex::TextureKind;
use crate::document::Document;
use crate::horschamptex::{Brosse, Recette};
use crate::vector::{Affine, VPoint};

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "annotations.json";
const FORMAT: &str = "shot-annotations";
/// 1 : une liste de traits à plat ; 2 : des calques (`calques`), chacun avec ses traits. La lecture accepte les deux.
const VERSION: u32 = 2;
/// Bornes d'un fichier abîmé : au-delà, c'est coupé.
pub const MAX_TRACES: usize = 5000;
pub const MAX_POINTS: usize = 3000;
pub const MAX_CALQUES: usize = 64;
/// Les séquences de l'affichage de la planche : au-delà, c'est coupé (un fichier abîmé).
pub const MAX_SEQUENCES: usize = 5000;
/// Nom du premier calque d'annotation (et de celui que donne la lecture d'un ancien fichier).
pub const NOM_PAR_DEFAUT: &str = "Annotations";
/// Nom du calque du dessin hors des cases.
pub const NOM_HORS_CADRES: &str = "Hors cadres";
/// Rouge d'origine du stylo, et sa taille (points de la planche, pour une case de largeur normale).
pub const COULEUR_PAR_DEFAUT: [u8; 3] = [220, 38, 38];
pub const TAILLE_PAR_DEFAUT: f32 = 5.0;
pub const TAILLE_MIN: f32 = 1.0;
pub const TAILLE_MAX: f32 = 60.0;

/// La case à laquelle un trait est attaché.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ancre {
    /// La vignette (le numéro du plan, ou de la marque d'image).
    #[serde(rename = "vignette")]
    pub card: u64,
    /// Nombre de cases entre cette vignette et la case visée : 0 pour la vignette elle-même, 1 pour la case qui la
    /// suit (une case vide, si c'est la dernière vignette)…
    #[serde(rename = "apres", default)]
    pub after: u32,
    /// Le rang de la case sur la planche au moment de l'attache (secours quand la vignette disparaît sans que
    /// l'ancienne mise en page soit connue).
    #[serde(rename = "rang", default)]
    pub rank: u32,
}

static PROCHAIN_TRAIT: AtomicU64 = AtomicU64::new(1);

/// Un trait d'annotation : son calque, sa case, sa couleur, ses points relatifs à cette case.
#[derive(Clone)]
pub struct Trace {
    /// Identifiant de la session (le lasso garde ses traits par ce numéro : il ne se trompe pas de trait quand la liste change).
    /// Il n'est pas enregistré, et ne compte ni dans la comparaison ni dans l'affichage de débogage.
    pub id: u64,
    /// Le calque d'annotation (`Calque::id`) qui porte ce trait.
    pub calque: u64,
    pub anchor: Ancre,
    pub color: [u8; 3],
    /// Opacité (255 : plein). Un trait d'annotation est plein ; un trait du dessin hors des cases garde l'opacité du pinceau.
    pub alpha: u8,
    /// Points relatifs au coin haut gauche de l'image de la case, en largeurs d'image (rayon compris).
    pub points: Vec<VPoint>,
    /// La recette du pinceau d'un trait du dessin hors des cases (`horschamptex.rs`) : avec elle, le trait a la texture du pinceau, comme dans
    /// une case ; sans elle (annotations, fichiers d'avant), c'est une ligne vectorielle.
    pub brosse: Option<Arc<Brosse>>,
}

impl Trace {
    pub fn new(calque: u64, anchor: Ancre, color: [u8; 3], points: Vec<VPoint>) -> Trace {
        Trace { id: PROCHAIN_TRAIT.fetch_add(1, Ordering::Relaxed), calque, anchor, color, alpha: 255, points, brosse: None }
    }

    /// Le même trait, à l'opacité `alpha`.
    pub fn avec_alpha(mut self, alpha: u8) -> Trace {
        self.alpha = alpha;
        self
    }

    /// Le même trait, avec la recette de son pinceau.
    pub fn avec_brosse(mut self, brosse: Option<Brosse>) -> Trace {
        self.brosse = brosse.map(Arc::new);
        self
    }
}

impl PartialEq for Trace {
    fn eq(&self, o: &Trace) -> bool {
        self.calque == o.calque && self.anchor == o.anchor && self.color == o.color && self.alpha == o.alpha && self.points == o.points && self.brosse == o.brosse
    }
}

impl std::fmt::Debug for Trace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Trace").field("calque", &self.calque).field("anchor", &self.anchor).field("color", &self.color).field("alpha", &self.alpha).field("points", &self.points).field("brosse", &self.brosse.is_some()).finish()
    }
}

/// Un calque d'annotation de la planche : un nom, un œil. Son rang dans `Annotations::calques` dit s'il est dessus ou dessous
/// (le dernier est au-dessus).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Calque {
    /// Enregistré : les traits y renvoient. Unique dans le projet.
    pub id: u64,
    pub nom: String,
    /// Montré ou caché (un affichage : rien n'est effacé).
    pub visible: bool,
    /// Le calque du dessin hors des cases (Anthony, 2026-10-30) : ce qui est tracé sur la planche hors du champ de toute case et qui
    /// n'appartient à aucun plan. Il sort dans l'export de la planche même sans l'option des annotations, et jamais ailleurs.
    pub planche: bool,
}

/// Les annotations d'un projet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Annotations {
    /// Les calques, du plus bas au plus haut. Aucun tant qu'on n'a pas tracé : le premier trait crée « Annotations ».
    pub calques: Vec<Calque>,
    /// Tous les traits, à plat, dans l'ordre où ils ont été posés.
    pub traces: Vec<Trace>,
    /// Les séquences que la planche montre, et dans quel ordre (`planchesequences.rs`) : enregistré ici, dans la clé `sequences` du fichier.
    pub affichage: crate::planchesequences::Affichage,
}

// ------------------------------------------------------------------------------------ les cases

/// Les images de toutes les cases de la planche, dans l'ordre de lecture : les vignettes, puis les cases vides.
pub fn cases(layout: &BoardLayout) -> Vec<Rect> {
    layout.cards.iter().map(|c| c.thumb).chain(layout.empty.iter().copied()).collect()
}

/// L'image de la case désignée par `a` (son rang et son rectangle). Une case au-delà de la planche est la dernière.
pub fn case_de(layout: &BoardLayout, a: &Ancre) -> Option<(usize, Rect)> {
    // (Un trait attaché à une case annulée reste où il est : il ne se coupe pas, `planannule.rs`.)
    let Some(k) = layout.cards.iter().position(|c| c.id == a.card) else {
        return layout.annulees.iter().find(|c| c.id == a.card).map(|c| (usize::MAX, c.thumb));
    };
    let all = cases(layout);
    let at = (k + a.after as usize).min(all.len().checked_sub(1)?);
    Some((at, all[at]))
}

/// La distance d'un point à un rectangle (0 dedans).
fn distance(r: Rect, p: Pos2) -> f32 {
    let dx = (r.left() - p.x).max(p.x - r.right()).max(0.0);
    let dy = (r.top() - p.y).max(p.y - r.bottom()).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// La case la plus proche du point `p` (planche) : son rang, son rectangle. En cas d'égalité, la première.
pub fn case_proche(layout: &BoardLayout, p: Pos2) -> Option<(usize, Rect)> {
    let all = cases(layout);
    all.iter().enumerate().map(|(i, r)| (i, *r, distance(*r, p))).min_by(|a, b| a.2.total_cmp(&b.2)).map(|(i, r, _)| (i, r))
}

/// L'ancre qui désigne la case de rang `rank`.
pub fn ancre_du_rang(layout: &BoardLayout, rank: usize) -> Option<Ancre> {
    let n = layout.cards.len();
    if n == 0 {
        return None;
    }
    let rank32 = rank.min(u32::MAX as usize) as u32;
    Some(if rank < n { Ancre { card: layout.cards[rank].id, after: 0, rank: rank32 } } else { Ancre { card: layout.cards[n - 1].id, after: (rank - (n - 1)).min(u32::MAX as usize) as u32, rank: rank32 } })
}

fn relatif(p: &VPoint, r: Rect) -> VPoint {
    let k = r.width().max(1e-3);
    VPoint { x: (p.x - r.left()) / k, y: (p.y - r.top()) / k, r: p.r / k }
}

fn absolu(p: &VPoint, r: Rect) -> VPoint {
    let k = r.width();
    VPoint { x: r.left() + p.x * k, y: r.top() + p.y * k, r: p.r * k }
}

fn centre(points: &[VPoint]) -> Option<Pos2> {
    let b = board::points_bounds(points);
    b.is_finite().then(|| b.center())
}

impl Annotations {
    /// Aucun trait (rien à effacer). Des calques sans trait ne comptent pas ici ; pour savoir s'il y a quelque chose à écrire
    /// dans le projet : `rien_a_ecrire`.
    pub fn is_empty(&self) -> bool {
        self.traces.is_empty()
    }

    /// Ni trait, ni calque que l'utilisateur aurait nommé, caché ou multiplié : le projet n'a pas besoin de `annotations.json`.
    pub fn rien_a_ecrire(&self) -> bool {
        self.traces.is_empty() && self.calques.iter().all(|c| c.nom == NOM_PAR_DEFAUT && c.visible && !c.planche) && self.calques.len() <= 1 && self.affichage.est_par_defaut()
    }

    // ------------------------------------------------------------------ les calques

    pub fn calque(&self, id: u64) -> Option<&Calque> {
        self.calques.iter().find(|c| c.id == id)
    }

    /// Un identifiant que personne n'a (les traits y renvoient : il est enregistré).
    fn id_libre(&self) -> u64 {
        self.calques.iter().map(|c| c.id).max().map_or(1, |m| m + 1)
    }

    /// Le nom du prochain calque : « Annotations » d'abord, puis « Annotations 2 », « Annotations 3 »…
    pub fn nom_libre(&self) -> String {
        let pris = |n: &str| self.calques.iter().any(|c| c.nom.trim().eq_ignore_ascii_case(n));
        if !pris(NOM_PAR_DEFAUT) {
            return NOM_PAR_DEFAUT.to_string();
        }
        (2..).map(|k| format!("{NOM_PAR_DEFAUT} {k}")).find(|n| !pris(n)).expect("un nom libre")
    }

    /// Pose un calque neuf au-dessus des autres. Rien s'il y en a déjà `MAX_CALQUES`.
    pub fn ajouter_calque(&mut self) -> Option<u64> {
        if self.calques.len() >= MAX_CALQUES {
            return None;
        }
        let id = self.id_libre();
        let nom = self.nom_libre();
        self.calques.push(Calque { id, nom, visible: true, planche: false });
        Some(id)
    }

    /// Le calque du dessin hors des cases (« Hors cadres »), créé au besoin, tout en bas de la liste (les annotations restent dessus).
    pub fn calque_hors_cadres(&mut self) -> u64 {
        if let Some(c) = self.calques.iter().find(|c| c.planche) {
            return c.id;
        }
        let id = self.id_libre();
        self.calques.insert(0, Calque { id, nom: NOM_HORS_CADRES.to_string(), visible: true, planche: true });
        id
    }

    /// L'identifiant du calque du dessin hors des cases, s'il existe.
    pub fn id_hors_cadres(&self) -> Option<u64> {
        self.calques.iter().find(|c| c.planche).map(|c| c.id)
    }

    /// Ajoute un trait du dessin hors des cases : points en coordonnées de la planche, attaché à la case la plus proche de son centre,
    /// avec sa couleur et son opacité. Vrai si le trait est gardé.
    pub fn ajouter_hors_cadres(&mut self, layout: &BoardLayout, points: &[VPoint], color: [u8; 3], alpha: u8) -> bool {
        self.ajouter_hors_cadres_brosse(layout, points, color, alpha, None)
    }

    /// Comme `ajouter_hors_cadres`, avec la recette du pinceau du trait : `brosse` = (la recette, l'appui et l'instant de chaque point, vides
    /// s'ils ne correspondent pas aux points, l'échelle de la case d'origine : unités de la planche par pixel du dessin). Le trait a alors la
    /// texture du pinceau, comme dans une case (`horschamptex.rs`).
    pub fn ajouter_hors_cadres_brosse(&mut self, layout: &BoardLayout, points: &[VPoint], color: [u8; 3], alpha: u8, brosse: Option<(Recette, Vec<f32>, Vec<f64>, f32)>) -> bool {
        if points.is_empty() || self.traces.len() >= MAX_TRACES || points.iter().any(|p| !(p.x.is_finite() && p.y.is_finite() && p.r.is_finite())) {
            return false;
        }
        let Some(c) = centre(points) else { return false };
        let Some((rank, r)) = case_proche(layout, c) else { return false };
        let Some(anchor) = ancre_du_rang(layout, rank) else { return false };
        // (Un trait trop long est réduit : l'appui et l'instant de chaque point suivent les mêmes indices.)
        let gardes = crate::horschamptex::indices_reduits(points.len(), MAX_POINTS);
        let relatifs: Vec<VPoint> = gardes.iter().map(|&i| relatif(&points[i], r)).collect();
        let brosse = brosse.filter(|(_, _, _, a)| a.is_finite() && *a > 1e-6).map(|(recette, appui, instants, a)| {
            let choisir = |n: usize| appui.len() == n;
            let (appui, instants) = if choisir(points.len()) && instants.len() == points.len() {
                (gardes.iter().map(|&i| appui[i]).collect(), gardes.iter().map(|&i| instants[i]).collect())
            } else {
                (Vec::new(), Vec::new())
            };
            Brosse::nouvelle(recette, appui, instants, r.width() / a)
        });
        let calque = self.calque_hors_cadres();
        self.traces.push(Trace::new(calque, anchor, color, relatifs).avec_alpha(alpha).avec_brosse(brosse));
        true
    }

    /// Les textures (pointe, papier) que citent les pinceaux des traits : à écrire avec le projet, comme celles des traits des plans.
    pub fn textures_citees(&self) -> Vec<(TextureKind, u32)> {
        self.traces.iter().filter_map(|t| t.brosse.as_ref()).flat_map(|b| b.textures()).collect()
    }

    /// Le calque `voulu` s'il existe ; sinon le premier ; sinon un calque « Annotations », créé.
    pub fn calque_ou_premier(&mut self, voulu: Option<u64>) -> u64 {
        if let Some(id) = voulu.filter(|id| self.calque(*id).is_some()) {
            return id;
        }
        if let Some(c) = self.calques.first() {
            return c.id;
        }
        self.ajouter_calque().expect("la liste est vide")
    }

    /// Renomme un calque (le nom est taillé aux bords ; un nom vide ne change rien). Vrai si le nom a changé.
    pub fn renommer_calque(&mut self, id: u64, nom: &str) -> bool {
        let nom: String = nom.trim().chars().take(60).collect();
        match self.calques.iter_mut().find(|c| c.id == id) {
            Some(c) if !nom.is_empty() && c.nom != nom => {
                c.nom = nom;
                true
            }
            _ => false,
        }
    }

    /// Retire un calque et ses traits (une étape d'annulation les rend). Rend combien de traits sont partis, rien si le calque n'existe pas.
    pub fn supprimer_calque(&mut self, id: u64) -> Option<usize> {
        let rang = self.calques.iter().position(|c| c.id == id)?;
        self.calques.remove(rang);
        let avant = self.traces.len();
        self.traces.retain(|t| t.calque != id);
        Some(avant - self.traces.len())
    }

    /// Monte (ou descend) un calque d'un cran. Vrai s'il a bougé.
    pub fn deplacer_calque(&mut self, id: u64, vers_le_haut: bool) -> bool {
        let Some(k) = self.calques.iter().position(|c| c.id == id) else { return false };
        let to = if vers_le_haut { k + 1 } else { k.wrapping_sub(1) };
        if to >= self.calques.len() {
            return false;
        }
        self.calques.swap(k, to);
        true
    }

    /// Pose un calque à la place `vers` de la liste (0 : tout en bas, le dernier : tout en haut) : un glissé dans la liste du panneau. Vrai
    /// s'il a bougé.
    pub fn placer_calque(&mut self, id: u64, vers: usize) -> bool {
        let Some(k) = self.calques.iter().position(|c| c.id == id) else { return false };
        let vers = vers.min(self.calques.len() - 1);
        if vers == k {
            return false;
        }
        let c = self.calques.remove(k);
        self.calques.insert(vers, c);
        true
    }

    /// Retire les traits `ids` (ceux qu'on a choisis au lasso). Rend combien sont partis.
    pub fn supprimer_traces(&mut self, ids: &[u64]) -> usize {
        let avant = self.traces.len();
        self.traces.retain(|t| !ids.contains(&t.id));
        avant - self.traces.len()
    }

    /// Montre ou cache un calque (un affichage). Vrai si l'état a changé.
    pub fn regler_visible(&mut self, id: u64, visible: bool) -> bool {
        match self.calques.iter_mut().find(|c| c.id == id) {
            Some(c) if c.visible != visible => {
                c.visible = visible;
                true
            }
            _ => false,
        }
    }

    pub fn traits_du_calque(&self, id: u64) -> usize {
        self.traces.iter().filter(|t| t.calque == id).count()
    }

    /// Remet de l'ordre après une lecture ou une modification à la main : des identifiants de calques tous différents, pas plus de
    /// `MAX_CALQUES` (les traits des calques en trop passent au dernier gardé : rien n'est perdu), et chaque trait dans un calque qui existe
    /// (sinon dans le premier, créé au besoin). Rend vrai si quelque chose a changé.
    pub fn normaliser(&mut self) -> bool {
        let mut changed = false;
        let mut vus: Vec<u64> = Vec::new();
        for k in 0..self.calques.len() {
            let id = self.calques[k].id;
            if id == 0 || vus.contains(&id) {
                // (Identifiant mal écrit ou en double : un neuf ; les traits qui y renvoient restent au premier calque de ce numéro.)
                let neuf = self.calques.iter().map(|c| c.id).chain(vus.iter().copied()).max().unwrap_or(0) + 1;
                self.calques[k].id = neuf;
                changed = true;
            }
            vus.push(self.calques[k].id);
        }
        if self.calques.len() > MAX_CALQUES {
            let garde = self.calques[MAX_CALQUES - 1].id;
            let en_trop: Vec<u64> = self.calques[MAX_CALQUES..].iter().map(|c| c.id).collect();
            for t in &mut self.traces {
                if en_trop.contains(&t.calque) {
                    t.calque = garde;
                }
            }
            self.calques.truncate(MAX_CALQUES);
            changed = true;
        }
        if self.traces.iter().any(|t| self.calque(t.calque).is_none()) {
            let premier = self.calque_ou_premier(None);
            let connus: Vec<u64> = self.calques.iter().map(|c| c.id).collect();
            for t in &mut self.traces {
                if !connus.contains(&t.calque) {
                    t.calque = premier;
                }
            }
            changed = true;
        }
        changed
    }

    /// Les traits à montrer, du bas vers le haut : calque par calque, dans l'ordre des calques, les calques cachés écartés. Un trait dont
    /// le calque manque est montré par-dessus (il n'est jamais perdu).
    pub fn ordre_de_peinture(&self) -> Vec<usize> {
        let mut v: Vec<(usize, usize)> = self
            .traces
            .iter()
            .enumerate()
            .filter_map(|(i, t)| match self.calques.iter().position(|c| c.id == t.calque) {
                Some(r) => self.calques[r].visible.then_some((r, i)),
                None => Some((usize::MAX, i)),
            })
            .collect();
        v.sort_unstable();
        v.into_iter().map(|(_, i)| i).collect()
    }

    // ------------------------------------------------------------------ les traits

    /// Les points du trait `i` sur la planche (rien si sa vignette n'existe plus : voir `rattacher_manquantes`).
    pub fn points_planche(&self, layout: &BoardLayout, i: usize) -> Option<Vec<VPoint>> {
        let t = self.traces.get(i)?;
        let (_, r) = case_de(layout, &t.anchor)?;
        Some(t.points.iter().map(|p| absolu(p, r)).collect())
    }

    /// Les points du trait d'identifiant `id` sur la planche.
    pub fn points_planche_de(&self, layout: &BoardLayout, id: u64) -> Option<Vec<VPoint>> {
        self.points_planche(layout, self.traces.iter().position(|t| t.id == id)?)
    }

    /// Ajoute un trait tracé sur la planche (`points` en coordonnées de la planche), attaché à la case la plus proche
    /// de son centre, dans le premier calque (créé au besoin). Vrai si le trait est gardé.
    pub fn ajouter(&mut self, layout: &BoardLayout, points: &[VPoint], color: [u8; 3]) -> bool {
        self.ajouter_dans(layout, None, points, color)
    }

    /// Comme `ajouter`, dans le calque `calque` (s'il n'existe pas : le premier).
    pub fn ajouter_dans(&mut self, layout: &BoardLayout, calque: Option<u64>, points: &[VPoint], color: [u8; 3]) -> bool {
        if points.is_empty() || self.traces.len() >= MAX_TRACES || points.iter().any(|p| !(p.x.is_finite() && p.y.is_finite() && p.r.is_finite())) {
            return false;
        }
        let Some(c) = centre(points) else { return false };
        let Some((rank, r)) = case_proche(layout, c) else { return false };
        let Some(anchor) = ancre_du_rang(layout, rank) else { return false };
        let points: Vec<VPoint> = reduire(points, MAX_POINTS).iter().map(|p| relatif(p, r)).collect();
        let calque = self.calque_ou_premier(calque);
        self.traces.push(Trace::new(calque, anchor, color, points));
        true
    }

    /// Les traits que touche le cercle (`p`, `radius`, planche), dans `calque` seulement s'il est donné.
    fn touches(&self, layout: &BoardLayout, calque: Option<u64>, p: Pos2, radius: f32) -> Vec<usize> {
        (0..self.traces.len()).filter(|&i| calque.is_none_or(|c| self.traces[i].calque == c) && self.points_planche(layout, i).is_some_and(|pts| board::touches(&pts, p, radius))).collect()
    }

    /// Gomme : retire les traits que touche le cercle (`p`, `radius`, planche), de tous les calques. Rend combien.
    pub fn effacer_en(&mut self, layout: &BoardLayout, p: Pos2, radius: f32) -> usize {
        self.retirer(self.touches(layout, None, p, radius))
    }

    /// Gomme d'un calque : comme `effacer_en`, mais les traits des autres calques restent.
    pub fn effacer_en_calque(&mut self, layout: &BoardLayout, calque: u64, p: Pos2, radius: f32) -> usize {
        self.retirer(self.touches(layout, Some(calque), p, radius))
    }

    fn retirer(&mut self, hit: Vec<usize>) -> usize {
        for &i in hit.iter().rev() {
            self.traces.remove(i);
        }
        hit.len()
    }

    /// Un trait est-il sous le cercle (`p`, `radius`) ?
    pub fn touche(&self, layout: &BoardLayout, p: Pos2, radius: f32) -> bool {
        !self.touches(layout, None, p, radius).is_empty()
    }

    /// Un trait du calque `calque` est-il sous le cercle ?
    pub fn touche_calque(&self, layout: &BoardLayout, calque: u64, p: Pos2, radius: f32) -> bool {
        !self.touches(layout, Some(calque), p, radius).is_empty()
    }

    // ------------------------------------------------------------------ le lasso

    /// Les traits du calque `calque` qui ont au moins un point dans le lasso `polygone` (planche) : la même règle que le lasso des
    /// traits vectoriels d'un dessin (`Element::touches`). Dans l'ordre où ils ont été posés.
    pub fn dans_le_lasso(&self, layout: &BoardLayout, calque: u64, polygone: &[Pos2]) -> Vec<u64> {
        if polygone.len() < 3 {
            return Vec::new();
        }
        (0..self.traces.len())
            .filter(|&i| self.traces[i].calque == calque)
            .filter(|&i| self.points_planche(layout, i).is_some_and(|pts| pts.iter().any(|p| crate::vector::point_in_polygon(p.pos(), polygone))))
            .map(|i| self.traces[i].id)
            .collect()
    }

    /// Le trait du calque `calque` sous le point `p` (à `tol` près) : le plus haut. Un clic du lasso choisit ainsi un trait.
    pub fn sous_le_point(&self, layout: &BoardLayout, calque: u64, p: Pos2, tol: f32) -> Option<u64> {
        self.touches(layout, Some(calque), p, tol).last().map(|&i| self.traces[i].id)
    }

    /// Ce que couvrent, sur la planche, les traits `ids` (rayon compris). Rien s'il n'y en a aucun de placé.
    pub fn bornes(&self, layout: &BoardLayout, ids: &[u64]) -> Option<Rect> {
        let mut out: Option<Rect> = None;
        for id in ids {
            if let Some(pts) = self.points_planche_de(layout, *id) {
                let b = board::points_bounds(&pts);
                if b.is_finite() {
                    out = Some(out.map_or(b, |o| o.union(b)));
                }
            }
        }
        out
    }

    /// Applique `aff` (planche) aux traits `ids`, puis rattache chacun à la case la plus proche de son nouveau centre (comme à
    /// la pose d'un trait). L'épaisseur suit l'échelle. Rend combien de traits ont bougé.
    pub fn transformer(&mut self, layout: &BoardLayout, ids: &[u64], aff: &Affine) -> usize {
        let mut n = 0;
        for t in &mut self.traces {
            if !ids.contains(&t.id) {
                continue;
            }
            let Some((_, r)) = case_de(layout, &t.anchor) else { continue };
            let pts: Vec<VPoint> = t.points.iter().map(|p| absolu(p, r)).collect();
            let new = transformee(&pts, aff);
            if new.iter().any(|p| !(p.x.is_finite() && p.y.is_finite() && p.r.is_finite())) {
                continue;
            }
            let Some(c) = centre(&new) else { continue };
            let Some((rank, rc)) = case_proche(layout, c) else { continue };
            let Some(anchor) = ancre_du_rang(layout, rank) else { continue };
            t.points = new
                .iter()
                .map(|p| {
                    let mut q = relatif(p, rc);
                    q.r = q.r.clamp(1e-5, 50.0);
                    q
                })
                .collect();
            t.anchor = anchor;
            // (Un trait au pinceau : la taille du pinceau suit l'échelle, et son rendu est à refaire à sa nouvelle place.)
            t.brosse = t.brosse.as_ref().map(|b| Arc::new(b.mise_a_l_echelle(aff.width_factor())));
            n += 1;
        }
        n
    }

    /// Les traits dont la vignette n'est plus sur la planche `apres` se rattachent à leur voisine la plus proche
    /// (voir le début du fichier). `avant` : la planche d'avant, d'où l'on sait où ils étaient. Rend combien de traits ont changé de case.
    pub fn rattacher_manquantes(&mut self, avant: Option<&BoardLayout>, apres: &BoardLayout) -> usize {
        if apres.cards.is_empty() {
            return 0;
        }
        let mut changes = 0;
        for t in &mut self.traces {
            // (Une vignette dont la séquence est seulement masquée n'a pas disparu : son trait attend, il ne passe pas à une voisine.)
            if apres.cards.iter().any(|c| c.id == t.anchor.card) || apres.masquees.contains(&t.anchor.card) {
                continue;
            }
            // Là où le trait était sur l'ancienne planche, puis la vignette restante la plus proche de ce point.
            let was = avant.and_then(|b| case_de(b, &t.anchor).map(|(_, r)| (b, r)));
            let new = was.and_then(|(b, r)| {
                let pts: Vec<VPoint> = t.points.iter().map(|p| absolu(p, r)).collect();
                let c = centre(&pts)?;
                let near = b.cards.iter().filter(|k| apres.cards.iter().any(|a| a.id == k.id)).min_by(|x, y| distance(x.thumb, c).total_cmp(&distance(y.thumb, c)))?;
                let rank = apres.cards.iter().position(|a| a.id == near.id)?;
                // Les points se reportent sur la case voisine (même place sur l'ancienne planche), relativement à elle.
                let anchor = Ancre { card: near.id, after: 0, rank: rank as u32 };
                let points = pts.iter().map(|p| relatif(p, near.thumb)).collect::<Vec<_>>();
                Some((anchor, points))
            });
            match new {
                Some((anchor, points)) => {
                    t.anchor = anchor;
                    t.points = points;
                    t.brosse = t.brosse.as_ref().map(|b| Arc::new(b.refaite()));
                }
                None => {
                    // Pas d'ancienne planche : la vignette de même rang, au même décalage.
                    let rank = (t.anchor.rank as usize).min(apres.cards.len() - 1);
                    t.anchor = Ancre { card: apres.cards[rank].id, after: 0, rank: rank as u32 };
                }
            }
            changes += 1;
        }
        changes
    }

    /// Le rang des cases est mis à jour (pour que le secours de `rattacher_manquantes` reste proche de la réalité).
    /// Ne compte pas comme une modification du projet. Rend vrai si un rang a changé.
    pub fn noter_rangs(&mut self, layout: &BoardLayout) -> bool {
        let mut changed = false;
        for t in &mut self.traces {
            // (Une case annulée n'a pas de rang.)
            if let Some((rank, _)) = case_de(layout, &t.anchor).filter(|(r, _)| *r != usize::MAX) {
                if t.anchor.rank as usize != rank {
                    t.anchor.rank = rank as u32;
                    changed = true;
                }
            }
        }
        changed
    }

    /// Ce que couvre l'ensemble des traits montrés sur la planche (pour cadrer l'export ; un calque caché n'y est pas).
    pub fn etendue(&self, layout: &BoardLayout) -> Option<Rect> {
        let mut out: Option<Rect> = None;
        for i in self.ordre_de_peinture() {
            if let Some(pts) = self.points_planche(layout, i) {
                let b = board::points_bounds(&pts);
                if b.is_finite() {
                    out = Some(out.map_or(b, |o| o.union(b)));
                }
            }
        }
        out
    }
}

/// Les points `pts` (planche) après `aff` : la position suit la transformation, le rayon l'échelle (moyenne géométrique).
pub fn transformee(pts: &[VPoint], aff: &Affine) -> Vec<VPoint> {
    let k = aff.width_factor();
    pts.iter()
        .map(|p| {
            let q = aff.apply(pos2(p.x, p.y));
            VPoint { x: q.x, y: q.y, r: p.r * k }
        })
        .collect()
}

/// Garde au plus `max` points d'un trait en prenant des points réguliers (le premier et le dernier restent).
pub fn reduire(points: &[VPoint], max: usize) -> Vec<VPoint> {
    if points.len() <= max.max(2) {
        return points.to_vec();
    }
    let max = max.max(2);
    (0..max).map(|k| points[(k * (points.len() - 1)) / (max - 1)]).collect()
}

/// Simplifie un trait en cours de tracé : retire les points à moins de `eps` de leur voisin gardé (le dernier reste).
pub fn epurer(points: &[VPoint], eps: f32) -> Vec<VPoint> {
    let mut out: Vec<VPoint> = Vec::with_capacity(points.len());
    for p in points {
        if out.last().is_none_or(|l| (l.pos() - p.pos()).length() >= eps) {
            out.push(*p);
        }
    }
    if let (Some(last), Some(l)) = (points.last(), out.last().copied()) {
        if l != *last {
            out.push(*last);
        }
    }
    out
}

// ------------------------------------------------------------------------------------ le fichier

#[derive(Serialize, Deserialize)]
struct TraceFichier {
    ancre: Ancre,
    couleur: [u8; 3],
    /// Opacité ; absente : pleine (les fichiers d'avant).
    #[serde(default = "opaque", skip_serializing_if = "est_opaque")]
    alpha: u8,
    points: Vec<[f32; 3]>,
    /// La recette du pinceau d'un trait du dessin hors des cases (absente : une ligne vectorielle, comme les fichiers d'avant).
    #[serde(default, rename = "pinceau", skip_serializing_if = "Option::is_none")]
    pinceau: Option<PinceauFichier>,
}

/// La recette d'un trait au pinceau (`horschamptex::Brosse`) : le réglage complet (pointe et papier par clé de contenu, comme un trait
/// vectoriel d'un plan), l'opacité, la dureté, la trace (appuis, instants, toucher, couleur B : le format d'un trait vectoriel) et la largeur de
/// la case en pixels du dessin.
#[derive(Serialize, Deserialize)]
struct PinceauFichier {
    #[serde(default, rename = "reglage", skip_serializing_if = "Option::is_none")]
    reglage: Option<BrushSettings>,
    #[serde(default = "un")]
    opacite: f32,
    #[serde(default = "un")]
    durete: f32,
    #[serde(default)]
    trace: crate::vector::Trace,
    #[serde(rename = "largeur")]
    largeur_px: f32,
    /// La taille du pinceau multipliée depuis (absente : 1).
    #[serde(default = "un", skip_serializing_if = "est_un")]
    taille: f32,
}

fn un() -> f32 {
    1.0
}

fn est_un(v: &f32) -> bool {
    (*v - 1.0).abs() < 1e-6
}

fn pinceau_fichier(b: &Brosse, n: usize) -> PinceauFichier {
    let aligne = b.appui.len() == n && b.instants.len() == n;
    PinceauFichier {
        reglage: b.recette.reglage,
        opacite: b.recette.opacite,
        durete: b.recette.durete,
        trace: crate::vector::Trace {
            pressure: if aligne { b.appui.clone() } else { Vec::new() },
            time: if aligne { b.instants.clone() } else { Vec::new() },
            feel: b.recette.toucher,
            secondary: b.recette.couleur_b,
            cache: Default::default(),
        },
        largeur_px: b.largeur_px,
        taille: b.taille,
    }
}

/// La recette lue : écartée si elle est absurde (largeur nulle, nombre qui n'en est pas un) ; le trait reste alors une ligne.
fn brosse_lue(p: PinceauFichier, n: usize) -> Option<Brosse> {
    let ok = |v: f32| v.is_finite() && v > 0.0;
    if !ok(p.largeur_px) || !ok(p.taille) || !p.opacite.is_finite() || !p.durete.is_finite() {
        return None;
    }
    let aligne = p.trace.pressure.len() == n && p.trace.time.len() == n && p.trace.pressure.iter().all(|v| v.is_finite()) && p.trace.time.iter().all(|v| v.is_finite());
    let recette = Recette { reglage: p.reglage, opacite: p.opacite.clamp(0.0, 1.0), durete: p.durete.clamp(0.0, 1.0), toucher: p.trace.feel, couleur_b: p.trace.secondary };
    let mut b = Brosse::nouvelle(recette, if aligne { p.trace.pressure } else { Vec::new() }, if aligne { p.trace.time } else { Vec::new() }, p.largeur_px);
    b.taille = p.taille;
    Some(b)
}

fn vrai() -> bool {
    true
}

fn opaque() -> u8 {
    255
}

fn est_opaque(a: &u8) -> bool {
    *a == 255
}

/// Un calque dans le fichier (version 2) : son identifiant, son nom, son œil, ses traits dans l'ordre où ils ont été posés.
#[derive(Serialize, Deserialize)]
struct CalqueFichier {
    id: u64,
    nom: String,
    #[serde(default = "vrai")]
    visible: bool,
    /// Le calque du dessin hors des cases (absent : un calque d'annotation ordinaire).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    planche: bool,
    #[serde(default)]
    traces: Vec<TraceFichier>,
}

#[derive(Serialize, Deserialize)]
struct Fichier {
    format: String,
    version: u32,
    /// Version 1 (d'avant les calques) : tous les traits, à plat. Lue comme un seul calque ; plus écrite.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    traces: Vec<TraceFichier>,
    /// Version 2 : les calques, du plus bas au plus haut.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    calques: Vec<CalqueFichier>,
    /// Les séquences que la planche montre, dans l'ordre de la planche (absent : toutes, dans l'ordre du film).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sequences: Vec<SequenceFichier>,
}

/// Une séquence de l'affichage de la planche (`planchesequences::Affichage`).
#[derive(Serialize, Deserialize)]
struct SequenceFichier {
    id: u64,
    #[serde(default = "vrai")]
    montree: bool,
}

fn arrondi(v: f32) -> f32 {
    (v * 10_000.0).round() / 10_000.0
}

/// Le texte de `annotations.json` pour `doc` ; rien s'il n'y a pas d'annotation (le fichier part alors, s'il est
/// lisible : `write_sidecar`). Version 2 : les calques et leurs traits.
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    if doc.annotations.rien_a_ecrire() {
        return Ok(None);
    }
    // (Un trait dont le calque manque irait nulle part : il passe au premier calque plutôt que d'être perdu.)
    let orphelins = doc.annotations.traces.iter().any(|t| doc.annotations.calque(t.calque).is_none());
    let rangee;
    let a = if orphelins {
        let mut c = doc.annotations.clone();
        c.normaliser();
        rangee = c;
        &rangee
    } else {
        &doc.annotations
    };
    let mut restant = MAX_TRACES;
    let calques = a
        .calques
        .iter()
        .map(|c| {
            let traces: Vec<TraceFichier> = a
                .traces
                .iter()
                .filter(|t| t.calque == c.id)
                .take(restant)
                .map(|t| {
                    let n = t.points.len().min(MAX_POINTS);
                    TraceFichier {
                        ancre: t.anchor,
                        couleur: t.color,
                        alpha: t.alpha,
                        points: t.points.iter().take(MAX_POINTS).map(|p| [arrondi(p.x), arrondi(p.y), arrondi(p.r)]).collect(),
                        pinceau: t.brosse.as_ref().map(|b| pinceau_fichier(b, t.points.len())).map(|mut p| {
                            // (Un trait de plus de `MAX_POINTS` points est coupé : les appuis suivent.)
                            p.trace.pressure.truncate(n);
                            p.trace.time.truncate(n);
                            p
                        }),
                    }
                })
                .collect();
            restant -= traces.len();
            CalqueFichier { id: c.id, nom: c.nom.clone(), visible: c.visible, planche: c.planche, traces }
        })
        .collect();
    let sequences = a.affichage.ordre.iter().take(MAX_SEQUENCES).map(|&(id, montree)| SequenceFichier { id, montree }).collect();
    let f = Fichier { format: FORMAT.into(), version: VERSION, traces: Vec::new(), calques, sequences };
    serde_json::to_vec_pretty(&f).map(Some).map_err(|e| e.to_string())
}

/// Un trait lu dans le fichier : écarté s'il n'a aucun point ; un point qui n'est pas un nombre fini est écarté, le rayon est borné.
fn trace_lue(t: TraceFichier, calque: u64) -> Option<Trace> {
    let lus = t.points.len().min(MAX_POINTS);
    let points: Vec<VPoint> = t.points.into_iter().take(MAX_POINTS).filter(|p| p.iter().all(|v| v.is_finite())).map(|[x, y, r]| VPoint { x, y, r: r.clamp(1e-5, 50.0) }).collect();
    // (Un point écarté défait l'alignement des appuis : la recette reste, sans appuis, ils se retrouvent d'après les rayons.)
    let brosse = t.pinceau.and_then(|p| brosse_lue(p, if points.len() == lus { points.len() } else { usize::MAX }));
    (!points.is_empty()).then(|| Trace::new(calque, t.ancre, t.couleur, points).avec_alpha(t.alpha).avec_brosse(brosse))
}

/// Les annotations lues dans le texte de `annotations.json` (version 1 : un seul calque, « Annotations » ; version 2 : ses calques). Les
/// traits sans point, ou dont un point n'est pas un nombre, sont écartés ; rien de plus que `MAX_CALQUES` calques et `MAX_TRACES` traits de
/// `MAX_POINTS` points.
pub(crate) fn parse(text: &str) -> Result<Annotations, String> {
    let f: Fichier = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if f.format != FORMAT {
        return Err("ce n'est pas un fichier d'annotations".into());
    }
    if f.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", f.version));
    }
    let mut a = Annotations::default();
    // (Un identifiant à zéro ou déjà pris par un calque d'avant : un neuf, au-dessus de tous ceux du fichier. Ses traits le suivent.)
    let mut neuf = f.calques.iter().map(|c| c.id).max().unwrap_or(0);
    for c in f.calques {
        if a.calques.len() >= MAX_CALQUES {
            // (Les calques en trop : leurs traits passent au dernier gardé, rien ne se perd.)
            let garde = a.calques[MAX_CALQUES - 1].id;
            let room = MAX_TRACES.saturating_sub(a.traces.len());
            a.traces.extend(c.traces.into_iter().filter_map(|t| trace_lue(t, garde)).take(room));
            continue;
        }
        let id = if c.id == 0 || a.calque(c.id).is_some() {
            neuf += 1;
            neuf
        } else {
            c.id
        };
        let nom = if c.nom.trim().is_empty() { NOM_PAR_DEFAUT.to_string() } else { c.nom.trim().chars().take(60).collect() };
        a.calques.push(Calque { id, nom, visible: c.visible, planche: c.planche && a.calques.iter().all(|k| !k.planche) });
        let room = MAX_TRACES.saturating_sub(a.traces.len());
        a.traces.extend(c.traces.into_iter().filter_map(|t| trace_lue(t, id)).take(room));
    }
    // Version 1 : les traits à plat vont dans un calque, le premier (créé : « Annotations »). Ils restent en tête, dans leur ordre.
    if !f.traces.is_empty() {
        let id = a.calque_ou_premier(None);
        let room = MAX_TRACES.saturating_sub(a.traces.len());
        let anciens: Vec<Trace> = f.traces.into_iter().filter_map(|t| trace_lue(t, id)).take(room).collect();
        a.traces.splice(0..0, anciens);
    }
    // L'affichage de la planche : un identifiant à zéro ou répété est écarté (la lecture de l'affichage oublie ce qui n'existe plus).
    let mut vus = std::collections::HashSet::new();
    a.affichage.ordre = f.sequences.into_iter().take(MAX_SEQUENCES).filter(|s| s.id != 0 && vus.insert(s.id)).map(|s| (s.id, s.montree)).collect();
    a.normaliser();
    Ok(a)
}

#[cfg(test)]
#[path = "annotations_tests.rs"]
mod tests;
