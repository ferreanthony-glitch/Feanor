//! La sélection agit sur tout (Anthony, 2026-11-10, partie 132) : deux réglages du panneau Calques et de la timeline qui ne
//! visaient que le calque courant (ou supprimaient les clés) agissent maintenant sur tout ce qui est choisi.
//!
//! - **L'opacité du calque** (le champ du panneau Calques, glissé ou tapé) : tous les calques choisis prennent la même valeur
//!   (valeur absolue, comme les logiciels de dessin courants). Chacun à sa façon : sa valeur fixe, ou la clé d'opacité de l'image
//!   courante s'il en a (`Layer::set_opacity_at`). Un glissé fait une seule étape (`App::edited`). Comme pour un seul calque, un
//!   calque verrouillé change aussi (le verrou protège le dessin, pas les réglages du calque).
//! - **Suppr sur des clés choisies** (la timeline ou le montage a la main, rien n'est choisi sur le dessin) : le dessin de chaque
//!   clé est vidé, la clé reste avec sa durée. Un dessin tenu sur plusieurs clés n'est vidé qu'une fois. Calques verrouillés ou
//!   masqués laissés de côté. Une seule étape. Supprimer vraiment les clés : le menu de la piste (« Supprimer les clés… »).

use crate::app::App;
use crate::document::CelId;
use crate::history::Edit;
use crate::raster::Tile;

impl App {
    /// L'opacité `v` (0 à 1) à l'image courante, sur le calque `li` et, s'il en est, sur tous les calques choisis. Rend le nombre
    /// de calques changés.
    pub(crate) fn regler_opacite(&mut self, li: usize, v: f32) -> usize {
        let f = self.cur_frame;
        let choisis = self.chosen_layers();
        let lis: Vec<usize> = if choisis.len() >= 2 && choisis.contains(&li) { choisis } else { vec![li] };
        for &l in &lis {
            if let Some(layer) = self.layers_mut().get_mut(l) {
                layer.set_opacity_at(f, v);
            }
        }
        if lis.len() >= 2 {
            self.status = format!("Opacité à {:.0} % sur {} calques.", v * 100.0, lis.len());
        }
        lis.len()
    }

    /// Une sélection est posée sur le dessin (traits, zone, plusieurs calques) : Suppr l'efface, elle.
    pub(crate) fn selection_sur_le_dessin(&self) -> bool {
        self.multi.sel.is_some() || self.bsel.is_some() || self.vsel.as_ref().is_some_and(|s| !s.items.is_empty())
    }

    /// Suppr sur les clés choisies : leurs dessins sont vidés, les clés restent. Une étape, tout ou rien.
    pub(crate) fn vider_cles(&mut self) {
        // (Aucune clé choisie : celle de l'image courante du calque courant, comme le faisait Suppr.)
        let choix: Vec<crate::montagekeys::KeyRef> = if self.key_sel.is_empty() {
            self.key_ref(self.cur_panel, self.cur_layer, self.cur_frame).into_iter().collect()
        } else {
            self.key_sel.clone()
        };
        self.vider_dessins_des_cles(&choix, true);
    }

    /// « Vider le dessin » avec plusieurs calques choisis : le dessin de chacun à l'image courante (`dessin.vider`).
    pub(crate) fn vider_dessins_choisis(&mut self) {
        let (pi, f) = (self.cur_panel, self.cur_frame);
        let choix: Vec<crate::montagekeys::KeyRef> = self.chosen_layers().into_iter().filter_map(|li| self.key_ref(pi, li, f)).collect();
        self.vider_dessins_des_cles(&choix, false);
    }

