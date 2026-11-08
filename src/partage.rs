//! Le partage d'un calque entre des plans choisis : **la planche-mère** (Anthony, 2026-11-07 : « un grand dessin, plusieurs cadres
//! caméra, donc plusieurs plans partageant ce dessin ; un plan se détache à la demande » ; 2026-11-08 : « une option de plan (un plan
//! réutilisé) ou de calque (un décor réutilisé) »). Conception : `suivi/PLANCHE-MERE.md` ; l'interface : `partageui.rs`.
//!
//! Un calque porte une **clé de partage** (`Layer::partage`). Tous les calques du film qui portent la même clé sont les **exemplaires**
//! d'un même calque partagé : ils montrent **les mêmes dessins** (les mêmes numéros de dessin : retoucher l'un, c'est retoucher tous) et
//! portent les mêmes réglages (nom, sorte, opacité, mode de fusion, effets…), chacun dans son plan, **sous la caméra de son plan** : un
//! dessin plus grand que la feuille est cadré autrement par chaque plan. Ce qui reste à chaque plan : la pose du calque (transformation,
//! clés de position, trajectoire), ses clés d'opacité, sa profondeur, son dossier, sa ligne réduite.
//!
//! `sync_partages` accorde les exemplaires à chaque image d'écran (comme les calques de toute la scène, `Document::sync_scene_layers`) :
//! le modèle est l'exemplaire du plan courant (celui qu'on retouche), sinon le premier dans l'ordre du film ; ses dessins sont coupés à
//! la durée de chaque plan, le dernier tenu (la planche-mère est un dessin tenu). Une clé qui n'a plus qu'un exemplaire disparaît
//! d'elle-même ; un calque qui ne peut plus être partagé (devenu dossier…) perd la sienne.
//!
//! **Détacher** (`detacher_partage`) : l'exemplaire garde une **copie** de ses dessins et perd la clé ; les autres ne changent pas.
//! **« Réutiliser ce plan »** (`reutiliser_plan_apres`) : un nouveau plan juste après, dans une scène neuve de la même séquence, dont
//! tous les calques de dessin sont liés à ceux du plan d'origine, avec sa propre caméra (celle de l'origine au départ), sa durée, ses
//! marques, ses textes vides. **Couper** un plan garde le lien des deux parties ; **réunir** deux plans détache d'abord le second ;
//! **dupliquer** ou **coller** un plan donne des copies sans lien (lier se demande, jamais en silence).
//!
//! Le fichier : `partage` du calque (format 51). Un calque partagé ne compte pas dans la durée de son plan (`own_frame_count`), comme un
//! calque de toute la scène.

use std::collections::HashMap;

use crate::document::{fresh_layer_id, own_frame_count, relink_copies, CelId, Document, Layer};

/// Où réutiliser un calque, depuis les menus : le plan suivant, le précédent, tous les plans de la séquence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cibles {
    Suivant,
    Precedent,
    Sequence,
}

impl Layer {
    /// Un calque qui peut être partagé entre des plans : un calque de dessin (vectoriel ou bitmap), ni dossier, ni calque de couleur
    /// (il suit son calque de trait), ni vidéo, ni calque d'effet, ni calque de toute la scène.
    pub fn partageable(&self) -> bool {
        !self.folder && self.couleur.is_none() && self.video.is_none() && self.effet.is_none() && self.scene_wide.is_none()
    }

    /// Cet exemplaire accordé au `modele` : ses dessins (coupés à `images`, le dernier tenu) et ses réglages ; le numéro, la pose, la
    /// trajectoire, les clés d'opacité, la profondeur, le dossier, la ligne réduite et la position verrouillée restent les siens.
    pub(crate) fn accorde_au_partage(&self, modele: &Layer, images: usize) -> Layer {
        let mut frames = modele.frames.clone();
        frames.truncate(images);
        if let Some(last) = frames.last().copied().flatten() {
            frames.resize(images, Some(last));
        }
        Layer {
            id: self.id,
            frames,
            transform: self.transform,
            pos_keys: self.pos_keys.clone(),
            trajectoire: self.trajectoire.clone(),
            opacity_keys: self.opacity_keys.clone(),
            depth: self.depth,
            parent: self.parent,
            collapsed: self.collapsed,
            position_locked: self.position_locked,
            partage: self.partage,
            ..modele.clone()
        }
    }
}

