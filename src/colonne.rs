//! La colonne des panneaux en pile (`docs/INTERFACE.md`, partie 194 ; l'idée vient de Blender) : à droite du dessin, au-dessus
//! de la bande du bas, une seule pile de panneaux les uns sous les autres. Chaque panneau a un en-tête : une poignée (⠿) qu'on
//! glisse vers le haut ou le bas pour le ranger ailleurs dans la pile, le chevron et le titre qui le replient, ▲ ▼ et ✕. Un
//! panneau ouvert qui ne grandit pas a un bord du bas qu'on tire pour régler sa hauteur ; un seul grandit et prend le reste
//! (« Propriétés » s'il est là et ouvert, sinon le dernier ouvert). Un panneau replié ne montre que son en-tête.
//!
//! **Propriétés** est un panneau à part : les panneaux de réglages (Réglages de l'outil, Fiche du plan, Effets, Scénario,
//! Espace 3D, Historique…) y sont des onglets d'icônes, à la verticale sur son bord gauche, comme l'éditeur de propriétés
//! de Blender. Son bouton ↗ sort l'onglet de devant en un panneau à lui, juste au-dessus de Propriétés ; fermer ce panneau
//! le remet en onglet.
//!
//! Ici, le modèle seul (retenu dans la disposition de chaque espace de travail, `DockLayout::colonne`), qui se teste sans
//! interface ; l'affichage est dans `colonneui.rs`. Les panneaux eux-mêmes ne changent pas : la pile appelle le même
//! `App::pane_ui` que les onglets d'egui_dock.

use serde::{Deserialize, Serialize};

use crate::dock::Pane;
use crate::workspace::Espace;

/// Hauteur du corps d'un panneau tirée à la main, au moins et au plus, en points (celles de la maquette validée).
pub const HAUTEUR_MIN: f32 = 70.0;
pub const HAUTEUR_MAX: f32 = 700.0;
/// Hauteur du corps d'un panneau qui ne sait pas dire la sienne (une liste, un tableau), tant qu'on ne l'a pas tirée.
pub const HAUTEUR_DEFAUT: f32 = 230.0;
/// Hauteur au moins du panneau qui grandit.
pub const GRANDIT_MIN: f32 = 120.0;
/// Hauteur de l'en-tête d'un panneau (poignée, chevron, titre, boutons).
pub const ENTETE: f32 = 24.0;
/// Épaisseur du bord du bas qu'on tire.
pub const BORD: f32 = 5.0;

/// Ce qu'est un panneau de la pile. (Une sorte de plus, plus tard : la Scène de l'étape 2, ou un panneau propre à
/// l'onglet 3D, s'ajoute ici sans toucher aux autres.)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sorte {
    /// Un panneau de FEANOR ; plusieurs, en onglets dans l'en-tête (la Couleur et les Brosses dédiées, comme avant).
    #[default]
    #[serde(rename = "simple")]
    Simple,
    /// « Propriétés » : ses panneaux en onglets d'icônes verticaux.
    #[serde(rename = "proprietes")]
    Proprietes,
}

/// Un panneau de la pile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanneauPile {
    #[serde(default)]
    pub sorte: Sorte,
    /// Ses panneaux : un d'ordinaire ; les onglets, pour Propriétés.
    #[serde(default)]
    pub panneaux: Vec<Pane>,
    /// Le rang de celui qui est devant.
    #[serde(default)]
    pub actif: usize,
    /// Replié : seul l'en-tête se voit.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub replie: bool,
    /// Hauteur du corps tirée à la main, en points ; rien : celle de son contenu (`HAUTEUR_DEFAUT` s'il ne la dit pas).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hauteur: Option<f32>,
    /// Sorti des onglets de Propriétés (↗) : le fermer l'y remet.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sorti: bool,
}

impl PanneauPile {
    /// Un panneau de FEANOR seul.
    pub fn simple(pane: Pane) -> PanneauPile {
        PanneauPile::groupe(vec![pane])
    }

    /// Plusieurs panneaux en onglets dans l'en-tête (la Couleur et les Brosses dédiées).
    pub fn groupe(panes: Vec<Pane>) -> PanneauPile {
        PanneauPile { sorte: Sorte::Simple, panneaux: panes, actif: 0, replie: false, hauteur: None, sorti: false }
    }