    /// Vide les dessins des clés `choix` (chacun une fois), en une étape. `cles` : le message parle des clés (Suppr), sinon des
    /// calques (Vider le dessin).
    fn vider_dessins_des_cles(&mut self, choix: &[crate::montagekeys::KeyRef], parle_cles: bool) {
        self.end_stroke();
        if self.content_refuses() {
            return;
        }
        let mut cels: Vec<CelId> = Vec::new();
        let mut calques: Vec<u64> = Vec::new();
        let mut cles = 0;
        let mut laisses: Vec<String> = Vec::new();
        for k in choix {
            let Some(p) = self.doc.panels.iter().find(|p| p.id == k.panel) else { continue };
            let Some(li) = p.layers.iter().position(|l| l.id == k.layer) else { continue };
            let l = &p.layers[li];
            let raison = if l.folder || l.video.is_some() {
                Some("pas de dessin")
            } else if l.locked || crate::folders::locked_by_folder(&p.layers, li) {
                Some("verrouillé")
            } else if crate::folders::effect(&p.layers, li).is_none() {
                Some("masqué")
            } else {
                None
            };
            if let Some(r) = raison {
                let t = format!("{} ({r})", l.name);
                if !laisses.contains(&t) {
                    laisses.push(t);
                }
                continue;
            }
            let Some(cel) = l.cel_at(k.start) else { continue };
            cles += 1;
            if !calques.contains(&l.id) {
                calques.push(l.id);
            }
            if !cels.contains(&cel) {
                cels.push(cel);
            }
        }
        let laisses = (!laisses.is_empty()).then(|| format!(" Laissé{} de côté : {}.", if laisses.len() > 1 { "s" } else { "" }, laisses.join(", ")));
        if cels.is_empty() {
            self.status = format!("Aucune clé à vider.{}", laisses.unwrap_or_default());
            return;
        }
        let mut edits: Vec<Edit> = Vec::new();
        for &cel in &cels {
            let Some(c) = self.doc.cels.get_mut(&cel) else { continue };
            if let Some(d) = c.vector.as_mut() {
                if !d.elements.is_empty() {
                    let before = std::mem::take(d);
                    c.version += 1;
                    edits.push(Edit::Vector { cel, drawing: before });
                    self.refresh_cel(cel);
                }
                continue;
            }
            let mut tiles: Vec<(usize, Option<Tile>)> = c.raster.tiles.iter_mut().enumerate().filter(|(_, t)| t.is_some()).map(|(i, t)| (i, t.take())).collect();
            // (Toile infinie : ce qui est hors de la feuille part aussi, comme « Vider le dessin ».)
            let dehors: Vec<usize> = c.raster.outer.keys().map(|&(tx, ty)| crate::raster::outer_id(tx, ty)).collect();
            for id in dehors {
                tiles.push((id, c.raster.replace_slot(id, None)));
            }
            if !tiles.is_empty() {
                c.version += 1;
                edits.push(Edit::Tiles { cel, tiles });
            }
        }
        self.vsel = None;
        self.renderer.invalidate_all();
        let (n, m) = (cles, calques.len());
        if !edits.is_empty() {
            let avant = self.history.context.replace(if parle_cles { "Vider les clés" } else { "Vider le dessin" }.to_string());
            self.history.push(Edit::Group(edits));
            self.history.context = avant;
            self.modified = true;
        }
        let s = |k: usize| if k > 1 { "s" } else { "" };
        self.status = if parle_cles {
            format!("{n} clé{} vidée{} sur {m} calque{} (les clés restent, avec leur durée ; pour les supprimer : clic droit sur la piste).{}", s(n), s(n), s(m), laisses.unwrap_or_default())
        } else {
            format!("Dessin vidé sur {m} calque{} à cette image.{}", s(m), laisses.unwrap_or_default())
        };
    }

