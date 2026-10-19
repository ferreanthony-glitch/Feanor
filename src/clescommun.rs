//! Ce que les trois sortes de clés ont en commun (passe d'interface, 2026-10-19) : les clés de
//! dessins (montage, `montagekeys.rs`), les clés de caméra (`camkeys.rs`) et les clés d'effets
//! (`fxkeys.rs`) se choisissent, se copient, se collent et se suppriment de la même façon.
//!
//! - **Le choix au clic** (`choose_group`) : un clic choisit la clé seule ; Ctrl l'ajoute ou la
//!   retire ; Maj prend toutes celles d'entre l'ancre et elle. Les trois `choose` y passent.
//! - **Le menu du clic droit** (`core_entries`, `App::key_menu_core`) : les mêmes cinq commandes,
//!   dans le même ordre, avec les mêmes noms (au nom de la clé près) et le même raccourci : Copier,
//!   Coller, Coller à la tête de lecture, Supprimer, Choisir toutes les clés. Chaque menu garde
//!   ensuite ce qui est à lui (poser une clé, dupliquer, le panneau Effets…).
//!
//! Les différences qui restent sont voulues ou à décider : voir docs/INTERFACE.md, partie 4.

use eframe::egui::{self, Ui};

use crate::app::App;

/// Les trois sortes de clés.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyKind {
    /// Début d'un dessin sur la piste d'un calque (montage).
    Drawing,
    /// Losange jaune de la piste Caméra.
    Camera,
    /// Losange violet de la piste d'un calque (effet choisi).
    Effect,
}

impl KeyKind {
    #[cfg(test)]
    pub(crate) const ALL: [KeyKind; 3] = [KeyKind::Drawing, KeyKind::Camera, KeyKind::Effect];

    /// « clé », « clé de caméra », « clé d'effet » (`many` : au pluriel).
    pub(crate) fn noun(self, many: bool) -> &'static str {
        match (self, many) {
            (KeyKind::Drawing, false) => "clé",
            (KeyKind::Drawing, true) => "clés",
            (KeyKind::Camera, false) => "clé de caméra",
            (KeyKind::Camera, true) => "clés de caméra",
            (KeyKind::Effect, false) => "clé d'effet",
            (KeyKind::Effect, true) => "clés d'effet",
        }
    }
}

/// Les commandes que le menu du clic droit offre sur une clé, quelle que soit sa sorte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyCmd {
    Copy,
    Paste,
    PasteAtHead,
    Delete,
    ChooseAll,
}

/// L'ordre du menu, le même pour les trois sortes.
pub(crate) const CORE: [KeyCmd; 5] = [KeyCmd::Copy, KeyCmd::Paste, KeyCmd::PasteAtHead, KeyCmd::Delete, KeyCmd::ChooseAll];

impl KeyCmd {
    /// L'action dont le raccourci est montré dans le menu (et qui la lance au clavier).
    pub(crate) fn action(self) -> &'static str {
        match self {
            KeyCmd::Copy => "edition.copier",
            KeyCmd::Paste => "edition.coller",
            KeyCmd::PasteAtHead => "edition.coller_a_la_tete",
            KeyCmd::Delete => "edition.supprimer",
            KeyCmd::ChooseAll => "edition.tout_selectionner",
        }
    }
}

/// Ce que le menu sait de la situation : combien de clés sont visées, s'il y a quelque chose à
/// coller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MenuState {
    /// Les clés visées (celles qui sont choisies ; au moins une pour le nom au singulier).
    pub n: usize,
    /// Copier et supprimer ont quelque chose à viser.
    pub has_target: bool,
    /// Coller a quelque chose à coller.
    pub can_paste: bool,
}

/// Une entrée du menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MenuEntry {
    pub cmd: KeyCmd,
    pub label: String,
    pub enabled: bool,
    /// La bulle d'aide (vide : pas de bulle).
    pub hint: String,
}

