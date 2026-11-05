//! « Mon panneau » (essai du menu Labo, 2026-10-28), l'écran : un panneau ancrable, comme les autres, avec une grille de boutons-icônes.
//! Un bouton est une commande du registre (`keymap::ACTIONS`, celui des raccourcis et de Ctrl+K : pas de second système), une
//! brosse de « Mes brosses » ou un outil perso (l'outil, ses réglages de brosse et sa couleur, remis d'un clic).
//!
//! - Ajouter : clic droit sur une icône de SHOT > « Ajouter à Mon panneau » (l'entrée n'existe que tant que le panneau est ouvert) ;
//!   le « + » du panneau cherche une commande ou une brosse, comme Ctrl+K ; « Enregistrer l'outil actuel ici » garde l'outil courant.
//! - Organiser : glisser pour réordonner ; clic droit sur un bouton : retirer, renommer (outil perso), séparateur, raccourci.
//! - « Afficher les noms » : le nom sous chaque icône, ou les icônes seules.
//!
//! Rien de lourd à l'écran : la grille ne mesure que ses quelques boutons, le clic agit tout de suite, rien n'est enregistré avant
//! un changement. Le choix est un réglage du poste (`Settings::mon_panneau`), pas du projet. La logique est dans `monpanneau.rs`.

use std::collections::HashMap;

use eframe::egui::{self, pos2, vec2, Align2, Color32, Context, FontId, Id, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};
use egui_phosphor::regular as ph;

use crate::app::{App, Tool};
use crate::brushtex::{Target, MINE};
use crate::bulles;
use crate::dock::Pane;
use crate::monpanneau::{case_la_plus_proche, commande_connue, disposer, outil_de, Bouton, OutilPerso, LARGEUR_SEPARATEUR};

/// Taille de l'icône d'un bouton.
const ICONE: f32 = 18.0;
/// Un bouton, icône seule ; avec son nom dessous.
const CASE: Vec2 = vec2(34.0, 34.0);
const CASE_NOM: Vec2 = vec2(64.0, 52.0);
const ECART: f32 = 3.0;
/// Résultats montrés par la recherche du « + ».
const RESULTATS: usize = 8;
const RESULTATS_BROSSES: usize = 4;

/// Ce que le panneau retient le temps d'une séance (rien de tout cela n'est enregistré).
#[derive(Default)]
pub(crate) struct Etat {
    /// La recherche du « + », ouverte, avec ce qui est tapé.
    pub recherche: Option<String>,
    recherche_prise: bool,
    /// Le nom d'un outil perso en cours de saisie : son rang et ce qui est tapé.
    pub renommer: Option<(usize, String)>,
    renommer_pris: bool,
    /// Le bouton qu'on glisse.
    glisse: Option<usize>,
}

// ---------------------------------------------------------------------------------------- liens avec le reste de SHOT

const OUVERT: &str = "shot_mon_panneau_ouvert";
const AJOUT: &str = "shot_mon_panneau_ajout";
const ICONES: &str = "shot_mon_panneau_icones";

/// Le panneau est-il ouvert ? (Dit à chaque image par `mon_panneau_suivre` : les menus du clic droit, dessinés sans accès à
/// l'application, le lisent pour montrer ou non « Ajouter à Mon panneau ».)
pub(crate) fn ouvert(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(Id::new(OUVERT))).unwrap_or(false)
}

/// Demande d'ajouter `action` à Mon panneau (lue à l'image suivante, `mon_panneau_suivre`).
pub(crate) fn demander_ajout(ctx: &Context, action: &str) {
    ctx.data_mut(|d| d.insert_temp(Id::new(AJOUT), action.to_string()));
}

/// L'icône, le nom et la description d'un bouton, comme ailleurs dans SHOT.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Infos {
    pub icone: String,
    pub nom: String,
    pub description: String,
}

