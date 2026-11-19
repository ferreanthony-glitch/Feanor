//! Le budget 3D de **cette machine** (partie 180) : jusqu'où un décor peut aller sans que FEANOR ralentisse.
//!
//! Anthony : « Je ne veux pas que le logiciel crash ou ralentisse à cause de fichiers 3D trop lourds. Pas de compromis avec les
//! ralentissements », puis « que les seuils soient mesurés en fonction de la config machine sur laquelle FEANOR tourne ». Donc
//! pas de chiffres devinés : un **étalonnage** court, sur des maillages et une texture d'essai (`etalonner`), mesure ce que la
//! carte graphique et le processeur de cet ordinateur font, et le budget s'en déduit (`budget_depuis`). Il est gardé dans les
//! réglages de l'application (`reglages.toml`, `budget_decor_3d`), avec la **clé de la machine** (carte, API, pilote,
//! processeur, `cle_machine`) : une autre carte, un autre pilote, un autre processeur, et il se remesure tout seul.
//!
//! **La règle** (`BUDGET_MS_ECRAN`) : tout le décor d'une image d'écran — l'image du plan et la vue Espace 3D, qui peuvent se
//! montrer ensemble (vue à côté) — tient en **4 ms de carte graphique**, soit 2 ms par passe : un quart d'une image à 60 Hz
//! (16,7 ms), le reste pour ce que FEANOR dessine déjà (la composition, les panneaux). La lecture à 24 i/s (41,7 ms par image)
//! et les vitesses J/K/L du montage gardent ainsi leur marge. Les ombres ont trois qualités (`Qualite`) : sans, basses
//! (carte de 1024, 3 × 3 prises), hautes (2048, 5 × 5) ; on prend la meilleure qui tient dans le budget, et sur une machine
//! faible FEANOR laisse tomber les ombres avant de ralentir. La mémoire : un quart de la mémoire vidéo connue
//! (`materiel::carte`), ou, sans elle (mémoire partagée d'un Mac, d'un graphisme intégré), un dixième de la mémoire vive, au
//! plus `decor3d::PLAFONDS.octets_textures`.
//!
//! Sans carte graphique (OpenGL de secours), le décor est rendu par le processeur sur un autre fil (`decor3d::montre`) : le
//! budget est alors un rendu en `BUDGET_MS_SECOURS` au plus, et pas d'ombres.

use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::decor3dgltf::{Primitive3d, Scene3d, Texture3d};

/// La carte graphique, pour tout le décor d'une image d'écran (millisecondes).
pub const BUDGET_MS_ECRAN: f32 = 4.0;
/// Passes de décor dans une même image d'écran au plus : l'image du plan et la vue Espace 3D (à côté).
pub const PASSES_PAR_IMAGE: f32 = 2.0;
/// Sans carte graphique : un rendu du décor par le processeur, sur un autre fil, en ce temps-là au plus.
pub const BUDGET_MS_SECOURS: f32 = 250.0;
/// Marge prise sur la mesure (une machine qui chauffe, une autre application qui dessine).
pub const MARGE: f32 = 0.8;

/// La qualité des ombres douces du décor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Qualite {
    #[default]
    Sans,
    Basses,
    Hautes,
}

impl Qualite {
    pub const TOUTES: [Qualite; 3] = [Qualite::Sans, Qualite::Basses, Qualite::Hautes];

    /// Prises du filtre (0 : pas d'ombres) et côté de la carte d'ombres.
    pub fn ombres(self) -> (u32, u32) {
        match self {
            Qualite::Sans => (0, 1),
            Qualite::Basses => (4, 1024),
            Qualite::Hautes => (9, 2048),
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            Qualite::Sans => "sans ombres",
            Qualite::Basses => "ombres douces (basses)",
            Qualite::Hautes => "ombres douces (hautes)",
        }
    }
}

/// Une droite de coût : `fixe + par_million × millions de triangles`, en millisecondes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cout {
    pub fixe: f32,
    pub par_million: f32,
}

impl Cout {
    pub fn ms(&self, triangles: u64) -> f32 {
        self.fixe + self.par_million * triangles as f32 / 1e6
    }

