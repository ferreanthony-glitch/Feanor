//! La fenêtre « Corbeille » (menu Fichier, `fichier.corbeille`) : ce que les gestes du montage ont retiré du film, récupérable
//! (`corbeille.rs`). Une ligne par entrée : vignette, nom, séquence d'origine, date, pourquoi ; Restaurer, Supprimer
//! définitivement (avec confirmation) ; en bas, Vider. La fenêtre dit aussi la place que la corbeille prend sur le disque.
//! Restaurer est une étape d'annulation. Supprimer définitivement et Vider sont **définitifs** : ils ne s'annulent pas et sortent l'entrée de
//! toutes les étapes d'annulation (`History::retirer_de_la_corbeille`) ; la confirmation le dit.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

use eframe::egui::{self, Align2, Color32, TextureHandle, TextureOptions};

use crate::app::App;
use crate::corbeille::Charge;
use crate::document::{Cel, CelId, Document, Layer, LayerKind};

const VIGNETTE: [usize; 2] = [96, 54];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Confirmation {
    Supprimer(u64),
    Vider,
}

#[derive(Default)]
pub(crate) struct Fenetre {
    pub confirmation: Option<Confirmation>,
    /// Les vignettes déjà faites (rien : le dessin n'a pas pu être relu).
    pub vignettes: HashMap<u64, Option<TextureHandle>>,
    /// Entrées dont la vignette se lit sur un autre fil.
    en_attente: HashSet<u64>,
    lecteur: Option<Receiver<(u64, Option<egui::ColorImage>)>>,
    /// Place sur le disque : (nombre d'entrées au moment du calcul, octets) ; calculée sur un autre fil.
    pub taille: Option<(usize, u64)>,
    calcul_taille: Option<Receiver<(usize, u64)>>,
}

/// Une restauration dont les dessins se relisent du dossier sur un autre fil (l'interface ne gèle pas) : le résultat arrive par `rx`.
pub(crate) struct Chargement {
    pub id: u64,
    rx: Receiver<Result<(Charge, Option<String>), String>>,
}

/// « 3,2 Mo », « 480 Ko ».
pub(crate) fn octets_texte(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1} Mo", n as f64 / 1_000_000.0).replace('.', ",")
    } else {
        format!("{} Ko", n.div_ceil(1000))
    }
}

/// La vignette d'une entrée (partie 206), faite comme celle d'un bloc de la timeline (`render::thumbnail`) : **la partie filmée** (le
/// cadre, au centre de la feuille), chaque pixel la moyenne exacte de tout ce qu'il couvre, à `w × h` pixels (ceux de l'écran, densité
/// comprise). Avant : toute la feuille (deux fois le cadre, d'office), un échantillon par point de 96 × 54 : sur une feuille de 3840 ×
/// 2160, un trait de 6 pixels tombait entre les échantillons, la vignette restait presque blanche.
pub(crate) fn miniature(cel: &Cel, tete: &crate::project::Tete, paper: [u8; 3], w: usize, h: usize) -> egui::ColorImage {
    let (fw, fh) = (tete.largeur.max(1), tete.hauteur.max(1));
    let mut mini = Document::new(fw, fh, tete.cadence.max(1));
    if tete.cadre[0] > 0 && tete.cadre[1] > 0 {
        mini.frame = tete.cadre;
    }
    mini.paper = paper;
    let id = mini.alloc();
    mini.cels.insert(id, Cel { raster: cel.raster.clone(), vector: None, version: 0, fond: None });
    let mut calque = Layer::new("Dessin", LayerKind::Bitmap);
    calque.frames = vec![Some(id)];
    mini.panels[0].layers = vec![calque];
    let plan = crate::render::panel_plan_at(&mini, 0, 0, Some(paper));
    crate::render::thumbnail(&mini, &plan, &mini.default_camera(), w.max(1), h.max(1))
}

