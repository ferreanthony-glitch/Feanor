fn main() {
    // La toolchain Windows « GNU » minimale de rustup ne fournit pas toutes les bibliothèques
    // d'import Windows : `link/` contient celle de shlwapi, réclamée par une dépendance d'egui.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "gnu" {
        let dir = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("link");
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    println!("cargo:rerun-if-changed=build.rs");
}