/// Une icône que SHOT vient de dessiner pour une action : retenue le temps de la séance, seulement tant que Mon panneau est ouvert
/// (sinon cette fonction ne fait que lire un drapeau). Elle donne leur « icône habituelle » aux commandes qui n'ont pas d'icône au
/// registre des barres et des outils.
pub(crate) fn apprendre(ctx: &Context, action: &str, symbole: &str, nom: &str, description: &str) {
    if action.is_empty() || symbole.is_empty() || !ouvert(ctx) {
        return;
    }
    ctx.data_mut(|d| {
        let m = d.get_temp_mut_or_default::<HashMap<String, Infos>>(Id::new(ICONES));
        if !m.get(action).is_some_and(|i| i.icone == symbole && i.nom == nom) {
            m.insert(action.to_string(), Infos { icone: symbole.to_string(), nom: nom.to_string(), description: description.to_string() });
        }
    });
}

/// Ce que montre le bouton d'une commande : l'outil (son icône, son nom, sa description), le panneau, la commande d'une barre, ce
/// que SHOT a dessiné pour elle, sinon une icône par défaut sous le nom du registre.
pub(crate) fn infos(ctx: &Context, action: &str) -> Infos {
    if let Some(t) = outil_de(action) {
        return Infos { icone: t.icon().into(), nom: t.label().into(), description: t.description().into() };
    }
    if let Some(e) = crate::barrepanneaux::entree_de_action(action) {
        return Infos { icone: e.pane.icon().into(), nom: e.nom.into(), description: e.description.into() };
    }
    use crate::barres::Genre;
    let barre = [crate::barres::COMMANDES, crate::barres::MONTAGE, crate::barres::TIMELINE]
        .into_iter()
        .flatten()
        .find(|c| c.action == action && matches!(c.genre, Genre::Bouton | Genre::Etat));
    if let Some(c) = barre {
        return Infos { icone: c.icone.into(), nom: c.nom.into(), description: c.description.into() };
    }
    if let Some(i) = ctx.data_mut(|d| d.get_temp_mut_or_default::<HashMap<String, Infos>>(Id::new(ICONES)).get(action).cloned()) {
        return i;
    }
    let nom = crate::keymap::action_info(action).map_or_else(|| action.to_string(), |i| i.label.to_string());
    Infos { icone: ph::LIGHTNING.into(), nom, description: String::new() }
}

/// Ce qu'un bouton montre à l'instant.
struct Vue {
    icone: String,
    nom: String,
    description: String,
    raccourci: String,
    allume: bool,
    actif: bool,
    pastille: Option<[u8; 3]>,
    separateur: bool,
    /// L'action, pour une commande.
    commande: Option<String>,
    outil_perso: bool,
}

/// Ce que demande un geste sur la grille (un seul par image).
enum Act {
    Clic(usize),
    Retirer(usize),
    Renommer(usize),
    Separateur(usize),
    Deplacer(usize, usize),
    Plus,
    Raccourci(String),
}

impl App {
    // ------------------------------------------------------------------------------------------------ l'état partagé

    /// À chaque image, avant les panneaux : Mon panneau est-il ouvert (dit aux menus du clic droit), et un ajout demandé par un
    /// clic droit.
    pub(crate) fn mon_panneau_suivre(&mut self, ctx: &Context) {
        let ouvert = self.pane_shown(Pane::MonPanneau);
        ctx.data_mut(|d| d.insert_temp(Id::new(OUVERT), ouvert));
        if let Some(action) = ctx.data_mut(|d| d.remove_temp::<String>(Id::new(AJOUT))) {
            if ouvert {
                self.mon_panneau_ajouter_commande(&action);
            }
        }
    }

    /// Garde le panneau dans les réglages du poste.
    fn mon_panneau_sauver(&mut self) {
        if let Err(e) = self.settings.save() {
            self.status = format!("Mon panneau : pas gardé pour la prochaine fois ({e}).");
        }
    }

    // --------------------------------------------------------------------------------------------------- ajouter

