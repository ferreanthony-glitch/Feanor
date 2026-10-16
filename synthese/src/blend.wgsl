// Résolution sur la carte graphique du système du mélange (blend.rs) : (λ + Laplacien) x = rhs,
// par Gauss-Seidel rouge-noir sur-relaxé (SOR). Chaque pixel porte jusqu'à 4 canaux.

struct Relax {
    w: u32,
    h: u32,
    parity: u32,   // 0 : pixels où x + y est pair, 1 : impair
    pad0: u32,
    lambda: f32,
    omega: f32,    // facteur de sur-relaxation
    pad1: f32,
    pad2: f32,
}

@group(0) @binding(0) var<uniform> R: Relax;
@group(0) @binding(1) var<storage, read_write> x: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> rhs: array<vec4<f32>>;

@compute @workgroup_size(16, 16)
fn relax(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= R.w || id.y >= R.h || ((id.x + id.y) & 1u) != R.parity) {
        return;
    }
    let p = id.y * R.w + id.x;
    var sum = vec4<f32>(0.0);
    var deg = 0.0;
    if (id.x > 0u) {
        sum += x[p - 1u];
        deg += 1.0;
    }
    if (id.x + 1u < R.w) {
        sum += x[p + 1u];
        deg += 1.0;
    }
    if (id.y > 0u) {
        sum += x[p - R.w];
        deg += 1.0;
    }
    if (id.y + 1u < R.h) {
        sum += x[p + R.w];
        deg += 1.0;
    }
    let gauss_seidel = (rhs[p] + sum) / (deg + R.lambda);
    x[p] = x[p] + R.omega * (gauss_seidel - x[p]);
}
