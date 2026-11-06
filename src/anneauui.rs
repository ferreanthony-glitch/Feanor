//! L'anneau sous le stylet, côté application (2026-11-06) : lire le bouton du stylet, dessiner l'anneau, lancer les commandes,
//! et l'onglet « Anneau » des Préférences. La logique du geste est dans `anneau.rs`, sans interface.
//!
//! Ce qu'Anthony a décidé après l'essai de la maquette :
//! - le bouton du stylet (le clic droit) sur le dessin déclenche l'anneau ; le clic bref garde ce qu'il faisait (la fenêtre des
//!   réglages de l'outil, `reglagesdroit.rs`) ;
//! - les huit commandes se choisissent dans les Préférences, dans un onglet à part, avec le délai d'affichage ;
//! - un clic droit sur un outil (ou sur toute icône de commande) propose de l'envoyer dans une des huit directions ;
//! - les pastilles sont un second anneau, autour du premier.
//!
//! Où il joue : sur le dessin du Storyboard et de l'Animation (le clic bref y ouvre les réglages) et sur la planche des
//! Vignettes (le clic bref y garde le menu de la case). Pas dans l'Espace 3D, où le clic droit tourne la caméra.
//!
//! **Sans latence.** À l'appui, les icônes, les noms et les pastilles sont relevés une fois ; pendant le geste, chaque image ne
//! fait que comparer un angle et peindre quelques formes. Rien n'est demandé au moteur de dessin. Au repos l'anneau ne demande
//! aucune repeinture : un seul réveil est programmé, celui du délai d'affichage (puis celui qui efface le nom de la commande).
//!
//! **Annulation.** L'anneau ne crée aucune étape à lui : la commande lancée fait la sienne, comme par sa touche.

use std::sync::Arc as Partage;
use std::time::Duration;

use eframe::egui::{self, vec2, Align2, Color32, Event, FontId, Id, Key, LayerId, Modifiers, Order, Painter, PointerButton, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, TouchPhase, Vec2};
use egui_phosphor::regular as ph;

use crate::anneau::{self, Arc, Geste, Issue, PARTS};
use crate::app::App;
use crate::personnagesui::Choix;

/// Combien de temps le nom de la commande lancée reste près de la pointe, en secondes.
const ECHO: f64 = 0.7;
/// Hauteur de l'aperçu des Préférences, en points : le premier anneau et ses noms, et le second anneau dans n'importe quelle
/// direction (ses noms sont ramenés dans l'aperçu), sans que l'onglet ait à défiler dans la fenêtre d'origine.
const APERCU_H: f32 = 300.0;
/// Largeur d'une liste de l'onglet des Préférences.
const LISTE_L: f32 = 176.0;

/// Clés des données d'egui par lesquelles un menu dessiné sans accès à l'application parle à l'anneau.
const DEMANDE: &str = "feanor_anneau_demande";
const PUBLIE: &str = "feanor_anneau_commandes";

/// Les huit commandes telles que les menus du clic droit les lisent : l'action et son nom.
type Publie = Partage<[(String, String); PARTS]>;

/// Une part telle qu'on la dessine : l'icône et le nom de sa commande.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Part {
    pub icone: String,
    pub nom: String,
}

/// Une pastille du second anneau telle qu'on la dessine.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pastille {
    pub choix: Choix,
    pub nom: String,
    pub ligne: [u8; 3],
    pub fond: Option<[u8; 3]>,
}

/// Où est un dessin à l'écran pendant cette image (le dessin à plat, ou la planche des Vignettes) : seul un appui qui tombe
/// dessus ouvre l'anneau.
#[derive(Clone, Copy, Debug)]
struct Dessin {
    rect: Rect,
    visible: Rect,
    couche: LayerId,
    /// Le clic bref ouvre-t-il les réglages de l'outil ? (Sur la planche, non : egui y ouvre le menu de la case.)
    reglages: bool,
}

impl Dessin {
    fn contient(&self, ctx: &egui::Context, pos: Pos2) -> bool {
        self.rect.contains(pos) && self.visible.contains(pos) && ctx.layer_id_at(pos) == Some(self.couche)
    }
}

/// Le nom de la commande qui vient de partir, montré un instant près de la pointe.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Echo {
    pub texte: String,
    pub ou: Pos2,
    pub fin: f64,
}

