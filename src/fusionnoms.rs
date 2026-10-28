//! Réunir deux plans qui ont un calque de même nom : la question (Anthony, 2026-10-28).
//!
//! Réunir deux plans (« Réunir avec le suivant » ou « le précédent », la barre noire des Vignettes, « Fusionner les plans choisis », la fusion
//! en glissant un plan sur un autre) fait un seul plan de leurs images, calque par calque : un calque du second va avec le calque de même
//! nom du premier s'ils sont réglés pareil (`document::layer_targets`). Deux calques « Dessin » n'en font donc plus qu'un. Quand ça se
//! présente, une fenêtre le demande d'abord, avec trois réponses :
//! - **Fusionner ces calques** : ce qui se faisait déjà ; les calques de même nom n'en font plus qu'un ;
//! - **Renommer avant de fusionner** : la liste des calques en conflit, chacun avec un champ pour son nouveau nom (« Dessin_2 » d'avance
//!   pour celui du second plan) ; une fois renommés, ils ne se réunissent plus (ni par nom ni par rang), sauf si on retape un nom que
//!   le premier plan a ;
//! - **Ne pas fusionner les calques** : ils restent séparés, celui du second plan renommé « Dessin_2 » (« _3 »… si ce nom existe).
//!
//! « Annuler » (ou Échap) abandonne la jonction : rien ne bouge. La fenêtre ne s'ouvre que s'il y a un nom en commun entre calques qui se
//! seraient réunis ; sinon la jonction se fait tout de suite. Une seule étape d'annulation pour toute la jonction, renommages compris
//! (`App::structure_op_dessins`). Aucun dessin n'est perdu, quelle que soit la réponse : les calques gardés à part portent tous leurs images.
//!
//! Le réglage passe par `JoinOpts` jusqu'à `Document::append_plan` (qui garde sa règle de réunion par nom puis par rang).

use std::collections::{BTreeMap, HashSet};

use eframe::egui::{self, vec2, Align2};

use crate::app::App;
use crate::document::{layer_targets, Document, Layer};

/// Ce que deviennent les calques de même nom d'une jonction.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum LayerJoin {
    /// « Fusionner ces calques » : la règle d'avant (par nom, puis par rang).
    #[default]
    Merge,
    /// « Renommer avant de fusionner » : ces calques du second plan (par numéro) prennent ces noms d'abord, et ne se réunissent pas par rang.
    Rename(BTreeMap<u64, String>),
    /// « Ne pas fusionner les calques » : ceux du second plan qui se seraient réunis par leur nom restent à part, renommés « Nom_2 »…
    Apart,
}

/// Les réglages d'une jonction de plans (`Document::join_plans_with`, `append_plan`).
#[derive(Clone, Debug, Default)]
pub struct JoinOpts {
    pub layers: LayerJoin,
}

impl JoinOpts {
    pub fn de(layers: LayerJoin) -> JoinOpts {
        JoinOpts { layers }
    }
}

impl LayerJoin {
    /// Prépare les calques `b` du second plan avant qu'ils rejoignent les calques `a` du premier : les renomme (`Rename`, `Apart`) et rend
    /// ceux qui ne doivent pas se réunir par rang.
    pub(crate) fn prepare(&self, a: &[Layer], b: &mut [Layer]) -> HashSet<u64> {
        let mut apart = HashSet::new();
        match self {
            LayerJoin::Merge => {}
            LayerJoin::Rename(names) => {
                for lb in b.iter_mut() {
                    if let Some(n) = names.get(&lb.id) {
                        lb.name = n.clone();
                        apart.insert(lb.id);
                    }
                }
            }
            LayerJoin::Apart => {
                let target = layer_targets(a, b, &HashSet::new());
                let mut pris: HashSet<String> = a.iter().chain(b.iter()).map(|l| l.name.clone()).collect();
                for (j, lb) in b.iter_mut().enumerate() {
                    if target[j].is_some_and(|k| a[k].name == lb.name) {
                        let neuf = nom_libre(&lb.name, &pris);
                        pris.insert(neuf.clone());
                        lb.name = neuf;
                        apart.insert(lb.id);
                    }
                }
            }
        }
        apart
    }
}

