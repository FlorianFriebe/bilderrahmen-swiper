//! Ziehen auf der Wand: Vorschau beim Ziehen und Loslassen (verschieben und festpinnen,
//! tauschen, Foto ablegen oder abnehmen, Hindernis setzen).

use egui::Painter;

use super::canvas::{drawn_obstacle, hit_test, view_of, View2};
use super::piece::draw_piece;
use super::*;
use super::{guides, resize};
use crate::wall::{Area, Obstacle, Piece};

/// Einrasten in Bildschirmpunkten (wird in cm umgerechnet).
const SNAP_PX: f32 = 8.0;
/// So lange (Sekunden) über einem anderen Foto halten, dann wird getauscht.
const HOLD_S: f64 = 0.6;

/// Zielposition (cm, linke obere Ecke) für einen gezogenen Platz.
fn piece_target(app: &SwiperApp, v: View2, idx: usize, grab: (f32, f32), at: Pos2) -> (f32, f32) {
    let (cx, cy) = v.cm(at);
    let w = app.wall();
    let (pw, ph) = w.pieces[idx].dims(app.sizes());
    let x = (cx - grab.0).clamp(-pw * 0.5, w.w - pw * 0.5);
    let y = (cy - grab.1).clamp(-ph * 0.5, w.h - ph * 0.5);
    // mit Strg ganz frei, sonst sanft an Kanten, Mitten und Hilfslinien einrasten
    let tol = if app.wall_ui.ctrl { 0.0 } else { SNAP_PX / v.s };
    wl::snap(w, app.sizes(), idx, x, y, tol)
}

/// Plätze, die gerade mitgezogen werden (bei Mehrfachauswahl alle ausgewählten).
pub(super) fn moving(app: &SwiperApp) -> Vec<usize> {
    match &app.wall_ui.drag {
        Some(Drag::Piece { idx, .. }) if app.wall_ui.multi.contains(idx) => app.wall_selected(),
        Some(Drag::Piece { idx, .. } | Drag::Resize { idx, .. }) => vec![*idx],
        _ => vec![],
    }
}

/// Foto unter dem Zeiger im inneren Bereich (die Mitte), außer `idx`.
fn inner_hit(app: &SwiperApp, v: View2, idx: usize, at: Pos2) -> Option<usize> {
    let (x, y) = v.cm(at);
    match hit_test(app, x, y, Some(idx))? {
        Hit::Piece(j) if app.wall().pieces[j].photo.is_some() => {
            let a = app.wall().pieces[j].area(app.sizes());
            let inner = Area {
                x: a.x + a.w * 0.25,
                y: a.y + a.h * 0.25,
                w: a.w * 0.5,
                h: a.h * 0.5,
            };
            inner.contains(x, y).then_some(j)
        }
        _ => None,
    }
}

/// Beim Ziehen merken, über welchem Foto der Zeiger seit wann steht (Tauschen durch Halten).
pub(super) fn update_hold(app: &mut SwiperApp, v: View2, at: Option<Pos2>, now: f64) {
    let over = match (&app.wall_ui.drag, at) {
        (Some(Drag::Piece { idx, .. }), Some(at)) if !app.wall_ui.multi.contains(idx) => {
            inner_hit(app, v, *idx, at)
        }
        _ => None,
    };
    app.wall_ui.hold = match (over, app.wall_ui.hold) {
        (Some(j), Some((k, t))) if j == k => Some((k, t)),
        (Some(j), _) => Some((j, now)),
        _ => None,
    };
}

/// Platz, mit dem beim Loslassen getauscht würde: mit Alt sofort, sonst nach kurzem Halten
/// über der Mitte eines anderen Fotos. Bei Mehrfachauswahl wird nie getauscht.
fn swap_target(app: &SwiperApp, v: View2, idx: usize, at: Pos2, now: f64) -> Option<usize> {
    if app.wall_ui.multi.contains(&idx) {
        return None;
    }
    let j = inner_hit(app, v, idx, at)?;
    let held = app
        .wall_ui
        .hold
        .is_some_and(|(k, t)| k == j && now - t >= HOLD_S);
    (swaps(app) || held).then_some(j)
}

