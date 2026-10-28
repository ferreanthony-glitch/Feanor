//! Coller ce qui vient d'un autre projet (`adaptation.rs` en dit la règle) : la question quand il y a un choix,
//! le calcul sur un fil de fond, la reprise du collage une fois la copie adaptée.
//!
//! # Le parcours
//!
//! Chaque collage (`paste_panels`, `paste_layer`, `paste_images`, `paste_keys_with`, `bitmap_paste`, `paste`, `multi_paste`,
//! clés de caméra, clés d'effets, effets) commence par `collage_preparer(copie, reprise)` :
//!
//! 1. La copie vient d'un projet de même cadre et de même feuille (et, si les durées sont gardées en images, de
//!    n'importe quelle cadence) : rien à faire, le collage continue tel quel (comportement d'avant, essais à l'appui).
//! 2. Sinon, pour un plan, un calque, des images ou des clés, et si les **proportions** des cadres ou les **cadences**
//!    diffèrent, une petite fenêtre demande (Contenir / Remplir ; garder les images / garder la durée en secondes) ; ce
//!    qu'on choisit est retenu dans les réglages, et « Ne plus demander » les applique ensuite sans question.
//! 3. La copie est adaptée. Ce qui porte des dessins (plans, calque, images, clés, grande zone) est rééchantillonné sur
//!    des fils de fond : « Collage en cours… » dans la barre d'état, avec l'avancement ; l'interface continue de vivre.
//!    Le reste (clés de caméra et d'effets, effets, éléments vectoriels, petites zones) est calculé tout de suite.
//! 4. Le collage reprend (`collage_reprendre`) : la fonction de collage habituelle est rappelée, qui retrouve la copie
//!    adaptée dans `Etat::pret` au lieu du presse-papiers. **Une seule étape d'annulation**, nommée « Coller (adapté de
//!    3840×2160) », et la barre d'état dit ce qui a été adapté.
//!
//! Le presse-papiers n'est jamais modifié : la copie d'origine sert encore, pour un autre projet ou une autre taille.
//!
//! # Pendant le calcul
//!
//! `modal_open` est vrai (raccourcis et bande d'onglets figés) et `onglet_occupe` refuse de changer d'onglet : le collage
//! se pose dans le plan, le calque et l'image où il a été demandé (`Ancre`). Un clic de souris reste possible, il ne
//! change rien à ce qui va être collé. Annuler (Ctrl+Z) attend la fin.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};

use crate::adaptation::{self as ad, Adaptation, Ajustement, Duree, Origine, Progression};
use crate::app::{App, LayerClip};
use crate::camkeys::CamClip;
use crate::document::PanelClip;
use crate::fxcopie::FxStackClip;
use crate::fxkeys::FxClip;
use crate::images::ImageClip;
use crate::montagekeys::KeyClip;
use crate::multisel::MultiClip;
use crate::pixsel::Floating;
use crate::vector::Element;

/// Ce qui est collé.
#[derive(Clone)]
pub(crate) enum Colle {
    Plans(PanelClip),
    Calque(LayerClip),
    Images(ImageClip),
    Cles(KeyClip),
    Zone(Floating),
    Elements(Vec<Element>),
    Multi(MultiClip),
    CamKeys(CamClip),
    FxKeys(FxClip),
    FxPile(FxStackClip),
}

/// Comment le collage reprend : la fonction à rappeler, avec ses réglages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Reprise {
    Plans,
    Calque,
    Images { replace: bool, at_head: bool },
    Cles { insert: bool },
    Zone,
    Elements,
    Multi,
    CamKeys { tete: bool },
    FxKeys { insert: bool },
    FxPile { targets: Vec<usize>, replace: bool },
}

