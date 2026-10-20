//! Copier-coller des effets d'un calque à un autre (Anthony, 2026-10-19 : « j'aimerais qu'on puisse
//! copier-coller un effet d'un calque à un autre »).
//!
//! Ce qui se copie : un effet de la pile d'un calque (sa sorte, son état actif, ses valeurs et ses
//! clés : toute son animation), ou tous les effets du calque. Le presse-papiers d'effets est en
//! mémoire dans l'application (`App::fx_stack_clipboard`) : il survit à l'ouverture d'un autre
//! projet, rien dans un effet ne renvoie à un dessin.
//!
//! Où ça se trouve :
//! - panneau Effets : l'icône « Copier l'effet » de chaque effet, le clic droit sur son nom, et le
//!   menu « Copier / coller » de la rangée du haut ;
//! - menus des calques (panneau Calques, montage, timeline) : sous-menu « Effets », en bas ;
//! - actions `fx.copier`, `fx.copier_tout`, `fx.coller`, `fx.remplacer` (palette Ctrl+K, raccourcis
//!   à donner soi-même : aucun par défaut) ;
//! - clavier : Ctrl+C / Ctrl+V quand le panneau Effets a la main (voir « focus » plus bas).
//!
//! Coller :
//! - les effets s'ajoutent à la fin de la pile du calque, comme « Ajouter un effet » (ils
//!   s'appliquent après ceux qui y sont) ; « Remplacer les effets » remplace toute la pile ;
//! - sur le calque courant, ou sur tous les calques choisis s'il y en a plusieurs (Ctrl / Maj + clic
//!   dans le panneau Calques) ; une seule étape d'annulation, même sur plusieurs calques ;
//! - les clés gardent leurs images ; celles qui tombent après la fin du plan sont laissées de côté
//!   (un effet dont toutes les clés tombent dehors garde alors, fixes, les valeurs de sa première
//!   clé) ;
//! - le tremblement du trait (calques vectoriels seulement) est laissé de côté sur un calque bitmap,
//!   et s'il est déjà sur le calque (un seul par calque) ; les dossiers n'ont pas d'effets ;
//! - la barre d'état dit ce qui a été collé où, et ce qui a été laissé de côté.
//!
//! Le focus : cliquer dans le panneau Effets lui donne la main (cadre de la couleur d'accent, et les
//! montages / timeline / dessin la perdent). Alors Ctrl+C copie l'effet choisi (le seul de la pile
//! s'il n'y en a qu'un) et Ctrl+V colle les effets copiés, avant tout autre presse-papiers (clés
//! d'effets, de caméra, de dessins, images, plans). Un clic ailleurs (une clé d'effet dans la
//! timeline, le montage, le dessin) rend la main à ce qui est cliqué, et « Créer une clé » rend la
//! main à la clé qu'il vient de créer : Ctrl+C copie alors ces clés, comme avant.
//!
//! La logique est ici en fonctions pures (`paste_onto`, `message`…), testées dans
//! `fxcopie_tests.rs` ; `impl App` ne fait que la brancher sur le document, les menus et
//! l'historique.

use eframe::egui::{self, Response};

use crate::app::App;
use crate::document::LayerKind;
use crate::fx::{Effect, EffectKind};

/// Les effets copiés.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FxStackClip {
    pub effects: Vec<Effect>,
}

/// Ce qu'un collage a donné sur la pile d'un calque.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Landed {
    /// Les sortes d'effets ajoutées, dans l'ordre.
    pub added: Vec<EffectKind>,
    /// Laissés de côté : effets réservés au vectoriel, sur un calque bitmap.
    pub vector_only_skipped: Vec<EffectKind>,
    /// Laissés de côté : effets qu'un calque n'a qu'une fois, déjà là.
    pub duplicates_skipped: Vec<EffectKind>,
    /// Clés après la fin du plan.
    pub keys_dropped: usize,
    /// Effets retirés de la pile (« Remplacer »).
    pub removed: usize,
}

/// Le tremblement du trait et les effets du trait ne sont qu'une fois par calque (les menus les
/// cochent et les décochent comme un interrupteur).
fn single_instance(kind: EffectKind) -> bool {
    kind.vector_only()
}