/// Les entrées communes du menu d'une clé de la sorte `kind`.
pub(crate) fn core_entries(kind: KeyKind, st: MenuState) -> Vec<MenuEntry> {
    let many = st.n > 1;
    // « Copier la clé » / « Copier les clés (3) ».
    let verb = |verb: &str| {
        if many {
            format!("{verb} les {} ({})", kind.noun(true), st.n)
        } else {
            format!("{verb} la {}", kind.noun(false))
        }
    };
    CORE.iter()
        .map(|&cmd| {
            let (label, enabled, hint) = match cmd {
                KeyCmd::Copy => (verb("Copier"), st.has_target, String::new()),
                KeyCmd::Paste => {
                    let tail = match kind {
                        KeyKind::Drawing => " Sur la piste du calque courant.",
                        KeyKind::Camera => " Dans le montage, d'un plan à l'autre.",
                        KeyKind::Effect => " Sur l'effet choisi dans le panneau Effets, s'il est de la même sorte que celui copié.",
                    };
                    (
                        format!("Coller les {}", kind.noun(true)),
                        st.can_paste,
                        format!("Insère les clés après la clé de la tête de lecture, en gardant l'écart entre elles ; les clés d'après sont repoussées.{tail}"),
                    )
                }
                KeyCmd::PasteAtHead => (
                    "Coller à la tête de lecture".to_string(),
                    st.can_paste,
                    "La première clé tombe sur la tête de lecture ; rien n'est repoussé, les clés rencontrées sont remplacées.".to_string(),
                ),
                KeyCmd::Delete => {
                    let hint = match kind {
                        KeyKind::Drawing => "Le dessin d'avant la remplace en tenant plus longtemps (sinon celui d'après).",
                        _ => "",
                    };
                    (verb("Supprimer"), st.has_target, hint.to_string())
                }
                KeyCmd::ChooseAll => {
                    let label = match kind {
                        KeyKind::Drawing => "Choisir toutes les clés du calque",
                        KeyKind::Camera => "Choisir toutes les clés de caméra du plan",
                        KeyKind::Effect => "Choisir toutes les clés de l'effet",
                    };
                    (label.to_string(), true, String::new())
                }
            };
            MenuEntry { cmd, label, enabled, hint }
        })
        .collect()
}

/// Le choix après un clic sur une clé (`group` : les clés du losange cliqué, une seule sauf pour
/// un losange d'effet qui en porte plusieurs, jamais vide) : elle seule ; `toggle` (Ctrl) : elle
/// s'ajoute, ou se retire si elle était entièrement choisie ; `range` (Maj) : les clés `between`
/// (de l'ancre à elle), s'il y a une ancre. Renvoie aussi la nouvelle ancre.
pub(crate) fn choose_group<K: Copy + PartialEq>(chosen: &[K], anchor: Option<K>, group: &[K], toggle: bool, range: Option<Vec<K>>) -> (Vec<K>, Option<K>) {
    let Some(first) = group.first().copied() else { return (chosen.to_vec(), anchor) };
    if let Some(between) = range.filter(|_| anchor.is_some()) {
        return (between, anchor);
    }
    if toggle {
        let mut out = chosen.to_vec();
        if group.iter().all(|m| out.contains(m)) {
            out.retain(|c| !group.contains(c));
        } else {
            out.extend(group.iter().filter(|m| !chosen.contains(m)));
        }
        return (out, Some(first));
    }
    (group.to_vec(), Some(first))
}