/// Le premier nom « Base_2 », « Base_3 »… qui n'est pas dans `pris` (un « _n » déjà au bout de `base` est remplacé : « Dessin_2 » → « Dessin_3 »).
pub fn nom_libre(base: &str, pris: &HashSet<String>) -> String {
    let souche = match base.rsplit_once('_') {
        Some((s, n)) if !s.is_empty() && !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => s,
        _ => base,
    };
    (2..).map(|n| format!("{souche}_{n}")).find(|c| !pris.contains(c)).unwrap_or_else(|| format!("{souche}_2"))
}

/// Un calque d'un plan qui se réunirait au calque de même nom d'un plan d'avant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameConflict {
    /// Le plan (son numéro) à qui est le calque.
    pub panel: u64,
    pub layer: u64,
    pub name: String,
}

impl Document {
    /// Les calques de même nom que la réunion des plans `ids` (qui se suivent, dans l'ordre du film) fondrait en un seul : ceux des plans
    /// d'après le premier. Rien ne bouge. Les calques Décor, que la jonction traite à part (`decor.rs`), n'y sont pas.
    pub fn join_name_conflicts(&self, ids: &[u64]) -> Vec<NameConflict> {
        let mut at: Vec<usize> = ids.iter().filter_map(|id| self.panels.iter().position(|p| p.id == *id)).collect();
        at.sort_unstable();
        at.dedup();
        let mut out = Vec::new();
        let Some(&first) = at.first() else { return out };
        // Les calques du plan réuni au fil des jonctions : ceux du premier, puis ceux que chaque plan ajoute (les autres s'y fondent).
        let sans_dessins = |l: &Layer| Layer { frames: Vec::new(), ..l.clone() };
        let mut pool: Vec<Layer> = self.panels[first].layers.iter().filter(|l| !l.decor).map(sans_dessins).collect();
        for &k in &at[1..] {
            let b: Vec<Layer> = self.panels[k].layers.iter().filter(|l| !l.decor).map(sans_dessins).collect();
            let target = layer_targets(&pool, &b, &HashSet::new());
            for (j, lb) in b.iter().enumerate() {
                if let Some(ka) = target[j] {
                    if pool[ka].name == lb.name {
                        out.push(NameConflict { panel: self.panels[k].id, layer: lb.id, name: lb.name.clone() });
                    }
                }
            }
            for (j, lb) in b.into_iter().enumerate() {
                if target[j].is_none() {
                    pool.push(lb);
                }
            }
        }
        out
    }
}

/// Ce que l'on répond à la question.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Reponse {
    Fusionner,
    /// Les nouveaux noms, par numéro de calque.
    Renommer(BTreeMap<u64, String>),
    Separer,
}

impl Reponse {
    pub(crate) fn en_options(&self) -> JoinOpts {
        JoinOpts::de(match self {
            Reponse::Fusionner => LayerJoin::Merge,
            Reponse::Renommer(noms) => LayerJoin::Rename(noms.clone()),
            Reponse::Separer => LayerJoin::Apart,
        })
    }
}

/// Une jonction de plans que la question retient (ou que rien ne retient).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Jonction {
    /// Le plan `a` et le suivant (« Réunir avec le suivant » ; `precedent` : on est parti du second, « Réunir avec le plan précédent »).
    Suivant { a: u64, precedent: bool },
    /// Les plans choisis, qui se suivent dans une même séquence.
    Choisis(Vec<u64>),
    /// Le plan `x` lâché au milieu du plan `y` (`fusionglisse.rs`).
    Glisse { x: u64, y: u64 },
    /// La barre noire des Vignettes entre le plan `a` (dont la dernière case finit à l'image `fin`) et le suivant.
    Case { a: u64, fin: usize },
}

