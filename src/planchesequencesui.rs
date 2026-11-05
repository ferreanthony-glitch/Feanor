//! La planche par séquences, côté interface (Anthony, 2026-10-30) : les titres au-dessus des séquences (double-clic : renommer), le bloc
//! « Séquences » du panneau Planche (cases à cocher, glisser pour l'ordre, trois raccourcis), et les commandes qui créent, coupent et renomment
//! des séquences depuis les vignettes. Le modèle et la mise en page : `planchesequences.rs`.
//!
//! Créer, couper, renommer une séquence sont des opérations du film (une étape d'annulation chacune, nommée). Choisir ce qui s'affiche, et
//! dans quel ordre, n'en est pas une : c'est un affichage (`Annotations::affichage`), retenu dans le projet mais que « annuler » ne touche pas.

use eframe::egui::{self, pos2, vec2, Color32, FontId, Key, Rect, RichText, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::board::BoardLayout;
use crate::boardui::Xf;

/// Le titre d'une séquence en cours de renommage, sur la planche.
pub(crate) struct TitreEdit {
    pub sequence: u64,
    pub buffer: String,
    pub focused: bool,
    pub had_focus: bool,
    /// Le nom s'écrit dans une petite fenêtre (le montage, la liste du panneau, ou un titre que la planche ne montre pas) et non sur le
    /// titre de la planche.
    pub fenetre: bool,
}

/// Longueur la plus grande du nom d'une séquence (un intitulé de scénario de 200 signes tient, avec son numéro).
pub(crate) const NOM_MAX: usize = 200;

impl App {
    // ------------------------------------------------------------------ ce qu'on affiche

    /// L'affichage enregistré, remis dans sa forme la plus courte : « toutes, dans l'ordre du film » ne s'écrit pas.
    fn affichage_ecrire(&mut self, liste: Vec<(u64, bool)>) {
        let film = self.doc.sequence_order();
        let par_defaut: Vec<(u64, bool)> = film.iter().map(|&q| (q, true)).collect();
        let nouveau = if liste == par_defaut { Vec::new() } else { liste };
        if self.doc.annotations.affichage.ordre != nouveau {
            self.doc.annotations.affichage.ordre = nouveau;
            self.modified = true;
        }
    }

    /// Montre ou cache la séquence `id`. La planche garde toujours une séquence au moins.
    pub(crate) fn affichage_basculer(&mut self, id: u64) {
        let mut liste = self.doc.annotations.affichage.liste(&self.doc);
        let Some(i) = liste.iter().position(|e| e.0 == id) else { return };
        if liste[i].1 && liste.iter().filter(|e| e.1).count() <= 1 {
            self.status = "La planche montre toujours une séquence au moins.".into();
            return;
        }
        liste[i].1 = !liste[i].1;
        let on = liste[i].1;
        self.affichage_ecrire(liste);
        self.status = format!("{} {}.", self.doc.sequence_label(id), if on { "montrée sur la planche" } else { "cachée de la planche (ses plans sont toujours dans le film)" });
    }

    /// Pose la séquence `id` au rang `vers` de la liste d'affichage (l'ordre du film ne change pas).
    pub(crate) fn affichage_deplacer(&mut self, id: u64, vers: usize) {
        let mut liste = self.doc.annotations.affichage.liste(&self.doc);
        let Some(de) = liste.iter().position(|e| e.0 == id) else { return };
        let vers = vers.min(liste.len() - 1);
        if de == vers {
            return;
        }
        let e = liste.remove(de);
        liste.insert(vers, e);
        self.affichage_ecrire(liste);
        self.status = format!("{} déplacée sur la planche (l'ordre du film ne change pas).", self.doc.sequence_label(id));
    }

    /// « Toutes » : toutes les séquences sont montrées, dans l'ordre où la planche les range.
    pub(crate) fn affichage_toutes(&mut self) {
        let liste: Vec<(u64, bool)> = self.doc.annotations.affichage.liste(&self.doc).into_iter().map(|e| (e.0, true)).collect();
        self.affichage_ecrire(liste);
        self.status = "La planche montre toutes les séquences.".into();
    }

    /// « Aucune autre » : seule la séquence du plan courant reste montrée.
    pub(crate) fn affichage_aucune_autre(&mut self) {
        let courante = self.doc.sequence_of(self.cur_panel);
        let liste: Vec<(u64, bool)> = self.doc.annotations.affichage.liste(&self.doc).into_iter().map(|e| (e.0, e.0 == courante)).collect();
        self.affichage_ecrire(liste);
        self.status = format!("La planche ne montre que {}.", self.doc.sequence_label(courante));
    }

    /// « Seulement la séquence courante » : comme « Aucune autre », puis la vue se cale sur elle.
    pub(crate) fn affichage_courante_seule(&mut self) {
        self.affichage_aucune_autre();
        self.board_fit();
    }

    /// Remet les séquences dans l'ordre du film (toutes montrées ou non comme avant).
    pub(crate) fn affichage_ordre_du_film(&mut self) {
        let film = self.doc.sequence_order();
        let avant = self.doc.annotations.affichage.liste(&self.doc);
        let liste: Vec<(u64, bool)> = film.iter().map(|&q| (q, avant.iter().find(|e| e.0 == q).is_none_or(|e| e.1))).collect();
        self.affichage_ecrire(liste);
        self.status = "La planche range les séquences dans l'ordre du film.".into();
    }

    /// Fait défiler la planche jusqu'au titre de la séquence `id` (s'il est montré).
    pub(crate) fn planche_aller_a_la_sequence(&mut self, id: u64) {
        let layout = self.board_layout();
        let Some(t) = layout.groupes.iter().find(|g| g.sequence == id).and_then(|g| g.titre) else { return };
        if self.board_zoom <= 0.0 {
            return;
        }
        self.board_pan.y = 10.0 - t.top() * self.board_zoom;
    }

    // ------------------------------------------------------------------ créer, couper, renommer

    /// « Nouvelle séquence » depuis les vignettes : une séquence neuve, avec un plan vide, à la fin du film. Une étape d'annulation.
    pub(crate) fn sequence_nouvelle_fin(&mut self) {
        let ok = self.avec_etape("Nouvelle séquence", |s| s.structure_op(|d, _| Some(d.nouvelle_sequence_a_la_fin())));
        if ok {
            let seq = self.doc.sequence_of(self.cur_panel);
            self.status = format!("Nouvelle séquence {} à la fin du film : dessine dans sa case.", self.doc.sequence_label(seq));
            self.planche_aller_a_la_sequence(seq);
        }
    }

    /// « Couper la séquence ici » : la case courante (son plan) et tout ce qui la suit dans sa séquence forment une séquence neuve. Une étape
    /// d'annulation. Rien si la case est la première de sa séquence.
    pub(crate) fn sequence_couper_ici(&mut self) {
        if self.storyboard_refuses() || !self.partage_peut_toucher(&[self.panel().id]) {
            return;
        }
        let ok = self.avec_etape("Couper la séquence ici", |s| s.structure_op_dessins(|d, i| d.couper_sequence_avant(i)));
        if ok {
            let seq = self.doc.sequence_of(self.cur_panel);
            self.status = format!("Séquence coupée : cette case commence {}.", self.doc.sequence_label(seq));
        } else {
            self.status = "Cette case est déjà la première de sa séquence : rien à couper.".into();
        }
    }

    /// Renomme la séquence `id` (une étape d'annulation, seulement si le nom change). Vide : son numéro automatique.
    pub(crate) fn sequence_renommer(&mut self, id: u64, nom: &str) {
        let Some(courant) = self.doc.sequences.iter().find(|s| s.id == id).map(|s| s.name.clone()) else { return };
        // (Le même nom, tel quel : rien, même s'il dépasse la longueur permise à la frappe.)
        if courant.trim() == nom.trim() {
            return;
        }
        let nom: String = nom.trim().chars().filter(|c| !c.is_control()).take(NOM_MAX).collect();
        if courant == nom || self.storyboard_refuses() {
            return;
        }
        self.avec_etape("Renommer la séquence", |s| s.record_step());
        if let Some(s) = self.doc.sequence_mut(id) {
            s.name = nom;
        }
        self.status = format!("Séquence renommée : {}.", self.doc.sequence_label(id));
    }

    /// « Renommer la séquence » (commande) : la séquence du plan courant se renomme.
    pub(crate) fn sequence_renommer_courante(&mut self) {
        let seq = self.doc.sequence_of(self.cur_panel);
        self.sequence_renommer_ouvrir(seq);
    }

    /// Ouvre le nom de la séquence `seq` à l'écriture : sur son titre, dans la planche, quand la planche est à l'écran et montre ce titre ;
    /// sinon (le montage, la liste du panneau, une séquence seule dans le film, cachée de la planche) dans une petite fenêtre. Dans les deux
    /// cas : Entrée valide, Échap abandonne, une seule étape d'annulation, « Renommer la séquence ».
    pub(crate) fn sequence_renommer_ouvrir(&mut self, seq: u64) {
        if self.storyboard_refuses() {
            return;
        }
        let sur_la_planche = self.board_shown() && self.board_layout().groupes.iter().any(|g| g.sequence == seq && g.titre.is_some());
        if sur_la_planche {
            self.titre_ouvrir(seq);
            self.planche_aller_a_la_sequence(seq);
        } else {
            self.titre_ouvrir_fenetre(seq);
        }
    }

    /// Ouvre le titre de la séquence `id` à l'écriture, sur la planche.
    pub(crate) fn titre_ouvrir(&mut self, id: u64) {
        self.titre_ouvrir_comme(id, false);
    }

    /// Ouvre le nom de la séquence `id` à l'écriture, dans une petite fenêtre.
    pub(crate) fn titre_ouvrir_fenetre(&mut self, id: u64) {
        self.titre_ouvrir_comme(id, true);
    }

    fn titre_ouvrir_comme(&mut self, id: u64, fenetre: bool) {
        if self.storyboard_refuses() {
            return;
        }
        let nom = self.doc.sequences.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default();
        // (Le nom écrit, ou vide : le numéro automatique est alors gardé comme indication.)
        self.vignettes.titre_edit = Some(TitreEdit { sequence: id, buffer: nom, focused: false, had_focus: false, fenetre });
    }

    /// Pose le titre en cours d'écriture : une étape d'annulation, nommée, si le nom a changé.
    pub(crate) fn titre_valider(&mut self) {
        if let Some(ed) = self.vignettes.titre_edit.take() {
            self.sequence_renommer(ed.sequence, &ed.buffer);
        }
    }

    // ------------------------------------------------------------------ les titres sur la planche

    /// La taille (points d'écran) du titre d'une séquence : celle des numéros de plan, en plus grand.
    fn taille_titre(&self, zoom: f32) -> f32 {
        let echelle = self.settings.vignettes.numeros_echelle();
        crate::vignettesprefs::taille_numero(zoom, echelle) * 1.3
    }

    /// Peint les titres des séquences, au-dessus de leur première rangée : le nom, un filet dessous, le nombre de plans en petit. Le titre en
    /// cours de renommage est peint par son éditeur.
    pub(crate) fn titres_peindre(&self, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout) {
        if layout.groupes.is_empty() {
            return;
        }
        let ids = self.doc.sequence_ids();
        let clip = painter.with_clip_rect(view);
        let encre = Color32::from_gray(15);
        let font = self.taille_titre(xf.zoom);
        for g in &layout.groupes {
            let Some(t) = g.titre else { continue };
            let r = xf.rect(t);
            if !r.intersects(view) {
                continue;
            }
            // Le filet sous le titre, à la largeur de la séquence.
            clip.hline(r.x_range(), r.bottom() + 2.0, Stroke::new(1.0_f32, Color32::from_gray(200)));
            if self.vignettes.titre_edit.as_ref().is_some_and(|e| e.sequence == g.sequence) {
                continue;
            }
            let nom = self.doc.sequence_label(g.sequence);
            let n = ids.iter().filter(|&&q| q == g.sequence).count();
            let petit = clip.layout_no_wrap(format!("{n} plan{}", if n > 1 { "s" } else { "" }), FontId::proportional((font * 0.5).max(10.0)), Color32::from_gray(120));
            // Un titre trop long pour la largeur de la séquence est coupé net par « … » (le nombre de plans garde sa place à sa droite) ;
            // le nom entier est dans la bulle du titre et dans l'éditeur.
            let place = (r.width() - petit.size().x - font * 0.5 - 4.0).max(font * 2.0);
            let mut job = egui::text::LayoutJob::simple(nom, FontId::proportional(font), encre, place);
            job.wrap.max_rows = 1;
            job.wrap.overflow_character = Some('…');
            let galley = clip.layout_job(job);
            let at = pos2(r.left(), r.bottom() - galley.size().y);
            let gras = (font * 0.035).max(0.4);
            for d in [vec2(0.0, 0.0), vec2(gras, 0.0), vec2(0.0, gras)] {
                clip.galley(at + d, galley.clone(), encre);
            }
            clip.galley(pos2(r.left() + galley.size().x + font * 0.5, r.bottom() - petit.size().y - 1.0), petit, Color32::from_gray(120));
        }
    }

    /// La bulle d'un titre (un titre visible, pas en cours d'écriture) : le nom entier (il peut être coupé par « … » sur la planche) et
    /// comment le renommer. Sans la bulle, personne ne savait que le titre se renomme.
    fn titres_bulles(&self, ui: &mut egui::Ui, xf: Xf, layout: &BoardLayout) {
        if layout.groupes.is_empty() {
            return;
        }
        let Some(p) = ui.input(|i| i.pointer.hover_pos()) else { return };
        let Some(g) = layout.titre_en(xf.to_board(p)) else { return };
        let seq = layout.groupes[g].sequence;
        if self.vignettes.titre_edit.as_ref().is_some_and(|e| e.sequence == seq && !e.fenetre) {
            return;
        }
        let Some(t) = layout.groupes[g].titre else { return };
        let nom = self.doc.sequence_label(seq);
        let resp = ui.interact(xf.rect(t).intersect(ui.clip_rect()), egui::Id::new(("titre_sequence", seq)), egui::Sense::hover());
        let _ = crate::bulles::bulle(resp, &nom, "Double-clic sur le titre, ou clic droit > Renommer la séquence : Entrée valide, Échap abandonne, Ctrl+Z rend l'ancien nom.", "");
    }

    /// Le menu du clic droit sur le titre d'une séquence : la renommer, puis le menu du fond de la planche.
    pub(crate) fn titre_menu(&mut self, ui: &mut egui::Ui, seq: u64) {
        let nom = self.doc.sequence_label(seq);
        ui.label(RichText::new(format!("Séquence {nom}")).weak().small());
        if ui.button("Renommer la séquence…").clicked() {
            ui.close_menu();
            self.sequence_renommer_ouvrir(seq);
        }
        ui.separator();
    }

    /// La petite fenêtre du nom d'une séquence (le montage, la liste du panneau, une séquence que la planche ne titre pas) : Entrée ou
    /// « Renommer » valide, Échap ou « Annuler » abandonne. Une seule étape d'annulation.
    pub(crate) fn sequence_fenetre(&mut self, ctx: &egui::Context) {
        let Some(ed) = self.vignettes.titre_edit.as_ref().filter(|e| e.fenetre) else { return };
        let id = ed.sequence;
        if !self.doc.sequences.iter().any(|s| s.id == id) {
            self.vignettes.titre_edit = None;
            return;
        }
        let indication = self.doc.sequence_label(id);
        let (mut ok, mut annuler) = (false, false);
        let Some(ed) = self.vignettes.titre_edit.as_mut() else { return };
        egui::Window::new("Renommer la séquence").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.label("Le nom de la séquence (son titre sur la planche). Vide : son numéro automatique.");
            let champ = ui.add(egui::TextEdit::singleline(&mut ed.buffer).hint_text(indication).char_limit(NOM_MAX).desired_width(340.0));
            if !ed.focused {
                champ.request_focus();
                ed.focused = true;
            }
            if champ.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                ok = true;
            }
            ui.horizontal(|ui| {
                if ui.button("Renommer").clicked() {
                    ok = true;
                }
                if ui.button("Annuler").clicked() {
                    annuler = true;
                }
            });
        });
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            annuler = true;
        }
        if ok {
            self.titre_valider();
        } else if annuler {
            self.vignettes.titre_edit = None;
        }
    }

    /// L'éditeur du titre en cours de renommage : Entrée (ou un clic ailleurs) valide, Échap abandonne.
    pub(crate) fn titre_editeur(&mut self, ui: &mut egui::Ui, xf: Xf, layout: &BoardLayout) {
        self.titres_bulles(ui, xf, layout);
        let Some(id) = self.vignettes.titre_edit.as_ref().filter(|e| !e.fenetre).map(|e| e.sequence) else { return };
        let Some(t) = layout.groupes.iter().find(|g| g.sequence == id).and_then(|g| g.titre) else {
            self.vignettes.titre_edit = None;
            return;
        };
        let r = xf.rect(t);
        let font = FontId::proportional(self.taille_titre(xf.zoom));
        let accent = crate::theme::accent_texte();
        let hint = self.doc.sequence_label(id);
        let Some(ed) = self.vignettes.titre_edit.as_mut() else { return };
        let mut issue = None;
        egui::Area::new(egui::Id::new("vignettes_titre_sequence")).order(egui::Order::Foreground).fixed_pos(pos2(r.left() - 2.0, r.top())).show(ui.ctx(), |ui| {
            egui::Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.0_f32, accent)).inner_margin(egui::Margin::symmetric(3, 1)).show(ui, |ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut ed.buffer)
                        .font(font)
                        .text_color(Color32::from_gray(15))
                        .frame(false)
                        .hint_text(hint)
                        .char_limit(NOM_MAX)
                        .desired_width((r.width() - 8.0).clamp(160.0, 600.0)),
                );
                if !ed.focused {
                    resp.request_focus();
                    ed.focused = true;
                }
                if resp.has_focus() {
                    ed.had_focus = true;
                }
                let (entree, echap) = ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
                if echap {
                    issue = Some(false);
                } else if entree || (ed.had_focus && resp.lost_focus()) {
                    issue = Some(true);
                }
            });
        });
        match issue {
            Some(true) => self.titre_valider(),
            Some(false) => self.vignettes.titre_edit = None,
            None => {}
        }
    }

    // ------------------------------------------------------------------ le bloc « Séquences » du panneau Planche

    /// Le bloc « Séquences » du panneau Planche : une case à cocher par séquence (montrée ou non sur la planche), glisser une séquence pour
    /// choisir son rang, trois raccourcis, et les commandes qui créent une séquence. L'ordre du film ne change pas.
    pub(crate) fn vignettes_sequences(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Séquences").small().weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let resp = ui.add(egui::Button::new(RichText::new(ph::PLUS)).min_size(vec2(22.0, 0.0)));
                let action = "sequence.nouvelle_fin";
                if self.aide(resp, "Nouvelle séquence", "Une séquence de plus à la fin du film, avec un plan vide : dessine dans sa case.", action).clicked() {
                    self.run_action(action);
                }
            });
        });
        let liste = self.doc.annotations.affichage.liste(&self.doc);
        let ids = self.doc.sequence_ids();
        let courante = self.doc.sequence_of(self.cur_panel);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for (nom, description, action) in [
                ("Toutes", "Montrer toutes les séquences sur la planche, dans l'ordre de cette liste.", "sequence.afficher_toutes"),
                ("Seulement la courante", "Ne montrer que la séquence du plan courant, et cadrer la planche dessus.", "sequence.afficher_courante"),
                ("Aucune autre", "Décocher toutes les séquences sauf celle du plan courant (sans recadrer).", "sequence.afficher_aucune_autre"),
            ] {
                let resp = ui.add(egui::Button::new(RichText::new(nom).small()));
                if self.aide(resp, nom, description, action).clicked() {
                    self.run_action(action);
                }
            }
        });
        // Les lignes : une case à cocher, la poignée de glisser (le nom), le nombre de plans. Les rectangles des lignes disent où tombe un glissé.
        let mut lignes: Vec<(u64, Rect)> = Vec::new();
        let mut lache: Option<u64> = None;
        let mut va_a: Option<u64> = None;
        let mut a_basculer: Option<u64> = None;
        let mut a_renommer: Option<u64> = None;
        for &(id, montree) in &liste {
            let n = ids.iter().filter(|&&q| q == id).count();
            let nom = self.doc.sequence_label(id);
            let ligne = ui.push_id(("sequence_planche", id), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    let mut on = montree;
                    let resp = ui.checkbox(&mut on, "");
                    if crate::bulles::bulle(resp, if montree { "Cacher la séquence" } else { "Montrer la séquence" }, "Un affichage de la planche : les plans de la séquence restent dans le film.", "").changed() {
                        a_basculer = Some(id);
                    }
                    let texte = if montree { RichText::new(&nom) } else { RichText::new(&nom).weak() };
                    let largeur = (ui.available_width() - 56.0).max(40.0);
                    let actuelle = id == courante;
                    let resp = ui.add(egui::Button::new(crate::bulles::encre_etat(texte, actuelle)).selected(actuelle).min_size(vec2(largeur, 0.0)).sense(egui::Sense::click_and_drag()));
                    let resp = crate::bulles::bulle(resp, &nom, "Un clic va à cette séquence ; double-clic ou clic droit : la renommer. Glisse-la pour choisir son rang sur la planche (l'ordre du film ne change pas).", "");
                    if resp.clicked() {
                        va_a = Some(id);
                    }
                    if resp.double_clicked() {
                        a_renommer = Some(id);
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Renommer la séquence…").clicked() {
                            ui.close_menu();
                            a_renommer = Some(id);
                        }
                    });
                    if resp.dragged() {
                        self.vignettes.seq_glisse = Some(id);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    }
                    if resp.drag_stopped() {
                        lache = Some(id);
                    }
                    ui.label(RichText::new(n.to_string()).small().weak());
                })
                .response
                .rect
            });
            lignes.push((id, ligne.inner));
        }
        self.sequences_glisser(ui, &lignes, lache);
        if let Some(id) = a_basculer {
            self.affichage_basculer(id);
        }
        if let Some(id) = a_renommer {
            self.titre_ouvrir_fenetre(id);
        }
        if let Some(id) = va_a {
            if let Some(i) = ids.iter().position(|&q| q == id) {
                self.select_panel(i);
            }
            self.planche_aller_a_la_sequence(id);
        }
        ui.add_space(2.0);
        ui.separator();
    }

    /// Le glissé d'une séquence dans la liste : un trait d'accent montre où elle tombera, et au lâcher elle est posée là.
    fn sequences_glisser(&mut self, ui: &mut egui::Ui, lignes: &[(u64, Rect)], lache: Option<u64>) {
        let Some(tenu) = lache.or(self.vignettes.seq_glisse) else { return };
        let (Some(d), Some(y)) = (lignes.iter().position(|l| l.0 == tenu), ui.input(|i| i.pointer.latest_pos()).map(|p| p.y)) else {
            self.vignettes.seq_glisse = None;
            return;
        };
        // La ligne devant laquelle elle tombe (0..=nombre de lignes), et le rang qu'elle aura une fois retirée d'où elle est.
        let devant = lignes.iter().filter(|l| l.1.center().y < y).count();
        let vers = if devant > d { devant - 1 } else { devant };
        if lache.is_some() {
            self.vignettes.seq_glisse = None;
            self.affichage_deplacer(tenu, vers);
            return;
        }
        let bornes = lignes.iter().fold(Rect::NOTHING, |r, l| r.union(l.1));
        let ligne_y = if devant < lignes.len() { lignes[devant].1.top() - 1.0 } else { lignes[lignes.len() - 1].1.bottom() + 1.0 };
        ui.painter().hline(bornes.x_range(), ligne_y, Stroke::new(2.0_f32, crate::theme::accent()));
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }

    /// Le menu « Séquence » d'une case (clic droit) : créer une séquence, la couper ici, la renommer.
    pub(crate) fn sequence_menu_items(&mut self, ui: &mut egui::Ui) {
        self.action_item(ui, "Nouvelle séquence à la fin du film", "sequence.nouvelle_fin");
        let n = self.board_selection().len();
        let texte = if n > 1 { format!("Nouvelle séquence avec les {n} cases choisies") } else { "Nouvelle séquence avec cette case".to_string() };
        self.action_item(ui, &texte, "sequence.nouvelle_avec_cases");
        self.action_item(ui, "Couper la séquence ici", "sequence.couper_ici");
        self.action_item(ui, "Renommer la séquence…", "sequence.renommer");
    }
}
