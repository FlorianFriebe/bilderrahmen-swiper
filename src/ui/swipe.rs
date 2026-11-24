//! Swipe-Ansicht: obere Leiste, Foto-Karte, Filmstreifen und schwebende Leiste.
//!
//! Die Leiste unten ist nach Wichtigkeit geordnet: in der Mitte die beiden
//! Entscheidungen (groß, farbig), links „Zurück“, rechts die Werkzeuge fürs Bild.

use egui::emath::Rot2;
use egui::epaint::{CornerRadius, RectShape, Shadow, TextShape};
use egui::{
    pos2, vec2, Align2, Color32, Id, Painter, PointerButton, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, Ui,
};

use super::kit::{self, Primary, Tone};
use super::style::*;
use super::{contain, cover_uv, dialogs, ellipsize, icons, FULL_UV};
use crate::app::{Anim, Dialog, SwiperApp, View};
use crate::folders;
use crate::imaging::{self, Orient};
use crate::util::fmt_int;

const STRIP_H: f32 = 64.0;
const TEXT_PANEL_W: f32 = 320.0;

pub fn toggle_text(app: &mut SwiperApp) {
    app.text_panel = !app.text_panel;
    if app.text_panel {
        app.cfg.text.enabled = true;
    }
}

pub fn show(app: &mut SwiperApp, ui: &mut Ui, ws: Rect, now: f64) {
    if !top_bar(app, ui, ws) {
        return;
    }
    let mut card_top = ws.top() + size::BAR + sp::XL;
    if let Some(h) = burst_chip(app, ui, ws, card_top) {
        card_top += h + sp::M;
    }

    let dock = Rect::from_min_max(pos2(ws.left(), ws.bottom() - size::DOCK), ws.max);
    let strip = Rect::from_min_max(
        pos2(ws.left(), dock.top() - sp::M - STRIP_H),
        pos2(ws.right(), dock.top() - sp::M),
    );
    let panel_w = if app.text_panel {
        TEXT_PANEL_W + sp::L
    } else {
        0.0
    };
    let card_area = Rect::from_min_max(
        pos2(ws.left() + panel_w, card_top),
        pos2(ws.right(), strip.top() - sp::L),
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
    card(app, ui, card_area, now);
    filmstrip(app, ui, strip);
    let dock_r = dock_ui(app, ui, dock, now, card_area.width());
    app.spots.buttons = Some(dock_r);
    shortcuts_button(app, ui, ws, dock_r);
    if app.text_panel && !app.drawer {
        dialogs::text_panel(
            app,
            ui,
            Rect::from_min_size(
                pos2(ws.left(), card_area.top()),
                vec2(TEXT_PANEL_W, card_area.height()),
            ),
        );
    }
}

// ------------------------------------------------------------------ obere Leiste

/// Gibt `false` zurück, wenn die Ansicht gewechselt wurde.
fn top_bar(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) -> bool {
    let p = ui.painter().clone();
    let bar_y = ws.top();
    // links: Menü und Start
    let group = Rect::from_min_size(ws.min, vec2(size::CONTROL * 2.0 + sp::XS * 3.0, size::BAR));
    kit::glass(&p, group, radius::L);
    let b1 = Rect::from_min_size(
        group.min + vec2(sp::XS, sp::XS),
        vec2(size::CONTROL, size::CONTROL),
    );
    let b2 = b1.translate(vec2(size::CONTROL + sp::XS, 0.0));
    app.spots.menu = Some(group);
    if kit::icon_button(ui, b1, icons::MENU, true)
        .on_hover_text("Einstellungen (Strg+B)")
        .clicked()
    {
        app.drawer = !app.drawer;
    }
    if kit::icon_button(ui, b2, icons::HOME, true)
        .on_hover_text("Ordner wählen")
        .clicked()
    {
        app.finish_anim();
        app.view = View::Home;
        app.refresh_folders();
        return false;
    }

    // Mitte: Quer / Hochkant
    let labels = vec![
        format!("Quer {}", app.ratio_label(Orient::Quer)),
        format!("Hochkant {}", app.ratio_label(Orient::Hochkant)),
    ];
    let sw = kit::segmented_width(ui, &labels);
    let seg = Rect::from_center_size(
        pos2(ws.center().x, bar_y + size::BAR / 2.0),
        vec2(sw, size::BAR),
    );
    let sel = match app.orientation_now() {
        Some(Orient::Quer) => Some(0),
        Some(Orient::Hochkant) => Some(1),
        None => None,
    };
    if let Some(i) = kit::segmented(ui, seg, &labels, sel) {
        app.set_orientation(if i == 0 {
            Orient::Quer
        } else {
            Orient::Hochkant
        });
    }
    // rechts: Behalten-Zähler (öffnet die Galerie)
    let kept = if app.cfg.goal > 0 {
        format!(
            "{} / {}",
            fmt_int(app.total_kept),
            fmt_int(app.cfg.goal as usize)
        )
    } else {
        fmt_int(app.total_kept)
    };
    let kw = kit::chip_width(ui, icons::HEART, &kept).max(size::BAR + 8.0);
    let kr = Rect::from_min_size(pos2(ws.right() - kw, bar_y), vec2(kw, size::BAR));
    if kit::chip(ui, kr, icons::HEART, true, &kept, Tone::Keep)
        .on_hover_text(format!(
            "Behaltene Bilder ansehen ({})",
            app.key_label("gallery")
        ))
        .clicked()
    {
        app.finish_anim();
        app.view = View::Gallery;
        return false;
    }

    // daneben: Ordner und Fortschritt
    let name = app
        .folder
        .as_ref()
        .and_then(|f| app.folders.iter().find(|i| &i.path == f))
        .map(|i| ellipsize(&i.label, 22))
        .unwrap_or_default();
    let count = format!(
        "{} / {}",
        fmt_int(app.folder_done()),
        fmt_int(app.items.len())
    );
    let nf = font(ts::BODY, W::Regular);
    let cf = font(ts::BODY, W::Semibold);
    let nw = kit::text_size(ui, &name, nf.clone()).x;
    let cw = kit::text_size(ui, &count, cf.clone()).x;
    let bar_w = 112.0;
    let mw = sp::L + nw + sp::M + cw + sp::L + bar_w + sp::L;
    let mr = Rect::from_min_size(pos2(kr.left() - sp::S - mw, bar_y), vec2(mw, size::BAR));
    if mr.left() > seg.right() + sp::L {
        kit::glass(&p, mr, radius::L);
        let cy = mr.center().y;
        let mut x = mr.left() + sp::L;
        p.text(pos2(x, cy), Align2::LEFT_CENTER, &name, nf, text_soft());
        x += nw + sp::M;
        p.text(pos2(x, cy), Align2::LEFT_CENTER, &count, cf, text());
        x += cw + sp::L;
        let f = if app.items.is_empty() {
            0.0
        } else {
            app.folder_done() as f32 / app.items.len() as f32
        };
        kit::progress(
            &p,
            Rect::from_min_size(pos2(x, cy - 2.0), vec2(bar_w, 4.0)),
            f,
        );
    }
    true
}

/// Hinweis auf eine Serie ähnlicher Fotos. Gibt die belegte Höhe zurück.
fn burst_chip(app: &mut SwiperApp, ui: &Ui, ws: Rect, y: f32) -> Option<f32> {
    let (gs, gl) = app.current_group()?;
    let open = app.items[gs..gs + gl]
        .iter()
        .filter(|p| !app.status.bilder.contains_key(*p))
        .count();
    let label = format!(
        "Serie: {open} ähnliche Fotos  ·  vergleichen ({})",
        app.key_label("burst")
    );
    let w = kit::chip_width(ui, icons::STACK, &label);
    let r = Rect::from_center_size(pos2(ws.center().x, y + 18.0), vec2(w, 36.0));
    if kit::chip(ui, r, icons::STACK, false, &label, Tone::Warn).clicked() {
        app.open_burst();
    }
    Some(36.0)
}

// ------------------------------------------------------------------ Karte

fn run_anim(app: &mut SwiperApp, ui: &Ui, now: f64) {
    let Some(a) = &app.anim else { return };
    let t = ((now - a.start) / a.dur).clamp(0.0, 1.0) as f32;
    let e = 1.0 - (1.0 - t).powi(3);
    app.dx = a.from + (a.to - a.from) * e;
    app.dy *= 0.85;
    if t >= 1.0 {
        let a = app.anim.take().unwrap();
        app.dx = a.to;
        if let Some(k) = a.commit {
            app.commit(k);
        } else {
            app.dy = 0.0;
        }
    }
    ui.ctx().request_repaint();
}

/// 0..1: wie weit die Karte nach einem Bildwechsel schon „aufgetaucht“ ist.
fn rise(ui: &Ui, path: &std::path::Path, now: f64) -> f32 {
    let id = Id::unique("card_rise");
    let key = egui::util::hash(path);
    let (last, since): (u64, f64) = ui.ctx().data(|d| d.get_temp(id)).unwrap_or((0, -10.0));
    let since = if last != key { now } else { since };
    ui.ctx().data_mut(|d| d.insert_temp(id, (key, since)));
    let t = ((now - since) as f32 / motion::SLOW).clamp(0.0, 1.0);
    if t < 1.0 {
        ui.ctx().request_repaint();
    }
    1.0 - (1.0 - t).powi(3)
}

fn message(p: &Painter, area: Rect, glyph: &str, msg: &str) {
    kit::icon(p, area.center() - vec2(0.0, 28.0), glyph, 32.0, muted());
    let g = p.layout(
        msg.to_string(),
        font(ts::LABEL, W::Regular),
        text_soft(),
        420.0,
    );
    p.galley(
        pos2(area.center().x - g.size().x / 2.0, area.center().y),
        g,
        text_soft(),
    );
}

fn card(app: &mut SwiperApp, ui: &mut Ui, area: Rect, now: f64) {
    let resp = ui.allocate_rect(area, Sense::click_and_drag());
    let Some(path) = app.current().cloned() else {
        return;
    };
    let mut painter = ui.painter_at(area.expand2(vec2(sp::XXL, sp::XXL)));
    let Some((mut edit, size, tex_id)) = app.ensure_texture(ui) else {
        match app.preview() {
            Some(Err(e)) => message(
                &painter,
                area,
                icons::WARNING,
                &format!(
                    "„{}“ kann nicht geöffnet werden.\n{e}\n\nMit {} überspringen.",
                    folders::file_name(&path),
                    app.key_label("skip")
                ),
            ),
            _ => message(&painter, area, icons::IMAGE, "Lädt …"),
        }
        return;
    };

    let e = rise(ui, &path, now);
    painter.set_opacity(painter.opacity() * (0.35 + 0.65 * e));

    let avail = area.shrink2(vec2(sp::S, sp::S));
    let orient = edit.orientation(size.x as u32, size.y as u32);
    let ratio = app.output_settings().ratio(orient);
    let fill = edit.fill;
    // Im Füll-Modus zeigt die Karte gleich das fertige Rahmenbild.
    let shape = if fill { vec2(ratio, 1.0) } else { size };
    let s = (avail.width() / shape.x).min(avail.height() / shape.y);
    let scale = 0.97 + 0.03 * e;
    let card = Rect::from_center_size(avail.center(), shape * s * scale)
        .translate(vec2(app.dx, app.dy + (1.0 - e) * sp::L));
    let s = if fill { 1.0 } else { s * scale };
    let angle = app.dx / area.width().max(1.0) * 0.26;
    let pivot = card.center();
    let rot = Rot2::from_angle(angle);
    let tr = |p: Pos2| pivot + rot * (p - pivot);
    let rad = radius::CARD;

    handle_card_input(app, ui, &resp, area, card, fill, now, &mut edit);
    let (x, y, cw, ch) = imaging::crop_rect(size.x, size.y, ratio, &mut edit);
    app.edits.insert(path.clone(), edit);

    // Stapel dahinter und Schatten
    if app.dx.abs() < 1.0 {
        let back = Rect::from_center_size(card.center(), card.size()).translate(vec2(0.0, sp::M));
        let back = back.shrink2(vec2(sp::XL, 0.0));
        painter.rect(
            back,
            rad,
            glass(),
            Stroke::new(1.0, glass_stroke()),
            StrokeKind::Inside,
        );
    }
    let sh = Shadow {
        offset: [0, 24],
        blur: 64,
        spread: 0,
        color: Color32::from_black_alpha(if is_dark() { 150 } else { 60 }),
    };
    painter.add(sh.as_shape(card, rad).with_angle_and_pivot(angle, pivot));

    let out_r = if fill {
        let blur_id = app.tex.as_ref().map(|t| t.blur.id()).unwrap_or(tex_id);
        painter.add(
            RectShape::filled(card, rad, Color32::from_gray(158))
                .with_texture(blur_id, cover_uv(size, card.size()))
                .with_angle_and_pivot(angle, pivot),
        );
        painter.add(
            RectShape::filled(contain(size, card), 0, Color32::WHITE)
                .with_texture(tex_id, FULL_UV)
                .with_angle_and_pivot(angle, pivot),
        );
        card
    } else {
        painter.add(
            RectShape::filled(card, rad, Color32::WHITE)
                .with_texture(tex_id, FULL_UV)
                .with_angle_and_pivot(angle, pivot),
        );
        let cr = Rect::from_min_size(card.min + vec2(x * s, y * s), vec2(cw * s, ch * s));
        crop_frame(&painter, card, cr, angle, pivot, &tr);
        cr
    };
    // feiner Rand um die ganze Karte
    painter.add(
        RectShape::stroke(
            card,
            rad,
            Stroke::new(1.0, Color32::from_white_alpha(if fill { 120 } else { 40 })),
            StrokeKind::Inside,
        )
        .with_angle_and_pivot(angle, pivot),
    );
    let overlay = app.overlay_for(&path);
    let text_on_top = overlay.as_ref().map(|o| o.pos >= 3).unwrap_or(false);
    if let Some(ov) = &overlay {
        preview_overlay(&painter, ov, out_r, &tr, angle);
    }

    // Chips oben auf dem Foto
    if !text_on_top {
        let info = match app.preview() {
            Some(Ok(pre)) => format!(
                "{}  ·  {}",
                ellipsize(&folders::file_name(&path), 28),
                pre.when.format("%d.%m.%Y")
            ),
            _ => folders::file_name(&path),
        };
        let inset = sp::L;
        photo_chip(
            &painter,
            ui,
            card.left_top() + vec2(inset, inset),
            false,
            icons::IMAGE,
            &info,
            &tr,
            angle,
        );
        let mut right = if fill {
            "Ganzes Foto · unscharfer Hintergrund".to_string()
        } else {
            format!("Zuschnitt {}", app.ratio_label(orient))
        };
        if !fill && edit.zoom > 1.001 {
            right = format!("{right}  ·  {:.0} %", edit.zoom * 100.0);
        }
        photo_chip(
            &painter,
            ui,
            card.right_top() + vec2(-inset, inset),
            true,
            if fill { icons::FILL } else { icons::CROP },
            &right,
            &tr,
            angle,
        );
    }
    stamp(&painter, app.dx, card, angle, &tr);
}

#[allow(clippy::too_many_arguments)]
fn handle_card_input(
    app: &mut SwiperApp,
    ui: &Ui,
    resp: &egui::Response,
    area: Rect,
    card: Rect,
    fill: bool,
    now: f64,
    edit: &mut imaging::Edit,
) {
    if resp.drag_started_by(PointerButton::Primary) {
        app.anim = None;
        app.swiping = true;
    }
    if app.swiping && resp.dragged_by(PointerButton::Primary) {
        let d = resp.drag_delta();
        app.dx += d.x;
        app.dy += d.y * 0.3;
    }
    if resp.dragged_by(PointerButton::Secondary) && !fill {
        let d = resp.drag_delta();
        edit.cx += d.x / card.width();
        edit.cy += d.y / card.height();
    }
    if resp.drag_stopped() && app.swiping {
        app.swiping = false;
        let (to, dur, commit) = if app.dx.abs() > (area.width() * 0.18).max(110.0) {
            (
                area.width() * 1.3 * app.dx.signum(),
                0.18,
                Some(app.dx > 0.0),
            )
        } else {
            (0.0, 0.22, None)
        };
        app.anim = Some(Anim {
            from: app.dx,
            to,
            start: now,
            dur,
            commit,
        });
    }
    if resp.double_clicked() {
        edit.reset_crop();
    }
    if resp.hovered() {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        if !fill {
            edit.zoom *= (scroll * 0.003).exp() * zoom;
        }
        ui.ctx().set_cursor_icon(if app.swiping {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::Grab
        });
    }
}

/// Abgedunkelte Ränder außerhalb des Ausschnitts, Drittel-Linien und heller Rahmen.
fn crop_frame(
    painter: &Painter,
    card: Rect,
    cr: Rect,
    angle: f32,
    pivot: Pos2,
    tr: &dyn Fn(Pos2) -> Pos2,
) {
    let dark = Color32::from_black_alpha(140);
    let bands = [
        Rect::from_min_max(card.min, pos2(card.right(), cr.top())),
        Rect::from_min_max(pos2(card.left(), cr.bottom()), card.max),
        Rect::from_min_max(pos2(card.left(), cr.top()), pos2(cr.left(), cr.bottom())),
        Rect::from_min_max(pos2(cr.right(), cr.top()), pos2(card.right(), cr.bottom())),
    ];
    let rad = radius::CARD;
    for b in bands {
        if b.width() < 0.5 || b.height() < 0.5 {
            continue;
        }
        let near = |a: f32, b: f32| (a - b).abs() < 0.75;
        let crad: CornerRadius = kit::corners(
            near(b.left(), card.left()) && near(b.top(), card.top()),
            near(b.right(), card.right()) && near(b.top(), card.top()),
            near(b.left(), card.left()) && near(b.bottom(), card.bottom()),
            near(b.right(), card.right()) && near(b.bottom(), card.bottom()),
            rad,
        );
        painter.add(RectShape::filled(b, crad, dark).with_angle_and_pivot(angle, pivot));
    }
    let thin = Stroke::new(1.0, Color32::from_white_alpha(56));
    for k in [1.0 / 3.0, 2.0 / 3.0] {
        let xx = cr.left() + cr.width() * k;
        let yy = cr.top() + cr.height() * k;
        painter.line_segment([tr(pos2(xx, cr.top())), tr(pos2(xx, cr.bottom()))], thin);
        painter.line_segment([tr(pos2(cr.left(), yy)), tr(pos2(cr.right(), yy))], thin);
    }
    // Ecken rund lassen, wo der Ausschnitt an die Karte stößt
    let near = |a: f32, b: f32| (a - b).abs() < 0.75;
    let crad = kit::corners(
        near(cr.left(), card.left()) && near(cr.top(), card.top()),
        near(cr.right(), card.right()) && near(cr.top(), card.top()),
        near(cr.left(), card.left()) && near(cr.bottom(), card.bottom()),
        near(cr.right(), card.right()) && near(cr.bottom(), card.bottom()),
        rad,
    );
    painter.add(
        RectShape::stroke(
            cr,
            crad,
            Stroke::new(1.5, Color32::from_white_alpha(230)),
            StrokeKind::Inside,
        )
        .with_angle_and_pivot(angle, pivot),
    );
}

/// Dunkler Chip auf dem Foto, dreht sich mit der Karte.
#[allow(clippy::too_many_arguments)]
fn photo_chip(
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

/// „BEHALTEN“ / „WEG“-Stempel beim Ziehen.
fn stamp(painter: &Painter, dx: f32, card: Rect, angle: f32, tr: &dyn Fn(Pos2) -> Pos2) {
    if dx.abs() <= 8.0 {
        return;
    }
    let keep = dx > 0.0;
    let alpha = (dx.abs() / 140.0).min(1.0);
    let col = if keep { GREEN } else { RED }.gamma_multiply(alpha);
    let label = if keep { "BEHALTEN" } else { "WEG" };
    let size_pt = (card.height() / 11.0).clamp(22.0, 48.0);
    let g = painter.layout_no_wrap(label.into(), font(size_pt, W::Bold), col);
    let bw = g.size().x + 40.0;
    let bh = g.size().y + 14.0;
    let (center, a2) = if keep {
        (
            pos2(card.left() + 40.0 + bw / 2.0, card.top() + 72.0 + bh / 2.0),
            angle - 0.22,
        )
    } else {
        (
            pos2(card.right() - 40.0 - bw / 2.0, card.top() + 72.0 + bh / 2.0),
            angle + 0.22,
        )
    };
    let c = tr(center);
    painter.add(
        RectShape::new(
            Rect::from_center_size(c, vec2(bw, bh)),
            radius::M,
            Color32::from_black_alpha((70.0 * alpha) as u8),
            Stroke::new(4.0, col),
            StrokeKind::Inside,
        )
        .with_angle(a2),
    );
    let r2 = Rot2::from_angle(a2);
    let tl = c + r2 * vec2(-g.size().x / 2.0, -g.size().y / 2.0);
    painter.add(Shape::Text(TextShape::new(tl, g, col).with_angle(a2)));
}

/// Text so auf die Vorschau zeichnen, wie er später im Bild steht.
fn preview_overlay(
    painter: &Painter,
    o: &imaging::Overlay,
    r: Rect,
    tr: &dyn Fn(Pos2) -> Pos2,
    angle: f32,
) {
    let lines: Vec<&str> = o
        .lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return;
    }
    let (px, margin, line_h) = o.metrics(r.width(), r.height());
    let px = px.max(6.0);
    let galleys: Vec<_> = lines
        .iter()
        .map(|l| painter.layout_no_wrap(l.to_string(), font(px, W::Semibold), Color32::WHITE))
        .collect();
    let block_w = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let block_h =
        line_h * (galleys.len() as f32 - 1.0) + galleys.last().map(|g| g.size().y).unwrap_or(0.0);
    let col = o.pos % 3;
    let top = o.pos >= 3;
    let pad = if o.style == 1 { px * 0.45 } else { 0.0 };
    let y0 = if top {
        r.top() + margin + pad
    } else {
        r.bottom() - margin - pad - block_h
    };
    let x_for = |w: f32| match col {
        0 => r.left() + margin + pad,
        1 => r.center().x - w / 2.0,
        _ => r.right() - margin - pad - w,
    };
    if o.style == 1 {
        let br = Rect::from_min_size(
            pos2(x_for(block_w) - pad, y0 - pad * 0.8),
            vec2(block_w + pad * 2.0, block_h + pad * 1.6),
        );
        painter.add(
            RectShape::filled(
                Rect::from_center_size(tr(br.center()), br.size()),
                (pad * 0.8) as u8,
                Color32::from_black_alpha(107),
            )
            .with_angle(angle),
        );
    }
    let alpha = if o.style == 2 { 0.72 } else { 0.96 };
    for (i, g) in galleys.into_iter().enumerate() {
        let p = pos2(x_for(g.size().x), y0 + i as f32 * line_h);
        if o.style != 1 {
            let off = (px * 0.06).max(1.0);
            let sh = if o.style == 2 { 90 } else { 150 };
            painter.add(Shape::Text(
                TextShape::new(tr(p + vec2(off, off)), g.clone(), Color32::WHITE)
                    .with_override_text_color(Color32::from_black_alpha(sh))
                    .with_angle(angle),
            ));
        }
        painter.add(Shape::Text(
            TextShape::new(tr(p), g, Color32::WHITE)
                .with_override_text_color(Color32::from_white_alpha((255.0 * alpha) as u8))
                .with_angle(angle),
        ));
    }
}

// ------------------------------------------------------------------ Filmstreifen

fn filmstrip(app: &mut SwiperApp, ui: &mut Ui, area: Rect) {
    let n = app.items.len();
    if n == 0 {
        return;
    }
    let each = 3usize;
    let lo = app.pos.saturating_sub(each);
    let hi = (app.pos + each + 1).min(n);
    let (w, h) = (56.0, 40.0);
    let (cw, ch) = (64.0, 48.0);
    let gap = sp::S;
    let count = hi - lo;
    let inner = (count as f32 - 1.0) * (w + gap) + cw;
    let pill = Rect::from_center_size(area.center(), vec2(inner + sp::S * 2.0, ch + sp::S * 2.0));
    kit::glass(ui.painter(), pill, radius::L);
    let mut x = pill.left() + sp::S;
    let mut jump = None;
    for i in lo..hi {
        let is_cur = i == app.pos;
        let (ww, hh) = if is_cur { (cw, ch) } else { (w, h) };
        let r = Rect::from_min_size(pos2(x, area.center().y - hh / 2.0), vec2(ww, hh));
        x += ww + gap;
        let p = app.items[i].clone();
        let entry = app.decided(&p).map(|e| e.d == "keep");
        let resp = ui.interact(r, ui.make_persistent_id(("strip", i)), Sense::click());
        let t = kit::anim(ui, resp.id, resp.hovered());
        let alpha = if is_cur { 1.0 } else { 0.55 + 0.4 * t };
        let rad = radius::S;
        match app.thumb(&p) {
            Some((id, size)) => {
                ui.painter().add(
                    RectShape::filled(r, rad, Color32::WHITE.gamma_multiply(alpha))
                        .with_texture(id, cover_uv(size, r.size())),
                );
            }
            None => {
                ui.painter().rect_filled(r, rad, field());
            }
        }
        let stroke = match (is_cur, entry) {
            (true, _) => Stroke::new(2.0, Color32::WHITE),
            (_, Some(true)) => Stroke::new(2.0, keep()),
            (_, Some(false)) => Stroke::new(2.0, RED.gamma_multiply(0.8)),
            _ => Stroke::NONE,
        };
        if stroke != Stroke::NONE {
            ui.painter()
                .rect_stroke(r, rad, stroke, StrokeKind::Outside);
        }
        if !is_cur {
            let tip = match entry {
                Some(true) => "Behalten  ·  Klick: nochmal entscheiden",
                Some(false) => "Weg  ·  Klick: nochmal entscheiden",
                None => "Klick: zu diesem Bild springen",
            };
            if resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(tip)
                .clicked()
            {
                jump = Some(i);
            }
        }
    }
    if let Some(i) = jump {
        app.jump_to(i);
    }
}

// ------------------------------------------------------------------ schwebende Leiste

#[derive(Clone, Copy)]
enum Act {
    Undo,
    Skip,
    Keep,
    Rotate,
    Orient,
    Fill,
    Text,
}

/// Zeichnet die Leiste und gibt ihre Fläche zurück.
fn dock_ui(app: &mut SwiperApp, ui: &mut Ui, area: Rect, now: f64, width: f32) -> Rect {
    let loaded = matches!(app.preview(), Some(Ok(_)));
    let has_current = app.current().is_some();
    let k = |a: &str| app.key_label(a);
    let pad = sp::M;
    let tool = size::TOOL;
    let (skip_w, keep_w) = (152.0, 184.0);
    let tools_w = tool * 4.0 + sp::XS * 3.0;
    let w =
        pad + tool + pad + 1.0 + pad + skip_w + sp::S + keep_w + pad + 1.0 + pad + tools_w + pad;
    let dock = Rect::from_center_size(area.center(), vec2(w, size::DOCK));
    let p = ui.painter().clone();
    kit::glass(&p, dock, radius::DOCK);

    let cy = dock.center().y;
    let row = |x: f32, w: f32| Rect::from_min_size(pos2(x, cy - tool / 2.0), vec2(w, tool));
    let divider_at = |x: f32| {
        p.line_segment(
            [pos2(x, cy - 20.0), pos2(x, cy + 20.0)],
            Stroke::new(1.0, divider()),
        );
    };
    let mut clicked = None;
    let mut x = dock.left() + pad;

    let r = row(x, tool);
    if kit::tool_button(
        ui,
        r,
        icons::UNDO,
        "Zurück",
        false,
        !app.status.verlauf.is_empty(),
    )
    .on_hover_text(format!("Letzte Entscheidung rückgängig ({})", k("undo")))
    .clicked()
    {
        clicked = Some(Act::Undo);
    }
    x = r.right() + pad;
    divider_at(x);
    x += 1.0 + pad;

    let r = row(x, skip_w);
    if kit::primary_button(ui, r, Primary::Skip, "Weg", &k("skip"), has_current)
        .on_hover_text(format!(
            "Überspringen ({}) oder Foto nach links ziehen",
            k("skip")
        ))
        .clicked()
    {
        clicked = Some(Act::Skip);
    }
    x = r.right() + sp::S;
    let r = row(x, keep_w);
    if kit::primary_button(ui, r, Primary::Keep, "Behalten", &k("keep"), loaded)
        .on_hover_text(format!(
            "Für den Rahmen speichern ({}) oder nach rechts ziehen",
            k("keep")
        ))
        .clicked()
    {
        clicked = Some(Act::Keep);
    }
    x = r.right() + pad;
    divider_at(x);
    x += 1.0 + pad;

    let fill_on = app.current_fill();
    let text_on = app.text_panel
        || (app.cfg.text.enabled && app.current().and_then(|p| app.overlay_for(p)).is_some());
    let tools = [
        (
            icons::ROTATE,
            "Drehen",
            format!("Drehen ({}, andersherum {})", k("rot_r"), k("rot_l")),
            false,
            Act::Rotate,
        ),
        (
            icons::SWAP,
            "Format",
            format!("Quer / Hochkant wechseln ({})", k("orient")),
            false,
            Act::Orient,
        ),
        (
            icons::FILL,
            "Ganz",
            format!(
                "Ganzes Foto auf unscharfem Hintergrund statt zuschneiden ({})",
                k("fill")
            ),
            fill_on,
            Act::Fill,
        ),
        (
            icons::TEXT,
            "Text",
            format!("Datum, Ort oder Notiz aufs Bild ({})", k("text")),
            text_on,
            Act::Text,
        ),
    ];
    for (glyph, label, tip, active, act) in tools {
        let r = row(x, tool);
        if kit::tool_button(ui, r, glyph, label, active, loaded)
            .on_hover_text(tip)
            .clicked()
        {
            clicked = Some(act);
        }
        x += tool + sp::XS;
    }

    match clicked {
        Some(Act::Undo) => app.undo(),
        Some(Act::Skip) => app.decide(false, now, width),
        Some(Act::Keep) => app.decide(true, now, width),
        Some(Act::Rotate) => app.rotate(1),
        Some(Act::Orient) => app.toggle_orientation(),
        Some(Act::Fill) => app.toggle_fill(),
        Some(Act::Text) => toggle_text(app),
        None => {}
    }
    dock
}

/// Kleines Tastatur-Symbol unten rechts: zeigt alle Tastenkürzel.
fn shortcuts_button(app: &mut SwiperApp, ui: &Ui, ws: Rect, dock: Rect) {
    let r = Rect::from_center_size(
        pos2(ws.right() - size::BAR / 2.0, dock.center().y),
        vec2(size::BAR, size::BAR),
    );
    if r.left() < dock.right() + sp::L {
        return;
    }
    kit::glass(ui.painter(), r, radius::L);
    if kit::icon_button(ui, r.shrink(sp::XS), icons::KEYBOARD, true)
        .on_hover_text("Tastenkürzel (F1)")
        .clicked()
    {
        app.dialog = Some(Dialog::Shortcuts);
    }
}
