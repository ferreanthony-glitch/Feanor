//! Le panneau « Scénario » (Anthony, 2026-10-30 : « c'est là qu'apparaît un scénario importé dans SHOT, il est
//! lisible à cet endroit »). Lecture seule.
//!
//! - **Mise en page de scénario** : intitulé de scène en gras et majuscules ; action en pleine largeur ; nom du
//!   personnage centré ; réplique dans une colonne étroite au milieu ; parenthèses de jeu en italique ; transition à
//!   droite ; notes discrètes. Une police à chasse fixe (comme un scénario) en trois tailles (petit, moyen, grand ;
//!   réglage du poste, retenu).
//! - **Zéro latence** : la hauteur de chaque élément est estimée (chasse fixe : les lignes se comptent) et gardée tant
//!   que la largeur, la taille et le scénario ne changent pas ; seuls les éléments visibles sont mis en page et dessinés.
//!   Un scénario de 120 pages défile aussi vite qu'un court.
//! - **Lien avec le storyboard** (`scenariogarde::Lien`) : le passage qui correspond au plan courant est surligné et le
//!   panneau le garde en vue quand on change de plan ; un clic sur une ligne amène au plan qui en vient. Les liens
//!   gardent les numéros stables des plans (pas leur rang) : un plan supprimé, réuni ou déplacé ne fausse rien ; sans
//!   plan, le clic va au plus proche, et un plan sans lien (créé à la main) montre le passage du plan lié qui le précède.
//!   Il n'y a de lien que pour les plans créés par le bouton « Créer les plans depuis le scénario… » ; sans eux, le
//!   panneau se lit simplement, sans surlignage.
//! - **Vide** : une phrase et « Importer un scénario… ».

use std::collections::HashSet;
use std::sync::Arc;

use eframe::egui::{self, pos2, text::LayoutJob, vec2, Align, Color32, FontId, Galley, Painter, Pos2, Rect, RichText, Sense, Ui};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::scenario::{Element, Kind};
use crate::scenariogarde::{Gardee, Taille};
use crate::scenariorecherche::{Occurrence, Recherche};
use crate::theme;

/// Marge à gauche et à droite du texte, en points.
const MARGE: f32 = 10.0;
/// Les textes discrets (notes, parenthèses) sont un peu plus petits.
const PETIT: f32 = 0.86;

/// Ce que le panneau retient d'une image à l'autre.
#[derive(Default)]
pub(crate) struct Etat {
    cache: Option<Cache>,
    /// (scénario, plan) montrés à l'image d'avant : un changement remet le passage en vue.
    dernier: Option<(u64, u64)>,
    /// Position à donner au défilement à cette image.
    defiler_vers: Option<f32>,
    /// Haut et hauteur de ce qui se voyait à l'image d'avant.
    vue_haut: f32,
    vue_hauteur: f32,
    /// La recherche (Ctrl+F dans le panneau), `scenariorecherche.rs`.
    pub(crate) recherche: Recherche,
}

