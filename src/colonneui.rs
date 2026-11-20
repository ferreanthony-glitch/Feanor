//! L'affichage de la colonne en pile (`colonne.rs`, `docs/INTERFACE.md` partie 194) : le panneau `Pane::Colonne` d'egui_dock
//! dessine lui-même sa pile, en haut le bouton « + Panneau », puis chaque panneau avec son en-tête (poignée ⠿, chevron et
//! titre, ▲ ▼ ✕ ; ↗ pour Propriétés), son corps et le bord du bas qu'on tire. Le corps de chaque panneau est le contenu de
//! toujours (`App::pane_ui`), dans les marges des onglets d'egui_dock.
//!
//! Ce qui change la disposition entière (faire flotter, détacher, fermer par la commande d'un panneau) attend la fin de
//! l'affichage des panneaux (`App::colonne_apres`) : pendant qu'egui_dock dessine, son état n'est pas dans `App::dock`.

use eframe::egui::{self, pos2, vec2, Align, CursorIcon, Layout, Pos2, Rect, RichText, Sense, Stroke, UiBuilder};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::colonne::{self, Fermeture, PanneauPile, BORD, ENTETE};
use crate::dock::{DockNode, FloatingPanel, Pane};

/// Marges du corps d'un panneau de la pile : celles des onglets d'egui_dock (partie 161).
const MARGE_X: f32 = 5.0;
const MARGE_Y: f32 = 3.0;
/// Largeur de la bande des onglets d'icônes de Propriétés, et taille d'un onglet.
pub(crate) const BANDE_ONGLETS: f32 = 30.0;
const CASE_ONGLET: f32 = 26.0;
/// Hauteur de la ligne du haut de la colonne (le nom de l'espace de travail, « + Panneau »).
pub(crate) const BARRE: f32 = 24.0;

/// L'aide de la poignée d'un panneau de la pile.
pub(crate) const POIGNEE_PILE: &str = "Glisser vers le haut ou le bas : ranger le panneau ailleurs dans la colonne (une ligne montre où il tombe) ; lâché hors de la colonne, il flotte. Clic droit : dans une fenêtre à part.";
/// L'aide de « + Panneau ».
pub(crate) const AIDE_AJOUT: &str = "Il arrive en bas de la colonne ; glisse sa poignée pour le ranger ailleurs. Ceux qui sont déjà là sont grisés.";
/// L'aide de ↗.
pub(crate) const AIDE_SORTIR: &str = "L'onglet devant devient un panneau à part, juste au-dessus de Propriétés ; tu peux ensuite le déplacer. Le fermer le remet en onglet.";
/// Ce que montre Propriétés quand tous ses onglets sont sortis.
pub(crate) const TOUS_SORTIS: &str = "Tous les onglets sont sortis en panneaux : ferme l'un d'eux (✕), il revient ici.";

/// Ce que la colonne retient d'une image d'écran à l'autre (rien de cela n'est dans la disposition).
#[derive(Clone, Debug, Default)]
pub(crate) struct EtatColonne {
    /// Le panneau qu'on glisse par sa poignée.
    pub glisse: Option<usize>,
    /// Le panneau dont on tire le bord du bas, et la hauteur de son corps au début du geste.
    pub tire: Option<(usize, f32)>,
    /// Le haut et le bas de chaque panneau de la pile à l'écran, à la dernière image : là où tombe un panneau glissé.
    pub tranches: Vec<(f32, f32)>,
    /// La place de la colonne à l'écran, à la dernière image.
    pub zone: Option<Rect>,
    /// La hauteur du contenu de chaque panneau, mesurée à son dernier affichage dans la pile (« celle de son contenu »).
    pub contenus: std::collections::HashMap<Pane, f32>,
    /// Les panneaux rangés hors de la pile (onglets d'egui_dock, panneaux flottants, fenêtres à part), lus avant l'affichage :
    /// « déjà là » dans le menu « + Panneau ».
    pub ailleurs: Vec<Pane>,
    /// Ce qui attend la fin de l'affichage des panneaux.
    pub apres: Vec<Apres>,
}

