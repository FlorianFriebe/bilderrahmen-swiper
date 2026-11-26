//! Die Wand selbst: maßstabsgetreu zeichnen, Lineal, Plätze und Hindernisse ziehen,
//! Fotos aus dem Band ablegen, tauschen und abnehmen.

use egui::{Color32, CursorIcon, Painter, Shape};

use super::*;
use crate::ui::image_cover;
use crate::wall::{Area, Obstacle, Piece};

/// Platz für das Lineal oben und links.
const RULER: f32 = 22.0;
/// Einrasten in Bildschirmpunkten (wird in cm umgerechnet).
const SNAP_PX: f32 = 8.0;

/// Wand-Rechteck auf dem Bildschirm und Umrechnungen.
#[derive(Clone, Copy)]
struct View2 {
    wr: Rect,
    s: f32,
}

impl View2 {
    fn cm(&self, p: Pos2) -> (f32, f32) {
        (
            (p.x - self.wr.left()) / self.s,
            (p.y - self.wr.top()) / self.s,
        )
    }
    fn px(&self, a: Area) -> Rect {
        Rect::from_min_size(
            self.wr.min + vec2(a.x * self.s, a.y * self.s),
            vec2(a.w * self.s, a.h * self.s),
        )
    }
}

fn view_of(app: &SwiperApp) -> Option<View2> {
    app.wall_ui.wall_rect.map(|(wr, s)| View2 { wr, s })
}

/// Oberster Platz, dann Hindernis, dann freie Wand unter dem Punkt (cm).
fn hit_test(app: &SwiperApp, x: f32, y: f32, skip: Option<usize>) -> Option<Hit> {
    let w = app.wall();
    let sizes = app.sizes();
    if let Some(i) = (0..w.pieces.len())
        .rev()
        .filter(|i| Some(*i) != skip)
        .find(|i| w.pieces[*i].area(sizes).contains(x, y))
    {
        return Some(Hit::Piece(i));
    }
    if let Some(i) = w.obstacles.iter().rposition(|o| o.area.contains(x, y)) {
        return Some(Hit::Obstacle(i));
    }
    (x >= 0.0 && y >= 0.0 && x <= w.w && y <= w.h).then_some(Hit::Wall(x, y))
}

/// Zielposition (cm, linke obere Ecke) für einen gezogenen Platz.
fn piece_target(app: &SwiperApp, v: View2, idx: usize, grab: (f32, f32), at: Pos2) -> (f32, f32) {
    let (cx, cy) = v.cm(at);
    let w = app.wall();
    let (pw, ph) = w.pieces[idx].dims(app.sizes());
    let x = (cx - grab.0).clamp(-pw * 0.5, w.w - pw * 0.5);
    let y = (cy - grab.1).clamp(-ph * 0.5, w.h - ph * 0.5);
    wl::snap(w, app.sizes(), idx, x, y, SNAP_PX / v.s)
}