impl Jonction {
    /// Les plans que cette jonction réunirait, dans l'ordre du film ; rien si elle ne peut pas se faire (le geste le dira).
    fn plans_a_voir(&self, doc: &Document) -> Vec<u64> {
        let pos = |id: u64| doc.panels.iter().position(|p| p.id == id);
        let meme_sequence = |ids: &[usize]| ids.windows(2).all(|w| doc.sequence_of(w[0]) == doc.sequence_of(w[1]));
        let ids = |at: &[usize]| at.iter().map(|&i| doc.panels[i].id).collect::<Vec<u64>>();
        match self {
            Jonction::Suivant { a, .. } | Jonction::Case { a, .. } => {
                let Some(i) = pos(*a).filter(|i| i + 1 < doc.panels.len()) else { return Vec::new() };
                if meme_sequence(&[i, i + 1]) {
                    ids(&[i, i + 1])
                } else {
                    Vec::new()
                }
            }
            Jonction::Choisis(chosen) => {
                let mut at: Vec<usize> = chosen.iter().filter_map(|id| pos(*id)).collect();
                at.sort_unstable();
                at.dedup();
                if at.len() >= 2 && at.windows(2).all(|w| w[1] == w[0] + 1) && meme_sequence(&at) {
                    ids(&at)
                } else {
                    Vec::new()
                }
            }
            Jonction::Glisse { x, y } => {
                let (Some(px), Some(py)) = (pos(*x), pos(*y)) else { return Vec::new() };
                if px != py && meme_sequence(&[px, py]) {
                    ids(&[px.min(py), px.max(py)])
                } else {
                    Vec::new()
                }
            }
        }
    }
}

/// Une ligne de la fenêtre : un calque en conflit, avec le nom qu'on lui donnera si on le renomme.
pub(crate) struct Ligne {
    pub plan: u64,
    pub layer: u64,
    pub nom: String,
    pub nouveau: String,
}

/// Où en est la fenêtre : le choix entre les trois réponses, ou la liste des noms à retaper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Etape {
    Choix,
    Renommer,
}

/// La fenêtre ouverte : la jonction qu'elle retient, les calques en conflit, l'étape.
pub(crate) struct FusionNoms {
    pub quoi: Jonction,
    pub lignes: Vec<Ligne>,
    pub etape: Etape,
}

impl FusionNoms {
    /// Les noms retapés, par numéro de calque : tous non vides, sinon rien (le bouton se grise).
    pub(crate) fn noms_retapes(&self) -> Option<BTreeMap<u64, String>> {
        let mut m = BTreeMap::new();
        for l in &self.lignes {
            let n = l.nouveau.trim();
            if n.is_empty() {
                return None;
            }
            m.insert(l.layer, n.to_string());
        }
        Some(m)
    }
}

impl App {
    /// Réunit les plans de `quoi` : tout de suite si aucun calque n'a le même nom, sinon la fenêtre pose la question d'abord (une réponse
    /// d'avance, `fusion_noms_auto`, la remplace). Vrai si la jonction est faite.
    pub(crate) fn jonction_demander(&mut self, quoi: Jonction) -> bool {
        let ids = quoi.plans_a_voir(&self.doc);
        let conflits = if ids.len() >= 2 { self.doc.join_name_conflicts(&ids) } else { Vec::new() };
        if conflits.is_empty() {
            return self.jonction_faire(quoi, &JoinOpts::default());
        }
        if let Some(r) = self.fusion_noms_auto.clone() {
            return self.jonction_faire(quoi, &r.en_options());
        }
        // Les noms d'avance : « Dessin_2 » pour le calque du second plan, « Dessin_3 » pour celui d'un troisième…
        let mut pris: HashSet<String> = ids.iter().filter_map(|id| self.doc.panels.iter().find(|p| p.id == *id)).flat_map(|p| p.layers.iter().map(|l| l.name.clone())).collect();
        let lignes = conflits
            .into_iter()
            .map(|c| {
                let nouveau = nom_libre(&c.name, &pris);
                pris.insert(nouveau.clone());
                Ligne { plan: c.panel, layer: c.layer, nom: c.name, nouveau }
            })
            .collect();
        self.fusion_noms = Some(FusionNoms { quoi, lignes, etape: Etape::Choix });
        self.status = "Des calques ont le même nom : choisis ce qu'ils deviennent avant de réunir les plans.".into();
        false
    }

