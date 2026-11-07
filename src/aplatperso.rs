//! La couleur d'aplat d'un calque de personnage vient du panneau Brosses dédiées (Anthony, 2026-10-29 : « Sur les calques personnages, l'aplat
//! ne change de couleur que si on modifie les paramètres de couleur de l'aplat dans le panneau Brosses dédiées »).
//!
//! # La règle
//!
//! - Un **calque de personnage** (`Layer::personnage = Some(id)`, `calquespastilles.rs`) a pour couleur d'aplat celle de **son**
//!   personnage, et rien d'autre ne la change : ni un trait fait sur ce calque avec une autre pastille (couleur neutre, pastille
//!   classique), ni « Cet aplat aussi sur… » / « Couleur du projet à tous les aplats » / un changement de couleur d'aplat avec la portée
//!   « Le plan » ou « Tout le storyboard » à partir d'un autre calque. `perso_aligner_couleur_aplat` ne fait que lui remettre la couleur de
//!   son personnage s'il en avait une autre ; un trait fait sur ce calque peint son aplat (`perso_poser`) de la couleur d'aplat de son
//!   personnage, quelle que soit la pastille. Dans Réglages (sous-ligne Aplat), la ligne « Couleur de l'aplat » montre la couleur sans la
//!   laisser modifier ; son renvoi (« Couleur de « Nom » : se change dans le panneau Brosses dédiées ») ouvre le panneau Brosses dédiées.
//! - Un calque dont le personnage n'existe plus (supprimé) est un calque ordinaire : sa couleur se change comme celle de n'importe quel
//!   aplat.
//! - Un calque **ordinaire** garde la règle des pastilles de `personnagesui.rs` (§33) : la couleur d'aplat du pinceau devient celle du
//!   calque.
//!
//! # Changer la couleur d'aplat d'un personnage
//!
//! Le changement (case de couleur du panneau Brosses dédiées, de la fenêtre « Modifier » ou du clic droit d'une pastille, pipette comprise : tout
//! passe par `perso_ecrire`) met à jour **tous les calques liés à ce personnage**, dans tous les plans du film **et dans toutes les
//! versions de plan** rangées (`planvers.rs`, et les plans du garage des versions de séquence, `variantes.rs`) :
//!
//! - la « Couleur de l'aplat » du calque (la forme de l'aplat, déduite du trait) ;
//! - la **peinture déjà posée dans la strate d'aplat** de chacun de leurs dessins : les pixels de l'**ancienne** couleur d'aplat du
//!   personnage prennent la nouvelle, leur opacité gardée. Les autres couleurs peintes à la main restent : le blanc d'un œil n'est pas touché.
//!
//! **Tolérance** (`TOLERANCE`) : deux niveaux par canal, mesurés sur les valeurs **prémultipliées** de la strate (celles qui sont
//! enregistrées). Un pixel de bord anticrénelé garde la couleur de l'aplat avec une opacité partielle : prémultiplié, il vaut
//! `couleur × opacité`, à l'arrondi près. Comparer à `ancienne couleur × opacité` (à deux niveaux près) reconnaît donc le bord du
//! pinceau, quelle que soit sa transparence, sans reconnaître un blanc peint à côté (à pleine opacité, l'écart du blanc à une couleur d'aplat
//! est de plusieurs dizaines de niveaux ; seuls les pixels presque transparents, invisibles, se confondent).
//!
//! Le geste est **une seule étape d'annulation** : comme les autres champs de couleur (`App::edited`), l'étape est celle du début du geste
//! (« Modifier le personnage ») ; pendant le glissé, seule la pastille suit ; la couleur des calques et celle de la peinture se posent quand
//! le geste s'arrête (plus de bouton enfoncé, une image sans nouveau changement), comme `aplatcouleur.rs`. La couleur des calques part avec
//! l'étape du geste (même cliché de la structure) ; la peinture, elle, est recolorée sur un fil de travail (**sans latence** : un projet de
//! plusieurs centaines de clés ne fige pas l'écran) et son résultat rejoint cette étape (`History::extend_last`), ou en fait une à lui si
//! quelque chose d'autre a été fait entre-temps. **L'enregistrement attend** que ce soit posé (`attendre_aplat_perso`, appelé par
//! `start_save`) ; un dessin retouché pendant le calcul est recolorié sur place, sans perdre ce qu'on y a fait.
//!
//! Ce qui n'est pas touché : la couleur du **trait** du personnage (hors sujet), les calques dont le personnage n'avait pas d'aplat avant
//! (leur couleur est alignée, sans toucher aux pixels), le contenu rogné des plans (`rognage.rs`), et rien quand le storyboard est verrouillé.
//! Aucun format de projet ne change.

