//! La fin des calques quand un plan s'allonge, et « Modifier la durée des clés d'images… » (partie 138, Anthony 2026-11-10).
//!
//! - **Étirer un plan étire ses dernières clés** (« Étirer un plan étire également toutes les dernières clés d'image du plan. Il ne
//!   laisse plus de vide. ») : quand la fin d'un plan s'allonge (poignée de durée de la timeline, bord de fin dans le montage,
//!   « Durée… », « Changer la durée des plans choisis… »), le dernier dessin de chaque calque est tenu jusqu'à la nouvelle fin, même
//!   s'il s'arrêtait avant l'ancienne (`etirer_fins`). Une clé vide (F4, un dessin sans trait) tenue reste vide à l'écran. Un calque
//!   verrouillé reste tel quel, la barre d'état le dit (`verrouilles_courts`). Ctrl tenue sur la poignée ne change pas (les clés
//!   suivent la poignée, partie 123). Les clés de caméra, d'opacité, de position et d'effets ne bougent pas.
//! - **Modifier la durée des clés d'images…** (clic droit sur des clés de la timeline ou du montage, menu Storyboard, action
//!   `cles.duree`, sans raccourci par défaut) : une petite fenêtre demande la durée en images (l'équivalent en secondes et images
//!   à la cadence du projet est montré), préremplie de la durée commune des clés choisies ou vide (« durées différentes »). Entrée
//!   valide, Échap annule. Chaque clé choisie prend cette durée (au moins une image), sur tous les calques concernés ; les clés
//!   suivantes du calque se décalent d'autant (`handles::set_run_length` : rôles, clés d'effets, de position et d'opacité d'après
//!   suivent). Un calque tenu jusqu'à la fin du plan le reste (sa dernière exposition, si elle n'est pas choisie, absorbe le
//!   décalage). Si une clé dépasse alors la fin du plan, le plan s'allonge (décision d'Anthony) : les plans suivants se décalent,
//!   les autres calques étirent leur dernière clé jusqu'à la nouvelle fin. Calques verrouillés laissés de côté. Une seule étape
//!   d'annulation ; « 5 clés passées à 12 images sur 2 calques, plan allongé à 3 s 4 i ».

use eframe::egui::{self, vec2, Align2};

use crate::app::App;
use crate::document::{Layer, Panel};
use crate::montagekeys::KeyRef;

/// Le calque a une fin à étirer : des dessins, pas un dossier ni une vidéo.
fn etirable(l: &Layer) -> bool {
    !l.folder && l.video.is_none() && l.frames.iter().any(Option::is_some)
}

/// Le dernier dessin du calque `l` tenu jusqu'à l'image `fin` (exclue), s'il s'arrêtait avant. Renvoie s'il a changé.
fn etirer_calque(l: &mut Layer, fin: usize) -> bool {
    let Some(last) = l.frames.iter().rposition(Option::is_some) else { return false };
    if last + 1 >= fin {
        return false;
    }
    let cel = l.frames[last];
    l.frames.truncate(last + 1);
    l.frames.resize(fin, cel);
    true
}

/// Chaque calque du plan (sauf les verrouillés) tient son dernier dessin jusqu'à l'image `fin` (exclue). Renvoie combien ont changé.
pub(crate) fn etirer_fins(p: &mut Panel, fin: usize) -> usize {
    let mut n = 0;
    for l in p.layers.iter_mut().filter(|l| etirable(l) && !l.locked) {
        n += usize::from(etirer_calque(l, fin));
    }
    n
}

/// Combien de calques verrouillés du plan s'arrêtent avant l'image `fin` (laissés tels quels par `etirer_fins`).
pub(crate) fn verrouilles_courts(p: &Panel, fin: usize) -> usize {
    p.layers.iter().filter(|l| etirable(l) && l.locked && l.frames.iter().rposition(Option::is_some).is_some_and(|last| last + 1 < fin)).count()
}

/// « 2 s 12 i », « 2 s », « 12 i » : une durée en secondes et images à `fps` images par seconde.
pub(crate) fn duree_texte(n: usize, fps: u32) -> String {
    let fps = fps.max(1) as usize;
    match (n / fps, n % fps) {
        (0, i) => format!("{i} i"),
        (s, 0) => format!("{s} s"),
        (s, i) => format!("{s} s {i} i"),
    }
}

