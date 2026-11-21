//! Partie 197 (Anthony : « un panneau qui propose de choisir ce qu'on y montre en partant de la séquence jusqu'aux calques ; ce serait
//! la barre latérale de la timeline mais sans la temporalité ») : le modèle du panneau Scène, son arbre. L'affichage est dans
//! `sceneui.rs`, la doc dans `docs/INTERFACE.md`, partie 197.
//!
//! - **Depuis** (`Racine`) : le film (les séquences, leurs plans, leurs calques), la séquence courante (ses plans, leurs calques) ou
//!   le plan courant (ses calques). Ce qu'on y a déplié ou replié (les plans, les séquences) est dans `ChoixScene`, un réglage du
//!   poste ; les dossiers de calques se replient dans le projet (`Layer::collapsed`), comme dans la timeline : les deux suivent.
//! - **L'arbre** (`arbre`) : une ligne par séquence, plan, calque ou caméra, dans l'ordre de la colonne de la timeline et avec ses
//!   règles (`calques_du_plan`) : dans le Storyboard, les rangées des pistes (`montagerows::layer_in_row` : un calque de couleur lié
//!   à son trait n'a pas de rangée), la caméra au-dessus des calques ; dans l'Animation, les lignes de la colonne des calques
//!   (`layerheads::rows_of_en` : les groupes cachés en moins), la caméra dessous. Un calque rangé dans un dossier fermé n'a pas de
//!   ligne, sauf le calque courant (`pistes::dossier_ferme`).
//! - **Sans tout refaire** (`Arbre`) : l'arbre n'est refait que si son empreinte change (la racine, le plan et le calque courants, ce
//!   qui est déplié, et la structure de ce qu'il montre : identifiants des séquences, des plans et des calques, dossiers, rangements).
//!   Les noms, l'œil, le cadenas se lisent dans le projet à chaque image, pour les seules lignes visibles (`Arbre::visibles`).

use std::hash::{Hash, Hasher};
use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::document::{Document, Layer};
use crate::groups::LayerGroup;

/// Hauteur de la ligne d'une séquence (racine Film).
pub const H_SEQUENCE: f32 = 22.0;
/// Hauteur de la ligne d'un plan : sa petite vignette, son nom, sa durée.
pub const H_PLAN: f32 = 26.0;
/// Hauteur de la ligne d'un calque (et de la caméra).
pub const H_CALQUE: f32 = 20.0;
/// Hauteur de la ligne complète du calque courant (`layerheads::ROW_FULL`, celle de la timeline).
pub const H_COURANT: f32 = crate::layerheads::ROW_FULL;
/// Retrait d'un niveau de l'arbre, en points (`.orow` de la maquette : 14).
pub const RETRAIT: f32 = 14.0;

/// D'où part ce que montre le panneau.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Racine {
    #[serde(rename = "film")]
    Film,
    #[serde(rename = "sequence")]
    Sequence,
    #[default]
    #[serde(rename = "plan")]
    Plan,
}

impl Racine {
    /// Dans l'ordre du choix « Depuis », de gauche à droite.
    pub const TOUTES: [Racine; 3] = [Racine::Film, Racine::Sequence, Racine::Plan];

    pub fn nom(self) -> &'static str {
        match self {
            Racine::Film => "Film",
            Racine::Sequence => "Séquence",
            Racine::Plan => "Plan",
        }
    }

    pub fn aide(self) -> &'static str {
        match self {
            Racine::Film => "Les séquences du film, leurs plans et leurs calques. Un clic sur une séquence mène à son premier plan.",
            Racine::Sequence => "Les plans de la séquence courante (vignette, nom, durée) ; le plan courant est déplié jusqu'à ses calques, les autres se déplient.",
            Racine::Plan => "Les calques du plan courant, en arbre avec leurs dossiers : la colonne de la timeline, sans le temps.",
        }
    }
}

