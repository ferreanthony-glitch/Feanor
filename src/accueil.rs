//! Écran d'accueil (Anthony, 2026-10-18, sa suggestion retenue : « un écran d'accueil (récents, « Nouveau ») quand aucun
//! projet n'est ouvert »), refait le 2026-10-29 (`docs/INTERFACE.md`, partie 74) : « Voilà ce que je veux en page
//! d'ouverture. Inutile d'afficher le logiciel vide derrière, juste cette fenêtre. »
//!
//! Sans projet ouvert, SHOT n'est plus un logiciel vide avec une carte au milieu : c'est **une fenêtre compacte**
//! (`accueilfenetre.rs`) qui ne contient que cette page.
//! - **En haut**, un bandeau sur toute la largeur : l'image d'accueil tirée au hasard dans le dossier des images d'accueil
//!   (`accueilimages.rs`, réglage de Préférences > Apparence), recadrée pour le remplir, avec le logo de SHOT posé dessus à
//!   gauche (sur un voile sombre qui le garde lisible). Sans image : le logo seul, sur le fond du bandeau. Les images sont
//!   toujours celles de vrais storyboarders, importées par l'utilisateur : SHOT n'en fournit aucune.
//! - **À gauche**, le cadre « Nouveau projet » : le formulaire de `nouveauui.rs` (nom et « Créer », dossier, titre, sous-titre,
//!   format, cadence, « Plus d'options »). Entrée dans le nom crée.
//! - **À droite**, le cadre « Projets récents » : le nom de chaque projet (le chemin entier en bulle), double-clic ou Entrée
//!   pour l'ouvrir, clic droit « Retirer de la liste » ; un projet introuvable est grisé. En bas, « Ouvrir un projet… » (un projet
//!   SHOT ou Storyboard Pro). La proposition de récupération après un plantage est en haut de ce cadre.
//! - **En bas à gauche**, « Rouvrir le dernier projet au lancement ».
//!
//! Le même contenu sert quand un onglet sans projet voisine avec des projets (Ctrl+T) : la page remplit alors la zone de
//! travail, centrée. Ce fichier fabrique aussi le petit logotype (« SHOT » et ses deux angles) tout à gauche de la barre de menus.

use eframe::egui::{self, pos2, vec2, Align, Color32, Layout, Rect, RichText, Sense, Stroke, UiBuilder};
use egui_phosphor::regular as ph;

use crate::accueilimages::Visuel;
use crate::app::{App, Pending};
use crate::logo::{self, Variante, VUE_H, VUE_L};
use crate::nouveauui::{Contexte, NewDocDialog, Sortie};
use crate::recoveryui::RecoveryChoix;
use crate::theme;

/// Hauteur du bandeau, en points.
pub(crate) const BANDEAU_H: f32 = 168.0;

/// Largeur du logo sur le bandeau, en points.
const LOGO_LARGEUR: f32 = 230.0;

/// Marge autour des deux cadres, et entre eux, en points.
const MARGE: f32 = 16.0;

/// Largeur la plus grande du contenu (les deux cadres) quand la fenêtre est plus large que l'accueil compact.
const CONTENU_MAX: f32 = 1100.0;

/// Hauteur de la ligne du bas (la case « Rouvrir le dernier projet »), en points.
const BAS_H: f32 = 40.0;

/// La part de l'image (en coordonnées de texture, 0 à 1) qui remplit une zone de `zone` points sans déformer l'image : le
/// centre si elle est plus large que la zone, le haut un peu favorisé si elle est plus haute (les visages sont en haut).
pub(crate) fn recadrage(pixels: [usize; 2], zone: egui::Vec2) -> Rect {
    let (w, h) = (pixels[0].max(1) as f32, pixels[1].max(1) as f32);
    let (image, cadre) = (w / h, zone.x.max(1.0) / zone.y.max(1.0));
    if image > cadre {
        let u = cadre / image;
        Rect::from_min_max(pos2((1.0 - u) / 2.0, 0.0), pos2((1.0 + u) / 2.0, 1.0))
    } else {
        let v = image / cadre;
        let haut = (1.0 - v) * 0.35;
        Rect::from_min_max(pos2(0.0, haut), pos2(1.0, haut + v))
    }
}

/// Le logo à montrer est-il celui des fonds sombres ? Non quand le fond des panneaux est clair
/// (thème clair : `Palette::claire`, `Visuals::light`).
pub(crate) fn logo_sombre() -> bool {
    !crate::theme::palette().claire
}

/// La texture du logo, à la taille exacte de l'écran (points × pixels par point), dans la variante
/// qui va avec le thème. Refaite seulement si l'un des trois change.
pub(crate) fn logo_texture(ctx: &egui::Context, largeur: f32) -> egui::TextureHandle {
    logo_texture_en(ctx, largeur, logo_sombre())
}

