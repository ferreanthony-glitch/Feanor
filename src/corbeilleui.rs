//! La fenêtre « Corbeille » (menu Fichier, `fichier.corbeille`) : ce que les gestes du montage ont retiré du film, récupérable
//! (`corbeille.rs`). Une ligne par entrée : vignette, nom, séquence d'origine, date, pourquoi ; Restaurer, Supprimer
//! définitivement (avec confirmation) ; en bas, Vider. La fenêtre dit aussi la place que la corbeille prend sur le disque.
//! Restaurer et supprimer sont des étapes d'annulation comme les autres.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use eframe::egui::{self, Align2, Color32, TextureHandle, TextureOptions};

use crate::app::App;
use crate::corbeille::Entree;

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
    /// Place sur le disque : (nombre d'entrées au moment du calcul, octets).
    pub taille: Option<(usize, u64)>,
}

/// « 3,2 Mo », « 480 Ko ».
pub(crate) fn octets_texte(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1} Mo", n as f64 / 1_000_000.0).replace('.', ",")
    } else {
        format!("{} Ko", n.div_ceil(1000))
    }
}

impl App {
    fn dossier_de_la_corbeille(&self) -> Option<PathBuf> {
        self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()))
    }

    /// La vignette de l'entrée : son premier dessin, en mémoire ou relu du dossier du projet (rendu rapide), réduit.
    fn vignette_de(&self, ctx: &egui::Context, e: &Entree) -> Option<TextureHandle> {
        let id = e.vignette?;
        let paper = self.doc.paper;
        let faire = |raster: &crate::raster::Raster| {
            let [w, h] = VIGNETTE;
            let (rw, rh) = (raster.width.max(1), raster.height.max(1));
            let mut pixels = Vec::with_capacity(w * h);
            for y in 0..h {
                for x in 0..w {
                    let p = raster.get((x * rw / w).min(rw - 1), (y * rh / h).min(rh - 1));
                    let a = p.a() as f32 / 255.0;
                    let mix = |c: u8, bg: u8| (c as f32 + bg as f32 * (1.0 - a)).min(255.0) as u8;
                    pixels.push(Color32::from_rgb(mix(p.r(), paper[0]), mix(p.g(), paper[1]), mix(p.b(), paper[2])));
                }
            }
            ctx.load_texture(format!("corbeille-{}", e.id), egui::ColorImage { size: [w, h], pixels }, TextureOptions::LINEAR)
        };
        match self.doc.cels.get(&id) {
            Some(c) => Some(faire(&c.raster)),
            None => {
                let dir = self.dossier_de_la_corbeille()?;
                let c = crate::project::charger_un_dessin_avec(&dir, id, e.tete.largeur, e.tete.hauteur, true).ok()?;
                Some(faire(&c.raster))
            }
        }
    }

    /// Ouvre la fenêtre de la corbeille.
    pub(crate) fn ouvrir_la_corbeille(&mut self) {
        self.corbeille_ui = Some(Fenetre::default());
    }

    /// Remet l'entrée `id` dans le film : une étape d'annulation. Un plan revient à sa place s'il en a encore une, sinon à la fin de
    /// sa séquence ou de la séquence courante ; des calques reviennent dans leur plan s'il existe.
    pub(crate) fn corbeille_restaurer(&mut self, id: u64) {
        let Some(e) = self.doc.corbeille.trouver(id).cloned() else { return };
        let refuse = match e.sorte {
            crate::corbeille::Sorte::Plan => self.storyboard_refuses(),
            crate::corbeille::Sorte::Calques => self.content_refuses(),
        };
        if refuse {
            return;
        }
        let dir = self.dossier_de_la_corbeille();
        let (charge, avertissement) = {
            let doc = &self.doc;
            match crate::corbeille::charger(&e, dir.as_deref(), &|c| doc.cels.contains_key(&c)) {
                Ok(x) => x,
                Err(m) => {
                    self.status = m;
                    return;
                }
            }
        };
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
        let snap = self.snapshot();
        let n = match id {
            Some(id) => usize::from(self.doc.retirer_de_la_corbeille(id)),
            None => std::mem::take(&mut self.doc.corbeille.entrees).len(),
        };
        if n == 0 {
            return;
        }
        self.commit_as(snap, if id.is_some() { "Supprimer définitivement de la corbeille" } else { "Vider la corbeille" });
        self.status = match n {
            1 => "Supprimé définitivement de la corbeille.".into(),
            n => format!("Corbeille vidée : {n} éléments supprimés définitivement."),
        };
    }

    /// La fenêtre, à chaque image d'écran tant qu'elle est ouverte.
    pub(crate) fn corbeille_fenetre_ui(&mut self, ctx: &egui::Context) {
        let Some(mut fen) = self.corbeille_ui.take() else { return };
        let mut ouverte = true;
        let mut restaurer = None;
        let mut supprimer = None;
        let n = self.doc.corbeille.len();
        // La place sur le disque se recalcule quand le contenu change.
        if fen.taille.is_none_or(|(k, _)| k != n) {
            let ids: BTreeSet<_> = self.doc.corbeille.cels().collect();
            let octets = self.dossier_de_la_corbeille().map_or(0, |d| crate::project::taille_des_dessins(&d, &ids));
            fen.taille = Some((n, octets));
        }
        egui::Window::new("Corbeille").open(&mut ouverte).collapsible(false).default_size([560.0, 420.0]).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            if n == 0 {
                ui.label("La corbeille est vide. Un plan, un calque ou des dessins que tu retires du film arrivent ici : tu peux les reprendre tant que tu ne les supprimes pas pour de bon.");
                return;
            }
            let octets = fen.taille.map_or(0, |(_, o)| o);
            let place = if self.path.is_none() {
                "pas encore enregistré".to_string()
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
                        let tex = fen.vignettes.entry(e.id).or_insert_with(|| self.vignette_de(ctx, e));
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
                            if crate::bulles::bulle(s, "Supprimer définitivement", "Ses dessins quitteront le dossier du projet au prochain enregistrement. Ctrl+Z le rend tant que la séance dure.", "").clicked() {
                                supprimer = Some(e.id);
                            }
                            let r = ui.button("Restaurer");
                            if crate::bulles::bulle(r, "Restaurer", "Un plan revient à sa place s'il en a encore une, sinon à la fin de sa séquence ; des calques reviennent dans leur plan. Une seule étape d'annulation.", "").clicked() {
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
                    ui.colored_label(ui.visuals().warn_fg_color, format!("Supprimer « {titre} » définitivement ?"));
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
                    ui.colored_label(ui.visuals().warn_fg_color, format!("Vider la corbeille : {n} élément{} supprimé{} définitivement ?", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" }));
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
                    if crate::bulles::bulle(v, "Vider la corbeille", "Supprime définitivement tout ce qu'elle garde (avec confirmation).", "").clicked() {
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
