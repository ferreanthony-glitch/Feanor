//! Le décor des Vignettes (Anthony, 2026-10-22) : un calque « Décor » par plan, fait par les brosses du décor, sous les
//! personnages, et partagé par toutes les cases d'une continuité.
//!
//! # Ce que c'est
//!
//! - **Un calque comme les autres, marqué `Layer::decor`.** Bitmap, tout en bas du plan (sous les calques des personnages et
//!   des traits), nommé « Décor ». Il est créé par le premier trait d'une brosse du décor (`decorui.rs`) et n'existe pas avant.
//!   Ce n'est pas l'ancien « calque de fond », retiré le 2026-10-21 : celui-là était un calque de toute la scène ; celui-ci
//!   est un calque ordinaire, un par plan, que seul un drapeau distingue.
//! - **Un seul dessin, exposé sur tout le plan.** Les cases d'un plan sont ses images (la première, puis les marques d'image) :
//!   le calque Décor tient *le même dessin* sur toutes les images du plan. Un trait ajouté dans une case se voit donc dans toutes
//!   celles de la continuité (c'est le même dessin), et les vignettes de ces cases se refont par la clé de leur cache (la version du
//!   dessin y est). Les autres calques gardent leurs dessins, case par case : personnages et traits ne sont jamais reportés.
//!   Le calque a le post-comportement « Maintenir » (`Behaviour::Hold`) : si le plan s'allonge, le décor suit.
//! - **Une coupe cochée (le plan se coupe) donne au plan suivant sa propre copie du décor** : `Document::split_panel` copie les
//!   dessins de la partie coupée, comme pour tout calque. Éditer l'un ne touche pas l'autre.
//! - **Une coupe décochée (les plans se réunissent)** : le décor du premier plan se tient sur tout le plan réuni (« Report du
//!   décor », allumé d'office) ; éteint, la case suivante repart d'un décor vide (ou de son propre décor, s'il en avait un).
//!   Le réglage n'est lu qu'à la jonction : ce qui est déjà réuni ne change pas quand on le bascule.
//!
//! Aucun changement de format de fichier : le calque s'écrit comme les autres, avec un champ `decor` de plus (absent : faux).
//! Une ancienne version l'ouvre comme un calque ordinaire nommé « Décor ».

use serde::{Deserialize, Serialize};

use crate::brush::BrushSettings;
use crate::document::{Behaviour, Document, Layer, LayerKind};

/// Le nom du calque.
pub const NOM: &str = "Décor";
/// Le gris-bleu clair des brosses du décor, pour que le décor se lise à part des personnages d'un coup d'œil.
pub const COULEUR_DEFAUT: [u8; 3] = [0x8F, 0xA3, 0xB8];

/// Une brosse du décor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Genre {
    Crayon,
    Feutre,
    Aplat,
}

impl Genre {
    pub const ALL: [Genre; 3] = [Genre::Crayon, Genre::Feutre, Genre::Aplat];

    pub fn nom(self) -> &'static str {
        match self {
            Genre::Crayon => "Crayon du décor",
            Genre::Feutre => "Feutre gris du décor",
            Genre::Aplat => "Aplat du décor",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Genre::Crayon => "Un crayon fin pour esquisser le décor. Ses traits vont toujours dans le calque Décor, partagé par les cases du plan.",
            Genre::Feutre => "Un feutre large et translucide pour poser les valeurs du décor. Ses traits vont toujours dans le calque Décor, partagé par les cases du plan.",
            Genre::Aplat => "Une grande brosse pleine pour les masses du décor. Ses traits vont toujours dans le calque Décor, partagé par les cases du plan.",
        }
    }

    /// La taille d'origine, en pixels du dessin (Préférences > Vignettes).
    pub fn taille_defaut(self) -> f32 {
        match self {
            Genre::Crayon => 5.0,
            Genre::Feutre => 24.0,
            Genre::Aplat => 90.0,
        }
    }

    /// Le rang dans les réglages (`ReglagesVignettes::tailles_decor`).
    pub fn rang(self) -> usize {
        match self {
            Genre::Crayon => 0,
            Genre::Feutre => 1,
            Genre::Aplat => 2,
        }
    }

    /// Le crayon prend l'outil Crayon, le feutre et l'aplat l'outil Pinceau.
    pub fn est_crayon(self) -> bool {
        self == Genre::Crayon
    }

    /// Le réglage de la brosse, à la taille `taille`.
    pub fn reglage(self, taille: f32) -> BrushSettings {
        let base = match self {
            Genre::Crayon => BrushSettings::graphite(),
            Genre::Feutre => BrushSettings::felt(),
            // Pleine : un bord net, une opacité entière, une taille qui ne suit pas l'appui.
            Genre::Aplat => BrushSettings { hardness: 1.0, opacity: 1.0, pressure_size: false, smoothing: 0.2, ..BrushSettings::brush() },
        };
        BrushSettings { size: taille.clamp(0.5, 400.0), ..base }
    }
}

