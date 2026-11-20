//! Partie 201 (Anthony : « qu'un calque créé se place toujours devant la caméra pour simplifier et fluidifier le passage au dessin » ;
//! « si ce n'est toujours, en tout cas par défaut ») : **un calque neuf se pose devant la caméra du plan**.
//!
//! - **Quand** : la caméra du plan, à l'image courante, n'est pas filmée par le chemin de toujours (`Document::repere_du_plan` : une
//!   caméra libre tirée dans la vue hors des anciennes limites ou dont le cadre sort de la feuille, partie 200.7 ; un plan venu du
//!   format 55 en Espace 3D). Là, une carte posée sur la feuille (la place d'un calque neuf jusqu'ici) peut tomber hors du cadre : on
//!   dessinait sur rien de visible. **Une caméra qui n'a jamais quitté la feuille** (le dessin à plat, la caméra des Vignettes) **ne
//!   change rien** : le calque est exactement celui d'avant (mêmes champs, mêmes pixels).
//! - **Où** : au milieu du cadre de la caméra à cette image, face à elle (perpendiculaire à son regard, son roulis compris), à la
//!   distance où sa feuille, à sa taille d'origine, remplit le cadre comme la caméra de référence la voit (la distance de référence
//!   fois le zoom). L'origine du calque (partie 182) est celle d'un calque neuf (le centre de la feuille) ; un calque neuf n'est pas
//!   aimanté au sol.
//! - **Par où** : « Nouveau calque » (la barre « + » de la timeline, les menus, la palette, la fenêtre « Nouveau calque »), coller une
//!   image comme nouveau calque, le calque d'un personnage (les pastilles), le calque d'une vidéo, et le premier calque d'un plan fait
//!   par « Nouveau plan depuis cette vue ». **Pas** : dupliquer, coller des calques (ils gardent la place de leur source), le calque de
//!   couleur (il suit son calque de trait), les dossiers, les calques d'effet, d'annotation et de couleur unie (ils n'ont pas de place
//!   de carte), les surfaces Sol, Mur, Ciel (déjà posées par la vue).
//! - **Un réglage** : Préférences > Caméra, « Placer un nouveau calque devant la caméra », coché d'origine (écrit seulement décoché) ;
//!   décoché, les calques se créent comme avant.
//!
//! Dans l'étape d'annulation de la création, comme avant.

use crate::app::App;
use crate::document::{Document, Layer};
use crate::espace3d::{Monde, Xf};
use crate::espace3dmath as m;

/// Le placement d'une carte devant la caméra `cam` du plan (dans le monde `world`), pour le calque `layer` (sa taille d'origine).
pub fn xf_devant(world: &Monde, cam: &crate::camera::Camera, layer: &Layer) -> Option<Xf> {
    let sc = world.scene_cam(cam);
    let v = sc.view(1.0);
    let d = world.d0 * cam.zoom.max(0.05);
    let avant = m::normalize(v.forward);
    let pos = m::add(sc.pos, m::scale(avant, d));
    let back = m::scale(avant, -1.0);
    #[rustfmt::skip]
    let axes: m::M4 = [
        v.right[0], v.right[1], v.right[2], 0.0,
        v.up[0], v.up[1], v.up[2], 0.0,
        back[0], back[1], back[2], 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];
    let taille = world.card_xf_de(layer, &Default::default(), 0.0).size;
    let xf = Xf { pos, rot: m::euler_of(&axes), size: taille };
    xf.pos.iter().chain(xf.rot.iter()).chain(xf.size.iter()).all(|v| v.is_finite()).then_some(xf)
}

/// Le calque `layer` (neuf) se pose-t-il devant la caméra du plan `pi` à l'image `frame` ? Une carte (ni dossier, ni effet, ni
/// annotation, ni couleur unie, ni calque de couleur, ni accrochée à la caméra), et une caméra que le chemin de toujours ne filme pas.
pub fn a_poser(doc: &Document, pi: usize, frame: usize, layer: &Layer) -> bool {
    crate::espace3d::is_card(layer) && !layer.accroche && !layer.annotation && layer.unie.is_none() && layer.couleur.is_none() && doc.repere_du_plan(pi, frame).is_some()
}

/// Pose le calque `li` du plan `pi` devant sa caméra à l'image `frame`, s'il le faut (`a_poser`). Vrai s'il a été posé.
pub fn poser_devant(doc: &mut Document, pi: usize, li: usize, frame: usize) -> bool {
    let Some(layer) = doc.panels.get(pi).and_then(|p| p.layers.get(li)) else { return false };
    if !a_poser(doc, pi, frame, layer) {
        return false;
    }
    let world = Monde::du_plan(doc, pi);
    let cam = doc.panel_camera_reelle(pi, frame);
    let Some(xf) = xf_devant(&world, &cam, layer) else { return false };
    let layer = &mut doc.panels[pi].layers[li];
    world.apply_card_xf_libre(layer, frame, &xf);
    true
}

impl App {
    /// Le calque neuf `li` du plan courant : devant la caméra, si le réglage le veut (Préférences > Caméra) et s'il le faut.
    pub(crate) fn calque_neuf_devant_la_camera(&mut self, li: usize) {
        if !self.settings.calque_devant_la_camera {
            return;
        }
        let (pi, frame) = (self.cur_panel, self.cur_frame.min(self.frame_count().saturating_sub(1)));
        if poser_devant(&mut self.doc, pi, li, frame) {
            self.renderer.invalidate_all();
        }
    }

    /// Préférences > Caméra : « Placer un nouveau calque devant la caméra ».
    pub(crate) fn calque_devant_pref_ui(&mut self, ui: &mut eframe::egui::Ui) {
        let r = ui.checkbox(&mut self.settings.calque_devant_la_camera, "Placer un nouveau calque devant la caméra");
        let r = crate::bulles::bulle(
            r,
            "Placer un nouveau calque devant la caméra",
            "Quand la caméra du plan a quitté la feuille (tirée dans la vue 3D), un calque neuf se pose face à elle, au milieu de son cadre : le premier trait tombe où elle regarde. Une caméra restée sur la feuille ne change rien. Décoché : les calques se créent comme avant.",
            "",
        );
        if r.changed() {
            let _ = self.settings.save();
        }
    }
}
