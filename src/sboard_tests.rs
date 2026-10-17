//! Import des projets Storyboard Pro (14a) : XML, lecture du `.sboard`, liste du script,
//! vitesse des sons, et projet Pelure construit.

use std::path::{Path, PathBuf};

use crate::sboard::{self, SbClip};
use crate::sbscript;
use crate::stretch;
use crate::xml;

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pelure-test-sboard-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn xml_tree_with_decoded_attributes() {
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<a x="1 &lt; 2 &amp; &quot;3&quot; &#233;">
  <b id="un"/>
  <c>texte &amp; suite<d/></c>
  <b id="deux"><e v="z"/></b>
</a>"#;
    let root = xml::parse(src).unwrap();
    assert_eq!(root.name, "a");
    assert_eq!(root.attr("x"), Some("1 < 2 & \"3\" é"));
    assert_eq!(root.children_named("b").map(|b| b.attr("id").unwrap()).collect::<Vec<_>>(), vec!["un", "deux"]);
    assert_eq!(root.child("c").unwrap().text, "texte & suite");
    assert_eq!(root.path("b/e"), None, "le premier <b> n'a pas d'enfant");
    let mut all = Vec::new();
    root.find_all("e", &mut all);
    assert_eq!(all.len(), 1);
    assert!(xml::parse("<a><b></a>").is_err());
}

#[test]
fn captions_become_plain_text() {
    let html = "<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.0//EN\">\n<html><head><meta name=\"qrichtext\" content=\"1\" /><style type=\"text/css\">\np, li { white-space: pre-wrap; }\n</style></head><body style=\" font-family:'Arial';\">\n<p style=\" margin-top:0px;\"> You need them.<br />And they need you.</p>\n<p style=\" margin-top:0px;\">L&apos;équipe &amp; moi&nbsp;!</p></body></html>";
    assert_eq!(sboard::caption_text(html), "You need them.\nAnd they need you.\nL'équipe & moi !");
    assert_eq!(sboard::caption_text("  simple  "), "simple");
    assert_eq!(sboard::caption_text(""), "");
}