    /// Le plus de triangles sous `ms`.
    pub fn triangles_sous(&self, ms: f32) -> u64 {
        if ms <= self.fixe {
            return 0;
        }
        ((ms - self.fixe) / self.par_million.max(1e-6) * 1e6) as u64
    }

    /// La droite qui passe par deux mesures (`petit` triangles en `ms_petit`, `gros` en `ms_gros`).
    pub fn de_mesures(petit: u64, ms_petit: f32, gros: u64, ms_gros: f32) -> Cout {
        let par_million = ((ms_gros - ms_petit) / ((gros.saturating_sub(petit)).max(1) as f32 / 1e6)).max(1e-4);
        Cout { fixe: (ms_petit - par_million * petit as f32 / 1e6).max(0.0), par_million }
    }
}

/// Les mesures d'un étalonnage (ce qu'on injecte dans les essais).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chronos {
    /// Carte graphique : la droite de coût de chaque qualité d'ombres, à la taille de la feuille ; rien : pas de carte.
    pub gpu: Option<[Cout; 3]>,
    /// Processeur : la droite de coût du rendu de secours et d'export, à la taille du cadre.
    pub cpu: Cout,
    /// Mémoire vidéo et mémoire vive, si on les connaît.
    pub vram: Option<u64>,
    pub ram: Option<u64>,
}

/// Le budget de cette machine, gardé dans les réglages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BudgetMachine {
    /// La machine mesurée (`cle_machine`).
    pub cle: String,
    /// Date de la mesure (secondes depuis 1970).
    pub date: u64,
    /// Jusqu'à tant de triangles (sans ombres) dans le budget de l'écran.
    pub triangles: u64,
    /// Jusqu'à tant de Mo de textures.
    pub textures_mo: u64,
    /// La carte graphique dessine le décor (sinon : le processeur, sur un autre fil).
    pub carte: bool,
    /// Les droites mesurées, pour choisir la qualité des ombres d'un décor donné et estimer un export.
    #[serde(default)]
    pub gpu: Option<[Cout; 3]>,
    #[serde(default)]
    pub cpu: Cout,
}

impl BudgetMachine {
    /// Le budget d'une passe sur la carte, en millisecondes.
    pub fn ms_par_passe() -> f32 {
        BUDGET_MS_ECRAN / PASSES_PAR_IMAGE * MARGE
    }

    /// La meilleure qualité d'ombres qui tient dans le budget pour un décor de `triangles` triangles.
    pub fn qualite_pour(&self, triangles: u64) -> Qualite {
        let Some(c) = self.gpu else { return Qualite::Sans };
        let ms = Self::ms_par_passe();
        Qualite::TOUTES.into_iter().rev().find(|q| c[*q as usize].ms(triangles) <= ms).unwrap_or(Qualite::Sans)
    }

    /// Le temps d'une image exportée par le processeur, pour `triangles` triangles (millisecondes).
    pub fn export_ms(&self, triangles: u64) -> f32 {
        self.cpu.ms(triangles)
    }

    pub fn texte(&self) -> String {
        format!("jusqu'à {} triangles, {} Mo de textures", grand_nombre(self.triangles), self.textures_mo)
    }
}

/// « 1,9 M », « 250 000 ».
pub fn grand_nombre(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1} M", n as f64 / 1e6).replace('.', ",")
    } else if n >= 10_000 {
        let s = n.to_string();
        let (a, b) = s.split_at(s.len() - 3);
        format!("{a} {b}")
    } else {
        n.to_string()
    }
}

/// Le budget qu'on tire des mesures.
pub fn budget_depuis(ch: &Chronos, cle: String, date: u64) -> BudgetMachine {
    let plafond = crate::decor3d::PLAFONDS.triangles as u64;
    let (triangles, carte) = match ch.gpu {
        Some(c) => (c[Qualite::Sans as usize].triangles_sous(BudgetMachine::ms_par_passe()).min(plafond), true),
        None => ((ch.cpu.triangles_sous(BUDGET_MS_SECOURS * MARGE)).min(plafond), false),
    };
    let memoire = match (ch.vram, ch.ram) {
        (Some(v), _) => v / 4,
        (None, Some(r)) => r / 10,
        (None, None) => 512 << 20,
    };
    let textures_mo = (memoire.min(crate::decor3d::PLAFONDS.octets_textures) >> 20).max(64);
    BudgetMachine { cle, date, triangles, textures_mo, carte, gpu: ch.gpu, cpu: ch.cpu }
}

