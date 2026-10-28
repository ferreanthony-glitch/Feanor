//! Le choix de couleur de SHOT, avec une pipette (Anthony, 2026-10-28 : « une pipette quand on ouvre un
//! panneau de sélection de couleur »).
//!
//! Toutes les cases de couleur de SHOT (Réglages, personnages, couleurs neutres, Préférences, effets, groupes…)
//! passent par les fonctions d'ici (`srgb`, `srgba`, `srgba_non_multipliee`, `srgb_en_ligne`) : elles
//! remplacent `color_edit_button_*` d'egui, dont la fenêtre ne laissait pas de place à un bouton de plus.
//! La fenêtre reste celle d'egui (carré de teinte, barre arc-en-ciel, champs R, V, B) ; une rangée avec la
//! pipette est ajoutée au-dessus.
//!
//! ## Le geste
//! 1. Clic sur la pipette : la fenêtre de choix reste ouverte, le curseur devient une pipette, et le geste est
//!    « armé » pour CE champ (`armer`).
//! 2. Une seule capture d'écran est demandée, quelques images plus tard (`ViewportCommand::Screenshot`, la
//!    réponse arrive en `Event::Screenshot`) : elle montre exactement ce qui est affiché, partout dans la
//!    fenêtre, fenêtre de choix comprise. Aucune capture à chaque image (règle « sans latence »). Pendant ce
//!    temps, rien n'est dessiné par-dessus, pour que la capture soit nette.
//! 3. Une loupe près du curseur montre les pixels visés (lue dans cette capture, pas dans l'écran).
//! 4. Le clic suivant, n'importe où, prend la couleur du pixel et l'écrit dans le champ (`Prise`, lue par le champ
//!    à l'image même). Échap ou un clic droit annule.
//!
//! Tant que le geste est armé, les clics, les appuis du stylet et Échap sont retirés de l'entrée d'egui
//! (`filtrer_entree`, appelée par `raw_input_hook`) : le clic de la pipette ne pose aucun trait, ne choisit rien
//! et n'active aucun bouton. Un voile transparent couvre aussi l'écran (le survol ne touche plus rien dessous).
//!
//! Si la capture n'est pas disponible (affichage OpenGL de secours), le clic demande à l'application la couleur
//! du dessin sous le curseur (`surveiller` rend la position, `fournir_repli` reçoit la couleur) : c'est celle
//! de la pipette de l'outil (Alt + clic), lue sans toucher à la couleur de dessin.
//!
//! Rien n'est gardé dans le projet : le geste vit dans la mémoire d'egui. Le champ qui reçoit la couleur est
//! celui d'où la pipette est partie ; la pipette de l'outil et la couleur de dessin ne changent pas.

use std::sync::Arc;

use eframe::egui::{
    self,
    color_picker::{self, Alpha},
    ecolor::Hsva,
    epaint::StrokeKind,
    vec2, Align2, Area, Color32, ColorImage, Context, CursorIcon, Event, FontId, Frame, Id, Key, Order, PointerButton, Pos2, Rect, Response, Sense, Stroke, Ui, UiKind,
    UserData, ViewportCommand,
};
use egui_phosphor::regular as ph;

/// Largeur des barres de la fenêtre de choix (celle d'egui).
const LARGEUR_CURSEUR: f32 = 275.0;
/// Images laissées après l'armement avant la demande de capture : le temps que la bulle de la pipette et le
/// survol du bouton disparaissent de l'écran.
const IMAGES_DE_PAUSE: u8 = 2;
/// La loupe : (2 × RAYON + 1) pixels de côté, chacun grossi d'`ECHELLE` points.
const RAYON: i64 = 4;
const ECHELLE: f32 = 9.0;

/// Où en est la demande de capture.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Etape {
    /// On laisse passer quelques images (la bulle du bouton s'efface).
    Pause(u8),
    /// L'image où la demande de capture est envoyée.
    Envoi,
    /// L'image que la capture montre : rien ne doit y être ajouté.
    Capture,
    /// On attend la capture (puis on l'a) : la pipette et la loupe se montrent.
    Attente,
}

/// Le geste de pipette en cours (au plus un à la fois).
#[derive(Clone)]
struct Geste {
    /// Le champ où la couleur sera écrite : l'identifiant de sa fenêtre de choix.
    cible: Id,
    etape: Etape,
    image: Option<Arc<ColorImage>>,
    /// Le dernier appui retiré de l'entrée, à traiter à l'image.
    clic: Option<(Pos2, PointerButton)>,
    annule: bool,
    /// La couleur est demandée à l'application (pas de capture) : sans réponse, le geste s'arrête.
    repli: bool,
}

