//! Le lasso dans la vue Espace 3D (`docs/INTERFACE.md`, partie 152). Anthony, 2026-11-13, après son essai : « L'outil
//! lasso ne fonctionne pas dans l'espace 3D ». À la partie 146, seuls les outils de dessin prenaient le pointeur dans la
//! vue ; l'outil Sélection le rendait aux cartes.
//!
//! **L'outil décide, comme pour le dessin.** Dans la vue Espace 3D, prendre l'outil Sélection (sa case, sa touche) donne
//! le pointeur à son lasso, sur **la carte du calque courant** (elle est entourée) ; prendre un outil de la vue (choisir,
//! déplacer, tourner, taille, caméra) le rend aux cartes, et pose ce qui était choisi. Tourner et glisser la vue restent
//! sur Alt+glisser, le clic droit, le bouton du milieu et la molette.
//!
//! **Un seul outil Sélection.** Le pointeur passe par la carte (`CarteVue::vers_dessin` : le point du plan de la carte
//! que le regard touche, même au-delà de ses bords, pour entourer ce qui est près du bord) et va au code de la feuille :
//! `App::select_press`, `select_drag`, `bitmap_drag`, `finish_vdrag`, `finish_bdrag` (calque vectoriel ou bitmap, lasso
//! ou rectangle, Maj pour ajouter, clic sur un objet, boîte et poignées). Ce que la feuille peint par-dessus le dessin
//! pour lui (le lasso en cours, le contour de ce qui est choisi, la boîte, ses poignées, son pivot :
//! `App::paint_vector_overlay`) est peint par la même fonction, posé sur la carte : `App::layer_to_screen` et
//! `App::layer_zoom` passent par `App::e3_carte_vue` tant que l'outil a le pointeur dans la vue.
//!
//! **Ce qui est choisi et qu'on déplace se voit sur la carte.** Pendant un geste de la boîte, la feuille montre la
//! sélection par une feuille flottante (`flottante.rs` : le dessin sans elle, et elle seule, posée avec la transformation
//! du geste). La vue fait de même sur la carte (`e3_feuille_flottante`, tracée par `e3_paint_cards`).
//!
//! **Ctrl.** Avec l'outil Sélection ou un outil de dessin, **Ctrl+clic** prend la carte sous le pointeur, comme à la
//! partie 146 ; **Ctrl+glisser** trace un lasso sur la carte du calque courant. Avec un outil de dessin, c'est le « Ctrl
//! tenu : lasso » de la feuille (`springtools.rs`) : l'outil d'avant revient quand Ctrl est relâché. On ne sait lequel
//! des deux qu'au premier mouvement du pointeur, ou au relâché (`Dessin3d::ctrl_appui`).
//!
//! **Ce qui n'est pas fait ici.** La sélection sur plusieurs calques à la fois (`multitools.rs`, Ctrl+Maj sur la feuille)
//! et la sélection par couleur : dans la vue, le lasso prend sur la carte du calque courant seulement.

use eframe::egui::{self, vec2, Align2, Color32, CursorIcon, Event, FontId, PointerButton, Pos2, Rect};

use crate::app::{App, Tool};
use crate::document::CelId;
use crate::espace3d;
use crate::espace3ddessin::{Manque, Regard3d, CARTES_CACHEES, PAS_UNE_CARTE};
use crate::espace3dui::Drag;
use crate::vector::Affine;

pub(crate) const DE_DOS: &str = "Rien à sélectionner ici : la carte du calque courant est vue de dos. Tourne la vue (Alt+glisser ou clic droit) pour la voir de face.";
pub(crate) const PAR_LA_TRANCHE: &str = "Rien à sélectionner ici : la carte du calque courant est vue par la tranche. Tourne la vue (Alt+glisser ou clic droit).";
pub(crate) const DERRIERE: &str = "Rien à sélectionner ici : la carte du calque courant est derrière l'œil de la vue. Recule ou tourne la vue.";
pub(crate) const HORS_DU_PLAN: &str = "Rien à sélectionner ici : le regard ne touche pas la carte du calque courant (elle est entourée).";

/// Un appui avec Ctrl qui glisse de plus que ça (pixels d'écran) est un lasso ; en deçà, c'est un clic : prendre la carte.
const CTRL_GLISSE: f32 = 4.0;

/// Pourquoi le regard ne donne aucun point de la carte à l'outil Sélection.
fn raison(manque: Manque) -> &'static str {
    match manque {
        Manque::Dos => DE_DOS,
        Manque::Tranche => PAR_LA_TRANCHE,
        Manque::Derriere => DERRIERE,
        Manque::HorsCarte => HORS_DU_PLAN,
    }
}

