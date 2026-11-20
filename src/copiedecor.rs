//! **Copier les objets du décor** (partie 190, `docs/INTERFACE.md`) : les objets du décor 3D d'une séquence (`decor3d.rs`,
//! parties 180 et 181) se copient, se coupent et se collent, d'une séquence à l'autre et d'un projet ouvert à l'autre ; le
//! panneau Espace 3D les copie ou les envoie vers une autre séquence du projet ; et une séquence qui naît des plans d'une autre
//! (« Couper la séquence ici », « Nouvelle séquence avec les plans choisis ») emporte une copie de son décor.
//!
//! - **Le presse-papiers** (`Copie3d::presse`) est un champ d'`App` hors de l'échange des onglets, comme les autres
//!   presse-papiers (`onglets.rs`) : ce qu'on copie dans un projet se colle dans un autre. Il garde des copies des objets
//!   (nom, fichier d'origine, place, origine, aimanté, ce qui a été allégé) et **leur modèle partagé** (`Arc<Modele3d>`) :
//!   coller ne relit rien et n'envoie rien de plus à la carte graphique (le modèle garde son `uid`). Dans un projet qui a déjà
//!   un modèle de même empreinte, c'est le sien qui est repris (un seul `Arc` par contenu dans un projet) ; sinon le modèle
//!   copié entre dans le projet, avec ses octets : l'enregistrement l'écrit sous son empreinte (`decor3d::plan_save`).
//! - **Ctrl+C, Ctrl+X, Ctrl+V** sont ceux de toujours (`edition.copier`, `edition.couper`, `edition.coller`) : ils visent les
//!   objets du décor quand **le décor a la main** (`Copie3d::main`) — le dernier appui était dans la vue Espace 3D (hors des
//!   outils de dessin, de trajectoire et de sélection pris dans la vue) ou dans le décor du panneau Espace 3D — et, pour copier
//!   et couper, qu'un objet y est choisi ; pour coller, que la dernière copie est celle d'objets 3D (`Copie3d::recente`).
//!   Sinon ces touches gardent leur sens d'avant. Comme pour le panneau Calques et le panneau Effets (`fxcopie.rs`), un appui
//!   ailleurs rend la main (`App::update`).
//! - **Où se pose ce qu'on colle** : dans le décor de la séquence du plan courant. Dans le même projet, tant que les objets
//!   copiés y sont encore (une copie, pas une coupe), la copie se pose **à côté**, décalée en largeur de la largeur du groupe
//!   (comme « Dupliquer ») ; collée encore, plus loin. Ailleurs (une autre séquence, un autre projet, après Ctrl+X) : **à la
//!   même place** dans le monde. L'aimant repose ensuite les objets aimantés et les cartes aimantées de la séquence
//!   (`reposer_les_cartes_aimantees`). Une étape d'annulation ; les objets collés sont choisis (vue et liste).
//! - **Le budget de cette machine** vaut pour la somme du décor (partie 181) : un collage qui le ferait dépasser est refusé,
//!   avec un message.

use std::collections::BTreeSet;

use crate::app::App;
use crate::decor3d::{Decor3d, Objet3d};
use crate::decor3drendu::Aspect;
use crate::document::Document;
use crate::espace3d::Monde;
use crate::espace3dmath::V3;

/// Des objets du décor copiés.
#[derive(Clone)]
pub(crate) struct PresseDecor {
    /// Des copies des objets (leur numéro d'origine gardé : le collage le remplace), le modèle partagé.
    pub objets: Vec<Objet3d>,
    /// L'aspect du décor d'où ils viennent : celui d'un décor que le collage crée (une séquence qui n'en avait pas).
    pub aspect: Aspect,
    /// Le projet d'où ils viennent (`App::doc_epoch`) et sa séquence.
    pub epoque: u64,
    pub seq: u64,
    /// Combien de fois ils ont été collés à côté d'eux-mêmes (le collage suivant se pose plus loin).
    pub a_cote: u32,
}

/// Le presse-papiers des objets du décor et qui a la main (un champ d'`App`, commun aux onglets).
#[derive(Default)]
pub(crate) struct Copie3d {
    pub(crate) presse: Option<PresseDecor>,
    /// Le dernier appui était dans la vue Espace 3D ou dans le décor du panneau Espace 3D : Ctrl+C, Ctrl+X et Ctrl+V visent les
    /// objets du décor (s'il y a de quoi).
    pub(crate) main: bool,
    /// La dernière copie (Ctrl+C, Ctrl+X) est celle d'objets du décor : Ctrl+V les colle.
    pub(crate) recente: bool,
}

