//! Une case de plus dans un plan = une clé de plus sur ses calques (Anthony, 2026-10-22 : « quand on crée une vignette de continuité,
//! elle doit devenir une nouvelle clé d'image sur la timeline »).
//!
//! Les cases d'un plan sont ses marques d'image (`Panel::marks`) ; les clés de dessin, elles, sont les expositions de chaque calque
//! (`Layer::frames` : un dessin tenu sur plusieurs images). Ce sont deux choses séparées : poser une marque ne touchait pas aux calques,
//! si bien que la nouvelle case montrait le dessin de la précédente, **tenu**, et que le retoucher retouchait aussi la case d'avant.
//! `cles_de_case` pose les clés quand la case est faite (une case dessinée après le dernier plan, deux plans réunis, une marque posée) :
//!
//! - un calque dont le dessin est tenu jusqu'à la nouvelle case reçoit **une copie de ce dessin**, à l'image de la case : rien ne saute
//!   à l'écran (la case montrait déjà ce dessin), et la retoucher laisse la case d'avant telle quelle ;
//! - un calque qui avait un dessin à la case d'avant mais rien à celle-ci (le plan qu'on vient de réunir n'avait pas ce calque) reçoit
//!   un **dessin vide** : la case est bien à elle, et on y dessine sans toucher à l'autre ;
//! - un calque qui n'a rien dessiné d'avant, un dossier, un calque d'effet, un calque vidéo, un calque de couleur (il suit son calque de trait,
//!   `sync_couleurs`), **le calque Décor** (un seul dessin pour toute la continuité, `decor.rs`) et **le calque d'annotation** du plan (des notes
//!   sur le plan, pas un dessin de la case, `calqueannot.rs`) ne reçoivent rien.
//!
//! Toujours dans l'étape d'annulation de ce qui fait la case : une annulation défait la case et ses clés.

use crate::camera::CameraKey;
use crate::document::{CelId, Document, Layer, Panel};

/// Les calques dont les clés font les cases d'un plan : les mêmes que ceux qui reçoivent une clé quand une case est faite
/// (`cles_de_case`).
fn calque_de_case(l: &Layer) -> bool {
    !(l.folder || l.effet.is_some() || l.video.is_some() || l.decor || l.annotation || l.couleur.is_some() || l.scene_wide.is_some())
}

/// Le dessin qui commence à l'image `f` de `l` : sa clé, s'il y en a une.
fn cle_a(l: &Layer, f: usize) -> Option<CelId> {
    let c = l.frames.get(f).copied().flatten()?;
    (f == 0 || l.frames[f - 1] != Some(c)).then_some(c)
}

/// Ce que devient une case (une marque d'image) quand on a retouché les clés de son plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sort {
    /// Elle garde sa place (rien n'a bougé, ou les calques ne sont pas d'accord).
    Reste,
    /// Ses clés sont maintenant à cette image : elle les suit.
    Va(usize),
    /// Ses clés ont été supprimées et les voisines ont pris leur place : la case est réunie à celle d'avant.
    Part,
}

/// Le sort de la case de l'image `f` : `porteurs` sont les clés qu'elle avait (calque, dessin) avant la retouche, `apres` le plan
/// retouché.
fn sort_de(porteurs: &[(u64, CelId)], apres: &Panel, f: usize) -> Sort {
    if porteurs.is_empty() {
        return Sort::Reste;
    }
    // Où est maintenant le début de chacun de ces dessins (l'exposition la plus proche, s'il en reste plusieurs).
    let mut ou: Vec<usize> = Vec::new();
    for &(calque, cel) in porteurs {
        let Some(l) = apres.layers.iter().find(|l| l.id == calque) else { continue };
        if let Some(s) = (0..l.frames.len()).filter(|&k| cle_a(l, k) == Some(cel)).min_by_key(|&k| k.abs_diff(f)) {
            ou.push(s);
        }
    }
    if ou.is_empty() {
        // Plus une seule des clés de la case : si un dessin voisin tient maintenant son image et que personne d'autre n'y pose de
        // clé, la case n'a plus de dessin à elle ; elle est réunie à la précédente. (Une image laissée vide, elle, reste une case.)
        let tenue = porteurs.iter().all(|&(calque, _)| apres.layers.iter().find(|l| l.id == calque).is_some_and(|l| l.frames.get(f).copied().flatten().is_some()));
        let portee = apres.layers.iter().filter(|l| calque_de_case(l)).any(|l| cle_a(l, f).is_some());
        return if tenue && !portee { Sort::Part } else { Sort::Reste };
    }
    let s = ou[0];
    if s != f && ou.iter().all(|&x| x == s) {
        Sort::Va(s)
    } else {
        Sort::Reste
    }
}