/// La hauteur estimée de chaque élément, pour le scénario, la largeur et la taille donnés.
struct Cache {
    scenario: u64,
    largeur: i32,
    taille: Taille,
    /// Début de chaque élément ; un de plus que d'éléments (le dernier est la hauteur totale).
    debuts: Vec<f32>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Col {
    Pleine,
    Replique,
    Parenthese,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Al {
    Gauche,
    Centre,
    Droite,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Style {
    Normal,
    Gras,
    Italique,
    /// Petit, italique, discret.
    Doux,
}

/// Une partie d'un élément : un nom de personnage, une parenthèse, le texte d'une réplique…
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Partie {
    pub texte: String,
    pub col: Col,
    pub al: Al,
    pub style: Style,
    /// Espace au-dessus, en hauteurs de ligne.
    pub avant: f32,
}

fn partie(texte: String, col: Col, al: Al, style: Style, avant: f32) -> Partie {
    Partie { texte, col, al, style, avant }
}

/// La mise en page d'un élément du scénario, partie par partie.
pub(crate) fn parties(e: &Element) -> Vec<Partie> {
    let mut v = Vec::new();
    match &e.kind {
        Kind::Heading(t) => v.push(partie(t.to_uppercase(), Col::Pleine, Al::Gauche, Style::Gras, 1.7)),
        Kind::Action(t) => v.push(partie(t.clone(), Col::Pleine, Al::Gauche, Style::Normal, 0.9)),
        Kind::Dialogue(s) => {
            let mut nom = s.name.to_uppercase();
            for t in &s.tags {
                nom.push_str(&format!(" ({t})"));
            }
            let nomme = !nom.trim().is_empty();
            if nomme {
                v.push(partie(nom, Col::Pleine, Al::Centre, Style::Normal, 0.9));
            }
            for a in &s.asides {
                v.push(partie(a.clone(), Col::Parenthese, Al::Centre, Style::Italique, 0.0));
            }
            if !s.text.is_empty() {
                v.push(partie(s.text.clone(), Col::Replique, Al::Gauche, Style::Normal, if nomme || !s.asides.is_empty() { 0.0 } else { 0.9 }));
            }
        }
        Kind::Transition(t) => v.push(partie(t.to_uppercase(), Col::Pleine, Al::Droite, Style::Normal, 0.9)),
        Kind::Section(t) => v.push(partie(t.to_uppercase(), Col::Pleine, Al::Gauche, Style::Gras, 1.9)),
        Kind::Synopsis(t) => v.push(partie(t.clone(), Col::Pleine, Al::Gauche, Style::Doux, 0.6)),
        Kind::Note(t) => v.push(partie(format!("[{t}]"), Col::Pleine, Al::Gauche, Style::Doux, 0.6)),
    }
    for n in &e.notes {
        v.push(partie(format!("[{n}]"), Col::Pleine, Al::Gauche, Style::Doux, 0.25));
    }
    v
}

/// Les mesures d'une police à chasse fixe à une taille et pour une largeur de texte.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mesures {
    /// Largeur d'un caractère et hauteur d'une ligne, au corps normal.
    pub cw: f32,
    pub lh: f32,
    pub corps: f32,
    /// Largeur du texte (hors marges).
    pub w: f32,
}

impl Mesures {
    fn de(ui: &Ui, taille: Taille, largeur: f32) -> Mesures {
        let corps = taille.points();
        let font = FontId::monospace(corps);
        let (cw, lh) = ui.fonts(|f| (f.glyph_width(&font, 'M'), f.row_height(&font)));
        Mesures { cw: cw.max(1.0), lh: lh.max(1.0), corps, w: (largeur - 2.0 * MARGE).max(40.0) }
    }

    pub(crate) fn colonne(&self, c: Col) -> f32 {
        let w = match c {
            Col::Pleine => self.w,
            Col::Replique => (self.w * 0.72).min(self.cw * 36.0),
            Col::Parenthese => (self.w * 0.52).min(self.cw * 24.0),
        };
        w.max(self.cw * 8.0).min(self.w)
    }

    fn echelle(style: Style) -> f32 {
        if style == Style::Doux {
            PETIT
        } else {
            1.0
        }
    }
}

/// Nombre de lignes d'un texte dans `cpl` caractères par ligne (retour à la ligne aux mots, comme la mise en page).
pub(crate) fn lignes(texte: &str, cpl: usize) -> usize {
    let cpl = cpl.max(1);
    texte
        .split('\n')
        .map(|l| {
            let mut n = 1;
            let mut cur = 0usize;
            for mot in l.split_whitespace() {
                let mut len = mot.chars().count();
                if cur == 0 {
                    cur = len;
                } else if cur + 1 + len <= cpl {
                    cur += 1 + len;
                    continue;
                } else {
                    n += 1;
                    cur = len;
                }
                // (Un mot plus long que la ligne se coupe.)
                while len > cpl {
                    n += 1;
                    len -= cpl;
                    cur = len;
                }
            }
            n
        })
        .sum()
}

/// Hauteur estimée d'une partie, espace au-dessus compris.
fn hauteur_partie(m: &Mesures, p: &Partie) -> f32 {
    let e = Mesures::echelle(p.style);
    let cpl = (m.colonne(p.col) / (m.cw * e)).floor().max(1.0) as usize;
    p.avant * m.lh + lignes(&p.texte, cpl) as f32 * m.lh * e
}

/// Hauteur estimée d'un élément.
pub(crate) fn hauteur_element(m: &Mesures, e: &Element) -> f32 {
    parties(e).iter().map(|p| hauteur_partie(m, p)).sum()
}

/// Les débuts des éléments (sommes cumulées des hauteurs) ; le dernier nombre est la hauteur totale.
pub(crate) fn debuts(m: &Mesures, g: &Gardee) -> Vec<f32> {
    let mut out = Vec::with_capacity(g.script.elements.len() + 1);
    let mut y = 0.0;
    for e in &g.script.elements {
        out.push(y);
        y += hauteur_element(m, e);
    }
    out.push(y + m.lh * 2.0);
    out
}

/// L'indice de l'élément qui occupe la position `y` (les débuts sont croissants).
pub(crate) fn element_a(debuts: &[f32], y: f32) -> usize {
    let n = debuts.len().saturating_sub(1);
    if n == 0 {
        return 0;
    }
    debuts.partition_point(|d| *d <= y).saturating_sub(1).min(n - 1)
}

/// Une partie mise en page, à poser à `dy` sous le haut de l'élément.
struct Posee {
    galley: Arc<Galley>,
    dy: f32,
    x: f32,
    couleur: Color32,
    gras: bool,
    /// Rang de la partie dans l'élément (voir `parties`) et largeur d'une lettre à son corps : pour surligner.
    idx: usize,
    cw: f32,
}

fn mettre_en_page(painter: &Painter, m: &Mesures, x0: f32, e: &Element) -> (Vec<Posee>, f32) {
    let mut out = Vec::new();
    let mut y = 0.0;
    for (idx, p) in parties(e).into_iter().enumerate() {
        let echelle = Mesures::echelle(p.style);
        let colw = m.colonne(p.col);
        let couleur = if p.style == Style::Doux { theme::texte_doux() } else { theme::texte() };
        let mut job = LayoutJob::simple(p.texte, FontId::monospace(m.corps * echelle), couleur, colw);
        job.halign = match p.al {
            Al::Gauche => Align::LEFT,
            Al::Centre => Align::Center,
            Al::Droite => Align::RIGHT,
        };
        if matches!(p.style, Style::Italique | Style::Doux) {
            for s in &mut job.sections {
                s.format.italics = true;
            }
        }
        let galley = painter.layout_job(job);
        y += p.avant * m.lh;
        let h = galley.size().y;
        // (Avec un alignement au centre ou à droite, la position du texte est son point d'ancrage : le milieu ou le bord droit de la colonne.)
        let gauche = x0 + (m.w - colw) / 2.0;
        let x = match p.al {
            Al::Gauche => gauche,
            Al::Centre => gauche + colw / 2.0,
            Al::Droite => gauche + colw,
        };
        out.push(Posee { galley, dy: y, x, couleur, gras: p.style == Style::Gras, idx, cw: m.cw * echelle });
        y += h;
    }
    (out, y)
}

/// L'identifiant du champ de recherche.
fn champ_id() -> egui::Id {
    egui::Id::new("scenario_recherche_champ")
}

/// Surligne une occurrence (lettre par lettre : elle peut passer à la ligne). La courante est plus marquée.
fn surligner(painter: &Painter, p: &Posee, pos: Pos2, o: &Occurrence, courante: bool) {
    let (fond, trait_) = if courante {
        (Color32::from_rgba_unmultiplied(theme::JADE[0], theme::JADE[1], theme::JADE[2], 130), Some(egui::Stroke::new(1.0_f32, Color32::from_rgb(theme::JADE[0], theme::JADE[1], theme::JADE[2]))))
    } else {
        (Color32::from_rgba_unmultiplied(theme::JADE[0], theme::JADE[1], theme::JADE[2], 55), None)
    };
    for k in o.debut..o.debut + o.len {
        let r = p.galley.pos_from_cursor(&p.galley.from_ccursor(egui::text::CCursor { index: k, prefer_next_row: true }));
        let rect = Rect::from_min_size(pos + r.min.to_vec2(), vec2(p.cw, r.height()));
        painter.rect_filled(rect, 1.0, fond);
        if let Some(t) = trait_ {
            painter.rect_stroke(rect, 1.0, t, egui::StrokeKind::Inside);
        }
    }
}

impl Etat {
    /// Le pointeur est sur le panneau (à cette image ou la précédente) : Ctrl+F y est la recherche, pas « Aller à un plan ».
    pub(crate) fn recherche_prise(&self, ctx: &egui::Context) -> bool {
        self.recherche.survol_image.is_some_and(|n| ctx.cumulative_pass_nr().saturating_sub(n) <= 1)
    }
}

impl App {
    /// Le contenu du panneau Scénario.
    pub(crate) fn scenario_panneau(&mut self, ui: &mut Ui) {
        let Some(g) = self.doc.scenario.clone() else {
            self.scenario_vide(ui);
            return;
        };
        self.scenario_recherche_touches(ui);
        self.scenario_entete(ui, &g);
        ui.separator();
        self.scenario_ui.recherche.mettre_a_jour(&g);
        self.scenario_texte(ui, &g);
    }

    /// Ctrl+F (pointeur sur le panneau) ouvre la recherche ; Échap la ferme.
    fn scenario_recherche_touches(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let survol = ui.rect_contains_pointer(ui.max_rect());
        if survol {
            self.scenario_ui.recherche.survol_image = Some(ctx.cumulative_pass_nr());
        }
        let champ = self.scenario_ui.recherche.ouverte && ctx.memory(|m| m.has_focus(champ_id()));
        let (ctrl_f, echap) = ctx.input(|i| {
            let touche = |k: egui::Key, ok: &dyn Fn(egui::Modifiers) -> bool| i.events.iter().any(|e| matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. } if *key == k && ok(*modifiers)));
            (touche(egui::Key::F, &|m| m.command && !m.shift && !m.alt), touche(egui::Key::Escape, &|m| !m.any()))
        });
        if ctrl_f && survol {
            self.scenario_ui.recherche.ouvrir();
        } else if echap && self.scenario_ui.recherche.ouverte && (champ || survol) {
            self.scenario_ui.recherche.fermer();
            ctx.memory_mut(|m| m.surrender_focus(champ_id()));
        }
        // Entrée : suivant ; Maj+Entrée : précédent (le champ garde la main).
        if champ && self.scenario_ui.recherche.ouverte {
            let (prec, suiv) = ui.input_mut(|i| (i.consume_key(egui::Modifiers::SHIFT, egui::Key::Enter), i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)));
            if prec {
                self.scenario_ui.recherche.precedent();
            } else if suiv {
                self.scenario_ui.recherche.suivant();
            }
        }
    }

    /// La barre de recherche : champ, compteur, précédent, suivant, fermer.
    fn scenario_barre_recherche(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let r = &mut self.scenario_ui.recherche;
        let mut ferme = false;
        let (mut prec, mut suiv) = (false, false);
        ui.horizontal(|ui| {
            ui.label(RichText::new(ph::MAGNIFYING_GLASS).weak());
            let reserve = 150.0;
            let sortie = egui::TextEdit::singleline(&mut r.terme).id(champ_id()).hint_text("Chercher dans le scénario").desired_width((ui.available_width() - reserve).max(60.0)).show(ui);
            if r.focus {
                r.focus = false;
                ui.memory_mut(|m| m.request_focus(champ_id()));
                let mut etat = sortie.state.clone();
                etat.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(r.terme.chars().count()))));
                etat.store(&ctx, champ_id());
            }
            let compteur = r.compteur();
            ui.label(RichText::new(compteur).small().weak());
            prec = crate::bulles::icone(ui, ph::CARET_UP, "Occurrence précédente", "Remonte à l'occurrence d'avant ; revient à la dernière après la première.", "Maj+Entrée").clicked();
            suiv = crate::bulles::icone(ui, ph::CARET_DOWN, "Occurrence suivante", "Descend à l'occurrence d'après ; revient à la première après la dernière.", "Entrée").clicked();
            ferme = crate::bulles::icone(ui, ph::X, "Fermer la recherche", "Ferme la barre et efface le surlignage.", "Échap").clicked();
        });
        if prec {
            r.precedent();
        }
        if suiv {
            r.suivant();
        }
        if ferme {
            r.fermer();
        }
    }

    /// Sans scénario : une phrase et le bouton qui lance l'import.
    fn scenario_vide(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.label("Aucun scénario dans ce projet.");
        ui.label(RichText::new("Importe un scénario (Fountain, .fdx, .docx ou texte) : il se lit ici, mis en page comme un scénario.").small().weak());
        ui.add_space(6.0);
        let resp = ui.button(format!("{}  Importer un scénario…", ph::FILE_ARROW_DOWN));
        if self.aide(resp, "Importer un scénario", "Choisir un fichier Fountain, .fdx, .docx ou texte : FEANOR le garde dans le projet et l'affiche ici. Aucun plan n'est créé.", "fichier.importer_scenario").clicked() {
            self.run_action("fichier.importer_scenario");
        }
    }

    /// Le nom du scénario, les trois tailles, et « Créer les plans depuis le scénario… ».
    fn scenario_entete(&mut self, ui: &mut Ui, g: &Gardee) {
        let mut bascule = false;
        ui.horizontal(|ui| {
            let titre = if g.script.title.trim().is_empty() { g.nom.clone() } else { g.script.title.trim().to_string() };
            ui.add(egui::Label::new(RichText::new(titre).strong()).truncate());
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if crate::bulles::icone(ui, ph::MAGNIFYING_GLASS, "Chercher dans le scénario", "Ouvre la recherche : sans tenir compte des majuscules ni des accents, toutes les occurrences surlignées.", "Ctrl+F").clicked() {
                    bascule = true;
                }
                for (t, taille_lettre) in [(Taille::Grand, 17.0), (Taille::Moyen, 13.0), (Taille::Petit, 10.0)] {
                    let sel = self.settings.scenario_taille == t;
                    let resp = ui.selectable_label(sel, RichText::new("A").size(taille_lettre));
                    let resp = crate::bulles::bulle(resp, &format!("Texte {}", t.nom().to_lowercase()), "La taille du texte du scénario, dans ce panneau. Retenue pour la prochaine fois.", "");
                    if resp.clicked() && !sel {
                        self.settings.scenario_taille = t;
                        if let Err(e) = self.settings.save() {
                            self.status = format!("Taille du scénario : pas gardée pour la prochaine fois ({e}).");
                        }
                    }
                }
            });
        });
        if bascule {
            if self.scenario_ui.recherche.ouverte {
                self.scenario_ui.recherche.fermer();
            } else {
                self.scenario_ui.recherche.ouvrir();
            }
        }
        if self.scenario_ui.recherche.ouverte {
            self.scenario_barre_recherche(ui);
        }
        let resp = ui.button(format!("{}  Créer les plans depuis le scénario…", ph::FILM_STRIP));
        if self
            .aide(
                resp,
                "Créer les plans depuis le scénario",
                "FEANOR lit le scénario et propose des séquences et des plans : tu choisis le découpage et le rythme, puis une seule étape d'annulation les crée à la suite des plans qui existent. Les plans créés sont reliés au texte.",
                "scenario.creer_plans",
            )
            .clicked()
        {
            self.run_action("scenario.creer_plans");
        }
    }

    /// Le plan à montrer : le lien du plan courant, sinon celui du plan lié qui le précède (puis qui le suit).
    pub(crate) fn scenario_lien_courant(&self, g: &Gardee) -> Option<usize> {
        if g.liens.is_empty() {
            return None;
        }
        let i = self.cur_panel.min(self.doc.panels.len().saturating_sub(1));
        let derriere = (0..=i).rev().find_map(|k| self.doc.panels.get(k).and_then(|p| g.lien_du_plan(p.id)));
        derriere.or_else(|| (i + 1..self.doc.panels.len()).find_map(|k| g.lien_du_plan(self.doc.panels[k].id)))
    }

    /// Un clic sur l'élément `i` : le plan qui en vient (ou, s'il n'existe plus, le plus proche). Faux si aucun plan n'est lié.
    pub(crate) fn scenario_aller_a_l_element(&mut self, g: &Gardee, i: usize) -> bool {
        let existants: HashSet<u64> = self.doc.panels.iter().map(|p| p.id).collect();
        let vivant = |id: u64| existants.contains(&id);
        let k = g.lien_de_l_element(i).filter(|&k| vivant(g.liens[k].plan)).or_else(|| g.lien_vivant_pres_de(i, &vivant));
        let Some(plan) = k.map(|k| g.liens[k].plan) else { return false };
        let Some(index) = self.doc.panels.iter().position(|p| p.id == plan) else { return false };
        self.end_stroke();
        self.select_panel(index);
        true
    }

    /// Le texte, défilant, dont seuls les éléments visibles sont mis en page.
    fn scenario_texte(&mut self, ui: &mut Ui, g: &Arc<Gardee>) {
        let taille = self.settings.scenario_taille;
        let avail = ui.available_width();
        let m = Mesures::de(ui, taille, avail - 14.0);
        // Les hauteurs, recalculées quand la largeur (par pas de 4 points), la taille ou le scénario changent.
        let largeur = (avail / 4.0).round() as i32;
        let bon = self.scenario_ui.cache.as_ref().is_some_and(|c| c.scenario == g.id && c.largeur == largeur && c.taille == taille && c.debuts.len() == g.script.elements.len() + 1);
        if !bon {
            self.scenario_ui.cache = Some(Cache { scenario: g.id, largeur, taille, debuts: debuts(&m, g) });
        }
        let Some(cache) = self.scenario_ui.cache.take() else { return };
        let total = cache.debuts.last().copied().unwrap_or(0.0);

        // Le passage du plan courant, et le remettre en vue quand le plan change.
        let courant = self.scenario_lien_courant(g);
        let plan_id = self.doc.panels.get(self.cur_panel).map(|p| p.id).unwrap_or(0);
        let cle = (g.id, plan_id);
        let (de, a) = courant.map_or((usize::MAX, 0), |k| (g.liens[k].de, g.liens[k].a));
        if self.scenario_ui.dernier != Some(cle) {
            self.scenario_ui.dernier = Some(cle);
            if let Some(&haut) = cache.debuts.get(de) {
                let (vh, vue) = (self.scenario_ui.vue_haut, self.scenario_ui.vue_hauteur);
                if vue <= 0.0 || haut < vh || haut > vh + vue * 0.6 {
                    self.scenario_ui.defiler_vers = Some((haut - vue.max(120.0) * 0.12).max(0.0));
                }
            }
        }
        // La recherche : mener la vue à l'occurrence courante (sa ligne exacte, par une seule mise en page).
        let rech = std::mem::take(&mut self.scenario_ui.recherche);
        let mut rech = rech;
        if rech.a_defiler {
            rech.a_defiler = false;
            if let Some(o) = rech.courante() {
                let haut = cache.debuts.get(o.element).copied().unwrap_or(0.0);
                let (posees, _) = mettre_en_page(ui.painter(), &m, 0.0, &g.script.elements[o.element]);
                let dans = posees.iter().find(|p| p.idx == o.partie).map_or(0.0, |p| p.dy + p.galley.pos_from_cursor(&p.galley.from_ccursor(egui::text::CCursor { index: o.debut, prefer_next_row: true })).min.y);
                let y = haut + dans;
                let (vh, vue) = (self.scenario_ui.vue_haut, self.scenario_ui.vue_hauteur);
                if vue <= 0.0 || y < vh + vue * 0.1 || y > vh + vue * 0.8 {
                    self.scenario_ui.defiler_vers = Some((y - vue.max(120.0) * 0.3).max(0.0));
                }
            }
        }
        let mut area = egui::ScrollArea::vertical().id_salt("panneau_scenario").auto_shrink([false, false]);
        if let Some(y) = self.scenario_ui.defiler_vers.take() {
            area = area.vertical_scroll_offset(y);
        }
        let mut clic: Option<usize> = None;
        let mut vue = (0.0, 0.0);
        let accent = theme::accent_texte();
        area.show_viewport(ui, |ui, viewport| {
            vue = (viewport.top(), viewport.height());
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), total), Sense::click());
            let painter = ui.painter_at(rect);
            let x0 = rect.left() + MARGE;
            let survol = resp.hover_pos().map(|p| element_a(&cache.debuts, p.y - rect.top()));
            let n = g.script.elements.len();
            let mut i = element_a(&cache.debuts, viewport.top());
            let mut y = cache.debuts.get(i).copied().unwrap_or(0.0);
            while i < n && y < viewport.bottom() {
                let (posees, h) = mettre_en_page(&painter, &m, x0, &g.script.elements[i]);
                let haut = rect.top() + y;
                let bande = Rect::from_min_size(pos2(rect.left(), haut), vec2(rect.width(), h.max(1.0)));
                if i >= de && i <= a {
                    painter.rect_filled(bande, 0.0, accent.gamma_multiply(0.16));
                    painter.rect_filled(Rect::from_min_size(bande.min, vec2(3.0, bande.height())), 0.0, accent);
                } else if survol == Some(i) && g.lien_de_l_element(i).is_some() {
                    painter.rect_filled(bande, 0.0, theme::eleve());
                }
                if let Some((plage, occs)) = rech.de_l_element(i) {
                    for (rang, o) in plage.zip(occs) {
                        if let Some(p) = posees.iter().find(|p| p.idx == o.partie) {
                            surligner(&painter, p, pos2(p.x, haut + p.dy), o, rang == rech.courant);
                        }
                    }
                }
                for p in posees {
                    let pos: Pos2 = pos2(p.x, haut + p.dy);
                    painter.galley(pos, p.galley.clone(), p.couleur);
                    if p.gras {
                        painter.galley(pos + vec2(0.7, 0.0), p.galley, p.couleur);
                    }
                }
                y += h;
                i += 1;
            }
            if survol.is_some_and(|s| g.lien_de_l_element(s).is_some()) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                clic = resp.interact_pointer_pos().map(|p| element_a(&cache.debuts, p.y - rect.top()));
            }
        });
        self.scenario_ui.vue_haut = vue.0;
        self.scenario_ui.vue_hauteur = vue.1;
        self.scenario_ui.cache = Some(cache);
        self.scenario_ui.recherche = rech;
        if let Some(i) = clic {
            if self.scenario_aller_a_l_element(g, i) {
                // Le passage cliqué est déjà sous les yeux : le panneau ne saute pas.
                let id = self.doc.panels.get(self.cur_panel).map(|p| p.id).unwrap_or(0);
                self.scenario_ui.dernier = Some((g.id, id));
            }
        }
    }
}