/// Le nom d'un groupe d'objets, pour la barre d'état : « Objet 3D « Banc » », « 3 objets 3D ».
fn quoi(objets: &[&Objet3d]) -> String {
    match objets {
        [o] => format!("Objet 3D « {} »", o.nom),
        _ => format!("{} objets 3D", objets.len()),
    }
}

/// La boîte de tous ces objets dans le monde.
fn boite_du_groupe(objets: &[Objet3d]) -> Option<(V3, V3)> {
    let mut it = objets.iter().map(|o| o.bornes_monde());
    let first = it.next()?;
    Some(it.fold(first, |(lo, hi), (a, b)| ([0, 1, 2].map(|k| lo[k].min(a[k])), [0, 1, 2].map(|k| hi[k].max(b[k])))))
}

/// Le pas du collage à côté : la largeur du groupe et un dixième, au moins 30 cm (celui de « Dupliquer » pour un objet seul).
pub(crate) fn pas_a_cote(objets: &[Objet3d]) -> f32 {
    boite_du_groupe(objets).map_or(0.3, |(lo, hi)| ((hi[0] - lo[0]) * 1.1).max(0.3))
}

impl Document {
    /// Pose des copies de `objets` dans le décor de la séquence `seq` (créé avec l'aspect `aspect` s'il n'existe pas) : des
    /// numéros neufs, décalées de `dx` mètres en largeur, le modèle de même empreinte déjà dans le projet repris (sinon celui de
    /// la copie, partagé). Rien d'autre (ni l'aimant ni l'étape d'annulation : à l'appelant). Renvoie leurs numéros.
    pub fn poser_copies_3d(&mut self, seq: u64, objets: &[Objet3d], aspect: Aspect, dx: f32) -> Vec<u64> {
        let mut neufs = Vec::with_capacity(objets.len());
        let mut copies = Vec::with_capacity(objets.len());
        for o in objets {
            let mut c = o.clone();
            c.id = self.alloc();
            if let Some(m) = self.decors3d.modele_d_empreinte(&o.modele.empreinte) {
                c.modele = m;
            }
            c.place.position[0] += dx;
            // (Partie 201 : « Caché dans le plan » ne vise que des plans de la séquence où il est posé ; vers une autre séquence, il part.)
            c.caches_dans.retain(|p| self.panels.iter().enumerate().any(|(i, q)| q.id == *p && self.sequence_of(i) == seq));
            neufs.push(c.id);
            copies.push(c);
        }
        self.decors3d.par_sequence.entry(seq).or_insert_with(|| Decor3d::new(aspect)).objets.extend(copies);
        neufs
    }

    /// Une copie du décor de la séquence `de` pour la séquence `vers` (une séquence neuve, faite des plans de `de`) : un décor
    /// neuf (`Decor3d::uid`, sa propre texture), le même aspect, les mêmes modèles (partagés), des objets aux numéros neufs, aux
    /// mêmes places. Les deux décors sont ensuite indépendants. Rien si `de` n'a pas de décor ou si `vers` en a déjà un.
    pub fn copier_decor_de_sequence(&mut self, de: u64, vers: u64) -> bool {
        if de == vers || self.decors3d.par_sequence.get(&vers).is_some_and(|d| !d.objets.is_empty()) {
            return false;
        }
        let Some(source) = self.decors3d.par_sequence.get(&de).filter(|d| !d.objets.is_empty()).cloned() else { return false };
        let mut d = Decor3d::new(source.aspect);
        for o in source.objets {
            let mut c = o;
            c.id = self.alloc();
            // (Partie 200 : « Caché dans le plan » vise les plans de l'autre séquence : rien ici ; la duplication de la séquence le
            // reporte sur les copies de ses plans, `dupliquer_sequence`.)
            c.caches_dans.clear();
            d.objets.push(c);
        }
        // (Partie 200 : ses lampes aussi, à des numéros neufs ; les clés des objets sont copiées avec eux.)
        for l in source.lampes {
            let mut c = l;
            c.id = self.alloc();
            d.lampes.push(c);
        }
        self.decors3d.par_sequence.insert(vers, d);
        true
    }
}