    /// Les calques choisis qu'un réglage de calque peut changer, et ceux laissés de côté (texte prêt pour la barre d'état).
    /// `dessin` : le réglage touche à ce qu'on voit du calque (retourner) — verrouillés, position verrouillée et masqués sont
    /// laissés de côté ; sinon (une clé d'opacité) seuls les dossiers et les calques vidéo le sont, comme pour un seul calque.
    fn calques_reglables(&self, dessin: bool) -> (Vec<usize>, Option<String>) {
        let layers = self.layers();
        let mut ok = Vec::new();
        let mut laisses = Vec::new();
        for li in self.chosen_layers() {
            let l = &layers[li];
            let raison = if l.folder {
                Some("dossier")
            } else if l.video.is_some() {
                Some("vidéo")
            } else if dessin && (l.locked || l.position_locked || crate::folders::locked_by_folder(layers, li)) {
                Some("verrouillé")
            } else if dessin && crate::folders::effect(layers, li).is_none() {
                Some("masqué")
            } else if dessin && l.trajectoire.is_some() {
                Some("à trajectoire")
            } else {
                None
            };
            match raison {
                Some(r) => laisses.push(format!("{} ({r})", l.name)),
                None => ok.push(li),
            }
        }
        let texte = (!laisses.is_empty()).then(|| format!(" Laissé{} de côté : {}.", if laisses.len() > 1 { "s" } else { "" }, laisses.join(", ")));
        (ok, texte)
    }

    /// « Retourner le calque » avec plusieurs calques choisis : tous ensemble, comme un seul objet, autour du milieu de leur
    /// boîte commune sur la feuille (comme la Sélection et Transformer). Une étape.
    pub(crate) fn retourner_calques_choisis(&mut self, horizontal: bool) {
        if self.content_refuses() {
            return;
        }
        let (lis, laisses) = self.calques_reglables(true);
        if lis.is_empty() {
            self.status = format!("Retourner : aucun des calques choisis ne peut l'être.{}", laisses.unwrap_or_default());
            return;
        }
        let (pi, f) = (self.cur_panel, self.cur_frame);
        let mut boite = eframe::egui::Rect::NOTHING;
        for &li in &lis {
            if let (Some(b), Some(m)) = (self.layer_content_box(li, f), crate::multisel::layer_to_stage(&self.doc, pi, f, li)) {
                boite = boite.union(m.apply_rect(b));
            }
        }
        let centre = if boite.is_positive() { boite.center() } else { self.doc.frame_rect().center() };
        let pivot = self.doc.stage_center();
        self.end_stroke();
        self.record_step();
        let miroir = |t: crate::document::LayerTransform| {
            if horizontal {
                crate::flip::mirror_transform_x(t, pivot, centre.x)
            } else {
                crate::flip::mirror_transform_y(t, pivot, centre.y)
            }
        };
        for &li in &lis {
            let l = &mut self.layers_mut()[li];
            l.transform = miroir(l.transform);
            for k in &mut l.pos_keys {
                k.t = miroir(k.t);
            }
        }
        self.renderer.invalidate_all();
        let n = lis.len();
        self.status = format!("{n} calques retournés ensemble {} (leurs dessins ne changent pas).{}", if horizontal { "de gauche à droite" } else { "de haut en bas" }, laisses.unwrap_or_default());
    }

    /// « Poser une clé d'opacité » avec plusieurs calques choisis : chacun à l'image courante, avec son opacité de cette image.
    pub(crate) fn cle_opacite_choisis(&mut self) {
        if self.content_refuses() {
            return;
        }
        let (lis, laisses) = self.calques_reglables(false);
        if lis.is_empty() {
            self.status = format!("Clé d'opacité : aucun des calques choisis ne peut en recevoir.{}", laisses.unwrap_or_default());
            return;
        }
        let f = self.cur_frame;
        self.end_stroke();
        self.named_step("Poser une clé d'opacité", |app| app.record_step());
        for &li in &lis {
            let v = self.layers()[li].opacity_at(f);
            self.layers_mut()[li].put_opacity_key(f, v);
        }
        self.renderer.invalidate_params();
        self.modified = true;
        let n = lis.len();
        self.status = format!("Clé d'opacité posée à l'image {} sur {n} calques.{}", self.frame_label(f), laisses.unwrap_or_default());
    }
}