    /// Propriétés, avec ces onglets.
    pub fn proprietes(onglets: Vec<Pane>) -> PanneauPile {
        PanneauPile { sorte: Sorte::Proprietes, ..PanneauPile::groupe(onglets) }
    }

    /// Le panneau devant (l'onglet devant, pour Propriétés), s'il y en a un.
    pub fn devant(&self) -> Option<Pane> {
        self.panneaux.get(self.actif.min(self.panneaux.len().saturating_sub(1))).copied()
    }

    pub fn est_proprietes(&self) -> bool {
        self.sorte == Sorte::Proprietes
    }

    /// Le titre de l'en-tête.
    pub fn titre(&self) -> &'static str {
        match self.sorte {
            Sorte::Proprietes => "Propriétés",
            Sorte::Simple => self.devant().map_or("Panneau", Pane::name),
        }
    }
}

/// La pile de la colonne, de haut en bas.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Colonne {
    #[serde(default)]
    pub panneaux: Vec<PanneauPile>,
}

/// Les panneaux qui ne vont jamais dans la colonne : le dessin, la bande du bas, la colonne elle-même, et les anciens
/// panneaux retirés (`DockLayout::sanitized` les enlève partout).
pub fn hors_colonne(pane: Pane) -> bool {
    matches!(pane, Pane::Canvas | Pane::Montage | Pane::Timeline | Pane::Colonne | Pane::View | Pane::Guides | Pane::Brushes | Pane::Layers | Pane::TimelineSequence | Pane::Banque3d)
}

/// Les panneaux qui sont des onglets de Propriétés quand on les ouvre (les réglages, les listes) ; les autres (la Couleur,
/// les Brosses dédiées, la Planche, les icônes des outils, la Scène de la partie 197) ont toujours leur propre panneau dans la pile.
pub fn en_onglet(pane: Pane) -> bool {
    !hors_colonne(pane) && !matches!(pane, Pane::Color | Pane::Personnages | Pane::Vignettes | Pane::Tools | Pane::Scene | Pane::Scene3d)
}

/// L'ordre des onglets de Propriétés : un onglet qui arrive se range à son rang parmi ceux qui y sont (Réglages, Fiche,
/// Effets, Scénario, Espace 3D, Historique…), comme dans la disposition d'avant.
const ORDRE_DES_ONGLETS: [Pane; 19] = [
    // (Partie 198 : ceux de l'onglet 3D, dans son ordre.)
    Pane::Objet3d,
    Pane::Camera3d,
    Pane::Lumieres3d,
    Pane::Monde3d,
    Pane::Decor3d,
    Pane::ToolOptions,
    Pane::Sheet,
    Pane::Effects,
    Pane::Scenario,
    Pane::Espace3d,
    Pane::History,
    Pane::Comments,
    Pane::Bruitages,
    Pane::Prereglages,
    Pane::XSheet,
    Pane::SuggestionsSonores,
    Pane::References,
    Pane::Bibliotheque,
    Pane::MonPanneau,
];

/// La famille de la Couleur : ses panneaux se rangent ensemble, dans cet ordre.
pub const FAMILLE_COULEUR: [Pane; 2] = [Pane::Color, Pane::Personnages];

/// Range `pane` dans `liste` à son rang de `ordre` (après le dernier qui le précède, sinon avant le premier qui le suit ;
/// à la fin s'il n'y a pas de rang). Rend sa place.
fn ranger(liste: &mut Vec<Pane>, pane: Pane, ordre: &[Pane]) -> usize {
    let rang = |p: Pane| ordre.iter().position(|o| *o == p);
    let at = match rang(pane) {
        None => liste.len(),
        Some(r) => match (liste.iter().rposition(|p| rang(*p).is_some_and(|a| a < r)), liste.iter().position(|p| rang(*p).is_some_and(|a| a > r))) {
            (Some(i), _) => i + 1,
            (None, Some(j)) => j,
            (None, None) => liste.len(),
        },
    };
    liste.insert(at, pane);
    at
}

/// Une hauteur tirée à la main, ramenée dans ses bornes.
pub fn borner_hauteur(h: f32) -> f32 {
    if h.is_finite() { h.clamp(HAUTEUR_MIN, HAUTEUR_MAX) } else { HAUTEUR_DEFAUT }
}