impl App {
    /// Les objets du décor que visent Copier, Couper, « Copier vers » et « Envoyer vers » : ceux choisis dans la vue Espace 3D,
    /// sinon celui choisi dans la liste du panneau.
    // (Fusion du 19 novembre : déjà sans appelant sur le `main` de GitHub b27db77 ; gardée pour la session du décor 3D.)
    #[allow(dead_code)]
    pub(crate) fn decor3d_objets_vises(&self) -> Vec<u64> {
        let vue = self.e3_objets_choisis();
        if !vue.is_empty() {
            return vue;
        }
        self.decor3d.choisi.filter(|&id| self.e3_objet(id).is_some()).into_iter().collect()
    }

    /// Ctrl+C et Ctrl+X visent les objets du décor : il a la main, et un objet est choisi dans la vue (la liste du panneau le
    /// choisit aussi dans la vue).
    pub(crate) fn decor3d_a_la_main(&self) -> bool {
        self.copie3d.main && !self.e3_objets_choisis().is_empty()
    }

    /// Ctrl+V colle des objets du décor : il a la main, et la dernière copie est la leur.
    pub(crate) fn decor3d_coller_a_la_main(&self) -> bool {
        self.copie3d.main && self.copie3d.recente && self.copie3d.presse.is_some()
    }

    /// Un appui dans la vue Espace 3D ou dans le décor du panneau : le décor a la main (Ctrl+C, Ctrl+X, Ctrl+V).
    pub(crate) fn decor3d_prendre_la_main(&mut self) {
        self.copie3d.main = true;
    }

    /// Met en presse les objets `ids` du décor de la séquence du plan courant : leur nom pour la barre d'état et leur nombre ;
    /// rien s'il n'y a rien à copier (la barre d'état le dit).
    fn decor3d_mettre_en_presse(&mut self, ids: &[u64]) -> Option<(String, usize)> {
        let seq = self.doc.sequence_of(self.cur_panel);
        let Some(d) = self.doc.decor_du_plan(self.cur_panel) else {
            self.status = "Cette séquence n'a pas de décor 3D : rien à copier.".into();
            return None;
        };
        let objets: Vec<&Objet3d> = d.objets.iter().filter(|o| ids.contains(&o.id)).collect();
        if objets.is_empty() {
            self.status = "Choisis un objet du décor 3D (dans la vue Espace 3D ou dans la liste du panneau) pour le copier.".into();
            return None;
        }
        let texte = quoi(&objets);
        let copies: Vec<Objet3d> = objets.into_iter().cloned().collect();
        let n = copies.len();
        self.copie3d.presse = Some(PresseDecor { objets: copies, aspect: d.aspect, epoque: self.doc_epoch, seq, a_cote: 0 });
        self.copie3d.recente = true;
        // La dernière copie est celle-ci : les clés copiées (caméra, effets) ne se collent plus au clavier dans la timeline.
        self.fx_clip_recent = false;
        self.cam_clip_recent = false;
        // Dans le presse-papiers du système : Ctrl+V vise de nouveau FEANOR (`pasteimage`), pas une image copiée avant.
        self.ctx.copy_text(format!("{}objets 3D copiés", crate::pasteimage::MARK));
        Some((texte, n))
    }

    /// Copie les objets `ids` du décor de la séquence du plan courant. Faux : rien à copier (la barre d'état le dit).
    pub(crate) fn decor3d_copier(&mut self, ids: &[u64]) -> bool {
        let Some((texte, n)) = self.decor3d_mettre_en_presse(ids) else { return false };
        let (s, le) = if n > 1 { ("s", "les") } else { ("", "le") };
        self.status = format!("{texte} copié{s} : Ctrl+V {le} colle dans le décor de la séquence du plan courant (ici, dans une autre séquence ou dans un autre projet ouvert).");
        true
    }

    /// Coupe les objets `ids` : copiés, puis enlevés du décor (une étape d'annulation) ; ceux qui reposaient dessus redescendent.
    pub(crate) fn decor3d_couper(&mut self, ids: &[u64]) {
        let Some((texte, n)) = self.decor3d_mettre_en_presse(ids) else { return };
        let seq = self.doc.sequence_of(self.cur_panel);
        if self.decor3d_transferer("Couper des objets 3D", Some((seq, ids)), None).is_some() {
            let (s, le) = if n > 1 { ("s", "les") } else { ("", "le") };
            let notes = std::mem::take(&mut self.status);
            self.status = format!("{texte} coupé{s} : Ctrl+V {le} colle (ici ou ailleurs) ; Ctrl+Z {le} rend.{}", if notes.is_empty() { String::new() } else { format!(" {notes}") });
        }
    }

