//! Dessins d'un projet Storyboard Pro (14a). Pelure écrit un script que l'on lance dans
//! Storyboard Pro, projet ouvert (vue « Script Editor », « Sandbox », bouton « Run » : la
//! marche indiquée par la documentation de son interface de script). Le script n'utilise que
//! des fonctions documentées de cette interface : pour chaque panneau et chaque calque de
//! dessin non vide, il ne laisse voir que ce calque (et ses groupes), exporte le panneau en PNG
//! sur fond transparent dans son propre dossier, puis remet la visibilité des calques comme
//! elle était (et annule tout, en une fois, à la fin). Il écrit enfin la liste des panneaux et
//! des calques (`MANIFEST`), que Pelure lit à l'import.
//!
//! Le fichier `MANIFEST` : une ligne par chose, champs séparés par des tabulations.
//! - `pelure-sboard	1`
//! - `projet	<chemin>	<cadence>	<largeur>	<hauteur>`
//! - `panneau	<rang>	<id>	<séquence>	<plan>	<nom>	<durée>`
//! - `calque	<rang du panneau>	<rang du calque>	<nom>	<visible 0/1>	<verrouillé 0/1>	<v|b|3d>	<image>`
//!   (`image` : chemin relatif au dossier, vide si le calque n'a pas de dessin).

use std::path::{Path, PathBuf};

/// Nom de la liste écrite par le script.
pub const MANIFEST: &str = "pelure.txt";
/// Nom du script écrit par Pelure.
pub const SCRIPT: &str = "export_pelure.js";

/// Dessins exportés par le script.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawings {
    pub dir: PathBuf,
    pub fps: Option<f64>,
    pub size: Option<[usize; 2]>,
    pub panels: Vec<DPanel>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DPanel {
    pub index: usize,
    pub id: String,
    pub scene: String,
    pub frames: usize,
    pub layers: Vec<DLayer>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DLayer {
    pub index: usize,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    /// Image du calque seul (chemin complet), s'il a un dessin.
    pub file: Option<PathBuf>,
}

/// Le script, qui exporte dans le dossier `dest`.
pub fn script(dest: &Path) -> String {
    let dest = dest.to_string_lossy().replace('\\', "/").replace('"', "\\\"");
    SCRIPT_TEXT.replace("@DOSSIER@", &dest)
}

/// Lit la liste écrite par le script dans le dossier `dir`.
pub fn read_manifest(dir: &Path) -> Result<Drawings, String> {
    let path = dir.join(MANIFEST);
    let bytes = std::fs::read(&path).map_err(|_| {
        format!("« {} » est introuvable : lancer d'abord le script dans Storyboard Pro.", path.display())
    })?;
    // Le script écrit en UTF-8 ; sinon, on lit les octets un à un (Latin-1).
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => e.into_bytes().iter().map(|&b| b as char).collect(),
    };
    parse_manifest(&text, dir)
}

/// Lit le texte de la liste ; les images sont cherchées sous `dir`.
pub fn parse_manifest(text: &str, dir: &Path) -> Result<Drawings, String> {
    let mut d = Drawings { dir: dir.to_path_buf(), ..Default::default() };
    let mut seen_header = false;
    for line in text.lines() {
        let f: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
        let num = |i: usize| f.get(i).and_then(|v| v.trim().parse::<usize>().ok());
        match f[0] {
            "pelure-sboard" => seen_header = true,
            "projet" => {
                d.fps = f.get(2).and_then(|v| v.trim().parse::<f64>().ok());
                if let (Some(w), Some(h)) = (num(3), num(4)) {
                    d.size = Some([w, h]);
                }
            }
            "panneau" => {
                let Some(index) = num(1) else { continue };
                d.panels.push(DPanel {
                    index,
                    id: f.get(2).unwrap_or(&"").to_string(),
                    scene: f.get(4).unwrap_or(&"").to_string(),
                    frames: num(6).unwrap_or(0),
                    layers: Vec::new(),
                });
            }
            "calque" => {
                let (Some(p), Some(index)) = (num(1), num(2)) else { continue };
                let Some(panel) = d.panels.iter_mut().rev().find(|x| x.index == p) else { continue };
                let rel = f.get(7).map(|s| s.trim()).unwrap_or("");
                let file = if rel.is_empty() { None } else { found_image(dir, rel) };
                panel.layers.push(DLayer {
                    index,
                    name: f.get(3).unwrap_or(&"").to_string(),
                    visible: f.get(4).map(|v| v.trim()) != Some("0"),
                    locked: f.get(5).map(|v| v.trim()) == Some("1"),
                    file,
                });
            }
            _ => {}
        }
    }
    if !seen_header {
        return Err("ce fichier n'est pas une liste écrite par le script de Pelure".into());
    }
    Ok(d)
}

/// L'image `rel` sous `dir`, ou à défaut la première image de son dossier.
fn found_image(dir: &Path, rel: &str) -> Option<PathBuf> {
    let path = dir.join(rel);
    if path.is_file() {
        return Some(path);
    }
    let folder = path.parent()?;
    let mut images: Vec<PathBuf> = std::fs::read_dir(folder)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")))
        .collect();
    images.sort();
    images.into_iter().next()
}