use std::collections::BTreeSet;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use eframe::egui;

use crate::app::App;
use crate::document::{CelId, Document, Panel};
use crate::history::Edit;
use crate::personnages::Personnage;
use crate::raster::{premultiply, Raster, Tile};

/// Écart toléré, par canal prémultiplié, entre un pixel de la strate et l'ancienne couleur d'aplat (voir l'en-tête).
pub(crate) const TOLERANCE: i32 = 2;

/// Les pixels de `r` qui ont la couleur `old` (à `TOLERANCE` niveaux prémultipliés près) prennent `new`, leur opacité gardée. Rend les
/// tuiles d'avant (annulation) ; rien si rien n'a changé.
pub(crate) fn recolorer_strate(r: &mut Raster, old: [u8; 3], new: [u8; 3]) -> Vec<(usize, Option<Tile>)> {
    // Ce que l'ancienne couleur donne, prémultipliée, à chaque opacité.
    let attendu: Vec<[u8; 3]> = (0..=255u8)
        .map(|a| {
            let c = premultiply(old[0], old[1], old[2], a);
            [c.r(), c.g(), c.b()]
        })
        .collect();
    let proche = |c: eframe::egui::Color32| {
        let e = attendu[c.a() as usize];
        c.a() > 0 && (c.r() as i32 - e[0] as i32).abs() <= TOLERANCE && (c.g() as i32 - e[1] as i32).abs() <= TOLERANCE && (c.b() as i32 - e[2] as i32).abs() <= TOLERANCE
    };
    let mut avant = Vec::new();
    for id in r.drawn_ids() {
        let Some(tuile) = r.slot(id) else { continue };
        if !tuile.iter().any(|c| proche(*c)) {
            continue;
        }
        avant.push((id, Some(tuile.clone())));
        for c in r.tile_mut(id).iter_mut() {
            if proche(*c) {
                *c = premultiply(new[0], new[1], new[2], c.a());
            }
        }
    }
    avant
}

/// Un changement de couleur d'aplat de personnage retenu pendant le glissé de la case de couleur.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Demande {
    perso: u64,
    /// La couleur que la peinture a (celle du personnage avant le premier changement du geste) ; rien : le personnage n'avait pas d'aplat.
    old: Option<[u8; 3]>,
    new: [u8; 3],
    /// L'image d'écran du dernier changement.
    image: u64,
    /// Le numéro de l'historique juste après l'étape du geste.
    serial: u64,
}

/// Le dessin d'un calque de personnage à recolorier, sur le fil de travail.
struct Tache {
    cel: CelId,
    /// Les versions du dessin au départ : (dessin, strate) ; le résultat n'est posé tel quel que si rien n'a changé.
    versions: (u64, u64),
    peinture: Raster,
    old: [u8; 3],
}

/// Un dessin recolorié.
struct Rendu {
    cel: CelId,
    versions: (u64, u64),
    peinture: Raster,
    avant: Vec<(usize, Option<Tile>)>,
    old: [u8; 3],
    new: [u8; 3],
}

/// Ce que le fil rend pour un changement de couleur.
struct Lot(Vec<Rendu>);

/// L'état de la couleur d'aplat des personnages d'un projet (rangé avec l'onglet, dans `personnagesui::Etat`).
#[derive(Default)]
pub(crate) struct Etat {
    attente: Option<Demande>,
    /// Un clic sur le renvoi des réglages d'un calque de personnage : le panneau Brosses dédiées s'ouvre à l'image d'après.
    pub(crate) montrer_le_panneau: bool,
    canal: Option<(Sender<Lot>, Receiver<Lot>)>,
    /// Calculs en route.
    en_route: usize,
    /// Le numéro de l'historique où un résultat qui arrive rejoint l'étape du geste.
    serial: u64,
}

impl Etat {
    /// Essais : un changement est retenu ou un calcul est en route.
    #[cfg(test)]
    pub(crate) fn occupe(&self) -> bool {
        self.attente.is_some() || self.en_route > 0
    }
}

