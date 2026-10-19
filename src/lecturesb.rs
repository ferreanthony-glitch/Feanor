//! Lecture du storyboard, côté SHOT (Anthony, 2026-10-19) : ce qu'on donne à un lecteur
//! (`LecteurStoryboard`) et ce qu'on en reçoit, sans rien de propre à un fournisseur.
//!
//! - **L'entrée** : pour chaque plan, une petite vignette (PNG), les trois champs de la fiche du plan
//!   (action, dialogue, notes) et ceux de ses images de storyboard, sa durée, ses transitions et ses
//!   commentaires non résolus (`entree_plan`, `vignette_png`). Les vignettes se font sur le fil de
//!   l'interface, quelques millisecondes par image d'écran (`suggestionsui.rs`) ; le reste est du texte.
//! - **La demande à un modèle** : les consignes (`CONSIGNES`) et le plan en JSON (`plan_en_json`) sont les
//!   mêmes pour le modèle local et pour Claude ; seul le transport change.
//! - **La réponse** : un JSON `{"indications": [...]}`, relu avec méfiance (`indications_depuis_texte`) :
//!   une sorte inconnue est écartée, les durées sont bornées au plan, les textes aussi.
//! - **Sans modèle** : `lecturerules.rs` lit les mêmes textes par des règles.

use crate::document::Document;
use crate::fournisseurs::{DemandeLecture, EntreePlan, ErreurGen, MarqueTexte};
use crate::indications::{Genre, IndicationBrute};

/// Largeur de la vignette envoyée à un lecteur : assez pour reconnaître un lieu et une action, peu
/// pour rester légère (un PNG de quelques dizaines de ko).
pub(crate) const LARGEUR_VIGNETTE: usize = 384;
/// Ce que la lecture parcourt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Portee {
    /// Tout le film.
    Film,
    /// La séquence (son identifiant).
    Sequence(u64),
    /// Ces plans-là (leur identifiant), dans l'ordre du film.
    Plans(Vec<u64>),
}

/// Les rangs (dans `doc.panels`) des plans de la portée, dans l'ordre du film.
pub fn plans_de_la_portee(doc: &Document, portee: &Portee) -> Vec<usize> {
    (0..doc.panels.len())
        .filter(|&i| match portee {
            Portee::Film => true,
            Portee::Sequence(s) => doc.sequence_of(i) == *s,
            Portee::Plans(ids) => ids.contains(&doc.panels[i].id),
        })
        .collect()
}

fn transition_texte(doc: &Document, out: Option<u64>, inn: Option<u64>) -> Option<String> {
    doc.transition_on(out, inn).map(|t| format!("{}, {} images", t.kind.name(), t.frames))
}

/// Ce que la lecture sait du plan `index`, sans sa vignette.
pub fn entree_plan(doc: &Document, index: usize) -> EntreePlan {
    let p = &doc.panels[index];
    EntreePlan {
        plan: p.id,
        titre: crate::storyboard::panel_title(doc, index),
        sequence: doc.sequence_label(doc.sequence_of(index)),
        debut: doc.panel_start(index),
        frames: p.frame_count(),
        action: p.action.trim().to_string(),
        dialogue: p.dialogue.trim().to_string(),
        notes: p.notes.trim().to_string(),
        marques: p
            .marks
            .iter()
            .map(|m| MarqueTexte { frame: m.frame, action: m.action.trim().to_string(), dialogue: m.dialogue.trim().to_string(), notes: m.notes.trim().to_string() })
            .filter(|m| !(m.action.is_empty() && m.dialogue.is_empty() && m.notes.is_empty()))
            .collect(),
        // (Au bord du film, la transition part du noir ou y va : le plan voisin est « rien ».)
        transition_entrante: transition_texte(doc, index.checked_sub(1).map(|k| doc.panels[k].id), Some(p.id)),
        transition_sortante: transition_texte(doc, Some(p.id), doc.panels.get(index + 1).map(|n| n.id)),
        commentaires: doc.comments.iter().filter(|c| c.panel == p.id && !c.done).map(|c| c.text.trim().to_string()).filter(|t| !t.is_empty()).collect(),
        vignette_png: None,
    }
}

/// La vignette du plan `index` (première image), en PNG de `largeur` points. Les effets pas encore
/// calculés ne sont pas attendus (`fx::differe`) : la vignette est celle que SHOT montrerait.
pub fn vignette_png(doc: &Document, index: usize, largeur: usize) -> Option<Vec<u8>> {
    let plan = crate::render::panel_plan_at(doc, index, 0, Some(doc.paper));
    let cam = doc.panel_camera(index, 0);
    let (tw, th) = crate::thumbs::thumb_size_at(doc, largeur, 100);
    let (image, _complete) = crate::fx::differe(|| crate::fxcache::soft_scope(|| crate::render::thumbnail(doc, &plan, &cam, tw, th)));
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
    let img = image::RgbaImage::from_raw(image.size[0] as u32, image.size[1] as u32, rgba)?;
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).ok()?;
    Some(out)
}

// -------------------------------------------------------------------------- demande à un modèle