impl Reprise {
    /// Le nom de l'étape d'annulation d'un collage adapté.
    fn nom(&self, src: &Origine) -> String {
        let quoi = match self {
            Reprise::Plans => "des plans",
            Reprise::Calque => "un calque",
            Reprise::Images { .. } => "des images",
            Reprise::Cles { .. } => "des clés",
            Reprise::Zone => "une zone",
            Reprise::Elements => "des éléments",
            Reprise::Multi => "une sélection",
            Reprise::CamKeys { .. } => "des clés de caméra",
            Reprise::FxKeys { .. } => "des clés d'effets",
            Reprise::FxPile { .. } => "des effets",
        };
        format!("Coller {quoi} (adapté de {}×{})", src.cadre[0], src.cadre[1])
    }
}

/// Ce que donne `collage_preparer`.
pub(crate) enum Preparation {
    /// La copie à coller maintenant (la même, ou sa version adaptée au rappel).
    Pret(Colle),
    /// Le collage reprendra seul (question posée, calcul en cours, ou déjà fait) : rien à faire maintenant.
    EnCours,
}

/// D'où viennent les copies qui ne portent pas leur origine (les presse-papiers d'éléments, de zones et de clés).
#[derive(Default)]
pub(crate) struct Origines {
    pub camera: Option<Origine>,
    pub effets_cles: Option<Origine>,
    pub pile: Option<Origine>,
    pub elements: Option<Origine>,
    pub zone: Option<Origine>,
    pub multi: Option<Origine>,
}

/// Où le collage a été demandé (identifiants : le plan, le calque et l'image peuvent bouger pendant le calcul).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ancre {
    pub panel: u64,
    pub layer: u64,
    pub frame: usize,
}

/// La question (proportions, cadence), quand il y a un choix.
pub(crate) struct Dialogue {
    pub colle: Colle,
    pub reprise: Reprise,
    pub src: Origine,
    pub dst: Origine,
    pub ajustement: Ajustement,
    pub duree: Duree,
    pub ne_plus_demander: bool,
}

/// Le calcul en arrière-plan.
pub(crate) struct Tache {
    rx: Receiver<Option<Colle>>,
    reprise: Reprise,
    resume: String,
    src: Origine,
    ancre: Ancre,
    progression: Arc<Progression>,
    debut: Instant,
}

/// Tout l'état du collage d'un autre projet : un seul à la fois, commun à tous les onglets (le calcul interdit de changer
/// d'onglet, `onglet_occupe`).
#[derive(Default)]
pub(crate) struct Etat {
    pub origines: Origines,
    pub dialogue: Option<Dialogue>,
    pub tache: Option<Tache>,
    /// La copie adaptée, que le collage rappelé prend à la place du presse-papiers.
    pub pret: Option<(Colle, Reprise)>,
}

impl Colle {
    fn origine(&self, o: &Origines) -> Option<Origine> {
        match self {
            Colle::Plans(c) => Some(c.origine),
            Colle::Calque(c) => Some(c.origine),
            Colle::Images(c) => Some(c.origine),
            Colle::Cles(c) => Some(c.origine),
            Colle::Zone(_) => o.zone,
            Colle::Elements(_) => o.elements,
            Colle::Multi(_) => o.multi,
            Colle::CamKeys(_) => o.camera,
            Colle::FxKeys(_) => o.effets_cles,
            Colle::FxPile(_) => o.pile,
        }
    }

    /// La question « contenir ou remplir ? garder les images ou les secondes ? » se pose pour ce qui porte des dessins.
    fn demande(&self) -> bool {
        matches!(self, Colle::Plans(_) | Colle::Calque(_) | Colle::Images(_) | Colle::Cles(_))
    }

    /// Le calcul passe par un fil de fond (des dessins entiers à rééchantillonner).
    fn lourd(&self) -> bool {
        match self {
            Colle::Plans(_) | Colle::Calque(_) | Colle::Images(_) | Colle::Cles(_) => true,
            Colle::Zone(f) => f.w * f.h > ZONE_LOURDE,
            Colle::Multi(m) => m.items.iter().any(|(_, w)| matches!(w, crate::multisel::ClipWhat::Pixels(f) if f.w * f.h > ZONE_LOURDE)),
            _ => false,
        }
    }