/// Tauscht das Ablegen auf einem Foto sofort? Nur mit Alt (sonst erst nach kurzem Halten).
fn swaps(app: &SwiperApp) -> bool {
    app.wall_ui.alt
}

/// Foto aus dem Band, das auf Platz `j` gelegt würde: leere Plätze immer, sonst wie `swaps`.
fn put_target(app: &SwiperApp, v: View2, at: Pos2) -> Option<usize> {
    let (x, y) = v.cm(at);
    match hit_test(app, x, y, None) {
        Some(Hit::Piece(j)) if swaps(app) || app.wall().pieces[j].photo.is_none() => Some(j),
        _ => None,
    }
}

/// Platz, der beim Loslassen getauscht oder belegt würde (für die Markierung).
pub(super) fn swap_hint(
    app: &SwiperApp,
    v: View2,
    pointer: Option<Pos2>,
    now: f64,
) -> Option<usize> {
    let at = pointer?;
    match &app.wall_ui.drag {
        Some(Drag::Piece { idx, .. }) => swap_target(app, v, *idx, at, now),
        Some(Drag::Photo(paths)) if paths.len() == 1 => put_target(app, v, at),
        _ => None,
    }
}

/// Neuer Platz für ein Foto aus dem Band, mittig unter dem Zeiger.
fn new_piece(app: &SwiperApp, v: View2, photo: &std::path::Path, at: Pos2) -> Piece {
    let w = app.wall();
    let hoch = match w.facing {
        wl::Facing::Quer => false,
        wl::Facing::Hoch => true,
        wl::Facing::Auto => w
            .photos
            .iter()
            .find(|p| p.path == photo)
            .is_some_and(|p| p.hoch()),
    };
    let mut p = Piece {
        photo: Some(photo.to_path_buf()),
        size: w.size.clone(),
        hoch,
        ..Default::default()
    };
    let (pw, ph) = p.dims(app.sizes());
    let (cx, cy) = v.cm(at);
    p.x = cx - pw / 2.0;
    p.y = cy - ph / 2.0;
    p
}

