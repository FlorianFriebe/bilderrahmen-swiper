//! Die Wand selbst: maßstabsgetreu zeichnen (mit Zoom), Lineal, Hindernisse mit Rand, Fotos
//! mit sanftem Gleiten beim Neuordnen, Nummern im Export, Ziehen beginnen, Klicks und Rechtsklick.
//! Je Schritt: Wand = nur Fläche und Hindernisse, Fotos = Fotos bearbeiten, Export = ansehen.

use egui::{Color32, CursorIcon, Painter, Shape};

use super::piece::{badges, draw_focus, draw_piece, glide, piece_tip};
use super::*;
use super::{drag, piece, resize, zoom};
use crate::wall::Area;

/// Platz für das Lineal oben und links.
const RULER: f32 = 22.0;
/// Wand-Rechteck auf dem Bildschirm und Umrechnungen.
#[derive(Clone, Copy)]
pub(super) struct View2 {
    pub wr: Rect,
    pub s: f32,
}

impl View2 {
    pub fn cm(&self, p: Pos2) -> (f32, f32) {
        (
            (p.x - self.wr.left()) / self.s,
            (p.y - self.wr.top()) / self.s,
        )
    }
    pub fn px(&self, a: Area) -> Rect {
        Rect::from_min_size(
            self.wr.min + vec2(a.x * self.s, a.y * self.s),
            vec2(a.w * self.s, a.h * self.s),
        )
    }
}

pub(super) fn view_of(app: &SwiperApp) -> Option<View2> {
    app.wall_ui.wall_rect.map(|(wr, s)| View2 { wr, s })
}

/// Reihenfolge beim Zeichnen: leere Plätze immer unten, Fotos darüber in ihrer Ebene.
pub(super) fn draw_order(pieces: &[crate::wall::Piece]) -> Vec<usize> {
    let (empty, photos): (Vec<usize>, Vec<usize>) =
        (0..pieces.len()).partition(|&i| pieces[i].photo.is_none());
    empty.into_iter().chain(photos).collect()
}

/// Was unter dem Punkt (cm) liegt: im Schritt Wand Hindernisse, sonst der oberste Platz;
/// danach die freie Wand.
pub(super) fn hit_test(app: &SwiperApp, x: f32, y: f32, skip: Option<usize>) -> Option<Hit> {
    let w = app.wall();
    let sizes = app.sizes();
    if app.wall_ui.step == Step::Wand {
        if let Some(i) = w.obstacles.iter().rposition(|o| o.area.contains(x, y)) {
            return Some(Hit::Obstacle(i));
        }
    } else if let Some(i) = draw_order(&w.pieces)
        .into_iter()
        .rev()
        .filter(|i| Some(*i) != skip)
        .find(|i| w.pieces[*i].area(sizes).contains(x, y))
    {
        return Some(Hit::Piece(i));
    }
    (x >= 0.0 && y >= 0.0 && x <= w.w && y <= w.h).then_some(Hit::Wall(x, y))
}