/// Ce que l'application garde de l'anneau d'une image à l'autre.
#[derive(Default)]
pub(crate) struct Etat {
    /// Le geste en cours, de l'appui au relâché (ou tant que le second anneau reste ouvert).
    pub geste: Option<Geste>,
    /// Les huit parts, relevées à l'appui.
    pub parts: [Part; PARTS],
    /// Les pastilles du second anneau, relevées à l'appui : la liste ne change pas sous la main.
    pub pastilles: Vec<Pastille>,
    /// Le nom de la dernière commande lancée.
    pub echo: Option<Echo>,
    /// Les dessins à l'écran pendant cette image : le dessin à plat (`canvas_panel`) et la planche des Vignettes (`boardui.rs`).
    dessins: [Option<Dessin>; 2],
    /// Le geste en cours est parti d'un dessin où le clic bref ouvre les réglages de l'outil.
    clic_reglages: bool,
    /// Préférences : la part dont la liste est survolée ou ouverte, mise en avant dans l'aperçu à l'image suivante.
    apercu_survol: Option<usize>,
}

/// La flèche d'une direction (une icône : la police d'egui n'a pas toutes les flèches).
pub(crate) fn fleche(part: usize) -> &'static str {
    [ph::ARROW_RIGHT, ph::ARROW_DOWN_RIGHT, ph::ARROW_DOWN, ph::ARROW_DOWN_LEFT, ph::ARROW_LEFT, ph::ARROW_UP_LEFT, ph::ARROW_UP, ph::ARROW_UP_RIGHT][part % PARTS]
}

/// L'icône d'une commande que les barres ne montrent nulle part.
fn icone_propre(action: &str) -> Option<&'static str> {
    Some(match action {
        "edition.annuler" => ph::ARROW_COUNTER_CLOCKWISE,
        "edition.retablir" => ph::ARROW_CLOCKWISE,
        "panneau.precedent" => ph::CARET_LEFT,
        "panneau.suivant" => ph::CARET_RIGHT,
        "panneau.nouveau" => ph::PLUS_SQUARE,
        "panneau.dupliquer" => ph::COPY,
        "image.precedente" => ph::CARET_DOUBLE_LEFT,
        "image.suivante" => ph::CARET_DOUBLE_RIGHT,
        "cle.precedente" => ph::SKIP_BACK,
        "cle.suivante" => ph::SKIP_FORWARD,
        "lecture.basculer" => ph::PLAY,
        "vue.miroir" => ph::FLIP_HORIZONTAL,
        "vue.ajuster" => ph::CORNERS_OUT,
        "vue.table_lumineuse" => ph::LIGHTBULB,
        "vue.concentration" => ph::FRAME_CORNERS,
        "mode.basculer" => ph::FILM_STRIP,
        _ => return None,
    })
}

/// Un nom assez court pour une étiquette de l'anneau : le début du nom du registre, sans sa parenthèse ni sa précision.
pub(crate) fn nom_court(nom: &str) -> String {
    let mut court = nom;
    for coupe in [" (", " : ", ", "] {
        if let Some(i) = court.find(coupe) {
            court = &court[..i];
        }
    }
    if court.chars().count() > 30 {
        let debut: String = court.chars().take(29).collect();
        return format!("{}…", debut.trim_end());
    }
    court.to_string()
}

/// Demande de poser `action` dans la part (lue à l'image suivante, `App::anneau_suivre`).
pub(crate) fn demander(ctx: &egui::Context, action: &str, part: usize) {
    ctx.data_mut(|d| d.insert_temp(Id::new(DEMANDE), (action.to_string(), part)));
}

/// L'entrée « Envoyer dans l'anneau » du clic droit d'une commande (`raccourcisui::shortcut_menu`) : les huit directions, chacune
/// avec la commande qu'elle porte. Absente quand l'anneau est éteint, ou pour une commande qui n'est pas au registre.
pub(crate) fn menu_envoyer(ui: &mut egui::Ui, action: &str) {
    if !crate::monpanneau::commande_connue(action) {
        return;
    }
    let Some(commandes) = ui.ctx().data(|d| d.get_temp::<Publie>(Id::new(PUBLIE))) else { return };
    ui.menu_button(format!("{}  Envoyer dans l'anneau", ph::COMPASS), |ui| {
        ui.label(RichText::new("Elle prend la place de la commande de cette direction.").small().weak());
        for part in anneau::GRILLE {
            let (actuelle, nom) = &commandes[part];
            let coche = if actuelle == action { format!("  {}", ph::CHECK) } else { String::new() };
            if ui.button(format!("{}  {} : {nom}{coche}", fleche(part), anneau::NOMS_DIRECTIONS[part])).clicked() {
                demander(ui.ctx(), action, part);
                ui.close_menu();
            }
        }
    });
}

impl App {
    // ---------------------------------------------------------------------------------------------- ce que l'anneau montre

    /// L'icône et le nom d'une commande dans l'anneau : ceux qu'elle a ailleurs dans FEANOR (`monpanneauui::infos`).
    pub(crate) fn anneau_part(&self, action: &str) -> Part {
        if action == anneau::PASTILLES {
            return Part { icone: ph::CIRCLES_THREE.into(), nom: "Pastilles".into() };
        }
        let infos = crate::monpanneauui::infos(&self.ctx, action);
        let icone = if infos.icone == ph::LIGHTNING { icone_propre(action).unwrap_or(ph::LIGHTNING).to_string() } else { infos.icone };
        Part { icone, nom: nom_court(&infos.nom) }
    }