/// Où tombe un panneau qu'on glisse, le pointeur à `y` : devant le premier panneau dont il est au-dessus du milieu, sinon
/// tout en bas (`tranches` : le haut et le bas de chaque panneau à l'écran, de haut en bas). La ligne de dépôt se trace là.
pub fn index_de_depot(tranches: &[(f32, f32)], y: f32) -> usize {
    tranches.iter().position(|(haut, bas)| y < (haut + bas) * 0.5).unwrap_or(tranches.len())
}

/// La place, dans la pile une fois le panneau retiré, d'un panneau glissé de `de` et lâché devant `cible` (de 0 au nombre de
/// panneaux, ce que rend `index_de_depot`).
pub fn place_apres_depot(de: usize, cible: usize) -> usize {
    if cible > de { cible - 1 } else { cible }
}

/// Ce que fait ✕ sur un panneau.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fermeture {
    /// Il est fermé.
    Ferme,
    /// Sorti des onglets de Propriétés : il y est revenu.
    RevenuEnOnglet,
    /// Rien (pas de tel panneau).
    Rien,
}

/// Une ligne du menu « + Panneau ».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Proposition {
    /// Rien : Propriétés ; sinon ce panneau.
    pub pane: Option<Pane>,
    /// Déjà dans l'espace de travail (la pile, un onglet, un panneau flottant, sa fenêtre) : grisé, « déjà là ».
    pub deja_la: bool,
}

impl Proposition {
    pub fn nom(&self) -> &'static str {
        self.pane.map_or("Propriétés", Pane::name)
    }
}

/// Les onglets de Propriétés à l'origine dans cet espace de travail (ceux qu'il retrouve quand on le rajoute par « + Panneau »).
pub fn onglets_d_origine(espace: Espace) -> Vec<Pane> {
    match espace {
        Espace::Vignettes => vec![Pane::ToolOptions, Pane::Scenario],
        Espace::Storyboard => vec![Pane::ToolOptions, Pane::Sheet, Pane::Effects, Pane::Scenario],
        Espace::Animation => vec![Pane::ToolOptions, Pane::Sheet, Pane::Effects],
        Espace::Montage => vec![Pane::Bruitages, Pane::Comments, Pane::History],
        Espace::TroisD => vec![Pane::Objet3d, Pane::Camera3d, Pane::Lumieres3d, Pane::Monde3d, Pane::Decor3d],
    }
}

/// Le menu « + Panneau » d'un espace de travail : Propriétés, puis les panneaux qui servent à ce travail (ceux que la barre
/// des panneaux proposait d'office, `barrepanneaux::panneaux_d_office`), puis les autres. Ni la feuille d'exposition hors de
/// l'animation, ni les essais (Suggestions sonores sans la case des Préférences, Mon panneau) s'ils ne sont pas déjà ouverts.
/// `present` : le panneau est déjà dans l'espace de travail ; `a_proprietes` : Propriétés est dans la pile.
pub fn propositions(espace: Espace, animation: bool, experimental: bool, a_proprietes: bool, present: &dyn Fn(Pane) -> bool) -> Vec<Proposition> {
    let mut out = vec![Proposition { pane: None, deja_la: a_proprietes }];
    let d_office = crate::barrepanneaux::panneaux_d_office(espace);
    let possible = |p: Pane| {
        !hors_colonne(p)
            && p != Pane::Tools
            && (animation || !p.animation_only())
            && (experimental || p != Pane::SuggestionsSonores || present(p))
            && (p != Pane::MonPanneau || present(p))
            // (Partie 197 : la Scène, du film jusqu'aux calques, sert au dessin du Storyboard et de l'Animation ; ni la planche des
            // Vignettes ni le Montage n'ont la colonne des calques dont elle est la vue en arbre.)
            && (p != Pane::Scene || matches!(espace, Espace::Storyboard | Espace::Animation) || present(p))
            // (Partie 198 : les panneaux de l'onglet 3D n'y sont proposés que là ; des autres, l'onglet 3D ne propose que l'Historique.)
            && (p.de_la_3d() == (espace == Espace::TroisD) || (espace == Espace::TroisD && p == Pane::History) || present(p))
    };
    let mut vus: Vec<Pane> = Vec::new();
    for p in d_office.iter().copied().chain(Pane::ALL) {
        if vus.contains(&p) || !possible(p) {
            continue;
        }
        vus.push(p);
        out.push(Proposition { pane: Some(p), deja_la: present(p) });
    }
    out
}

