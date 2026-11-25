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

use super::style::{self as pal, *};
use super::{ellipsize, icons};

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

mod buttons;
mod widgets;
pub use buttons::*;
pub use widgets::*;

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
