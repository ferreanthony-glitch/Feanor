//! Groupes de couleur des calques (13b) : chaque calque appartient à un des 13 groupes du projet
//! (le premier, « Défaut », est celui des calques qu'on n'a rangés nulle part). Un groupe a un nom
//! et une couleur, montrée dans l'en-tête des calques, et peut être caché de la timeline.

/// Nombre de groupes d'un projet (le groupe 0 et douze couleurs).
pub const COUNT: usize = 13;

#[derive(Clone, Debug, PartialEq)]
pub struct LayerGroup {
    pub name: String,
    pub color: [u8; 3],
    /// Faux : les calques du groupe ne sont plus montrés dans la timeline (filtre de groupe).
    pub shown: bool,
}

/// Les groupes d'un projet neuf.
pub fn defaults() -> Vec<LayerGroup> {
    [
        ("Défaut", [128, 128, 128]),
        ("Bleu foncé", [38, 64, 150]),
        ("Vert foncé", [30, 110, 50]),
        ("Cyan foncé", [20, 110, 125]),
        ("Rouge foncé", [140, 30, 35]),
        ("Violet", [110, 55, 160]),
        ("Brun", [120, 80, 40]),
        ("Bleu", [60, 125, 235]),
        ("Vert", [70, 190, 80]),
        ("Cyan", [60, 200, 215]),
        ("Rouge", [225, 60, 60]),
        ("Magenta", [215, 70, 195]),
        ("Orange", [245, 140, 40]),
    ]
    .into_iter()
    .map(|(name, color)| LayerGroup { name: name.into(), color, shown: true })
    .collect()
}

/// Groupes lus dans un projet : il en faut exactement `COUNT` ; ceux qui manquent reprennent
/// leur réglage d'origine, ceux en trop sont oubliés.
pub fn completed(mut groups: Vec<LayerGroup>) -> Vec<LayerGroup> {
    let base = defaults();
    groups.truncate(COUNT);
    let n = groups.len();
    groups.extend(base.into_iter().skip(n));
    groups
}
