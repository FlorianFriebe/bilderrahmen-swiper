//! Knöpfe: Symbol-, Werkzeug-, Haupt-, Navigations- und Nebenknöpfe, Pillen.

use super::*;

// ------------------------------------------------------------------ Knöpfe

/// Quadratischer Symbol-Knopf ohne Rahmen (in Glasgruppen und Leisten).
pub fn icon_button(ui: &Ui, r: Rect, glyph: &str, enabled: bool) -> Response {
    let resp = clickable(
        ui,
        r,
        ("icon_btn", glyph, r.min.x as i32, r.min.y as i32),
        enabled,
    );
    let t = anim(ui, resp.id, resp.hovered() && enabled);
    let p = ui.painter();
    if t > 0.0 {
        p.rect_filled(r, radius::M, glass_hover().gamma_multiply(t));
    }
    let col = if enabled {
        mix(text_soft(), text(), t)
    } else {
        disabled()
    };
    icon(p, r.center(), glyph, size::ICON, col);
    resp
}

/// Werkzeug als Symbol; `label` dient nur als Kennung (der Name steht im Tooltip).
pub struct Tool<'a> {
    pub glyph: &'a str,
    pub label: &'a str,
    /// aktuell belegte Taste, klein oben rechts
    pub key: &'a str,
    pub active: bool,
    pub enabled: bool,
}

pub fn tool_button(ui: &Ui, r: Rect, t: Tool<'_>) -> Response {
    let Tool {
        glyph,
        label,
        key,
        active,
        enabled,
    } = t;
    let resp = clickable(ui, r, ("tool_btn", label), enabled);
    let t = anim(ui, resp.id, resp.hovered() && enabled);
    let a = anim(ui, resp.id.with("on"), active);
    let p = ui.painter();
    let fill = mix(Color32::TRANSPARENT, glass_hover(), t);
    let fill = mix(fill, accent_soft(), a);
    p.rect_filled(r, radius::L, fill);
    let col = if !enabled {
        disabled()
    } else {
        mix(mix(text_soft(), text(), t), accent(), a)
    };
    // nur das Symbol; der Name steht im Tooltip
    icon(p, r.center(), glyph, 24.0, col);
    key_hint(p, r, key, if a > 0.5 { accent() } else { muted() });
    resp
}

/// Werkzeug mit angehängtem Pfeil (z. B. Text): beide teilen sich eine Fläche,
/// die bei „an“ gemeinsam hervorgehoben wird. Gibt (Werkzeug, Pfeil) zurück.
pub fn split_tool(ui: &Ui, r: Rect, caret_w: f32, t: Tool<'_>, open: bool) -> (Response, Response) {
    let main = Rect::from_min_max(r.min, pos2(r.right() - caret_w, r.bottom()));
    let cr = Rect::from_min_max(pos2(main.right(), r.top()), r.max);
    let enabled = t.enabled;
    let rm = clickable(ui, main, ("split_main", t.label), enabled);
    let rc = clickable(ui, cr, ("split_caret", t.label), enabled);
    let hm = anim(ui, rm.id, rm.hovered() && enabled);
    let hc = anim(ui, rc.id, rc.hovered() && enabled);
    let a = anim(ui, rm.id.with("on"), t.active);
    let o = anim(ui, rc.id.with("open"), open);
    let p = ui.painter();
    // gemeinsame Fläche: aktiv = blau über beide Teile
    p.rect_filled(r, radius::L, accent_soft().gamma_multiply(a));
    if hm > 0.0 {
        p.rect_filled(
            main,
            corners(true, false, true, false, radius::L),
            glass_hover().gamma_multiply(hm),
        );
    }
    if hc > 0.0 || o > 0.0 {
        p.rect_filled(
            cr,
            corners(false, true, false, true, radius::L),
            mix(
                glass_hover().gamma_multiply(hc),
                accent_soft().gamma_multiply(0.8),
                o,
            ),
        );
    }
    // feine Trennlinie zwischen Werkzeug und Pfeil
    p.line_segment(
        [
            pos2(cr.left(), r.top() + 18.0),
            pos2(cr.left(), r.bottom() - 18.0),
        ],
        Stroke::new(1.0, mix(divider(), accent().gamma_multiply(0.35), a)),
    );
    let col = if !enabled {
        disabled()
    } else {
        mix(mix(text_soft(), text(), hm), accent(), a)
    };
    icon(p, main.center(), t.glyph, 24.0, col);
    key_hint(p, main, t.key, if a > 0.5 { accent() } else { muted() });
    let ccol = if !enabled {
        disabled()
    } else {
        mix(mix(muted(), text(), hc), accent(), a.max(o))
    };
    icon(
        p,
        cr.center(),
        if open {
            icons::CARET_UP
        } else {
            icons::CARET_DOWN
        },
        14.0,
        ccol,
    );
    (rm, rc)
}