impl Document {
    /// Tous les plans qui portent des calques : ceux du film, les autres versions de chaque plan, et (`avec_variantes`) ceux du garage
    /// des versions de séquence avec leurs versions.
    fn plans_a_recolorier(&mut self, avec_variantes: bool) -> Vec<&mut Panel> {
        let mut out: Vec<&mut Panel> = self.panels.iter_mut().collect();
        for v in self.versions.values_mut() {
            out.extend(v.autres.iter_mut().map(|r| &mut r.panel));
        }
        if avec_variantes {
            for v in Arc::make_mut(&mut self.variantes).values_mut() {
                for p in v.garage.values_mut() {
                    out.push(&mut p.panel);
                    if let Some(vs) = p.versions.as_mut() {
                        out.extend(vs.autres.iter_mut().map(|r| &mut r.panel));
                    }
                }
            }
        }
        for g in &mut self.annules.plans {
            let g = Arc::make_mut(g);
            out.push(&mut g.panel);
            if let Some(v) = g.versions.as_mut() {
                out.extend(v.autres.iter_mut().map(|r| &mut r.panel));
            }
        }
        out
    }

    /// Un plan du garage des versions de séquence a-t-il un calque de ce personnage ?
    fn garage_a_un_calque_de(&self, id: u64) -> bool {
        self.plans_de_variantes().any(|p| p.layers.iter().any(|l| l.personnage == Some(id)))
    }

    /// Tous les plans (film, autres versions, garage), en lecture.
    #[cfg(test)]
    pub(crate) fn tous_les_plans(&self) -> Vec<&Panel> {
        self.panels.iter().chain(self.plans_ranges()).collect()
    }
}

impl App {
    /// Le personnage du calque `li`, s'il en a un et qu'il existe encore dans le projet.
    pub(crate) fn perso_du_calque(&self, li: usize) -> Option<&Personnage> {
        let id = self.layers().get(li)?.personnage?;
        self.doc.personnages.iter().find(|p| p.id == id)
    }

    /// Le calque `li` a pour couleur d'aplat celle de son personnage : la ligne « Couleur de l'aplat » ne se modifie pas.
    pub(crate) fn aplat_vient_du_personnage(&self, li: usize) -> bool {
        self.perso_du_calque(li).is_some()
    }

    /// La couleur d'aplat que doit avoir le calque `li` d'après son personnage ; rien pour un calque ordinaire (ou si son personnage n'a
    /// pas d'aplat).
    pub(crate) fn aplat_couleur_imposee(&self, li: usize) -> Option<[u8; 3]> {
        self.perso_du_calque(li).and_then(|p| p.fill)
    }

    /// Le réglage d'aplat `modele` tel que le calque `li` le reçoit (« Ajouter un aplat », « Cet aplat aussi sur… ») : un calque de
    /// personnage garde la couleur de son personnage (ou, si son personnage n'a pas d'aplat, celle qu'il avait), le reste du réglage
    /// (débord, trous…) vient du modèle.
    pub(crate) fn aplat_pour_calque(&self, li: usize, modele: crate::fond::FondReglages) -> crate::fond::FondReglages {
        if !self.aplat_vient_du_personnage(li) {
            return modele;
        }
        let couleur = self.aplat_couleur_imposee(li).or_else(|| self.layers().get(li).and_then(|l| l.fond).map(|f| f.couleur)).unwrap_or(modele.couleur);
        crate::fond::FondReglages { couleur, ..modele }
    }

    /// La couleur d'aplat de `p` vient de changer de `avant` à `apres` : retenue jusqu'à la fin du geste (`tick_aplat_perso`). À appeler après
    /// l'étape d'annulation du changement (`perso_ecrire`, appelé par les champs de couleur qui passent par `App::edited`).
    pub(crate) fn aplat_perso_demander(&mut self, perso: u64, avant: Option<[u8; 3]>, apres: [u8; 3]) {
        let image = self.ctx.cumulative_pass_nr();
        let serial = self.history.serial();
        let old = match self.perso.aplat.attente {
            Some(d) if d.perso == perso => d.old,
            _ => avant,
        };
        self.perso.aplat.attente = Some(Demande { perso, old, new: apres, image, serial });
    }

    /// À chaque image d'écran : les résultats arrivés sont posés, et un changement retenu dont la case de couleur n'est plus tirée (bouton
    /// relâché, pas de nouveau changement depuis l'image d'avant, pas de calcul en route) est lancé. Rien au repos.
    pub(crate) fn tick_aplat_perso(&mut self) {
        if std::mem::take(&mut self.perso.aplat.montrer_le_panneau) {
            self.show_pane(crate::dock::Pane::Personnages);
        }
        self.aplat_perso_recevoir();
        let Some(d) = self.perso.aplat.attente else { return };
        if self.perso.aplat.en_route > 0 || self.ctx.input(|i| i.pointer.any_down()) || self.ctx.cumulative_pass_nr() <= d.image {
            self.ctx.request_repaint_after(std::time::Duration::from_millis(16));
            return;
        }
        self.aplat_perso_lancer();
    }

