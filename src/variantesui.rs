//! Les variantes de séquence dans l'interface (`variantes.rs` pour le modèle et le fichier) : le petit sélecteur
//! « A · B · C » en haut de la vue Vignettes, l'entrée « Variantes de la séquence » du menu Storyboard et du clic droit
//! sur la barre de séquence du montage, les commandes `variante.*` (Ctrl+K), la confirmation de suppression et le
//! renommage.
//!
//! **Un geste = une étape d'annulation, nommée** (panneau Historique) : « Nouvelle variante de la séquence », « Passer à la
//! variante B », « Supprimer la variante B », « Renommer la variante B ». Aucune ne touche un pixel sur le fil de
//! l'interface : changer de variante échange des listes de plans (les tuiles sont partagées).
//!
//! Le storyboard verrouillé refuse tout cela. Dans un film partagé il faut le jeton du montage, et chaque plan que
//! l'échange fait sortir du film doit être à moi ou libre (comme pour supprimer des plans, `partage_peut_toucher`).

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::document::Document;
use crate::theme;
use crate::variantes::{etiquette, lettre};

/// Les fenêtres des variantes (un champ dans `App`).
#[derive(Default)]
pub(crate) struct VariantesUi {
    /// La variante dont on confirme la suppression : (séquence, numéro).
    pub supprimer: Option<(u64, u32)>,
    /// La variante qu'on renomme : (séquence, numéro, nom en cours de frappe, mettre le curseur dans le champ).
    pub renommer: Option<(u64, u32, String, bool)>,
}

impl App {
    /// La séquence du plan courant.
    fn sequence_courante(&self) -> Option<u64> {
        self.doc.panels.get(self.cur_panel)?;
        Some(self.doc.sequence_of(self.cur_panel))
    }