/// Was beim Loslassen passieren würde, schon beim Ziehen zeigen.
pub(super) fn preview(
    app: &SwiperApp,
    p: &Painter,
    v: View2,
    pointer: Option<Pos2>,
    swap: Option<usize>,
) {
    let (Some(drag), Some(at)) = (app.wall_ui.drag.clone(), pointer) else {
        return;
    };
    let sizes = app.sizes();
    let (ww, wh) = (app.wall().w, app.wall().h);
    match drag {
        Drag::Piece { idx, grab } if swap.is_none() && !over_strip(app, at) => {
            let (x, y) = piece_target(app, v, idx, grab, at);
            let w = app.wall();
            let (dx, dy) = (x - w.pieces[idx].x, y - w.pieces[idx].y);
            let all = moving(app);
            for &i in &all {
                let pc = Piece {
                    x: w.pieces[i].x + dx,
                    y: w.pieces[i].y + dy,
                    ..w.pieces[i].clone()
                };
                let r = v.px(pc.area(sizes));
                draw_piece(app, p, r, &pc, false, true);
                p.rect_stroke(r, 2, Stroke::new(2.0, accent()), StrokeKind::Outside);
            }
            // Hilfslinien und Abstände in cm (am gezogenen Foto)
            let a = Piece {
                x,
                y,
                ..w.pieces[idx].clone()
            }
            .area(sizes);
            let (lines, dists) = wl::guides(w, sizes, a, &all);
            guides::draw(p, v, &lines, &dists);
        }
        Drag::Band(start, _) => {
            let r = v.px(drawn_area(start, v.cm(at), ww, wh));
            p.rect_filled(r, 2, accent_soft());
            p.rect_stroke(r, 2, Stroke::new(1.0, accent()), StrokeKind::Inside);
        }
        Drag::Photo(paths) if swap.is_none() && v.wr.contains(at) && !paths.is_empty() => {
            let pc = new_piece(app, v, &paths[0], at);
            let r = v.px(pc.area(sizes));
            draw_piece(app, p, r, &pc, false, true);
            p.rect_stroke(r, 2, Stroke::new(2.0, accent()), StrokeKind::Outside);
            if paths.len() > 1 {
                // wie viele mitkommen
                let label = format!("+{}", paths.len() - 1);
                let c = r.right_top();
                p.circle_filled(c, 12.0, accent());
                p.text(
                    c,
                    Align2::CENTER_CENTER,
                    label,
                    font(ts::CAPTION, W::Bold),
                    bg(),
                );
            }
        }
        Drag::Guide(upright, _) => {
            let (x, y) = v.cm(at);
            super::rulers::draw_one(p, v, upright, if upright { x } else { y }, true);
        }
        Drag::Obstacle { idx, grab } => {
            let (x, y) = v.cm(at);
            let a = app.wall().obstacles[idx].area;
            let na = Area {
                x: (x - grab.0).clamp(0.0, (ww - a.w).max(0.0)),
                y: (y - grab.1).clamp(0.0, (wh - a.h).max(0.0)),
                ..a
            };
            p.rect_stroke(v.px(na), 2, Stroke::new(2.0, accent()), StrokeKind::Inside);
        }
        Drag::NewObstacle(start) => drawn_obstacle(p, v, drawn_area(start, v.cm(at), ww, wh)),
        Drag::Resize { idx, right, bottom } => {
            if let Some(pc) = app.wall().pieces.get(idx) {
                let (x, y) = v.cm(at);
                let q = resize::resized(pc, sizes, right, bottom, x, y);
                resize::preview(app, p, v, &q);
            }
        }
        _ => {}
    }
}

/// Doppelklick auf einen leeren Platz: das nächste freie Foto aus dem Band kommt darauf.
pub(super) fn fill_empty(app: &mut SwiperApp, i: usize) {
    if app.wall().pieces[i].photo.is_some() {
        return;
    }
    let next = wl::unplaced(app.wall()).first().map(|p| p.path.clone());
    match next {
        Some(f) => app.wall_put_on(i, &f),
        None => app.toast("Alle Fotos hängen schon. Hol dir mehr über „Fotos wählen“.".into()),
    }
}

fn over_strip(app: &SwiperApp, at: Pos2) -> bool {
    app.wall_ui.strip_rect.is_some_and(|r| r.contains(at))
}

fn drawn_area(a: (f32, f32), b: (f32, f32), ww: f32, wh: f32) -> Area {
    let (bx, by) = (b.0.clamp(0.0, ww), b.1.clamp(0.0, wh));
    Area {
        x: a.0.min(bx),
        y: a.1.min(by),
        w: (a.0 - bx).abs(),
        h: (a.1 - by).abs(),
    }
}

