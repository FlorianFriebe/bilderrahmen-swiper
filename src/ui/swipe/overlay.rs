//! Hilfslinien, Foto-Infos, Behalten/Weg-Stempel und Text-Vorschau auf der Karte.

use super::*;

pub const GUIDES: [&str; 7] = [
    "aus",
    "Drittel",
    "Goldener Schnitt",
    "Raster",
    "Diagonale",
    "Goldenes Dreieck",
    "Goldene Spirale",
];

/// Hilfslinien wie in Lightroom über dem Ausschnitt `r`.
/// `flip`: Bit 0 spiegelt waagrecht, Bit 1 senkrecht (für Dreieck, Spirale, Diagonale).
pub(super) fn guides(painter: &Painter, r: Rect, kind: u8, flip: u8, tr: &dyn Fn(Pos2) -> Pos2) {
    if kind == 0 || r.width() < 4.0 {
        return;
    }
    let st = Stroke::new(1.0, Color32::from_white_alpha(90));
    let fx = flip & 1 != 0;
    let fy = flip & 2 != 0;
    // normierte Koordinaten (0..1) auf die Fläche, ggf. gespiegelt
    let at = |u: f32, v: f32| {
        let u = if fx { 1.0 - u } else { u };
        let v = if fy { 1.0 - v } else { v };
        tr(pos2(r.left() + u * r.width(), r.top() + v * r.height()))
    };
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([at(a.0, a.1), at(b.0, b.1)], st);
    };
    let (w, h) = (r.width(), r.height());
    match kind {
        1 | 2 => {
            let k = if kind == 1 {
                1.0 / 3.0
            } else {
                2.0 - (1.0 + 5f32.sqrt()) / 2.0
            };
            for t in [k, 1.0 - k] {
                line((t, 0.0), (t, 1.0));
                line((0.0, t), (1.0, t));
            }
        }
        3 => {
            let cell = w.min(h) / 6.0;
            let st = Stroke::new(1.0, Color32::from_white_alpha(60));
            let mut x = r.left() + (w % cell) / 2.0;
            while x <= r.right() {
                painter.line_segment([tr(pos2(x, r.top())), tr(pos2(x, r.bottom()))], st);
                x += cell;
            }
            let mut y = r.top() + (h % cell) / 2.0;
            while y <= r.bottom() {
                painter.line_segment([tr(pos2(r.left(), y)), tr(pos2(r.right(), y))], st);
                y += cell;
            }
        }
        4 => {
            // 45°-Linien aus allen vier Ecken
            let s = w.min(h);
            let (du, dv) = (s / w, s / h);
            line((0.0, 0.0), (du, dv));
            line((1.0, 0.0), (1.0 - du, dv));
            line((0.0, 1.0), (du, 1.0 - dv));
            line((1.0, 1.0), (1.0 - du, 1.0 - dv));
        }
        5 => {
            // Diagonale und die Lote der beiden anderen Ecken darauf
            line((0.0, 0.0), (1.0, 1.0));
            let foot = |px: f32, py: f32| {
                let (dx, dy) = (w, h);
                let t = (px * w * dx + py * h * dy) / (dx * dx + dy * dy);
                (t, t)
            };
            line((1.0, 0.0), foot(1.0, 0.0));
            line((0.0, 1.0), foot(0.0, 1.0));
        }
        _ => {
            // goldene Spirale aus Viertelkreisen, auf die Fläche gestreckt
            let phi = (1.0 + 5f32.sqrt()) / 2.0;
            let wide = w >= h;
            let map = |u: f32, v: f32| {
                // u: 0..phi entlang der langen Seite, v: 0..1 entlang der kurzen
                if wide {
                    (u / phi, v)
                } else {
                    (v, u / phi)
                }
            };
            let (mut rx, mut ry, mut rw, mut rh) = (0.0f32, 0.0f32, phi, 1.0f32);
            let mut pts = vec![];
            for i in 0..12 {
                // Quadrat abtrennen (links, oben, rechts, unten), darin ein Viertelkreis
                let (c, a0, s) = match i % 4 {
                    0 => {
                        let s = rh;
                        let c = (rx + s, ry + s);
                        rx += s;
                        rw -= s;
                        (c, 180.0f32, s)
                    }
                    1 => {
                        let s = rw;
                        let c = (rx, ry + s);
                        ry += s;
                        rh -= s;
                        (c, 270.0, s)
                    }
                    2 => {
                        let s = rh;
                        let c = (rx + rw - s, ry);
                        rw -= s;
                        (c, 0.0, s)
                    }
                    _ => {
                        let s = rw;
                        let c = (rx + s, ry + rh - s);
                        rh -= s;
                        (c, 90.0, s)
                    }
                };
                for k in 0..=16 {
                    if k == 0 && !pts.is_empty() {
                        continue;
                    }
                    let a = (a0 + 90.0 * k as f32 / 16.0).to_radians();
                    pts.push((c.0 + s * a.cos(), c.1 + s * a.sin()));
                }
            }
            let pts: Vec<Pos2> = pts
                .into_iter()
                .map(|(u, v)| {
                    let (a, b) = map(u, v);
                    at(a, b)
                })
                .collect();
            painter.add(egui::Shape::line(pts, st));
        }
    }
}