/// Un petit projet comme ceux que Storyboard Pro écrit : deux plans (le second de deux
/// panneaux), une séquence, des légendes, des calques, deux pistes son.
pub(crate) const SBOARD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<project source="essai" version="2700">
 <options>
  <resolution name="HD" size="64,32"/>
  <framerate val="25"/>
 </options>
 <scenes>
  <scene name="Top" id="top" nbframes="30">
   <columns>
    <column type="0" name="ATV-1" id="1">
     <warpSeq exposures="1-10" id="s1" start="1" end="10"/>
     <warpSeq exposures="11-30" id="s2" start="1" end="20"/>
    </column>
    <column type="1" name="A1" disabled="false">
     <soundSequence startFrame="3" stopFrame="12" name="bip.wav" clippingTimeStart="0.2" clippingTimeStop="1"/>
     <soundSequence startFrame="15" stopFrame="20" name="bip.wav" clippingTimeStart="0" clippingTimeStop="0.5" volume="0.5">
      <soundEffect name="WSOLATempo" disabled="false"><attrs><tempo val="200" defaultValue="100"/></attrs></soundEffect>
      <soundEffect name="Reverb" disabled="false"><attrs><mixin val="40"/></attrs></soundEffect>
     </soundSequence>
    </column>
    <column type="1" name="A2" disabled="true">
     <soundSequence startFrame="1" stopFrame="5" name="absent.wav" clippingTimeStart="0" clippingTimeStop="3"/>
    </column>
   </columns>
  </scene>
  <scene name="shot" id="s1" nbframes="10">
   <columns><column type="0" name="ATV-2" id="1"><warpSeq exposures="1-10" id="p1" start="1" end="10"/></column></columns>
   <metas><meta type="sceneInfo"><sceneInfo name="01" sequenceName="A"/></meta></metas>
  </scene>
  <scene name="shot (2)" id="s2" nbframes="20">
   <columns><column type="0" name="ATV-3" id="1">
    <warpSeq exposures="1-8" id="p2" start="1" end="8"/>
    <warpSeq exposures="9-20" id="p3" start="1" end="12"/>
   </column></columns>
   <metas><meta type="sceneInfo"><sceneInfo name="02" sequenceName="A"/></meta></metas>
  </scene>
  <scene name="panel" id="p1" nbframes="10">
   <rootgroup name="Top"><nodeslist>
    <module type="READ" name="A"><attrs><opacity val="100"/><lineArtDrawingMode val="VectorDrawingMode"/></attrs></module>
    <module type="GAUSSIANBLUR-PLUGIN" name="Blur"/>
    <module type="READ" name="BG"><attrs><opacity val="51"/><lineArtDrawingMode val="BitmapDrawingMode"/></attrs></module>
   </nodeslist></rootgroup>
   <metas><meta type="infoFields"><infoFields>
    <field id="1" type="TEXT" value="&lt;html&gt;&lt;body&gt;&lt;p&gt;Bonjour.&lt;/p&gt;&lt;/body&gt;&lt;/html&gt;"/>
    <field id="2" type="TEXT" value="Il entre."/>
    <field id="3" type="TEXT" value="Int. jour"/>
    <field id="4" type="TEXT" value=""/>
   </infoFields></meta></metas>
  </scene>
  <scene name="panel (2)" id="p2" nbframes="8">
   <rootgroup name="Top"><nodeslist>
    <module type="READ" name="A"/>
    <module type="READ" name="BG"/>
   </nodeslist></rootgroup>
   <metas><meta type="infoFields"><infoFields><field id="1" type="TEXT" value="Au revoir."/></infoFields></meta></metas>
  </scene>
  <scene name="panel (3)" id="p3" nbframes="12">
   <rootgroup name="Top"><nodeslist><module type="READ" name="BG"/></nodeslist></rootgroup>
  </scene>
  <scene name="orphelin" id="o1" nbframes="5"/>
 </scenes>
 <timeline><scene id="top"/></timeline>
 <metas>
  <meta type="scenePanelInfoFieldList" name="scenePanelInfoFieldList"><scenePanelInfoFieldList>
   <field id="1" name="Dialog"/><field id="2" name="Action Notes"/><field id="3" name="Slugging"/><field id="4" name="Notes"/>
  </scenePanelInfoFieldList></meta>
  <meta type="string" name="projectTitle"><string value="Mon storyboard"/></meta>
 </metas>
</project>"#;

#[test]
fn sboard_structure_captions_layers_and_sound() {
    let sb = sboard::parse(SBOARD, "fichier").unwrap();
    assert_eq!((sb.title.as_str(), sb.fps, sb.size), ("Mon storyboard", 25, [64, 32]));
    assert_eq!(sb.shots.len(), 2, "le panneau orphelin (hors du film) n'est pas repris");
    let (a, b) = (&sb.shots[0], &sb.shots[1]);
    assert_eq!((a.name.as_str(), a.sequence.as_str(), a.start), ("01", "A", 1));
    assert_eq!((b.name.as_str(), b.start), ("02", 11));
    assert_eq!(b.panels.iter().map(|p| p.frames).collect::<Vec<_>>(), vec![8, 12]);
    let p = &a.panels[0];
    assert_eq!((p.dialogue.as_str(), p.action.as_str(), p.notes.as_str()), ("Bonjour.", "Il entre.", "Slugging : Int. jour"));
    assert_eq!(p.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), vec!["A", "BG"], "du premier plan au fond, sans l'effet");
    assert!(p.layers[0].vector && !p.layers[1].vector);
    assert!((p.layers[1].opacity - 0.51).abs() < 1e-6);
    assert_eq!(b.panels[0].dialogue, "Au revoir.");

    assert_eq!(sb.tracks.len(), 2);
    assert!(!sb.tracks[0].muted && sb.tracks[1].muted);
    let c = &sb.tracks[0].clips[1];
    assert_eq!((c.start, c.stop, c.frames()), (15, 20, 6));
    assert_eq!((c.tempo, c.speed, c.volume), (2.0, 1.0, 0.5));
    assert_eq!(c.other_effects, vec!["Reverb".to_string()]);
    assert!(sboard::parse("<autre/>", "x").is_err());
}