/// Loslassen: Platz verschieben oder tauschen, Foto ablegen, Hindernis setzen.
pub(super) fn finish(app: &mut SwiperApp, ui: &Ui) {
    if app.wall_ui.drag.is_none() {
        return;
    }
    let (released, down, at, now) = ui.input(|i| {
        (
            i.pointer.any_released(),
            i.pointer.primary_down() || i.pointer.middle_down(),
            i.pointer.latest_pos(),
            i.time,
        )
    });
    if !released && down {
        return;
    }
    let Some(drag) = app.wall_ui.drag.take() else {
        return;
    };
    app.wall_ui.jump = matches!(drag, Drag::Piece { .. } | Drag::Photo(_));
    let (Some(v), Some(at)) = (view_of(app), at) else {
        return;
    };
    let (ww, wh) = (app.wall().w, app.wall().h);
    match drag {
        Drag::Pan | Drag::Focus(_) => {}
        Drag::Resize { idx, right, bottom } => {
            if let Some(pc) = app.wall().pieces.get(idx).cloned() {
                let (x, y) = v.cm(at);
                let q = resize::resized(&pc, app.sizes(), right, bottom, x, y);
                app.wall_set_piece(idx, q);
            }
        }
        Drag::Band(start, add) => {
            let a = drawn_area(start, v.cm(at), ww, wh);
            if a.w > 0.5 || a.h > 0.5 {
                app.wall_select_area(a, add);
            }
        }
        Drag::Piece { idx, grab } => {
            if idx >= app.wall().pieces.len() {
                return;
            }
            let many = app.wall_ui.multi.contains(&idx);
            if over_strip(app, at) {
                if many {
                    app.wall_take_off_many();
                } else {
                    app.wall_take_off(idx);
                }
            } else if many {
                let (x, y) = piece_target(app, v, idx, grab, at);
                let pc = &app.wall().pieces[idx];
                let (dx, dy) = (x - pc.x, y - pc.y);
                let all = app.wall_selected();
                app.wall_move_many(&all, dx, dy);
            } else if let Some(j) = swap_target(app, v, idx, at, now) {
                app.wall_swap(idx, j);
            } else {
                let (x, y) = piece_target(app, v, idx, grab, at);
                let pc = &app.wall().pieces[idx];
                if (pc.x - x).abs() < 0.01 && (pc.y - y).abs() < 0.01 {
                    return;
                }
                app.wall_move(idx, x, y);
            }
        }
        Drag::Photo(paths) => {
            if !v.wr.contains(at) || paths.is_empty() {
                return;
            }
            app.wall_ui.strip_sel.clear();
            let (x, y) = v.cm(at);
            match put_target(app, v, at).filter(|_| paths.len() == 1) {
                Some(j) => app.wall_put_on(j, &paths[0]),
                None => app.wall_place_many(&paths, (x, y)),
            }
        }
        Drag::Guide(upright, k) => {
            let (x, y) = v.cm(at);
            let val = if upright { x } else { y };
            let max = if upright { ww } else { wh };
            // zurück aufs Lineal oder aus der Fläche gezogen: Linie weg
            let inside = (0.0..=max).contains(&val) && v.wr.contains(at);
            match (k, inside) {
                (None, true) => app.wall_add_guide(upright, wl::round_mm(val)),
                (Some(k), true) => app.wall_set_guide(upright, k, Some(wl::round_mm(val))),
                (Some(k), false) => app.wall_set_guide(upright, k, None),
                (None, false) => {}
            }
        }
        Drag::Obstacle { idx, grab } => {
            let (x, y) = v.cm(at);
            let Some(o) = app.wall().obstacles.get(idx) else {
                return;
            };
            let nx = (x - grab.0).clamp(0.0, (ww - o.area.w).max(0.0));
            let ny = (y - grab.1).clamp(0.0, (wh - o.area.h).max(0.0));
            if (nx - o.area.x).abs() < 0.01 && (ny - o.area.y).abs() < 0.01 {
                return;
            }
            app.wall_checkpoint();
            let o = &mut app.wall_mut().obstacles[idx];
            o.area.x = nx;
            o.area.y = ny;
            app.wall_relayout();
        }
        Drag::NewObstacle(start) => {
            let a = drawn_area(start, v.cm(at), ww, wh);
            if a.w < 2.0 || a.h < 2.0 {
                return;
            }
            app.wall_checkpoint();
            app.wall_mut().obstacles.push(Obstacle {
                area: a,
                ..Default::default()
            });
            app.wall_ui.draw = false;
            app.wall_ui.sel = None;
            app.wall_ui.sel_obstacle = Some(app.wall().obstacles.len() - 1);
            app.wall_relayout();
            app.toast("Hindernis eingezeichnet. Rand und Name stellst du rechts ein.".into());
        }
    }
}
