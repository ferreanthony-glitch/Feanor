//! Partie 136 : les réglages d'un calque agissent sur tous les calques choisis (Anthony, 2026-11-10, règle « la sélection agit sur
//! tout »). Le mode de fusion, l'œil, le cadenas, la pelure d'oignon et le pochoir : la valeur choisie va à tous les calques choisis ;
//! un interrupteur donne à tous l'inverse de l'état du calque cliqué. Un clic sur un calque qui n'est pas dans le choix ne touche
//! que lui. Les effets de trait et de matière, le flou, leurs préréglages, leurs réglages et leurs clés : sur chaque calque choisi qui
//! peut les recevoir, les autres laissés de côté avec un message (`fxui.rs`). Une étape d'annulation par geste.

use crate::app::App;
use crate::document::Layer;
use crate::fx::EffectKind;
use crate::blend::BlendMode;

impl App {
    /// Les calques qu'un réglage fait sur le calque `li` touche : tous les calques choisis si `li` en est (et qu'ils sont deux au
    /// moins), sinon `li` seul.
    pub(crate) fn calques_du_geste(&self, li: usize) -> Vec<usize> {
        let choisis = self.chosen_layers();
        if choisis.len() >= 2 && choisis.contains(&li) {
            choisis
        } else {
            vec![li]
        }
    }

    /// Un interrupteur de calque (`lire` / `ecrire`) cliqué sur le calque `li` : tous les calques du geste prennent l'inverse de
    /// l'état de `li`. `dossiers` : les dossiers le prennent aussi (sinon ils sont laissés de côté). Une étape d'annulation. Rend
    /// le nombre de calques changés.
    pub(crate) fn basculer_calques(&mut self, li: usize, nom: &str, lire: fn(&Layer) -> bool, ecrire: fn(&mut Layer, bool), dossiers: bool) -> usize {
        let Some(v) = self.layers().get(li).map(|l| !lire(l)) else { return 0 };
        let lis = self.calques_du_geste(li);
        // (Un calque seul : comme avant, même un dossier.)
        let (ok, laisses) = self.sans_les_dossiers(&lis, dossiers || lis.len() == 1);
        if ok.is_empty() {
            self.status = format!("{nom} : un dossier ne le prend pas.");
            return 0;
        }
        self.end_stroke();
        self.record_step();
        for &k in &ok {
            ecrire(&mut self.layers_mut()[k], v);
        }
        self.renderer.invalidate_all();
        if lis.len() >= 2 {
            self.status = format!("{nom} : {} sur {} calque{}.{}", if v { "allumé" } else { "éteint" }, ok.len(), pluriel(ok.len()), laisses);
        }
        ok.len()
    }

    /// Le mode de fusion `mode` choisi pour le calque `li` : sur tous les calques du geste (pas les dossiers). Une étape.
    pub(crate) fn regler_mode_calques(&mut self, li: usize, mode: BlendMode) {
        let lis = self.calques_du_geste(li);
        let (ok, laisses) = self.sans_les_dossiers(&lis, lis.len() == 1);
        if ok.is_empty() || ok.iter().all(|&k| self.layers()[k].blend == mode) {
            return;
        }
        self.end_stroke();
        self.record_step();
        for &k in &ok {
            self.layers_mut()[k].blend = mode;
        }
        self.renderer.invalidate_all();
        if lis.len() >= 2 {
            self.status = format!("Mode de fusion « {} » sur {} calque{}.{}", mode.label(), ok.len(), pluriel(ok.len()), laisses);
        }
    }

    /// Les calques `lis` sans les dossiers (si `dossiers` est faux), et le texte de ceux laissés de côté.
    fn sans_les_dossiers(&self, lis: &[usize], dossiers: bool) -> (Vec<usize>, String) {
        let mut ok = Vec::new();
        let mut laisses = Vec::new();
        for &k in lis {
            match self.layers().get(k) {
                Some(l) if l.folder && !dossiers => laisses.push(format!("{} (dossier)", l.name)),
                Some(_) => ok.push(k),
                None => {}
            }
        }
        (ok, laisses_de_cote(&laisses))
    }