/// La texture du logo dans la variante demandée (`sombre` : lettres claires pour un fond sombre).
pub(crate) fn logo_texture_en(ctx: &egui::Context, largeur: f32, sombre: bool) -> egui::TextureHandle {
    let pixels = (largeur * ctx.pixels_per_point()).round().max(8.0) as u32;
    let id = egui::Id::new(("shot_logo", largeur as u32, sombre));
    if let Some((cle, texture)) = ctx.data(|d| d.get_temp::<(u32, egui::TextureHandle)>(id)) {
        if cle == pixels {
            return texture;
        }
    }
    let variante = if sombre { Variante::Sombre } else { Variante::Clair };
    let (w, h, rgba) = logo::image(pixels, variante);
    let image = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &rgba);
    let texture = ctx.load_texture(format!("shot-logo-{}-{}", largeur as u32, sombre), image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, (pixels, texture.clone())));
    texture
}

/// La texture du petit logotype (le mot « SHOT » et ses deux angles, sans la devise) de la barre de menus, à la taille
/// exacte de l'écran (`hauteur` points × pixels par point). Les angles sont orange (`rehaut`) dans le thème SHOT ; les
/// lettres, de la couleur du texte (blanc cassé ou noir cinéma). Rendu une fois et gardé : refait seulement si la taille
/// en pixels, la variante ou la couleur des angles change.
pub(crate) fn logotype_texture(ctx: &egui::Context, hauteur: f32) -> (egui::TextureHandle, egui::Vec2) {
    let p = theme::palette();
    let pixels = (hauteur * ctx.pixels_per_point()).round().max(8.0) as u32;
    let angles = if p.identite { Some([p.rehaut.r(), p.rehaut.g(), p.rehaut.b()]) } else { None };
    let cle = (pixels, !p.claire, angles);
    let id = egui::Id::new("shot_logotype");
    let taille = |w: u32, h: u32| vec2(w as f32 / ctx.pixels_per_point(), h as f32 / ctx.pixels_per_point());
    if let Some((c, texture, w, h)) = ctx.data(|d| d.get_temp::<((u32, bool, Option<[u8; 3]>), egui::TextureHandle, u32, u32)>(id)) {
        if c == cle {
            return (texture, taille(w, h));
        }
    }
    let variante = if p.claire { Variante::Clair } else { Variante::Sombre };
    let (w, h, rgba) = logo::logotype(pixels, variante, angles);
    let image = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &rgba);
    let texture = ctx.load_texture("shot-logotype", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, (cle, texture.clone(), w, h)));
    (texture, taille(w, h))
}

/// Hauteur du petit logotype de la barre de menus, en points : celle de la barre (un peu moins, pour qu'il y respire).
pub(crate) const LOGOTYPE_HAUTEUR: f32 = 20.0;

/// Le petit logotype tout à gauche de la barre de menus : discret, sans bulle ni clic, suivi d'un petit espace.
pub(crate) fn logotype_barre(ui: &mut egui::Ui) {
    let (texture, taille) = logotype_texture(ui.ctx(), LOGOTYPE_HAUTEUR);
    ui.add(egui::Image::new(egui::load::SizedTexture::new(texture.id(), taille)).sense(Sense::hover()));
    ui.add_space(6.0);
}

/// Un cadre à titre : fond des panneaux, filet, titre en gras, puis `contenu`. Il occupe tout `rect` (les deux cadres de l'accueil
/// ont la même hauteur).
fn cadre(ui: &mut egui::Ui, rect: Rect, titre: &str, contenu: impl FnOnce(&mut egui::Ui)) {
    let p = theme::palette();
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        egui::Frame::new().fill(p.panneau).stroke(Stroke::new(1.0_f32, p.bord)).corner_radius(6.0).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            ui.set_width(rect.width() - 26.0);
            ui.set_min_height(rect.height() - 26.0);
            ui.label(RichText::new(titre).strong().color(p.texte_fort));
            ui.add_space(6.0);
            contenu(ui);
        });
    });
}

impl App {
    /// L'accueil est-il à montrer ? Voir `accueil_visible`.
    pub(crate) fn welcome_shown(&self) -> bool {
        self.accueil_visible()
    }

    /// Une fois par image : quand l'accueil s'en va pour de bon (un projet est là), la prochaine apparition tirera une autre
    /// image et refera le formulaire avec les dernières valeurs retenues.
    pub(crate) fn accueil_suivi(&mut self) {
        if !self.welcome || self.path.is_some() || self.modified {
            self.confort.accueil.oublier_le_tirage();
            self.accueil_etat.form = None;
        }
    }

