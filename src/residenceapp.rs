//! Les dessins en mémoire, côté application (`residence.rs` pour le modèle) : à chaque image d'écran, les pixels arrivés
//! du chargeur sont posés, ce qui est voulu (le plan courant, ses voisins, ce que la lecture va montrer, les prochaines
//! annulations) est demandé, et au-delà du budget les dessins les moins récemment voulus sont laissés. Rien n'attend le
//! disque sur le fil de l'interface : laisser un dessin, c'est remplacer son image par une image vide ; le faire revenir,
//! c'est une demande au chargeur (bitmap) ou au rejeu (image d'un dessin vectoriel).

use std::collections::HashSet;
use std::sync::Arc;

use eframe::egui;

use crate::app::App;
use crate::document::{Cel, CelId};
use crate::raster::Raster;
use crate::residence::{demander, Absent, Arrivee, Source, Stats, Tache, Travail, OCTETS_TUILE, TOUR};

/// Laisser au plus ce nombre de dessins par contrôle (le reste au contrôle suivant) : l'image d'écran reste courte.
const LAISSES_PAR_CONTROLE: usize = 256;

/// Contrôle du budget toutes les `CONTROLE` images d'écran (et dès que des pixels arrivent).
const CONTROLE: u64 = 15;

#[cfg(test)]
thread_local! {
    /// Essais : images où un dessin à montrer n'était pas revenu et où la vignette du plan l'a remplacé…
    pub(crate) static IMAGES_EN_ATTENTE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// …et celles où il n'y avait même pas de vignette (une image vide).
    pub(crate) static IMAGES_VIDES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl App {
    /// Les dessins voulus en mémoire, les plus pressés d'abord : l'image à l'écran, le plan courant (toutes ses images, sa
    /// pelure, sa table lumineuse), ce que la lecture va montrer, les plans voisins, les dessins des prochaines
    /// annulations.
    pub(crate) fn dessins_voulus(&self) -> Vec<CelId> {
        let mut out: Vec<CelId> = Vec::new();
        let mut vus: HashSet<CelId> = HashSet::new();
        let ajouter = |out: &mut Vec<CelId>, vus: &mut HashSet<CelId>, id: CelId| {
            if vus.insert(id) {
                out.push(id);
            }
        };
        let n = self.doc.panels.len();
        if n == 0 {
            return out;
        }
        let cur = self.cur_panel.min(n - 1);
        for item in self.build_plan(self.cur_frame, true, false).items {
            ajouter(&mut out, &mut vus, item.cel);
            for s in item.ctg.iter().flat_map(|c| c.sources.iter()) {
                ajouter(&mut out, &mut vus, *s);
            }
        }
        for id in crate::empreinte::dessins_du_plan(&self.doc.panels[cur]) {
            ajouter(&mut out, &mut vus, id);
        }
        // Les voisins (changer de plan, une transition).
        let mut obligatoires: Vec<CelId> = Vec::new();
        for p in [cur + 1, cur.wrapping_sub(1)] {
            if let Some(panel) = self.doc.panels.get(p) {
                obligatoires.extend(crate::empreinte::dessins_du_plan(panel));
            }
        }
        // Les prochaines annulations et le prochain rétablissement : Ctrl+Z reste instantané.
        obligatoires.extend(self.history.dessins_proches(4));
        // La lecture : ce que les six secondes qui viennent montrent, tant que ça tient dans ce que le budget laisse après
        // le reste (au-delà, les plans suivants viennent à mesure : la lecture ne demande pas plus que la mémoire).
        if self.playing && self.play_montage {
            let fps = self.doc.fps.max(1) as usize;
            let (start, total) = (self.doc.panel_start(cur) + self.cur_frame, self.doc.total_frames());
            let mut plans: Vec<usize> = Vec::new();
            for k in (0..fps * 6).step_by(4) {
                let g = start + k;
                if g >= total {
                    break;
                }
                let (p, _) = self.doc.locate(g);
                if !plans.contains(&p) {
                    plans.push(p);
                }
            }
            let budget = crate::residence::budget();
            let mut reste = if budget == 0 {
                u64::MAX
            } else {
                let deja: HashSet<CelId> = out.iter().chain(obligatoires.iter()).copied().collect();
                budget.saturating_sub(deja.iter().map(|id| self.octets_estimes(id)).sum::<u64>())
            };
            for p in plans {
                let ids = crate::empreinte::dessins_du_plan(&self.doc.panels[p]);
                let nouveaux: u64 = ids.iter().filter(|id| !vus.contains(*id)).map(|id| self.octets_estimes(id)).sum();
                if nouveaux > reste {
                    break;
                }
                reste -= nouveaux;
                for id in ids {
                    ajouter(&mut out, &mut vus, id);
                }
            }
        }
        for id in obligatoires {
            ajouter(&mut out, &mut vus, id);
        }
        // Une opération sur tout le projet attend (changer sa taille) : tout.
        if self.residence.tout_garder {
            for id in self.doc.cels.keys() {
                ajouter(&mut out, &mut vus, *id);
            }
        }
        // Les dessins absents dont le fichier est dans un autre dossier que celui du projet (collés d'un autre projet, un
        // projet fait de plans choisis) : ils reviennent tous, derrière le reste (ce dossier-là n'est pas le nôtre).
        let dossier = self.path.as_ref().and_then(|p| p.parent());
        for (id, c) in self.doc.cels.tous() {
            if let Some(Source::Fichier { dossier: d, .. }) = c.absent.as_ref().map(|a| &a.source) {
                if Some(d.as_path()) != dossier {
                    ajouter(&mut out, &mut vus, *id);
                }
            }
        }
        out
    }

    /// Ce qu'un dessin tient (ou tiendra, absent) en mémoire, en octets : estimation pour les préchargements. Un dessin
    /// absent dont on ne sait pas le poids (projet ouvert sans lire ses pixels) compte pour huit tuiles.
    fn octets_estimes(&self, id: &CelId) -> u64 {
        let Some(c) = self.doc.cels.get_ou_absent(id) else { return 0 };
        let n = match &c.absent {
            Some(a) if a.tuiles > 0 => a.tuiles,
            Some(_) => 8,
            None => crate::residence::tuiles_de(&c.raster),
        };
        n as u64 * OCTETS_TUILE
    }

    /// L'image à l'écran montre un dessin qui n'est pas encore revenu (ni ses pixels, ni sa texture sur la carte
    /// graphique) : la vignette du plan se pose sur son cadre en attendant (elle a son propre cache), et l'écran se
    /// repeint à l'arrivée des pixels. Rien n'attend.
    pub(crate) fn poser_vignette_d_attente(&mut self, ctx: &egui::Context, painter: &egui::Painter, rect: egui::Rect, size: egui::Vec2) {
        if self.cur_panel >= self.doc.panels.len() {
            return;
        }
        let deadline = std::time::Instant::now() + crate::thumbs::THUMB_TIME;
        let Some(tex) = self.thumbs.texture(ctx, &self.doc, self.cur_panel, crate::thumbs::STRIP_WIDTH, deadline) else {
            #[cfg(test)]
            IMAGES_VIDES.with(|n| n.set(n.get() + 1));
            return;
        };
        #[cfg(test)]
        IMAGES_EN_ATTENTE.with(|n| n.set(n.get() + 1));
        let cam = self.doc.panel_camera(self.cur_panel, 0);
        let coins = cam.corners(self.doc.frame);
        let mut mesh = egui::Mesh::with_texture(tex);
        for (p, uv) in coins.into_iter().zip([egui::pos2(0.0, 0.0), egui::pos2(1.0, 0.0), egui::pos2(1.0, 1.0), egui::pos2(0.0, 1.0)]) {
            mesh.vertices.push(egui::epaint::Vertex { pos: self.view.to_screen(rect, size, p), uv, color: egui::Color32::WHITE });
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(egui::Shape::mesh(mesh));
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    }

    /// Une image d'écran pour les dessins en mémoire (voir le haut du module).
    pub(crate) fn residence_tick(&mut self, ctx: &egui::Context) {
        let _t = crate::perf::section("dessins en mémoire");
        self.residence.tour += 1;
        let tour = self.residence.tour;
        TOUR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let global = TOUR.load(std::sync::atomic::Ordering::Relaxed);
        let mut lacher: Vec<Raster> = Vec::new();
        // 1. Ce qui est arrivé.
        let arrivees: Vec<Arrivee> = std::mem::take(&mut *self.residence.boite.lock().unwrap_or_else(|e| e.into_inner()));
        let mut revenus = false;
        for a in arrivees {
            match a {
                Arrivee::Lu { absent, pixels: Ok(r) } => {
                    let ids = self.doc.cels.resoudre(&absent, &r);
                    if ids.is_empty() {
                        lacher.push(r);
                        continue;
                    }
                    revenus = true;
                    for id in ids {
                        self.residence.demandes.remove(&id);
                        self.residence.echecs.remove(&id);
                        self.residence.stats.revenus += 1;
                    }
                }
                Arrivee::Lu { absent, pixels: Err(e) } => {
                    for (id, c) in self.doc.cels.tous() {
                        if c.absent.as_ref().is_some_and(|x| Arc::ptr_eq(x, &absent)) {
                            self.residence.echecs.insert(*id, (tour, e.clone()));
                        }
                    }
                    self.status = format!("Un dessin n'a pas pu être relu du dossier du projet : {e}");
                }
                Arrivee::Verifie { id, version, identite } => {
                    self.residence.verifs.remove(&id);
                    if let Some(i) = identite {
                        self.residence.verifies.insert(id, (version, i, tour));
                    }
                }
            }
        }
        if revenus {
            self.renderer.invalidate_all();
            ctx.request_repaint();
        }
        if !self.doc.cels.is_empty() {
            // 2. Ce qui est voulu : marqué vu, et demandé s'il est absent.
            let mut voulus = self.dessins_voulus();
            for id in std::mem::take(&mut self.residence.en_plus) {
                if !voulus.contains(&id) {
                    voulus.push(id);
                }
            }
            self.demander_les_absents(ctx, &voulus, global);
            // 3. Le budget.
            if revenus || tour >= self.residence.controle + CONTROLE {
                self.residence.controle = tour;
                self.tenir_le_budget(&voulus, &mut lacher);
            }
        }
        // 4. Une annulation, une commande qui attendaient leurs dessins.
        self.reprendre_l_historique();
        self.reprendre_l_action();
        // 5. Les vieilles étapes d'annulation se compriment en fond (les récentes restent telles quelles).
        self.history.comprimer_les_anciennes(crate::histocomprime::GARDER, Some(ctx));
        if !lacher.is_empty() {
            crate::lacher::en_fond(lacher);
        }
    }

    /// Marque vus les dessins `voulus`, et demande le retour de ceux qui sont absents : au chargeur (bitmap, lu de son
    /// fichier), au rejeu (image d'un dessin vectoriel). Une demande se renouvelle de temps en temps (elle ne se périme
    /// pas tant qu'on la veut) ; après un échec, on attend avant de réessayer.
    fn demander_les_absents(&mut self, ctx: &egui::Context, voulus: &[CelId], global: u64) {
        let tour = self.residence.tour;
        let (w, h) = (self.doc.width, self.doc.height);
        let courant: HashSet<CelId> = self.doc.panels.get(self.cur_panel).map(|p| crate::empreinte::dessins_du_plan(p).into_iter().collect()).unwrap_or_default();
        let mut attente = false;
        let mut rejeux: Vec<(CelId, bool)> = Vec::new();
        for (i, id) in voulus.iter().enumerate() {
            self.residence.vu.insert(*id, tour);
            let Some(a) = self.doc.cels.get_ou_absent(id).and_then(|c| c.absent.clone()) else { continue };
            attente = true;
            if self.residence.demandes.get(id).is_some_and(|&t| t + 60 > tour) || self.residence.echecs.get(id).is_some_and(|(t, _)| *t + 600 > tour) {
                continue;
            }
            self.residence.demandes.insert(*id, tour);
            match a.source {
                Source::Fichier { .. } => demander(Tache {
                    tour: global,
                    rang: i.min(u32::MAX as usize) as u32,
                    travail: Travail::Lire { absent: a, w, h },
                    boite: self.residence.boite.clone(),
                    reveil: Some(ctx.clone()),
                }),
                Source::Rendu => {
                    if !self.rejeu.is_pending(*id) {
                        // (Le plan courant passe avant tout chargement ; le reste derrière les gestes.)
                        rejeux.push((*id, courant.contains(id)));
                    }
                }
            }
        }
        for (id, prio) in rejeux {
            self.rejeu_with(id, prio);
        }
        if attente {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }

    /// Un dessin bitmap peut-il être laissé (ses pixels relus de son fichier) ? Écrit à sa version dans le dossier du
    /// projet (ni modifié depuis, ni en cours d'écriture), projet classique (un projet partagé garde tout), sans strate
    /// Fond, aucun geste dessus.
    fn bitmap_laissable(&self, id: CelId, c: &Cel) -> bool {
        c.vector.is_none()
            && c.fond.is_none()
            && self.partage.is_none()
            && self.saving.is_none()
            && self.path.is_some()
            && self.written.get(&id) == Some(&c.version)
            && !self.cel_busy(id)
            && self.flottante.as_ref().is_none_or(|f| f.src != id)
    }

    /// Au-delà du budget : les dessins présents les moins récemment voulus sont laissés, jusqu'à 85 % du budget. Une
    /// image vectorielle part tout de suite (ses traits restent) ; un dessin bitmap une fois son fichier vérifié (au
    /// contrôle suivant). Un projet partagé garde tout.
    fn tenir_le_budget(&mut self, voulus: &[CelId], lacher: &mut Vec<Raster>) {
        let tour = self.residence.tour;
        let budget = crate::residence::budget();
        let voulu: HashSet<CelId> = voulus.iter().copied().collect();
        let mut total: u64 = 0;
        let (mut presents, mut absents) = (0usize, 0usize);
        let mut candidats: Vec<(u64, CelId, u64)> = Vec::new();
        for (id, c) in self.doc.cels.tous() {
            if c.absent.is_some() {
                absents += 1;
                continue;
            }
            presents += 1;
            let n = self.residence.tuiles(*id, c) as u64 * OCTETS_TUILE;
            total += n;
            if n > 0 && !voulu.contains(id) {
                candidats.push((self.residence.vu.get(id).copied().unwrap_or(0), *id, n));
            }
        }
        if self.residence.poids.len() > presents + 64 {
            let cels = &self.doc.cels;
            self.residence.poids.retain(|id, _| cels.get(id).is_some());
        }
        let (revenus, laisses) = (self.residence.stats.revenus, self.residence.stats.laisses);
        self.residence.stats = Stats { octets_presents: total, presents, absents, revenus, laisses, budget };
        if budget == 0 || total <= budget || self.partage.is_some() {
            return;
        }
        let cible = budget / 100 * 85;
        candidats.sort_unstable();
        let dossier = self.path.as_ref().and_then(|p| p.parent()).map(std::path::Path::to_path_buf);
        let mut n_laisses = 0;
        for (_, id, n) in candidats {
            if total <= cible || n_laisses >= LAISSES_PAR_CONTROLE {
                break;
            }
            let Some(c) = self.doc.cels.get(&id) else { continue };
            let absent = if c.vector.is_some() {
                if self.cel_busy(id) || self.rejeu.is_pending(id) || self.flottante.as_ref().is_some_and(|f| f.src == id) {
                    continue;
                }
                Absent::rendu(crate::residence::tuiles_de(&c.raster))
            } else {
                let Some(dossier) = dossier.clone() else { continue };
                if !self.bitmap_laissable(id, c) {
                    continue;
                }
                let version = c.version;
                let hors = {
                    let mut memo = self.doc.memo_enregistrement.lock().unwrap_or_else(|e| e.into_inner());
                    crate::project::memo_hors(&mut memo, id, c)
                };
                // Le fichier, vérifié il y a peu à cette version : on s'y fie. Sinon il est vérifié d'abord, sur le fil du
                // chargeur (le dessin part au contrôle suivant s'il est toujours à laisser).
                match self.residence.verifies.get(&id) {
                    Some(&(v, i, t)) if v == version && t + 600 > tour => {
                        let a = Absent::fichier(dossier, id, hors, Some(i), crate::residence::tuiles_de(&c.raster));
                        #[cfg(test)]
                        crate::residence::noter_empreinte(&a, c);
                        a
                    }
                    _ => {
                        if self.residence.verifs.insert(id) {
                            let mut fichiers = vec![dossier.join("dessins").join(format!("{id}.png"))];
                            if hors {
                                fichiers.push(dossier.join("dessins").join(format!("{id}_hors.png")));
                                fichiers.push(dossier.join("dessins").join(format!("{id}_hors.json")));
                            }
                            let tour_global = TOUR.load(std::sync::atomic::Ordering::Relaxed);
                            demander(Tache { tour: tour_global, rang: 0, travail: Travail::Verifier { id, version, fichiers }, boite: self.residence.boite.clone(), reveil: Some(self.ctx.clone()) });
                        }
                        continue;
                    }
                }
            };
            let Some(c) = self.doc.cels.get_ou_absent_mut(&id) else { continue };
            let vide = Raster::new(c.raster.width, c.raster.height);
            lacher.push(std::mem::replace(&mut c.raster, vide));
            c.absent = Some(absent);
            total = total.saturating_sub(n);
            n_laisses += 1;
            self.residence.stats.laisses += 1;
            self.residence.poids.remove(&id);
        }
        self.residence.stats.octets_presents = total;
        self.residence.stats.presents -= n_laisses;
        self.residence.stats.absents += n_laisses;
    }

    /// Les dessins du plan courant qui ne sont pas en mémoire (juste après un changement de plan, le temps qu'ils
    /// reviennent).
    pub(crate) fn absents_du_plan_courant(&self) -> Vec<CelId> {
        self.doc.panels.get(self.cur_panel).map(|p| crate::empreinte::dessins_du_plan(p).into_iter().filter(|id| self.doc.cels.est_absent(id)).collect()).unwrap_or_default()
    }

    /// La commande `action` lit ou modifie les dessins du plan courant, et certains ne sont pas encore revenus en mémoire :
    /// elle attend leur retour (puis part d'elle-même, si l'on est toujours sur ce plan). Vrai si elle attend.
    pub(crate) fn action_attend_le_plan(&mut self, action: &str) -> bool {
        const PREFIXES: [&str; 6] = ["calque.", "images.", "dessin.", "instance.", "edition.", "fond."];
        if !PREFIXES.iter().any(|p| action.starts_with(p)) || matches!(action, "edition.annuler" | "edition.retablir") {
            return false;
        }
        let absents = self.absents_du_plan_courant();
        if absents.is_empty() {
            return false;
        }
        let plan = self.doc.panels[self.cur_panel].id;
        self.residence.action_en_attente = Some((plan, action.to_string()));
        self.residence.en_plus.extend(absents.iter().copied());
        self.status = format!("Le plan revient en mémoire ({} dessin(s)) : la commande part dès qu'il est là.", absents.len());
        true
    }

    /// Une commande qui attendait les dessins du plan courant : lancée dès qu'ils sont là (oubliée si l'on a changé de plan).
    fn reprendre_l_action(&mut self) {
        let Some((plan, action)) = self.residence.action_en_attente.clone() else { return };
        if self.doc.panels.get(self.cur_panel).map(|p| p.id) != Some(plan) {
            self.residence.action_en_attente = None;
            return;
        }
        let absents = self.absents_du_plan_courant();
        if !absents.is_empty() {
            self.residence.en_plus.extend(absents);
            return;
        }
        self.residence.action_en_attente = None;
        self.run_action(&action);
    }

    /// Un appui sur le dessin alors qu'un dessin de l'image n'est pas encore revenu en mémoire : le geste ne commence pas
    /// (il peindrait dans une image vide) ; la barre d'état le dit. Vrai si le dessin est prêt.
    pub(crate) fn dessin_pret_pour_un_geste(&mut self) -> bool {
        let absents: Vec<CelId> = self.build_plan(self.cur_frame, false, false).items.iter().map(|i| i.cel).filter(|id| self.doc.cels.est_absent(id)).collect();
        if absents.is_empty() {
            return true;
        }
        self.residence.en_plus.extend(absents);
        self.status = "Le dessin revient en mémoire : un instant.".into();
        false
    }

    /// Un enregistrement a réussi dans `dossier` : les dessins absents dont le fichier était ailleurs (« Enregistrer sous »,
    /// dessins collés d'un autre projet) y ont maintenant le leur, écrit à leur version ; ils s'y relisent désormais (l'ancien
    /// dossier peut disparaître sans rien emporter). Ceux du projet partagé ne changent pas (il n'en a pas).
    pub(crate) fn absents_apres_enregistrement(&mut self, dossier: &std::path::Path) {
        let a_suivre: Vec<(CelId, bool, usize)> = self
            .doc
            .cels
            .tous()
            .filter_map(|(id, c)| match c.absent.as_ref().map(|a| (&a.source, a.tuiles)) {
                Some((Source::Fichier { dossier: d, hors, .. }, tuiles)) if d != dossier && self.written.get(id) == Some(&c.version) => Some((*id, *hors, tuiles)),
                _ => None,
            })
            .collect();
        for (id, hors, tuiles) in a_suivre {
            if let Some(c) = self.doc.cels.get_ou_absent_mut(&id) {
                c.absent = Some(Absent::fichier(dossier.to_path_buf(), id, hors, None, tuiles));
            }
        }
    }

    /// La prochaine annulation (`redo` : le prochain rétablissement) touche un dessin absent : elle attend son retour,
    /// demandé aussitôt (une seconde demande pendant l'attente compte pour une annulation de plus). Vrai si elle attend.
    pub(crate) fn historique_attend(&mut self, redo: bool) -> bool {
        let absents: Vec<CelId> = self.history.dessins_de_la_prochaine(redo).into_iter().filter(|id| self.doc.cels.est_absent(id)).collect();
        if absents.is_empty() {
            return false;
        }
        let serial = self.history.serial();
        match &mut self.residence.attente_historique {
            Some((r, n, s)) if *r == redo && *s == serial => *n += 1,
            a => *a = Some((redo, 1, serial)),
        }
        // (Tout de suite : ils sont dans ce qui est voulu, `dessins_proches`.)
        let ctx = self.ctx.clone();
        let voulus = self.dessins_voulus();
        let global = TOUR.load(std::sync::atomic::Ordering::Relaxed);
        for id in &absents {
            self.residence.demandes.remove(id);
        }
        self.demander_les_absents(&ctx, &voulus, global);
        self.status = format!("{} : {} dessin(s) reviennent du dossier du projet…", if redo { "Rétablir" } else { "Annuler" }, absents.len());
        true
    }

    /// Une annulation qui attendait ses dessins : faite dès qu'ils sont revenus (oubliée si l'historique a changé depuis).
    fn reprendre_l_historique(&mut self) {
        let Some((redo, n, serial)) = self.residence.attente_historique else { return };
        if self.history.serial() != serial {
            self.residence.attente_historique = None;
            return;
        }
        if !self.history.prete(&self.doc, redo) {
            return;
        }
        self.residence.attente_historique = None;
        for k in 0..n {
            let avant = self.history.serial();
            if redo {
                self.redo();
            } else {
                self.undo();
            }
            // (Celle-ci attend à son tour ses dessins : elle reprend là, avec celles qui restent.)
            if let Some(a) = self.residence.attente_historique.as_mut() {
                a.1 = n - k;
                break;
            }
            if self.history.serial() == avant {
                break;
            }
        }
    }
}