#[test]
fn script_manifest_is_read_back() {
    let dir = temp("liste");
    let img = dir.join("images/0001/002");
    std::fs::create_dir_all(&img).unwrap();
    std::fs::write(img.join("calque-01-0001.png"), b"x").unwrap();
    let text = "pelure-sboard\t1\nprojet\tC:/p.sboard\t25\t1920\t1080\n\
                panneau\t0\tp1\tA\t01\t\t10\n\
                calque\t0\t0\tA\t1\t0\tv\t\n\
                panneau\t1\tp2\tA\t02\t\t8\n\
                calque\t1\t2\tBG\t0\t1\tb\timages/0001/002/autre-nom.png\n";
    let d = sbscript::parse_manifest(text, &dir).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!((d.fps, d.size), (Some(25.0), Some([1920, 1080])));
    assert_eq!(d.panels.len(), 2);
    assert_eq!(d.panels[0].layers[0].file, None);
    let l = &d.panels[1].layers[0];
    assert_eq!((l.name.as_str(), l.visible, l.locked, l.index), ("BG", false, true, 2));
    assert!(l.file.as_ref().unwrap().ends_with("calque-01-0001.png"), "fichier nommé autrement : la première image du dossier");
    assert!(sbscript::parse_manifest("rien", &dir).is_err());
    let s = sbscript::script(Path::new(r"C:\Dossier\Dessins"));
    assert!(s.contains("var PELURE_DOSSIER = \"C:/Dossier/Dessins\";"));
    assert!(s.contains("exportToBitmap") && s.contains("cancelUndoRedoAccum"));
}

/// Un son stéréo : la note `hz` pendant `seconds`.
fn sine(hz: f32, seconds: f32) -> Vec<[f32; 2]> {
    let n = (seconds * crate::audio::RATE as f32) as usize;
    (0..n)
        .map(|i| {
            let v = (std::f32::consts::TAU * hz * i as f32 / crate::audio::RATE as f32).sin() * 0.5;
            [v, v]
        })
        .collect()
}

/// Fréquence mesurée par les passages par zéro (en montant).
fn frequency(s: &[[f32; 2]]) -> f32 {
    let ups = s.windows(2).filter(|w| w[0][0] < 0.0 && w[1][0] >= 0.0).count();
    ups as f32 / (s.len() as f32 / crate::audio::RATE as f32)
}

#[test]
fn speed_changes_keep_or_change_the_pitch() {
    let src = sine(440.0, 1.0);
    let fast = stretch::resample(&src, 2.0);
    assert_eq!(fast.len(), src.len() / 2);
    assert!((frequency(&fast) - 880.0).abs() < 10.0, "rééchantillonné : la hauteur monte ({})", frequency(&fast));
    let tempo = stretch::wsola(&src, 1.5);
    assert_eq!(tempo.len(), (src.len() as f64 / 1.5) as usize);
    let f = frequency(&tempo[2048..tempo.len() - 2048]);
    assert!((f - 440.0).abs() < 15.0, "WSOLA : même hauteur ({f})");
    let slow = stretch::wsola(&src, 0.5);
    assert_eq!(slow.len(), src.len() * 2);
    let f = frequency(&slow[2048..slow.len() - 2048]);
    assert!((f - 440.0).abs() < 15.0, "WSOLA ralenti : même hauteur ({f})");
    let (pcm, bytes) = stretch::to_sound(&src);
    assert_eq!(pcm.frames(), src.len());
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(bytes.len(), 44 + src.len() * 4);
}

/// Un WAV 16 bits stéréo à 48 kHz.
fn wav(samples: &[[f32; 2]]) -> Vec<u8> {
    stretch::to_sound(samples).1
}