/// Les consignes d'un modèle qui lit un plan. Identiques pour le modèle local et pour Claude.
pub const CONSIGNES: &str = "\
You are a sound designer reading the storyboard of an animated film, one shot at a time. \
For the shot you are given, propose the TEMPORARY sounds an animatic needs: sound effects (genre \"bruitage\"), \
ambiences (\"ambiance\"), an intention music (\"musique\") only if the texts ask for one, and voices (\"voix\") \
for the dialogue lines. Use the texts first and the thumbnail (if any) to understand the place and the action. \
Propose few, useful sounds; never invent dialogue; no sound if the shot calls for none.\n\
Times are in frames of the shot, starting at 0 (\"image\" = first frame, \"images\" = duration, the shot lasts \
the number of frames given). \"description\" is a short phrase in FRENCH for the director. \"prompt\" is a short \
ENGLISH prompt for a sound generator (empty for a voice). For a voice, \"texte\" is the exact line spoken and \
\"personnage\" the speaker when known.\n\
Answer with JSON only, no comment: \
{\"indications\":[{\"image\":0,\"images\":24,\"genre\":\"bruitage|ambiance|musique|voix\",\"description\":\"...\",\"prompt\":\"...\",\"texte\":\"\",\"personnage\":\"\"}]}";

/// Le plan `index` de la demande en JSON (sans la vignette, que chaque transport joint à sa manière).
pub fn plan_en_json(demande: &DemandeLecture, index: usize) -> serde_json::Value {
    let p = &demande.plans[index];
    let voisin = |k: Option<&EntreePlan>| k.map(|q| serde_json::json!({"titre": q.titre, "action": q.action}));
    serde_json::json!({
        "id": p.plan,
        "titre": p.titre,
        "sequence": p.sequence,
        "cadence": demande.fps,
        "images": p.frames,
        "action": p.action,
        "dialogue": p.dialogue,
        "notes": p.notes,
        "marques": p.marques.iter().map(|m| serde_json::json!({"image": m.frame, "action": m.action, "dialogue": m.dialogue, "notes": m.notes})).collect::<Vec<_>>(),
        "transition_entrante": p.transition_entrante,
        "transition_sortante": p.transition_sortante,
        "commentaires": p.commentaires,
        "plan_precedent": voisin(index.checked_sub(1).and_then(|k| demande.plans.get(k))),
        "plan_suivant": voisin(demande.plans.get(index + 1)),
    })
}

// ------------------------------------------------------------------------------------ réponse

fn texte_de(v: &serde_json::Value, clef: &str, max: usize) -> String {
    let s = v.get(clef).and_then(|x| x.as_str()).unwrap_or("").trim();
    s.chars().take(max).collect()
}

fn entier_de(v: &serde_json::Value, clef: &str) -> Option<usize> {
    let x = v.get(clef)?;
    x.as_u64().map(|n| n as usize).or_else(|| x.as_f64().filter(|f| f.is_finite() && *f >= 0.0).map(|f| f.round() as usize))
}

/// Le premier objet JSON d'un texte de modèle (qui peut l'entourer de phrases ou de ```json).
pub fn extraire_json(texte: &str) -> Option<&str> {
    let debut = texte.find(['{', '['])?;
    let fin = texte.rfind(['}', ']'])?;
    (fin > debut).then(|| &texte[debut..=fin])
}

/// Les indications d'une réponse de modèle pour un plan de `frames` images. Une réponse qui n'est pas
/// du JSON est une erreur ; une indication bancale est écartée, les autres restent.
pub fn indications_depuis_texte(texte: &str, frames: usize) -> Result<Vec<IndicationBrute>, ErreurGen> {
    let json = extraire_json(texte).ok_or_else(|| ErreurGen::Invalide("la réponse du modèle n'est pas du JSON".into()))?;
    let valeur: serde_json::Value = serde_json::from_str(json).map_err(|e| ErreurGen::Invalide(format!("la réponse du modèle est illisible ({e})")))?;
    let liste = valeur.get("indications").unwrap_or(&valeur).as_array().ok_or_else(|| ErreurGen::Invalide("la réponse du modèle n'a pas de liste d'indications".into()))?;
    Ok(indications_depuis_liste(liste, frames))
}

/// Les indications d'une liste JSON déjà lue (la réponse du serveur local).
pub fn indications_depuis_liste(liste: &[serde_json::Value], frames: usize) -> Vec<IndicationBrute> {
    let frames = frames.max(1);
    let mut out = Vec::new();
    for v in liste.iter().take(200) {
        let kind = match v.get("genre").and_then(|g| g.as_str()).map(crate::indications::normaliser).as_deref() {
            Some("bruitage" | "sfx") => Genre::Bruitage,
            Some("ambiance") => Genre::Ambiance,
            Some("musique") => Genre::Musique,
            Some("voix") => Genre::Voix,
            _ => continue,
        };
        let description = texte_de(v, "description", 400);
        let prompt = texte_de(v, "prompt", 600);
        let text = texte_de(v, "texte", 1_000);
        if description.is_empty() && prompt.is_empty() && text.is_empty() {
            continue;
        }
        if kind.est_voix() && text.is_empty() {
            continue;
        }
        let start = entier_de(v, "image").unwrap_or(0).min(frames - 1);
        let duree = entier_de(v, "images").unwrap_or(frames - start).max(1);
        out.push(IndicationBrute { start, frames: duree, kind: Some(kind), description, prompt, text, character: texte_de(v, "personnage", 100), voice: texte_de(v, "voix", 100) });
    }
    out
}