/// La couleur prise, en attente de son champ.
#[derive(Clone, Copy)]
struct Prise {
    cible: Id,
    rgb: [u8; 3],
    passe: u64,
}

fn cle_geste() -> Id {
    Id::new("champcouleur_geste")
}

fn cle_prise() -> Id {
    Id::new("champcouleur_prise")
}

fn lire(ctx: &Context) -> Option<Geste> {
    ctx.data(|d| d.get_temp::<Geste>(cle_geste()))
}

fn ecrire(ctx: &Context, geste: Geste) {
    ctx.data_mut(|d| d.insert_temp(cle_geste(), geste));
}

fn effacer(ctx: &Context) {
    ctx.data_mut(|d| d.remove::<Geste>(cle_geste()));
}

fn deposer(ctx: &Context, cible: Id, rgb: [u8; 3]) {
    let passe = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(cle_prise(), Prise { cible, rgb, passe }));
}

/// La couleur prise pour ce champ, si elle est là (elle est alors rendue une seule fois).
pub(crate) fn prise_pour(ctx: &Context, cible: Id) -> Option<[u8; 3]> {
    ctx.data_mut(|d| match d.get_temp::<Prise>(cle_prise()) {
        Some(p) if p.cible == cible => {
            d.remove::<Prise>(cle_prise());
            Some(p.rgb)
        }
        _ => None,
    })
}

/// Une pipette est armée (pour n'importe quel champ).
#[cfg(test)]
pub(crate) fn armee(ctx: &Context) -> bool {
    lire(ctx).is_some()
}

fn armee_pour(ctx: &Context, cible: Id) -> bool {
    lire(ctx).is_some_and(|g| g.cible == cible)
}

/// Arme la pipette pour le champ `cible` (un geste déjà armé pour un autre champ est remplacé).
pub(crate) fn armer(ctx: &Context, cible: Id) {
    ecrire(ctx, Geste { cible, etape: Etape::Pause(IMAGES_DE_PAUSE), image: None, clic: None, annule: false, repli: false });
    ctx.request_repaint();
}

/// À appeler depuis `raw_input_hook`, avant tout le reste : tant qu'une pipette est armée, les appuis de la souris
/// et du stylet, et Échap, ne vont plus à egui ni à l'application. L'appui (premier bouton : prendre ; second :
/// annuler) est gardé pour `surveiller`.
pub(crate) fn filtrer_entree(ctx: &Context, entree: &mut egui::RawInput) {
    let Some(mut geste) = lire(ctx) else { return };
    let (mut clic, mut annule) = (geste.clic, geste.annule);
    entree.events.retain(|e| match e {
        Event::PointerButton { pos, button, pressed, .. } => {
            if *pressed && matches!(button, PointerButton::Primary | PointerButton::Secondary) {
                clic = Some((*pos, *button));
            }
            false
        }
        Event::Touch { .. } => false,
        Event::Key { key: Key::Escape, pressed, .. } => {
            annule |= *pressed;
            false
        }
        _ => true,
    });
    if (clic, annule) != (geste.clic, geste.annule) {
        geste.clic = clic;
        geste.annule = annule;
        ecrire(ctx, geste);
        ctx.request_repaint();
    }
}

/// À appeler au début de chaque image (avant les panneaux). Fait avancer le geste : demande la capture, la
/// reçoit, prend la couleur au clic, dessine la pipette et la loupe. Rend la position du clic quand la capture
/// n'est pas disponible : l'application répond alors par `fournir_repli`.
pub(crate) fn surveiller(ctx: &Context) -> Option<Pos2> {
    // Une couleur prise que son champ n'a pas lue (il n'est plus à l'écran) est jetée.
    let maintenant = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        if d.get_temp::<Prise>(cle_prise()).is_some_and(|p| p.passe + 3 < maintenant) {
            d.remove::<Prise>(cle_prise());
        }
    });
    let mut geste = lire(ctx)?;
    // L'application n'a pas répondu à la demande de repli : fini.
    if geste.repli || geste.annule {
        effacer(ctx);
        ctx.request_repaint();
        return None;
    }

    // La capture est arrivée.
    let recue = ctx.input(|i| {
        i.events.iter().find_map(|e| match e {
            Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    });
    if let Some(image) = recue {
        if geste.etape == Etape::Attente {
            geste.image = Some(image);
        }
    }

    // La demande de capture (une seule), et les images où rien ne doit être dessiné pour qu'elle soit nette.
    let mut cacher = true;
    geste.etape = match geste.etape {
        Etape::Pause(n) if n > 1 => Etape::Pause(n - 1),
        Etape::Pause(_) => Etape::Envoi,
        Etape::Envoi => {
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(UserData::default()));
            Etape::Capture
        }
        Etape::Capture => Etape::Attente,
        Etape::Attente => {
            cacher = false;
            Etape::Attente
        }
    };

    // Le clic : le premier bouton prend la couleur, le second annule.
    if let Some((pos, bouton)) = geste.clic.take() {
        if bouton == PointerButton::Secondary {
            effacer(ctx);
            ctx.request_repaint();
            return None;
        }
        if let Some(rgb) = geste.image.as_deref().and_then(|im| pixel_sous(im, pos, ctx.screen_rect())) {
            deposer(ctx, geste.cible, rgb);
            effacer(ctx);
            ctx.request_repaint();
            return None;
        }
        // Pas de capture : on demande à l'application.
        geste.repli = true;
        ecrire(ctx, geste);
        ctx.request_repaint();
        return Some(pos);
    }

    voile(ctx, &geste, !cacher);
    ecrire(ctx, geste);
    ctx.request_repaint();
    None
}

