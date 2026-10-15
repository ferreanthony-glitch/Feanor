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

use eframe::egui::{pos2, vec2, Pos2, Vec2};

/// Focale du zoom 100 %, en mm (capteur de 36 mm de large).
pub const REF_FOCAL: f32 = 35.0;

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
        }
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

    /// Point de la feuille filmé au point (`u`, `v`) de l'image finale `frame`.
    pub fn to_stage(&self, frame: [usize; 2], u: f32, v: f32) -> Pos2 {
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

/// Pose (ou remplace) la clé de l'image `frame`, en gardant les clés triées (et l'adoucissement
/// d'une clé remplacée).
pub fn set_key(keys: &mut Vec<CameraKey>, frame: usize, cam: Camera) {
    match keys.iter().position(|k| k.frame >= frame) {
        Some(i) if keys[i].frame == frame => keys[i].cam = cam,
        Some(i) => keys.insert(i, CameraKey::new(frame, cam)),
        None => keys.push(CameraKey::new(frame, cam)),
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
