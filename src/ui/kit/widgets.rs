//! Segmente, Chips, Häkchen, Fortschritt, Meldung (Toast), Quer/Hochkant-Schalter, Text kürzen.

use super::*;

// ------------------------------------------------------------------ Segmente und Chips

/// Umschalter mit gleitender Markierung (z. B. Quer / Hochkant in der Leiste).
pub fn segmented_bare(
    ui: &Ui,
    inner: Rect,
    labels: &[String],
    selected: Option<usize>,
) -> Option<usize> {
    let p = ui.painter();
    let f = font(ts::BODY, W::Medium);
    let widths: Vec<f32> = labels
        .iter()
        .map(|l| text_size(ui, l, f.clone()).x + sp::L * 2.0)
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + sp::XS * (labels.len() as f32 - 1.0);
    let scale = (inner.width() / total).min(1.0);
    let mut x = inner.left();
    let mut rects = vec![];
    for w in &widths {
        rects.push(Rect::from_min_size(
            pos2(x, inner.top()),
            vec2(w * scale, inner.height()),
        ));
        x += w * scale + sp::XS;
    }
    let id = ui.make_persistent_id(("segmented", labels.join("|")));
    if let Some(s) = selected {
        let target = rects[s];
        let lx = ui
            .ctx()
            .animate_value_with_time(id.with("x"), target.left(), motion::NORMAL);
        let lw = ui
            .ctx()
            .animate_value_with_time(id.with("w"), target.width(), motion::NORMAL);
        let hl = Rect::from_min_size(pos2(lx, target.top()), vec2(lw, target.height()));
        shadow(p, hl, radius::M, 8, 2, Color32::from_black_alpha(50));
        p.rect_filled(
            hl,
            radius::M,
            if is_dark() {
                Color32::from_white_alpha(235)
            } else {
                Color32::WHITE
            },
        );
    }
    let mut clicked = None;
    for (i, (l, rr)) in labels.iter().zip(&rects).enumerate() {
        let resp = clickable(ui, *rr, ("seg", l), true);
        let on = selected == Some(i);
        let t = anim(ui, resp.id, resp.hovered() && !on);
        let col = if on {
            Color32::from_rgb(0x11, 0x13, 0x18)
        } else {
            mix(text_soft(), text(), t)
        };
        p.text(rr.center(), Align2::CENTER_CENTER, l, f.clone(), col);
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

pub fn segmented_width(ui: &Ui, labels: &[String]) -> f32 {
    let f = font(ts::BODY, W::Medium);
    labels
        .iter()
        .map(|l| text_size(ui, l, f.clone()).x + sp::L * 2.0)
        .sum::<f32>()
        + sp::XS * (labels.len() as f32 + 1.0)
}

#[derive(Clone, Copy)]
pub enum Tone {
    Keep,
    Warn,
    Accent,
}

pub(super) fn tone_colors(t: Tone) -> (Color32, Color32, Color32) {
    match t {
        Tone::Keep => (keep_soft(), GREEN.gamma_multiply(0.45), keep()),
        Tone::Warn => (warn_soft(), AMBER.gamma_multiply(0.55), warn()),
        Tone::Accent => (accent_soft(), BLUE.gamma_multiply(0.55), accent()),
    }
}

/// Klickbarer Chip mit optionalem Symbol.
pub fn chip(ui: &Ui, r: Rect, glyph: &str, filled_icon: bool, label: &str, tone: Tone) -> Response {
    let resp = clickable(ui, r, ("chip", label), true);
    let t = anim(ui, resp.id, resp.hovered());
    let (fill, stroke, fg) = tone_colors(tone);
    let p = ui.painter();
    shadow(p, r, radius::L, 24, 6, pal::shadow().gamma_multiply(0.7));
    p.rect(
        r,
        radius::L,
        mix(fill, fill.gamma_multiply(1.6), t),
        Stroke::new(1.0, stroke),
        StrokeKind::Inside,
    );
    let f = font(ts::LABEL, W::Semibold);
    let lw = text_size(ui, label, f.clone()).x;
    let gw = if glyph.is_empty() { 0.0 } else { 18.0 + sp::S };
    let x = r.center().x - (lw + gw) / 2.0;
    if !glyph.is_empty() {
        let c = pos2(x + 9.0, r.center().y);
        if filled_icon {
            icon_filled(p, c, glyph, 16.0, fg);
        } else {
            icon(p, c, glyph, 16.0, fg);
        }
    }
    p.text(
        pos2(x + gw, r.center().y),
        Align2::LEFT_CENTER,
        label,
        f,
        fg,
    );
    resp
}

pub fn chip_width(ui: &Ui, glyph: &str, label: &str) -> f32 {
    text_size(ui, label, font(ts::LABEL, W::Semibold)).x
        + if glyph.is_empty() { 0.0 } else { 18.0 + sp::S }
        + sp::L * 2.0
}

/// Häkchen-Kästchen (nur Zeichnung).
pub fn checkmark(p: &Painter, r: Rect, on: bool) {
    if on {
        p.rect_filled(r, 6, BLUE);
        icon(
            p,
            r.center(),
            icons::CHECK,
            r.width() * 0.75,
            Color32::WHITE,
        );
    } else {
        p.rect_stroke(
            r,
            6,
            Stroke::new(1.5, divider().gamma_multiply(2.0)),
            StrokeKind::Inside,
        );
    }
}

/// Dünner Fortschrittsbalken mit Verlauf.
pub fn progress(p: &Painter, r: Rect, f: f32) {
    let rad = (r.height() / 2.0) as u8;
    p.rect_filled(r, rad, divider());
    if f > 0.0 {
        let w = (r.width() * f.clamp(0.0, 1.0)).max(r.height());
        let fr = Rect::from_min_size(r.min, vec2(w, r.height()));
        let mut mesh = egui::Mesh::default();
        let (a, b) = (keep(), accent());
        for (pt, c) in [
            (fr.left_top(), a),
            (fr.right_top(), b),
            (fr.right_bottom(), b),
            (fr.left_bottom(), a),
        ] {
            mesh.colored_vertex(pt, c);
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(mesh);
    }
}

/// Überschrift einer Gruppe im Menü.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(sp::L);
    ui.label(
        egui::RichText::new(title.to_uppercase())
            .font(font(ts::CAPTION, W::Semibold))
            .color(muted())
            .extra_letter_spacing(0.8),
    );
    ui.add_space(sp::XS);
}

/// Ecken eines Rechtecks einzeln runden (für Bänder auf der Karte).
pub fn corners(nw: bool, ne: bool, sw: bool, se: bool, r: u8) -> CornerRadius {
    let c = |b: bool| if b { r } else { 0 };
    CornerRadius {
        nw: c(nw),
        ne: c(ne),
        sw: c(sw),
        se: c(se),
    }
}

/// Kleine Meldung unter der oberen Leiste, optional mit einem Knopf
/// (z. B. „Rückgängig“). Gibt `true` zurück, wenn der Knopf geklickt wurde.
pub fn toast(
    ui: &Ui,
    top_center: Pos2,
    msg: &str,
    alpha: f32,
    glyph: (&str, Color32),
    action: Option<(&str, &str)>,
) -> bool {
    if alpha <= 0.01 {
        return false;
    }
    let f = font(ts::SMALL + 0.5, W::Medium);
    let g = ui.painter().layout(msg.to_string(), f, text(), 520.0);
    let h = (g.size().y + sp::M * 2.0).max(40.0);
    let af = font(ts::SMALL, W::Semibold);
    let action_w = action
        .map(|(label, key)| {
            text_size(ui, label, af.clone()).x
                + 16.0
                + sp::S
                + if key.is_empty() {
                    0.0
                } else {
                    text_size(ui, key, font(10.0, W::Semibold)).x + sp::S
                }
                + sp::M * 2.0
        })
        .unwrap_or(0.0);
    let w = sp::M
        + 16.0
        + sp::S
        + g.size().x
        + sp::L
        + if action.is_some() {
            action_w + sp::XS
        } else {
            0.0
        };
    let r = Rect::from_min_size(top_center - vec2(w / 2.0, (1.0 - alpha) * 8.0), vec2(w, h));
    let mut clicked = false;
    egui::Area::new(Id::unique("toast_area"))
        .order(egui::Order::Tooltip)
        .fixed_pos(r.min)
        .interactable(action.is_some())
        .show(ui.ctx(), |ui| {
            let mut p = ui.painter().clone();
            p.set_opacity(alpha);
            shadow(&p, r, radius::L, 24, 8, pal::shadow());
            p.rect(
                r,
                radius::L,
                panel(),
                Stroke::new(1.0, glass_stroke()),
                StrokeKind::Inside,
            );
            icon(
                &p,
                pos2(r.left() + sp::M + 8.0, r.center().y),
                glyph.0,
                16.0,
                glyph.1,
            );
            p.galley(
                pos2(
                    r.left() + sp::M + 16.0 + sp::S,
                    r.center().y - g.size().y / 2.0,
                ),
                g,
                text(),
            );
            if let Some((label, key)) = action {
                let br = Rect::from_min_size(
                    pos2(r.right() - sp::XS - 2.0 - action_w, r.top() + 5.0),
                    vec2(action_w, h - 10.0),
                );
                let resp = ui
                    .interact(br, Id::unique("toast_action"), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let t = anim(ui, resp.id, resp.hovered());
                p.rect_filled(br, radius::M, mix(field(), glass_hover(), t));
                let x = br.left() + sp::M;
                icon(&p, pos2(x + 8.0, br.center().y), icons::UNDO, 15.0, text());
                p.text(
                    pos2(x + 16.0 + sp::S, br.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    af.clone(),
                    text(),
                );
                if !key.is_empty() {
                    p.text(
                        pos2(br.right() - sp::M, br.center().y),
                        Align2::RIGHT_CENTER,
                        key,
                        font(10.0, W::Semibold),
                        muted(),
                    );
                }
                clicked = resp.clicked();
            }
        });
    clicked
}

/// Bild-Rechteck gedreht malen. egui dreht bei `RectShape` nur die Umrisse, nicht
/// die Textur – deshalb erst ungedreht zerlegen und dann das ganze Netz drehen.
pub fn rotated_image(p: &Painter, shape: RectShape, angle: f32, pivot: Pos2) {
    if angle == 0.0 {
        p.add(shape);
        return;
    }
    let ctx = p.ctx();
    let mut tess = egui::epaint::Tessellator::new(
        ctx.pixels_per_point(),
        ctx.tessellation_options(|o| *o),
        [1, 1],
        Vec::new(),
    );
    let mut mesh = egui::epaint::Mesh::default();
    tess.tessellate_rect(&shape, &mut mesh);
    mesh.rotate(egui::emath::Rot2::from_angle(angle), pivot);
    p.add(egui::Shape::mesh(mesh));
}

/// Breite von [`orient_switch`].
pub const ORIENT_SWITCH_W: f32 = 40.0 * 2.0 + sp::XS;

/// Umschalter Quer/Hochkant nur mit Symbolen (kleines Rechteck quer bzw. hoch).
pub fn orient_switch(
    ui: &Ui,
    inner: Rect,
    tips: [&str; 2],
    selected: Option<usize>,
) -> Option<usize> {
    let p = ui.painter();
    let mut clicked = None;
    for (i, tip) in tips.iter().enumerate() {
        let rr = Rect::from_min_size(
            pos2(inner.left() + i as f32 * (40.0 + sp::XS), inner.top()),
            vec2(40.0, inner.height()),
        );
        let resp = clickable(ui, rr, ("orient_seg", i), true).on_hover_text(*tip);
        let on = selected == Some(i);
        let t = anim(ui, resp.id, resp.hovered() && !on);
        if on {
            shadow(p, rr, radius::M, 8, 2, Color32::from_black_alpha(50));
            p.rect_filled(
                rr,
                radius::M,
                if is_dark() {
                    Color32::from_white_alpha(235)
                } else {
                    Color32::WHITE
                },
            );
        } else if t > 0.0 {
            p.rect_filled(rr, radius::M, glass_hover().gamma_multiply(t));
        }
        let col = if on {
            Color32::from_rgb(0x11, 0x13, 0x18)
        } else {
            mix(text_soft(), text(), t)
        };
        let size = if i == 0 {
            vec2(18.0, 12.0)
        } else {
            vec2(12.0, 18.0)
        };
        p.rect_stroke(
            Rect::from_center_size(rr.center(), size),
            2,
            Stroke::new(1.8, col),
            StrokeKind::Middle,
        );
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// Text hinten kürzen („2021_08_12_Russl…“), bis er in `w` passt.
pub fn fit_text(ui: &Ui, s: &str, f: egui::FontId, w: f32) -> String {
    if text_size(ui, s, f.clone()).x <= w {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut n = chars.len();
    while n > 3 {
        n -= 1;
        let t = format!("{}…", chars[..n].iter().collect::<String>());
        if text_size(ui, &t, f.clone()).x <= w {
            return t;
        }
    }
    s.to_string()
}

/// Text vorne kürzen („…Bilder › 2021“), bis er in `w` passt.
pub fn fit_text_start(ui: &Ui, s: &str, f: egui::FontId, w: f32) -> String {
    let mut n = s.chars().count();
    let mut t = s.to_string();
    while n > 4 && text_size(ui, &t, f.clone()).x > w {
        n -= 1;
        t = ellipsize(s, n);
    }
    t
}
