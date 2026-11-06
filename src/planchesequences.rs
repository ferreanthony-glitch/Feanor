//! La planche par séquences (Anthony, 2026-10-30) : « afficher les vignettes par séquences, pouvoir créer des séquences de vignettes,
//! choisir d'afficher une séquence, certaines séquences (et choisir l'ordre) ou toutes les séquences sur le canevas ». Le modèle, la mise
//! en page et les opérations sur le film sont ici ; l'interface (titres, liste du panneau, commandes) est dans `planchesequencesui.rs`.
//!
//! # Ce qui est affiché
//!
//! - **`Affichage`** : la liste des séquences avec, pour chacune, « montrée » ou non, dans l'ordre où la planche les range. Rien d'enregistré :
//!   toutes les séquences, dans l'ordre du film (comme avant). Une séquence que la liste ne connaît pas encore (ajoutée depuis) se montre, à sa
//!   place dans l'ordre du film (juste après celle qui la précède dans le film). Une séquence qui n'existe plus est oubliée. **L'ordre choisi ne
//!   change que la planche** : le film (le montage, les exports, la numérotation) garde son ordre. La planche montre toujours au moins une
//!   séquence.
//! - Enregistré avec le projet dans `annotations.json` (clé `sequences`, `Annotations::affichage`) : ce fichier est déjà lu et écrit partout
//!   (enregistrer, enregistrer par-dessus, film partagé) avec tolérance ; un projet d'avant s'ouvre avec toutes les séquences dans l'ordre du film,
//!   et une version de SHOT d'avant ignore la clé. `project::VERSION` ne change pas.
//!
//! # La mise en page
//!
//! - **Une séquence = une planche à elle** : `board::layout_indices` range les plans de la séquence exactement comme `board::layout_with`
//!   rangeait tout le film (sens de lecture, cases par ligne, textes, mouvements de caméra, cases pâles), et `layout_sequences` empile ces planches
//!   l'une sous l'autre : **chaque séquence commence sur une nouvelle rangée**, avec son **titre** au-dessus (son nom, ou « Séquence 2 »).
//! - Les cases pâles « nouvelle case » sont à la fin de chaque séquence : **une seule** case pâle à la fin de chaque séquence (celle qui suit la
//!   dernière case), et, pour la dernière séquence seulement, les rangées pâles du réglage (celles de la planche d'avant). Dessiner dedans fait
//!   un plan **dans cette séquence**, juste après son dernier plan.
//! - **Entre deux séquences, aucune barre de coupe** : le titre les sépare (une barre n'a pas de sens à travers un titre). Reste la barre de
//!   la dernière case de chaque séquence, qui mène à sa case pâle.
//! - Avec **une seule séquence** dans le projet, rien n'est ajouté (ni titre ni décalage) : les cases sont exactement où elles étaient.
//! - Tout le reste suit : les cases sont des `CardPlace` comme avant (même identifiants, même rang de plan dans le film) ; un trait d'annotation ou
//!   du dessin hors des cases est attaché à **la case la plus proche** (voir `annotations.rs`) : il suit sa case quand on réordonne les séquences, et
//!   se cache avec elle quand sa séquence est masquée, sans jamais être perdu ni passer à une voisine (`BoardLayout::masquees`).

use std::collections::HashSet;

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

use crate::board::{self, BoardLayout, BoardSettings, GroupeBoard, MoveBoard, Options, VueSequences, MARGIN, PAGE_PAD};
use crate::document::Document;

/// Hauteur réservée au titre d'une séquence, au-dessus de sa première rangée (points de la planche), dont la hauteur du titre lui-même.
pub const TITRE_H: f32 = 46.0;
pub const TITRE_RECT_H: f32 = 40.0;
/// Écart entre le bas d'une séquence et le titre de la suivante.
pub const GROUPE_GAP: f32 = 44.0;
/// Largeur d'un titre vide d'une séquence étroite.
pub const TITRE_LARGEUR_MIN: f32 = 260.0;

