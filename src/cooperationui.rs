//! Travailler à plusieurs : l'interface (`docs/COOPERATION.md`, §9 et §11).
//!
//! Ce fichier branche la session (`cooperation.rs`) sur l'application : les barrières de lecture seule (`content_refuses`,
//! `storyboard_refuses`), les pastilles du montage et de la planche, la fenêtre « Équipe », « Partager ce projet… », les
//! commandes pour prendre et rendre, les préférences. Aucune entrée-sortie ici : tout ce qui touche au dossier partagé est sur le
//! fil de la session ou celui de l'enregistrement.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;

use eframe::egui::{self, vec2, Align2, Color32, FontId, Pos2, Rect, RichText, Stroke};
use egui_phosphor::regular as ph;

use crate::app::{App, Mode};
use crate::bulles;
use crate::cooperation::{Delais, Horloge, Pastille, Refus, Session};
use crate::depot::{DossierPartage, OrigineCopie};
use crate::equipe::Identite;
use crate::project::{self, partage_io, Progress};
use crate::theme;
use crate::verrous::Cible;

/// Rayon d'une pastille de prise sur un bloc ou une vignette.
pub(crate) const PASTILLE: f32 = 8.0;

/// L'état de l'interface d'équipe d'un projet.
#[derive(Default)]
pub(crate) struct EquipeUi {
    /// La fenêtre « Équipe » est ouverte.
    pub fenetre: bool,
    /// Une reprise à confirmer : les cibles et la question posée.
    pub reprise: Option<(Vec<Cible>, String)>,
    /// « Partager ce projet… » en cours d'écriture.
    pub partage: Option<PartageEnCours>,
    /// Combien de notifications l'utilisateur a vues dans la fenêtre (le reste est « nouveau »), et combien sont déjà passées
    /// dans la barre d'état.
    pub vues: usize,
    pub annoncees: usize,
    /// Un brouillon ou une copie en conflit dont l'import a échoué : le message.
    pub erreur: Option<String>,
}

/// Le film partagé qu'on écrit sur un fil de fond.
pub(crate) struct PartageEnCours {
    handle: JoinHandle<Result<(), String>>,
    progress: Arc<Progress>,
    manifeste: PathBuf,
    nom: String,
}

/// Ce qu'il faut savoir de moi pour rejoindre un film partagé sur le fil d'ouverture.
pub(crate) struct ParamsEquipe {
    pub moi: Identite,
    pub delais: Delais,
    pub ctx: egui::Context,
}

/// Le projet qui vient d'être lu est un film partagé : la session démarre ici, sur le fil d'ouverture (ma fiche est écrite, celles
/// des autres lues : aucune de ces entrées-sorties n'est sur le fil de l'interface).
pub(crate) fn rejoindre_dans_le_fil(loaded: &mut project::Loaded, p: ParamsEquipe, manifeste: &Path) {
    let Some(o) = loaded.partage.take() else { return };
    let depot = Arc::new(DossierPartage::nouveau(manifeste));
    match Session::rejoindre(depot, p.moi, o, &mut loaded.doc, p.delais, Horloge::reelle(), true, Some(p.ctx)) {
        Ok(s) => loaded.session = Some(s),
        Err(e) => loaded.erreur_equipe = Some(e),
    }
}

/// Les réglages de l'équipe, en durées du protocole.
pub(crate) fn delais_de(r: &crate::equipe::ReglageEquipe) -> Delais {
    let r = r.clone().nettoye();
    Delais { bail: std::time::Duration::from_secs(u64::from(r.bail_minutes) * 60), balayage: std::time::Duration::from_secs(u64::from(r.balayage_secondes)), ..Delais::default() }
}

/// Noir ou blanc, selon ce qui se lit sur `fond`.
fn encre_sur(fond: Color32) -> Color32 {
    if theme::luminance(fond) > 0.45 {
        Color32::from_rgb(20, 20, 20)
    } else {
        Color32::WHITE
    }
}

