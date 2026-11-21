//! **Dupliquer la séquence** (partie 191, `docs/INTERFACE.md`) : une séquence neuve, juste après l'originale, faite d'une copie
//! indépendante de tout ce que la séquence possède. Ensuite, rien ne relie les deux : retoucher un dessin, un calque, une clé de
//! caméra ou un objet du décor de l'une laisse l'autre telle qu'elle était.
//!
//! - **Les plans** : numéros neufs de plans, de scènes, de marques d'image et de calques ; les dessins sont dupliqués (les tuiles
//!   restent partagées jusqu'à la première retouche, comme pour « Dupliquer le plan ») et leurs traits prennent des numéros neufs
//!   (partie 125 : une copie n'est pas une part du trait d'origine ; les parts d'un même trait dans la copie en gardent un commun).
//!   Durées, caméra, effets, textes, couleur de repère, calques de toute la scène : tels quels.
//! - **Ce qui tient aux plans** : leurs versions (V1, V2…, partie 168, avec leurs couleurs de repère), leurs commentaires, leur
//!   contenu rogné, les transitions entre deux plans de la séquence, les liens des références (une référence liée à un plan l'est
//!   aussi à sa copie), les notes et les annotations de la planche attachées à leurs vignettes.
//! - **La coupe de sortie** : la transition qui menait de la fin de l'originale au plan suivant (ou à la fin du film) est copiée à la
//!   fin de la copie, qui est maintenant devant ce plan ; celle de l'originale attend, sans jouer, que ses deux plans se suivent de
//!   nouveau (`transitions::zones`). La coupe entre l'originale et sa copie est franche.
//! - **Les autres versions de la séquence** (A, B…, `variantes.rs`) : copiées aussi, avec leur garage (des copies des plans qu'elles
//!   montrent hors du film). Un emplacement qui montre un plan d'une autre séquence (déplacé depuis) le montre toujours.
//! - **Le décor 3D** : une copie indépendante (`Document::copier_decor_de_sequence`, partie 190) ; les modèles restent partagés.
//! - **La planche-mère** (partie 126, `partage.rs`) : deux calques liés dans la séquence le restent entre leurs copies, sous une clé neuve ;
//!   rien ne lie une copie à l'original ni à un plan d'une autre séquence (comme « Dupliquer le plan »).
//! - **Le nom** : « Poursuite (copie) », puis « Poursuite (copie 2) »… ; une séquence sans nom donne une copie sans nom (son
//!   numéro automatique la distingue). En numérotation verrouillée, la séquence et ses plans reçoivent des numéros neufs entre
//!   leurs voisins (`numerotation.rs`) : un numéro est unique dans le projet.
//! - Ce qui ne se copie pas : le son (les pistes sont en temps du montage, elles ne bougent avec aucun plan), les plans écartés
//!   (`planannule.rs`, ils restent à l'originale), la corbeille, les envois.
//! - Une étape d'annulation (`App::structure_op_dessins`) ; la copie devient la séquence courante (son premier plan) ; refusé sous
//!   le verrou du storyboard, ou quand tout le film est écarté.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::annotations::{Ancre, Trace};
use crate::app::App;
use crate::board::{NoteStroke, NoteText};
use crate::commentaires::Comment;
use crate::document::{fresh_layer_id, relink_copies, CelId, Document, Panel, Scene};
use crate::planvers::{Rangee, VersionsDuPlan};
use crate::rognage::Reserve;
use crate::variantes::{Contenu, Emplacement, Parque, VarianteRangee, VariantesDeSequence};

/// Ce que la copie renumérote, de l'original vers la copie.
#[derive(Default)]
struct Correspondances {
    scenes: HashMap<u64, u64>,
    /// Les plans (du film, et du garage des versions de séquence).
    plans: HashMap<u64, u64>,
    /// Les clés de partage (planche-mère) : une clé neuve par clé de l'original.
    partages: HashMap<u64, u64>,
    /// Les dessins : un même dessin n'est copié qu'une fois, où qu'il serve dans la séquence.
    dessins: HashMap<CelId, CelId>,
}