impl Layer {
    /// Le calque Décor d'un plan : bitmap, un seul dessin tenu jusqu'au bout du plan (et plus loin si le plan s'allonge).
    pub fn new_decor() -> Layer {
        Layer { decor: true, post: Behaviour::Hold, auto_break: false, ..Layer::new(NOM, LayerKind::Bitmap) }
    }
}

/// Ce qu'une jonction de plans doit savoir du décor du second (`Document::decor_avant_jonction`).
pub(crate) struct Jonction {
    a_len: usize,
    b_len: usize,
    /// Le calque Décor du second plan, retiré de lui avant la jonction.
    b: Option<Layer>,
}

impl Document {
    /// Le rang du calque Décor du plan `index`, s'il en a un.
    pub fn decor_de(&self, index: usize) -> Option<usize> {
        self.panels.get(index)?.layers.iter().position(|l| l.decor)
    }

    /// Le plan `index` prêt à recevoir un trait de décor à l'image `frame` : son calque Décor existe (en bas du plan) et montre un
    /// dessin à cette image. Rend le rang du calque et si quelque chose a changé (calque créé, dessin créé). Un dessin neuf couvre
    /// toute la plage d'images vides qui contient `frame` : tout le plan, d'ordinaire ; la case seule, si « Report du décor » est
    /// éteint et que le calque est encore vide.
    pub fn assurer_decor(&mut self, index: usize, frame: usize) -> Option<(usize, bool)> {
        let n = self.panels.get(index)?.frame_count();
        let frame = frame.min(n - 1);
        let mut changed = false;
        let at = match self.decor_de(index) {
            Some(i) => i,
            None => {
                self.panels[index].layers.insert(0, Layer::new_decor());
                changed = true;
                0
            }
        };
        if self.panels[index].layers[at].shown_at(frame).is_none() {
            let cel = self.new_cel(LayerKind::Bitmap);
            let (mut a, mut b) = {
                let l = &self.panels[index].layers[at];
                let mut a = frame;
                while a > 0 && l.cel_at(a - 1).is_none() {
                    a -= 1;
                }
                let mut b = frame + 1;
                while b < n && l.cel_at(b).is_none() {
                    b += 1;
                }
                (a, b)
            };
            let seule = self.panels[index].layers[at].frames.iter().all(Option::is_none);
            if seule && !self.report_decor {
                let (ca, cb) = self.case_de(index, frame);
                (a, b) = (a.max(ca), b.min(cb));
            }
            let layer = &mut self.panels[index].layers[at];
            for f in a..b {
                layer.set(f, Some(cel));
            }
            changed = true;
        }
        Some((at, changed))
    }

    /// La case (image du storyboard) qui contient l'image `frame` du plan `index` : de sa marque (ou du début du plan) à la
    /// marque suivante (ou à la fin du plan).
    fn case_de(&self, index: usize, frame: usize) -> (usize, usize) {
        let p = &self.panels[index];
        let start = p.marks.iter().rev().find(|m| m.frame <= frame).map_or(0, |m| m.frame);
        let end = p.marks.iter().find(|m| m.frame > frame).map_or(p.frame_count(), |m| m.frame);
        (start, end)
    }

