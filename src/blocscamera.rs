//! Les blocs de la piste Caméra de la timeline (partie 170, Anthony, 2026-11-18 : « des préréglages d'animation de caméra à
//! intégrer directement dans la timeline »).
//!
//! Un bloc **est** un effet de caméra vivant du plan (`Panel::effets_camera`, `camfx::Vivant`) : son préréglage, son image de
//! départ, sa durée et ses réglages. Il n'invente aucune notion nouvelle : la caméra du plan est ses clés, avec ses effets
//! vivants cuits dedans l'un après l'autre (`camfx::cuire_tous`). Un plan en porte autant qu'on veut : un tremblement posé
//! sur un zoom tremble autour du zoom.
//!
//! Dans la timeline, les blocs sont sous la rangée des clés de caméra, en jaune (la couleur de la caméra), un par rang
//! quand ils se chevauchent. On les prend à la souris :
//! - **le corps** : le bloc glisse (son image de départ) ;
//! - **un bord** : le bloc s'allonge ou raccourcit (sa durée), le bord d'en face ne bouge pas ;
//! - **un clic** : le bloc est choisi (contour), la section « Effets de caméra » du panneau Caméra règle celui-là, et Suppr
//!   l'enlève ;
//! - **le clic droit** : son menu (retirer, convertir en clés).
//!
//! Chaque geste est une étape d'annulation. Aucune boîte de dialogue (la règle d'Anthony du 13/11/2026).
//!
//! Un préréglage glissé depuis le panneau Préréglages (`panneauprereglages.rs`, charge `PresetDrag`) se lâche sur la rangée
//! de la caméra ou sur celle des blocs : un nouveau bloc commence à l'image visée. Un clic sur le préréglage le pose à la tête
//! de lecture.
//!
//! **Le chemin de la caméra** (`Panel::trajectoire_camera`, partie 172) a sa barre sous les blocs, en contour : de son départ
//! à sa fin. Une trajectoire toute faite (`trajformes`, charge `FormeDrag`) s'y lâche. Le corps glisse le chemin dans le temps,
//! un bord recale son départ ou sa fin (le temps des marques suit) ; le clic droit le retire, ou dit si la caméra garde son
//! sujet dans le cadre. Le chemin lui-même se retouche dans la vue Espace 3D, avec l'outil Trajectoire.

use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, StrokeKind};

use crate::app::App;
use crate::camfx::{Params, Preset, Vivant};
use crate::theme;
use crate::trajformes::FormeDrag;

/// Hauteur d'un rang de blocs dans la timeline.
pub(crate) const BLOC_H: f32 = 17.0;
/// Largeur, en points, de la prise d'un bord de bloc.
const BORD: f32 = 6.0;

/// Un préréglage de caméra qu'on glisse du panneau Préréglages vers la timeline (la charge du glisser-déposer d'egui).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PresetDrag {
    pub preset: Preset,
}

/// Ce que l'on tient d'un bloc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Prise {
    /// Le corps : le bloc glisse.
    Corps,
    /// Le bord gauche : l'image de départ bouge, la fin reste.
    Debut,
    /// Le bord droit : la durée change, le départ reste.
    Fin,
}

/// Un bloc pris à la souris.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Glisse {
    pub panel: u64,
    pub rang: usize,
    pub prise: Prise,
    pub start0: usize,
    pub duree0: usize,
    pub x0: f32,
}

/// Le chemin de la caméra pris à la souris (partie 172) : sa prise, son départ et sa fin à l'appui.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GlisseChemin {
    pub panel: u64,
    pub prise: Prise,
    pub depart0: usize,
    pub fin0: usize,
    pub x0: f32,
}

/// L'état des blocs dans l'application.
#[derive(Clone, Debug, Default)]
pub(crate) struct Etat {
    /// Le bloc choisi : (id du plan, rang dans `effets_camera`).
    pub choisi: Option<(u64, usize)>,
    /// Le bloc qu'on fait glisser.
    pub glisse: Option<Glisse>,
    /// Le chemin de la caméra qu'on fait glisser.
    pub chemin: Option<GlisseChemin>,
}

/// Le rang d'affichage de chaque bloc (dans l'ordre de la liste) et le nombre de rangs : un bloc va dans le premier rang
/// où il ne chevauche aucun bloc déjà posé.
pub(crate) fn rangs(effets: &[Vivant]) -> (Vec<usize>, usize) {
    // (La liste n'est pas triée par départ : chaque rang garde tous ses blocs, et l'on vérifie le chevauchement réel.)
    let mut occupes: Vec<Vec<(usize, usize)>> = Vec::new();
    let mut out = Vec::with_capacity(effets.len());
    for v in effets {
        let (a, b) = (v.start, v.start + v.params.duration.max(1));
        let r = occupes.iter().position(|occ| occ.iter().all(|&(x, y)| b <= x || y <= a)).unwrap_or_else(|| {
            occupes.push(Vec::new());
            occupes.len() - 1
        });
        occupes[r].push((a, b));
        out.push(r);
    }
    (out, occupes.len())
}