    /// Ajoute une commande du registre à la fin du panneau.
    pub(crate) fn mon_panneau_ajouter_commande(&mut self, action: &str) {
        if !commande_connue(action) {
            self.status = "Cette commande ne peut pas aller dans Mon panneau.".into();
            return;
        }
        let nom = infos(&self.ctx, action).nom;
        if self.settings.mon_panneau.ajouter(Bouton::Commande { action: action.to_string() }) {
            self.mon_panneau_sauver();
            self.status = format!("« {nom} » ajouté à Mon panneau.");
        } else {
            self.status = format!("« {nom} » est déjà dans Mon panneau.");
        }
    }

    /// Ajoute la brosse de rang `i` de la bibliothèque (on ne garde que son nom : la brosse reste dans la bibliothèque).
    pub(crate) fn mon_panneau_ajouter_brosse(&mut self, i: usize) {
        let Some(p) = self.brushes.presets.get(i) else { return };
        let (nom, source, gomme) = (p.name.clone(), p.source.clone(), p.target == Target::Eraser);
        if self.settings.mon_panneau.ajouter(Bouton::Brosse { nom: nom.clone(), source, gomme }) {
            self.mon_panneau_sauver();
            self.status = format!("La brosse « {nom} » est ajoutée à Mon panneau.");
        } else {
            self.status = format!("La brosse « {nom} » est déjà dans Mon panneau.");
        }
    }

    /// L'outil courant, les réglages de sa brosse et la couleur d'avant-plan, sous le nom `nom`.
    pub(crate) fn capturer_outil_perso(&self, nom: String) -> OutilPerso {
        let pinceau = match self.tool {
            Tool::Brush | Tool::Mixer => Some(self.brush),
            Tool::Pencil => Some(self.pencil),
            Tool::Eraser => Some(self.eraser),
            _ => None,
        };
        OutilPerso { nom, outil: self.tool.action().to_string(), pinceau, couleur: self.color }
    }

    /// « Enregistrer l'outil actuel ici » : un outil perso de plus, dont on saisit tout de suite le nom.
    pub(crate) fn mon_panneau_enregistrer_outil(&mut self) {
        let nom = self.settings.mon_panneau.nom_libre(self.tool);
        let outil = self.capturer_outil_perso(nom.clone());
        self.settings.mon_panneau.ajouter(Bouton::Outil(outil));
        let rang = self.settings.mon_panneau.boutons.len() - 1;
        self.mon_panneau_sauver();
        self.mon_panneau.renommer = Some((rang, nom.clone()));
        self.mon_panneau.renommer_pris = false;
        self.status = format!("L'outil actuel est gardé dans Mon panneau (« {nom} ») : tu peux le renommer.");
    }

    // --------------------------------------------------------------------------------------------------- le clic

    /// Remet l'outil, la brosse et la couleur d'un outil perso, d'un coup.
    pub(crate) fn appliquer_outil_perso(&mut self, o: &OutilPerso) -> bool {
        let Some(tool) = o.outil() else {
            self.status = format!("« {} » : cet outil n'existe plus.", o.nom);
            return false;
        };
        self.set_tool(tool);
        if let Some(p) = o.pinceau {
            match tool {
                Tool::Pencil => self.pencil = p,
                Tool::Eraser => self.eraser = p,
                Tool::Brush | Tool::Mixer => self.brush = p,
                _ => {}
            }
            self.brush_previews.memory.clear();
        }
        self.set_front(o.couleur);
        self.status = format!("Outil perso « {} ».", o.nom);
        true
    }

    /// Le rang, dans la bibliothèque, de la brosse que désigne un bouton.
    fn trouver_brosse(&self, nom: &str, source: &str, gomme: bool) -> Option<usize> {
        self.brushes.presets.iter().position(|p| p.name == nom && p.source == source && (p.target == Target::Eraser) == gomme)
    }