/// Ce que montre le panneau Scène, et ce qu'on y a déplié ou replié (réglage du poste, `Settings::scene`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChoixScene {
    #[serde(default)]
    pub racine: Racine,
    /// Les plans dépliés à la main (identifiants), en plus du plan courant.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plans_ouverts: Vec<u64>,
    /// Les plans repliés à la main alors qu'ils sont le plan courant (déplié d'office).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plans_fermes: Vec<u64>,
    /// Les séquences dépliées à la main (racine Film), en plus de la séquence courante.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sequences_ouvertes: Vec<u64>,
    /// Les séquences repliées à la main alors qu'elles sont la séquence courante.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sequences_fermees: Vec<u64>,
}

fn basculer(liste: &mut Vec<u64>, id: u64) {
    match liste.iter().position(|x| *x == id) {
        Some(k) => {
            liste.remove(k);
        }
        None => liste.push(id),
    }
}

impl ChoixScene {
    pub fn est_d_origine(&self) -> bool {
        *self == ChoixScene::default()
    }

    /// Le plan `id` est-il déplié ? Le plan courant l'est d'office.
    pub fn plan_ouvert(&self, id: u64, courant: bool) -> bool {
        if courant {
            !self.plans_fermes.contains(&id)
        } else {
            self.plans_ouverts.contains(&id)
        }
    }

    pub fn basculer_plan(&mut self, id: u64, courant: bool) {
        basculer(if courant { &mut self.plans_fermes } else { &mut self.plans_ouverts }, id);
    }

    /// La séquence `id` est-elle dépliée ? La séquence courante l'est d'office.
    pub fn sequence_ouverte(&self, id: u64, courante: bool) -> bool {
        if courante {
            !self.sequences_fermees.contains(&id)
        } else {
            self.sequences_ouvertes.contains(&id)
        }
    }

    pub fn basculer_sequence(&mut self, id: u64, courante: bool) {
        basculer(if courante { &mut self.sequences_fermees } else { &mut self.sequences_ouvertes }, id);
    }
}

/// Ce qu'est une ligne de l'arbre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quoi {
    /// Une séquence (racine Film) : ses plans sont `debut..debut + n`.
    Sequence { seq: u64, debut: usize, n: usize },
    /// Un plan (son rang dans le film).
    Plan { pi: usize },
    /// Un calque du plan `pi`, au rang `li` de sa pile (`id` : pour vérifier qu'il y est toujours).
    Calque { pi: usize, li: usize, id: u64 },
    /// La caméra du plan `pi`.
    Camera { pi: usize },
}

/// Une ligne de l'arbre : ce qu'elle montre, son niveau (son retrait), si elle est dépliée (une séquence, un plan, un dossier ouvert),
/// son haut (depuis le haut de la liste) et sa hauteur.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ligne {
    pub quoi: Quoi,
    pub niveau: u8,
    pub ouvert: bool,
    pub top: f32,
    pub h: f32,
}

/// Combien de dossiers contiennent le calque `li`.
fn profondeur(layers: &[Layer], li: usize) -> u8 {
    let mut n = 0u8;
    let mut k = li;
    while let Some(p) = crate::folders::parent_index(layers, k) {
        n = n.saturating_add(1);
        if n > 16 {
            break;
        }
        k = p;
    }
    n
}

/// Les calques que la colonne de la timeline montre pour un plan, de haut en bas, avec leur retrait : `cur` est le calque courant
/// si c'est le plan courant (il garde sa ligne même dans un dossier fermé). `sb` : la timeline du Storyboard (ses pistes), sinon
/// celle de l'Animation (sa colonne des calques).
pub fn calques_du_plan(layers: &[Layer], groups: &[LayerGroup], cur: Option<usize>, sb: bool) -> Vec<(usize, u8)> {
    let cur = cur.unwrap_or(usize::MAX);
    if sb {
        (0..crate::montagerows::row_count(layers))
            .filter_map(|row| crate::montagerows::layer_in_row(layers, row))
            .filter(|&k| crate::folders::parent_index(layers, k).is_none_or(|fi| !crate::pistes::dossier_ferme(layers, fi, cur)))
            .map(|k| (k, profondeur(layers, k)))
            .collect()
    } else {
        crate::layerheads::rows_of_en(layers, groups, cur, false)
            .into_iter()
            .map(|r| (r.li, profondeur(layers, r.li) + u8::from(layers[r.li].keys_follow_line())))
            .collect()
    }
}