    fn anneau_parts(&self) -> [Part; PARTS] {
        std::array::from_fn(|part| self.anneau_part(&self.settings.anneau.commandes[part]))
    }

    /// Les pastilles du second anneau : celles de la barre, dans le même ordre (personnages, classiques, décors là où le décor
    /// est proposé).
    fn anneau_liste_pastilles(&self) -> Vec<Pastille> {
        self.perso_tous()
            .into_iter()
            .filter_map(|choix| {
                let (ligne, fond) = self.perso_couleurs(choix)?;
                Some(Pastille { choix, nom: self.perso_nom(choix).unwrap_or_default(), ligne, fond })
            })
            .collect()
    }

    /// La part qui déploie le second anneau et le nombre de pastilles ; rien si aucune part ne porte les pastilles ou s'il n'y
    /// en a aucune.
    fn anneau_part_des_pastilles(&self, nombre: usize) -> Option<(usize, usize)> {
        let part = self.settings.anneau.commandes.iter().position(|c| c == anneau::PASTILLES)?;
        (nombre > 0).then_some((part, nombre))
    }

    // ------------------------------------------------------------------------------------------------------- les entrées

    /// L'anneau est-il en service ? (Préférences > Anneau.)
    pub(crate) fn anneau_en_service(&self) -> bool {
        self.settings.anneau.actif
    }

    /// Un dessin est à l'écran, ici, pendant cette image : le dessin à plat (`canvas_panel`, `reglages` : son clic bref ouvre les
    /// réglages de l'outil) ou la planche des Vignettes (`boardui.rs` : son clic bref garde le menu de la case).
    pub(crate) fn anneau_noter_le_dessin(&mut self, response: &egui::Response, rect: Rect, reglages: bool) {
        let dessin = Dessin { rect, visible: response.interact_rect, couche: response.layer_id, reglages };
        if let Some(place) = self.anneau.dessins.iter_mut().find(|d| d.is_none()) {
            *place = Some(dessin);
        }
    }