pub(super) fn show(app: &mut SwiperApp, ui: &mut Ui, area: Rect) {
    let (ww, wh) = (app.wall().w.max(1.0), app.wall().h.max(1.0));
    let avail = Rect::from_min_max(area.min + vec2(RULER, RULER), area.max - vec2(sp::S, sp::S));
    let canvas = ui.interact(
        area,
        ui.make_persistent_id("wall_canvas"),
        Sense::click_and_drag(),
    );
    zoom::input(app, ui, area, avail.center());
    let fit = (avail.width() / ww).min(avail.height() / wh).max(0.01);
    let s = fit * app.wall_ui.zoom();
    zoom::clamp(app, area, vec2(ww * s, wh * s));
    let wr = Rect::from_center_size(avail.center() + app.wall_ui.pan, vec2(ww * s, wh * s));
    let v = View2 { wr, s };
    app.wall_ui.wall_rect = Some((wr, s));
    let p = ui.painter().with_clip_rect(area);

    // Wand
    p.rect(
        wr,
        radius::S / 2,
        field(),
        Stroke::new(1.0, divider()),
        StrokeKind::Outside,
    );
    // Rand zur Kante, ganz leise
    let m = app.wall().margin;
    if m > 0.1 && m * 2.0 < ww.min(wh) {
        let inner = v.px(Area {
            x: m,
            y: m,
            w: ww - 2.0 * m,
            h: wh - 2.0 * m,
        });
        dashed_rect(&p, inner, divider());
    }
    ruler(&p, wr, s, ww, wh);

    let resp = canvas;
    let hover = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|h| area.contains(*h));
    let hit = hover.and_then(|h| {
        let (x, y) = v.cm(h);
        hit_test(app, x, y, None)
    });
    let space = ui.input(|i| i.key_down(egui::Key::Space));
    let step = app.wall_ui.step;

    // Ziehen beginnen
    let middle = ui.input(|i| i.pointer.middle_down());
    let shift = ui.input(|i| i.modifiers.shift);
    app.wall_ui.alt = ui.input(|i| i.modifiers.alt);
    let corner = hover.and_then(|h| resize::corner_at(app, v, h));
    if (resp.drag_started() || (middle && resp.hovered())) && app.wall_ui.drag.is_none() {
        if let Some(o) = ui.input(|i| i.pointer.press_origin()) {
            let (x, y) = v.cm(o);
            app.wall_ui.menu = None;
            app.wall_ui.drag = if space || middle || step == Step::Export {
                Some(Drag::Pan)
            } else if app.wall_ui.draw {
                Some(Drag::NewObstacle((x.clamp(0.0, ww), y.clamp(0.0, wh))))
            } else if let Some((idx, right, bottom)) = resize::corner_at(app, v, o) {
                Some(Drag::Resize { idx, right, bottom })
            } else {
                start_drag(app, x, y, shift)
            };
        }
    }
    match app.wall_ui.drag {
        Some(Drag::Pan) => app.wall_ui.pan += ui.input(|i| i.pointer.delta()),
        Some(Drag::Focus(i)) => {
            let d = ui.input(|i| i.pointer.delta());
            piece::focus_drag(app, v, i, d);
        }
        _ => {}
    }

    // Hindernisse mit ihrem Rand (bearbeiten nur im Schritt Wand)
    for (i, o) in app.wall().obstacles.iter().enumerate() {
        let moving = matches!(app.wall_ui.drag, Some(Drag::Obstacle { idx, .. }) if idx == i);
        let picked = app.wall_ui.sel_obstacle == Some(i) && step == Step::Wand;
        if let Some(k) = o.keep_out() {
            if o.margin > 0.05 {
                dashed_rect(&p, v.px(k), muted().gamma_multiply(0.5));
            }
        }
        let r = v.px(o.area);
        let col = if moving || picked { accent() } else { muted() };
        obstacle(
            &p,
            r,
            &o.label,
            col,
            hit == Some(Hit::Obstacle(i)) || picked,
        );
    }

    // Plätze (gleiten beim Neuordnen sanft an ihre neue Stelle); im Schritt Wand keine Fotos
    let sizes = app.sizes().to_vec();
    let n = if step == Step::Wand {
        0
    } else {
        app.wall().pieces.len()
    };
    let nums = if step == Step::Export {
        wl::numbers(app.wall(), &sizes)
    } else {
        vec![]
    };
    let moving = drag::moving(app);
    let jump = std::mem::take(&mut app.wall_ui.jump);
    let pointer = ui.input(|i| i.pointer.latest_pos());
    let swap = drag::swap_hint(app, v, pointer);
    let behind: Vec<Area> = app
        .wall()
        .obstacles
        .iter()
        .filter(|o| o.behind)
        .map(|o| o.area)
        .collect();
    // bündig abgeschnitten: was über den Rand ragt, wird nicht gezeigt (und nicht gedruckt)
    let warn_at = app.wall().cover_warn();
    let trim = app.wall().trims().then(|| v.px(app.wall().inner()));
    let full = p.clone();
    let p = match trim {
        Some(t) => p.with_clip_rect(t.intersect(area)),
        None => p,
    };
    let order = if n == 0 {
        vec![]
    } else {
        draw_order(&app.wall().pieces)
    };
    for i in order {
        let w = app.wall();
        let pc = w.pieces[i].clone();
        let target = pc.area(&sizes);
        let a = glide(ui, &pc, i, target, jump);
        let r = v.px(a);
        let cov = wl::covered(&w.pieces, &sizes, i);
        let blocked = wl::blocked(w, &sizes, &pc);
        let faded = moving.contains(&i);
        draw_piece(app, &p, r, &pc, faded, cov < 0.2);
        let mut ring: Option<Color32> = None;
        let picked = app.wall_ui.sel == Some(i) || app.wall_ui.multi.contains(&i);
        let hot = hit == Some(Hit::Piece(i));
        // Warnungen erst, wenn man das Foto anfasst, und im Export immer
        let show_warn = picked || hot || step == Step::Export;
        if show_warn && blocked {
            ring = Some(skip());
        } else if show_warn && cov >= warn_at && pc.photo.is_some() {
            ring = Some(warn());
        }
        if picked || swap == Some(i) || (hot && step == Step::Fotos) {
            ring = Some(accent());
        }
        if app.wall_ui.swap_from == Some(i) {
            ring = Some(accent());
        }
        if let Some(c) = ring {
            let rr = trim.map_or(r, |t| r.intersect(t));
            full.rect_stroke(rr, 2, Stroke::new(2.0, c), StrokeKind::Outside);
        }
        badges(app, &p, r, &pc, show_warn);

        if swap == Some(i) {
            p.rect_filled(r, 2, accent_soft());
            kit::icon(&p, r.center(), icons::SWAP, 20.0, text());
        }
    }
    // Nummern zuletzt, an einer sichtbaren Ecke jedes Fotos
    for (i, num) in nums.iter().enumerate() {
        if let Some(num) = num {
            let (x, y) = wl::visible_spot(app.wall(), &sizes, i);
            piece::number(&p, v.wr.min + vec2(x * v.s, y * v.s), *num);
        }
    }
    let p = full;
    if let Some(i) = app
        .wall_ui
        .sel
        .filter(|i| *i < n && app.wall_ui.multi.len() < 2 && step == Step::Fotos)
    {
        if app.wall_ui.drag.is_none() && app.wall_ui.focus.is_none() {
            let a = glide(
                ui,
                &app.wall().pieces[i],
                i,
                app.wall().pieces[i].area(&sizes),
                jump,
            );
            resize::handles(&p, v.px(a));
        }
    }
    // Hindernisse, hinter denen Fotos verschwinden dürfen, liegen über den Fotos
    for a in &behind {
        let r = v.px(*a);
        p.rect_filled(r, 3, field().gamma_multiply(0.85));
        dashed_rect(&p, r, muted());
    }

    // Bildausschnitt verschieben: ganzes Foto blass über allem, der Abzug selbst obenauf
    if let Some(i) = app.wall_ui.focus.filter(|i| *i < n) {
        let pc = app.wall().pieces[i].clone();
        let r = v.px(pc.area(&sizes));
        // Rest der Wand abdunkeln, damit nur dieses Foto zählt
        p.rect_filled(v.wr.expand(sp::S), 0, SCRIM);
        draw_focus(app, &p, r, &pc);
        draw_piece(app, &p, r, &pc, false, true);
        p.rect_stroke(r, 2, Stroke::new(2.0, accent()), StrokeKind::Outside);
    }

    drag::preview(app, &p, v, pointer, swap);

    // Klicks, Menü, Mauszeiger
    if app.wall_ui.drag.is_none() {
        if (space || step == Step::Export) && resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        } else if let Some((_, right, bottom)) = corner {
            ui.ctx().set_cursor_icon(resize_cursor(right, bottom));
        } else if hit.is_some() && hit == app.wall_ui.focus.map(Hit::Piece) {
            ui.ctx().set_cursor_icon(CursorIcon::Move);
        } else if app.wall_ui.draw && resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        } else if matches!(hit, Some(Hit::Piece(_) | Hit::Obstacle(_))) {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
        if let Some(Hit::Piece(i)) = hit {
            // Tooltip am Foto selbst, nicht unter der ganzen Wand
            let r = v.px(app.wall().pieces[i].area(&sizes));
            let _ = ui
                .interact(r, ui.make_persistent_id("wall_tip"), Sense::hover())
                .on_hover_text(piece_tip(app, i));
        }
    } else if let Some(Drag::Resize { right, bottom, .. }) = app.wall_ui.drag {
        ui.ctx().set_cursor_icon(resize_cursor(right, bottom));
    } else {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    }
    if resp.clicked() && app.wall_ui.swap_from.is_some() {
        app.wall_ui.menu = None;
        if let (Some(a), Some(Hit::Piece(b))) = (app.wall_ui.swap_from.take(), hit) {
            if a != b {
                app.wall_swap(a, b);
            }
        }
    } else if resp.clicked() && !app.wall_ui.draw {
        app.wall_ui.menu = None;
        app.wall_ui.sel_obstacle = None;
        if app.wall_ui.focus.is_some() && hit != app.wall_ui.focus.map(Hit::Piece) {
            app.wall_ui.focus = None;
        }
        match hit {
            Some(Hit::Piece(i)) if shift => app.wall_select_toggle(i),
            Some(Hit::Piece(i)) => app.wall_select(Some(i)),
            Some(Hit::Obstacle(i)) => {
                app.wall_select(None);
                app.wall_ui.sel_obstacle = Some(i);
            }
            _ => app.wall_select(None),
        }
    }
    if resp.double_clicked() && step == Step::Fotos {
        if let Some(Hit::Piece(i)) = hit {
            if app.wall().pieces[i].photo.is_some() {
                // Bildausschnitt direkt an der Wand verschieben
                app.wall_select(Some(i));
                app.wall_ui.focus = Some(i);
            } else {
                drag::fill_empty(app, i);
            }
        }
    }
    if resp.secondary_clicked() && step != Step::Export {
        if let (Some(h @ (Hit::Piece(_) | Hit::Obstacle(_))), Some(at)) = (hit, hover) {
            app.wall_ui.menu = Some((h, at));
            match h {
                Hit::Piece(i) if app.wall_ui.multi.contains(&i) => {}
                Hit::Piece(i) => app.wall_select(Some(i)),
                Hit::Obstacle(i) => app.wall_ui.sel_obstacle = Some(i),
                _ => {}
            }
        }
    }
}

