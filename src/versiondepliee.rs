//! Les versions dépliées dans la timeline (partie 167, Anthony, 2026-11-17 : « un clic sur une flèche vers la droite la déploie à
//! côté dans la timeline pour la lire et la comparer directement à l'autre version »).
//!
//! Une version rangée d'un plan (`planvers.rs`) peut se **déplier** juste après la version en cours : un bloc de plus dans la
//! bande, en lecture seule, et les plans suivants glissent **à l'affichage seulement**. Rien n'est écrit dans le projet, rien ne
//! s'annule : le montage, les durées, les exports, l'animatique ne voient que le film (`Document`). C'est une carte entre deux
//! suites d'images :
//!
//! - les images **du film** (`g`, celles du montage, de 0 à `total_frames`) ;
//! - les images **de l'écran** (`d`) : celles du film, avec, après la dernière image de chaque plan qui a des versions dépliées,
//!   les images de ces versions, l'une après l'autre (dans l'ordre de leurs numéros).
//!
//! Tout ce que la bande dessine à une image du film se pose à `d_of(g)` ; une image de l'écran se retrouve par `lieu(d)` : une
//! image du film, ou une image d'une version dépliée. La lecture du film traverse les deux (`App::depli_pas`, `versiondeplieeui.rs`).

use crate::document::Document;

/// Une version dépliée, à sa place dans la bande.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Insert {
    /// Le rang du plan dans le film.
    pub host: usize,
    /// Le plan (son numéro) et la version dépliée.
    pub plan: u64,
    pub numero: u32,
    /// L'image du film juste après le plan : la version se pose avant elle.
    pub at: usize,
    /// Le nombre d'images de la version.
    pub len: usize,
}

/// Où tombe une image de l'écran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lieu {
    /// Une image du film.
    Film(usize),
    /// L'image `frame` de la version dépliée `k` (rang dans `Carte::inserts`).
    Deplie { k: usize, frame: usize },
}

/// La carte film ↔ écran des versions dépliées.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Carte {
    /// Dans l'ordre de la bande (par plan, puis par numéro).
    pub inserts: Vec<Insert>,
}

impl Carte {
    /// La carte des versions `depliees` (plan, numéro) du document : celles qui ne sont plus des versions rangées d'un plan du
    /// film sont ignorées.
    pub fn de(doc: &Document, depliees: &[(u64, u32)]) -> Carte {
        if depliees.is_empty() {
            return Carte::default();
        }
        let mut inserts = Vec::new();
        let mut start = 0;
        for (host, p) in doc.panels.iter().enumerate() {
            let len = p.frame_count();
            start += len;
            let Some(v) = doc.versions_du_plan(p.id) else { continue };
            let mut numeros: Vec<u32> = depliees.iter().filter(|(plan, _)| *plan == p.id).map(|(_, n)| *n).collect();
            numeros.sort_unstable();
            numeros.dedup();
            for numero in numeros {
                if let Some(r) = v.rangee(numero) {
                    inserts.push(Insert { host, plan: p.id, numero, at: start, len: r.panel.frame_count().max(1) });
                }
            }
        }
        Carte { inserts }
    }

    pub fn vide(&self) -> bool {
        self.inserts.is_empty()
    }

    /// Le nombre d'images que les versions dépliées ajoutent à l'écran.
    pub fn en_plus(&self) -> usize {
        self.inserts.iter().map(|i| i.len).sum()
    }

    /// Le décalage (en images) de l'image `g` du film à l'écran : la somme des versions dépliées posées avant elle.
    pub fn decalage(&self, g: usize) -> usize {
        self.inserts.iter().take_while(|i| i.at <= g).map(|i| i.len).sum()
    }

    /// L'image de l'écran de l'image `g` du film.
    pub fn d_of(&self, g: usize) -> usize {
        g + self.decalage(g)
    }

    /// La première image de l'écran de la version dépliée `k`.
    pub fn debut(&self, k: usize) -> usize {
        let i = &self.inserts[k];
        i.at + self.inserts[..k].iter().map(|j| j.len).sum::<usize>()
    }

    /// L'image de l'écran de l'image `frame` de la version dépliée `k`.
    pub fn d_de_lieu(&self, lieu: Lieu) -> usize {
        match lieu {
            Lieu::Film(g) => self.d_of(g),
            Lieu::Deplie { k, frame } => self.debut(k) + frame.min(self.inserts[k].len.saturating_sub(1)),
        }
    }

    /// Où tombe l'image `d` de l'écran (au-delà de la fin : une image du film au-delà de la fin, que l'appelant borne).
    pub fn lieu(&self, d: usize) -> Lieu {
        let mut decale = 0;
        for (k, i) in self.inserts.iter().enumerate() {
            let debut = i.at + decale;
            if d < debut {
                return Lieu::Film(d - decale);
            }
            if d < debut + i.len {
                return Lieu::Deplie { k, frame: d - debut };
            }
            decale += i.len;
        }
        Lieu::Film(d - decale)
    }

    /// L'image du film la plus proche de l'image `d` de l'écran : la dernière du plan pour une image d'une version dépliée (les
    /// gestes de la bande qui visent une image du film, une clé glissée par exemple, ne tombent jamais dans une version dépliée).
    pub fn film_de(&self, d: usize) -> usize {
        match self.lieu(d) {
            Lieu::Film(g) => g,
            Lieu::Deplie { k, .. } => self.inserts[k].at.saturating_sub(1),
        }
    }

    /// Le nombre d'images de l'écran pour un film de `total` images.
    pub fn total(&self, total: usize) -> usize {
        total + self.en_plus()
    }

    /// Les tronçons de l'écran où le film n'est que décalé : (première image du film, image du film après la dernière, décalage).
    /// Le dernier va jusqu'à `usize::MAX` (le son peut dépasser la fin des plans).
    pub fn troncons(&self) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        let (mut g0, mut decale) = (0, 0);
        for i in &self.inserts {
            // (Plusieurs versions dépliées du même plan se suivent : un seul tronçon s'arrête avant elles.)
            if i.at > g0 {
                out.push((g0, i.at, decale));
                g0 = i.at;
            }
            decale += i.len;
        }
        out.push((g0, usize::MAX, decale));
        out
    }

    /// Le rang de la version dépliée `numero` du plan `plan`.
    pub fn rang(&self, plan: u64, numero: u32) -> Option<usize> {
        self.inserts.iter().position(|i| i.plan == plan && i.numero == numero)
    }
}