    /// La copie adaptée ; rien si elle ne laisse rien à coller (une zone entièrement hors de la feuille).
    fn adapter(self, a: &Adaptation, p: &Progression) -> Option<Colle> {
        Some(match self {
            Colle::Plans(c) => Colle::Plans(ad::adapte_plans(&c, a, p)),
            Colle::Calque(c) => Colle::Calque(ad::adapte_calque_copie(&c, a, p)),
            Colle::Images(c) => Colle::Images(ad::adapte_images_copiees(&c, a, p)),
            Colle::Cles(c) => Colle::Cles(ad::adapte_cles_copiees(&c, a, p)),
            Colle::Zone(f) => Colle::Zone(ad::adapte_zone(&f, a)?),
            Colle::Elements(e) => Colle::Elements(ad::adapte_elements(&e, a)),
            Colle::Multi(m) => {
                let r = ad::adapte_multi(&m, a);
                if r.items.is_empty() {
                    return None;
                }
                Colle::Multi(r)
            }
            Colle::CamKeys(c) => Colle::CamKeys(ad::adapte_cles_camera(&c, a)),
            Colle::FxKeys(c) => Colle::FxKeys(ad::adapte_cles_effet(&c, a)),
            Colle::FxPile(c) => Colle::FxPile(ad::adapte_pile_effets(&c, a)),
        })
    }
}

/// Au-dessus de cette surface (pixels), une zone bitmap est rééchantillonnée sur un fil de fond.
pub(crate) const ZONE_LOURDE: usize = 1_000_000;

impl App {
    /// Au début de chaque collage : la copie à coller maintenant, ou rien (le collage reprendra seul). Voir le module.
    pub(crate) fn collage_preparer(&mut self, colle: Colle, reprise: Reprise) -> Preparation {
        // Le rappel d'une copie adaptée.
        if let Some((adaptee, r)) = self.collage.pret.take() {
            if r == reprise {
                return Preparation::Pret(adaptee);
            }
        }
        if self.collage.tache.is_some() || self.collage.dialogue.is_some() {
            self.status = "Un collage est déjà en cours.".into();
            return Preparation::EnCours;
        }
        let dst = Origine::de(&self.doc);
        let Some(src) = colle.origine(&self.collage.origines) else { return Preparation::Pret(colle) };
        let reg = self.settings.collage;
        // Rien à choisir (mêmes cadre et feuille, même cadence) ou rien à demander : le réglage retenu s'applique.
        let choix = src.autres_proportions(&dst) || src.cadence != dst.cadence;
        if colle.demande() && reg.demander && choix {
            self.collage.dialogue = Some(Dialogue { colle, reprise, src, dst, ajustement: reg.ajustement, duree: reg.duree, ne_plus_demander: false });
            self.status = "Coller depuis un autre projet : choisis comment l'adapter.".into();
            return Preparation::EnCours;
        }
        let a = Adaptation::new(src, dst, reg.ajustement, reg.duree);
        if a.identite() {
            return Preparation::Pret(colle);
        }
        self.collage_lancer(colle, reprise, a);
        Preparation::EnCours
    }

    /// Adapte la copie : tout de suite si c'est léger, sur un fil de fond sinon.
    fn collage_lancer(&mut self, colle: Colle, reprise: Reprise, a: Adaptation) {
        if a.identite() {
            self.collage_reprendre(Some(colle), reprise, a.src, String::new());
            return;
        }
        let resume = a.resume();
        if !colle.lourd() {
            let adaptee = colle.adapter(&a, &Progression::default());
            self.collage_reprendre(adaptee, reprise, a.src, resume);
            return;
        }
        let progression = Arc::new(Progression::default());
        let (tx, rx) = channel();
        let p = progression.clone();
        let ok = std::thread::Builder::new().name("collage".into()).spawn(move || {
            let _ = tx.send(colle.adapter(&a, &p));
        });
        if ok.is_err() {
            self.status = "Collage impossible : le calcul n'a pas pu démarrer.".into();
            return;
        }
        let ancre = Ancre { panel: self.panel().id, layer: self.layer().id, frame: self.cur_frame };
        self.collage.tache = Some(Tache { rx, reprise, resume, src: a.src, ancre, progression, debut: Instant::now() });
        self.status = "Collage en cours… mise à l'échelle des dessins.".into();
    }