/// Blasse Tastenbeschriftung oben rechts in einem Knopf.
pub fn key_hint(p: &Painter, r: Rect, key: &str, col: Color32) {
    if key.is_empty() {
        return;
    }
    p.text(
        pos2(r.right() - 7.0, r.top() + 6.0),
        Align2::RIGHT_TOP,
        key,
        font(10.0, W::Semibold),
        col.gamma_multiply(0.7),
    );
}

/// Flacher, farbiger Chip ohne Schatten (in Leisten).
pub fn pill(ui: &Ui, r: Rect, glyph: &str, label: &str, tone: Tone) -> Response {
    let resp = clickable(ui, r, ("pill", label), true);
    let t = anim(ui, resp.id, resp.hovered());
    let (fill, _, fg) = tone_colors(tone);
    let p = ui.painter();
    p.rect_filled(r, radius::M, mix(fill, fill.gamma_multiply(1.7), t));
    if t > 0.0 {
        p.rect_stroke(
            r,
            radius::M,
            Stroke::new(1.0, fg.gamma_multiply(0.45 * t)),
            StrokeKind::Inside,
        );
    }
    let f = font(ts::BODY, W::Semibold);
    let lw = text_size(ui, label, f.clone()).x;
    let x = r.center().x - (16.0 + sp::S - 2.0 + lw) / 2.0;
    icon_filled(p, pos2(x + 8.0, r.center().y), glyph, 16.0, fg);
    p.text(
        pos2(x + 16.0 + sp::S - 2.0, r.center().y),
        Align2::LEFT_CENTER,
        label,
        f,
        fg,
    );
    resp
}

pub fn pill_width(ui: &Ui, label: &str) -> f32 {
    text_size(ui, label, font(ts::BODY, W::Semibold)).x + 16.0 + sp::S + sp::M * 2.0
}

