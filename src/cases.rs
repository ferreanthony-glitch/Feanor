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
//!   `sync_couleurs`) et **le calque Décor** (un seul dessin pour toute la continuité, `decor.rs`) ne reçoivent rien.
//!
//! Toujours dans l'étape d'annulation de ce qui fait la case : une annulation défait la case et ses clés.

use crate::camera::CameraKey;
use crate::document::Document;

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
            if l.folder || l.effet.is_some() || l.video.is_some() || l.decor || l.couleur.is_some() || l.scene_wide.is_some() {
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
