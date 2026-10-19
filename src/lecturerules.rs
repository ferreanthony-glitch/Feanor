//! Lecture du storyboard **sans modèle** (Anthony, 2026-10-19) : un lecteur de secours, par règles,
//! qui sert quand aucun modèle n'est installé ni aucun service branché — et qui permet de produire
//! et d'essayer des propositions de sons sans rien télécharger.
//!
//! Il lit les textes du plan (action, dialogue, notes, et ceux de ses images de storyboard) :
//!
//! - **Les répliques** (champ dialogue) deviennent des voix : une ligne, une voix. « MARIE : Viens ici. »
//!   donne le personnage et la réplique ; sans nom, la ligne entière est la réplique. Les répliques d'un
//!   plan se suivent dans le temps, l'une après l'autre.
//! - **Les mots-clés** de l'action et des notes (jamais du dialogue : un personnage qui dit « il pleut »
//!   ne fait pas tomber la pluie) font des bruitages, des ambiances et une musique d'intention. La
//!   recherche ignore la casse, les accents et la ponctuation ; « sans musique », « pas de pluie »
//!   écartent le mot qui suit.
//! - **Les consignes écrites** sont reprises telles quelles : une ligne « SFX : porte qui grince, pas dans
//!   l'escalier » (ou « Son : », « Bruitage : », « Ambiance : », « Musique : ») donne une indication par
//!   élément, que les mots-clés la connaissent ou non (sinon le texte sert de description et de prompt,
//!   sans traduction : à corriger avec « Modifier »).
//!
//! Les ambiances et la musique couvrent le reste du plan (au plus trente secondes), un bruitage dure
//! ce que sa règle dit. Une règle ne produit qu'une ambiance par plan, un bruitage par image de
//! storyboard ; une seule musique par plan.

use std::collections::HashSet;

use crate::fournisseurs::{Controle, DemandeLecture, EntreePlan, ErreurGen, LecteurStoryboard};
use crate::indications::{normaliser, Genre, IndicationBrute};

/// Au plus autant d'indications par plan (un texte absurde ne fait pas une liste interminable).
const MAX_PAR_PLAN: usize = 40;
/// Ce qu'une ambiance ou une musique dure au plus, en secondes.
const MAX_AMBIANCE_S: usize = 30;
/// Durée d'un silence entre deux répliques, en secondes.
const PAUSE_REPLIQUES_S: f32 = 0.4;
/// Combien de caractères la voix dit par seconde (le rythme d'une phrase posée).
const CARACTERES_PAR_SECONDE: f32 = 14.0;

/// Le lecteur sans modèle.
pub struct LecteurRegles;

impl LecteurStoryboard for LecteurRegles {
    fn nom(&self) -> String {
        "règles (sans modèle)".into()
    }

    fn lire_plan(&self, demande: &DemandeLecture, index: usize, controle: &Controle) -> Result<Vec<IndicationBrute>, ErreurGen> {
        controle.verifier()?;
        let plan = demande.plans.get(index).ok_or_else(|| ErreurGen::Invalide("plan inconnu".into()))?;
        Ok(lire(plan, demande.fps))
    }
}

// ---------------------------------------------------------------------------------------- règles

struct Regle {
    /// Mots cherchés, déjà sans accents ni majuscules : un mot entier (ou une suite de mots), ou un
    /// début de mot s'il finit par « * ».
    mots: &'static [&'static str],
    genre: Genre,
    francais: &'static str,
    anglais: &'static str,
    /// Durée d'un bruitage, en secondes (sans objet pour une ambiance ou une musique : le plan).
    secondes: f32,
}

const fn regle(mots: &'static [&'static str], genre: Genre, francais: &'static str, anglais: &'static str, secondes: f32) -> Regle {
    Regle { mots, genre, francais, anglais, secondes }
}

use Genre::{Ambiance as A, Bruitage as B, Musique as M};