impl Document {
    /// Un mouvement de caméra vient d'être **créé** sur la première case du plan `index` (Anthony, 2026-10-22) : les cases qui la suivent dans
    /// le plan (sa continuité) deviennent **un plan à part**, une coupe juste après la case qui porte le mouvement. Le mouvement ne montre que le
    /// dessin de la première case ; le plan neuf garde les dessins des autres cases tels qu'ils étaient (`split_plan` en fait des copies,
    /// décor compris : une copie indépendante, comme en recochant une coupe) et **la caméra qu'elles montraient avant** (`avant`, les clés
    /// d'avant le mouvement : aucune ou une seule). `finale` : les clés du mouvement, que le premier plan garde. Rend le rang du plan neuf, ou
    /// rien (le plan n'a pas de case de plus : rien n'est changé).
    pub fn scinder_apres_mouvement(&mut self, index: usize, avant: &[CameraKey], finale: Vec<CameraKey>) -> Option<usize> {
        let p = self.panels.get(index)?;
        let (id, at) = (p.id, p.marks.first()?.frame);
        self.panels[index].camera = avant.to_vec();
        let suite = self.split_plan(index, at);
        if let Some(premier) = self.panels.iter_mut().find(|p| p.id == id) {
            premier.camera = finale;
        }
        suite
    }

    /// Comme `scinder_apres_mouvement`, pour un mouvement créé sur la case qui commence à l'image `apres` du plan (partie 142 : une case de
    /// continuité porte son propre mouvement, B vers C) : les cases **derrière elle** deviennent un plan à part, avec la caméra qu'elles
    /// montraient avant (le cadre que la case tenait) ; les cases d'avant et celle du mouvement restent. `finale` : les clés du plan après
    /// le geste ; le premier plan garde celles d'avant la coupe.
    pub fn scinder_apres_mouvement_de(&mut self, index: usize, avant: &[CameraKey], finale: Vec<CameraKey>, apres: usize) -> Option<usize> {
        if apres == 0 {
            return self.scinder_apres_mouvement(index, avant, finale);
        }
        let p = self.panels.get(index)?;
        let id = p.id;
        let at = p.marks.iter().map(|m| m.frame).filter(|&f| f > apres).min()?;
        self.panels[index].camera = avant.to_vec();
        let suite = self.split_plan(index, at);
        if let Some(premier) = self.panels.iter_mut().find(|p| p.id == id) {
            premier.camera = finale.into_iter().filter(|k| k.frame < at).collect();
        }
        suite
    }

    /// Les cases suivent leurs clés (Anthony, 2026-10-24 : « une clé du Storyboard doit rester sa propre case dans les Vignettes »).
    /// Une case (marque d'image) et sa clé de dessin sont la même chose vue de deux côtés ; or le montage et la timeline ne connaissent que
    /// les clés : en glisser une, tirer la limite entre deux expositions (les clés d'après suivent), ou en supprimer une laissait la marque
    /// où elle était, et la case montrait le dessin de sa voisine (deux cases d'une même image : « réunies »). `avant` : les plans tels
    /// qu'ils étaient avant la retouche (le cliché de l'étape d'annulation). Pour chaque plan dont les marques n'ont pas été touchées par
    /// la retouche elle-même (poser, retirer, couper, réunir : elles savent ce qu'elles font) :
    /// - une case dont **toutes** les clés (sur chaque calque de dessin qui en avait une à son image) sont maintenant à une même autre
    ///   image s'y rend ; si les calques ne sont pas d'accord (on n'a glissé que la clé de l'un), elle reste, collée aux autres ;
    /// - une case dont les clés ont été supprimées, une voisine tenant maintenant son image, est réunie à la précédente : ses textes
    ///   s'ajoutent aux siens et les notes de la planche passent à celle-ci (`drop_joint_mark`) ;
    /// - si l'ordre des cases ne tenait plus (clés réordonnées), rien ne bouge.
    ///
    /// Dans l'étape d'annulation de la retouche. Renvoie le nombre de cases déplacées ou réunies.
    pub(crate) fn cases_suivent_les_cles(&mut self, avant: &[Panel]) -> usize {
        let mut touchees = 0;
        for i in 0..self.panels.len() {
            if self.panels[i].marks.is_empty() {
                continue;
            }
            let Some(old) = avant.iter().find(|p| p.id == self.panels[i].id) else { continue };
            let new = &self.panels[i];
            let memes_clefs = old.layers.len() == new.layers.len() && old.layers.iter().zip(&new.layers).all(|(a, b)| a.id == b.id && a.frames == b.frames);
            if old.marks != new.marks || memes_clefs {
                continue;
            }
            let sorts: Vec<Sort> = old
                .marks
                .iter()
                .map(|m| {
                    let porteurs: Vec<(u64, CelId)> = old.layers.iter().filter(|l| calque_de_case(l)).filter_map(|l| cle_a(l, m.frame).map(|c| (l.id, c))).collect();
                    sort_de(&porteurs, new, m.frame)
                })
                .collect();
            if sorts.iter().all(|s| *s == Sort::Reste) {
                continue;
            }
            // Les cases qui restent, à leur nouvelle image : dans l'ordre, après la première image, avant la fin.
            let n = new.frame_count();
            let frames: Vec<usize> = old.marks.iter().zip(&sorts).filter_map(|(m, s)| match s {
                Sort::Reste => Some(m.frame),
                Sort::Va(f) => Some(*f),
                Sort::Part => None,
            }).collect();
            if frames.windows(2).any(|w| w[0] >= w[1]) || frames.iter().any(|&f| f == 0 || f >= n) {
                continue;
            }
            for (m, s) in old.marks.iter().zip(&sorts).rev() {
                if *s == Sort::Part {
                    self.drop_joint_mark(i, m.frame);
                    touchees += 1;
                }
            }
            for (m, s) in old.marks.iter().zip(&sorts) {
                if let Sort::Va(f) = s {
                    if let Some(mark) = self.panels[i].marks.iter_mut().find(|x| x.id == m.id) {
                        mark.frame = *f;
                        touchees += 1;
                    }
                }
            }
            self.panels[i].marks.sort_by_key(|m| m.frame);
        }
        touchees
    }