/// La feuille flottante d'un geste sur une sélection, telle que la vue la pose sur la carte du dessin `src` : le dessin
/// sans la sélection (`base`), la sélection seule (`feuille`), la transformation qui mène un pixel de la feuille au point
/// du dessin où il se voit, et la taille de l'image de la feuille.
#[derive(Clone, Copy)]
pub(crate) struct FeuilleFlottante {
    pub base: CelId,
    pub feuille: CelId,
    pub aff: Affine,
    pub taille: [f32; 2],
}

impl App {
    /// Dans la vue Espace 3D, le pointeur **sélectionne** : l'outil Sélection y a été pris, son lasso prend sur la carte du
    /// calque courant. (Sinon il manipule les cartes, dessine, `e3_dessine`, ou trace une trajectoire, `e3_trace`.)
    pub(crate) fn e3_selectionne(&self) -> bool {
        self.e3d.dessin && self.e3_vue_montree() && self.tool == Tool::Select
    }

    /// L'outil Sélection vient d'être pris dans la vue Espace 3D (`e3_outil_pris`) : le pointeur sélectionne, sur la carte
    /// du calque courant, qui est choisie (elle est entourée, on la reconnaît).
    pub(crate) fn e3_selectionner(&mut self) {
        self.end_stroke();
        self.e3d.dessin = true;
        self.e3d.vive = None;
        self.e3d.ctrl_appui = None;
        self.espace3d.drag = None;
        // (On sélectionne sur le calque courant : la caméra choisie seule est lâchée, même quand ce calque n'est pas une carte
        // ou que les cartes sont cachées, `e3_calque_choisi`.)
        self.e3_calque_choisi(self.cur_layer);
        let (id, nom, carte) = (self.layer().id, self.layer().name.clone(), espace3d::is_card(self.layer()));
        if carte && self.espace3d.show_cards {
            self.espace3d.sel = vec![espace3d::card_selection(id)];
            self.espace3d.last_layer = id;
        }
        self.status = if carte {
            format!("Espace 3D : Sélection sur la carte « {nom} » (elle est entourée). Entoure d'un lasso ce que tu veux prendre, puis glisse-le. Ctrl+clic : prendre une autre carte. Alt+glisser ou clic droit : tourner la vue. Un outil de la vue rend la main aux cartes.")
        } else {
            PAS_UNE_CARTE.into()
        };
    }

    /// L'outil Sélection perd le pointeur de la vue sans quitter la main (un outil de la vue est repris, `e3_manipuler`) :
    /// le geste en cours est fini, la zone bitmap choisie est posée, et plus rien n'est choisi. Sans cela une sélection
    /// qu'on ne voit plus resterait sous Suppr.
    pub(crate) fn e3_selection_quitter(&mut self) {
        if self.tool != Tool::Select {
            return;
        }
        self.e3_sel_lacher(false);
        self.drop_bitmap_selection();
        self.flush_flottante();
        self.vsel = None;
    }

    // -------------------------------------------------------------------------------------------- le pointeur