/// L'effet `e` pour un plan de `len` images : ses clés après la fin du plan en sont retirées. Renvoie
/// aussi combien. Si toutes tombent dehors, l'effet garde les valeurs de sa première clé, fixes (c'est
/// ce qu'il montrait dans tout le plan).
pub(crate) fn fit_to_plan(e: &Effect, len: usize) -> (Effect, usize) {
    let mut out = e.clone();
    let dropped = out.keys.iter().filter(|k| k.frame >= len).count();
    if dropped > 0 {
        if dropped == out.keys.len() {
            out.values = out.values_at(0);
        }
        out.keys.retain(|k| k.frame < len);
    }
    (out, dropped)
}

/// Colle `clip` sur la pile `stack` d'un calque (`vector` : il est vectoriel) dans un plan de `len`
/// images : à la fin de la pile, ou à la place de toute la pile si `replace`. Si rien ne peut être collé,
/// la pile n'est pas touchée (même en `replace`).
pub(crate) fn paste_onto(stack: &mut Vec<Effect>, clip: &[Effect], vector: bool, len: usize, replace: bool) -> Landed {
    let mut landed = Landed::default();
    let mut incoming: Vec<Effect> = Vec::new();
    for e in clip {
        if e.kind.vector_only() && !vector {
            landed.vector_only_skipped.push(e.kind);
            continue;
        }
        let there = incoming.iter().any(|x| x.kind == e.kind) || (!replace && stack.iter().any(|x| x.kind == e.kind));
        if single_instance(e.kind) && there {
            landed.duplicates_skipped.push(e.kind);
            continue;
        }
        let (fit, dropped) = fit_to_plan(e, len);
        landed.keys_dropped += dropped;
        incoming.push(fit);
    }
    if incoming.is_empty() {
        // (Rien ne se colle : les clés laissées de côté ne comptent pas.)
        landed.keys_dropped = 0;
        return landed;
    }
    if replace {
        landed.removed = stack.len();
        stack.clear();
    }
    landed.added = incoming.iter().map(|e| e.kind).collect();
    stack.extend(incoming);
    landed
}

/// Ce que donne un collage sur tous les calques visés.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Report {
    /// Les calques qui ont reçu au moins un effet.
    pub layers: Vec<usize>,
    /// Le plus d'effets reçus par un calque.
    pub most: usize,
    /// Les sortes collées quelque part, sans doublon, dans l'ordre.
    pub kinds: Vec<EffectKind>,
    /// Combien de calques bitmap ont laissé de côté des effets vectoriels, et lesquels.
    pub bitmap_layers: usize,
    pub vector_only: Vec<EffectKind>,
    /// Combien de calques avaient déjà l'effet qu'ils n'ont qu'une fois, et lequel.
    pub duplicate_layers: usize,
    pub duplicates: Vec<EffectKind>,
    pub keys_dropped: usize,
    pub folders: usize,
    pub removed: usize,
}

impl Report {
    /// Ajoute ce qui s'est passé sur le calque `li`.
    pub(crate) fn absorb(&mut self, li: usize, l: &Landed) {
        if !l.added.is_empty() {
            self.layers.push(li);
            self.most = self.most.max(l.added.len());
            for k in &l.added {
                if !self.kinds.contains(k) {
                    self.kinds.push(*k);
                }
            }
        }
        if !l.vector_only_skipped.is_empty() {
            self.bitmap_layers += 1;
            for k in &l.vector_only_skipped {
                if !self.vector_only.contains(k) {
                    self.vector_only.push(*k);
                }
            }
        }
        if !l.duplicates_skipped.is_empty() {
            self.duplicate_layers += 1;
            for k in &l.duplicates_skipped {
                if !self.duplicates.contains(k) {
                    self.duplicates.push(*k);
                }
            }
        }
        self.keys_dropped += l.keys_dropped;
        self.removed += l.removed;
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n > 1 { many } else { one })
}

/// L'accord du participe d'un effet : « Flou gaussien collé », « Ombre portée collée », « Niveaux
/// collés ».
fn agree(kind: EffectKind, masculine: &str) -> String {
    let feminine = matches!(
        kind,
        EffectKind::BrightnessContrast | EffectKind::HueSaturation | EffectKind::Posterize | EffectKind::Sharpen | EffectKind::Mosaic | EffectKind::DropShadow | EffectKind::Glow | EffectKind::Wave | EffectKind::Ripple
    );
    let plural = matches!(kind, EffectKind::Levels | EffectKind::Clouds);
    format!("{masculine}{}{}", if feminine { "e" } else { "" }, if plural { "s" } else { "" })
}