impl Document {
    /// Les exemplaires du partage `cle` : (rang du plan, rang du calque), dans l'ordre du film.
    pub fn exemplaires(&self, cle: u64) -> Vec<(usize, usize)> {
        let mut v = Vec::new();
        for (pi, p) in self.panels.iter().enumerate() {
            for (li, l) in p.layers.iter().enumerate() {
                if l.partage == Some(cle) {
                    v.push((pi, li));
                }
            }
        }
        v
    }

    /// Les plans (rangs) qui ont un exemplaire du calque `li` du plan `pi`, lui compris, dans l'ordre du film ; vide s'il n'est pas
    /// partagé.
    pub fn plans_du_partage(&self, pi: usize, li: usize) -> Vec<usize> {
        match self.panels.get(pi).and_then(|p| p.layers.get(li)).and_then(|l| l.partage) {
            Some(cle) => self.exemplaires(cle).into_iter().map(|(p, _)| p).collect(),
            None => Vec::new(),
        }
    }

    /// Le plan `pi` a au moins un calque partagé.
    pub fn plan_partage(&self, pi: usize) -> bool {
        self.panels.get(pi).is_some_and(|p| p.layers.iter().any(|l| l.partage.is_some()))
    }

    /// Les plans visés par « réutiliser dans… » depuis le plan `pi` : le suivant, le précédent, ou les autres plans de sa séquence.
    pub fn cibles(&self, pi: usize, quoi: Cibles) -> Vec<usize> {
        if pi >= self.panels.len() {
            return Vec::new();
        }
        match quoi {
            Cibles::Suivant => (pi + 1 < self.panels.len()).then_some(pi + 1).into_iter().collect(),
            Cibles::Precedent => pi.checked_sub(1).into_iter().collect(),
            Cibles::Sequence => {
                let (a, b) = self.sequence_run(pi);
                (a..b).filter(|&i| i != pi).collect()
            }
        }
    }

    /// Accorde les exemplaires de chaque partage (voir l'en-tête) ; le modèle est l'exemplaire du plan `cur`, sinon le premier du film.
    /// Un calque qui ne peut plus être partagé, ou une clé qui n'a plus qu'un exemplaire, perd sa clé. Vrai si quelque chose a changé.
    pub fn sync_partages(&mut self, cur: usize) -> bool {
        let mut changed = false;
        for p in &mut self.panels {
            for l in &mut p.layers {
                if l.partage.is_some() && !l.partageable() {
                    l.partage = None;
                    changed = true;
                }
            }
        }
        // Les modèles et le nombre d'exemplaires de chaque clé.
        let mut modeles: HashMap<u64, (Layer, usize)> = HashMap::new();
        for (pi, p) in self.panels.iter().enumerate() {
            for l in &p.layers {
                let Some(k) = l.partage else { continue };
                match modeles.get_mut(&k) {
                    None => {
                        modeles.insert(k, (l.clone(), 1));
                    }
                    Some(m) => {
                        m.1 += 1;
                        if pi == cur {
                            m.0 = l.clone();
                        }
                    }
                }
            }
        }
        for p in &mut self.panels {
            let n = own_frame_count(p);
            for l in &mut p.layers {
                let Some(k) = l.partage else { continue };
                match modeles.get(&k) {
                    Some((_, 1)) | None => {
                        l.partage = None;
                        changed = true;
                    }
                    Some((m, _)) => {
                        let want = l.accorde_au_partage(m, n);
                        if *l != want {
                            *l = want;
                            changed = true;
                        }
                    }
                }
            }
        }
        changed
    }

    /// Le calque `li` du plan `pi` reçoit (ou garde) sa clé de partage, et chaque plan de `cibles` qui n'en a pas d'exemplaire en reçoit
    /// un (une copie du calque, mêmes dessins, au même rang ; au-dessus de tout s'il a moins de calques). Rend la clé ; rien si le calque
    /// ne peut pas être partagé.
    pub fn reutiliser_calque(&mut self, pi: usize, li: usize, cibles: &[usize]) -> Option<u64> {
        if !self.panels.get(pi)?.layers.get(li)?.partageable() {
            return None;
        }
        let cle = match self.panels[pi].layers[li].partage {
            Some(k) => k,
            None => {
                let k = fresh_layer_id();
                self.panels[pi].layers[li].partage = Some(k);
                k
            }
        };
        let modele = self.panels[pi].layers[li].clone();
        for &ci in cibles {
            if ci == pi || ci >= self.panels.len() || self.panels[ci].layers.iter().any(|x| x.partage == Some(cle)) {
                continue;
            }
            let n = own_frame_count(&self.panels[ci]);
            let neuf = Layer { id: fresh_layer_id(), parent: None, ..modele.clone() }.accorde_au_partage(&modele, n);
            let at = li.min(self.panels[ci].layers.len());
            self.panels[ci].layers.insert(at, neuf);
        }
        Some(cle)
    }

