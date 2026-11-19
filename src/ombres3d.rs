//! **Les ombres des cartes dans la vue Espace 3D** (partie 189, `docs/INTERFACE.md` ; `docs/ESPACE3D.md`, partie 19).
//!
//! La partie 185 (`ombres.rs`) a donné aux cartes les ombres du décor 3D dans l'image du plan et les exports. La vue Espace 3D (plein
//! écran, à côté, vues multiples) montre maintenant les mêmes, sous le même soleil, de quelque côté qu'on la regarde (les ombres du
//! soleil sont dans le monde) :
//! 1. **une carte porte l'ombre de son encre** sur le décor de la vue ;
//! 2. **le décor et les autres cartes ombrent une carte** de la vue, comme une face du décor à l'ombre ;
//! 3. les mêmes cases (`porte_ombre`, `recoit_ombres`), les mêmes calques sans ombre (`ombres::concerne` : dossiers, effets,
//!    annotations, calques accrochés à la caméra), la même règle : sans les ombres du décor (l'aspect, ou un ordinateur « sans
//!    ombres »), rien ne change et rien ne coûte rien de plus.
//!
//! **Comment.** Les cartes de la vue sont dessinées par egui, par-dessus l'image du décor que la carte graphique a faite dans ses
//! propres cibles (`decor3dgpu::VueDecor`). Ici, une fois par image (`e3_ombres_de_la_vue`), les cartes qui portent une ombre (leurs
//! quatre coins dans le monde, leur dessin tel que la vue le montre : la texture d'egui, et leur opacité) partent avec le décor :
//! - **la carte d'ombres des cartes** : elles sont dessinées vues du soleil (la matrice de la carte d'ombres du décor,
//!   `decor3dgpu::eclairage`), là où leur encre est au moins à moitié opaque. Le soleil étant le même de partout, elle sert à toutes
//!   les vues, et n'est refaite que si une carte, son dessin ou le soleil a bougé (`CartesVue::cle`) ;
//! - **le décor de la vue** la lit, avec le filtre du décor, au point du monde de chacun de ses fragments : il perd le soleil que
//!   les cartes lui cachent avant sa brume, là où il le met lui-même (le même calcul que l'image du plan, `gpu::OMBRES_COMMUN`) ;
//! - **une carte qui reçoit les ombres** : juste après son dessin, un rappel de la carte graphique (`decor3dgpu::RecueCarte`) repasse
//!   sur son maillage (les mêmes triangles qu'egui, la même lecture de son dessin) et retranche de l'écran ce que l'ombre ôte à sa
//!   couleur (le décor et les autres cartes, la pente de la carte dans la carte d'ombres : jamais dans sa propre ombre), comme
//!   `assombrir_carte` dans l'image du plan.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use eframe::egui::{self, Rect};
use eframe::egui_wgpu::wgpu;

use crate::app::App;
use crate::decor3dgpu::{CartesVue, PorteuseVue, RecueCarte};
use crate::espace3dmath::{self as m, View, V3};