    /// Le pointeur dans la vue Espace 3D quand il sélectionne : appui, glisser, lâcher, sur la carte du calque courant
    /// vue par `regard`. Alt+appui tourne la vue (`Drag::Orbit`, suivi par `e3_input`) ; Ctrl : un clic prend la carte
    /// sous le pointeur, un glisser trace le lasso.
    pub(crate) fn e3_selection_pointeur(&mut self, ctx: &egui::Context, response: &egui::Response, regard: Regard3d) {
        self.e3d.regard = Some(regard);
        // La boîte de la sélection est à jour avant d'agir (`seltools.rs`), comme sur la feuille.
        self.sync_sel_box();
        let (events, mods, primary_down, hover) = ctx.input(|i| (i.events.clone(), i.modifiers, i.pointer.primary_down(), i.pointer.hover_pos()));
        let modal = self.modal_open();
        let dans_la_vue = |pos: Pos2| regard.rect.contains(pos) && response.interact_rect.contains(pos) && ctx.layer_id_at(pos) == Some(response.layer_id);
        for event in &events {
            match event {
                Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers, .. } => {
                    if modal || !dans_la_vue(*pos) {
                        continue;
                    }
                    // Un appui dans la vue reprend la main au montage, comme dans le dessin à plat.
                    self.panel_focus = false;
                    self.images_focus = false;
                    self.key_focus = false;
                    self.spring_used();
                    if modifiers.alt {
                        self.e3_sel_lacher(modifiers.shift);
                        self.espace3d.drag = Some(Drag::Orbit);
                    } else if modifiers.command || modifiers.ctrl {
                        self.e3d.ctrl_appui = Some((*pos, modifiers.shift));
                    } else {
                        self.e3_sel_presser(regard, *pos, modifiers.shift);
                    }
                }
                Event::PointerMoved(pos) => {
                    if !self.e3_ctrl_glisse(regard, *pos, &mods) {
                        self.e3_sel_glisser(regard, *pos, &mods);
                    }
                }
                Event::PointerButton { button: PointerButton::Primary, pressed: false, .. } => {
                    self.e3_ctrl_lache(regard);
                    self.e3_sel_lacher(mods.shift);
                }
                Event::PointerGone => {
                    self.e3d.ctrl_appui = None;
                    self.e3_sel_lacher(mods.shift);
                }
                _ => {}
            }
        }
        // Relâchement manqué (perte de focus…) : on clôt le geste quand même.
        if !primary_down {
            self.e3d.ctrl_appui = None;
            self.e3_sel_lacher(mods.shift);
        }
        self.e3d.pointeur = hover.filter(|p| regard.rect.contains(*p));
    }

    /// Le pointeur bouge après un appui avec Ctrl. Vrai si le mouvement est pris ici : l'appui n'a pas encore assez
    /// glissé (ni clic, ni lasso), ou il vient de devenir un lasso. Avec un outil de dessin, le lasso prend alors l'outil
    /// Sélection le temps de tenir Ctrl (`prendre_le_lasso`, comme dans une case des Vignettes) ; le geste part du point
    /// de l'appui.
    pub(crate) fn e3_ctrl_glisse(&mut self, regard: Regard3d, pos: Pos2, mods: &egui::Modifiers) -> bool {
        let Some((depart, maj)) = self.e3d.ctrl_appui else { return false };
        if depart.distance(pos) <= CTRL_GLISSE {
            return true;
        }
        self.e3d.ctrl_appui = None;
        self.prendre_le_lasso();
        if self.e3_selectionne() {
            self.e3_sel_presser(regard, depart, maj);
            self.e3_sel_glisser(regard, pos, mods);
        }
        true
    }

    /// Le bouton est relâché après un appui avec Ctrl qui n'a pas glissé : c'était un clic, la carte sous le pointeur
    /// devient le calque courant (ce qui était choisi sur l'autre carte est posé, comme en changeant de calque).
    pub(crate) fn e3_ctrl_lache(&mut self, regard: Regard3d) {
        if let Some((depart, _)) = self.e3d.ctrl_appui.take() {
            self.e3_prendre_la_carte(regard, depart);
        }
    }

    /// L'appui : le lasso (ou le rectangle) commence au point de la carte que le regard touche, ou la boîte de ce qui
    /// est choisi est prise (une poignée, un bord, le pivot, l'intérieur). Carte de dos, par la tranche ou derrière
    /// l'œil : rien ne commence, la barre d'état dit pourquoi.
    pub(crate) fn e3_sel_presser(&mut self, regard: Regard3d, pos: Pos2, maj: bool) {
        if !self.espace3d.show_cards {
            self.status = CARTES_CACHEES.into();
            return;
        }
        let Some(carte) = self.e3_carte_du_calque(regard) else {
            // (Un dossier, un calque verrouillé… : le refus habituel ; sinon, un calque d'effet.)
            if self.layer_editable() {
                self.status = PAS_UNE_CARTE.into();
            }
            return;
        };
        let p = match carte.vers_dessin(pos) {
            Ok(p) => p,
            Err(manque) => {
                self.status = raison(manque).into();
                return;
            }
        };
        // (La tolérance d'un clic sur un trait se lit sous le pointeur : `e3_zoom_du_dessin`.)
        self.e3d.pointeur = Some(pos);
        // (La caméra choisie seule entre-temps est lâchée : le lasso prend sur le calque courant, `e3_calque_choisi`.)
        self.e3_calque_choisi(self.cur_layer);
        self.select_press(p, pos, maj);
    }

    /// Le pointeur bouge pendant un geste de l'outil : le lasso s'allonge, ou la boîte suit, au point de la carte que le
    /// regard touche. Là où il ne la touche pas (de dos, par la tranche), le point est laissé.
    pub(crate) fn e3_sel_glisser(&mut self, regard: Regard3d, pos: Pos2, mods: &egui::Modifiers) {
        if self.vdrag.is_none() && self.bdrag.is_none() {
            return;
        }
        let Some(carte) = self.e3_carte_du_calque(regard) else { return };
        let Ok(p) = carte.vers_dessin(pos) else { return };
        self.e3d.pointeur = Some(pos);
        let ctrl = mods.command || mods.ctrl;
        if self.vdrag.is_some() {
            self.select_drag(p, mods.shift, ctrl);
        } else {
            self.bitmap_drag(p, mods.shift, ctrl);
        }
    }

    /// Le bouton est relâché : le lasso choisit, le geste de la boîte est gardé (une étape d'annulation, comme à plat).
    pub(crate) fn e3_sel_lacher(&mut self, maj: bool) {
        if self.vdrag.is_some() {
            self.finish_vdrag(maj);
        }
        if self.bdrag.is_some() {
            self.finish_bdrag();
        }
    }

    // -------------------------------------------------------------------------------------------- par-dessus la vue

    /// La feuille flottante du geste en cours sur le dessin `cel` (une sélection qu'on déplace, agrandit ou tourne, ou
    /// qui attend son rendu exact), à poser sur sa carte ; rien sans geste, ou pour une enveloppe (elle ne se fait pas
    /// dans la vue).
    pub(crate) fn e3_feuille_flottante(&self, cel: CelId) -> Option<FeuilleFlottante> {
        let f = self.flottante.as_ref().filter(|f| f.src == cel && f.warp.is_none())?;
        let (base, feuille) = crate::flottante::slot_ids(f.slot);
        let image = &self.doc.cels.get(&feuille)?.raster;
        self.doc.cels.get(&base)?;
        // (Du pixel de la feuille élargie au point du dessin, puis le geste : comme `Flottante::items`.)
        let aff = match f.margin {
            (0, 0) => f.aff,
            (l, t) => Affine::translate(vec2(-(l as f32), -(t as f32))).then(&f.aff),
        };
        Some(FeuilleFlottante { base, feuille, aff, taille: [image.width as f32, image.height as f32] })
    }

    /// La légende du bas de la vue quand le pointeur sélectionne.
    pub(crate) fn e3_selection_legende(&self) -> String {
        let carte = if espace3d::is_card(self.layer()) { format!("la carte « {} »", self.layer().name) } else { "ce calque (pas une carte)".to_string() };
        format!("Sélection sur {carte} · lasso : entourer · glisser ce qui est choisi : le déplacer · poignées : agrandir, tourner · Maj : ajouter · Ctrl+clic : prendre une autre carte · Alt+glisser ou clic droit : tourner la vue · outils de la vue : manipuler")
    }

    /// Ce que l'outil Sélection ajoute à l'image de la vue : ce que la feuille peint pour lui par-dessus le dessin (le
    /// lasso en cours, le contour de ce qui est choisi, la boîte, ses poignées et son pivot), **posé sur la carte**, la
    /// légende des gestes en bas de la vue (`rect`) et le curseur.
    pub(crate) fn e3_selection_peindre(&mut self, ctx: &egui::Context, painter: &egui::Painter, response: &egui::Response, rect: Rect, regard: Regard3d) {
        self.e3d.regard = Some(regard);
        let (maj, alt, ctrl) = ctx.input(|i| (i.modifiers.shift, i.modifiers.alt, i.modifiers.command || i.modifiers.ctrl));
        self.paint_vector_overlay(painter, rect, maj);
        painter.text(rect.left_bottom() + vec2(8.0, -6.0), Align2::LEFT_BOTTOM, self.e3_selection_legende(), FontId::proportional(11.0), Color32::from_gray(190));
        let Some(pos) = response.hover_pos() else { return };
        if self.modal_open() {
            return;
        }
        if matches!(self.espace3d.drag, Some(Drag::Orbit)) {
            ctx.set_cursor_icon(CursorIcon::Grabbing);
            return;
        }
        let en_geste = self.vdrag.is_some() || self.bdrag.is_some();
        if !en_geste && alt {
            ctx.set_cursor_icon(CursorIcon::Grab);
            return;
        }
        if !en_geste && ctrl {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
            return;
        }
        // La boîte de ce qui est choisi : un curseur par partie, comme sur la feuille (`seltools.rs`).
        if self.active_or_hit_handle(pos).is_some() && self.box_cursor(ctx, painter, pos) {
            return;
        }
        let touche = self.e3_carte_du_calque(regard).filter(|_| self.espace3d.show_cards).is_some_and(|c| c.vers_dessin(pos).is_ok());
        ctx.set_cursor_icon(if touche || en_geste { CursorIcon::Crosshair } else { CursorIcon::NotAllowed });
    }
}
