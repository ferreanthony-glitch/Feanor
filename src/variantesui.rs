//! Les versions de séquence dans l'interface (`variantes.rs` pour le modèle et le fichier ; le code dit encore « variante ») :
//! le petit sélecteur « A · B · C » en haut de la vue Vignettes, l'entrée « Versions de la séquence » du menu Storyboard et du
//! clic droit sur la barre de séquence du montage, les commandes `variante.*` (Ctrl+K), la confirmation de suppression, le
//! renommage et la fenêtre « Mettre à jour dans les autres versions de séquence ».
//!
//! **Un geste = une étape d'annulation, nommée** (panneau Historique) : « Nouvelle version de la séquence », « Passer à la
//! version de séquence B », « Supprimer la version de séquence B », « Renommer la version de séquence B », « Mettre à jour le
//! plan dans N versions de séquence ». Aucune ne touche un pixel sur le fil de l'interface : changer de version de séquence
//! échange des listes de plans (les tuiles sont partagées).
//!
//! Le storyboard verrouillé refuse tout cela. Dans un film partagé il faut le jeton du montage, et chaque plan que l'échange fait
//! sortir du film doit être à moi ou libre (comme pour supprimer des plans, `partage_peut_toucher`).

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::document::Document;
use crate::theme;
use crate::variantes::{etiquette, lettre, Nouvelle};

/// La fenêtre « Mettre à jour dans les autres versions de séquence » : le plan `plan` de `sequence`, et pour chaque autre version
/// de séquence qui le montre (numéro, case cochée).
pub(crate) struct MiseAJour {
    pub sequence: u64,
    pub plan: u64,
    pub coches: Vec<(u32, bool)>,
}

/// Les fenêtres des versions de séquence (un champ dans `App`).
#[derive(Default)]
pub(crate) struct VariantesUi {
    /// La version de séquence dont on confirme la suppression : (séquence, numéro).
    pub supprimer: Option<(u64, u32)>,
    /// La version de séquence qu'on renomme : (séquence, numéro, nom en cours de frappe, mettre le curseur dans le champ).
    pub renommer: Option<(u64, u32, String, bool)>,
    /// La mise à jour d'un plan dans les autres versions de séquence.
    pub mettre_a_jour: Option<MiseAJour>,
}

/// « montrée par la version de séquence B » : ce que disent les cartes et les lignes d'une version de plan quand une autre
/// version de séquence la montre (vide : aucune).
pub(crate) fn montree_par(doc: &Document, sequence: u64, plan: u64, version: u32) -> String {
    let noms: Vec<String> = doc.variantes_montrant(sequence, plan).into_iter().filter(|(_, v)| *v == version).map(|(n, _)| lettre(n)).collect();
    match noms.len() {
        0 => String::new(),
        1 => format!("montrée par la version de séquence {}", noms[0]),
        _ => format!("montrée par les versions de séquence {}", noms.join(", ")),
    }
}

impl App {
    /// La séquence du plan courant.
    fn sequence_courante(&self) -> Option<u64> {
        self.doc.panels.get(self.cur_panel)?;
        Some(self.doc.sequence_of(self.cur_panel))
    }