    /// Les calques du geste fait sur `li` qui prennent l'effet `kind`, et le texte de ceux laissés de côté (dossier, ou sorte de calque
    /// qui ne le prend pas).
    pub(crate) fn calques_pour_l_effet(&self, li: usize, kinds: &[EffectKind]) -> (Vec<usize>, String) {
        let mut ok = Vec::new();
        let mut laisses = Vec::new();
        for k in self.calques_du_geste(li) {
            let Some(l) = self.layers().get(k) else { continue };
            if kinds.iter().all(|&kind| self.fx_allowed(k, kind)) {
                ok.push(k);
            } else {
                laisses.push(format!("{} ({})", l.name, if l.folder { "dossier" } else { "calque vectoriel seulement" }));
            }
        }
        (ok, laisses_de_cote(&laisses))
    }

    /// Dans le calque `k`, l'effet qui répond à l'effet `i` du calque `li` : le même rang parmi les effets de même sorte, sinon le
    /// premier de cette sorte. Rien si `k` n'a pas d'effet de cette sorte.
    pub(crate) fn effet_pareil(&self, li: usize, i: usize, k: usize) -> Option<usize> {
        let fx = &self.layers().get(li)?.fx;
        let kind = fx.get(i)?.kind;
        let rang = fx[..i].iter().filter(|e| e.kind == kind).count();
        let autres: Vec<usize> = self.layers().get(k)?.fx.iter().enumerate().filter(|(_, e)| e.kind == kind).map(|(j, _)| j).collect();
        autres.get(rang).or(autres.first()).copied()
    }

    /// Les effets qui répondent à l'effet `i` du calque `li` sur les calques du geste, `li` compris (calque, rang de l'effet).
    pub(crate) fn effets_du_geste(&self, li: usize, i: usize) -> Vec<(usize, usize)> {
        let mut out = vec![(li, i)];
        for k in self.calques_du_geste(li) {
            if k != li {
                if let Some(j) = self.effet_pareil(li, i, k) {
                    out.push((k, j));
                }
            }
        }
        out
    }

    /// Règle les paramètres `k..k + valeurs.len()` de l'effet `i` du calque `li` à l'image courante, et de l'effet de même sorte de
    /// chaque calque choisi (`effets_du_geste`). Un interrupteur ou un choix (`fige`) vaut pour toutes les clés ; sinon, avec des
    /// clés (ou la création automatique), la clé de l'image courante est posée ; sans clé, l'effet change sur tout le calque. Pas
    /// d'étape ici : le geste du panneau en a déjà une (`edited`). Rend le nombre d'effets réglés.
    pub(crate) fn fx_regler(&mut self, li: usize, i: usize, k: usize, valeurs: &[f32], fige: bool) -> usize {
        let f = self.cur_frame;
        let auto = self.fx_auto_key;
        let cibles = self.effets_du_geste(li, i);
        for &(l, j) in &cibles {
            let Some(effect) = self.layers_mut().get_mut(l).and_then(|l| l.fx.get_mut(j)) else { continue };
            if fige {
                for (c, &v) in valeurs.iter().enumerate() {
                    effect.set_param(k + c, v);
                }
                continue;
            }
            let mut new = effect.values_at(f);
            for (c, &v) in valeurs.iter().enumerate() {
                if let Some(x) = new.get_mut(k + c) {
                    *x = v;
                }
            }
            if auto || !effect.keys.is_empty() {
                effect.set_key(f, new);
            } else {
                effect.values = new;
            }
        }
        if cibles.len() >= 2 {
            self.status = format!("Réglage de l'effet sur {} calques.", cibles.len());
        }
        cibles.len()
    }
}

fn pluriel(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}

/// « Laissé de côté : A (dossier), B (…). » (vide s'il n'y en a pas), avec une espace devant.
pub(crate) fn laisses_de_cote(laisses: &[String]) -> String {
    if laisses.is_empty() {
        String::new()
    } else {
        format!(" Laissé{} de côté : {}.", pluriel(laisses.len()), laisses.join(", "))
    }
}