    /// Rappelle la fonction de collage avec la copie adaptée : une étape d'annulation nommée, puis le message d'état.
    fn collage_reprendre(&mut self, adaptee: Option<Colle>, reprise: Reprise, src: Origine, resume: String) {
        let Some(adaptee) = adaptee else {
            self.status = "Rien à coller : la copie sort entièrement de la feuille de ce projet.".into();
            return;
        };
        self.collage.pret = Some((adaptee, reprise.clone()));
        let nom = if resume.is_empty() { None } else { Some(reprise.nom(&src)) };
        let avant = nom.as_ref().map(|n| self.history.context.replace(n.clone()));
        let serial = self.history.serial();
        match reprise {
            Reprise::Plans => self.paste_panels(),
            Reprise::Calque => self.paste_layer(),
            Reprise::Images { replace, at_head } => self.paste_images_with(replace, at_head),
            Reprise::Cles { insert } => self.paste_keys_with(insert),
            Reprise::Zone => self.bitmap_paste(),
            Reprise::Elements => self.paste(),
            Reprise::Multi => self.multi_paste(),
            Reprise::CamKeys { tete } => {
                if tete {
                    self.paste_cam_keys_at_head()
                } else {
                    self.paste_cam_keys()
                }
            }
            Reprise::FxKeys { insert } => {
                if insert {
                    self.paste_fx_keys()
                } else {
                    self.paste_fx_keys_at_head()
                }
            }
            Reprise::FxPile { targets, replace } => self.fx_paste(targets, replace),
        }
        if let Some(avant) = avant {
            self.history.context = avant;
        }
        // (Rien de non consommé ne reste : un collage refusé avant de lire la copie ne la garde pas.)
        self.collage.pret = None;
        if !resume.is_empty() && self.history.serial() != serial {
            self.status = format!("{resume} {}", self.status);
        }
    }

    /// À chaque image d'écran : le calcul est-il fini ?
    pub(crate) fn tick_collage(&mut self, ctx: &egui::Context) {
        let Some(t) = &self.collage.tache else { return };
        match t.rx.try_recv() {
            Ok(resultat) => {
                let Some(t) = self.collage.tache.take() else { return };
                // Le collage se pose là où il a été demandé.
                if let Some(pi) = self.doc.panels.iter().position(|p| p.id == t.ancre.panel) {
                    self.cur_panel = pi;
                    if let Some(li) = self.layers().iter().position(|l| l.id == t.ancre.layer) {
                        self.cur_layer = li;
                    }
                    self.cur_frame = t.ancre.frame.min(self.frame_count().saturating_sub(1));
                }
                self.collage_reprendre(resultat, t.reprise, t.src, t.resume);
            }
            Err(TryRecvError::Empty) => {
                let fait = t.progression.fait.load(Ordering::Relaxed);
                let total = t.progression.total.load(Ordering::Relaxed);
                let secondes = t.debut.elapsed().as_secs_f32();
                self.status = if total > 0 {
                    format!("Collage en cours… {fait}/{total} dessins ({secondes:.1} s).")
                } else {
                    "Collage en cours…".into()
                };
                ctx.request_repaint_after(Duration::from_millis(30));
            }
            Err(TryRecvError::Disconnected) => {
                self.collage.tache = None;
                self.status = "Le collage a échoué : le calcul s'est arrêté.".into();
            }
        }
    }

    /// Un collage attend (question ou calcul) : on ne change pas d'onglet.
    pub(crate) fn collage_occupe(&self) -> bool {
        self.collage.tache.is_some() || self.collage.dialogue.is_some()
    }

    /// « Coller » dans la fenêtre : les choix sont retenus, le calcul part.
    pub(crate) fn collage_valider(&mut self) {
        let Some(d) = self.collage.dialogue.take() else { return };
        self.settings.collage.ajustement = d.ajustement;
        self.settings.collage.duree = d.duree;
        if d.ne_plus_demander {
            self.settings.collage.demander = false;
        }
        if let Err(e) = self.settings.save() {
            self.status = format!("Réglages non enregistrés : {e}");
        }
        let a = Adaptation::new(d.src, d.dst, d.ajustement, d.duree);
        self.collage_lancer(d.colle, d.reprise, a);
    }