#[test]
fn processed_clip_plays_the_stretched_part() {
    // Une seconde de son : 0,5 s à 440 Hz puis 0,5 s de silence.
    let mut src = sine(440.0, 0.5);
    src.extend(vec![[0.0, 0.0]; src.len()]);
    let pcm = stretch::to_sound(&src).0;
    // Tempo 200 % : 0,5 s de son étiré ; l'extrait en joue de 0,1 s à 0,3 s (5 images à 25 i/s).
    let clip = SbClip {
        file: "x.wav".into(),
        start: 1,
        stop: 5,
        from: 0.1,
        to: 0.5,
        volume: 0.5,
        tempo: 2.0,
        speed: 1.0,
        envelope: false,
        other_effects: Vec::new(),
    };
    let s = crate::sbimport::processed_clip(&pcm, &clip, 25.0);
    assert_eq!(s.len(), (0.2 * crate::audio::RATE as f64).round() as usize);
    // Temps étiré 0,1 → 0,3 s = son d'origine 0,2 → 0,6 s : la note, puis le silence.
    let peak = |a: usize, b: usize| s[a..b].iter().fold(0f32, |m, v| m.max(v[0].abs()));
    assert!(peak(0, 2000) > 0.2 && peak(0, 2000) < 0.26, "volume 0,5 sur une note de 0,5 ({})", peak(0, 2000));
    assert!(peak(s.len() - 1000, s.len()) < 0.01, "la fin de l'extrait tombe dans le silence");
}

#[test]
fn sboard_becomes_plans_with_marks_drawings_and_sound() {
    let dir = temp("projet");
    std::fs::create_dir_all(dir.join("audio")).unwrap();
    std::fs::write(dir.join("audio/bip.wav"), wav(&sine(440.0, 1.0))).unwrap();
    // Dessins exportés : le calque A du panneau p1 (rouge), et BG identique dans p1 et p2.
    let red = image::RgbaImage::from_pixel(64, 32, image::Rgba([255, 0, 0, 255]));
    let mut blue = image::RgbaImage::new(64, 32);
    blue.put_pixel(10, 10, image::Rgba([0, 0, 255, 255]));
    for (rel, img) in [("i/a1.png", &red), ("i/bg1.png", &blue), ("i/bg2.png", &blue)] {
        std::fs::create_dir_all(dir.join("i")).unwrap();
        img.save(dir.join(rel)).unwrap();
    }
    let manifest = "pelure-sboard\t1\n\
                    panneau\t0\tp1\tA\t01\t\t10\ncalque\t0\t0\tBG\t1\t1\tb\ti/bg1.png\ncalque\t0\t1\tA\t1\t0\tv\ti/a1.png\n\
                    panneau\t1\tp2\tA\t02\t\t8\ncalque\t1\t0\tA\t0\t0\tv\t\ncalque\t1\t1\tBG\t1\t1\tb\ti/bg2.png\n";
    let drawings = sbscript::parse_manifest(manifest, &dir).unwrap();
    let sb = sboard::parse(SBOARD, "x").unwrap();
    let (doc, warnings) = crate::sbimport::build(&sb, &dir, Some(&drawings), &|_| {});
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!((doc.frame, doc.fps, doc.properties.title.as_str()), ([64, 32], 25, "Mon storyboard"));
    assert_eq!(doc.sequences.len(), 1, "les deux plans sont dans la séquence A");
    assert_eq!(doc.sequences[0].name, "A");
    assert_eq!(doc.panels.len(), 2, "un plan par plan");
    assert_eq!(doc.scenes.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["01", "02"]);
    let (p1, p2) = (&doc.panels[0], &doc.panels[1]);
    assert_eq!((p1.frame_count(), p2.frame_count()), (10, 20));
    assert_eq!((p1.dialogue.as_str(), p1.action.as_str()), ("Bonjour.", "Il entre."));
    assert_eq!(p2.marks.len(), 1, "le second panneau du plan 02 devient une image de storyboard");
    assert_eq!(p2.marks[0].frame, 8);
    assert_eq!(p2.dialogue, "Au revoir.");
    // Plan 01 : calques du fond au premier plan, rangés comme dans le projet (BG au fond).
    assert_eq!(p1.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), vec!["BG", "A"]);
    assert!(p1.layers[0].locked);
    let bg1 = p1.layers[0].frames[0].unwrap();
    assert_eq!(p1.layers[0].frames.len(), 10);
    let cel = &doc.cels[&bg1];
    // L'image (64×32) est au centre de la feuille (128×64) : son pixel (10, 10) en (42, 26).
    assert_eq!(cel.raster.get(42, 26).b(), 255);
    assert_eq!(cel.raster.get(41, 26).a(), 0);
    // BG identique dans p2 : le même dessin.
    let bg_p2 = p2.layers.iter().find(|l| l.name == "BG").unwrap();
    assert_eq!(bg_p2.frames[0], Some(bg1));

    // Son : piste A1 (un extrait tel quel, un extrait recalculé), piste A2 muette (son absent).
    assert_eq!(doc.audio_tracks.len(), 2);
    assert!(doc.audio_tracks[1].muted);
    let clips = &doc.audio_tracks[0].clips;
    assert_eq!(clips.len(), 2);
    assert_eq!((clips[0].start, clips[0].offset, clips[0].length), (2, 5, 10), "0,2 s à 25 i/s = 5 images");
    assert_eq!((clips[1].start, clips[1].offset, clips[1].length), (14, 0, 6));
    assert_ne!(clips[0].sound, clips[1].sound, "l'extrait au tempo changé a son propre son");
    assert!(warnings.iter().any(|w| w.contains("absent.wav")));
    assert!(warnings.iter().any(|w| w.contains("Reverb")));
}