impl Colonne {
    pub fn new(panneaux: Vec<PanneauPile>) -> Colonne {
        Colonne { panneaux }
    }

    pub fn is_empty(&self) -> bool {
        self.panneaux.is_empty()
    }

    /// Tous les panneaux de FEANOR de la pile (onglets de Propriétés compris), de haut en bas.
    pub fn panes(&self) -> Vec<Pane> {
        self.panneaux.iter().flat_map(|p| p.panneaux.iter().copied()).collect()
    }

    pub fn contient(&self, pane: Pane) -> bool {
        self.panneaux.iter().any(|p| p.panneaux.contains(&pane))
    }

    /// Le rang du panneau Propriétés dans la pile.
    pub fn proprietes(&self) -> Option<usize> {
        self.panneaux.iter().position(PanneauPile::est_proprietes)
    }

    /// Le rang du panneau de la pile qui a `pane` (en onglet ou non).
    pub fn rang_de(&self, pane: Pane) -> Option<usize> {
        self.panneaux.iter().position(|p| p.panneaux.contains(&pane))
    }

    /// `pane` est un onglet de Propriétés.
    pub fn en_onglet_de_proprietes(&self, pane: Pane) -> bool {
        self.proprietes().is_some_and(|i| self.panneaux[i].panneaux.contains(&pane))
    }

    /// L'onglet devant de Propriétés.
    pub fn onglet_devant(&self) -> Option<Pane> {
        self.proprietes().and_then(|i| self.panneaux[i].devant())
    }

    /// Le panneau qui grandit et prend la hauteur qui reste : Propriétés s'il est là et ouvert, sinon le dernier ouvert.
    pub fn qui_grandit(&self) -> Option<usize> {
        match self.proprietes() {
            Some(i) if !self.panneaux[i].replie => Some(i),
            _ => self.panneaux.iter().rposition(|p| !p.replie),
        }
    }

    /// `pane` se voit : devant dans un panneau de la pile qui n'est pas replié.
    pub fn devant(&self, pane: Pane) -> bool {
        self.panneaux.iter().any(|p| !p.replie && p.devant() == Some(pane))
    }

    /// Range le panneau `de` à la place `vers` (bornée à la pile).
    pub fn deplacer(&mut self, de: usize, vers: usize) {
        if de >= self.panneaux.len() {
            return;
        }
        let p = self.panneaux.remove(de);
        let vers = vers.min(self.panneaux.len());
        self.panneaux.insert(vers, p);
    }

    /// ▲ : un cran plus haut.
    pub fn monter(&mut self, i: usize) {
        if i > 0 {
            self.deplacer(i, i - 1);
        }
    }

    /// ▼ : un cran plus bas.
    pub fn descendre(&mut self, i: usize) {
        if i + 1 < self.panneaux.len() {
            self.deplacer(i, i + 1);
        }
    }

    /// Le panneau glissé de `de` est lâché devant `cible` (`index_de_depot`).
    pub fn deposer(&mut self, de: usize, cible: usize) {
        self.deplacer(de, place_apres_depot(de, cible));
    }

    /// Replie ou déplie le panneau `i`.
    pub fn replier(&mut self, i: usize) {
        if let Some(p) = self.panneaux.get_mut(i) {
            p.replie = !p.replie;
        }
    }

    /// La hauteur du corps du panneau `i`, tirée à la main (bornée).
    pub fn regler_hauteur(&mut self, i: usize, h: f32) {
        if let Some(p) = self.panneaux.get_mut(i) {
            p.hauteur = Some(borner_hauteur(h));
        }
    }

    /// Les hauteurs tirées à la main sont oubliées (« Réajuster les panneaux ») : chaque panneau reprend celle de son contenu.
    pub fn oublier_les_hauteurs(&mut self) {
        for p in &mut self.panneaux {
            p.hauteur = None;
        }
    }

    /// Met devant `pane` dans son panneau de la pile, sans le déplier. Faux s'il n'y est pas.
    pub fn mettre_devant(&mut self, pane: Pane) -> bool {
        let Some(i) = self.rang_de(pane) else { return false };
        let p = &mut self.panneaux[i];
        p.actif = p.panneaux.iter().position(|q| *q == pane).unwrap_or(0);
        true
    }