    /// Échap abandonne le geste et ne fait rien d'autre : à appeler avant les raccourcis.
    pub(crate) fn anneau_echap(&mut self, ctx: &egui::Context) {
        if self.anneau.geste.is_some() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.anneau.geste = None;
        }
    }

    /// À chaque image, avant les panneaux : les huit commandes sont dites aux menus du clic droit, et une commande envoyée
    /// depuis l'un d'eux est posée.
    pub(crate) fn anneau_suivre(&mut self, ctx: &egui::Context) {
        if !self.settings.anneau.actif {
            ctx.data_mut(|d| {
                d.remove::<Publie>(Id::new(PUBLIE));
                d.remove::<(String, usize)>(Id::new(DEMANDE));
            });
            return;
        }
        if ctx.data(|d| d.get_temp::<Publie>(Id::new(PUBLIE))).is_none() {
            self.anneau_publier(ctx);
        }
        if let Some((action, part)) = ctx.data_mut(|d| d.remove_temp::<(String, usize)>(Id::new(DEMANDE))) {
            self.anneau_poser(part, &action);
        }
    }

    fn anneau_publier(&self, ctx: &egui::Context) {
        let parts = self.anneau_parts();
        let commandes: Publie = Partage::new(std::array::from_fn(|part| (self.settings.anneau.commandes[part].clone(), parts[part].nom.clone())));
        ctx.data_mut(|d| d.insert_temp(Id::new(PUBLIE), commandes));
    }

    /// Pose une commande dans une part (Préférences, ou clic droit sur une icône). Si elle était dans une autre part, les deux
    /// échangent leur place. Gardé dans les réglages du poste.
    pub(crate) fn anneau_poser(&mut self, part: usize, action: &str) {
        if part >= PARTS || !(action == anneau::PASTILLES || crate::monpanneau::commande_connue(action)) {
            self.status = "Cette commande ne peut pas aller dans l'anneau.".into();
            return;
        }
        let nom = self.anneau_part(action).nom;
        let place = anneau::NOMS_DIRECTIONS[part].to_lowercase();
        if self.settings.anneau.commandes[part] == action {
            self.status = format!("Anneau : « {nom} » est déjà à la place « {place} ».");
            return;
        }
        let remplacee = self.anneau_part(&self.settings.anneau.commandes[part]).nom;
        let echange = anneau::poser(&mut self.settings.anneau.commandes, part, action);
        self.settings_a_ecrire = true;
        self.anneau.geste = None;
        let ctx = self.ctx.clone();
        self.anneau_publier(&ctx);
        self.status = match echange {
            Some(autre) => format!("Anneau : « {nom} » passe à la place « {place} », « {remplacee} » à la place « {} ».", anneau::NOMS_DIRECTIONS[autre].to_lowercase()),
            None => format!("Anneau : « {nom} » remplace « {remplacee} » à la place « {place} »."),
        };
    }

    /// Un appui du bouton du stylet peut-il ouvrir l'anneau ? Pas pendant un trait.
    fn anneau_peut_s_ouvrir(&self) -> bool {
        self.stroke.is_none() && self.erase_pen().is_none()
    }

    fn anneau_ouvrir(&mut self, centre: Pos2, instant: f64, clic_reglages: bool) {
        self.anneau.clic_reglages = clic_reglages;
        self.anneau.parts = self.anneau_parts();
        self.anneau.pastilles = self.anneau_liste_pastilles();
        self.anneau.echo = None;
        self.anneau.geste = Some(Geste::nouveau(centre, instant));
    }

    fn anneau_bouger(&mut self, pointeur: Pos2) {
        let pastilles = self.anneau_part_des_pastilles(self.anneau.pastilles.len());
        if let Some(geste) = self.anneau.geste.as_mut() {
            geste.bouger(pointeur, pastilles);
        }
    }

    fn anneau_lacher(&mut self, instant: f64) {
        let Some(mut geste) = self.anneau.geste.take() else { return };
        match geste.lacher() {
            // Le clic bref, ou le relâché au centre : ce que le clic droit faisait avant l'anneau. (La Sélection et la Pipette
            // gardent leur menu, et la planche des Vignettes celui de la case : egui les ouvre lui-même sur ce clic droit.)
            Issue::Centre => {
                if self.anneau.clic_reglages && self.reglages_droit_permis() {
                    self.clic_droit_dans_le_dessin(Some(geste.centre));
                }
            }
            Issue::Part(part) => self.anneau_lancer(part, geste.pointeur, instant),
            Issue::Pastille(i) => self.anneau_choisir(i, geste.pointeur, instant),
            Issue::ResteOuvert => self.anneau.geste = Some(geste),
        }
    }

    /// Lance la commande d'une part, comme par sa touche (`run_action` : mêmes garde-fous, même étape d'annulation).
    fn anneau_lancer(&mut self, part: usize, ou: Pos2, instant: f64) {
        let action = self.settings.anneau.commandes[part].clone();
        if action == anneau::PASTILLES {
            self.status = "Anneau : aucune pastille à montrer. Ajoute un personnage ou une brosse dans la barre des pastilles.".into();
            return;
        }
        let texte = self.anneau.parts[part].nom.clone();
        self.run_action(&action);
        self.anneau.echo = Some(Echo { texte, ou, fin: instant + ECHO });
    }

    /// Choisit une pastille du second anneau, comme un clic dans la barre des pastilles. Celle qui dessine déjà le reste.
    fn anneau_choisir(&mut self, i: usize, ou: Pos2, instant: f64) {
        let Some(pastille) = self.anneau.pastilles.get(i).cloned() else { return };
        if self.perso.actif != Some(pastille.choix) {
            self.perso_choisir(pastille.choix);
        }
        self.anneau.echo = Some(Echo { texte: pastille.nom, ou, fin: instant + ECHO });
    }

    /// L'anneau, une fois par image, après les panneaux et juste avant la fenêtre des réglages du clic droit (qu'un clic bref
    /// ouvre) : le bouton du stylet est lu, le geste avance, l'anneau est peint par-dessus tout.
    pub(crate) fn anneau_fenetre(&mut self, ctx: &egui::Context) {
        let dessins = std::mem::take(&mut self.anneau.dessins);
        let maintenant = ctx.input(|i| i.time);
        self.anneau_peindre_echo(ctx, maintenant);
        if !self.settings.anneau.actif || self.modal_open() {
            self.anneau.geste = None;
            return;
        }
        let (boutons, appuis, parti, tenu, pointeur) = ctx.input(|i| {
            // (Deux listes presque toujours vides : aucune allocation hors des images où un bouton change.)
            let mut boutons: Vec<(bool, Pos2)> = Vec::new();
            let mut appuis: Vec<Pos2> = Vec::new();
            let mut parti = false;
            for e in &i.events {
                match e {
                    Event::PointerGone => parti = true,
                    Event::PointerButton { pos, button, pressed, .. } => {
                        if *button == PointerButton::Secondary {
                            boutons.push((*pressed, *pos));
                        }
                        if *pressed {
                            appuis.push(*pos);
                        }
                    }
                    Event::Touch { pos, phase: TouchPhase::Start, .. } => appuis.push(*pos),
                    _ => {}
                }
            }
            (boutons, appuis, parti, i.pointer.button_down(PointerButton::Secondary), i.pointer.latest_pos())
        });

        // Le second anneau resté ouvert : un appui choisit une pastille, ou referme sans rien faire d'autre (le voile peint à
        // l'image d'avant a pris cet appui : ni le dessin ni les panneaux ne l'ont vu).
        if self.anneau.geste.as_ref().is_some_and(|g| g.colle) {
            if let Some(&pos) = appuis.first() {
                let geste = self.anneau.geste.take();
                if let Some(i) = geste.and_then(|g| g.appui(pos)) {
                    self.anneau_choisir(i, pos, maintenant);
                }
                return;
            }
            if let Some(p) = pointeur {
                self.anneau_bouger(p);
            }
            self.anneau_voile(ctx);
            self.anneau_peindre(ctx);
            return;
        }

        let mut appuye = false;
        for (presse, pos) in boutons {
            if presse {
                if self.anneau.geste.is_none() && self.anneau_peut_s_ouvrir() {
                    if let Some(dessin) = dessins.iter().flatten().find(|d| d.contient(ctx, pos)) {
                        self.anneau_ouvrir(pos, maintenant, dessin.reglages);
                        appuye = true;
                    }
                }
            } else if self.anneau.geste.as_ref().is_some_and(|g| !g.colle) {
                self.anneau_bouger(pos);
                self.anneau_lacher(maintenant);
            }
        }
        let Some(colle) = self.anneau.geste.as_ref().map(|g| g.colle) else { return };
        if colle {
            // Relâché sur la part des pastilles pendant cette image : le voile se pose dès maintenant.
            self.anneau_voile(ctx);
            self.anneau_peindre(ctx);
            return;
        }
        if (!tenu && !appuye) || parti {
            // Le relâché n'arrivera pas (le pointeur a quitté la fenêtre, ou elle a perdu la main) : le geste est abandonné,
            // rien n'est lancé. (Comme un trait, que `canvas_input` termine au départ du pointeur.)
            self.anneau.geste = None;
            return;
        }
        if let Some(p) = pointeur {
            self.anneau_bouger(p);
        }
        let delai = self.settings.anneau.delai();
        if let Some(geste) = self.anneau.geste.as_mut() {
            geste.temps(maintenant, delai);
            let attente = geste.attente(maintenant, delai);
            if attente > 0.0 {
                // Le seul réveil que l'anneau demande : celui qui le fait apparaître.
                ctx.request_repaint_after(Duration::from_secs_f64(attente));
            }
        }
        self.anneau_peindre(ctx);
    }

    /// Le voile transparent du second anneau resté ouvert : il prend l'appui suivant, où qu'il tombe (comme la fenêtre des
    /// réglages du clic droit).
    fn anneau_voile(&self, ctx: &egui::Context) {
        let ecran = ctx.screen_rect();
        egui::Area::new(Id::new("anneau_voile")).order(Order::Foreground).fixed_pos(ecran.min).constrain(false).show(ctx, |ui| {
            ui.allocate_rect(ecran, Sense::click_and_drag());
        });
    }

    fn anneau_peindre(&self, ctx: &egui::Context) {
        let Some(geste) = self.anneau.geste.as_ref() else { return };
        let peintre = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("anneau_stylet")));
        peindre(&peintre, geste, &self.anneau.parts, &self.anneau.pastilles, self.perso.actif, false);
    }

    /// Le nom de la commande qui vient de partir, près de la pointe, puis plus rien : un seul réveil, celui qui l'efface.
    fn anneau_peindre_echo(&mut self, ctx: &egui::Context, maintenant: f64) {
        let Some(echo) = self.anneau.echo.as_ref() else { return };
        if maintenant >= echo.fin {
            self.anneau.echo = None;
            return;
        }
        let peintre = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("anneau_echo")));
        etiquette(&peintre, echo.ou - vec2(0.0, 18.0), vec2(-0.5, -1.0), &echo.texte, true, ctx.screen_rect());
        ctx.request_repaint_after(Duration::from_secs_f64(echo.fin - maintenant));
    }

    // --------------------------------------------------------------------------------------------------- les Préférences

    /// Préférences > Anneau > Le geste : l'anneau en service ou non, et le délai avant qu'il se dessine.
    pub(crate) fn anneau_geste_prefs_ui(&mut self, ui: &mut egui::Ui) {
        let resp = ui.checkbox(&mut self.settings.anneau.actif, "Anneau sous le stylet");
        let resp = crate::bulles::bulle(
            resp,
            "Anneau sous le stylet",
            "Le bouton du stylet (le clic droit) sur le dessin. Un clic bref ouvre les réglages de l'outil, comme avant. Appuyer, glisser dans une direction puis relâcher lance la commande de cette direction. Maintenir montre l'anneau. Décoché : le clic droit d'avant, rien de plus.",
            "",
        );
        if resp.changed() {
            self.anneau.geste = None;
            self.settings_a_ecrire = true;
        }
        ui.add_enabled_ui(self.settings.anneau.actif, |ui| {
            ui.horizontal(|ui| {
                ui.label("Délai avant d'afficher l'anneau");
                let mut v = self.settings.anneau.delai_ms;
                let resp = ui.add(egui::Slider::new(&mut v, 0..=anneau::DELAI_MAX).step_by(10.0).suffix(" ms"));
                let resp = crate::bulles::bulle(
                    resp,
                    "Délai avant d'afficher l'anneau",
                    "Le temps pendant lequel il faut maintenir le bouton du stylet avant que l'anneau se dessine. Court : il se montre souvent. Long : il se fait oublier dès que la main connaît les directions. Un geste plus vif que ce délai ne montre rien et lance quand même la commande. 180 ms d'origine.",
                    "",
                );
                if resp.changed() {
                    self.settings.anneau.delai_ms = v.min(anneau::DELAI_MAX);
                    self.settings_a_ecrire = true;
                }
                if ui.small_button("Rétablir").on_hover_text("Revenir au délai d'origine : 180 ms.").clicked() {
                    self.settings.anneau.delai_ms = anneau::DELAI_D_ORIGINE;
                    self.settings_a_ecrire = true;
                }
            });
        });
        ui.label(RichText::new("Clic bref : les réglages de l'outil. Glisser et relâcher : la commande part, sans rien afficher. Maintenir : l'anneau se montre. Échap abandonne le geste.").small().weak());
    }

    /// Préférences > Anneau > Les huit commandes : l'aperçu de l'anneau, puis une liste par direction, rangées en rose des vents.
    pub(crate) fn anneau_commandes_prefs_ui(&mut self, ui: &mut egui::Ui) {
        let parts = self.anneau_parts();
        let pastilles = self.anneau_liste_pastilles();

        // L'aperçu : l'anneau tel qu'il se dessine. La direction dont la liste est survolée s'allume ; celle des pastilles déploie
        // le second anneau.
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width().min(560.0), APERCU_H), Sense::hover());
        let survol = self.anneau.apercu_survol.take();
        let mut geste = Geste::nouveau(rect.center(), 0.0);
        geste.affiche = true;
        geste.survol = survol;
        if let (Some(part), Some((part_pastilles, nombre))) = (survol, self.anneau_part_des_pastilles(pastilles.len())) {
            if part == part_pastilles {
                geste.arc = Arc::nouveau(part, nombre);
            }
        }
        peindre(&ui.painter_at(rect), &geste, &parts, &pastilles, self.perso.actif, true);

        ui.label("Commande de chaque direction");
        // Les commandes proposées, puis celles de l'anneau qui n'en sont pas (envoyées par un clic droit).
        let mut liste: Vec<String> = anneau::PROPOSEES.iter().map(|a| a.to_string()).collect();
        for commande in &self.settings.anneau.commandes {
            if !liste.contains(commande) {
                liste.push(commande.clone());
            }
        }
        let lignes: Vec<(String, String)> = liste.into_iter().map(|action| { let p = self.anneau_part(&action); (action, format!("{}  {}", p.icone, p.nom)) }).collect();
        let commandes = self.settings.anneau.commandes.clone();
        let mut choix: Option<(usize, String)> = None;
        let mut survole: Option<usize> = None;
        ui.add_enabled_ui(self.settings.anneau.actif, |ui| {
            egui::Grid::new("anneau_rose_des_vents").num_columns(3).spacing(vec2(10.0, 8.0)).show(ui, |ui| {
                // Haut gauche, haut, haut droite ; gauche, le centre, droite ; bas gauche, bas, bas droite.
                for (i, cellule) in [Some(5), Some(6), Some(7), Some(4), None, Some(0), Some(3), Some(2), Some(1)].into_iter().enumerate() {
                    match cellule {
                        None => {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(format!("{}  Centre", ph::SLIDERS_HORIZONTAL)).small().weak());
                                ui.label(RichText::new("Les réglages de l'outil").weak());
                            });
                        }
                        Some(part) => {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(format!("{}  {}", fleche(part), anneau::NOMS_DIRECTIONS[part])).small().weak());
                                let ouverte = egui::ComboBox::from_id_salt(("anneau_part", part))
                                    .width(LISTE_L)
                                    .height(420.0)
                                    .selected_text(format!("{}  {}", parts[part].icone, parts[part].nom))
                                    .show_ui(ui, |ui| {
                                        for (action, texte) in &lignes {
                                            if ui.selectable_label(*action == commandes[part], texte).clicked() {
                                                choix = Some((part, action.clone()));
                                            }
                                        }
                                    });
                                if ouverte.response.hovered() || ouverte.inner.is_some() {
                                    survole = Some(part);
                                }
                            });
                        }
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        });
        self.anneau.apercu_survol = survole;
        if let Some((part, action)) = choix {
            self.anneau_poser(part, &action);
        }
        ui.label(RichText::new("Une commande déjà dans l'anneau échange sa place avec celle qu'elle remplace. Toute autre commande : clic droit sur son icône, puis « Envoyer dans l'anneau ».").small().weak());
        if ui.button("Rétablir les commandes d'origine").on_hover_text("Remet les huit commandes de départ : crayon et pinceau en haut, gomme et annuler en bas, plan précédent à gauche, plan suivant et nouveau plan à droite, pastilles en haut à gauche.").clicked() {
            self.settings.anneau.commandes = anneau::commandes_d_origine();
            self.settings_a_ecrire = true;
            let ctx = ui.ctx().clone();
            self.anneau_publier(&ctx);
            self.status = "Anneau : les huit commandes d'origine sont revenues.".into();
        }
    }
}

