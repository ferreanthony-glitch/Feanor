//! Le fond d'un trait de personnage (`personnages.rs`) : comment un trait dessiné avec un personnage porte
//! la couleur de fond de ce personnage, sur un calque que plusieurs personnages se partagent.
//!
//! # Le choix (documenté dans docs/INTERFACE.md, « Brosses par personnage »)
//!
//! Le fond sous le trait (`fond.rs`) est **une seule forme par dessin**, déduite de tout le trait du dessin,
//! d'**une** couleur par calque (`FondReglages::couleur`), avec par-dessus ce qu'on a peint dans la strate Fond
//! (`Strate::peinture`, restée dans la forme) et ce qu'on y a coupé (`Strate::coupe`). Changer la couleur du
//! calque quand on change de personnage recolorierait le fond de tous les personnages du calque : faux. Donner à
//! chaque trait sa propre forme multiplierait les calculs et changerait le format des dessins : non plus.
//!
//! On se sert donc de la strate Fond telle qu'elle est : **à la fin d'un trait de personnage, les pixels que la
//! forme du fond vient de gagner grâce à ce trait** (forme d'après moins forme d'avant) sont peints, dans la
//! peinture du fond, de la couleur de fond du personnage — ou, si le personnage n'a pas de fond, coupés du fond
//! (la coupe). La peinture reste dans la forme et passe sous le trait : un autre personnage qui dessine ensuite
//! sur le même calque colore son fond à lui, sans toucher à celui de l'autre. Aucun format ne change (la strate
//! Fond existe depuis le format 23) ; un projet d'avant s'ouvre tel quel ; une version qui ne connaît pas les
//! personnages montre les fonds tels qu'ils ont été peints.
//!
//! # Sans latence
//!
//! Aucun calcul sur le fil de l'interface : la forme se calcule comme celle du fond, sur un fil de fond
//! (`Fil`), avec le dessin d'avant et d'après le trait (des copies qui partagent leurs tuiles : rien n'est
//! recopié). Le fil garde la forme d'après le dernier trait : au trait suivant, un seul calcul de forme. Le
//! résultat est posé (`App::perso_poser`) à l'image d'écran où il arrive ; s'il arrive tout de suite après le
//! trait, il rejoint son étape d'annulation (un trait de personnage = une étape, fond compris) ; sinon il rejoint
//! **l'étape de son trait, où qu'elle soit** (`History::extend_step`, par le numéro propre de l'étape) : parmi les étapes à
//! annuler même si d'autres gestes ont eu lieu depuis, ou parmi celles à rétablir si le trait a été défait entre-temps (le fond
//! est alors rangé avec l'étape sans rien changer à l'écran, et revient avec elle). Seule une étape qui n'existe plus (trop
//! ancienne, regroupée) laisse le fond posé sans étape.
//!
//! # Limites
//!
//! Les pixels déjà couverts par la forme d'un autre personnage (un personnage dessiné *dans* le fond d'un autre,
//! sur le même calque) gardent la couleur d'avant : on les recolorie avec le pot de peinture dans la strate Fond
//! (qui prend la couleur de fond du personnage choisi), ou on dessine sur un autre calque.

use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;

use eframe::egui;

use crate::document::CelId;
use crate::fond::FondReglages;
use crate::raster::Raster;

/// Un trait de personnage à traiter.
pub(crate) struct Travail {
    pub cel: CelId,
    /// Le numéro de l'historique juste après l'étape du trait (`History::serial`).
    pub serial: u64,
    /// Le numéro propre de l'étape du trait (`History::last_id`) ; 0 : le trait n'a pas d'étape. Un résultat tardif rejoint cette étape.
    pub etape: u64,
    pub reglages: FondReglages,
    /// La couleur de fond du personnage ; rien : il n'a pas de fond, les pixels gagnés sont coupés.
    pub couleur: Option<[u8; 3]>,
    /// Le trait du dessin avant et après le trait de personnage.
    pub avant: Raster,
    pub apres: Raster,
    /// Un trait fait sans pastille (aucune couleur d'aplat) : la peinture de l'aplat du dessin. Si le dessin a toute sa forme peinte
    /// d'une même couleur (« La clé » du changement de couleur, `aplatmulti.rs`), ce que le trait gagne la prend aussi, au lieu de
    /// retomber sur la couleur du calque (`teinte_de_la_cle`).
    pub suivre: Option<Raster>,
}