/// Le nom de la copie de la séquence nommée `nom` : « Nom (copie) », ou « Nom (copie 2) »… si ce nom est pris parmi `pris`. Vide
/// pour une séquence sans nom (son numéro automatique la distingue). Jamais plus long que le nom d'une séquence
/// (`planchesequencesui::NOM_MAX`).
pub(crate) fn nom_de_copie<'a>(pris: impl Iterator<Item = &'a str> + Clone, nom: &str) -> String {
    let nom = nom.trim();
    if nom.is_empty() {
        return String::new();
    }
    let base = nom.split(" (copie").next().unwrap_or(nom).trim();
    let libre = |c: &str| !pris.clone().any(|p| p.trim() == c);
    let avec = |fin: String| {
        let place = crate::planchesequencesui::NOM_MAX.saturating_sub(fin.chars().count());
        let base: String = base.chars().take(place).collect();
        format!("{}{fin}", base.trim_end())
    };
    let premier = avec(" (copie)".to_string());
    if libre(&premier) {
        return premier;
    }
    (2..).map(|k| avec(format!(" (copie {k})"))).find(|c| libre(c)).unwrap_or(premier)
}

impl Document {
    /// « Dupliquer la séquence » : la séquence `seq` reçoit, juste après elle, une copie indépendante (voir le haut de ce fichier).
    /// Rend le rang du premier plan de la copie ; rien (et rien n'a changé) si la séquence n'est pas dans le film.
    pub fn dupliquer_sequence(&mut self, seq: u64) -> Option<usize> {
        let (start, end) = self.course_de_sequence(seq).filter(|(a, b)| a < b)?;
        let sources: Vec<Panel> = self.panels[start..end].to_vec();
        let suivant = self.panels.get(end).map(|p| p.id);
        let nom = self.sequences.iter().find(|s| s.id == seq).map(|s| s.name.clone()).unwrap_or_default();
        let neuve = self.new_sequence();
        let nom = nom_de_copie(self.sequences.iter().map(|s| s.name.as_str()), &nom);
        if let Some(s) = self.sequence_mut(neuve) {
            s.name = nom;
        }
        let mut c = Correspondances::default();
        // Les scènes du film, dans l'ordre : leur nom, pas leur numéro propre (il est unique dans le projet).
        for p in &sources {
            if !c.scenes.contains_key(&p.scene) {
                let id = self.scene_de_la_copie(p.scene, &mut c);
                let name = self.scene(p.scene).map(|s| s.name.clone()).unwrap_or_default();
                self.scenes.push(Scene { id, name, number: String::new(), sequence: neuve });
            }
        }
        // Les plans du film.
        let mut copies: Vec<Panel> = Vec::with_capacity(sources.len());
        for src in &sources {
            let scene = c.scenes[&src.scene];
            let p = self.plan_de_la_copie(src, scene, &mut c);
            c.plans.insert(src.id, p.id);
            copies.push(p);
        }
        // Les calques de toute la scène : la copie a les siens (clé neuve, dessins dupliqués), scène par scène.
        let mut a = 0;
        while a < copies.len() {
            let mut b = a + 1;
            while b < copies.len() && copies[b].scene == copies[a].scene {
                b += 1;
            }
            let scene = copies[a].scene;
            self.adopt_scene_layers(&mut copies[a..b], scene);
            a = b;
        }
        // Les versions des plans, leurs commentaires, leur contenu rogné.
        for (src, copie) in sources.iter().zip(&copies) {
            if let Some(v) = self.versions.get(&src.id).cloned() {
                let v = self.versions_de_la_copie(&v, copie.scene, &mut c);
                self.versions.insert(copie.id, v);
            }
            if let Some(r) = self.reserves.get(&src.id).cloned() {
                let r = self.reserve_de_la_copie(&r, src, copie, &mut c);
                self.reserves.insert(copie.id, r);
            }
        }
        let commentaires = self.commentaires_de_la_copie(|p| c.plans.get(&p).copied());
        self.comments.extend(commentaires);
        // Les transitions entre deux plans de la séquence, et celle de la coupe de sortie.
        let mut transitions: Vec<_> = self
            .transitions
            .iter()
            .filter(|t| t.out.is_some_and(|o| c.plans.contains_key(&o)) && t.inn.is_some_and(|i| c.plans.contains_key(&i)))
            .cloned()
            .collect();
        for t in &mut transitions {
            t.out = t.out.map(|o| c.plans[&o]);
            t.inn = t.inn.map(|i| c.plans[&i]);
        }
        let dernier = sources.last().map(|p| p.id);
        if let Some(mut t) = self.transitions.iter().find(|t| t.out == dernier && t.inn == suivant).cloned() {
            t.out = dernier.map(|d| c.plans[&d]);
            transitions.push(t);
        }
        for mut t in transitions {
            t.id = self.alloc();
            self.transitions.push(t);
        }
        // Les références liées à un plan de la séquence le sont aussi à sa copie.
        for r in &mut self.references {
            let neufs: Vec<u64> = r.plans.iter().filter_map(|p| c.plans.get(p).copied()).collect();
            for n in neufs {
                if !r.plans.contains(&n) {
                    r.plans.push(n);
                }
            }
        }
        // Les notes et les annotations de la planche attachées aux vignettes de la séquence (le plan, ses marques d'image).
        let vignettes: HashMap<u64, u64> = sources
            .iter()
            .zip(&copies)
            .flat_map(|(s, p)| std::iter::once((s.id, p.id)).chain(s.marks.iter().zip(&p.marks).map(|(a, b)| (a.id, b.id))))
            .collect();
        self.planche_de_la_copie(&vignettes);
        // Les autres versions de la séquence, avec leur garage.
        if let Some(v) = self.variantes.get(&seq).cloned() {
            let v = self.variantes_de_la_copie(&v, neuve, &mut c);
            Arc::make_mut(&mut self.variantes).insert(neuve, v);
        }
        // Le décor 3D : une copie indépendante, les modèles partagés (partie 190).
        let caches: Vec<Vec<u64>> = self.decors3d.par_sequence.get(&seq).map(|d| d.objets.iter().map(|o| o.caches_dans.clone()).collect()).unwrap_or_default();
        self.copier_decor_de_sequence(seq, neuve);
        // (Partie 200 : un objet caché dans un plan de l'originale l'est dans la copie de ce plan ; les objets sont copiés dans l'ordre.)
        if let Some(d) = self.decors3d.par_sequence.get_mut(&neuve) {
            for (o, avant) in d.objets.iter_mut().zip(&caches) {
                o.caches_dans = avant.iter().filter_map(|p| c.plans.get(p).copied()).collect();
            }
        }
        // (Partie 204 : un plan sorti de l'environnement de la séquence : sa copie aussi. Un environnement propre est copié pour la copie
        // du plan (« Caché dans le plan » le suit) ; un lien vers une autre séquence reste un lien vers elle.)
        let sortis: Vec<(u64, u64, crate::decor3d::EnvPlan)> = c.plans.iter().filter_map(|(&src, &neuf)| self.decors3d.envs.get(&src).map(|e| (src, neuf, *e))).collect();
        for (src, neuf, e) in sortis {
            if let crate::decor3d::EnvPlan::Propre { .. } = e {
                self.copier_env(crate::decor3d::cle_de_plan(src), crate::decor3d::cle_de_plan(neuf), |p| p == src);
                if let Some(d) = self.decors3d.par_sequence.get_mut(&crate::decor3d::cle_de_plan(neuf)) {
                    for o in &mut d.objets {
                        for p in &mut o.caches_dans {
                            *p = neuf;
                        }
                    }
                }
            }
            self.decors3d.envs.insert(neuf, e);
        }
        // Les traits des dessins copiés : des numéros neufs, communs aux parts d'un même trait dans la copie.
        self.renumeroter_la_copie(&c);
        let index = self.insert_and_normalize(end, copies);
        // En numérotation verrouillée, les scènes du film viennent de recevoir leur numéro : les versions rangées qui les citent le
        // reprennent (une même scène garde un même numéro d'une version de séquence à l'autre).
        self.accorder_les_numeros_des_variantes(neuve);
        Some(index)
    }