    /// Fait la jonction `quoi` avec la réponse `opts` : une étape d'annulation, et les mêmes phrases que sans la question.
    pub(crate) fn jonction_faire(&mut self, quoi: Jonction, opts: &JoinOpts) -> bool {
        // (L'étape d'annulation porte le nom de la commande, aussi quand la réponse vient de la fenêtre, après la commande.)
        let nom = match &quoi {
            Jonction::Suivant { precedent: false, .. } => "Réunir le plan et le suivant",
            Jonction::Suivant { precedent: true, .. } => "Réunir avec le plan précédent",
            Jonction::Choisis(_) => "Fusionner les plans choisis",
            Jonction::Glisse { .. } => "Fusionner deux plans",
            Jonction::Case { .. } => "Réunir deux vignettes en un plan",
        };
        self.avec_etape(nom, |s| s.jonction_faire_nue(quoi, opts))
    }

    fn jonction_faire_nue(&mut self, quoi: Jonction, opts: &JoinOpts) -> bool {
        match quoi {
            Jonction::Suivant { a, precedent } => {
                if self.structure_op_dessins(|d, _| {
                    let i = d.panels.iter().position(|p| p.id == a)?;
                    d.join_next_plan_without_mark_with(i, opts)
                }) {
                    self.status = format!("Plans réunis : {}.", crate::storyboard::panel_title(&self.doc, self.cur_panel));
                    true
                } else {
                    self.status = if precedent { "Pas de plan précédent dans cette séquence." } else { "Pas de plan suivant dans cette séquence." }.into();
                    false
                }
            }
            Jonction::Choisis(ids) => {
                let n = ids.len();
                if self.structure_op_dessins(|d, _| d.join_plans_with(&ids, opts)) {
                    self.board_sel.clear();
                    self.status = format!("{n} plans fusionnés : {}.", crate::storyboard::panel_title(&self.doc, self.cur_panel));
                    true
                } else {
                    self.status = "Les plans choisis doivent se suivre, dans une même séquence.".into();
                    false
                }
            }
            Jonction::Glisse { x, y } => {
                let fait = self.structure_op_dessins(|d, _| d.merge_plan_into_with(x, y, opts));
                if fait {
                    self.board_sel.clear();
                    self.status = format!("Plans fusionnés : {}.", crate::storyboard::panel_title(&self.doc, self.cur_panel));
                } else {
                    self.status = "Ces deux plans ne peuvent pas être fusionnés : ils doivent être de la même séquence.".into();
                }
                fait
            }
            Jonction::Case { a, fin } => {
                let ok = self.structure_op_dessins(|d, _| {
                    let ia = d.panels.iter().position(|p| p.id == a)?;
                    let debut = d.panels.get(ia)?.frame_count();
                    let i = d.join_next_plan_with(ia, opts)?;
                    // La case de droite devient une clé de dessin de plus sur chaque calque du plan (`cases.rs`).
                    d.cles_de_case(i, debut);
                    Some(i)
                });
                if ok {
                    self.cur_frame = fin;
                    self.board_sel = vec![self.doc.panels[self.cur_panel].id];
                    self.status = format!("Plans réunis : la vignette de droite est une image du {}.", crate::storyboard::panel_title(&self.doc, self.cur_panel).to_lowercase());
                } else {
                    self.status = "Ces deux plans ne sont pas dans la même séquence : impossible de les réunir.".into();
                }
                ok
            }
        }
    }

    /// La réponse donnée à la fenêtre : la jonction se fait (une étape d'annulation) et la fenêtre se ferme. Vrai si la jonction est faite.
    pub(crate) fn fusion_noms_repondre(&mut self, reponse: Reponse) -> bool {
        let Some(dlg) = self.fusion_noms.take() else { return false };
        self.jonction_faire(dlg.quoi, &reponse.en_options())
    }