/// Les séquences que la planche montre, et dans quel ordre.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Affichage {
    /// Les séquences (identifiant, montrée) dans l'ordre de la planche. Vide : toutes, dans l'ordre du film.
    pub ordre: Vec<(u64, bool)>,
}

impl Affichage {
    /// Rien à enregistrer : toutes les séquences, dans l'ordre du film.
    pub fn est_par_defaut(&self) -> bool {
        self.ordre.is_empty()
    }

    /// La liste complète pour ce film : les séquences connues et encore là, dans l'ordre choisi, puis celles qui manquent à leur place du film
    /// (juste après leur précédente), montrées.
    pub fn liste(&self, doc: &Document) -> Vec<(u64, bool)> {
        liste_de(&self.ordre, &doc.sequence_order())
    }

    /// Les séquences montrées, dans l'ordre de la planche. Toujours au moins une : si aucune n'était montrée, toutes le sont.
    pub fn montrees(&self, doc: &Document) -> Vec<u64> {
        let liste = self.liste(doc);
        let montrees: Vec<u64> = liste.iter().filter(|e| e.1).map(|e| e.0).collect();
        if montrees.is_empty() {
            liste.iter().map(|e| e.0).collect()
        } else {
            montrees
        }
    }
}

/// Voir `Affichage::liste` (`film` : les séquences dans l'ordre du film).
pub fn liste_de(ordre: &[(u64, bool)], film: &[u64]) -> Vec<(u64, bool)> {
    let mut out: Vec<(u64, bool)> = Vec::with_capacity(film.len());
    let existe: HashSet<u64> = film.iter().copied().collect();
    let mut vues: HashSet<u64> = HashSet::new();
    for &(id, on) in ordre {
        if existe.contains(&id) && vues.insert(id) {
            out.push((id, on));
        }
    }
    for (i, &id) in film.iter().enumerate() {
        if vues.contains(&id) {
            continue;
        }
        // À sa place du film : juste après la séquence qui la précède (déjà dans la liste), sinon en tête.
        let at = if i == 0 { 0 } else { out.iter().position(|e| e.0 == film[i - 1]).map_or(out.len(), |p| p + 1) };
        out.insert(at, (id, true));
        vues.insert(id);
    }
    out
}

/// Ce que montre la planche de ce film : rien (une seule séquence : la planche d'avant, sans titre), ou les séquences montrées dans l'ordre
/// choisi, chacune avec son titre.
pub fn vue(doc: &Document) -> Option<VueSequences> {
    if doc.sequences.len() <= 1 || doc.sequence_order().len() <= 1 {
        return None;
    }
    Some(VueSequences { sequences: doc.annotations.affichage.montrees(doc), titres: true })
}

/// Les rangs des plans que la planche montre, dans l'ordre où elle les range : ce que l'export de la planche exporte (« tout »). Sans
/// plusieurs séquences : tout le film.
pub fn plans_affiches(doc: &Document) -> Vec<usize> {
    match vue(doc) {
        None => (0..doc.panels.len()).collect(),
        Some(v) => {
            let ids = doc.sequence_ids();
            v.sequences.iter().flat_map(|&q| (0..ids.len()).filter(|&i| ids[i] == q).collect::<Vec<usize>>()).collect()
        }
    }
}

/// La planche d'un film réduit à ses plans exportés (`planche::construire`) : par séquences, avec leurs titres, quand ces plans sont dans
/// plusieurs séquences ; sinon la planche d'avant. Les séquences se rangent dans l'ordre où elles apparaissent.
pub fn layout_export(sub: &Document, s: &BoardSettings, opt: &Options) -> BoardLayout {
    let sequences = sub.sequence_order();
    if sequences.len() > 1 {
        layout_sequences(sub, s, false, opt, &VueSequences { sequences, titres: true })
    } else {
        board::layout_with(sub, s, &Default::default(), false, opt)
    }
}