/// Ce qui change la disposition entière, fait après l'affichage des panneaux (`App::colonne_apres`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Apres {
    /// Le panneau de la pile (ses panneaux, celui de devant) devient un panneau flottant, posé là.
    Flotter(Vec<Pane>, usize, Pos2),
    /// Fermé par sa commande (comme Affichage > Panneaux).
    Fermer(Pane),
    /// Un panneau rangé hors de la colonne (un onglet d'egui_dock, un panneau flottant) y revient.
    DansLaColonne(Pane),
}

/// Ce qu'un clic dans la pile demande, fait une fois la pile dessinée.
enum Geste {
    Replier(usize),
    Monter(usize),
    Descendre(usize),
    Fermer(usize),
    Sortir,
    SortirCelui(Pane),
    Deposer(usize, usize),
    Devant(Pane),
    Detacher(Pane),
    Flotter(usize, Pos2),
    FermerCelui(Pane),
    Ajouter(Option<Pane>),
}

/// Le nom, la description et l'action d'un panneau, pour sa bulle (celles de l'ancienne barre des panneaux).
pub(crate) fn bulle_de(pane: Pane) -> (&'static str, &'static str, &'static str) {
    match crate::barrepanneaux::entree_de_pane(pane) {
        Some(e) => (e.nom, e.description, e.action),
        None => (pane.name(), "", ""),
    }
}

impl App {
    /// La colonne, dans le corps du panneau `Pane::Colonne`.
    pub(crate) fn colonne_ui(&mut self, ui: &mut egui::Ui) {
        let zone = ui.max_rect();
        self.colonne_etat.zone = Some(zone);
        let mut gestes: Vec<Geste> = Vec::new();
        ui.painter().rect_filled(zone, 0.0, crate::theme::panneau());

        // La ligne du haut : le nom de l'espace de travail, et « + Panneau ».
        let barre = Rect::from_min_size(zone.min, vec2(zone.width(), BARRE));
        ui.painter().rect_filled(barre, 0.0, crate::theme::entete());
        let mut haut = ui.new_child(UiBuilder::new().max_rect(barre.shrink2(vec2(6.0, 1.0))).layout(Layout::left_to_right(Align::Center)));
        haut.label(RichText::new(self.espace.nom()).small().strong());
        haut.with_layout(Layout::right_to_left(Align::Center), |ui| self.colonne_menu_ajout(ui, &mut gestes));

        // La pile, sous elle.
        let pile = Rect::from_min_max(pos2(zone.left(), barre.bottom()), zone.max);
        let n = self.colonne.panneaux.len();
        let grandit = self.colonne.qui_grandit();
        let voulues: Vec<Option<colonne::Voulue>> = (0..n).map(|i| self.colonne_voulue(i)).collect();
        let hauteurs = colonne::hauteurs(pile.height(), &voulues, grandit);
        let mut tranches = Vec::with_capacity(n);
        let mut corps = ui.new_child(UiBuilder::new().max_rect(pile).layout(Layout::top_down(Align::Min)));
        corps.set_clip_rect(pile.intersect(ui.clip_rect()));
        egui::ScrollArea::vertical().id_salt("colonne_pile").auto_shrink([false, false]).show(&mut corps, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            for i in 0..n {
                let haut = ui.cursor().top();
                self.colonne_panneau(ui, i, hauteurs.get(i).copied().unwrap_or(0.0), Some(i) == grandit, n, &mut gestes);
                tranches.push((haut, ui.cursor().top()));
            }
            if n == 0 {
                ui.add_space(8.0);
                ui.label(RichText::new("La colonne est vide : « + Panneau », en haut, y ajoute un panneau.").small().weak());
            }
        });