    /// Le numéro neuf de la scène `ancienne` dans la copie (le même à chaque appel).
    fn scene_de_la_copie(&mut self, ancienne: u64, c: &mut Correspondances) -> u64 {
        if let Some(&n) = c.scenes.get(&ancienne) {
            return n;
        }
        let n = self.alloc();
        c.scenes.insert(ancienne, n);
        n
    }

    /// Une copie du plan `src` dans la scène `scene` : numéros neufs de plan, de marques et de calques, dessins dupliqués (un dessin
    /// déjà copié pour la séquence est repris), clés de partage neuves. Les calques de toute la scène gardent leurs dessins :
    /// `adopt_scene_layers` les copie ensuite.
    fn plan_de_la_copie(&mut self, src: &Panel, scene: u64, c: &mut Correspondances) -> Panel {
        let mut panel = src.clone();
        panel.id = self.alloc();
        panel.scene = scene;
        for m in &mut panel.marks {
            m.id = self.alloc();
        }
        let old_ids: Vec<u64> = panel.layers.iter().map(|l| l.id).collect();
        for layer in &mut panel.layers {
            layer.id = fresh_layer_id();
            layer.partage = layer.partage.map(|k| *c.partages.entry(k).or_insert_with(fresh_layer_id));
            if layer.scene_wide.is_some() {
                continue;
            }
            for f in layer.frames.iter_mut().flatten() {
                *f = match c.dessins.get(f) {
                    Some(&d) => d,
                    None => {
                        let d = self.duplicate_cel(*f);
                        c.dessins.insert(*f, d);
                        d
                    }
                };
            }
        }
        self.copier_griffonnages(&src.layers, &mut c.dessins);
        relink_copies(&mut panel.layers, &old_ids, &c.dessins);
        panel
    }