/// Dunkler Chip auf dem Foto, dreht sich mit der Karte.
#[allow(clippy::too_many_arguments)]
pub(super) fn photo_chip(
    painter: &Painter,
    ui: &Ui,
    anchor: Pos2,
    right_aligned: bool,
    glyph: &str,
    label: &str,
    tr: &dyn Fn(Pos2) -> Pos2,
    angle: f32,
) {
    let f = font(ts::SMALL, W::Medium);
    let lw = kit::text_size(ui, label, f.clone()).x;
    let w = sp::M + 14.0 + sp::S + lw + sp::M;
    let h = 32.0;
    let min = if right_aligned {
        anchor - vec2(w, 0.0)
    } else {
        anchor
    };
    let r = Rect::from_min_size(min, vec2(w, h));
    let c = tr(r.center());
    painter.add(
        RectShape::new(
            Rect::from_center_size(c, r.size()),
            radius::S + 2,
            Color32::from_black_alpha(130),
            Stroke::new(1.0, Color32::from_white_alpha(30)),
            StrokeKind::Inside,
        )
        .with_angle(angle),
    );
    let rot = Rot2::from_angle(angle);
    let ig = painter.layout_no_wrap(glyph.to_string(), icon_font(14.0), Color32::WHITE);
    let ip = c + rot * vec2(-w / 2.0 + sp::M, -ig.size().y / 2.0);
    painter.add(Shape::Text(
        TextShape::new(ip, ig, Color32::WHITE).with_angle(angle),
    ));
    let g = painter.layout_no_wrap(label.to_string(), f, Color32::WHITE);
    let tp = c + rot * vec2(-w / 2.0 + sp::M + 14.0 + sp::S, -g.size().y / 2.0);
    painter.add(Shape::Text(
        TextShape::new(tp, g, Color32::WHITE).with_angle(angle),
    ));
}

/// Beim Ziehen: leuchtender Kartenrand (grün = behalten, rot = weg) und
/// ein kleines rundes Symbol in der oberen Ecke.
pub(super) fn stamp(painter: &Painter, dx: f32, card: Rect, angle: f32, tr: &dyn Fn(Pos2) -> Pos2) {
    if dx.abs() <= 6.0 {
        return;
    }
    let keep = dx > 0.0;
    let a = ((dx.abs() - 6.0) / 150.0).clamp(0.0, 1.0);
    let base = if keep { GREEN } else { RED };
    let c = tr(card.center());
    let rad = radius::CARD as f32;
    // äußeres Leuchten: mehrere weiche Ringe
    for k in 1..=12 {
        let f = k as f32 / 12.0;
        let grow = f * 26.0;
        let alpha = (1.0 - f).powi(2) * 0.45 * a;
        painter.add(
            RectShape::stroke(
                Rect::from_center_size(c, card.size() + egui::Vec2::splat(grow * 2.0)),
                (rad + grow) as u8,
                Stroke::new(3.0, base.gamma_multiply(alpha)),
                StrokeKind::Middle,
            )
            .with_angle(angle),
        );
    }
    // inneres Leuchten
    for k in 0..8 {
        let f = k as f32 / 8.0;
        let inset = f * 22.0;
        let alpha = (1.0 - f).powi(2) * 0.24 * a;
        painter.add(
            RectShape::stroke(
                Rect::from_center_size(c, card.size() - egui::Vec2::splat(inset * 2.0)),
                (rad - inset).max(4.0) as u8,
                Stroke::new(3.0, base.gamma_multiply(alpha)),
                StrokeKind::Middle,
            )
            .with_angle(angle),
        );
    }
    // klare Kante
    painter.add(
        RectShape::stroke(
            Rect::from_center_size(c, card.size()),
            radius::CARD,
            Stroke::new(2.0, base.gamma_multiply(0.9 * a)),
            StrokeKind::Inside,
        )
        .with_angle(angle),
    );
    // rundes Symbol oben in der Ecke, zur Zugrichtung hin
    let r = 28.0 * (0.8 + 0.2 * a);
    let at = if keep {
        pos2(card.right() - sp::XL - 28.0, card.top() + 72.0 + 28.0)
    } else {
        pos2(card.left() + sp::XL + 28.0, card.top() + 72.0 + 28.0)
    };
    let at = tr(at);
    painter.circle_filled(
        at + vec2(0.0, 6.0),
        r + 4.0,
        Color32::from_black_alpha((60.0 * a) as u8),
    );
    painter.circle_filled(at, r, base.gamma_multiply(0.92 * a));
    painter.circle_stroke(
        at,
        r,
        Stroke::new(1.0, Color32::from_white_alpha((70.0 * a) as u8)),
    );
    let glyph = if keep { icons::HEART } else { icons::X };
    let f = if keep {
        icon_fill_font(r * 0.95)
    } else {
        icon_font(r * 0.95)
    };
    painter.text(
        at,
        Align2::CENTER_CENTER,
        glyph,
        f,
        Color32::WHITE.gamma_multiply(a),
    );
}