    /// Colle les objets copiés dans le décor de la séquence du plan courant (une étape d'annulation) : à côté d'eux s'ils y sont
    /// encore (même projet, même séquence), sinon à la même place ; puis ils sont choisis.
    pub(crate) fn decor3d_coller(&mut self) {
        let Some(p) = self.copie3d.presse.clone() else {
            self.status = "Aucun objet 3D copié.".into();
            return;
        };
        let seq = self.doc.sequence_of(self.cur_panel);
        let ici = self.doc.decors3d.par_sequence.get(&seq);
        let encore_la = p.epoque == self.doc_epoch && p.seq == seq && p.objets.iter().any(|o| ici.is_some_and(|d| d.objet(o.id).is_some_and(|x| x.place == o.place)));
        let dx = if encore_la { pas_a_cote(&p.objets) * (p.a_cote + 1) as f32 } else { 0.0 };
        let Some(neufs) = self.decor3d_transferer("Coller des objets 3D", None, Some((seq, &p.objets, p.aspect, dx))) else { return };
        if encore_la {
            if let Some(q) = self.copie3d.presse.as_mut() {
                q.a_cote += 1;
            }
        }
        self.decor3d_choisir_ceux_ci(&neufs);
        let refs: Vec<&Objet3d> = p.objets.iter().collect();
        let ou = if encore_la { "à côté".to_string() } else { "à la même place".to_string() };
        let notes = std::mem::take(&mut self.status);
        self.status = format!("{} collé{} dans le décor de la séquence {}, {ou}.{}", quoi(&refs), if refs.len() > 1 { "s" } else { "" }, self.doc.sequence_label(seq), if notes.is_empty() { String::new() } else { format!(" {notes}") });
    }

    /// « Copier vers la séquence » (`deplacer` faux) ou « Envoyer vers la séquence » (vrai) : les objets `ids` du décor de la
    /// séquence du plan courant vont, à la même place, dans le décor de la séquence `vers` (une étape d'annulation). Envoyés, ils
    /// quittent celui-ci.
    pub(crate) fn decor3d_vers_sequence(&mut self, ids: &[u64], vers: u64, deplacer: bool) {
        let seq = self.doc.sequence_of(self.cur_panel);
        if vers == seq {
            self.status = "C'est déjà la séquence de ce décor.".into();
            return;
        }
        if !self.doc.sequences.iter().any(|s| s.id == vers) {
            return;
        }
        let Some(d) = self.doc.decor_du_plan(self.cur_panel) else { return };
        let objets: Vec<Objet3d> = d.objets.iter().filter(|o| ids.contains(&o.id)).cloned().collect();
        if objets.is_empty() {
            self.status = "Choisis un objet du décor 3D à envoyer.".into();
            return;
        }
        let aspect = d.aspect;
        let nom = if deplacer { "Envoyer des objets 3D vers une séquence" } else { "Copier des objets 3D vers une séquence" };
        let retirer = deplacer.then_some((seq, ids));
        if self.decor3d_transferer(nom, retirer, Some((vers, &objets, aspect, 0.0))).is_none() {
            return;
        }
        if deplacer {
            self.decor3d.choisi = None;
        }
        let refs: Vec<&Objet3d> = objets.iter().collect();
        let notes = std::mem::take(&mut self.status);
        self.status = format!(
            "{} {} vers la séquence {} (à la même place ; Ctrl+Z {}).{}",
            quoi(&refs),
            match (deplacer, refs.len() > 1) {
                (true, true) => "envoyés",
                (true, false) => "envoyé",
                (false, true) => "copiés",
                (false, false) => "copié",
            },
            self.doc.sequence_label(vers),
            if deplacer { "les ramène ici" } else { "les y retire" },
            if notes.is_empty() { String::new() } else { format!(" {notes}") }
        );
    }

