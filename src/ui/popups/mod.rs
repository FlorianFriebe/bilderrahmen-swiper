//! Kleine Popups an den Leisten: Text, Profil (Format und Größe) und App (Zahnrad).
//! Sie ersetzen das frühere Einstellungsfenster mit Seiten.

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, Ui};

use super::style::*;
use super::{dialogs, icons, kit};
use crate::app::{
    Dialog, SwiperApp, View, DATE_FORMATS, FRAMES, HOCH_FORMATS, QUER_FORMATS, RESOLUTIONS,
};
use crate::imaging::{self, Orient};
use crate::system;
use crate::util::open_folder;

mod copy;
mod gear;
mod profile;
mod text;
pub use copy::*;
pub use gear::*;
pub use profile::*;
pub use text::*;

/// Popup über (oder unter) einem Knopf einer Leiste. `add` gibt `false` zurück, wenn es
/// zugehen soll. Gibt `false` zurück, wenn es geschlossen werden soll.
pub fn form_popup(
    ui: &Ui,
    id: &str,
    item: Rect,
    w: f32,
    above: bool,
    add: impl FnOnce(&mut Ui) -> bool,
) -> bool {
    let screen = ui.ctx().content_rect();
    let left = (item.center().x - w / 2.0)
        .min(screen.right() - w - sp::L)
        .max(screen.left() + sp::L);
    let (pivot, at) = if above {
        (Align2::LEFT_BOTTOM, pos2(left, item.top() - sp::M))
    } else {
        (Align2::LEFT_TOP, pos2(left, item.bottom() + sp::M))
    };
    let max_h = if above {
        item.top() - sp::M - screen.top() - sp::L
    } else {
        screen.bottom() - item.bottom() - sp::M - sp::L
    };
    let mut keep = true;
    let area = egui::Area::new(ui.make_persistent_id(("form_popup", id)))
        .order(egui::Order::Foreground)
        .pivot(pivot)
        .fixed_pos(at)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(panel().to_opaque())
                .stroke(Stroke::new(1.0, glass_stroke()))
                .corner_radius(radius::L)
                .shadow(egui::Shadow {
                    offset: [0, 12],
                    blur: 40,
                    spread: 0,
                    color: Color32::from_black_alpha(if is_dark() { 130 } else { 40 }),
                })
                .inner_margin(egui::Margin::same(sp::S as i8))
                .show(ui, |ui| {
                    ui.set_width(w - sp::S * 2.0);
                    egui::ScrollArea::vertical()
                        .max_height(max_h.max(160.0))
                        .min_scrolled_height(max_h.clamp(160.0, 720.0))
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            keep = add(ui);
                        });
                });
        });
    let (clicked_at, esc) = ui.input(|i| {
        (
            i.pointer
                .any_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten(),
            i.key_pressed(egui::Key::Escape),
        )
    });
    // offene Auswahllisten (ComboBox) liegen außerhalb des Popups
    let child = egui::Popup::is_any_open(ui.ctx());
    let outside =
        !child && clicked_at.is_some_and(|p| !area.response.rect.contains(p) && !item.contains(p));
    keep && !outside && !(esc && !child)
}

/// Zeile wie im Ablauf-Menü: Titel, optional Untertitel, Haken wenn gewählt.
pub fn menu_row(ui: &mut Ui, label: &str, sub: &str, sel: bool) -> egui::Response {
    let h = if sub.is_empty() { 38.0 } else { 52.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let t = kit::anim(ui, resp.id, resp.hovered());
    let p = ui.painter();
    if t > 0.0 {
        p.rect_filled(r, radius::M, glass_hover().gamma_multiply(t));
    }
    let col = if sel { accent() } else { text() };
    let ty = if sub.is_empty() {
        r.center().y
    } else {
        r.top() + 17.0
    };
    let right = if sel { 32.0 } else { sp::M };
    let lf = font(ts::BODY, W::Medium);
    let shown = kit::fit_text(ui, label, lf.clone(), r.width() - sp::M - right);
    p.text(
        pos2(r.left() + sp::M, ty),
        Align2::LEFT_CENTER,
        shown,
        lf,
        col,
    );
    if !sub.is_empty() {
        let sf = font(ts::SMALL, W::Regular);
        let shown = kit::fit_text(ui, sub, sf.clone(), r.width() - sp::M - right);
        p.text(
            pos2(r.left() + sp::M, r.top() + 36.0),
            Align2::LEFT_CENTER,
            shown,
            sf,
            muted(),
        );
    }
    if sel {
        kit::icon(
            p,
            pos2(r.right() - sp::M - 8.0, r.center().y),
            icons::CHECK,
            16.0,
            accent(),
        );
    }
    resp
}

/// Kleine Überschrift in einem Popup.
fn caption(ui: &mut Ui, title: &str) {
    ui.add_space(sp::S);
    ui.horizontal(|ui| {
        ui.add_space(sp::M);
        ui.label(
            egui::RichText::new(title.to_uppercase())
                .font(font(ts::CAPTION, W::Semibold))
                .color(muted())
                .extra_letter_spacing(0.8),
        );
    });
    ui.add_space(sp::XS);
}

fn divider_line(ui: &mut Ui) {
    ui.add_space(sp::XS);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [r.left_center(), r.right_center()],
        Stroke::new(1.0, divider()),
    );
    ui.add_space(sp::XS);
}

/// Eingerückte Zeile mit Beschriftung links und Inhalt rechts.
fn field_row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(sp::M);
        ui.allocate_ui_with_layout(
            vec2(104.0, 26.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_size(vec2(104.0, 26.0));
                ui.label(egui::RichText::new(label).color(text_soft()));
            },
        );
        add(ui);
    });
}