/// Où l'arbre en est : la racine, la timeline (Storyboard ou Animation), le plan et le calque courants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ou {
    pub racine: Racine,
    pub sb: bool,
    pub cur_panel: usize,
    pub cur_layer: usize,
}

/// Les suites de plans de même séquence, dans l'ordre du film : (séquence, premier plan, nombre de plans).
pub fn suites_de_sequences(doc: &Document) -> Vec<(u64, usize, usize)> {
    let ids = doc.sequence_ids();
    let mut out: Vec<(u64, usize, usize)> = Vec::new();
    for (pi, s) in ids.into_iter().enumerate() {
        match out.last_mut() {
            Some(last) if last.0 == s => last.2 += 1,
            _ => out.push((s, pi, 1)),
        }
    }
    out
}

/// L'arbre que le panneau montre (les hauts et les hauteurs posés).
pub fn arbre(doc: &Document, choix: &ChoixScene, ou: Ou) -> Vec<Ligne> {
    let mut out: Vec<Ligne> = Vec::new();
    if doc.panels.is_empty() {
        return out;
    }
    let cur_panel = ou.cur_panel.min(doc.panels.len() - 1);
    let mut pousser = |out: &mut Vec<Ligne>, quoi: Quoi, niveau: u8, ouvert: bool, h: f32| out.push(Ligne { quoi, niveau, ouvert, top: 0.0, h });
    let contenu = |out: &mut Vec<Ligne>, pi: usize, niveau: u8| {
        let p = &doc.panels[pi];
        let courant = pi == cur_panel;
        let camera = Ligne { quoi: Quoi::Camera { pi }, niveau, ouvert: false, top: 0.0, h: H_CALQUE };
        if ou.sb {
            out.push(camera);
        }
        for (li, retrait) in calques_du_plan(&p.layers, &doc.groups, courant.then_some(ou.cur_layer), ou.sb) {
            let l = &p.layers[li];
            let plein = courant && li == ou.cur_layer && !l.folder;
            out.push(Ligne { quoi: Quoi::Calque { pi, li, id: l.id }, niveau: niveau + retrait, ouvert: l.folder && !l.collapsed, top: 0.0, h: if plein { H_COURANT } else { H_CALQUE } });
        }
        if !ou.sb {
            out.push(camera);
        }
    };
    match ou.racine {
        Racine::Plan => contenu(&mut out, cur_panel, 0),
        Racine::Sequence => {
            let (debut, fin) = doc.sequence_run(cur_panel);
            for pi in debut..fin {
                let ouvert = choix.plan_ouvert(doc.panels[pi].id, pi == cur_panel);
                pousser(&mut out, Quoi::Plan { pi }, 0, ouvert, H_PLAN);
                if ouvert {
                    contenu(&mut out, pi, 1);
                }
            }
        }
        Racine::Film => {
            for (seq, debut, n) in suites_de_sequences(doc) {
                let courante = (debut..debut + n).contains(&cur_panel);
                let ouverte = choix.sequence_ouverte(seq, courante);
                pousser(&mut out, Quoi::Sequence { seq, debut, n }, 0, ouverte, H_SEQUENCE);
                if !ouverte {
                    continue;
                }
                for pi in debut..debut + n {
                    let ouvert = choix.plan_ouvert(doc.panels[pi].id, pi == cur_panel);
                    pousser(&mut out, Quoi::Plan { pi }, 1, ouvert, H_PLAN);
                    if ouvert {
                        contenu(&mut out, pi, 2);
                    }
                }
            }
        }
    }
    let mut top = 0.0;
    for l in &mut out {
        l.top = top;
        top += l.h;
    }
    out
}

fn hacher_calques<H: Hasher>(h: &mut H, layers: &[Layer]) {
    layers.len().hash(h);
    for l in layers {
        (l.id, l.parent, l.folder, l.collapsed, l.group, l.keys_follow_line()).hash(h);
    }
}

