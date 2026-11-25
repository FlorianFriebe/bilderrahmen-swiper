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
use crate::app::{Dialog, SwiperApp, View};
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
    let mut card_top = ws.top() + size::TOPBAR + sp::XL;
    if let Some(h) = burst_chip(app, ui, ws, card_top) {
        card_top += h + sp::M;
    }
    if let Some(h) = dup_chip(app, ui, ws, card_top) {
        card_top += h + sp::M;
    }
    if card_top > ws.top() + size::TOPBAR + sp::XL + 1.0 {
        app.spots.chips_bottom = Some(card_top - sp::M);
    }

    let dock = Rect::from_min_max(pos2(ws.left(), ws.bottom() - size::DOCK), ws.max);
    let strip = Rect::from_min_max(
        pos2(ws.left(), dock.top() - sp::M - STRIP_H),
        pos2(ws.right(), dock.top() - sp::M),
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
        pos2(ws.right() - squeeze, strip.top() - sp::L),
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

// ------------------------------------------------------------------ obere Leiste

/// Kompakte Leiste oben in der Mitte. Gibt `false` zurück, wenn die Ansicht gewechselt wurde.
fn top_bar(app: &mut SwiperApp, ui: &mut Ui, ws: Rect) -> bool {
    let p = ui.painter().clone();
    let pad = sp::S;
    let h = size::CONTROL;

    let pause_w = h;
    let labels = vec![
        format!("Quer {}", app.ratio_label(Orient::Quer)),
        format!("Hochkant {}", app.ratio_label(Orient::Hochkant)),
    ];
    let quick = app.quick_sort();
    let seg_w = if quick {
        0.0
    } else {
        kit::segmented_width(ui, &labels) - sp::XS * 2.0
    };
    // Dezente Anzeige des Schritts (Schnell sortieren / Feinschliff / Nachbearbeiten)
    let step: Option<(&str, String, &str)> = if quick {
        Some((
            icons::LIGHTNING,
            "Schnell sortieren".into(),
            "Erster Durchgang: nur behalten oder weg. Zuschneiden kommt im Feinschliff, wenn der Ordner fertig ist.",
        ))
    } else if app.reedit.is_some() {
        Some(match app.polish_left() {
            Some((i, n)) => (
                icons::PENCIL,
                format!("Feinschliff {i} / {n}"),
                "Klick: Feinschliff beenden (die übrigen Fotos bleiben, wie sie sind)",
            ),
            None => (
                icons::PENCIL,
                "Nachbearbeiten".into(),
                "Klick: abbrechen und zurück zur Galerie",
            ),
        })
    } else {
        None
    };
    let sf = font(ts::SMALL, W::Semibold);
    let step_w = step
        .as_ref()
        .map(|(_, l, _)| 14.0 + sp::XS + kit::text_size(ui, l, sf.clone()).x + sp::M * 2.0)
        .unwrap_or(0.0);
    let name = app
        .folder
        .as_ref()
        .and_then(|f| app.folders.iter().find(|i| &i.path == f))
        .map(|i| ellipsize(&i.label, 20))
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
    let mini = 72.0;
    let kept = if app.cfg.goal > 0 {
        format!(
            "{} / {}",
            fmt_int(app.total_kept),
            fmt_int(app.cfg.goal as usize)
        )
    } else {
        fmt_int(app.total_kept)
    };
    let kept_w = kit::pill_width(ui, &kept);
    let gap = sp::XS;
    let sep = sp::M * 2.0 + 1.0;
    let mut meta_w = sp::S + nw + sp::M + cw + sp::M + mini + sp::S;
    let seg_part = if quick { 0.0 } else { seg_w + sep };
    let step_part = if step.is_some() { step_w + sp::M } else { 0.0 };
    let fixed = pad * 2.0 + pause_w + gap + h + sep + seg_part + step_part + gap + kept_w + gap + h;
    let show_name = fixed + meta_w <= ws.width();
    if !show_name {
        meta_w = sp::S + cw + sp::S;
    }
    let total = fixed + meta_w;
    let bar = Rect::from_center_size(
        pos2(ws.center().x, ws.top() + size::TOPBAR / 2.0),
        vec2(total, size::TOPBAR),
    );
    kit::glass(&p, bar, radius::XL - 2);
    app.spots.menu = Some(bar);
    let cy = bar.center().y;
    let row = |x: f32, w: f32| Rect::from_min_size(pos2(x, cy - h / 2.0), vec2(w, h));
    let mut x = bar.left() + pad;

    p.rect_filled(row(x, pause_w), radius::M, field());
    if kit::icon_button(ui, row(x, pause_w), icons::HOME, true)
        .on_hover_text(
            "Zum Startbildschirm. Dein Stand wird gespeichert und steht unter „Weitermachen“.",
        )
        .clicked()
    {
        app.pause();
        return false;
    }
    x += pause_w + gap;
    if kit::icon_button(ui, row(x, h), icons::GEAR, true)
        .on_hover_text("Einstellungen (Strg+B)")
        .clicked()
    {
        app.dialog = Some(Dialog::Settings);
    }
    x += h + sp::M;
    kit::vsep(&p, x, cy);
    x += 1.0 + sp::M;

    if !quick {
        let sel = match app.orientation_now() {
            Some(Orient::Quer) => Some(0),
            Some(Orient::Hochkant) => Some(1),
            None => None,
        };
        if let Some(i) = kit::segmented_bare(ui, row(x, seg_w), &labels, sel) {
            app.set_orientation(if i == 0 {
                Orient::Quer
            } else {
                Orient::Hochkant
            });
        }
        x += seg_w + sp::M;
        kit::vsep(&p, x, cy);
        x += 1.0 + sp::M;
    }
    if let Some((glyph, label, tip)) = &step {
        let r = Rect::from_min_size(pos2(x, cy - 14.0), vec2(step_w, 28.0));
        let resp = ui
            .interact(r, ui.make_persistent_id("step_tag"), Sense::click())
            .on_hover_text(*tip);
        let t = kit::anim(ui, resp.id, resp.hovered() && app.reedit.is_some());
        p.rect_filled(r, radius::M, accent_soft().gamma_multiply(0.6 + 0.4 * t));
        kit::icon(&p, pos2(r.left() + sp::M + 7.0, cy), glyph, 14.0, accent());
        p.text(
            pos2(r.left() + sp::M + 14.0 + sp::XS, cy),
            Align2::LEFT_CENTER,
            label,
            sf.clone(),
            accent(),
        );
        if resp.clicked() && app.reedit.is_some() {
            app.cancel_reedit();
            return false;
        }
        x += step_w + sp::M;
    }

    // Ordner und Fortschritt
    let mut mx = x + sp::S;
    if show_name {
        p.text(pos2(mx, cy), Align2::LEFT_CENTER, &name, nf, text_soft());
        mx += nw + sp::M;
    }
    p.text(pos2(mx, cy), Align2::LEFT_CENTER, &count, cf, text());
    mx += cw + sp::M;
    if show_name {
        let f = if app.items.is_empty() {
            0.0
        } else {
            app.folder_done() as f32 / app.items.len() as f32
        };
        kit::progress(
            &p,
            Rect::from_min_size(pos2(mx, cy - 2.0), vec2(mini, 4.0)),
            f,
        );
    }
    x += meta_w + gap;

    if kit::pill(ui, row(x, kept_w), icons::HEART, &kept, Tone::Keep)
        .on_hover_text(format!(
            "Behaltene Bilder ansehen ({})",
            app.key_label("gallery")
        ))
        .clicked()
    {
        app.finish_anim();
        app.open_gallery();
        return false;
    }
    x += kept_w + gap;
    if kit::icon_button(ui, row(x, h).shrink(2.0), icons::KEYBOARD, true)
        .on_hover_text("Alle Tastenkürzel (F1)")
        .clicked()
    {
        app.dialog = Some(Dialog::Shortcuts);
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

/// Hinweis, wenn dasselbe Motiv schon behalten wurde (z. B. als Kopie in einem
/// anderen Ordner). Mit kleinem Vorschaubild; Klick zeigt das frühere Bild groß.
fn dup_chip(app: &mut SwiperApp, ui: &Ui, ws: Rect, y: f32) -> Option<f32> {
    let cur = app.current().cloned()?;
    if app.status.bilder.get(&cur).is_some_and(|e| e.d == "keep") {
        return None;
    }
    let (other, out) = app.duplicate_of(&cur)?;
    let folder = other.parent().map(folders::file_name).unwrap_or_default();
    let label = format!(
        "Schon behalten: {} aus „{}“  ·  ansehen",
        ellipsize(&folders::file_name(&other), 24),
        ellipsize(&folder, 24)
    );
    let th = 28.0;
    let w = kit::chip_width(ui, icons::STACK, &label) + th + sp::S;
    let r = Rect::from_center_size(pos2(ws.center().x, y + 18.0), vec2(w, 36.0));
    let resp = kit::chip(ui, r, icons::STACK, false, &label, Tone::Warn).on_hover_text(
        "Gleiches Motiv mit gleicher Aufnahmezeit (oder praktisch identisch) wurde schon behalten.\nRechte Maustaste halten: groß ansehen",
    );
    // kleines Vorschaubild rechts im Chip
    let tr = Rect::from_center_size(
        pos2(r.right() - sp::S - th / 2.0, r.center().y),
        vec2(th, th),
    );
    if let Some((id, size)) = app.thumb(&other) {
        super::image_cover(ui, tr, radius::S, id, size, Color32::WHITE);
    }
    let title = format!("Schon behalten: {}", folders::file_name(&other));
    if kit::peek_pressed(ui, &resp) {
        app.peek(vec![out.clone()], 0, title);
    } else if resp.clicked() {
        use crate::ui::viewer::{Mode, Source, Viewer};
        let mut v = Viewer::new(vec![Source::File(out)], 0, Mode::Frame, title);
        v.frame = app.frame_res();
        app.viewer = Some(v);
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

fn card(app: &mut SwiperApp, ui: &mut Ui, area: Rect, now: f64) -> Option<Rect> {
    let resp = ui.allocate_rect(area, Sense::click_and_drag());
    let path = app.current().cloned()?;
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
        return None;
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

    let quick = app.quick_sort();
    // Schnell sortieren: nur ansehen, Mausrad und zwei Finger zoomen ans Bild heran,
    // gezoomt verschiebt Ziehen den Ausschnitt, Doppelklick zeigt wieder alles.
    let view_id = Id::unique("quick_view");
    let mut view: Rect = ui
        .ctx()
        .data(|d| d.get_temp::<(PathBuf, Rect)>(view_id))
        .filter(|(p, _)| *p == path)
        .map(|(_, r)| r)
        .unwrap_or(FULL_UV);
    let zoomed = quick && !fill && view.width() < 0.999;
    if quick && !fill && app.anim.is_none() && app.dx.abs() < 0.5 {
        if resp.hovered() {
            let (scroll, pinch, hover) = ui.input(|i| {
                (
                    i.smooth_scroll_delta.y,
                    i.zoom_delta(),
                    i.pointer.hover_pos(),
                )
            });
            let f = (scroll * 0.003).exp() * pinch;
            if (f - 1.0).abs() > 1e-4 {
                let at = hover.unwrap_or(card.center());
                let rel = ((at - card.min) / card.size()).clamp(vec2(0.0, 0.0), vec2(1.0, 1.0));
                let p_uv = view.min + rel * view.size();
                let w = (view.width() / f).clamp(1.0 / 8.0, 1.0);
                let min = p_uv - rel * w;
                view = Rect::from_min_size(min, vec2(w, w));
            }
        }
        if zoomed && resp.dragged_by(PointerButton::Primary) {
            let d = resp.drag_delta() / card.size() * view.size();
            view = view.translate(-d);
        }
        if resp.double_clicked() {
            view = FULL_UV;
        }
        // im Bild bleiben
        let w = view.width();
        let min = pos2(
            view.min.x.clamp(0.0, 1.0 - w),
            view.min.y.clamp(0.0, 1.0 - w),
        );
        view = Rect::from_min_size(min, vec2(w, w));
        ui.ctx()
            .data_mut(|d| d.insert_temp(view_id, (path.clone(), view)));
    }
    if !zoomed {
        handle_card_input(app, ui, &resp, card, size, ratio, fill, s, &mut edit);
    } else if resp.hovered() {
        ui.ctx().set_cursor_icon(if resp.dragged() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::Grab
        });
    }
    let quick_uv = if quick && !fill { view } else { FULL_UV };
    let (x, y, cw, ch) = if fill {
        imaging::fill_rect(size.x, size.y, ratio, &mut edit)
    } else {
        imaging::crop_rect(size.x, size.y, ratio, &mut edit)
    };
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
        let bg = app.fill_bg_of(&edit);
        let bg_id = app.fill_bg_texture(ui.ctx(), ratio, bg).unwrap_or(tex_id);
        kit::rotated_image(
            &painter,
            RectShape::filled(card, rad, Color32::WHITE).with_texture(bg_id, FULL_UV),
            angle,
            pivot,
        );
        // Foto an seiner Stelle; was über den Rand ragt, wird abgeschnitten
        let pr = Rect::from_min_size(
            card.min + vec2(x * card.width(), y * card.height()),
            vec2(cw * card.width(), ch * card.height()),
        );
        let vis = pr.intersect(card);
        if vis.is_positive() {
            let uv = Rect::from_min_max(
                pos2(
                    (vis.left() - pr.left()) / pr.width(),
                    (vis.top() - pr.top()) / pr.height(),
                ),
                pos2(
                    (vis.right() - pr.left()) / pr.width(),
                    (vis.bottom() - pr.top()) / pr.height(),
                ),
            );
            let near = |a: f32, b: f32| (a - b).abs() < 0.75;
            let crad = if vis == card {
                CornerRadius::same(rad)
            } else {
                kit::corners(
                    near(vis.left(), card.left()) && near(vis.top(), card.top()),
                    near(vis.right(), card.right()) && near(vis.top(), card.top()),
                    near(vis.left(), card.left()) && near(vis.bottom(), card.bottom()),
                    near(vis.right(), card.right()) && near(vis.bottom(), card.bottom()),
                    rad,
                )
            };
            if edit.angle.abs() < 0.01 {
                kit::rotated_image(
                    &painter,
                    RectShape::filled(vis, crad, Color32::WHITE).with_texture(tex_id, uv),
                    angle,
                    pivot,
                );
            } else {
                straightened(
                    &painter, vis, crad, pr, tex_id, size, edit.angle, angle, pivot,
                );
            }
        }
        card
    } else {
        if edit.angle.abs() < 0.01 || quick_uv != FULL_UV {
            kit::rotated_image(
                &painter,
                RectShape::filled(card, rad, Color32::WHITE).with_texture(tex_id, quick_uv),
                angle,
                pivot,
            );
        } else {
            straightened(
                &painter,
                card,
                CornerRadius::same(rad),
                card,
                tex_id,
                size,
                edit.angle,
                angle,
                pivot,
            );
        }
        let cr = Rect::from_min_size(card.min + vec2(x * s, y * s), vec2(cw * s, ch * s));
        if !quick {
            crop_frame(&painter, card, cr, angle, pivot);
        }
        cr
    };
    if !quick {
        guides(&painter, out_r, app.cfg.guide, app.cfg.guide_flip, &tr);
    }
    if !quick && app.dx.abs() < 0.5 && app.anim.is_none() && app.horizon_line.is_none() {
        let target = grip_rect(card, size, ratio, fill, s, &edit);
        let hover = ui.input(|i| i.pointer.hover_pos());
        // bei „Ganz“ nur zeigen, solange die Maus über der Karte ist
        if !fill || hover.is_some_and(|h| card.contains(h)) {
            grip_handles(&painter, target, hover);
        }
    }
    // Horizont-Linie, die gerade mit Strg gezogen wird
    if let Some((a, b)) = app.horizon_line {
        let st = Stroke::new(2.0, Color32::from_rgb(0xFF, 0xD2, 0x4A));
        painter.line_segment([a, b], st);
        painter.circle_filled(a, 4.0, st.color);
        painter.circle_filled(b, 4.0, st.color);
    }
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
    let text_on_top = overlay.as_ref().map(|o| o.look.row() == 0).unwrap_or(false);
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
            format!("Ganzes Foto · {}", app.fill_bg_of(&edit).label())
        } else {
            format!("Zuschnitt {}", app.ratio_label(orient))
        };
        if edit.angle.abs() >= 0.05 {
            right = format!("{right}  ·  {:+.1}°", edit.angle).replace('.', ",");
        }
        if !edit.tone.is_neutral() {
            right = format!("{right}  ·  Licht");
        }
        if (edit.zoom - 1.0).abs() > 0.001 {
            right = format!("{right}  ·  {:.0} %", edit.zoom * 100.0);
        }
        if !quick {
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
    }
    stamp(&painter, app.dx, card, angle, &tr);
    Some(card)
}

/// Was gerade mit der Maus am Foto gezogen wird.
#[derive(Clone, Copy, PartialEq)]
enum Grip {
    /// Foto wegwischen (außerhalb des Rahmens oder beim schnellen Sortieren)
    Swipe,
    /// Ausschnitt (bzw. bei „Ganz“ das Foto) verschieben
    Move,
    /// an einer Ecke größer oder kleiner ziehen (0 links oben, 1 rechts oben,
    /// 2 links unten, 3 rechts unten); die gegenüberliegende Ecke bleibt stehen
    Corner(u8),
}

/// Wie nah (in Punkten) man an eine Ecke kommen muss, um sie zu greifen.
const GRIP_REACH: f32 = 18.0;

/// Fläche, die man zieht: der helle Ausschnitt, bei „Ganz“ das Foto selbst.
fn grip_rect(
    card: Rect,
    size: egui::Vec2,
    ratio: f32,
    fill: bool,
    s: f32,
    edit: &imaging::Edit,
) -> Rect {
    let mut e = *edit;
    if fill {
        let (x, y, w, h) = imaging::fill_rect(size.x, size.y, ratio, &mut e);
        Rect::from_min_size(
            card.min + vec2(x * card.width(), y * card.height()),
            vec2(w * card.width(), h * card.height()),
        )
    } else {
        let (x, y, w, h) = imaging::crop_rect(size.x, size.y, ratio, &mut e);
        Rect::from_min_size(card.min + vec2(x * s, y * s), vec2(w * s, h * s))
    }
}

fn corners_of(r: Rect) -> [Pos2; 4] {
    [
        r.left_top(),
        r.right_top(),
        r.left_bottom(),
        r.right_bottom(),
    ]
}

fn corner_at(r: Rect, p: Pos2) -> Option<u8> {
    corners_of(r)
        .iter()
        .position(|c| c.distance(p) <= GRIP_REACH)
        .map(|i| i as u8)
}

/// Maus und Touchpad an der Karte: Ziehen außerhalb des hellen Rahmens (beim schnellen
/// Sortieren überall) wischt das Foto weg: nach rechts behalten, nach links weg.
/// Linksziehen im Rahmen verschiebt den Ausschnitt,
/// Ecken ziehen ändert die Größe, Mausrad und Zwei-Finger-Zoom zoomen,
/// Strg + Ziehen legt eine Horizont-Linie zum Begradigen.
#[allow(clippy::too_many_arguments)]
fn handle_card_input(
    app: &mut SwiperApp,
    ui: &Ui,
    resp: &egui::Response,
    card: Rect,
    size: egui::Vec2,
    ratio: f32,
    fill: bool,
    s: f32,
    edit: &mut imaging::Edit,
) {
    let grip_id = Id::unique("card_grip");
    let quick = app.quick_sort();
    if let Some(Grip::Swipe) = ui.ctx().data(|d| d.get_temp::<Grip>(grip_id)) {
        if resp.dragged() {
            let d = resp.drag_delta();
            app.dx += d.x;
            app.dy += d.y * 0.3;
        }
        if resp.drag_stopped() || !resp.dragged() {
            ui.ctx().data_mut(|d| d.remove::<Grip>(grip_id));
            let now = ui.input(|i| i.time);
            let width = card.width();
            let thr = (width * 0.22).clamp(60.0, 180.0);
            if app.dx > thr {
                app.decide(true, now, width);
            } else if app.dx < -thr {
                app.decide(false, now, width);
            } else {
                app.anim = Some(crate::app::Anim {
                    from: app.dx,
                    to: 0.0,
                    start: now,
                    dur: 0.18,
                    commit: None,
                });
            }
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        return;
    }
    if app.anim.is_some() || app.dx.abs() > 0.5 {
        return;
    }
    let target = grip_rect(card, size, ratio, fill, s, edit);
    let ctrl = ui.input(|i| i.modifiers.command);
    let primary = resp.drag_started_by(PointerButton::Primary);
    if quick {
        if primary || resp.drag_started_by(PointerButton::Secondary) {
            ui.ctx().data_mut(|d| d.insert_temp(grip_id, Grip::Swipe));
        } else if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        return;
    }
    if primary && !ctrl {
        let origin = ui.input(|i| i.pointer.press_origin());
        if origin.is_some_and(|o| !target.contains(o) && corner_at(target, o).is_none()) {
            ui.ctx().data_mut(|d| d.insert_temp(grip_id, Grip::Swipe));
            return;
        }
    }
    if primary && ctrl {
        if let Some(p) = resp.interact_pointer_pos() {
            app.horizon_line = Some((p, p));
        }
    } else if primary || resp.drag_started_by(PointerButton::Secondary) {
        let origin = ui
            .input(|i| i.pointer.press_origin())
            .unwrap_or(card.center());
        let grip = match corner_at(target, origin) {
            Some(c) if primary => Grip::Corner(c),
            _ => Grip::Move,
        };
        ui.ctx().data_mut(|d| d.insert_temp(grip_id, grip));
    }
    if let Some((a, _)) = app.horizon_line {
        if resp.dragged_by(PointerButton::Primary) {
            if let Some(p) = resp.interact_pointer_pos() {
                app.horizon_line = Some((a, p));
            }
        }
        if resp.drag_stopped() {
            if let Some((a, b)) = app.horizon_line.take() {
                let d = b - a;
                if d.length() > 30.0 {
                    // fast waagrecht: an der Waagrechten ausrichten, sonst an der Senkrechten
                    let phi = if d.x.abs() >= d.y.abs() {
                        (d.y / d.x).atan()
                    } else {
                        -(d.x / d.y).atan()
                    };
                    edit.angle = (edit.angle - phi.to_degrees())
                        .clamp(-imaging::MAX_ANGLE, imaging::MAX_ANGLE);
                }
            }
        }
        return;
    }
    let grip: Option<Grip> = ui.ctx().data(|d| d.get_temp(grip_id));
    let dragging =
        resp.dragged_by(PointerButton::Primary) || resp.dragged_by(PointerButton::Secondary);
    match grip {
        Some(Grip::Move) if dragging => {
            let d = resp.drag_delta();
            edit.cx += d.x / card.width();
            edit.cy += d.y / card.height();
        }
        Some(Grip::Corner(c)) if dragging => {
            if let Some(p) = resp.interact_pointer_pos() {
                // gegenüberliegende Ecke bleibt stehen, Seitenverhältnis bleibt gleich
                let anchor = corners_of(target)[3 - c as usize];
                let aspect = target.width() / target.height().max(1.0);
                let dx = (p.x - anchor.x).abs();
                let dy = (p.y - anchor.y).abs();
                let w = dx.max(dy * aspect).max(24.0);
                let h = w / aspect;
                let sx = if c % 2 == 1 { 1.0 } else { -1.0 };
                let sy = if c >= 2 { 1.0 } else { -1.0 };
                let center = anchor + vec2(sx * w / 2.0, sy * h / 2.0);
                if fill {
                    edit.zoom *= w / target.width();
                } else {
                    edit.zoom *= target.width() / w;
                }
                edit.cx = (center.x - card.left()) / card.width();
                edit.cy = (center.y - card.top()) / card.height();
            }
        }
        _ => {}
    }
    if resp.drag_stopped() {
        ui.ctx().data_mut(|d| d.remove::<Grip>(grip_id));
    }
    if resp.double_clicked() {
        edit.reset_crop();
    }
    if resp.hovered() {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        edit.zoom *= (scroll * 0.003).exp() * zoom;
        let hover = ui.input(|i| i.pointer.hover_pos());
        let cursor = match (grip, hover) {
            (Some(Grip::Corner(c)), _) => corner_cursor(c),
            (Some(Grip::Move), _) => egui::CursorIcon::Grabbing,
            (None, Some(h)) if ctrl => {
                let _ = h;
                egui::CursorIcon::Crosshair
            }
            (None, Some(h)) => match corner_at(target, h) {
                Some(c) => corner_cursor(c),
                None => egui::CursorIcon::Grab,
            },
            _ => egui::CursorIcon::Default,
        };
        ui.ctx().set_cursor_icon(cursor);
    }
}

fn corner_cursor(c: u8) -> egui::CursorIcon {
    if c == 0 || c == 3 {
        egui::CursorIcon::ResizeNwSe
    } else {
        egui::CursorIcon::ResizeNeSw
    }
}

/// Griffe an den Ecken der Fläche, die man ziehen kann.
fn grip_handles(painter: &Painter, r: Rect, hover: Option<Pos2>) {
    let len = 18.0_f32.min(r.width() / 3.0).min(r.height() / 3.0);
    for (i, c) in corners_of(r).iter().enumerate() {
        let sx = if i % 2 == 1 { -1.0 } else { 1.0 };
        let sy = if i >= 2 { -1.0 } else { 1.0 };
        let hot = hover.is_some_and(|h| h.distance(*c) <= GRIP_REACH);
        let st = Stroke::new(if hot { 5.0 } else { 4.0 }, Color32::WHITE);
        let shadow = Stroke::new(st.width + 2.0, Color32::from_black_alpha(70));
        let pts = [*c + vec2(0.0, sy * len), *c, *c + vec2(sx * len, 0.0)];
        painter.add(Shape::line(pts.to_vec(), shadow));
        painter.add(Shape::line(pts.to_vec(), st));
    }
}

/// Foto um `deg` gedreht und vergrößert (wie beim Speichern) in `clip` malen.
/// Die Fläche bleibt ein abgerundetes Rechteck; gedreht werden nur die Bildkoordinaten.
#[allow(clippy::too_many_arguments)]
fn straightened(
    painter: &Painter,
    clip: Rect,
    crad: CornerRadius,
    photo: Rect,
    tex: egui::TextureId,
    size: egui::Vec2,
    deg: f32,
    swipe_angle: f32,
    pivot: Pos2,
) {
    let k = imaging::straighten_scale(size.x, size.y, deg);
    let back = Rot2::from_angle(-deg.to_radians());
    let c = photo.center();
    let ctx = painter.ctx();
    let mut tess = egui::epaint::Tessellator::new(
        ctx.pixels_per_point(),
        ctx.tessellation_options(|o| *o),
        [1, 1],
        Vec::new(),
    );
    let mut mesh = egui::epaint::Mesh::default();
    tess.tessellate_rect(
        &RectShape::filled(clip, crad, Color32::WHITE).with_texture(tex, FULL_UV),
        &mut mesh,
    );
    for v in &mut mesh.vertices {
        // Bildschirmpunkt -> Punkt im ungedrehten Foto
        let q = c + back * (v.pos - c) / k;
        v.uv = pos2(
            (q.x - photo.left()) / photo.width(),
            (q.y - photo.top()) / photo.height(),
        );
    }
    mesh.rotate(Rot2::from_angle(swipe_angle), pivot);
    painter.add(Shape::mesh(mesh));
}

/// Abgedunkelte Ränder außerhalb des Ausschnitts, Drittel-Linien und heller Rahmen.
fn crop_frame(painter: &Painter, card: Rect, cr: Rect, angle: f32, pivot: Pos2) {
    let dark = Color32::from_black_alpha(175);
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
fn guides(painter: &Painter, r: Rect, kind: u8, flip: u8, tr: &dyn Fn(Pos2) -> Pos2) {
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

/// Beim Ziehen: leuchtender Kartenrand (grün = behalten, rot = weg) und
/// ein kleines rundes Symbol in der oberen Ecke.
fn stamp(painter: &Painter, dx: f32, card: Rect, angle: f32, tr: &dyn Fn(Pos2) -> Pos2) {
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
fn preview_overlay(
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

// ------------------------------------------------------------------ Filmstreifen

/// Zustand des Bilderbands: Mitte des sichtbaren Teils (in Punkten ab dem ersten
/// Foto), ob es dem aktuellen Foto folgt, und für welches Foto zuletzt.
#[derive(Clone, Copy, Default)]
struct StripState {
    offset: f32,
    follow: bool,
    pos: usize,
}

fn filmstrip(app: &mut SwiperApp, ui: &mut Ui, area: Rect) {
    let n = app.items.len();
    if n == 0 {
        return;
    }
    let (w, h) = (64.0, 48.0);
    let gap = sp::S;
    let pitch = w + gap;
    let content = n as f32 * pitch - gap;
    let max_w = area.width().min(760.0);
    let inner_w = content.min(max_w);
    let pill = Rect::from_center_size(area.center(), vec2(inner_w + sp::S * 2.0, h + sp::S * 2.0));
    kit::glass(ui.painter(), pill, radius::L);
    let inner = pill.shrink(sp::S);

    let id = Id::unique("filmstrip");
    let mut st: StripState = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(StripState {
        offset: app.pos as f32 * pitch,
        follow: true,
        pos: app.pos,
    });
    if st.pos != app.pos {
        st.pos = app.pos;
        st.follow = true;
    }
    let resp = ui.interact(pill, id.with("drag"), Sense::click_and_drag());
    if resp.dragged() {
        st.offset -= resp.drag_delta().x;
        st.follow = false;
    }
    if resp.hovered() {
        let d = ui.input(|i| i.smooth_scroll_delta);
        let d = if d.x.abs() > d.y.abs() { d.x } else { d.y };
        if d != 0.0 {
            st.offset -= d;
            st.follow = false;
        }
    }
    if st.follow {
        let target = app.pos as f32 * pitch;
        let diff = target - st.offset;
        if diff.abs() < 0.5 {
            st.offset = target;
        } else {
            st.offset += diff * 0.25;
            ui.ctx().request_repaint();
        }
    }
    // Ränder: das erste und letzte Foto dürfen nicht weiter als bis zum Rand wandern
    let half = inner.width() / 2.0;
    let lo = (half - w / 2.0).min(content / 2.0 - w / 2.0);
    let hi = (content - half - w / 2.0).max(lo);
    st.offset = st.offset.clamp(lo, hi);
    ui.ctx().data_mut(|d| d.insert_temp(id, st));

    // Bildschirm-x der Mitte von Foto i
    let x_of = |i: usize| inner.center().x + i as f32 * pitch - st.offset;
    let index_at = |x: f32| -> Option<usize> {
        let f = (x - inner.center().x + st.offset) / pitch;
        let i = f.round();
        (i >= 0.0 && (i as usize) < n && (f - i).abs() <= 0.5).then_some(i as usize)
    };
    let painter = ui.painter_at(inner);
    let hover = resp
        .hover_pos()
        .filter(|_| !resp.dragged())
        .and_then(|p| index_at(p.x));
    let first = index_at(inner.left()).unwrap_or(0).saturating_sub(1);
    let last = (index_at(inner.right()).unwrap_or(n - 1) + 2).min(n);
    for i in first..last {
        let cx = x_of(i);
        let r = Rect::from_center_size(pos2(cx, inner.center().y), vec2(w, h));
        if !r.intersects(inner) {
            continue;
        }
        let is_cur = i == app.pos;
        let p = app.items[i].clone();
        let entry = app.decided(&p).map(|e| e.d == "keep");
        let alpha = match (is_cur, hover == Some(i), entry) {
            (true, _, _) => 1.0,
            (_, true, _) => 0.95,
            (_, _, Some(false)) => 0.4,
            _ => 0.7,
        };
        let rad = radius::S;
        match app.thumb(&p) {
            Some((tid, size)) => {
                painter.add(
                    RectShape::filled(r, rad, Color32::WHITE.gamma_multiply(alpha))
                        .with_texture(tid, cover_uv(size, r.size())),
                );
            }
            None => {
                painter.rect_filled(r, rad, field());
            }
        }
        if is_cur {
            painter.rect_stroke(r, rad, Stroke::new(2.5, accent()), StrokeKind::Inside);
        }
        // Punkt: grün behalten, rot weg
        if let Some(k) = entry {
            let c = pos2(r.right() - 8.0, r.bottom() - 8.0);
            painter.circle_filled(c, 6.0, Color32::from_black_alpha(90));
            painter.circle(
                c,
                4.5,
                if k { GREEN } else { RED },
                Stroke::new(1.5, Color32::WHITE),
            );
        }
    }
    if let Some(i) = hover {
        let tip = match app.decided(&app.items[i]).map(|e| e.d == "keep") {
            Some(true) => "Behalten  ·  Klick: nochmal entscheiden",
            Some(false) => "Weg  ·  Klick: nochmal entscheiden",
            None => "Klick: zu diesem Foto springen",
        };
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        if i != app.pos {
            resp.clone().on_hover_text(tip);
        }
    } else if resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    if resp.clicked() {
        if let Some(i) = resp.interact_pointer_pos().and_then(|p| index_at(p.x)) {
            if i != app.pos {
                app.jump_to(i);
            }
        }
    }
    if kit::peek_pressed(ui, &resp) {
        if let Some(i) = resp.hover_pos().and_then(|p| index_at(p.x)) {
            let title = app
                .folder
                .as_ref()
                .and_then(|f| app.folders.iter().find(|i| &i.path == f))
                .map(|f| f.label.clone())
                .unwrap_or_default();
            app.peek(app.items.clone(), i, title);
        }
    }
}

// ------------------------------------------------------------------ schwebende Leiste

#[derive(Clone, Copy)]
enum Act {
    Undo,
    Skip,
    Keep,
    Rotate,
    Back,
    Orient,
    Text,
    TextPanel,
    FillPanel,
    AdjustPanel,
}

/// Zeichnet die Leiste und gibt ihre Fläche zurück.
fn dock_ui(app: &mut SwiperApp, ui: &mut Ui, area: Rect, now: f64, width: f32) -> Rect {
    let loaded = matches!(app.preview(), Some(Ok(_)));
    let has_current = app.current().is_some();
    let k = |a: &str| app.key_label(a);
    let ks = |a: &str| app.key_short(a);
    let pad = sp::M;
    let tool = size::TOOL;
    let caret = 24.0;
    let (skip_w, keep_w) = (112.0, 112.0);
    // Schnell sortieren: nur Zurück, Weg, Behalten und Drehen
    let quick = app.quick_sort();
    let polish = app.polish_left();
    let tools_w = if quick {
        tool
    } else {
        tool * 3.0 + caret * 3.0 + sp::XS * 2.0
    };
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

    if let Some((i, n)) = polish {
        // Feinschliff: links Weg, in der Mitte Zurück und Vor
        let r = row(x, tool);
        if kit::tool_button(
            ui,
            r,
            kit::Tool {
                glyph: icons::X,
                label: "Weg",
                key: &ks("undo"),
                active: false,
                enabled: has_current,
            },
        )
        .on_hover_text(format!(
            "Doch nicht für den Rahmen: aus den Behaltenen nehmen ({})",
            k("undo")
        ))
        .clicked()
        {
            clicked = Some(Act::Skip);
        }
        x = r.right() + pad;
        divider_at(x);
        x += 1.0 + pad;
        let r = row(x, skip_w);
        if kit::nav_button(ui, r, icons::ARROW_LEFT, &app.key_changed("skip"), i > 1)
            .on_hover_text(format!("Vorheriges Foto ({})", k("skip")))
            .clicked()
        {
            clicked = Some(Act::Back);
        }
        x = r.right() + sp::S;
        let r = row(x, keep_w);
        if kit::nav_button(ui, r, icons::ARROW_RIGHT, &app.key_changed("keep"), loaded)
            .on_hover_text(if i < n {
                format!("Speichern und nächstes Foto ({})", k("keep"))
            } else {
                format!("Speichern und Feinschliff beenden ({})", k("keep"))
            })
            .clicked()
        {
            clicked = Some(Act::Keep);
        }
    } else {
        let undo_key = ks("undo");
        let r = row(x, tool);
        if kit::tool_button(
            ui,
            r,
            kit::Tool {
                glyph: icons::UNDO,
                label: "Zurück",
                key: &undo_key,
                active: false,
                enabled: !app.status.verlauf.is_empty(),
            },
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
        if kit::primary_button(
            ui,
            r,
            Primary::Skip,
            "",
            &app.key_changed("skip"),
            has_current,
        )
        .on_hover_text(format!("Weg: aussortieren ({})", k("skip")))
        .clicked()
        {
            clicked = Some(Act::Skip);
        }
        x = r.right() + sp::S;
        let r = row(x, keep_w);
        if kit::primary_button(ui, r, Primary::Keep, "", &app.key_changed("keep"), loaded)
            .on_hover_text(if app.reedit.is_some() {
                format!("Neu speichern ({})", k("keep"))
            } else {
                format!("Behalten: für den Rahmen speichern ({})", k("keep"))
            })
            .clicked()
        {
            clicked = Some(Act::Keep);
        }
    }
    x += keep_w + pad;
    divider_at(x);
    x += 1.0 + pad;

    if quick {
        let key = ks("rot_r");
        if kit::tool_button(
            ui,
            row(x, tool),
            kit::Tool {
                glyph: icons::ROTATE,
                label: "Drehen",
                key: &key,
                active: false,
                enabled: loaded,
            },
        )
        .on_hover_text(format!("Drehen ({})", k("rot_r")))
        .clicked()
        {
            clicked = Some(Act::Rotate);
        }
        match clicked {
            Some(Act::Undo) => app.undo(),
            Some(Act::Skip) => app.decide(false, now, width),
            Some(Act::Keep) => app.decide(true, now, width),
            Some(Act::Rotate) => app.rotate(1),
            _ => {}
        }
        return dock;
    }

    // Drehen: Klick dreht um 90°, der Pfeil öffnet Begradigen und Licht
    let adjusted = app
        .current()
        .and_then(|p| app.edits.get(p))
        .is_some_and(|e| e.angle.abs() >= 0.05 || !e.tone.is_neutral());
    let r = row(x, tool + caret);
    let rot_key = ks("rot_r");
    let (main, arrow) = kit::split_tool(
        ui,
        r,
        caret,
        kit::Tool {
            glyph: icons::ROTATE,
            label: "Drehen",
            key: &rot_key,
            active: adjusted,
            enabled: loaded,
        },
        app.adjust_panel,
    );
    if main
        .on_hover_text(format!(
            "Drehen ({}, andersherum {})",
            k("rot_r"),
            k("rot_l")
        ))
        .clicked()
    {
        clicked = Some(Act::Rotate);
    }
    if arrow.on_hover_text("Begradigen und Licht").clicked() {
        clicked = Some(Act::AdjustPanel);
    }
    let adjust_anchor = r;
    x += tool + caret + sp::XS;

    // Format: Klick wechselt Quer/Hochkant, der Pfeil öffnet Quer, Hochkant, Ganz und Rand
    let r = row(x, tool + caret);
    let orient_key = ks("orient");
    let (main, arrow) = kit::split_tool(
        ui,
        r,
        caret,
        kit::Tool {
            glyph: if app.current_fill() {
                icons::FILL
            } else {
                icons::SWAP
            },
            label: if app.current_fill() { "Ganz" } else { "Format" },
            key: &orient_key,
            active: app.current_fill(),
            enabled: loaded,
        },
        app.fill_panel,
    );
    if main
        .on_hover_text(format!(
            "Quer / Hochkant wechseln ({}). Ganzes Foto: {}",
            k("orient"),
            k("fill")
        ))
        .clicked()
    {
        clicked = Some(Act::Orient);
    }
    if arrow
        .on_hover_text("Quer, Hochkant, Ganz und Rand")
        .clicked()
    {
        clicked = Some(Act::FillPanel);
    }
    let fill_anchor = r;
    x += tool + caret + sp::XS;

    // Text: Klick schaltet an/aus, der Pfeil daneben öffnet das Feld
    let text_on = app.cfg.text.enabled;
    let r = row(x, tool + caret);
    let text_key = ks("text");
    let (main, arrow) = kit::split_tool(
        ui,
        r,
        caret,
        kit::Tool {
            glyph: icons::TEXT,
            label: "Text",
            key: &text_key,
            active: text_on,
            enabled: loaded,
        },
        app.text_panel,
    );
    if main
        .on_hover_text(format!(
            "Datum, Ort oder Notiz aufs Bild: an / aus ({})",
            k("text")
        ))
        .clicked()
    {
        clicked = Some(Act::Text);
    }
    if arrow
        .on_hover_text("Ort, Notiz und Text-Stil einstellen")
        .clicked()
    {
        clicked = Some(Act::TextPanel);
    }

    match clicked {
        Some(Act::Undo) => app.undo(),
        Some(Act::Skip) => app.decide(false, now, width),
        Some(Act::Keep) => app.decide(true, now, width),
        Some(Act::Back) => app.polish_back(),
        Some(Act::Rotate) => app.rotate(1),
        Some(Act::Orient) => app.toggle_orientation(),
        Some(Act::Text) => toggle_text(app),
        Some(Act::TextPanel) => toggle_text_panel(app),
        Some(Act::FillPanel) => {
            app.fill_panel = !app.fill_panel;
            app.adjust_panel = false;
        }
        Some(Act::AdjustPanel) => {
            app.adjust_panel = !app.adjust_panel;
            app.fill_panel = false;
        }
        None => {}
    }
    if app.fill_panel && app.dialog.is_none() {
        dialogs::fill_panel(app, ui, fill_anchor);
    }
    if app.adjust_panel && app.dialog.is_none() {
        dialogs::adjust_panel(app, ui, adjust_anchor);
    }
    dock
}
