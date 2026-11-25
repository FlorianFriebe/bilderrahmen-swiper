//! Wiederverwendbare Bausteine: Glasflächen, Knöpfe, Segmente, Chips.
//!
//! Jeder Baustein bekommt seine Fläche von außen (`Rect`) und kümmert sich nur um
//! Aussehen und Rückmeldung (Hover, Klick, Übergang).

use std::collections::HashMap;

use egui::epaint::{RectShape, Shadow};
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Painter, Pos2, Rect,
    Response, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, Vec2,
};

use super::icons;
use super::style::{self as pal, *};

// ------------------------------------------------------------------ Grundlagen

/// Sanfter Übergang 0..1 für Hover- und Aktiv-Zustände.
/// Rechte Maustaste wurde gerade über dieser Fläche gedrückt (für die Vorschau).
pub fn peek_pressed(ui: &Ui, resp: &Response) -> bool {
    resp.contains_pointer()
        && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary))
}

pub fn anim(ui: &Ui, id: Id, on: bool) -> f32 {
    ui.ctx().animate_bool_with_time(id, on, motion::FAST)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

pub fn text_size(ui: &Ui, text: &str, font: FontId) -> Vec2 {
    ui.fonts_mut(|f| {
        f.layout_no_wrap(text.to_string(), font, Color32::WHITE)
            .size()
    })
}

pub fn shadow(p: &Painter, r: Rect, rad: u8, blur: u8, dy: i8, color: Color32) {
    p.add(
        Shadow {
            offset: [0, dy],
            blur,
            spread: 0,
            color,
        }
        .as_shape(r, rad),
    );
}

/// Weicher Schein unter farbigen Knöpfen: mehrere flache Schichten statt einer harten Kante,
/// direkt am Knopf und kaum nach unten versetzt.
pub fn glow(p: &Painter, r: Rect, rad: u8, color: Color32) {
    for (blur, dy, a) in [
        (6u8, 1i8, 0.45f32),
        (16, 2, 0.28),
        (32, 4, 0.16),
        (56, 6, 0.08),
    ] {
        shadow(p, r, rad, blur, dy, color.gamma_multiply(a));
    }
}

/// Glasfläche: weicher Schatten, halbtransparente Füllung, feiner Rand, Lichtkante oben.
pub fn glass(p: &Painter, r: Rect, rad: u8) {
    glass_with(p, r, rad, pal::glass());
}

pub fn glass_with(p: &Painter, r: Rect, rad: u8, fill: Color32) {
    glow(p, r, rad, pal::shadow());
    p.rect(
        r,
        rad,
        fill,
        Stroke::new(1.0, glass_stroke()),
        StrokeKind::Inside,
    );
    let inset = rad as f32 * 0.8;
    p.line_segment(
        [
            pos2(r.left() + inset, r.top() + 1.0),
            pos2(r.right() - inset, r.top() + 1.0),
        ],
        Stroke::new(1.0, glass_highlight()),
    );
}

/// Dunkler Chip, der auf Fotos gut lesbar ist (in beiden Modi gleich).
pub fn photo_chip(p: &Painter, r: Rect) {
    p.rect(
        r,
        radius::S + 2,
        Color32::from_black_alpha(120),
        Stroke::new(1.0, Color32::from_white_alpha(36)),
        StrokeKind::Inside,
    );
}

pub fn icon(p: &Painter, c: Pos2, glyph: &str, size: f32, color: Color32) {
    p.text(c, Align2::CENTER_CENTER, glyph, icon_font(size), color);
}

pub fn icon_filled(p: &Painter, c: Pos2, glyph: &str, size: f32, color: Color32) {
    p.text(c, Align2::CENTER_CENTER, glyph, icon_fill_font(size), color);
}

/// Senkrechter Farbverlauf mit runden Ecken (über eine winzige Textur).
pub fn gradient(ui: &Ui, r: Rect, rad: u8, top: Color32, bottom: Color32) {
    let key = (
        u32::from_le_bytes(top.to_array()),
        u32::from_le_bytes(bottom.to_array()),
    );
    let id = Id::unique("kit_gradients");
    let ctx = ui.ctx().clone();
    let cached = ctx.data(|d| {
        d.get_temp::<HashMap<(u32, u32), TextureHandle>>(id)
            .and_then(|m| m.get(&key).map(|t| t.id()))
    });
    // Textur außerhalb des Speicher-Zugriffs laden, sonst blockiert der Kontext.
    let tex = cached.unwrap_or_else(|| {
        let h = ctx.load_texture(
            "verlauf",
            egui::ColorImage::new([1, 2], vec![top, bottom]),
            TextureOptions::LINEAR,
        );
        let tid = h.id();
        ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<HashMap<(u32, u32), TextureHandle>>(id)
                .insert(key, h);
        });
        tid
    });
    ui.painter().add(
        RectShape::filled(r, rad, Color32::WHITE)
            .with_texture(tex, Rect::from_min_max(pos2(0.0, 0.25), pos2(1.0, 0.75))),
    );
}

fn clickable(
    ui: &Ui,
    r: Rect,
    salt: impl std::hash::Hash + std::fmt::Debug,
    enabled: bool,
) -> Response {
    let id = ui.make_persistent_id(salt);
    let resp = ui.interact(
        r,
        id,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if enabled && resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp
}

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

/// Werkzeug mit Symbol und kleiner Beschriftung darunter.
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
    icon(p, r.center() - vec2(0.0, 8.0), glyph, 22.0, col);
    p.text(
        r.center() + vec2(0.0, 16.0),
        Align2::CENTER_CENTER,
        label,
        font(ts::CAPTION, W::Medium),
        if enabled {
            mix(muted(), col, a.max(t))
        } else {
            disabled()
        },
    );
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
    icon(p, main.center() - vec2(0.0, 8.0), t.glyph, 22.0, col);
    p.text(
        main.center() + vec2(0.0, 16.0),
        Align2::CENTER_CENTER,
        t.label,
        font(ts::CAPTION, W::Medium),
        if enabled {
            mix(muted(), col, a.max(hm))
        } else {
            disabled()
        },
    );
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
            mix(Color32::WHITE, Color32::from_gray(232), t),
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
    let fg = if enabled {
        Color32::from_gray(24)
    } else {
        disabled()
    };
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

fn hex_lighten(c: Color32, t: f32) -> Color32 {
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

fn tone_colors(t: Tone) -> (Color32, Color32, Color32) {
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