/// Les règles, dans l'ordre : les plus précises d'abord (la musique n'en garde qu'une).
const REGLES: &[Regle] = &[
    // --- bruitages
    regle(&["porte claque", "claque la porte", "claquement de porte", "claquer la porte", "porte qui claque", "claquent la porte"], B, "Porte qui claque", "heavy wooden door slamming shut", 1.5),
    regle(&["porte s ouvre", "ouvre la porte", "ouvrir la porte", "ouvrent la porte", "porte qui grince", "porte grince", "grincement*"], B, "Porte qui s'ouvre en grinçant", "old wooden door creaking open", 2.0),
    regle(&["frappe a la porte", "frappent a la porte", "coups a la porte", "toc toc", "toque a la porte", "frappe a la fenetre"], B, "Coups à la porte", "knocking on a wooden door, three knocks", 2.0),
    regle(&["bruit de pas", "bruits de pas", "pas lourds", "pas legers", "pas dans", "pas sur", "marche dans", "marche sur", "marche lentement", "marchent", "courent", "courir", "court dans", "court sur"], B, "Bruits de pas", "footsteps on the ground at a steady walking pace", 3.0),
    regle(&["explos*", "detonation*", "deflagration*"], B, "Explosion", "big explosion with a deep boom and falling debris", 3.0),
    regle(&["coup de feu", "coups de feu", "fusillade*", "tire sur", "tire avec", "tire un", "tirent"], B, "Coup de feu", "single gunshot, sharp, with a short echo", 1.5),
    regle(&["voiture qui passe", "voiture passe", "voiture demarre", "demarre en trombe", "freinage*", "pneus qui crissent", "crissement de pneus", "derape*"], B, "Voiture qui passe", "car passing by quickly on asphalt, tires screeching", 3.0),
    regle(&["klaxon*"], B, "Klaxon", "car horn honking twice", 1.5),
    regle(&["telephone sonne", "telephone qui sonne", "sonnerie*", "portable sonne", "portable qui sonne"], B, "Sonnerie de téléphone", "old telephone ringing", 3.0),
    regle(&["sonnette*", "on sonne", "sonne a la porte"], B, "Sonnette", "doorbell ringing twice", 2.0),
    regle(&["verre se brise", "verre qui se brise", "vitre se brise", "vitre qui se brise", "bris de verre", "brise la vitre", "brise le verre", "verre brise"], B, "Verre qui se brise", "glass shattering on a hard floor", 2.0),
    regle(&["coup de poing", "coups de poing", "gifle*", "coup de pied", "coups de pied"], B, "Coup", "punch impact, dull thud", 1.0),
    regle(&["cri", "cris", "crie", "crient", "hurle", "hurlent", "hurlement*"], B, "Cri", "human scream of fear, slightly distant", 2.0),
    regle(&["tonnerre", "eclair*", "foudre"], B, "Coup de tonnerre", "thunder crack followed by a distant rumble", 4.0),
    regle(&["goutte*"], B, "Gouttes d'eau", "slow water drops dripping in a cave", 4.0),
    regle(&["plouf", "plonge*", "eclaboussure*", "saute dans l eau"], B, "Plouf", "splash of a body diving into water", 2.0),
    regle(&["applaudi*"], B, "Applaudissements", "audience applause", 4.0),
    regle(&["coup d epee", "coups d epee", "epee*", "sabre*"], B, "Coup d'épée", "sword swing whoosh followed by a metallic clang", 1.5),
    regle(&["cloche", "cloches", "carillon*", "glas"], B, "Cloche", "church bell ringing slowly", 4.0),
    regle(&["alarme*", "sirene*"], B, "Alarme", "loud alarm siren", 4.0),
    regle(&["battements de coeur", "battement de coeur", "coeur qui bat", "coeur bat"], B, "Battements de cœur", "slow heartbeat thumping", 4.0),
    regle(&["respiration*", "halete*", "haletant*"], B, "Respiration", "heavy tense breathing", 3.0),
    regle(&["rire", "rires", "eclate de rire", "eclatent de rire", "ricane*"], B, "Rires", "short laughter of a person", 2.0),
    // --- ambiances
    regle(&["pluie", "pleut", "pleuvoir", "averse*", "il pleut"], A, "Pluie", "steady rain falling on the ground, ambience", 0.0),
    regle(&["vent", "bourrasque*", "tempete*", "rafale*"], A, "Vent", "wind blowing through trees, ambience", 0.0),
    regle(&["foule*", "manifestation*", "stade"], A, "Foule", "crowd murmuring in a public square, ambience", 0.0),
    regle(&["cafe", "restaurant*", "taverne*", "bistrot*"], A, "Café, restaurant", "busy restaurant ambience with distant chatter and cutlery", 0.0),
    regle(&["oiseau*", "gazouill*", "campagne", "champs"], A, "Campagne, oiseaux", "countryside ambience with birds singing and a light breeze", 0.0),
    regle(&["foret*", "bois", "grillon*", "cigale*"], A, "Forêt", "forest ambience with insects and rustling leaves", 0.0),
    regle(&["mer", "ocean*", "vague*", "plage", "rivage"], A, "Mer, vagues", "gentle ocean waves on a beach, ambience", 0.0),
    regle(&["ville", "rue", "rues", "trafic*", "circulation", "embouteillage*", "boulevard*"], A, "Ville", "city street ambience with distant traffic", 0.0),
    regle(&["usine*", "machines", "atelier*", "chantier*"], A, "Usine, atelier", "industrial workshop ambience with machinery hum", 0.0),
    regle(&["cour de recreation", "ecole", "cantine"], A, "Cour d'école", "children playing in a schoolyard, distant shouting", 0.0),
    regle(&["riviere*", "ruisseau*", "torrent*", "cascade*", "fleuve*"], A, "Cours d'eau", "babbling river stream ambience", 0.0),
    regle(&["feu de camp", "crepit*", "flammes", "incendie*", "cheminee*"], A, "Feu qui crépite", "fire crackling steadily, ambience", 0.0),
    regle(&["grotte*", "caverne*", "cave"], A, "Grotte", "echoing cave ambience with distant water drips", 0.0),
    regle(&["metro", "gare", "train*", "quai"], A, "Gare, train", "train station ambience with distant announcements and trains", 0.0),
    // --- musique d'intention (une seule par plan)
    regle(&["piano"], M, "Musique d'intention : piano", "soft solo piano underscore, intimate and slow", 0.0),
    regle(&["violon*"], M, "Musique d'intention : violon", "slow expressive solo violin melody", 0.0),
    regle(&["guitare*"], M, "Musique d'intention : guitare", "gentle acoustic guitar melody", 0.0),
    regle(&["fanfare*", "trompette*", "cuivres"], M, "Musique d'intention : fanfare", "festive brass fanfare", 0.0),
    regle(&["comptine*", "berceuse*", "boite a musique"], M, "Musique d'intention : berceuse", "gentle lullaby played on a music box", 0.0),
    regle(&["musique*", "melodie*", "chanson*", "orchestre*", "theme musical"], M, "Musique d'intention", "subtle cinematic instrumental underscore", 0.0),
];

