use crate::flow::*;

/// Paysage doux, défini partout (somme d'ondes, valeurs autour de 128) : on peut le déplacer ou
/// le tourner sans perte.
fn scene(x: f32, y: f32) -> f32 {
    let waves = [
        (0.11, 0.05, 0.3, 40.0),
        (-0.07, 0.13, 1.1, 35.0),
        (0.21, -0.17, 2.0, 25.0),
        (0.03, 0.31, 0.7, 15.0),
        (0.37, 0.09, 1.7, 12.0),
    ];
    128.0 + waves.iter().map(|&(a, b, p, amp): &(f32, f32, f32, f32)| amp * (a * x + b * y + p).sin()).sum::<f32>()
}

fn render(w: usize, h: usize, f: impl Fn(f32, f32) -> f32) -> Gray {
    Gray::new(w, h, (0..w * h).map(|i| f((i % w) as f32, (i / w) as f32)).collect())
}

/// Erreur moyenne (en pixels) du flux sur l'intérieur, `margin` pixels loin des bords.
fn endpoint_error(flow: &Flow, truth: impl Fn(f32, f32) -> [f32; 2], margin: usize) -> f32 {
    let (mut sum, mut n) = (0.0, 0);
    for y in margin..flow.height - margin {
        for x in margin..flow.width - margin {
            let (u, t) = (flow.data[y * flow.width + x], truth(x as f32, y as f32));
            sum += ((u[0] - t[0]).powi(2) + (u[1] - t[1]).powi(2)).sqrt();
            n += 1;
        }
    }
    sum / n as f32
}

#[test]
fn translation_is_found() {
    let (dx, dy) = (3.4, -2.3);
    let from = render(160, 120, scene);
    let to = render(160, 120, |x, y| scene(x - dx, y - dy));
    let f = flow(&from, &to, &FlowParams::default());
    let e = endpoint_error(&f, |_, _| [dx, dy], 8);
    assert!(e < 0.1, "erreur {e}");
}

#[test]
fn large_motion_goes_through_the_pyramid() {
    let (dx, dy) = (21.5, -13.0);
    let from = render(240, 180, scene);
    let to = render(240, 180, |x, y| scene(x - dx, y - dy));
    let f = flow(&from, &to, &FlowParams::default());
    // Sans la passe retour, des rangées de patchs restaient fausses aux frontières entre fils
    // (erreur moyenne 2,5).
    let e = endpoint_error(&f, |_, _| [dx, dy], 24);
    assert!(e < 0.3, "erreur {e}");
    // En s'arrêtant à la moitié de la taille, le flux agrandi reste juste.
    let half = flow(&from, &to, &FlowParams { finest_level: 1, ..FlowParams::default() });
    let e = endpoint_error(&half, |_, _| [dx, dy], 24);
    assert!(e < 0.3, "erreur à mi-taille {e}");
}

#[test]
fn rotation_and_zoom() {
    // to(x) = from(A⁻¹(x − c) + c) : le pixel x de `from` est en A(x − c) + c dans `to`.
    let (angle, scale, c) = (3f32.to_radians(), 1.04f32, [100.0f32, 75.0f32]);
    let (cs, sn) = (angle.cos(), angle.sin());
    let from = render(200, 150, scene);
    let to = render(200, 150, |x, y| {
        let (px, py) = (x - c[0], y - c[1]);
        scene((cs * px + sn * py) / scale + c[0], (-sn * px + cs * py) / scale + c[1])
    });
    let f = flow(&from, &to, &FlowParams::default());
    let truth = |x: f32, y: f32| {
        let (px, py) = (x - c[0], y - c[1]);
        [scale * (cs * px - sn * py) + c[0] - x, scale * (sn * px + cs * py) + c[1] - y]
    };
    let e = endpoint_error(&f, truth, 16);
    assert!(e < 0.3, "erreur {e}");
}

#[test]
fn same_image_does_not_move() {
    let im = render(120, 90, scene);
    let f = flow(&im, &im, &FlowParams { threads: 3, ..FlowParams::default() });
    assert!(f.data.iter().all(|u| u[0].abs() < 0.05 && u[1].abs() < 0.05));
}

#[test]
fn resized_scales_the_vectors() {
    let f = Flow { width: 10, height: 8, data: vec![[2.0, 1.0]; 80] };
    let g = f.resized(20, 32);
    assert!(g.data.iter().all(|u| (u[0] - 4.0).abs() < 1e-5 && (u[1] - 4.0).abs() < 1e-5));
}