/// Le script (ECMAScript 3 : l'interface de script de Storyboard Pro est en Qt Script).
const SCRIPT_TEXT: &str = r#"// Pelure : export des dessins de ce projet Storyboard Pro, calque par calque.
// A lancer dans Storyboard Pro, le projet ouvert : vue Script Editor, Sandbox,
// coller ce texte, puis Run. Le projet n'est pas modifie : la visibilite des
// calques est remise comme avant, et toutes les etapes sont annulees a la fin.
// Les images et la liste "pelure.txt" vont dans le dossier :
var PELURE_DOSSIER = "@DOSSIER@";

function pelureNettoie(s)
{
  return String(s).replace(/[\t\r\n]/g, " ");
}

function pelureZeros(v, n)
{
  var s = "" + v;
  while (s.length < n) s = "0" + s;
  return s;
}

function pelureExporte()
{
  var sb = new StoryboardManager();
  var lm = new LayerManager();
  var exp = new ExportManager();
  exp.setTransparentBG(true);
  exp.setShowCamera(false);
  exp.setCameraScaling(false);
  exp.setBitmapFitCameraPath(false);
  exp.setMaintainSize(true);

  var racine = new Dir;
  racine.path = PELURE_DOSSIER + "/images";
  racine.mkdirs();

  var lignes = new Array(0);
  lignes.push("pelure-sboard\t1");
  lignes.push("projet\t" + pelureNettoie(project.currentProjectPath()) + "\t" + project.getFrameRate()
    + "\t" + project.defaultResolutionX() + "\t" + project.defaultResolutionY());

  var n = sb.numberOfPanelsInProject();
  project.beginUndoRedoAccum("Pelure");
  try
  {
    for (var k = 0; k < n; ++k)
    {
      var panelId = sb.panelInProject(k);
      var sceneId = sb.sceneIdOfPanel(panelId);
      var seqId = sb.sequenceIdOfScene(sceneId);
      lignes.push("panneau\t" + k + "\t" + panelId + "\t" + pelureNettoie(sb.nameOfSequence(seqId))
        + "\t" + pelureNettoie(sb.nameOfScene(sceneId)) + "\t" + pelureNettoie(sb.nameOfPanel(panelId))
        + "\t" + sb.getPanelDuration(panelId));
      MessageLog.trace("Pelure : panneau " + (k + 1) + " / " + n);

      var count = lm.numberOfLayers(panelId);
      var visible = new Array(count);
      var dessin = new Array(count);
      for (var j = 0; j < count; ++j)
      {
        visible[j] = lm.layerVisibility(panelId, j);
        dessin[j] = !lm.isGroupLayer(panelId, j)
          && (lm.isVectorLayer(panelId, j) || lm.isBitmapLayer(panelId, j) || lm.is3DLayer(panelId, j));
      }
      for (var j = 0; j < count; ++j)
      {
        if (!dessin[j]) continue;
        var sorte = lm.isVectorLayer(panelId, j) ? "v" : (lm.isBitmapLayer(panelId, j) ? "b" : "3d");
        var image = "";
        if (!lm.isEmpty(panelId, j))
        {
          for (var o = 0; o < count; ++o)
            if (dessin[o]) lm.setLayerVisible(panelId, o, o == j);
          var g = lm.groupOfLayer(panelId, j);
          while (g >= 0)
          {
            lm.setLayerVisible(panelId, g, true);
            g = lm.groupOfLayer(panelId, g);
          }
          var sous = "images/" + pelureZeros(k, 4) + "/" + pelureZeros(j, 3);
          var d = new Dir;
          d.path = PELURE_DOSSIER + "/" + sous;
          d.mkdirs();
          var choix = new Array(0);
          choix.push(panelId);
          exp.setSelectedPanels(choix);
          exp.exportToBitmap(PELURE_DOSSIER + "/" + sous, "calque", "png");
          var noms = d.entryList("*.png");
          if (noms.length > 0) image = sous + "/" + noms[0];
        }
        lignes.push("calque\t" + k + "\t" + j + "\t" + pelureNettoie(lm.layerName(panelId, j))
          + "\t" + (visible[j] ? 1 : 0) + "\t" + (lm.getLayerLock(panelId, j) ? 1 : 0)
          + "\t" + sorte + "\t" + image);
      }
      for (var j = 0; j < count; ++j)
        lm.setLayerVisible(panelId, j, visible[j]);
    }
  }
  finally
  {
    project.cancelUndoRedoAccum();
  }

  var f = new File(PELURE_DOSSIER + "/pelure.txt");
  f.open(FileAccess.WriteOnly);
  for (var i = 0; i < lignes.length; ++i) f.writeLine(lignes[i]);
  f.close();
  MessageBox.information("Pelure : " + n + " panneaux exportes dans " + PELURE_DOSSIER);
}

pelureExporte();
"#;