/// La clé de cette machine : la carte (nom, API, pilote) et le processeur. Rien de la carte : pas de carte graphique.
pub fn cle_machine(carte: Option<&eframe::egui_wgpu::wgpu::AdapterInfo>) -> String {
    let cpu = crate::materiel::processeur().nom;
    match carte {
        Some(i) => format!("{} | {:?} | {} {} | {cpu}", i.name, i.backend, i.driver, i.driver_info),
        None => format!("sans carte graphique | {cpu}"),
    }
}

/// Le budget gardé vaut pour cette machine.
pub fn vaut_pour(b: Option<&BudgetMachine>, cle: &str) -> bool {
    b.is_some_and(|b| b.cle == cle)
}

// ------------------------------------------------------------------------------------------------ l'étalonnage

/// Une grille de `cote` × `cote` carrés (deux triangles chacun), ondulée, texturée : la scène d'essai.
pub fn scene_d_essai(cote: usize, texture: u32) -> Scene3d {
    let mut p = Primitive3d { couleur: [0.8, 0.75, 0.7, 1.0], texture: (texture > 0).then_some(0), ..Default::default() };
    let n = cote + 1;
    for j in 0..n {
        for i in 0..n {
            let (u, v) = (i as f32 / cote as f32, j as f32 / cote as f32);
            let y = (u * 25.0).sin() * (v * 19.0).cos() * 0.15;
            p.positions.push([(u - 0.5) * 8.0, y - 1.5, -(v * 8.0) - 0.5]);
            p.normales.push([0.0, 1.0, 0.0]);
            p.uvs.push([u * 4.0, v * 4.0]);
        }
    }
    for j in 0..cote {
        for i in 0..cote {
            let a = (j * n + i) as u32;
            p.indices.extend_from_slice(&[a, a + 1, a + n as u32, a + 1, a + n as u32 + 1, a + n as u32]);
        }
    }
    let textures = if texture > 0 {
        let rgba: Vec<u8> = (0..texture * texture).flat_map(|k| if (k % texture / 32 + k / texture / 32) % 2 == 0 { [200, 180, 150, 255] } else { [90, 120, 80, 255] }).collect();
        vec![Texture3d { largeur: texture, hauteur: texture, rgba: std::sync::Arc::new(rgba), alpha: false, source: std::sync::Arc::new(Vec::new()), mime: "image/png" }]
    } else {
        Vec::new()
    };
    Scene3d { primitives: vec![p], textures }
}

