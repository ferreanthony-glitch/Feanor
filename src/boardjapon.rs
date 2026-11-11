//! La planche des Vignettes lue **à la japonaise** (Anthony, 2026-10-24, le panneau « Vignettes ») : les cases se suivent de haut en bas,
//! en colonne ; quand la colonne est pleine, la suivante commence **à droite**. Le texte de chaque case est **à sa droite**, comme sur une
//! feuille d'e-konte. C'est l'image dans un miroir de `board::layout_with` (les rangées de gauche à droite, le texte dessous) : le même
//! résultat (`BoardLayout`), donc les mêmes clics, glissés, aimants et exports.
//!
//! - **La colonne** : sa hauteur est celle d'une rangée de la planche en lignes, comptée en cases (le réglage « cases par ligne » devient
//!   « cases par colonne », six d'origine : six cases et cinq écarts de coupe) ; une case de plus passe en haut de la colonne suivante,
//!   jamais coupée. Chaque case garde au-dessus d'elle la ligne de son numéro de plan (`LABEL_H`).
//! - **Le texte** d'une case est dans une bande de la largeur d'une case, à `note_gap` (12 % + 8 points) de sa droite, en haut de l'image ;
//!   **une ligne par champ coché** (trois au plus) ; sans champ coché il n'y a pas de bande et la colonne n'est large que d'une case. La case
//!   suivante de la colonne est sous l'image, ou sous le texte s'il descend plus bas.
//! - **Un mouvement de caméra** : l'enveloppe des cadres est posée comme sur la planche en lignes, à l'échelle des cases ; le texte est à
//!   droite du **cadre le plus à droite** (B…) et de l'enveloppe à cette hauteur ; **la colonne suivante est entièrement à droite** du
//!   mouvement et de son texte (la colonne s'élargit d'autant, comme la rangée descend sous B et ses notes), et la case suivante de la
//!   colonne entièrement sous l'enveloppe et son texte. Une enveloppe qui monte pousse sa case plus bas, sans toucher les cases d'avant ;
//!   **le cadre le plus à gauche s'aligne sur le bord de la colonne** (le miroir de « le cadre le plus haut s'aligne sur la rangée ») : c'est
//!   A qui est en retrait, à droite (`CardPlace::ligne` est la case au bord de la colonne). Au bord haut de la planche, tout glisse d'autant
//!   (`BoardLayout::decalage`, la vue compense).
//! - **Pendant un geste de caméra**, la case A reste où elle était (`Reserve::pose` et `pose_y`), l'enveloppe ne rétrécit pas (`Reserve::rect`) :
//!   au lâcher, tout se range d'après les cadres.
//! - **Les cases pâles** suivent le même sens : au moins `pale_rows` colonnes de cases en tout (trois d'origine), et toujours une colonne
//!   pâle après la dernière colonne occupée.
//! - Les barres entre deux cases d'une colonne sont **horizontales** (grises dans un plan, noires à une coupe).

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

use crate::board::{
    gap_cut, gap_same, note_gap, plan_boards, BoardLayout, BoardSettings, CadreBoard, CardPlace, CutBar, MoveBoard, Options, TuileBoard, BAR_CUT_W, BAR_FRACTION, BAR_W, CARD_MAX, CARD_MIN,
    LABEL_H, MARGIN, MAX_COLUMNS, NOTE_LINE_H, PAGE_PAD,
};
use crate::boardsens::Sens;
use crate::document::Document;
use crate::vignettecam::{self, Empreinte, Place};

/// La largeur de la bande de texte, d'une largeur de case (celle de la bande sous la case en lignes : les lignes se coupent au même endroit).
pub fn largeur_texte(card_w: f32) -> f32 {
    card_w
}

/// La hauteur d'une colonne : `colonnes` cases (avec leur ligne de numéro) et leurs écarts de coupe.
pub fn hauteur_colonne(card_w: f32, thumb_h: f32, colonnes: usize) -> f32 {
    colonnes as f32 * (LABEL_H + thumb_h) + colonnes.saturating_sub(1) as f32 * gap_cut(card_w)
}