/// Un faux export des dessins de la partie 1 (des carrés de couleur, la moitié de la taille
/// du projet), écrit dans le dossier `PELURE_FAUX_EXPORT`, pour un essai à l'écran.
#[test]
#[ignore]
fn fake_export_for_screen_check() {
    let Ok(out) = std::env::var("PELURE_FAUX_EXPORT") else { return };
    let out = PathBuf::from(out);
    let path = Path::new(r"C:\Users\ferre\Documents\Work\BRAWLSTAR\BRAWLIDAYS\BRAWLIDAYS_predec PART 1_101326_v04\BRAWLIDAYS_predec PART 1_101326_v01.1.sboard");
    let sb = sboard::read(path).unwrap();
    let mut text = String::from("pelure-sboard\t1\nprojet\tessai\t25\t960\t540\n");
    let mut k = 0;
    for shot in &sb.shots {
        for p in &shot.panels {
            text += &format!("panneau\t{k}\tfaux{k}\t{}\t{}\t\t{}\n", shot.sequence, shot.name, p.frames);
            // Le script range les calques dans son propre ordre : ici, à l'envers du projet.
            for (j, l) in p.layers.iter().rev().enumerate() {
                let rel = format!("images/{k:04}/{j:03}/calque.png");
                let mut img = image::RgbaImage::new(960, 540);
                let color = [(j * 70 % 256) as u8, (90 + j * 40 % 160) as u8, (200 - j * 30 % 180) as u8, 255];
                if l.name.starts_with("BG") {
                    for px in img.pixels_mut() {
                        *px = image::Rgba([230, 225, 210, 255]);
                    }
                } else {
                    let (x0, y0) = (40 + j as u32 * 70, 60 + (k as u32 % 6) * 70);
                    for y in y0..(y0 + 60).min(540) {
                        for x in x0..(x0 + 60).min(960) {
                            img.put_pixel(x, y, image::Rgba(color));
                        }
                    }
                }
                std::fs::create_dir_all(out.join(&rel).parent().unwrap()).unwrap();
                img.save(out.join(&rel)).unwrap();
                text += &format!("calque\t{k}\t{j}\t{}\t1\t0\tb\t{rel}\n", l.name);
            }
            k += 1;
        }
    }
    std::fs::write(out.join(sbscript::MANIFEST), text).unwrap();
}