/// La réponse de l'application à `surveiller` quand la capture manquait : la couleur sous le curseur, ou rien.
pub(crate) fn fournir_repli(ctx: &Context, rgb: Option<[u8; 3]>) {
    if let Some(geste) = lire(ctx) {
        effacer(ctx);
        if let Some(rgb) = rgb {
            deposer(ctx, geste.cible, rgb);
        }
        ctx.request_repaint();
    }
}

/// Le pixel de la capture sous le point `pos` de l'écran (en points), s'il y est. La capture est en pixels
/// de la carte graphique : le rapport vient de sa taille.
pub(crate) fn pixel_sous(image: &ColorImage, pos: Pos2, ecran: Rect) -> Option<[u8; 3]> {
    let (x, y) = index_pixel(image, pos, ecran)?;
    let c = pixel_a(image, x, y)?;
    Some([c.r(), c.g(), c.b()])
}

fn index_pixel(image: &ColorImage, pos: Pos2, ecran: Rect) -> Option<(i64, i64)> {
    if ecran.width() <= 0.0 || ecran.height() <= 0.0 || !pos.x.is_finite() || !pos.y.is_finite() {
        return None;
    }
    let [w, h] = image.size;
    if w == 0 || h == 0 {
        return None;
    }
    let fx = (pos.x - ecran.min.x) * w as f32 / ecran.width();
    let fy = (pos.y - ecran.min.y) * h as f32 / ecran.height();
    // Hors de l'écran : rien. Le bord droit et le bord bas tombent sur le dernier pixel.
    if fx < -0.5 || fy < -0.5 || fx > w as f32 + 0.5 || fy > h as f32 + 0.5 {
        return None;
    }
    Some(((fx.floor() as i64).clamp(0, w as i64 - 1), (fy.floor() as i64).clamp(0, h as i64 - 1)))
}

fn pixel_a(image: &ColorImage, x: i64, y: i64) -> Option<Color32> {
    let [w, h] = image.size;
    if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
        return None;
    }
    image.pixels.get(y as usize * w + x as usize).copied()
}