    /// Met devant `pane` dans son panneau de la pile, et le déplie. Faux s'il n'y est pas.
    pub fn montrer(&mut self, pane: Pane) -> bool {
        if !self.mettre_devant(pane) {
            return false;
        }
        if let Some(i) = self.rang_de(pane) {
            self.panneaux[i].replie = false;
        }
        true
    }

    /// ↗ « Sortir l'onglet en panneau » : l'onglet devant de Propriétés devient un panneau à lui, juste au-dessus de
    /// Propriétés, et quitte les onglets. Rend le panneau sorti.
    pub fn sortir_l_onglet(&mut self) -> Option<Pane> {
        let i = self.proprietes()?;
        let props = &mut self.panneaux[i];
        let pane = props.devant()?;
        let rang = props.panneaux.iter().position(|p| *p == pane)?;
        props.panneaux.remove(rang);
        props.actif = rang.min(props.panneaux.len().saturating_sub(1));
        self.panneaux.insert(i, PanneauPile { sorti: true, ..PanneauPile::simple(pane) });
        Some(pane)
    }

    /// Remet `pane` en onglet de Propriétés, à son rang ; l'onglet devant reste devant. Faux sans Propriétés.
    fn en_onglet(&mut self, pane: Pane) -> bool {
        let Some(i) = self.proprietes() else { return false };
        let props = &mut self.panneaux[i];
        if !props.panneaux.contains(&pane) {
            let devant = props.devant();
            let at = ranger(&mut props.panneaux, pane, &ORDRE_DES_ONGLETS);
            props.actif = devant.and_then(|d| props.panneaux.iter().position(|q| *q == d)).unwrap_or(at);
        }
        true
    }

    /// ✕ sur le panneau `i` : sorti des onglets de Propriétés, il y revient (devant) ; sinon il est fermé.
    pub fn fermer_panneau(&mut self, i: usize) -> Fermeture {
        let Some(p) = self.panneaux.get(i) else { return Fermeture::Rien };
        if p.sorti && !p.est_proprietes() && self.proprietes().is_some() {
            let p = self.panneaux.remove(i);
            let devant = p.devant();
            for pane in &p.panneaux {
                self.en_onglet(*pane);
            }
            if let Some(d) = devant {
                self.montrer(d);
            }
            return Fermeture::RevenuEnOnglet;
        }
        self.panneaux.remove(i);
        Fermeture::Ferme
    }

    /// Retire `pane` de la pile (un onglet de Propriétés, ou un panneau qui part avec lui s'il était seul). Propriétés reste,
    /// même sans onglet. Vrai s'il y était.
    pub fn retirer(&mut self, pane: Pane) -> bool {
        let Some(i) = self.rang_de(pane) else { return false };
        let p = &mut self.panneaux[i];
        let devant = p.devant();
        p.panneaux.retain(|q| *q != pane);
        p.actif = devant.and_then(|d| p.panneaux.iter().position(|q| *q == d)).unwrap_or(0);
        if p.panneaux.is_empty() && !p.est_proprietes() {
            self.panneaux.remove(i);
        }
        true
    }

    /// Range `pane` dans la pile (`DockLayout::open` : sa commande, Affichage > Panneaux, la palette, la fin d'une fenêtre à part) :
    /// un panneau de réglages devient un onglet de Propriétés, à son rang ; la Couleur et les Brosses dédiées se rangent
    /// ensemble ; la Planche, les icônes des outils et la Scène (partie 197) arrivent en haut de la pile ; le reste (et tout, sans Propriétés) arrive en
    /// bas. La Couleur sans son panneau revient en haut (sous la Planche), comme à l'origine. Ce qui était devant le reste :
    /// `App::toggle_pane` met devant ce qu'on ouvre (`montrer`), comme egui_dock le fait d'un onglet.
    pub fn ouvrir(&mut self, pane: Pane) {
        if hors_colonne(pane) || self.contient(pane) {
            return;
        }
        if en_onglet(pane) && self.en_onglet(pane) {
            return;
        }
        if FAMILLE_COULEUR.contains(&pane) {
            if let Some(p) = self.panneaux.iter_mut().find(|p| !p.est_proprietes() && p.panneaux.iter().any(|q| FAMILLE_COULEUR.contains(q))) {
                let devant = p.devant();
                let at = ranger(&mut p.panneaux, pane, &FAMILLE_COULEUR);
                p.actif = devant.and_then(|d| p.panneaux.iter().position(|q| *q == d)).unwrap_or(at);
                return;
            }
            let haut = self.panneaux.iter().take_while(|p| p.panneaux.iter().any(|q| matches!(q, Pane::Vignettes | Pane::Tools))).count();
            self.panneaux.insert(haut, PanneauPile::simple(pane));
            return;
        }
        if matches!(pane, Pane::Vignettes | Pane::Tools | Pane::Scene | Pane::Scene3d) {
            self.panneaux.insert(0, PanneauPile::simple(pane));
            return;
        }
        self.panneaux.push(PanneauPile::simple(pane));
    }