/// Platz, mit dem beim Loslassen getauscht würde (Zeiger im inneren Bereich eines anderen).
fn swap_target(app: &SwiperApp, v: View2, idx: usize, at: Pos2) -> Option<usize> {
    let (x, y) = v.cm(at);
    match hit_test(app, x, y, Some(idx))? {
        Hit::Piece(j) => {
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

/// Neuer Platz für ein Foto aus dem Band, mittig unter dem Zeiger.
fn new_piece(app: &SwiperApp, v: View2, photo: &std::path::Path, at: Pos2) -> Piece {
    let w = app.wall();
    let hoch = w
        .photos
        .iter()
        .find(|p| p.path == photo)
        .is_some_and(|p| p.hoch());
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

pub(super) fn show(app: &mut SwiperApp, ui: &mut Ui, area: Rect) {
    let (ww, wh) = (app.wall().w.max(1.0), app.wall().h.max(1.0));
    let avail = Rect::from_min_max(area.min + vec2(RULER, RULER), area.max);
    let s = (avail.width() / ww).min(avail.height() / wh).max(0.01);
    let wr = Rect::from_center_size(avail.center(), vec2(ww * s, wh * s));
    let v = View2 { wr, s };
    app.wall_ui.wall_rect = Some((wr, s));
    let p = ui.painter().clone();

    // Wand
    p.rect(
        wr,
        radius::S / 2,
        field(),
        Stroke::new(1.0, divider()),
        StrokeKind::Outside,
    );
    ruler(&p, wr, s, ww, wh);

    let resp = ui.interact(
        wr.expand(6.0),
        ui.make_persistent_id("wall_canvas"),
        Sense::click_and_drag(),
    );
    let hover = ui.input(|i| i.pointer.hover_pos());
    let hit = hover.and_then(|h| {
        let (x, y) = v.cm(h);
        hit_test(app, x, y, None)
    });

    // Ziehen beginnen
    if resp.drag_started() && app.wall_ui.drag.is_none() {
        if let Some(o) = ui.input(|i| i.pointer.press_origin()) {
            let (x, y) = v.cm(o);
            app.wall_ui.menu = None;
            app.wall_ui.drag = if app.wall_ui.draw {
                Some(Drag::NewObstacle((x.clamp(0.0, ww), y.clamp(0.0, wh))))
            } else {
                match hit_test(app, x, y, None) {
                    Some(Hit::Piece(i)) => {
                        let (px, py) = (app.wall().pieces[i].x, app.wall().pieces[i].y);
                        app.wall_ui.sel = Some(i);
                        Some(Drag::Piece {
                            idx: i,
                            grab: (x - px, y - py),
                        })
                    }
                    Some(Hit::Obstacle(i)) => {
                        let a = app.wall().obstacles[i].area;
                        Some(Drag::Obstacle {
                            idx: i,
                            grab: (x - a.x, y - a.y),
                        })
                    }
                    _ => None,
                }
            };
        }
    }

    // Hindernisse
    for (i, o) in app.wall().obstacles.iter().enumerate() {
        let moving = matches!(app.wall_ui.drag, Some(Drag::Obstacle { idx, .. }) if idx == i);
        let r = v.px(o.area);
        let col = if moving { accent() } else { muted() };
        obstacle(&p, r, &o.label, col, hit == Some(Hit::Obstacle(i)));
    }

    // Plätze
    let sizes = app.sizes().to_vec();
    let n = app.wall().pieces.len();
    let dragging = match &app.wall_ui.drag {
        Some(Drag::Piece { idx, .. }) => Some(*idx),
        _ => None,
    };
    let pointer = ui.input(|i| i.pointer.latest_pos());
    let swap = match (&app.wall_ui.drag, pointer) {
        (Some(Drag::Piece { idx, .. }), Some(at)) => swap_target(app, v, *idx, at),
        (Some(Drag::Photo(_)), Some(at)) => match hit_test(app, v.cm(at).0, v.cm(at).1, None) {
            Some(Hit::Piece(j)) => Some(j),
            _ => None,
        },
        _ => None,
    };
    for i in 0..n {
        let w = app.wall();
        let pc = w.pieces[i].clone();
        let r = v.px(pc.area(&sizes));
        let cov = wl::covered(&w.pieces, &sizes, i);
        let blocked = wl::blocked(w, &sizes, &pc);
        let faded = dragging == Some(i);
        draw_piece(app, ui, &p, r, &pc, faded, cov < 0.2);
        let mut ring: Option<Color32> = None;
        if blocked {
            ring = Some(skip());
        } else if cov >= wl::COVER_WARN && pc.photo.is_some() {
            ring = Some(warn());
        }
        if app.wall_ui.sel == Some(i) || swap == Some(i) || hit == Some(Hit::Piece(i)) {
            ring = Some(accent());
        }
        if let Some(c) = ring {
            p.rect_stroke(r, 2, Stroke::new(2.0, c), StrokeKind::Outside);
        }
        if cov >= wl::COVER_WARN && pc.photo.is_some() && r.width() > 44.0 {
            let label = format!("{} %", (cov * 100.0).round() as u32);
            let cr = Rect::from_min_size(r.min + vec2(3.0, 3.0), vec2(38.0, 16.0));
            p.rect_filled(cr, 4, SCRIM);
            p.text(
                cr.center(),
                Align2::CENTER_CENTER,
                label,
                font(ts::CAPTION, W::Semibold),
                warn(),
            );
        }
        if swap == Some(i) {
            p.rect_filled(r, 2, accent_soft());
            kit::icon(&p, r.center(), icons::SWAP, 20.0, text());
        }
    }

    // Vorschau beim Ziehen
    if let (Some(drag), Some(at)) = (app.wall_ui.drag.clone(), pointer) {
        match drag {
            Drag::Piece { idx, grab } if swap.is_none() && !over_strip(app, at) => {
                let (x, y) = piece_target(app, v, idx, grab, at);
                let pc = Piece {
                    x,
                    y,
                    ..app.wall().pieces[idx].clone()
                };
                let r = v.px(pc.area(&sizes));
                draw_piece(app, ui, &p, r, &pc, false, true);
                p.rect_stroke(r, 2, Stroke::new(2.0, accent()), StrokeKind::Outside);
            }
            Drag::Photo(path) if swap.is_none() && wr.contains(at) => {
                let pc = new_piece(app, v, &path, at);
                let r = v.px(pc.area(&sizes));
                draw_piece(app, ui, &p, r, &pc, false, true);
                p.rect_stroke(r, 2, Stroke::new(2.0, accent()), StrokeKind::Outside);
            }
            Drag::Obstacle { idx, grab } => {
                let (x, y) = v.cm(at);
                let a = app.wall().obstacles[idx].area;
                let na = Area {
                    x: (x - grab.0).clamp(0.0, ww - a.w),
                    y: (y - grab.1).clamp(0.0, wh - a.h),
                    ..a
                };
                p.rect_stroke(v.px(na), 2, Stroke::new(2.0, accent()), StrokeKind::Inside);
            }
            Drag::NewObstacle(start) => {
                let a = drawn_area(start, v.cm(at), ww, wh);
                obstacle(&p, v.px(a), "", accent(), true);
                size_badge(
                    &p,
                    v.px(a),
                    &format!("{} × {} cm", fmt_cm(a.w), fmt_cm(a.h)),
                );
            }
            _ => {}
        }
    }

    // Klicks, Menü, Mauszeiger
    if app.wall_ui.drag.is_none() {
        if app.wall_ui.draw && resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        } else if matches!(hit, Some(Hit::Piece(_) | Hit::Obstacle(_))) {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
        if let Some(Hit::Piece(i)) = hit {
            let w = app.wall();
            let pc = &w.pieces[i];
            let name = pc
                .photo
                .as_deref()
                .map(crate::folders::file_name)
                .unwrap_or_else(|| "Leerer Platz".into());
            let cov = wl::covered(&w.pieces, &sizes, i);
            let mut tip = format!(
                "{name}\n{} {}",
                pc.size,
                if pc.hoch { "hochkant" } else { "quer" }
            );
            if cov > 0.01 {
                tip.push_str(&format!(" · {} % verdeckt", (cov * 100.0).round()));
            }
            if wl::blocked(w, &sizes, pc) {
                tip.push_str("\nLiegt auf einem Hindernis oder ragt über die Wand.");
            }
            tip.push_str(
                "\nZiehen: verschieben · auf ein anderes Foto: tauschen · Rechtsklick: mehr",
            );
            if pc
                .photo
                .as_deref()
                .is_some_and(|f| app.original_of(f).is_some())
            {
                tip.push_str("\nDoppelklick: Zuschnitt ändern");
            }
            let _ = resp.clone().on_hover_text(tip);
        }
    } else {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    }
    if resp.clicked() && !app.wall_ui.draw {
        app.wall_ui.menu = None;
        app.wall_ui.sel = match hit {
            Some(Hit::Piece(i)) => Some(i),
            _ => None,
        };
    }
    if resp.double_clicked() {
        if let Some(Hit::Piece(i)) = hit {
            match app.wall().pieces[i].photo.clone() {
                Some(f) => {
                    if app.original_of(&f).is_some() {
                        app.wall_reedit(&f);
                    }
                }
                None => fill_empty(app, i),
            }
        }
    }
    if resp.secondary_clicked() {
        if let (Some(h), Some(at)) = (hit, hover) {
            app.wall_ui.menu = Some((h, at));
            if let Hit::Piece(i) = h {
                app.wall_ui.sel = Some(i);
            }
        }
    }
}

/// Leerer Platz bekommt das nächste Foto aus dem Band.
fn fill_empty(app: &mut SwiperApp, i: usize) {
    if app.wall().pieces[i].photo.is_some() {
        return;
    }
    let next = wl::unplaced(app.wall()).first().map(|p| p.path.clone());
    if let Some(f) = next {
        app.wall_checkpoint();
        app.wall_mut().pieces[i].photo = Some(f);
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
pub(super) fn finish_drag(app: &mut SwiperApp, ui: &Ui) {
    if app.wall_ui.drag.is_none() {
        return;
    }
    let (released, down, at) = ui.input(|i| {
        (
            i.pointer.any_released(),
            i.pointer.primary_down(),
            i.pointer.latest_pos(),
        )
    });
    if !released && down {
        return;
    }
    let Some(drag) = app.wall_ui.drag.take() else {
        return;
    };
    let (Some(v), Some(at)) = (view_of(app), at) else {
        return;
    };
    let (ww, wh) = (app.wall().w, app.wall().h);
    match drag {
        Drag::Piece { idx, grab } => {
            if idx >= app.wall().pieces.len() {
                return;
            }
            if over_strip(app, at) {
                if app.wall().pieces[idx].photo.is_some() {
                    app.wall_checkpoint();
                    app.wall_mut().pieces[idx].photo = None;
                }
            } else if let Some(j) = swap_target(app, v, idx, at) {
                app.wall_checkpoint();
                let w = app.wall_mut();
                let a = w.pieces[idx].photo.take();
                let b = w.pieces[j].photo.take();
                w.pieces[idx].photo = b;
                w.pieces[j].photo = a;
            } else {
                let (x, y) = piece_target(app, v, idx, grab, at);
                let pc = &app.wall().pieces[idx];
                if (pc.x - x).abs() < 0.01 && (pc.y - y).abs() < 0.01 {
                    return;
                }
                app.wall_checkpoint();
                let w = app.wall_mut();
                let mut pc = w.pieces.remove(idx);
                pc.x = x;
                pc.y = y;
                // verschobene Fotos liegen oben
                w.pieces.push(pc);
                w.manual = true;
                app.wall_ui.sel = Some(app.wall().pieces.len() - 1);
            }
        }
        Drag::Photo(path) => {
            if !v.wr.contains(at) {
                return;
            }
            let (x, y) = v.cm(at);
            app.wall_checkpoint();
            match hit_test(app, x, y, None) {
                Some(Hit::Piece(j)) => app.wall_mut().pieces[j].photo = Some(path),
                _ => {
                    let pc = new_piece(app, v, &path, at);
                    let w = app.wall_mut();
                    w.pieces.push(pc);
                    w.manual = true;
                    let idx = w.pieces.len() - 1;
                    let (x, y) = (w.pieces[idx].x, w.pieces[idx].y);
                    let (x, y) = wl::snap(app.wall(), app.sizes(), idx, x, y, SNAP_PX / v.s);
                    let w = app.wall_mut();
                    w.pieces[idx].x = x;
                    w.pieces[idx].y = y;
                    app.wall_ui.sel = Some(idx);
                }
            }
        }
        Drag::Obstacle { idx, grab } => {
            let (x, y) = v.cm(at);
            app.wall_checkpoint();
            if let Some(o) = app.wall_mut().obstacles.get_mut(idx) {
                o.area.x = (x - grab.0).clamp(0.0, (ww - o.area.w).max(0.0));
                o.area.y = (y - grab.1).clamp(0.0, (wh - o.area.h).max(0.0));
            }
            app.wall_settings_changed();
        }
        Drag::NewObstacle(start) => {
            let a = drawn_area(start, v.cm(at), ww, wh);
            if a.w < 2.0 || a.h < 2.0 {
                return;
            }
            app.wall_checkpoint();
            app.wall_mut().obstacles.push(Obstacle {
                area: a,
                label: String::new(),
            });
            app.wall_ui.draw = false;
            app.wall_settings_changed();
            app.toast(
                "Hindernis eingezeichnet. Rechtsklick darauf: benennen oder entfernen.".into(),
            );
        }
    }
}

/// `label`: Größe auf leere Plätze schreiben (nicht, wenn sie großteils verdeckt sind).
fn draw_piece(
    app: &SwiperApp,
    ui: &Ui,
    p: &Painter,
    r: Rect,
    pc: &Piece,
    faded: bool,
    label: bool,
) {
    let tint = if faded {
        Color32::WHITE.gamma_multiply(0.3)
    } else {
        Color32::WHITE
    };
    if !faded {
        kit::shadow(p, r, 2, 10, 2, shadow());
    }
    let thumb = pc.photo.as_deref().and_then(|f| app.thumb(f));
    match (&pc.photo, thumb) {
        (Some(_), Some((tex, size))) => image_cover(ui, r, 2, tex, size, tint),
        (Some(_), None) => {
            p.rect_filled(r, 2, control());
        }
        (None, _) => {
            // deckend, damit überlappende leere Plätze wie gestapeltes Papier wirken
            let fill = kit::mix(bg(), text(), 0.07);
            p.rect_filled(
                r,
                2,
                if faded {
                    fill.gamma_multiply(0.3)
                } else {
                    fill
                },
            );
            dashed_rect(p, r, muted().gamma_multiply(if faded { 0.3 } else { 0.8 }));
            if r.width() > 34.0 && r.height() > 20.0 && label {
                p.text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    &pc.size,
                    font(ts::CAPTION, W::Medium),
                    muted(),
                );
            }
        }
    }
}

fn dashed_rect(p: &Painter, r: Rect, col: Color32) {
    let pts = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
        r.left_top(),
    ];
    p.extend(Shape::dashed_line(&pts, Stroke::new(1.0, col), 4.0, 3.0));
}

fn obstacle(p: &Painter, r: Rect, label: &str, col: Color32, hot: bool) {
    p.rect_filled(r, 3, if hot { control() } else { divider() });
    dashed_rect(p, r, col);
    // feine Schraffur, damit es nicht wie ein Foto aussieht
    let step = 10.0;
    let mut d = step;
    let stroke = Stroke::new(1.0, col.gamma_multiply(0.35));
    while d < r.width() + r.height() {
        let a = pos2(
            r.left() + d.min(r.width()),
            r.top() + (d - r.width()).max(0.0),
        );
        let b = pos2(
            r.left() + (d - r.height()).max(0.0),
            r.top() + d.min(r.height()),
        );
        p.line_segment([a, b], stroke);
        d += step;
    }
    if !label.is_empty() && r.width() > 40.0 {
        size_badge(p, r, label);
    }
}

fn size_badge(p: &Painter, r: Rect, label: &str) {
    let f = font(ts::CAPTION, W::Semibold);
    let w = label.chars().count() as f32 * 6.5 + sp::M;
    let br = Rect::from_center_size(r.center(), vec2(w, 18.0));
    p.rect_filled(br, 4, SCRIM);
    p.text(br.center(), Align2::CENTER_CENTER, label, f, Color32::WHITE);
}

/// Lineal oben und links in cm.
fn ruler(p: &Painter, wr: Rect, s: f32, ww: f32, wh: f32) {
    let step = [5.0, 10.0, 20.0, 25.0, 50.0, 100.0, 200.0]
        .into_iter()
        .find(|st| st * s >= 44.0)
        .unwrap_or(500.0);
    let col = muted();
    let f = font(ts::CAPTION, W::Regular);
    let stroke = Stroke::new(1.0, divider());
    let mut c = 0.0;
    while c <= ww + 0.01 {
        let x = wr.left() + c * s;
        p.line_segment([pos2(x, wr.top() - 6.0), pos2(x, wr.top() - 1.0)], stroke);
        let label = if c == 0.0 { "0 cm".into() } else { fmt_cm(c) };
        p.text(
            pos2(x, wr.top() - 8.0),
            Align2::CENTER_BOTTOM,
            label,
            f.clone(),
            col,
        );
        c += step;
    }
    let mut c = step;
    while c <= wh + 0.01 {
        let y = wr.top() + c * s;
        p.line_segment([pos2(wr.left() - 6.0, y), pos2(wr.left() - 1.0, y)], stroke);
        p.text(
            pos2(wr.left() - 8.0, y),
            Align2::RIGHT_CENTER,
            fmt_cm(c),
            f.clone(),
            col,
        );
        c += step;
    }
}