    /// Une copie des versions d'un plan : mêmes numéros, dates, couleurs et sons posés ; les plans des versions rangées copiés dans la
    /// scène `scene`.
    fn versions_de_la_copie(&mut self, v: &VersionsDuPlan, scene: u64, c: &mut Correspondances) -> VersionsDuPlan {
        let mut autres = Vec::with_capacity(v.autres.len());
        for r in &v.autres {
            let mut p = self.plan_de_la_copie(&r.panel, scene, c);
            self.adopt_scene_layers(std::slice::from_mut(&mut p), scene);
            autres.push(Rangee { numero: r.numero, date: r.date, panel: p });
        }
        VersionsDuPlan { actuelle: v.actuelle, date_actuelle: v.date_actuelle, prochaine: v.prochaine, autres, couleurs: v.couleurs.clone(), sons: v.sons.clone() }
    }

    /// Une copie du contenu rogné du plan `src` pour sa copie `copie` : les calques suivent leur copie (même rang), les dessins sont
    /// copiés, les marques et les commentaires sortis ont des numéros neufs.
    fn reserve_de_la_copie(&mut self, r: &Reserve, src: &Panel, copie: &Panel, c: &mut Correspondances) -> Reserve {
        let calques: HashMap<u64, u64> = src.layers.iter().zip(&copie.layers).map(|(a, b)| (a.id, b.id)).collect();
        let mut r = r.clone();
        for piece in [&mut r.head, &mut r.tail] {
            for l in &mut piece.layers {
                l.layer = calques.get(&l.layer).copied().unwrap_or(l.layer);
                for f in l.frames.iter_mut().flatten() {
                    *f = match c.dessins.get(f) {
                        Some(&d) => d,
                        None => {
                            let d = self.duplicate_cel(*f);
                            c.dessins.insert(*f, d);
                            d
                        }
                    };
                }
            }
            for m in &mut piece.marks {
                m.id = self.alloc();
            }
            for k in &mut piece.comments {
                k.id = self.alloc();
                k.panel = copie.id;
            }
        }
        r
    }