#[cfg(test)]
thread_local! {
    /// Essais : la vue comme avant la partie 189 (aucune ombre de carte préparée), pour comparer au pixel près.
    pub(crate) static AVANT_189: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Les ombres des cartes de la vue Espace 3D à une image : les cartes qui en portent (pour le décor de chaque vue) et les calques
/// dont la carte en reçoit.
#[derive(Clone)]
pub(crate) struct OmbresDeLaVue {
    pub cartes: Arc<CartesVue>,
    pub recoivent: Vec<usize>,
}

impl App {
    /// Les ombres des cartes de la vue à cette image ; rien (le décor et les cartes comme avant, rien de plus à faire) sans carte
    /// graphique, sans décor 3D, sans ses ombres (l'aspect, ou la qualité de cet ordinateur), cartes cachées, ou aucune carte qui
    /// porte ou reçoit une ombre.
    pub(crate) fn e3_ombres_de_la_vue(&mut self, ctx: &egui::Context) -> Option<OmbresDeLaVue> {
        #[cfg(test)]
        if AVANT_189.with(|a| a.get()) {
            return None;
        }
        if !self.espace3d.show_cards || self.gpu.is_none() {
            return None;
        }
        let d = self.doc.decor_du_plan(self.cur_panel)?;
        if !d.aspect.ombres {
            return None;
        }
        let objets: Vec<crate::decor3d::Pose3d> = d.vus().map(|o| (o.modele.clone(), o.matrice())).collect();
        // (La qualité que la vue donne à son décor, `e3_peindre_decor` : sans ombres, pas d'ombres de cartes non plus.)
        if objets.is_empty() || crate::decor3d::qualite(crate::decor3d::triangles_de(&objets)).ombres().0 == 0 {
            return None;
        }
        let world = self.e3_world();
        let frame = self.cur_frame;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let (mut porteuses, mut recoivent) = (Vec::new(), Vec::new());
        for li in self.e3_visible_cards() {
            let layer = &self.layers()[li];
            if !crate::ombres::concerne(layer) {
                continue;
            }
            let (porte, recoit) = (layer.porte_ombre, layer.recoit_ombres);
            if recoit {
                recoivent.push(li);
            }
            if !porte {
                continue;
            }
            let coins = world.card_corners(&world.carte(layer, frame));
            let opacite = layer.opacity_at(frame);
            let (id, cel) = (layer.id, layer.shown_at(frame));
            let Some((tex, version)) = self.e3_dessin_montre(ctx, id, cel) else { continue };
            // (Une texture que la carte graphique n'a pas encore reçue, faite à cette image : la vue sera refaite à la suivante.)
            let Some(texture) = self.e3_texture_wgpu(tex) else {
                self.espace3d.reportee = true;
                ctx.request_repaint();
                continue;
            };
            (li, tex, version, opacite.to_bits(), coins.map(|c| c.map(f32::to_bits))).hash(&mut h);
            porteuses.push(PorteuseVue { coins, texture, opacite });
        }
        if porteuses.is_empty() && recoivent.is_empty() {
            return None;
        }
        // (On dessine dans la vue : le dessin de la carte change sans que sa version le dise encore, `e3_texture_vive`.)
        if self.e3_dessine() || self.e3_selectionne() {
            ctx.cumulative_pass_nr().hash(&mut h);
        }
        Some(OmbresDeLaVue { cartes: Arc::new(CartesVue { porteuses, cle: h.finish() }), recoivent })
    }

    /// Le dessin que la vue montre sur la carte du calque `id` (comme `e3_paint_cards` : la sélection flottante enlevée ; le dessin
    /// d'avant quand le nouveau attend), et sa version.
    fn e3_dessin_montre(&mut self, ctx: &egui::Context, id: u64, cel: Option<crate::document::CelId>) -> Option<(egui::TextureId, Option<u64>)> {
        let image = cel.map(|c| self.e3_feuille_flottante(c).map_or(c, |f| f.base));
        if let Some(tex) = image.and_then(|c| self.e3_card_texture(ctx, c)) {
            return Some((tex, image.and_then(|c| self.espace3d.card_textures.get(&c)).map(|t| t.version)));
        }
        let avant = self.espace3d.vu_par_calque.get(&id)?;
        let t = self.espace3d.card_textures.get(avant)?;
        Some((t.tex.id(), Some(t.version)))
    }

    /// La texture de la carte graphique derrière la texture d'egui `id` (rien : egui ne l'a pas encore envoyée).
    pub(crate) fn e3_texture_wgpu(&self, id: egui::TextureId) -> Option<wgpu::Texture> {
        let g = self.gpu.as_ref()?;
        let r = g.render_state().renderer.read();
        r.texture(&id)?.texture.clone()
    }

    /// L'ombre reçue par la carte du calque `li` de la vue (rien : elle n'en reçoit pas, ou pas à cette image), à poser juste après
    /// son dessin `dessin` (le maillage d'egui, `card_mesh`, de texture `tex`, dont le point (u, v) est `at(u, v)`), dans la zone
    /// `zone` de la vue de prise `cle` ; `rang` : son rang parmi les cartes de cette vue qui reçoivent les ombres.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn e3_ombre_recue(&self, li: usize, dessin: &egui::Mesh, tex: egui::TextureId, at: &dyn Fn(f32, f32) -> V3, view: &View, zone: Rect, cle: u64, rang: u32) -> Option<(Rect, RecueCarte)> {
        let o = self.espace3d.ombres.as_ref()?;
        if !o.recoivent.contains(&li) || zone.width() <= 0.0 || zone.height() <= 0.0 {
            return None;
        }
        let cible_egui = self.gpu.as_ref()?.render_state().target_format;
        let texture = self.e3_texture_wgpu(tex)?;
        let normale = m::cross(m::sub(at(1.0, 0.0), at(0.0, 0.0)), m::sub(at(0.0, 1.0), at(0.0, 0.0)));
        let len = m::length(normale);
        if !(len.is_finite() && len > 1e-12) {
            return None;
        }
        // (L'opacité telle qu'egui la pose : la couleur des sommets, sur 255.)
        let opacite = (self.layers()[li].opacity_at(self.cur_frame) * 255.0).round() / 255.0;
        let n = crate::espace3dview::SUBDIV_CARTE;
        let ortho = crate::espace3dvues::est_ortho(view);
        let mut sommets = Vec::with_capacity(dessin.vertices.len() * 8);
        for (k, v) in dessin.vertices.iter().enumerate() {
            let p = at((k % (n + 1)) as f32 / n as f32, (k / (n + 1)) as f32 / n as f32);
            let ndc = [(v.pos.x - zone.left()) / zone.width() * 2.0 - 1.0, 1.0 - (v.pos.y - zone.top()) / zone.height() * 2.0];
            let w = if ortho { 1.0 } else { view.to_view(p)[2].max(crate::espace3dview::NEAR) };
            sommets.extend_from_slice(&[ndc[0], ndc[1], v.uv.x, v.uv.y, p[0], p[1], p[2], w]);
        }
        let r = RecueCarte { cle, rang, total: o.recoivent.len() as u32, sommets, indices: dessin.indices.clone(), texture, opacite, normale: m::scale(normale, 1.0 / len), cible_egui };
        Some((zone, r))
    }
}
