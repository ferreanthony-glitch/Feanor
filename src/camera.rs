//! Caméra d'un panneau : où elle vise sur la feuille, son zoom (la focale), son avancée vers la
//! feuille, sa rotation, et ses images clés.
//!
//! La caméra regarde la feuille de face, depuis la distance de référence `ref_distance` (celle
//! d'où un objectif de 35 mm voit exactement le cadre). Le cadre filmé sur la feuille est un
//! rectangle de la taille du cadre final (`Document::frame`) divisée par le grossissement
//! (`magnification` : la focale et l'avancée), centré sur (`x`, `y`) et tourné de `angle` degrés.
//! L'avancée (travelling avant) fait aussi grossir les calques en profondeur selon leur distance
//! (`Layer::transform_seen`), ce que le zoom ne fait pas. Sans image clé, la caméra est celle du
//! panneau d'avant (ou d'après) dans la scène, sinon au centre ; avec une seule, elle est fixe ;
//! avec plusieurs, elle va de l'une à l'autre (positions, avancée et rotation en ligne droite, zoom
//! à vitesse régulière), ralentie au départ et à l'arrivée selon les adoucis de sortie et d'entrée.
//!
//! Bascule et panoramique (9f) : la caméra tourne sur elle-même, à sa place (son œil ne bouge
//! pas). Tous les calques étant déjà ramenés sur le plan de référence tels que l'œil les voit
//! (multiplan), ce que filme la caméra tournée est ce plan vu en perspective : le passage de
//! l'image finale à la feuille devient une homographie (`homography`), le cadre filmé un
//! trapèze. Le multiplan, lui, ne dépend que de la place de l'œil et ne change pas.

use eframe::egui::{pos2, vec2, Pos2, Vec2};

use crate::persp::{self, Homography};

/// Focale du zoom 100 %, en mm (capteur de 36 mm de large).
pub const REF_FOCAL: f32 = 35.0;

/// Largeur du capteur (film 35 mm plein cadre), en mm : celle qui lie la focale à l'angle de
/// champ, et le zoom 100 % (35 mm) à la distance de référence.
pub const SENSOR_WIDTH: f32 = 36.0;

/// Angle de champ horizontal, en degrés, d'un objectif de `focal` mm sur un capteur de
/// `SENSOR_WIDTH` mm de large.
pub fn angle_of_view(focal: f32) -> f32 {
    2.0 * (SENSOR_WIDTH / (2.0 * focal.max(0.1))).atan().to_degrees()
}

/// Zoom (1 = `REF_FOCAL`) d'une focale en mm, dans les bornes du modèle.
pub fn zoom_for_focal(focal: f32) -> f32 {
    (focal / REF_FOCAL).clamp(0.05, 50.0)
}

/// Un objectif proposé dans la liste des focales (réglages de la caméra).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocalPreset {
    pub name: &'static str,
    pub mm: f32,
    /// Ce que l'objectif donne, en une phrase.
    pub note: &'static str,
    /// Objectif anamorphique : il pose aussi le cadre 2,39:1 (un repère d'affichage seulement).
    pub anamorphic: bool,
}

/// Rapport du cadre posé avec l'objectif anamorphique (le « cinémascope »), largeur:hauteur.
pub const ANAMORPHIC_FORMAT: [f32; 2] = [2.39, 1.0];

/// Focales proposées, de la plus courte à la plus longue (l'anamorphique, à part, en dernier).
pub const FOCAL_PRESETS: [FocalPreset; 8] = [
    FocalPreset { name: "Très grand angle", mm: 14.0, note: "Champ très large, perspective exagérée.", anamorphic: false },
    FocalPreset { name: "Grand angle (courte)", mm: 24.0, note: "Large, profondeur marquée : décors, espaces.", anamorphic: false },
    FocalPreset { name: "Normale", mm: 35.0, note: "Le zoom 100 % de SHOT : le cadre vu tel quel.", anamorphic: false },
    FocalPreset { name: "Standard", mm: 50.0, note: "Proche du regard : champ sans exagération.", anamorphic: false },
    FocalPreset { name: "Portrait", mm: 85.0, note: "Resserré, fonds aplatis : visages.", anamorphic: false },
    FocalPreset { name: "Téléobjectif (longue)", mm: 135.0, note: "Étroit, plans écrasés les uns sur les autres.", anamorphic: false },
    FocalPreset { name: "Long téléobjectif", mm: 200.0, note: "Très étroit : sujets lointains, perspective aplatie.", anamorphic: false },
    FocalPreset {
        name: "Anamorphique",
        mm: 40.0,
        note: "40 mm et cadre 2,39:1 posé en repère. Un vrai objectif anamorphique comprime l'image en largeur (×2 en général) \
               et on la redresse ensuite : SHOT ne déforme pas le dessin, il montre seulement le cadre.",
        anamorphic: true,
    },
];