    /// `cles_de_case` pour une case **rendue continue** de la précédente (Anthony, 2026-10-30 : « elle doit s'afficher immédiatement à partir du
    /// moment où on assigne la case comme continue ») : une case qui n'a rien dessiné sur un calque (pas de dessin, ou un dessin sans trait) reçoit
    /// **une copie du dessin de la case d'avant**, pour que la vignette le montre tout de suite. Une case qui a déjà du dessin à elle le garde
    /// (rien n'est perdu). Renvoie le nombre de clés posées ou remplacées.
    pub fn cles_de_case_heritees(&mut self, index: usize, frame: usize) -> usize {
        let mut posees = self.cles_de_case(index, frame);
        let Some(p) = self.panels.get(index) else { return posees };
        let n = p.frame_count();
        if frame == 0 || frame >= n {
            return posees;
        }
        for li in 0..self.panels[index].layers.len() {
            let l = &self.panels[index].layers[li];
            if !calque_de_case(l) {
                continue;
            }
            let (Some(a), Some(ici)) = (l.cel_at(frame - 1), l.cel_at(frame)) else { continue };
            if a == ici || !self.cel_a_du_trait(a) || self.cel_a_du_trait(ici) {
                continue;
            }
            let copie = self.duplicate_cel(a);
            let calque = &mut self.panels[index].layers[li];
            let (debut, fin) = calque.run_bounds(frame);
            for f in debut.max(frame)..fin {
                calque.frames[f] = Some(copie);
            }
            crate::calquecouleur::noter_copie(self, index, li, a, copie);
            posees += 1;
        }
        posees
    }

    /// Le dessin `id` a-t-il quelque chose de tracé (des pixels, ou des objets vectoriels) ?
    pub(crate) fn cel_a_du_trait(&self, id: CelId) -> bool {
        self.cels.get(&id).is_some_and(|c| !c.raster.is_blank() || c.vector.as_ref().is_some_and(|v| !v.elements.is_empty()))
    }

    /// Pose, à l'image `frame` du plan `index`, la clé de la case qui commence là sur chacun de ses calques de dessin (voir plus haut).
    /// Renvoie le nombre de clés posées.
    pub fn cles_de_case(&mut self, index: usize, frame: usize) -> usize {
        let Some(p) = self.panels.get(index) else { return 0 };
        let n = p.frame_count();
        if frame == 0 || frame >= n {
            return 0;
        }
        let mut posees = 0;
        for li in 0..self.panels[index].layers.len() {
            let l = &self.panels[index].layers[li];
            if l.folder || l.effet.is_some() || l.video.is_some() || l.decor || l.annotation || l.couleur.is_some() || l.scene_wide.is_some() {
                continue;
            }
            let (avant, ici, genre) = (l.cel_at(frame - 1), l.cel_at(frame), l.kind);
            match (avant, ici) {
                // Le dessin de la case d'avant est tenu jusqu'ici : la nouvelle case en a une copie, jusqu'à la fin de son exposition.
                (Some(a), Some(c)) if a == c => {
                    let copie = self.duplicate_cel(c);
                    let calque = &mut self.panels[index].layers[li];
                    let (_, fin) = calque.run_bounds(frame);
                    for f in frame..fin {
                        calque.frames[f] = Some(copie);
                    }
                    crate::calquecouleur::noter_copie(self, index, li, c, copie);
                    posees += 1;
                }
                // Le calque avait un dessin, pas la nouvelle case : un dessin vide, à elle.
                (Some(_), None) => {
                    let vide = self.new_cel(genre);
                    let calque = &mut self.panels[index].layers[li];
                    let fin = calque.empty_run_end(frame, n);
                    for f in frame..fin {
                        calque.set(f, Some(vide));
                    }
                    posees += 1;
                }
                _ => {}
            }
        }
        posees
    }
}
