//! Hilfslinien beim Ziehen: bündige Kanten und Mitten als feine Linien, Abstände zu den
//! Nachbarn als Maßlinien mit cm-Angabe.

use egui::{Color32, Painter, Shape};

use super::canvas::View2;
use super::*;
use crate::wall::{Line, Measure};

pub(super) fn draw(p: &Painter, v: View2, lines: &[Line], dists: &[Measure]) {
    let pt = |x: f32, y: f32| pos2(v.wr.left() + x * v.s, v.wr.top() + y * v.s);
    let stroke = Stroke::new(1.0, accent());
    for l in lines {
        let (a, b) = if l.upright {
            (pt(l.at, l.from), pt(l.at, l.to))
        } else {
            (pt(l.from, l.at), pt(l.to, l.at))
        };
        p.extend(Shape::dashed_line(&[a, b], stroke, 5.0, 3.0));
    }
    for d in dists {
        let (a, b) = (pt(d.a.0, d.a.1), pt(d.b.0, d.b.1));
        if a.distance(b) < 6.0 {
            continue;
        }
        p.line_segment([a, b], stroke);
        // kleine Endstriche quer zur Linie
        let n = (b - a).normalized().rot90() * 4.0;
        p.line_segment([a - n, a + n], stroke);
        p.line_segment([b - n, b + n], stroke);
        let label = format!("{} cm", fmt_cm(d.cm));
        let f = font(ts::CAPTION, W::Semibold);
        let w = label.chars().count() as f32 * 6.5 + sp::S;
        let r = Rect::from_center_size(a.lerp(b, 0.5), vec2(w, 16.0));
        p.rect_filled(r, 4, accent());
        p.text(r.center(), Align2::CENTER_CENTER, label, f, Color32::WHITE);
    }
}