/// Ce qu'une case prend, relativement au coin haut-gauche de son image (A pour un mouvement de caméra).
struct Etendue {
    /// Jusqu'où l'enveloppe va à gauche (négatif ou nul).
    x_lo: f32,
    /// Le retrait de A à droite du bord gauche de la colonne : **le cadre le plus à gauche du mouvement s'aligne sur ce bord**, comme n'importe
    /// quelle case (Anthony, 2026-10-24, le miroir de « le cadre le plus haut s'aligne sur le haut de la rangée »). Nul sans cadre à gauche de A.
    dec: f32,
    /// Le bord droit de tout ce que la case prend, **depuis le bord gauche de la colonne** : le retrait, l'image, l'enveloppe, le texte.
    droite: f32,
    /// De combien l'enveloppe monte au-dessus de l'image (positif ou nul).
    monte: f32,
    /// Le bord bas de tout ce que la case prend : l'image, l'enveloppe, le texte.
    bas: f32,
    /// La bande de texte.
    note: Rect,
}

/// La planche lue à la japonaise (voir le début du fichier). Mêmes arguments que `board::layout_with`.
pub fn layout(doc: &Document, s: &BoardSettings, last_joins: bool, opt: &Options, indices: &[usize]) -> BoardLayout {
    let cols = s.columns.clamp(1, MAX_COLUMNS);
    let card_w = if s.card_w.is_nan() { BoardSettings::default().card_w } else { s.card_w.clamp(CARD_MIN, CARD_MAX) };
    let thumb_h = card_w * doc.frame[1] as f32 / doc.frame[0].max(1) as f32;
    let mut boards: Vec<(usize, u64, usize, usize)> = Vec::new();
    for &i in indices.iter().filter(|&&i| i < doc.panels.len()) {
        boards.extend(plan_boards(doc, i).into_iter().map(|(id, frame, end)| (i, id, frame, end)));
    }
    let n = boards.len();
    // Les lignes de texte à droite de chaque case : une par champ coché (zéro : pas de zone de texte).
    let n_l = opt.textes.lignes();
    let joined_after = |k: usize| match boards.get(k + 1) {
        Some(next) => next.2 > 0,
        None => last_joins,
    };
    let (g_same, g_cut) = (gap_same(card_w), gap_cut(card_w));
    let ng = note_gap(card_w);
    let gap_after = |k: usize| if joined_after(k) { g_same } else { g_cut };
    let texte_w = if n_l == 0 { 0.0 } else { largeur_texte(card_w) };
    let bloc = LABEL_H + thumb_h;
    let col_h = hauteur_colonne(card_w, thumb_h, cols);
    let col_gap = g_cut;
    // (Sans champ coché : une colonne n'est large que d'une case.)
    let col_nominal = card_w + if n_l == 0 { 0.0 } else { ng + texte_w };
    let ng_texte = if n_l == 0 { 0.0 } else { ng };
    // Une empreinte n'est jamais plus haute qu'une colonne (sa case et la ligne de son numéro comprises), ni plus large que trois.
    let haut_max = (col_h - LABEL_H).max(thumb_h);
    let large_max = 3.0 * col_nominal + 2.0 * col_gap;

    // Les empreintes (la première image d'un plan dont la caméra bouge), à l'échelle des cases.
    let place = Place { card_w, thumb_h, row_w: large_max, total: (haut_max - thumb_h).max(0.0), max_h: haut_max };
    let prints: Vec<Option<Empreinte>> = boards.iter().map(|b| vignettecam::empreinte_de_la_case(doc, b.0, b.2, &place)).collect();
    // L'enveloppe (élargie à ce que le geste en cours a déjà couvert : elle ne rétrécit pas tant qu'il dure).
    let polys: Vec<Option<Vec<Pos2>>> = prints
        .iter()
        .enumerate()
        .map(|(k, e)| {
            let e = e.as_ref()?;
            let hold = opt.hold.filter(|h| h.plan == doc.panels[boards[k].0].id && h.image == boards[k].2);
            Some(match hold {
                None => e.enveloppe.clone(),
                Some(h) => {
                    let mut pts = e.enveloppe.clone();
                    pts.extend(vignettecam::coins_de(h.rect));
                    let tenue = vignettecam::enveloppe_convexe(&pts);
                    // (Jamais plus haute qu'une colonne : sinon la case ne tiendrait nulle part.)
                    if vignettecam::boite(&tenue).height() > haut_max { e.enveloppe.clone() } else { tenue }
                }
            })
        })
        .collect();

    // Ce que prend chaque case.
    let etendues: Vec<Etendue> = (0..n)
        .map(|k| {
            let hauteur_texte = n_l as f32 * NOTE_LINE_H;
            match (&prints[k], &polys[k]) {
                (Some(e), Some(poly)) => {
                    let boite = vignettecam::boite(poly);
                    // Le texte est à droite du cadre le plus à droite, et de l'enveloppe à sa hauteur, en haut de ce cadre.
                    let b = e.cadres.iter().max_by(|a, b| a.rect.right().total_cmp(&b.rect.right())).map_or(Rect::from_min_size(Pos2::ZERO, vec2(card_w, thumb_h)), |c| c.rect);
                    let droite_env = crate::viewbar::clip_to_rect(poly, Rect::from_min_max(pos2(-1e6, b.top()), pos2(1e6, b.bottom()))).iter().map(|q| q.x).fold(b.right(), f32::max);
                    let note = Rect::from_min_size(pos2(droite_env + ng_texte, b.top()), vec2(texte_w, hauteur_texte));
                    // (Pendant un geste, A garde le retrait qu'elle avait à l'appui : elle ne bouge pas, `Reserve::pose_dec`.)
                    let dec = opt.hold.filter(|h| h.plan == doc.panels[boards[k].0].id && boards[k].2 == h.image).and_then(|h| h.pose_dec).unwrap_or_else(|| (-boite.left()).max(0.0));
                    Etendue { x_lo: boite.left().min(0.0), dec, droite: dec + boite.right().max(card_w).max(note.right()), monte: (-boite.top()).max(0.0), bas: boite.bottom().max(thumb_h).max(note.bottom()), note }
                }
                _ => {
                    let note = Rect::from_min_size(pos2(card_w + ng_texte, 0.0), vec2(texte_w, hauteur_texte));
                    Etendue { x_lo: 0.0, dec: 0.0, droite: note.right(), monte: 0.0, bas: thumb_h.max(note.bottom()), note }
                }
            }
        })
        .collect();

    // Le geste de caméra en cours : la case A du plan reste où elle était (colonne, x, haut de l'image).
    let forces: Vec<Option<(usize, f32, f32)>> = (0..n)
        .map(|k| {
            let h = opt.hold.filter(|h| h.plan == doc.panels[boards[k].0].id && boards[k].2 == h.image)?;
            let (c, x) = h.pose?;
            Some((c, x, h.pose_y?))
        })
        .collect();

    // Le fil des cases, colonne par colonne : (colonne, haut de l'image, bas de ce que la case prend), le haut de l'image compté depuis
    // le haut des colonnes.
    let monte_de = |k: usize, premiere: bool| if premiere { LABEL_H } else { LABEL_H.max(etendues[k].monte) };
    let mut poses: Vec<(usize, f32, f32)> = Vec::with_capacity(n);
    // `marge` : ce que le texte plus haut que l'image a ajouté à la colonne (elle garde ses `cols` cases) ; une enveloppe, elle, déborde
    // sous la colonne sans lui rien ajouter : la case A reste en place (le papier est infini) et les suivantes passent à la colonne d'après.
    let (mut col, mut y, mut compte, mut marge) = (0usize, 0.0f32, 0usize, 0.0f32);
    for k in 0..n {
        let bas = etendues[k].bas;
        let haut_image = if let Some((c, _, hy)) = forces[k] {
            if c != col {
                col = c;
                compte = 0;
                marge = 0.0;
            }
            hy
        } else {
            let tient = compte == 0 || (compte < cols && y + monte_de(k, false) + thumb_h <= col_h + marge + 1e-3);
            if !tient {
                col += 1;
                y = 0.0;
                compte = 0;
                marge = 0.0;
            }
            y + monte_de(k, compte == 0)
        };
        poses.push((col, haut_image, haut_image + bas));
        compte += 1;
        if prints[k].is_none() {
            marge += (bas - thumb_h).max(0.0);
        }
        y = haut_image + bas + gap_after(k);
    }
    let occupees = poses.iter().map(|p| p.0).max().map_or(1, |m| m + 1);

    // Les cases pâles : ce qui reste de la colonne courante et les colonnes suivantes, au moins `pale_rows` en tout et une colonne après
    // la dernière occupée.
    let mut pale: Vec<(usize, f32)> = Vec::new();
    let total_colonnes = match opt.pale_rows {
        Some(min_colonnes) => {
            let total = min_colonnes.max(occupees + 1).max(col + 1);
            let (mut c, mut yy, mut cnt, mut slack) = (col, y, compte, marge);
            for _ in 0..total.saturating_mul(cols + 1).saturating_add(1) {
                if opt.pale_max.is_some_and(|m| pale.len() >= m) {
                    break;
                }
                if cnt >= cols || (cnt > 0 && yy + bloc > col_h + slack + 1e-3) {
                    c += 1;
                    yy = 0.0;
                    cnt = 0;
                    slack = 0.0;
                }
                if c >= total {
                    break;
                }
                pale.push((c, yy + LABEL_H));
                cnt += 1;
                yy += bloc + g_cut;
            }
            // (Un nombre de cases pâles limité : seules les colonnes des cases comptent.)
            if opt.pale_max.is_some() {
                occupees.max(col + 1).max(pale.last().map_or(0, |p| p.0 + 1))
            } else {
                total
            }
        }
        None => occupees,
    };

    // Les colonnes : leur x (depuis le bord gauche des colonnes), leur largeur (tout ce que leurs cases prennent, texte compris). Une colonne
    // commence un écart de coupe après la précédente : l'enveloppe d'un mouvement ne mord jamais sur elle, c'est A qui est en retrait pour que
    // le cadre le plus à gauche s'aligne sur le bord de la colonne (`Etendue::dec`) ; celle où la case A est tenue garde la place de A.
    let mut droite_col = vec![card_w; total_colonnes];
    let mut x_tenu: Vec<Option<f32>> = vec![None; total_colonnes];
    for (k, p) in poses.iter().enumerate() {
        droite_col[p.0] = droite_col[p.0].max(etendues[k].droite);
        if let Some((c, x, _)) = forces[k] {
            if let Some(t) = x_tenu.get_mut(c) {
                *t = Some(x - etendues[k].dec);
            }
        }
    }
    for c in occupees..total_colonnes {
        droite_col[c] = col_nominal;
    }
    let mut tx = vec![0.0f32; total_colonnes];
    for c in 0..total_colonnes {
        tx[c] = match x_tenu[c] {
            Some(x) => x,
            None if c == 0 => 0.0,
            None => tx[c - 1] + droite_col[c - 1] + col_gap,
        };
    }

    // Une enveloppe qui dépasse le bord gauche ou haut de la planche : tout glisse d'autant (la vue compense, la case A ne bouge pas à l'écran).
    // (A gauche, seulement pendant un geste : au repos, le cadre le plus à gauche est au bord de la colonne.)
    let mut decalage = Vec2::ZERO;
    for (k, p) in poses.iter().enumerate() {
        if p.0 == 0 {
            decalage.x = decalage.x.max(-(PAGE_PAD + tx[0] + etendues[k].dec + etendues[k].x_lo));
        }
        decalage.y = decalage.y.max(-(PAGE_PAD + p.1 - etendues[k].monte));
    }
    let (origine_x, origine_y) = (MARGIN + PAGE_PAD + decalage.x, MARGIN + PAGE_PAD + decalage.y);

    let mut out = BoardLayout { decalage, origine_x, origine_y, sens: Sens::Japonais, ..Default::default() };
    for (k, &(c, haut_image, bas)) in poses.iter().enumerate() {
        // Le bord gauche de la colonne, et A (en retrait de `dec` à sa droite quand un cadre du mouvement va à gauche d'elle).
        let colonne_x = origine_x + tx[c];
        let x = colonne_x + etendues[k].dec;
        let top = origine_y + haut_image;
        let thumb = Rect::from_min_size(pos2(x, top), vec2(card_w, thumb_h));
        let (i, id, frame, end) = boards[k];
        let p = &doc.panels[i];
        let mut foot = thumb;
        let mut mv = None;
        let note = etendues[k].note.translate(vec2(x, top));
        if let Some(e) = &prints[k] {
            let au = vec2(x, top);
            let cadres: Vec<CadreBoard> = e.cadres.iter().map(|c| CadreBoard { rang: c.rang, rect: c.rect.translate(au), coins: c.coins.map(|q| q + au), reduit: c.reduit }).collect();
            let tuile = TuileBoard { rect: e.tuile.rect.translate(au), zone: e.tuile.zone, masque: e.tuile.masque.clone() };
            foot = foot.union(tuile.rect);
            mv = Some(out.moves.len());
            out.moves.push(MoveBoard { cadres, enveloppe: e.enveloppe.iter().map(|q| *q + au).collect(), tuile, balayage: e.balayage.iter().map(|p| p.iter().map(|q| *q + au).collect()).collect() });
        }
        let card = Rect::from_min_max(pos2(x, top - LABEL_H), pos2(x + card_w, top + thumb_h));
        let reach = colonne_x + droite_col[c];
        let reach_y = origine_y + bas;
        // La case à la ligne : au bord gauche de la colonne, à la hauteur de A. Les barres et les repères de pose s'y alignent.
        let ligne = Rect::from_min_size(pos2(colonne_x, top), vec2(card_w, thumb_h));
        out.cards.push(CardPlace { panel: i, plan: p.id, id, frame, end, scene: p.scene, card, thumb, note, row: c, mouvement: mv.is_some(), foot, reach, reach_y, mv, ligne });
        let gap = gap_after(k);
        let zone = Rect::from_min_max(pos2(colonne_x, reach_y), pos2(colonne_x + card_w, reach_y + gap));
        let cut = !joined_after(k);
        let bar = Rect::from_center_size(pos2(ligne.center().x, reach_y + gap * 0.5), vec2(card_w * BAR_FRACTION, if cut { BAR_CUT_W } else { BAR_W }));
        out.bars.push(CutBar { after: k, cut, bar, zone });
    }
    for &(c, haut_image) in &pale {
        out.empty.push(Rect::from_min_size(pos2(origine_x + tx[c], origine_y + haut_image), vec2(card_w, thumb_h)));
    }
    // Une seule grande page, à la taille de ce que les cases prennent (les exports) ; la planche, elle, va jusqu'aux dernières cases pâles.
    let mut contenu = Rect::NOTHING;
    for c in &out.cards {
        contenu = contenu.union(c.card).union(c.foot).union(c.note);
    }
    let page = Rect::from_min_max(pos2(MARGIN, MARGIN), pos2(contenu.right() + PAGE_PAD, contenu.bottom() + PAGE_PAD));
    let mut size = vec2(page.right() + MARGIN, page.bottom() + MARGIN);
    for e in &out.empty {
        size = size.max(vec2(e.right() + PAGE_PAD + MARGIN, e.bottom() + PAGE_PAD + MARGIN));
    }
    out.pages = vec![page];
    out.size = size;
    out
}