    /// La page d'accueil : tout ce que la zone de travail montre sans projet ouvert (`frame`, à la place du dock).
    pub(crate) fn accueil_page(&mut self, ctx: &egui::Context) {
        let visuel = self.accueil_visuel(ctx);
        let mut action: Option<&'static str> = None;
        let fond = theme::palette().fond;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(fond)).show(ctx, |ui| {
            let plein = ui.max_rect();
            let bandeau = Rect::from_min_size(plein.min, vec2(plein.width(), BANDEAU_H));
            if self.bandeau(ui, ctx, &visuel, bandeau) {
                action = Some("fichier.preferences");
            }
            // (Les deux cadres : 60 % et 40 % de la largeur, côte à côte, centrés si la fenêtre est large.)
            let largeur = (plein.width() - 2.0 * MARGE).min(CONTENU_MAX);
            let x0 = plein.center().x - largeur / 2.0;
            let haut = plein.top() + BANDEAU_H + MARGE;
            let bas = plein.bottom() - BAS_H;
            let hauteur = (bas - haut).max(260.0);
            let gauche_l = ((largeur - MARGE) * 0.6).round();
            let gauche = Rect::from_min_size(pos2(x0, haut), vec2(gauche_l, hauteur));
            let droite = Rect::from_min_size(pos2(x0 + gauche_l + MARGE, haut), vec2(largeur - gauche_l - MARGE, hauteur));
            self.cadre_nouveau(ui, gauche);
            if let Some(a) = self.cadre_recents(ui, ctx, droite) {
                action = Some(a);
            }
            let case = Rect::from_min_size(pos2(x0, bas + 8.0), vec2(largeur, 24.0));
            ui.scope_builder(UiBuilder::new().max_rect(case), |ui| {
                if ui.checkbox(&mut self.settings.reopen_last, "Rouvrir le dernier projet au lancement").changed() {
                    let _ = self.settings.save();
                }
            });
        });
        if let Some(a) = action {
            self.run_action(a);
        }
    }

    /// Le bandeau : l'image recadrée, le voile, le logo, le bouton des Préférences. Vrai si on a cliqué sur ce bouton.
    fn bandeau(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, visuel: &Visuel, rect: Rect) -> bool {
        let p = theme::palette();
        let peintre = ui.painter_at(rect);
        peintre.rect_filled(rect, 0.0, p.entete);
        let sur_image = matches!(visuel, Visuel::Image(..));
        if let Visuel::Image(texture, taille) = visuel {
            peintre.image(texture.id(), rect, recadrage(*taille, rect.size()), Color32::WHITE);
            // Un voile sombre à gauche, qui s'éteint vers le milieu : le logo se lit sur n'importe quel dessin.
            let mut voile = egui::Mesh::default();
            let (noir, rien) = (Color32::from_black_alpha(205), Color32::TRANSPARENT);
            let fin = rect.left() + rect.width() * 0.55;
            voile.colored_vertex(rect.left_top(), noir);
            voile.colored_vertex(pos2(fin, rect.top()), rien);
            voile.colored_vertex(pos2(fin, rect.bottom()), rien);
            voile.colored_vertex(rect.left_bottom(), noir);
            voile.add_triangle(0, 1, 2);
            voile.add_triangle(0, 2, 3);
            peintre.add(egui::Shape::mesh(voile));
        }
        // Le logo : lettres claires sur l'image (voile sombre), de la variante du thème sur le fond du bandeau.
        let texture = logo_texture_en(ctx, LOGO_LARGEUR, if sur_image { true } else { logo_sombre() });
        let hauteur = LOGO_LARGEUR * VUE_H / VUE_L;
        let place = Rect::from_min_size(pos2(rect.left() + 28.0, rect.center().y - hauteur / 2.0), vec2(LOGO_LARGEUR, hauteur));
        peintre.image(texture.id(), place, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        // Les Préférences (c'est là qu'on choisit les images d'accueil) : un petit bouton en haut à droite.
        let bouton = Rect::from_min_size(rect.right_top() + vec2(-44.0, 10.0), vec2(34.0, 30.0));
        peintre.rect_filled(bouton, 6.0, Color32::from_black_alpha(120));
        ui.scope_builder(UiBuilder::new().max_rect(bouton), |ui| {
            let resp = crate::bulles::icone(ui, ph::GEAR, "Préférences", "Les réglages de SHOT, dont les images d'accueil (Apparence).", "");
            resp.clicked()
        })
        .inner
    }

    /// Le cadre « Nouveau projet » : le formulaire partagé avec la fenêtre de Fichier > Nouveau.
    fn cadre_nouveau(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let mut form = self.accueil_etat.form.take().unwrap_or_else(|| NewDocDialog::open(&self.settings.new_project));
        if std::mem::take(&mut self.accueil_etat.focus_nom) {
            form.focus_nom = true;
        }
        let user = self.settings.new_project.presets.clone();
        let dossiers = self.dossiers_du_formulaire(&form.folder);
        let mut sortie = Sortie::default();
        cadre(ui, rect, "Nouveau projet", |ui| {
            egui::ScrollArea::vertical().id_salt("accueil_formulaire").auto_shrink([false, false]).show(ui, |ui| {
                let cx = Contexte { prereglages: &user, dossiers: &dossiers, nom_requis: true, options_ouvertes: false };
                sortie = form.formulaire(ui, &cx);
            });
        });
        self.appliquer_prereglages(&mut sortie);
        if sortie.creer && form.peut_creer(true) && self.create_project(&form) {
            // (Le projet est ouvert : le formulaire est refait, avec les valeurs retenues, à la prochaine apparition de l'accueil.)
            return;
        }
        self.accueil_etat.form = Some(form);
    }

    /// Le cadre « Projets récents », avec la récupération après plantage en haut s'il y en a une. Rend la commande demandée
    /// (« Ouvrir un projet… »).
    fn cadre_recents(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, rect: Rect) -> Option<&'static str> {
        let recents: Vec<std::path::PathBuf> = self.settings.recent_projects.clone();
        self.recent_presence.refresh(ctx, &recents);
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let mut ouvrir: Option<std::path::PathBuf> = None;
        let mut retirer: Option<std::path::PathBuf> = None;
        let mut selection = self.accueil_etat.selection.clone();
        let mut action = None;
        let mut secours = RecoveryChoix::default();
        let (doux, fort) = (theme::texte_doux(), theme::texte_fort());
        let presence = &self.recent_presence;
        let orphelins = !self.orphans.is_empty();
        cadre(ui, rect, "Projets récents", |ui| {
            if orphelins {
                ui.scope(|ui| {
                    ui.label(RichText::new("SHOT s'est fermé sans enregistrer").strong().color(theme::avertissement()));
                    egui::ScrollArea::vertical().id_salt("accueil_secours").max_height(190.0).auto_shrink([false, true]).show(ui, |ui| {
                        secours = self.recovery_contenu(ui);
                    });
                });
                ui.add_space(6.0);
                ui.separator();
            }
            // (Dix projets au plus, `recents::MAX` : la liste tient dans le cadre, sans défilement. Le bouton est tout en bas.)
            let hauteur = (ui.available_height() - 42.0).max(60.0);
            ui.allocate_ui(vec2(ui.available_width(), hauteur), |ui| {
                if recents.is_empty() {
                    ui.label(RichText::new("Aucun projet récent.").color(doux));
                }
                for path in &recents {
                    let existe = presence.exists(path);
                    let nom = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
                    let texte = if existe { RichText::new(nom).color(fort) } else { RichText::new(format!("{nom}   introuvable")).color(doux).italics() };
                    let choisi = selection.as_ref() == Some(path);
                    let resp = ui.selectable_label(choisi, texte);
                    let resp = resp.on_hover_text(if existe { path.display().to_string() } else { format!("Introuvable : {}", path.display()) });
                    if resp.clicked() {
                        selection = Some(path.clone());
                        resp.request_focus();
                    }
                    if resp.double_clicked() || (resp.has_focus() && enter) {
                        ouvrir = Some(path.clone());
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Retirer de la liste").on_hover_text("Le projet reste sur le disque ; il ne figure plus dans les récents.").clicked() {
                            retirer = Some(path.clone());
                            ui.close_menu();
                        }
                    });
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let b = ui.add(egui::Button::new("Ouvrir un projet…").min_size(vec2(150.0, 28.0)));
                if crate::bulles::bulle(b, "Ouvrir un projet", "Un projet SHOT ou un projet Storyboard Pro (.sboard).", "Ctrl+O").clicked() {
                    action = Some("fichier.ouvrir");
                }
            });
        });
        self.accueil_etat.selection = selection;
        self.recovery_appliquer(secours);
        if let Some(path) = retirer {
            self.settings.recent_projects.retain(|p| *p != path);
            if self.accueil_etat.selection.as_ref() == Some(&path) {
                self.accueil_etat.selection = None;
            }
            let _ = self.settings.save();
        }
        if let Some(path) = ouvrir {
            if self.recent_presence.exists(&path) {
                // (L'accueil reste là jusqu'à ce que le projet soit chargé : s'il ne s'ouvre pas, la fenêtre reste compacte.)
                self.recent_to_open = Some(path);
                self.request(Pending::OpenRecent);
            } else {
                self.status = format!("Ce projet est introuvable : {}", path.display());
            }
        }
        action
    }
}