    /// Prend la brosse d'un bouton : son réglage devient celui du pinceau (ou de la gomme), et l'outil passe dessus.
    pub(crate) fn appliquer_brosse_nommee(&mut self, nom: &str, source: &str, gomme: bool) -> bool {
        let Some(i) = self.trouver_brosse(nom, source, gomme) else {
            self.status = if self.brushes_job.is_some() {
                "Les pinceaux importés se chargent encore : réessaie dans un instant.".into()
            } else {
                format!("La brosse « {nom} » n'est plus dans la bibliothèque.")
            };
            return false;
        };
        let p = self.brushes.presets[i].clone();
        if gomme {
            self.eraser = p.settings;
            self.set_tool(Tool::Eraser);
        } else {
            self.brush = p.settings;
            if !matches!(self.tool, Tool::Brush | Tool::Mixer) {
                self.set_tool(Tool::Brush);
            }
        }
        self.brush_previews.memory.clear();
        self.brush_previews.base = Some((p.target, p.name.clone(), p.source.clone()));
        self.status = format!("Réglage « {} ».", p.name);
        true
    }

    /// Un clic sur le bouton de rang `i`.
    pub(crate) fn mon_panneau_clic(&mut self, i: usize) {
        let Some(b) = self.settings.mon_panneau.boutons.get(i).cloned() else { return };
        match b {
            Bouton::Commande { action } => self.run_action(&action),
            Bouton::Brosse { nom, source, gomme } => {
                self.appliquer_brosse_nommee(&nom, &source, gomme);
            }
            Bouton::Outil(o) => {
                self.appliquer_outil_perso(&o);
            }
            Bouton::Separateur | Bouton::Inconnu => {}
        }
    }

    // ------------------------------------------------------------------------------------------------ l'affichage

    /// Ce que le bouton `b` montre à l'instant.
    fn mon_panneau_vue(&self, b: &Bouton) -> Vue {
        let vide = |separateur: bool| Vue {
            icone: String::new(),
            nom: String::new(),
            description: String::new(),
            raccourci: String::new(),
            allume: false,
            actif: true,
            pastille: None,
            separateur,
            commande: None,
            outil_perso: false,
        };
        match b {
            Bouton::Commande { action } => {
                let i = infos(&self.ctx, action);
                Vue {
                    icone: i.icone,
                    nom: i.nom,
                    description: i.description,
                    raccourci: self.raccourci(action),
                    allume: outil_de(action) == Some(self.tool),
                    commande: Some(action.clone()),
                    ..vide(false)
                }
            }
            Bouton::Brosse { nom, source, gomme } => {
                let trouvee = self.trouver_brosse(nom, source, *gomme).map(|i| &self.brushes.presets[i]);
                let (outil_ok, courant) = if *gomme { (self.tool == Tool::Eraser, self.eraser) } else { (matches!(self.tool, Tool::Brush | Tool::Mixer), self.brush) };
                let description = match trouvee {
                    Some(_) => source.clone(),
                    None if self.brushes_job.is_some() => "Les pinceaux importés se chargent encore.".to_string(),
                    None => "Cette brosse n'est plus dans la bibliothèque : clic droit pour retirer ce bouton.".to_string(),
                };
                Vue {
                    icone: (if *gomme { ph::ERASER } else { ph::PAINT_BRUSH }).to_string(),
                    nom: nom.clone(),
                    description,
                    allume: outil_ok && trouvee.is_some_and(|p| p.settings == courant),
                    actif: trouvee.is_some(),
                    ..vide(false)
                }
            }
            Bouton::Outil(o) => {
                let tool = o.outil();
                let allume = tool == Some(self.tool)
                    && self.color == o.couleur
                    && o.pinceau.is_none_or(|p| match self.tool {
                        Tool::Pencil => self.pencil == p,
                        Tool::Eraser => self.eraser == p,
                        _ => self.brush == p,
                    });
                Vue {
                    icone: tool.map_or_else(|| ph::LIGHTNING.to_string(), |t| t.icon().to_string()),
                    nom: o.nom.clone(),
                    description: match tool {
                        Some(t) => format!("{} avec ses réglages et sa couleur, d'un clic. Clic droit : renommer, retirer.", t.label()),
                        None => "Cet outil n'existe plus : clic droit pour retirer ce bouton.".to_string(),
                    },
                    allume,
                    actif: tool.is_some(),
                    pastille: Some(o.couleur),
                    outil_perso: true,
                    ..vide(false)
                }
            }
            Bouton::Separateur | Bouton::Inconnu => vide(true),
        }
    }