    /// Une case de plus au bout du plan `index`, à partir de l'image `from` (« Report du décor » éteint : la case repart d'un décor vide,
    /// au lieu de tenir celui d'avant). Rien si le plan n'a pas de décor, ou si le report est allumé.
    pub fn decor_case_ajoutee(&mut self, index: usize, from: usize) {
        if self.report_decor {
            return;
        }
        let Some(at) = self.decor_de(index) else { return };
        let n = self.panels[index].frame_count();
        if from == 0 || from >= n || self.panels[index].layers[at].shown_at(from - 1).is_none() {
            return;
        }
        let vide = self.new_cel(LayerKind::Bitmap);
        let l = &mut self.panels[index].layers[at];
        for f in from..n {
            l.set(f, Some(vide));
        }
    }

    /// Avant de réunir le plan `index` et le suivant : le calque Décor du second est mis de côté (la jonction ordinaire,
    /// qui réunit les calques de même nom, n'y touche pas ; `decor_apres_jonction` en fait ce qu'il faut).
    pub(crate) fn decor_avant_jonction(&mut self, index: usize) -> Jonction {
        let a_len = self.panels[index].frame_count();
        let b = &mut self.panels[index + 1];
        let b_len = b.frame_count();
        // (La durée du second plan ne dépend plus de son décor, qui part.)
        b.length = b_len;
        let layer = b.layers.iter().position(|l| l.decor).map(|k| b.layers.remove(k));
        Jonction { a_len, b_len, b: layer }
    }

    /// Après la jonction (le plan réuni est à `index`) : le décor du plan réuni.
    /// - « Report du décor » allumé : le décor du premier plan est tenu sur toute la suite (celui du second plan, s'il en avait un, n'est
    ///   gardé que si le premier n'avait rien à la jonction) ;
    /// - éteint : la suite garde son propre décor ; sans décor à elle, elle repart d'un dessin vide.
    pub(crate) fn decor_apres_jonction(&mut self, index: usize, j: Jonction) {
        let report = self.report_decor;
        let Jonction { a_len, b_len, b } = j;
        let Some(ka) = self.decor_de(index) else {
            // Le premier plan n'avait pas de décor : celui du second, s'il en avait un, passe en bas du plan réuni, à sa place dans le temps.
            if let Some(mut lb) = b {
                lb.frames.truncate(b_len);
                let mut frames = vec![None; a_len];
                frames.extend(lb.frames);
                lb.frames = frames;
                lb.roles.clear();
                lb.trim();
                // (Tout en bas du plan, hors de tout dossier : le dossier où le second plan l'avait rangé n'est plus juste au-dessus,
                // et la pile était en désordre — un calque rangé loin de son dossier, que la lecture du projet remettait ailleurs.
                // Passe de débogage au hasard, 2026-10-23.)
                lb.parent = None;
                self.panels[index].layers.insert(0, lb);
            }
            return;
        };
        let a_last = self.panels[index].layers[ka].shown_at(a_len - 1);
        // Un dessin vide pour la suite, si elle ne doit rien montrer du décor d'avant.
        let vide = (!report && b.is_none() && a_last.is_some()).then(|| self.new_cel(LayerKind::Bitmap));
        let l = &mut self.panels[index].layers[ka];
        // Le début du calque tel qu'il se voyait (le « Maintenir » prolongeait son dernier dessin jusqu'à la jonction).
        l.frames.resize(a_len, None);
        if let Some(c) = a_last {
            let dernier = l.frames.iter().rposition(Option::is_some).map_or(0, |k| k + 1);
            for f in dernier..a_len {
                l.frames[f] = Some(c);
            }
        }
        match (report, a_last, b) {
            (true, Some(c), _) => l.frames.extend(std::iter::repeat_n(Some(c), b_len)),
            (_, _, Some(lb)) => {
                let mut suite = lb.frames;
                suite.truncate(b_len);
                l.frames.extend(suite);
            }
            _ => {
                if let Some(v) = vide {
                    l.frames.extend(std::iter::repeat_n(Some(v), b_len));
                }
            }
        }
        l.trim();
    }

    /// Réunit le plan `index` et le suivant en tenant compte du décor (`decor_avant_jonction`, `decor_apres_jonction`).
    pub(crate) fn join_with_decor_with(&mut self, index: usize, opts: &crate::fusionnoms::JoinOpts) {
        let j = self.decor_avant_jonction(index);
        self.append_next_with(index, opts);
        self.decor_apres_jonction(index, j);
    }
}