    /// « + Panneau » : `pane` arrive en bas de la pile, dans son panneau à lui (même un panneau de réglages).
    pub fn ajouter(&mut self, pane: Pane) {
        if hors_colonne(pane) || self.contient(pane) {
            return;
        }
        self.panneaux.push(PanneauPile::simple(pane));
    }

    /// « + Panneau » > Propriétés : il arrive en bas de la pile avec ces onglets (ceux d'origine de l'espace de travail, moins
    /// ceux qui sont déjà ailleurs : `deja`).
    pub fn ajouter_proprietes(&mut self, onglets: &[Pane], deja: &dyn Fn(Pane) -> bool) {
        if self.proprietes().is_some() {
            return;
        }
        let onglets: Vec<Pane> = onglets.iter().copied().filter(|p| !hors_colonne(*p) && !deja(*p) && !self.contient(*p)).collect();
        self.panneaux.push(PanneauPile::proprietes(onglets));
    }

    /// Une pile lue dans les réglages, remise d'aplomb : chaque panneau une seule fois (ni ceux de `seen`, déjà ailleurs, ni
    /// ceux qui ne vont pas dans la colonne), un seul Propriétés (les onglets d'un deuxième le rejoignent), plus de panneau
    /// vide (sauf Propriétés), un rang devant qui existe, des hauteurs dans leurs bornes.
    pub fn nettoyee(self, seen: &mut Vec<Pane>) -> Colonne {
        let mut out: Vec<PanneauPile> = Vec::new();
        let mut props: Option<usize> = None;
        for mut p in self.panneaux {
            let devant = p.devant();
            let mut gardes = Vec::new();
            for pane in std::mem::take(&mut p.panneaux) {
                if !hors_colonne(pane) && !seen.contains(&pane) {
                    seen.push(pane);
                    gardes.push(pane);
                }
            }
            p.actif = devant.and_then(|d| gardes.iter().position(|q| *q == d)).unwrap_or(0);
            p.panneaux = gardes;
            p.hauteur = p.hauteur.filter(|h| h.is_finite()).map(borner_hauteur);
            if p.est_proprietes() {
                p.sorti = false;
                if let Some(i) = props {
                    out[i].panneaux.extend(p.panneaux);
                    continue;
                }
                props = Some(out.len());
            } else if p.panneaux.is_empty() {
                continue;
            }
            out.push(p);
        }
        Colonne { panneaux: out }
    }

    /// Deux piles égales, aux hauteurs près (elles bougent d'un rien quand on tire un bord).
    pub fn proche_de(&self, autre: &Colonne) -> bool {
        self.panneaux.len() == autre.panneaux.len()
            && self.panneaux.iter().zip(&autre.panneaux).all(|(a, b)| {
                a.sorte == b.sorte
                    && a.panneaux == b.panneaux
                    && a.devant() == b.devant()
                    && a.replie == b.replie
                    && a.sorti == b.sorti
                    && match (a.hauteur, b.hauteur) {
                        (Some(x), Some(y)) => (x - y).abs() < 1.0,
                        (None, None) => true,
                        _ => false,
                    }
            })
    }
}

/// Ce que demande le corps d'un panneau de la pile (partie 196.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Voulue {
    /// Sa hauteur, en points.
    pub h: f32,
    /// Ce qu'il garde au moins : `h` pour la hauteur de son contenu (la Couleur, la Planche…) ou celle qu'on a tirée à la main
    /// (jamais moins, la colonne défile plutôt) ; moins pour un panneau qui ne sait pas dire la sienne (une liste, à `HAUTEUR_DEFAUT`
    /// d'origine) : il rapetisse jusque-là si celui qui grandit passerait sous `GRANDIT_MIN`.
    pub plancher: f32,
}

