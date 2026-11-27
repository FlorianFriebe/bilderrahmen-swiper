//! Größe an der Ecke ziehen: das ausgewählte Foto springt auf die nächste Druckgröße,
//! die gegenüberliegende Ecke bleibt stehen. Griffe an den Ecken und Vorschau mit Größe.

use egui::{Color32, Painter};

use super::canvas::View2;
use super::piece::draw_piece;
use super::*;
use crate::wall::{Piece, PrintSize};

/// Griff an der Ecke in Bildschirmpunkten.
const HANDLE: f32 = 8.0;
/// So nah muss der Zeiger an der Ecke sein.
const REACH: f32 = 10.0;

/// Ecke des ausgewählten Fotos unter dem Zeiger: (Platz, rechts?, unten?).
pub(super) fn corner_at(app: &SwiperApp, v: View2, at: Pos2) -> Option<(usize, bool, bool)> {
    if app.wall_ui.multi.len() >= 2
        || app.wall_ui.focus.is_some()
        || app.wall_ui.step != Step::Fotos
    {
        return None;
    }
    let i = app.wall_ui.sel?;
    let r = v.px(app.wall().pieces.get(i)?.area(app.sizes()));
    if r.width() < 3.0 * REACH || r.height() < 3.0 * REACH {
        return None;
    }
    for (right, bottom) in [(false, false), (true, false), (false, true), (true, true)] {
        let c = pos2(
            if right { r.right() } else { r.left() },
            if bottom { r.bottom() } else { r.top() },
        );
        if c.distance(at) <= REACH {
            return Some((i, right, bottom));
        }
    }
    None
}

/// Nächste Druckgröße für ein Foto, dessen Ecke bei (`x`, `y`) cm liegen soll.
pub fn resized(
    pc: &Piece,
    sizes: &[PrintSize],
    right: bool,
    bottom: bool,
    x: f32,
    y: f32,
) -> Piece {
    let a = pc.area(sizes);
    // diese Ecke bleibt stehen
    let fx = if right { a.x } else { a.right() };
    let fy = if bottom { a.y } else { a.bottom() };
    let (dw, dh) = ((x - fx).abs(), (y - fy).abs());
    let best = sizes
        .iter()
        .map(|s| {
            let q = Piece {
                size: s.name.clone(),
                ..pc.clone()
            };
            let (w, h) = q.dims(sizes);
            ((w - dw).powi(2) + (h - dh).powi(2), q)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, q)| q);
    let Some(mut q) = best else {
        return pc.clone();
    };
    let (w, h) = q.dims(sizes);
    q.x = if right { fx } else { fx - w };
    q.y = if bottom { fy } else { fy - h };
    q
}

/// Kleine Griffe an den vier Ecken des ausgewählten Fotos.
pub(super) fn handles(p: &Painter, r: Rect) {
    if r.width() < 3.0 * REACH || r.height() < 3.0 * REACH {
        return;
    }
    for c in [
        r.left_top(),
        r.right_top(),
        r.left_bottom(),
        r.right_bottom(),
    ] {
        let h = Rect::from_center_size(c, vec2(HANDLE, HANDLE));
        p.rect_filled(h, 2, Color32::WHITE);
        p.rect_stroke(h, 2, Stroke::new(1.5, accent()), StrokeKind::Inside);
    }
}

/// Vorschau beim Ziehen an der Ecke: das Foto in der neuen Größe und die Größe daneben.
pub(super) fn preview(app: &SwiperApp, p: &Painter, v: View2, q: &Piece) {
    let r = v.px(q.area(app.sizes()));
    draw_piece(app, p, r, q, false, true);
    p.rect_stroke(r, 2, Stroke::new(2.0, accent()), StrokeKind::Outside);
    let f = font(ts::CAPTION, W::Semibold);
    let label = format!("{} {}", q.size, if q.hoch { "hochkant" } else { "quer" });
    let at = r.center_bottom() + vec2(0.0, sp::S);
    let g = p.layout_no_wrap(label, f, Color32::WHITE);
    let bg = Rect::from_center_size(
        at + vec2(0.0, g.size().y / 2.0 + 2.0),
        g.size() + vec2(12.0, 6.0),
    );
    p.rect_filled(bg, radius::S, SCRIM);
    p.galley(bg.center() - g.size() / 2.0, g, Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wall::default_sizes;

    #[test]
    fn corner_snaps_to_the_nearest_print_size() {
        let s = default_sizes();
        let pc = Piece {
            size: "10x15".into(),
            x: 10.0,
            y: 10.0,
            ..Default::default()
        };
        // rechte untere Ecke weit nach außen: größer, links oben bleibt
        let q = resized(&pc, &s, true, true, 10.0 + 29.0, 10.0 + 21.0);
        assert_eq!(q.size, "20x30");
        assert_eq!((q.x, q.y), (10.0, 10.0));
        // linke obere Ecke: rechts unten bleibt stehen
        let q = resized(&pc, &s, false, false, 10.0, 10.0);
        let a = pc.area(&s);
        let b = q.area(&s);
        assert!((a.right() - b.right()).abs() < 0.01 && (a.bottom() - b.bottom()).abs() < 0.01);
    }
}