/// L'empreinte de ce que l'arbre montre (pas des noms ni des réglages, lus à chaque image) : il n'est refait que si elle change.
pub fn empreinte(doc: &Document, choix: &ChoixScene, ou: Ou) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (ou.racine, ou.sb, ou.cur_panel, ou.cur_layer, doc.panels.len()).hash(&mut h);
    (&choix.plans_ouverts, &choix.plans_fermes, &choix.sequences_ouvertes, &choix.sequences_fermees).hash(&mut h);
    if !ou.sb {
        for g in &doc.groups {
            g.shown.hash(&mut h);
        }
    }
    if doc.panels.is_empty() {
        return h.finish();
    }
    let cur_panel = ou.cur_panel.min(doc.panels.len() - 1);
    let plan = |h: &mut std::collections::hash_map::DefaultHasher, pi: usize| {
        let p = &doc.panels[pi];
        p.id.hash(h);
        if choix.plan_ouvert(p.id, pi == cur_panel) {
            hacher_calques(h, &p.layers);
        }
    };
    match ou.racine {
        Racine::Plan => hacher_calques(&mut h, &doc.panels[cur_panel].layers),
        Racine::Sequence => {
            let (debut, fin) = doc.sequence_run(cur_panel);
            (debut, fin).hash(&mut h);
            for pi in debut..fin {
                plan(&mut h, pi);
            }
        }
        Racine::Film => {
            for (seq, debut, n) in suites_de_sequences(doc) {
                (seq, debut, n).hash(&mut h);
                if choix.sequence_ouverte(seq, (debut..debut + n).contains(&cur_panel)) {
                    for pi in debut..debut + n {
                        plan(&mut h, pi);
                    }
                }
            }
        }
    }
    h.finish()
}

/// L'arbre gardé d'une image à l'autre (`App::scene_arbre`), refait seulement quand son empreinte change.
#[derive(Debug, Default)]
pub struct Arbre {
    empreinte: Option<u64>,
    pub lignes: Vec<Ligne>,
    /// La hauteur de toute la liste.
    pub hauteur: f32,
    /// Combien de fois il a été refait (pour les essais et le diagnostic).
    pub reconstructions: u64,
    /// Combien de lignes la dernière image a peintes (seulement celles qu'on voit).
    pub peintes: usize,
    /// Partie 206 : la hauteur de la barre du haut (« Depuis » et le fil d'Ariane) à la dernière image ; 0 : pas encore peinte.
    pub barre: f32,
}

impl Arbre {
    /// Refait l'arbre si ce qu'il montre a changé ; rend vrai s'il l'a été.
    pub fn a_jour(&mut self, doc: &Document, choix: &ChoixScene, ou: Ou) -> bool {
        let e = empreinte(doc, choix, ou);
        if self.empreinte == Some(e) {
            return false;
        }
        self.lignes = arbre(doc, choix, ou);
        self.hauteur = self.lignes.last().map_or(0.0, |l| l.top + l.h);
        self.empreinte = Some(e);
        self.reconstructions += 1;
        true
    }

    /// Oublie l'arbre : l'image suivante le refait (après un changement que l'empreinte ne voit pas).
    pub fn oublier(&mut self) {
        self.empreinte = None;
    }

    /// Les rangs des lignes qui touchent `y0..y1` (depuis le haut de la liste).
    pub fn visibles(&self, y0: f32, y1: f32) -> Range<usize> {
        let debut = self.lignes.partition_point(|l| l.top + l.h <= y0);
        let fin = self.lignes.partition_point(|l| l.top < y1);
        debut..fin.max(debut)
    }
}

/// Le fil d'Ariane : « Film », « Film › Séquence 3 », « Film › Séquence 3 › Plan 3 ».
pub fn fil(doc: &Document, racine: Racine, cur_panel: usize) -> Vec<String> {
    let mut out = vec!["Film".to_string()];
    if doc.panels.is_empty() || racine == Racine::Film {
        return out;
    }
    let pi = cur_panel.min(doc.panels.len() - 1);
    out.push(format!("Séquence {}", doc.sequence_label(doc.sequence_of(pi))));
    if racine == Racine::Plan {
        out.push(crate::storyboard::panel_title(doc, pi));
    }
    out
}
