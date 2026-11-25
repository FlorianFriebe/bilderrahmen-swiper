//! Swipe-Ansicht: obere Leiste, Foto-Karte, Filmstreifen und schwebende Leiste.
//!
//! Die Leiste unten ist nach Wichtigkeit geordnet: in der Mitte die beiden
//! Entscheidungen (groß, farbig), links „Zurück“, rechts die Werkzeuge fürs Bild.

use std::path::PathBuf;

use egui::emath::Rot2;
use egui::epaint::{CornerRadius, RectShape, Shadow, TextShape};
use egui::{
    pos2, vec2, Align2, Color32, Id, Painter, PointerButton, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, Ui,
};

use super::kit::{self, Primary, Tone};
use super::style::*;
use super::{cover_uv, dialogs, ellipsize, icons, FULL_UV};
use crate::app::{SwiperApp, View};
use crate::folders;
use crate::imaging::{self, Orient};
use crate::util::fmt_int;

const STRIP_H: f32 = 64.0;
const TEXT_PANEL_W: f32 = 320.0;

/// Text aufs Bild an- oder ausschalten (Text-Knopf, Taste T).
pub fn toggle_text(app: &mut SwiperApp) {
    app.cfg.text.enabled = !app.cfg.text.enabled;
    app.toast(if app.cfg.text.enabled {
        "Text aufs Bild ist an.".into()
    } else {
        "Text aufs Bild ist aus.".into()
    });
}

mod card;
mod crop;
mod dock;
mod overlay;
mod strip;
mod topbar;
use card::*;
use crop::*;
use dock::*;
pub use overlay::GUIDES;
use overlay::*;
use strip::*;
use topbar::*;

/// Feld für Ort, Notiz und Aussehen öffnen oder schließen (Pfeil am Text-Knopf).
pub fn toggle_text_panel(app: &mut SwiperApp) {
    app.text_panel = !app.text_panel;
    if app.text_panel {
        app.cfg.text.enabled = true;
    }
}

pub fn show(app: &mut SwiperApp, ui: &mut Ui, full: Rect, ws: Rect, now: f64) {
    if !top_bar(app, ui, ws) {
        return;
    }
    // niedrige Fenster (Laptop): weniger Rand und ein flacheres Bilderband, damit das Foto größer wird
    let short = ws.height() < 760.0;
    let (top_gap, strip_h, gap) = if short {
        (sp::M, 48.0, sp::S)
    } else {
        (sp::XL, STRIP_H, sp::L)
    };
    let mut card_top = ws.top() + size::TOPBAR + top_gap;
    if let Some(h) = burst_chip(app, ui, ws, card_top) {
        card_top += h + sp::M;
    }
    if let Some(h) = dup_chip(app, ui, ws, card_top) {
        card_top += h + sp::M;
    }
    if let Some(h) = note_chip(app, ui, ws, card_top) {
        card_top += h + sp::M;
    }
    if card_top > ws.top() + size::TOPBAR + top_gap + 1.0 {
        app.spots.chips_bottom = Some(card_top - sp::M);
    }

    let dock = Rect::from_min_max(pos2(ws.left(), ws.bottom() - size::DOCK), ws.max);
    let strip = Rect::from_min_max(
        pos2(ws.left(), dock.top() - gap - strip_h),
        pos2(ws.right(), dock.top() - gap),
    );
    // Das Textfeld schwebt links neben dem Foto. Nur wenn es sonst das Foto
    // verdecken würde, wird die Karte gleichmäßig schmaler (sie bleibt mittig).
    let squeeze_id = Id::unique("panel_squeeze");
    let mut squeeze: f32 = if app.text_panel {
        ui.ctx().data(|d| d.get_temp(squeeze_id)).unwrap_or(0.0)
    } else {
        0.0
    };
    let card_area = Rect::from_min_max(
        pos2(ws.left() + squeeze, card_top),
        pos2(ws.right() - squeeze, strip.top() - gap),
    );
    let panel = Rect::from_min_size(
        pos2(full.left() + sp::XL, card_area.top()),
        vec2(TEXT_PANEL_W, card_area.height()),
    );

    run_anim(app, ui, now);
    if let Some(scan) = &app.scan {
        let (d, t) = scan.progress();
        ui.painter().text(
            card_area.center(),
            Align2::CENTER_CENTER,
            format!(
                "Lese Aufnahmedaten …  {} / {}",
                fmt_int(d.min(t)),
                fmt_int(t)
            ),
            font(ts::TITLE, W::Medium),
            text_soft(),
        );
        return;
    }
    if app.view != View::Swipe {
        return;
    }
    app.spots.card = Some(card_area);
    app.spots.strip = Some(strip);
    let drawn = card(app, ui, card_area, now);
    if app.text_panel {
        if let (Some(c), true) = (drawn, app.dx.abs() < 1.0) {
            let panel_x = (c.left() - sp::L - TEXT_PANEL_W).max(panel.left());
            let overlap = panel_x + TEXT_PANEL_W + sp::L - c.left();
            let max = (ws.width() / 2.0 - 160.0).max(0.0);
            let next = (squeeze + overlap).clamp(0.0, max);
            if (next - squeeze).abs() > 0.5 {
                squeeze = next;
                ui.ctx().request_repaint();
            }
            ui.ctx().data_mut(|d| d.insert_temp(squeeze_id, squeeze));
            let pr = Rect::from_min_size(pos2(panel_x, panel.top()), panel.size());
            filmstrip(app, ui, strip);
            let dock_r = dock_ui(app, ui, dock, now, card_area.width());
            app.spots.buttons = Some(dock_r);
            if app.dialog.is_none() {
                dialogs::text_panel(app, ui, pr);
            }
            return;
        }
    }
    filmstrip(app, ui, strip);
    let dock_r = dock_ui(app, ui, dock, now, card_area.width());
    app.spots.buttons = Some(dock_r);
}