/// Text so auf die Vorschau zeichnen, wie er später im Bild steht.
pub(super) fn preview_overlay(
    painter: &Painter,
    o: &imaging::Overlay,
    r: Rect,
    tr: &dyn Fn(Pos2) -> Pos2,
    angle: f32,
) {
    let lines = o.shown_lines();
    if lines.is_empty() {
        return;
    }
    let look = &o.look;
    let (px, margin, line_h) = o.metrics(r.width(), r.height());
    let px = px.max(6.0);
    let [cr, cg, cb] = look.color;
    let galleys: Vec<_> = lines
        .into_iter()
        .map(|l| painter.layout_no_wrap(l, overlay_ui_font(px, look.font), Color32::WHITE))
        .collect();
    let block_w = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let block_h =
        line_h * (galleys.len() as f32 - 1.0) + galleys.last().map(|g| g.size().y).unwrap_or(0.0);
    let boxed = look.bg >= 2;
    let pad = if boxed { px * 0.5 } else { 0.0 };
    let y0 = match look.row() {
        0 => r.top() + margin + pad,
        1 => r.center().y - block_h / 2.0,
        _ => r.bottom() - margin - pad - block_h,
    };
    let x_for = |w: f32| match look.col() {
        0 => r.left() + margin + pad,
        1 => r.center().x - w / 2.0,
        _ => r.right() - margin - pad - w,
    };
    let light = (cr as u32 + cg as u32 + cb as u32) > 380;
    let shade = |a: u8| {
        if light {
            Color32::from_black_alpha(a)
        } else {
            Color32::from_white_alpha(a)
        }
    };
    if boxed {
        let br = Rect::from_min_size(
            pos2(x_for(block_w) - pad, y0 - pad * 0.8),
            vec2(block_w + pad * 2.0, block_h + pad * 1.6),
        );
        let rr = Rect::from_center_size(tr(br.center()), br.size());
        let rad = (pad * 0.9) as u8;
        if look.bg == 3 {
            painter.add(
                RectShape::new(
                    rr,
                    rad,
                    shade(80),
                    Stroke::new(1.0, Color32::from_white_alpha(46)),
                    StrokeKind::Inside,
                )
                .with_angle(angle),
            );
        } else {
            painter.add(RectShape::filled(rr, rad, shade(115)).with_angle(angle));
        }
    }
    let alpha = look.opacity.clamp(0.0, 1.0);
    let fg = Color32::from_rgba_unmultiplied(cr, cg, cb, (255.0 * alpha) as u8);
    for (i, g) in galleys.into_iter().enumerate() {
        let p = pos2(x_for(g.size().x), y0 + i as f32 * line_h);
        if look.bg == 1 {
            let off = (px * 0.06).max(1.0);
            painter.add(Shape::Text(
                TextShape::new(tr(p + vec2(off, off)), g.clone(), Color32::WHITE)
                    .with_override_text_color(shade((150.0 * alpha) as u8))
                    .with_angle(angle),
            ));
        }
        painter.add(Shape::Text(
            TextShape::new(tr(p), g, Color32::WHITE)
                .with_override_text_color(fg)
                .with_angle(angle),
        ));
    }
}