fn resize_cursor(right: bool, bottom: bool) -> CursorIcon {
    if right == bottom {
        CursorIcon::ResizeNwSe
    } else {
        CursorIcon::ResizeNeSw
    }
}

pub(super) fn dashed_rect(p: &Painter, r: Rect, col: Color32) {
    let pts = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
        r.left_top(),
    ];
    p.extend(Shape::dashed_line(&pts, Stroke::new(1.0, col), 4.0, 3.0));
}

pub(super) fn obstacle(p: &Painter, r: Rect, label: &str, col: Color32, hot: bool) {
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

pub(super) fn size_badge(p: &Painter, r: Rect, label: &str) {
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

/// Neues Hindernis beim Aufziehen, mit Maßen.
pub(super) fn drawn_obstacle(p: &Painter, v: View2, a: Area) {
    obstacle(p, v.px(a), "", accent(), true);
    size_badge(p, v.px(a), &format!("{} × {} cm", fmt_cm(a.w), fmt_cm(a.h)));
}

/// Was ein Ziehen auf der Wand bei (x, y) cm beginnt: Foto (oder alle ausgewählten),
/// Bildausschnitt, Hindernis oder ein Auswahl-Rechteck auf freier Fläche.
fn start_drag(app: &mut SwiperApp, x: f32, y: f32, shift: bool) -> Option<Drag> {
    match hit_test(app, x, y, None) {
        Some(Hit::Piece(i)) if app.wall_ui.focus == Some(i) => {
            app.wall_checkpoint();
            Some(Drag::Focus(i))
        }
        Some(Hit::Piece(i)) => {
            app.wall_ui.focus = None;
            let (px, py) = (app.wall().pieces[i].x, app.wall().pieces[i].y);
            if !app.wall_ui.multi.contains(&i) {
                if shift {
                    app.wall_select_toggle(i);
                } else {
                    app.wall_select(Some(i));
                }
            }
            app.wall_ui.sel = Some(i);
            app.wall_ui.sel_obstacle = None;
            Some(Drag::Piece {
                idx: i,
                grab: (x - px, y - py),
            })
        }
        Some(Hit::Obstacle(i)) => {
            let a = app.wall().obstacles[i].area;
            app.wall_select(None);
            app.wall_ui.sel_obstacle = Some(i);
            Some(Drag::Obstacle {
                idx: i,
                grab: (x - a.x, y - a.y),
            })
        }
        _ if app.wall_ui.step == Step::Fotos => {
            app.wall_ui.focus = None;
            Some(Drag::Band((x, y), shift))
        }
        _ => None,
    }
}
