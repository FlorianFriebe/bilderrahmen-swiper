//! Eigene Hilfslinien: aus dem Lineal oben (waagrecht) oder links (senkrecht) in die Collage
//! ziehen. Fotos rasten daran ein. Eine Linie lässt sich verschieben; zurück aufs Lineal
//! gezogen verschwindet sie.

use egui::{Painter, Shape};

use super::canvas::{View2, RULER};
use super::*;

/// Wie nah (Bildschirmpunkte) der Zeiger an einer Linie sein muss, um sie zu fassen.
const GRAB_PX: f32 = 4.0;

/// Alle eigenen Hilfslinien zeichnen.
pub(super) fn draw(app: &SwiperApp, p: &Painter, v: View2, skip: Option<(bool, usize)>) {
    let g = &app.wall().guides;
    for (k, x) in g.x.iter().enumerate() {
        if skip != Some((true, k)) {
            draw_one(p, v, true, *x, false);
        }
    }
    for (k, y) in g.y.iter().enumerate() {
        if skip != Some((false, k)) {
            draw_one(p, v, false, *y, false);
        }
    }
}

/// Eine Linie bei `at` cm; `active` beim Ziehen mit Maß.
pub(super) fn draw_one(p: &Painter, v: View2, upright: bool, at: f32, active: bool) {
    let col = if active { accent() } else { warn() };
    let stroke = Stroke::new(1.0, col);
    let (a, b) = if upright {
        let x = v.wr.left() + at * v.s;
        (pos2(x, v.wr.top()), pos2(x, v.wr.bottom()))
    } else {
        let y = v.wr.top() + at * v.s;
        (pos2(v.wr.left(), y), pos2(v.wr.right(), y))
    };
    p.extend(Shape::dashed_line(&[a, b], stroke, 6.0, 3.0));
    if active {
        let label = format!("{} cm", fmt_cm(wl::round_mm(at)));
        let f = font(ts::CAPTION, W::Semibold);
        let w = label.chars().count() as f32 * 6.5 + sp::S;
        let c = if upright {
            pos2(a.x, a.y - 10.0)
        } else {
            pos2(a.x - w / 2.0 - 2.0, a.y)
        };
        let r = Rect::from_center_size(c, vec2(w, 16.0));
        p.rect_filled(r, 4, accent());
        p.text(r.center(), Align2::CENTER_CENTER, label, f, bg());
    }
}

/// Liegt `at` auf dem Lineal? `Some(true)` = linkes Lineal (senkrechte Linie),
/// `Some(false)` = oberes Lineal (waagrechte Linie).
pub(super) fn on_ruler(v: View2, at: Pos2) -> Option<bool> {
    let top = Rect::from_min_max(
        pos2(v.wr.left(), v.wr.top() - RULER),
        pos2(v.wr.right(), v.wr.top()),
    );
    let left = Rect::from_min_max(
        pos2(v.wr.left() - RULER, v.wr.top()),
        pos2(v.wr.left(), v.wr.bottom()),
    );
    if top.contains(at) {
        Some(false)
    } else if left.contains(at) {
        Some(true)
    } else {
        None
    }
}

/// Eigene Linie unter dem Zeiger: (senkrecht?, Nummer).
pub(super) fn guide_at(app: &SwiperApp, v: View2, at: Pos2) -> Option<(bool, usize)> {
    if !v.wr.contains(at) {
        return None;
    }
    let g = &app.wall().guides;
    let near_x =
        g.x.iter()
            .position(|x| (v.wr.left() + x * v.s - at.x).abs() <= GRAB_PX);
    if let Some(k) = near_x {
        return Some((true, k));
    }
    g.y.iter()
        .position(|y| (v.wr.top() + y * v.s - at.y).abs() <= GRAB_PX)
        .map(|k| (false, k))
}
