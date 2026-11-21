//! La banque 3D (partie 181, `docs/INTERFACE.md`) : 45 modèles low poly choisis par Anthony le 2026-11-19 dans cinq kits de
//! Kenney (licence CC0 : domaine public, `assets/banque3d/LICENCES.md`), montrés dans la Bibliothèque (onglet « 3D »,
//! `banque3dui.rs`) et posés dans le décor 3D de la séquence comme un fichier importé (`decor3dui.rs` : même lecture, même
//! budget de la machine, même fenêtre « Ce décor est lourd »).
//!
//! **Rangés dans le programme** (`include_bytes!`, `banque3dfichiers.rs`), pas à côté. La bibliothèque de bruitages, elle, ne
//! livre aucun son : elle liste les dossiers de l'utilisateur (`bruitages.rs`). Ce que FEANOR livre avec lui (polices, licence
//! de ffmpeg, page de relecture) est déjà rangé ainsi dans `feanor.exe` : un seul fichier à copier, rien qui puisse manquer ou
//! être d'une autre version, et la banque est là même quand FEANOR tourne depuis une clé ou un dossier sans droits. Le prix :
//! environ 1,4 Mo de plus dans l'exécutable.
//!
//! Le catalogue (`catalogue.json`, format `feanor-banque3d` version 1) dit pour chaque modèle sa famille, son nom, son fichier,
//! son aperçu, ses triangles, sa taille, son kit, son auteur, sa licence et sa source.

use std::sync::OnceLock;

use serde::Deserialize;

/// Les familles, dans l'ordre où la Bibliothèque les montre.
pub const FAMILLES: [&str; 5] = ["Nature", "Routes", "Ville rétro", "Banlieue", "Commerces"];
/// Le catalogue, rangé dans le programme.
pub const CATALOGUE: &str = include_str!("../assets/banque3d/catalogue.json");
/// Le format et la version de catalogue que ce FEANOR sait lire.
pub const FORMAT: &str = "feanor-banque3d";
pub const VERSION: u32 = 1;
/// Le crédit (« À propos ») : la licence CC0 ne l'exige pas, FEANOR le donne tout de même.
pub const CREDIT: &str = "Banque 3D : modèles de Kenney (www.kenney.nl), licence CC0 (domaine public)";
/// Le site de Kenney.
pub const SITE_KENNEY: &str = "https://www.kenney.nl";

/// Un modèle de la banque.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ModeleBanque {
    pub famille: String,
    pub nom: String,
    /// Son `.glb`, dans `assets/banque3d/`.
    pub fichier: String,
    /// Son aperçu `.png`, dans `assets/banque3d/`.
    pub apercu: String,
    pub triangles: u64,
    pub octets: u64,
    pub kit: String,
    pub auteur: String,
    pub licence: String,
    pub source: String,
}

#[derive(Deserialize)]
struct Catalogue {
    format: String,
    version: u32,
    modeles: Vec<ModeleBanque>,
}

/// Lit un catalogue : refusé s'il n'est pas du format `feanor-banque3d`, ou d'une version plus récente.
pub fn lire_catalogue(texte: &str) -> Result<Vec<ModeleBanque>, String> {
    let c: Catalogue = serde_json::from_str(texte).map_err(|e| format!("catalogue de la banque 3D illisible : {e}"))?;
    if c.format != FORMAT {
        return Err(format!("catalogue de la banque 3D : format « {} » inconnu", c.format));
    }
    if c.version > VERSION {
        return Err(format!("catalogue de la banque 3D d'une version plus récente ({})", c.version));
    }
    Ok(c.modeles)
}

/// Le nom français de chaque modèle de la banque, par le nom de son fichier dans le catalogue. Courts : ils tiennent sous une vignette.
pub const NOMS_FR: [(&str, &str); 45] = [
    ("building-type-a", "Maison A"),
    ("building-type-c", "Maison C"),
    ("building-type-f", "Maison F"),
    ("driveway-long", "Allée longue"),
    ("tree-large", "Grand arbre"),
    ("tree-small", "Petit arbre"),
    ("building-a", "Immeuble"),
    ("building-skyscraper-a", "Gratte-ciel"),
    ("detail-parasol-a", "Parasol"),
    ("campfire_logs", "Feu de camp"),
    ("fence_simple", "Barrière"),
    ("flower_redA", "Fleur rouge"),
    ("grass_large", "Herbe haute"),
    ("log", "Rondin"),
    ("path_stone", "Pas de pierre"),
    ("plant_bush", "Buisson"),
    ("rock_largeA", "Gros rocher"),
    ("rock_smallA", "Petit rocher"),
    ("stone_largeC", "Grosse pierre C"),
    ("stone_largeD", "Grosse pierre D"),
    ("stone_smallG", "Petite pierre G"),
    ("stone_smallH", "Petite pierre H"),
    ("stone_tallH", "Pierre haute H"),
    ("stone_tallI", "Pierre haute I"),
    ("stump_round", "Souche"),
    ("tree_default", "Arbre"),
    ("tree_fat", "Arbre large"),
    ("tree_oak", "Chêne"),
    ("tree_pineTallA", "Grand pin"),
    ("tree_simple", "Arbre simple"),
    ("construction-cone", "Cône de chantier"),
    ("construction-fence", "Grille de chantier"),
    ("dumpster", "Benne"),
    ("light-curved", "Lampadaire"),
    ("road-bend", "Virage"),
    ("road-crossroad", "Carrefour"),
    ("road-straight", "Route droite"),
    ("traffic-light", "Feu tricolore"),
    ("detail-barrier-strong-type-b", "Glissière béton"),
    ("detail-barrier-type-b", "Glissière"),
    ("detail-bench", "Banc"),
    ("truck-flat", "Camion plateau"),
    ("truck-green", "Camion vert"),
    ("wall-c-flat-low", "Muret"),
    ("wall-fence", "Mur grillagé"),
];