/// Le départ d'un nouvel effet de `duree` images visé à l'image `frame` d'un plan de `len` images : là où l'on vise, ramené
/// en arrière juste assez pour que l'effet tienne dans le plan quand il le peut.
pub(crate) fn depart_qui_tient(frame: usize, duree: usize, len: usize) -> usize {
    let frame = frame.min(len.saturating_sub(1));
    if duree <= len && frame + duree > len {
        len - duree
    } else {
        frame
    }
}

impl App {
    /// Le rang du bloc choisi dans le plan courant, s'il existe encore.
    pub(crate) fn bloc_camera_choisi(&self) -> Option<usize> {
        let (pid, i) = self.blocs.choisi?;
        let p = self.doc.panels.get(self.cur_panel)?;
        (p.id == pid && i < p.effets_camera.len()).then_some(i)
    }

    /// Choisit le bloc `rang` du plan courant (ou aucun) : la section « Effets de caméra » le règle, Suppr l'enlève. Les clés
    /// de caméra choisies sont lâchées (Suppr ne vise qu'une chose à la fois).
    pub(crate) fn choisir_bloc_camera(&mut self, rang: Option<usize>) {
        let pid = self.doc.panels.get(self.cur_panel).map(|p| p.id);
        self.blocs.choisi = match (rang, pid) {
            (Some(i), Some(pid)) => Some((pid, i)),
            _ => None,
        };
        if rang.is_some() {
            self.cam_key_sel.clear();
            self.key_focus = true;
            if let Some(v) = self.effet_vivant() {
                self.cam_fx = v.params;
            }
        }
    }

    /// Suppr vise le bloc choisi : le dernier clic était dans la timeline (pas dans le dessin) et aucune clé de caméra n'est
    /// choisie.
    pub(crate) fn bloc_camera_a_le_focus(&self) -> bool {
        self.key_focus && self.bloc_camera_choisi().is_some() && self.cam_targets().is_empty()
    }

    /// Enlève le bloc choisi (une étape d'annulation). Les clés de caméra du plan ne changent pas.
    pub(crate) fn supprimer_bloc_camera(&mut self) {
        let Some(i) = self.bloc_camera_choisi() else { return };
        let pi = self.cur_panel;
        let nom = self.doc.panels[pi].effets_camera[i].params.preset.label();
        self.record_named_step(&format!("Retirer l'effet de caméra : {nom}"));
        self.doc.panels[pi].effets_camera.remove(i);
        self.blocs.choisi = None;
        self.modified = true;
        self.status = format!("{nom} retiré. Les clés de caméra du plan n'ont pas changé ; Ctrl+Z le remet.");
    }

    /// Pose un nouvel effet vivant `preset` dans le plan courant, à partir de l'image `frame` (ramenée pour qu'il tienne dans
    /// le plan quand il le peut), avec les derniers réglages utilisés (intensité, graine) et la durée du préréglage. Il est
    /// choisi. Une étape d'annulation. Rend son rang.
    pub(crate) fn ajouter_effet_camera(&mut self, preset: Preset, frame: usize) -> Option<usize> {
        let fps = self.doc.fps;
        let pi = self.cur_panel;
        let len = self.doc.panels.get(pi)?.frame_count().max(1);
        let params = self.cam_fx.with_preset(preset, fps);
        let start = depart_qui_tient(frame, params.duration, len);
        let v = Vivant { params, start }.sanitized(fps);
        self.end_stroke();
        self.record_named_step(&format!("Effet de caméra : {}", preset.label()));
        let p = &mut self.doc.panels[pi];
        p.effets_camera.push(v);
        // (Un effet du lot 3 déplace la caméra dans l'espace : le plan retient qu'elle est libre, partie 172.)
        p.camera_libre |= !p.espace && p.a_un_effet_dans_l_espace();
        let rang = p.effets_camera.len() - 1;
        self.modified = true;
        self.choisir_bloc_camera(Some(rang));
        let n = self.doc.panels[pi].effets_camera.len();
        self.status = format!(
            "{} posé des images {} à {}, vivant, sans clé{}. Tire ses bords dans la timeline pour sa durée ; Suppr l'enlève.",
            preset.label(),
            self.frame_label(v.start),
            self.frame_label(v.last_frame(len)),
            if n > 1 { format!(" ({n} effets sur ce plan, ils s'ajoutent)") } else { String::new() }
        );
        Some(rang)
    }