/// Les consignes écrites reconnues : (étiquette sans accents, genre).
const ETIQUETTES: &[(&str, Genre)] = &[("sfx", B), ("son", B), ("sons", B), ("bruitage", B), ("bruitages", B), ("ambiance", A), ("ambiances", A), ("musique", M)];

/// `texte` est déjà passé par `normaliser`, entouré d'espaces. Rend la place du premier mot de la
/// règle qui s'y trouve sans être nié (« sans musique », « pas de pluie »).
fn trouve(padded: &str, regle: &Regle) -> Option<usize> {
    for mot in regle.mots {
        let motif = match mot.strip_suffix('*') {
            Some(debut) => format!(" {debut}"),
            None => format!(" {mot} "),
        };
        let mut depuis = 0;
        while let Some(p) = padded[depuis..].find(&motif) {
            let place = depuis + p;
            if !est_nie(&padded[..place]) {
                return Some(place);
            }
            // Un mot qui chevauche le suivant (« pas pas ») : on repart juste après son début.
            depuis = place + 1;
        }
    }
    None
}

/// Les mots qui précèdent disent-ils « sans … », « pas de … », « aucun … », « plus de … » ?
fn est_nie(avant: &str) -> bool {
    let mut mots = avant.split_whitespace().rev();
    match (mots.next(), mots.next()) {
        (Some("sans" | "aucun" | "aucune"), _) => true,
        (Some("de" | "d"), Some("pas" | "plus")) => true,
        _ => false,
    }
}