    /// Une opération sur les versions de séquence de `sequence`, avec une étape d'annulation nommée `nom`. `sort` : l'opération
    /// fait sortir des plans du film (dans un film partagé, ils doivent être à moi ou libres). `op` rend faux si rien n'a changé.
    fn variante_op(&mut self, nom: &str, sequence: u64, sort: bool, op: impl FnOnce(&mut Document) -> bool) -> bool {
        // Verrouillé : rien ne change ; film partagé : il faut le jeton du montage.
        if self.storyboard_refuses() {
            return false;
        }
        if self.doc.variantes_partielles {
            self.status = "Des dessins de versions de séquence manquent (fichiers pas encore arrivés dans le dossier partagé, ou illisibles) : les versions de séquence ne changent pas tant qu'ils ne sont pas là.".into();
            return false;
        }
        if sort {
            let ids = self.doc.plans_de_sequence(sequence);
            if !self.partage_peut_toucher(&ids) {
                return false;
            }
        }
        self.end_stroke();
        self.poser_les_selections();
        self.playing = false;
        let courant = self.doc.panels.get(self.cur_panel).map(|p| p.id);
        let snap = self.snapshot();
        if !op(&mut self.doc) {
            return false;
        }
        // Le plan courant reste courant s'il est encore là ; sinon le premier de la séquence. (Avant l'étape
        // d'annulation : elle relit les calques du plan courant.)
        match courant.and_then(|id| self.doc.panels.iter().position(|p| p.id == id)) {
            Some(i) => self.cur_panel = i,
            None => {
                self.cur_panel = self.doc.course_de_sequence(sequence).map_or(0, |(debut, _)| debut);
                self.cur_frame = 0;
            }
        }
        self.cur_panel = self.cur_panel.min(self.doc.panels.len() - 1);
        self.cur_layer = self.cur_layer.min(self.layers().len() - 1);
        let avant = std::mem::replace(&mut self.history.context, Some(nom.to_string()));
        self.commit_structure(snap);
        self.history.context = avant;
        self.board_sel.clear();
        self.follow_frame = true;
        self.after_rognage();
        true
    }

    /// « Nouvelle version de séquence » : le découpage en cours est gardé, et on continue sur les mêmes plans (`Partager`), sur
    /// une copie des plans (`Copier`) ou sur un plan vide (`Vide`).
    pub(crate) fn nouvelle_variante_de_sequence(&mut self, sequence: u64, mode: Nouvelle) {
        let ancienne = self.doc.numero_de_variante(sequence);
        let now = crate::recovery::now_secs();
        let mut neuve = 0;
        let nom = match mode {
            Nouvelle::Partager => "Nouvelle version de la séquence",
            Nouvelle::Copier => "Nouvelle version de la séquence, plans copiés",
            Nouvelle::Vide => "Nouvelle version de la séquence, vide",
        };
        if self.variante_op(nom, sequence, mode == Nouvelle::Vide, |d| match d.nouvelle_variante(sequence, mode, now) {
            Some(n) => {
                neuve = n;
                true
            }
            None => false,
        }) {
            let seq = self.doc.sequence_label(sequence);
            let comment = match mode {
                Nouvelle::Partager => ", avec les mêmes plans (retoucher un plan le retouche dans les deux ; une nouvelle version du plan reste à part)",
                Nouvelle::Copier => ", avec une copie des plans (rien ne la relie à l'autre)",
                Nouvelle::Vide => ", à partir d'un plan vide",
            };
            self.status = format!(
                "Nouvelle version de la séquence {seq} : {}{comment}. La {} est gardée (« Passer à la version de séquence » la ramène).",
                lettre(neuve),
                lettre(ancienne)
            );
        } else if !self.doc.storyboard_locked && self.doc.nombre_de_variantes(sequence) >= crate::variantes::MAX_VARIANTES {
            self.status = format!("Nouvelle version impossible : la séquence a déjà {} versions.", crate::variantes::MAX_VARIANTES);
        }
    }

    /// Passe la séquence à sa version de séquence rangée `numero`.
    pub(crate) fn passer_a_la_variante(&mut self, sequence: u64, numero: u32) {
        let ancienne = self.doc.numero_de_variante(sequence);
        let avant = self.doc.plans_de_sequence(sequence).len();
        let nom = format!("Passer à la version de séquence {}", lettre(numero));
        if self.variante_op(&nom, sequence, true, |d| d.changer_de_variante(sequence, numero)) {
            let apres = self.doc.plans_de_sequence(sequence).len();
            let seq = self.doc.sequence_label(sequence);
            let nom_neuve = self.doc.etiquette_de_variante(sequence, numero);
            self.status = format!(
                "La séquence {seq} est maintenant en version {nom_neuve} ({} plan{}, avant : {avant}) ; la {} est gardée.",
                apres,
                if apres > 1 { "s" } else { "" },
                lettre(ancienne)
            );
        }
    }