    /// Pose `preset` à la tête de lecture du plan courant (un clic dans le panneau Préréglages).
    pub(crate) fn ajouter_effet_camera_a_la_tete(&mut self, preset: Preset) {
        let f = self.cur_frame;
        self.ajouter_effet_camera(preset, f);
    }

    /// « Convertir en clés » quand le plan a des effets vivants : ils deviennent tous des clés de caméra, exactement celles
    /// qu'ils montraient (`Document::cuire_effet_vivant`), puis ils s'en vont. Les clés nouvelles ou changées sont choisies.
    /// Une étape d'annulation.
    pub(crate) fn convertir_effets_en_cles(&mut self, pi: usize) {
        let Some(p) = self.doc.panels.get(pi) else { return };
        if p.effets_camera.is_empty() {
            return;
        }
        let n = p.effets_camera.len();
        let noms: Vec<&str> = p.effets_camera.iter().map(|v| v.params.preset.label()).collect();
        let nom = if n == 1 { noms[0].to_string() } else { format!("{n} effets") };
        let avant = p.camera.clone();
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        self.doc.cuire_effet_vivant(pi);
        let panel = self.doc.panels[pi].id;
        let nouvelles: Vec<crate::camkeys::CamKeyRef> = self.doc.panels[pi]
            .camera
            .iter()
            .filter(|k| !avant.iter().any(|a| a.frame == k.frame && a.cam == k.cam))
            .map(|k| crate::camkeys::CamKeyRef { panel, frame: k.frame })
            .collect();
        let m = nouvelles.len();
        self.choose_cam_keys(nouvelles);
        self.blocs.choisi = None;
        self.status = format!(
            "{nom} : {m} clé{} de caméra, qui donnent exactement ce que montraient les effets. Elles sont choisies ; Ctrl+Z annule tout.",
            if m > 1 { "s" } else { "" }
        );
        self.commit_as(snap, &format!("Convertir en clés : {nom}"));
    }

    /// Hauteur de la rangée des blocs sous les clés de caméra : un rang par niveau de chevauchement, aucun sans bloc ; un de
    /// plus pendant qu'on glisse un préréglage, pour qu'il ait où se poser.
    pub(crate) fn blocs_camera_h(&self, ctx: &egui::Context) -> f32 {
        let n = self.doc.panels.get(self.cur_panel).map_or(0, |p| rangs(&p.effets_camera).1);
        let chemin = self.doc.panels.get(self.cur_panel).is_some_and(|p| p.trajectoire_camera.is_some());
        let glisse = egui::DragAndDrop::has_payload_of_type::<PresetDrag>(ctx);
        // (Une trajectoire toute faite se pose sur la barre du chemin : elle n'apparaît que le temps du glisser, s'il n'y en a pas.)
        let forme = egui::DragAndDrop::has_payload_of_type::<FormeDrag>(ctx) && !chemin;
        (n + usize::from(glisse) + usize::from(chemin) + usize::from(forme)) as f32 * BLOC_H
    }

    /// La place du chemin de la caméra dans la rangée des blocs (sous les blocs) : de son départ à sa fin (bornée au plan).
    fn rangee_du_chemin(&self, ctx: &egui::Context, band: Rect, left: f32, cell: f32) -> Option<(Rect, usize, usize)> {
        let p = self.doc.panels.get(self.cur_panel)?;
        let t = p.trajectoire_camera.as_ref()?;
        let len = p.frame_count().max(1);
        let (a, b) = (t.depart.min(len - 1), t.fin.unwrap_or(len - 1).max(t.depart));
        // (Pendant qu'on glisse un préréglage, sa place est juste sous les blocs : le chemin descend d'un rang.)
        let glisse = usize::from(egui::DragAndDrop::has_payload_of_type::<PresetDrag>(ctx));
        let y = band.top() + (rangs(&p.effets_camera).1 + glisse) as f32 * BLOC_H;
        Some((Rect::from_min_max(pos2(left + a as f32 * cell, y + 1.0), pos2((left + (b + 1) as f32 * cell).max(left + a as f32 * cell + 3.0), y + BLOC_H - 1.0)), a, b))
    }