// ----------------------------------------------------------------------------- les indications

fn images(secondes: f32, fps: u32) -> usize {
    ((secondes * fps as f32).round() as usize).max(1)
}

fn majuscule(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Les durées d'un son qui couvre le plan à partir de `start`.
fn reste_du_plan(plan_frames: usize, start: usize, fps: u32) -> usize {
    plan_frames.saturating_sub(start).clamp(1, MAX_AMBIANCE_S * fps as usize)
}

fn depuis_regle(r: &Regle, start: usize, plan_frames: usize, fps: u32) -> IndicationBrute {
    let frames = if r.genre == B { images(r.secondes, fps) } else { reste_du_plan(plan_frames, start, fps) };
    IndicationBrute { start, frames, kind: Some(r.genre), description: r.francais.into(), prompt: r.anglais.into(), ..Default::default() }
}

/// Un texte de bruitage, d'ambiance ou de musique écrit par le réalisateur, repris tel quel.
fn depuis_consigne(genre: Genre, texte: &str, start: usize, plan_frames: usize, fps: u32) -> IndicationBrute {
    let frames = if genre == B { images(2.0, fps) } else { reste_du_plan(plan_frames, start, fps) };
    IndicationBrute { start, frames, kind: Some(genre), description: majuscule(texte), prompt: texte.into(), ..Default::default() }
}

/// « MARIE (off) : Viens. » → ("MARIE", "Viens.") ; « - Viens. » → ("", "Viens.").
pub(crate) fn separer_replique(ligne: &str) -> (String, String) {
    let ligne = ligne.trim().trim_start_matches(['-', '–', '—', '•', '*']).trim();
    let nom_plausible = |prefixe: &str| {
        let sans_didascalie = retirer_parentheses(prefixe);
        let mots: Vec<&str> = sans_didascalie.split_whitespace().collect();
        if mots.is_empty() || mots.len() > 3 || sans_didascalie.chars().any(|c| c.is_ascii_digit() || matches!(c, '.' | '!' | '?' | '«' | '»' | '"')) {
            return None;
        }
        let majuscules = sans_didascalie.chars().filter(|c| c.is_alphabetic()).all(char::is_uppercase);
        let capitalises = mots.iter().all(|m| m.chars().next().is_some_and(char::is_uppercase));
        (majuscules || capitalises).then(|| sans_didascalie.split_whitespace().collect::<Vec<_>>().join(" "))
    };
    if let Some(i) = ligne.find([':', '：']) {
        let (prefixe, reste) = ligne.split_at(i);
        let reste = reste.trim_start_matches([':', '：']).trim();
        if let (Some(nom), false) = (nom_plausible(prefixe), reste.is_empty()) {
            return (nom, nettoyer_replique(reste));
        }
    }
    (String::new(), nettoyer_replique(ligne))
}

fn retirer_parentheses(s: &str) -> String {
    let mut out = String::new();
    let mut profondeur = 0;
    for c in s.chars() {
        match c {
            '(' | '[' => profondeur += 1,
            ')' | ']' if profondeur > 0 => profondeur -= 1,
            c if profondeur == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn nettoyer_replique(s: &str) -> String {
    s.trim().trim_matches(['«', '»', '"', '“', '”']).trim().to_string()
}

/// Les indications d'un plan, dans l'ordre du temps.
pub fn lire(plan: &EntreePlan, fps: u32) -> Vec<IndicationBrute> {
    let fps = fps.max(1);
    let derniere = plan.frames.saturating_sub(1);
    // Le plan lui-même (image 0), puis chacune de ses images de storyboard.
    let mut sources: Vec<(usize, &str, &str, &str)> = vec![(0, plan.action.as_str(), plan.dialogue.as_str(), plan.notes.as_str())];
    sources.extend(plan.marques.iter().map(|m| (m.frame.min(derniere), m.action.as_str(), m.dialogue.as_str(), m.notes.as_str())));
    let mut out: Vec<IndicationBrute> = Vec::new();
    let mut ambiances: HashSet<usize> = HashSet::new();
    let mut musique = false;
    let mut vues: HashSet<String> = HashSet::new();
    for (start, action, dialogue, notes) in sources {
        let sonore = format!("{action}\n{notes}");
        // Les consignes écrites (« SFX : … ») sont lues sans leur étiquette : le mot « musique » de
        // « Musique : valse lente » n'est pas une musique de plus.
        let mut texte_utile = String::new();
        let mut consignes: Vec<(Genre, &str)> = Vec::new();
        for ligne in sonore.lines() {
            match ligne.split_once([':', '：']).and_then(|(e, c)| ETIQUETTES.iter().find(|(nom, _)| *nom == normaliser(e)).map(|&(_, g)| (g, c))) {
                Some((genre, contenu)) => {
                    consignes.push((genre, contenu));
                    texte_utile.push_str(contenu);
                }
                None => texte_utile.push_str(ligne),
            }
            texte_utile.push('\n');
        }
        let padded = format!(" {} ", normaliser(&texte_utile));
        // Celles que les règles ne connaissent pas sont reprises telles quelles (les autres sont
        // trouvées par les mots-clés, avec leur description soignée).
        for (genre, contenu) in consignes {
            for morceau in contenu.split([',', ';', '/', '+']) {
                let morceau = morceau.trim();
                if morceau.is_empty() || morceau.chars().count() > 160 || REGLES.iter().any(|r| trouve(&format!(" {} ", normaliser(morceau)), r).is_some()) {
                    continue;
                }
                if vues.insert(format!("consigne:{}", normaliser(morceau))) {
                    out.push(depuis_consigne(genre, morceau, start, plan.frames, fps));
                }
            }
        }
        // Les mots-clés.
        let mut dans_cette_image: HashSet<usize> = HashSet::new();
        for (k, r) in REGLES.iter().enumerate() {
            if trouve(&padded, r).is_none() {
                continue;
            }
            match r.genre {
                Genre::Ambiance if !ambiances.insert(k) => continue,
                Genre::Musique if std::mem::replace(&mut musique, true) => continue,
                Genre::Bruitage if !dans_cette_image.insert(k) => continue,
                _ => {}
            }
            out.push(depuis_regle(r, start, plan.frames, fps));
        }
        // Les répliques, l'une après l'autre.
        let mut au = start;
        for ligne in dialogue.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let (personnage, texte) = separer_replique(ligne);
            if !texte.chars().any(char::is_alphanumeric) {
                continue;
            }
            let texte: String = texte.chars().take(600).collect();
            let duree = images((texte.chars().count() as f32 / CARACTERES_PAR_SECONDE).max(1.0), fps);
            let debut = au.min(derniere);
            let extrait: String = texte.chars().take(70).collect::<String>() + if texte.chars().count() > 70 { "…" } else { "" };
            let description = if personnage.is_empty() { format!("Voix : « {extrait} »") } else { format!("{personnage} : « {extrait} »") };
            out.push(IndicationBrute { start: debut, frames: duree, kind: Some(Genre::Voix), description, prompt: String::new(), text: texte, character: personnage, voice: String::new() });
            au += duree + images(PAUSE_REPLIQUES_S, fps);
        }
    }
    out.truncate(MAX_PAR_PLAN);
    // Dans l'ordre du temps (stable : à l'image égale, les sons avant les voix, comme écrits).
    out.sort_by_key(|i| i.start);
    out
}