    /// Pose le changement retenu : la couleur des calques tout de suite, la peinture de leurs strates sur un fil de travail.
    fn aplat_perso_lancer(&mut self) {
        let Some(d) = self.perso.aplat.attente.take() else { return };
        let Some(p) = self.doc.personnages.iter().find(|p| p.id == d.perso) else { return };
        // (Le personnage a changé de couleur depuis, ou a disparu : une annulation est passée par là ; rien à suivre.)
        if p.fill != Some(d.new) {
            return;
        }
        if self.doc.storyboard_locked {
            self.status = "Storyboard verrouillé : les calques de ce personnage gardent l'ancienne couleur d'aplat.".into();
            return;
        }
        self.end_stroke();
        let id = d.perso;
        let (new, old) = (d.new, d.old);
        // Ce qui change : des calques de couleur d'aplat différente, et les dessins de leurs strates.
        let mut calques = 0usize;
        let mut taches: Vec<Tache> = Vec::new();
        let mut vus: BTreeSet<CelId> = BTreeSet::new();
        for plan in self.doc.panels.iter().chain(self.doc.plans_ranges()) {
            for l in plan.layers.iter().filter(|l| l.personnage == Some(id) && !l.folder) {
                let Some(f) = l.fond else { continue };
                if f.couleur != new {
                    calques += 1;
                }
                let depart = old.unwrap_or(f.couleur);
                if depart == new {
                    continue;
                }
                for &cel in l.frames.iter().flatten() {
                    if !vus.insert(cel) {
                        continue;
                    }
                    let Some(s) = self.doc.cels.get(&cel).and_then(|c| c.fond.as_deref()) else { continue };
                    if s.peinture.raster.is_blank() {
                        continue;
                    }
                    let versions = (self.doc.cels[&cel].version, s.peinture.version);
                    taches.push(Tache { cel, versions, peinture: s.peinture.raster.clone(), old: depart });
                }
            }
        }
        if calques == 0 && taches.is_empty() {
            return;
        }
        // Le geste a son étape ; si autre chose s'est fait depuis, ce changement a la sienne (la couleur des calques fait partie de la structure).
        if self.history.serial() != d.serial {
            self.record_named_step("Couleur d'aplat du personnage");
        }
        self.perso.aplat.serial = self.history.serial();
        let avec_variantes = self.doc.garage_a_un_calque_de(id);
        for plan in self.doc.plans_a_recolorier(avec_variantes) {
            for l in plan.layers.iter_mut().filter(|l| l.personnage == Some(id) && !l.folder) {
                if let Some(f) = l.fond.as_mut() {
                    f.couleur = new;
                }
            }
        }
        self.modified = true;
        self.renderer.invalidate_all();
        let n = taches.len();
        if !taches.is_empty() {
            let tx = self.perso.aplat.canal.get_or_insert_with(channel::<Lot>).0.clone();
            let ctx = self.ctx.clone();
            let lance = std::thread::Builder::new().name("aplat-personnage".into()).spawn(move || {
                let mut rendus = Vec::new();
                for Tache { cel, versions, mut peinture, old } in taches {
                    let avant = recolorer_strate(&mut peinture, old, new);
                    if !avant.is_empty() {
                        rendus.push(Rendu { cel, versions, peinture, avant, old, new });
                    }
                }
                let _ = tx.send(Lot(rendus));
                ctx.request_repaint();
            });
            if lance.is_ok() {
                self.perso.aplat.en_route += 1;
            }
        }
        self.status = format!("Aplat de « {} » : {} calque{} et {} dessin{} (calcul en arrière-plan).", self.nom_de_perso(id), calques, if calques > 1 { "s" } else { "" }, n, if n > 1 { "s" } else { "" });
    }

    fn nom_de_perso(&self, id: u64) -> String {
        self.doc.personnages.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default()
    }