/// Le choix une fois le rectangle tendu sur `picked` : sans touche, ces clés seules ; `add` (Maj) :
/// elles s'ajoutent au choix d'avant ; `toggle` (Ctrl) : celles du choix d'avant qu'il touche sont
/// retirées, les autres ajoutées. Le même pour les clés de dessins et de caméra.
///
/// Quelle sorte de clé le rectangle choisit-il ? Celle de la rangée où il PART : la piste Caméra,
/// les clés de caméra ; les pistes des calques, les clés de dessins, sur toutes les rangées qu'il
/// couvre. (Les clés d'effets sont en haut d'une piste de calque : elles se choisissent au clic.)
pub(crate) fn rect_choice<K: Copy + PartialEq>(before: &[K], picked: &[K], add: bool, toggle: bool) -> Vec<K> {
    if toggle {
        let mut out: Vec<K> = before.iter().copied().filter(|k| !picked.contains(k)).collect();
        out.extend(picked.iter().copied().filter(|k| !before.contains(k)));
        return out;
    }
    let mut out = if add { before.to_vec() } else { Vec::new() };
    for k in picked {
        if !out.contains(k) {
            out.push(*k);
        }
    }
    out
}

impl App {
    /// Ce que le menu d'une clé de la sorte `kind` doit savoir.
    pub(crate) fn key_menu_state(&self, kind: KeyKind) -> MenuState {
        match kind {
            KeyKind::Drawing => MenuState { n: self.key_sel.len().max(1), has_target: true, can_paste: self.key_clipboard.is_some() },
            KeyKind::Camera => {
                let n = self.cam_targets().len();
                MenuState { n, has_target: n > 0, can_paste: self.cam_key_clipboard.is_some() }
            }
            KeyKind::Effect => {
                let n = self.fx_targets().iter().map(|t| t.2.len()).sum::<usize>();
                MenuState { n, has_target: n > 0, can_paste: self.fx_key_clipboard.is_some() }
            }
        }
    }

    /// Les commandes communes du menu du clic droit sur une clé : Copier, Coller, Coller à la tête
    /// de lecture, Supprimer, Choisir toutes les clés. Les trois menus passent par ici.
    pub(crate) fn key_menu_core(&mut self, ui: &mut Ui, kind: KeyKind) {
        let st = self.key_menu_state(kind);
        for e in core_entries(kind, st) {
            let shortcut = self.keymap.label(e.cmd.action());
            let resp = ui.add_enabled(e.enabled, egui::Button::new(&e.label).shortcut_text(shortcut));
            let resp = if e.hint.is_empty() { resp } else { crate::bulles::bulle(resp, &e.label, &e.hint, "") };
            if resp.clicked() {
                ui.close_menu();
                self.run_key_cmd(kind, e.cmd);
            }
        }
    }

    /// Lance la commande `cmd` sur les clés de la sorte `kind`.
    pub(crate) fn run_key_cmd(&mut self, kind: KeyKind, cmd: KeyCmd) {
        match (kind, cmd) {
            (KeyKind::Drawing, KeyCmd::Copy) => self.copy_keys(),
            (KeyKind::Drawing, KeyCmd::Paste) => self.paste_keys(),
            (KeyKind::Drawing, KeyCmd::PasteAtHead) => self.paste_keys_at_head(),
            (KeyKind::Drawing, KeyCmd::Delete) => self.delete_keys(false),
            (KeyKind::Drawing, KeyCmd::ChooseAll) => self.choose_all_keys(),
            (KeyKind::Camera, KeyCmd::Copy) => self.copy_cam_keys(),
            (KeyKind::Camera, KeyCmd::Paste) => self.paste_cam_keys(),
            (KeyKind::Camera, KeyCmd::PasteAtHead) => self.paste_cam_keys_at_head(),
            (KeyKind::Camera, KeyCmd::Delete) => self.delete_cam_keys(),
            (KeyKind::Camera, KeyCmd::ChooseAll) => self.choose_all_cam_keys(),
            (KeyKind::Effect, KeyCmd::Copy) => self.copy_fx_keys(),
            (KeyKind::Effect, KeyCmd::Paste) => self.paste_fx_keys(),
            (KeyKind::Effect, KeyCmd::PasteAtHead) => self.paste_fx_keys_at_head(),
            (KeyKind::Effect, KeyCmd::Delete) => self.delete_fx_keys(),
            (KeyKind::Effect, KeyCmd::ChooseAll) => self.choose_all_fx_keys(),
        }
    }
}