/// Le calque était tenu jusqu'à la fin du plan (`plan_len`).
fn tenu_jusqu_au_bout(l: &Layer, plan_len: usize) -> bool {
    l.frames.len() >= plan_len && l.frames.last().is_some_and(Option::is_some)
}

/// Les clés `starts` (débuts d'expositions dessinées) du calque passées à `n` images chacune, la suite décalée d'autant ; un calque
/// tenu jusqu'à la fin du plan (`plan_len`) le reste : sa dernière exposition, si elle n'est pas une des clés, absorbe le décalage
/// (sans descendre sous une image ; si elle commence déjà après la fin, elle garde sa durée et le plan s'allongera). Renvoie le
/// nouveau début de chaque clé (dans l'ordre croissant) et si sa dernière exposition est à tenir jusqu'à la fin du plan.
pub(crate) fn poser_durees(l: &mut Layer, starts: &[usize], n: usize, plan_len: usize) -> (Vec<usize>, bool) {
    let n = n.max(1);
    let held = tenu_jusqu_au_bout(l, plan_len);
    let mut keys: Vec<(usize, usize)> = starts.iter().filter(|&&s| s < l.frames.len() && l.frames[s].is_some() && l.run_bounds(s).0 == s).map(|&s| (s, l.run_bounds(s).1)).collect();
    keys.sort_unstable();
    keys.dedup();
    // Les nouveaux débuts : chaque clé avance de ce que les clés d'avant ont gagné ou perdu.
    let mut shift = 0isize;
    let mut new_starts = Vec::new();
    for &(s, e) in &keys {
        new_starts.push((s as isize + shift) as usize);
        shift += n as isize - (e - s) as isize;
    }
    // De la dernière à la première : une clé changée ne déplace pas celles d'avant.
    for &(s, e) in keys.iter().rev() {
        crate::handles::set_run_length(l, s, e, n);
    }
    let len = l.frames.len();
    let last_start = if len > 0 { l.run_bounds(len - 1).0 } else { 0 };
    let tenir = held && !new_starts.contains(&last_start);
    if tenir && len > plan_len && last_start < plan_len {
        l.frames.truncate(plan_len);
        l.roles.retain(|f, _| *f < plan_len);
    }
    (new_starts, tenir)
}

/// La fenêtre « Modifier la durée des clés d'images… » : les clés visées, le texte du champ, le focus à donner.
pub(crate) struct DureeCles {
    pub keys: Vec<KeyRef>,
    pub text: String,
    pub focus: bool,
}

fn s(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}

impl App {
    /// Les clés de dessins visées par « Modifier la durée des clés d'images… » : en Animation, celles que touchent les images
    /// choisies de la timeline sur les calques choisis ; en Storyboard, les clés choisies du montage ; sinon la clé de l'image
    /// courante du calque courant.
    pub(crate) fn cles_visees(&self) -> Vec<KeyRef> {
        let panel = self.doc.panels[self.cur_panel].id;
        let mut out: Vec<KeyRef> = Vec::new();
        if self.mode == crate::app::Mode::Animation {
            if let Some((a, b)) = self.frame_sel.filter(|(a, b)| a < b) {
                for li in self.image_layers() {
                    let l = &self.layers()[li];
                    for (st, e, cel) in crate::handles::runs(l) {
                        if cel.is_some() && st < b && e > a {
                            out.push(KeyRef { panel, layer: l.id, start: st });
                        }
                    }
                }
            }
        } else {
            for k in &self.key_sel {
                let Some(p) = self.doc.panels.iter().find(|p| p.id == k.panel) else { continue };
                let Some(l) = p.layers.iter().find(|l| l.id == k.layer) else { continue };
                if l.frames.get(k.start).copied().flatten().is_some() && l.run_bounds(k.start).0 == k.start {
                    out.push(*k);
                }
            }
        }
        if out.is_empty() {
            let l = self.layer();
            if l.frames.get(self.cur_frame).copied().flatten().is_some() {
                out.push(KeyRef { panel, layer: l.id, start: l.run_bounds(self.cur_frame).0 });
            }
        }
        out
    }