/// Les vrais projets de l'utilisateur, lus seulement (`cargo test -- --ignored`).
#[test]
#[ignore]
fn real_sboard_project_reads() {
    let root = Path::new(r"C:\Users\ferre\Documents\Work\BRAWLSTAR\BRAWLIDAYS");
    let parts = [
        (root.join(r"BRAWLIDAYS_predec PART 1_101326_v04\BRAWLIDAYS_predec PART 1_101326_v01.1.sboard"), Some(834)),
        (root.join(r"BRAWLIDAYS_predec PART 2_101426_v01\BRAWLIDAYS_predec PART 2_101426_v01.sboard"), None),
    ];
    for (path, expected) in parts {
        let t = std::time::Instant::now();
        let sb = sboard::read(&path).unwrap();
        let read = t.elapsed();
        let panels: usize = sb.shots.iter().map(|s| s.panels.len()).sum();
        let frames: usize = sb.shots.iter().flat_map(|s| &s.panels).map(|p| p.frames).sum();
        println!("{} : {} plans, {panels} panneaux, {frames} images, {} pistes, lu en {read:?}, {:?}", sb.title, sb.shots.len(), sb.tracks.len(), sb.warnings);
        for s in sb.shots.iter().take(4) {
            println!("  {} (séquence « {} ») depuis {} : {:?}", s.name, s.sequence, s.start, s.panels.iter().map(|p| (p.frames, p.layers.len())).collect::<Vec<_>>());
        }
        assert_eq!(sb.fps, 25);
        if let Some(n) = expected {
            assert_eq!(frames, n, "la scène racine compte {n} images");
        }
        let (doc, warnings) = crate::sbimport::build(&sb, path.parent().unwrap(), None, &|_| {});
        println!("Pelure : {} plans, {} séquences, {} sons, en {:?} ; {:#?}", doc.panels.len(), doc.sequences.len(), doc.sounds.len(), t.elapsed(), warnings);
        assert_eq!(doc.total_frames(), frames);
    }
}

#[test]
fn project_copy_skips_logs_and_locks() {
    let src = temp("copie-src");
    std::fs::create_dir_all(src.join("elements/Draw")).unwrap();
    std::fs::create_dir_all(src.join("logs")).unwrap();
    std::fs::write(src.join("p.sboard"), b"x").unwrap();
    std::fs::write(src.join("p.sboard.lock"), b"x").unwrap();
    std::fs::write(src.join("elements/Draw/a.tvg"), b"xyz").unwrap();
    std::fs::write(src.join("logs/l.txt"), b"x").unwrap();
    let dest = temp("copie-dest");
    let bytes = crate::sbbatch::copy_project(&src, &dest.join("projet")).unwrap();
    let ok = (dest.join("projet/p.sboard").is_file(), dest.join("projet/elements/Draw/a.tvg").is_file());
    let skipped = (dest.join("projet/p.sboard.lock").exists(), dest.join("projet/logs").exists());
    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dest).ok();
    assert_eq!((ok, skipped, bytes), ((true, true), (false, false), 4));
}

#[test]
fn batch_command_line_and_versions() {
    let args = crate::sbbatch::batch_args(Path::new(r"C:\t\p.sboard"), Path::new(r"C:\t\s.js"));
    let args: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    assert_eq!(args, vec!["-scene", r"C:\t\p.sboard", "-batch", "-compile", r"C:\t\s.js"]);
    let v = crate::sbbatch::version_of(Path::new(r"C:\PF\Toon Boom Animation\Toon Boom Storyboard Pro 27\win64\bin\StoryboardPro.exe"));
    assert_eq!(v, 27);
    // Le script n'ouvre aucune boîte (mode batch) et lit les dessins vectoriels par getData, le
    // dessin désigné par son nom en « exposure » (traduit par Drawing.Key), jamais en
    // « drawingId » (l'identifiant interne : le nom y désigne un autre dessin).
    let s = sbscript::script(Path::new(r"C:\d"));
    assert!(!s.contains("MessageBox") && s.contains("Drawing.query.getData") && s.contains("Drawing.Key"));
    assert!(s.contains("exposure: noms[c]") && !s.contains("drawingId:"), "aucune clé en drawingId");
    assert!(s.contains("couleurs.json") && s.contains("\"getData\""));
}