    /// Le cœur de couper, coller, copier et envoyer vers une séquence, **en une étape d'annulation** nommée `nom` : `retirer`
    /// enlève des objets du décor d'une séquence, `poser` pose des copies dans celui d'une séquence (décalées de `dx` mètres en
    /// largeur) ; l'aimant repose ensuite les objets des décors touchés, et les cartes aimantées de leurs séquences. Rien (et rien
    /// n'a changé) si le contenu est verrouillé ou si le collage ferait dépasser le budget de cette machine (la barre d'état le
    /// dit) ; sinon les numéros des objets posés (et dans la barre d'état, ce que l'aimant a dit).
    pub(crate) fn decor3d_transferer(&mut self, nom: &str, retirer: Option<(u64, &[u64])>, poser: Option<(u64, &[Objet3d], Aspect, f32)>) -> Option<Vec<u64>> {
        if self.content_refuses() {
            return None;
        }
        self.decor3d_retouche_finir();
        // Le budget de cette machine vaut pour tout le décor de la séquence (partie 181) : vérifié avant de rien changer.
        if let (Some((vers, objets, _, _)), Some(b)) = (poser, self.decor3d_budget().cloned()) {
            let mut essai = self.doc.decors3d.par_sequence.get(&vers).cloned().unwrap_or_else(|| Decor3d::new(Default::default()));
            if let Some((_, ids)) = retirer.filter(|(de, _)| *de == vers) {
                essai.objets.retain(|o| !ids.contains(&o.id));
            }
            let avant = essai.poids();
            essai.objets.extend(objets.iter().cloned());
            let p = essai.poids();
            let trop_triangles = p.triangles > b.triangles && p.triangles > avant.triangles;
            let trop_textures = p.octets_textures > b.textures_mo << 20 && p.octets_textures > avant.octets_textures;
            if trop_triangles || trop_textures {
                let g = crate::decor3dbudget::grand_nombre;
                self.status = if trop_triangles {
                    format!("Refusé : le décor de la séquence aurait {} triangles, plus que ce que cet ordinateur dessine sans ralentir ({}). Enlève ou simplifie un objet avant.", g(p.triangles), g(b.triangles))
                } else {
                    "Refusé : le décor de la séquence aurait plus de textures que ce que cet ordinateur garde. Enlève ou allège un objet avant.".into()
                };
                return None;
            }
        }
        let sol = Monde::of(&self.doc).sol_y();
        let snap = self.snapshot();
        let mut touchees: BTreeSet<u64> = BTreeSet::new();
        let mut notes: Vec<String> = Vec::new();
        if let Some((de, ids)) = retirer {
            if let Some(mut d) = self.doc.decors3d.par_sequence.get(&de).cloned() {
                d.objets.retain(|o| !ids.contains(&o.id));
                if d.objets.is_empty() {
                    self.doc.decors3d.par_sequence.remove(&de);
                } else {
                    notes.extend(crate::decor3daimant::aimanter(&mut d, sol));
                    self.doc.decors3d.par_sequence.insert(de, d);
                }
                touchees.insert(de);
            }
            // (Ni choisis dans la vue, ni dans la liste : ils n'y sont plus.)
            self.espace3d.sel.retain(|&s| crate::poignees3d::objet_of(s).is_none_or(|o| !ids.contains(&o)));
            if self.decor3d.choisi.is_some_and(|c| ids.contains(&c)) {
                self.decor3d.choisi = None;
            }
        }
        let mut neufs = Vec::new();
        if let Some((vers, objets, aspect, dx)) = poser {
            neufs = self.doc.poser_copies_3d(vers, objets, aspect, dx);
            if let Some(mut d) = self.doc.decors3d.par_sequence.get(&vers).cloned() {
                notes.extend(crate::decor3daimant::aimanter(&mut d, sol));
                self.doc.decors3d.par_sequence.insert(vers, d);
            }
            touchees.insert(vers);
        }
        for s in touchees {
            self.reposer_les_cartes_aimantees(s);
        }
        let before = self.history.context.replace(nom.into());
        self.commit_structure(snap);
        self.history.context = before;
        notes.dedup();
        self.status = notes.join(" ");
        Some(neufs)
    }

    /// Les objets `ids` (du décor de la séquence du plan courant) deviennent ce qui est choisi : dans la vue Espace 3D et, le
    /// dernier, dans la liste du panneau.
    pub(crate) fn decor3d_choisir_ceux_ci(&mut self, ids: &[u64]) {
        if ids.is_empty() {
            return;
        }
        self.espace3d.last_layer = self.layer().id;
        self.espace3d.sel = ids.iter().map(|&id| crate::poignees3d::objet_selection(id)).collect();
        self.decor3d.choisi = ids.last().copied();
    }
}