fn couleur(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

impl App {
    // ------------------------------------------------------------------ rejoindre un film partagé

    /// Mon identité d'équipier (l'identifiant est tiré à la première utilisation et retenu dans les réglages).
    pub(crate) fn identite_equipe(&mut self) -> Identite {
        if self.settings.equipe.id.is_empty() {
            self.settings.equipe.id = crate::equipe::nouvel_id();
            let _ = self.settings.save();
        }
        crate::equipe::identite(&self.settings.equipe.clone().nettoye(), &self.settings.comment_author)
    }

    /// Ferme la session d'un projet qui s'en va (autre projet dans l'onglet, onglet fermé) : ce qu'on tenait est rendu, sur un fil
    /// pour ne rien figer.
    pub(crate) fn lacher_la_session(&mut self) {
        if let Some(mut s) = self.partage.take() {
            let _ = std::thread::Builder::new().name("cooperation-fermeture".into()).spawn(move || s.fermer());
        }
        self.equipe_ui = EquipeUi::default();
    }

    // ------------------------------------------------------------------ chaque image

    /// À chaque image d'écran : ce que la session a préparé est posé dans le document (rien n'est décodé ici), et ce que je regarde est
    /// dit aux autres.
    pub(crate) fn cooperation_tick(&mut self, ctx: &egui::Context) {
        self.partage_en_cours_tick(ctx);
        let Some(mut s) = self.partage.take() else { return };
        let plan = self.doc.panels.get(self.cur_panel).map(|p| p.id);
        s.regarde_si_change(plan, if self.mode == Mode::Animation { "Animation" } else { "Storyboard" });
        let fx = s.sonder(&mut self.doc);
        if self.history.serial() != s.serial_vu {
            s.serial_vu = self.history.serial();
            s.apres_structure(&self.doc);
        }
        let a_rejouer = std::mem::take(&mut s.a_rejouer);
        let a_ecrire = s.commentaires_a_ecrire();
        self.partage = Some(s);
        if fx.documents_changes {
            self.renderer.invalidate_all();
            self.clamp_nav();
            self.validate_selection();
            for id in a_rejouer {
                self.rejeu_apres_ouverture(id);
            }
            ctx.request_repaint();
        }
        if fx.commentaires {
            if self.comment_ui.sel.is_some_and(|id| self.doc.comment(id).is_none()) {
                self.comment_ui.sel = None;
            }
            ctx.request_repaint();
        }
        if !fx.prises_perdues.is_empty() {
            // L'historique ne saurait pas défaire ce que la perte a défait : on le repart à zéro (une fois, avec un message).
            self.history.clear();
            self.renderer.invalidate_all();
            self.vsel = None;
            self.stroke = None;
        }
        if let Some(m) = fx.messages.last() {
            self.status = m.clone();
        }
        if let Some(s) = self.partage.as_ref() {
            // La dernière notification passe une fois dans la barre d'état (si on veut les voir).
            if s.notif_total > self.equipe_ui.annoncees {
                if self.settings.equipe.notifications && fx.messages.is_empty() {
                    if let Some(n) = s.notifications.back() {
                        self.status = n.texte.clone();
                    }
                }
                self.equipe_ui.annoncees = s.notif_total;
            }
        }
        // Les commentaires que j'ai changés sont à écrire au prochain enregistrement : le projet est « modifié ».
        if a_ecrire {
            self.modified = true;
        }
        self.equipe_fenetre(ctx);
        self.reprise_fenetre(ctx);
    }

    // ------------------------------------------------------------------ les barrières

    fn titre_court(&self, plan: u64) -> String {
        match self.doc.panels.iter().position(|p| p.id == plan) {
            Some(i) => crate::storyboard::panel_title(&self.doc, i).to_lowercase(),
            None => "ce plan".into(),
        }
    }

    /// Le plan courant n'est pas à moi : modifier son contenu est refusé (avec la raison). Vrai : refusé.
    pub(crate) fn partage_refuse_plan(&mut self) -> bool {
        let Some(plan) = self.doc.panels.get(self.cur_panel).map(|p| p.id) else { return false };
        self.partage_refuse_ce_plan(plan)
    }

    pub(crate) fn partage_refuse_ce_plan(&mut self, plan: u64) -> bool {
        let Some(s) = self.partage.as_mut() else { return false };
        let mut r = s.peut_modifier_plan(plan);
        if r == Err(Refus::Libre) && self.settings.equipe.prise_automatique {
            let faite = s.prendre(&[Cible::Plan(plan)], false).remove(0).1;
            if faite.is_ok() {
                r = s.peut_modifier_plan(plan);
            }
        }
        match r {
            Ok(()) => false,
            Err(refus) => {
                let t = self.titre_court(plan);
                self.status = refus.phrase(&capitale(&t));
                true
            }
        }
    }

    /// Le montage n'est pas à moi (pas de jeton) : changer l'ordre, le nombre, la durée, les noms, le son… est refusé.
    pub(crate) fn partage_refuse_montage(&mut self) -> bool {
        let Some(s) = self.partage.as_mut() else { return false };
        let mut r = s.peut_modifier_montage();
        if r == Err(Refus::Libre) && self.settings.equipe.prise_automatique {
            let faite = s.prendre(&[Cible::Montage], false).remove(0).1;
            if faite.is_ok() {
                r = s.peut_modifier_montage();
            }
        }
        match r {
            Ok(()) => false,
            Err(Refus::PrisPar(nom)) => {
                self.status = format!("Le montage (ordre, durées, noms, son…) est tenu par {nom}. Tu peux dessiner dans tes plans.");
                true
            }
            Err(Refus::Libre) => {
                self.status = "Il faut le jeton du montage pour changer l'ordre, les durées ou les noms : clic droit sur un plan > Prendre le montage.".into();
                true
            }
            Err(Refus::Synchronisation) => {
                self.status = "Un instant : je vérifie que le montage est à jour avant de te le laisser.".into();
                true
            }
            Err(r) => {
                self.status = r.phrase("Le montage");
                true
            }
        }
    }

    /// Détruire, couper ou fusionner des plans : chacun doit être à moi ou libre (pris au passage), jamais à un autre. Faux : refusé.
    pub(crate) fn partage_peut_toucher(&mut self, ids: &[u64]) -> bool {
        let Some(s) = self.partage.as_mut() else { return true };
        let mut a_prendre = Vec::new();
        for &p in ids {
            if s.tiens(Cible::Plan(p)) {
                continue;
            }
            match s.peut_modifier_plan(p) {
                Err(Refus::Libre) => a_prendre.push(Cible::Plan(p)),
                Err(r) => {
                    let t = self.titre_court(p);
                    self.status = r.phrase(&capitale(&t));
                    return false;
                }
                Ok(()) => {}
            }
        }
        if !a_prendre.is_empty() {
            let s = self.partage.as_mut().expect("session");
            for (_, r) in s.prendre(&a_prendre, false) {
                if let Err(refus) = r {
                    self.status = refus.phrase("Ce plan");
                    return false;
                }
            }
        }
        true
    }

    // ------------------------------------------------------------------ les commandes

    /// Les commandes `equipe.*` et `fichier.partager` ; vrai si `action` en était une.
    pub(crate) fn cooperation_action(&mut self, action: &str) -> bool {
        match action {
            "equipe.fenetre" => self.equipe_ui.fenetre = !self.equipe_ui.fenetre,
            "fichier.partager" => self.partager_ce_projet(),
            "equipe.prendre_plan" => self.prendre_ces_plans(false, false),
            "equipe.prendre_sequence" => self.prendre_ces_plans(true, false),
            "equipe.rendre_plan" => self.rendre_ces_cibles(false),
            "equipe.rendre_sequence" => self.rendre_ces_cibles(true),
            "equipe.prendre_montage" => self.prendre_le_montage(),
            "equipe.rendre_montage" => self.rendre_le_montage(),
            "equipe.rendre_tout" => self.rendre_tout(),
            _ => return false,
        }
        true
    }

    /// Les numéros des plans visés : le plan courant, les plans choisis, ou toute la séquence du plan courant.
    fn plans_vises(&self, sequence: bool) -> Vec<u64> {
        let Some(courant) = self.doc.panels.get(self.cur_panel) else { return Vec::new() };
        if sequence {
            let seq = self.doc.sequence_of(self.cur_panel);
            let ids = self.doc.sequence_ids();
            return self.doc.panels.iter().enumerate().filter(|(i, _)| ids[*i] == seq).map(|(_, p)| p.id).collect();
        }
        let mut choisis = self.board_selection();
        if choisis.is_empty() {
            choisis.push(courant.id);
        }
        choisis
    }

    /// « Prendre ce plan » (ou les plans choisis, ou la séquence).
    pub(crate) fn prendre_ces_plans(&mut self, sequence: bool, reprendre: bool) {
        if self.partage.is_none() {
            self.status = "Ce projet n'est pas partagé : Fichier > Partager ce projet… d'abord.".into();
            return;
        }
        let ids = self.plans_vises(sequence);
        let cibles: Vec<Cible> = ids.iter().map(|p| Cible::Plan(*p)).collect();
        let resultats = self.partage.as_mut().expect("session").prendre(&cibles, reprendre);
        let (mut pris, mut a_reprendre, mut refus) = (0, Vec::new(), Vec::new());
        for (c, r) in resultats {
            match r {
                Ok(()) => pris += 1,
                Err(Refus::Perimee { nom, depuis_minutes }) => a_reprendre.push((c, nom, depuis_minutes)),
                Err(r) => {
                    if let Cible::Plan(p) = c {
                        refus.push(r.phrase(&capitale(&self.titre_court(p))));
                    }
                }
            }
        }
        if !a_reprendre.is_empty() {
            let (_, nom, minutes) = &a_reprendre[0];
            let question = format!(
                "{nom} n'a pas donné signe de vie depuis {minutes} min. Reprendre {} ? Ce qu'il n'a pas enregistré sera mis de côté s'il revient.",
                if a_reprendre.len() == 1 { "ce plan".to_string() } else { format!("ces {} plans", a_reprendre.len()) }
            );
            self.equipe_ui.reprise = Some((a_reprendre.into_iter().map(|(c, _, _)| c).collect(), question));
        }
        self.status = match (pris, refus.first()) {
            (0, Some(p)) => p.clone(),
            (1, None) => "Plan pris : il est à toi tant que SHOT est ouvert (ou que tu le rends).".into(),
            (n, None) if n > 1 => format!("{n} plans pris."),
            (n, Some(p)) => format!("{n} plan(s) pris ; {p}"),
            _ => self.status.clone(),
        };
    }

    /// Rend ce plan, les plans choisis ou la séquence. Ce qui n'est pas enregistré l'est d'abord.
    pub(crate) fn rendre_ces_cibles(&mut self, sequence: bool) {
        let ids = self.plans_vises(sequence);
        let cibles: Vec<Cible> = ids.iter().map(|p| Cible::Plan(*p)).collect();
        self.rendre_apres_avoir_enregistre(cibles);
    }

    fn rendre_apres_avoir_enregistre(&mut self, cibles: Vec<Cible>) {
        let Some(s) = self.partage.as_mut() else { return };
        let miennes: Vec<Cible> = cibles.into_iter().filter(|c| s.tiens(*c)).collect();
        if miennes.is_empty() {
            self.status = "Tu ne tiens rien là.".into();
            return;
        }
        if self.modified || self.saving.is_some() {
            // L'enregistrement part, puis la prise est rendue (sinon le travail non écrit serait perdu pour les autres).
            s.rendre_apres_enregistrement(&miennes);
            self.status = "J'enregistre, puis je rends.".into();
            self.save();
        } else {
            s.rendre(&miennes);
            self.status = "Rendu : les autres peuvent le prendre.".into();
        }
    }

    fn prendre_le_montage(&mut self) {
        let Some(s) = self.partage.as_mut() else { return };
        match s.prendre(&[Cible::Montage], false).remove(0).1 {
            Ok(()) => self.status = "Tu tiens le montage (ordre, durées, noms, son, transitions).".into(),
            Err(Refus::Perimee { nom, depuis_minutes }) => {
                self.equipe_ui.reprise = Some((vec![Cible::Montage], format!("{nom} n'a pas donné signe de vie depuis {depuis_minutes} min. Reprendre le montage ?")));
            }
            Err(r) => self.status = r.phrase("Le montage"),
        }
    }

    fn rendre_le_montage(&mut self) {
        self.rendre_apres_avoir_enregistre(vec![Cible::Montage]);
    }

    fn rendre_tout(&mut self) {
        let cibles = self.partage.as_ref().map(Session::mes_cibles).unwrap_or_default();
        self.rendre_apres_avoir_enregistre(cibles);
    }

    // ------------------------------------------------------------------ les menus

    /// Les entrées d'équipe d'un menu de plan (montage, planche).
    pub(crate) fn equipe_menu_items(&mut self, ui: &mut egui::Ui, index: usize) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        let Some(s) = self.partage.as_ref() else { return };
        let c = Cible::Plan(plan);
        let tiens = s.tiens(c);
        let perime = matches!(s.pastille(plan), Some(Pastille::Perimee { .. }));
        let montage = s.tiens(Cible::Montage);
        ui.separator();
        if !tiens {
            let titre = if perime { "Reprendre ce plan" } else { "Prendre ce plan" };
            if ui.button(titre).on_hover_text("Un seul à la fois dessine dans un plan : les autres le regardent, le jouent et le commentent.").clicked() {
                ui.close_menu();
                self.select_panel(index);
                self.prendre_ces_plans(false, false);
            }
        } else if ui.button("Rendre ce plan").on_hover_text("Enregistre, puis libère le plan pour les autres.").clicked() {
            ui.close_menu();
            self.select_panel(index);
            self.rendre_ces_cibles(false);
        }
        if ui.button("Prendre la séquence").on_hover_text("Tous les plans libres de la séquence d'un coup ; ceux d'un autre sont laissés.").clicked() {
            ui.close_menu();
            self.select_panel(index);
            self.prendre_ces_plans(true, false);
        }
        if montage {
            if ui.button("Rendre le montage").clicked() {
                ui.close_menu();
                self.rendre_le_montage();
            }
        } else if ui.button("Prendre le montage").on_hover_text("Le jeton du montage : ordre, durées, noms des plans, son, transitions.").clicked() {
            ui.close_menu();
            self.prendre_le_montage();
        }
        ui.separator();
    }

    // ------------------------------------------------------------------ pastilles

    /// La phrase d'une pastille (survol).
    pub(crate) fn pastille_phrase(&self, plan: u64) -> Option<String> {
        let s = self.partage.as_ref()?;
        let mut t = match s.pastille(plan)? {
            Pastille::Moi { provisoire, pret } => {
                if !pret {
                    "Tu l'as pris : je vérifie qu'il est à jour…".to_string()
                } else if provisoire {
                    "Tu tiens ce plan (prise en confirmation : si un autre l'a pris avant toi, il sera à lui).".to_string()
                } else {
                    "Tu tiens ce plan.".to_string()
                }
            }
            Pastille::Autre { nom, .. } => format!("{nom} tient ce plan : tu peux le regarder, le jouer et le commenter."),
            Pastille::Perimee { nom, .. } => format!("{nom} tenait ce plan et n'a pas donné signe de vie : clic droit > Reprendre ce plan."),
            Pastille::Attente => "Le fichier de ce plan (ou de ses dessins) n'est pas encore arrivé : le plan n'est pas prêt.".to_string(),
        };
        let regardent: Vec<String> = s.vue().equipe.iter().filter(|e| !e.moi && e.en_ligne && e.plan == Some(plan)).map(|e| e.nom.clone()).collect();
        if !regardent.is_empty() {
            t.push_str(&format!(" {} le regarde{}.", regardent.join(", "), if regardent.len() > 1 { "nt" } else { "" }));
        }
        Some(t)
    }

    /// Peint la pastille du plan `plan` dans le coin haut gauche de `bloc` et les ronds de ceux qui le regardent. Rend la zone où la
    /// souris la survole.
    pub(crate) fn peindre_pastille(&self, painter: &egui::Painter, bloc: Rect, plan: u64, zoom: f32) -> Option<Rect> {
        let s = self.partage.as_ref()?;
        let r = (PASTILLE * zoom).clamp(5.0, 10.0);
        let centre = Pos2::new(bloc.left() + r + 6.0, bloc.top() + r + 3.0);
        let mut zone = None;
        let moi = s.moi.clone();
        // Un bloc trop étroit pour une pastille : une bande à la couleur du porteur le long de son bord bas.
        if bloc.width() < 34.0 {
            let (fond, plein) = match s.pastille(plan) {
                Some(Pastille::Moi { .. }) => (couleur(moi.couleur), true),
                Some(Pastille::Autre { couleur: c, .. }) => (couleur(c), true),
                Some(Pastille::Perimee { couleur: c, .. }) => (couleur(c), false),
                Some(Pastille::Attente) => (Color32::from_gray(120), true),
                None => return None,
            };
            let bande = Rect::from_min_max(Pos2::new(bloc.left(), bloc.bottom() - 4.0), bloc.right_bottom());
            if plein {
                painter.rect_filled(bande, 1.0, fond);
            } else {
                painter.rect_stroke(bande, 1.0, Stroke::new(1.0_f32, fond), egui::StrokeKind::Inside);
            }
            return Some(bande.expand2(vec2(0.0, 3.0)));
        }
        if let Some(p) = s.pastille(plan) {
            let (fond, lettres, contour, creux) = match &p {
                Pastille::Moi { provisoire, pret } => (couleur(moi.couleur), crate::equipe::initiales(&moi.nom), if *provisoire || !*pret { theme::avertissement() } else { Color32::WHITE }, false),
                Pastille::Autre { nom, couleur: c } => (couleur(*c), crate::equipe::initiales(nom), theme::encre(), false),
                Pastille::Perimee { nom, couleur: c } => (couleur(*c), crate::equipe::initiales(nom), theme::encre(), true),
                Pastille::Attente => (Color32::from_gray(110), String::new(), theme::encre(), false),
            };
            if creux {
                painter.circle_filled(centre, r, Color32::from_black_alpha(120));
                painter.circle_stroke(centre, r, Stroke::new(1.5_f32, fond));
            } else {
                painter.circle_filled(centre, r, fond);
                painter.circle_stroke(centre, r, Stroke::new(1.0_f32, contour));
            }
            let texte = if matches!(p, Pastille::Attente) { ph::HOURGLASS_MEDIUM.to_string() } else { lettres };
            let encre = if creux { Color32::WHITE } else { encre_sur(fond) };
            painter.text(centre, Align2::CENTER_CENTER, texte, FontId::proportional(r * 1.0), encre);
            zone = Some(Rect::from_center_size(centre, vec2(r * 2.0 + 2.0, r * 2.0 + 2.0)));
        }
        // Ceux qui le regardent : un petit rond creux chacun, sous la pastille.
        let vue = s.vue();
        let mut y = centre.y + r + 5.0;
        for e in vue.equipe.iter().filter(|e| !e.moi && e.en_ligne && e.plan == Some(plan)).take(3) {
            let c = Pos2::new(centre.x, y);
            painter.circle_stroke(c, 4.5, Stroke::new(1.5_f32, couleur(e.couleur)));
            y += 10.0;
        }
        zone
    }

    // ------------------------------------------------------------------ la barre d'état

    /// Dans la barre d'état : l'équipe en ligne, et ce qui demande un regard (dépôt coupé, copies en conflit, brouillons).
    pub(crate) fn cooperation_status_ui(&mut self, ui: &mut egui::Ui) {
        let Some(s) = self.partage.as_ref() else { return };
        let v = s.vue();
        let autres = v.equipe.iter().filter(|e| !e.moi && e.en_ligne).count();
        let a_voir = v.copies.len() + v.brouillons.len();
        let nouvelles = s.notif_total.saturating_sub(self.equipe_ui.vues);
        let hors = v.hors_ligne.clone();
        let mon = s.mes_cibles().len();
        ui.separator();
        let mut texte = format!("{} {autres}", ph::USERS_THREE);
        if a_voir > 0 {
            texte.push_str(&format!("  {} {a_voir}", ph::WARNING));
        }
        let encre = if hors.is_some() { theme::erreur() } else if nouvelles > 0 || a_voir > 0 { theme::avertissement() } else { ui.visuals().text_color() };
        let resp = ui.add(egui::Button::new(RichText::new(texte).small().color(encre)).frame(false));
        let mut description = format!("{autres} autre(s) en ligne, tu tiens {mon} chose(s).");
        if let Some(h) = &hors {
            description.push_str(&format!(" Le dossier partagé ne répond pas : {h}. Ton travail reste en mémoire."));
        }
        if a_voir > 0 {
            description.push_str(&format!(" {a_voir} brouillon(s) ou copie(s) en conflit à regarder."));
        }
        if nouvelles > 0 {
            description.push_str(&format!(" {nouvelles} nouveauté(s)."));
        }
        let raccourci = self.raccourci("equipe.fenetre");
        if bulles::bulle(resp, "Équipe", &description, &raccourci).clicked() {
            self.equipe_ui.fenetre = !self.equipe_ui.fenetre;
        }
    }

    // ------------------------------------------------------------------ la fenêtre Équipe

    fn equipe_fenetre(&mut self, ctx: &egui::Context) {
        if !self.equipe_ui.fenetre || self.partage.is_none() {
            return;
        }
        let mut ouverte = true;
        let mut action: Option<&str> = None;
        let mut importer: Vec<(String, u64)> = Vec::new();
        let mut importer_copie = None;
        let mut ignorer_copie = None;
        let mut forcer: Vec<Cible> = Vec::new();
        let vue = self.partage.as_ref().expect("session").vue();
        let notifications: Vec<String> = self.partage.as_ref().expect("session").notifications.iter().rev().take(30).map(|n| n.texte.clone()).collect();
        let nom_film = self.path.as_ref().and_then(|p| p.file_stem()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let titre_plans: std::collections::HashMap<u64, String> = self.doc.panels.iter().enumerate().map(|(i, p)| (p.id, crate::storyboard::panel_title(&self.doc, i))).collect();
        let moi_cle = self.partage.as_ref().expect("session").moi.id.clone();
        let depot_nom = self.partage.as_ref().expect("session").depot().nom();
        egui::Window::new(format!("Équipe — {nom_film}")).open(&mut ouverte).default_width(420.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if bulles::icone(ui, ph::LOCK_SIMPLE, "Prendre ce plan", "Le plan courant (ou les plans choisis) devient le tien : les autres ne le modifient plus.", &self.raccourci("equipe.prendre_plan")).clicked() {
                    action = Some("equipe.prendre_plan");
                }
                if bulles::icone(ui, ph::LOCK_SIMPLE_OPEN, "Rendre ce plan", "Enregistre puis libère le plan courant.", &self.raccourci("equipe.rendre_plan")).clicked() {
                    action = Some("equipe.rendre_plan");
                }
                ui.separator();
                if bulles::icone(ui, ph::STACK, "Prendre la séquence", "Tous les plans libres de la séquence courante.", &self.raccourci("equipe.prendre_sequence")).clicked() {
                    action = Some("equipe.prendre_sequence");
                }
                if bulles::icone(ui, ph::STACK_SIMPLE, "Rendre la séquence", "Enregistre puis libère la séquence courante.", &self.raccourci("equipe.rendre_sequence")).clicked() {
                    action = Some("equipe.rendre_sequence");
                }
                ui.separator();
                if bulles::icone(ui, ph::FILM_STRIP, "Prendre le montage", "Le jeton du montage : ordre, durées, noms, son, transitions.", &self.raccourci("equipe.prendre_montage")).clicked() {
                    action = Some("equipe.prendre_montage");
                }
                if bulles::icone(ui, ph::FILM_SLATE, "Rendre le montage", "Enregistre puis rend le jeton du montage.", &self.raccourci("equipe.rendre_montage")).clicked() {
                    action = Some("equipe.rendre_montage");
                }
                ui.separator();
                if bulles::icone(ui, ph::HAND_PALM, "Tout rendre", "Rend tout ce que tu tiens (après avoir enregistré).", &self.raccourci("equipe.rendre_tout")).clicked() {
                    action = Some("equipe.rendre_tout");
                }
            });
            for c in vue.synchro_lente.iter().filter(|c| self.partage.as_ref().is_some_and(|s| s.tiens(**c))) {
                let t = match c {
                    Cible::Plan(p) => titre_plans.get(p).cloned().unwrap_or_else(|| "un plan".into()).to_lowercase(),
                    Cible::Montage => "le montage".into(),
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} {t} attend encore son dernier fichier.", ph::HOURGLASS_MEDIUM)).small());
                    if bulles::icone(ui, ph::FAST_FORWARD, "Prendre tel quel", "La synchronisation est lente : prendre le plan tel que ce poste l'a. Ce que l'autre a écrit en plus arrivera comme brouillon ou copie en conflit.", "").clicked() {
                        forcer.push(*c);
                    }
                });
            }
            ui.label(RichText::new(depot_nom.as_str()).small().weak());
            let en_attente = vue.plans.values().filter(|i| i.attente.is_some()).count();
            if en_attente > 0 {
                ui.label(RichText::new(format!("{} {en_attente} plan(s) pas encore arrivé(s) : un export les montrerait vides.", ph::HOURGLASS_MEDIUM)).small().color(theme::avertissement()));
            }
            if let Some(h) = &vue.hors_ligne {
                ui.add_space(4.0);
                ui.label(RichText::new(format!("{} Le dossier partagé ne répond pas : {h}", ph::WARNING)).color(theme::erreur()));
                ui.label(RichText::new("Tu peux continuer sur ce que tu tiens ; tout se synchronise au retour.").small().weak());
            }
            ui.separator();
            if vue.equipe.is_empty() {
                ui.label("Personne d'autre pour l'instant.");
            }
            for e in &vue.equipe {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::hover());
                    let c = couleur(e.couleur);
                    if e.en_ligne {
                        ui.painter().circle_filled(rect.center(), 8.0, c);
                        ui.painter().text(rect.center(), Align2::CENTER_CENTER, crate::equipe::initiales(&e.nom), FontId::proportional(8.0), encre_sur(c));
                    } else {
                        ui.painter().circle_stroke(rect.center(), 8.0, Stroke::new(1.5_f32, c));
                    }
                    let mut ligne = e.nom.clone();
                    if e.moi {
                        ligne.push_str(" (toi)");
                    }
                    ui.label(RichText::new(ligne).strong());
                    if !e.en_ligne {
                        ui.label(RichText::new("hors ligne").small().weak());
                    } else if let Some(p) = e.plan {
                        let t = titre_plans.get(&p).cloned().unwrap_or_else(|| "un plan".into());
                        ui.label(RichText::new(format!("regarde {} ({})", t.to_lowercase(), e.mode)).small().weak());
                    }
                });
                if !e.tient.is_empty() {
                    let tient: Vec<String> = e
                        .tient
                        .iter()
                        .map(|c| match c {
                            Cible::Montage => "le montage".to_string(),
                            Cible::Plan(p) => titre_plans.get(p).cloned().unwrap_or_else(|| "un plan".into()).to_lowercase(),
                        })
                        .collect();
                    ui.label(RichText::new(format!("   tient : {}", tient.join(", "))).small());
                }
            }
            if !vue.copies.is_empty() {
                ui.separator();
                ui.label(RichText::new(format!("{} Copies en conflit ({})", ph::WARNING, vue.copies.len())).strong());
                ui.label(RichText::new("Un client de synchronisation (Dropbox, Drive…) a gardé deux versions d'un fichier.").small().weak());
                for c in &vue.copies {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(c.nom()).small());
                        if let OrigineCopie::Plan(p) = c.origine {
                            if bulles::icone(ui, ph::GIT_BRANCH, "Importer comme version", "Ajoute le contenu de cette copie comme version du plan (tu dois tenir le plan). La copie est rangée dans partage/importes.", "").clicked() {
                                importer_copie = Some((c.clone(), p));
                            }
                        }
                        if bulles::icone(ui, ph::EYE_SLASH, "Ignorer", "Ne plus montrer cette copie (elle reste dans le dossier).", "").clicked() {
                            ignorer_copie = Some(c.clone());
                        }
                    });
                }
            }
            if !vue.brouillons.is_empty() {
                ui.separator();
                ui.label(RichText::new(format!("{} Brouillons ({})", ph::NOTE_PENCIL, vue.brouillons.len())).strong());
                ui.label(RichText::new("Du travail mis de côté (un plan perdu dans une course, repris pendant une coupure…).").small().weak());
                for nom in &vue.brouillons {
                    let Some(plan) = nom_de_brouillon(nom) else { continue };
                    ui.horizontal(|ui| {
                        let t = titre_plans.get(&plan.0).cloned().unwrap_or_else(|| "plan disparu".into());
                        let par = if plan.1 == moi_cle { "toi".to_string() } else { plan.1.clone() };
                        ui.label(RichText::new(format!("{t} — de {par}")).small()).on_hover_text(nom);
                        if bulles::icone_si(ui, titre_plans.contains_key(&plan.0), ph::GIT_BRANCH, "Importer comme version", "Ajoute ce brouillon comme version du plan (tu dois tenir le plan).", "").clicked() {
                            importer.push((nom.clone(), plan.0));
                        }
                    });
                }
            }
            if let Some(e) = &self.equipe_ui.erreur {
                ui.separator();
                ui.label(RichText::new(e).color(theme::erreur()));
            }
            ui.separator();
            ui.label(RichText::new("Nouveautés").strong());
            if notifications.is_empty() {
                ui.label(RichText::new("Rien pour l'instant.").small().weak());
            }
            egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                for n in &notifications {
                    ui.label(RichText::new(n).small());
                }
            });
        });
        self.equipe_ui.fenetre = ouverte;
        if let Some(s) = self.partage.as_ref() {
            self.equipe_ui.vues = s.notif_total;
        }
        if let Some(a) = action {
            self.run_action(a);
        }
        for c in forcer {
            if let Some(s) = self.partage.as_mut() {
                s.forcer_la_synchronisation(c);
            }
        }
        for (nom, plan) in importer {
            self.importer_brouillon(&nom, plan);
        }
        if let Some((c, plan)) = importer_copie {
            self.importer_copie(c, plan);
        }
        if let Some(c) = ignorer_copie {
            if let Some(s) = self.partage.as_ref() {
                s.copie_traitee(&c);
            }
        }
    }

    /// Un brouillon devient une version du plan (que je dois tenir) ; il est rangé dans `partage/importes`.
    fn importer_brouillon(&mut self, nom: &str, plan: u64) {
        let Some(s) = self.partage.as_ref() else { return };
        let octets = match s.lire_brouillon(nom) {
            Ok(o) => o,
            Err(e) => {
                self.equipe_ui.erreur = Some(e);
                return;
            }
        };
        let de = format!("brouillon de {}", nom_de_brouillon(nom).map_or("?".to_string(), |(_, q)| q));
        if self.importer_version(plan, &octets, &de) {
            if let Some(s) = self.partage.as_ref() {
                s.brouillon_importe(nom);
            }
        }
    }

    fn importer_copie(&mut self, c: crate::depot::CopieEnConflit, plan: u64) {
        let Some(s) = self.partage.as_ref() else { return };
        let depot = s.depot();
        let octets = match depot.lire_copie(&c) {
            Ok(o) => o,
            Err(e) => {
                self.equipe_ui.erreur = Some(e);
                return;
            }
        };
        if self.importer_version(plan, &octets, "copie en conflit") {
            let _ = depot.ranger_copie(&c);
            if let Some(s) = self.partage.as_ref() {
                s.copie_traitee(&c);
            }
        }
    }

    fn importer_version(&mut self, plan: u64, octets: &[u8], de: &str) -> bool {
        self.equipe_ui.erreur = None;
        if self.partage_refuse_ce_plan(plan) {
            self.equipe_ui.erreur = Some("Prends le plan pour y importer une version.".into());
            return false;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let Some(mut s) = self.partage.take() else { return false };
        let r = s.importer_comme_version(&mut self.doc, plan, octets, de);
        self.partage = Some(s);
        match r {
            Ok(numero) => {
                let avant = std::mem::replace(&mut self.history.context, Some("Importer une version".to_string()));
                self.commit_structure(snap);
                self.history.context = avant;
                self.status = format!("Version importée dans {} : {}.", self.titre_court(plan), crate::planvers::etiquette(numero));
                true
            }
            Err(e) => {
                self.equipe_ui.erreur = Some(e);
                false
            }
        }
    }

    /// La confirmation d'une reprise (bail expiré).
    fn reprise_fenetre(&mut self, ctx: &egui::Context) {
        let Some((cibles, question)) = self.equipe_ui.reprise.clone() else { return };
        let mut fermer = false;
        let mut confirmer = false;
        egui::Window::new("Reprendre ?").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.set_max_width(400.0);
            ui.label(&question);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                confirmer = ui.button("Reprendre").clicked();
                fermer = ui.button("Annuler").clicked();
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            fermer = true;
        }
        if confirmer {
            if let Some(s) = self.partage.as_mut() {
                let r = s.prendre(&cibles, true);
                let ok = r.iter().filter(|(_, x)| x.is_ok()).count();
                self.status = format!("{ok} reprise(s) faite(s).");
            }
            self.equipe_ui.reprise = None;
        } else if fermer {
            self.equipe_ui.reprise = None;
        }
    }

    // ------------------------------------------------------------------ après enregistrement, annuler

    /// Les plans que je ne tiens pas reprennent leur dernier état reçu après un « annuler » (§10).
    pub(crate) fn partage_apres_historique(&mut self) {
        if let Some(mut s) = self.partage.take() {
            s.rafraichir_etranger(&mut self.doc);
            self.partage = Some(s);
        }
    }

    /// Dit, dans la barre d'état, ce qu'un enregistrement a laissé de côté (prises en confirmation, plans en attente…).
    pub(crate) fn partage_avant_enregistrement(&mut self) {
        let Some(s) = self.partage.as_ref() else { return };
        let conf = s.en_confirmation();
        let attente = s.en_verification();
        if !conf.is_empty() || !attente.is_empty() {
            let titres: Vec<String> = conf
                .iter()
                .chain(attente.iter())
                .map(|c| match c {
                    Cible::Plan(p) => self.titre_court(*p),
                    Cible::Montage => "le montage".into(),
                })
                .collect();
            self.status = format!("Enregistrement : {} sera écrit au prochain enregistrement (prise en confirmation).", titres.join(", "));
        }
    }

    // ------------------------------------------------------------------ partager ce projet

    /// « Partager ce projet… » : on choisit où créer le film partagé (le dossier Drive / Dropbox / réseau), il s'écrit sur un fil, puis
    /// s'ouvre dans un nouvel onglet. Le projet ouvert n'est pas touché.
    pub(crate) fn partager_ce_projet(&mut self) {
        if self.partage.is_some() {
            self.status = "Ce film est déjà partagé. « Enregistrer sous… » en fait une copie à toi, à plat.".into();
            return;
        }
        if self.equipe_ui.partage.is_some() {
            self.status = "Le partage est déjà en cours.".into();
            return;
        }
        self.end_stroke();
        let nom_defaut = {
            let t: String = self.doc.properties.title.trim().chars().map(|c| if r#"<>:"/\|?*"#.contains(c) { '_' } else { c }).collect();
            format!("{}.pelure", self.base_name_or(if t.is_empty() { "Mon film" } else { &t }))
        };
        let Some(choisi) = rfd::FileDialog::new()
            .set_title("Partager ce projet : choisis le dossier partagé (Drive, Dropbox, réseau) ; un dossier à ce nom y sera créé")
            .add_filter("Projet SHOT", &["pelure"])
            .set_file_name(nom_defaut)
            .save_file()
        else {
            return;
        };
        let (_, manifeste) = project::target_for(&choisi);
        self.demarrer_le_partage(manifeste);
    }

    /// Écrit le film partagé dans `manifeste` (un dossier neuf).
    pub(crate) fn demarrer_le_partage(&mut self, manifeste: PathBuf) {
        if let Err(e) = project::check_target(&manifeste, None) {
            self.status = format!("Partage annulé : {e}");
            return;
        }
        let moi = self.identite_equipe();
        let signature = partage_io::Signature { id: moi.id.clone(), nom: moi.nom.clone(), le: crate::equipe::maintenant_ms() };
        let travail = match partage_io::preparer_partage(&self.doc, &manifeste, &signature, &partage_io::nouveau_projet_id()) {
            Ok(t) => t,
            Err(e) => {
                self.status = format!("Partage impossible : {e}");
                return;
            }
        };
        let progress = Arc::new(Progress::default());
        let (p, wake) = (progress.clone(), self.ctx.clone());
        let handle = std::thread::spawn(move || {
            let r = travail.ecrire(Some(&p), (crate::parallele::coeurs() / 2).clamp(2, 8));
            wake.request_repaint();
            r
        });
        let nom = manifeste.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.equipe_ui.partage = Some(PartageEnCours { handle, progress, manifeste, nom });
        self.status = "Partage en cours…".into();
    }

    /// Le film partagé qu'on écrit : l'avancement, puis son ouverture dans un nouvel onglet.
    fn partage_en_cours_tick(&mut self, ctx: &egui::Context) {
        let Some(p) = &self.equipe_ui.partage else { return };
        if !p.handle.is_finished() {
            let (fait, total) = (p.progress.done.load(Ordering::Relaxed), p.progress.total.load(Ordering::Relaxed));
            let nom = p.nom.clone();
            egui::Window::new("Partage").title_bar(false).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
                ui.label(format!("Création du film partagé « {nom} »…"));
                let fraction = if total == 0 { 0.0 } else { fait as f32 / total as f32 };
                ui.add(egui::ProgressBar::new(fraction).desired_width(280.0).text(if total == 0 { "préparation".to_string() } else { format!("{fait} / {total} dessins") }));
                ui.label(RichText::new("Le projet ouvert n'est pas touché. Le film partagé s'ouvrira dans un nouvel onglet.").small().weak());
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }
        let Some(p) = self.equipe_ui.partage.take() else { return };
        match p.handle.join().unwrap_or_else(|_| Err("l'écriture a planté".into())) {
            Ok(()) => {
                self.status = format!("Film partagé créé : {}. Je l'ouvre.", p.manifeste.display());
                self.open_path(p.manifeste);
            }
            Err(e) => self.status = format!("Partage impossible : {e}"),
        }
    }

    // ------------------------------------------------------------------ préférences

    /// Préférences > Équipe.
    pub(crate) fn equipe_prefs_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Équipe").strong());
        let mut change = false;
        ui.horizontal(|ui| {
            ui.label("Mon nom :");
            let hint = crate::commentaires::author_name(&self.settings.comment_author);
            let resp = ui.add(egui::TextEdit::singleline(&mut self.settings.equipe.nom).hint_text(hint).desired_width(140.0));
            change |= resp.lost_focus();
            let mut c = couleur(self.settings.equipe.couleur.unwrap_or_else(|| crate::equipe::couleur_pour(&self.settings.equipe.id)));
            if ui.color_edit_button_srgba(&mut c).changed() {
                self.settings.equipe.couleur = Some([c.r(), c.g(), c.b()]);
                change = true;
            }
        });
        ui.label(RichText::new("Le nom et la couleur que les autres voient sur les plans que tu tiens.").small().weak());
        ui.horizontal(|ui| {
            ui.label("Une prise périme après");
            let mut m = self.settings.equipe.bail_minutes;
            if ui.add(egui::DragValue::new(&mut m).range(2..=240).suffix(" min")).changed() {
                self.settings.equipe.bail_minutes = m;
                change = true;
            }
            ui.label("sans nouvelles.");
        });
        let resp = ui.checkbox(&mut self.settings.equipe.prise_automatique, "Prendre le plan libre que je commence à modifier").on_hover_text("Sinon, modifier un plan libre est refusé : clic droit > Prendre ce plan.");
        change |= resp.changed();
        let resp = ui.checkbox(&mut self.settings.equipe.notifications, "Dire dans la barre d'état ce que font les autres").on_hover_text("« Léa a enregistré le plan 12 »…");
        change |= resp.changed();
        if change {
            self.settings.equipe = self.settings.equipe.clone().nettoye();
            let _ = self.settings.save();
        }
    }
}

/// « Plan 12 » à partir de « plan 12 » : la phrase commence par une capitale.
fn capitale(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// `plan-<n°>.<équipier>.<heure>.<raison>.json` : le plan et l'équipier d'un brouillon.
fn nom_de_brouillon(nom: &str) -> Option<(u64, String)> {
    let reste = nom.strip_prefix("plan-")?;
    let (plan, suite) = reste.split_once('.')?;
    let (equipier, _) = suite.split_once('.')?;
    Some((plan.parse().ok()?, equipier.to_string()))
}