/// Texte d'un objectif : « Portrait · 85 mm · 24° ».
pub fn focal_label(p: &FocalPreset) -> String {
    format!("{} · {} mm · {:.0}°", p.name, p.mm, angle_of_view(p.mm))
}

/// Distance de la caméra au plan de référence (profondeur 0), en pixels de la feuille : celle
/// d'où un objectif de `REF_FOCAL` voit exactement le cadre.
pub fn ref_distance(frame: [usize; 2]) -> f32 {
    frame[0].max(1) as f32 * REF_FOCAL / 36.0
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Centre du cadre, en pixels de la feuille.
    pub x: f32,
    pub y: f32,
    /// 1 = le cadre fait la taille de l'image finale ; 2 = deux fois plus serré (focale doublée).
    pub zoom: f32,
    /// Rotation du cadre, en degrés (sens horaire à l'écran).
    pub angle: f32,
    /// Avancée de la caméra vers la feuille (travelling avant), en pixels ; négative : recul.
    pub dolly: f32,
    /// Bascule, en degrés : positive, la caméra regarde vers le haut.
    pub tilt: f32,
    /// Panoramique, en degrés : positif, la caméra regarde vers la droite.
    pub pan: f32,
}

/// Une caméra tournée doit encore voir le plan de référence par les quatre coins du cadre, sous
/// au moins cet angle (degrés) : au-delà, le cadre filmerait l'horizon.
pub const MIN_GRAZING: f32 = 5.0;

/// Passage de l'image finale à la feuille, calculé une fois pour beaucoup de points.
#[derive(Clone, Copy, Debug)]
pub enum FrameMap {
    /// Caméra droite : le calcul de toujours (`Camera::to_stage`).
    Level(Camera, [usize; 2]),
    /// Caméra basculée ou en panoramique : homographie (w > 0 devant la caméra).
    Turned(Homography),
}

impl FrameMap {
    /// Point de la feuille filmé au point (`u`, `v`) de l'image ; rien si ce rayon ne rencontre
    /// pas le plan de référence.
    pub fn at(&self, u: f32, v: f32) -> Option<Pos2> {
        match self {
            FrameMap::Level(cam, frame) => Some(cam.to_stage(*frame, u, v)),
            FrameMap::Turned(h) => {
                let w = h[6] * u + h[7] * v + h[8];
                (w > 1e-9).then(|| pos2((h[0] * u + h[1] * v + h[2]) / w, (h[3] * u + h[4] * v + h[5]) / w))
            }
        }
    }
}

impl Camera {
    /// Caméra au centre d'une feuille `width`×`height`, sans zoom, avancée ni rotation.
    pub fn centered(width: usize, height: usize) -> Camera {
        Camera {
            x: width as f32 / 2.0,
            y: height as f32 / 2.0,
            zoom: 1.0,
            angle: 0.0,
            dolly: 0.0,
            tilt: 0.0,
            pan: 0.0,
        }
    }

    /// Ni bascule ni panoramique : le cadre filmé est un rectangle.
    pub fn is_level(&self) -> bool {
        self.tilt == 0.0 && self.pan == 0.0
    }

