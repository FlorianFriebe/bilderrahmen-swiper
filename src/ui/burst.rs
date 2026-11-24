//! Serien-Vergleich: ähnliche Fotos nebeneinander, die besten auswählen.

use std::path::PathBuf;

use egui::epaint::RectShape;
use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui};

use super::kit::{self, Primary};
use super::style::*;
use super::{contain, icons, FULL_UV};
use crate::app::{SwiperApp, View};
use crate::folders;

pub fn show(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) {
    let Some((s, l)) = app.groups.get(app.pos).copied() else {
        app.view = View::Swipe;
        return;
    };
    let items: Vec<PathBuf> = app.items[s..s + l]
        .iter()
        .filter(|p| !app.status.bilder.contains_key(*p))
        .cloned()
        .collect();
    if items.len() < 2 {
        app.view = View::Swipe;
        return;
    }
    let p = ui.painter().clone();
    p.text(
        pos2(ws.center().x, ws.top() + 16.0),
        Align2::CENTER_CENTER,
        format!("Serie: {} ähnliche Fotos", items.len()),
        font(ts::HEADLINE, W::Bold),
        text(),
    );
    p.text(
        pos2(ws.center().x, ws.top() + 52.0),
        Align2::CENTER_CENTER,
        "Klicke die Fotos an, die du behalten willst. Der Rest wird übersprungen.",
        font(ts::LABEL, W::Regular),
        text_soft(),
    );
    let area = Rect::from_min_max(
        pos2(ws.left(), ws.top() + 88.0),
        pos2(ws.right(), ws.bottom() - size::DOCK - sp::XL),
    );
    let n = items.len();
    let cols = match n {
        2 | 4 => 2,
        3 | 5 | 6 => 3,
        _ => 4,
    };
    let rows = n.div_ceil(cols);
    let gap = sp::L;
    let cw = (area.width() - gap * (cols as f32 - 1.0)) / cols as f32;
    let ch = (area.height() - gap * (rows as f32 - 1.0)) / rows as f32;
    let mut toggle = None;
    for (i, path) in items.iter().enumerate() {
        let r = Rect::from_min_size(
            area.min
                + vec2(
                    (i % cols) as f32 * (cw + gap),
                    (i / cols) as f32 * (ch + gap),
                ),
            vec2(cw, ch),
        );
        if photo(app, ui, r, path) {
            toggle = Some(path.clone());
        }
    }
    if let Some(p) = toggle {
        if !app.burst_sel.remove(&p) {
            app.burst_sel.insert(p);
        }
    }

    // Leiste unten
    let k = app.burst_sel.len();
    let label = match k {
        0 => "Alle überspringen".to_string(),
        1 => "1 Foto behalten".to_string(),
        _ => format!("{k} Fotos behalten"),
    };
    let w1 = 220.0;
    let w2 = 260.0;
    let dock = Rect::from_center_size(
        pos2(ws.center().x, ws.bottom() - size::DOCK / 2.0),
        vec2(sp::M * 3.0 + w1 + w2, size::DOCK),
    );
    kit::glass(&p, dock, radius::DOCK);
    let r1 = Rect::from_min_size(
        pos2(dock.left() + sp::M, dock.center().y - size::PRIMARY / 2.0),
        vec2(w1, size::PRIMARY),
    );
    let r2 = Rect::from_min_size(pos2(r1.right() + sp::M, r1.top()), vec2(w2, size::PRIMARY));
    if kit::secondary(ui, r1, icons::ARROW_LEFT, "Einzeln durchgehen", true)
        .on_hover_text("Esc")
        .clicked()
    {
        app.view = View::Swipe;
    }
    let kind = if k > 0 { Primary::Keep } else { Primary::Skip };
    if kit::primary_button(ui, r2, kind, &label, "Enter", true)
        .on_hover_text("Ausgewählte behalten, den Rest überspringen")
        .clicked()
    {
        app.confirm_burst();
    }
}

/// Ein Foto der Serie. Gibt `true` zurück, wenn es angeklickt wurde.
fn photo(app: &mut SwiperApp, ui: &Ui, r: Rect, path: &PathBuf) -> bool {
    let resp = ui
        .interact(r, ui.make_persistent_id(("burst", path)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let sel = app.burst_sel.contains(path);
    let t = kit::anim(ui, resp.id, resp.hovered());
    let s = kit::anim(ui, resp.id.with("sel"), sel);
    let p = ui.painter();
    kit::glass_with(p, r, radius::XL, kit::mix(glass(), glass_hover(), t));

    let shown = match app.loader.get(path).and_then(|r| r.ok()) {
        Some(pre) => {
            let size = vec2(pre.image.size[0] as f32, pre.image.size[1] as f32);
            let key = path.with_extension("burstprev");
            let id = match app.thumb_tex.get(&key) {
                Some(Some(t)) => t.id(),
                _ => {
                    let h = ui.ctx().load_texture(
                        "serie",
                        (*pre.image).clone(),
                        egui::TextureOptions::LINEAR,
                    );
                    let id = h.id();
                    app.thumb_tex.insert(key, Some(h));
                    id
                }
            };
            Some((id, size))
        }
        None => app.thumb(path),
    };
    if let Some((id, size)) = shown {
        let dim_others = !sel && !app.burst_sel.is_empty();
        let ir = contain(size, r.shrink(sp::M));
        p.add(
            RectShape::filled(
                ir,
                radius::M,
                Color32::WHITE.gamma_multiply(if dim_others { 0.55 } else { 1.0 }),
            )
            .with_texture(id, FULL_UV),
        );
    }
    if s > 0.0 {
        p.rect_stroke(
            r,
            radius::XL,
            Stroke::new(3.0, GREEN.gamma_multiply(s)),
            StrokeKind::Inside,
        );
        let c = pos2(r.right() - 28.0, r.top() + 28.0);
        p.circle_filled(c, 18.0 * (0.6 + 0.4 * s), GREEN.gamma_multiply(s));
        kit::icon_filled(p, c, icons::HEART, 18.0, Color32::WHITE.gamma_multiply(s));
    }
    if let Some((when, _)) = app.dates.get(path) {
        let label = format!(
            "{}  ·  {}",
            folders::file_name(path),
            when.format("%H:%M:%S")
        );
        let f = font(ts::SMALL, W::Medium);
        let w = kit::text_size(ui, &label, f.clone()).x + sp::M * 2.0;
        let cr = Rect::from_min_size(
            pos2(r.left() + sp::L, r.bottom() - sp::L - 28.0),
            vec2(w, 28.0),
        );
        kit::photo_chip(p, cr);
        p.text(cr.center(), Align2::CENTER_CENTER, label, f, Color32::WHITE);
    }
    resp.clicked()
}