/// Le voile qui couvre l'écran pendant le geste (le survol ne touche plus rien dessous) et, quand `dessiner`,
/// la pipette au bout du curseur et la loupe.
fn voile(ctx: &Context, geste: &Geste, dessiner: bool) {
    let ecran = ctx.screen_rect();
    Area::new(Id::new("champcouleur_voile")).order(Order::Debug).fixed_pos(ecran.min).interactable(true).show(ctx, |ui| {
        ui.allocate_rect(ecran, Sense::click());
        if !dessiner {
            return;
        }
        // Le curseur du système est remplacé par la pipette dessinée.
        ctx.set_cursor_icon(CursorIcon::None);
        let Some(pointeur) = ctx.input(|i| i.pointer.hover_pos()) else { return };
        let peintre = ui.painter();
        // La loupe, si la capture est là : en bas à droite du curseur, ou de l'autre côté près d'un bord.
        if let Some(image) = geste.image.as_deref() {
            let cote = (2 * RAYON + 1) as f32 * ECHELLE;
            let hauteur = cote + 18.0;
            let mut coin = pointeur + vec2(16.0, 14.0);
            if coin.x + cote > ecran.max.x - 4.0 {
                coin.x = pointeur.x - 16.0 - cote;
            }
            if coin.y + hauteur > ecran.max.y - 4.0 {
                coin.y = pointeur.y - 14.0 - hauteur;
            }
            let loupe = Rect::from_min_size(coin, vec2(cote, cote));
            peintre.rect_filled(loupe.expand(2.0), 3.0, Color32::from_black_alpha(230));
            let centre = index_pixel(image, pointeur, ecran);
            if let Some((cx, cy)) = centre {
                for j in -RAYON..=RAYON {
                    for i in -RAYON..=RAYON {
                        let cellule = Rect::from_min_size(loupe.min + vec2((i + RAYON) as f32, (j + RAYON) as f32) * ECHELLE, vec2(ECHELLE, ECHELLE));
                        let couleur = pixel_a(image, cx + i, cy + j).map(|c| c.to_opaque()).unwrap_or(Color32::from_gray(40));
                        peintre.rect_filled(cellule, 0.0, couleur);
                    }
                }
                let milieu = Rect::from_min_size(loupe.min + vec2(RAYON as f32, RAYON as f32) * ECHELLE, vec2(ECHELLE, ECHELLE));
                peintre.rect_stroke(milieu.expand(1.0), 0.0, Stroke::new(1.0_f32, Color32::BLACK), StrokeKind::Outside);
                peintre.rect_stroke(milieu, 0.0, Stroke::new(1.0_f32, Color32::WHITE), StrokeKind::Inside);
                let texte = pixel_sous(image, pointeur, ecran).map(|c| crate::couleur::hex(c)).unwrap_or_default();
                peintre.text(loupe.center_bottom() + vec2(0.0, 3.0), Align2::CENTER_TOP, texte, FontId::monospace(12.0), Color32::WHITE);
            }
        }
        // La pipette : sa pointe, en bas à gauche de l'icône, sur le pixel visé (un contour sombre la détache de tout fond).
        let police = FontId::proportional(24.0);
        for decalage in [vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0), vec2(1.0, 1.0)] {
            peintre.text(pointeur + decalage, Align2::LEFT_BOTTOM, ph::EYEDROPPER, police.clone(), Color32::BLACK);
        }
        peintre.text(pointeur, Align2::LEFT_BOTTOM, ph::EYEDROPPER, police, Color32::WHITE);
    });
}

// --- les champs -----------------------------------------------------------------------------------------

/// Une case de couleur sans transparence (`[u8; 3]`) : remplace `ui.color_edit_button_srgb`.
pub fn srgb(ui: &mut Ui, couleur: &mut [u8; 3]) -> Response {
    let mut c = Color32::from_rgb(couleur[0], couleur[1], couleur[2]);
    let reponse = champ(ui, &mut c, Alpha::Opaque);
    if reponse.changed() {
        *couleur = [c.r(), c.g(), c.b()];
    }
    reponse
}

/// Une case de couleur avec transparence (`Color32`) : remplace `ui.color_edit_button_srgba`.
pub fn srgba(ui: &mut Ui, couleur: &mut Color32) -> Response {
    champ(ui, couleur, Alpha::BlendOrAdditive)
}

/// Une case de couleur avec transparence (`[u8; 4]`, non multipliée) : remplace
/// `ui.color_edit_button_srgba_unmultiplied`.
pub fn srgba_non_multipliee(ui: &mut Ui, couleur: &mut [u8; 4]) -> Response {
    let mut c = Color32::from_rgba_unmultiplied(couleur[0], couleur[1], couleur[2], couleur[3]);
    let reponse = champ(ui, &mut c, Alpha::OnlyBlend);
    if reponse.changed() {
        *couleur = c.to_srgba_unmultiplied();
    }
    reponse
}

/// Le choix de couleur posé dans la page (sans case ni fenêtre), avec sa pipette : remplace
/// `color_picker::color_picker_color32`. Rend vrai quand la couleur change.
pub fn srgb_en_ligne(ui: &mut Ui, couleur: &mut [u8; 3]) -> bool {
    let id = ui.auto_id_with("en_ligne");
    let ctx = ui.ctx().clone();
    let actuelle = Color32::from_rgb(couleur[0], couleur[1], couleur[2]);
    let mut hsva = hsva_de(ui, id, actuelle);
    let mut change = false;
    if let Some(rgb) = prise_pour(&ctx, id) {
        hsva = avec_prise(hsva, rgb);
        change = true;
    }
    ui.vertical(|ui| {
        rangee_pipette(ui, id);
        change |= color_picker::color_picker_hsva_2d(ui, &mut hsva, Alpha::Opaque);
    });
    let nouvelle = Color32::from(hsva);
    memoriser(ui, id, nouvelle, hsva);
    if change {
        *couleur = [nouvelle.r(), nouvelle.g(), nouvelle.b()];
    }
    change
}