/// La couleur dont toute la forme de l'aplat d'un dessin est peinte, s'il y en a une qui n'est pas celle du calque : la forme (`forme`,
/// une valeur par pixel du dessin) est peinte à 90 % au moins de la même couleur opaque, sur 200 pixels au moins. Une zone peinte à la
/// main (un blanc d'œil, un seau dans une zone) ne fait pas une teinte : elle ne couvre pas la forme.
pub(crate) fn teinte_de_la_cle(peinture: &Raster, forme: &[u8], calque: [u8; 3]) -> Option<[u8; 3]> {
    let w = peinture.width;
    let mut compte: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
    let mut total = 0usize;
    for (i, &f) in forme.iter().enumerate() {
        if f == 0 {
            continue;
        }
        total += 1;
        let c = peinture.get(i % w, i / w);
        if c.a() == 255 {
            *compte.entry([c.r(), c.g(), c.b()]).or_insert(0) += 1;
        }
    }
    let (couleur, n) = compte.into_iter().max_by_key(|(_, n)| *n)?;
    (total >= 200 && n * 10 >= total * 9 && couleur != calque).then_some(couleur)
}

/// Les pixels que la forme du fond a gagnés grâce au trait.
pub(crate) struct Fini {
    pub cel: CelId,
    pub serial: u64,
    pub etape: u64,
    pub couleur: Option<[u8; 3]>,
    /// Un booléen par pixel du dessin (`largeur × hauteur`).
    pub masque: Vec<bool>,
}

impl Fini {
    pub fn est_vide(&self) -> bool {
        !self.masque.iter().any(|&m| m)
    }
}

/// Ce que le fil se rappelle du dernier trait : le trait d'après et sa forme (le trait d'avant du suivant).
struct Memoire {
    reglages: FondReglages,
    trait_: Raster,
    forme: Arc<Vec<u8>>,
}

/// La forme gagnée : les pixels couverts après le trait et qui l'étaient moins avant.
pub(crate) fn pixels_gagnes(avant: &[u8], apres: &[u8]) -> Vec<bool> {
    avant.iter().zip(apres).map(|(&b, &a)| a > 0 && a > b).collect()
}

/// Le calcul d'un trait (le même sur le fil de fond et dans les essais).
fn calculer(t: &Travail, memoire: &mut Option<Memoire>) -> Fini {
    let apres = Arc::new(crate::fond::silhouette(&t.apres, &t.reglages));
    let avant = match memoire.take() {
        Some(m) if m.reglages == t.reglages && m.trait_.tiles == t.avant.tiles => m.forme,
        _ => Arc::new(crate::fond::silhouette(&t.avant, &t.reglages)),
    };
    let mut masque = pixels_gagnes(&avant, &apres);
    let mut couleur = t.couleur;
    if let Some(peinture) = &t.suivre {
        // Sans pastille : ce qui est gagné prend la teinte de la clé, s'il y en a une ; sinon rien n'est peint.
        couleur = crate::persofond::teinte_de_la_cle(peinture, &avant, t.reglages.couleur);
        if couleur.is_none() {
            masque.iter_mut().for_each(|m| *m = false);
        }
    }
    *memoire = Some(Memoire { reglages: t.reglages, trait_: t.apres.clone(), forme: apres });
    Fini { cel: t.cel, serial: t.serial, etape: t.etape, couleur, masque }
}

/// Le fil de fond des traits de personnage : les traits sont traités dans l'ordre où ils arrivent.
pub(crate) struct Fil {
    envoi: Option<Sender<Travail>>,
    recu: Receiver<Fini>,
    /// Traits envoyés et pas encore rendus.
    en_cours: usize,
}

impl Fil {
    pub fn new(ctx: egui::Context) -> Fil {
        let (envoi, travaux) = channel::<Travail>();
        let (rendu, recu) = channel::<Fini>();
        let _ = std::thread::Builder::new().name("fond-personnage".into()).spawn(move || {
            let mut memoire = None;
            // (La boucle finit quand le fil est lâché : l'envoi se ferme.)
            while let Ok(t) = travaux.recv() {
                let fini = calculer(&t, &mut memoire);
                if rendu.send(fini).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        });
        Fil { envoi: Some(envoi), recu, en_cours: 0 }
    }

    pub fn envoyer(&mut self, t: Travail) {
        if let Some(tx) = &self.envoi {
            if tx.send(t).is_ok() {
                self.en_cours += 1;
            }
        }
    }

    pub fn en_cours(&self) -> usize {
        self.en_cours
    }

    /// Les traits rendus depuis la dernière fois.
    pub fn recevoir(&mut self) -> Vec<Fini> {
        let mut out = Vec::new();
        loop {
            match self.recu.try_recv() {
                Ok(f) => out.push(f),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        self.en_cours = self.en_cours.saturating_sub(out.len());
        out
    }

    /// Attend (au plus `delai`) que tous les traits envoyés soient rendus. Pour les essais.
    pub fn attendre(&mut self, delai: std::time::Duration) -> Vec<Fini> {
        use std::sync::mpsc::RecvTimeoutError;
        use std::time::Instant;
        let fin = Instant::now() + delai;
        let mut out = self.recevoir();
        while self.en_cours > 0 {
            let reste = fin.saturating_duration_since(Instant::now());
            if reste.is_zero() {
                break;
            }
            match self.recu.recv_timeout(reste) {
                Ok(f) => {
                    self.en_cours = self.en_cours.saturating_sub(1);
                    out.push(f);
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
        out
    }
}