    /// Homographie de l'image finale (pixels) vers la feuille, pour n'importe quelle caméra ; w
    /// est positif pour un rayon qui rencontre le plan de référence devant la caméra.
    ///
    /// Un point (u, v) de l'image donne un rayon de la caméra : (u − l/2, v − h/2) tourné du roulis
    /// (`angle`), puis (x, −y, −f) avec f = zoom × D (focale en pixels), tourné de la bascule
    /// (autour de l'axe horizontal de la caméra) puis du panoramique (autour de la verticale). Il
    /// rencontre le plan z = 0 depuis l'œil (à la distance E) au point
    /// (x_c − E·dx/dz, y_c + E·dy/dz) de la feuille (y vers le bas sur la feuille, vers le haut
    /// dans le monde). Caméra droite : c'est exactement `to_stage`.
    pub fn homography(&self, frame: [usize; 2]) -> Homography {
        let (fw, fh) = (frame[0] as f32, frame[1] as f32);
        let f = self.zoom.max(0.01) * ref_distance(frame);
        let e = self.distance(frame);
        let (sr, cr) = self.angle.to_radians().sin_cos();
        let (st, ct) = self.tilt.to_radians().sin_cos();
        let (sp, cp) = self.pan.to_radians().sin_cos();
        let center: Homography = [1.0, 0.0, -fw / 2.0, 0.0, 1.0, -fh / 2.0, 0.0, 0.0, 1.0];
        let roll: Homography = [cr, -sr, 0.0, sr, cr, 0.0, 0.0, 0.0, 1.0];
        let lens: Homography = [1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -f];
        let tilt: Homography = [1.0, 0.0, 0.0, 0.0, ct, -st, 0.0, st, ct];
        let pan: Homography = [cp, 0.0, -sp, 0.0, 1.0, 0.0, sp, 0.0, cp];
        // Signe changé (une homographie est la même à un facteur près) : w > 0 devant.
        let ground: Homography = [e, 0.0, -self.x, 0.0, -e, -self.y, 0.0, 0.0, -1.0];
        [pan, tilt, lens, roll, center].iter().fold(ground, |m, b| persp::mul(&m, b))
    }

    /// Homographie inverse (feuille → image finale), composée des inverses de chaque étape (plus
    /// précis que d'inverser la matrice) ; w est positif pour un point devant la caméra.
    pub fn homography_inverse(&self, frame: [usize; 2]) -> Homography {
        let (fw, fh) = (frame[0] as f32, frame[1] as f32);
        let f = self.zoom.max(0.01) * ref_distance(frame);
        let e = self.distance(frame);
        let (sr, cr) = self.angle.to_radians().sin_cos();
        let (st, ct) = self.tilt.to_radians().sin_cos();
        let (sp, cp) = self.pan.to_radians().sin_cos();
        let ground: Homography = [1.0 / e, 0.0, -self.x / e, 0.0, -1.0 / e, self.y / e, 0.0, 0.0, -1.0];
        let pan: Homography = [cp, 0.0, sp, 0.0, 1.0, 0.0, -sp, 0.0, cp];
        let tilt: Homography = [1.0, 0.0, 0.0, 0.0, ct, st, 0.0, -st, ct];
        let lens: Homography = [1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0 / f];
        let roll: Homography = [cr, sr, 0.0, -sr, cr, 0.0, 0.0, 0.0, 1.0];
        let center: Homography = [1.0, 0.0, fw / 2.0, 0.0, 1.0, fh / 2.0, 0.0, 0.0, 1.0];
        [roll, lens, tilt, pan, ground].iter().fold(center, |m, b| persp::mul(&m, b))
    }

    /// Passage de l'image finale à la feuille.
    pub fn frame_map(&self, frame: [usize; 2]) -> FrameMap {
        if self.is_level() {
            FrameMap::Level(*self, frame)
        } else {
            FrameMap::Turned(self.homography(frame))
        }
    }

    /// Les quatre coins du cadre voient le plan de référence sous au moins `MIN_GRAZING` degrés.
    pub fn sees_reference(&self, frame: [usize; 2]) -> bool {
        if self.is_level() {
            return true;
        }
        let h = self.homography(frame);
        let (fw, fh) = (frame[0] as f32, frame[1] as f32);
        let f = self.zoom.max(0.01) * ref_distance(frame);
        let min = MIN_GRAZING.to_radians().sin();
        [(0.0, 0.0), (fw, 0.0), (fw, fh), (0.0, fh)].iter().all(|&(u, v)| {
            // w = −dz pour un rayon de longueur √((u − l/2)² + (v − h/2)² + f²) : w / longueur est le
            // sinus de l'angle sous lequel il rencontre le plan.
            let w = h[6] * u + h[7] * v + h[8];
            let len = ((u - fw / 2.0).powi(2) + (v - fh / 2.0).powi(2) + f * f).sqrt();
            w / len > min
        })
    }