        // Le panneau qu'on glisse : pâli, et une ligne où il tomberait (hors de la colonne : il flottera).
        if let Some(de) = self.colonne_etat.glisse {
            let pointeur = ui.ctx().pointer_latest_pos();
            if let Some(&(h, b)) = tranches.get(de) {
                ui.painter().rect_filled(Rect::from_min_max(pos2(zone.left(), h), pos2(zone.right(), b)).intersect(pile), 0.0, crate::theme::fond().gamma_multiply(0.55));
            }
            if let Some(p) = pointeur.filter(|p| zone.contains(*p)) {
                let cible = colonne::index_de_depot(&tranches, p.y);
                let y = match tranches.get(cible) {
                    Some((h, _)) => *h,
                    None => tranches.last().map_or(pile.top(), |(_, b)| *b),
                };
                let ligne = Rect::from_min_max(pos2(zone.left() + 4.0, y - 1.5), pos2(zone.right() - 4.0, y + 1.5)).intersect(pile.expand(2.0));
                ui.painter().rect_filled(ligne, 2.0, crate::theme::accent_texte());
            }
            ui.ctx().request_repaint();
        }
        self.colonne_etat.tranches = tranches;

        for g in gestes {
            self.colonne_geste(g);
        }
        // (Un geste dont la poignée ou le bord n'a pas vu la fin, le panneau rangé ou fermé entre-temps : il s'arrête là.)
        if !ui.input(|i| i.pointer.any_down() || i.pointer.any_released()) {
            self.colonne_etat.glisse = None;
            self.colonne_etat.tire = None;
        }
    }

    /// Ce que demande le corps du panneau `i` : sa hauteur tirée à la main, sinon celle de son contenu (gardées, la colonne défile
    /// plutôt), sinon la hauteur d'origine d'une liste (qui cède s'il le faut, partie 196.5) ; rien s'il est replié.
    fn colonne_voulue(&self, i: usize) -> Option<colonne::Voulue> {
        let p = self.colonne.panneaux.get(i)?;
        if p.replie {
            return None;
        }
        if let Some(h) = p.hauteur {
            return Some(colonne::Voulue::ferme(h));
        }
        let contenu = p.devant().filter(|pane| !crate::dockui::defile_seul(*pane)).and_then(|pane| self.colonne_etat.contenus.get(&pane).copied());
        Some(match contenu {
            Some(h) => colonne::Voulue::ferme((h + 2.0 * MARGE_Y + 1.0).clamp(colonne::HAUTEUR_MIN.min(h + 2.0 * MARGE_Y + 1.0), colonne::HAUTEUR_MAX)),
            // (Les Préréglages gardent au moins leur aperçu et quelques lignes de leur liste.)
            None => match p.devant() {
                Some(Pane::Prereglages) => match crate::panneauprereglages::plancher(&self.ctx) {
                    Some(m) => colonne::Voulue::souple_au_moins(colonne::HAUTEUR_DEFAUT, m + 2.0 * MARGE_Y + 1.0),
                    None => colonne::Voulue::souple(colonne::HAUTEUR_DEFAUT),
                },
                _ => colonne::Voulue::souple(colonne::HAUTEUR_DEFAUT),
            },
        })
    }

    /// Le menu « + Panneau » : les panneaux que cet espace de travail peut montrer, ceux qui y sont déjà grisés.
    fn colonne_menu_ajout(&mut self, ui: &mut egui::Ui, gestes: &mut Vec<Geste>) {
        let resp = ui
            .menu_button(RichText::new(format!("{}  Panneau", ph::PLUS)).small(), |ui| {
                ui.label(RichText::new("Ajouter un panneau").small().weak());
                let a_proprietes = self.colonne.proprietes().is_some();
                let animation = self.mode == crate::app::Mode::Animation;
                let liste = colonne::propositions(self.espace, animation, self.settings.experimental, a_proprietes, &|p| self.colonne.contient(p) || self.colonne_etat.ailleurs.contains(&p));
                for prop in liste {
                    let icone = prop.pane.map_or(ph::SQUARES_FOUR, Pane::icon);
                    let texte = if prop.deja_la { format!("{icone}  {}   (déjà là)", prop.nom()) } else { format!("{icone}  {}", prop.nom()) };
                    let (nom, description) = match prop.pane {
                        Some(p) => {
                            let (n, d, _) = bulle_de(p);
                            (n, d)
                        }
                        None => ("Propriétés", "Les panneaux de réglages en onglets d'icônes, sur son bord gauche ; il grandit et prend la hauteur qui reste."),
                    };
                    let b = ui.add_enabled(!prop.deja_la, egui::Button::new(texte).frame(false));
                    if crate::bulles::bulle(b, nom, description, "").clicked() {
                        gestes.push(Geste::Ajouter(prop.pane));
                        ui.close_menu();
                    }
                }
            })
            .response;
        crate::bulles::bulle(resp, "Ajouter un panneau", AIDE_AJOUT, "");
    }

    /// Un panneau de la pile : son en-tête, son corps (`h` points), le bord du bas qu'on tire s'il ne grandit pas.
    fn colonne_panneau(&mut self, ui: &mut egui::Ui, i: usize, h: f32, grandit: bool, n: usize, gestes: &mut Vec<Geste>) {
        let Some(p) = self.colonne.panneaux.get(i).cloned() else { return };
        let w = ui.available_width();
        let (tete, _) = ui.allocate_exact_size(vec2(w, ENTETE), Sense::hover());
        self.colonne_entete(ui, tete, i, &p, n, gestes);
        if p.replie {
            return;
        }
        let (corps, _) = ui.allocate_exact_size(vec2(w, h.max(0.0)), Sense::hover());
        if p.est_proprietes() {
            self.colonne_proprietes(ui, corps, &p, gestes);
        } else if let Some(pane) = p.devant() {
            self.colonne_corps(ui, corps, pane);
        }
        if grandit {
            return;
        }
        // Le bord du bas : tiré, il règle la hauteur du corps (bornée, `colonne::borner_hauteur`).
        let (bord, resp) = ui.allocate_exact_size(vec2(w, BORD), Sense::drag());
        let resp = resp.on_hover_cursor(CursorIcon::ResizeVertical);
        if resp.drag_started() {
            self.colonne_etat.tire = Some((i, h));
        }
        if resp.dragged() {
            if let (Some((j, h0)), Some(origine), Some(ici)) = (self.colonne_etat.tire, ui.ctx().input(|x| x.pointer.press_origin()), ui.ctx().pointer_latest_pos()) {
                if j == i {
                    self.colonne.regler_hauteur(i, h0 + ici.y - origine.y);
                }
            }
        }
        if resp.drag_stopped() {
            self.colonne_etat.tire = None;
        }
        let couleur = if resp.hovered() || resp.dragged() { crate::theme::accent_texte().gamma_multiply(0.6) } else { crate::theme::fond() };
        ui.painter().rect_filled(bord, 0.0, couleur);
    }

    /// L'en-tête du panneau `i` : poignée, chevron et titre (replier, déplier), les onglets d'en-tête s'il a plusieurs panneaux,
    /// puis ↗ (Propriétés), ▲, ▼ et ✕. Clic droit : fenêtre à part, flotter, replier, fermer.
    fn colonne_entete(&mut self, ui: &mut egui::Ui, rect: Rect, i: usize, p: &PanneauPile, n: usize, gestes: &mut Vec<Geste>) {
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 0.0, crate::theme::entete());
        painter.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, crate::theme::bord()));
        // (Le fond de l'en-tête d'abord : les boutons posés dessus passent devant lui.)
        let fond = ui.interact(rect, ui.id().with(("entete_pile", i)), Sense::click());
        let interieur = rect.shrink2(vec2(2.0, 1.0));

        // À droite : ✕, ▼, ▲, puis ↗ pour Propriétés.
        let mut droite = ui.new_child(UiBuilder::new().max_rect(interieur).layout(Layout::right_to_left(Align::Center)));
        droite.spacing_mut().item_spacing.x = 1.0;
        let seul = n <= 1;
        let fermer_aide = if seul {
            "Il faut au moins un panneau dans la colonne (Affichage > Panneaux la vide)."
        } else if p.sorti && self.colonne.proprietes().is_some() {
            "Il revient en onglet de Propriétés."
        } else {
            "Il revient avec « + Panneau »."
        };
        let x = droite.add_enabled(!seul, bouton_entete(ph::X));
        if crate::bulles::bulle(x, "Fermer le panneau", fermer_aide, "").clicked() {
            gestes.push(Geste::Fermer(i));
        }
        let bas = droite.add_enabled(i + 1 < n, bouton_entete(ph::ARROW_DOWN));
        if crate::bulles::bulle(bas, "Descendre le panneau", "Un cran plus bas dans la colonne.", "").clicked() {
            gestes.push(Geste::Descendre(i));
        }
        let monte = droite.add_enabled(i > 0, bouton_entete(ph::ARROW_UP));
        if crate::bulles::bulle(monte, "Monter le panneau", "Un cran plus haut dans la colonne.", "").clicked() {
            gestes.push(Geste::Monter(i));
        }
        if p.est_proprietes() {
            let sortir = droite.add_enabled(p.devant().is_some(), bouton_entete(ph::ARROW_SQUARE_OUT));
            if crate::bulles::bulle(sortir, "Sortir l'onglet en panneau", AIDE_SORTIR, "").clicked() {
                gestes.push(Geste::Sortir);
            }
        }
        let pris = droite.min_rect().width();

        // À gauche : la poignée, le chevron et le titre, les onglets de l'en-tête.
        let reste = Rect::from_min_max(interieur.min, pos2((interieur.right() - pris - 4.0).max(interieur.left()), interieur.bottom()));
        let mut gauche = ui.new_child(UiBuilder::new().max_rect(reste).layout(Layout::left_to_right(Align::Center)));
        gauche.set_clip_rect(reste.intersect(ui.clip_rect()));
        gauche.spacing_mut().item_spacing.x = 3.0;
        let poignee = gauche.add(egui::Label::new(RichText::new(ph::DOTS_SIX_VERTICAL).size(13.0).color(crate::theme::texte_doux())).sense(Sense::click_and_drag()).selectable(false));
        let poignee = crate::bulles::bulle(poignee.on_hover_cursor(CursorIcon::Grab), "Déplacer le panneau", POIGNEE_PILE, "");
        if poignee.drag_started() {
            self.colonne_etat.glisse = Some(i);
        }
        if poignee.drag_stopped() {
            if self.colonne_etat.glisse == Some(i) {
                match ui.ctx().pointer_latest_pos() {
                    Some(ici) if self.colonne_etat.zone.is_some_and(|z| !z.contains(ici)) => gestes.push(Geste::Flotter(i, ici)),
                    Some(ici) => gestes.push(Geste::Deposer(i, colonne::index_de_depot(&self.colonne_etat.tranches, ici.y))),
                    None => {}
                }
            }
            self.colonne_etat.glisse = None;
        }
        let chevron = if p.replie { ph::CARET_RIGHT } else { ph::CARET_DOWN };
        let icone = if p.est_proprietes() { ph::SQUARES_FOUR } else { p.devant().map_or(ph::SQUARES_FOUR, Pane::icon) };
        let titre = gauche.add(
            egui::Label::new(RichText::new(format!("{chevron} {icone}  {}", p.titre())).strong().size(11.5)).sense(Sense::click()).selectable(false).truncate(),
        );
        let (nom, description) = if p.est_proprietes() {
            ("Propriétés", "Les panneaux de réglages, en onglets d'icônes sur le bord gauche. Un clic sur le titre replie ou déplie le panneau.")
        } else {
            let (n, d, _) = p.devant().map_or(("Panneau", "", ""), bulle_de);
            (n, d)
        };
        let titre = crate::bulles::bulle(titre, nom, description, "");
        if titre.clicked() {
            gestes.push(Geste::Replier(i));
        }
        // Plusieurs panneaux (la Couleur et les Brosses dédiées) : un onglet d'icône chacun, dans l'en-tête.
        if !p.est_proprietes() && p.panneaux.len() > 1 {
            for pane in &p.panneaux {
                let on = p.devant() == Some(*pane) && !p.replie;
                let r = gauche.add(egui::Button::new(crate::bulles::encre_etat(RichText::new(pane.icon()).size(12.0), on)).selected(on).frame(on).min_size(vec2(18.0, 18.0)));
                let (n, d, a) = bulle_de(*pane);
                if crate::bulles::bulle(r, n, d, &self.raccourci(a)).clicked() {
                    gestes.push(Geste::Devant(*pane));
                }
            }
        }
        for r in [&fond, &poignee, &titre] {
            r.context_menu(|ui| self.colonne_menu_entete(ui, i, p, n, gestes));
        }
    }

    /// Le clic droit sur l'en-tête d'un panneau de la pile.
    fn colonne_menu_entete(&mut self, ui: &mut egui::Ui, i: usize, p: &PanneauPile, n: usize, gestes: &mut Vec<Geste>) {
        if let (false, Some(pane)) = (p.est_proprietes(), p.devant()) {
            if ui.button("Dans une fenêtre à part").on_hover_text("Une vraie fenêtre, à poser sur un autre écran ; la fermer remet le panneau dans la colonne").clicked() {
                gestes.push(Geste::Detacher(pane));
                ui.close_menu();
            }
            if ui.button("Panneau flottant").on_hover_text("Il quitte la colonne et flotte au-dessus du dessin ; glisse sa poignée pour le ranger ailleurs").clicked() {
                let ici = self.colonne_etat.zone.map_or(pos2(200.0, 200.0), |z| pos2(z.left() - colonne_largeur_flottant() - 20.0, z.top() + 40.0));
                gestes.push(Geste::Flotter(i, ici));
                ui.close_menu();
            }
        }
        if p.est_proprietes() && ui.add_enabled(p.devant().is_some(), egui::Button::new("Sortir l'onglet en panneau")).clicked() {
            gestes.push(Geste::Sortir);
            ui.close_menu();
        }
        if ui.button(if p.replie { "Déplier" } else { "Replier" }).clicked() {
            gestes.push(Geste::Replier(i));
            ui.close_menu();
        }
        if ui.add_enabled(n > 1, egui::Button::new("Fermer le panneau")).clicked() {
            gestes.push(Geste::Fermer(i));
            ui.close_menu();
        }
    }

    /// Le corps de Propriétés : les onglets d'icônes sur le bord gauche, l'onglet devant à côté.
    fn colonne_proprietes(&mut self, ui: &mut egui::Ui, corps: Rect, p: &PanneauPile, gestes: &mut Vec<Geste>) {
        let bande = Rect::from_min_size(corps.min, vec2(BANDE_ONGLETS, corps.height()));
        ui.painter().rect_filled(bande, 0.0, crate::theme::entete());
        ui.painter().line_segment([bande.right_top(), bande.right_bottom()], Stroke::new(1.0, crate::theme::bord()));
        let mut onglets = ui.new_child(UiBuilder::new().max_rect(bande.shrink2(vec2(2.0, 4.0))).layout(Layout::top_down(Align::Center)));
        onglets.set_clip_rect(bande.intersect(ui.clip_rect()));
        onglets.spacing_mut().item_spacing.y = 2.0;
        let devant = p.devant();
        for pane in &p.panneaux {
            let on = devant == Some(*pane);
            #[allow(deprecated)]
            let r = onglets.add_sized(vec2(CASE_ONGLET, CASE_ONGLET), egui::SelectableLabel::new(on, crate::bulles::encre_etat(RichText::new(pane.icon()).size(15.0), on)));
            let (nom, description, action) = bulle_de(*pane);
            let r = crate::bulles::bulle(r, nom, description, &self.raccourci(action));
            if r.clicked() {
                gestes.push(Geste::Devant(*pane));
            }
            r.context_menu(|ui| {
                if ui.button("Sortir l'onglet en panneau").on_hover_text(AIDE_SORTIR).clicked() {
                    gestes.push(Geste::SortirCelui(*pane));
                    ui.close_menu();
                }
                if ui.button("Dans une fenêtre à part").clicked() {
                    gestes.push(Geste::Detacher(*pane));
                    ui.close_menu();
                }
                if ui.button("Fermer l'onglet").clicked() {
                    gestes.push(Geste::FermerCelui(*pane));
                    ui.close_menu();
                }
            });
        }
        let reste = Rect::from_min_max(pos2(bande.right() + 1.0, corps.top()), corps.max);
        match devant {
            Some(pane) => self.colonne_corps(ui, reste, pane),
            None => {
                let mut c = ui.new_child(UiBuilder::new().max_rect(reste.shrink(8.0)).layout(Layout::top_down(Align::Min)));
                c.label(RichText::new(TOUS_SORTIS).small().weak());
            }
        }
    }

    /// Le contenu de `pane` dans `rect`, avec les marges et le défilement qu'il avait en onglet d'egui_dock ; sa hauteur est
    /// retenue (`EtatColonne::contenus`).
    fn colonne_corps(&mut self, ui: &mut egui::Ui, rect: Rect, pane: Pane) {
        ui.painter().rect_filled(rect, 0.0, crate::theme::panneau());
        let interieur = rect.shrink2(vec2(MARGE_X, MARGE_Y));
        if interieur.height() <= 1.0 || interieur.width() <= 1.0 {
            return;
        }
        let mut c = ui.new_child(UiBuilder::new().id_salt(("pile", pane)).max_rect(interieur).layout(Layout::top_down(Align::Min)));
        c.set_clip_rect(interieur.intersect(ui.clip_rect()));
        // (Partie 195 : les écarts d'origine. Ceux de la pile, à zéro pour coller les panneaux, passaient au contenu : « Pinceau » et
        // « calque vectoriel », dans l'en-tête des Réglages, se touchaient.)
        c.spacing_mut().item_spacing = ui.ctx().style().spacing.item_spacing;
        if crate::dockui::defile_seul(pane) {
            self.pane_ui(&mut c, pane);
            return;
        }
        let sortie = egui::ScrollArea::vertical().id_salt(("pile_defile", pane)).auto_shrink([false, false]).max_height(interieur.height()).show(&mut c, |ui| {
            let haut = ui.cursor().top();
            self.pane_ui(ui, pane);
            ui.cursor().top() - haut
        });
        let h = sortie.inner;
        if h.is_finite() && h > 0.0 {
            let avant = self.colonne_etat.contenus.insert(pane, h);
            if avant.is_none_or(|a| (a - h).abs() > 0.5) {
                ui.ctx().request_repaint();
            }
            if crate::ajuste::measured(pane) {
                self.pane_heights.insert(pane, h);
            }
        }
    }

    /// Fait ce qu'un clic dans la pile a demandé.
    fn colonne_geste(&mut self, g: Geste) {
        match g {
            Geste::Replier(i) => self.colonne.replier(i),
            Geste::Monter(i) => self.colonne.monter(i),
            Geste::Descendre(i) => self.colonne.descendre(i),
            Geste::Deposer(de, cible) => {
                self.colonne.deposer(de, cible);
                if let Some(p) = self.colonne.panneaux.get(colonne::place_apres_depot(de, cible)) {
                    if colonne::place_apres_depot(de, cible) != de {
                        self.status = format!("Panneau « {} » rangé.", p.titre());
                    }
                }
            }
            Geste::Fermer(i) => {
                let titre = self.colonne.panneaux.get(i).map(PanneauPile::titre).unwrap_or_default();
                let pane = self.colonne.panneaux.get(i).and_then(|p| (!p.est_proprietes()).then(|| p.panneaux.clone())).unwrap_or_default();
                match self.colonne.fermer_panneau(i) {
                    Fermeture::Ferme => {
                        // (Le panneau Espace 3D fermé à la main ne revient qu'avec la vue, ou à la demande.)
                        if pane.contains(&Pane::Espace3d) {
                            self.e3_panneau.a_la_main = false;
                        }
                        self.status = format!("Panneau « {titre} » fermé : « + Panneau » le rouvre.");
                    }
                    Fermeture::RevenuEnOnglet => self.status = format!("« {titre} » est revenu en onglet de Propriétés."),
                    Fermeture::Rien => {}
                }
            }
            Geste::Sortir => {
                if let Some(p) = self.colonne.sortir_l_onglet() {
                    self.status = format!("« {} » est maintenant un panneau.", p.name());
                }
            }
            Geste::SortirCelui(pane) => {
                self.colonne.mettre_devant(pane);
                self.colonne_geste(Geste::Sortir);
            }
            Geste::Devant(pane) => {
                self.colonne.montrer(pane);
            }
            Geste::Detacher(pane) => self.detach_request = Some(pane),
            Geste::Flotter(i, ici) => {
                if let Some(p) = self.colonne.panneaux.get(i).filter(|p| !p.est_proprietes()) {
                    self.colonne_etat.apres.push(Apres::Flotter(p.panneaux.clone(), p.actif, ici));
                }
            }
            Geste::FermerCelui(pane) => self.colonne_etat.apres.push(Apres::Fermer(pane)),
            Geste::Ajouter(None) => {
                let ailleurs = self.colonne_etat.ailleurs.clone();
                self.colonne.ajouter_proprietes(&colonne::onglets_d_origine(self.espace), &|p| ailleurs.contains(&p));
                self.status = "Panneau « Propriétés » ajouté en bas de la colonne.".into();
            }
            Geste::Ajouter(Some(pane)) => {
                if self.refuse_hors_animation(pane) {
                    return;
                }
                if pane == Pane::Espace3d {
                    self.e3_panneau.a_la_main = !self.espace3d_vue_ouverte();
                }
                self.colonne.ajouter(pane);
                self.status = format!("Panneau « {} » ajouté en bas de la colonne.", pane.name());
            }
        }
    }

    /// Après l'affichage des panneaux (`show_dock`) : ce que la colonne a demandé et qui change la disposition entière.
    pub(crate) fn colonne_apres(&mut self) {
        for a in std::mem::take(&mut self.colonne_etat.apres) {
            match a {
                Apres::Flotter(panes, actif, ici) => {
                    let mut layout = self.dock_layout();
                    for p in &panes {
                        layout.colonne.retirer(*p);
                    }
                    layout.fenetres.push(FloatingPanel {
                        panneaux: DockNode::Tabs { panneaux: panes.clone(), actif },
                        x: (ici.x - 20.0).max(0.0),
                        y: (ici.y - 10.0).max(0.0),
                        largeur: colonne_largeur_flottant(),
                        hauteur: 320.0,
                    });
                    if layout.colonne.is_empty() {
                        layout.close(Pane::Colonne);
                    }
                    self.set_dock_layout(layout);
                    if let Some(p) = panes.first() {
                        self.status = format!("Panneau « {} » flottant : glisse sa poignée sur la colonne pour le remettre, ou clic droit « Dans la colonne ».", p.name());
                    }
                }
                Apres::Fermer(pane) => {
                    if self.pane_shown(pane) {
                        self.toggle_pane(pane);
                    }
                }
                Apres::DansLaColonne(pane) => {
                    let mut layout = self.dock_layout();
                    layout.ranger_dans_la_colonne(pane);
                    self.set_dock_layout(layout);
                    self.colonne.montrer(pane);
                }
            }
        }
    }
}

/// La largeur d'un panneau qui quitte la colonne pour flotter : celle de la colonne.
fn colonne_largeur_flottant() -> f32 {
    crate::dock::COLONNE
}

/// Un petit bouton d'en-tête (▲ ▼ ✕ ↗), sans cadre.
fn bouton_entete(symbole: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(symbole.to_string()).size(12.0)).frame(false).min_size(vec2(18.0, 18.0))
}