// ------------------------------------------------------------------------------------------------------------ le dessin

/// Les points d'un arc de cercle, tous les trois degrés au plus.
fn arc(centre: Pos2, rayon: f32, debut: f32, fin: f32) -> Vec<Pos2> {
    let n = (((fin - debut).abs() / 3.0).ceil() as usize).max(2);
    (0..=n).map(|i| anneau::point(centre, rayon, debut + (fin - debut) * i as f32 / n as f32)).collect()
}

/// Une portion d'anneau pleine : un arc tracé d'un trait aussi large que l'anneau (ses bords sont lissés, ses bouts droits).
fn portion(peintre: &Painter, centre: Pos2, dedans: f32, dehors: f32, debut: f32, fin: f32, couleur: Color32) {
    peintre.add(Shape::line(arc(centre, (dedans + dehors) / 2.0, debut, fin), Stroke::new(dehors - dedans, couleur)));
}

/// Une étiquette : le texte dans un petit cadre. `fraction` dit où le cadre s'accroche au point d'ancrage, en parts de sa taille
/// (0, 0 : par son coin haut gauche ; -0,5, -1 : par le milieu de son bord bas). Elle reste dans `limite`.
fn etiquette(peintre: &Painter, ancre: Pos2, fraction: Vec2, texte: &str, vive: bool, limite: Rect) {
    let couleur = if vive { crate::theme::accent_texte() } else { crate::theme::texte() };
    let galley = peintre.layout_no_wrap(texte.to_owned(), FontId::proportional(12.5), couleur);
    let taille = galley.size() + vec2(16.0, 10.0);
    let mut rect = Rect::from_min_size(ancre + vec2(fraction.x * taille.x, fraction.y * taille.y), taille);
    // Ramenée dans la limite (le bord de l'écran, ou celui de l'aperçu).
    let dx = (limite.left() - rect.left()).max(0.0) + (limite.right() - rect.right()).min(0.0);
    let dy = (limite.top() - rect.top()).max(0.0) + (limite.bottom() - rect.bottom()).min(0.0);
    rect = rect.translate(vec2(dx, dy));
    let bord = if vive { crate::theme::accent_texte() } else { crate::theme::bord_fort() };
    peintre.rect(rect, 3.0, crate::theme::fenetre(), Stroke::new(1.0_f32, bord), StrokeKind::Inside);
    peintre.galley(rect.min + vec2(8.0, 5.0), galley, couleur);
}

