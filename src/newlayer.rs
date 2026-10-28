//! Fenêtre « Nouveau calque » (13b) : nom, sorte, groupe de couleur, mode de fusion,
//! pré / post-comportement (13c), et place dans la pile (sur / sous le calque courant, tout en
//! haut / en bas, sur / sous le groupe du calque courant).

use eframe::egui::{self, Align2, RichText};

use crate::app::App;
use crate::blend::BlendMode;
use crate::document::{Behaviour, Layer, LayerKind};

/// Où poser le nouveau calque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    AboveCurrent,
    BelowCurrent,
    Top,
    Bottom,
    AboveGroup,
    BelowGroup,
}

impl Place {
    pub(crate) const ALL: [Place; 6] =
        [Place::AboveCurrent, Place::BelowCurrent, Place::Top, Place::Bottom, Place::AboveGroup, Place::BelowGroup];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Place::AboveCurrent => "Sur le calque courant",
            Place::BelowCurrent => "Sous le calque courant",
            Place::Top => "Tout au-dessus",
            Place::Bottom => "Tout en dessous",
            Place::AboveGroup => "Sur le groupe du calque courant",
            Place::BelowGroup => "Sous le groupe du calque courant",
        }
    }
}

/// Rang où va le nouveau calque (les calques à partir de ce rang montent d'un cran).
pub(crate) fn insert_index(layers: &[Layer], cur: usize, place: Place) -> usize {
    let n = layers.len();
    let cur = cur.min(n.saturating_sub(1));
    let group = layers.get(cur).map_or(0, |l| l.group);
    let in_group = || layers.iter().enumerate().filter(move |(_, l)| l.group == group).map(|(i, _)| i);
    match place {
        Place::AboveCurrent => (cur + 1).min(n),
        Place::BelowCurrent => cur,
        Place::Top => n,
        Place::Bottom => 0,
        Place::AboveGroup => in_group().max().map_or(n, |i| i + 1),
        Place::BelowGroup => in_group().min().unwrap_or(0),
    }
}

/// Ce qu'on règle dans la fenêtre.
pub(crate) struct NewLayerForm {
    pub name: String,
    pub kind: LayerKind,
    pub group: u8,
    /// Le groupe du calque courant, quel que soit `group`.
    pub same_group: bool,
    pub blend: BlendMode,
    pub place: Place,
    /// Pré / post-comportement (13c).
    pub pre: Behaviour,
    pub post: Behaviour,
    focused: bool,
}

impl App {
    /// Ouvre la fenêtre « Nouveau calque ».
    pub(crate) fn open_new_layer_dialog(&mut self) {
        let n = self.layers().len() + 1;
        self.new_layer_dialog = Some(NewLayerForm {
            name: format!("Calque {n}"),
            kind: LayerKind::Bitmap,
            group: 0,
            same_group: false,
            blend: BlendMode::Normal,
            place: Place::AboveCurrent,
            pre: Behaviour::Nothing,
            post: Behaviour::Nothing,
            focused: false,
        });
    }

    /// Crée le calque réglé dans la fenêtre (une étape d'annulation).
    fn create_layer_from(&mut self, f: &NewLayerForm) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.drop_bitmap_selection();
        let snap = self.snapshot();
        let name = match f.name.trim() {
            "" => format!("Calque {}", self.layers().len() + 1),
            s => s.to_string(),
        };
        let mut layer = Layer::new(name.clone(), f.kind);
        layer.blend = f.blend;
        layer.pre = f.pre;
        layer.post = f.post;
        layer.group = if f.same_group { self.layer().group } else { f.group };
        let at = insert_index(self.layers(), self.cur_layer, f.place);
        self.layers_mut().insert(at, layer);
        self.cur_layer = at;
        self.layer_sel.clear();
        self.vsel = None;
        self.commit_structure(snap);
        self.status = format!("Calque « {name} » créé.");
    }

    /// La fenêtre « Nouveau calque », si elle est ouverte.
    pub(crate) fn new_layer_window(&mut self, ctx: &egui::Context) {
        let Some(mut f) = self.new_layer_dialog.take() else { return };
        let mut keep = true;
        let mut create = false;
        let groups = self.doc.groups.clone();
        let current_group = groups.get(self.layer().group as usize).map(|g| g.name.clone()).unwrap_or_default();
        egui::Window::new("Nouveau calque")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Grid::new("grille_nouveau_calque").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("Nom");
                    let mut out = egui::TextEdit::singleline(&mut f.name).desired_width(220.0).show(ui);
                    let resp = out.response.clone();
                    if !f.focused {
                        // Tout le nom proposé choisi : on tape le sien directement.
                        resp.request_focus();
                        let n = f.name.chars().count();
                        out.state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(0),
                            egui::text::CCursor::new(n),
                        )));
                        out.state.store(ui.ctx(), resp.id);
                        f.focused = true;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        create = true;
                    }
                    ui.end_row();

                    ui.label("Sorte");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut f.kind, LayerKind::Bitmap, "Bitmap")
                            .on_hover_text("De la peinture en pixels, pinceaux texturés.");
                        ui.radio_value(&mut f.kind, LayerKind::Vector, "Vectoriel")
                            .on_hover_text("Des traits qui restent modifiables.");
                    });
                    ui.end_row();

                    ui.label("Groupe");
                    ui.vertical(|ui| {
                        ui.add_enabled_ui(!f.same_group, |ui| {
                            let now = groups.get(f.group as usize).map(|g| g.name.clone()).unwrap_or_default();
                            egui::ComboBox::from_id_salt("groupe_nouveau_calque").selected_text(now).width(200.0).show_ui(ui, |ui| {
                                for (g, group) in groups.iter().enumerate() {
                                    let text = RichText::new(format!("{}  {}", egui_phosphor::regular::SQUARE, group.name))
                                        .color(crate::layerheads::group_color(group.color));
                                    ui.selectable_value(&mut f.group, g as u8, text);
                                }
                            });
                        });
                        ui.checkbox(&mut f.same_group, format!("Groupe du calque courant ({current_group})"));
                    });
                    ui.end_row();

                    ui.label("Mode");
                    egui::ComboBox::from_id_salt("mode_nouveau_calque").selected_text(f.blend.label()).width(200.0).show_ui(ui, |ui| {
                        for m in BlendMode::ALL {
                            if BlendMode::FAMILY_STARTS.contains(&m) {
                                ui.separator();
                            }
                            ui.selectable_value(&mut f.blend, m, m.label()).on_hover_text(m.hint());
                        }
                    });
                    ui.end_row();

                    for (label, value, salt) in [("Avant la 1re image", &mut f.pre, "avant_nouveau_calque"), ("Après la dernière", &mut f.post, "apres_nouveau_calque")] {
                        ui.label(label);
                        egui::ComboBox::from_id_salt(salt).selected_text(value.label()).width(200.0).show_ui(ui, |ui| {
                            for b in Behaviour::ALL {
                                ui.selectable_value(value, b, b.label());
                            }
                        });
                        ui.end_row();
                    }

                    ui.label("Place");
                    egui::ComboBox::from_id_salt("place_nouveau_calque").selected_text(f.place.label()).width(200.0).show_ui(ui, |ui| {
                        for p in Place::ALL {
                            ui.selectable_value(&mut f.place, p, p.label());
                        }
                    });
                    ui.end_row();
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Créer").clicked() {
                        create = true;
                    }
                    if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        keep = false;
                    }
                });
            });
        if create {
            self.create_layer_from(&f);
        } else if keep {
            self.new_layer_dialog = Some(f);
        }
    }
}