/// Les noms des sortes, séparés par des virgules (quatre au plus).
fn names(kinds: &[EffectKind]) -> String {
    let mut s: Vec<&str> = kinds.iter().take(4).map(|k| k.label()).collect();
    if kinds.len() > 4 {
        s.push("…");
    }
    s.join(", ")
}

/// La phrase de la barre d'état après un collage. `layer_name` : le nom du calque quand il n'y en a
/// qu'un.
pub(crate) fn message(rep: &Report, replace: bool, layer_name: &str) -> String {
    let n = rep.layers.len();
    let place = if n == 1 { format!("le calque « {layer_name} »") } else { format!("{n} calques") };
    let mut out = if n == 0 {
        "Rien collé".to_string()
    } else if replace {
        let what = if rep.kinds.len() == 1 && rep.most == 1 { rep.kinds[0].label().to_string() } else { names(&rep.kinds) };
        format!("Effets remplacés sur {place} : {what}")
    } else if rep.kinds.len() == 1 && rep.most == 1 {
        let k = rep.kinds[0];
        format!("{} {} sur {place}", k.label(), agree(k, "collé"))
    } else {
        format!("{} sur {place}", plural(rep.most, "effet collé", "effets collés"))
    };
    if rep.bitmap_layers > 0 {
        out += &format!(
            " ; {} ignoré sur {} bitmap (calques vectoriels seulement)",
            names(&rep.vector_only),
            plural(rep.bitmap_layers, "calque", "calques")
        );
    }
    if rep.duplicate_layers > 0 {
        out += &format!(" ; {} déjà sur {} : non ajouté", names(&rep.duplicates), plural(rep.duplicate_layers, "calque", "calques"));
    }
    if rep.folders > 0 {
        out += &format!(" ; {} ignoré (un dossier n'a pas d'effets)", plural(rep.folders, "dossier", "dossiers"));
    }
    if rep.keys_dropped > 0 {
        out += &format!(" ; {} après la fin du plan laissée{} de côté", plural(rep.keys_dropped, "clé", "clés"), if rep.keys_dropped > 1 { "s" } else { "" });
    }
    out.push('.');
    out
}

impl App {
    /// Le panneau Effets a la main (le dernier clic était dedans) : Ctrl+C et Ctrl+V visent ses
    /// effets avant les autres presse-papiers.
    pub(crate) fn fx_pane_has_focus(&self) -> bool {
        self.fx_pane_focus
    }

    /// Un clic dans le panneau Effets lui donne la main : le montage, la timeline et le dessin la
    /// perdent (leur cadre disparaît, et copier / coller ne les visent plus).
    pub(crate) fn take_fx_pane_focus(&mut self) {
        self.fx_pane_focus = true;
        self.panel_focus = false;
        self.images_focus = false;
        self.key_focus = false;
    }

    /// Les calques sur lesquels colle une commande lancée depuis le calque `li` : tous les calques
    /// choisis si `li` en est, sinon `li` seul.
    pub(crate) fn fx_paste_targets(&self, li: usize) -> Vec<usize> {
        if self.is_chosen_layer(li) {
            self.chosen_layers()
        } else {
            vec![li]
        }
    }

    /// Ce que contient le presse-papiers d'effets, pour une bulle : « Flou gaussien (2 clés), Lueur ».
    pub(crate) fn fx_clip_summary(&self) -> Option<String> {
        let clip = self.fx_stack_clipboard.as_ref()?;
        let parts: Vec<String> = clip
            .effects
            .iter()
            .map(|e| match e.keys.len() {
                0 => e.kind.label().to_string(),
                k => format!("{} ({})", e.kind.label(), plural(k, "clé", "clés")),
            })
            .collect();
        Some(parts.join(", "))
    }

    fn set_fx_stack_clip(&mut self, effects: Vec<Effect>) {
        self.fx_stack_clipboard = Some(FxStackClip { effects });
        // La dernière chose copiée : les clés (d'effets, de caméra) ne se collent plus au clavier
        // dans la timeline (Ctrl+V y vise ce qui a été copié en dernier).
        self.fx_clip_recent = false;
        self.cam_clip_recent = false;
        // Dans le presse-papiers de Windows : Ctrl+V vise de nouveau SHOT (`pasteimage`).
        self.ctx.copy_text(format!("{}effets copiés", crate::pasteimage::MARK));
    }

