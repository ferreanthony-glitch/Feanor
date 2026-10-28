//! « Mon panneau » (essai du menu Labo, 2026-10-28), la logique sans écran : la liste des boutons que l'utilisateur
//! range dans son panneau (des commandes du registre, des brosses de « Mes brosses », des outils perso, des
//! séparateurs), la façon de les poser sur la grille, et les réglages enregistrés. L'affichage est dans `monpanneauui.rs`.
//!
//! Ce que garde le poste (`Settings::mon_panneau`, `reglages.toml`) : jamais le projet, donc aucun changement de
//! format. Un bouton qu'une autre version ne comprend pas (un genre inconnu, une valeur abîmée) est ignoré à la
//! lecture : il ne fait ni planter ni perdre les autres réglages. Une commande qui n'existe plus au registre
//! (`keymap::ACTIONS`) est retirée de même (`Reglage::nettoye`).

use eframe::egui::{pos2, vec2, Rect, Pos2};
use serde::{Deserialize, Deserializer, Serialize};

use crate::app::Tool;
use crate::brush::BrushSettings;

/// L'action qui ouvre et ferme le panneau (menu Labo, palette Ctrl+K).
pub(crate) const ACTION: &str = "vue.mon_panneau";

/// Un outil perso : l'outil courant au moment où on l'a gardé, avec ses réglages de brosse et sa couleur. Un clic
/// remet les trois d'un coup.
#[derive(Clone, Debug, PartialEq)]
pub struct OutilPerso {
    /// Le nom qu'on lui a donné.
    pub nom: String,
    /// L'action de l'outil (`outil.pinceau`…), comme dans `Tool::action`.
    pub outil: String,
    /// Les réglages de la brosse du pinceau, du crayon ou de la gomme ; rien pour les autres outils.
    pub pinceau: Option<BrushSettings>,
    /// La couleur d'avant-plan.
    pub couleur: [u8; 3],
}

impl OutilPerso {
    /// L'outil de cette action, s'il existe encore.
    pub fn outil(&self) -> Option<Tool> {
        outil_de(&self.outil)
    }
}

/// Cette action existe-t-elle au registre des commandes (`keymap::ACTIONS`), faite et utilisable ?
pub fn commande_connue(action: &str) -> bool {
    crate::keymap::action_info(action).is_some_and(|a| a.implemented)
}

/// L'outil que désigne une action (`outil.pinceau`…).
pub fn outil_de(action: &str) -> Option<Tool> {
    Tool::ALL.into_iter().find(|t| t.action() == action)
}

/// Un bouton du panneau.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(into = "Brut")]
pub enum Bouton {
    /// Une commande du registre : un clic la lance, comme au clavier ou à la palette.
    Commande { action: String },
    /// Une brosse de la bibliothèque (le plus souvent « Mes brosses ») : un clic la prend.
    Brosse { nom: String, source: String, gomme: bool },
    /// Un outil perso.
    Outil(OutilPerso),
    /// Un trait entre deux groupes de boutons.
    Separateur,
    /// Un genre que cette version ne connaît pas : jamais gardé (`Reglage::nettoye`).
    Inconnu,
}

/// La forme écrite d'un bouton, à plat : une table de `reglages.toml`, tolérante (tout champ manquant a une valeur).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Brut {
    genre: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    nom: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    source: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    gomme: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    outil: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    couleur: Option<[u8; 3]>,
    // (Le pinceau, une table, en dernier : TOML veut les valeurs simples avant les tables.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pinceau: Option<BrushSettings>,
}

const COMMANDE: &str = "commande";
const BROSSE: &str = "brosse";
const OUTIL: &str = "outil";
const SEPARATEUR: &str = "separateur";

impl From<Bouton> for Brut {
    fn from(b: Bouton) -> Brut {
        match b {
            Bouton::Commande { action } => Brut { genre: COMMANDE.into(), id: action, ..Brut::default() },
            Bouton::Brosse { nom, source, gomme } => Brut { genre: BROSSE.into(), id: nom, source, gomme, ..Brut::default() },
            Bouton::Outil(o) => Brut { genre: OUTIL.into(), nom: o.nom, outil: o.outil, couleur: Some(o.couleur), pinceau: o.pinceau, ..Brut::default() },
            Bouton::Separateur => Brut { genre: SEPARATEUR.into(), ..Brut::default() },
            Bouton::Inconnu => Brut::default(),
        }
    }
}