    /// Passe à la version de séquence avant / après la choisie (en tournant).
    fn variante_voisine(&mut self, sens: i32) {
        let Some(sequence) = self.sequence_courante() else { return };
        let Some(v) = self.doc.variantes_de(sequence) else {
            self.status = "Cette séquence n'a qu'une version : « Nouvelle version de la séquence » en crée une autre.".into();
            return;
        };
        let numeros = v.numeros();
        let k = numeros.iter().position(|n| *n == v.actuelle).unwrap_or(0) as i32;
        let cible = numeros[(k + sens).rem_euclid(numeros.len() as i32) as usize];
        self.passer_a_la_variante(sequence, cible);
    }

    /// Demande confirmation avant de supprimer la version de séquence `numero` (la dernière ne se supprime pas).
    pub(crate) fn supprimer_variante_demander(&mut self, sequence: u64, numero: u32) {
        if self.doc.storyboard_locked {
            self.status = crate::projetui::STORYBOARD_LOCKED.into();
            return;
        }
        if self.doc.nombre_de_variantes(sequence) < 2 {
            self.status = "La dernière version d'une séquence ne se supprime pas.".into();
            return;
        }
        self.variantes_ui.supprimer = Some((sequence, numero));
    }

    /// Supprime la version de séquence `numero` (une étape d'annulation la ramène).
    pub(crate) fn supprimer_la_variante(&mut self, sequence: u64, numero: u32) {
        let sort = self.doc.numero_de_variante(sequence) == numero;
        let nom = format!("Supprimer la version de séquence {}", lettre(numero));
        let label = self.doc.etiquette_de_variante(sequence, numero);
        let mut reste = 0;
        if self.variante_op(&nom, sequence, sort, |d| match d.supprimer_variante(sequence, numero) {
            Some(r) => {
                reste = r;
                true
            }
            None => false,
        }) {
            self.status = format!("Version de séquence {label} supprimée ; la séquence est en version {} (Ctrl+Z la ramène).", lettre(reste));
        }
    }

    /// Donne le nom court `nom` à la version de séquence `numero`.
    pub(crate) fn renommer_la_variante(&mut self, sequence: u64, numero: u32, nom: &str) {
        // (Un nom ne change ni l'ordre ni le contenu des plans : mais il est écrit dans l'annexe du montage, donc le jeton.)
        let etape = format!("Renommer la version de séquence {}", lettre(numero));
        if self.variante_op(&etape, sequence, false, |d| d.renommer_variante(sequence, numero, nom)) {
            self.status = format!("Version de séquence renommée : {}.", self.doc.etiquette_de_variante(sequence, numero));
        }
    }

    /// Ouvre la comparaison du plan courant avec le plan de même rang d'une autre version de sa séquence.
    fn variante_comparer(&mut self, sequence: u64, numero: u32) {
        let index = self.cur_panel;
        self.comparer_ouvrir(index, Some(crate::comparer::Source::Variante(numero)), Some(crate::comparer::Source::Actuelle));
        let _ = sequence;
    }

    // ------------------------------------------------------------------ mettre à jour dans les autres versions de séquence