    /// La durée (images) de la clé `k`, si elle est toujours là.
    fn duree_cle(&self, k: &KeyRef) -> Option<usize> {
        let p = self.doc.panels.iter().find(|p| p.id == k.panel)?;
        let l = p.layers.iter().find(|l| l.id == k.layer)?;
        l.frames.get(k.start).copied().flatten()?;
        let (st, e) = l.run_bounds(k.start);
        (st == k.start).then_some(e - st)
    }

    /// « Modifier la durée des clés d'images… » : ouvre la fenêtre, le champ prérempli de la durée commune (ou vide).
    pub(crate) fn open_key_duration_dialog(&mut self) {
        if self.content_refuses() {
            return;
        }
        let keys = self.cles_visees();
        if keys.is_empty() {
            self.status = "Pas de clé choisie : choisir des clés de dessins (timeline ou montage), ou placer la tête de lecture sur un dessin.".into();
            return;
        }
        let durees: Vec<usize> = keys.iter().filter_map(|k| self.duree_cle(k)).collect();
        let text = match durees.first() {
            Some(&d) if durees.iter().all(|&x| x == d) => d.to_string(),
            _ => String::new(),
        };
        self.duree_cles = Some(DureeCles { keys, text, focus: true });
    }

    /// Chaque clé `keys` passe à `n` images (voir le haut du fichier), en une étape d'annulation. Faux si rien n'a changé.
    pub(crate) fn change_key_durations(&mut self, keys: &[KeyRef], n: usize) -> bool {
        if n == 0 {
            self.status = "Durée refusée : une clé dure au moins une image.".into();
            return false;
        }
        if self.content_refuses() {
            return false;
        }
        let mut plans: Vec<u64> = keys.iter().map(|k| k.panel).collect();
        plans.sort_unstable();
        plans.dedup();
        if !self.partage_peut_toucher(&plans) {
            return false;
        }
        // Par calque : ses clés.
        let mut groups: Vec<(u64, u64, Vec<usize>)> = Vec::new();
        for k in keys {
            match groups.iter_mut().find(|g| g.0 == k.panel && g.1 == k.layer) {
                Some(g) => g.2.push(k.start),
                None => groups.push((k.panel, k.layer, vec![k.start])),
            }
        }
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let fps = self.doc.fps;
        let (mut faites, mut calques) = (0usize, 0usize);
        let mut verrouilles: Vec<String> = Vec::new();
        let mut allonges: Vec<usize> = Vec::new();
        let mut sel = Vec::new();
        for &pid in &plans {
            let Some(pi) = self.doc.panels.iter().position(|p| p.id == pid) else { continue };
            let plan_len = self.doc.panels[pi].frame_count();
            let mut touches: Vec<(u64, bool)> = Vec::new();
            for (_, lid, starts) in groups.iter().filter(|g| g.0 == pid) {
                let Some(li) = self.doc.panels[pi].layers.iter().position(|l| l.id == *lid) else { continue };
                let l = &mut self.doc.panels[pi].layers[li];
                if l.locked {
                    verrouilles.push(l.name.clone());
                    continue;
                }
                let avant = l.frames.clone();
                let (new_starts, tenir) = poser_durees(l, starts, n, plan_len);
                if l.frames != avant {
                    calques += 1;
                }
                faites += new_starts.len();
                sel.extend(new_starts.iter().map(|&start| KeyRef { panel: pid, layer: *lid, start }));
                touches.push((*lid, tenir));
            }
            // La nouvelle fin : celle d'avant, ou plus loin si une clé la dépasse (le plan s'allonge, les plans suivants se décalent).
            let p = &mut self.doc.panels[pi];
            let fin = p.layers.iter().map(|l| l.frames.len()).max().unwrap_or(0).max(plan_len);
            for l in p.layers.iter_mut().filter(|l| etirable(l) && !l.locked) {
                match touches.iter().find(|t| t.0 == l.id) {
                    // Un calque changé tenu jusqu'au bout le reste (sa dernière clé n'était pas choisie).
                    Some(&(_, true)) => {
                        etirer_calque(l, fin);
                    }
                    Some(_) => {}
                    // Les autres calques : leur dernière clé s'étire jusqu'à la nouvelle fin si le plan s'allonge.
                    None if fin > plan_len => {
                        etirer_calque(l, fin);
                    }
                    None => {}
                }
            }
            if fin > plan_len {
                p.length = fin;
                allonges.push(pi);
            }
        }
        if calques == 0 {
            self.status = if verrouilles.is_empty() { format!("Ces clés duraient déjà {n} image{}.", s(n)) } else { format!("Rien n'a changé : calque{} verrouillé{} ({}).", s(verrouilles.len()), s(verrouilles.len()), verrouilles.join(", ")) };
            return false;
        }
        self.key_sel = sel;
        if self.mode == crate::app::Mode::Animation {
            self.frame_sel = None;
        }
        self.vsel = None;
        self.renderer.invalidate_all();
        self.commit_as(snap, "Durée des clés");
        let mut msg = format!("{faites} clé{} passée{} à {n} image{} sur {calques} calque{}", s(faites), s(faites), s(n), s(calques));
        for pi in allonges {
            let d = self.doc.panels[pi].frame_count();
            msg += &format!(", {} allongé à {}", crate::storyboard::panel_title(&self.doc, pi).to_lowercase(), duree_texte(d, fps));
        }
        if !verrouilles.is_empty() {
            msg += &format!(" ; laissé{} de côté (verrouillé{}) : {}", s(verrouilles.len()), s(verrouilles.len()), verrouilles.iter().map(|v| format!("« {v} »")).collect::<Vec<_>>().join(", "));
        }
        self.status = msg + ".";
        true
    }