    /// Une opération sur les variantes de `sequence`, avec une étape d'annulation nommée `nom`. `sort` : l'opération fait
    /// sortir des plans du film (dans un film partagé, ils doivent être à moi ou libres). `op` rend faux si rien n'a changé.
    fn variante_op(&mut self, nom: &str, sequence: u64, sort: bool, op: impl FnOnce(&mut Document) -> bool) -> bool {
        // Verrouillé : rien ne change ; film partagé : il faut le jeton du montage.
        if self.storyboard_refuses() {
            return false;
        }
        if self.doc.variantes_partielles {
            self.status = "Des dessins de variantes manquent (fichiers pas encore arrivés dans le dossier partagé, ou illisibles) : les variantes ne changent pas tant qu'ils ne sont pas là.".into();
            return false;
        }
        if sort {
            let ids = self.doc.plans_de_sequence(sequence);
            if !self.partage_peut_toucher(&ids) {
                return false;
            }
        }
        self.end_stroke();
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

    /// « Nouvelle variante » de la séquence : le jeu de plans en cours est gardé, et on continue sur une copie (`vide` faux)
    /// ou sur un plan vide.
    pub(crate) fn nouvelle_variante_de_sequence(&mut self, sequence: u64, vide: bool) {
        let ancienne = self.doc.numero_de_variante(sequence);
        let now = crate::recovery::now_secs();
        let mut neuve = 0;
        let nom = if vide { "Nouvelle variante de la séquence, vide" } else { "Nouvelle variante de la séquence" };
        if self.variante_op(nom, sequence, vide, |d| match d.nouvelle_variante(sequence, vide, now) {
            Some(n) => {
                neuve = n;
                true
            }
            None => false,
        }) {
            let seq = self.doc.sequence_label(sequence);
            self.status = format!(
                "Nouvelle variante de la séquence {seq} : {}{}. La {} est gardée (« Passer à la variante » la ramène).",
                lettre(neuve),
                if vide { ", à partir d'un plan vide" } else { ", une copie de la précédente" },
                lettre(ancienne)
            );
        } else if !self.doc.storyboard_locked && self.doc.nombre_de_variantes(sequence) >= crate::variantes::MAX_VARIANTES {
            self.status = format!("Nouvelle variante impossible : la séquence a déjà {} variantes.", crate::variantes::MAX_VARIANTES);
        }
    }

    /// Passe la séquence à sa variante rangée `numero`.
    pub(crate) fn passer_a_la_variante(&mut self, sequence: u64, numero: u32) {
        let ancienne = self.doc.numero_de_variante(sequence);
        let avant = self.doc.plans_de_sequence(sequence).len();
        let nom = format!("Passer à la variante {}", lettre(numero));
        if self.variante_op(&nom, sequence, true, |d| d.changer_de_variante(sequence, numero)) {
            let apres = self.doc.plans_de_sequence(sequence).len();
            let seq = self.doc.sequence_label(sequence);
            let nom_neuve = self.doc.etiquette_de_variante(sequence, numero);
            self.status = format!(
                "La séquence {seq} est maintenant en variante {nom_neuve} ({} plan{}, avant : {avant}) ; la {} est gardée.",
                apres,
                if apres > 1 { "s" } else { "" },
                lettre(ancienne)
            );
        }
    }

    /// Passe à la variante avant / après la variante choisie (en tournant).
    fn variante_voisine(&mut self, sens: i32) {
        let Some(sequence) = self.sequence_courante() else { return };
        let Some(v) = self.doc.variantes_de(sequence) else {
            self.status = "Cette séquence n'a qu'une variante : « Nouvelle variante de la séquence » en crée une autre.".into();
            return;
        };
        let numeros = v.numeros();
        let k = numeros.iter().position(|n| *n == v.actuelle).unwrap_or(0) as i32;
        let cible = numeros[(k + sens).rem_euclid(numeros.len() as i32) as usize];
        self.passer_a_la_variante(sequence, cible);
    }

    /// Demande confirmation avant de supprimer la variante `numero` (la dernière ne se supprime pas).
    pub(crate) fn supprimer_variante_demander(&mut self, sequence: u64, numero: u32) {
        if self.doc.storyboard_locked {
            self.status = crate::projetui::STORYBOARD_LOCKED.into();
            return;
        }
        if self.doc.nombre_de_variantes(sequence) < 2 {
            self.status = "La dernière variante d'une séquence ne se supprime pas.".into();
            return;
        }
        self.variantes_ui.supprimer = Some((sequence, numero));
    }

    /// Supprime la variante `numero` (une étape d'annulation la ramène).
    pub(crate) fn supprimer_la_variante(&mut self, sequence: u64, numero: u32) {
        let sort = self.doc.numero_de_variante(sequence) == numero;
        let nom = format!("Supprimer la variante {}", lettre(numero));
        let label = self.doc.etiquette_de_variante(sequence, numero);
        let mut reste = 0;
        if self.variante_op(&nom, sequence, sort, |d| match d.supprimer_variante(sequence, numero) {
            Some(r) => {
                reste = r;
                true
            }
            None => false,
        }) {
            self.status = format!("Variante {label} supprimée ; la séquence est en variante {} (Ctrl+Z la ramène).", lettre(reste));
        }
    }

    /// Donne le nom court `nom` à la variante `numero`.
    pub(crate) fn renommer_la_variante(&mut self, sequence: u64, numero: u32, nom: &str) {
        // (Un nom ne change ni l'ordre ni le contenu des plans : mais il est écrit dans l'annexe du montage, donc le jeton.)
        let etape = format!("Renommer la variante {}", lettre(numero));
        if self.variante_op(&etape, sequence, false, |d| d.renommer_variante(sequence, numero, nom)) {
            self.status = format!("Variante renommée : {}.", self.doc.etiquette_de_variante(sequence, numero));
        }
    }

    /// Ouvre la comparaison du plan courant avec le plan de même rang d'une autre variante de sa séquence.
    fn variante_comparer(&mut self, sequence: u64, numero: u32) {
        let index = self.cur_panel;
        self.comparer_ouvrir(index, Some(crate::comparer::Source::Variante(numero)), Some(crate::comparer::Source::Actuelle));
        let _ = sequence;
    }

    /// Les commandes `variante.*` ; vrai si `action` en était une.
    pub(crate) fn variante_action(&mut self, action: &str) -> bool {
        let Some(sequence) = self.sequence_courante() else { return action.starts_with("variante.") };
        match action {
            "variante.nouvelle" => self.nouvelle_variante_de_sequence(sequence, false),
            "variante.vide" => self.nouvelle_variante_de_sequence(sequence, true),
            "variante.precedente" => self.variante_voisine(-1),
            "variante.suivante" => self.variante_voisine(1),
            "variante.renommer" => {
                let numero = self.doc.numero_de_variante(sequence);
                if self.doc.variantes_de(sequence).is_none() {
                    self.status = "Cette séquence n'a qu'une variante : « Nouvelle variante de la séquence » en crée une autre.".into();
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
                None => self.status = "Cette séquence n'a qu'une variante : rien à comparer.".into(),
            },
            _ => return false,
        }
        true
    }

    // ------------------------------------------------------------------ le sélecteur et les menus

    /// Le petit sélecteur « A · B · C » en haut de la vue Vignettes, pour la séquence du plan courant. Discret : une
    /// séquence qui n'a qu'une variante n'a que l'icône (à demi effacée) pour en créer une autre.
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
                            let description = if courante { "La variante du film : montage, lecture et exports la suivent. Clic droit : renommer, comparer, supprimer." } else { "Clic : la séquence passe à cette variante (l'actuelle est gardée). Clic droit : renommer, comparer, supprimer." };
                            let resp = bulles::etat(ui, courante, &lettre(n), &format!("Variante {nom}"), description, "");
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
                    bulles::bulle(r.response, "Variantes de la séquence", "Essayer un autre découpage de la séquence sans perdre le premier : la variante choisie est celle du film, les autres sont gardées.", "");
                });
            })
            .response
            .rect;
        ui.data_mut(|d| d.insert_temp(memo, zone));
        if let Some(n) = passer {
            self.passer_a_la_variante(sequence, n);
        }
    }

    /// Le clic droit sur une variante `numero` : la renommer, la comparer, la supprimer.
    fn variante_menu_une(&mut self, ui: &mut egui::Ui, sequence: u64, numero: u32) {
        let nom = self.doc.etiquette_de_variante(sequence, numero);
        ui.label(RichText::new(format!("Variante {nom}")).weak().small());
        if ui.button("Renommer…").clicked() {
            ui.close_menu();
            let court = self.doc.variantes_de(sequence).map_or(String::new(), |v| v.nom(numero).to_string());
            self.variantes_ui.renommer = Some((sequence, numero, court, true));
        }
        let courante = self.doc.numero_de_variante(sequence) == numero;
        let resp = ui.add_enabled(!courante, egui::Button::new("Comparer à la variante choisie"));
        if bulles::bulle(resp, "Comparer", "Le plan de même rang de cette variante et celui du film, côte à côte ou superposés (lecture seule).", "").clicked() {
            ui.close_menu();
            self.variante_comparer(sequence, numero);
        }
        if ui.add_enabled(self.doc.nombre_de_variantes(sequence) > 1, egui::Button::new("Supprimer la variante…")).clicked() {
            ui.close_menu();
            self.supprimer_variante_demander(sequence, numero);
        }
    }

    /// Le contenu du menu des variantes de `sequence` (sélecteur de la vue Vignettes, barre de séquence du montage) :
    /// créer, passer à, renommer, comparer, supprimer.
    pub(crate) fn variantes_menu_contenu(&mut self, ui: &mut egui::Ui, sequence: u64) {
        self.action_item(ui, "Nouvelle variante, en dupliquant", "variante.nouvelle");
        self.action_item(ui, "Nouvelle variante, en partant d'un plan vide", "variante.vide");
        let Some(v) = self.doc.variantes_de(sequence) else { return };
        let (actuelle, numeros) = (v.actuelle, v.numeros());
        ui.separator();
        for n in numeros {
            let nom = self.doc.etiquette_de_variante(sequence, n);
            let texte = if n == actuelle { format!("{nom}  (choisie)") } else { format!("Passer à la variante {nom}") };
            if ui.add_enabled(n != actuelle, egui::Button::new(texte)).clicked() {
                ui.close_menu();
                self.passer_a_la_variante(sequence, n);
            }
        }
        ui.separator();
        self.action_item(ui, "Renommer la variante…", "variante.renommer");
        self.action_item(ui, "Comparer le plan à une autre variante…", "variante.comparer");
        self.action_item(ui, "Supprimer la variante…", "variante.supprimer");
    }

    /// Le sous-menu « Variantes de la séquence » (Storyboard, clic droit sur la barre de séquence).
    pub(crate) fn variantes_sous_menu(&mut self, ui: &mut egui::Ui) {
        let Some(sequence) = self.sequence_courante() else { return };
        let n = self.doc.nombre_de_variantes(sequence);
        let titre = if n > 1 { format!("Variantes de la séquence ({})", self.doc.etiquette_de_variante(sequence, self.doc.numero_de_variante(sequence))) } else { "Variantes de la séquence".to_string() };
        ui.menu_button(titre, |ui| self.variantes_menu_contenu(ui, sequence));
    }

    // ------------------------------------------------------------------ les fenêtres

    /// La confirmation de suppression et le renommage, s'il y en a un en cours.
    pub(crate) fn variantes_fenetres(&mut self, ctx: &egui::Context) {
        if let Some((sequence, numero)) = self.variantes_ui.supprimer {
            let existe = self.doc.variantes_de(sequence).is_some_and(|v| v.numeros().contains(&numero));
            if !existe || self.doc.storyboard_locked {
                self.variantes_ui.supprimer = None;
            } else {
                let nom = self.doc.etiquette_de_variante(sequence, numero);
                let (plans, images) = self.resume_variante(sequence, numero);
                let (mut faire, mut fermer) = (false, false);
                egui::Window::new("Supprimer la variante").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
                    ui.set_max_width(380.0);
                    ui.label(RichText::new(format!("Supprimer la variante {nom} de la séquence {} ?", self.doc.sequence_label(sequence))).strong());
                    ui.label(format!("{plans} plan{} et {images} images seront supprimés. Les autres variantes restent telles quelles. Ctrl+Z les ramène.", if plans > 1 { "s" } else { "" }));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Supprimer").color(theme::erreur())).clicked() {
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
                let titre = format!("Renommer la variante {}", lettre(numero));
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
    }

    /// Combien de plans et d'images a la variante `numero` de la séquence (choisie ou rangée).
    pub(crate) fn resume_variante(&self, sequence: u64, numero: u32) -> (usize, usize) {
        let images = |ps: &[crate::document::Panel]| ps.iter().map(crate::document::Panel::frame_count).sum::<usize>();
        if self.doc.numero_de_variante(sequence) == numero {
            return match self.doc.course_de_sequence(sequence) {
                Some((a, b)) => (b - a, images(&self.doc.panels[a..b])),
                None => (0, 0),
            };
        }
        self.doc.variantes_de(sequence).and_then(|v| v.rangee(numero)).map_or((0, 0), |r| (r.contenu.panels.len(), images(&r.contenu.panels)))
    }
}

/// « Séquence 2 · B » : le nom d'une séquence sur la barre du montage quand elle a plusieurs variantes.
pub(crate) fn suffixe_barre(doc: &Document, sequence: u64) -> String {
    if doc.nombre_de_variantes(sequence) > 1 {
        format!(" · {}", etiquette(doc.numero_de_variante(sequence), doc.variantes_de(sequence).map_or("", |v| v.nom(v.actuelle))))
    } else {
        String::new()
    }
}