/// Senkrechter Trennstrich in Leisten.
pub fn vsep(p: &Painter, x: f32, cy: f32) {
    p.line_segment(
        [pos2(x, cy - 14.0), pos2(x, cy + 14.0)],
        Stroke::new(1.0, divider()),
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Primary {
    Keep,
    Skip,
}

/// Die beiden Hauptknöpfe „Weg“ und „Behalten“.
pub fn primary_button(
    ui: &Ui,
    r: Rect,
    kind: Primary,
    label: &str,
    key: &str,
    enabled: bool,
) -> Response {
    let resp = clickable(ui, r, ("primary", kind as u8), enabled);
    let t = anim(ui, resp.id, resp.hovered() && enabled);
    let press = if resp.is_pointer_button_down_on() {
        0.97
    } else {
        1.0
    };
    let r = Rect::from_center_size(r.center(), r.size() * press);
    let p = ui.painter();
    let rad = radius::XL;
    let fg = match kind {
        Primary::Keep => Color32::WHITE,
        Primary::Skip => skip(),
    };
    if !enabled {
        p.rect(
            r,
            rad,
            pal::glass(),
            Stroke::new(1.0, glass_stroke()),
            StrokeKind::Inside,
        );
    } else {
        match kind {
            Primary::Keep => {
                glow(p, r, rad, GREEN.gamma_multiply(0.6 + 0.3 * t));
                let top = mix(hex_lighten(GREEN, 0.06), hex_lighten(GREEN, 0.16), t);
                gradient(ui, r, rad, top, GREEN_DARK);
                p.line_segment(
                    [
                        pos2(r.left() + 14.0, r.top() + 1.0),
                        pos2(r.right() - 14.0, r.top() + 1.0),
                    ],
                    Stroke::new(1.0, Color32::from_white_alpha(70)),
                );
            }
            Primary::Skip => {
                p.rect(
                    r,
                    rad,
                    mix(skip_soft(), skip_soft().gamma_multiply(1.8), t),
                    Stroke::new(1.0, RED.gamma_multiply(0.55 + 0.25 * t)),
                    StrokeKind::Inside,
                );
            }
        }
    }
    let fg = if enabled { fg } else { disabled() };
    // Inhalt: Symbol, Text, Tasten-Chip
    let ic = 24.0;
    let lab = font(ts::BUTTON, W::Semibold);
    let key_font = font(ts::CAPTION, W::Semibold);
    let lw = if label.is_empty() {
        -sp::M
    } else {
        text_size(ui, label, lab.clone()).x
    };
    let kw = if key.is_empty() {
        0.0
    } else {
        text_size(ui, key, key_font.clone()).x + 12.0
    };
    let total = ic + sp::M + lw + if kw > 0.0 { sp::M + kw } else { 0.0 };
    let mut x = r.center().x - total / 2.0;
    let cy = r.center().y;
    match kind {
        Primary::Keep => icon_filled(p, pos2(x + ic / 2.0, cy), icons::HEART, ic, fg),
        Primary::Skip => {
            p.text(
                pos2(x + ic / 2.0, cy),
                Align2::CENTER_CENTER,
                icons::X,
                FontId::new(ic, egui::FontFamily::Name("ph".into())),
                fg,
            );
        }
    }
    x += ic + sp::M;
    if !label.is_empty() {
        p.text(pos2(x, cy), Align2::LEFT_CENTER, label, lab, fg);
    }
    x += lw + sp::M;
    if kw > 0.0 {
        let kr = Rect::from_min_size(pos2(x, cy - 10.0), vec2(kw, 20.0));
        p.rect_filled(
            kr,
            6,
            Color32::from_white_alpha(if is_dark() { 30 } else { 60 })
                .gamma_multiply(if kind == Primary::Keep { 1.2 } else { 0.8 }),
        );
        p.text(
            kr.center(),
            Align2::CENTER_CENTER,
            key,
            key_font,
            fg.gamma_multiply(0.85),
        );
    }
    resp
}

/// Schlichter weißer Knopf zum Blättern (Feinschliff: Zurück / Vor), mit Pfeil und Taste.
pub fn nav_button(ui: &Ui, r: Rect, glyph: &str, key: &str, enabled: bool) -> Response {
    let resp = clickable(ui, r, ("nav", glyph), enabled);
    let t = anim(ui, resp.id, resp.hovered() && enabled);
    let press = if resp.is_pointer_button_down_on() {
        0.97
    } else {
        1.0
    };
    let r = Rect::from_center_size(r.center(), r.size() * press);
    let p = ui.painter();
    let rad = radius::XL;
    if enabled {
        glow(
            p,
            r,
            rad,
            Color32::from_black_alpha(if is_dark() { 90 } else { 30 }),
        );
        p.rect(
            r,
            rad,
            mix(Color32::WHITE, LIGHT_PRESSED, t),
            Stroke::new(
                1.0,
                Color32::from_black_alpha(if is_dark() { 0 } else { 28 }),
            ),
            StrokeKind::Inside,
        );
    } else {
        p.rect(
            r,
            rad,
            pal::glass(),
            Stroke::new(1.0, glass_stroke()),
            StrokeKind::Inside,
        );
    }
    let fg = if enabled { LIGHT_INK } else { disabled() };
    let ic = 24.0;
    let key_font = font(ts::CAPTION, W::Semibold);
    let kw = if key.is_empty() {
        0.0
    } else {
        text_size(ui, key, key_font.clone()).x + 12.0
    };
    let total = ic + if kw > 0.0 { sp::M + kw } else { 0.0 };
    let cy = r.center().y;
    let x = r.center().x - total / 2.0;
    p.text(
        pos2(x + ic / 2.0, cy),
        Align2::CENTER_CENTER,
        glyph,
        FontId::new(ic, egui::FontFamily::Name("ph".into())),
        fg,
    );
    if kw > 0.0 {
        let kr = Rect::from_min_size(pos2(x + ic + sp::M, cy - 10.0), vec2(kw, 20.0));
        p.rect_filled(
            kr,
            6,
            Color32::from_black_alpha(if enabled { 18 } else { 0 }),
        );
        p.text(
            kr.center(),
            Align2::CENTER_CENTER,
            key,
            key_font,
            fg.gamma_multiply(0.8),
        );
    }
    resp
}

pub(super) fn hex_lighten(c: Color32, t: f32) -> Color32 {
    mix(c, Color32::WHITE, t)
}

/// Großer blauer Hauptknopf (z. B. „645 Fotos durchgehen“).
pub fn cta(ui: &Ui, r: Rect, glyph: &str, label: &str, enabled: bool) -> Response {
    let resp = clickable(ui, r, ("cta", label), enabled);
    let t = anim(ui, resp.id, resp.hovered() && enabled);
    let press = if resp.is_pointer_button_down_on() {
        0.98
    } else {
        1.0
    };
    let r = Rect::from_center_size(r.center(), r.size() * press);
    let p = ui.painter();
    let rad = radius::XL - 2;
    if enabled {
        glow(p, r, rad, BLUE_DARK.gamma_multiply(0.7 + 0.3 * t));
        gradient(
            ui,
            r,
            rad,
            mix(hex_lighten(BLUE, 0.1), hex_lighten(BLUE, 0.2), t),
            BLUE_DARK,
        );
        p.line_segment(
            [
                pos2(r.left() + 16.0, r.top() + 1.0),
                pos2(r.right() - 16.0, r.top() + 1.0),
            ],
            Stroke::new(1.0, Color32::from_white_alpha(70)),
        );
    } else {
        glass(p, r, rad);
    }
    let fg = if enabled { Color32::WHITE } else { muted() };
    let lab = font(17.0, W::Semibold);
    let lw = text_size(ui, label, lab.clone()).x;
    let gw = if glyph.is_empty() { 0.0 } else { 20.0 + sp::M };
    let x = r.center().x - (lw + gw) / 2.0;
    if !glyph.is_empty() {
        icon_filled(p, pos2(x + 10.0, r.center().y), glyph, 20.0, fg);
    }
    p.text(
        pos2(x + gw, r.center().y),
        Align2::LEFT_CENTER,
        label,
        lab,
        fg,
    );
    resp
}

/// Zweitrangiger Knopf: Glasfläche mit Text und optionalem Symbol.
pub fn secondary(ui: &Ui, r: Rect, glyph: &str, label: &str, enabled: bool) -> Response {
    let resp = clickable(ui, r, ("secondary", label, r.min.x as i32), enabled);
    let t = anim(ui, resp.id, resp.hovered() && enabled);
    let p = ui.painter();
    glass_with(p, r, radius::M, mix(pal::glass(), glass_hover(), t));
    let fg = if enabled { text() } else { disabled() };
    let f = font(ts::BODY, W::Medium);
    let lw = text_size(ui, label, f.clone()).x;
    let gw = if glyph.is_empty() { 0.0 } else { 18.0 + sp::S };
    let x = r.center().x - (lw + gw) / 2.0;
    if !glyph.is_empty() {
        icon(p, pos2(x + 9.0, r.center().y), glyph, 18.0, fg);
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

/// Breite, die `secondary` für einen Text braucht.
pub fn secondary_width(ui: &Ui, glyph: &str, label: &str) -> f32 {
    let lw = text_size(ui, label, font(ts::BODY, W::Medium)).x;
    lw + if glyph.is_empty() { 0.0 } else { 18.0 + sp::S } + sp::L * 2.0
}