    /// Pose les résultats arrivés du fil de travail ; leurs étapes d'annulation rejoignent celle du geste.
    pub(crate) fn aplat_perso_recevoir(&mut self) {
        if self.perso.aplat.en_route == 0 {
            return;
        }
        let Some((_, rx)) = self.perso.aplat.canal.as_ref() else { return };
        let arrives: Vec<Lot> = rx.try_iter().collect();
        for Lot(rendus) in arrives {
            self.perso.aplat.en_route = self.perso.aplat.en_route.saturating_sub(1);
            self.aplat_perso_poser(rendus);
        }
        if self.perso.aplat.en_route > 0 {
            self.ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    /// Pose des dessins recoloriés. Un dessin retouché depuis le départ du calcul est recolorié sur place (rare : un seul dessin à la fois).
    fn aplat_perso_poser(&mut self, rendus: Vec<Rendu>) {
        let mut edits: Vec<Edit> = Vec::new();
        for r in rendus {
            let Some(c) = self.doc.cels.get_mut(&r.cel) else { continue };
            let Some(s) = c.fond.as_mut() else { continue };
            let (peinture, avant) = if (c.version, s.peinture.version) == r.versions {
                (r.peinture, r.avant)
            } else {
                let mut p = s.peinture.raster.clone();
                let avant = recolorer_strate(&mut p, r.old, r.new);
                (p, avant)
            };
            if avant.is_empty() {
                continue;
            }
            s.peinture.raster = peinture;
            s.peinture.version += 1;
            c.version += 1;
            edits.push(Edit::FondTiles { cel: r.cel, tiles: avant, coupe: false });
        }
        if edits.is_empty() {
            return;
        }
        let avant_serial = self.history.serial();
        if avant_serial == self.perso.aplat.serial {
            self.history.extend_last(Edit::Group(edits));
            self.gesture.suivre(avant_serial, self.history.serial());
        } else {
            let contexte = self.history.context.replace("Couleur d'aplat du personnage".to_string());
            self.history.push(Edit::Group(edits));
            self.history.context = contexte;
        }
        self.perso.aplat.serial = self.history.serial();
        self.modified = true;
        self.renderer.invalidate_all();
    }

    /// Lance tout de suite le changement retenu (même si la case de couleur est encore tirée) et attend que ses résultats soient posés :
    /// avant d'enregistrer (le fichier doit les avoir) et dans les essais.
    pub(crate) fn attendre_aplat_perso(&mut self) {
        let debut = std::time::Instant::now();
        let limite = std::time::Duration::from_secs(60);
        loop {
            self.aplat_perso_recevoir();
            if self.perso.aplat.en_route > 0 {
                if debut.elapsed() >= limite {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
                continue;
            }
            if self.perso.aplat.attente.is_none() {
                return;
            }
            self.aplat_perso_lancer();
        }
    }
}

impl App {
    /// La ligne « Couleur de l'aplat » d'un calque de personnage, qui ne se modifie pas : la couleur du personnage, le renvoi au panneau
    /// Personnages (un clic l'ouvre), et la portée, grisée.
    pub(crate) fn aplat_couleur_du_personnage_ui(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some((nom, couleur)) = self.perso_du_calque(li).map(|p| (p.name.clone(), p.fill)) else { return };
        let couleur = couleur.or_else(|| self.layers().get(li).and_then(|l| l.fond).map(|f| f.couleur)).unwrap_or([255, 255, 255]);
        ui.horizontal(|ui| {
            ui.label("Couleur de l'aplat");
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(34.0, 16.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 3.0, egui::Color32::from_rgb(couleur[0], couleur[1], couleur[2]));
            ui.painter().rect_stroke(rect, 3.0, egui::Stroke::new(1.0_f32, crate::theme::bord()), egui::StrokeKind::Inside);
            crate::bulles::bulle(resp, "Couleur de l'aplat", "Elle vient du personnage de ce calque : on la change dans le panneau Brosses dédiées, et tous ses calques, dans tous les plans, la suivent.", "");
        });
        let nom = crate::calquespastilles::nom_du_calque(&nom);
        let renvoi = egui::RichText::new(format!("Couleur de « {nom} » : se change dans le panneau Brosses dédiées")).small().underline();
        // (Un bouton sans cadre : un lien. Un `Label` cliquable ne recevait pas l'appui.)
        // (Sans bulle : une bulle sur ce bouton mangeait l'appui, essai à l'appui ; le texte dit déjà ce qu'il fait.)
        let resp = ui.add(egui::Button::new(renvoi).frame(false)).on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() {
            // (Pas tout de suite : le panneau Réglages est en train d'être dessiné par le dock, qui garde sa disposition ; 	ick_aplat_perso l'ouvre à l'image d'après.)
            self.perso.aplat.montrer_le_panneau = true;
            ui.ctx().request_repaint();
        }
        // La portée du changement de couleur : celle d'un personnage est toujours « tous ses calques, dans tous les plans ».
        ui.add_enabled_ui(false, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Changer la couleur de l'aplat sur :").small());
                for p in crate::aplatcouleur::AplatPortee::TOUS {
                    let _ = ui.selectable_label(false, egui::RichText::new(p.label()).small());
                }
            });
        })
        .response
        .on_disabled_hover_text("Le changement se fait dans le panneau Brosses dédiées et touche tous les calques du personnage, dans tous les plans et toutes les versions de plan.");
    }
}
