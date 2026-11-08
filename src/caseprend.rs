//! La case qui naît prend le papier (partie 128 ; Anthony, 2026-11-08, ses deux captures : « les caméras non activées au début cachent le trait
//! quand elles s'activent au contact d'un nouveau trait. Il faut que les caméras prennent également ce qui a été dessiné avant. Les caméras
//! doivent être perçues comme des cases dans lesquelles on dessine et non pas des objets. »).
//!
//! # Ce que c'est
//!
//! Un trait tracé sur la planche est réparti au lâcher (`horschamp.rs`) : la part dans le champ d'une case va dans le dessin de son plan, le
//! reste va sur le papier, dans le calque « Hors cadres », peint **sous** les cases. Une case pâle n'a pas de plan : rien ne lui est donné.
//! Quand elle devient un plan plus tard (un trait dedans, la continuité), son fond recouvrait les traits du papier qui passaient là : ils
//! n'étaient pas perdus, mais cachés. Maintenant **la case qui naît prend les traits du papier qui passent dans son champ** (son cadre plus
//! le débord) : la part dans le champ entre dans le dessin du plan, le reste reste sur le papier, exactement comme un trait qu'on vient de
//! tracer. La règle des cases, lue dans l'autre sens : elles ne coupent jamais le dessin, elles ne le cachent jamais non plus.
//!
//! # Comment
//!
//! Rien de neuf dans la mécanique : les traits du papier dont une part tombe dans le champ de la case née sont pris comme le ferait le lasso
//! de la planche (`App::selp_dans_le_lasso`, le champ pour lasso), puis **reposés à l'identique** par la repose de la sélection
//! (`App::selp_reporter`, affinité identité) : tout ou rien (calculé avant de rien retirer), l'ordre de peinture gardé, les refus de la
//! partie 122 (une case qui ne peut pas recevoir : la part reste sur le papier, par-dessus), les numéros de lien des parts. L'étape
//! d'annulation est regroupée avec celle de la création (`History::marque`, `regrouper_depuis`) : Ctrl+Z défait la case et rend le papier.
//! Appelé là où une case pâle devient un plan (`boardui.rs` : dessiner dans une case pâle, la barre de continuité).
//!
//! # Limites (à suivre)
//!
//! - Seuls les **traits vectoriels** du papier sont repris ; les pixels bitmap du papier (la couche bitmap de la planche, chantier
//!   `vignettes-bitmap`) viendront après sa fusion.
//! - Une case née **par le bout d'un trait** (partie 113 : un trait dont le bout tombe dans la case pâle suivante la fait naître,
//!   `horschamp_creer_cases`) reçoit ce trait mais ne reprend pas encore les traits d'avant : à brancher dans `horschamp.rs`, tenu par
//!   l'autre session.
//! - Un plan **bitmap** qui naît ne reprend rien (la repose demande un calque vectoriel) : même suite.

use eframe::egui::Pos2;

use crate::app::App;
use crate::board::BoardLayout;
use crate::vector::Affine;
use crate::vignettesel::{PartRef, SelPlanche, TraitEntier};

/// Le nom de l'étape (regroupée avec celle de la création).
pub(crate) const ETAPE: &str = "La case prend le papier";

impl App {
    /// Les traits du papier qui passent dans le champ d'une des cases `cases` (rangs dans `layout`) : chaque trait entier dont une part est
    /// sur la planche (calque « Hors cadres ») et dont un point tombe dans ce champ ; jamais deux fois le même. Rien n'est écrit.
    pub(crate) fn traits_du_papier_dans(&self, layout: &BoardLayout, cases: &[usize]) -> Vec<TraitEntier> {
        let champs = crate::horschamp::champs(layout, self.settings.vignettes.debord_fraction());
        let mut out: Vec<TraitEntier> = Vec::new();
        for &k in cases {
            let Some(r) = champs.get(k) else { continue };
            let poly: [Pos2; 4] = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
            for t in self.selp_dans_le_lasso(layout, &poly) {
                if t.objet || !t.parts.iter().any(|p| matches!(p, PartRef::Planche { .. })) {
                    continue;
                }
                if !out.iter().any(|o| o.parts.iter().any(|p| t.parts.contains(p))) {
                    out.push(t);
                }
            }
        }
        out
    }

    /// Les cases `cases` (rangs dans `layout`) viennent de naître : elles prennent les traits du papier qui passent dans leur champ (voir
    /// l'en-tête). Rend combien de traits ont été repris ; 0 si rien ne passait là, ou si la planche ne se prête pas à la repose (pas de
    /// calque vectoriel courant).
    pub(crate) fn cases_prennent_le_papier(&mut self, layout: &BoardLayout, cases: &[usize]) -> usize {
        if cases.is_empty() || !self.selp_possible() {
            return 0;
        }
        let traits = self.traits_du_papier_dans(layout, cases);
        if traits.is_empty() {
            return 0;
        }
        let n = traits.len();
        let mut s = SelPlanche::default();
        s.traits = traits;
        s.serial = self.history.serial();
        self.vignettes.selp = Some(s);
        self.selp_reporter(layout, &Affine::IDENTITY, ETAPE);
        self.selp_lever();
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