impl From<Brut> for Bouton {
    fn from(b: Brut) -> Bouton {
        match b.genre.as_str() {
            COMMANDE if !b.id.is_empty() => Bouton::Commande { action: b.id },
            BROSSE if !b.id.is_empty() => Bouton::Brosse { nom: b.id, source: b.source, gomme: b.gomme },
            OUTIL if !b.outil.is_empty() => Bouton::Outil(OutilPerso { nom: b.nom, outil: b.outil, pinceau: b.pinceau, couleur: b.couleur.unwrap_or([0, 0, 0]) }),
            SEPARATEUR => Bouton::Separateur,
            _ => Bouton::Inconnu,
        }
    }
}

/// Lit la liste des boutons une table après l'autre : une table abîmée ou d'un genre inconnu est laissée de côté, sans
/// faire échouer la lecture de tous les réglages.
fn boutons_tolerants<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Bouton>, D::Error> {
    let tables: Vec<toml::Value> = Vec::deserialize(d)?;
    Ok(tables.into_iter().filter_map(|t| t.try_into::<Brut>().ok()).map(Bouton::from).filter(|b| *b != Bouton::Inconnu).collect())
}

/// Ce que le poste retient de Mon panneau.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reglage {
    /// Montrer le nom sous chaque icône (sinon : les icônes seules).
    #[serde(rename = "noms", skip_serializing_if = "std::ops::Not::not")]
    pub noms: bool,
    /// Les boutons, dans l'ordre de la grille.
    #[serde(rename = "boutons", skip_serializing_if = "Vec::is_empty", deserialize_with = "boutons_tolerants")]
    pub boutons: Vec<Bouton>,
}

impl Reglage {
    /// Rien à écrire : le panneau d'origine (vide, icônes seules).
    pub fn est_vide(&self) -> bool {
        !self.noms && self.boutons.is_empty()
    }

    /// Retire ce qu'on ne peut plus montrer : un genre inconnu, une commande qui n'existe plus ou n'est pas faite
    /// (`connue` dit si une action existe), un outil perso dont l'outil a disparu, un séparateur de trop (en tête, ou
    /// deux de suite).
    pub fn nettoye(self, connue: impl Fn(&str) -> bool) -> Reglage {
        let mut boutons: Vec<Bouton> = Vec::new();
        for b in self.boutons {
            let garde = match &b {
                Bouton::Commande { action } => connue(action),
                Bouton::Brosse { nom, .. } => !nom.is_empty(),
                Bouton::Outil(o) => o.outil().is_some(),
                Bouton::Separateur => !matches!(boutons.last(), None | Some(Bouton::Separateur)),
                Bouton::Inconnu => false,
            };
            if garde {
                boutons.push(b);
            }
        }
        Reglage { boutons, ..self }
    }

    /// La commande est-elle déjà dans le panneau ?
    pub fn contient_commande(&self, action: &str) -> bool {
        self.boutons.iter().any(|b| matches!(b, Bouton::Commande { action: a } if a == action))
    }

    /// La brosse est-elle déjà dans le panneau ?
    pub fn contient_brosse(&self, nom: &str, source: &str, gomme: bool) -> bool {
        self.boutons.iter().any(|b| matches!(b, Bouton::Brosse { nom: n, source: s, gomme: g } if n == nom && s == source && *g == gomme))
    }

    /// Ajoute un bouton à la fin ; faux (rien n'est ajouté) si c'est une commande ou une brosse déjà là.
    pub fn ajouter(&mut self, b: Bouton) -> bool {
        let deja = match &b {
            Bouton::Commande { action } => self.contient_commande(action),
            Bouton::Brosse { nom, source, gomme } => self.contient_brosse(nom, source, *gomme),
            Bouton::Inconnu => true,
            Bouton::Outil(_) | Bouton::Separateur => false,
        };
        if !deja {
            self.boutons.push(b);
        }
        !deja
    }

    /// Retire le bouton de rang `i`.
    pub fn retirer(&mut self, i: usize) -> Option<Bouton> {
        (i < self.boutons.len()).then(|| self.boutons.remove(i))
    }