    /// Les commentaires des plans que `copie_de` connaît, copiés (numéros neufs) sur leur copie.
    fn commentaires_de_la_copie(&mut self, copie_de: impl Fn(u64) -> Option<u64>) -> Vec<Comment> {
        let mut out: Vec<Comment> = self.comments.iter().filter_map(|k| copie_de(k.panel).map(|p| Comment { panel: p, ..k.clone() })).collect();
        for k in &mut out {
            k.id = self.alloc();
        }
        out
    }

    /// Les notes de la planche (traits, textes) et les annotations (hors du calque du dessin hors des cases) attachées à une vignette de
    /// `vignettes` : une copie attachée à la vignette copiée, à la même place par rapport à elle.
    fn planche_de_la_copie(&mut self, vignettes: &HashMap<u64, u64>) {
        let traits: Vec<NoteStroke> = self.board.strokes.iter().filter_map(|s| s.anchor.and_then(|a| vignettes.get(&a)).map(|&n| NoteStroke { anchor: Some(n), ..s.clone() })).collect();
        let textes: Vec<NoteText> = self.board.texts.iter().filter_map(|t| t.anchor.and_then(|a| vignettes.get(&a)).map(|&n| NoteText { anchor: Some(n), ..t.clone() })).collect();
        self.board.strokes.extend(traits);
        self.board.texts.extend(textes);
        let a = &mut self.annotations;
        let hors_cases: Vec<u64> = a.calques.iter().filter(|k| k.planche).map(|k| k.id).collect();
        let neufs: Vec<Trace> = a
            .traces
            .iter()
            .filter(|t| !hors_cases.contains(&t.calque))
            .filter_map(|t| {
                let &card = vignettes.get(&t.anchor.card)?;
                let mut n = Trace::new(t.calque, Ancre { card, ..t.anchor }, t.color, t.points.clone());
                n.alpha = t.alpha;
                n.brosse = t.brosse.clone();
                n.dessus = t.dessus;
                Some(n)
            })
            .collect();
        a.traces.extend(neufs);
    }

    /// Une copie des versions de séquence `v` pour la séquence `neuve` : le garage d'abord (des copies de ses plans, avec leurs
    /// versions, leur contenu rogné et leurs commentaires), puis le découpage de chaque version rangée, renuméroté.
    fn variantes_de_la_copie(&mut self, v: &VariantesDeSequence, neuve: u64, c: &mut Correspondances) -> VariantesDeSequence {
        let mut garage = BTreeMap::new();
        for (id, parque) in &v.garage {
            let scene = self.scene_de_la_copie(parque.panel.scene, c);
            let mut panel = self.plan_de_la_copie(&parque.panel, scene, c);
            self.adopt_scene_layers(std::slice::from_mut(&mut panel), scene);
            let versions = parque.versions.as_ref().map(|x| self.versions_de_la_copie(x, scene, c));
            let reserve = parque.reserve.as_ref().map(|r| self.reserve_de_la_copie(r, &parque.panel, &panel, c));
            let mut comments: Vec<Comment> = parque.comments.iter().map(|k| Comment { panel: panel.id, ..k.clone() }).collect();
            for k in &mut comments {
                k.id = self.alloc();
            }
            c.plans.insert(*id, panel.id);
            garage.insert(panel.id, Parque { panel, versions, reserve, comments });
        }
        let mut autres = Vec::with_capacity(v.autres.len());
        for r in &v.autres {
            let x = &r.contenu;
            let mut scenes = Vec::with_capacity(x.scenes.len());
            for s in &x.scenes {
                let id = self.scene_de_la_copie(s.id, c);
                scenes.push(Scene { id, name: s.name.clone(), number: String::new(), sequence: neuve });
            }
            let mut slots = Vec::with_capacity(x.slots.len());
            for e in &x.slots {
                let scene = self.scene_de_la_copie(e.scene, c);
                // (Un plan qu'elle montre et que le film a mis dans une autre séquence reste ce plan-là : `accorder_au_film`.)
                slots.push(Emplacement { plan: c.plans.get(&e.plan).copied().unwrap_or(e.plan), scene, ..e.clone() });
            }
            let plan = |p: u64| c.plans.get(&p).copied().unwrap_or(p);
            let mut transitions = x.transitions.clone();
            for t in &mut transitions {
                t.out = t.out.map(plan);
                t.inn = t.inn.map(plan);
            }
            let references = x.references.iter().map(|(rid, liens)| (*rid, liens.iter().map(|&p| plan(p)).collect())).collect();
            for t in &mut transitions {
                t.id = self.alloc();
            }
            autres.push(VarianteRangee { numero: r.numero, nom: r.nom.clone(), date: r.date, contenu: Contenu { scenes, slots, transitions, references } });
        }
        VariantesDeSequence {
            actuelle: v.actuelle,
            nom_actuelle: v.nom_actuelle.clone(),
            date_actuelle: v.date_actuelle,
            autres,
            // (Les copies ne sont pas les plans d'un envoi : rien ne les fait remonter à l'original, `plan_dans_envoi`.)
            origines: BTreeMap::new(),
            garage,
        }
    }