    /// Le nom de la rangée des blocs, dans la colonne des noms de la timeline.
    pub(crate) fn blocs_camera_nom(&mut self, ui: &egui::Ui, painter: &egui::Painter, row: Rect) {
        if row.height() < 1.0 {
            return;
        }
        let row = row.shrink2(vec2(0.0, 1.0));
        painter.rect_filled(row, 2.0, theme::rangee_sombre());
        let n = self.panel().effets_camera.len();
        let chemin = if self.panel().trajectoire_camera.is_some() { " et son chemin" } else { "" };
        let label = match n {
            0 if !chemin.is_empty() => "Chemin de la caméra".to_string(),
            0 => "Lâche le préréglage ici".to_string(),
            1 => format!("1 effet vivant{chemin}"),
            _ => format!("{n} effets vivants{chemin}"),
        };
        painter.text(pos2(row.left() + 18.0, row.top() + BLOC_H * 0.5), Align2::LEFT_CENTER, label, FontId::proportional(11.0), ui.visuals().weak_text_color());
        let c = pos2(row.left() + 9.0, row.top() + BLOC_H * 0.5);
        painter.rect_filled(Rect::from_center_size(c, vec2(7.0, 7.0)), 1.0, theme::camera());
    }

    /// La rangée des blocs (`band`, sous la rangée des clés `keys_row`) : les peindre, les prendre, et recevoir un préréglage
    /// glissé. `left` est l'abscisse de l'image 0 du plan, `cell` la largeur d'une image ; `grid` est la réponse de la grille
    /// de la timeline (un clic ailleurs lâche le bloc choisi).
    pub(crate) fn blocs_camera_bande(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, band: Rect, keys_row: Rect, left: f32, cell: f32, grid: &egui::Response) {
        let pi = self.cur_panel;
        let Some(p) = self.doc.panels.get(pi) else { return };
        let pid = p.id;
        let len = p.frame_count().max(1);
        let effets = p.effets_camera.clone();
        let (rgs, _) = rangs(&effets);
        let rect_of = |i: usize| {
            let v = &effets[i];
            let x0 = left + v.start as f32 * cell;
            let x1 = left + (v.start + v.params.duration.max(1)) as f32 * cell;
            let y = band.top() + rgs[i] as f32 * BLOC_H;
            Rect::from_min_max(pos2(x0, y + 1.0), pos2(x1.max(x0 + 3.0), y + BLOC_H - 1.0))
        };
        // Un clic dans la grille, hors des blocs : le bloc choisi est lâché.
        if grid.clicked() || grid.drag_started() {
            self.blocs.choisi = None;
        }
        if band.height() >= 1.0 {
            painter.rect_filled(band.shrink2(vec2(0.0, 1.0)), 0.0, theme::rangee_sombre());
        }
        let resp = ui.interact(band, ui.id().with(("blocs_camera", pid)), Sense::click_and_drag());
        let sous = |p: Pos2| -> Option<(usize, Prise)> {
            (0..effets.len()).rev().find_map(|i| {
                let r = rect_of(i);
                if !r.expand2(vec2(2.0, 0.0)).contains(p) {
                    return None;
                }
                let prise = if r.width() >= 3.0 * BORD && p.x - r.left() < BORD {
                    Prise::Debut
                } else if p.x > r.right() - BORD || r.width() < 3.0 * BORD && p.x > r.center().x {
                    Prise::Fin
                } else {
                    Prise::Corps
                };
                Some((i, prise))
            })
        };
        // Le curseur dit ce que fera la prise.
        if let Some(h) = resp.hover_pos() {
            match self.blocs.glisse.map(|g| g.prise).or_else(|| sous(h).map(|s| s.1)) {
                Some(Prise::Debut | Prise::Fin) => ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal),
                Some(Prise::Corps) => ui.ctx().set_cursor_icon(if self.blocs.glisse.is_some() { CursorIcon::Grabbing } else { CursorIcon::Grab }),
                None => {}
            }
        }
        // Prendre un bloc.
        if resp.drag_started() {
            // (Là où le bouton a été pressé : au début du glisser, le pointeur a déjà bougé d'un seuil.)
            if let Some(p0) = ui.input(|i| i.pointer.press_origin()).or_else(|| resp.interact_pointer_pos()) {
                if let Some((i, prise)) = sous(p0) {
                    let v = effets[i];
                    self.record_named_step(match prise {
                        Prise::Corps => "Déplacer l'effet de caméra",
                        Prise::Debut | Prise::Fin => "Changer la durée de l'effet de caméra",
                    });
                    self.choisir_bloc_camera(Some(i));
                    self.blocs.glisse = Some(Glisse { panel: pid, rang: i, prise, start0: v.start, duree0: v.params.duration.max(1), x0: p0.x });
                }
            }
        }
        if let (Some(g), Some(p)) = (self.blocs.glisse, resp.interact_pointer_pos()) {
            if g.panel == pid && g.rang < effets.len() && resp.dragged() {
                let d = ((p.x - g.x0) / cell).round() as i64;
                let (s0, d0) = (g.start0 as i64, g.duree0 as i64);
                let (start, duree) = match g.prise {
                    Prise::Corps => ((s0 + d).clamp(0, len as i64 - 1), d0),
                    Prise::Debut => {
                        let s = (s0 + d).clamp(0, s0 + d0 - 1);
                        (s, d0 - (s - s0))
                    }
                    Prise::Fin => (s0, (d0 + d).max(1)),
                };
                let fps = self.doc.fps;
                let cible = &mut self.doc.panels[pi].effets_camera[g.rang];
                let next = Vivant { start: start as usize, params: Params { duration: duree as usize, ..cible.params } }.sanitized(fps);
                if next != *cible {
                    *cible = next;
                    self.modified = true;
                    self.status = format!(
                        "{} : des images {} à {} ({} images).",
                        next.params.preset.label(),
                        self.frame_label(next.start),
                        self.frame_label(next.start + next.params.duration),
                        next.params.duration
                    );
                }
            }
        }
        if resp.drag_stopped() {
            self.blocs.glisse = None;
        }
        // Un clic : choisir le bloc, ou poser la tête de lecture sur une place vide.
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                match sous(p) {
                    Some((i, _)) => self.choisir_bloc_camera(Some(i)),
                    None => {
                        self.blocs.choisi = None;
                        let col = ((p.x - left) / cell).floor().max(0.0) as usize;
                        self.set_frame_free(col);
                    }
                }
            }
        }
        // Le clic droit : le menu du bloc.
        if resp.secondary_clicked() {
            if let Some((i, _)) = resp.interact_pointer_pos().and_then(sous) {
                self.choisir_bloc_camera(Some(i));
            }
        }
        if self.bloc_camera_choisi().is_some() {
            resp.context_menu(|ui| {
                if ui.button("Retirer cet effet").on_hover_text("Enlève l'effet (Suppr). Les clés de caméra du plan ne changent pas.").clicked() {
                    self.supprimer_bloc_camera();
                    ui.close_menu();
                }
                if ui.button("Convertir les effets en clés").on_hover_text("Tous les effets vivants du plan deviennent des clés de caméra, avec la même image.").clicked() {
                    self.convertir_effets_en_cles(pi);
                    ui.close_menu();
                }
            });
        }
        // Peindre les blocs (le plan s'arrête à `len` : au-delà, le bloc est pâle, l'effet y est coupé).
        let fin_du_plan = left + len as f32 * cell;
        let choisi = self.bloc_camera_choisi();
        let clip = painter.clip_rect();
        let effets = self.doc.panels.get(pi).map(|p| p.effets_camera.clone()).unwrap_or_default();
        let (rgs, _) = rangs(&effets);
        for (i, v) in effets.iter().enumerate() {
            let x0 = left + v.start as f32 * cell;
            let x1 = (left + (v.start + v.params.duration.max(1)) as f32 * cell).max(x0 + 3.0);
            let y = band.top() + rgs[i] as f32 * BLOC_H;
            let r = Rect::from_min_max(pos2(x0, y + 1.0), pos2(x1, y + BLOC_H - 1.0));
            if r.right() < clip.left() || r.left() > clip.right() {
                continue;
            }
            let dedans = Rect::from_min_max(r.min, pos2(r.right().min(fin_du_plan), r.bottom()));
            if dedans.is_positive() {
                painter.rect_filled(dedans, 2.0, theme::camera());
            }
            if r.right() > fin_du_plan {
                let dehors = Rect::from_min_max(pos2(r.left().max(fin_du_plan), r.top()), r.max);
                painter.rect_filled(dehors, 2.0, theme::camera().gamma_multiply(0.35));
            }
            // Les deux bords, qu'on prend.
            let bord = Color32::from_black_alpha(90);
            painter.line_segment([pos2(r.left() + 1.5, r.top() + 2.0), pos2(r.left() + 1.5, r.bottom() - 2.0)], Stroke::new(2.0_f32, bord));
            painter.line_segment([pos2(r.right() - 1.5, r.top() + 2.0), pos2(r.right() - 1.5, r.bottom() - 2.0)], Stroke::new(2.0_f32, bord));
            if r.width() > 26.0 {
                let txt = painter.with_clip_rect(r.shrink2(vec2(5.0, 0.0)).intersect(clip));
                txt.text(pos2(r.left() + 7.0, r.center().y), Align2::LEFT_CENTER, v.params.preset.label(), FontId::proportional(11.0), Color32::from_rgb(43, 29, 0));
            }
            if choisi == Some(i) {
                painter.rect_stroke(r.expand(1.0), 3.0, Stroke::new(2.0_f32, theme::contour()), StrokeKind::Outside);
            }
        }
        self.chemin_camera_bande(ui, painter, band, left, cell, len);
        self.blocs_camera_depot(ui, painter, band.union(keys_row), band, left, cell, len);
        self.forme_depot(ui, painter, band.union(keys_row), band, left, cell, len);
    }

    /// La barre du chemin de la caméra (partie 172) : la peindre, la prendre (le corps glisse le chemin dans le temps, un bord
    /// recale son départ ou sa fin), et son menu.
    fn chemin_camera_bande(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, band: Rect, left: f32, cell: f32, len: usize) {
        let pi = self.cur_panel;
        let Some(pid) = self.doc.panels.get(pi).map(|p| p.id) else { return };
        let Some((r, a, b)) = self.rangee_du_chemin(ui.ctx(), band, left, cell) else {
            self.blocs.chemin = None;
            return;
        };
        let resp = ui.interact(r.expand2(vec2(2.0, 0.0)), ui.id().with(("chemin_camera", pid)), Sense::click_and_drag());
        let prise_sous = |x: f32| {
            if r.width() >= 3.0 * BORD && x - r.left() < BORD {
                Prise::Debut
            } else if x > r.right() - BORD {
                Prise::Fin
            } else {
                Prise::Corps
            }
        };
        if let Some(h) = resp.hover_pos() {
            match self.blocs.chemin.map(|g| g.prise).unwrap_or_else(|| prise_sous(h.x)) {
                Prise::Debut | Prise::Fin => ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal),
                Prise::Corps => ui.ctx().set_cursor_icon(if self.blocs.chemin.is_some() { CursorIcon::Grabbing } else { CursorIcon::Grab }),
            }
        }
        if resp.drag_started() {
            if let Some(p0) = ui.input(|i| i.pointer.press_origin()).or_else(|| resp.interact_pointer_pos()) {
                let prise = prise_sous(p0.x);
                self.record_named_step(match prise {
                    Prise::Corps => "Déplacer le chemin de la caméra",
                    Prise::Debut | Prise::Fin => "Recaler le chemin de la caméra",
                });
                self.blocs.chemin = Some(GlisseChemin { panel: pid, prise, depart0: a, fin0: b, x0: p0.x });
            }
        }
        if let (Some(g), Some(p), true) = (self.blocs.chemin, resp.interact_pointer_pos(), resp.dragged()) {
            if g.panel == pid {
                let d = ((p.x - g.x0) / cell).round() as i64;
                let (a0, b0, dernier) = (g.depart0 as i64, g.fin0 as i64, len as i64 - 1);
                let (na, nb) = match g.prise {
                    Prise::Corps => {
                        let a = (a0 + d).clamp(0, (dernier - (b0 - a0)).max(0));
                        (a, (a + b0 - a0).min(dernier))
                    }
                    Prise::Debut => ((a0 + d).clamp(0, b0 - 1), b0),
                    Prise::Fin => (a0, (b0 + d).clamp(a0 + 1, dernier.max(a0 + 1))),
                };
                if let Some(t) = self.doc.panels[pi].trajectoire_camera.as_mut() {
                    if (t.depart, t.fin) != (na as usize, Some(nb as usize)) {
                        t.depart = na as usize;
                        t.fin = Some(nb as usize);
                        self.modified = true;
                        self.status = format!("Chemin de la caméra : des images {} à {}.", self.frame_label(na as usize), self.frame_label(nb as usize));
                    }
                }
            }
        }
        if resp.drag_stopped() {
            self.blocs.chemin = None;
        }
        if resp.clicked() {
            self.status = "Chemin de la caméra : glisse-le, ou tire un bord pour recaler son départ ou sa fin ; il se retouche dans la vue Espace 3D, avec l'outil Trajectoire. Clic droit : son menu.".into();
        }
        resp.context_menu(|ui| {
            let vise = self.doc.panels.get(pi).and_then(|p| p.trajectoire_camera.as_ref()).is_some_and(|t| t.vise.is_some());
            let mut garde = vise;
            if ui.checkbox(&mut garde, "La caméra garde son sujet dans le cadre").on_hover_text("Elle tourne en suivant son chemin, pour voir toujours au même endroit le point qu'elle voit au centre du cadre au départ.").changed() {
                self.chemin_camera_garde_le_sujet(garde);
                ui.close_menu();
            }
            if ui.button("Retirer le chemin de la caméra").on_hover_text("La caméra ne suit plus de chemin ; ses clés et ses effets ne changent pas.").clicked() {
                self.record_named_step("Retirer le chemin de la caméra");
                self.doc.panels[pi].trajectoire_camera = None;
                self.modified = true;
                self.status = "Chemin de la caméra retiré ; Ctrl+Z le remet.".into();
                ui.close_menu();
            }
        });
        // La barre : en contour, à la couleur de la caméra.
        let clip = painter.clip_rect();
        if r.right() < clip.left() || r.left() > clip.right() {
            return;
        }
        let couleur = theme::camera();
        painter.rect_filled(r, 2.0, couleur.gamma_multiply(0.22));
        painter.rect_stroke(r, 2.0, Stroke::new(1.5_f32, couleur), StrokeKind::Inside);
        let bord = couleur.gamma_multiply(0.8);
        painter.line_segment([pos2(r.left() + 1.5, r.top() + 2.0), pos2(r.left() + 1.5, r.bottom() - 2.0)], Stroke::new(2.0_f32, bord));
        painter.line_segment([pos2(r.right() - 1.5, r.top() + 2.0), pos2(r.right() - 1.5, r.bottom() - 2.0)], Stroke::new(2.0_f32, bord));
        if r.width() > 26.0 {
            let vise = self.doc.panels.get(pi).and_then(|p| p.trajectoire_camera.as_ref()).is_some_and(|t| t.vise.is_some());
            let txt = painter.with_clip_rect(r.shrink2(vec2(5.0, 0.0)).intersect(clip));
            let nom = if vise { "Chemin de la caméra · garde son sujet" } else { "Chemin de la caméra" };
            txt.text(pos2(r.left() + 7.0, r.center().y), Align2::LEFT_CENTER, format!("{} {nom}", egui_phosphor::regular::PATH), FontId::proportional(11.0), ui.visuals().text_color());
        }
        if resp.hovered() || self.blocs.chemin.is_some() {
            painter.rect_stroke(r.expand(1.0), 3.0, Stroke::new(1.5_f32, theme::contour()), StrokeKind::Outside);
        }
    }

    /// La caméra du chemin du plan courant garde son sujet dans le cadre (`garde`), ou garde son orientation. Le sujet est le
    /// point du plan de référence qu'elle voit au centre du cadre au départ du chemin. Une étape d'annulation.
    pub(crate) fn chemin_camera_garde_le_sujet(&mut self, garde: bool) {
        let pi = self.cur_panel;
        let Some(depart) = self.doc.panels.get(pi).and_then(|p| p.trajectoire_camera.as_ref()).map(|t| t.depart) else { return };
        let vise = garde.then(|| {
            let cam = self.doc.camera_sans_trajectoire(pi, depart);
            let [fw, fh] = self.doc.frame;
            let p = crate::camespace::point_vu(&cam, self.doc.frame, fw as f32 / 2.0, fh as f32 / 2.0);
            let m = crate::espace3d::Monde::of(&self.doc);
            [(p[0] - m.stage[0] / 2.0) * m.k, (p[1] + m.stage[1] / 2.0) * m.k, p[2] * m.k]
        });
        self.record_named_step(if garde { "La caméra garde son sujet" } else { "La caméra garde son orientation" });
        if let Some(t) = self.doc.panels[pi].trajectoire_camera.as_mut() {
            t.vise = vise;
        }
        self.modified = true;
        self.status = if garde {
            "La caméra garde son sujet dans le cadre en suivant son chemin (le point qu'elle voit au centre du cadre au départ).".into()
        } else {
            "La caméra garde son orientation en suivant son chemin.".into()
        };
    }

    /// Une trajectoire toute faite glissée depuis le panneau Préréglages, au-dessus de la rangée de la caméra ou des blocs : un
    /// repère de son chemin là où il se posera ; au lâcher, le chemin de la caméra (partie 172).
    fn forme_depot(&mut self, ui: &egui::Ui, painter: &egui::Painter, zone: Rect, band: Rect, left: f32, cell: f32, len: usize) {
        let ctx = ui.ctx().clone();
        let Some(drag) = egui::DragAndDrop::payload::<FormeDrag>(&ctx) else { return };
        let Some(p) = ctx.pointer_hover_pos() else { return };
        if !zone.contains(p) {
            return;
        }
        let frame = (((p.x - left) / cell).floor().max(0.0) as usize).min(len.saturating_sub(1));
        let duree = ((crate::trajformes::SECONDES * self.doc.fps.max(1) as f32).round() as usize).max(1);
        let fin = (frame + duree).min(len.saturating_sub(1)).max(frame);
        let y = band.top() + rangs(&self.panel().effets_camera).1 as f32 * BLOC_H;
        let r = Rect::from_min_max(pos2(left + frame as f32 * cell, y + 1.0), pos2(left + (fin + 1) as f32 * cell, y + BLOC_H - 1.0));
        painter.rect_filled(r, 2.0, theme::camera().gamma_multiply(0.25));
        painter.rect_stroke(r, 2.0, Stroke::new(1.5_f32, theme::camera()), StrokeKind::Inside);
        painter.text(pos2(r.left() + 7.0, r.center().y), Align2::LEFT_CENTER, drag.forme.label(), FontId::proportional(11.0), ui.visuals().text_color());
        ctx.set_cursor_icon(CursorIcon::Copy);
        if ctx.input(|i| i.pointer.any_released()) {
            if let Some(d) = egui::DragAndDrop::take_payload::<FormeDrag>(&ctx) {
                self.poser_trajectoire_toute_faite(d.forme, frame);
            }
        }
    }

    /// Un préréglage glissé depuis le panneau Préréglages, au-dessus de la rangée de la caméra ou des blocs : un repère de sa
    /// durée là où il se posera ; au lâcher, le nouveau bloc.
    fn blocs_camera_depot(&mut self, ui: &egui::Ui, painter: &egui::Painter, zone: Rect, band: Rect, left: f32, cell: f32, len: usize) {
        let ctx = ui.ctx().clone();
        let Some(drag) = egui::DragAndDrop::payload::<PresetDrag>(&ctx) else { return };
        let Some(p) = ctx.pointer_hover_pos() else { return };
        if !zone.contains(p) {
            return;
        }
        let frame = ((p.x - left) / cell).floor().max(0.0) as usize;
        let duree = Params::new(drag.preset, self.doc.fps).duration.max(1);
        let start = depart_qui_tient(frame, duree, len);
        let n = rangs(&self.panel().effets_camera).1;
        let y = band.top() + n as f32 * BLOC_H;
        let r = Rect::from_min_max(pos2(left + start as f32 * cell, y + 1.0), pos2(left + (start + duree) as f32 * cell, y + BLOC_H - 1.0));
        painter.rect_filled(r, 2.0, theme::camera().gamma_multiply(0.35));
        painter.rect_stroke(r, 2.0, Stroke::new(1.5_f32, theme::camera()), StrokeKind::Inside);
        ctx.set_cursor_icon(CursorIcon::Copy);
        if ctx.input(|i| i.pointer.any_released()) {
            if let Some(d) = egui::DragAndDrop::take_payload::<PresetDrag>(&ctx) {
                self.ajouter_effet_camera(d.preset, frame);
            }
        }
    }

    /// La liste des effets vivants du plan courant, dans la section « Effets de caméra » du panneau Caméra : chacun se choisit
    /// d'un clic (c'est lui que la section règle), et « Ajouter » en pose un de plus à la tête de lecture.
    pub(crate) fn liste_des_effets_ui(&mut self, ui: &mut egui::Ui) {
        let effets = self.panel().effets_camera.clone();
        let len = self.frame_count();
        let courant = self.rang_effet_vivant();
        if effets.len() >= 2 {
            ui.label(egui::RichText::new(format!("{} effets vivants sur ce plan, ils s'ajoutent dans cet ordre :", effets.len())).small().weak());
            for (i, v) in effets.iter().enumerate() {
                let texte = format!("{}  ·  {} à {}", v.params.preset.label(), self.frame_label(v.start.min(len.saturating_sub(1))), self.frame_label(v.last_frame(len)));
                if ui.selectable_label(courant == Some(i), texte).on_hover_text("Choisir cet effet : la section le règle. Dans la timeline, son bloc a un contour.").clicked() {
                    self.choisir_bloc_camera(Some(i));
                }
            }
        }
        let mut ajoute = None;
        ui.menu_button(format!("{} Ajouter un effet", egui_phosphor::regular::PLUS), |ui| {
            // (Une famille par sous-menu, comme le panneau Préréglages : partie 171.)
            for famille in crate::camfx::Famille::ALL {
                ui.menu_button(famille.label(), |ui| {
                    for pr in famille.presets() {
                        if ui.button(pr.label()).on_hover_text(pr.note()).clicked() {
                            ajoute = Some(pr);
                            ui.close_menu();
                        }
                    }
                });
            }
        })
        .response
        .on_hover_text("Pose un effet de plus à la tête de lecture, sans toucher aux autres. Le panneau Préréglages les montre en mouvement, à glisser sur la timeline.");
        if let Some(pr) = ajoute {
            self.ajouter_effet_camera_a_la_tete(pr);
        }
    }
}