    /// Déplace le bouton de rang `de` : il se retrouve au rang `vers` de la liste obtenue. Faux si rien ne change.
    pub fn deplacer(&mut self, de: usize, vers: usize) -> bool {
        if de >= self.boutons.len() || vers >= self.boutons.len() || de == vers {
            return false;
        }
        let b = self.boutons.remove(de);
        self.boutons.insert(vers, b);
        true
    }

    /// Un séparateur juste après le bouton de rang `i` (pas contre un autre séparateur, pas tout au bout). Son rang.
    pub fn separateur_apres(&mut self, i: usize) -> Option<usize> {
        let n = self.boutons.len();
        if i >= n || self.boutons[i] == Bouton::Separateur || i + 1 == n || self.boutons[i + 1] == Bouton::Separateur {
            return None;
        }
        self.boutons.insert(i + 1, Bouton::Separateur);
        Some(i + 1)
    }

    /// Donne un nom à l'outil perso de rang `i` (un nom vide ne change rien). Vrai s'il a changé.
    pub fn renommer(&mut self, i: usize, nom: &str) -> bool {
        let nom = nom.trim();
        match self.boutons.get_mut(i) {
            Some(Bouton::Outil(o)) if !nom.is_empty() && o.nom != nom => {
                o.nom = nom.to_string();
                true
            }
            _ => false,
        }
    }

    /// Un nom libre pour un outil perso de plus : « Pinceau perso », « Pinceau perso 2 »…
    pub fn nom_libre(&self, outil: Tool) -> String {
        let base = format!("{} perso", outil.label());
        let pris = |n: &str| self.boutons.iter().any(|b| matches!(b, Bouton::Outil(o) if o.nom == n));
        if !pris(&base) {
            return base;
        }
        (2..).map(|k| format!("{base} {k}")).find(|n| !pris(n)).unwrap_or(base)
    }
}

// ---------------------------------------------------------------------------------------------------- la grille

/// Largeur d'un séparateur sur la grille.
pub(crate) const LARGEUR_SEPARATEUR: f32 = 10.0;

/// Pose des cases (largeur de chacune, hauteur commune, écart) sur des lignes de `place` points au plus, de gauche à
/// droite, à la ligne quand la suivante ne tient plus. `separateurs` dit lesquelles sont des séparateurs : jamais en
/// début de ligne (sans largeur alors), et ils ne font jamais passer à la ligne. Rend le rectangle de chaque case (origine
/// en haut à gauche de la grille) et la hauteur totale.
pub(crate) fn disposer(largeurs: &[f32], separateurs: &[bool], hauteur: f32, ecart: f32, place: f32) -> (Vec<Rect>, f32) {
    let mut rects = Vec::with_capacity(largeurs.len());
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let mut rangees = if largeurs.is_empty() { 0 } else { 1 };
    for (i, &l) in largeurs.iter().enumerate() {
        let sep = separateurs.get(i).copied().unwrap_or(false);
        if sep {
            // Un séparateur ne commence pas une ligne et ne la fait pas passer : sans place, il disparaît.
            let visible = x > 0.0 && x + l <= place;
            let l = if visible { l } else { 0.0 };
            rects.push(Rect::from_min_size(pos2(x, y), vec2(l, hauteur)));
            x += l;
            continue;
        }
        if x > 0.0 && x + l > place {
            x = 0.0;
            y += hauteur + ecart;
            rangees += 1;
        }
        rects.push(Rect::from_min_size(pos2(x, y), vec2(l, hauteur)));
        x += l + ecart;
    }
    let total = if rangees == 0 { 0.0 } else { rangees as f32 * hauteur + (rangees - 1) as f32 * ecart };
    (rects, total)
}

/// Le rang de la case (visible) la plus proche de `p` : celle qui le contient, sinon celle dont le centre est le plus
/// près.
pub(crate) fn case_la_plus_proche(rects: &[Rect], p: Pos2) -> Option<usize> {
    let ok = |i: &usize| rects[*i].width() > 0.0;
    if let Some(i) = (0..rects.len()).filter(ok).find(|&i| rects[i].contains(p)) {
        return Some(i);
    }
    (0..rects.len()).filter(ok).min_by(|&a, &b| rects[a].center().distance(p).total_cmp(&rects[b].center().distance(p)))
}
