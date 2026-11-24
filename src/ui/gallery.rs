//! Galerie der behaltenen Bilder.

use std::collections::HashSet;
use std::path::PathBuf;

use egui::epaint::RectShape;
use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Ui};

use super::kit;
use super::style::*;
use super::{contain, ellipsize, icons, FULL_UV};
use crate::app::{Dialog, SwiperApp, View};
use crate::folders;
use crate::util::{fmt_int, open_folder};

pub fn show(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    let list = app.kept_list();
    if !header(app, ui, ws, list.len()) {
        return;
    }
    if list.is_empty() {
        kit::icon(
            ui.painter(),
            ws.center() - vec2(0.0, 28.0),
            icons::HEART,
            36.0,
            muted(),
        );
        ui.painter().text(
            ws.center() + vec2(0.0, 12.0),
            Align2::CENTER_CENTER,
            "Noch keine Bilder behalten.",
            font(ts::TITLE, W::Medium),
            text_soft(),
        );
        return;
    }
    let area = Rect::from_min_max(pos2(ws.left(), ws.top() + size::BAR + sp::XL), ws.max);
    let gap = sp::L;
    let cols = ((area.width() + gap) / (260.0 + gap)).floor().max(1.0) as usize;
    let tw = (area.width() - gap * (cols as f32 - 1.0)) / cols as f32;
    let th = tw * 0.66;
    let rows = list.len().div_ceil(cols);
    let mut discard = None;
    let mut visible = vec![];
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(&mut child, th + gap, rows, |ui, range| {
            for row in range {
                let (row_rect, _) =
                    ui.allocate_exact_size(vec2(area.width(), th + gap), Sense::hover());
                for c in 0..cols {
                    let i = row * cols + c;
                    let Some((src, out)) = list.get(i) else { break };
                    visible.push(out.clone());
                    let r = Rect::from_min_size(
                        row_rect.min + vec2(c as f32 * (tw + gap), 0.0),
                        vec2(tw, th),
                    );
                    if tile(app, ui, r, i, src, out) {
                        discard = Some(src.clone());
                    }
                }
            }
        });
    let vis: HashSet<PathBuf> = visible.into_iter().collect();
    app.thumbs
        .retain_queue(|p| vis.contains(p) || !p.starts_with(&app.cfg.target));
    if let Some(src) = discard {
        app.discard_kept(&src);
    }
}

/// Obere Leiste. Gibt `false` zurück, wenn die Ansicht gewechselt wurde.
fn header(app: &mut SwiperApp, ui: &mut Ui, ws: Rect, n: usize) -> bool {
    let back = Rect::from_min_size(
        ws.min,
        vec2(
            kit::secondary_width(ui, icons::ARROW_LEFT, "Zurück"),
            size::BAR,
        ),
    );
    if kit::secondary(ui, back, icons::ARROW_LEFT, "Zurück", true)
        .on_hover_text("Esc")
        .clicked()
    {
        app.view = if app.folder.is_some() && app.current().is_some() {
            View::Swipe
        } else {
            View::Home
        };
        return false;
    }
    ui.painter().text(
        pos2(ws.center().x, back.center().y),
        Align2::CENTER_CENTER,
        format!("Behaltene Bilder ({})", fmt_int(n)),
        font(ts::TITLE, W::Semibold),
        text(),
    );
    let mut right = ws.right();
    for (glyph, label, copy) in [
        (icons::FOLDER_OPEN, "Zielordner öffnen", false),
        (icons::USB, "Auf Stick / SD-Karte", true),
    ] {
        let w = kit::secondary_width(ui, glyph, label);
        let r = Rect::from_min_size(pos2(right - w, ws.top()), vec2(w, size::BAR));
        if kit::secondary(ui, r, glyph, label, true).clicked() {
            if copy {
                app.dialog = Some(Dialog::Copy);
            } else {
                open_folder(&app.cfg.target);
            }
        }
        right = r.left() - sp::S;
    }
    true
}

/// Eine Kachel. Gibt `true` zurück, wenn das Bild rausgeworfen werden soll.
fn tile(
    app: &SwiperApp,
    ui: &Ui,
    r: Rect,
    i: usize,
    src: &std::path::Path,
    out: &std::path::Path,
) -> bool {
    let resp = ui.interact(r, ui.make_persistent_id(("g", i)), Sense::hover());
    let t = kit::anim(ui, resp.id, resp.contains_pointer());
    let p = ui.painter();
    kit::glass_with(p, r, radius::L, kit::mix(glass(), glass_hover(), t));
    if let Some((id, size)) = app.thumb(out) {
        let ir = contain(size, r.shrink(sp::S));
        p.add(RectShape::filled(ir, radius::S, Color32::WHITE).with_texture(id, FULL_UV));
    }
    if t <= 0.01 {
        return false;
    }
    let folder = if out
        .parent()
        .map(|p| p.ends_with("Hochkant"))
        .unwrap_or(false)
    {
        "Hochkant"
    } else {
        "Quer"
    };
    let band = Rect::from_min_max(pos2(r.left(), r.bottom() - 36.0), r.max);
    p.rect_filled(
        band,
        kit::corners(false, false, true, true, radius::L),
        Color32::from_black_alpha((180.0 * t) as u8),
    );
    p.text(
        band.left_center() + vec2(sp::M, 0.0),
        Align2::LEFT_CENTER,
        format!("{}  ·  {folder}", ellipsize(&folders::file_name(src), 22)),
        font(ts::SMALL, W::Medium),
        Color32::WHITE.gamma_multiply(t),
    );
    let lf = font(ts::SMALL, W::Semibold);
    let bw = sp::M + 14.0 + sp::S + kit::text_size(ui, "Rauswerfen", lf.clone()).x + sp::M;
    let xr = Rect::from_min_size(
        pos2(r.right() - sp::S - bw, r.top() + sp::S),
        vec2(bw, 32.0),
    );
    let x = ui
        .interact(xr, ui.make_persistent_id(("discard", i)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Aus dem Zielordner löschen (das Original bleibt)");
    let h = kit::anim(ui, x.id, x.hovered());
    p.rect_filled(xr, radius::S + 2, RED.gamma_multiply((0.75 + 0.25 * h) * t));
    kit::icon(
        p,
        pos2(xr.left() + sp::M + 7.0, xr.center().y),
        icons::TRASH,
        14.0,
        Color32::WHITE.gamma_multiply(t),
    );
    p.text(
        pos2(xr.left() + sp::M + 14.0 + sp::S, xr.center().y),
        Align2::LEFT_CENTER,
        "Rauswerfen",
        lf,
        Color32::WHITE.gamma_multiply(t),
    );
    x.clicked()
}