    /// La même caméra, bascule et panoramique réduits ensemble autant qu'il faut pour que le cadre
    /// voie encore le plan de référence (`sees_reference`).
    pub fn limited(&self, frame: [usize; 2]) -> Camera {
        if self.sees_reference(frame) {
            return *self;
        }
        let turned = |k: f32| Camera { tilt: self.tilt * k, pan: self.pan * k, ..*self };
        let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            if turned(mid).sees_reference(frame) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        turned(lo)
    }

    fn rotate(&self, v: Vec2) -> Vec2 {
        let (s, c) = self.angle.to_radians().sin_cos();
        vec2(v.x * c - v.y * s, v.x * s + v.y * c)
    }

    /// Distance de la caméra au plan de référence, en pixels (jamais nulle : au plus près, 2 %
    /// de la distance de référence).
    pub fn distance(&self, frame: [usize; 2]) -> f32 {
        let d0 = ref_distance(frame);
        (d0 - self.dolly).max(d0 * 0.02)
    }

    /// Grossissement du plan de référence : l'objectif (zoom) et l'avancée.
    pub fn magnification(&self, frame: [usize; 2]) -> f32 {
        self.zoom.max(0.01) * ref_distance(frame) / self.distance(frame)
    }

    /// Focale équivalente, en mm.
    pub fn focal(&self) -> f32 {
        self.zoom * REF_FOCAL
    }

    /// Point de la feuille filmé au point (`u`, `v`) de l'image finale `frame` (caméra tournée :
    /// par l'homographie ; pour beaucoup de points, `frame_map`).
    pub fn to_stage(&self, frame: [usize; 2], u: f32, v: f32) -> Pos2 {
        if !self.is_level() {
            return crate::persp::apply(&self.homography(frame), pos2(u, v));
        }
        let zoom = self.magnification(frame);
        let d = vec2(u - frame[0] as f32 / 2.0, v - frame[1] as f32 / 2.0) / zoom;
        pos2(self.x, self.y) + self.rotate(d)
    }

    /// Coins du cadre sur la feuille : haut-gauche, haut-droite, bas-droite, bas-gauche.
    pub fn corners(&self, frame: [usize; 2]) -> [Pos2; 4] {
        let (w, h) = (frame[0] as f32, frame[1] as f32);
        [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)].map(|(u, v)| self.to_stage(frame, u, v))
    }

    /// Entre `a` (t = 0) et `b` (t = 1).
    pub fn lerp(a: &Camera, b: &Camera, t: f32) -> Camera {
        let mix = |p: f32, q: f32| p + (q - p) * t;
        Camera {
            x: mix(a.x, b.x),
            y: mix(a.y, b.y),
            // Zoom géométrique : passer de 1 à 4 semble régulier (2 à mi-chemin).
            zoom: (a.zoom.max(0.01).ln() + (b.zoom.max(0.01).ln() - a.zoom.max(0.01).ln()) * t).exp(),
            angle: mix(a.angle, b.angle),
            dolly: mix(a.dolly, b.dolly),
            tilt: mix(a.tilt, b.tilt),
            pan: mix(a.pan, b.pan),
        }
    }

    /// Décale la caméra (agrandissement de la feuille).
    pub fn shifted(&self, dx: f32, dy: f32) -> Camera {
        Camera {
            x: self.x + dx,
            y: self.y + dy,
            ..*self
        }
    }
}

/// Image clé de caméra : à l'image `frame` du panneau, la caméra est `cam`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraKey {
    pub frame: usize,
    pub cam: Camera,
    /// Mouvement adouci à cette clé (ancien réglage, avant les adoucis en images) : la caméra y
    /// arrive et en repart lentement, sur tout l'intervalle.
    pub smooth: bool,
    /// Adoucis d'entrée (avant la clé) et de sortie (après), en images : la caméra ralentit en
    /// arrivant, accélère en repartant ; 0 : vitesse constante.
    pub ease_in: f32,
    pub ease_out: f32,
}