/// Les identifiants des vignettes (plans et marques d'image) du plan `i`.
fn ids_de(doc: &Document, i: usize) -> Vec<u64> {
    board::plan_boards(doc, i).into_iter().map(|b| b.0).collect()
}

fn dep(r: Rect, v: Vec2) -> Rect {
    r.translate(v)
}

fn dep_pos(p: Pos2, v: Vec2) -> Pos2 {
    p + v
}

/// La planche par séquences (voir le début du fichier) : chaque séquence de `vue.sequences`, rangée comme une planche à elle, empilées de haut
/// en bas dans l'ordre de `vue`. Mêmes arguments que `board::layout_with`.
pub fn layout_sequences(doc: &Document, s: &BoardSettings, last_joins: bool, opt: &Options, vue: &VueSequences) -> BoardLayout {
    let ids = doc.sequence_ids();
    let montrees: HashSet<u64> = vue.sequences.iter().copied().collect();
    // Les rangs de plan de chaque séquence montrée, dans l'ordre de la planche.
    let groupes: Vec<(u64, Vec<usize>)> = vue
        .sequences
        .iter()
        .filter_map(|&q| {
            let indices: Vec<usize> = (0..ids.len()).filter(|&i| ids[i] == q).collect();
            (!indices.is_empty()).then_some((q, indices))
        })
        .collect();
    let mut out = BoardLayout { sens: opt.sens, ..Default::default() };
    out.masquees = (0..ids.len()).filter(|&i| !montrees.contains(&ids[i])).flat_map(|i| ids_de(doc, i)).collect();
    if groupes.is_empty() {
        return out;
    }
    // Chaque séquence : sa planche, rangée à part. Les séquences d'avant n'ont qu'une rangée pâle ; la dernière a celles du réglage.
    let dernier = groupes.len() - 1;
    let planches: Vec<BoardLayout> = groupes
        .iter()
        .enumerate()
        .map(|(g, (_, indices))| {
            let o = if g == dernier { *opt } else { Options { pale_rows: opt.pale_rows.map(|_| 1), pale_max: opt.pale_rows.map(|_| 1), ..*opt } };
            board::layout_indices(doc, s, last_joins && g == dernier, &o, indices)
        })
        .collect();
    // Toutes les séquences partagent le même bord gauche : celle qui a glissé le plus à droite (une enveloppe de caméra qui dépasse) le donne.
    let dmax_x = planches.iter().map(|l| l.decalage.x).fold(0.0, f32::max);
    let (mut curseur, mut rang0) = (MARGIN, 0usize);
    let mut bas = MARGIN;
    let (mut largeur, mut decalage_y) = (0.0f32, 0.0f32);
    for (g, ((sequence, indices), l)) in groupes.iter().zip(planches).enumerate() {
        let dx = dmax_x - l.decalage.x;
        // Le haut de la première rangée de la séquence (celle de la planche d'avant, `MARGIN + PAGE_PAD`) vient sous son titre.
        let dy = curseur + TITRE_H - (MARGIN + PAGE_PAD);
        let v = vec2(dx, dy);
        let (cartes0, vides0, moves0) = (out.cards.len(), out.empty.len(), out.moves.len());
        let origine_x = MARGIN + PAGE_PAD + dmax_x;
        let mut contenu = Rect::NOTHING;
        for c in &l.cards {
            let mut c = *c;
            c.card = dep(c.card, v);
            c.thumb = dep(c.thumb, v);
            c.note = dep(c.note, v);
            c.foot = dep(c.foot, v);
            c.ligne = dep(c.ligne, v);
            c.reach += v.x;
            c.reach_y += v.y;
            c.row += rang0;
            c.mv = c.mv.map(|m| m + moves0);
            contenu = contenu.union(c.card).union(c.foot).union(c.note);
            out.cards.push(c);
        }
        for m in &l.moves {
            out.moves.push(MoveBoard {
                cadres: m.cadres.iter().map(|c| board::CadreBoard { rang: c.rang, rect: dep(c.rect, v), coins: c.coins.map(|q| dep_pos(q, v)), reduit: c.reduit }).collect(),
                enveloppe: m.enveloppe.iter().map(|q| dep_pos(*q, v)).collect(),
                tuile: board::TuileBoard { rect: dep(m.tuile.rect, v), zone: m.tuile.zone, masque: m.tuile.masque.clone() },
                balayage: m.balayage.iter().map(|p| p.iter().map(|q| dep_pos(*q, v)).collect()).collect(),
            });
        }
        for b in &l.bars {
            out.bars.push(board::CutBar { after: b.after + cartes0, cut: b.cut, bar: dep(b.bar, v), zone: dep(b.zone, v) });
        }
        // (La première page de la séquence monte jusqu'à son titre : sur l'export, le titre est sur la page.)
        let titre_haut = vue.titres.then_some(curseur - 6.0);
        for (k, p) in l.pages.iter().enumerate() {
            let mut p = dep(*p, v);
            if let (0, Some(h)) = (k, titre_haut) {
                p.min.y = p.min.y.min(h);
            }
            out.pages.push(p);
        }
        for e in &l.empty {
            let e = dep(*e, v);
            contenu = contenu.union(e);
            out.empty.push(e);
        }
        let rangees = l.cards.iter().map(|c| c.row + 1).max().unwrap_or(0);
        let derniere = indices.iter().copied().max().unwrap_or(0);
        let titre = vue.titres.then(|| Rect::from_min_size(pos2(origine_x, curseur), vec2((contenu.right() - origine_x).max(TITRE_LARGEUR_MIN), TITRE_RECT_H)));
        out.groupes.push(GroupeBoard {
            sequence: *sequence,
            titre,
            cartes: cartes0..out.cards.len(),
            vides: vides0..out.empty.len(),
            rang0,
            origine_y: l.origine_y + v.y,
            dernier_plan: derniere,
            plan_suivant: doc.panels.get(derniere + 1).map(|p| p.id),
        });
        if g == 0 {
            out.origine_y = l.origine_y + v.y;
        }
        decalage_y += l.decalage.y;
        largeur = largeur.max(l.size.x + dx);
        // Le bas de la séquence (sans la marge du bas de la planche d'avant) ; la suivante commence un écart plus bas.
        bas = l.size.y - MARGIN + dy;
        curseur = bas + GROUPE_GAP;
        rang0 += rangees;
    }
    out.origine_x = MARGIN + PAGE_PAD + dmax_x;
    out.decalage = vec2(dmax_x, decalage_y);
    out.size = vec2(largeur, bas + MARGIN);
    out
}