impl Voulue {
    /// La hauteur d'un contenu, ou tirée à la main : gardée.
    pub fn ferme(h: f32) -> Voulue {
        Voulue { h, plancher: h }
    }
    /// La hauteur d'origine d'une liste : elle cède si la place manque, jusqu'à `HAUTEUR_MIN`.
    pub fn souple(h: f32) -> Voulue {
        Voulue { h, plancher: HAUTEUR_MIN.min(h) }
    }
    /// La hauteur d'origine d'une liste qui a une partie fixe en haut (l'aperçu des Préréglages) : elle cède jusqu'à `plancher`.
    pub fn souple_au_moins(h: f32, plancher: f32) -> Voulue {
        let plancher = plancher.max(HAUTEUR_MIN.min(h));
        Voulue { h: h.max(plancher), plancher }
    }
}

/// La hauteur du corps de chaque panneau de la pile, dans une colonne de `total` points (en-têtes et bords compris) :
/// `voulues[i]` est ce que demande le panneau `i` (`Voulue`), rien s'il est replié.
///
/// Partie 196.5 (la Couleur écrasée à 55 points sous Propriétés et au-dessus de Préréglages) : un panneau qui dit la hauteur de son
/// contenu, ou dont on a tiré la hauteur, la garde toujours ; quand elle ne tient pas, **la colonne défile**, aucun panneau n'est
/// coupé. Celui qui grandit (`grandit`, Propriétés d'ordinaire) prend ce qui reste, au moins `GRANDIT_MIN` ; pour qu'il les ait, les
/// listes à leur hauteur d'origine rapetissent d'abord (au prorata, jusqu'à `HAUTEUR_MIN` chacune). (Partie 195 : il gardait le tiers
/// de la place, de force, et les autres se partageaient le reste au prorata : c'est ce qui écrasait la Couleur.)
pub fn hauteurs(total: f32, voulues: &[Option<Voulue>], grandit: Option<usize>) -> Vec<f32> {
    let n = voulues.len();
    let grandit = grandit.filter(|g| *g < n && voulues[*g].is_some());
    let ouverts = voulues.iter().filter(|v| v.is_some()).count();
    let fixe = n as f32 * ENTETE + ouverts.saturating_sub(usize::from(grandit.is_some())) as f32 * BORD;
    let reste = (total - fixe).max(0.0);
    let autres = || voulues.iter().enumerate().filter(move |(i, _)| Some(*i) != grandit).filter_map(|(_, v)| *v);
    let demandes: f32 = autres().map(|v| v.h.max(0.0)).sum();
    // Les listes cèdent ce qui manque à celui qui grandit, pas plus, au prorata de ce qu'elles peuvent céder, jamais sous leur plancher.
    // (Audit du 2026-11-21 : celui qui grandit et qui dit la hauteur de son contenu, les réglages de la caméra par exemple, la reçoit tant
    // que les listes peuvent la céder ; avant, il n'avait que `GRANDIT_MIN` d'assuré et défilait à côté d'une liste à moitié vide.)
    let besoin = grandit.and_then(|g| voulues[g]).filter(|v| v.plancher >= v.h).map_or(GRANDIT_MIN, |v| v.h.max(GRANDIT_MIN));
    let manque = if grandit.is_some() { (besoin - (reste - demandes)).max(0.0) } else { 0.0 };
    let cedable: f32 = autres().map(|v| (v.h - v.plancher).max(0.0)).sum();
    let part = if cedable > 0.0 { (manque / cedable).min(1.0) } else { 0.0 };
    let mut out: Vec<f32> = voulues
        .iter()
        .enumerate()
        .map(|(i, v)| match v {
            Some(_) if Some(i) == grandit => 0.0,
            Some(v) => (v.h - (v.h - v.plancher).max(0.0) * part).max(v.plancher.min(v.h)).max(0.0),
            None => 0.0,
        })
        .collect();
    if let Some(g) = grandit {
        let pris: f32 = out.iter().sum();
        out[g] = (reste - pris).max(GRANDIT_MIN);
    }
    out
}