#[test]
fn default_sequence_names_are_not_sequences() {
    // NAJI, BRAWLIDAYS partie 1 : « sequenceExists » vide, et « 0 » dans chaque plan.
    let with = |v: &str| {
        let meta = format!(" <meta type=\"bool\" name=\"sequenceExists\"><bool value=\"{v}\"/></meta>\n </metas>\n</project>");
        sboard::parse(&SBOARD.replace(" </metas>\n</project>", &meta), "f").unwrap()
    };
    assert!(with("").shots.iter().all(|s| s.sequence.is_empty()), "pas de séquence");
    assert!(with("true").shots.iter().all(|s| s.sequence == "A"), "les séquences du projet");
    // Sans la méta (projets plus anciens) : les noms sont gardés.
    assert!(sboard::parse(SBOARD, "f").unwrap().shots.iter().all(|s| s.sequence == "A"));
}

#[test]
fn drawing_colours_are_read_once_from_their_file() {
    let dir = temp("couleurs");
    std::fs::write(dir.join(sbscript::MANIFEST), "pelure-sboard\t1\n").unwrap();
    std::fs::write(dir.join(sbscript::COLORS), r#"{"noir": {"name": "Black", "r": 0, "g": 0, "b": 0, "a": 255}, "rouge": [255, 44, 0, 255]}"#).unwrap();
    let d = sbscript::read_manifest(&dir).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(d.palette.get("noir"), Some(&[0.0, 0.0, 0.0, 255.0]));
    assert_eq!(d.palette.get("rouge"), Some(&[255.0, 44.0, 0.0, 255.0]));
}

#[test]
fn vector_layer_comes_back_as_vectors() {
    // Un faux export : le dessin du tutoriel (traits) et son image, comme Storyboard Pro les écrirait.
    let dir = temp("vecteurs");
    std::fs::create_dir_all(dir.join("i")).unwrap();
    let traits = crate::sbvector_tests::tutorial();
    std::fs::write(dir.join("i/traits.json"), traits.to_string()).unwrap();
    let raw = crate::sbvector::raw_elements(&traits, &Default::default());
    // L'image à la taille du projet (64×32), dessin réduit et centré, axe vertical vers le bas.
    let s = 0.015;
    let tf = |x: f32, y: f32| [32.0 + (x + 555.0) * s, 16.0 - (y - 72.0) * s];
    let elements: Vec<crate::vector::Element> = raw
        .iter()
        .map(|e| match e {
            crate::vector::Element::Fill { contours, color, opacity } => crate::vector::Element::Fill {
                contours: contours.iter().map(|c| c.iter().map(|p| tf(p[0], p[1])).collect()).collect(),
                color: *color,
                opacity: *opacity,
            },
            crate::vector::Element::Stroke { points, color, opacity, hardness, brush } => crate::vector::Element::Stroke {
                points: points.iter().map(|p| { let q = tf(p.x, p.y); crate::vector::VPoint { x: q[0], y: q[1], r: (p.r * s).max(0.3) } }).collect(),
                color: *color,
                opacity: *opacity,
                hardness: *hardness,
                brush: brush.clone(),
            },
            other => other.clone(),
        })
        .collect();
    let png = crate::vector::VectorDrawing { elements }.render(64, 32);
    let rgba = png.to_rgba();
    image::RgbaImage::from_raw(64, 32, rgba).unwrap().save(dir.join("i/a.png")).unwrap();
    let manifest = "pelure-sboard\t1\npanneau\t0\tp1\tA\t01\t\t10\ncalque\t0\t0\tA\t1\t0\tv\ti/a.png\ti/traits.json\tok\n";
    let drawings = sbscript::parse_manifest(manifest, &dir).unwrap();
    let sb = sboard::parse(SBOARD, "x").unwrap();
    let (doc, _) = crate::sbimport::build(&sb, &dir, Some(&drawings), &|_| {});
    std::fs::remove_dir_all(&dir).ok();
    let a = doc.panels[0].layers.iter().find(|l| l.name == "A").unwrap();
    assert_eq!(a.kind, crate::document::LayerKind::Vector);
    let cel = &doc.cels[&a.frames[0].unwrap()];
    assert_eq!(cel.vector.as_ref().map(|v| v.elements.len()), Some(8), "deux zones et six traits, modifiables");
    // Le calque vectoriel sans dessin du plan 02 reste vectoriel lui aussi (vide).
}