    /// Ouvre la fenêtre « Mettre à jour dans les autres versions de séquence » pour le plan `index` : elle liste les autres
    /// versions de séquence où ce plan apparaît. Les versions qui montrent déjà la version courante du plan sont grisées.
    pub(crate) fn mettre_a_jour_demander(&mut self, index: usize) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        if self.doc.storyboard_locked {
            self.status = crate::projetui::STORYBOARD_LOCKED.into();
            return;
        }
        let sequence = self.doc.sequence_of(index);
        if self.doc.variantes_de(sequence).is_none() {
            self.status = "Cette séquence n'a qu'une version : « Nouvelle version de la séquence » en crée une autre.".into();
            return;
        }
        let montrant = self.doc.variantes_montrant(sequence, plan);
        if montrant.is_empty() {
            self.status = "Ce plan n'est dans aucune autre version de la séquence.".into();
            return;
        }
        let courante = self.doc.numero_de_version(plan);
        let coches = montrant.into_iter().map(|(n, v)| (n, v != courante)).collect();
        self.variantes_ui.mettre_a_jour = Some(MiseAJour { sequence, plan, coches });
    }

    /// Les versions de séquence `cibles` montrent désormais la version courante du plan `plan` (une étape d'annulation).
    pub(crate) fn mettre_a_jour_dans_les_variantes(&mut self, sequence: u64, plan: u64, cibles: &[u32]) {
        let version = self.doc.numero_de_version(plan);
        let mut n = 0;
        let mut a_essaye = false;
        let nom = format!("Mettre à jour le plan dans {} version{} de séquence", cibles.len(), if cibles.len() > 1 { "s" } else { "" });
        if self.variante_op(&nom, sequence, false, |d| {
            a_essaye = true;
            n = d.mettre_a_jour_variantes(sequence, plan, cibles);
            n > 0
        }) {
            let lettres: Vec<String> = cibles.iter().map(|c| lettre(*c)).collect();
            self.status = format!(
                "Le plan montre maintenant sa version {} dans {n} autre{} version{} de séquence ({}). Ctrl+Z défait.",
                etiquette_version(version),
                if n > 1 { "s" } else { "" },
                if n > 1 { "s" } else { "" },
                lettres.join(", ")
            );
        } else if a_essaye {
            // (Pas de refus du verrou ni du jeton : leur message reste.)
            self.status = "Rien à mettre à jour : les versions de séquence choisies montrent déjà cette version du plan.".into();
        }
    }

    /// Les commandes `variante.*` ; vrai si `action` en était une.
    pub(crate) fn variante_action(&mut self, action: &str) -> bool {
        let Some(sequence) = self.sequence_courante() else { return action.starts_with("variante.") };
        match action {
            "variante.nouvelle" => self.nouvelle_variante_de_sequence(sequence, Nouvelle::Partager),
            "variante.copie" => self.nouvelle_variante_de_sequence(sequence, Nouvelle::Copier),
            "variante.vide" => self.nouvelle_variante_de_sequence(sequence, Nouvelle::Vide),
            "variante.precedente" => self.variante_voisine(-1),
            "variante.suivante" => self.variante_voisine(1),
            "variante.mettre_a_jour" => self.mettre_a_jour_demander(self.cur_panel),
            "variante.renommer" => {
                let numero = self.doc.numero_de_variante(sequence);
                if self.doc.variantes_de(sequence).is_none() {
                    self.status = "Cette séquence n'a qu'une version : « Nouvelle version de la séquence » en crée une autre.".into();
                } else {
                    let nom = self.doc.variantes_de(sequence).map_or(String::new(), |v| v.nom(numero).to_string());
                    self.variantes_ui.renommer = Some((sequence, numero, nom, true));
                }
            }
            "variante.supprimer" => {
                let numero = self.doc.numero_de_variante(sequence);
                self.supprimer_variante_demander(sequence, numero);
            }
            "variante.comparer" => match self.doc.variantes_de(sequence) {
                Some(v) => {
                    let courante = v.actuelle;
                    let autre = v.numeros().into_iter().filter(|n| *n != courante).max_by_key(|n| *n < courante).unwrap_or(courante);
                    self.variante_comparer(sequence, autre);
                }
                None => self.status = "Cette séquence n'a qu'une version : rien à comparer.".into(),
            },
            _ => return false,
        }
        true
    }

    // ------------------------------------------------------------------ le sélecteur et les menus

    /// Le petit sélecteur « A · B · C » en haut de la vue Vignettes, pour la séquence du plan courant. Discret : une séquence qui
    /// n'a qu'une version n'a que l'icône (à demi effacée) pour en créer une autre.
    pub(crate) fn variantes_barre(&mut self, ui: &mut egui::Ui) {
        let Some(sequence) = self.sequence_courante() else { return };
        let (actuelle, numeros) = match self.doc.variantes_de(sequence) {
            Some(v) => (v.actuelle, v.numeros()),
            None => (1, vec![1]),
        };
        let plusieurs = numeros.len() > 1;
        let memo = egui::Id::new("vignettes_variantes");
        let avant: Option<egui::Rect> = ui.data(|d| d.get_temp(memo));
        let touche = avant.is_some_and(|r| ui.ctx().pointer_hover_pos().is_some_and(|p| r.contains(p)));
        let titre = format!("Séquence {}", self.doc.sequence_label(sequence));
        let mut passer = None;
        let zone = ui
            .scope(|ui| {
                ui.set_opacity(if plusieurs || touche { 1.0 } else { 0.3 });
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.add_space(6.0);
                    if plusieurs {
                        ui.label(RichText::new(titre.clone()).small().color(theme::texte_doux()));
                        for &n in &numeros {
                            let nom = self.doc.etiquette_de_variante(sequence, n);
                            let courante = n == actuelle;
                            let description = if courante {
                                "La version de séquence du film : montage, lecture et exports la suivent. Clic droit : renommer, comparer, supprimer."
                            } else {
                                "Clic : la séquence passe à cette version de séquence (l'actuelle est gardée). Clic droit : renommer, comparer, supprimer."
                            };
                            let resp = bulles::etat(ui, courante, &lettre(n), &format!("Version de séquence {nom}"), description, "");
                            if resp.clicked() && !courante {
                                passer = Some(n);
                            }
                            resp.context_menu(|ui| self.variante_menu_une(ui, sequence, n));
                        }
                        let nom_courant = self.doc.variantes_de(sequence).map_or("", |v| v.nom(actuelle)).to_string();
                        if !nom_courant.is_empty() {
                            ui.label(RichText::new(nom_courant).small().color(theme::texte_doux()));
                        }
                    }
                    let r = ui.menu_button(RichText::new(ph::GIT_BRANCH).size(bulles::TAILLE), |ui| self.variantes_menu_contenu(ui, sequence));
                    bulles::bulle(
                        r.response,
                        "Versions de la séquence",
                        "Essayer un autre découpage de la séquence sans perdre le premier : les plans (et toutes leurs versions) sont communs, chaque version de séquence garde son choix de plans, leurs durées et la version de chacun.",
                        "",
                    );
                });
            })
            .response
            .rect;
        ui.data_mut(|d| d.insert_temp(memo, zone));
        if let Some(n) = passer {
            self.passer_a_la_variante(sequence, n);
        }
    }

    /// Le clic droit sur une version de séquence `numero` : la renommer, la comparer, la supprimer.
    fn variante_menu_une(&mut self, ui: &mut egui::Ui, sequence: u64, numero: u32) {
        let nom = self.doc.etiquette_de_variante(sequence, numero);
        ui.label(RichText::new(format!("Version de séquence {nom}")).weak().small());
        if ui.button("Renommer…").clicked() {
            ui.close_menu();
            let court = self.doc.variantes_de(sequence).map_or(String::new(), |v| v.nom(numero).to_string());
            self.variantes_ui.renommer = Some((sequence, numero, court, true));
        }
        let courante = self.doc.numero_de_variante(sequence) == numero;
        let resp = ui.add_enabled(!courante, egui::Button::new("Comparer à la version choisie"));
        if bulles::bulle(resp, "Comparer", "Le plan de même rang de cette version de séquence et celui du film, côte à côte ou superposés (lecture seule).", "").clicked() {
            ui.close_menu();
            self.variante_comparer(sequence, numero);
        }
        if ui.add_enabled(self.doc.nombre_de_variantes(sequence) > 1, egui::Button::new("Supprimer la version de séquence…")).clicked() {
            ui.close_menu();
            self.supprimer_variante_demander(sequence, numero);
        }
    }

    /// Le contenu du menu des versions de `sequence` (sélecteur de la vue Vignettes, barre de séquence du montage) : créer,
    /// passer à, renommer, comparer, mettre à jour, supprimer.
    pub(crate) fn variantes_menu_contenu(&mut self, ui: &mut egui::Ui, sequence: u64) {
        self.action_item(ui, "Nouvelle version, en gardant les plans", "variante.nouvelle");
        self.action_item(ui, "Nouvelle version, en copiant les plans", "variante.copie");
        self.action_item(ui, "Nouvelle version, en partant d'un plan vide", "variante.vide");
        let Some(v) = self.doc.variantes_de(sequence) else { return };
        let (actuelle, numeros) = (v.actuelle, v.numeros());
        ui.separator();
        for n in numeros {
            let nom = self.doc.etiquette_de_variante(sequence, n);
            let texte = if n == actuelle { format!("{nom}  (choisie)") } else { format!("Passer à la version de séquence {nom}") };
            if ui.add_enabled(n != actuelle, egui::Button::new(texte)).clicked() {
                ui.close_menu();
                self.passer_a_la_variante(sequence, n);
            }
        }
        ui.separator();
        self.action_item(ui, "Mettre à jour le plan dans les autres versions de séquence…", "variante.mettre_a_jour");
        self.action_item(ui, "Renommer la version de séquence…", "variante.renommer");
        self.action_item(ui, "Comparer le plan à une autre version de séquence…", "variante.comparer");
        self.action_item(ui, "Supprimer la version de séquence…", "variante.supprimer");
    }

    /// Le sous-menu « Versions de la séquence » (Storyboard, clic droit sur la barre de séquence).
    pub(crate) fn variantes_sous_menu(&mut self, ui: &mut egui::Ui) {
        let Some(sequence) = self.sequence_courante() else { return };
        let n = self.doc.nombre_de_variantes(sequence);
        let titre = if n > 1 { format!("Versions de la séquence ({})", self.doc.etiquette_de_variante(sequence, self.doc.numero_de_variante(sequence))) } else { "Versions de la séquence".to_string() };
        ui.menu_button(titre, |ui| self.variantes_menu_contenu(ui, sequence));
    }

    // ------------------------------------------------------------------ les fenêtres

    /// La confirmation de suppression, le renommage et la mise à jour, s'il y en a un en cours.
    pub(crate) fn variantes_fenetres(&mut self, ctx: &egui::Context) {
        if let Some((sequence, numero)) = self.variantes_ui.supprimer {
            let existe = self.doc.variantes_de(sequence).is_some_and(|v| v.numeros().contains(&numero));
            if !existe || self.doc.storyboard_locked {
                self.variantes_ui.supprimer = None;
            } else {
                let nom = self.doc.etiquette_de_variante(sequence, numero);
                let (plans, images) = self.resume_variante(sequence, numero);
                let (mut faire, mut fermer) = (false, false);
                egui::Window::new("Supprimer la version de séquence").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
                    ui.set_max_width(380.0);
                    ui.label(RichText::new(format!("Supprimer la version {nom} de la séquence {} ?", self.doc.sequence_label(sequence))).strong());
                    if plans == 0 {
                        ui.label("Aucun plan n'est supprimé : les autres versions de séquence montrent les mêmes. Seul son découpage (le choix des plans, leurs durées, la version de chacun) disparaît. Ctrl+Z la ramène.");
                    } else {
                        ui.label(format!(
                            "{plans} plan{} et {images} images seront supprimés (ceux que les autres versions de séquence ne montrent pas). Les autres versions restent telles quelles. Ctrl+Z les ramène.",
                            if plans > 1 { "s" } else { "" }
                        ));
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Supprimer").color(theme::erreur())).stroke(egui::Stroke::new(1.0_f32, theme::erreur()))).clicked() {
                            faire = true;
                        }
                        if ui.button("Annuler").clicked() {
                            fermer = true;
                        }
                    });
                });
                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    fermer = true;
                }
                if faire {
                    self.variantes_ui.supprimer = None;
                    self.supprimer_la_variante(sequence, numero);
                } else if fermer {
                    self.variantes_ui.supprimer = None;
                }
            }
        }
        if let Some((sequence, numero, mut texte, mut focus)) = self.variantes_ui.renommer.take() {
            if self.doc.variantes_de(sequence).is_some_and(|v| v.numeros().contains(&numero)) {
                let (mut ok, mut annuler) = (false, false);
                let titre = format!("Renommer la version de séquence {}", lettre(numero));
                egui::Window::new(titre).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
                    ui.label("Un nom court, après la lettre (« serré », « avec la poursuite »…). Vide : la lettre seule.");
                    let champ = ui.add(egui::TextEdit::singleline(&mut texte).char_limit(crate::variantes::MAX_NOM).desired_width(260.0));
                    if focus {
                        champ.request_focus();
                        focus = false;
                    }
                    if champ.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        ok = true;
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Renommer").clicked() {
                            ok = true;
                        }
                        if ui.button("Annuler").clicked() {
                            annuler = true;
                        }
                    });
                });
                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    annuler = true;
                }
                if ok {
                    self.renommer_la_variante(sequence, numero, &texte);
                } else if !annuler {
                    self.variantes_ui.renommer = Some((sequence, numero, texte, focus));
                }
            }
        }
        self.mettre_a_jour_fenetre(ctx);
    }

    /// La fenêtre « Mettre à jour dans les autres versions de séquence » : une case par version de séquence où le plan apparaît.
    fn mettre_a_jour_fenetre(&mut self, ctx: &egui::Context) {
        let Some(mut m) = self.variantes_ui.mettre_a_jour.take() else { return };
        // Le plan, les versions de séquence ou le verrou ont pu changer depuis l'ouverture : elle se ferme.
        let montrant = self.doc.variantes_montrant(m.sequence, m.plan);
        if self.doc.storyboard_locked || montrant.is_empty() || !self.doc.panels.iter().any(|p| p.id == m.plan) {
            return;
        }
        m.coches.retain(|(n, _)| montrant.iter().any(|(k, _)| k == n));
        let courante = self.doc.numero_de_version(m.plan);
        let index = self.doc.panels.iter().position(|p| p.id == m.plan).unwrap_or(0);
        let titre_plan = crate::storyboard::panel_title(&self.doc, index);
        let (mut faire, mut annuler) = (false, false);
        egui::Window::new("Mettre à jour dans les autres versions de séquence").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.set_max_width(400.0);
            ui.label(RichText::new(format!("{titre_plan} : faire montrer sa version {} par…", etiquette_version(courante))).strong());
            ui.label(RichText::new("Seules les versions cochées changent. Leur durée, leur ordre et leurs autres plans ne bougent pas ; l'ancienne version du plan reste dans le plan.").color(theme::texte_doux()));
            ui.add_space(4.0);
            for (numero, coche) in &mut m.coches {
                let (_, version) = montrant.iter().find(|(k, _)| k == numero).copied().unwrap_or((*numero, courante));
                let deja = version == courante;
                let nom = self.doc.etiquette_de_variante(m.sequence, *numero);
                let texte = if deja { format!("{nom} — montre déjà {}", etiquette_version(version)) } else { format!("{nom} — montre {}", etiquette_version(version)) };
                ui.add_enabled_ui(!deja, |ui| {
                    ui.checkbox(coche, texte);
                });
            }
            ui.add_space(6.0);
            let n = m.coches.iter().filter(|(k, c)| *c && montrant.iter().any(|(q, v)| q == k && *v != courante)).count();
            ui.horizontal(|ui| {
                if ui.add_enabled(n > 0, egui::Button::new("Mettre à jour")).clicked() {
                    faire = true;
                }
                if ui.button("Annuler").clicked() {
                    annuler = true;
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            annuler = true;
        }
        if faire {
            let cibles: Vec<u32> = m.coches.iter().filter(|(k, c)| *c && montrant.iter().any(|(q, v)| q == k && *v != courante)).map(|(k, _)| *k).collect();
            self.mettre_a_jour_dans_les_variantes(m.sequence, m.plan, &cibles);
        } else if !annuler {
            self.variantes_ui.mettre_a_jour = Some(m);
        }
    }

    /// Combien de plans et d'images supprimerait la suppression de la version de séquence `numero` : seulement les plans que
    /// ni une autre version de séquence ni le film ne montrent.
    pub(crate) fn resume_variante(&self, sequence: u64, numero: u32) -> (usize, usize) {
        let exclusifs = self.doc.plans_exclusifs_de_variante(sequence, numero);
        (exclusifs.len(), exclusifs.iter().map(|(_, images)| *images).sum())
    }
}

/// « V2 » : le nom d'une version de plan (celui de `planvers::etiquette`).
fn etiquette_version(version: u32) -> String {
    crate::planvers::etiquette(version)
}

/// « Séquence 2 · B » : le nom d'une séquence sur la barre du montage quand elle a plusieurs versions.
pub(crate) fn suffixe_barre(doc: &Document, sequence: u64) -> String {
    if doc.nombre_de_variantes(sequence) > 1 {
        format!(" · {}", etiquette(doc.numero_de_variante(sequence), doc.variantes_de(sequence).map_or("", |v| v.nom(v.actuelle))))
    } else {
        String::new()
    }
}