    /// La fenêtre « Modifier la durée des clés d'images… » : un champ en images, l'équivalent en secondes et images, Entrée ou Échap.
    pub(crate) fn key_duration_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.duree_cles.take() else { return };
        dlg.keys.retain(|k| self.duree_cle(k).is_some());
        if dlg.keys.is_empty() {
            return;
        }
        let fps = self.doc.fps;
        let durees: Vec<usize> = dlg.keys.iter().filter_map(|k| self.duree_cle(k)).collect();
        let differentes = durees.iter().any(|&d| d != durees[0]);
        let voulue: Option<usize> = dlg.text.trim().parse::<usize>().ok();
        let (mut apply, mut close) = (false, false);
        egui::Window::new("Durée des clés d'images")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                let (k, m) = crate::montagekeys::key_counts(&dlg.keys);
                ui.label(format!("{k} clé{} choisie{} sur {m} calque{} ({fps} i/s).", s(k), s(k), s(m)));
                ui.horizontal(|ui| {
                    let resp = ui.add(egui::TextEdit::singleline(&mut dlg.text).desired_width(80.0).hint_text(if differentes { "durées différentes" } else { "images" }));
                    if dlg.focus {
                        resp.request_focus();
                        dlg.focus = false;
                    }
                    ui.label("images");
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && voulue.is_some_and(|n| n > 0) {
                        apply = true;
                    }
                });
                match voulue {
                    Some(0) => {
                        ui.colored_label(ui.visuals().warn_fg_color, "Au moins une image.");
                    }
                    Some(n) => {
                        ui.label(format!("= {} à {fps} i/s. Les clés suivantes de chaque calque se décalent d'autant ; si une clé dépasse la fin du plan, le plan s'allonge.", duree_texte(n, fps)));
                    }
                    None if dlg.text.trim().is_empty() && differentes => {
                        ui.label(egui::RichText::new("Les clés choisies ont des durées différentes : taper la durée que chacune prendra.").weak());
                    }
                    None => {
                        ui.colored_label(ui.visuals().warn_fg_color, "Un nombre d'images, par exemple 12.");
                    }
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(voulue.is_some_and(|n| n > 0), egui::Button::new("Valider")).clicked() {
                        apply = true;
                    }
                    if ui.button("Annuler").clicked() {
                        close = true;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if apply {
            if let Some(n) = voulue {
                self.change_key_durations(&dlg.keys, n);
            }
            return;
        }
        if !close {
            self.duree_cles = Some(dlg);
        }
    }
}
