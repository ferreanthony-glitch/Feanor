// Le dessin de l'icône est celui de l'application (`src/logo.rs`, sans autre dépendance que
// tiny-skia) : il sert ici à fabriquer l'icône de `feanor.exe`.
#[allow(dead_code)]
#[path = "src/logo.rs"]
mod logo;

use std::path::{Path, PathBuf};

fn main() {
    // La toolchain Windows « GNU » minimale de rustup ne fournit pas toutes les bibliothèques
    // d'import Windows : `link/` contient celle de shlwapi, réclamée par une dépendance d'egui.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "gnu" {
        let dir = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("link");
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    if target_os == "windows" && target_env == "msvc" {
        embarquer_icone();
        embarquer_manifeste();
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/logo.rs");
    println!("cargo:rerun-if-env-changed=SHOT_TAS_STANDARD");
}

/// Le manifeste de l'application, mis dans `feanor.exe` (et dans les exécutables des essais, qui mesurent ainsi ce que FEANOR
/// fait vraiment) par l'éditeur de liens de MSVC, sans autre outil : `/MANIFEST:EMBED` + `/MANIFESTINPUT`.
///
/// Il ne dit qu'une chose : le **tas segmenté** de Windows 10 2004 et plus (`heapType` = `SegmentHeap`) au lieu du tas
/// par défaut. Le tas par défaut rend les blocs de 16 Ko des tuiles (`Arc<[u32; 4096]>`) en un temps qui grandit comme
/// le carré de leur nombre : 160 000 tuiles (un film de 300 plans) se rendaient en 8 min 14 s dans un programme à part,
/// et FEANOR mettait un quart d'heure à se fermer ou à remplacer le projet, fenêtre « ne répond pas » ; avec le tas
/// segmenté, 0,7 s (`docs/PERFORMANCE.md`, section 25). Un Windows plus ancien ignore la ligne.
///
/// Rien d'autre n'y est : il n'y avait pas de manifeste avant (winit règle la mise à l'échelle de l'écran au démarrage,
/// les boîtes de dialogue sont celles du système), et `/MANIFESTUAC:NO` n'ajoute pas le bloc `trustInfo` que l'éditeur de
/// liens met sinon d'office. `SHOT_TAS_STANDARD=1` à la compilation le retire (mesures de comparaison seulement).
fn embarquer_manifeste() {
    if std::env::var_os("SHOT_TAS_STANDARD").is_some() {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let manifeste = out.join("feanor.manifest");
    std::fs::write(
        &manifeste,
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <assembly xmlns=\"urn:schemas-microsoft-com:asm.v1\" manifestVersion=\"1.0\">\n\
         \x20 <application xmlns=\"urn:schemas-microsoft-com:asm.v3\">\n\
         \x20   <windowsSettings>\n\
         \x20     <heapType xmlns=\"http://schemas.microsoft.com/SMI/2020/WindowsSettings\">SegmentHeap</heapType>\n\
         \x20   </windowsSettings>\n\
         \x20 </application>\n\
         </assembly>\n",
    )
    .expect("écrire feanor.manifest");
    // (Pour tout ce que le paquet lie : l'exécutable, et les exécutables d'essais de la bibliothèque.)
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTUAC:NO");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifeste.display());
}

/// Met l'icône de FEANOR et les informations de version dans `feanor.exe` : un fichier de ressources
/// Windows compilé par `rc.exe` (Windows SDK) et passé à l'éditeur de liens. Sans `rc.exe` (SDK
/// absent), l'exécutable se construit quand même, sans icône dans l'Explorateur : la fenêtre,
/// elle, a son icône (`ViewportBuilder::with_icon`).
fn embarquer_icone() {
    let Some(rc) = trouver_rc() else { return };
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let ico = out.join("feanor.ico");
    std::fs::write(&ico, logo::ico(&[16, 24, 32, 48, 64, 128, 256])).expect("écrire feanor.ico");
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".into());
    let mut n = version.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let (a, b, c) = (n.next().unwrap_or(0), n.next().unwrap_or(0), n.next().unwrap_or(0));
    // Le fichier .rc reste en ASCII (pas de page de code à deviner) ; les chemins y prennent des « / ».
    let script = format!(
        "1 ICON \"{ico}\"\n\
         1 VERSIONINFO\n\
         FILEVERSION {a},{b},{c},0\n\
         PRODUCTVERSION {a},{b},{c},0\n\
         FILEOS 0x4\n\
         FILETYPE 0x1\n\
         BEGIN\n\
         BLOCK \"StringFileInfo\"\n\
         BEGIN\n\
         BLOCK \"040C04B0\"\n\
         BEGIN\n\
         VALUE \"FileDescription\", \"FEANOR - Forge your story\"\n\
         VALUE \"ProductName\", \"FEANOR\"\n\
         VALUE \"InternalName\", \"feanor\"\n\
         VALUE \"OriginalFilename\", \"feanor.exe\"\n\
         VALUE \"FileVersion\", \"{version}\"\n\
         VALUE \"ProductVersion\", \"{version}\"\n\
         END\n\
         END\n\
         BLOCK \"VarFileInfo\"\n\
         BEGIN\n\
         VALUE \"Translation\", 0x040C, 1200\n\
         END\n\
         END\n",
        ico = ico.display().to_string().replace('\\', "/"),
    );
    let rc_file = out.join("feanor.rc");
    std::fs::write(&rc_file, script).expect("écrire feanor.rc");
    let res = out.join("feanor.res");
    match std::process::Command::new(&rc).arg("/nologo").arg("/fo").arg(&res).arg(&rc_file).output() {
        Ok(o) if o.status.success() => println!("cargo:rustc-link-arg-bins={}", res.display()),
        Ok(o) => println!("cargo:warning=rc.exe a échoué, pas d'icône dans feanor.exe : {}", String::from_utf8_lossy(&o.stdout).trim()),
        Err(e) => println!("cargo:warning=rc.exe n'a pas pu démarrer, pas d'icône dans feanor.exe : {e}"),
    }
}

/// `rc.exe` du Windows SDK : la variable `RC` si elle existe, sinon la plus récente version
/// installée sous `Windows Kits\10\bin`.
fn trouver_rc() -> Option<PathBuf> {
    if let Some(rc) = std::env::var_os("RC").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(rc);
    }
    let racine = std::env::var_os("ProgramFiles(x86)").or_else(|| std::env::var_os("ProgramFiles"))?;
    let bin = Path::new(&racine).join("Windows Kits").join("10").join("bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(&bin).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    // Les dossiers « 10.0.26100.0 » : ordre des numéros, pas du texte.
    let cle = |p: &PathBuf| -> Vec<u32> { p.file_name().and_then(|n| n.to_str()).unwrap_or("").split('.').map(|s| s.parse().unwrap_or(0)).collect() };
    versions.sort_by_key(cle);
    versions.reverse();
    versions.into_iter().map(|v| v.join("x64").join("rc.exe")).find(|p| p.is_file())
}