/// Étalonne cette machine : la carte graphique (si `carte`) à la taille de la feuille `feuille`, chaque qualité d'ombres, sur
/// une petite et une grosse scène d'essai ; le processeur à la taille du cadre `cadre`. Sur un autre fil (une seconde ou
/// deux).
pub fn etalonner(carte: Option<&eframe::egui_wgpu::RenderState>, feuille: [u32; 2], cadre: [usize; 2]) -> Chronos {
    let monde = crate::espace3d::Monde::new([feuille[0] as usize, feuille[1] as usize], cadre);
    let oeil = [0.0, 0.0, monde.d0];
    let prise = crate::decor3drendu::Prise::de_la_feuille(&monde, oeil).expect("œil devant la feuille");
    let petite = std::sync::Arc::new(scene_d_essai(1, 256));
    // (Un million de triangles : assez pour que la pente se lise bien au-dessus du coût fixe et du bruit de mesure.)
    let grosse = std::sync::Arc::new(scene_d_essai(707, 1024));
    let aspect = crate::decor3drendu::Aspect::default();
    let gpu = carte.and_then(|rs| {
        let mut mt = crate::decor3dgpu::Moteur::new(&rs.device, &rs.queue);
        let mut couts = [Cout::default(); 3];
        for q in Qualite::TOUTES {
            let mut mesure = |s: &std::sync::Arc<Scene3d>, id: u64| {
                let d = [crate::decor3dgpu::Dessin { id, scene: s.clone(), matrice: crate::espace3dmath::IDENT, aspect }];
                crate::decor3dgpu::rendre_hors_ecran(&rs.device, &rs.queue, &mut mt, &d, &prise, feuille, true, q, 8).map(|r| r.1)
            };
            let (a, b) = (mesure(&petite, 1)?, mesure(&grosse, 2)?);
            couts[q as usize] = Cout::de_mesures(petite.triangles() as u64, a, grosse.triangles() as u64, b);
        }
        // Des ombres ne coûtent jamais moins que pas d'ombres (une mesure bruitée ne doit pas dire le contraire).
        for q in [Qualite::Basses, Qualite::Hautes] {
            let (avant, c) = (couts[q as usize - 1], &mut couts[q as usize]);
            c.fixe = c.fixe.max(avant.fixe);
            c.par_million = c.par_million.max(avant.par_million);
        }
        Some(couts)
    });
    let cpu = {
        let temps = |s: &Scene3d| {
            let debut = Instant::now();
            let _ = crate::decor3drendu::rendre(s, &crate::espace3dmath::IDENT, &crate::decor3drendu::Prise::de_la_feuille(&crate::espace3d::Monde::new(cadre, cadre), [0.0, 0.0, crate::espace3d::Monde::new(cadre, cadre).d0]).expect("œil"), cadre, 1, 0, &aspect);
            debut.elapsed().as_secs_f32() * 1000.0
        };
        let (a, b) = (temps(&petite), temps(&grosse));
        Cout::de_mesures(petite.triangles() as u64, a, grosse.triangles() as u64, b)
    };
    let vram = carte.and_then(|rs| crate::materiel::carte(&rs.adapter.get_info()).vram);
    let ram = crate::materiel::memoire().map(|m| m.totale);
    Chronos { gpu, cpu, vram, ram }
}

// --------------------------------------------------------------------------------------------- en arrière-plan

/// L'étalonnage en cours : un seul à la fois par fil qui le demande (l'application n'en a qu'un, celui de l'interface ; les
/// essais, qui tournent ensemble, ne se volent pas leurs résultats).
static EN_COURS: Mutex<Vec<(std::thread::ThreadId, std::thread::JoinHandle<BudgetMachine>)>> = Mutex::new(Vec::new());

/// Lance l'étalonnage sur un autre fil (rien s'il tourne déjà).
pub fn lancer(carte: Option<eframe::egui_wgpu::RenderState>, feuille: [u32; 2], cadre: [usize; 2]) {
    let moi = std::thread::current().id();
    let mut g = EN_COURS.lock().unwrap_or_else(|e| e.into_inner());
    if g.iter().any(|(t, _)| *t == moi) {
        return;
    }
    g.push((
        moi,
        std::thread::spawn(move || {
            let cle = cle_machine(carte.as_ref().map(|rs| rs.adapter.get_info()).as_ref());
            // (Une carte qui refuse une passe ne fait pas tomber FEANOR : on se replie sur le processeur.)
            let ch = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| etalonner(carte.as_ref(), feuille, cadre))).unwrap_or_else(|_| etalonner(None, feuille, cadre));
            let date = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            budget_depuis(&ch, cle, date)
        }),
    ));
}

/// L'étalonnage tourne.
pub fn en_cours() -> bool {
    let moi = std::thread::current().id();
    EN_COURS.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|(t, _)| *t == moi)
}

/// Le budget mesuré, s'il vient de finir.
pub fn fini() -> Option<BudgetMachine> {
    let moi = std::thread::current().id();
    let mut g = EN_COURS.lock().unwrap_or_else(|e| e.into_inner());
    let k = g.iter().position(|(t, h)| *t == moi && h.is_finished())?;
    g.remove(k).1.join().ok()
}