/// La case de couleur et sa fenêtre de choix (le même comportement que celui d'egui, plus la pipette).
/// L'identifiant de la fenêtre est celui qu'egui donnait : `any_popup_open` reste vrai tant qu'elle est ouverte
/// (un geste d'annulation par ouverture : `App::edited`).
fn champ(ui: &mut Ui, couleur: &mut Color32, alpha: Alpha) -> Response {
    let ctx = ui.ctx().clone();
    let popup_id = ui.auto_id_with("popup");
    let ouvert = ui.memory(|m| m.is_popup_open(popup_id));
    let mut bouton = bouton_couleur(ui, *couleur, ouvert);
    if bouton.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }

    let mut hsva = hsva_de(ui, popup_id, *couleur);
    let mut change = false;
    // La couleur que la pipette a prise pour ce champ.
    if let Some(rgb) = prise_pour(&ctx, popup_id) {
        hsva = avec_prise(hsva, rgb);
        change = true;
    }

    if ui.memory(|m| m.is_popup_open(popup_id)) {
        let zone = Area::new(popup_id)
            .kind(UiKind::Picker)
            .order(Order::Foreground)
            .fixed_pos(bouton.rect.max)
            .show(&ctx, |ui| {
                ui.spacing_mut().slider_width = LARGEUR_CURSEUR;
                Frame::popup(ui.style()).show(ui, |ui| {
                    rangee_pipette(ui, popup_id);
                    if color_picker::color_picker_hsva_2d(ui, &mut hsva, alpha) {
                        change = true;
                    }
                });
            })
            .response;
        // Pendant le geste de la pipette, la fenêtre reste ouverte (Échap et les clics sont pour la pipette).
        if !bouton.clicked() && !armee_pour(&ctx, popup_id) && (ui.input(|i| i.key_pressed(Key::Escape)) || zone.clicked_elsewhere()) {
            ui.memory_mut(|m| m.close_popup());
        }
    }

    if change {
        *couleur = Color32::from(hsva);
        bouton.mark_changed();
    }
    memoriser(ui, popup_id, *couleur, hsva);
    bouton
}

/// La rangée du haut de la fenêtre de choix : le bouton pipette.
fn rangee_pipette(ui: &mut Ui, cible: Id) {
    let ctx = ui.ctx().clone();
    let armee = armee_pour(&ctx, cible);
    ui.horizontal(|ui| {
        let r = crate::bulles::etat(
            ui,
            armee,
            ph::EYEDROPPER,
            "Pipette",
            "Prendre la couleur d'un pixel de l'écran : clique ensuite n'importe où dans SHOT. Échap ou clic droit : annuler. Le dessin et la couleur de dessin ne changent pas.",
            "",
        );
        #[cfg(test)]
        ctx.data_mut(|d| d.insert_temp(Id::new("champcouleur_essai_bouton"), r.rect));
        if r.clicked() && !armee {
            armer(&ctx, cible);
        }
    });
}

/// La case : un carré de la couleur, sur le damier si elle est transparente (comme celle d'egui).
fn bouton_couleur(ui: &mut Ui, couleur: Color32, ouvert: bool) -> Response {
    let taille = ui.spacing().interact_size;
    let (rect, reponse) = ui.allocate_exact_size(taille, Sense::click());
    reponse.widget_info(|| egui::WidgetInfo::new(egui::WidgetType::ColorButton));
    if ui.is_rect_visible(rect) {
        let visuels = if ouvert { &ui.visuals().widgets.open } else { ui.style().interact(&reponse) };
        let rect = rect.expand(visuels.expansion);
        color_picker::show_color_at(ui.painter(), couleur, rect.shrink(1.0));
        ui.painter().rect_stroke(rect, visuels.corner_radius.at_most(2), (1.0, visuels.bg_fill), StrokeKind::Inside);
    }
    reponse
}

/// La couleur de la pipette dans la fenêtre : la transparence du champ est gardée.
fn avec_prise(avant: Hsva, rgb: [u8; 3]) -> Hsva {
    let mut h = Hsva::from(Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
    h.a = avant.a;
    h
}

/// La teinte et la saturation que le champ montrait (un gris n'a pas de teinte : on garde celle qu'on réglait).
fn hsva_de(ui: &Ui, id: Id, couleur: Color32) -> Hsva {
    ui.data(|d| d.get_temp::<(Color32, Hsva)>(id)).filter(|(c, _)| *c == couleur).map(|(_, h)| h).unwrap_or_else(|| Hsva::from(couleur))
}

fn memoriser(ui: &Ui, id: Id, couleur: Color32, hsva: Hsva) {
    ui.data_mut(|d| d.insert_temp(id, (couleur, hsva)));
}
