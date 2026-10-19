//! Palette de commandes (Anthony, 2026-10-18, axe d'ergonomie 2) : Ctrl+K ouvre un champ ; on tape
//! quelques lettres du nom d'une commande (« persp », « retourner », « flou »), la palette propose
//! les commandes de Pelure avec leur raccourci. Flèches : choisir ; Entrée ou clic : lancer ;
//! Échap : fermer. Contre les « je ne trouve pas » : toutes les commandes faites du fichier de
//! raccourcis y sont.

use eframe::egui::{self, Align2, RichText};

use crate::app::App;

/// Un texte sans majuscules ni accents, pour comparer.
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'œ' => 'o',
            c => c,
        })
        .collect()
}

/// Les commandes (`id`, nom) qui répondent à `query`, les meilleures d'abord : chaque mot tapé se
/// trouve dans le nom ou l'identifiant. Devant : la commande elle-même (« persp » →
/// `outil.perspective`), puis un nom dont un mot commence par ce qu'on a tapé, le plus tôt dans le
/// nom, puis les noms courts.
pub fn rank(commands: &[(&str, &str)], query: &str) -> Vec<usize> {
    let words: Vec<String> = fold(query).split_whitespace().map(str::to_string).collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(usize, (usize, usize, usize))> = commands
        .iter()
        .enumerate()
        .filter_map(|(i, (id, name))| {
            let (n, id) = (fold(name), fold(id));
            if !words.iter().all(|w| n.contains(w.as_str()) || id.contains(w.as_str())) {
                return None;
            }
            let last = id.rsplit('.').next().unwrap_or("");
            let parts: Vec<&str> = n.split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty()).collect();
            let score: usize = words
                .iter()
                .map(|w| if last.starts_with(w.as_str()) { 0 } else if parts.iter().any(|p| p.starts_with(w.as_str())) { 1 } else { 2 })
                .sum();
            let place = parts.iter().position(|p| p.starts_with(words[0].as_str())).unwrap_or(usize::MAX);
            Some((i, (score, place, name.chars().count())))
        })
        .collect();
    hits.sort_by_key(|&(_, key)| key);
    hits.into_iter().map(|(i, _)| i).collect()
}

/// Les commandes que la palette propose : toutes celles qui existent, sauf les doubles
/// (`keymap::ALIASES`), qui feraient deux fois le même nom dans la liste.
pub fn palette_commands() -> Vec<(&'static str, &'static str)> {
    crate::keymap::ACTIONS
        .iter()
        .filter(|a| a.implemented && !crate::keymap::is_alias(a.id))
        .map(|a| (a.id, a.label))
        .collect()
}

/// La palette ouverte : ce qui est tapé, et la ligne choisie.
#[derive(Default)]
pub(crate) struct Palette {
    pub query: String,
    pub selected: usize,
}

impl App {
    /// La palette de commandes, si elle est ouverte.
    pub(crate) fn palette_window(&mut self, ctx: &egui::Context) {
        let Some(mut p) = self.palette.take() else { return };
        let commands = palette_commands();
        let found = rank(&commands, &p.query);
        let shown = found.len().min(14);
        let (up, down, enter, escape) = ctx.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        if down && shown > 0 {
            p.selected = (p.selected + 1).min(shown - 1);
        }
        if up {
            p.selected = p.selected.saturating_sub(1);
        }
        let mut run: Option<&str> = None;
        let mut keep = !escape;
        egui::Window::new("Palette de commandes")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_TOP, [0.0, 70.0])
            .show(ctx, |ui| {
                ui.set_width(520.0);
                let resp = ui.add(egui::TextEdit::singleline(&mut p.query).hint_text("Une commande : persp, retourner, flou…").desired_width(f32::INFINITY));
                if !resp.has_focus() {
                    resp.request_focus();
                }
                if resp.changed() {
                    p.selected = 0;
                }
                if p.query.trim().is_empty() {
                    ui.label(RichText::new("Tape quelques lettres du nom d'une commande.").small().weak());
                } else if found.is_empty() {
                    ui.label(RichText::new("Aucune commande ne correspond.").small().weak());
                }
                for (row, &i) in found.iter().take(shown).enumerate() {
                    let (id, name) = commands[i];
                    // Toutes les touches actives de la commande (celles du jeu choisi et les siennes
                    // comprises), et d'où elles viennent quand ce n'est pas SHOT.
                    let key = self.keymap.keys_text(id);
                    let origin = self.keymap.origin_of(id);
                    let r = ui
                        .horizontal(|ui| {
                            let r = ui.add(egui::SelectableLabel::new(row == p.selected, name));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(id).small().weak());
                                if !key.is_empty() {
                                    ui.label(RichText::new(key).small().strong());
                                    if origin != crate::keymap::Origin::Default {
                                        ui.label(RichText::new(origin.label()).small().weak());
                                    }
                                }
                            });
                            r
                        })
                        .inner;
                    if r.clicked() {
                        run = Some(id);
                    }
                }
                if found.len() > shown {
                    ui.label(RichText::new(format!("… et {} autres : précise ta recherche.", found.len() - shown)).small().weak());
                }
            });
        if enter && run.is_none() {
            run = found.get(p.selected).map(|&i| commands[i].0);
        }
        if let Some(id) = run {
            keep = false;
            self.run_action(id);
        }
        if keep {
            self.palette = Some(p);
        }
    }
}