    /// Le panneau : « Enregistrer l'outil actuel ici » et « Afficher les noms » en haut, la grille des boutons, puis la saisie d'un
    /// nom et la recherche du « + » quand elles sont ouvertes.
    pub(crate) fn mon_panneau(&mut self, ui: &mut Ui) {
        let mut enregistrer = false;
        let mut noms = self.settings.mon_panneau.noms;
        ui.horizontal_wrapped(|ui| {
            let r = ui.button(format!("{}  Enregistrer l'outil actuel ici", ph::FLOPPY_DISK_BACK));
            enregistrer = bulles::bulle(
                r,
                "Enregistrer l'outil actuel ici",
                "Garde l'outil courant, les réglages de sa brosse et la couleur dans un bouton : un clic sur ce bouton remet les trois d'un coup.",
                "",
            )
            .clicked();
            bulles::interrupteur(ui, &mut noms, ph::TEXT_AA, "Afficher les noms", "Le nom sous chaque icône ; éteint : les icônes seules.", "");
        });
        if enregistrer {
            self.mon_panneau_enregistrer_outil();
        }
        if noms != self.settings.mon_panneau.noms {
            self.settings.mon_panneau.noms = noms;
            self.mon_panneau_sauver();
        }
        ui.add_space(4.0);

        let n = self.settings.mon_panneau.boutons.len();
        let vues: Vec<Vue> = self.settings.mon_panneau.boutons.iter().map(|b| self.mon_panneau_vue(b)).collect();
        let case = if noms { CASE_NOM } else { CASE };
        let mut largeurs: Vec<f32> = vues.iter().map(|v| if v.separateur { LARGEUR_SEPARATEUR } else { case.x }).collect();
        let mut separateurs: Vec<bool> = vues.iter().map(|v| v.separateur).collect();
        // Le « + » est la dernière case de la grille.
        largeurs.push(case.x);
        separateurs.push(false);
        let place = ui.available_width();
        let (cases, hauteur) = disposer(&largeurs, &separateurs, case.y, ECART, place);
        let (zone, _) = ui.allocate_exact_size(vec2(place, hauteur.max(case.y)), Sense::hover());
        let rects: Vec<Rect> = cases.iter().map(|r| r.translate(zone.min.to_vec2())).collect();

        let mut acts: Vec<Act> = Vec::new();
        let mut cible: Option<usize> = None;
        let pointeur = ui.input(|i| i.pointer.interact_pos());
        if self.mon_panneau.glisse.is_some_and(|g| g >= n) {
            self.mon_panneau.glisse = None;
        }
        let base = ui.id().with("mon_panneau_case");
        for (i, v) in vues.iter().enumerate() {
            let rect = rects[i];
            if rect.width() <= 0.0 {
                continue;
            }
            let resp = ui.interact(rect, base.with(i), Sense::click_and_drag());
            if resp.drag_started() {
                self.mon_panneau.glisse = Some(i);
            }
            let glisse = self.mon_panneau.glisse == Some(i);
            if glisse {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                if let Some(c) = pointeur.and_then(|p| case_la_plus_proche(&rects[..n], p)) {
                    cible = Some(c);
                }
                if resp.drag_stopped() {
                    if let Some(c) = cible {
                        acts.push(Act::Deplacer(i, c));
                    }
                    self.mon_panneau.glisse = None;
                }
            }
            if v.separateur {
                let x = rect.center().x;
                ui.painter().line_segment([pos2(x, rect.top() + 5.0), pos2(x, rect.bottom() - 5.0)], Stroke::new(1.5_f32, crate::theme::bord()));
                let resp = bulles::bulle(resp, "Séparateur", "Un trait entre deux groupes de boutons. Glisser pour le déplacer ; clic droit : le retirer.", "");
                resp.context_menu(|ui| {
                    if ui.button(format!("{}  Retirer", ph::X)).clicked() {
                        acts.push(Act::Retirer(i));
                        ui.close_menu();
                    }
                });
                continue;
            }
            dessiner_case(ui, rect, &resp, v, noms, glisse);
            let resp = bulles::bulle(resp, &v.nom, &v.description, &v.raccourci);
            if resp.clicked() && v.actif {
                acts.push(Act::Clic(i));
            }
            resp.context_menu(|ui| {
                if let Some(action) = &v.commande {
                    let ligne = if v.raccourci.is_empty() { "aucun raccourci".to_string() } else { format!("raccourci : {}", v.raccourci) };
                    ui.label(RichText::new(ligne).weak());
                    if ui.button(format!("{}  Assigner un raccourci…", ph::KEYBOARD)).clicked() {
                        acts.push(Act::Raccourci(action.clone()));
                        ui.close_menu();
                    }
                }
                if v.outil_perso && ui.button(format!("{}  Renommer…", ph::PENCIL_SIMPLE_LINE)).clicked() {
                    acts.push(Act::Renommer(i));
                    ui.close_menu();
                }
                if ui.button(format!("{}  Ajouter un séparateur", ph::DOTS_SIX_VERTICAL)).clicked() {
                    acts.push(Act::Separateur(i));
                    ui.close_menu();
                }
                if ui.button(format!("{}  Retirer", ph::X)).clicked() {
                    acts.push(Act::Retirer(i));
                    ui.close_menu();
                }
            });
        }
        // Le « + ».
        {
            let rect = rects[n];
            let resp = ui.interact(rect, base.with(n), Sense::click());
            let ouverte = self.mon_panneau.recherche.is_some();
            let plus = Vue {
                icone: ph::PLUS.to_string(),
                nom: "Ajouter une commande".into(),
                description: "Chercher une commande par son nom, comme Ctrl+K. Clic droit sur n'importe quelle icône de FEANOR : « Ajouter à Mon panneau ».".into(),
                raccourci: String::new(),
                allume: ouverte,
                actif: true,
                pastille: None,
                separateur: false,
                commande: None,
                outil_perso: false,
            };
            dessiner_case(ui, rect, &resp, &plus, false, false);
            if bulles::bulle(resp, &plus.nom, &plus.description, "").clicked() {
                acts.push(Act::Plus);
            }
        }
        // La case où le bouton glissé se posera.
        if let (Some(_), Some(c)) = (self.mon_panneau.glisse, cible) {
            ui.painter().rect_stroke(rects[c].shrink(1.0), 4.0, Stroke::new(2.0_f32, crate::theme::accent_texte()), StrokeKind::Inside);
        }
        if self.mon_panneau.glisse.is_some() && !ui.input(|i| i.pointer.any_down()) {
            self.mon_panneau.glisse = None;
        }
        if n == 0 {
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Rien ici pour l'instant. « Enregistrer l'outil actuel ici » garde l'outil, sa brosse et sa couleur. Clic droit sur une icône de FEANOR : \
                     « Ajouter à Mon panneau ». Le + cherche une commande.",
                )
                .small()
                .weak(),
            );
        }

        if let Some(act) = acts.into_iter().next() {
            self.mon_panneau_agir(ui.ctx(), act);
        }
        self.mon_panneau_nom_ui(ui);
        self.mon_panneau_recherche_ui(ui);
    }

    /// Fait ce que demande un geste sur la grille.
    fn mon_panneau_agir(&mut self, ctx: &Context, act: Act) {
        match act {
            Act::Clic(i) => self.mon_panneau_clic(i),
            Act::Retirer(i) => {
                if self.settings.mon_panneau.retirer(i).is_some() {
                    // (Un nom en cours de saisie vise un rang : il ne vise plus rien.)
                    self.mon_panneau.renommer = None;
                    self.mon_panneau_sauver();
                }
            }
            Act::Renommer(i) => {
                if let Some(Bouton::Outil(o)) = self.settings.mon_panneau.boutons.get(i) {
                    self.mon_panneau.renommer = Some((i, o.nom.clone()));
                    self.mon_panneau.renommer_pris = false;
                }
            }
            Act::Separateur(i) => {
                if self.settings.mon_panneau.separateur_apres(i).is_some() {
                    self.mon_panneau.renommer = None;
                    self.mon_panneau_sauver();
                } else {
                    self.status = "Un séparateur va entre deux boutons.".into();
                }
            }
            Act::Deplacer(de, vers) => {
                if self.settings.mon_panneau.deplacer(de, vers) {
                    self.mon_panneau.renommer = None;
                    self.mon_panneau_sauver();
                }
            }
            Act::Plus => {
                if self.mon_panneau.recherche.is_some() {
                    self.mon_panneau.recherche = None;
                } else {
                    self.mon_panneau.recherche = Some(String::new());
                    self.mon_panneau.recherche_prise = false;
                }
            }
            Act::Raccourci(action) => crate::raccourcisui::ask_assign(ctx, &action),
        }
    }

    /// La saisie du nom d'un outil perso, sous la grille.
    fn mon_panneau_nom_ui(&mut self, ui: &mut Ui) {
        let Some((i, mut nom)) = self.mon_panneau.renommer.take() else { return };
        let mut fini: Option<bool> = None;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Nom");
            let r = ui.add(egui::TextEdit::singleline(&mut nom).desired_width(crate::dockui::fit_w(ui, 150.0)));
            if !self.mon_panneau.renommer_pris {
                r.request_focus();
                self.mon_panneau.renommer_pris = true;
            }
            let entree = r.lost_focus() && ui.input(|inp| inp.key_pressed(egui::Key::Enter));
            if ui.button("OK").clicked() || entree {
                fini = Some(true);
            }
            if ui.button("Annuler").clicked() || ui.input(|inp| inp.key_pressed(egui::Key::Escape)) {
                fini = Some(false);
            }
        });
        match fini {
            Some(true) => {
                if self.settings.mon_panneau.renommer(i, &nom) {
                    self.mon_panneau_sauver();
                }
            }
            Some(false) => {}
            None => self.mon_panneau.renommer = Some((i, nom)),
        }
    }

    /// La recherche du « + » : une commande par son nom (ou sa fin d'identifiant), comme Ctrl+K, et les brosses de « Mes brosses ».
    fn mon_panneau_recherche_ui(&mut self, ui: &mut Ui) {
        let Some(mut q) = self.mon_panneau.recherche.take() else { return };
        let mut garder = true;
        let mut ajout_commande: Option<&'static str> = None;
        let mut ajout_brosse: Option<usize> = None;
        ui.add_space(6.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(ph::MAGNIFYING_GLASS);
            let r = ui.add(egui::TextEdit::singleline(&mut q).hint_text("Une commande : persp, retourner, flou…").desired_width(crate::dockui::fit_w(ui, 240.0)));
            if !self.mon_panneau.recherche_prise {
                r.request_focus();
                self.mon_panneau.recherche_prise = true;
            }
            if bulles::icone(ui, ph::X, "Fermer la recherche", "Échap fait de même.", "").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                garder = false;
            }
        });
        if !q.trim().is_empty() {
            let commandes = crate::palette::palette_commands();
            let trouvees = crate::palette::rank(&commandes, &q);
            for &k in trouvees.iter().take(RESULTATS) {
                let (id, nom) = commandes[k];
                let i = infos(&self.ctx, id);
                let deja = self.settings.mon_panneau.contient_commande(id);
                let cle = self.raccourci(id);
                let texte = if cle.is_empty() { format!("{}  {}", i.icone, nom) } else { format!("{}  {}   ({})", i.icone, nom, cle) };
                let r = ui.add_enabled(!deja, egui::SelectableLabel::new(false, texte));
                if r.clicked() {
                    ajout_commande = Some(id);
                }
            }
            let mot = crate::pinceaux::plie(q.trim());
            let brosses: Vec<(usize, String, bool)> = self
                .brushes
                .presets
                .iter()
                .enumerate()
                .filter(|(_, p)| p.source == MINE && crate::pinceaux::plie(&p.name).contains(&mot))
                .take(RESULTATS_BROSSES)
                .map(|(i, p)| (i, p.name.clone(), p.target == Target::Eraser))
                .collect();
            for (i, nom, gomme) in brosses {
                let p = &self.brushes.presets[i];
                let deja = self.settings.mon_panneau.contient_brosse(&p.name, &p.source, gomme);
                let texte = format!("{}  {}   (Mes brosses)", if gomme { ph::ERASER } else { ph::PAINT_BRUSH }, nom);
                if ui.add_enabled(!deja, egui::SelectableLabel::new(false, texte)).clicked() {
                    ajout_brosse = Some(i);
                }
            }
            if trouvees.len() > RESULTATS {
                ui.label(RichText::new(format!("… et {} autres : précise ta recherche.", trouvees.len() - RESULTATS)).small().weak());
            } else if trouvees.is_empty() {
                ui.label(RichText::new("Aucune commande ne correspond.").small().weak());
            }
        }
        if let Some(id) = ajout_commande {
            self.mon_panneau_ajouter_commande(id);
            garder = false;
        }
        if let Some(i) = ajout_brosse {
            self.mon_panneau_ajouter_brosse(i);
            garder = false;
        }
        if garder {
            self.mon_panneau.recherche = Some(q);
        }
    }
}