/// Les modèles de la banque (le catalogue rangé dans le programme, lu une fois).
pub fn modeles() -> &'static [ModeleBanque] {
    static M: OnceLock<Vec<ModeleBanque>> = OnceLock::new();
    M.get_or_init(|| lire_catalogue(CATALOGUE).unwrap_or_default())
}

/// Les octets d'un fichier de la banque (`chemin` tel que le catalogue l'écrit).
pub fn octets(chemin: &str) -> Option<&'static [u8]> {
    crate::banque3dfichiers::FICHIERS.iter().find(|(c, _)| *c == chemin).map(|(_, o)| *o)
}

impl ModeleBanque {
    /// « Maison A », « Feu de camp » : le nom du modèle en français (`NOMS_FR`, audit du 2026-11-21 : les noms de fichier anglais
    /// étaient coupés sous les vignettes, « onstruction con… »). Un modèle sans nom français garde le nom de son fichier, lisible
    /// (« building type a »). La recherche trouve toujours le nom du fichier (« tree ») en plus du nom français (« arbre »).
    pub fn nom_lisible(&self) -> String {
        if let Some((_, fr)) = NOMS_FR.iter().find(|(nom, _)| *nom == self.nom) {
            return fr.to_string();
        }
        let n = self.nom.replace(['-', '_'], " ");
        let mut c = n.chars();
        c.next().map(|p| p.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    }

    pub fn glb(&self) -> Option<&'static [u8]> {
        octets(&self.fichier)
    }

    pub fn png(&self) -> Option<&'static [u8]> {
        octets(&self.apercu)
    }

    /// Ce que l'objet posé retient de son origine (le « fichier d'origine » du panneau Espace 3D).
    pub fn source_texte(&self) -> String {
        format!("Banque 3D : {}, {} (CC0 · Kenney)", self.kit, self.nom)
    }

    /// La bulle de la vignette : nom, triangles, kit, licence.
    pub fn bulle(&self) -> String {
        format!("{}\n{} triangles\n{}\nCC0 · Kenney", self.nom_lisible(), crate::decor3dbudget::grand_nombre(self.triangles), self.kit)
    }

    /// La recherche (sans tenir compte des majuscules) trouve-t-elle ce modèle ? Dans son nom, sa famille et son kit.
    pub fn correspond(&self, recherche: &str) -> bool {
        let r = recherche.trim().to_lowercase();
        r.is_empty() || r.split_whitespace().all(|mot| [&self.nom_lisible(), &self.nom, &self.famille, &self.kit].iter().any(|t| t.to_lowercase().contains(mot)))
    }
}

/// Les familles et leurs modèles (rangs dans `modeles()`) que la recherche trouve, dans l'ordre de `FAMILLES` (une famille
/// inconnue du catalogue vient après) ; une famille sans modèle trouvé n'est pas montrée.
pub fn par_famille(recherche: &str) -> Vec<(String, Vec<usize>)> {
    let tout = modeles();
    let mut familles: Vec<String> = FAMILLES.iter().map(|f| f.to_string()).collect();
    for m in tout {
        if !familles.contains(&m.famille) {
            familles.push(m.famille.clone());
        }
    }
    familles
        .into_iter()
        .map(|f| {
            let rangs = tout.iter().enumerate().filter(|(_, m)| m.famille == f && m.correspond(recherche)).map(|(i, _)| i).collect::<Vec<_>>();
            (f, rangs)
        })
        .filter(|(_, r)| !r.is_empty())
        .collect()
}

/// Lit et mesure le modèle `rang` de la banque (sur le fil de l'import, `App::decor3d_lancer`) : comme un fichier importé.
pub fn analyser(rang: usize) -> Result<crate::decor3d::Analyse, String> {
    let m = modeles().get(rang).ok_or_else(|| "modèle de la banque 3D inconnu".to_string())?;
    let glb = m.glb().ok_or_else(|| format!("le fichier {} manque à la banque 3D", m.fichier))?;
    crate::decor3d::analyser_octets(glb, None, m.nom_lisible(), m.source_texte())
}
