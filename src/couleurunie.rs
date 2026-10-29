//! Le calque « Couleur unie » (Anthony, 2026-10-29 : « un calque color card, qui applique une couleur unie sur toute l'image à la
//! création du calque. La couleur est ajustable dans les paramètres du calque. On peut dessiner sur le calque color card, on peut tout
//! faire dessus. »).
//!
//! # Ce que c'est
//! Un calque de dessin ordinaire (bitmap, avec ses images, ses clés, ses effets, ses outils : tout marche dessus) qui porte en plus une
//! couleur (`Layer::unie`). Cette couleur couvre **toute la feuille**, sous les dessins du calque : un mouvement de caméra ne découvre
//! jamais de bord. Elle suit l'opacité (donc les clés d'opacité), le mode de fusion, la visibilité et la place du calque dans la pile
//! et les dossiers, comme ses dessins.
//!
//! # Léger
//! Aucun pixel n'est gardé pour le fond : le plan pose un élément `FondPart::Unie(couleur)` (`render::unie_item`) juste sous le dessin
//! du calque, et le dessin de cet élément (`fx::unie_cel`) est **un seul pixel** que la composition étire sur la feuille (par le chemin
//! des dessins d'une autre taille que la feuille, qu'ont déjà l'écran, la carte graphique, les vignettes, la lecture, les exports et le
//! PDF). Dans le projet, seule la couleur est écrite (`couleur_unie`, format 43).
//!
//! # La gomme et le fond
//! Le fond n'est pas dans le dessin : la gomme efface le dessin, jamais le fond. Changer la couleur recolore le fond sans toucher à ce
//! qu'on a dessiné par-dessus.
//!
//! # Couleur d'origine
//! Gris moyen (`DEFAUT`, 128 128 128) : visible tout de suite sur le papier blanc (le blanc ne se verrait pas : on croirait que rien
//! n'est arrivé), neutre pour une couleur qu'on va changer, et il laisse lire un trait noir comme un trait blanc.
//!
//! # Pour un import
//! `Document::ajouter_couleur_unie(plan, couleur, nom, rang)` crée le calque (sans historique) et rend son numéro.

use eframe::egui::{self, RichText};

use crate::app::App;
use crate::document::{Document, Layer, LayerKind};

/// Le gris moyen d'origine.
pub const DEFAUT: [u8; 3] = [128, 128, 128];

/// Nom du premier calque Couleur unie d'un plan.
pub const NOM: &str = "Couleur unie";

impl Layer {
    /// Un calque Couleur unie : bitmap, de la couleur `couleur` sous ses dessins, sur toute la feuille.
    pub fn new_unie(name: impl Into<String>, couleur: [u8; 3]) -> Layer {
        Layer { unie: Some(couleur), ..Layer::new(name, LayerKind::Bitmap) }
    }
}

/// « Couleur unie », puis « Couleur unie 2 »… : un nom que les calques du plan n'ont pas.
pub fn nom_libre(layers: &[Layer]) -> String {
    let pris = |n: &str| layers.iter().any(|l| l.name == n);
    if !pris(NOM) {
        return NOM.to_string();
    }
    (2..).map(|k| format!("{NOM} {k}")).find(|n| !pris(n)).expect("un nom libre")
}

impl Document {
    /// Ajoute au plan `plan` un calque Couleur unie de la couleur `couleur` (sans étape d'annulation : pour un import). `nom` : son nom
    /// (sinon « Couleur unie »), `rang` : sa place dans la pile (0 : tout en bas ; rien : tout en haut). Rend son numéro.
    pub fn ajouter_couleur_unie(&mut self, plan: usize, couleur: [u8; 3], nom: Option<&str>, rang: Option<usize>) -> Option<u64> {
        let panel = self.panels.get_mut(plan)?;
        let name = nom.map_or_else(|| nom_libre(&panel.layers), str::to_string);
        let layer = Layer::new_unie(name, couleur);
        let id = layer.id;
        let at = rang.unwrap_or(panel.layers.len()).min(panel.layers.len());
        panel.layers.insert(at, layer);
        Some(id)
    }
}

impl App {
    /// « Nouveau calque Couleur unie » : au-dessus du calque courant (dans son dossier s'il est rangé), gris moyen, une étape d'annulation.
    pub(crate) fn nouveau_calque_couleur_unie(&mut self) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let name = nom_libre(self.layers());
        let at = self.cur_layer + 1;
        let mut layer = Layer::new_unie(name.clone(), DEFAUT);
        layer.parent = self.layer().parent;
        self.layers_mut().insert(at, layer);
        self.cur_layer = at;
        self.layer_sel.clear();
        self.vsel = None;
        self.commit_structure(snap);
        self.status = format!("Calque « {name} » : toute la feuille en gris moyen ; sa couleur se change dans le panneau Calques, et on dessine par-dessus.");
    }

    /// La couleur du calque courant, dans le panneau Calques, s'il est de la sorte Couleur unie : la case de couleur commune (avec sa
    /// pipette). Une étape d'annulation par geste ; le fond est recoloré, les dessins du calque ne changent pas.
    pub(crate) fn couleur_unie_ui(&mut self, ui: &mut egui::Ui) {
        let li = self.cur_layer;
        let Some(mut couleur) = self.layers().get(li).and_then(|l| l.unie) else { return };
        ui.horizontal(|ui| {
            crate::bulles::etiquette(ui, egui_phosphor::regular::SQUARE, "Couleur unie du calque", "Le fond du calque, sur toute la feuille, sous ses dessins.");
            ui.label(RichText::new("Couleur").small());
            let resp = crate::champcouleur::srgb(ui, &mut couleur);
            if self.edited(&resp) {
                self.layers_mut()[li].unie = Some(couleur);
                self.renderer.invalidate_all();
            }
        });
    }
}