/// Dessine la case d'un bouton : son fond (allumé ou survolé), l'icône, la pastille de couleur d'un outil perso, le nom dessous.
fn dessiner_case(ui: &Ui, rect: Rect, resp: &egui::Response, v: &Vue, noms: bool, fantome: bool) {
    let visuals = ui.visuals();
    let fond = if v.allume {
        crate::theme::accent()
    } else if resp.hovered() && v.actif {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        Color32::TRANSPARENT
    };
    let mut encre = if v.allume { crate::theme::sur_accent() } else { visuals.text_color() };
    if !v.actif {
        encre = visuals.weak_text_color();
    }
    if fantome {
        encre = encre.gamma_multiply(0.45);
    }
    let peintre = ui.painter();
    peintre.rect_filled(rect, 4.0, fond);
    let centre = if noms { pos2(rect.center().x, rect.top() + rect.height() * 0.38) } else { rect.center() };
    peintre.text(centre, Align2::CENTER_CENTER, &v.icone, FontId::proportional(ICONE), encre);
    if let Some([r, g, b]) = v.pastille {
        let p = centre + vec2(ICONE * 0.5, ICONE * 0.5);
        peintre.circle(p, 4.5, Color32::from_rgb(r, g, b), Stroke::new(1.2_f32, visuals.text_color()));
    }
    if noms && !v.nom.is_empty() {
        let mut job = egui::text::LayoutJob::simple_singleline(v.nom.clone(), FontId::proportional(10.5), encre);
        job.wrap.max_width = rect.width() - 4.0;
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.wrap.overflow_character = Some('…');
        let galerie = peintre.layout_job(job);
        let pos = pos2(rect.center().x - galerie.size().x * 0.5, rect.bottom() - galerie.size().y - 3.0);
        peintre.galley(pos, galerie, encre);
    }
}