/// Où le cadre du nom d'une part s'accroche à son point d'ancrage : toujours vers le dehors de l'anneau.
const ACCROCHES: [Vec2; PARTS] = [vec2(0.0, -0.5), vec2(0.0, 0.0), vec2(-0.5, 0.0), vec2(-1.0, 0.0), vec2(-1.0, -0.5), vec2(-1.0, -1.0), vec2(-0.5, -1.0), vec2(0.0, -1.0)];

fn rvb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Dessine l'anneau d'un geste : le trait de l'appui à la pointe, puis, s'il est affiché, le premier anneau (parts, icônes,
/// noms) et le second (bande, pastilles, noms). `apercu` : celui des Préférences, sans le trait ni le rond central allumé.
pub(crate) fn peindre(peintre: &Painter, geste: &Geste, parts: &[Part; PARTS], pastilles: &[Pastille], active: Option<Choix>, apercu: bool) {
    use anneau::*;
    let centre = geste.centre;
    let vif = crate::theme::rehaut();
    let limite = peintre.clip_rect();
    if !apercu && !geste.colle {
        // Le trait du geste : seul retour visible d'un geste fait sans regarder.
        peintre.line_segment([centre, geste.pointeur], Stroke::new(2.0_f32, vif));
        peintre.circle_filled(geste.pointeur, 3.5, vif);
    }
    if !geste.affiche {
        return;
    }
    let fond = crate::theme::fenetre();
    let bord = crate::theme::bord();
    let bord_fort = crate::theme::bord_fort();
    let voile = vif.gamma_multiply(0.22);

    // Le second anneau d'abord : il passe sous le premier là où ils se touchent.
    if let Some(a) = geste.arc.filter(|a| a.nombre == pastilles.len()) {
        let (debut, fin) = (a.centre() - a.demi_ouverture() - 3.0, a.centre() + a.demi_ouverture() + 3.0);
        portion(peintre, centre, R_ARC_DEDANS, R_ARC_DEHORS, debut, fin, fond);
        for rayon in [R_ARC_DEDANS, R_ARC_DEHORS] {
            peintre.add(Shape::line(arc(centre, rayon, debut, fin), Stroke::new(1.0_f32, bord_fort)));
        }
        for bout in [debut, fin] {
            peintre.line_segment([point(centre, R_ARC_DEDANS, bout), point(centre, R_ARC_DEHORS, bout)], Stroke::new(1.0_f32, bord_fort));
        }
        // Une pastille ne déborde jamais sur sa voisine, même nombreuses.
        let rayon = RAYON_PASTILLE.min(a.pas.to_radians() * R_PASTILLE / 2.0 - 1.5).max(3.0);
        for (i, pastille) in pastilles.iter().enumerate() {
            let c = point(centre, R_PASTILLE, a.angle(i));
            let trait_ = (rayon * 0.3).max(2.0);
            // Comme dans la barre : l'aplat au milieu, le trait autour.
            peintre.circle(c, rayon - trait_ / 2.0, pastille.fond.map_or(fond, rvb), Stroke::new(trait_, rvb(pastille.ligne)));
            peintre.circle_stroke(c, rayon + 0.5, Stroke::new(1.0_f32, bord_fort));
            if geste.pastille == Some(i) {
                peintre.circle_stroke(c, rayon + 4.0, Stroke::new(2.5_f32, vif));
            } else if active == Some(pastille.choix) {
                // Celle qui dessine déjà.
                peintre.circle_stroke(c, rayon + 4.0, Stroke::new(1.0_f32, crate::theme::texte()));
            }
        }
        for (i, pastille) in pastilles.iter().enumerate() {
            // Les noms : tous tant qu'ils tiennent, sinon celui de la pastille visée seulement.
            if a.pas < 20.0 && geste.pastille != Some(i) {
                continue;
            }
            let angle = a.angle(i).to_radians();
            let accroche = vec2((angle.cos() - 1.0) / 2.0, (angle.sin() - 1.0) / 2.0);
            etiquette(peintre, point(centre, R_NOM_PASTILLE, a.angle(i)), accroche, &pastille.nom, geste.pastille == Some(i), limite);
        }
    }

    // Le premier anneau.
    peintre.circle_filled(centre + vec2(0.0, 3.0), R_DISQUE + 3.0, Color32::from_black_alpha(70));
    peintre.circle(centre, R_DISQUE, fond, Stroke::new(1.0_f32, bord_fort));
    if let Some(part) = geste.survol {
        portion(peintre, centre, R_PART_DEDANS, R_PART_DEHORS, angle_part(part) - 20.5, angle_part(part) + 20.5, voile);
    }
    for part in 0..PARTS {
        let a = angle_part(part) + 22.5;
        peintre.line_segment([point(centre, R_PART_DEDANS, a), point(centre, R_PART_DEHORS, a)], Stroke::new(1.0_f32, bord));
    }
    let au_centre = geste.survol.is_none() && !apercu && !geste.colle;
    peintre.circle(centre, R_CENTRE, crate::theme::fond(), Stroke::new(if au_centre { 1.6_f32 } else { 1.0 }, if au_centre { vif } else { bord_fort }));
    peintre.text(centre, Align2::CENTER_CENTER, ph::SLIDERS_HORIZONTAL, FontId::proportional(18.0), crate::theme::texte_doux());
    for (part, p) in parts.iter().enumerate() {
        let vise = geste.survol == Some(part);
        let couleur = if vise { crate::theme::accent_texte() } else { crate::theme::texte() };
        peintre.text(point(centre, R_ICONE, angle_part(part)), Align2::CENTER_CENTER, &p.icone, FontId::proportional(21.0), couleur);
        // Le nom d'une part qui tomberait sous le second anneau s'efface le temps du choix.
        if geste.arc.is_some_and(|a| a.couvre(part)) {
            continue;
        }
        etiquette(peintre, point(centre, R_NOM, angle_part(part)), ACCROCHES[part], &p.nom, vise, limite);
    }
}