    /// « Annuler » dans la fenêtre : rien n'est collé, la copie reste dans le presse-papiers.
    pub(crate) fn collage_annuler(&mut self) {
        if self.collage.dialogue.take().is_some() {
            self.status = "Collage annulé.".into();
        }
    }

    /// La petite fenêtre : proportions et cadence.
    pub(crate) fn collage_fenetre(&mut self, ctx: &egui::Context) {
        let Some(d) = self.collage.dialogue.as_mut() else { return };
        let (mut coller, mut annuler) = (false, false);
        let proportions = d.src.autres_proportions(&d.dst);
        let cadences = d.src.cadence != d.dst.cadence;
        let apercu = Adaptation::new(d.src, d.dst, d.ajustement, d.duree).resume();
        egui::Window::new("Coller depuis un autre projet")
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.screen_rect().center() - egui::vec2(230.0, 110.0))
            .show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.label(format!(
                    "La copie vient d'un projet de {}×{} px à {} i/s ; celui-ci fait {}×{} px à {} i/s.",
                    d.src.cadre[0], d.src.cadre[1], d.src.cadence, d.dst.cadre[0], d.dst.cadre[1], d.dst.cadence
                ));
                if proportions {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Proportions différentes").strong());
                    ui.radio_value(&mut d.ajustement, Ajustement::Contenir, "Contenir : tout le cadre copié reste visible, avec des bandes si besoin");
                    ui.radio_value(&mut d.ajustement, Ajustement::Remplir, "Remplir : le cadre de ce projet est rempli, ce qui dépasse sort du cadre");
                }
                if cadences {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Cadence différente").strong());
                    ui.radio_value(&mut d.duree, Duree::Images, "Garder les images : la durée en secondes change");
                    ui.radio_value(&mut d.duree, Duree::Secondes, "Garder la durée en secondes : les images sont recalculées");
                }
                ui.add_space(6.0);
                ui.label(RichText::new(&apercu).weak().small());
                ui.checkbox(&mut d.ne_plus_demander, "Ne plus demander (Fichier > Préférences… pour changer)");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    coller = ui.button("Coller").clicked();
                    annuler = ui.button("Annuler").clicked();
                });
            });
        if coller {
            self.collage_valider();
        } else if annuler {
            self.collage_annuler();
        }
    }

    /// Les réglages du collage dans les préférences (Projets).
    pub(crate) fn collage_prefs_ui(&mut self, ui: &mut egui::Ui) {
        let reg = &mut self.settings.collage;
        let avant = *reg;
        ui.horizontal(|ui| {
            ui.label("Proportions différentes :");
            ui.radio_value(&mut reg.ajustement, Ajustement::Contenir, "Contenir").on_hover_text("Tout le dessin collé reste visible ; des bandes vides peuvent rester autour.");
            ui.radio_value(&mut reg.ajustement, Ajustement::Remplir, "Remplir").on_hover_text("Le dessin collé remplit tout le cadre ; ses bords peuvent être coupés.");
        });
        ui.horizontal(|ui| {
            ui.label("Cadence différente :");
            ui.radio_value(&mut reg.duree, Duree::Images, "Garder les images").on_hover_text("Chaque image collée reste une image : le plan change de durée.");
            ui.radio_value(&mut reg.duree, Duree::Secondes, "Garder la durée en secondes").on_hover_text("Le plan collé dure autant de secondes qu'avant : le nombre d'images change.");
        });
        ui.checkbox(&mut reg.demander, "Demander à chaque collage dont les proportions ou la cadence diffèrent")
            .on_hover_text("Décoché, le collage suivant se fait sans fenêtre, avec les choix ci-dessus.");
        if *reg != avant {
            if let Err(e) = self.settings.save() {
                self.status = format!("Réglages non enregistrés : {e}");
            }
        }
    }
}