/// Ce qu'il faut pour la vignette d'une entrée : son dessin en mémoire, ou à relire du dossier du projet.
enum Source {
    Memoire(Box<Cel>),
    Fichier(CelId),
}

impl App {
    fn dossier_de_la_corbeille(&self) -> Option<PathBuf> {
        self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()))
    }

    /// Ouvre la fenêtre de la corbeille.
    pub(crate) fn ouvrir_la_corbeille(&mut self) {
        self.corbeille_ui = Some(Fenetre::default());
    }

    /// Une restauration relit ses dessins sur un autre fil (les essais l'attendent).
    #[cfg(test)]
    pub(crate) fn corbeille_en_cours(&self) -> bool {
        self.corbeille_charge.is_some()
    }

    /// Remet l'entrée `id` dans le film : une étape d'annulation. Un plan revient à sa place s'il en a encore une, sinon à la fin de
    /// sa séquence ou de la séquence courante ; des calques reviennent dans leur plan s'il existe. Si des dessins sont à relire du
    /// dossier du projet, la lecture se fait sur un autre fil (`corbeille_sonder` pose le résultat) ; sinon tout de suite.
    pub(crate) fn corbeille_restaurer(&mut self, id: u64) {
        if self.corbeille_charge.is_some() {
            self.status = "Une restauration est déjà en cours.".into();
            return;
        }
        let Some(e) = self.doc.corbeille.trouver(id).cloned() else { return };
        let refuse = match e.sorte {
            crate::corbeille::Sorte::Plan => self.storyboard_refuses(),
            crate::corbeille::Sorte::Calques => self.content_refuses(),
        };
        if refuse {
            return;
        }
        let dir = self.dossier_de_la_corbeille();
        let deja: HashSet<CelId> = e.cels.iter().copied().filter(|c| self.doc.cels.contains_key(c)).collect();
        let a_relire = matches!(e.contenu, crate::corbeille::Contenu::Fichier { .. }) || e.cels.iter().any(|c| !deja.contains(c));
        if a_relire {
            let (tx, rx) = channel();
            std::thread::spawn(move || {
                let r = crate::corbeille::charger(&e, dir.as_deref(), &|c| deja.contains(&c));
                let _ = tx.send(r);
            });
            self.corbeille_charge = Some(Chargement { id, rx });
            self.status = "Restauration en cours : les dessins se relisent du dossier du projet…".into();
            return;
        }
        match crate::corbeille::charger(&e, dir.as_deref(), &|c| deja.contains(&c)) {
            Ok((charge, avertissement)) => self.corbeille_appliquer(id, charge, avertissement),
            Err(m) => self.status = m,
        }
    }

    /// Pose le résultat d'une restauration lue sur un autre fil, s'il est arrivé (à chaque image d'écran).
    pub(crate) fn corbeille_sonder(&mut self) {
        let Some(c) = &self.corbeille_charge else { return };
        let id = c.id;
        match c.rx.try_recv() {
            Ok(Ok((charge, avertissement))) => {
                self.corbeille_charge = None;
                self.corbeille_appliquer(id, charge, avertissement);
            }
            Ok(Err(m)) => {
                self.corbeille_charge = None;
                self.status = m;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.corbeille_charge = None;
                self.status = "La restauration a échoué : sa lecture s'est interrompue.".into();
            }
        }
    }

    fn corbeille_appliquer(&mut self, id: u64, charge: Charge, avertissement: Option<String>) {
        self.end_stroke();
        self.poser_les_selections();
        let snap = self.snapshot();
        let sequence = self.doc.sequence_of(self.cur_panel);
        match self.doc.restaurer_entree(id, charge, sequence) {
            Ok((phrase, index)) => {
                self.cur_panel = index;
                self.clamp_nav();
                self.commit_as(snap, "Restaurer depuis la corbeille");
                self.board_sel.clear();
                self.status = match avertissement {
                    Some(a) => format!("{phrase} {a}"),
                    None => phrase,
                };
            }
            Err(m) => self.status = m,
        }
    }
    /// Supprime pour de bon l'entrée `id` (ou toutes, `None`) : leurs dessins quitteront le dossier au prochain enregistrement. Une
    /// étape d'annulation tant que la séance dure.
    pub(crate) fn corbeille_supprimer(&mut self, id: Option<u64>) {
        if self.doc.corbeille.is_empty() {
            return;
        }
        // Vraiment définitif : pas d'étape d'annulation, et l'entrée sort aussi de toutes les étapes déjà là (sinon un Ctrl+Z la rendrait sans
        // ses dessins, que l'enregistrement suivant efface).
        let ids: std::collections::BTreeSet<u64> = match id {
            Some(id) => std::iter::once(id).collect(),
            None => self.doc.corbeille.entrees.iter().map(|e| e.id).collect(),
        };
        let avant = self.doc.corbeille.len();
        self.doc.corbeille.entrees.retain(|e| !ids.contains(&e.id));
        let n = avant - self.doc.corbeille.len();
        if n == 0 {
            return;
        }
        self.history.retirer_de_la_corbeille(&ids);
        // (Partie 205 : l'environnement 3D propre d'un plan supprimé pour de bon part avec lui.)
        self.doc.oublier_les_envs_perdus();
        self.modified = true;
        self.status = match n {
            1 => "Supprimé définitivement de la corbeille (ça ne s'annule pas).".into(),
            n => format!("Corbeille vidée : {n} éléments supprimés définitivement (ça ne s'annule pas)."),
        };
    }

    /// Partie 206 : Échap ferme la Corbeille, comme les autres fenêtres ; une confirmation en cours (« Supprimer… », « Vider… ») est
    /// d'abord abandonnée. Appelée **avant les raccourcis** (`handle_shortcuts`) : la touche est prise, elle ne fait rien d'autre
    /// (« Tout désélectionner », la sélection d'un bloc…) ; une fois la fenêtre fermée, Échap retrouve son usage. Une fenêtre modale
    /// par-dessus, ou une liste déroulante à l'écran, la gardent pour elles.
    pub(crate) fn corbeille_echap(&mut self, ctx: &egui::Context) {
        if self.corbeille_ui.is_none() || self.modal_open() {
            return;
        }
        if crate::exportui::echap_ferme(ctx) {
            match self.corbeille_ui.as_mut() {
                Some(fen) if fen.confirmation.is_some() => fen.confirmation = None,
                _ => self.corbeille_ui = None,
            }
        }
    }

    /// La fenêtre, à chaque image d'écran tant qu'elle est ouverte.
    pub(crate) fn corbeille_fenetre_ui(&mut self, ctx: &egui::Context) {
        // (Une restauration lue sur un autre fil se pose ici, fenêtre ouverte ou non.)
        self.corbeille_sonder();
        if self.corbeille_charge.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        let Some(mut fen) = self.corbeille_ui.take() else { return };
        let mut ouverte = true;
        let mut restaurer = None;
        let mut supprimer = None;
        let n = self.doc.corbeille.len();
        // La place sur le disque se calcule sur un autre fil (la lecture du dossier ne gèle pas l'interface) ; elle se recalcule quand le
        // contenu change.
        if let Some(rx) = &fen.calcul_taille {
            match rx.try_recv() {
                Ok(r) => {
                    fen.taille = Some(r);
                    fen.calcul_taille = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => fen.calcul_taille = None,
                Err(_) => {}
            }
        }
        if fen.calcul_taille.is_none() && fen.taille.is_none_or(|(k, _)| k != n) {
            match self.dossier_de_la_corbeille() {
                Some(d) => {
                    let ids: BTreeSet<_> = self.doc.corbeille.cels().collect();
                    let (tx, rx) = channel();
                    std::thread::spawn(move || {
                        let _ = tx.send((n, crate::project::taille_des_dessins(&d, &ids)));
                    });
                    fen.calcul_taille = Some(rx);
                }
                None => fen.taille = Some((n, 0)),
            }
        }
        // Les vignettes : celles dont le dessin est en mémoire se font tout de suite (96 × 54 échantillons) ; les autres se lisent du
        // dossier sur un autre fil, une à une, et arrivent quand elles sont prêtes.
        if let Some(rx) = &fen.lecteur {
            loop {
                match rx.try_recv() {
                    Ok((id, img)) => {
                        let tex = img.map(|i| ctx.load_texture(format!("corbeille-{id}"), i, TextureOptions::LINEAR));
                        fen.en_attente.remove(&id);
                        fen.vignettes.insert(id, tex);
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        for id in std::mem::take(&mut fen.en_attente) {
                            fen.vignettes.insert(id, None);
                        }
                        fen.lecteur = None;
                        break;
                    }
                }
            }
        }
        // Les vignettes se font toutes sur un autre fil (une vignette exacte coûte quelques millisecondes) : celles dont le dessin est
        // en mémoire avec une copie du dessin (ses tuiles sont partagées, rien n'est recopié), les autres relues du dossier.
        let paper = self.doc.paper;
        let ppp = ctx.pixels_per_point().max(1.0);
        let taille = ((VIGNETTE[0] as f32 * ppp).round() as usize, (VIGNETTE[1] as f32 * ppp).round() as usize);
        let dossier = self.dossier_de_la_corbeille();
        let mut a_lire: Vec<(u64, Source, crate::project::Tete)> = Vec::new();
        for e in self.doc.corbeille.entrees.iter() {
            if fen.vignettes.contains_key(&e.id) || fen.en_attente.contains(&e.id) {
                continue;
            }
            match e.vignette {
                None => {
                    fen.vignettes.insert(e.id, None);
                }
                Some(cel) => match self.doc.cels.get(&cel) {
                    Some(c) => a_lire.push((e.id, Source::Memoire(Box::new(c.clone())), e.tete.clone())),
                    None if dossier.is_some() => a_lire.push((e.id, Source::Fichier(cel), e.tete.clone())),
                    None => {
                        fen.vignettes.insert(e.id, None);
                    }
                },
            }
        }
        if !a_lire.is_empty() && fen.lecteur.is_none() {
            fen.en_attente.extend(a_lire.iter().map(|(id, ..)| *id));
            let (tx, rx) = channel();
            std::thread::spawn(move || {
                for (id, source, tete) in a_lire {
                    let cel = match source {
                        Source::Memoire(c) => Some(*c),
                        Source::Fichier(cel) => dossier.as_deref().and_then(|d| crate::project::charger_un_dessin_avec(d, cel, tete.largeur, tete.hauteur, true).ok()),
                    };
                    let img = cel.map(|c| miniature(&c, &tete, paper, taille.0, taille.1));
                    if tx.send((id, img)).is_err() {
                        return;
                    }
                }
            });
            fen.lecteur = Some(rx);
        }
        if fen.lecteur.is_some() || fen.calcul_taille.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        egui::Window::new("Corbeille").open(&mut ouverte).collapsible(false).default_size([560.0, 420.0]).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            if n == 0 {
                ui.label("La corbeille est vide. Un plan, un calque ou des dessins que tu retires du film arrivent ici : tu peux les reprendre tant que tu ne les supprimes pas pour de bon.");
                return;
            }
            let octets = fen.taille.map_or(0, |(_, o)| o);
            let place = if self.path.is_none() {
                "pas encore enregistré".to_string()
            } else if fen.taille.is_none_or(|(k, _)| k != n) {
                "place sur le disque : calcul…".to_string()
            } else if octets == 0 {
                "rien d'enregistré sur le disque pour l'instant".to_string()
            } else {
                format!("{} sur le disque", octets_texte(octets))
            };
            ui.label(egui::RichText::new(format!("{n} élément{} · {place}. Les dessins d'un élément restent dans le dossier du projet tant qu'il est là.", if n > 1 { "s" } else { "" })).weak());
            ui.separator();
            egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
                for e in self.doc.corbeille.entrees.iter().rev() {
                    ui.horizontal(|ui| {
                        let tex = fen.vignettes.get(&e.id).and_then(|t| t.as_ref());
                        match tex {
                            Some(t) => {
                                ui.image((t.id(), egui::vec2(VIGNETTE[0] as f32, VIGNETTE[1] as f32)));
                            }
                            None => {
                                let (r, _) = ui.allocate_exact_size(egui::vec2(VIGNETTE[0] as f32, VIGNETTE[1] as f32), egui::Sense::hover());
                                ui.painter().rect_stroke(r, 2.0, ui.visuals().widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
                            }
                        }
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&e.titre).strong());
                            let seq = if e.sequence_nom.is_empty() { String::new() } else { format!("{} · ", e.sequence_nom) };
                            ui.label(egui::RichText::new(format!("{seq}{}", crate::envois::date_courte(e.date))).weak());
                            ui.label(egui::RichText::new(&e.raison).weak().small());
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let s = ui.button("Supprimer…");
                            if crate::bulles::bulle(s, "Supprimer définitivement", "Définitif : ses dessins quitteront le dossier du projet au prochain enregistrement, et Ctrl+Z ne le rendra pas. Une confirmation te le redemande.", "").clicked() {
                                supprimer = Some(e.id);
                            }
                            let r = ui.button("Restaurer");
                            if crate::bulles::bulle(r, "Restaurer", "Un plan revient à sa place s'il en a encore une, sinon à la fin de sa séquence, sous son numéro s'il n'est à personne d'autre ; le dernier plan d'une séquence supprimée la ramène avec lui. Des calques reviennent dans leur plan. Une seule étape d'annulation.", "").clicked() {
                                restaurer = Some(e.id);
                            }
                        });
                    });
                    ui.separator();
                }
            });
            match fen.confirmation {
                Some(Confirmation::Supprimer(id)) => {
                    let titre = self.doc.corbeille.trouver(id).map_or_else(String::new, |e| e.titre.clone());
                    ui.colored_label(ui.visuals().warn_fg_color, format!("Supprimer « {titre} » définitivement ? Ça ne s'annule pas."));
                    ui.horizontal(|ui| {
                        if ui.button("Supprimer définitivement").clicked() {
                            self.corbeille_supprimer(Some(id));
                            fen.confirmation = None;
                        }
                        if ui.button("Annuler").clicked() {
                            fen.confirmation = None;
                        }
                    });
                }
                Some(Confirmation::Vider) => {
                    ui.colored_label(ui.visuals().warn_fg_color, format!("Vider la corbeille : {n} élément{} supprimé{} définitivement ? Ça ne s'annule pas.", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" }));
                    ui.horizontal(|ui| {
                        if ui.button("Vider la corbeille").clicked() {
                            self.corbeille_supprimer(None);
                            fen.confirmation = None;
                        }
                        if ui.button("Annuler").clicked() {
                            fen.confirmation = None;
                        }
                    });
                }
                None => {
                    let v = ui.button("Vider la corbeille…");
                    if crate::bulles::bulle(v, "Vider la corbeille", "Supprime définitivement tout ce qu'elle garde. Définitif : ça ne s'annule pas (avec confirmation).", "").clicked() {
                        fen.confirmation = Some(Confirmation::Vider);
                    }
                }
            }
        });
        if let Some(id) = supprimer {
            fen.confirmation = Some(Confirmation::Supprimer(id));
        }
        if let Some(id) = restaurer {
            self.corbeille_restaurer(id);
        }
        // Les vignettes d'entrées qui ne sont plus là partent.
        let presentes: BTreeSet<u64> = self.doc.corbeille.entrees.iter().map(|e| e.id).collect();
        fen.vignettes.retain(|id, _| presentes.contains(id));
        if ouverte {
            self.corbeille_ui = Some(fen);
        }
    }
}