    /// Copie l'effet de rang `rank` de la pile du calque `li`.
    pub(crate) fn fx_copy_effect(&mut self, li: usize, rank: usize) {
        let Some(e) = self.layers().get(li).and_then(|l| l.fx.get(rank)).cloned() else {
            self.status = "Pas d'effet à copier : choisis un effet dans la pile du calque (clic sur son nom, dans le panneau Effets).".into();
            return;
        };
        let keys = e.keys.len();
        let label = e.kind.label();
        self.set_fx_stack_clip(vec![e]);
        let keys = if keys > 0 { format!(" avec {}", plural(keys, "clé", "clés")) } else { String::new() };
        self.status = format!("Effet copié : {label}{keys}. Ctrl+V dans le panneau Effets (ou Effets > Coller l'effet, dans le menu d'un calque) le colle sur le calque, ou sur les calques choisis.");
    }

    /// Copie tous les effets du calque `li`.
    pub(crate) fn fx_copy_all(&mut self, li: usize) {
        let Some(layer) = self.layers().get(li) else { return };
        if layer.folder {
            self.status = "Un dossier n'a pas d'effets.".into();
            return;
        }
        if layer.fx.is_empty() {
            self.status = format!("Aucun effet sur le calque « {} » : rien à copier.", layer.name);
            return;
        }
        let (effects, name) = (layer.fx.clone(), layer.name.clone());
        let n = effects.len();
        self.set_fx_stack_clip(effects);
        self.status = format!("{} du calque « {name} » copiés. « Coller les effets » (Effets, dans le menu d'un calque) les ajoute au calque, ou aux calques choisis ; « Remplacer les effets » remplace leur pile.", plural(n, "effet", "effets"));
    }

    /// Ctrl+C dans le panneau Effets, et l'action `fx.copier` : copie l'effet choisi, ou le seul de
    /// la pile du calque courant.
    pub(crate) fn fx_copy_chosen(&mut self) {
        let li = self.cur_layer;
        let n = self.layers().get(li).map_or(0, |l| l.fx.len());
        match (self.fx_index(), n) {
            (Some(i), _) => self.fx_copy_effect(li, i),
            (None, 1) => self.fx_copy_effect(li, 0),
            (None, 0) => self.status = "Aucun effet sur ce calque : rien à copier.".into(),
            (None, _) => self.status = "Choisis d'abord l'effet à copier (clic sur son nom, dans le panneau Effets), ou copie tous les effets du calque.".into(),
        }
    }