impl CameraKey {
    pub fn new(frame: usize, cam: Camera) -> CameraKey {
        CameraKey {
            frame,
            cam,
            smooth: false,
            ease_in: 0.0,
            ease_out: 0.0,
        }
    }

    /// Adoucis effectifs : en images, l'ancien « adoucir » valant un demi-intervalle.
    fn eases(&self, span: f32) -> (f32, f32) {
        let pick = |v: f32| if v > 0.0 { v } else if self.smooth { span * 0.5 } else { 0.0 };
        (pick(self.ease_in), pick(self.ease_out))
    }
}

/// Avancement (0 à 1) au temps `t` (0 à 1) d'un mouvement de `span` images, qui accélère sur
/// `out` images au départ et ralentit sur `inn` images à l'arrivée (vitesse constante entre les
/// deux, sans à-coup). Deux adoucis plus longs que le mouvement se partagent sa durée.
pub fn ease(t: f32, span: f32, out: f32, inn: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let span = span.max(1e-3);
    let (mut a, mut b) = ((out / span).clamp(0.0, 1.0), (inn / span).clamp(0.0, 1.0));
    if a + b > 1.0 {
        let s = a + b;
        a /= s;
        b /= s;
    }
    if a <= 0.0 && b <= 0.0 {
        return t;
    }
    let v = 2.0 / (2.0 - a - b);
    if t < a {
        v * t * t / (2.0 * a)
    } else if t <= 1.0 - b {
        v * (t - a / 2.0)
    } else {
        1.0 - v * (1.0 - t) * (1.0 - t) / (2.0 * b)
    }
}

/// Caméra à l'image `f` d'un panneau dont les images clés sont `keys` (triées par image) ; sans
/// clé, `default`. Avant la première clé et après la dernière, la caméra ne bouge pas.
pub fn camera_at(keys: &[CameraKey], f: usize, default: Camera) -> Camera {
    match keys {
        [] => default,
        [k] => k.cam,
        _ => {
            let next = keys.iter().position(|k| k.frame > f);
            match next {
                None => keys[keys.len() - 1].cam,
                Some(0) => keys[0].cam,
                Some(i) => {
                    let (a, b) = (&keys[i - 1], &keys[i]);
                    let span = (b.frame - a.frame).max(1) as f32;
                    let t = (f - a.frame) as f32 / span;
                    let (_, out) = a.eases(span);
                    let (inn, _) = b.eases(span);
                    Camera::lerp(&a.cam, &b.cam, ease(t, span, out, inn))
                }
            }
        }
    }
}

/// Cadence la plus lente qu'on puisse régler à la postérisation temporelle de la caméra.
pub const SACCADE_MAX: usize = 120;

/// Postérisation temporelle de la caméra d'un plan (« saccade ») : le mouvement, interpolé en
/// douceur entre les clés, est montré **par paliers** : la caméra ne change que toutes les
/// `cadence` images et se tient entre deux. Aucune clé n'est posée ni changée : le réglage
/// s'applique quand la caméra est évaluée (`Document::panel_camera`), et le décocher rend le
/// mouvement doux, à l'identique.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saccade {
    pub actif: bool,
    /// « Toutes les N images » : au moins 2.
    pub cadence: usize,
}

impl Default for Saccade {
    fn default() -> Self {
        Saccade { actif: false, cadence: 2 }
    }
}

impl Saccade {
    /// Une cadence réglable (entre 2 et `SACCADE_MAX`).
    pub fn cadence_valide(n: usize) -> usize {
        n.clamp(2, SACCADE_MAX)
    }

    /// Le réglage, la cadence ramenée dans ses bornes.
    pub fn normalise(self) -> Saccade {
        Saccade { cadence: Saccade::cadence_valide(self.cadence), ..self }
    }