    /// L'exemplaire `li` du plan `pi` quitte son partage : il garde une copie de ses dessins (les autres gardent les leurs) et perd sa
    /// clé ; s'il ne reste qu'un exemplaire, celui-ci perd la clé aussi. Vrai si le calque était partagé.
    pub fn detacher_partage(&mut self, pi: usize, li: usize) -> bool {
        let Some(cle) = self.panels.get(pi).and_then(|p| p.layers.get(li)).and_then(|l| l.partage) else { return false };
        let autres = self.exemplaires(cle).into_iter().filter(|&(p, l)| (p, l) != (pi, li)).count();
        self.panels[pi].layers[li].partage = None;
        if autres > 0 {
            self.copier_dessins_du_calque(pi, li);
        }
        if autres == 1 {
            for (p, l) in self.exemplaires(cle) {
                self.panels[p].layers[l].partage = None;
            }
        }
        true
    }

    /// Tous les calques partagés du plan `pi` se détachent (« Détacher ce plan »). Rend combien l'étaient.
    pub fn detacher_plan(&mut self, pi: usize) -> usize {
        let n = self.panels.get(pi).map_or(0, |p| p.layers.len());
        (0..n).filter(|&li| self.detacher_partage(pi, li)).count()
    }

    /// « Réutiliser ce plan » : une copie du plan `index` juste après lui, dans une scène neuve de la même séquence, dont chaque calque de
    /// dessin est lié au sien (même clé, mêmes dessins) ; sa caméra, sa durée et ses marques sont celles de l'origine, ses textes vides.
    /// Rend le rang du nouveau plan.
    pub fn reutiliser_plan_apres(&mut self, index: usize) -> usize {
        let sequence = self.sequence_of(index);
        let scene = self.new_scene(sequence);
        let mut copie = self.duplicate_panel(index);
        copie.scene = scene;
        copie.action.clear();
        copie.dialogue.clear();
        copie.notes.clear();
        // (La copie a reçu des copies des dessins : les calques partageables reprennent les dessins d'origine, et la clé.)
        let n = copie.layers.len().min(self.panels[index].layers.len());
        for li in 0..n {
            if !self.panels[index].layers[li].partageable() {
                continue;
            }
            let cle = match self.panels[index].layers[li].partage {
                Some(k) => k,
                None => {
                    let k = fresh_layer_id();
                    self.panels[index].layers[li].partage = Some(k);
                    k
                }
            };
            let source = &self.panels[index].layers[li];
            copie.layers[li].partage = Some(cle);
            copie.layers[li].frames = source.frames.clone();
            copie.layers[li].names = source.names.clone();
            copie.layers[li].color_marks = source.color_marks.clone();
        }
        let mut copies = vec![copie];
        self.adopt_scene_layers(&mut copies, scene);
        self.insert_and_normalize(index + 1, copies)
    }

    /// Les dessins du calque `li` du plan `pi` remplacés par des copies (les noms d'instance et les marques de couleur suivent).
    fn copier_dessins_du_calque(&mut self, pi: usize, li: usize) {
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        let mut frames = std::mem::take(&mut self.panels[pi].layers[li].frames);
        for f in frames.iter_mut().flatten() {
            *f = match copies.get(f) {
                Some(&c) => c,
                None => {
                    let c = self.duplicate_cel(*f);
                    copies.insert(*f, c);
                    c
                }
            };
        }
        let old_ids = vec![self.panels[pi].layers[li].id];
        self.panels[pi].layers[li].frames = frames;
        relink_copies(std::slice::from_mut(&mut self.panels[pi].layers[li]), &old_ids, &copies);
    }
}