    /// Les traits des dessins copiés prennent des numéros neufs ; les parts d'un même trait (dans un ou plusieurs dessins de la copie)
    /// en gardent un commun, sans lien avec l'original (partie 125, `vector::renumeroter` pour un seul dessin).
    fn renumeroter_la_copie(&mut self, c: &Correspondances) {
        let mut neufs: HashMap<u64, u64> = HashMap::new();
        for d in c.dessins.values() {
            let Some(v) = self.cels.get_mut(d).and_then(|x| x.vector.as_mut()) else { continue };
            for e in &mut v.elements {
                if let crate::vector::Element::Stroke { trace, .. } = e {
                    if let Some(l) = trace.lien {
                        trace.lien = Some(*neufs.entry(l).or_insert_with(crate::horschamp::nouveau_lien));
                    }
                }
            }
        }
    }

    /// Les scènes des versions rangées de la séquence `seq` que le film montre aussi reprennent le numéro propre qu'elles y ont.
    fn accorder_les_numeros_des_variantes(&mut self, seq: u64) {
        if !self.numbering_locked || !self.variantes.contains_key(&seq) {
            return;
        }
        let numeros: HashMap<u64, String> = self.scenes.iter().filter(|s| s.sequence == seq).map(|s| (s.id, s.number.clone())).collect();
        if let Some(v) = Arc::make_mut(&mut self.variantes).get_mut(&seq) {
            for r in &mut v.autres {
                for s in &mut r.contenu.scenes {
                    if let Some(n) = numeros.get(&s.id) {
                        s.number = n.clone();
                    }
                }
            }
        }
    }
}

impl App {
    /// « Dupliquer la séquence » (la commande) : la séquence du plan courant.
    pub(crate) fn sequence_dupliquer_courante(&mut self) {
        let seq = self.doc.sequence_of(self.cur_panel);
        self.sequence_dupliquer(seq);
    }

    /// « Dupliquer la séquence » `seq` (les menus d'une séquence) : une copie indépendante juste après elle, qui devient la séquence
    /// courante. Une étape d'annulation.
    pub(crate) fn sequence_dupliquer(&mut self, seq: u64) {
        if self.doc.film_vide() {
            self.status = crate::planannuleui::REFUS_SUPPORT.into();
            return;
        }
        let avant = self.doc.sequence_label(seq);
        let ok = self.avec_etape("Dupliquer la séquence", |s| s.structure_op_dessins(|d, _| d.dupliquer_sequence(seq)));
        if !ok {
            return;
        }
        let copie = self.doc.sequence_of(self.cur_panel);
        let (a, b) = self.doc.sequence_run(self.cur_panel);
        let plans = if b - a == 1 { "1 plan".to_string() } else { format!("{} plans", b - a) };
        self.status = format!("Séquence {avant} dupliquée : sa copie, {}, la suit ({plans}). Ctrl+Z la retire.", self.doc.sequence_label(copie));
        self.planche_aller_a_la_sequence(copie);
    }
}