    /// L'image dont la caméra est montrée à l'image `f` d'un plan dont les clés sont `keys` : le début
    /// de son palier de `cadence` images (compté depuis le début du plan). À partir de la dernière clé,
    /// l'image elle-même : la caméra ne bouge plus, et le mouvement finit pile sur son cadre final
    /// au lieu de rester jusqu'à `cadence - 1` images en deçà.
    pub fn image_montree(self, keys: &[CameraKey], f: usize) -> usize {
        if !self.actif || keys.len() < 2 {
            return f;
        }
        let n = Saccade::cadence_valide(self.cadence);
        if keys.last().is_some_and(|k| f >= k.frame) {
            return f;
        }
        f - f % n
    }
}

/// Pose (ou remplace) la clé de l'image `frame`, en gardant les clés triées (et l'adoucissement
/// d'une clé remplacée).
pub fn set_key(keys: &mut Vec<CameraKey>, frame: usize, cam: Camera) {
    match keys.iter().position(|k| k.frame >= frame) {
        Some(i) if keys[i].frame == frame => keys[i].cam = cam,
        Some(i) => keys.insert(i, CameraKey::new(frame, cam)),
        None => keys.push(CameraKey::new(frame, cam)),
    }
}

/// Pose la clé `key` en entier (caméra et adoucis) à son image, en gardant les clés triées ; elle
/// remplace celle qui y était.
pub fn put_key(keys: &mut Vec<CameraKey>, key: CameraKey) {
    match keys.iter().position(|k| k.frame >= key.frame) {
        Some(i) if keys[i].frame == key.frame => keys[i] = key,
        Some(i) => keys.insert(i, key),
        None => keys.push(key),
    }
}

/// Coupe les clés d'un panneau à l'image `at` : celles d'avant, et celles d'après ramenées à 0.
/// Le mouvement est gardé : chaque partie reçoit une clé à la coupure si la caméra y bougeait.
pub fn split_keys(keys: &[CameraKey], at: usize, default: Camera) -> (Vec<CameraKey>, Vec<CameraKey>) {
    if keys.len() <= 1 {
        return (keys.to_vec(), keys.iter().map(|k| CameraKey { frame: 0, ..*k }).collect());
    }
    let mut first: Vec<CameraKey> = keys.iter().filter(|k| k.frame < at).copied().collect();
    let mut second: Vec<CameraKey> = keys
        .iter()
        .filter(|k| k.frame >= at)
        .map(|k| CameraKey { frame: k.frame - at, ..*k })
        .collect();
    let before = camera_at(keys, at.saturating_sub(1), default);
    let after = camera_at(keys, at, default);
    if !second.is_empty() && first.last().is_none_or(|k| k.frame + 1 < at) {
        set_key(&mut first, at.saturating_sub(1), before);
    }
    if second.first().is_none_or(|k| k.frame > 0) {
        set_key(&mut second, 0, after);
    }
    (first, second)
}

/// Clés d'un panneau de `a_len` images suivi d'un autre : ses clés puis celles de l'autre décalées.
/// Une partie sans clé garde sa caméra fixe (`a_cam`, `b_cam` : celle qu'elle montrait), avec des
/// clés à son début et à sa fin.
pub fn join_keys(a: &[CameraKey], a_len: usize, a_cam: Camera, b: &[CameraKey], b_len: usize, b_cam: Camera) -> Vec<CameraKey> {
    if a.is_empty() && b.is_empty() && a_cam == b_cam {
        return Vec::new();
    }
    let fixed = |keys: &[CameraKey], len: usize, shown: Camera| -> Vec<CameraKey> {
        if keys.len() >= 2 {
            return keys.to_vec();
        }
        let cam = camera_at(keys, 0, shown);
        let mut v = vec![CameraKey::new(0, cam)];
        if len > 1 {
            v.push(CameraKey::new(len - 1, cam));
        }
        v
    };
    let mut out = fixed(a, a_len, a_cam);
    for k in fixed(b, b_len, b_cam) {
        let frame = k.frame + a_len;
        set_key(&mut out, frame, k.cam);
        if let Some(x) = out.iter_mut().find(|x| x.frame == frame) {
            (x.smooth, x.ease_in, x.ease_out) = (k.smooth, k.ease_in, k.ease_out);
        }
    }
    out
}