    /// « Annuler » : la fenêtre se ferme, rien ne bouge.
    pub(crate) fn fusion_noms_annuler(&mut self) {
        if self.fusion_noms.take().is_some() {
            self.status = "Plans non réunis : rien n'a changé.".into();
        }
    }

    /// La fenêtre « Calques de même nom ».
    pub(crate) fn fusion_noms_ui(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.fusion_noms.take() else { return };
        // (Un plan qui n'existe plus — annulation, film partagé — ferme la question : la jonction n'a plus de sens.)
        if dlg.lignes.iter().any(|l| !self.doc.panels.iter().any(|p| p.id == l.plan)) {
            return;
        }
        let titre_du_plan = |app: &App, id: u64| app.doc.panels.iter().position(|p| p.id == id).map_or_else(String::new, |i| crate::storyboard::panel_title(&app.doc, i));
        let mut reponse: Option<Reponse> = None;
        let mut annuler = false;
        egui::Window::new("Calques de même nom")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                let n = dlg.lignes.len();
                ui.label(format!("Les plans à réunir ont {} calque{} de même nom :", if n > 1 { n.to_string() } else { "un".into() }, if n > 1 { "s" } else { "" }));
                match dlg.etape {
                    Etape::Choix => {
                        for l in &dlg.lignes {
                            ui.label(format!("   « {} » ({})", l.nom, titre_du_plan(self, l.plan)));
                        }
                        ui.add_space(4.0);
                        let a = ui.button("Fusionner ces calques");
                        if crate::bulles::bulle(a, "Fusionner ces calques", "Les calques de même nom n'en font plus qu'un : les images du second plan viennent à la suite de celles du premier, sur le même calque.", "").clicked() {
                            reponse = Some(Reponse::Fusionner);
                        }
                        let b = ui.button("Renommer avant de fusionner");
                        if crate::bulles::bulle(b, "Renommer avant de fusionner", "Tu donnes un autre nom aux calques du second plan, qui restent alors séparés de ceux du premier.", "").clicked() {
                            dlg.etape = Etape::Renommer;
                        }
                        let c = ui.button("Ne pas fusionner les calques");
                        if crate::bulles::bulle(c, "Ne pas fusionner les calques", "Chaque calque reste à part : celui du second plan est renommé « Dessin_2 » (« _3 »… si ce nom existe déjà).", "").clicked() {
                            reponse = Some(Reponse::Separer);
                        }
                        ui.add_space(4.0);
                        if ui.button("Annuler").clicked() {
                            annuler = true;
                        }
                    }
                    Etape::Renommer => {
                        ui.label(egui::RichText::new("Le nom que prendra chaque calque du second plan, avant la fusion :").weak().small());
                        egui::Grid::new("fusion_noms_grille").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
                            let titres: Vec<String> = dlg.lignes.iter().map(|l| titre_du_plan(self, l.plan)).collect();
                            for (l, titre) in dlg.lignes.iter_mut().zip(titres) {
                                ui.label(titre);
                                ui.label(format!("« {} » →", l.nom));
                                ui.add(egui::TextEdit::singleline(&mut l.nouveau).desired_width(150.0));
                                ui.end_row();
                            }
                        });
                        let noms = dlg.noms_retapes();
                        if noms.is_none() {
                            ui.colored_label(ui.visuals().warn_fg_color, "Chaque calque a besoin d'un nom.");
                        }
                        ui.horizontal(|ui| {
                            if ui.add_enabled(noms.is_some(), egui::Button::new("Renommer et fusionner")).clicked() {
                                reponse = noms.clone().map(Reponse::Renommer);
                            }
                            if ui.button("Retour").clicked() {
                                dlg.etape = Etape::Choix;
                            }
                            if ui.button("Annuler").clicked() {
                                annuler = true;
                            }
                        });
                    }
                }
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            annuler = true;
        }
        if let Some(r) = reponse {
            self.fusion_noms = Some(dlg);
            self.fusion_noms_repondre(r);
        } else if annuler {
            self.fusion_noms = Some(dlg);
            self.fusion_noms_annuler();
        } else {
            self.fusion_noms = Some(dlg);
        }
    }
}