impl Document {
    /// Coupe la séquence du plan `index` juste avant son plan (celui de la case visée) : le plan et tous ceux qui le suivent dans la séquence forment
    /// une séquence neuve, sans nom (son numéro automatique). La première garde son identité (nom, numéro, versions). Renvoie le rang du plan,
    /// ou rien (et rien n'a bougé) s'il est déjà le premier de sa séquence.
    pub fn couper_sequence_avant(&mut self, index: usize) -> Option<usize> {
        if index >= self.panels.len() {
            return None;
        }
        // Le début de son plan (une scène d'avant les plans peut avoir plusieurs panneaux : on ne la sépare pas).
        let (debut, _) = self.scene_run(index);
        let (premier, fin) = self.sequence_run(debut);
        if debut <= premier {
            return None;
        }
        let id = self.panels[debut].id;
        let neuve = self.new_sequence();
        let scenes: Vec<u64> = self.panels[debut..fin].iter().map(|p| p.scene).collect();
        for sc in scenes {
            if let Some(s) = self.scene_mut(sc) {
                s.sequence = neuve;
            }
        }
        Some(self.normalize_keeping(id))
    }

    /// Une séquence neuve à la fin du film, avec un plan vide. Renvoie le rang de ce plan.
    pub fn nouvelle_sequence_a_la_fin(&mut self) -> usize {
        let dernier = self.panels.len().saturating_sub(1);
        self.add_sequence_after(dernier)
    }
}
