//! La case qui naît prend le papier (partie 128 ; Anthony, 2026-11-08, ses deux captures : « les caméras non activées au début cachent le trait
//! quand elles s'activent au contact d'un nouveau trait. Il faut que les caméras prennent également ce qui a été dessiné avant. Les caméras
//! doivent être perçues comme des cases dans lesquelles on dessine et non pas des objets. »).
//!
//! # Ce que c'est
//!
//! Un trait posé hors de toute case est sur le papier de la planche, dans le calque « Hors cadres » (`horschamp.rs`) : dans aucun plan.
//! (Depuis la partie 131 il se peint par-dessus les images des cases sur la feuille ; avec le passe-partout, dessous, comme avant.) Une case
//! pâle n'a pas de plan : rien ne lui appartient. Quand elle devient un plan plus tard (un trait dedans, la continuité), les traits du papier
//! qui passaient là ne seraient pas dans son plan, et le passe-partout les cacherait sous son image. **La case qui naît prend les traits du papier qui passent dans
//! son champ** (son cadre plus le débord), **entiers** (partie 131 : un trait n'est jamais coupé, il appartient tout entier à un seul
//! endroit) : ils entrent dans le dessin de son plan, à leur place exacte sur la planche, et la case les montre où qu'ils aillent. Les cases
//! ne coupent jamais le dessin, elles ne le cachent jamais non plus.
//!
//! # Comment
//!
//! Rien de neuf dans la mécanique : chaque trait du papier dont un point tombe dans le champ de la case née est pris comme le ferait le lasso
//! de la planche, puis **reposé à l'identique dans le dessin de cette case** par la repose de la sélection (`App::selp_reporter_vers`,
//! affinité identité, la case née pour destination) : tout ou rien (calculé avant de rien retirer), les refus de la partie 122 (une case
//! qui ne peut pas le recevoir : le trait reste sur le papier). L'étape d'annulation est regroupée avec celle de la création
//! (`History::marque`, `regrouper_depuis`) : Ctrl+Z défait la case et rend le papier. Appelé là où une case pâle devient un plan
//! (`boardui.rs` : dessiner dans une case pâle, la barre de continuité).
//!
//! Un trait d'avant la partie 131, déjà coupé en parts : seule sa part du papier qui passe dans le champ est prise (entière) ; ses parts
//! rangées dans le dessin d'autres cases y restent.
//!
//! # Limites (à suivre)
//!
//! - Seuls les **traits vectoriels** du papier sont repris ; les pixels bitmap du papier (la couche bitmap de la planche, partie 127)
//!   restent sur le papier, sous la case.
//! - Un plan **bitmap** qui naît ne reprend rien (la repose demande un calque vectoriel) : même suite.

use eframe::egui::Pos2;

use crate::app::App;
use crate::board::BoardLayout;
use crate::vector::Affine;
use crate::vignettesel::{PartRef, SelPlanche, TraitEntier};

/// Le nom de l'étape (regroupée avec celle de la création).
pub(crate) const ETAPE: &str = "La case prend le papier";

impl App {
    /// Les traits du papier qui passent dans le champ de la case `k` (rang dans `layout`) : chaque trait du calque « Hors cadres » dont un point
    /// tombe dans ce champ, entier, seul (une part d'un trait d'avant la partie 131 ne tire pas ses autres parts hors des dessins où elles
    /// sont). Rien n'est écrit.
    pub(crate) fn traits_du_papier_dans(&self, layout: &BoardLayout, k: usize) -> Vec<TraitEntier> {
        let champs = crate::horschamp::champs(layout, self.settings.vignettes.debord_fraction());
        let (Some(r), Some(calque)) = (champs.get(k), self.doc.annotations.hors_cadres_montre()) else { return Vec::new() };
        let poly: [Pos2; 4] = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
        self.doc.annotations.dans_le_lasso(layout, calque, &poly).into_iter().map(|id| TraitEntier { parts: vec![PartRef::Planche { id }], objet: false }).collect()
    }

    /// Les cases `cases` (rangs dans `layout`) viennent de naître : chacune prend les traits du papier qui passent dans son champ (voir
    /// l'en-tête), la première dans l'ordre de lecture d'abord. Rend combien de traits ont été repris ; 0 si rien ne passait là, ou si la
    /// planche ne se prête pas à la repose (pas de calque vectoriel courant).
    pub(crate) fn cases_prennent_le_papier(&mut self, layout: &BoardLayout, cases: &[usize]) -> usize {
        if cases.is_empty() || !self.selp_possible() {
            return 0;
        }
        let mut n = 0;
        for &k in cases {
            // (Après la reprise d'une case d'avant : ce qu'elle a pris n'est plus sur le papier.)
            let traits = self.traits_du_papier_dans(layout, k);
            if traits.is_empty() {
                continue;
            }
            let sur_le_papier = self.doc.annotations.traces.len();
            let mut s = SelPlanche::default();
            s.traits = traits;
            s.serial = self.history.serial();
            self.vignettes.selp = Some(s);
            self.selp_reporter_vers(layout, &Affine::IDENTITY, ETAPE, Some(k));
            self.selp_lever();
            // (Ceux qui ont vraiment quitté le papier : une repose refusée ne compte pas.)
            n += sur_le_papier.saturating_sub(self.doc.annotations.traces.len());
        }
        n
    }

    /// Après une création sur la planche : les cases qui n'étaient pas dans `cartes_avant` (numéros de cases) prennent le papier, et tout ce
    /// qui a été posé dans l'historique depuis `marque` (la création, la reprise) ne fait qu'une étape. Rend combien de traits ont été repris.
    pub(crate) fn cases_nees_prennent_le_papier(&mut self, cartes_avant: &[u64], marque: u64) -> usize {
        let layout = self.board_layout();
        let neuves: Vec<usize> = layout.cards.iter().enumerate().filter(|(_, c)| !cartes_avant.contains(&c.id)).map(|(k, _)| k).collect();
        let n = self.cases_prennent_le_papier(&layout, &neuves);
        self.history.regrouper_depuis(marque);
        n
    }
}