    /// Colle les effets copiés sur les calques `targets` (rangs dans le plan courant) : à la fin de
    /// leur pile, ou à la place de toute leur pile si `replace`. Une seule étape d'annulation.
    pub(crate) fn fx_paste(&mut self, targets: Vec<usize>, replace: bool) {
        let Some(clip) = self.fx_stack_clipboard.clone() else {
            self.status = "Aucun effet copié : copie d'abord un effet (panneau Effets) ou tous les effets d'un calque.".into();
            return;
        };
        if self.content_refuses() {
            return;
        }
        let len = self.frame_count();
        let mut rep = Report::default();
        // (calque, nouvelle pile, rang du premier effet collé)
        let mut changes: Vec<(usize, Vec<Effect>, usize)> = Vec::new();
        for &li in &targets {
            let Some(layer) = self.layers().get(li) else { continue };
            if layer.folder {
                rep.folders += 1;
                continue;
            }
            let mut stack = layer.fx.clone();
            let landed = paste_onto(&mut stack, &clip.effects, layer.kind == LayerKind::Vector, len, replace);
            rep.absorb(li, &landed);
            if !landed.added.is_empty() {
                let first = stack.len() - landed.added.len();
                changes.push((li, stack, first));
            }
        }
        // (Le nom du calque, quand il n'y en a qu'un.)
        let one = rep.layers.first().or(targets.first()).copied().unwrap_or(self.cur_layer);
        let name = self.layers().get(one).map(|l| l.name.clone()).unwrap_or_default();
        if changes.is_empty() {
            self.status = message(&rep, replace, &name);
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let mut select = None;
        for (li, stack, first) in changes {
            if li == self.cur_layer {
                select = Some(first);
            }
            self.layers_mut()[li].fx = stack;
        }
        if replace {
            // (Les rangs ont changé : ni effet choisi, ni clé choisie.)
            self.deselect_fx();
            self.fx_key_sel.clear();
            self.fx_key_anchor = None;
        }
        // Sur le calque courant, le premier effet collé devient le choix (ses clés se voient).
        if let Some(first) = select {
            self.select_fx(first);
        }
        self.status = message(&rep, replace, &name);
        let step = if replace {
            "Remplacer les effets"
        } else if clip.effects.len() == 1 {
            "Coller l'effet"
        } else {
            "Coller les effets"
        };
        self.renderer.invalidate_all();
        self.commit_named(snap, step);
    }

    /// Les commandes `fx.copier`, `fx.copier_tout`, `fx.coller` et `fx.remplacer`.
    pub(crate) fn fx_clip_action(&mut self, action: &str) -> bool {
        let li = self.cur_layer;
        match action {
            "fx.copier" => self.fx_copy_chosen(),
            "fx.copier_tout" => self.fx_copy_all(li),
            "fx.coller" => self.fx_paste(self.fx_paste_targets(li), false),
            "fx.remplacer" => self.fx_paste(self.fx_paste_targets(li), true),
            _ => return false,
        }
        true
    }

    /// Une entrée de menu avec son raccourci ; clic droit : lui assigner un raccourci. Vrai si cliquée.
    fn fx_clip_item(&mut self, ui: &mut egui::Ui, text: &str, action: &str, enabled: bool, hover: &str) -> bool {
        let shortcut = self.keymap.label(action);
        let resp: Response = ui.add_enabled(enabled, egui::Button::new(text).shortcut_text(&shortcut)).on_hover_text(hover).on_disabled_hover_text(hover);
        if resp.clicked() {
            ui.close_menu();
            return true;
        }
        if resp.secondary_clicked() {
            ui.close_menu();
            self.start_assign(action);
        }
        false
    }

    /// Les entrées de copier-coller des effets pour le calque `li` : dans le sous-menu « Effets » des
    /// calques (`rank` vide) et dans les menus du panneau Effets (`rank` : l'effet visé, `pane` : le
    /// panneau reprend alors la main).
    pub(crate) fn fx_clip_menu(&mut self, ui: &mut egui::Ui, li: usize, rank: Option<usize>, pane: bool) {
        let Some((folder, has, name)) = self.layers().get(li).map(|l| (l.folder, !l.fx.is_empty(), l.name.clone())) else { return };
        if folder {
            return;
        }
        if pane {
            self.take_fx_pane_focus();
        }
        if let Some(i) = rank {
            if self.fx_clip_item(ui, "Copier l'effet", "fx.copier", true, "Copie cet effet avec ses réglages et toutes ses clés. Ctrl+C dans le panneau Effets fait de même pour l'effet choisi.") {
                self.fx_copy_effect(li, i);
            }
        }
        if self.fx_clip_item(ui, "Copier tous les effets du calque", "fx.copier_tout", has, &format!("Copie toute la pile d'effets du calque « {name} » (réglages et clés).")) {
            self.fx_copy_all(li);
        }
        let targets = self.fx_paste_targets(li);
        let (what, summary) = (if self.fx_stack_clipboard.as_ref().is_some_and(|c| c.effects.len() == 1) { "Coller l'effet" } else { "Coller les effets" }, self.fx_clip_summary());
        let where_ = if targets.len() > 1 { format!("sur les {} calques choisis", targets.len()) } else { format!("sur le calque « {name} »") };
        let paste_hover = match &summary {
            Some(s) => format!("Ajoute à la fin de la pile, {where_} : {s}. Les clés gardent leurs images (celles d'après la fin du plan sont laissées de côté). Le tremblement du trait ne va que sur un calque vectoriel."),
            None => "Rien n'est copié : copie d'abord un effet ou tous les effets d'un calque.".to_string(),
        };
        if self.fx_clip_item(ui, what, "fx.coller", summary.is_some(), &paste_hover) {
            self.fx_paste(targets.clone(), false);
        }
        let replace_hover = match &summary {
            Some(s) => format!("Remplace toute la pile d'effets, {where_}, par : {s}. Une seule étape d'annulation."),
            None => "Rien n'est copié : copie d'abord un effet ou tous les effets d'un calque.".to_string(),
        };
        if self.fx_clip_item(ui, "Remplacer les effets", "fx.remplacer", summary.is_some(), &replace_hover) {
            self.fx_paste(targets, true);
        }
    }
}
