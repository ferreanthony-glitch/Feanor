//! Caméra d'un panneau : où elle vise sur la feuille, son zoom, sa rotation, et ses images clés.
//!
//! Le cadre filmé est un rectangle de la taille du cadre final (`Document::frame`) divisée par le
//! zoom, centré sur (`x`, `y`) et tourné de `angle` degrés sur la feuille. Sans image clé, la
//! caméra est au centre de la feuille ; avec une seule, elle est fixe ; avec plusieurs, elle va de
//! l'une à l'autre (positions et rotation en ligne droite, zoom à vitesse régulière).

use eframe::egui::{pos2, vec2, Pos2, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Centre du cadre, en pixels de la feuille.
    pub x: f32,
    pub y: f32,
    /// 1 = le cadre fait la taille de l'image finale ; 2 = deux fois plus serré.
    pub zoom: f32,
    /// Rotation du cadre, en degrés (sens horaire à l'écran).
    pub angle: f32,
}

impl Camera {
    /// Caméra au centre d'une feuille `width`×`height`, sans zoom ni rotation.
    pub fn centered(width: usize, height: usize) -> Camera {
        Camera {
            x: width as f32 / 2.0,
            y: height as f32 / 2.0,
            zoom: 1.0,
            angle: 0.0,
        }
    }

    fn rotate(&self, v: Vec2) -> Vec2 {
        let (s, c) = self.angle.to_radians().sin_cos();
        vec2(v.x * c - v.y * s, v.x * s + v.y * c)
    }

    /// Point de la feuille filmé au point (`u`, `v`) de l'image finale `frame`.
    pub fn to_stage(&self, frame: [usize; 2], u: f32, v: f32) -> Pos2 {
        let zoom = self.zoom.max(0.01);
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
                    let t = (f - a.frame) as f32 / (b.frame - a.frame).max(1) as f32;
                    Camera::lerp(&a.cam, &b.cam, t)
                }
            }
        }
    }
}

/// Pose (ou remplace) la clé de l'image `frame`, en gardant les clés triées.
pub fn set_key(keys: &mut Vec<CameraKey>, frame: usize, cam: Camera) {
    match keys.iter().position(|k| k.frame >= frame) {
        Some(i) if keys[i].frame == frame => keys[i].cam = cam,
        Some(i) => keys.insert(i, CameraKey { frame, cam }),
        None => keys.push(CameraKey { frame, cam }),
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
/// Une partie sans clé garde sa caméra fixe (clés à son début et à sa fin).
pub fn join_keys(a: &[CameraKey], a_len: usize, b: &[CameraKey], b_len: usize, default: Camera) -> Vec<CameraKey> {
    if a.is_empty() && b.is_empty() {
        return Vec::new();
    }
    let fixed = |keys: &[CameraKey], len: usize| -> Vec<CameraKey> {
        if keys.len() >= 2 {
            return keys.to_vec();
        }
        let cam = camera_at(keys, 0, default);
        let mut v = vec![CameraKey { frame: 0, cam }];
        if len > 1 {
            v.push(CameraKey { frame: len - 1, cam });
        }
        v
    };
    let mut out = fixed(a, a_len);
    for k in fixed(b, b_len) {
        set_key(&mut out, k.frame + a_len, k.cam);
    }
    out
}
